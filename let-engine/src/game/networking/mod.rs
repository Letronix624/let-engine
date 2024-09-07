//! Networking, server and client ablilities built in the game engine.

mod client;
mod server;

use std::{
    io::{self, ErrorKind},
    net::SocketAddr,
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

/// Serialize the given data to a streamable format.
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

    data.extend((data_len as u32).to_le_bytes());

    data.extend(serialized_data);

    Ok(data)
}

/// ## Message format
///
/// - Indexed and data length prefixed prefixed
///
/// \[u32order_number\]\[u32data_len\])(u8data)
fn serialize_udp(order_number: u32, message: &impl Serialize) -> bincode::Result<Vec<u8>> {
    let serialized_data = bincode::serialize(message)?;

    let data_len = serialized_data.len();
    let mut data: Vec<u8> = Vec::with_capacity(data_len + 8);

    let order_number = order_number.to_le_bytes();
    data.extend(order_number);

    data.extend((data_len as u32).to_le_bytes());

    data.extend(serialized_data);

    Ok(data)
}
