//! Networking, server and client ablilities built in the game engine.

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
    net::SocketAddr,
    time::SystemTime,
};

pub use client::*;
use serde::Serialize;
pub use server::*;
use smol::channel::{Receiver, Sender};

/// Messages received by a remote connection.
#[derive(Debug)]
pub enum RemoteMessage<Msg> {
    /// The client has connected to the server successfully.
    Connected,
    /// The remote has sent a message using TCP.
    Tcp(Msg),
    /// The remote has sent a message using UDP.
    Udp(Msg),
    /// The client has been disconnected from the server.
    Disconnected(Disconnected),
    /// There was a problem reading and deserializing the received data.
    DeserialisationError(bincode::Error),
}

type Messages<Msg> = (
    Sender<(Connection, RemoteMessage<Msg>)>,
    Receiver<(Connection, RemoteMessage<Msg>)>,
);

/// The identification of a connection containing both TCP and UDP connection addresses for one user.
///
/// The IP of both is the same, but the port is different.
#[derive(Clone, Copy, Debug, PartialEq, PartialOrd, Hash)]
pub struct Connection {
    tcp_addr: SocketAddr,
    udp_addr: SocketAddr,
}

impl Eq for Connection {}

impl Connection {
    fn new(tcp_addr: SocketAddr, udp_port: u16) -> Self {
        Self {
            tcp_addr,
            udp_addr: SocketAddr::new(tcp_addr.ip(), udp_port),
        }
    }

    /// Returns the TCP address of this user.
    pub fn tcp_addr(&self) -> SocketAddr {
        self.tcp_addr
    }

    /// Returns the UDP address of this user.
    pub fn udp_addr(&self) -> SocketAddr {
        self.udp_addr
    }
}

/// The connection to the peer has been stopped.
///
/// The reason for the disconnect is
#[derive(Debug)]
pub enum Disconnected {
    /// The peer has gracefully shut down the connection
    RemoteShutdown,
    /// An unexpected termination of the connection has occured.
    ConnectionAborted,
    /// The connection has been forcibly closed by the remote.
    ///
    /// The remote could be rebooting, shutting down or the application could have crashed.
    ConnectionReset,
    /// The peer has been disconnected for misbehaving and sending packets
    /// not according to the system.
    MisbehavingPeer,
    /// An unexplainable error has occured.
    Other(io::Error),
}

impl std::fmt::Display for Disconnected {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let data = match self {
            Disconnected::RemoteShutdown => "Remote shutdown",
            Disconnected::ConnectionAborted => "Connection aborted",
            Disconnected::ConnectionReset => "Connection reset",
            Disconnected::MisbehavingPeer => "Peer misbehaving",
            Disconnected::Other(e) => &format!("{e}"),
        };

        f.write_str(data)
    }
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

/// Serialize the given data to a streamable message format.
///
/// ## Message format
///
/// - Length prefixed with a u32
///
/// \[u32data_len\](u8data)
fn serialize_tcp(message: &impl Serialize) -> bincode::Result<Vec<u8>> {
    let serialized_data = bincode::serialize(message)?;

    let data_len = serialized_data.len();

    let mut data: Vec<u8> = Vec::with_capacity(data_len + 4);

    data.extend_from_slice(&(data_len as u32).to_le_bytes());

    data.extend(serialized_data);

    Ok(data)
}

/// Serialize the given data to a streamable message format.
///
/// ## Message format
///
/// - Indexed and data length prefixed
///
/// \[u32order_number\]\[u32data_len\])(u8data)
fn serialize_udp(order_number: u32, message: &impl Serialize) -> bincode::Result<Vec<u8>> {
    let serialized_data = bincode::serialize(message)?;

    let data_len = serialized_data.len();
    let mut data: Vec<u8> = Vec::with_capacity(data_len + 8);

    data.extend_from_slice(&order_number.to_le_bytes());

    data.extend_from_slice(&(data_len as u32).to_le_bytes());

    data.extend(serialized_data);

    Ok(data)
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

    pub fn completed(&mut self, buf: &[u8]) -> Option<&Vec<u8>> {
        let bytes_to_copy = buf.len().min(self.bytes_left);
        self.buf.extend_from_slice(&buf[..bytes_to_copy]);
        self.bytes_left -= bytes_to_copy;
        self.timestamp = SystemTime::now();
        (self.bytes_left == 0).then_some(&self.buf)
    }

    pub fn outdated(&self) -> bool {
        self.timestamp.elapsed().unwrap() > crate::SETTINGS.tick_system.get().tick_wait
    }
}
