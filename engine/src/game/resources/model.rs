use super::Loader;
use crate::{data::Data, Vertex};
use vulkano::buffer::Subbuffer;

/// The model of an object made of vertices and indices.
#[derive(Clone, Debug, PartialEq)]
pub struct Model {
    vertex_sub_buffer: Subbuffer<[Vertex]>,
    index_sub_buffer: Subbuffer<[u32]>,
    data: Data,
}

impl Model {
    /// Makes a new model with given data.
    pub(crate) fn new(data: Data, loader: &mut Loader) -> Self {
        let vertex_sub_buffer = loader
            .vertex_buffer_allocator
            .allocate_slice(data.vertices.clone().len() as _)
            .unwrap();
        let index_sub_buffer = loader
            .index_buffer_allocator
            .allocate_slice(data.indices.clone().len() as _)
            .unwrap();

        vertex_sub_buffer
            .write()
            .unwrap()
            .copy_from_slice(&data.vertices);
        index_sub_buffer
            .write()
            .unwrap()
            .copy_from_slice(&data.indices);

        Self {
            vertex_sub_buffer,
            index_sub_buffer,
            data,
        }
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
