use std::collections::BTreeMap;

use anyhow::Result;
use glam::UVec2;

#[cfg(feature = "client")]
use crate::scenes::Scene;
use crate::{
    objects::Descriptor,
    resources::{
        buffer::{Buffer, BufferUsage, LoadedBuffer, Location},
        data::Data,
        material::Material,
        model::{LoadedModel, Model, Vertex},
        texture::{LoadedTexture, Texture, TextureSettings, ViewTypeDim},
    },
};

/// Definition for a gpu backend for the let-engine.
pub trait GpuBackend: Sized {
    type Error: std::error::Error + Send + Sync;

    /// Will be stored in the [`EngineContext`](crate::engine::EngineContext)
    /// to interface the backend from multiple threads.
    type Interface: GpuInterfacer<Self::LoadedTypes>;

    /// Settings used by the backend to define the functionality.
    type Settings: Default + Clone + Send + Sync;

    type LoadedTypes: Loaded;

    /// Constructor of the backend with required settings.
    ///
    /// Also returns the interfacer for user input to the gpu backend.
    fn new(
        settings: Self::Settings,
        #[cfg(feature = "client")] event_loop: &winit::event_loop::EventLoop<()>,
    ) -> Result<(Self, Self::Interface), Self::Error>;

    /// Gives a window reference to the backend to draw to.
    #[cfg(feature = "client")]
    fn init_window(
        &mut self,
        event_loop: &winit::event_loop::ActiveEventLoop,
        window: &std::sync::Arc<winit::window::Window>,
    );

    /// This is used for draws. A function `pre_present_notify` gets included,
    /// which should be called right before presenting for optimization.
    #[cfg(feature = "client")]
    fn draw(
        &mut self,
        scene: &Scene<Self::LoadedTypes>,
        pre_present_notify: impl FnOnce(),
    ) -> Result<(), Self::Error>;

    #[cfg(feature = "egui")]
    fn update_egui(&mut self, event: &winit::event::WindowEvent) -> bool;

    #[cfg(feature = "egui")]
    fn draw_egui(&mut self) -> egui::Context;

    /// Gets called when the window has changed size.
    fn resize_event(&mut self, new_size: UVec2);
}

pub trait GpuInterfacer<T: Loaded>: Clone + Send + Sync {
    type Interface<'a>: GpuInterface<T>
    where
        Self: 'a;

    /// Returns the interface of the backend.
    fn interface<'a>(&'a self) -> Self::Interface<'a>;
}

pub trait GpuInterface<T: Loaded> {
    fn load_material<V: Vertex>(&self, material: &Material<V>) -> Result<T::MaterialId<V>>;
    fn load_buffer<B: Data>(&self, buffer: &Buffer<B>) -> Result<T::BufferId<B>>;
    fn load_model<V: Vertex>(&self, model: &Model<V>) -> Result<T::ModelId<V>>;
    fn load_texture(&self, texture: &Texture) -> Result<T::TextureId>;
    fn load_buffer_gpu_only<B: Data>(
        &self,
        size: usize,
        usage: BufferUsage,
    ) -> Result<T::BufferId<B>>;
    fn load_model_gpu_only<V: Vertex>(
        &self,
        vertex_size: usize,
        index_size: usize,
    ) -> Result<T::ModelId<V>>;
    fn load_texture_gpu_only(
        &self,
        dimensions: ViewTypeDim,
        settings: TextureSettings,
    ) -> Result<T::TextureId>;

    fn add_virtual_material<V: Vertex>(&self, id: T::MaterialId<V>) -> Result<T::MaterialId<V>>;
    fn add_virtual_buffer<B: Data>(&self, id: T::BufferId<B>) -> Result<T::BufferId<B>>;
    fn add_virtual_model<V: Vertex>(&self, id: T::ModelId<V>) -> Result<T::ModelId<V>>;
    fn add_virtual_texture(&self, id: T::TextureId) -> Result<T::TextureId>;

