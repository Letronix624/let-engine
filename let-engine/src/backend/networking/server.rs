use std::{
    collections::VecDeque,
    sync::{atomic::AtomicBool, Arc, LazyLock},
    time::{Duration, SystemTime},
};

use anyhow::Result;
use foldhash::HashMap;
use let_engine_core::backend::NetworkingBackend;
use smol::{
    channel::{unbounded, Sender},
    io::{AsyncReadExt, AsyncWriteExt},
    lock::Mutex,
    net::{SocketAddr, TcpListener, TcpStream, UdpSocket},
};
use thiserror::Error;

use super::{serialize_tcp, Connection, Disconnected, Messages, Networking, RemoteMessage};

type Pending = Mutex<HashMap<[u8; 128], (TcpStream, SocketAddr)>>;

#[derive(Clone)]
struct Peer {
    tcp_stream: TcpStream,
    order_number: u32,
    ping_timestamp: Option<SystemTime>,
    ping: Duration,
    settings: Arc<Networking>,

    last_package: SystemTime,
    last_package_durations: VecDeque<Duration>,
    rate_average: Duration,
}

impl Peer {
    pub fn new(tcp_stream: TcpStream, settings: Arc<Networking>) -> Self {
        let mut last_package_durations = VecDeque::with_capacity(10);
        last_package_durations.extend([Duration::from_secs(600); 10]);
        Self {
            tcp_stream,
            order_number: 1,
            ping_timestamp: None,
            ping: Duration::default(),
            settings,

            last_package: SystemTime::now(),
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
            .push_back(self.last_package.elapsed().unwrap());
        self.last_package_durations.pop_front();
        self.last_package = SystemTime::now();

        self.rate_average = self
            .last_package_durations
            .iter()
            .fold(Duration::ZERO, |acc, &x| acc + x)
            / 10;
    }

    pub fn over_rate_limit(&self) -> bool {
        self.rate_average < self.settings.rate_limit.load()
    }
}

pub(crate) static LAST_ORDS: LazyLock<parking_lot::Mutex<HashMap<SocketAddr, u32>>> =
    LazyLock::new(|| parking_lot::Mutex::new(HashMap::default()));

struct Socket {
    udp_socket: UdpSocket,
    settings: Arc<Networking>,

    connections_map: Mutex<HashMap<Connection, Peer>>,
    /// Both TCP and UDP lead to the same Connection
    connections: Mutex<HashMap<SocketAddr, Connection>>,
    connecting: Pending,
    running: AtomicBool,
}

impl Socket {
    /// Records the time and stops the echoing.
    ///
    /// Returns true if ping is over ping limit
    async fn ping(&self, connection: &Connection) -> bool {
        let mut peers = self.connections_map.lock().await;
        let Some(peer) = peers.get_mut(connection) else {
            return false;
        };

        let time = std::mem::take(&mut peer.ping_timestamp);

        if let Some(time) = time {
            peer.ping = time.elapsed().unwrap();
            peer.ping > self.settings.max_ping()
        } else {
            // send 8 byte message to be echoed
            let _ = self.udp_socket.send_to(&[0; 8], connection.udp_addr).await;
            peer.ping_timestamp = Some(SystemTime::now());
            false
        }
    }
}

/// A server instance that allows you to send messages to your client.
pub struct GameServer<B: NetworkingBackend> {
    socket: Arc<Socket>,
    pub(crate) messages: Messages<B::Msg>,
    settings: Arc<Networking>,
}

impl<B: NetworkingBackend> Clone for GameServer<B> {
    fn clone(&self) -> Self {
        Self {
            socket: self.socket.clone(),
            messages: self.messages.clone(),
            settings: self.settings.clone(),
        }
    }
}

impl<B: NetworkingBackend + 'static> GameServer<B> {
    /// Creates a new server using the given address.
    pub(crate) fn new(addr: SocketAddr, settings: Arc<Networking>) -> Result<Self> {
        smol::block_on(async {
            let tcp_listener = TcpListener::bind(addr).await?;

            let udp_socket = UdpSocket::bind(addr).await?;

            let server = Self {
                socket: Arc::new(Socket {
                    udp_socket,
                    settings: settings.clone(),
                    connections_map: Mutex::new(HashMap::default()),
                    connections: Mutex::new(HashMap::default()),
                    connecting: Mutex::new(HashMap::default()),
                    running: false.into(),
                }),
                messages: unbounded(),
                settings,
            };

            server.accept_connetions(tcp_listener);

            Ok(server)
        })
    }

