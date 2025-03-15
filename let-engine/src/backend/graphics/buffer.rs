use std::{
    sync::{atomic::AtomicU8, Arc},
    time::Duration,
};

use anyhow::Result;
use bytemuck::AnyBitPattern;
use let_engine_core::resources::buffer::{
    Buffer, BufferAccess, BufferUsage, LoadedBuffer, PreferOperation,
};
use parking_lot::Mutex;
use thiserror::Error;
use vulkano::{
    buffer::{BufferCreateInfo, BufferUsage as VkBufferUsage, Subbuffer},
    command_buffer::{
        AutoCommandBufferBuilder, CopyBufferInfo, PrimaryAutoCommandBuffer,
        PrimaryCommandBufferAbstract,
    },
    descriptor_set::layout::DescriptorType,
    memory::allocator::{AllocationCreateInfo, MemoryTypeFilter},
    sync::{GpuFuture, HostAccessError},
};

use super::{
    vulkan::{Queues, Vulkan},
    GraphicsInterface,
};

#[derive(Clone)]
enum AccessMethod<T: AnyBitPattern + Send + Sync> {
    Fixed,
    Staged {
        staging: Subbuffer<T>,
        read: Arc<PrimaryAutoCommandBuffer>,
        write: Arc<PrimaryAutoCommandBuffer>,
    },
    Pinned(PreferOperation),
    RingBuffer {
        buffers: Vec<Subbuffer<T>>,
        turn: Arc<AtomicU8>,
        prefer: PreferOperation,
    },
}

impl<T: AnyBitPattern + Send + Sync> From<AccessMethod<T>> for BufferAccess {
    fn from(value: AccessMethod<T>) -> Self {
        match value {
            AccessMethod::Fixed => BufferAccess::Fixed,
            AccessMethod::Staged { .. } => BufferAccess::Staged,
            AccessMethod::Pinned(prefer) => BufferAccess::Pinned(prefer),
            AccessMethod::RingBuffer {
                prefer, buffers, ..
            } => BufferAccess::RingBuffer {
                prefer_operation: prefer,
                buffers: buffers.len(),
            },
        }
    }
}

#[derive(Clone)]
pub struct GpuBuffer<T: AnyBitPattern + Send + Sync> {
    data: Subbuffer<T>,
    access_method: AccessMethod<T>,

    usage: BufferUsage,

    future: Arc<Mutex<Option<Box<dyn GpuFuture + Send>>>>,
    queues: Arc<Queues>,
}

type BufferCreation<T> = (Subbuffer<T>, AccessMethod<T>);

impl<T: AnyBitPattern + Send + Sync> GpuBuffer<T> {
    /// Creates a new buffer.
    pub fn new(buffer: Buffer<T>, interface: &GraphicsInterface) -> Result<Self> {
        let vulkan = interface.vulkan.clone();

        let future = vulkan.future.clone();
        let queues = vulkan.queues.clone();

        let (data, access_method) = Self::create_buffer_and_staging(&vulkan, buffer)?;

        Ok(Self {
            data,
            access_method,

            usage: *buffer.usage(),

            future,
            queues,
        })
    }

    /// Creates a new buffer which can only be accessed in the shaders.
    pub fn new_gpu_only(
        size: usize,
        usage: BufferUsage,
        interface: &GraphicsInterface,
    ) -> Result<Self> {
        let vulkan = interface.vulkan.clone();

        let buffer = vulkano::buffer::Buffer::new_unsized(
            vulkan.memory_allocator.clone(),
            BufferCreateInfo {
                usage: match usage {
                    BufferUsage::Uniform => VkBufferUsage::UNIFORM_BUFFER,
                    BufferUsage::Storage => VkBufferUsage::STORAGE_BUFFER,
                },
                ..Default::default()
            },
            AllocationCreateInfo {
                memory_type_filter: MemoryTypeFilter::PREFER_DEVICE,
                ..Default::default()
            },
            size as u64,
        )?;

        Ok(Self {
            data: buffer,
            access_method: AccessMethod::Fixed,
            usage,
            future: vulkan.future.clone(),
            queues: vulkan.queues.clone(),
        })
    }

