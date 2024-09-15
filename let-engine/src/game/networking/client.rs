use std::{
    net::SocketAddr,
    sync::{
        atomic::{AtomicBool, AtomicU32},
        Arc,
    },
    time::{Duration, SystemTime},
};

use anyhow::Result;
use crossbeam::atomic::AtomicCell;
use futures::future::Either;
use rand::Rng;
use smol::{
    channel::{unbounded, Sender},
    io::{AsyncReadExt, AsyncWriteExt},
    lock::Mutex,
    net::{TcpStream, UdpSocket},
    Timer,
};
use thiserror::Error;

use serde::{Deserialize, Serialize};

use crate::SETTINGS;

use super::{serialize_tcp, Connection, Disconnected, Messages, RemoteMessage};

struct Socket {
    client: Mutex<Option<TcpStream>>,

    connected: AtomicBool,
    udp_socket: UdpSocket,
    udp_order: AtomicU32,

    remote_connection: AtomicCell<Connection>,

    ping_timestamp: AtomicCell<Option<SystemTime>>,
    ping: AtomicCell<Duration>,
}

impl Socket {
    /// Sends the first ping message
    async fn start_ping(&self) {
        // send 8 byte message to be echoed
        let _ = self.udp_socket.send(&[0; 8]).await;
        self.ping_timestamp.store(Some(SystemTime::now()));
    }

    /// Sends the second ping message and records time.
    async fn stop_ping(&self) {
        // send 8 byte echo back for the server to calculate the ping.
        let _ = self.udp_socket.send(&[0; 8]).await;

        let time = self.ping_timestamp.take();

        if let Some(time) = time {
            self.ping.store(time.elapsed().unwrap());
        }
    }
}

/// A client instance that allows you to connect to a server using the same game engine
/// and send/receive messages.
/// Msg must have Serialize and Deserialize from serde implemented.
#[derive(Clone)]
pub struct GameClient<Msg>
where
    for<'a> Msg: Send + Sync + Serialize + Deserialize<'a> + 'static,
{
    socket: Arc<Socket>,
    pub(crate) messages: Messages<Msg>,
}

