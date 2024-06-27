use std::str::FromStr;

use ahash::HashMap;
use anyhow::{anyhow, Result};
use async_std::{
    channel::{unbounded, Receiver, Sender},
    io::{ReadExt, WriteExt},
    net::{Ipv4Addr, SocketAddr, SocketAddrV4, TcpListener, TcpStream, UdpSocket},
    sync::{Arc, Mutex},
    task,
};
use serde::{Deserialize, Serialize};

use super::RemoteMessage;

/// A server instance that allows you to send messages to your client.
#[derive(Clone)]
pub struct GameServer<Msg>
where
    for<'a> Msg: Send + Sync + Serialize + Deserialize<'a> + Clone,
{
    udp_socket: Arc<UdpSocket>,
    connections: Arc<Mutex<HashMap<SocketAddr, TcpStream>>>,
    messages: Messages<Msg>,
}

type Messages<Msg> = (
    Sender<(SocketAddr, RemoteMessage<Msg>)>,
    Receiver<(SocketAddr, RemoteMessage<Msg>)>,
);

impl<Msg> GameServer<Msg>
where
    for<'a> Msg: Send + Sync + Serialize + Deserialize<'a> + Clone + 'static,
{
    /// Creates a new server using the given address.
    pub(crate) fn new(addr: SocketAddr) -> Result<Self> {
        task::block_on(async {
            let tcp_listener = TcpListener::bind(addr).await?;

            let udp_socket = Arc::new(UdpSocket::bind(addr).await?);

            let mut server = Self {
                udp_socket,
                connections: Arc::new(Mutex::new(HashMap::default())),
                messages: unbounded(),
            };

            server.accept_connetions(tcp_listener);
            server.recv_udp_messages();

            Ok(server)
        })
    }

    pub(crate) async fn stop(&mut self) -> Result<()> {
        let connections = std::mem::take(&mut *self.connections.lock_arc().await);
        for connection in connections.into_values() {
            connection.shutdown(std::net::Shutdown::Both)?;
        }

        Ok(())
    }

    /// Creates a new server only accessable on this machine with the given port.
    pub(crate) fn new_local(port: u16) -> Result<Self> {
        let addr = SocketAddrV4::new(Ipv4Addr::new(127, 0, 0, 1), port);
        Self::new(addr.into())
    }

    /// Creates a new server accessable by every device trying to connect with the given port.
    pub(crate) fn new_public(port: u16) -> Result<Self> {
        let addr = local_ip_addr::get_local_ip_address()?;
        let addr = SocketAddr::new(Ipv4Addr::from_str(&addr)?.into(), port);
        Self::new(addr)
    }

    pub(crate) fn accept_connetions(&mut self, listener: TcpListener) {
        let messages = self.messages.0.clone();
        let connections = self.connections.clone();
        task::spawn(async {
            let messages = messages;
            let connections = connections;
            let listener = listener;
            while let Ok((stream, addr)) = listener.accept().await {
                let stream2 = stream.clone();
                if messages
                    .send((addr, RemoteMessage::Connected))
                    .await
                    .is_ok()
                {
                    task::spawn(Self::recv_messages(stream2, addr, messages.clone()));
                    connections.lock_arc().await.insert(addr, stream);
                }
            }
        });
    }

    pub(crate) fn recv_udp_messages(&mut self) {
        let messages = self.messages.0.clone();
        let connections = self.connections.clone();
        let udp_socket = self.udp_socket.clone();
        task::spawn(async {
            let messages = messages;
            let connections = connections;
            let udp_socket = udp_socket;
            loop {
                let mut buf = [0; 1024];

                if let Ok((size, addr)) = udp_socket.recv_from(&mut buf).await {
                    if connections.lock().await.contains_key(&addr) {
                        if let Ok(message) = bincode::deserialize::<Msg>(&buf[..size]) {
                            if messages
                                .send((addr, RemoteMessage::Udp(message)))
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
        addr: SocketAddr,
        messages: Sender<(SocketAddr, RemoteMessage<Msg>)>,
    ) {
        let messages = messages;
        loop {
            let mut buf = [0u8; 1024];
            let size = stream.read(&mut buf).await;

            if let Ok(size) = size {
                if size == 0 {
                    if messages
                        .send((addr, RemoteMessage::Disconnected))
                        .await
                        .is_err()
                    {
                        break;
                    }
                    break;
                }
                if let Ok(message) = bincode::deserialize::<Msg>(&buf[..size]) {
                    if messages
                        .send((addr, RemoteMessage::Tcp(message)))
                        .await
                        .is_err()
                    {
                        break;
                    }
                }
            } else {
                if messages
                    .send((addr, RemoteMessage::Disconnected))
                    .await
                    .is_err()
                {
                    break;
                }
                break;
            }
        }
    }

    pub(crate) async fn receive_messages(&mut self) -> Vec<(SocketAddr, RemoteMessage<Msg>)> {
        let mut messages: Vec<(SocketAddr, RemoteMessage<Msg>)> = vec![];
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
    /// Broadcasts a message to every client through TCP.
    ///
    /// This function should be used to broadcast important messages.
    pub async fn broadcast(&mut self, message: &Msg) -> Result<()> {
        for connection in self.connections.lock_arc().await.values_mut() {
            connection.write_all(&bincode::serialize(message)?).await?;
        }
        Ok(())
    }

    /// Sends a message to a specific target through TCP.
    ///
    /// This function should be used to send important messages.
    pub async fn send(&mut self, receiver: SocketAddr, message: &Msg) -> Result<()> {
        self.connections
            .lock_arc()
            .await
            .get_mut(&receiver)
            .ok_or(anyhow!("Receiver does not exist"))?
            .write_all(&bincode::serialize(message)?)
            .await?;
        Ok(())
    }

    /// Broadcasts a message to every client through UDP.
    ///
    /// This function should be used to broadcast messages with the lowest latency possible.
    pub async fn udp_broadcast(&mut self, message: &Msg) -> Result<()> {
        for connection in self.connections.lock_arc().await.keys() {
            self.udp_socket
                .send_to(&bincode::serialize(message)?, connection)
                .await?;
        }
        Ok(())
    }

    /// Sends a message to a specific target through UDP.
    ///
    /// This function should be used to send messages with the lowest latency possible.
    pub async fn udp_send(&self, receiver: SocketAddr, message: &Msg) -> Result<()> {
        self.udp_socket
            .send_to(&bincode::serialize(message)?, receiver)
            .await?;
        Ok(())
    }

    /// Disconnects the specified user.
    pub async fn disconnect_user(&mut self, user: SocketAddr) -> Result<()> {
        self.messages
            .0
            .send((user, RemoteMessage::Disconnected))
            .await?;
        let connection = self
            .connections
            .lock()
            .await
            .remove(&user)
            .ok_or(anyhow!("User not found"))?;

        connection.shutdown(std::net::Shutdown::Both)?;

        Ok(())
    }

    /// Returns a list of all connections currently initiated with the server.
    pub async fn connections(&self) -> Vec<SocketAddr> {
        self.connections.lock().await.keys().cloned().collect()
    }
}
