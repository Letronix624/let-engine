//! Networking, server and client ablilities built in the game engine.
//!
//! Networking through the public internet requires port forwarding the same TCP and UDP port.

// Formats
//
// # TCP
//
// TCP can only send 2 kinds of messages: Auth messages and Data messages.
//
// Auth messages are made out of 128 random bytes, where the first 4 bytes are 0. They are the first message that arrives.
//
// Auth messages during a registered connection will be seen as misbehaving peer and disconnected.
//
// Data messages include a 4 byte header with the length prefix and a rest as big as the u32 that comes from the length.
//
// # UDP
//
// UDP has 3 kinds of messages: Auth messages, Ping messages and Data messages.
//
// Auth messages are the same random bytes as TCP and are retried 10 times before giving up the connection.
//
// Auth messages start with 4 bytes made only out of zeros, because zeroes are not valid order numbers
//
// The rest of the messages have a 8 byte header with the first 4 bytes as the order number and the rest as lenght prefix.
//
// A Ping packet also works as the ack auth back message signalling to stop sending the auth message.
//
// It's mainly there to calculate ping and consists of a valid order number and a length of 0, thereby always 8 bytes of data.
//
// A data packet consists of a valid order number, length over 0 and leading data as big as the length number indicates.
//
// To combat UDP fragmentation and corruption there is a order number. Any packet that does not follow the right order will be ignored.
// If another packet arrives with an order not one bigger than the last one, the data will be discarted.
// If a packet arrives with an order number exactly 1 bigger than the last one, it will be kept track of again.
//
// There is a lot of discarting here. Users have to expect that UDP is not perfect and reliable.

mod client;
mod server;

use std::{
    io::{self, ErrorKind},
    net::{IpAddr, SocketAddr},
    time::{Duration, SystemTime},
};

pub use client::*;
use let_engine_core::backend::networking::{NetEvent, NetworkingBackend};
pub use server::*;
use smol::{
    channel::{Receiver, bounded},
    future::race,
};
use thiserror::Error;

const SAFE_MTU_SIZE: usize = 1200;

pub trait NetSerializable
where
    for<'a> Self: Serialize<HighSerializer<AlignedVec, ArenaHandle<'a>, rancor::Error>> + Archive,
    for<'a> Self::Archived: CheckBytes<HighValidator<'a, rancor::Error>> + 'static,
    Self: Send + Sync,
{
}

impl<T> NetSerializable for T
where
    for<'a> T: Serialize<HighSerializer<AlignedVec, ArenaHandle<'a>, rancor::Error>> + Archive,
    for<'a> T::Archived: CheckBytes<HighValidator<'a, rancor::Error>> + 'static,
    T: Send + Sync,
{
}

pub struct DefaultNetworkingBackend<ServerMsg, ClientMsg> {
    server_interface: server::ServerInterface<ServerMsg>,
    client_interface: client::ClientInterface<ClientMsg>,
    server_receiver: Receiver<(Connection, ServerMessage)>,
    client_receiver: Receiver<ClientMessage>,
}