    fn map_virtual_material<V: Vertex>(
        &self,
        from: T::MaterialId<V>,
        to: T::MaterialId<V>,
    ) -> Result<()>;
    fn map_virtual_buffer<B: Data>(&self, from: T::BufferId<B>, to: T::BufferId<B>) -> Result<()>;
    fn map_virtual_model<V: Vertex>(&self, from: T::ModelId<V>, to: T::ModelId<V>) -> Result<()>;
    fn map_virtual_texture(&self, from: T::TextureId, to: T::TextureId) -> Result<()>;

    fn material<V: Vertex>(&self, id: T::MaterialId<V>) -> Option<&T::Material<V>>;
    fn buffer<B: Data>(&self, id: T::BufferId<B>) -> Option<&T::Buffer<B>>;
    fn model<V: Vertex>(&self, id: T::ModelId<V>) -> Option<&T::Model<V>>;
    fn texture(&self, id: T::TextureId) -> Option<&T::Texture>;

    fn remove_material<V: Vertex>(&self, id: T::MaterialId<V>) -> Result<()>;
    fn remove_buffer<B: Data>(&self, id: T::BufferId<B>) -> Result<()>;
    fn remove_model<V: Vertex>(&self, id: T::ModelId<V>) -> Result<()>;
    fn remove_texture(&self, id: T::TextureId) -> Result<()>;

    /// Validates if the backend allows this combination of material, model and buffers.
    fn validate_appearance<V: Vertex>(
        &self,
        material: T::MaterialId<V>,
        model: T::ModelId<V>,
        descriptors: &BTreeMap<Location, Descriptor<T>>,
    ) -> Result<(), T::AppearanceCreationError>;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ResourceType {
    Material,
    Buffer,
    Model,
    Texture,
}

pub trait ResourceId {
    fn resource_type(&self) -> ResourceType;
}

impl GpuInterface<()> for () {
    fn load_material<V: Vertex>(&self, _material: &Material<V>) -> Result<()> {
        Ok(())
    }

    fn load_buffer<B: Data>(&self, _buffer: &Buffer<B>) -> Result<()> {
        Ok(())
    }

    fn load_model<V: Vertex>(&self, _model: &Model<V>) -> Result<()> {
        Ok(())
    }

    fn load_texture(&self, _texture: &Texture) -> Result<()> {
        Ok(())
    }

    fn load_buffer_gpu_only<B: Data>(&self, _size: usize, _usage: BufferUsage) -> Result<()> {
        Ok(())
    }

    fn load_model_gpu_only<V: Vertex>(
        &self,
        _vertex_size: usize,
        _index_size: usize,
    ) -> Result<()> {
        Ok(())
    }

    fn load_texture_gpu_only(
        &self,
        _dimensions: ViewTypeDim,
        _settings: TextureSettings,
    ) -> Result<()> {
        Ok(())
    }

    fn add_virtual_material<V: Vertex>(&self, _id: ()) -> Result<()> {
        Ok(())
    }
    fn add_virtual_buffer<B: Data>(&self, _id: ()) -> Result<()> {
        Ok(())
    }
    fn add_virtual_model<V: Vertex>(&self, _id: ()) -> Result<()> {
        Ok(())
    }
    fn add_virtual_texture(&self, _id: ()) -> Result<()> {
        Ok(())
    }

    fn map_virtual_material<V: Vertex>(&self, _from: (), _to: ()) -> Result<()> {
        Ok(())
    }
    fn map_virtual_buffer<B: Data>(&self, _from: (), _to: ()) -> Result<()> {
        Ok(())
    }
    fn map_virtual_model<V: Vertex>(&self, _from: (), _to: ()) -> Result<()> {
        Ok(())
    }
    fn map_virtual_texture(&self, _from: (), _to: ()) -> Result<()> {
        Ok(())
    }

    fn material<V: Vertex>(&self, _id: ()) -> Option<&()> {
        None
    }

    fn buffer<B: Data>(&self, _id: ()) -> Option<&()> {
        None
    }

    fn model<V: Vertex>(&self, _id: ()) -> Option<&()> {
        None
    }

    fn texture(&self, _id: ()) -> Option<&()> {
        None
    }

