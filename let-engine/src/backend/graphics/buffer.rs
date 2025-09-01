use std::{
    marker::PhantomData,
    sync::{atomic::AtomicU8, Arc},
};

use concurrent_slotmap::{Key, SlotId};
use let_engine_core::resources::{
    buffer::{Buffer, BufferAccess, BufferUsage, LoadedBuffer, PreferOperation},
    data::Data,
};
use thiserror::Error;
use vulkano::{
    buffer::{Buffer as VkBuffer, BufferCreateInfo, BufferUsage as VkBufferUsage},
    descriptor_set::layout::DescriptorType,
    memory::allocator::{AllocationCreateInfo, DeviceLayout, MemoryTypeFilter},
    sync::HostAccessError,
    DeviceSize,
};
use vulkano_taskgraph::{
    command_buffer::CopyBufferInfo,
    resource::{AccessTypes, HostAccessType},
    Id,
};

use super::{
    vulkan::{Vulkan, VK},
    VulkanError,
};

#[derive(Clone)]
enum AccessMethod {
    Fixed,
    Staged {
        staging_id: Id<VkBuffer>,
    },
    Pinned(PreferOperation),
    RingBuffer {
        buffers: Vec<Id<VkBuffer>>,
        turn: Arc<AtomicU8>,
        prefer: PreferOperation,
    },
}

impl PartialEq for AccessMethod {
    fn eq(&self, other: &Self) -> bool {
        match self {
            Self::Fixed => other == &Self::Fixed,
            Self::Staged { staging_id } => {
                if let Self::Staged {
                    staging_id: otherid,
                } = other
                {
                    staging_id == otherid
                } else {
                    false
                }
            }
            Self::Pinned(a) => {
                if let Self::Pinned(b) = other {
                    a == b
                } else {
                    false
                }
            }
            Self::RingBuffer {
                buffers: b1,
                turn: t1,
                prefer: p1,
            } => {
                if let Self::RingBuffer {
                    buffers: b2,
                    turn: t2,
                    prefer: p2,
                } = other
                {
                    b1 == b2 && p1 == p2 && Arc::ptr_eq(t1, t2)
                } else {
                    false
                }
            }
        }
    }
}