impl<ServerMsg, ClientMsg> NetworkingBackend for DefaultNetworkingBackend<ServerMsg, ClientMsg>
where
    ClientMsg: NetSerializable,
    ServerMsg: NetSerializable,
    for<'a> ClientMsg::Archived: CheckBytes<HighValidator<'a, rancor::Error>> + 'static,
    for<'a> ServerMsg::Archived: CheckBytes<HighValidator<'a, rancor::Error>> + 'static,
{
    type Settings = NetworkingSettings;
    type Error = NetworkingError;

    type ServerEvent<'a> = RemoteMessage<'a, <ClientMsg as Archive>::Archived>;
    type ClientEvent<'a> = RemoteMessage<'a, <ServerMsg as Archive>::Archived>;

    type Connection = Connection;

    type ServerInterface = server::ServerInterface<ServerMsg>;
    type ClientInterface = client::ClientInterface<ClientMsg>;

    fn new(settings: Self::Settings) -> Result<Self, Self::Error> {
        let (server_sender, server_receiver) = bounded(2);
        let (client_sender, client_receiver) = bounded(2);

        let arena = std::sync::Arc::new(parking_lot::Mutex::new(Arena::new()));

        let server_interface =
            server::ServerInterface::new(settings.clone(), server_sender, arena.clone()).unwrap();
        let client_interface =
            client::ClientInterface::new(settings, client_sender, arena).unwrap();

        Ok(Self {
            server_interface,
            client_interface,
            server_receiver,
            client_receiver,
        })
    }

    fn server_interface(&self) -> &Self::ServerInterface {
        &self.server_interface
    }

    fn client_interface(&self) -> &Self::ClientInterface {
        &self.client_interface
    }

    fn receive<F>(&mut self, f: F) -> Result<(), Self::Error>
    where
        F: for<'a> FnOnce(NetEvent<'a, Self>),
    {
        enum Event {
            Server((Connection, ServerMessage)),
            Client(ClientMessage),
        }

        let event = smol::block_on(race(
            async {
                // Server
                Event::Server(self.server_receiver.recv().await.unwrap())
            },
            async {
                // Client
                Event::Client(self.client_receiver.recv().await.unwrap())
            },
        ));

        match event {
            Event::Server((connection, message)) => match message {
                ServerMessage::Error(e) => return Err(NetworkingError::Server(ServerError::Io(e))),
                ServerMessage::Warning(w) => f(NetEvent::Server {
                    connection,
                    event: RemoteMessage::Warning(w),
                }),
                ServerMessage::Connected => f(NetEvent::Server {
                    connection,
                    event: RemoteMessage::Connected,
                }),
                ServerMessage::Disconnected(reason) => f(NetEvent::Server {
                    connection,
                    event: RemoteMessage::Disconnected(reason),
                }),
                ServerMessage::Tcp(msg) => {
                    let result = rkyv::access(&msg);

                    f(match result {
                        Ok(archive) => NetEvent::Server {
                            connection,
                            event: RemoteMessage::Tcp(archive),
                        },

                        Err(e) => NetEvent::Server {
                            connection,
                            event: RemoteMessage::Warning(Warning::UnintelligableContent(e)),
                        },
                    })
                }
                ServerMessage::Udp(msg) => {
                    let result = rkyv::access(&msg);

                    f(match result {
                        Ok(archive) => NetEvent::Server {
                            connection,
                            event: RemoteMessage::Udp(archive),
                        },
                        Err(e) => NetEvent::Server {
                            connection,
                            event: RemoteMessage::Warning(Warning::UnintelligableContent(e)),
                        },
                    })
                }
            },
            Event::Client(message) => match message {
                ClientMessage::Error(e) => return Err(NetworkingError::Client(e)),
                ClientMessage::Warning(w) => f(NetEvent::Client {
                    event: RemoteMessage::Warning(w),
                }),
                ClientMessage::Connected => f(NetEvent::Client {
                    event: RemoteMessage::Connected,
                }),
                ClientMessage::Disconnected(reason) => f(NetEvent::Client {
                    event: RemoteMessage::Disconnected(reason),
                }),
                ClientMessage::Tcp(msg) => {
                    let result = rkyv::access(&msg);

                    f(match result {
                        Ok(archive) => NetEvent::Client {
                            event: RemoteMessage::Tcp(archive),
                        },
                        Err(e) => NetEvent::Client {
                            event: RemoteMessage::Warning(Warning::UnintelligableContent(e)),
                        },
                    })
                }
                ClientMessage::Udp(msg) => {
                    let result = rkyv::access(&msg);

                    f(match result {
                        Ok(archive) => NetEvent::Client {
                            event: RemoteMessage::Udp(archive),
                        },
                        Err(e) => NetEvent::Client {
                            event: RemoteMessage::Warning(Warning::UnintelligableContent(e)),
                        },
                    })
                }
            },
        }

        Ok(())
    }
}

#[derive(Debug, Error)]
pub enum NetworkingError {
    /// An IO error from the system.
    #[error(transparent)]
    Io(std::io::Error),

    #[error(transparent)]
    Server(ServerError),