    fn create_buffer_and_staging(vulkan: &Vulkan, buffer: Buffer<T>) -> Result<BufferCreation<T>> {
        let memory_allocator = vulkan.memory_allocator.clone();

        let buffer_usage = *buffer.usage();
        let usage = match buffer_usage {
            BufferUsage::Uniform => VkBufferUsage::UNIFORM_BUFFER,
            BufferUsage::Storage => VkBufferUsage::STORAGE_BUFFER,
        };

        let buffer_access = *buffer.optimisation();

        let (memory_type_filter, staging_memory_type_filter) = match buffer_access {
            BufferAccess::Fixed | BufferAccess::Staged => (
                MemoryTypeFilter::PREFER_DEVICE,
                Some(MemoryTypeFilter::PREFER_HOST | MemoryTypeFilter::HOST_SEQUENTIAL_WRITE),
            ),
            BufferAccess::Pinned(PreferOperation::Read) => (
                MemoryTypeFilter::PREFER_DEVICE | MemoryTypeFilter::HOST_RANDOM_ACCESS,
                None,
            ),
            BufferAccess::Pinned(PreferOperation::Write) => (
                MemoryTypeFilter::PREFER_DEVICE | MemoryTypeFilter::HOST_SEQUENTIAL_WRITE,
                None,
            ),
            BufferAccess::RingBuffer {
                prefer_operation: PreferOperation::Read,
                ..
            } => (
                MemoryTypeFilter::PREFER_DEVICE | MemoryTypeFilter::HOST_RANDOM_ACCESS,
                None,
            ),
            BufferAccess::RingBuffer {
                prefer_operation: PreferOperation::Write,
                ..
            } => (
                MemoryTypeFilter::PREFER_DEVICE | MemoryTypeFilter::HOST_SEQUENTIAL_WRITE,
                None,
            ),
        };

        // Create data buffer
        let data = vulkano::buffer::Buffer::new_sized::<T>(
            memory_allocator.clone(),
            BufferCreateInfo {
                usage: match buffer_access {
                    BufferAccess::Fixed => usage | VkBufferUsage::TRANSFER_DST,
                    // TODO: specialize staged to either one
                    BufferAccess::Staged => {
                        usage | VkBufferUsage::TRANSFER_SRC | VkBufferUsage::TRANSFER_DST
                    }
                    _ => usage,
                },
                ..Default::default()
            },
            AllocationCreateInfo {
                memory_type_filter,
                ..Default::default()
            },
        )?;

        // If not staging or fixed, write data directly into the buffer.
        if staging_memory_type_filter.is_none() {
            *data.write()? = *buffer;
        };

        let access_method = match buffer_access {
            BufferAccess::Fixed => {
                let staging = vulkano::buffer::Buffer::from_data(
                    memory_allocator.clone(),
                    BufferCreateInfo {
                        usage: usage | VkBufferUsage::TRANSFER_SRC | VkBufferUsage::TRANSFER_DST,
                        ..Default::default()
                    },
                    AllocationCreateInfo {
                        memory_type_filter: staging_memory_type_filter.unwrap(),
                        ..Default::default()
                    },
                    *buffer,
                )?;

                // Copy staging buffer to data.
                let write = Self::copy_staging(staging.clone(), data.clone(), vulkan)?;

                Self::execute_command(write.clone(), &vulkan.queues, vulkan.future.clone())?;

                AccessMethod::Fixed
            }
            BufferAccess::Staged => {
                let staging = vulkano::buffer::Buffer::from_data(
                    memory_allocator.clone(),
                    BufferCreateInfo {
                        usage: usage | VkBufferUsage::TRANSFER_SRC | VkBufferUsage::TRANSFER_DST,
                        ..Default::default()
                    },
                    AllocationCreateInfo {
                        memory_type_filter: staging_memory_type_filter.unwrap(),
                        ..Default::default()
                    },
                    *buffer,
                )?;

                // Copy staging buffer to data.
                let write = Self::copy_staging(staging.clone(), data.clone(), vulkan)?;

                Self::execute_command(write.clone(), &vulkan.queues, vulkan.future.clone())?;

                // Make reading command buffer
                let read = Self::copy_staging(data.clone(), staging.clone(), vulkan)?;

                AccessMethod::Staged {
                    staging,
                    read,
                    write,
                }
            }
            BufferAccess::Pinned(prefer) => AccessMethod::Pinned(prefer),
            BufferAccess::RingBuffer {
                prefer_operation,
                buffers,
            } => {
                let buffer_count = buffers - 1;

                let mut buffers = Vec::with_capacity(buffer_count);

                for _ in 0..buffer_count {
                    // Create other ring buffers
                    let data = vulkano::buffer::Buffer::new_sized::<T>(
                        memory_allocator.clone(),
                        BufferCreateInfo {
                            usage,
                            ..Default::default()
                        },
                        AllocationCreateInfo {
                            memory_type_filter,
                            ..Default::default()
                        },
                    )?;
                    // Write data into this buffer
                    *data.write()? = *buffer;

                    buffers.push(data);
                }

                let turn = Arc::new(0.into());

                AccessMethod::RingBuffer {
                    buffers,
                    turn,
                    prefer: prefer_operation,
                }
            }
        };

        Ok((data, access_method))
    }

