use std::{any::Any, collections::BTreeMap, sync::Arc};

use anyhow::Result;
use glam::UVec2;
use raw_window_handle::{HasDisplayHandle, HasWindowHandle};

use crate::{
    objects::{scenes::Scene, Descriptor},
    resources::{
        buffer::{Buffer, LoadedBuffer, Location},
        data::Data,
        material::Material,
        model::{LoadedModel, Model, Vertex},
        texture::{LoadedTexture, Texture},
    },
};

/// Definition for a graphics backend for the let-engine.
pub trait GraphicsBackend {
    type CreateError: std::error::Error + Send + Sync;

    /// Will be stored in the [`EngineContext`](crate::engine::EngineContext)
    /// to interface the backend from multiple threads.
    type Interface: GraphicsInterface<Self::LoadedTypes>;

    /// Settings used by the backend to define the functionality.
    type Settings: Default + Clone;

    type LoadedTypes: Loaded;

    /// Constructor of the backend with required settings.
    fn new(
        settings: Self::Settings,
        handle: impl HasDisplayHandle,
    ) -> Result<Self, Self::CreateError>
    where
        Self: Sized;

    /// Gives a window reference to the backend to draw to.
    fn init_window(
        &mut self,
        window: &Arc<impl HasWindowHandle + HasDisplayHandle + Any + Send + Sync>,
        scene: &Arc<Scene<Self::LoadedTypes>>,
    );

    /// Returns the interface of the backend.
    fn interface(&self) -> &Self::Interface;

    /// Updates the backend.
    ///
    /// This is used for redraws. A function `pre_present_notify` gets included,
    /// which should be called right before presenting for optimisation.
    fn update(&mut self, pre_present_notify: impl FnOnce());

    /// Gets called when the window has changed size.
    fn resize_event(&mut self, new_size: UVec2);
}

pub trait GraphicsInterface<T: Loaded>: Clone + Send + Sync {
    fn load_material<V: Vertex>(&self, material: &Material) -> Result<T::Material>;
    fn load_buffer<B: Data>(&self, buffer: &Buffer<B>) -> Result<T::Buffer<B>>;
    fn load_model<V: Vertex>(&self, model: &Model<V>) -> Result<T::Model<V>>;
    fn load_texture(&self, texture: &Texture) -> Result<T::Texture>;
}

impl GraphicsInterface<()> for () {
    fn load_material<V: Vertex>(&self, _material: &Material) -> Result<()> {
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
}

/// Loaded version of types used by the graphics backend.
pub trait Loaded: Clone + Default {
    /// The type of a material when it is loaded.
    type Material: Clone + Send + Sync;

    /// The type of a buffer when it is loaded.
    type Buffer<B: Data>: LoadedBuffer<B>;
    type DrawableBuffer: Clone + Send + Sync;

    /// The type of a model when it is loaded.
    type Model<V: Vertex>: LoadedModel<V>;
    type DrawableModel: Clone + Send + Sync;

    /// The type of a texture when it is loaded.
    type Texture: LoadedTexture;

    /// Error returned when combination of model material and buffers do not work together.
    type AppearanceCreationError: std::fmt::Debug;

    /// Validates if the backend allows this combination of material, model and buffers.
    ///
    /// Returns a string with an error message of what went wrong in case it did.
    fn initialize_appearance(
        material: &Self::Material,
        model: &Self::DrawableModel,
        descriptors: &BTreeMap<Location, Descriptor<Self>>,
    ) -> Result<(), Self::AppearanceCreationError>;

    fn draw_buffer<B: Data>(buffer: Self::Buffer<B>) -> Self::DrawableBuffer;
    fn draw_model<V: Vertex>(model: Self::Model<V>) -> Self::DrawableModel;
}

impl Loaded for () {
    type Texture = ();
    type Material = ();
    type Buffer<B: Data> = ();
    type DrawableBuffer = ();
    type Model<V: Vertex> = ();
    type DrawableModel = ();
    type AppearanceCreationError = ();

    fn initialize_appearance(
        _material: &Self::Material,
        _model: &Self::DrawableModel,
        _descriptors: &BTreeMap<Location, Descriptor<Self>>,
    ) -> Result<(), Self::AppearanceCreationError> {
        Ok(())
    }

    fn draw_buffer<B: Data>(_buffer: Self::Buffer<B>) -> Self::DrawableBuffer {}

    fn draw_model<V: Vertex>(_model: Self::Model<V>) -> Self::DrawableModel {}
}

impl GraphicsBackend for () {
    type CreateError = std::io::Error;
    type Interface = ();
    type Settings = ();

    type LoadedTypes = ();

    fn new(
        _settings: Self::Settings,
        _handle: impl HasDisplayHandle,
    ) -> Result<Self, Self::CreateError> {
        Ok(())
    }

    fn init_window(
        &mut self,
        _window: &Arc<impl HasWindowHandle + HasDisplayHandle + Any + Send + Sync>,
        _scene: &Arc<Scene<Self>>,
    ) {
    }

    fn interface(&self) -> &Self::Interface {
        &()
    }

    fn update(&mut self, _: impl FnOnce()) {}

    fn resize_event(&mut self, _new_size: UVec2) {}
}
