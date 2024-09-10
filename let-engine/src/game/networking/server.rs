use std::sync::{atomic::AtomicBool, Arc, LazyLock};

use ahash::HashMap;
use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use smol::{
    channel::{unbounded, Sender},
    io::{AsyncReadExt, AsyncWriteExt},
    lock::Mutex,
    net::{SocketAddr, TcpListener, TcpStream, UdpSocket},
};

use super::{serialize_tcp, Connection, Disconnected, Messages, RemoteMessage};

type Pending = Arc<Mutex<HashMap<[u8; 128], (TcpStream, SocketAddr)>>>;

#[derive(Clone)]
struct Peer {
    tcp_stream: TcpStream,
    order_number: u32,
}

impl Peer {
    pub fn new(tcp_stream: TcpStream) -> Self {
        Self {
            tcp_stream,
            order_number: 1,
        }
    }

    pub fn order_number(&mut self) -> u32 {
        self.order_number += 1;
        self.order_number
    }
}

pub(crate) static LAST_ORDS: LazyLock<parking_lot::Mutex<HashMap<SocketAddr, u32>>> =
    LazyLock::new(|| parking_lot::Mutex::new(HashMap::default()));

/// A server instance that allows you to send messages to your client.
#[derive(Clone)]
pub struct GameServer<Msg>
where
    for<'a> Msg: Send + Sync + Serialize + Deserialize<'a>,
{
    udp_socket: Arc<UdpSocket>,

    connections_map: Arc<Mutex<HashMap<Connection, Peer>>>,
    connections: Arc<Mutex<HashMap<SocketAddr, Connection>>>,
    connecting: Pending,

    pub(crate) messages: Messages<Msg>,
    running: Arc<AtomicBool>,
}

