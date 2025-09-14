use std::{
    collections::VecDeque,
    future::Future,
    marker::PhantomData,
    net::{IpAddr, ToSocketAddrs},
    sync::Arc,
    time::{Duration, Instant},
};

use anyhow::Result;
use foldhash::HashMap;
use let_engine_core::backend::networking::ServerInterface as CoreServerInterface;
use rkyv::{
    Serialize,
    api::high::HighSerializer,
    rancor,
    ser::allocator::{Arena, ArenaHandle},
    util::AlignedVec,
};
use smol::{
    channel::Sender,
    io::{AsyncReadExt, AsyncWriteExt},
    lock::Mutex,
    net::{SocketAddr, TcpListener, TcpStream, UdpSocket},
};
use thiserror::Error;

use super::{Connection, Disconnected, NetworkingSettings, SAFE_MTU_SIZE, Warning};

#[derive(Clone)]
struct Peer {
    tcp_stream: TcpStream,
    order_number: u32,
    ping_timestamp: Option<Instant>,
    ping: Duration,
    settings: Arc<NetworkingSettings>,

    last_package: Instant,
    last_package_durations: VecDeque<Duration>,
    rate_average: Duration,
}

impl Peer {
    pub fn new(tcp_stream: TcpStream, settings: Arc<NetworkingSettings>) -> Self {
        let mut last_package_durations = VecDeque::with_capacity(10);
        last_package_durations.extend([Duration::from_secs(600); 10]);
        Self {
            tcp_stream,
            order_number: 1,
            ping_timestamp: None,
            ping: Duration::default(),
            settings,

            last_package: Instant::now(),
            last_package_durations,
            rate_average: Duration::MAX,
        }
    }

    pub fn order_number(&mut self) -> u32 {
        self.order_number += 1;
        self.order_number
    }

    pub fn record_rate(&mut self) {
        self.last_package_durations
            .push_back(self.last_package.elapsed());
        self.last_package_durations.pop_front();
        self.last_package = Instant::now();

        self.rate_average = self
            .last_package_durations
            .iter()
            .fold(Duration::ZERO, |acc, &x| acc + x)
            / 10;
    }

    pub fn over_rate_limit(&self) -> bool {
        self.rate_average < self.settings.rate_limit
    }
}

struct Socket {
    udp_socket: UdpSocket,
    local_addr: SocketAddr,

    peers: HashMap<Connection, Peer>,
    /// Both TCP and UDP lead to the same Connection
    pending: HashMap<[u8; 128], (TcpStream, SocketAddr)>,
}

impl Socket {
    /// Records the time and stops the echoing.
    ///
    /// Returns true if ping is over ping limit.
    ///
    /// Connection must be validated.
    async fn ping(
        &mut self,
        max_ping: Duration,
        connection: Connection,
    ) -> Result<bool, std::io::Error> {
        let peer = self.peers.get_mut(&connection).unwrap();

        let time = std::mem::take(&mut peer.ping_timestamp);

        if let Some(time) = time {
            peer.ping = time.elapsed();
            Ok(peer.ping > max_ping)
        } else {
            // send 8 byte message to be echoed
            peer.ping_timestamp = Some(Instant::now());
            self.udp_socket
                .send_to(&[0; 8], connection.udp_addr())
                .await?;
            Ok(false)
        }
    }

    fn connection(&self, addr: &SocketAddr) -> Option<Connection> {
        for key in self.peers.keys() {
            if key.contains_addr(addr) {
                return Some(*key);
            }
        }
        None
    }
}

pub(super) enum ServerMessage {
    Connected,
    Disconnected(Disconnected),
    Warning(Warning),
    Tcp(Vec<u8>),
    Udp(Vec<u8>),
    Error(std::io::Error),
}

