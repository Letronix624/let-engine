use std::{
    marker::PhantomData,
    sync::{atomic::AtomicUsize, Arc},
};

use anyhow::Result;
use concurrent_slotmap::{Key, SlotId};
use glam::Vec2;
use let_engine_core::resources::{
    buffer::{BufferAccess, PreferOperation},
    model::{LoadedModel, Model, Vertex, VertexBufferDescription},
};
use thiserror::Error;
use vulkano::{
    buffer::{Buffer, BufferCreateInfo, BufferUsage},
    memory::allocator::{AllocationCreateInfo, DeviceLayout, MemoryTypeFilter},
    DeviceSize,
};
use vulkano_taskgraph::{
    command_buffer::CopyBufferInfo,
    resource::{AccessTypes, HostAccessType},
    Id,
};

use super::{vulkan::VK, VulkanError};

#[derive(Clone)]
enum AccessMethod {
    Fixed,
    Staged {
        vertex_staging_id: Id<Buffer>,
        index_staging_id: Option<Id<Buffer>>,
    },
    Pinned(PreferOperation),
    // RingBuffer {
    //     buffers: Vec<Id<VkBuffer>>,
    //     turn: Arc<AtomicU8>,
    //     prefer: PreferOperation,
    // },
}

impl PartialEq for AccessMethod {
    fn eq(&self, other: &Self) -> bool {
        match self {
            Self::Fixed => other == &Self::Fixed,
            Self::Staged {
                vertex_staging_id,
                index_staging_id,
            } => {
                if let Self::Staged {
                    vertex_staging_id: vertex,
                    index_staging_id: index,
                } = other
                {
                    vertex_staging_id == vertex && index_staging_id == index
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
        }
    }
}

/// Representation of a GPU loaded model made out of vertices V.
///
/// This structure does not contain any data but only handles to data stored on the GPU.
pub struct GpuModel<V: Vertex = Vec2> {
    vertex_buffer_id: Id<Buffer>,
    index_buffer_id: Option<Id<Buffer>>,
    vertex_buffer_description: VertexBufferDescription,

    max_vertices: usize,
    max_indices: usize,
    vertex_len: AtomicUsize,
    index_len: AtomicUsize,

    access_method: AccessMethod,

    _phantom: PhantomData<Arc<V>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ModelId<T: Vertex>(SlotId, PhantomData<T>);

impl<T: Vertex> Key for ModelId<T> {
    #[inline]
    fn from_id(id: SlotId) -> Self {
        Self(id, PhantomData)
    }

    #[inline]
    fn as_id(self) -> SlotId {
        self.0
    }
}

impl<V: Vertex> PartialEq for GpuModel<V> {
    fn eq(&self, other: &Self) -> bool {
        self.vertex_buffer_id == other.vertex_buffer_id
            && self.index_buffer_id == other.index_buffer_id
            && other.access_method == self.access_method
    }
}

impl<V: Vertex> GpuModel<V> {
    /// Loads the model into the GPU and returns an instance of this Model handle.
    pub(crate) fn new(model: &Model<V>) -> Result<Self, ModelError> {
        if model.is_empty() {
            return Err(ModelError::EmptyModel);
        }

        let max_vertices = model.max_vertices();
        let max_indices = model.max_indices();

        let buffer_access = *model.buffer_access();

        let vulkan = VK.get().ok_or(ModelError::BackendNotInitialized)?;

        let (usage, memory_type_filter) = match buffer_access {
            BufferAccess::Fixed => (BufferUsage::TRANSFER_DST, MemoryTypeFilter::PREFER_DEVICE),
            BufferAccess::Pinned(PreferOperation::Read) => (
                BufferUsage::empty(),
                MemoryTypeFilter::PREFER_DEVICE | MemoryTypeFilter::HOST_RANDOM_ACCESS,
            ),
            BufferAccess::Pinned(PreferOperation::Write) => (
                BufferUsage::empty(),
                MemoryTypeFilter::PREFER_DEVICE | MemoryTypeFilter::HOST_SEQUENTIAL_WRITE,
            ),
            BufferAccess::Staged => (
                BufferUsage::TRANSFER_SRC | BufferUsage::TRANSFER_DST,
                MemoryTypeFilter::PREFER_HOST | MemoryTypeFilter::HOST_SEQUENTIAL_WRITE,
            ),
            access => return Err(ModelError::UnsupportedAccess(access)),
        };

        let vertex_layout = DeviceLayout::new_unsized::<[V]>(max_vertices as DeviceSize)
            .ok_or(ModelError::EmptyModel)?;
        let index_layout = DeviceLayout::new_unsized::<[u32]>(max_indices as DeviceSize);

        // Create buffers
        let vertex_buffer_id = vulkan
            .resources
            .create_buffer(
                &BufferCreateInfo {
                    usage: usage | BufferUsage::VERTEX_BUFFER,
                    ..Default::default()
                },
                &AllocationCreateInfo {
                    memory_type_filter,
                    ..Default::default()
                },
                vertex_layout,
            )
            .unwrap();

        let mut accesses = vec![(vertex_buffer_id, HostAccessType::Write)];

        let index_buffer_id = if model.is_indexed() {
            let index_buffer_id = vulkan
                .resources
                .create_buffer(
                    &BufferCreateInfo {
                        usage: usage | BufferUsage::INDEX_BUFFER,
                        ..Default::default()
                    },
                    &AllocationCreateInfo {
                        memory_type_filter,
                        ..Default::default()
                    },
                    index_layout.unwrap(),
                )
                .unwrap();
            accesses.push((index_buffer_id, HostAccessType::Write));

            Some(index_buffer_id)
        } else {
            None
        };

        let flight = vulkan.transfer_flight().unwrap();

        flight.wait(None).unwrap();

        let access_method = match buffer_access {
            BufferAccess::Fixed => {
                Self::staged_write(
                    vulkan,
                    vertex_layout,
                    index_layout,
                    vertex_buffer_id,
                    index_buffer_id,
                    model,
                );

                AccessMethod::Fixed
            }
            BufferAccess::Staged => {
                let (vertex_staging_id, index_staging_id) = Self::staged_write(
                    vulkan,
                    vertex_layout,
                    index_layout,
                    vertex_buffer_id,
                    index_buffer_id,
                    model,
                );

                AccessMethod::Staged {
                    vertex_staging_id,
                    index_staging_id,
                }
            }
            BufferAccess::Pinned(prefer_operation) => {
                // Next up: write to the buffers
                unsafe {
                    vulkano_taskgraph::execute(
                        vulkan.queues.transfer(),
                        &vulkan.resources,
                        vulkan.transfer_flight,
                        |_, ctx| {
                            let write: &mut [V] = ctx.write_buffer(
                                vertex_buffer_id,
                                0..(model.vertex_len() * std::mem::size_of::<V>()) as DeviceSize,
                            )?;

                            write.copy_from_slice(model.vertices());

                            if let Some(index_buffer_id) = index_buffer_id {
                                let write: &mut [u32] = ctx.write_buffer(
                                    index_buffer_id,
                                    0..(model.index_len() * 4) as DeviceSize,
                                )?;

                                write.copy_from_slice(model.indices());
                            }
                            Ok(())
                        },
                        accesses,
                        [],
                        [],
                    )
                }
                .unwrap();

                AccessMethod::Pinned(prefer_operation)
            }
            access => return Err(ModelError::UnsupportedAccess(access)),
        };

        vulkan.add_resource(super::vulkan::Resource::Buffer {
            id: vertex_buffer_id,
            access_types: AccessTypes::VERTEX_ATTRIBUTE_READ,
        });
        if let Some(id) = index_buffer_id {
            vulkan.add_resource(super::vulkan::Resource::Buffer {
                id,
                access_types: AccessTypes::INDEX_READ,
            });
        }

        Ok(Self {
            vertex_buffer_id,
            index_buffer_id,
            vertex_buffer_description: V::description(),
            max_vertices,
            max_indices,
            vertex_len: model.vertex_len().into(),
            index_len: model.index_len().into(),
            access_method,
            _phantom: PhantomData,
        })
    }

    /// Creates a new model that is only accessible in the shaders.
    pub(crate) fn new_gpu_only(
        vertex_size: DeviceSize,
        index_size: DeviceSize,
    ) -> Result<Self, ModelError> {
        let vulkan = VK.get().ok_or(ModelError::BackendNotInitialized)?;

        let Some(vertex_layout) = DeviceLayout::new_unsized::<[V]>(vertex_size) else {
            return Err(ModelError::EmptyModel);
        };
        let index_layout = DeviceLayout::new_unsized::<[u32]>(index_size);

        let vertex_buffer_id = vulkan
            .resources
            .create_buffer(
                &BufferCreateInfo {
                    usage: BufferUsage::VERTEX_BUFFER,
                    ..Default::default()
                },
                &AllocationCreateInfo {
                    memory_type_filter: MemoryTypeFilter::PREFER_DEVICE,
                    ..Default::default()
                },
                vertex_layout,
            )
            .unwrap();

        let index_buffer_id = index_layout.map(|size| {
            vulkan
                .resources
                .create_buffer(
                    &BufferCreateInfo {
                        usage: BufferUsage::INDEX_BUFFER,
                        ..Default::default()
                    },
                    &AllocationCreateInfo {
                        memory_type_filter: MemoryTypeFilter::PREFER_DEVICE,
                        ..Default::default()
                    },
                    size,
                )
                .unwrap()
        });

        vulkan.add_resource(super::vulkan::Resource::Buffer {
            id: vertex_buffer_id,
            access_types: AccessTypes::VERTEX_ATTRIBUTE_READ,
        });
        if let Some(id) = index_buffer_id {
            vulkan.add_resource(super::vulkan::Resource::Buffer {
                id,
                access_types: AccessTypes::INDEX_READ,
            });
        }

        Ok(Self {
            vertex_buffer_id,
            index_buffer_id,
            vertex_buffer_description: V::description(),
            max_vertices: vertex_size as usize,
            max_indices: index_size as usize,
            vertex_len: (vertex_size as usize).into(),
            index_len: (index_size as usize).into(),
            access_method: AccessMethod::Fixed,
            _phantom: PhantomData,
        })
    }

    fn staged_write(
        vulkan: &super::vulkan::Vulkan,
        vertex_layout: DeviceLayout,
        index_layout: Option<DeviceLayout>,
        vertex_buffer_id: Id<Buffer>,
        index_buffer_id: Option<Id<Buffer>>,
        model: &Model<V>,
    ) -> (Id<Buffer>, Option<Id<Buffer>>) {
        let vertex_staging_id = vulkan
            .resources
            .create_buffer(
                &BufferCreateInfo {
                    usage: BufferUsage::VERTEX_BUFFER
                        | BufferUsage::TRANSFER_SRC
                        | BufferUsage::TRANSFER_DST,
                    ..Default::default()
                },
                &AllocationCreateInfo {
                    memory_type_filter: MemoryTypeFilter::PREFER_HOST
                        | MemoryTypeFilter::HOST_SEQUENTIAL_WRITE,
                    ..Default::default()
                },
                vertex_layout,
            )
            .unwrap();
        let mut host_accesses = vec![(vertex_staging_id, HostAccessType::Write)];
        let mut buffer_accesses = vec![
            (vertex_staging_id, AccessTypes::COPY_TRANSFER_READ),
            (vertex_buffer_id, AccessTypes::COPY_TRANSFER_WRITE),
        ];
        let index_staging_id = if index_buffer_id.is_some() {
            Some(
                vulkan
                    .resources
                    .create_buffer(
                        &BufferCreateInfo {
                            usage: BufferUsage::INDEX_BUFFER
                                | BufferUsage::TRANSFER_SRC
                                | BufferUsage::TRANSFER_DST,
                            ..Default::default()
                        },
                        &AllocationCreateInfo {
                            memory_type_filter: MemoryTypeFilter::PREFER_HOST
                                | MemoryTypeFilter::HOST_SEQUENTIAL_WRITE,
                            ..Default::default()
                        },
                        index_layout.unwrap(),
                    )
                    .unwrap(),
            )
        } else {
            None
        };

        if let (Some(buffer_id), Some(staging_id)) = (index_buffer_id, index_staging_id) {
            host_accesses.push((staging_id, HostAccessType::Write));
            buffer_accesses.extend_from_slice(&[
                (buffer_id, AccessTypes::COPY_TRANSFER_WRITE),
                (staging_id, AccessTypes::COPY_TRANSFER_READ),
            ]);
        }

        unsafe {
            vulkano_taskgraph::execute(
                vulkan.queues.transfer(),
                &vulkan.resources,
                vulkan.transfer_flight,
                |cb, ctx| {
                    let vertex_write: &mut [V] = ctx.write_buffer(
                        vertex_staging_id,
                        0..(model.vertex_len() * std::mem::size_of::<V>()) as DeviceSize,
                    )?;
                    vertex_write.copy_from_slice(model.vertices());
                    cb.copy_buffer(&CopyBufferInfo {
                        src_buffer: vertex_staging_id,
                        dst_buffer: vertex_buffer_id,
                        ..Default::default()
                    })?;

                    if let (Some(buffer_id), Some(staging_id)) = (index_buffer_id, index_staging_id)
                    {
                        let write: &mut [u32] =
                            ctx.write_buffer(staging_id, 0..(model.index_len() * 4) as DeviceSize)?;
                        write.copy_from_slice(model.indices());
                        cb.copy_buffer(&CopyBufferInfo {
                            src_buffer: staging_id,
                            dst_buffer: buffer_id,
                            ..Default::default()
                        })?;
                    }

                    Ok(())
                },
                host_accesses,
                buffer_accesses,
                [],
            )
        }
        .unwrap();

        (vertex_staging_id, index_staging_id)
    }

    /// Returns the buffer access of this model.
    pub fn buffer_access(&self) -> BufferAccess {
        match self.access_method {
            AccessMethod::Fixed => BufferAccess::Fixed,
            AccessMethod::Staged { .. } => BufferAccess::Staged,
            AccessMethod::Pinned(prefer) => BufferAccess::Pinned(prefer),
        }
    }

    /// Returns true if this object supports indices.
    ///
    /// This model must be created with indices in the first place to return true.
    pub fn is_indexed(&self) -> bool {
        self.index_buffer_id.is_some()
    }

    /// Returns the size dimensions of this model.
    ///
    /// The first element is the vertex length and the second the index length.
    /// In case this object is not indexed, the second element will be 0.
    pub fn dimensions(&self) -> (usize, usize) {
        (self.vertex_count(), self.index_count())
    }

    /// Reads the GPU model back to a regular CPU model.
    ///
    /// Mind that reading from the GPU is slow.
    pub fn to_model(&self) -> Result<Model<V>, ModelError> {
        let mut model = Model::with_access(self.buffer_access());
        model.set_max_vertices(self.max_vertices);
        model.set_max_indices(self.max_indices);

        self.read_vertices(|read| model.set_vertices(read.to_vec()))?;

        self.read_indices(|read| model.set_indices(read.to_vec()))?;

        Ok(model)
    }

    /// Writes the data from the given model into this model.
    pub fn write_model(&self, model: &Model<V>) -> Result<(), ModelError> {
        if AccessMethod::Fixed == self.access_method {
            return Err(ModelError::UnsupportedAccess(BufferAccess::Fixed));
        };

        let new_vertex_len = model.vertex_len();
        let new_index_len = model.index_len();

        if new_vertex_len > self.max_vertices {
            return Err(ModelError::BufferOverflow);
        };

        if new_index_len > self.max_indices {
            return Err(ModelError::BufferOverflow);
        };

        let vulkan = VK.get().ok_or(ModelError::BackendNotInitialized)?;

        let queue = vulkan.queues.transfer();

        let flight = vulkan.transfer_flight().unwrap();

        match self.access_method {
            AccessMethod::Fixed => unreachable!(),
            AccessMethod::Staged {
                vertex_staging_id,
                index_staging_id,
            } => {
                flight.wait(None).map_err(|e| ModelError::Other(e.into()))?;

                let mut host_writes = vec![(vertex_staging_id, HostAccessType::Write)];
                let mut buffer_writes = vec![
                    (self.vertex_buffer_id, AccessTypes::COPY_TRANSFER_WRITE),
                    (vertex_staging_id, AccessTypes::COPY_TRANSFER_READ),
                ];

                if let (Some(index_buffer_id), Some(index_staging_id)) =
                    (self.index_buffer_id, index_staging_id)
                {
                    host_writes.push((index_staging_id, HostAccessType::Write));
                    buffer_writes.extend_from_slice(&[
                        (index_buffer_id, AccessTypes::COPY_TRANSFER_WRITE),
                        (index_staging_id, AccessTypes::COPY_TRANSFER_READ),
                    ]);
                };

                unsafe {
                    vulkano_taskgraph::execute(
                        queue,
                        &vulkan.resources,
                        vulkan.transfer_flight,
                        |cb, ctx| {
                            let vertices: &mut [V] = ctx.write_buffer(
                                vertex_staging_id,
                                0..std::mem::size_of_val(model.vertices()) as DeviceSize,
                            )?;

                            vertices.copy_from_slice(model.vertices());

                            cb.copy_buffer(&CopyBufferInfo {
                                src_buffer: vertex_staging_id,
                                dst_buffer: self.vertex_buffer_id,
                                ..Default::default()
                            })?;

                            if let (Some(index_buffer_id), Some(index_staging_id)) =
                                (self.index_buffer_id, index_staging_id)
                            {
                                let indices: &mut [u32] = ctx.write_buffer(
                                    index_staging_id,
                                    0..std::mem::size_of_val(model.indices()) as DeviceSize,
                                )?;

                                indices.copy_from_slice(model.indices());

                                cb.copy_buffer(&CopyBufferInfo {
                                    src_buffer: index_staging_id,
                                    dst_buffer: index_buffer_id,
                                    ..Default::default()
                                })?;
                            }

                            Ok(())
                        },
                        host_writes,
                        buffer_writes,
                        [],
                    )
                }
                .unwrap();
            }
            AccessMethod::Pinned(_) => {
                flight.wait(None).map_err(|e| ModelError::Other(e.into()))?;
                unsafe {
                    let mut host_accesses = vec![(self.vertex_buffer_id, HostAccessType::Write)];

                    if let Some(index_buffer_id) = self.index_buffer_id {
                        host_accesses.push((index_buffer_id, HostAccessType::Write));
                    }
                    vulkano_taskgraph::execute(
                        queue,
                        &vulkan.resources,
                        vulkan.transfer_flight,
                        |_, ctx| {
                            let vertices: &mut [V] = ctx.write_buffer(
                                self.vertex_buffer_id,
                                0..(std::mem::size_of_val(model.vertices())) as DeviceSize,
                            )?;

                            vertices.copy_from_slice(model.vertices());

                            if let Some(index_buffer_id) = self.index_buffer_id {
                                let indices: &mut [u32] = ctx.write_buffer(
                                    index_buffer_id,
                                    0..std::mem::size_of_val(model.indices()) as DeviceSize,
                                )?;

                                indices.copy_from_slice(model.indices());
                            }

                            Ok(())
                        },
                        host_accesses,
                        [],
                        [],
                    )
                }
                .unwrap();
            }
        }

        self.vertex_len
            .store(new_vertex_len, std::sync::atomic::Ordering::Relaxed);
        self.index_len
            .store(new_index_len, std::sync::atomic::Ordering::Relaxed);

        Ok(())
    }
}

impl<V: Vertex> GpuModel<V> {
    pub(crate) fn vertex_buffer_id(&self) -> Id<Buffer> {
        self.vertex_buffer_id
    }

    pub(crate) fn index_buffer_id(&self) -> Option<Id<Buffer>> {
        self.index_buffer_id
    }

    pub(crate) fn vertex_len(&self) -> usize {
        self.vertex_len.load(std::sync::atomic::Ordering::Relaxed)
    }

    pub(crate) fn index_len(&self) -> usize {
        self.index_len.load(std::sync::atomic::Ordering::Relaxed)
    }

    pub(crate) fn vertex_buffer_description(&self) -> &VertexBufferDescription {
        &self.vertex_buffer_description
    }
}

impl<V: Vertex> LoadedModel<V> for GpuModel<V> {
    type Error = ModelError;

    /// Reads only the vertex buffer from the GPU.
    ///
    /// Mind that reading from the GPU is slow.
    fn read_vertices<R: FnOnce(&[V])>(&self, f: R) -> Result<(), Self::Error> {
        if AccessMethod::Fixed == self.access_method {
            return Err(ModelError::UnsupportedAccess(BufferAccess::Fixed));
        };

        let vulkan = VK.get().ok_or(ModelError::BackendNotInitialized)?;

        let queue = vulkan.queues.transfer();

        let flight = vulkan.transfer_flight().unwrap();

        match &self.access_method {
            AccessMethod::Fixed => unreachable!(),
            AccessMethod::Staged {
                vertex_staging_id,
                index_staging_id: _,
            } => {
                flight.wait(None).unwrap();

                unsafe {
                    vulkano_taskgraph::execute(
                        queue,
                        &vulkan.resources,
                        vulkan.transfer_flight,
                        |cb, _| {
                            cb.copy_buffer(&CopyBufferInfo {
                                src_buffer: self.vertex_buffer_id,
                                dst_buffer: *vertex_staging_id,
                                ..Default::default()
                            })?;
                            Ok(())
                        },
                        [],
                        [
                            (self.vertex_buffer_id, AccessTypes::COPY_TRANSFER_READ),
                            (*vertex_staging_id, AccessTypes::COPY_TRANSFER_WRITE),
                        ],
                        [],
                    )
                }
                .unwrap();

                flight.wait(None).unwrap();

                unsafe {
                    vulkano_taskgraph::execute(
                        queue,
                        &vulkan.resources,
                        vulkan.transfer_flight,
                        |_, ctx| {
                            let read: &[V] = ctx.read_buffer(
                                *vertex_staging_id,
                                0..(self.vertex_count() * std::mem::size_of::<V>()) as DeviceSize,
                            )?;

                            f(read);

                            Ok(())
                        },
                        [(*vertex_staging_id, HostAccessType::Read)],
                        [],
                        [],
                    )
                }
                .unwrap();
            }
            AccessMethod::Pinned(_) => {
                unsafe {
                    vulkano_taskgraph::execute(
                        queue,
                        &vulkan.resources,
                        vulkan.transfer_flight,
                        |_, ctx| {
                            let read = ctx.read_buffer(
                                self.vertex_buffer_id,
                                0..(self.vertex_count() * std::mem::size_of::<V>()) as DeviceSize,
                            )?;
                            f(read);
                            Ok(())
                        },
                        [(self.vertex_buffer_id, HostAccessType::Read)],
                        [],
                        [],
                    )
                }
                .unwrap();
            }
        }

        Ok(())
    }

    /// Reads the index buffer from the GPU.
    ///
    /// Mind that reading from the GPU is slow.
    fn read_indices<R: FnOnce(&[u32])>(&self, f: R) -> Result<(), Self::Error> {
        if AccessMethod::Fixed == self.access_method {
            return Err(ModelError::UnsupportedAccess(BufferAccess::Fixed));
        };

        let vulkan = VK.get().ok_or(ModelError::BackendNotInitialized)?;

        let queue = vulkan.queues.transfer();

        if let Some(index_buffer_id) = self.index_buffer_id {
            let flight = vulkan.transfer_flight().unwrap();
            match self.access_method {
                AccessMethod::Fixed => unreachable!(),
                AccessMethod::Staged {
                    vertex_staging_id: _,
                    index_staging_id,
                } => {
                    flight.wait(None).map_err(|e| ModelError::Other(e.into()))?;

                    unsafe {
                        vulkano_taskgraph::execute(
                            queue,
                            &vulkan.resources,
                            vulkan.transfer_flight,
                            |cb, _| {
                                cb.copy_buffer(&CopyBufferInfo {
                                    src_buffer: self.index_buffer_id.unwrap(),
                                    dst_buffer: index_staging_id.unwrap(),
                                    ..Default::default()
                                })?;
                                Ok(())
                            },
                            [],
                            [
                                (
                                    self.index_buffer_id.unwrap(),
                                    AccessTypes::COPY_TRANSFER_READ,
                                ),
                                (index_staging_id.unwrap(), AccessTypes::COPY_TRANSFER_WRITE),
                            ],
                            [],
                        )
                    }
                    .unwrap();

                    flight.wait(None).map_err(|e| ModelError::Other(e.into()))?;

                    unsafe {
                        vulkano_taskgraph::execute(
                            queue,
                            &vulkan.resources,
                            vulkan.transfer_flight,
                            |_, ctx| {
                                let read: &[u32] = ctx.read_buffer(
                                    index_staging_id.unwrap(),
                                    0..(self.vertex_count() * 4) as DeviceSize,
                                )?;

                                f(read);

                                Ok(())
                            },
                            [(index_staging_id.unwrap(), HostAccessType::Read)],
                            [],
                            [],
                        )
                    }
                    .unwrap();
                }
                AccessMethod::Pinned(_) => {
                    unsafe {
                        vulkano_taskgraph::execute(
                            queue,
                            &vulkan.resources,
                            vulkan.transfer_flight,
                            |_, ctx| {
                                let read = ctx.read_buffer(
                                    index_buffer_id,
                                    0..(self.index_count() * 4) as DeviceSize,
                                )?;
                                f(read);
                                Ok(())
                            },
                            [(index_buffer_id, HostAccessType::Read)],
                            [],
                            [],
                        )
                    }
                    .unwrap();
                }
            }
        } else {
            f(&[]);
        };

        Ok(())
    }

    /// Returns the number of elements present in the vertex buffer.
    fn vertex_count(&self) -> usize {
        self.vertex_len.load(std::sync::atomic::Ordering::Relaxed)
    }

    /// Returns the maximum amounts of vertices that can be written in this buffer.
    fn max_vertices(&self) -> usize {
        self.max_vertices
    }

    /// Returns the number of elements present in the index buffer.
    ///
    /// Returns 0 when this model is not indexed.
    fn index_count(&self) -> usize {
        self.index_len.load(std::sync::atomic::Ordering::Relaxed)
    }

    /// Returns the maximum amounts of indices that can be written in this buffer.
    fn max_indices(&self) -> usize {
        self.max_indices
    }

    fn write_vertices<W: FnOnce(&mut [V])>(
        &self,
        f: W,
        new_vertex_size: usize,
    ) -> std::result::Result<(), Self::Error> {
        if let AccessMethod::Fixed = self.access_method {
            return Err(ModelError::UnsupportedAccess(BufferAccess::Fixed));
        };

        if new_vertex_size > self.max_vertices {
            return Err(ModelError::BufferOverflow);
        } else if new_vertex_size == 0 {
            return Err(ModelError::EmptyModel);
        };

        let vulkan = VK.get().ok_or(ModelError::BackendNotInitialized)?;

        let queue = vulkan.queues.transfer();

        let flight = vulkan.transfer_flight().unwrap();

        match self.access_method {
            AccessMethod::Fixed => unreachable!(),
            AccessMethod::Staged {
                vertex_staging_id, ..
            } => {
                flight.wait(None).map_err(|e| ModelError::Other(e.into()))?;
                unsafe {
                    vulkano_taskgraph::execute(
                        queue,
                        &vulkan.resources,
                        vulkan.transfer_flight,
                        |cb, ctx| {
                            let write = ctx.write_buffer(
                                vertex_staging_id,
                                0..(new_vertex_size * std::mem::size_of::<V>()) as DeviceSize,
                            )?;

                            f(write);

                            cb.copy_buffer(&CopyBufferInfo {
                                src_buffer: vertex_staging_id,
                                dst_buffer: self.vertex_buffer_id,
                                ..Default::default()
                            })?;

                            Ok(())
                        },
                        [(vertex_staging_id, HostAccessType::Write)],
                        [
                            (self.vertex_buffer_id, AccessTypes::COPY_TRANSFER_WRITE),
                            (vertex_staging_id, AccessTypes::COPY_TRANSFER_READ),
                        ],
                        [],
                    )
                }
                .unwrap();
            }
            AccessMethod::Pinned(_) => {
                flight.wait(None).map_err(|e| ModelError::Other(e.into()))?;
                unsafe {
                    vulkano_taskgraph::execute(
                        queue,
                        &vulkan.resources,
                        vulkan.transfer_flight,
                        |_, ctx| {
                            let write: &mut [V] = ctx.write_buffer(
                                self.vertex_buffer_id,
                                0..(new_vertex_size * std::mem::size_of::<V>()) as DeviceSize,
                            )?;

                            f(write);

                            Ok(())
                        },
                        [(self.vertex_buffer_id, HostAccessType::Write)],
                        [],
                        [],
                    )
                }
                .unwrap();
            }
        }

        self.vertex_len
            .store(new_vertex_size, std::sync::atomic::Ordering::Relaxed);

        Ok(())
    }

    fn write_indices<W: FnOnce(&mut [u32])>(
        &self,
        f: W,
        new_index_size: usize,
    ) -> std::result::Result<(), Self::Error> {
        if let AccessMethod::Fixed = self.access_method {
            return Err(ModelError::UnsupportedAccess(BufferAccess::Fixed));
        };

        let vulkan = VK.get().ok_or(ModelError::BackendNotInitialized)?;

        let queue = vulkan.queues.transfer();

        if new_index_size > self.max_indices {
            return Err(ModelError::BufferOverflow);
        } else if new_index_size == 0 {
            return Err(ModelError::EmptyModel);
        };

        if let Some(id) = self.index_buffer_id {
            let flight = vulkan.transfer_flight().unwrap();
            flight.wait(None).map_err(|e| ModelError::Other(e.into()))?;

            match self.access_method {
                AccessMethod::Fixed => unreachable!(),
                AccessMethod::Staged {
                    index_staging_id, ..
                } => {
                    let index_staging_id = index_staging_id.unwrap();
                    flight.wait(None).map_err(|e| ModelError::Other(e.into()))?;
                    unsafe {
                        vulkano_taskgraph::execute(
                            queue,
                            &vulkan.resources,
                            vulkan.transfer_flight,
                            |cb, ctx| {
                                let write = ctx.write_buffer(
                                    index_staging_id,
                                    0..(new_index_size * 4) as DeviceSize,
                                )?;

                                f(write);

                                cb.copy_buffer(&CopyBufferInfo {
                                    src_buffer: index_staging_id,
                                    dst_buffer: id,
                                    ..Default::default()
                                })?;

                                Ok(())
                            },
                            [(index_staging_id, HostAccessType::Write)],
                            [
                                (id, AccessTypes::COPY_TRANSFER_WRITE),
                                (index_staging_id, AccessTypes::COPY_TRANSFER_READ),
                            ],
                            [],
                        )
                    }
                    .unwrap();
                }
                AccessMethod::Pinned(_) => {
                    unsafe {
                        vulkano_taskgraph::execute(
                            queue,
                            &vulkan.resources,
                            vulkan.transfer_flight,
                            |_, ctx| {
                                let write: &mut [u32] =
                                    ctx.write_buffer(id, 0..(new_index_size * 4) as DeviceSize)?;

                                f(write);

                                Ok(())
                            },
                            [(id, HostAccessType::Write)],
                            [],
                            [],
                        )
                    }
                    .unwrap();
                }
            }
        }

        self.index_len
            .store(new_index_size, std::sync::atomic::Ordering::Relaxed);

        Ok(())
    }
}

pub use vulkano::{buffer::AllocateBufferError, sync::HostAccessError};

/// Errors that occur in the context of models.
#[derive(Debug, Error)]
pub enum ModelError {
    /// Returns when attempting to create a model,
    /// but the engine has not been started with [`Engine::start`](crate::Engine::start),
    /// or the backend has closed down.
    #[error("Can not create model: Engine not initialized.")]
    BackendNotInitialized,

    /// Returns when the access operation is not supported with the currently set access setting.
    #[error("Requested access operation not possible with current access setting: {0:?}")]
    UnsupportedAccess(BufferAccess),

    /// Returns if the provided model for the creation function contains no data.
    #[error("The provided model for the creation of a GPU model instance is empty.")]
    EmptyModel,

    /// Returns if a problem occurs when trying to access the data from the GPU.
    #[error("{0}")]
    HostAccess(HostAccessError),

    /// Returns when there was a problem allocating a buffer.
    #[error("{0}")]
    Allocation(AllocateBufferError),

    /// Returned when the data provided to a write method exceeds the buffer's capacity.
    #[error("Provided more elements than the buffer's size allows.")]
    BufferOverflow,

    /// Returns when an unexpected Vulkan error occurs.
    #[error("An unexpected Vulkan error occured: {0}")]
    Other(VulkanError),
}
