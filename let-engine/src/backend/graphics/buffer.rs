use std::{
    marker::PhantomData,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering::Relaxed},
    },
};

use concurrent_slotmap::{Key, SlotId};
use let_engine_core::resources::{
    buffer::{Buffer, BufferAccess, BufferUsage, LoadedBuffer, PreferOperation},
    data::Data,
};
use thiserror::Error;
use vulkano::{
    DeviceSize,
    buffer::{Buffer as VkBuffer, BufferCreateInfo, BufferUsage as VkBufferUsage},
    descriptor_set::layout::DescriptorType,
    memory::allocator::{AllocationCreateInfo, DeviceLayout, MemoryTypeFilter},
    sync::HostAccessError,
};
use vulkano_taskgraph::{
    Id,
    command_buffer::CopyBufferInfo,
    resource::{AccessTypes, HostAccessType},
};

use crate::backend::graphics::vulkan::Resource;

use super::{
    VulkanError,
    vulkan::{VK, Vulkan},
};

enum GpuBufferInner {
    Fixed(Id<VkBuffer>),
    Staged {
        buffer_id: Id<VkBuffer>,
        staging_id: Id<VkBuffer>,
    },
    Pinned {
        buffer_id: Id<VkBuffer>,
        prefer_operation: PreferOperation,
    },
    RingBuffer {
        buffer_ids: Box<[Id<VkBuffer>]>,
        turn: AtomicUsize,
    },
}

impl From<&GpuBufferInner> for BufferAccess {
    fn from(value: &GpuBufferInner) -> Self {
        match value {
            GpuBufferInner::Fixed(_) => BufferAccess::Fixed,
            GpuBufferInner::Staged { .. } => BufferAccess::Staged,
            GpuBufferInner::Pinned {
                prefer_operation, ..
            } => BufferAccess::Pinned(*prefer_operation),
            GpuBufferInner::RingBuffer {
                buffer_ids: buffers,
                ..
            } => BufferAccess::RingBuffer {
                buffers: buffers.len(),
            },
        }
    }
}

#[derive(Debug, Error)]
pub enum GpuBufferError {
    /// Returns when a buffer is attempted to be created with an invalid size, either too big or too small.
    #[error("Can not create a buffer: out of bounds size.")]
    InvalidSize,

    // TODO: make more errors
    /// Returns if there was an error attempting to read or write to the buffer from or to the GPU.
    #[error("{0}")]
    HostAccess(HostAccessError),

    /// Returns when the access operation is not supported with the currently set access setting.
    #[error("Requested access operation not possible with current access setting: {0:?}")]
    UnsupportedAccess(BufferAccess),

    #[error("There was an error loading this buffer: {0}")]
    Other(VulkanError),
}

/// A GPU loaded representation of a data buffer.
pub struct GpuBuffer<T: Data> {
    inner: GpuBufferInner,

    usage: BufferUsage,
    _phantom_data: PhantomData<Arc<T>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct BufferId<T: Data>(SlotId, PhantomData<T>);

impl<T: Data> Key for BufferId<T> {
    #[inline]
    fn from_id(id: SlotId) -> Self {
        Self(id, PhantomData)
    }

    #[inline]
    fn as_id(self) -> SlotId {
        self.0
    }
}

impl<T: Data> GpuBuffer<T> {
    /// Creates a new buffer.
    pub(crate) fn new(buffer: &Buffer<T>, vulkan: &Vulkan) -> Result<Self, GpuBufferError> {
        let usage = match buffer.usage() {
            BufferUsage::Uniform => VkBufferUsage::UNIFORM_BUFFER,
            BufferUsage::Storage => VkBufferUsage::STORAGE_BUFFER,
        };

        let buffer_inner = match *buffer.optimization() {
            BufferAccess::Fixed => {
                let (buffer_id, _) = GpuBuffer::write_staging(vulkan, usage, buffer);

                GpuBufferInner::Fixed(buffer_id)
            }
            BufferAccess::Staged => {
                let (buffer_id, staging_id) = GpuBuffer::write_staging(vulkan, usage, buffer);

                GpuBufferInner::Staged {
                    buffer_id,
                    staging_id,
                }
            }
            BufferAccess::Pinned(prefer_operation) => {
                Self::pinned(buffer, vulkan, usage, prefer_operation)
            }
            BufferAccess::RingBuffer { buffers } => Self::ring(buffer, vulkan, usage, buffers)?,
            access => return Err(GpuBufferError::UnsupportedAccess(access)),
        };
        vulkan.wait_transfer();

        vulkan.flag_taskgraph_to_be_rebuilt();

        Ok(Self {
            inner: buffer_inner,

            usage: *buffer.usage(),
            _phantom_data: PhantomData,
        })
    }

