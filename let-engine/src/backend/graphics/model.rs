use std::{
    marker::PhantomData,
    sync::{atomic::AtomicUsize, Arc},
};

use anyhow::Result;
use let_engine_core::resources::{
    buffer::BufferAccess,
    model::{LoadedModel, Model, Vertex, VertexBufferDescription},
};
use thiserror::Error;
use vulkano::{
    buffer::{Buffer, BufferCreateInfo, BufferUsage},
    memory::allocator::{AllocationCreateInfo, DeviceLayout, MemoryTypeFilter},
};
use vulkano_taskgraph::{resource::HostAccessType, Id};

use super::{vulkan::VK, VulkanError};

use let_engine_core::resources::data;

/// Representation of a GPU loaded model made out of vertices V.
///
/// This structure does not contain any data but only handles to data stored on the GPU.
#[derive(Clone)]
pub struct GpuModel<V: Vertex = data::Vert> {
    vertex_buffer_id: Id<Buffer>,
    index_buffer_id: Option<Id<Buffer>>,

    max_vertices: usize,
    max_indices: usize,
    vertex_len: Arc<AtomicUsize>,
    index_len: Arc<AtomicUsize>,

    buffer_access: BufferAccess,
    _phantom: PhantomData<Arc<V>>,
}

impl<V: Vertex> PartialEq for GpuModel<V> {
    fn eq(&self, other: &Self) -> bool {
        self.vertex_buffer_id == other.vertex_buffer_id
            && self.index_buffer_id == other.index_buffer_id
            && other.buffer_access == self.buffer_access
    }
}

impl<V: Vertex> GpuModel<V> {
    /// Loads the model into the GPU and returns an instance of this Model handle.
    pub fn new(model: &Model<V>) -> Result<Self, ModelError> {
        if model.is_empty() {
            return Err(ModelError::EmptyModel);
        }

        let max_vertices = model.max_vertices();
        let max_indices = model.max_indices();

        let buffer_access = *model.buffer_access();

        let vulkan = VK.get().ok_or(ModelError::BackendNotInitialized)?;

        // Create buffers
        let vertex_buffer_id = vulkan
            .resources
            .create_buffer(
                BufferCreateInfo {
                    usage: BufferUsage::VERTEX_BUFFER,
                    ..Default::default()
                },
                AllocationCreateInfo {
                    memory_type_filter: MemoryTypeFilter::PREFER_DEVICE
                        | MemoryTypeFilter::HOST_SEQUENTIAL_WRITE,
                    ..Default::default()
                },
                DeviceLayout::new_unsized::<[V]>(max_vertices as u64).unwrap(),
            )
            .unwrap();

        let index_buffer_id = if model.is_indexed() {
            let index_buffer_id = vulkan
                .resources
                .create_buffer(
                    BufferCreateInfo {
                        usage: BufferUsage::INDEX_BUFFER,
                        ..Default::default()
                    },
                    AllocationCreateInfo {
                        memory_type_filter: MemoryTypeFilter::PREFER_DEVICE
                            | MemoryTypeFilter::HOST_SEQUENTIAL_WRITE,
                        ..Default::default()
                    },
                    DeviceLayout::new_unsized::<[u32]>(max_indices as u64).unwrap(),
                )
                .unwrap();

            Some(index_buffer_id)
        } else {
            None
        };

        // Next up: write to the buffers
        unsafe {
            vulkano_taskgraph::execute(
                &vulkan.queues.get_transfer(),
                &vulkan.resources,
                vulkan.transfer_flight,
                |_, ctx| {
                    let write: &mut [V] =
                        ctx.write_buffer(vertex_buffer_id, 0..model.vertex_len() as u64)?;

                    write.copy_from_slice(model.vertices());

                    if let Some(index_buffer_id) = index_buffer_id {
                        let write: &mut [u32] =
                            ctx.write_buffer(index_buffer_id, 0..model.index_len() as u64)?;

                        write.copy_from_slice(model.indices());
                    }
                    Ok(())
                },
                [(vertex_buffer_id, HostAccessType::Write)],
                [],
                [],
            )
        }
        .unwrap();

        Ok(Self {
            vertex_buffer_id,
            index_buffer_id,
            max_vertices,
            max_indices,
            vertex_len: Arc::new(model.vertex_len().into()),
            index_len: Arc::new(model.index_len().into()),
            buffer_access,
            _phantom: PhantomData,
        })
    }

