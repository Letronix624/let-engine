use std::sync::Arc;

use anyhow::Result;
use let_engine_core::resources::{
    buffer::BufferAccess,
    model::{LoadedModel, Model, Vertex, VertexBufferDescription},
};
use parking_lot::Mutex;
use thiserror::Error;
use vulkano::{
    buffer::{Buffer, BufferCreateInfo, BufferUsage, Subbuffer},
    memory::allocator::{AllocationCreateInfo, MemoryTypeFilter},
    Validated,
};

use super::vulkan::VK;

use let_engine_core::resources::data;

/// Representation of a GPU loaded model made out of vertices V.
///
/// This structure does not contain any data but only handles to data stored on the GPU.
#[derive(Clone)]
pub struct GpuModel<V: Vertex = data::Vert> {
    inner_model: Arc<Mutex<InnerModel<V>>>,
    buffer_access: BufferAccess,
}

#[derive(Clone, Debug, PartialEq)]
pub struct InnerModel<V: Vertex> {
    vertex_sub_buffer: Subbuffer<[V]>,
    index_sub_buffer: Option<Subbuffer<[u32]>>,
    vertex_len: usize,
    index_len: usize,
}

impl<V: Vertex> PartialEq for GpuModel<V> {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.inner_model, &other.inner_model)
            && other.buffer_access == self.buffer_access
    }
}