    /// Creates a reusable command buffer for moving one buffer to another.
    fn copy_staging(
        src: Subbuffer<T>,
        dst: Subbuffer<T>,
        vulkan: &Vulkan,
    ) -> Result<Arc<PrimaryAutoCommandBuffer>> {
        let command_buffer_allocator = vulkan.command_buffer_allocator.clone();
        let queues = vulkan.queues.clone();

        // Create Command Buffer
        let mut command_buffer_builder = AutoCommandBufferBuilder::primary(
            command_buffer_allocator,
            queues.transfer_id(),
            vulkano::command_buffer::CommandBufferUsage::MultipleSubmit,
        )?;

        command_buffer_builder.copy_buffer(CopyBufferInfo::buffers(src, dst))?;

        let command_buffer = command_buffer_builder.build()?;

        Ok(command_buffer)
    }

    fn execute_command(
        command_buffer: Arc<PrimaryAutoCommandBuffer>,
        queues: &Arc<Queues>,
        future: Arc<Mutex<Option<Box<dyn GpuFuture + Send>>>>,
    ) -> Result<()> {
        let transfer_future = command_buffer
            .execute(queues.get_transfer().clone())?
            .then_signal_semaphore_and_flush()?;

        let mut future = future.lock();

        if let Some(old_future) = future.take() {
            *future = Some(old_future.join(transfer_future).boxed_send());
        } else {
            *future = Some(transfer_future.boxed_send());
        };

        Ok(())
    }
}

#[derive(Clone, Debug)]
pub struct DrawableBuffer {
    data: Subbuffer<[u8]>,
    ring: Option<(Vec<Subbuffer<[u8]>>, Arc<AtomicU8>)>,
    usage: BufferUsage,
}

impl DrawableBuffer {
    pub fn from_buffer<B: AnyBitPattern + Send + Sync>(buffer: GpuBuffer<B>) -> Self {
        let ring = if let AccessMethod::RingBuffer { buffers, turn, .. } = &buffer.access_method {
            Some((
                buffers
                    .iter()
                    .map(|x| x.reinterpret_ref())
                    .cloned()
                    .collect(),
                turn.clone(),
            ))
        } else {
            None
        };

        Self {
            data: buffer.data.as_bytes().clone(),
            ring,
            usage: buffer.usage,
        }
    }

    pub fn buffer(&self) -> Subbuffer<[u8]> {
        if let Some((buffers, turn)) = &self.ring {
            let index = turn.fetch_add(1, std::sync::atomic::Ordering::Relaxed) as usize
                % (buffers.len() + 1);
            if index == 0 {
                self.data.clone()
            } else {
                buffers[index - 1].clone()
            }
        } else {
            self.data.clone()
        }
    }

