use bytemuck::AnyBitPattern;

#[derive(Debug, Clone, Copy)]
pub struct Buffer<T: AnyBitPattern + Send + Sync> {
    data: T,
    // size: u64,
    usage: BufferUsage,
    buffer_access: BufferAccess,
}

impl<T: AnyBitPattern + Send + Sync> Buffer<T> {
    pub fn from_data(usage: BufferUsage, optimisation: BufferAccess, data: T) -> Self {
        Self {
            data,
            // size: std::mem::size_of::<T>() as u64,
            usage,
            buffer_access: optimisation,
        }
    }

    /// Returns the intended usage of this buffer.
    pub fn usage(&self) -> &BufferUsage {
        &self.usage
    }

    /// Sets the intended usage of this buffer.
    pub fn set_usage(&mut self, usage: BufferUsage) {
        self.usage = usage;
    }

    /// Returns what this buffer is optimized for.
    pub fn optimisation(&self) -> &BufferAccess {
        &self.buffer_access
    }

    /// Sets the intended optimisation for this buffer.
    pub fn set_optimisation(&mut self, optimisation: BufferAccess) {
        self.buffer_access = optimisation;
    }
}

impl<T: AnyBitPattern + Send + Sync> std::ops::Deref for Buffer<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.data
    }
}

impl<T: AnyBitPattern + Send + Sync> std::ops::DerefMut for Buffer<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.data
    }
}

pub trait LoadedBuffer<B: AnyBitPattern + Send + Sync>: Clone + Send + Sync {
    type Error: std::error::Error + Send + Sync;

    fn data(&self) -> Result<B, Self::Error>;

    fn write_data_mut<F: FnMut(&mut B)>(&self, f: F) -> Result<(), Self::Error>;
}

impl<B: AnyBitPattern + Send + Sync> LoadedBuffer<B> for () {
    type Error = std::io::Error;

    fn data(&self) -> Result<B, Self::Error> {
        Ok(B::zeroed())
    }

    fn write_data_mut<F: FnMut(&mut B)>(&self, _f: F) -> Result<(), Self::Error> {
        Ok(())
    }
}

/// The descriptor location of a resource to be accessed in the shaders.
#[derive(Default, Debug, Clone, Copy, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub struct Location {
    pub set: u32,
    pub binding: u32,
}

impl Location {
    pub fn new(set: u32, binding: u32) -> Self {
        Self { set, binding }
    }
}

impl From<(u32, u32)> for Location {
    fn from(value: (u32, u32)) -> Self {
        Self {
            set: value.0,
            binding: value.1,
        }
    }
}

/// Describes the intended usage of a buffer.
#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq)]
pub enum BufferUsage {
    /// The buffer is used as a uniform buffer, typically for small pieces of data
    /// that are accessed frequently in shaders (e.g., constants or matrices).
    Uniform,

    /// The buffer is used as a storage buffer, which can handle larger, more flexible
    /// data structures and allow for read/write access in shaders.
    Storage,
}

/// The operation that the buffer access should be optimized for.
#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq)]
pub enum PreferOperation {
    /// Prefer reading over writing.
    Read,

    /// Prefer writing over reading.
    Write,
}

/// Determines the access pattern used for a buffer, optimizing it for specific scenarios.
///
/// This determines what a buffer is used for and how it is usable.
///
/// # Default
///
/// The default variant is [`BufferAccess::Fixed`]
#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq, Default)]
pub enum BufferAccess {
    /// A simple buffer access pattern for static data that is uploaded once and never changed after.
    ///
    /// This one has the lowest memory usage and best GPU performance.
    ///
    /// # Examples
    /// - Texture that never changes
    /// - Model that never changes
    /// - Buffer containing data that never changes
    #[default]
    Fixed,

    /// Allows reading from and writing to the buffer using a staging buffer.
    ///
    /// This pattern has good GPU performance, but slow reading and writing performance.
    ///
    /// Recommended for occasional large data transfers.
    ///
    /// # Examples
    /// - Texture that occasionally gets read and written to by the CPU and GPU
    /// - Model that occasionally gets read and written to by the CPU and GPU
    /// - Buffer that occasionally gets read and written to by the CPU and GPU
    Staged,

    /// Pins the CPU and GPU memory to a single buffer without staging.
    ///
    /// This has fast read and write performance, where one can be preferred over the other
    /// with the `PreferOperation`, but introduces GPU overhead.
    ///
    /// Waits for the GPU to finish before accessing buffers, which is not
    /// recommended for per frame use.
    ///
    /// Recommended for occasional small data transfers.
    ///
    /// # Examples
    /// - Model that occasionally gets read and written to by the CPU and GPU
    /// - Buffer that occasionally gets read and written to by the CPU and GPU
    Pinned(PreferOperation),

    /// Implements a ring buffer strategy for managing per-frame buffer allocations.
    ///
    /// This method avoids synchronisation issues when updating buffers per frame.
    /// Adds as much GPU overhead as a pinned buffer.
    ///
    /// This is recommended for small data that changes every single frame.
    ///
    /// Allows for reading and writing from buffers, where one can be preferred over the
    /// other with the `PreferOperation`.
    ///
    /// # Examples
    /// - Dynamic model that changes shape by the CPU or GPU each frame
    /// - Dynamic buffer that gets read and written to by the CPU and GPU each frame
    RingBuffer {
        /// The access operation to prefer over the other.
        prefer_operation: PreferOperation,

        /// The amount of buffers to allocate in the ring buffer.
        buffers: usize,
    },
}
