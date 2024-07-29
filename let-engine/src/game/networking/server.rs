use std::sync::atomic::AtomicBool;

use ahash::HashMap;
use anyhow::{anyhow, Result};
use async_std::{
    channel::{unbounded, Sender},
    io::{ReadExt, WriteExt},
    net::{SocketAddr, TcpListener, TcpStream, UdpSocket},
    sync::{Arc, Mutex},
    task,
};
use serde::{Deserialize, Serialize};

use super::{serialize_tcp, Connection, Disconnected, Messages, RemoteMessage};

/// A server instance that allows you to send messages to your client.
#[derive(Clone)]
pub struct GameServer<Msg>
where
    for<'a> Msg: Send + Sync + Serialize + Deserialize<'a>,
{
    udp_socket: Arc<UdpSocket>,
    stream_map: Arc<Mutex<HashMap<Connection, TcpStream>>>,
    connections: Arc<Mutex<HashMap<SocketAddr, Connection>>>,
    messages: Messages<Msg>,
    running: Arc<AtomicBool>,
}

impl<Msg> GameServer<Msg>
where
    for<'a> Msg: Send + Sync + Serialize + Deserialize<'a> + Clone + 'static,
{
    /// Creates a new server using the given address.
    pub(crate) fn new(addr: SocketAddr) -> Result<Self> {
        task::block_on(async {
            let tcp_listener = TcpListener::bind(addr).await?;

            let udp_socket = Arc::new(UdpSocket::bind(addr).await?);

            let server = Self {
                udp_socket,
                stream_map: Mutex::new(HashMap::default()).into(),
                connections: Mutex::new(HashMap::default()).into(),
                messages: unbounded(),
                running: Arc::new(false.into()),
            };

            server.accept_connetions(tcp_listener);
            server.recv_udp_messages();

            Ok(server)
        })
    }

    fn accept_connetions(&self, listener: TcpListener) {
        let messages = self.messages.0.clone();
        let connections = self.connections.clone();
        let tcp_map = self.stream_map.clone();
        let running = self.running.clone();
        task::spawn(async {
            let messages = messages;
            let tcp_map = tcp_map;
            let connections = connections;
            let listener = listener;
            let running = running;
            while let Ok((mut stream, addr)) = listener.accept().await {
                let mut buf = [0; 2];

                let op = stream.read(&mut buf);

                if async_std::future::timeout(std::time::Duration::from_secs(2), op)
                    .await
                    .is_err()
                {
                    return;
                };
                if buf == [0; 2] {
                    return;
                }

                let port = u16::from_le_bytes(buf);

                let connection = Connection::new(addr, port);

                if running.load(std::sync::atomic::Ordering::Acquire)
                    && messages
                        .clone()
                        .send((connection, RemoteMessage::Connected))
                        .await
                        .is_ok()
                {
                    let stream2 = stream.clone();
                    let tcp_map2 = tcp_map.clone();
                    let connections2 = connections.clone();
                    let messages2 = messages.clone();
                    task::spawn(async move {
                        let stream = stream2;
                        let tcp_map = tcp_map2;
                        let connections = connections2;
                        let messages = messages2;
                        Self::recv_messages(
                            stream,
                            connection,
                            messages.clone(),
                            tcp_map,
                            connections,
                        )
                        .await;
                    });
                    tcp_map.lock_arc().await.insert(connection, stream);

                    let mut connections = connections.lock_arc().await;

                    connections.insert(connection.tcp_addr(), connection);
                    connections.insert(connection.udp_addr(), connection);
                }
            }
        });
    }

    fn recv_udp_messages(&self) {
        let messages = self.messages.0.clone();
        let connections = self.connections.clone();
        let udp_socket = self.udp_socket.clone();
        let running = self.running.clone();
        task::spawn(async {
            let messages = messages;
            let connections = connections;
            let udp_socket = udp_socket;
            let running = running;
            loop {
                let mut buf = [0; 1024];

                if let Ok((size, addr)) = udp_socket.recv_from(&mut buf).await {
                    if !running.load(std::sync::atomic::Ordering::Acquire) {
                        if messages.is_closed() {
                            break;
                        }
                        continue;
                    }
                    if let Some(connection) = connections.lock().await.get(&addr) {
                        if let Ok(message) = bincode::deserialize::<Msg>(&buf[..size]) {
                            if messages
                                .send((*connection, RemoteMessage::Udp(message)))
                                .await
                                .is_err()
                            {
                                break;
                            };
                        }
                    }
                }
            }
        });
    }

    /// Receives messages from each TCP connection.
    async fn recv_messages(
        mut stream: TcpStream,
        addr: Connection,
        messages: Sender<(Connection, RemoteMessage<Msg>)>,
        stream_map: Arc<Mutex<HashMap<Connection, TcpStream>>>,
        connections: Arc<Mutex<HashMap<SocketAddr, Connection>>>,
    ) {
        let disconnect_reason;
        let mut size_buf = [0u8; 4];
        loop {
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

            // Read as many bytes as in the size prefix
            let mut buf = vec![0u8; size];
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
        let connections = std::mem::take(&mut *self.stream_map.lock_arc().await);
        for connection in connections.into_values() {
            connection.shutdown(std::net::Shutdown::Both)?;
        }
        *self.connections.lock_arc().await = HashMap::default();

        Ok(())
    }

    /// Starts the server up.
    pub fn start(&self) {
        self.running
            .store(true, std::sync::atomic::Ordering::Release);
    }

    /// Broadcasts a message to every client through TCP.
    ///
    /// This function should be used to broadcast important messages.
    pub async fn broadcast(&self, message: &Msg) -> Result<()> {
        let mut stream_map = self.stream_map.lock_arc().await;
        for (user, connection) in stream_map.clone().iter_mut() {
            let result = connection.write_all(&serialize_tcp(&message)?).await;
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
            .stream_map
            .lock_arc()
            .await
            .get_mut(&receiver)
            .ok_or(anyhow!("Receiver does not exist"))?
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
        let mut stream_map = self.stream_map.lock_arc().await;
        for connection in stream_map.clone().keys() {
            let result = self
                .udp_socket
                .send_to(&bincode::serialize(message)?, connection.udp_addr)
                .await;
            if let Err(e) = result {
                Self::disconnect_user_with(
                    *connection,
                    e.into(),
                    &self.messages.0,
                    &mut stream_map,
                    &mut *self.connections.lock_arc().await,
                )
                .await?;
            }
        }
        Ok(())
    }

    /// Sends a message to a specific target through UDP.
    ///
    /// This function should be used to send messages with the lowest latency possible.
    pub async fn udp_send(&self, receiver: Connection, message: &Msg) -> Result<()> {
        let result = self
            .udp_socket
            .send_to(&bincode::serialize(message)?, receiver.udp_addr)
            .await;
        if let Err(e) = result {
            Self::disconnect_user_with(
                receiver,
                e.into(),
                &self.messages.0,
                &mut *self.stream_map.lock_arc().await,
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
        stream_map: &mut HashMap<Connection, TcpStream>,
        connections: &mut HashMap<SocketAddr, Connection>,
    ) -> Result<()> {
        messages
            .send((user, RemoteMessage::Disconnected(reason)))
            .await?;
        let connection = stream_map.remove(&user).ok_or(anyhow!("User not found"))?;
        connections.remove(&user.tcp_addr);
        connections.remove(&user.udp_addr);

        connection.shutdown(std::net::Shutdown::Both)?;

        Ok(())
    }

    /// Disconnects the specified user.
    pub async fn disconnect_user(&self, user: Connection, reason: Disconnected) -> Result<()> {
        self.messages
            .0
            .send((user, RemoteMessage::Disconnected(reason)))
            .await?;
        let connection = self
            .stream_map
            .lock()
            .await
            .remove(&user)
            .ok_or(anyhow!("User not found"))?;
        self.connections.lock_arc().await.remove(&user.tcp_addr);
        self.connections.lock_arc().await.remove(&user.udp_addr);

        connection.shutdown(std::net::Shutdown::Both)?;

        Ok(())
    }

    /// Returns a list of all connections currently initiated with the server.
    pub async fn connections(&self) -> Vec<Connection> {
        self.stream_map.lock().await.keys().cloned().collect()
    }
}