    /// Creates a new buffer which can only be accessed in the shaders.
    pub(crate) fn new_gpu_only(
        size: DeviceSize,
        usage: BufferUsage,
        vulkan: &Vulkan,
    ) -> Result<Self, GpuBufferError> {
        let Some(size) = DeviceLayout::new_unsized::<T>(size) else {
            return Err(GpuBufferError::InvalidSize);
        };

        let buffer_id = vulkan
            .resources
            .create_buffer(
                &BufferCreateInfo {
                    usage: match usage {
                        BufferUsage::Uniform => VkBufferUsage::UNIFORM_BUFFER,
                        BufferUsage::Storage => VkBufferUsage::STORAGE_BUFFER,
                    },
                    ..Default::default()
                },
                &AllocationCreateInfo {
                    memory_type_filter: MemoryTypeFilter::PREFER_DEVICE,
                    ..Default::default()
                },
                size,
            )
            .map_err(|e| GpuBufferError::Other(e.unwrap().into()))?;

        vulkan.flag_taskgraph_to_be_rebuilt();

        Ok(Self {
            inner: GpuBufferInner::Fixed(buffer_id),
            usage,
            _phantom_data: PhantomData,
        })
    }

    /// Creates a device and host buffer writing the contents of the given buffer data.
    ///
    /// The first buffer is the GPU buffer, the second the CPU buffer.
    fn write_staging(
        vulkan: &super::Vulkan,
        usage: VkBufferUsage,
        buffer: &Buffer<T>,
    ) -> (Id<VkBuffer>, Id<VkBuffer>) {
        let buffer_id = vulkan
            .resources
            .create_buffer(
                &BufferCreateInfo {
                    usage: usage | VkBufferUsage::TRANSFER_SRC | VkBufferUsage::TRANSFER_DST,
                    ..Default::default()
                },
                &AllocationCreateInfo {
                    memory_type_filter: MemoryTypeFilter::PREFER_DEVICE,
                    ..Default::default()
                },
                DeviceLayout::new_sized::<T>(),
            )
            .unwrap();

        let staging_id = vulkan
            .resources
            .create_buffer(
                &BufferCreateInfo {
                    usage: usage | VkBufferUsage::TRANSFER_SRC | VkBufferUsage::TRANSFER_DST,
                    ..Default::default()
                },
                &AllocationCreateInfo {
                    memory_type_filter: MemoryTypeFilter::PREFER_HOST
                        | MemoryTypeFilter::HOST_SEQUENTIAL_WRITE,
                    ..Default::default()
                },
                DeviceLayout::new_sized::<T>(),
            )
            .unwrap();

        vulkan.wait_transfer();
        // Copy staging buffer to data.
        unsafe {
            vulkano_taskgraph::execute(
                vulkan.queues.transfer(),
                &vulkan.resources,
                vulkan.transfer_flight,
                |cb, ctx| {
                    // Write staging buffer
                    let write: &mut T = ctx.write_buffer::<T>(staging_id, ..)?;

                    *write = *buffer.data();

                    // Copy staging buffer to fixed buffer
                    cb.copy_buffer(&CopyBufferInfo {
                        src_buffer: staging_id,
                        dst_buffer: buffer_id,
                        ..Default::default()
                    })?;

                    Ok(())
                },
                [(staging_id, HostAccessType::Write)],
                [
                    (staging_id, AccessTypes::COPY_TRANSFER_READ),
                    (buffer_id, AccessTypes::COPY_TRANSFER_WRITE),
                ],
                [],
            )
        }
        .unwrap();

        (buffer_id, staging_id)
    }
}

// Access technique implementations
impl<T: Data> GpuBuffer<T> {
    fn pinned(
        buffer: &Buffer<T>,
        vulkan: &Vulkan,
        usage: VkBufferUsage,
        prefer_operation: PreferOperation,
    ) -> GpuBufferInner {
        let memory_type_filter = match prefer_operation {
            PreferOperation::Read => {
                MemoryTypeFilter::PREFER_DEVICE | MemoryTypeFilter::HOST_RANDOM_ACCESS
            }
            PreferOperation::Write => {
                MemoryTypeFilter::PREFER_DEVICE | MemoryTypeFilter::HOST_SEQUENTIAL_WRITE
            }
        };
        // Create data buffer
        let buffer_id = vulkan
            .resources
            .create_buffer(
                &BufferCreateInfo {
                    usage,
                    ..Default::default()
                },
                &AllocationCreateInfo {
                    memory_type_filter,
                    ..Default::default()
                },
                DeviceLayout::new_sized::<T>(),
            )
            .unwrap();
        vulkan.wait_transfer();
        unsafe {
            vulkano_taskgraph::execute(
                vulkan.queues.transfer(),
                &vulkan.resources,
                vulkan.transfer_flight,
                |_, ctx| {
                    let write = ctx.write_buffer::<T>(buffer_id, ..)?;

                    *write = *buffer.data();

                    Ok(())
                },
                [(buffer_id, HostAccessType::Write)],
                [],
                [],
            )
        }
        .unwrap();

        GpuBufferInner::Pinned {
            buffer_id,
            prefer_operation,
        }
    }