    /// Reads the model data from the GPU and returns it.
    ///
    /// Mind that reading from the GPU is slow.
    pub fn model(&self) -> Result<Model<V>, ModelError> {
        let mut model = Model::default();

        self.read_vertices(|vertices| {
            model.set_vertices(vertices.to_vec());
        })?;
        self.read_indices(|indices| {
            model.set_indices(indices.to_vec());
        })?;

        Ok(model)
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

    /// Writes the data of the given model to the GPU.
    pub fn write(&self, model: Model<V>) -> Result<(), ModelError> {
        if model.vertex_len() > self.max_vertices() {
            return Err(ModelError::BufferOverflow);
        }

        if model.index_len() > self.max_vertices() {
            return Err(ModelError::BufferOverflow);
        }

        self.write_vertices(model.vertices())?;

        if model.is_indexed() {
            self.write_indices(model.indices())?;
        }

        Ok(())
    }
}

impl<V: Vertex> LoadedModel<V> for GpuModel<V> {
    type Error = ModelError;

    /// Reads only the vertex buffer from the GPU.
    ///
    /// Mind that reading from the GPU is slow.
    fn read_vertices<R: FnOnce(&[V])>(&self, f: R) -> Result<(), Self::Error> {
        let vulkan = VK.get().ok_or(ModelError::BackendNotInitialized)?;

        let flight = vulkan.resources.flight(vulkan.graphics_flight).unwrap();
        flight.wait(None).map_err(|e| ModelError::Other(e.into()))?;

        unsafe {
            vulkano_taskgraph::execute(
                vulkan.queues.get_transfer(),
                &vulkan.resources,
                vulkan.transfer_flight,
                |_, ctx| {
                    let read =
                        ctx.read_buffer(self.vertex_buffer_id, 0..self.vertex_count() as u64)?;
                    f(read);
                    Ok(())
                },
                [(self.vertex_buffer_id, HostAccessType::Read)],
                [],
                [],
            )
        }
        .unwrap();

        Ok(())
    }

    /// Reads the index buffer from the GPU.
    ///
    /// Mind that reading from the GPU is slow.
    fn read_indices<R: FnOnce(&[u32])>(&self, f: R) -> Result<(), Self::Error> {
        let vulkan = VK.get().ok_or(ModelError::BackendNotInitialized)?;

        if let Some(index_buffer_id) = self.index_buffer_id {
            let flight = vulkan.resources.flight(vulkan.graphics_flight).unwrap();
            flight.wait(None).map_err(|e| ModelError::Other(e.into()))?;

            unsafe {
                vulkano_taskgraph::execute(
                    vulkan.queues.get_transfer(),
                    &vulkan.resources,
                    vulkan.transfer_flight,
                    |_, ctx| {
                        let read =
                            ctx.read_buffer(index_buffer_id, 0..self.index_count() as u64)?;
                        f(read);
                        Ok(())
                    },
                    [(index_buffer_id, HostAccessType::Read)],
                    [],
                    [],
                )
            }
            .unwrap();
        } else {
            f(&[]);
        };

        Ok(())
    }

    /// Returns the number of elements present in the vertex buffer.
    fn vertex_count(&self) -> usize {
        self.vertex_len.load(std::sync::atomic::Ordering::Relaxed)
    }

    /// Returns the number of elements present in the index buffer.
    ///
    /// Returns 0 when this model is not indexed.
    fn index_count(&self) -> usize {
        self.index_len.load(std::sync::atomic::Ordering::Relaxed)
    }