    pub(crate) fn descriptor_type(&self) -> DescriptorType {
        match self.usage {
            BufferUsage::Uniform => DescriptorType::UniformBuffer,
            BufferUsage::Storage => DescriptorType::StorageImage,
        }
    }
}

#[derive(Debug, Error)]
pub enum BufferError {
    // TODO: make more errors
    /// Returns if there was an error attempting to read or write to the buffer from or to the GPU.
    #[error("{0}")]
    HostAccess(HostAccessError),

    /// Returns when the access operation is not supported with the currently set access setting.
    #[error("Requested access operation not possible with current access setting: {0:?}")]
    UnsupportedAccess(BufferAccess),

    #[error("There was an error loading this buffer: {0}")]
    Other(anyhow::Error),
}

impl<B: AnyBitPattern + Send + Sync> LoadedBuffer<B> for GpuBuffer<B> {
    type Error = BufferError;

    fn data(&self) -> std::result::Result<B, Self::Error> {
        match &self.access_method {
            AccessMethod::Fixed => Err(BufferError::UnsupportedAccess(BufferAccess::Fixed)),
            AccessMethod::Staged { staging, read, .. } => {
                // Execute read command and wait for it to finish.
                read.clone()
                    .execute(self.queues.get_transfer().clone())
                    .map_err(|e| BufferError::Other(e.into()))?
                    .then_signal_fence_and_flush()
                    .map_err(|e| BufferError::Other(e.into()))?
                    .wait(None)
                    .map_err(|e| BufferError::Other(e.into()))?;

                // Return data
                let read = staging.read().map_err(BufferError::HostAccess)?;

                Ok(*read)
            }
            AccessMethod::Pinned(..) => {
                // Wait for data access
                if let Some(future) = self.future.lock().take() {
                    future
                        .then_signal_fence_and_flush()
                        .unwrap()
                        .wait(None)
                        .unwrap();
                };

                // Read data
                Ok(*(self.data.read().map_err(BufferError::HostAccess)?))
            }
            AccessMethod::RingBuffer { buffers, turn, .. } => {
                // Choose buffer not in use by the GPU
                let index =
                    turn.load(std::sync::atomic::Ordering::Relaxed) as usize % (buffers.len() + 1);
                let buffer = if index == 0 {
                    &self.data
                } else {
                    &buffers[index - 1]
                };

                // Read from this buffer
                Ok(*(buffer.read().map_err(BufferError::HostAccess)?))
            }
        }
    }

    fn write_data_mut<F: FnMut(&mut B)>(&self, mut f: F) -> std::result::Result<(), Self::Error> {
        match &self.access_method {
            AccessMethod::Fixed => return Err(BufferError::UnsupportedAccess(BufferAccess::Fixed)),
            AccessMethod::Staged { staging, write, .. } => {
                {
                    let mut guard = staging.write().map_err(BufferError::HostAccess)?;

                    f(&mut guard);
                }

                Self::execute_command(write.clone(), &self.queues, self.future.clone())
                    .map_err(BufferError::Other)?;
            }
            AccessMethod::Pinned(..) => {
                // Wait for buffer access
                if let Some(future) = self.future.lock().take() {
                    dbg!(future.queue());
                    future
                        .then_signal_fence_and_flush()
                        .unwrap()
                        .wait(None)
                        .unwrap();
                    dbg!("Waited");
                };

                // Write
                let mut guard = self.data.write().map_err(BufferError::HostAccess)?;

                f(&mut guard);
            }
            AccessMethod::RingBuffer { buffers, turn, .. } => {
                // dbg!(data.write().is_err() && self.data.write().is_err());

                // Choose buffer not in use by the GPU
                let index =
                    turn.load(std::sync::atomic::Ordering::Relaxed) as usize % (buffers.len() + 1);
                let buffer = if index == 0 {
                    &self.data
                } else {
                    &buffers[index - 1]
                };

                // Write
                let mut guard = buffer.write().map_err(BufferError::HostAccess)?;

                f(&mut guard);
            }
        }

        Ok(())
    }
}
