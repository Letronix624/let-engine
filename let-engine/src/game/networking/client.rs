use std::net::SocketAddr;

use anyhow::Result;
use async_std::{
    channel::unbounded,
    io::{ReadExt, WriteExt},
    net::{TcpStream, UdpSocket},
    sync::{Arc, Mutex},
    task,
};
use crossbeam::atomic::AtomicCell;
use thiserror::Error;

use serde::{Deserialize, Serialize};

use super::{Messages, RemoteMessage};

/// A client instance that allows you to connect to a server using the same game engine
/// and send/receive messages.
#[derive(Clone)]
pub struct GameClient<Msg>
where
    for<'a> Msg: Send + Sync + Serialize + Deserialize<'a> + 'static,
{
    client: Arc<Mutex<Option<TcpStream>>>,
    udp_socket: Arc<UdpSocket>,
    messages: Messages<Msg>,
    remote_addr: Arc<AtomicCell<SocketAddr>>,
}

impl<Msg> GameClient<Msg>
where
    for<'a> Msg: Send + Sync + Serialize + Deserialize<'a> + Clone + 'static,
{
    pub(crate) fn new(remote_addr: SocketAddr) -> Result<Self> {
        task::block_on(async {
            let udp_socket = Arc::new(UdpSocket::bind("127.0.0.1:0").await?);
            let client = Self {
                client: Arc::new(Mutex::new(None)),
                udp_socket,
                messages: unbounded(),
                remote_addr: Arc::new(AtomicCell::new(remote_addr)),
            };

            client.recv_udp_messages();

            Ok(client)
        })
    }

    fn recv_messages(&self) {
        let client = self.client.clone();
        let messages = self.messages.0.clone();

        let remote_addr = self.remote_addr.clone();
        task::spawn(async {
            let remote_addr = remote_addr;
            let remote_addr = remote_addr.load();
            let messages = messages;
            let client = client;

            let Some(mut client) = client.lock_arc().await.clone() else {
                return;
            };

            let mut buf = [0; 1024];
            // let mut data = Vec::with_capacity(1024);

            while let Ok(size) = client.read(&mut buf).await {
                if size == 0 {
                    break;
                };

                let Ok(message) = bincode::deserialize(&buf[..size]) else {
                    continue;
                };

                if messages
                    .send((remote_addr, RemoteMessage::Tcp(message)))
                    .await
                    .is_err()
                {
                    break;
                };
            }
            let _ = messages
                .send((remote_addr, RemoteMessage::Disconnected))
                .await;
        });
    }

    fn recv_udp_messages(&self) {
        let udp_socket = self.udp_socket.clone();
        let messages = self.messages.0.clone();
        let remote_addr = self.remote_addr.clone();
        task::spawn(async {
            let udp_socket = udp_socket;
            let messages = messages;
            let remote_addr = remote_addr;

            let mut buf = [0; 1024];

            'h: loop {
                while let Ok((size, addr)) = udp_socket.recv_from(&mut buf).await {
                    if addr != remote_addr.load() {
                        continue;
                    };

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
        });
    }

    pub(crate) async fn receive_messages(&self) -> Vec<(SocketAddr, RemoteMessage<Msg>)> {
        let mut messages: Vec<(SocketAddr, RemoteMessage<Msg>)> = vec![];
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
        self.remote_addr.load()
    }

    /// Changes the remote address of the server to connect to, but does not immediately connect to it
    /// when the server is still running.
    pub fn set_remote_addr(&self, addr: SocketAddr) {
        self.remote_addr.store(addr);
    }

    /// Connects to the servers remote address.
    pub async fn connect(&self) -> Result<(), ClientError> {
        if self.client.lock_arc().await.is_some() {
            return Err(ClientError::StillConnected);
        }
        let client = TcpStream::connect(self.remote_addr.load())
            .await
            .map_err(ClientError::Io)?;
        *self.client.lock_arc().await = Some(client);
        self.recv_messages();
        Ok(())
    }

    /// Stops the connection to the server.
    pub async fn disconnect(&self) -> Result<(), ClientError> {
        if let Some(client) = self.client.lock_arc().await.as_ref() {
            client
                .shutdown(std::net::Shutdown::Both)
                .map_err(ClientError::Io)?;
        } else {
            return Err(ClientError::NotConnected);
        };
        *self.client.lock_arc().await = None;

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
                .write_all(&bincode::serialize(message).map_err(ClientError::Bincode)?)
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
            .send_to(
                &bincode::serialize(message).map_err(ClientError::Bincode)?,
                self.remote_addr(),
            )
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
    Io(async_std::io::Error),
    #[error("An unexplainable error has occured.")]
    Bincode(Box<bincode::ErrorKind>),
}