impl From<AccessMethod> for BufferAccess {
    fn from(value: AccessMethod) -> Self {
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

/// A GPU loaded representation of a data buffer.
#[derive(PartialEq)]
pub struct GpuBuffer<T: Data> {
    pub(crate) buffer_id: Id<VkBuffer>,
    access_method: AccessMethod,

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
        let buffer_usage = *buffer.usage();
        let usage = match buffer_usage {
            BufferUsage::Uniform => VkBufferUsage::UNIFORM_BUFFER,
            BufferUsage::Storage => VkBufferUsage::STORAGE_BUFFER,
        };

        let buffer_access = *buffer.optimisation();

        let memory_type_filter = match buffer_access {
            BufferAccess::Fixed | BufferAccess::Staged => MemoryTypeFilter::PREFER_DEVICE,

            BufferAccess::Pinned(PreferOperation::Read) => {
                MemoryTypeFilter::PREFER_DEVICE | MemoryTypeFilter::HOST_RANDOM_ACCESS
            }
            BufferAccess::Pinned(PreferOperation::Write) => {
                MemoryTypeFilter::PREFER_DEVICE | MemoryTypeFilter::HOST_SEQUENTIAL_WRITE
            }
            BufferAccess::RingBuffer {
                prefer_operation: PreferOperation::Read,
                ..
            } => MemoryTypeFilter::PREFER_DEVICE | MemoryTypeFilter::HOST_RANDOM_ACCESS,
            BufferAccess::RingBuffer {
                prefer_operation: PreferOperation::Write,
                ..
            } => MemoryTypeFilter::PREFER_DEVICE | MemoryTypeFilter::HOST_SEQUENTIAL_WRITE,
            access => return Err(GpuBufferError::UnsupportedAccess(access)),
        };

        // Create data buffer
        let buffer_id = vulkan
            .resources
            .create_buffer(
                &BufferCreateInfo {
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
                &AllocationCreateInfo {
                    memory_type_filter,
                    ..Default::default()
                },
                DeviceLayout::new_sized::<T>(),
            )
            .unwrap();

        let access_method = match buffer_access {
            BufferAccess::Fixed => {
                Self::write_staging(vulkan, usage, buffer, buffer_id);

                AccessMethod::Fixed
            }
            BufferAccess::Staged => {
                let staging_id = Self::write_staging(vulkan, usage, buffer, buffer_id);

                AccessMethod::Staged { staging_id }
            }
            BufferAccess::Pinned(prefer) => {
                let flight = vulkan.transfer_flight().unwrap();

                flight.wait(None).unwrap();
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

                AccessMethod::Pinned(prefer)
            }
            BufferAccess::RingBuffer {
                prefer_operation: _,
                buffers: _,
            } => {
                // let buffer_count = buffers - 1;

                // let mut buffers = Vec::with_capacity(buffer_count);

                // for _ in 0..buffer_count {
                //     // Create other ring buffers
                //     let data = vulkano::buffer::Buffer::new_sized::<T>(
                //         memory_allocator.clone(),
                //         BufferCreateInfo {
                //             usage,
                //             ..Default::default()
                //         },
                //         AllocationCreateInfo {
                //             memory_type_filter,
                //             ..Default::default()
                //         },
                //     )
                //     .map_err(|e| GpuBufferError::Other(e.unwrap().into()))?;

                //     // Write data into this buffer
                //     *data.write().unwrap() = *buffer;

                //     let buffer_id = vulkan.resources.add_buffer(data.buffer().clone());

                //     buffers.push(buffer_id);
                // }

                // let turn = Arc::new(0.into());

                // AccessMethod::RingBuffer {
                //     buffers,
                //     turn,
                //     prefer: prefer_operation,
                // }
                todo!();
            }
            access => return Err(GpuBufferError::UnsupportedAccess(access)),
        };

        let access_types = match buffer.usage() {
            BufferUsage::Uniform => {
                AccessTypes::VERTEX_SHADER_UNIFORM_READ | AccessTypes::FRAGMENT_SHADER_UNIFORM_READ
            }
            BufferUsage::Storage => {
                AccessTypes::FRAGMENT_SHADER_STORAGE_READ | AccessTypes::VERTEX_SHADER_STORAGE_READ
            }
        };

        vulkan.add_resource(super::vulkan::Resource::Buffer {
            id: buffer_id,
            access_types,
        });

        Ok(Self {
            buffer_id,
            access_method,

            usage: *buffer.usage(),
            _phantom_data: PhantomData,
        })
    }

    fn write_staging(
        vulkan: &super::Vulkan,
        usage: VkBufferUsage,
        buffer: &Buffer<T>,
        buffer_id: Id<VkBuffer>,
    ) -> Id<VkBuffer> {
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

        let flight = vulkan.resources.flight(vulkan.transfer_flight).unwrap();

        flight.wait(None).unwrap();
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

        staging_id
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

        let access_types = match usage {
            BufferUsage::Uniform => {
                AccessTypes::VERTEX_SHADER_UNIFORM_READ | AccessTypes::FRAGMENT_SHADER_UNIFORM_READ
            }
            BufferUsage::Storage => {
                AccessTypes::FRAGMENT_SHADER_STORAGE_READ | AccessTypes::VERTEX_SHADER_STORAGE_READ
            }
        };

        vulkan.add_resource(super::vulkan::Resource::Buffer {
            id: buffer_id,
            access_types,
        });

        Ok(Self {
            buffer_id,
            access_method: AccessMethod::Fixed,
            usage,
            _phantom_data: PhantomData,
        })
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

impl<B: Data> LoadedBuffer<B> for GpuBuffer<B> {
    type Error = GpuBufferError;

    fn data<F>(&self, f: F) -> std::result::Result<(), Self::Error>
    where
        F: FnOnce(&B),
    {
        let vulkan = VK.get().unwrap();

        let queue = vulkan.queues.transfer();

        let flight = vulkan.transfer_flight().unwrap();

        match &self.access_method {
            AccessMethod::Fixed => {
                return Err(GpuBufferError::UnsupportedAccess(BufferAccess::Fixed))
            }
            AccessMethod::Staged { staging_id } => {
                flight.wait(None).unwrap();

                // Task 1: data -> staging
                unsafe {
                    vulkano_taskgraph::execute(
                        queue,
                        &vulkan.resources,
                        vulkan.transfer_flight,
                        |cb, _| {
                            cb.copy_buffer(&CopyBufferInfo {
                                src_buffer: self.buffer_id,
                                dst_buffer: *staging_id,
                                ..Default::default()
                            })?;
                            Ok(())
                        },
                        [],
                        [
                            (self.buffer_id, AccessTypes::COPY_TRANSFER_READ),
                            (*staging_id, AccessTypes::COPY_TRANSFER_WRITE),
                        ],
                        [],
                    )
                }
                .unwrap();

                flight.wait(None).unwrap();

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
            AccessMethod::Pinned(..) => {
                flight.wait(None).unwrap();

                unsafe {
                    vulkano_taskgraph::execute(
                        queue,
                        &vulkan.resources,
                        vulkan.transfer_flight,
                        |_, ctx| {
                            let read = ctx.read_buffer(self.buffer_id, ..)?;
                            f(read);
                            Ok(())
                        },
                        [(self.buffer_id, HostAccessType::Read)],
                        [],
                        [],
                    )
                }
                .unwrap();
            }
            AccessMethod::RingBuffer { .. } => {
                todo!()
            }
        };

        Ok(())
    }

    fn write_data<F>(&self, f: F) -> std::result::Result<(), Self::Error>
    where
        F: FnOnce(&mut B),
    {
        let vulkan = VK.get().unwrap();
        match &self.access_method {
            AccessMethod::Fixed => {
                return Err(GpuBufferError::UnsupportedAccess(BufferAccess::Fixed))
            }
            AccessMethod::Staged { staging_id } => {
                let queue = vulkan.queues.transfer();
                let flight = vulkan.transfer_flight().unwrap();

                flight.wait(None).unwrap();

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
                                dst_buffer: self.buffer_id,
                                ..Default::default()
                            })?;

                            Ok(())
                        },
                        [(*staging_id, HostAccessType::Write)],
                        [
                            (*staging_id, AccessTypes::COPY_TRANSFER_READ),
                            (self.buffer_id, AccessTypes::COPY_TRANSFER_WRITE),
                        ],
                        [],
                    )
                }
                .unwrap();
            }
            AccessMethod::Pinned(..) => {
                // Wait for buffer access
                let flight = vulkan.transfer_flight().unwrap();

                flight.wait(None).unwrap();

                unsafe {
                    vulkano_taskgraph::execute(
                        vulkan.queues.transfer(),
                        &vulkan.resources,
                        vulkan.transfer_flight,
                        |_, ctx| {
                            let write = ctx.write_buffer(self.buffer_id, ..)?;
                            f(write);

                            Ok(())
                        },
                        [(self.buffer_id, HostAccessType::Write)],
                        [],
                        [],
                    )
                }
                .unwrap();
            }
            AccessMethod::RingBuffer { .. } => {
                todo!()
            }
        }

        Ok(())
    }
}
