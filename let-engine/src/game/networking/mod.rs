//! Networking, server and client ablilities built in the game engine.

mod client;
mod server;

pub use client::*;
pub use server::*;

/// Messages received by a remote connection.
#[derive(Clone)]
pub enum RemoteMessage<Msg> {
    /// The user has connected to the server successfully.
    Connected,
    /// The user has sent data to the server using TCP.
    Tcp(Msg),
    /// The user has sent data to the server using UDP.
    Udp(Msg),
    /// The user has been disconnected from the server.
    Disconnected,
}
