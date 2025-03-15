use std::{any::Any, collections::BTreeMap, sync::Arc};

use anyhow::Result;
use bytemuck::AnyBitPattern;
use glam::UVec2;
use raw_window_handle::{HasDisplayHandle, HasWindowHandle};

use crate::{
    objects::{scenes::Scene, Descriptor},
    resources::{
        buffer::{Buffer, LoadedBuffer, Location},
        material::Material,
        model::{LoadedModel, Model, Vertex},
        texture::{LoadedTexture, Texture},
    },
};

/// Definition for a graphics backend for the let-engine.
pub trait GraphicsBackend {
    /// Will be stored in the [`EngineContext`](crate::engine::EngineContext)
    /// to interface the backend from multiple threads.
    type Interface: GraphicsInterface<Self::LoadedTypes>;

    /// Settings used by the backend to define the functionality.
    type Settings: Default + Clone;

    type LoadedTypes: Loaded;

    /// Constructor of the backend with required settings.
    fn new(settings: Self::Settings, handle: impl HasDisplayHandle) -> Self;

    /// Gives a window reference to the backend to draw to.
    fn init_window(
        &mut self,
        window: Arc<impl HasWindowHandle + HasDisplayHandle + Any + Send + Sync>,
    );

    /// Returns the interface of the backend.
    fn interface(&self) -> &Self::Interface;

    /// Updates the backend.
    fn update(&mut self, scene: &Scene<Self::LoadedTypes>);

    /// Gets called when the window has changed size.
    fn resize_event(&mut self, new_size: UVec2);
}

pub trait GraphicsInterface<T: Loaded>: Clone + Send + Sync {
    /// Validates if the backend allows this combination of material, model and buffers.
    ///
    /// Returns a string with an error message of what went wrong in case it did.
    fn initialize_appearance(
        &self,
        material: &T::Material,
        model: &T::DrawableModel,
        descriptors: &BTreeMap<Location, Descriptor<T>>,
    ) -> Result<(), T::AppearanceCreationError>;

    fn load_material<V: Vertex>(&self, material: Material) -> Result<T::Material>;
    fn load_buffer<B: AnyBitPattern + Send + Sync>(
        &self,
        buffer: Buffer<B>,
    ) -> Result<T::Buffer<B>>;
    fn load_model<V: Vertex>(&self, model: Model<V>) -> Result<T::Model<V>>;
    fn load_texture(&self, texture: Texture) -> Result<T::Texture>;
}

impl GraphicsInterface<()> for () {
    fn initialize_appearance(
        &self,
        _material: &(),
        _model: &(),
        _descriptors: &BTreeMap<Location, Descriptor<()>>,
    ) -> Result<(), ()> {
        Ok(())
    }

    fn load_material<V: Vertex>(&self, _material: Material) -> Result<()> {
        Ok(())
    }

    fn load_buffer<B: AnyBitPattern + Send + Sync>(&self, _buffer: Buffer<B>) -> Result<()> {
        Ok(())
    }

    fn load_model<V: Vertex>(&self, _model: Model<V>) -> Result<()> {
        Ok(())
    }

    fn load_texture(&self, _texture: Texture) -> Result<()> {
        Ok(())
    }
}

/// Loaded version of types used by the graphics backend.
pub trait Loaded: Clone + Default {
    /// The type of a material when it is loaded.
    type Material: Clone + Send + Sync;

    /// The type of a buffer when it is loaded.
    type Buffer<B: AnyBitPattern + Send + Sync>: LoadedBuffer<B>;
    type DrawableBuffer: Clone + Send + Sync;

    /// The type of a model when it is loaded.
    type Model<V: Vertex>: LoadedModel<V>;
    type DrawableModel: Clone + Send + Sync;

    /// The type of a texture when it is loaded.
    type Texture: LoadedTexture;

    /// Error returned when combination of model material and buffers do not work together.
    type AppearanceCreationError: std::fmt::Debug;

    fn draw_buffer<B: AnyBitPattern + Send + Sync>(buffer: Self::Buffer<B>)
        -> Self::DrawableBuffer;
    fn draw_model<V: Vertex>(model: Self::Model<V>) -> Self::DrawableModel;
}

impl Loaded for () {
    type Texture = ();
    type Material = ();
    type Buffer<B: AnyBitPattern + Send + Sync> = ();
    type DrawableBuffer = ();
    type Model<V: Vertex> = ();
    type DrawableModel = ();
    type AppearanceCreationError = ();

    fn draw_buffer<B: AnyBitPattern + Send + Sync>(
        _buffer: Self::Buffer<B>,
    ) -> Self::DrawableBuffer {
    }

    fn draw_model<V: Vertex>(_model: Self::Model<V>) -> Self::DrawableModel {}
}

impl GraphicsBackend for () {
    type Interface = ();
    type Settings = ();

    type LoadedTypes = ();

    fn new(_settings: Self::Settings, _handle: impl HasDisplayHandle) -> Self {}

    fn init_window(
        &mut self,
        _window: Arc<impl HasWindowHandle + HasDisplayHandle + Any + Send + Sync>,
    ) {
    }

    fn interface(&self) -> &Self::Interface {
        &()
    }

    fn update(&mut self, _scene: &Scene<Self>) {}

    fn resize_event(&mut self, _new_size: UVec2) {}
}