    /// Takes a slice of vertices and copies it from slice to the GPU buffer.
    fn write_vertices(&self, vertices: &[V]) -> std::result::Result<(), Self::Error> {
        let vulkan = VK.get().ok_or(ModelError::BackendNotInitialized)?;

        let flight = vulkan.resources.flight(vulkan.graphics_flight).unwrap();
        flight.wait(None).map_err(|e| ModelError::Other(e.into()))?;

        let vertices_len = vertices.len();

        if vertices_len > self.max_vertices {
            return Err(ModelError::BufferOverflow);
        };

        unsafe {
            vulkano_taskgraph::execute(
                vulkan.queues.get_transfer(),
                &vulkan.resources,
                vulkan.transfer_flight,
                |_, ctx| {
                    let write: &mut [V] =
                        ctx.write_buffer(self.vertex_buffer_id, 0..vertices_len as u64)?;

                    write.copy_from_slice(vertices);

                    Ok(())
                },
                [(self.vertex_buffer_id, HostAccessType::Write)],
                [],
                [],
            )
        }
        .unwrap();

        self.vertex_len
            .store(vertices_len, std::sync::atomic::Ordering::Relaxed);

        Ok(())
    }

    /// Returns the maximum amounts of vertices that can be written in this buffer.
    fn max_vertices(&self) -> usize {
        self.max_vertices
    }

    /// Takes a slice of indices and copies it from slice to the GPU buffer in case indices are on.
    fn write_indices(&self, indices: &[u32]) -> std::result::Result<(), Self::Error> {
        let vulkan = VK.get().ok_or(ModelError::BackendNotInitialized)?;

        let indices_len = indices.len();

        if indices_len > self.max_indices {
            return Err(ModelError::BufferOverflow);
        };

        if let Some(id) = self.index_buffer_id {
            let flight = vulkan.resources.flight(vulkan.graphics_flight).unwrap();
            flight.wait(None).map_err(|e| ModelError::Other(e.into()))?;

            unsafe {
                vulkano_taskgraph::execute(
                    vulkan.queues.get_transfer(),
                    &vulkan.resources,
                    vulkan.transfer_flight,
                    |_, ctx| {
                        let write: &mut [u32] = ctx.write_buffer(id, 0..indices_len as u64)?;

                        write.copy_from_slice(indices);

                        Ok(())
                    },
                    [(id, HostAccessType::Write)],
                    [],
                    [],
                )
            }
            .unwrap();
        }

        self.index_len
            .store(indices_len, std::sync::atomic::Ordering::Relaxed);

        Ok(())
    }

    /// Returns the maximum amounts of indices that can be written in this buffer.
    fn max_indices(&self) -> usize {
        self.max_indices
    }
}

#[derive(Clone, Debug)]
pub struct DrawableModel {
    vertex_buffer_id: Id<Buffer>,
    index_buffer_id: Option<Id<Buffer>>,
    vertex_buffer_description: VertexBufferDescription,
    vertex_len: u32,
    index_len: u32,
}

impl DrawableModel {
    pub fn from_model<V: Vertex>(model: GpuModel<V>) -> Self {
        Self {
            vertex_buffer_id: model.vertex_buffer_id,
            index_buffer_id: model.index_buffer_id,
            vertex_buffer_description: V::description(),
            vertex_len: model.vertex_count() as u32,
            index_len: model.index_count() as u32,
        }
    }

    pub(crate) fn vertex_buffer_description(&self) -> &VertexBufferDescription {
        &self.vertex_buffer_description
    }

    pub(crate) fn vertex_buffer_id(&self) -> &Id<Buffer> {
        &self.vertex_buffer_id
    }

    pub(crate) fn index_buffer_id(&self) -> Option<&Id<Buffer>> {
        self.index_buffer_id.as_ref()
    }

    /// Returns the number of vertices this model contains.
    pub fn vertex_len(&self) -> u32 {
        self.vertex_len
    }

    /// Returns the number of indices this model contains.
    ///
    /// Returns 0 when this model is not indexed.
    pub fn index_len(&self) -> u32 {
        self.index_len
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
