pub mod audio;
pub mod gpu;
pub mod networking;

/// Backend types to be used during runtime of the game engine.
///
/// Each backend must implement their respective trait to be able to be interfaced in the event loop by the user.
pub trait Backends {
    type Gpu: gpu::GpuBackend;

    type Kira: audio::AudioBackend;

    type Networking: networking::NetworkingBackend;
}