/// A server instance that allows you to send messages to your client.
pub struct ServerInterface<Msg> {
    socket: Arc<Mutex<Option<Socket>>>,
    messages: Sender<(Connection, ServerMessage)>,
    settings: Arc<NetworkingSettings>,
    arena: Arc<parking_lot::Mutex<Arena>>,
    _msg: PhantomData<Msg>,
}

impl<Msg> CoreServerInterface<Connection> for ServerInterface<Msg>
where
    Msg:
        Send + Sync + for<'a> Serialize<HighSerializer<AlignedVec, ArenaHandle<'a>, rancor::Error>>,
{
    type Msg = Msg;
    type Error = ServerError;

    fn start<Addr: ToSocketAddrs>(&self, addr: Addr) -> std::result::Result<(), ServerError> {
        let addr = addr
            .to_socket_addrs()
            .map_err(ServerError::Io)?
            .next()
            .unwrap();

        let mut socket = self.socket.lock_blocking();
        if socket.is_some() {
            return Err(ServerError::AlreadyRunning);
        }
        let (tcp_listener, udp_socket) = smol::block_on(async {
            (
                TcpListener::bind(addr).await.map_err(ServerError::Io),
                UdpSocket::bind(addr).await.map_err(ServerError::Io),
            )
        });

        *socket = Some(Socket {
            udp_socket: udp_socket?,
            local_addr: addr,
            peers: HashMap::default(),
            pending: HashMap::default(),
        });

        std::mem::drop(socket);

        self.recv_udp_messages();
        self.accept_connetions(tcp_listener?);

        Ok(())
    }

    fn stop(&self) -> std::result::Result<(), Self::Error> {
        smol::block_on(async {
            let Some(socket) = self.socket.lock().await.take() else {
                return Err(ServerError::NotRunning);
            };

            for connection in socket.peers.into_values() {
                connection
                    .tcp_stream
                    .shutdown(std::net::Shutdown::Both)
                    .map_err(ServerError::Io)?;
            }
            Ok(())
        })
    }

    fn local_addr(&self) -> Option<SocketAddr> {
        self.socket
            .lock_blocking()
            .as_ref()
            .map(|socket| socket.local_addr)
    }

    fn send(&self, conn: Connection, message: &Self::Msg) -> std::result::Result<(), Self::Error> {
        if !self.is_connected(&conn)? {
            return Err(ServerError::UserNotFound);
        }

        let data = {
            let mut arena = self.arena.lock();
            super::serialize_tcp_into(message, &mut arena)
        };

        let socket = self.socket.clone();

        self.spawn(async move {
            let mut socket = socket.lock().await;

            let peer = socket.as_mut().unwrap().peers.get_mut(&conn).unwrap();

            if let Err(e) = peer.tcp_stream.write_all(&data).await {
                return Err((conn, e));
            }
            Ok(None)
        });
        Ok(())
    }

    /// Sends a message to a specific target through UDP.
    ///
    /// This function should be used to send messages with the lowest latency possible.
    fn fast_send(
        &self,
        conn: Connection,
        message: &Self::Msg,
    ) -> std::result::Result<(), Self::Error> {
        if !self.is_connected(&conn)? {
            return Err(ServerError::UserNotFound);
        }

        let mut data = {
            let mut arena = self.arena.lock();
            super::serialize_udp_into(message, &mut arena)
        };

        let socket = self.socket.clone();

        self.spawn(async move {
            let mut socket = socket.lock().await;

            // Add order number to first four bytes
            {
                let socket = socket.as_mut().unwrap();
                let peer = socket.peers.get_mut(&conn).unwrap();
                data[0..4].copy_from_slice(&(peer.order_number() as u32).to_le_bytes());
            }

            let chunks = data.chunks(SAFE_MTU_SIZE);

            for chunk in chunks {
                socket
                    .as_ref()
                    .unwrap()
                    .udp_socket
                    .send_to(chunk, conn.udp_addr())
                    .await
                    .map_err(|e| (conn, e))?;
            }
            Ok(None)
        });

        Ok(())
    }

    fn broadcast(&self, message: &Self::Msg) -> std::result::Result<(), Self::Error> {
        let messages = self.messages.clone();

        let data = {
            let mut arena = self.arena.lock();
            super::serialize_tcp_into(message, &mut arena)
        };

        let socket = self.socket.clone();

        smol::spawn(async move {
            let mut socket = socket.lock().await;

            for (connection, peer) in socket.as_mut().unwrap().peers.iter_mut() {
                if let Err(e) = peer.tcp_stream.write_all(&data).await {
                    messages
                        .send((*connection, ServerMessage::Error(e)))
                        .await
                        .unwrap();
                }
            }
        })
        .detach();
        Ok(())
    }

    /// Broadcasts a message to every client through UDP.
    ///
    /// This function should be used to broadcast messages with the lowest latency possible.
    fn fast_broadcast(&self, message: &Self::Msg) -> std::result::Result<(), Self::Error> {
        let messages = self.messages.clone();

        let mut data = {
            let mut arena = self.arena.lock();
            super::serialize_udp_into(message, &mut arena)
        };

        let socket = self.socket.clone();

        smol::spawn(async move {
            let mut socket = socket.lock().await;
            let udp_socket = socket.as_ref().unwrap().udp_socket.clone();

            for (connection, peer) in socket.as_mut().unwrap().peers.iter_mut() {
                data[0..4].copy_from_slice(&peer.order_number().to_le_bytes());

                let chunks = data.chunks(SAFE_MTU_SIZE);

                for chunk in chunks {
                    if let Err(e) = udp_socket.send_to(chunk, connection.udp_addr()).await {
                        messages
                            .send((*connection, ServerMessage::Error(e)))
                            .await
                            .unwrap();
                    }
                }
            }
        })
        .detach();
        Ok(())
    }

    fn disconnect(&self, conn: Connection) -> std::result::Result<(), Self::Error> {
        if !self.is_connected(&conn)? {
            return Err(ServerError::UserNotFound);
        }

        let mut socket = self.socket.lock_blocking();
        let socket_mut = socket.as_mut().unwrap();

        let peer = socket_mut.peers.remove(&conn).unwrap();

        peer.tcp_stream
            .shutdown(std::net::Shutdown::Both)
            .map_err(ServerError::Io)?;

        Ok(())
    }

    fn connections(&self) -> impl Iterator<Item = Connection> {
        let socket = self.socket.lock_blocking();

        socket
            .as_ref()
            .map(|socket| {
                let iter: Vec<Connection> = socket.peers.keys().copied().collect();
                iter.into_iter()
            })
            .unwrap_or(vec![].into_iter())
    }

    /// Returns true if the given connection is connected.
    ///
    /// Can only return a [`ServerError::NotRunning`] in case the server is not running.
    fn is_connected(&self, connection: &Connection) -> Result<bool, ServerError> {
        let socket = self.socket.lock_blocking();

        socket
            .as_ref()
            .map(|socket| Ok(socket.peers.contains_key(connection)))
            .unwrap_or(Err(ServerError::NotRunning))
    }
}