    fn accept_connetions(&self, listener: TcpListener) {
        let socket = self.socket.clone();
        let settings = self.settings.clone();
        smol::spawn(async {
            let socket = socket;
            let listener = listener;
            let settings = settings;

            while let Ok((mut stream, addr)) = listener.accept().await {
                if settings.max_connections() <= socket.connections_map.lock().await.len() {
                    let _ = stream.shutdown(std::net::Shutdown::Both);
                    continue;
                }
                let mut buf = [0; 128];

                let op = stream.read_exact(&mut buf);

                use futures::future::Either;

                // 3 seconds or max ping limit
                match futures::future::select(
                    op,
                    smol::Timer::after(std::time::Duration::from_secs(3)),
                )
                .await
                {
                    Either::Left(result) => {
                        if result.0.is_err() {
                            return;
                        }
                    }
                    Either::Right(_) => return,
                };

                socket.connecting.lock().await.insert(buf, (stream, addr));
            }
        })
        .detach();
    }

    async fn connect_client(
        messages: Sender<(Connection, RemoteMessage<B::Msg>)>,
        socket: Arc<Socket>,
        stream: TcpStream,
        tcp_addr: SocketAddr,
        udp_addr: SocketAddr,
        settings: Arc<Networking>,
    ) {
        let connection = Connection::new(tcp_addr, udp_addr.port());

        if socket.running.load(std::sync::atomic::Ordering::Acquire)
            && messages
                .clone()
                .send((connection, RemoteMessage::Connected))
                .await
                .is_ok()
        {
            socket
                .connections_map
                .lock()
                .await
                .insert(connection, Peer::new(stream.clone(), settings.clone()));

            {
                let mut connections_lock = socket.connections.lock().await;
                connections_lock.insert(connection.tcp_addr(), connection);
                connections_lock.insert(connection.udp_addr(), connection);
            }

            let messages2 = messages.clone();
            let socket = socket.clone();
            smol::spawn(async move {
                let socket = socket;
                let stream = stream;
                let messages = messages2;
                Self::recv_messages(stream, connection, messages.clone(), socket, settings).await;
            })
            .detach();
        }
    }