    #[error(transparent)]
    Client(ClientError),
}

/// Settings for the networking system of let-engine.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct NetworkingSettings {
    /// The number of auth request retries before giving up the connection
    /// and failing to connect as client.
    ///
    /// ## Default
    ///
    /// `5`
    pub auth_retries: usize,

    /// The time between retries.
    ///
    /// ## Default
    ///
    /// `2 seconds`
    pub auth_retry_wait: Duration,

    /// The time between ping requests.
    ///
    /// ## Default
    ///
    /// `5 seconds`
    pub ping_wait: Duration,

    /// The maximum allowed ping before sending warnings.
    ///
    /// ## Default
    ///
    /// `5 seconds`
    pub max_ping: Duration,

    /// Maximum amount of concurrent connections allowed before warning
    ///
    /// # Default
    ///
    /// `20`
    pub max_connections: usize,

    /// The minimum duration between multiple packets allowed before warning
    ///
    /// ## Default
    ///
    /// `10 milliseconds`
    pub rate_limit: Duration,

    /// Max package size limit for the built in TCP protocol in bytes
    ///
    /// ## Default
    ///
    /// `1048576` bytes or 1MiB
    pub tcp_max_size: usize,

    /// Max package size limit for the built in UDP protocol in bytes
    ///
    /// ## Default
    ///
    /// `1200` bytes
    pub udp_max_size: usize,
}

impl Default for NetworkingSettings {
    fn default() -> Self {
        Self {
            auth_retries: 5,
            auth_retry_wait: Duration::from_secs(2),
            ping_wait: Duration::from_secs(5),
            max_ping: Duration::from_secs(5),
            rate_limit: Duration::from_millis(10),
            max_connections: 20,
            tcp_max_size: 1048576,
            udp_max_size: 1200,
        }
    }
}

