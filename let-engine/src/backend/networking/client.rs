use std::{
    future::Future,
    marker::PhantomData,
    net::ToSocketAddrs,
    sync::{Arc, atomic::AtomicU32},
    time::{Duration, Instant},
};

use anyhow::Result;
use futures::future::Either;
use rand::Rng;
use rkyv::{
    Serialize,
    api::high::HighSerializer,
    rancor,
    ser::allocator::{Arena, ArenaHandle},
    util::AlignedVec,
};
use smol::{
    Timer,
    channel::Sender,
    io::{AsyncReadExt, AsyncWriteExt},
    lock::Mutex,
    net::{TcpStream, UdpSocket},
};
use thiserror::Error;

use super::{Connection, Disconnected, NetworkingSettings, SAFE_MTU_SIZE, Warning};

struct Socket {
    client: Mutex<Option<TcpStream>>,

    udp_socket: UdpSocket,
    udp_order: AtomicU32,

    remote_connection: Mutex<Option<Connection>>,

    arena: Arc<parking_lot::Mutex<Arena>>,

    ping: Mutex<Ping>,
}

struct Ping {
    timestamp: Option<Instant>,
    ping: Duration,
}

impl Socket {
    /// Sends the first ping message
    async fn start_ping(&self) {
        // send 8 byte message to be echoed
        let mut ping = self.ping.lock().await;

        let _ = self.udp_socket.send(&[0; 8]).await;
        ping.timestamp = Some(Instant::now());
    }

    /// Sends the second ping message and records time.
    async fn stop_ping(&self) {
        // send 8 byte echo back for the server to calculate the ping.
        let mut ping = self.ping.lock().await;

        let _ = self.udp_socket.send(&[0; 8]).await;

        if let Some(timestamp) = ping.timestamp.take() {
            ping.ping = timestamp.elapsed();
        }
    }
}

pub(super) enum ClientMessage {
    Error(ClientError),
    Warning(Warning),
    Tcp(Vec<u8>),
    Udp(Vec<u8>),
    Connected,
    Disconnected(Disconnected),
}

/// A client instance that allows you to connect to a server using the same game engine
/// and send/receive messages.
pub struct ClientInterface<Msg> {
    socket: Arc<Socket>,
    messages: Sender<ClientMessage>,
    settings: NetworkingSettings,
    _msg: PhantomData<Msg>,
}

impl<Msg> Clone for ClientInterface<Msg> {
    fn clone(&self) -> Self {
        Self {
            socket: self.socket.clone(),
            messages: self.messages.clone(),
            settings: self.settings.clone(),
            _msg: PhantomData,
        }
    }
}

