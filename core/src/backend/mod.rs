use serde::{Deserialize, Serialize};

pub mod graphics;

/// Backend types to be used during runtime of the game engine.
///
/// Each backend must implement their respective trait to be able to be interfaced in the event loop by the user.
pub trait Backends {
    type Graphics: graphics::GraphicsBackend;

    // type Audio: Backend;

    type Networking: NetworkingBackend;
}

pub trait NetworkingBackend {
    type Msg: for<'de> NetworkingMessages<'de>;
    type Settings: Default + Clone;
}

pub trait NetworkingMessages<'de>:
    Send + Sync + Serialize + Deserialize<'de> + Clone + 'static
{
    type Tcp;
    type Udp;
}

impl NetworkingBackend for () {
    type Msg = ();
    type Settings = ();
}

impl NetworkingMessages<'_> for () {
    type Tcp = ();
    type Udp = ();
}