    fn recv_udp_messages(&self) {
        let server = self.clone();
        let settings = self.settings.clone();
        smol::spawn(async {
            let server = server;
            let socket = server.socket;
            let settings = settings;

            let mut buffered_messages: HashMap<SocketAddr, super::BufferingMessage> =
                HashMap::default();

            let mut buf: [u8; 1024] = [0; 1024];

            loop {
                if let Ok((size, addr)) = socket.udp_socket.recv_from(&mut buf).await {
                    // Break loop if stop function was used.
                    if !socket.running.load(std::sync::atomic::Ordering::Acquire) {
                        break;
                    }

                    // If the remote connection has an incompleted message
                    if let Some(buffering_message) = buffered_messages.get_mut(&addr) {
                        // Add buffer to the message
                        let Some(message) = buffering_message.completed(&buf[..size]) else {
                            continue;
                        };
                        let Some(connection) = socket.connections.lock().await.get(&addr).cloned()
                        else {
                            continue;
                        };

                        // Send completed message
                        if let Ok(message) = bincode::deserialize::<B::Msg>(message) {
                            if server
                                .messages
                                .0
                                .send((connection, RemoteMessage::Udp(message)))
                                .await
                                .is_err()
                            {
                                break;
                            };
                        }
                        buffered_messages.remove(&addr);
                        continue;
                    }

                    match size {
                        // 8 bytes = ping
                        8 => {
                            let Some(connection) =
                                socket.connections.lock().await.get(&addr).cloned()
                            else {
                                continue;
                            };
                            if socket.ping(&connection).await
                                && server
                                    .messages
                                    .0
                                    .send((
                                        connection,
                                        RemoteMessage::Warning(super::Misbehaviour::PingTooHigh),
                                    ))
                                    .await
                                    .is_err()
                            {
                                break;
                            };

                            continue;
                        }
                        // Ignore messages smaller than the header.
                        size if size < 8 => {
                            continue;
                        }
                        size if size > settings.udp_size_limit() => {
                            let Some(connection) =
                                socket.connections.lock().await.get(&addr).cloned()
                            else {
                                continue;
                            };
                            if server
                                .messages
                                .0
                                .send((
                                    connection,
                                    RemoteMessage::Warning(super::Misbehaviour::MessageTooBig),
                                ))
                                .await
                                .is_err()
                            {
                                break;
                            };
                            continue;
                        }
                        _ => (),
                    }

                    // Get order number
                    let ord = u32::from_le_bytes(buf[0..4].try_into().unwrap());

                    // If order number is 0, see message as session auth request.
                    if ord == 0 {
                        if let Some(connecting) = socket.connecting.lock().await.remove(&buf[..128])
                        {
                            // send 8 bytes to indicate approval
                            let _ = socket.udp_socket.send_to(&[0; 8], addr).await;
                            Self::connect_client(
                                server.messages.0.clone(),
                                socket.clone(),
                                connecting.0.clone(),
                                connecting.1,
                                addr,
                                settings.clone(),
                            )
                            .await;
                        }
                        continue;
                    }

                    // Verify order
                    if let Some(last_ord) = LAST_ORDS.lock().insert(addr, ord) {
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
                        let _ = socket.udp_socket.send(&buf).await;
                    }

                    let Some(connection) = socket.connections.lock().await.get(&addr).cloned()
                    else {
                        continue;
                    };

                    if let Some(peer) = socket.connections_map.lock().await.get_mut(&connection) {
                        peer.record_rate();
                        if peer.over_rate_limit()
                            && server
                                .messages
                                .0
                                .send((
                                    connection,
                                    RemoteMessage::Warning(super::Misbehaviour::RateLimitHit),
                                ))
                                .await
                                .is_err()
                        {
                            break;
                        };
                    };

                    // Clear memory of failed buffers.
                    buffered_messages.retain(|_, x| !x.outdated());

                    let mut buffering_message = super::BufferingMessage::new(len);

                    // If the packet holds the whole message don't bother buffering it.
                    if let Some(data) = buffering_message.completed(&buf[8..]) {
                        if let Ok(message) = bincode::deserialize::<B::Msg>(data) {
                            if server
                                .messages
                                .0
                                .send((connection, RemoteMessage::Udp(message)))
                                .await
                                .is_err()
                            {
                                break;
                            };
                        }
                    } else {
                        buffered_messages.insert(addr, buffering_message);
                    }
                }
            }
        })
        .detach();
    }