impl<Msg> let_engine_core::backend::networking::ClientInterface<Connection> for ClientInterface<Msg>
where
    Msg:
        Send + Sync + for<'a> Serialize<HighSerializer<AlignedVec, ArenaHandle<'a>, rancor::Error>>,
{
    type Msg = Msg;
    type Error = ClientError;

    /// Starts connecting and sends a message back if connecting succeeds.
    fn connect<Addr: ToSocketAddrs>(&self, addr: Addr) -> std::result::Result<(), Self::Error> {
        let addr = addr
            .to_socket_addrs()
            .map_err(ClientError::Io)?
            .next()
            .unwrap();

        smol::block_on(async {
            // Error if there is a connection.
            if self.socket.client.lock().await.is_some() {
                return Err(ClientError::StillConnected);
            }
            Ok(())
        })?;
        let socket = self.socket.clone();
        let settings = self.settings.clone();
        let messages = self.messages.clone();

        self.spawn(async move {
            let inf = Self {
                socket,
                settings,
                messages,
                _msg: PhantomData,
            };

            // Use the UdpSocket connect function to allow receiving data without port forwarding or handling the NAT stuff.
            inf.socket
                .udp_socket
                .connect(addr)
                .await
                .map_err(ClientError::Io)?;

            let mut client = inf.socket.client.lock().await;

            let mut tcp_socket = TcpStream::connect(addr).await.map_err(ClientError::Io)?;

            let mut buf = [0; 128];

            rand::rng().fill(&mut buf[4..]);

            // Send random ID for UDP identification
            tcp_socket
                .write_all(&buf)
                .await
                .map_err(|_| ClientError::ServerFull)?;

            let retries = inf.settings.auth_retries;
            let wait_time = inf.settings.auth_retry_wait;

            let mut fail = true;

            for i in 0..retries {
                inf.socket
                    .udp_socket
                    .send(&buf)
                    .await
                    .map_err(ClientError::Io)?;

                let mut _buf = [0; 8];
                let recv = inf.socket.udp_socket.recv(&mut buf);
                let select = futures::future::select(Box::pin(recv), Timer::after(wait_time));

                match select.await {
                    Either::Left((result, _)) => {
                        let size = result.map_err(ClientError::Io)?;
                        if size != 8 {
                            return Err(ClientError::InvalidResponse);
                        }
                        fail = false;
                        break;
                    }
                    Either::Right(_) => {
                        inf.messages
                            .send(ClientMessage::Warning(Warning::Retry(i + 1)))
                            .await
                            .unwrap();
                    }
                }
            }

            if fail {
                return Err(ClientError::InvalidResponse);
            }

            *client = Some(tcp_socket);

            inf.recv_messages();
            inf.recv_udp_messages();
            inf.start_pinging();

            Ok(Some(ClientMessage::Connected))
        });

        Ok(())
    }

    fn disconnect(&self) -> std::result::Result<(), Self::Error> {
        let socket = self.socket.clone();
        smol::spawn(async {
            let socket = socket;
            Self::disconnect_with(&socket.client).await;
        })
        .detach();

        Ok(())
    }

    fn peer_conn(&self) -> Option<Connection> {
        *self.socket.remote_connection.lock_blocking()
    }

    fn local_conn(&self) -> Option<Connection> {
        let client = self.socket.client.lock_blocking();
        client.as_ref().map(|client| {
            let addr = client.local_addr().unwrap();
            Connection {
                ip: addr.ip(),
                tcp_port: addr.port(),
                udp_port: self.socket.udp_socket.local_addr().unwrap().port(),
            }
        })
    }

    fn send(&self, message: &Self::Msg) -> std::result::Result<(), Self::Error> {
        let socket = self.socket.clone();

        let data = {
            let mut arena = socket.arena.lock();
            super::serialize_tcp_into(message, &mut arena)
        };

        self.spawn(async move {
            let mut client = socket.client.lock().await;
            if let Some(client) = client.as_mut() {
                client
                    .write_all(&data)
                    .await
                    .map(|_| None)
                    .map_err(ClientError::Io)
            } else {
                Err(ClientError::NotConnected)
            }
        });

        Ok(())
    }

    fn fast_send(&self, message: &Self::Msg) -> std::result::Result<(), Self::Error> {
        let socket = self.socket.clone();
        {
            if socket.client.lock_blocking().is_none() {
                return Err(ClientError::NotConnected);
            }
        }
        let data = {
            let mut arena = socket.arena.lock();
            let mut data = super::serialize_udp_into(message, &mut arena);

            let order_number = socket
                .udp_order
                .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            data[0..4].copy_from_slice(&order_number.to_le_bytes());
            data
        };

        self.spawn(async move {
            for chunk in data.chunks(SAFE_MTU_SIZE) {
                socket
                    .udp_socket
                    .send(chunk)
                    .await
                    .map_err(ClientError::Io)?;
            }

            Ok(None)
        });

        Ok(())
    }
}

impl<Msg> ClientInterface<Msg> {
    pub(super) fn new(
        settings: NetworkingSettings,
        messages: Sender<ClientMessage>,
        arena: Arc<parking_lot::Mutex<Arena>>,
    ) -> Result<Self, ClientError> {
        smol::block_on(async {
            let udp_socket = UdpSocket::bind("0.0.0.0:0")
                .await
                .map_err(ClientError::Io)?;

            let client = Self {
                socket: Arc::new(Socket {
                    client: Mutex::new(None),
                    udp_socket,
                    udp_order: AtomicU32::new(1),
                    remote_connection: Mutex::new(None),
                    arena,
                    ping: Mutex::new(Ping {
                        timestamp: None,
                        ping: Duration::default(),
                    }),
                }),
                messages,
                settings,
                _msg: PhantomData,
            };

            Ok(client)
        })
    }

