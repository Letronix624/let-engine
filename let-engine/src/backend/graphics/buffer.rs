use std::{
    cell::OnceCell,
    marker::PhantomData,
    sync::{atomic::AtomicU8, Arc},
};

use bytemuck::AnyBitPattern;
use let_engine_core::resources::buffer::{
    Buffer, BufferAccess, BufferUsage, LoadedBuffer, PreferOperation,
};
use thiserror::Error;
use vulkano::{
    buffer::{Buffer as VkBuffer, BufferCreateInfo, BufferUsage as VkBufferUsage},
    descriptor_set::layout::DescriptorType,
    memory::allocator::{AllocationCreateInfo, DeviceLayout, MemoryTypeFilter},
    sync::{GpuFuture, HostAccessError},
};
use vulkano_taskgraph::{
    command_buffer::CopyBufferInfo,
    graph::TaskGraph,
    resource::{AccessTypes, HostAccessType},
    Id, Task,
};

use super::{vulkan::VK, VulkanError};

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

#[derive(Clone)]
pub struct GpuBuffer<T: AnyBitPattern + Send + Sync> {
    buffer_id: Id<VkBuffer>,
    access_method: AccessMethod,

    usage: BufferUsage,
    _phantom_data: PhantomData<Arc<T>>,
}

type BufferCreation = (Id<VkBuffer>, AccessMethod);

