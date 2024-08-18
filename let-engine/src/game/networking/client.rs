use std::{net::SocketAddr, sync::Arc};

use anyhow::Result;
use crossbeam::atomic::AtomicCell;
use smol::{
    channel::{unbounded, Sender},
    io::{AsyncReadExt, AsyncWriteExt},
    lock::Mutex,
    net::{TcpStream, UdpSocket},
};
use thiserror::Error;

use serde::{Deserialize, Serialize};

use super::{serialize_tcp, Connection, Disconnected, Messages, RemoteMessage};

/// A client instance that allows you to connect to a server using the same game engine
/// and send/receive messages.
#[derive(Clone)]
pub struct GameClient<Msg>
where
    for<'a> Msg: Send + Sync + Serialize + Deserialize<'a> + 'static,
{
    client: Arc<Mutex<Option<TcpStream>>>,
    udp_socket: Arc<UdpSocket>,
    pub(crate) messages: Messages<Msg>,
    remote_connection: Arc<AtomicCell<Connection>>,
}

impl<Msg> GameClient<Msg>
where
    for<'a> Msg: Send + Sync + Serialize + Deserialize<'a> + Clone + 'static,
{
    pub(crate) fn new(remote_addr: SocketAddr) -> Result<Self> {
        smol::block_on(async {
            let udp_socket = UdpSocket::bind("0.0.0.0:0")
                .await
                .map_err(ClientError::Io)?
                .into();

            let client = Self {
                client: Arc::new(Mutex::new(None)),
                udp_socket,
                messages: unbounded(),
                remote_connection: Arc::new(AtomicCell::new(Connection::new(
                    remote_addr,
                    remote_addr.port(),
                ))),
            };

            client.recv_udp_messages();

            Ok(client)
        })
    }

    fn recv_messages(&self) {
        let client = self.client.clone();
        let messages = self.messages.0.clone();

        let remote_connection = self.remote_connection.clone();
        smol::spawn(async {
            let connection = remote_connection;
            let connection = connection.load();

            let messages = messages;
            let client = client;

            let disconnect_reason;

            let mut size_buf = [0u8; 4];
            loop {
                let mut client = client.lock_arc().await.clone();
                if let Some(stream) = client.as_mut() {
                    // Get u32 size prefix
                    if let Err(e) = stream.read_exact(&mut size_buf).await {
                        disconnect_reason = e.into();
                        break;
                    };

                    // Read as many bytes as in the size prefix
                    let mut buf = vec![0u8; u32::from_le_bytes(size_buf) as usize];
                    if let Err(e) = stream.read_exact(&mut buf).await {
                        disconnect_reason = e.into();
                        break;
                    };

                    // Send the message if it's correctly deserialized.
                    let _ = match bincode::deserialize::<Msg>(&buf) {
                        Ok(message) => {
                            messages
                                .send((connection, RemoteMessage::Tcp(message)))
                                .await
                        }
                        Err(e) => {
                            messages
                                .send((connection, RemoteMessage::DeserialisationError(e)))
                                .await
                        }
                    };
                }
            }
            Self::disconnect_with(messages, connection, disconnect_reason, client).await;
        })
        .detach();
    }

    fn recv_udp_messages(&self) {
        let udp_socket = self.udp_socket.clone();
        let messages = self.messages.0.clone();
        let remote_addr = self.remote_connection.clone();
        smol::spawn(async {
            let udp_socket = udp_socket;
            let messages = messages;
            let remote_addr = remote_addr;

            let mut buf = [0; 1024];

            'h: loop {
                while let Ok(size) = udp_socket.recv(&mut buf).await {
                    let Ok(message) = bincode::deserialize(&buf[..size]) else {
                        continue;
                    };

                    if messages
                        .send((remote_addr.load(), RemoteMessage::Udp(message)))
                        .await
                        .is_err()
                    {
                        break 'h;
                    };
                }
            }
        })
        .detach();
    }

    async fn disconnect_with(
        messages: Sender<(Connection, RemoteMessage<Msg>)>,
        connection: Connection,
        reason: Disconnected,
        client: Arc<Mutex<Option<TcpStream>>>,
    ) {
        *client.lock_arc().await = None;

        let _ = messages
            .send((connection, RemoteMessage::Disconnected(reason)))
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

impl<Msg> GameClient<Msg>
where
    for<'a> Msg: Send + Sync + Serialize + Deserialize<'a> + Clone,
{
    /// Returns the remote address of the server to connect to.
    pub fn remote_addr(&self) -> SocketAddr {
        self.remote_connection.load().tcp_addr
    }

    /// Changes the remote address of the server to connect to, but does not immediately connect to it
    /// when the server is still running.
    pub fn set_remote_addr(&self, addr: SocketAddr) {
        self.remote_connection
            .store(Connection::new(addr, addr.port()));
    }

    /// Connects to the servers remote address.
    pub async fn connect(&self) -> Result<(), ClientError> {
        // Error if there is a connection.
        if self.client.lock_arc().await.is_some() {
            return Err(ClientError::StillConnected);
        }

        let addr = self.remote_addr();

        // Use the UdpSocket connect function to allow receiving data without port forwarding or handling the NAT stuff.
        self.udp_socket
            .connect(&addr)
            .await
            .map_err(ClientError::Io)?;

        let mut tcp_socket = TcpStream::connect(addr).await.map_err(ClientError::Io)?;

        // Send UDP port for identification
        tcp_socket
            .write_all(
                &self
                    .udp_socket
                    .local_addr()
                    .map_err(ClientError::Io)?
                    .port()
                    .to_le_bytes(),
            )
            .await
            .map_err(ClientError::Io)?;

        *self.client.lock_arc().await = Some(tcp_socket);
        self.recv_messages();
        Ok(())
    }

    /// Stops the connection to the server.
    pub async fn disconnect(&self) -> Result<(), ClientError> {
        let mut client = self.client.lock_arc().await;
        if let Some(client) = client.as_ref() {
            client
                .shutdown(std::net::Shutdown::Both)
                .map_err(ClientError::Io)?;
        } else {
            return Err(ClientError::NotConnected);
        };
        *client = None;

        Ok(())
    }

    /// Sends a message to the server using TCP.
    ///
    /// Recommended over UDP when the reliability of the delivery of the message is more important than speed.
    ///
    /// # TCP
    /// - TCP makes sure that your packets arrive. If one or two get lost, it automatically retransmits them.
    /// - TCP has error correction checking the data integrity.
    /// - It's more reliable for the price of higher latency.
    ///
    /// ## Use Cases
    /// - Chat messages
    ///
    ///   they need to be sent reliably and in order. If a message is lost it probably causes confusion in the conversation.
    ///
    ///
    /// - Game State Updates
    ///
    ///   sending information critical to the game state like player health updates, inventory changes or game events
    ///   like picking up an item. This is important to ensure that all players have the same view of the game state.
    ///
    ///
    /// - Player Actions
    ///
    ///   sending actions like pressing a button, opening a door, triggering a skill.
    pub async fn send(&self, message: &Msg) -> Result<(), ClientError> {
        if let Some(client) = self.client.lock_arc().await.as_mut() {
            client
                .write_all(&serialize_tcp(message).map_err(ClientError::Bincode)?)
                .await
                .map_err(ClientError::Io)?;
        } else {
            return Err(ClientError::NotConnected);
        }
        Ok(())
    }

    /// Sends a message to the server using UDP.
    ///
    /// Recommended over TCP when speed and latency is important, even when packet loss can occur.
    ///
    /// # UDP
    /// - UDP does not guarantee packet delivery and things like that. That makes it much faster and suiable
    ///
    ///   for real-time applications.
    /// - Because UDP does not retransmit lost packets, there is no guarantee that all packets arrive or arrive in order.
    ///
    /// ## Use Cases
    /// - Movement data updates of the player
    ///
    ///   losing a few packets of this data is acceptable because the next packet will update the position anyway.
    ///
    ///
    /// - Realtime State Updates
    ///
    ///   object position updates, NPCs or projectiles need to be updated frequently. Just like the player movement,
    ///   losing a few packets should not be critical.
    ///
    ///
    /// - Video/Audio Streaming
    ///
    ///   transmitting live video or voice chat audio streams require low latency.
    ///   Losing a packet or two might cause a noticable glitch, but it should run smoothly overall.
    pub async fn udp_send(&self, message: &Msg) -> Result<(), ClientError> {
        self.udp_socket
            .send(&bincode::serialize(message).map_err(ClientError::Bincode)?)
            .await
            .map_err(ClientError::Io)?;

        Ok(())
    }
}

/// Errors of the client.
#[derive(Debug, Error)]
pub enum ClientError {
    /// An error that gets output whenever a function that requires the server to be connected
    #[error("The client is still connected to the server.")]
    StillConnected,
    #[error("The client is not connected to any server.")]
    NotConnected,
    #[error("An Io error has occured: {0}")]
    Io(smol::io::Error),
    #[error("An unexplainable error has occured.")]
    Bincode(Box<bincode::ErrorKind>),
}