    fn spawn(
        &self,
        future: impl Future<Output = Result<Option<ClientMessage>, ClientError>> + Send + 'static,
    ) {
        let sender = self.messages.clone();
        smol::spawn(async move {
            if let Some(message) = match future.await {
                Ok(t) => t,
                Err(e) => Some(ClientMessage::Error(e)),
            } {
                sender.send(message).await.unwrap();
            }
        })
        .detach();
    }

    fn start_pinging(&self) {
        let socket = self.socket.clone();
        let settings = self.settings.clone();

        smol::spawn(async move {
            loop {
                socket.start_ping().await;

                Timer::after(settings.ping_wait).await;
            }
        })
        .detach();
    }

    fn recv_messages(&self) {
        let socket = self.socket.clone();
        let messages = self.messages.clone();
        let settings = self.settings.clone();
        self.spawn(async move {
            let mut disconnect_reason = Disconnected::RemoteShutdown;

            let mut size_buf = [0u8; 4];
            let mut client = socket.client.lock().await.clone();
            while let Some(stream) = client.as_mut() {
                let mut buf = Vec::with_capacity(1038);

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
                    size if size > settings.tcp_max_size => {
                        messages
                            .send(ClientMessage::Warning(super::Warning::MessageTooBig))
                            .await
                            .unwrap();
                        continue;
                    }
                    _ => (),
                }
                buf.resize(size, 0);

                if let Err(e) = stream.read_exact(&mut buf).await {
                    disconnect_reason = e.into();
                    break;
                };

                messages.send(ClientMessage::Tcp(buf)).await.unwrap();
            }

            Self::disconnect_with(&socket.client).await;
            Ok(Some(ClientMessage::Disconnected(disconnect_reason)))
        });
    }

    fn recv_udp_messages(&self) {
        let messages = self.messages.clone();
        let socket = self.socket.clone();
        let settings = self.settings.clone();
        smol::spawn(async move {
            let mut buf: [u8; SAFE_MTU_SIZE] = [0; SAFE_MTU_SIZE];

            let mut buffered_message: Option<super::BufferingMessage> = None;

            let mut last_ord = 0;

            while let Ok(size) = socket.udp_socket.recv(&mut buf).await {
                if let Some(mut message) = buffered_message.take()
                    && !message.outdated()
                {
                    if message.completed(&buf[..size]) {
                        messages
                            .send(ClientMessage::Udp(message.consume()))
                            .await
                            .unwrap();
                    } else {
                        buffered_message = Some(message);
                    }
                    continue;
                }
                buffered_message = None;

                match size {
                    // 8 bytes = ping
                    8 => {
                        socket.stop_ping().await;
                    }
                    // Ignore messages smaller than the header.
                    size if size < 8 => {
                        continue;
                    }
                    size if size > settings.udp_max_size => {
                        messages
                            .send(ClientMessage::Warning(super::Warning::MessageTooBig))
                            .await
                            .unwrap();
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

                // Try again if size is above set limit
                if len > settings.udp_max_size {
                    continue;
                }

                let mut buffering = super::BufferingMessage::new(len);

                if buffering.completed(&buf[8..]) {
                    messages
                        .send(ClientMessage::Udp(buffering.consume()))
                        .await
                        .unwrap();
                } else {
                    buffered_message = Some(buffering);
                }
            }
        })
        .detach();
    }

    async fn disconnect_with(client: &Mutex<Option<TcpStream>>) {
        let client = std::mem::take(&mut *client.lock().await);

        if let Some(client) = client.as_ref() {
            let _ = client.shutdown(std::net::Shutdown::Both);
        };
    }

    /// Returns the last calculated ping of the last running connection.
    ///
    /// May return a duration of 0 in case no calculation has been done before this function.
    pub fn ping(&self) -> Duration {
        self.socket.ping.lock_blocking().ping
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
}