impl<T: AnyBitPattern + Send + Sync> GpuBuffer<T> {
    /// Creates a new buffer.
    pub fn new(buffer: Buffer<T>) -> Result<Self, GpuBufferError> {
        let vulkan = VK.get().ok_or(GpuBufferError::BackendNotInitialized)?;

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
        let buffer_id = vulkan
            .resources
            .create_buffer(
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
                DeviceLayout::new_sized::<T>(),
            )
            .unwrap();

        // If not staging or fixed, write data directly into the buffer.
        if staging_memory_type_filter.is_none() {
            unsafe {
                vulkano_taskgraph::execute(
                    vulkan.queues.get_transfer(),
                    &vulkan.resources,
                    vulkan.transfer_flight,
                    |_, ctx| {
                        let write = ctx.write_buffer::<T>(buffer_id, ..)?;

                        *write = *buffer;

                        Ok(())
                    },
                    [(buffer_id, HostAccessType::Write)],
                    [],
                    [],
                )
            }
            .unwrap();
        };

        let access_method = match buffer_access {
            BufferAccess::Fixed => {
                let staging_id = vulkan
                    .resources
                    .create_buffer(
                        BufferCreateInfo {
                            usage: usage
                                | VkBufferUsage::TRANSFER_SRC
                                | VkBufferUsage::TRANSFER_DST,
                            ..Default::default()
                        },
                        AllocationCreateInfo {
                            memory_type_filter: staging_memory_type_filter.unwrap(),
                            ..Default::default()
                        },
                        DeviceLayout::new_sized::<T>(),
                    )
                    .unwrap();

                // Copy staging buffer to data.
                unsafe {
                    vulkano_taskgraph::execute(
                        vulkan.queues.get_transfer(),
                        &vulkan.resources,
                        vulkan.transfer_flight,
                        |cb, ctx| {
                            // Write staging buffer
                            let write = ctx.write_buffer::<T>(staging_id, ..)?;

                            *write = *buffer;

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

                AccessMethod::Fixed
            }
            BufferAccess::Staged => {
                let staging_id = vulkan
                    .resources
                    .create_buffer(
                        BufferCreateInfo {
                            usage: usage
                                | VkBufferUsage::TRANSFER_SRC
                                | VkBufferUsage::TRANSFER_DST,
                            ..Default::default()
                        },
                        AllocationCreateInfo {
                            memory_type_filter: staging_memory_type_filter.unwrap(),
                            ..Default::default()
                        },
                        DeviceLayout::new_sized::<T>(),
                    )
                    .map_err(|e| GpuBufferError::Other(e.unwrap().into()))?;

                // Copy staging buffer to data.
                unsafe {
                    vulkano_taskgraph::execute(
                        vulkan.queues.get_transfer(),
                        &vulkan.resources,
                        vulkan.transfer_flight,
                        |cb, ctx| {
                            // Write staging buffer
                            let write = ctx.write_buffer::<T>(staging_id, ..)?;

                            *write = *buffer;

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

                AccessMethod::Staged { staging_id }
            }
            BufferAccess::Pinned(prefer) => AccessMethod::Pinned(prefer),
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
        };

        Ok(Self {
            buffer_id,
            access_method,

            usage: *buffer.usage(),
            _phantom_data: PhantomData,
        })
    }

    /// Creates a new buffer which can only be accessed in the shaders.
    pub fn new_gpu_only(size: u64, usage: BufferUsage) -> Result<Self, GpuBufferError> {
        let vulkan = VK.get().ok_or(GpuBufferError::BackendNotInitialized)?;

        let Some(size) = DeviceLayout::new_unsized::<T>(size) else {
            return Err(GpuBufferError::InvalidSize);
        };

        let buffer_id = vulkan
            .resources
            .create_buffer(
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
                size,
            )
            .map_err(|e| GpuBufferError::Other(e.unwrap().into()))?;

        Ok(Self {
            buffer_id,
            access_method: AccessMethod::Fixed,
            usage,
            _phantom_data: PhantomData,
        })
    }
}

#[derive(Clone, Debug)]
pub struct DrawableBuffer {
    buffer_id: Id<VkBuffer>,
    ring: Option<(Vec<Id<VkBuffer>>, Arc<AtomicU8>)>,
    usage: BufferUsage,
}

impl DrawableBuffer {
    pub fn from_buffer<B: AnyBitPattern + Send + Sync>(buffer: GpuBuffer<B>) -> Self {
        let ring = if let AccessMethod::RingBuffer { buffers, turn, .. } = &buffer.access_method {
            Some((buffers.clone(), turn.clone()))
        } else {
            None
        };

        Self {
            buffer_id: buffer.buffer_id,
            ring,
            usage: buffer.usage,
        }
    }

    pub fn buffer(&self) -> Id<VkBuffer> {
        // Roll the ring buffer at fetch
        // TODO change this: do this once per frame not multiple times. FIXME
        if let Some((buffers, turn)) = &self.ring {
            let index = turn.fetch_add(1, std::sync::atomic::Ordering::Relaxed) as usize
                % (buffers.len() + 1);
            if index == 0 {
                self.buffer_id.clone()
            } else {
                buffers[index - 1].clone()
            }
        } else {
            self.buffer_id.clone()
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
pub enum GpuBufferError {
    /// Returns when attempting to create a buffer,
    /// but the engine has not been started with [`Engine::start`](crate::Engine::start),
    /// or the backend has closed down.
    #[error("Can not create buffer: Engine not initialized.")]
    BackendNotInitialized,

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

struct TransferTask {
    src: Id<VkBuffer>,
    dst: Id<VkBuffer>,
}

impl Task for TransferTask {
    type World = ();

    unsafe fn execute(
        &self,
        cbf: &mut vulkano_taskgraph::command_buffer::RecordingCommandBuffer<'_>,
        _tcx: &mut vulkano_taskgraph::TaskContext<'_>,
        _world: &Self::World,
    ) -> vulkano_taskgraph::TaskResult {
        cbf.copy_buffer(&CopyBufferInfo {
            src_buffer: self.src,
            dst_buffer: self.dst,
            ..Default::default()
        })?;

        Ok(())
    }
}

struct ReadTask<'a, T: AnyBitPattern + Send + Sync> {
    src: Id<VkBuffer>,
    function: &'a dyn Fn(&T),
}

unsafe impl<T: AnyBitPattern + Send + Sync> Send for ReadTask<'_, T> {}
unsafe impl<T: AnyBitPattern + Send + Sync> Sync for ReadTask<'_, T> {}

impl<T: AnyBitPattern + Send + Sync> Task for ReadTask<'static, T> {
    type World = ();

    unsafe fn execute(
        &self,
        _cbf: &mut vulkano_taskgraph::command_buffer::RecordingCommandBuffer<'_>,
        tcx: &mut vulkano_taskgraph::TaskContext<'_>,
        _world: &Self::World,
    ) -> vulkano_taskgraph::TaskResult {
        let read: &T = tcx.read_buffer(self.src, ..)?;

        (self.function)(read);

        Ok(())
    }
}

impl<B: AnyBitPattern + Send + Sync> LoadedBuffer<B> for GpuBuffer<B> {
    type Error = GpuBufferError;

    fn data(&self) -> std::result::Result<B, Self::Error> {
        let vulkan = VK.get().ok_or(GpuBufferError::BackendNotInitialized)?;
        match &self.access_method {
            AccessMethod::Fixed => Err(GpuBufferError::UnsupportedAccess(BufferAccess::Fixed)),
            AccessMethod::Staged { staging_id } => {
                let task_graph = TaskGraph::new(&vulkan.resources);

                // Task 1: data -> staging
                let transfer = task_graph
                    .create_task_node(
                        "transfer",
                        vulkano_taskgraph::QueueFamilyType::Transfer,
                        TransferTask {
                            src: self.buffer_id,
                            dst: *staging_id,
                        },
                    )
                    .buffer_access(self.buffer_id, AccessTypes::COPY_TRANSFER_READ)
                    .buffer_access(*staging_id, AccessTypes::COPY_TRANSFER_WRITE)
                    .build();

                let mut buffer = Option<usize>;

                // Task 2: read staging
                let read = task_graph.create_task_node("read", vulkano_taskgraph::QueueFamilyType::Transfer, ReadTask {src: staging_id, function: |data| {
                }});

                todo!()
            }
            AccessMethod::Pinned(..) => {
                // Wait for data access
                if let Some(future) = vulkan.future.lock().take() {
                    future
                        .then_signal_fence_and_flush()
                        .unwrap()
                        .wait(None)
                        .unwrap();
                };

                // Read data
                Ok(*(self.data.read().map_err(GpuBufferError::HostAccess)?))
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
                Ok(*(buffer.read().map_err(GpuBufferError::HostAccess)?))
            }
        }
    }

    fn write_data_mut<F: FnOnce(&mut B)>(&self, mut f: F) -> std::result::Result<(), Self::Error> {
        let vulkan = VK.get().ok_or(GpuBufferError::BackendNotInitialized)?;
        match &self.access_method {
            AccessMethod::Fixed => {
                return Err(GpuBufferError::UnsupportedAccess(BufferAccess::Fixed))
            }
            AccessMethod::Staged { staging, write, .. } => {
                {
                    let mut guard = staging.write().map_err(GpuBufferError::HostAccess)?;

                    f(&mut guard);
                }

                Self::execute_command(write.clone(), &vulkan.queues, vulkan.future.clone())?;
            }
            AccessMethod::Pinned(..) => {
                // Wait for buffer access
                if let Some(future) = vulkan.future.lock().take() {
                    dbg!(future.queue());
                    future
                        .then_signal_fence_and_flush()
                        .unwrap()
                        .wait(None)
                        .unwrap();
                    dbg!("Waited");
                };

                // Write
                let mut guard = self.data.write().map_err(GpuBufferError::HostAccess)?;

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
                let mut guard = buffer.write().map_err(GpuBufferError::HostAccess)?;

                f(&mut guard);
            }
        }

        Ok(())
    }
}
