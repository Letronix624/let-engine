//! Networking, server and client ablilities built in the game engine.

mod client;
mod server;

use std::net::SocketAddr;

use async_std::channel::{Receiver, Sender};
pub use client::*;
pub use server::*;

/// Messages received by a remote connection.
#[derive(Clone)]
pub enum RemoteMessage<Msg> {
    /// The client has connected to the server successfully.
    Connected,
    /// The remote has sent a message using TCP.
    Tcp(Msg),
    /// The remote has sent a message using UDP.
    Udp(Msg),
    /// The client has been disconnected from the server.
    Disconnected,
}

type Messages<Msg> = (
    Sender<(SocketAddr, RemoteMessage<Msg>)>,
    Receiver<(SocketAddr, RemoteMessage<Msg>)>,
);

// # let engine variable size packet padding system for TCP
//
// - Messages smaller than the configured packet size are interpreted as a single message.
//
// - Messages exactly the configured packet size or bigger get a second package sent with
//   the first byte as an empty padding byte to be trimmed.