    fn ring(
        buffer: &Buffer<T>,
        vulkan: &Vulkan,
        usage: VkBufferUsage,
        buffers: usize,
    ) -> Result<GpuBufferInner, GpuBufferError> {
        let mut buffer_ids = Vec::with_capacity(buffers);

        // Create other ring buffers
        for _ in 0..buffers {
            let buffer_id = vulkan
                .resources
                .create_buffer(
                    &BufferCreateInfo {
                        usage,
                        ..Default::default()
                    },
                    &AllocationCreateInfo {
                        memory_type_filter: MemoryTypeFilter::PREFER_DEVICE
                            | MemoryTypeFilter::HOST_SEQUENTIAL_WRITE,
                        ..Default::default()
                    },
                    DeviceLayout::new_sized::<T>(),
                )
                .map_err(|e| GpuBufferError::Other(e.unwrap().into()))?;

            buffer_ids.push(buffer_id);
        }

        vulkan.wait_transfer();
        unsafe {
            vulkano_taskgraph::execute(
                vulkan.queues.transfer(),
                &vulkan.resources,
                vulkan.transfer_flight,
                |_, ctx| {
                    for buffer_id in buffer_ids.iter() {
                        let write = ctx.write_buffer::<T>(*buffer_id, ..)?;

                        *write = *buffer.data();
                    }

                    Ok(())
                },
                buffer_ids
                    .iter()
                    .map(|buffer_id| (*buffer_id, HostAccessType::Write)),
                [],
                [],
            )
        }
        .unwrap();

        Ok(GpuBufferInner::RingBuffer {
            buffer_ids: buffer_ids.into_boxed_slice(),
            turn: 0.into(),
        })
    }
}

impl<T: Data> GpuBuffer<T> {
    /// Returns the current buffer ID
    pub(crate) fn buffer_id(&self) -> Id<VkBuffer> {
        match &self.inner {
            GpuBufferInner::Fixed(buffer_id) => *buffer_id,
            GpuBufferInner::Staged { buffer_id, .. } => *buffer_id,
            GpuBufferInner::Pinned { buffer_id, .. } => *buffer_id,
            GpuBufferInner::RingBuffer {
                buffer_ids, turn, ..
            } => buffer_ids[turn.load(Relaxed)],
        }
    }

    pub(crate) fn descriptor_type(&self) -> DescriptorType {
        match self.usage {
            BufferUsage::Uniform => DescriptorType::UniformBuffer,
            BufferUsage::Storage => DescriptorType::StorageImage,
        }
    }

    pub(crate) fn access_types(&self) -> AccessTypes {
        match self.usage {
            BufferUsage::Uniform => {
                AccessTypes::VERTEX_SHADER_UNIFORM_READ | AccessTypes::FRAGMENT_SHADER_UNIFORM_READ
            }
            BufferUsage::Storage => {
                AccessTypes::FRAGMENT_SHADER_STORAGE_READ | AccessTypes::VERTEX_SHADER_STORAGE_READ
            }
        }
    }

    pub(crate) fn resources(&self) -> Vec<Resource> {
        match &self.inner {
            GpuBufferInner::Fixed(buffer_id)
            | GpuBufferInner::Staged { buffer_id, .. }
            | GpuBufferInner::Pinned { buffer_id, .. } => {
                vec![Resource::Buffer {
                    id: *buffer_id,
                    access_types: self.access_types(),
                }]
            }
            GpuBufferInner::RingBuffer { buffer_ids, .. } => buffer_ids
                .iter()
                .map(|buffer_id| Resource::Buffer {
                    id: *buffer_id,
                    access_types: self.access_types(),
                })
                .collect(),
        }
    }
}

impl<B: Data> LoadedBuffer<B> for GpuBuffer<B> {
    type Error = GpuBufferError;