/// Messages received by a remote connection.
#[derive(Debug)]
pub enum RemoteMessage<'a, Msg> {
    /// The client has connected to the server successfully.
    Connected,

    /// The remote has sent a message using TCP.
    Tcp(&'a Msg),

    /// The remote has sent a message using UDP.
    Udp(&'a Msg),

    /// The remote has sent non conformant packets.
    Warning(Warning),

    /// The client has been disconnected from the server.
    Disconnected(Disconnected),
}

/// Misbehaviour recorded by the remote peer.
#[derive(Debug)]
pub enum Warning {
    /// The rate at which the packets are sent is faster than the configured limit.
    RateLimitHit,

    /// The header of the message shows a size bigger than the configured limit.
    MessageTooBig,

    /// There was a problem reading and deserializing the received data.
    UnintelligableContent(rkyv::rancor::Error),

    /// The ping limit as set in the networking settings was hit.
    PingTooHigh,

    /// There was a problem connecting, which caused a retry.
    Retry(usize),
}

/// The connection to the peer has been stopped.
///
/// The reason for the disconnect is
#[derive(Debug, Error)]
pub enum Disconnected {
    /// The peer has gracefully shut down the connection
    #[error("Remote Shutdown")]
    RemoteShutdown,

    /// An unexpected termination of the connection has occured.
    #[error("Connection Aborted")]
    ConnectionAborted,

    /// The connection has been forcibly closed by the remote.
    ///
    /// The remote could be rebooting, shutting down or the application could have crashed.
    #[error("Connection Reset")]
    ConnectionReset,

    /// The peer has been disconnected for misbehaving and sending packets
    /// not according to the system.
    #[error("Peer Misbehaving")]
    MisbehavingPeer,

    /// An unexplainable error has occured.
    #[error(transparent)]
    Other(io::Error),
}

impl From<io::Error> for Disconnected {
    fn from(value: io::Error) -> Self {
        match value.kind() {
            ErrorKind::UnexpectedEof => Self::RemoteShutdown,
            ErrorKind::ConnectionAborted => Self::ConnectionAborted,
            ErrorKind::ConnectionReset => Self::ConnectionReset,
            _ => Self::Other(value),
        }
    }
}

/// The identification of a connection containing both TCP and UDP connection addresses for one user.
///
/// The IP of both is the same, but the port is different.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Hash)]
pub struct Connection {
    ip: IpAddr,
    tcp_port: u16,
    udp_port: u16,
}

impl Connection {
    fn from_tcp_udp_addr(tcp_addr: SocketAddr, udp_addr: SocketAddr) -> Self {
        Self {
            ip: tcp_addr.ip(),
            tcp_port: tcp_addr.port(),
            udp_port: udp_addr.port(),
        }
    }

    /// Returns the IP address of this connection.
    pub fn ip_addr(&self) -> IpAddr {
        self.ip
    }

    /// Returns the TCP port of this connection.
    pub fn tcp_port(&self) -> u16 {
        self.tcp_port
    }

    /// Returns the UDP port of this connection.
    pub fn udp_port(&self) -> u16 {
        self.udp_port
    }

    /// Returns the TCP address of this connection.
    pub fn tcp_addr(&self) -> SocketAddr {
        SocketAddr::new(self.ip, self.tcp_port)
    }

    /// Returns the UDP address of this connection.
    pub fn udp_addr(&self) -> SocketAddr {
        SocketAddr::new(self.ip, self.udp_port)
    }

    pub fn contains_addr(&self, addr: &SocketAddr) -> bool {
        self.ip == addr.ip() && (self.tcp_port == addr.port() || self.udp_port == addr.port())
    }
}

use rkyv::{
    Archive, Serialize,
    api::high::{HighSerializer, HighValidator, to_bytes_in_with_alloc},
    bytecheck::CheckBytes,
    rancor::{self, Source},
    ser::allocator::{Arena, ArenaHandle},
    util::AlignedVec,
};

/// Serialize the given data to a streamable message format.
///
/// ## Message format
///
/// - Length prefixed with a u32
///
/// \[u32data_len\](u8data)
fn serialize_tcp_into<T, E>(message: &T, arena: &mut Arena) -> AlignedVec
where
    T: for<'a> Serialize<HighSerializer<AlignedVec, ArenaHandle<'a>, E>>,
    E: Source,
{
    let mut data = AlignedVec::new();
    data.extend_from_slice(&[0; 4]);
    let mut data = to_bytes_in_with_alloc(message, data, arena.acquire()).unwrap();

    let len = data.len() - 4;

    data[0..4].copy_from_slice(&(len as u32).to_le_bytes());

    data
}

/// Serialize the given data to a streamable message format.
///
/// ## Message format
///
/// - Indexed and data length prefixed
///
/// \[u32order_number\]\[u32data_len\])(u8data)
///
/// Order number has to be added by yourself
fn serialize_udp_into<E: Source>(
    message: &impl for<'a> Serialize<HighSerializer<AlignedVec, ArenaHandle<'a>, E>>,
    arena: &mut Arena,
) -> AlignedVec {
    let mut data = AlignedVec::with_capacity(1024);
    data.extend_from_slice(&[0; 8]);

    let mut data = to_bytes_in_with_alloc(message, data, arena.acquire()).unwrap();

    let data_len = data.len() - 8;
    data[4..8].copy_from_slice(&(data_len as u32).to_le_bytes());

    data
}

struct BufferingMessage {
    bytes_left: usize,
    buf: Vec<u8>,
    timestamp: SystemTime,
}

impl BufferingMessage {
    pub fn new(size: usize) -> Self {
        let buf = Vec::with_capacity(size);

        Self {
            bytes_left: size,
            buf,
            timestamp: SystemTime::now(),
        }
    }

    pub fn completed(&mut self, buf: &[u8]) -> bool {
        let bytes_to_copy = buf.len().min(self.bytes_left);
        self.buf.extend_from_slice(&buf[..bytes_to_copy]);
        self.bytes_left -= bytes_to_copy;
        self.timestamp = SystemTime::now();
        self.bytes_left == 0
    }

    pub fn consume(self) -> Vec<u8> {
        self.buf
    }

    pub fn outdated(&self) -> bool {
        self.timestamp.elapsed().unwrap() > Duration::from_secs(1)
    }
}