impl<Msg> Clone for ServerInterface<Msg> {
    fn clone(&self) -> Self {
        Self {
            socket: self.socket.clone(),
            messages: self.messages.clone(),
            settings: self.settings.clone(),
            arena: self.arena.clone(),
            _msg: PhantomData,
        }
    }
}

impl<Msg> ServerInterface<Msg>
where
    Msg:
        Send + Sync + for<'a> Serialize<HighSerializer<AlignedVec, ArenaHandle<'a>, rancor::Error>>,
{
    /// Creates a new server using the given address.
    pub(super) fn new(
        settings: NetworkingSettings,
        messages: Sender<(Connection, ServerMessage)>,
        arena: Arc<parking_lot::Mutex<Arena>>,
    ) -> Result<Self> {
        let settings = Arc::new(settings);

        let server = Self {
            socket: Arc::new(Mutex::new(None)),
            messages,
            settings,
            arena,
            _msg: PhantomData,
        };

        Ok(server)
    }

    fn spawn(
        &self,
        future: impl Future<
            Output = Result<Option<(Connection, ServerMessage)>, (Connection, std::io::Error)>,
        > + Send
        + 'static,
    ) {
        let sender = self.messages.clone();
        smol::spawn(async move {
            if let Some(message) = match future.await {
                Ok(t) => t,
                Err(e) => Some((e.0, ServerMessage::Error(e.1))),
            } {
                sender.send(message).await.unwrap()
            }
        })
        .detach();
    }

    fn accept_connetions(&self, listener: TcpListener) {
        let socket = self.socket.clone();
        let settings = self.settings.clone();
        smol::spawn(async move {
            while let Ok((mut stream, addr)) = listener.accept().await {
                let mut socket = socket.lock().await;
                let socket_mut = socket.as_mut().unwrap();

                if settings.max_connections <= socket_mut.peers.len() {
                    let _ = stream.shutdown(std::net::Shutdown::Both);
                    continue;
                }
                let mut buf = [0; 128];

                let op = stream.read_exact(&mut buf);

                use futures::future::Either;

                // Maximum time to respond as ping limit
                match futures::future::select(op, smol::Timer::after(settings.max_ping)).await {
                    Either::Left(result) => {
                        if result.0.is_err() {
                            return;
                        }
                    }
                    Either::Right(_) => return,
                };

                socket_mut.pending.insert(buf, (stream, addr));
            }
        })
        .detach();
    }

    async fn connect_client(
        messages: Sender<(Connection, ServerMessage)>,
        socket: Arc<Mutex<Option<Socket>>>,
        socket_mut: &mut Socket,
        stream: TcpStream,
        connection: Connection,
        settings: Arc<NetworkingSettings>,
    ) {
        socket_mut
            .peers
            .insert(connection, Peer::new(stream.clone(), settings.clone()));

        smol::spawn(async move {
            messages
                .send((connection, ServerMessage::Connected))
                .await
                .unwrap();
            Self::recv_tcp_messages(stream, connection, messages.clone(), socket, settings).await;
        })
        .detach();
    }

    fn recv_udp_messages(&self) {
        let settings = self.settings.clone();
        let socket = self.socket.clone();
        let messages = self.messages.clone();

        let udp_socket = socket.lock_blocking().as_ref().unwrap().udp_socket.clone();
        smol::spawn(async move {
            let mut buffered_messages: HashMap<SocketAddr, super::BufferingMessage> =
                HashMap::default();

            let mut buf: [u8; SAFE_MTU_SIZE] = [0; SAFE_MTU_SIZE];

            let mut ords: HashMap<SocketAddr, u32> = HashMap::default();

            while let Ok((size, addr)) = udp_socket.recv_from(&mut buf).await {
                // Break loop if stop function was used.
                let mut socket_lock = socket.lock_arc().await;
                let Some(socket_mut) = socket_lock.as_mut() else {
                    break;
                };

                // If the remote connection has an incompleted message
                if let Some(mut buffering_message) = buffered_messages.remove(&addr) {
                    // Add buffer to the message
                    if !buffering_message.completed(&buf[..size]) {
                        buffered_messages.insert(addr, buffering_message);
                        continue;
                    };
                    let Some(connection) = socket_mut.connection(&addr) else {
                        continue;
                    };

                    // Send completed message
                    messages
                        .send((connection, ServerMessage::Udp(buffering_message.consume())))
                        .await
                        .unwrap();
                    continue;
                }

                match size {
                    // 8 bytes = ping
                    8 => {
                        let Some(connection) = socket_mut.connection(&addr) else {
                            continue;
                        };

                        match socket_mut.ping(settings.max_ping, connection).await {
                            Ok(ping_over_limit) => {
                                if ping_over_limit {
                                    messages
                                        .send((
                                            connection,
                                            ServerMessage::Warning(super::Warning::PingTooHigh),
                                        ))
                                        .await
                                        .unwrap();
                                }
                            }
                            Err(e) => {
                                messages
                                    .send((connection, ServerMessage::Error(e)))
                                    .await
                                    .unwrap();
                            }
                        }

                        continue;
                    }
                    // Ignore messages smaller than the header.
                    size if size < 8 => {
                        continue;
                    }
                    size if size > settings.udp_max_size => {
                        let Some(connection) = socket_mut.connection(&addr) else {
                            continue;
                        };
                        messages
                            .send((
                                connection,
                                ServerMessage::Warning(super::Warning::MessageTooBig),
                            ))
                            .await
                            .unwrap();

                        continue;
                    }
                    _ => (),
                }

                // Get order number
                let ord = u32::from_le_bytes(buf[0..4].try_into().unwrap());

                // If order number is 0, see message as session auth request.
                if ord == 0 {
                    if let Some((tcp_stream, tcp_addr)) = socket_mut.pending.remove(&buf[..128]) {
                        // send 8 bytes to indicate approval
                        socket_mut.udp_socket.send_to(&[0; 8], addr).await.unwrap();
                        Self::connect_client(
                            messages.clone(),
                            socket.clone(),
                            socket_mut,
                            tcp_stream,
                            Connection::from_tcp_udp_addr(tcp_addr, addr),
                            settings.clone(),
                        )
                        .await;
                    }
                    continue;
                }

                // Verify order
                if let Some(last_ord) = ords.insert(addr, ord) {
                    match ord {
                        ord if ord == last_ord + 1 => (), // in order -> allow
                        _ => {
                            // out of order -> discard
                            continue;
                        }
                    }
                };

                // following code only runs if the user is authenticated

                let len = u32::from_le_bytes(buf[4..8].try_into().unwrap()) as usize;

                if len == 0 {
                    // message length of 0 means try ping
                    let _ = socket_mut.udp_socket.send(&buf).await;
                }

                let Some(connection) = socket_mut.connection(&addr) else {
                    continue;
                };

                if let Some(peer) = socket_mut.peers.get_mut(&connection) {
                    peer.record_rate();
                    if peer.over_rate_limit() {
                        messages
                            .send((
                                connection,
                                ServerMessage::Warning(super::Warning::RateLimitHit),
                            ))
                            .await
                            .unwrap();
                    }
                };

                // Clear memory of failed buffers.
                buffered_messages.retain(|_, x| !x.outdated());

                let mut buffering_message = super::BufferingMessage::new(len);

                // If the packet holds the whole message don't bother buffering it.
                if buffering_message.completed(&buf[8..]) {
                    messages
                        .send((connection, ServerMessage::Udp(buffering_message.consume())))
                        .await
                        .unwrap();
                } else {
                    buffered_messages.insert(addr, buffering_message);
                }
            }
        })
        .detach();
    }

    /// Receives messages from each TCP connection.
    async fn recv_tcp_messages(
        mut stream: TcpStream,
        connection: Connection,
        messages: Sender<(Connection, ServerMessage)>,
        socket: Arc<Mutex<Option<Socket>>>,
        settings: Arc<NetworkingSettings>,
    ) {
        let disconnect_reason;
        let mut size_buf = [0u8; 4];

        loop {
            let mut buf: Vec<u8> = Vec::with_capacity(1032);

            // Get u32 size prefix
            if let Err(e) = stream.read_exact(&mut size_buf).await {
                disconnect_reason = e.into();
                break;
            };
            {
                let mut socket = socket.lock().await;
                if let Some(peer) = socket.as_mut().unwrap().peers.get_mut(&connection) {
                    peer.record_rate();
                    if peer.over_rate_limit() {
                        messages
                            .send((
                                connection,
                                ServerMessage::Warning(super::Warning::RateLimitHit),
                            ))
                            .await
                            .unwrap();
                    }
                };
            }

            let size = u32::from_le_bytes(size_buf) as usize;
            match size {
                0 => {
                    disconnect_reason = Disconnected::MisbehavingPeer;
                    break;
                }
                size if size > settings.tcp_max_size => {
                    messages
                        .send((
                            connection,
                            ServerMessage::Warning(super::Warning::MessageTooBig),
                        ))
                        .await
                        .unwrap();
                    continue;
                }
                _ => (),
            }
            buf.resize(size, 0);

            // Read as many bytes as in the size prefix
            if let Err(e) = stream.read_exact(&mut buf).await {
                disconnect_reason = e.into();
                break;
            };

            messages
                .send((connection, ServerMessage::Tcp(buf)))
                .await
                .unwrap();
        }

        let mut socket = socket.lock().await;
        let _ = Self::disconnect_user_with(
            connection,
            disconnect_reason,
            &messages,
            socket.as_mut().unwrap(),
        )
        .await;
    }

    async fn disconnect_user_with(
        conn: Connection,
        reason: Disconnected,
        messages: &Sender<(Connection, ServerMessage)>,
        socket: &mut Socket,
    ) -> Result<(), ServerError> {
        messages
            .send((conn, ServerMessage::Disconnected(reason)))
            .await
            .unwrap();
        let peer = socket
            .peers
            .remove(&conn)
            .ok_or(ServerError::UserNotFound)?;

        peer.tcp_stream
            .shutdown(std::net::Shutdown::Both)
            .map_err(ServerError::Io)?;

        Ok(())
    }

    /// Requests a ping to the client to update the ping value.
    ///
    /// Only returns an error in case the connection does not exist.
    pub fn request_repinging(&self, connection: &Connection) -> Result<(), ServerError> {
        let socket = self.socket.clone();
        let settings = self.settings.clone();

        if !self.is_connected(connection)? {
            return Err(ServerError::UserNotFound);
        };

        let connection = *connection;
        self.spawn(async move {
            let mut socket = socket.lock().await;
            socket
                .as_mut()
                .unwrap()
                .ping(settings.max_ping, connection)
                .await
                .map_err(|e| (connection, e))?;
            Ok(None)
        });
        Ok(())
    }

    /// Returns the ping of the given user.
    pub fn ping(&self, connection: &Connection) -> Result<Duration, ServerError> {
        let socket = self.socket.lock_blocking();
        if let Some(user) = socket.as_ref().unwrap().peers.get(connection) {
            Ok(user.ping)
        } else {
            Err(ServerError::UserNotFound)
        }
    }

    /// Hosts a server in this engine struct with the given port to accept clients from the same device and send/receive messages.
    pub fn start_local(&self, port: u16) -> Result<(), ServerError> {
        let addr = SocketAddr::new(IpAddr::V4(std::net::Ipv4Addr::new(127, 0, 0, 1)), port);
        self.start(addr)
    }

    /// Allows users from the local network to join the game using the given port and allows users from around the world to join
    /// if this port is forwarded in your network.
    pub fn start_public(&self, port: u16) -> Result<(), ServerError> {
        let addr = SocketAddr::new(local_ip_address::local_ip().unwrap(), port);
        self.start(addr)
    }
}

/// All kinds of errors that can be returned by the server.
#[derive(Debug, Error)]
pub enum ServerError {
    /// Returns when you attempt to start the server, even when it is already running.
    #[error("The server is already running.")]
    AlreadyRunning,
    /// Returns when running a method that requires the server to be active.
    #[error("The server is not running.")]
    NotRunning,
    /// Returns when the user is not connected to the server.
    #[error("This user is not connected to the server.")]
    UserNotFound,
    /// Returns if an IO or OS error has occured.
    #[error(transparent)]
    Io(std::io::Error),
}