    fn remove_material<V: Vertex>(&self, _id: ()) -> Result<()> {
        Ok(())
    }

    fn remove_buffer<B: Data>(&self, _id: ()) -> Result<()> {
        Ok(())
    }

    fn remove_model<V: Vertex>(&self, _id: ()) -> Result<()> {
        Ok(())
    }

    fn remove_texture(&self, _id: ()) -> Result<()> {
        Ok(())
    }

    fn validate_appearance<V: Vertex>(
        &self,
        _material: (),
        _model: (),
        _descriptors: &BTreeMap<Location, Descriptor<()>>,
    ) -> Result<(), std::io::Error> {
        Ok(())
    }
}

/// Loaded version of types used by the gpu backend.
pub trait Loaded: Clone + Default {
    /// The type of a material when it is loaded.
    type Material<V: Vertex>: Send + Sync;
    type MaterialId<V: Vertex>: Copy + Send + Sync;
    /// # Safety
    /// The given unit vertex type is just a placeholder and is not usable.
    unsafe fn material_id_unit<V: Vertex>(material: Self::MaterialId<V>) -> Self::MaterialId<()>;

    /// The type of a buffer when it is loaded.
    type Buffer<B: Data>: LoadedBuffer<B>;
    type BufferId<B: Data>: Copy + Send + Sync;
    fn buffer_id_u8<B: Data>(buffer: Self::BufferId<B>) -> Self::BufferId<u8>;

    /// The type of a model when it is loaded.
    type Model<V: Vertex>: LoadedModel<V>;
    type ModelId<V: Vertex>: Copy + Send + Sync;
    /// # Safety
    /// Different vertex types are not compatible with each other. Do not use modified ID.
    unsafe fn model_id_unit<V: Vertex>(model: Self::ModelId<V>) -> Self::ModelId<()>;

    /// The type of a texture when it is loaded.
    type Texture: LoadedTexture;
    type TextureId: Copy + Send + Sync;

    /// Error returned when combination of model material and buffers do not work together.
    type AppearanceCreationError: std::error::Error;
}

impl Loaded for () {
    type Material<V: Vertex> = ();
    type MaterialId<V: Vertex> = ();
    unsafe fn material_id_unit<V: Vertex>(_material: Self::MaterialId<V>) -> Self::MaterialId<()> {}
    type Buffer<B: Data> = ();
    type BufferId<B: Data> = ();
    fn buffer_id_u8<B: Data>(_buffer: Self::BufferId<B>) -> Self::BufferId<u8> {}
    type Model<V: Vertex> = ();
    type ModelId<V: Vertex> = ();
    unsafe fn model_id_unit<V: Vertex>(_model: Self::ModelId<V>) -> Self::ModelId<()> {}
    type Texture = ();
    type TextureId = ();

    type AppearanceCreationError = std::io::Error;
}

impl GpuBackend for () {
    type Error = std::io::Error;
    type Interface = ();
    type Settings = ();

    type LoadedTypes = ();

    fn new(
        _settings: Self::Settings,
        #[cfg(feature = "client")] _event_loop: &winit::event_loop::EventLoop<()>,
    ) -> Result<(Self, Self::Interface), Self::Error> {
        Ok(((), ()))
    }

    #[cfg(feature = "client")]
    fn init_window(
        &mut self,
        _event_loop: &winit::event_loop::ActiveEventLoop,
        _window: &std::sync::Arc<winit::window::Window>,
    ) {
    }

    #[cfg(feature = "client")]
    fn draw(&mut self, _scene: &Scene<Self>, _: impl FnOnce()) -> Result<(), Self::Error> {
        Ok(())
    }

    #[cfg(feature = "egui")]
    fn update_egui(&mut self, _event: &winit::event::WindowEvent) -> bool {
        false
    }

    #[cfg(feature = "egui")]
    fn draw_egui(&mut self) -> egui::Context {
        egui::Context::default()
    }

    fn resize_event(&mut self, _new_size: UVec2) {}
}

impl GpuInterfacer<()> for () {
    type Interface<'a> = ();
    fn interface<'a>(&'a self) -> Self::Interface<'a> {}
}
