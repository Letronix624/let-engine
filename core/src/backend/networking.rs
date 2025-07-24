use std::net::{SocketAddr, ToSocketAddrs};

pub enum NetEvent<'a, B: NetworkingBackend> {
    Server {
        connection: B::Connection,
        event: B::ServerEvent<'a>,
    },
    Client {
        event: B::ClientEvent<'a>,
    },
    Error(B::Error),
}

pub trait NetworkingBackend: Sized {
    type Settings: Send + Sync + Default + Clone;
    type Error: std::error::Error + Send + Sync;

    type Connection: Send + Sync + Clone;

    type ServerEvent<'a>;
    type ClientEvent<'a>;

    type ServerInterface: ServerInterface<Self::Connection>;
    type ClientInterface: ClientInterface<Self::Connection>;

    /// Creates the networking backend with a callback method that should be called every time an event occurred.
    fn new(settings: &Self::Settings) -> Result<Self, Self::Error>;

    fn server_interface(&self) -> &Self::ServerInterface;
    fn client_interface(&self) -> &Self::ClientInterface;

    fn receive<F>(&mut self, f: F) -> Result<(), Self::Error>
    where
        F: for<'a> FnOnce(NetEvent<'a, Self>);
}

pub trait ClientInterface<C: Send + Sync>: Send + Sync + Clone {
    type Msg;
    type Error: std::error::Error + Send + Sync;

    /// Connects to the servers remote address.
    fn connect<Addr: ToSocketAddrs>(&self, addr: Addr) -> Result<(), Self::Error>;
    /// Stops the connection to the server.
    fn disconnect(&self) -> Result<(), Self::Error>;

    /// Returns the local address
    fn local_conn(&self) -> Option<C>;
    /// Returns the remote address of the server.
    fn peer_conn(&self) -> Option<C>;

    /// Sends a message with reliability and error correction.
    ///
    /// Recommended when the reliability is more important than speed.
    ///
    /// ## Use Cases
    /// - Chat messages
    ///
    ///   they need to be sent reliably and in order.
    ///
    ///
    /// - Game State Updates
    ///
    ///   sending information critical to the game state like player health updates, inventory changes or game events
    ///   like picking up an item.
    ///
    /// - Player Actions
    ///
    ///   sending actions like pressing a button, opening a door, triggering a skill.
    fn send(&self, message: &Self::Msg) -> Result<(), Self::Error>;
    /// Sends a message as fast as possible with no error correction and reliability.
    ///
    /// Recommended when speed and latency is important, even when packets can be lost.
    ///
    /// ## Use Cases
    /// - Movement data updates of the player
    ///
    ///   losing a few packets of this data is acceptable because the next packet will update the position anyway.
    ///
    ///
    /// - Realtime State Updates
    ///
    ///   object position updates, NPCs or projectiles need to be updated frequently.
    ///   Losing a few packets should not be critical.
    ///
    ///
    /// - Video/Audio Streaming
    ///
    ///   transmitting live video or voice chat audio streams require low latency.
    ///   Losing a packet or two might cause a noticable glitch, but it should run smoothly overall.
    fn fast_send(&self, message: &Self::Msg) -> Result<(), Self::Error>;
}

pub trait ServerInterface<C: Send + Sync>: Send + Sync + Clone {
    type Msg;
    type Error: std::error::Error + Send + Sync;

    fn start<Addr: ToSocketAddrs>(&self, addr: Addr) -> Result<(), Self::Error>;
    fn stop(&self) -> Result<(), Self::Error>;

    fn local_addr(&self) -> Option<SocketAddr>;

    fn send(&self, conn: C, message: &Self::Msg) -> Result<(), Self::Error>;
    fn fast_send(&self, conn: C, message: &Self::Msg) -> Result<(), Self::Error>;
    fn broadcast(&self, message: &Self::Msg) -> Result<(), Self::Error>;
    fn fast_broadcast(&self, message: &Self::Msg) -> Result<(), Self::Error>;

    fn disconnect(&self, conn: C) -> Result<(), Self::Error>;

    fn connections(&self) -> impl Iterator<Item = C>;

    /// Returns true if the given connection is connected.
    fn is_connected(&self, connection: &C) -> Result<bool, Self::Error>;
}

impl NetworkingBackend for () {
    type Settings = ();
    type Error = std::io::Error;

    type Connection = ();

    type ServerEvent<'a> = ();
    type ClientEvent<'a> = ();

    type ServerInterface = ();
    type ClientInterface = ();

    fn new(_settings: &Self::Settings) -> Result<Self, Self::Error> {
        Ok(())
    }

    fn server_interface(&self) -> &Self::ServerInterface {
        &()
    }
    fn client_interface(&self) -> &Self::ClientInterface {
        &()
    }

    /// This message should block until a message is received.
    fn receive<F>(&mut self, _f: F) -> Result<(), Self::Error>
    where
        F: FnOnce(NetEvent<Self>),
    {
        std::thread::park();
        Ok(())
    }
}

impl ClientInterface<()> for () {
    type Msg = ();
    type Error = std::io::Error;

    fn connect<Addr>(&self, _addr: Addr) -> Result<(), Self::Error> {
        Ok(())
    }
    fn disconnect(&self) -> Result<(), Self::Error> {
        Ok(())
    }

    fn local_conn(&self) -> Option<()> {
        None
    }
    fn peer_conn(&self) -> Option<()> {
        None
    }

    fn send(&self, _message: &Self::Msg) -> Result<(), Self::Error> {
        Ok(())
    }
    fn fast_send(&self, _message: &Self::Msg) -> Result<(), Self::Error> {
        Ok(())
    }
}

impl ServerInterface<()> for () {
    type Msg = ();
    type Error = std::io::Error;

    fn start<Addr>(&self, _addr: Addr) -> Result<(), Self::Error> {
        Ok(())
    }
    fn stop(&self) -> Result<(), Self::Error> {
        Ok(())
    }

    fn local_addr(&self) -> Option<SocketAddr> {
        None
    }

    fn send(&self, _conn: (), _message: &Self::Msg) -> Result<(), Self::Error> {
        Ok(())
    }
    fn fast_send(&self, _conn: (), _message: &Self::Msg) -> Result<(), Self::Error> {
        Ok(())
    }
    fn broadcast(&self, _message: &Self::Msg) -> Result<(), Self::Error> {
        Ok(())
    }
    fn fast_broadcast(&self, _message: &Self::Msg) -> Result<(), Self::Error> {
        Ok(())
    }

    fn disconnect(&self, _conn: ()) -> Result<(), Self::Error> {
        Ok(())
    }

    fn connections(&self) -> impl Iterator<Item = ()> {
        [].into_iter()
    }

    fn is_connected(&self, _connection: &()) -> Result<bool, Self::Error> {
        Ok(false)
    }
}
