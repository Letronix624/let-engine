use crate::{error::NoDataError, prelude::*};
use anyhow::Result;
use parking_lot::Mutex;
use std::sync::Arc;
use vulkano::buffer::Subbuffer;

/// The custom model of an object made of vertices and indices.
#[derive(Clone, Debug, PartialEq)]
pub struct ModelData {
    vertex_sub_buffer: Subbuffer<[Vertex]>,
    index_sub_buffer: Subbuffer<[u32]>,
    data: Data,
}

impl ModelData {
    /// Makes a new model with given data.
    ///
    /// Can return an error in case the GPU memory is full.
    pub fn new(data: Data, resources: &impl Resource) -> Result<Self> {
        Self::new_from_loader(data, resources.resources().loader())
    }

    pub(crate) fn new_from_loader(data: Data, loader: &Arc<Mutex<Loader>>) -> Result<Self> {
        if data.is_empty() {
            return Err(NoDataError.into());
        }
        let loader = loader.lock();
        let vertex_sub_buffer = loader
            .vertex_buffer_allocator
            .allocate_slice(data.vertices.clone().len() as _)?;
        let index_sub_buffer = loader
            .index_buffer_allocator
            .allocate_slice(data.indices.clone().len() as _)?;

        vertex_sub_buffer.write()?.copy_from_slice(&data.vertices);
        index_sub_buffer.write()?.copy_from_slice(&data.indices);

        Ok(Self {
            vertex_sub_buffer,
            index_sub_buffer,
            data,
        })
    }

    /// Returns the index and vertex data of this object.
    pub fn get_data(&self) -> &Data {
        &self.data
    }

    /// Returns the size of this model in number of indices.
    pub fn get_size(&self) -> usize {
        self.data.indices.len()
    }

    pub(crate) fn get_vertex_buffer(&self) -> Subbuffer<[Vertex]> {
        self.vertex_sub_buffer.clone()
    }

    pub(crate) fn get_index_buffer(&self) -> Subbuffer<[u32]> {
        self.index_sub_buffer.clone()
    }
}

/// The model of an appearance.
#[derive(Clone, Debug, PartialEq)]
pub enum Model {
    /// Your own model data.
    Custom(ModelData),
    /// A default model most useful for most things.
    ///
    /// A square going from -1.0 to 1.0 in both x and y.
    Square,
    /// A triangle going from -1.0 to 1.0 in both x and y.
    Triangle,
}

impl Default for Model {
    fn default() -> Self {
        Self::Square
    }
}