    fn data<F>(&self, f: F) -> std::result::Result<(), Self::Error>
    where
        F: FnOnce(&B),
    {
        let vulkan = VK.get().unwrap();

        let queue = vulkan.queues.transfer();

        match &self.inner {
            GpuBufferInner::Staged {
                buffer_id,
                staging_id,
            } => {
                vulkan.wait_transfer();

                // Task 1: data -> staging
                unsafe {
                    vulkano_taskgraph::execute(
                        queue,
                        &vulkan.resources,
                        vulkan.transfer_flight,
                        |cb, _| {
                            cb.copy_buffer(&CopyBufferInfo {
                                src_buffer: *buffer_id,
                                dst_buffer: *staging_id,
                                ..Default::default()
                            })?;
                            Ok(())
                        },
                        [],
                        [
                            (*buffer_id, AccessTypes::COPY_TRANSFER_READ),
                            (*staging_id, AccessTypes::COPY_TRANSFER_WRITE),
                        ],
                        [],
                    )
                }
                .unwrap();

                vulkan.wait_transfer();

                // Task 2: read staging
                unsafe {
                    vulkano_taskgraph::execute(
                        queue,
                        &vulkan.resources,
                        vulkan.transfer_flight,
                        |_, ctx| {
                            let read: &B = ctx.read_buffer(*staging_id, ..)?;

                            f(read);

                            Ok(())
                        },
                        [(*staging_id, HostAccessType::Read)],
                        [],
                        [],
                    )
                }
                .unwrap();
            }
            GpuBufferInner::Pinned { buffer_id, .. } => {
                vulkan.wait_transfer();
                unsafe {
                    vulkano_taskgraph::execute(
                        queue,
                        &vulkan.resources,
                        vulkan.transfer_flight,
                        |_, ctx| {
                            let read = ctx.read_buffer(*buffer_id, ..)?;
                            f(read);
                            Ok(())
                        },
                        [(*buffer_id, HostAccessType::Read)],
                        [],
                        [],
                    )
                }
                .unwrap();
            }
            other => {
                return Err(GpuBufferError::UnsupportedAccess(other.into()));
            }
        };
        vulkan.wait_transfer();

        Ok(())
    }

    fn write_data<F>(&self, f: F) -> std::result::Result<(), Self::Error>
    where
        F: FnOnce(&mut B),
    {
        let vulkan = VK.get().unwrap();

        match &self.inner {
            GpuBufferInner::Fixed(_) => {
                return Err(GpuBufferError::UnsupportedAccess(BufferAccess::Fixed));
            }
            GpuBufferInner::Staged {
                buffer_id,
                staging_id,
            } => {
                let queue = vulkan.queues.transfer();

                vulkan.wait_transfer();
                vulkan.graphics_flight().unwrap().wait_idle().unwrap();
                unsafe {
                    vulkano_taskgraph::execute(
                        queue,
                        &vulkan.resources,
                        vulkan.transfer_flight,
                        |cb, ctx| {
                            let write: &mut B = ctx.write_buffer(*staging_id, ..)?;

                            f(write);

                            cb.copy_buffer(&CopyBufferInfo {
                                src_buffer: *staging_id,
                                dst_buffer: *buffer_id,
                                ..Default::default()
                            })?;

                            Ok(())
                        },
                        [(*staging_id, HostAccessType::Write)],
                        [
                            (*staging_id, AccessTypes::COPY_TRANSFER_READ),
                            (*buffer_id, AccessTypes::COPY_TRANSFER_WRITE),
                        ],
                        [],
                    )
                }
                .unwrap();
            }
            GpuBufferInner::Pinned { buffer_id, .. } => {
                // Wait for buffer access

                vulkan.wait_transfer();
                vulkan.graphics_flight().unwrap().wait_idle().unwrap();
                unsafe {
                    vulkano_taskgraph::execute(
                        vulkan.queues.transfer(),
                        &vulkan.resources,
                        vulkan.transfer_flight,
                        |_, ctx| {
                            let write = ctx.write_buffer(*buffer_id, ..)?;
                            f(write);

                            Ok(())
                        },
                        [(*buffer_id, HostAccessType::Write)],
                        [],
                        [],
                    )
                }
                .unwrap();
            }
            GpuBufferInner::RingBuffer {
                buffer_ids, turn, ..
            } => {
                // Write to next turn that the GPU is not currently accessing
                let index = (turn.load(Relaxed) + 1) % buffer_ids.len();
                let buffer_id = buffer_ids[index];

                vulkan.wait_transfer();
                unsafe {
                    vulkano_taskgraph::execute(
                        vulkan.queues.transfer(),
                        &vulkan.resources,
                        vulkan.transfer_flight,
                        |_, ctx| {
                            let write = ctx.write_buffer(buffer_id, ..)?;
                            f(write);

                            Ok(())
                        },
                        [(buffer_id, HostAccessType::Write)],
                        [],
                        [],
                    )
                }
                .unwrap();

                turn.store(index, Relaxed);
            }
        }
        vulkan.wait_transfer();

        Ok(())
    }
}