impl<Msg> GameServer<Msg>
where
    for<'a> Msg: Send + Sync + Serialize + Deserialize<'a> + Clone + 'static,
{
    /// Creates a new server using the given address.
    pub(crate) fn new(addr: SocketAddr) -> Result<Self> {
        smol::block_on(async {
            let tcp_listener = TcpListener::bind(addr).await?;

            let udp_socket = Arc::new(UdpSocket::bind(addr).await?);

            let server = Self {
                udp_socket,
                connections_map: Mutex::new(HashMap::default()).into(),
                connections: Mutex::new(HashMap::default()).into(),
                connecting: Mutex::new(HashMap::default()).into(),
                messages: unbounded(),
                running: Arc::new(false.into()),
            };

            server.accept_connetions(tcp_listener);

            Ok(server)
        })
    }

    fn accept_connetions(&self, listener: TcpListener) {
        let connecting = self.connecting.clone();
        smol::spawn(async {
            let connecting = connecting;
            let listener = listener;
            while let Ok((mut stream, addr)) = listener.accept().await {
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

                connecting.lock_arc().await.insert(buf, (stream, addr));
            }
        })
        .detach();
    }

    async fn connect_client(
        messages: Sender<(Connection, RemoteMessage<Msg>)>,
        connections: Arc<Mutex<HashMap<SocketAddr, Connection>>>,
        tcp_map: Arc<Mutex<HashMap<Connection, Peer>>>,
        running: Arc<AtomicBool>,
        stream: TcpStream,
        tcp_addr: SocketAddr,
        udp_addr: SocketAddr,
    ) {
        let connection = Connection::new(tcp_addr, udp_addr.port());

        if running.load(std::sync::atomic::Ordering::Acquire)
            && messages
                .clone()
                .send((connection, RemoteMessage::Connected))
                .await
                .is_ok()
        {
            tcp_map
                .lock_arc()
                .await
                .insert(connection, Peer::new(stream.clone()));

            {
                let mut connections_lock = connections.lock_arc().await;
                connections_lock.insert(connection.tcp_addr(), connection);
                connections_lock.insert(connection.udp_addr(), connection);
            }

            let tcp_map2 = tcp_map.clone();
            let connections2 = connections.clone();
            let messages2 = messages.clone();
            smol::spawn(async move {
                let stream = stream;
                let tcp_map = tcp_map2;
                let connections = connections2;
                let messages = messages2;
                Self::recv_messages(stream, connection, messages.clone(), tcp_map, connections)
                    .await;
            })
            .detach();
        }
    }

    fn recv_udp_messages(&self) {
        let server = self.clone();
        smol::spawn(async {
            let server = server;

            let mut buffered_messages: HashMap<SocketAddr, super::BufferingMessage> =
                HashMap::default();

            let mut buf: [u8; 1024] = [0; 1024];

            loop {
                if let Ok((size, addr)) = server.udp_socket.recv_from(&mut buf).await {
                    // Break loop if stop function was used.
                    if !server.running.load(std::sync::atomic::Ordering::Acquire) {
                        break;
                    }

                    // If the remote connection has an incompleted message
                    if let Some(buffering_message) = buffered_messages.get_mut(&addr) {
                        // Add buffer to the message
                        let Some(message) = buffering_message.completed(&buf[..size]) else {
                            continue;
                        };

                        let Some(connection) =
                            server.connections.lock_arc().await.get(&addr).cloned()
                        else {
                            continue;
                        };

                        // Send completed message
                        if let Ok(message) = bincode::deserialize::<Msg>(message) {
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

                    // Ignore messages smaller than the header.
                    if size < 8 {
                        continue;
                    }

                    // Get order number
                    let ord = u32::from_le_bytes(buf[0..4].try_into().unwrap());

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

                    // If order number is 0, see message as session auth request.
                    if ord == 0 {
                        if let Some(connecting) =
                            server.connecting.lock_arc().await.remove(&buf[..128])
                        {
                            Self::connect_client(
                                server.messages.0.clone(),
                                server.connections.clone(),
                                server.connections_map.clone(),
                                server.running.clone(),
                                connecting.0.clone(),
                                connecting.1,
                                addr,
                            )
                            .await;
                        }
                        continue;
                    }

                    let Some(connection) = server.connections.lock_arc().await.get(&addr).cloned()
                    else {
                        continue;
                    };
                    // following code only runs if the user is authenticated

                    let len = u32::from_le_bytes(buf[4..8].try_into().unwrap()) as usize;

                    if len == 0 {
                        // message length of 0 means try ping
                        let _ = server.udp_socket.send(&buf).await;
                    }

                    // Clear memory of failed buffers.
                    buffered_messages.retain(|_, x| !x.outdated());

                    let mut buffering_message = super::BufferingMessage::new(len);

                    // If the packet holds the whole message don't bother buffering it.
                    if let Some(data) = buffering_message.completed(&buf[8..]) {
                        if let Ok(message) = bincode::deserialize::<Msg>(data) {
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
        addr: Connection,
        messages: Sender<(Connection, RemoteMessage<Msg>)>,
        stream_map: Arc<Mutex<HashMap<Connection, Peer>>>,
        connections: Arc<Mutex<HashMap<SocketAddr, Connection>>>,
    ) {
        let disconnect_reason;
        let mut size_buf = [0u8; 4];

        let mut buf = Vec::with_capacity(1032);
        loop {
            buf.clear();

            // Get u32 size prefix
            if let Err(e) = stream.read_exact(&mut size_buf).await {
                disconnect_reason = e.into();
                break;
            };

            let size = u32::from_le_bytes(size_buf) as usize;
            if size == 0 {
                disconnect_reason = Disconnected::MisbehavingPeer;
                break;
            }
            buf.resize(size, 0);

            // Read as many bytes as in the size prefix
            if let Err(e) = stream.read_exact(&mut buf).await {
                disconnect_reason = e.into();
                break;
            };

            // Send the message if it's correctly deserialized.
            let _ = match bincode::deserialize::<Msg>(&buf) {
                Ok(message) => messages.send((addr, RemoteMessage::Tcp(message))).await,
                Err(e) => {
                    messages
                        .send((addr, RemoteMessage::DeserialisationError(e)))
                        .await
                }
            };
        }

        let _ = Self::disconnect_user_with(
            addr,
            disconnect_reason,
            &messages,
            &mut *stream_map.lock_arc().await,
            &mut *connections.lock_arc().await,
        )
        .await;
    }

    #[cfg(feature = "client")]
    pub(crate) async fn receive_messages(&self) -> Vec<(Connection, RemoteMessage<Msg>)> {
        let mut messages: Vec<(Connection, RemoteMessage<Msg>)> = vec![];
        while let Ok(message) = self.messages.1.try_recv() {
            messages.push((message.0, message.1));
        }
        messages
    }
}

impl<Msg> GameServer<Msg>
where
    for<'a> Msg: Send + Sync + Serialize + Deserialize<'a> + Clone + 'static,
{
    /// Stops the server
    pub async fn stop(&self) -> Result<()> {
        self.running
            .store(false, std::sync::atomic::Ordering::Release);
        let connections = std::mem::take(&mut *self.connections_map.lock_arc().await);
        for connection in connections.into_values() {
            connection.tcp_stream.shutdown(std::net::Shutdown::Both)?;
        }
        *self.connections.lock_arc().await = HashMap::default();

        Ok(())
    }

    /// Starts the server up.
    pub fn start(&self) {
        self.running
            .store(true, std::sync::atomic::Ordering::Release);
        self.recv_udp_messages();
    }

    /// Broadcasts a message to every client through TCP.
    ///
    /// This function should be used to broadcast important messages.
    pub async fn broadcast(&self, message: &Msg) -> Result<()> {
        let mut stream_map = self.connections_map.lock_arc().await;
        for (user, connection) in stream_map.clone().iter_mut() {
            let result = connection
                .tcp_stream
                .write_all(&serialize_tcp(&message)?)
                .await;
            if let Err(e) = result {
                Self::disconnect_user_with(
                    *user,
                    e.into(),
                    &self.messages.0,
                    &mut stream_map,
                    &mut *self.connections.lock_arc().await,
                )
                .await?
            }
        }
        Ok(())
    }

    /// Sends a message to a specific target through TCP.
    ///
    /// This function should be used to send important messages.
    pub async fn send(&self, receiver: Connection, message: &Msg) -> Result<()> {
        let result = self
            .connections_map
            .lock_arc()
            .await
            .get_mut(&receiver)
            .ok_or(anyhow!("Receiver does not exist"))?
            .tcp_stream
            .write_all(&super::serialize_tcp(message)?)
            .await;
        if let Err(e) = result {
            self.disconnect_user(receiver, e.into()).await?;
        }
        Ok(())
    }

    /// Broadcasts a message to every client through UDP.
    ///
    /// This function should be used to broadcast messages with the lowest latency possible.
    pub async fn udp_broadcast(&self, message: &Msg) -> Result<()> {
        let mut peers = self.connections_map.lock_arc().await;
        let mut disconnect = Vec::new();
        for (connection, peer) in peers.iter_mut() {
            let result = self
                .udp_socket
                .send_to(
                    &super::serialize_udp(peer.order_number(), message)?,
                    connection.udp_addr,
                )
                .await;
            if let Err(e) = result {
                disconnect.push((*connection, e));
            }
        }

        // retain does not work in async
        for (connection, e) in disconnect {
            Self::disconnect_user_with(
                connection,
                e.into(),
                &self.messages.0,
                &mut peers,
                &mut *self.connections.lock_arc().await,
            )
            .await?;
        }
        Ok(())
    }

    /// Sends a message to a specific target through UDP.
    ///
    /// This function should be used to send messages with the lowest latency possible.
    pub async fn udp_send(&self, receiver: Connection, message: &Msg) -> Result<()> {
        let mut peers = self.connections_map.lock_arc().await;
        let peer = peers.get_mut(&receiver).ok_or(anyhow!("User not found"))?;
        let result = self
            .udp_socket
            .send_to(
                &super::serialize_udp(peer.order_number(), message)?,
                receiver.udp_addr,
            )
            .await;
        if let Err(e) = result {
            Self::disconnect_user_with(
                receiver,
                e.into(),
                &self.messages.0,
                &mut *self.connections_map.lock_arc().await,
                &mut *self.connections.lock_arc().await,
            )
            .await?;
        }
        Ok(())
    }

    async fn disconnect_user_with(
        user: Connection,
        reason: Disconnected,
        messages: &Sender<(Connection, RemoteMessage<Msg>)>,
        stream_map: &mut HashMap<Connection, Peer>,
        connections: &mut HashMap<SocketAddr, Connection>,
    ) -> Result<()> {
        messages
            .send((user, RemoteMessage::Disconnected(reason)))
            .await?;
        let connection = stream_map.remove(&user).ok_or(anyhow!("User not found"))?;
        connections.remove(&user.tcp_addr);
        connections.remove(&user.udp_addr);

        connection.tcp_stream.shutdown(std::net::Shutdown::Both)?;

        Ok(())
    }

    /// Disconnects the specified user.
    pub async fn disconnect_user(&self, user: Connection, reason: Disconnected) -> Result<()> {
        self.messages
            .0
            .send((user, RemoteMessage::Disconnected(reason)))
            .await?;
        let connection = self
            .connections_map
            .lock()
            .await
            .remove(&user)
            .ok_or(anyhow!("User not found"))?;
        self.connections.lock_arc().await.remove(&user.tcp_addr);
        self.connections.lock_arc().await.remove(&user.udp_addr);

        connection.tcp_stream.shutdown(std::net::Shutdown::Both)?;

        Ok(())
    }

    /// Returns a list of all connections currently initiated with the server.
    pub async fn connections(&self) -> Vec<Connection> {
        self.connections_map.lock().await.keys().cloned().collect()
    }
}