impl<Msg> GameClient<Msg>
where
    for<'a> Msg: Send + Sync + Serialize + Deserialize<'a> + Clone + 'static,
{
    pub(crate) fn new(remote_addr: SocketAddr) -> Result<Self> {
        smol::block_on(async {
            let udp_socket = UdpSocket::bind("0.0.0.0:0")
                .await
                .map_err(ClientError::Io)?;

            let client = Self {
                socket: Arc::new(Socket {
                    client: Mutex::new(None),
                    udp_socket,
                    connected: AtomicBool::new(false),
                    udp_order: AtomicU32::new(1),
                    remote_connection: AtomicCell::new(Connection::new(
                        remote_addr,
                        remote_addr.port(),
                    )),
                    ping_timestamp: AtomicCell::new(None),
                    ping: AtomicCell::new(Duration::default()),
                }),
                messages: unbounded(),
            };

            client.recv_udp_messages();
            client.start_pinging();

            Ok(client)
        })
    }

    fn start_pinging(&self) {
        let socket = self.socket.clone();

        smol::spawn(async {
            let socket = socket;

            loop {
                Timer::after(SETTINGS.networking.ping_wait()).await;

                if !socket.connected.load(std::sync::atomic::Ordering::Acquire) {
                    continue;
                }
                if let Some(timestamp) = socket.ping_timestamp.load() {
                    if timestamp.elapsed().unwrap() > Duration::from_secs(10) {
                        socket.start_ping().await;
                    }
                } else {
                    socket.start_ping().await;
                }
            }
        })
        .detach();
    }

    fn recv_messages(&self) {
        let socket = self.socket.clone();
        let messages = self.messages.0.clone();
        smol::spawn(async {
            let socket = socket;
            let messages = messages;
            let connection = socket.remote_connection.load();

            let disconnect_reason;

            let mut size_buf = [0u8; 4];
            let mut buf = Vec::with_capacity(1038);

            loop {
                let mut client = socket.client.lock().await.clone();
                if let Some(stream) = client.as_mut() {
                    buf.clear();

                    // Get u32 size prefix
                    if let Err(e) = stream.read_exact(&mut size_buf).await {
                        disconnect_reason = e.into();
                        break;
                    };

                    // Read as many bytes as in the size prefix
                    let size = u32::from_le_bytes(size_buf) as usize;

                    match size {
                        size if size < 4 => {
                            continue;
                        }
                        size if size > SETTINGS.networking.tcp_size_limit() => {
                            let _ = messages
                                .send((
                                    socket.remote_connection.load(),
                                    RemoteMessage::Warning(super::Misbehaviour::MessageTooBig),
                                ))
                                .await;
                            continue;
                        }
                        _ => (),
                    }
                    buf.resize(size, 0);

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
                                .send((
                                    connection,
                                    RemoteMessage::Warning(
                                        super::Misbehaviour::UnintelligableContent(e),
                                    ),
                                ))
                                .await
                        }
                    };
                }
            }
            Self::disconnect_with(messages, connection, disconnect_reason, &socket.client).await;
            socket
                .connected
                .store(false, std::sync::atomic::Ordering::Release);
        })
        .detach();
    }

    fn recv_udp_messages(&self) {
        let messages = self.messages.0.clone();
        let socket = self.socket.clone();
        smol::spawn(async {
            let socket = socket;
            let messages = messages;

            let mut buf: [u8; 1024] = [0; 1024];

            let mut buffered_message: Option<super::BufferingMessage> = None;

            let mut last_ord = 0;

            loop {
                let Ok(size) = socket.udp_socket.recv(&mut buf).await else {
                    continue;
                };

                if let Some(message) = buffered_message.as_mut() {
                    if !message.outdated() {
                        if let Some(data) = message.completed(&buf[..size]) {
                            Self::submit_udp_message(&messages, &socket.remote_connection, data)
                                .await;
                            buffered_message = None;
                        }
                        continue;
                    }
                    buffered_message = None;
                }

                match size {
                    // 8 bytes = ping
                    8 => {
                        socket.stop_ping().await;
                    }
                    // Ignore messages smaller than the header.
                    size if size < 8 => {
                        continue;
                    }
                    size if size > SETTINGS.networking.udp_size_limit() => {
                        if messages
                            .send((
                                socket.remote_connection.load(),
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

                // Verify order
                match ord {
                    ord if ord == last_ord + 1 => (), // in order -> allow
                    _ => {
                        // out of order -> discard
                        last_ord = ord;
                        continue;
                    }
                }
                last_ord = ord;

                // Get length
                let len = u32::from_le_bytes(buf[4..8].try_into().unwrap()) as usize;

                // Try again if len is over 128MB
                // TODO: Let user choose size limit.
                if len > 128000000 {
                    continue;
                }

                let mut buffering = super::BufferingMessage::new(len);

                if let Some(data) = buffering.completed(&buf[8..]) {
                    Self::submit_udp_message(&messages, &socket.remote_connection, data).await;
                } else {
                    buffered_message = Some(buffering);
                }
            }
        })
        .detach();
    }

    async fn submit_udp_message(
        messages: &Sender<(Connection, RemoteMessage<Msg>)>,
        remote_addr: &AtomicCell<Connection>,
        buf: &[u8],
    ) {
        if let Ok(message) = bincode::deserialize::<Msg>(buf) {
            let _ = messages
                .send((remote_addr.load(), RemoteMessage::Udp(message)))
                .await
                .is_err();
        }
    }

    async fn disconnect_with(
        messages: Sender<(Connection, RemoteMessage<Msg>)>,
        connection: Connection,
        reason: Disconnected,
        client: &Mutex<Option<TcpStream>>,
    ) {
        let client = std::mem::take(&mut *client.lock().await);

        if let Some(client) = client.as_ref() {
            let _ = client.shutdown(std::net::Shutdown::Both);
        };

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
        self.socket.remote_connection.load().tcp_addr
    }

    /// Changes the remote address of the server to connect to, but does not immediately connect to it
    /// when the server is still running.
    pub fn set_remote_addr(&self, addr: SocketAddr) {
        self.socket
            .remote_connection
            .store(Connection::new(addr, addr.port()));
    }

    /// Connects to the servers remote address.
    pub async fn connect(&self) -> Result<(), ClientError> {
        // Error if there is a connection.
        if self.socket.client.lock().await.is_some() {
            return Err(ClientError::StillConnected);
        }

        let addr = self.remote_addr();

        // Use the UdpSocket connect function to allow receiving data without port forwarding or handling the NAT stuff.
        self.socket
            .udp_socket
            .connect(&addr)
            .await
            .map_err(ClientError::Io)?;

        let mut tcp_socket = TcpStream::connect(addr).await.map_err(ClientError::Io)?;

        let mut buf = [0; 128];

        rand::thread_rng().fill(&mut buf[4..]);

        // Send random ID for UDP identification
        tcp_socket
            .write_all(&buf)
            .await
            .map_err(|_| ClientError::ServerFull)?;

        let retries = SETTINGS.networking.auth_retries();
        let wait_time = SETTINGS.networking.auth_retry_wait();

        for _ in 0..retries {
            self.socket
                .udp_socket
                .send(&buf)
                .await
                .map_err(ClientError::Io)?;

            let mut _buf = [0; 8];
            let recv = self.socket.udp_socket.recv(&mut buf);
            let select = futures::future::select(Box::pin(recv), Timer::after(wait_time));

            match select.await {
                Either::Left((result, _)) => {
                    let size = result.map_err(ClientError::Io)?;
                    if size != 8 {
                        return Err(ClientError::InvalidResponse);
                    }
                    break;
                }
                Either::Right(_) => (),
            }
        }

        self.socket
            .connected
            .store(true, std::sync::atomic::Ordering::Release);
        *self.socket.client.lock().await = Some(tcp_socket);
        self.recv_messages();

        Ok(())
    }

    /// Stops the connection to the server.
    pub async fn disconnect(&self) -> Result<(), ClientError> {
        let status = self
            .socket
            .connected
            .swap(false, std::sync::atomic::Ordering::AcqRel);
        if !status {
            return Err(ClientError::NotConnected);
        };
        let mut client = self.socket.client.lock().await;
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
        if let Some(client) = self.socket.client.lock().await.as_mut() {
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
    pub async fn fast_send(&self, message: &Msg) -> Result<(), ClientError> {
        if !self
            .socket
            .connected
            .load(std::sync::atomic::Ordering::Acquire)
        {
            return Err(ClientError::NotConnected);
        }

        let data = super::serialize_udp(
            self.socket
                .udp_order
                .fetch_add(1, std::sync::atomic::Ordering::SeqCst),
            message,
        )
        .map_err(ClientError::Bincode)?;
        let chunks = data.chunks(1024);

        for chunk in chunks {
            self.socket
                .udp_socket
                .send(chunk)
                .await
                .map_err(ClientError::Io)?;
        }

        Ok(())
    }

    /// Returns the last calculated ping of the last running connection.
    ///
    /// May return a duration of 0 in case no calculation has been done before this function.
    pub fn ping(&self) -> Duration {
        self.socket.ping.load()
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
    #[error("The server you attepted to connect to is full.")]
    ServerFull,
    /// The server sends a message invalid to the let-engine interface.
    #[error("The server is sending invalid data.")]
    InvalidResponse,
    #[error("An Io error has occured: {0}")]
    Io(smol::io::Error),
    #[error("An unexplainable error has occured.")]
    Bincode(Box<bincode::ErrorKind>),
}