    /// Receives messages from each TCP connection.
    async fn recv_messages(
        mut stream: TcpStream,
        connection: Connection,
        messages: Sender<(Connection, RemoteMessage<B::Msg>)>,
        socket: Arc<Socket>,
        settings: Arc<Networking>,
    ) {
        let disconnect_reason;
        let mut size_buf = [0u8; 4];

        let mut buf: Vec<u8> = Vec::with_capacity(1032);
        loop {
            buf.clear();

            // Get u32 size prefix
            if let Err(e) = stream.read_exact(&mut size_buf).await {
                disconnect_reason = e.into();
                break;
            };

            if let Some(peer) = socket.connections_map.lock().await.get_mut(&connection) {
                peer.record_rate();
                if peer.over_rate_limit() {
                    let _ = messages
                        .send((
                            connection,
                            RemoteMessage::Warning(super::Misbehaviour::RateLimitHit),
                        ))
                        .await;
                }
            };

            let size = u32::from_le_bytes(size_buf) as usize;
            match size {
                0 => {
                    disconnect_reason = Disconnected::MisbehavingPeer;
                    break;
                }
                size if size > settings.tcp_size_limit() => {
                    let _ = messages
                        .send((
                            connection,
                            RemoteMessage::Warning(super::Misbehaviour::MessageTooBig),
                        ))
                        .await;
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

            // Send the message if it's correctly deserialized.
            let _ = match bincode::deserialize::<B::Msg>(&buf) {
                Ok(message) => {
                    messages
                        .send((connection, RemoteMessage::Tcp(message)))
                        .await
                }
                Err(e) => {
                    messages
                        .send((
                            connection,
                            RemoteMessage::Warning(super::Misbehaviour::UnintelligableContent(e)),
                        ))
                        .await
                }
            };
        }

        let _ = Self::disconnect_user_with(
            connection,
            disconnect_reason,
            &messages,
            &mut *socket.connections_map.lock().await,
            &mut *socket.connections.lock().await,
        )
        .await;
    }

    #[cfg(feature = "client")]
    pub(crate) async fn receive_messages(&self) -> Vec<(Connection, RemoteMessage<B::Msg>)> {
        let mut messages: Vec<(Connection, RemoteMessage<B::Msg>)> = vec![];
        while let Ok(message) = self.messages.1.try_recv() {
            messages.push((message.0, message.1));
        }
        messages
    }

    /// Stops the server
    ///
    /// May return an OS IO error
    pub async fn stop(&self) -> Result<(), smol::io::Error> {
        self.socket
            .running
            .store(false, std::sync::atomic::Ordering::Release);
        let connections = std::mem::take(&mut *self.socket.connections_map.lock().await);
        for connection in connections.into_values() {
            connection.tcp_stream.shutdown(std::net::Shutdown::Both)?;
        }
        *self.socket.connections.lock().await = HashMap::default();

        Ok(())
    }

    /// Starts the server up.
    pub fn start(&self) {
        self.socket
            .running
            .store(true, std::sync::atomic::Ordering::Release);
        self.recv_udp_messages();
    }

    /// Broadcasts a message to every client through TCP.
    ///
    /// This function should be used to broadcast important messages.
    pub async fn broadcast(&self, message: &B::Msg) -> Result<(), ServerError> {
        let mut stream_map = self.socket.connections_map.lock().await;
        for (user, connection) in stream_map.clone().iter_mut() {
            let result = connection
                .tcp_stream
                .write_all(&serialize_tcp(&message).map_err(ServerError::SerialisationError)?)
                .await;
            if let Err(e) = result {
                Self::disconnect_user_with(
                    *user,
                    e.into(),
                    &self.messages.0,
                    &mut stream_map,
                    &mut *self.socket.connections.lock().await,
                )
                .await?
            }
        }
        Ok(())
    }

    /// Sends a message to a specific target through TCP.
    ///
    /// This function should be used to send important messages.
    pub async fn send(&self, receiver: Connection, message: &B::Msg) -> Result<(), ServerError> {
        let result = self
            .socket
            .connections_map
            .lock()
            .await
            .get_mut(&receiver)
            .ok_or(ServerError::UserNotFound)?
            .tcp_stream
            .write_all(&super::serialize_tcp(message).map_err(ServerError::SerialisationError)?)
            .await;
        if let Err(e) = result {
            self.disconnect_user(receiver, e.into()).await?;
        }
        Ok(())
    }

    /// Broadcasts a message to every client through UDP.
    ///
    /// This function should be used to broadcast messages with the lowest latency possible.
    pub async fn fast_broadcast(&self, message: &B::Msg) -> Result<(), ServerError> {
        let mut peers = self.socket.connections_map.lock().await;
        let mut disconnect = Vec::new();
        for (connection, peer) in peers.iter_mut() {
            // TODO: Optimize by not serializing for each client but only serialize once and
            //       only update the order number for each.
            let data = super::serialize_udp(peer.order_number(), message)
                .map_err(ServerError::SerialisationError)?;
            let chunks = data.chunks(1024);

            for chunk in chunks {
                let result = self
                    .socket
                    .udp_socket
                    .send_to(chunk, connection.udp_addr)
                    .await;
                if let Err(e) = result {
                    disconnect.push((*connection, e));
                }
            }
        }

        // retain does not work in async
        for (connection, e) in disconnect {
            Self::disconnect_user_with(
                connection,
                e.into(),
                &self.messages.0,
                &mut peers,
                &mut *self.socket.connections.lock().await,
            )
            .await?;
        }
        Ok(())
    }

    /// Sends a message to a specific target through UDP.
    ///
    /// This function should be used to send messages with the lowest latency possible.
    pub async fn fast_send(
        &self,
        receiver: Connection,
        message: &B::Msg,
    ) -> Result<(), ServerError> {
        let mut peers = self.socket.connections_map.lock().await;
        let peer = peers.get_mut(&receiver).ok_or(ServerError::UserNotFound)?;

        let data = super::serialize_udp(peer.order_number(), message)
            .map_err(ServerError::SerialisationError)?;
        let chunks = data.chunks(1024);

        for chunk in chunks {
            self.socket
                .udp_socket
                .send_to(chunk, receiver.udp_addr)
                .await
                .map_err(ServerError::Io)?;
        }

        Ok(())
    }

    async fn disconnect_user_with(
        user: Connection,
        reason: Disconnected,
        messages: &Sender<(Connection, RemoteMessage<B::Msg>)>,
        stream_map: &mut HashMap<Connection, Peer>,
        connections: &mut HashMap<SocketAddr, Connection>,
    ) -> Result<(), ServerError> {
        messages
            .send((user, RemoteMessage::Disconnected(reason)))
            .await
            .map_err(|_| ServerError::MessageChannelClosed)?;
        let connection = stream_map.remove(&user).ok_or(ServerError::UserNotFound)?;
        connections.remove(&user.tcp_addr);
        connections.remove(&user.udp_addr);

        connection
            .tcp_stream
            .shutdown(std::net::Shutdown::Both)
            .map_err(ServerError::Io)?;

        Ok(())
    }

    /// Disconnects the specified user.
    pub async fn disconnect_user(
        &self,
        user: Connection,
        reason: Disconnected,
    ) -> Result<(), ServerError> {
        self.messages
            .0
            .send((user, RemoteMessage::Disconnected(reason)))
            .await
            .map_err(|_| ServerError::MessageChannelClosed)?;
        let connection = self
            .socket
            .connections_map
            .lock()
            .await
            .remove(&user)
            .ok_or(ServerError::UserNotFound)?;
        self.socket.connections.lock().await.remove(&user.tcp_addr);
        self.socket.connections.lock().await.remove(&user.udp_addr);

        connection
            .tcp_stream
            .shutdown(std::net::Shutdown::Both)
            .map_err(ServerError::Io)?;

        Ok(())
    }

    /// Returns a list of all connections currently initiated with the server.
    pub async fn connections(&self) -> Vec<Connection> {
        self.socket
            .connections_map
            .lock()
            .await
            .keys()
            .cloned()
            .collect()
    }

    /// Requests a ping to the client to update the ping value.
    pub async fn request_repinging(&self, connection: &Connection) {
        self.socket.ping(connection).await;
    }

    /// Returns the ping of the given user.
    pub async fn ping(&self, connection: &Connection) -> Result<Duration, ServerError> {
        let peer = self.socket.connections_map.lock().await;
        if let Some(user) = peer.get(connection) {
            Ok(user.ping)
        } else {
            Err(ServerError::UserNotFound)
        }
    }
}

/// All kinds of errors that can be returned by the server.
#[derive(Debug, Error)]
pub enum ServerError {
    /// Returns when the user is not connected to the server.
    #[error("This user is not connected to the server.")]
    UserNotFound,
    /// Returns if an IO or OS error has occured.
    #[error("{0}")]
    Io(smol::io::Error),
    /// Returns if the message channel somehow unexpectedly closes and returns errors.
    #[error("This server can not be used anymore: The message channel is closed.")]
    MessageChannelClosed,
    #[error("{0}")]
    SerialisationError(bincode::Error),
}