impl<V: Vertex> GpuModel<V> {
    /// Loads the model into the GPU and returns an instance of this Model handle.
    pub fn new(model: &Model<V>) -> Result<Self, ModelError> {
        if model.is_empty() {
            return Err(ModelError::EmptyModel);
        }

        let buffer_access = *model.buffer_access();

        let vulkan = VK.get().ok_or(ModelError::BackendNotInitialized)?;

        let memory_allocator = vulkan.memory_allocator.clone();

        let vertex_sub_buffer = Buffer::new_slice::<V>(
            memory_allocator.clone(),
            BufferCreateInfo {
                usage: BufferUsage::VERTEX_BUFFER,
                ..Default::default()
            },
            AllocationCreateInfo {
                memory_type_filter: MemoryTypeFilter::PREFER_DEVICE
                    | MemoryTypeFilter::HOST_SEQUENTIAL_WRITE,
                ..Default::default()
            },
            model.max_vertices() as u64,
        )
        .map_err(Validated::unwrap)
        .map_err(ModelError::Allocation)?;

        {
            let mut write = vertex_sub_buffer.write().unwrap();

            write[0..model.vertex_len()].copy_from_slice(model.vertices());
        }

        let index_sub_buffer = if model.is_indexed() {
            let buffer = Buffer::new_slice::<u32>(
                memory_allocator.clone(),
                BufferCreateInfo {
                    usage: BufferUsage::INDEX_BUFFER,
                    ..Default::default()
                },
                AllocationCreateInfo {
                    memory_type_filter: MemoryTypeFilter::PREFER_DEVICE
                        | MemoryTypeFilter::HOST_SEQUENTIAL_WRITE,
                    ..Default::default()
                },
                model.index_len() as u64,
            )
            .map_err(Validated::unwrap)
            .map_err(ModelError::Allocation)?;

            {
                let mut write = buffer.write().unwrap();

                write[0..model.index_len()].copy_from_slice(model.indices());
            }

            Some(buffer)
        } else {
            None
        };

        Ok(Self {
            inner_model: Arc::new(Mutex::new(InnerModel {
                vertex_sub_buffer,
                index_sub_buffer,
                vertex_len: model.vertex_len(),
                index_len: model.index_len(),
            })),
            buffer_access,
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
        self.inner_model.lock().index_sub_buffer.is_some()
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
    fn read_vertices<R: FnMut(&[V])>(&self, mut f: R) -> Result<(), Self::Error> {
        let inner = self.inner_model.lock();
        let vertices = inner
            .vertex_sub_buffer
            .read()
            .map_err(ModelError::HostAccess)?;
        f(&vertices);

        Ok(())
    }

    /// Reads the index buffer from the GPU.
    ///
    /// Mind that reading from the GPU is slow.
    fn read_indices<R: FnMut(&[u32])>(&self, mut f: R) -> Result<(), Self::Error> {
        if let Some(index_sub_buffer) = self.inner_model.lock().index_sub_buffer.as_ref() {
            let mut index_buffer = index_sub_buffer.read().map_err(ModelError::HostAccess)?;
            f(&mut index_buffer);
        } else {
            f(&mut []);
        };

        Ok(())
    }

    /// Returns the number of elements present in the vertex buffer.
    fn vertex_count(&self) -> usize {
        self.inner_model.lock().vertex_len
    }

    /// Returns the number of elements present in the index buffer.
    ///
    /// Returns 0 when this model is not indexed.
    fn index_count(&self) -> usize {
        self.inner_model.lock().index_len
    }

    /// Takes a slice of vertices and copies it from slice to the GPU buffer.
    fn write_vertices(&self, vertices: &[V]) -> std::result::Result<(), Self::Error> {
        let mut inner = self.inner_model.lock();

        let vertices_len = vertices.len();

        if vertices_len > inner.vertex_sub_buffer.len() as usize {
            return Err(ModelError::BufferOverflow);
        };

        inner.vertex_len = vertices_len;

        let mut guard = inner
            .vertex_sub_buffer
            .write()
            .map_err(ModelError::HostAccess)?;

        guard[..vertices_len].copy_from_slice(vertices);

        Ok(())
    }

    /// Returns the maximum amounts of vertices that can be written in this buffer.
    fn max_vertices(&self) -> usize {
        self.inner_model.lock().vertex_sub_buffer.len() as usize
    }

    /// Takes a slice of indices and copies it from slice to the GPU buffer in case indices are on.
    fn write_indices(&self, indices: &[u32]) -> std::result::Result<(), Self::Error> {
        let mut inner = self.inner_model.lock();

        let indices_len = indices.len();

        if indices_len
            > inner
                .index_sub_buffer
                .as_ref()
                .map(|x| x.len() as usize)
                .unwrap_or_default()
        {
            return Err(ModelError::BufferOverflow);
        };

        inner.index_len = indices_len;

        if let Some(index_sub_buffer) = inner.index_sub_buffer.as_ref() {
            let mut guard = index_sub_buffer.write().map_err(ModelError::HostAccess)?;

            guard[..indices_len].copy_from_slice(indices);
        } else {
            return Err(ModelError::BufferOverflow);
        }

        Ok(())
    }

    /// Returns the maximum amounts of indices that can be written in this buffer.
    fn max_indices(&self) -> usize {
        self.inner_model
            .lock()
            .index_sub_buffer
            .as_ref()
            .map(|x| x.len() as usize)
            .unwrap_or_default()
    }
}

#[derive(Clone, Debug)]
pub struct DrawableModel {
    vertex_sub_buffer: Subbuffer<[u8]>,
    index_sub_buffer: Option<Subbuffer<[u32]>>,
    vertex_buffer_description: VertexBufferDescription,
    vertex_len: u32,
    index_len: u32,
}

impl DrawableModel {
    pub fn from_model<V: Vertex>(model: GpuModel<V>) -> Self {
        let inner = model.inner_model.lock();

        let vertex_sub_buffer = inner.vertex_sub_buffer.as_bytes().clone();

        let index_sub_buffer = inner.index_sub_buffer.clone();

        Self {
            vertex_sub_buffer,
            index_sub_buffer,
            vertex_buffer_description: V::description(),
            vertex_len: inner.vertex_len as u32,
            index_len: inner.index_len as u32,
        }
    }

    pub(crate) fn vertex_buffer_description(&self) -> &VertexBufferDescription {
        &self.vertex_buffer_description
    }

    pub(crate) fn vertex_buffer(&self) -> &Subbuffer<[u8]> {
        &self.vertex_sub_buffer
    }

    pub(crate) fn index_buffer(&self) -> Option<&Subbuffer<[u32]>> {
        self.index_sub_buffer.as_ref()
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
}
