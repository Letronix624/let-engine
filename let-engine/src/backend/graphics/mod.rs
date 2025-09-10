//! Default graphics backend made with `Vulkano`

use std::{
    cell::OnceCell,
    collections::BTreeMap,
    sync::{Arc, OnceLock},
};

use anyhow::{Context, Result, anyhow};
use buffer::{BufferId, GpuBuffer};
use concurrent_slotmap::{Key, hyaline::Guard};
use crossbeam::channel::{Receiver, Sender, bounded};
use draw::Draw;
use glam::UVec2;
use let_engine_core::{
    backend::graphics::{GraphicsBackend, Loaded},
    objects::{Color, Descriptor, scenes::Scene},
    resources::{
        Format,
        buffer::Location,
        data::Data,
        model::{Vertex, VertexBufferDescription},
        texture::LoadedTexture,
    },
};
use material::{GpuMaterial, MaterialId, ShaderError, eq_vertex_input_state};
use model::{GpuModel, ModelId};
use parking_lot::RwLock;
use texture::{GpuTexture, TextureId, image_view_type_to_vulkano};
use thiserror::Error;
use vulkan::{VK, Vulkan};
use vulkano::{
    LoadingError, VulkanError as VulkanoError,
    buffer::AllocateBufferError,
    descriptor_set::layout::DescriptorType,
    format::NumericType,
    image::{AllocateImageError, view::ImageViewType},
    memory::allocator::MemoryAllocatorError,
    pipeline::graphics::vertex_input::VertexDefinition,
    shader::spirv::SpirvBytesNotMultipleOf4,
};

use winit::event_loop::{ActiveEventLoop, EventLoop};

pub use vulkano::DeviceSize;

pub mod buffer;
pub mod material;
pub mod model;
pub mod texture;

mod draw;
mod vulkan;

pub struct DefaultGraphicsBackend {
    draw: OnceCell<Draw>,
    settings_receiver: Receiver<Graphics>,
    interfacer: GraphicsInterfacer,
}

#[derive(Debug, Error)]
pub enum DefaultGraphicsBackendError {
    /// Gets returned when the engine fails to find or load the vulkan library.
    #[error("Failed to load vulkan library: {0}")]
    Loading(LoadingError),

    /// Gets returned when the device running the backend does not meet the backends requirements.
    #[error(
        "
    This device does not support the requirements of this graphics backend:\n
    {0}\n
    Make sure you have a Vulkan 1.2 capable device and the newest graphics drivers.
    "
    )]
    Unsupported(&'static str),

    /// Gets returned when Vulkan fails to execute an operation.
    #[error("An error with Vulkan occured: {0}")]
    Vulkan(VulkanError),
}

impl From<VulkanError> for DefaultGraphicsBackendError {
    fn from(value: VulkanError) -> Self {
        Self::Vulkan(value)
    }
}

impl GraphicsBackend for DefaultGraphicsBackend {
    type Error = DefaultGraphicsBackendError;

    type Settings = Graphics;
    type Interface = GraphicsInterfacer;

    type LoadedTypes = VulkanTypes;

    fn new(
        settings: Self::Settings,
        event_loop: &EventLoop<()>,
    ) -> Result<(Self, Self::Interface), Self::Error> {
        // Initialize backend in case it is not already initialized.
        if VK.get().is_none() {
            let vulkan = Vulkan::init(event_loop, settings)?;
            let _ = VK.set(vulkan);
        }
        let settings = Arc::new(RwLock::new(settings));

        let settings_channels = bounded(3);

        let interfacer = GraphicsInterfacer {
            settings,
            settings_sender: settings_channels.0,
            available_present_modes: OnceLock::new(),
        };

        Ok((
            Self {
                draw: OnceCell::new(),
                settings_receiver: settings_channels.1,
                interfacer: interfacer.clone(),
            },
            interfacer,
        ))
    }

    fn init_window(&mut self, event_loop: &ActiveEventLoop, window: &Arc<winit::window::Window>) {
        // TODO: Remove unwraps
        let settings = *self.interfacer.settings.read();
        let draw = Draw::new(
            settings,
            self.settings_receiver.clone(),
            &self.interfacer.available_present_modes,
            event_loop,
            window,
        )
        .unwrap();

        let _ = self.draw.set(draw);
    }

    fn draw(
        &mut self,
        scene: &Scene<Self::LoadedTypes>,
        pre_present_notify: impl FnOnce(),
    ) -> Result<(), Self::Error> {
        if let Some(draw) = self.draw.get_mut() {
            draw.redraw_event(scene, pre_present_notify)
                .map_err(|e| e.into())
        } else {
            Ok(())
        }
    }

    #[cfg(feature = "egui")]
    fn update_egui(&mut self, event: &winit::event::WindowEvent) -> bool {
        if let Some(draw) = self.draw.get_mut() {
            draw.egui_update(event)
        } else {
            false
        }
    }

    #[cfg(feature = "egui")]
    fn draw_egui(&mut self) -> egui::Context {
        self.draw.get_mut().unwrap().draw_egui()
    }

    fn resize_event(&mut self, new_size: UVec2) {
        if let Some(draw) = self.draw.get_mut() {
            draw.resize(new_size);
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct VulkanTypes;

impl Loaded for VulkanTypes {
    type Material = GpuMaterial;
    type MaterialId = MaterialId;

    type Buffer<B: Data> = GpuBuffer<B>;
    type BufferId<B: Data> = BufferId<B>;

    #[inline]
    fn buffer_id_u8<B: Data>(buffer: Self::BufferId<B>) -> Self::BufferId<u8> {
        unsafe { std::mem::transmute(buffer) }
    }

    type Model<V: Vertex> = GpuModel<V>;
    type ModelId<V: Vertex> = ModelId<V>;

    #[inline]
    unsafe fn model_id_u8<V: Vertex>(model: Self::ModelId<V>) -> Self::ModelId<u8> {
        unsafe { std::mem::transmute(model) }
    }

    type Texture = GpuTexture;
    type TextureId = TextureId;

    type AppearanceCreationError = AppearanceCreationError;
}

/// Errors that can occur when attempting to create an `Apperance` instance due to mismatches
/// between shader requirements and provided descriptor or model layouts.
#[derive(Debug, Clone, Error)]
pub enum AppearanceCreationError {
    /// Occurs when descriptors are missing at specific locations.
    ///
    /// Contains the list of locations where descriptors are missing.
    #[error("Shaders require a buffer at the locations: {0:?}.")]
    MissingDescriptors(Vec<Location>),

    /// Occurs when more descriptors are provided than the material can accept.
    ///
    /// Contains a list of locations where excess descriptors are provided.
    #[error("Too many descriptors at the locations: {0:?}.")]
    ExcessDescriptors(Vec<Location>),

    /// Occurs when the model's vertex type does not match the material's expected vertex type.
    #[error("Mismatched types of model vertices and expected vertices in the material.")]
    WrongVertexType,

    /// Occurs when the type of a descriptor at a specific location does not match the the shader's.
    ///
    /// - `location`: The descriptor's location.
    /// - `allowed_types`: The types allowed by the shader.
    /// - `provided_type`: The type provided by the user.
    #[error("Shader requires {location:?} to be {allowed_types:?}, but got {provided_type:?}.")]
    WrongDescriptorType {
        location: Location,
        allowed_types: Vec<DescriptorType>,
        provided_type: DescriptorType,
    },

    /// Occurs when the format of a texture at a specific location does not match the shader's.
    ///
    /// - `location`: The texture's location.
    /// - `expected_format`: The expected format.
    /// - `provided_format`: The provided format.
    #[error(
        "Shader requires format {expected_format:?} at texture location {location:?}, but got {provided_format:?}."
    )]
    WrongTextureFormat {
        location: Location,
        expected_format: vulkano::format::Format,
        provided_format: vulkano::format::Format,
    },

    /// Occurs when the shader requires a multisampled texture, but the current backend does not support multisampling.
    #[error(
        "Shader requires a multisampled texture, which is currently not supported in this backend."
    )]
    NoMultisampleSupport,

    /// Occurs when the numeric type of a texture format at a specific location does not match the numeric type expected by the shader.
    ///
    /// - `location`: The texture's location.
    /// - `expected_type`: The expected numeric type.
    /// - `provided_type`: The provided numeric type.
    #[error(
        "Shader requires numeric type {expected_type:?} at {location:?}, but instead got {provided_type:?}."
    )]
    WrongNumericType {
        location: Location,
        expected_type: NumericType,
        provided_type: NumericType,
    },

    /// Occurs when the view type of a texture at a specific location does not match the shader's.
    ///
    /// - `location`: The texture's location.
    /// - `expected_type`: The expected view type.
    /// - `provided_type`: The provided view type.
    #[error(
        "Shader requires view type {expected_type:?} at {location:?}, but got {provided_type:?}."
    )]
    WrongViewType {
        location: Location,
        expected_type: ImageViewType,
        provided_type: ImageViewType,
    },

    /// Occurs when the material ID of the appearance is not valid.
    #[error("The provided material ID is not valid.")]
    InvalidMaterialId,

    /// Occurs when the model ID of the appearance is not valid.
    #[error("The provided model ID is not valid.")]
    InvalidModelId,

    /// Occurs when the buffer ID of a descriptor in the appearance is not valid.
    #[error("The provided buffer ID is not valid.")]
    InvalidBufferId,

    /// Occurs when the texture ID of a descriptor in the appearance is not valid.
    #[error("The provided texture ID is not valid.")]
    InvalidTextureId,
}

#[derive(Debug, Clone)]
pub struct GraphicsInterfacer {
    settings: Arc<RwLock<Graphics>>,
    settings_sender: Sender<Graphics>,

    // Gets written to in swapchain.rs
    available_present_modes: OnceLock<Box<[PresentMode]>>,
}

impl let_engine_core::backend::graphics::GraphicsInterfacer<VulkanTypes> for GraphicsInterfacer {
    type Interface<'a> = GraphicsInterface<'a>;

    fn interface<'a>(&'a self) -> Self::Interface<'a> {
        let vulkan = VK.get().unwrap();
        GraphicsInterface {
            settings: &self.settings,
            settings_sender: &self.settings_sender,
            present_modes: self.available_present_modes.get().map(|v| &**v),
            guard: unsafe { vulkan.collector.pin() },
        }
    }
}

#[derive(Debug)]
pub struct GraphicsInterface<'a> {
    settings: &'a RwLock<Graphics>,
    settings_sender: &'a Sender<Graphics>,
    present_modes: Option<&'a [PresentMode]>,

    guard: Guard<'a>,
}

impl<'a> let_engine_core::backend::graphics::GraphicsInterface<VulkanTypes>
    for GraphicsInterface<'a>
{
    fn load_material<V: let_engine_core::resources::model::Vertex>(
        &self,
        material: &let_engine_core::resources::material::Material,
    ) -> Result<MaterialId> {
        let vulkan = VK.get().context("Vulkan uninitialized")?;
        let material = GpuMaterial::new::<V>(material, vulkan).expect("failed to load material");

        Ok(vulkan.materials.insert(material, &self.guard))
    }

    fn load_buffer<B: Data>(
        &self,
        buffer: &let_engine_core::resources::buffer::Buffer<B>,
    ) -> Result<BufferId<B>> {
        let vulkan = VK.get().context("Vulkan uninitialized")?;

        let buffer = GpuBuffer::new(buffer, vulkan).context("failed to load buffer")?;
        let buffer = unsafe { std::mem::transmute::<GpuBuffer<B>, GpuBuffer<u8>>(buffer) };

        Ok(BufferId::from_id(
            vulkan.buffers.insert(buffer, &self.guard),
        ))
    }

    fn load_model<V: let_engine_core::resources::model::Vertex>(
        &self,
        model: &let_engine_core::resources::model::Model<V>,
    ) -> Result<ModelId<V>> {
        let vulkan = VK.get().context("Vulkan uninitialized")?;

        let model = GpuModel::new(model).context("failed to load model")?;
        let model = unsafe { std::mem::transmute::<GpuModel<V>, GpuModel<u8>>(model) };

        Ok(ModelId::from_id(vulkan.models.insert(model, &self.guard)))
    }

    fn load_texture(
        &self,
        texture: &let_engine_core::resources::texture::Texture,
    ) -> Result<TextureId> {
        let vulkan = VK.get().context("Vulkan uninitialized")?;

        let texture = GpuTexture::new(texture, vulkan).context("failed to load texture")?;

        Ok(vulkan.textures.insert(texture, &self.guard))
    }

    fn load_buffer_gpu_only<B: Data>(
        &self,
        size: usize,
        usage: let_engine_core::resources::buffer::BufferUsage,
    ) -> Result<<VulkanTypes as Loaded>::BufferId<B>> {
        let vulkan = VK.get().context("Vulkan uninitialized")?;

        let buffer = GpuBuffer::new_gpu_only(size as DeviceSize, usage, vulkan)
            .context("failed to load buffer")?;
        let buffer = unsafe { std::mem::transmute::<GpuBuffer<B>, GpuBuffer<u8>>(buffer) };

        Ok(BufferId::from_id(
            vulkan.buffers.insert(buffer, &self.guard),
        ))
    }

    fn load_model_gpu_only<V: Vertex>(
        &self,
        vertex_size: usize,
        index_size: usize,
    ) -> Result<<VulkanTypes as Loaded>::ModelId<V>> {
        let vulkan = VK.get().context("Vulkan uninitialized")?;

        let model = GpuModel::new_gpu_only(vertex_size as DeviceSize, index_size as DeviceSize)
            .context("failed to load model")?;
        let model = unsafe { std::mem::transmute::<GpuModel<V>, GpuModel<u8>>(model) };

        Ok(ModelId::from_id(vulkan.models.insert(model, &self.guard)))
    }

    fn load_texture_gpu_only(
        &self,
        dimensions: let_engine_core::resources::texture::ViewTypeDim,
        settings: let_engine_core::resources::texture::TextureSettings,
    ) -> Result<<VulkanTypes as Loaded>::TextureId> {
        let vulkan = VK.get().context("Vulkan uninitialized")?;

        let texture = GpuTexture::new_gpu_only(dimensions, settings, vulkan)
            .context("failed to load texture")?;

        Ok(vulkan.textures.insert(texture, &self.guard))
    }

    fn material(
        &self,
        id: <VulkanTypes as Loaded>::MaterialId,
    ) -> Option<&<VulkanTypes as Loaded>::Material> {
        let vulkan = VK.get().unwrap();

        vulkan.materials.get(id, &self.guard)
    }

    fn buffer<B: Data>(&self, id: BufferId<B>) -> Option<&GpuBuffer<B>> {
        let vulkan = VK.get().unwrap();

        // SAFETY: transmute is safe here, because the generic is not present in the byte representation and drop logic.
        unsafe { std::mem::transmute(vulkan.buffers.get(id.as_id(), &self.guard)) }
    }

    fn model<V: Vertex>(&self, id: ModelId<V>) -> Option<&<VulkanTypes as Loaded>::Model<V>> {
        let vulkan = VK.get().unwrap();

        // SAFETY: transmute is safe here, because the generic is not present in the byte representation and drop logic.
        //         The vertex type might mismatch to the original format, but this is only possible if the user used unsafe
        //         logic to reinterpret the vertex type of an ID to a non-compatible type, which is totally on them.
        unsafe { std::mem::transmute(vulkan.models.get(id.as_id(), &self.guard)) }
    }

    fn texture(&self, id: TextureId) -> Option<&<VulkanTypes as Loaded>::Texture> {
        let vulkan = VK.get().unwrap();

        vulkan.textures.get(id, &self.guard)
    }

    fn remove_material(&self, id: <VulkanTypes as Loaded>::MaterialId) -> Result<()> {
        let vulkan = VK.get().unwrap();

        vulkan.materials.remove(id, &self.guard);

        Ok(())
    }

    fn remove_buffer<B: Data>(&self, id: BufferId<B>) -> Result<()> {
        let vulkan = VK.get().unwrap();

        if vulkan.buffers.remove(id.as_id(), &self.guard).is_some() {
            vulkan.flag_taskgraph_to_be_rebuilt();
        }

        Ok(())
    }

    fn remove_model<V: Vertex>(&self, id: ModelId<V>) -> Result<()> {
        let vulkan = VK.get().unwrap();

        if vulkan.models.remove(id.as_id(), &self.guard).is_some() {
            vulkan.flag_taskgraph_to_be_rebuilt();
        }

        Ok(())
    }

    fn remove_texture(&self, id: <VulkanTypes as Loaded>::TextureId) -> Result<()> {
        let vulkan = VK.get().unwrap();

        if vulkan.textures.remove(id, &self.guard).is_some() {
            vulkan.flag_taskgraph_to_be_rebuilt();
        };

        Ok(())
    }

    fn validate_appearance(
        &self,
        material_id: MaterialId,
        model_id: ModelId<u8>,
        descriptors: &BTreeMap<Location, Descriptor<VulkanTypes>>,
    ) -> Result<(), AppearanceCreationError> {
        let vulkan = VK.get().context("Vulkan uninitialized").unwrap();

        let guard = unsafe { vulkan.collector.pin() };

        let material = vulkan
            .materials
            .get(material_id, &guard)
            .ok_or(AppearanceCreationError::InvalidMaterialId)?;
        let model = vulkan
            .models
            .get(model_id.as_id(), &guard)
            .ok_or(AppearanceCreationError::InvalidModelId)?;

        let requirements = &material.graphics_shaders().requirements;

        if requirements.len() != descriptors.len() {
            let missing_descriptors: Vec<Location> = requirements
                .keys()
                .filter(|key| !descriptors.contains_key(key))
                .copied()
                .collect();
            if !missing_descriptors.is_empty() {
                return Err(AppearanceCreationError::MissingDescriptors(
                    missing_descriptors,
                ));
            }

            let excess_descriptors: Vec<Location> = descriptors
                .keys()
                .filter(|key| !requirements.contains_key(key))
                .copied()
                .collect();

            return Err(AppearanceCreationError::ExcessDescriptors(
                excess_descriptors,
            ));
        }

        // Vertex
        let entry_point = &material.graphics_shaders().vertex;
        let vertex_input_state =
            vertex_buffer_description_to_vulkano(model.vertex_buffer_description().clone())
                .definition(entry_point)
                .unwrap(); // TODO

        if !eq_vertex_input_state(&vertex_input_state, &material.vertex_input_state) {
            return Err(AppearanceCreationError::WrongVertexType);
        }

        // Descriptors

        for (location, requirement) in requirements {
            let buffer = descriptors.get(location).unwrap();

            match buffer {
                Descriptor::Texture(texture_id) => {
                    let texture = vulkan
                        .textures
                        .get(*texture_id, &guard)
                        .ok_or(AppearanceCreationError::InvalidTextureId)?;
                    let types = &requirement.descriptor_types;
                    let texture_type = texture.descriptor_type();
                    if !types.contains(&texture_type) {
                        return Err(AppearanceCreationError::WrongDescriptorType {
                            location: *location,
                            allowed_types: types.clone(),
                            provided_type: texture_type,
                        });
                    }

                    let texture_format = format_to_vulkano(&texture.settings().format);

                    if let Some(format) = requirement.image_format
                        && format != texture_format
                    {
                        return Err(AppearanceCreationError::WrongTextureFormat {
                            location: *location,
                            expected_format: format,
                            provided_format: texture_format,
                        });
                    }

                    if requirement.image_multisampled {
                        return Err(AppearanceCreationError::NoMultisampleSupport);
                    }

                    if let Some(numeric_type) = requirement.image_scalar_type {
                        let texture_numeric_type = texture_format
                            .numeric_format_color()
                            .or(texture_format.numeric_format_depth())
                            .or(texture_format.numeric_format_stencil())
                            .unwrap()
                            .numeric_type();

                        if numeric_type != texture_numeric_type {
                            return Err(AppearanceCreationError::WrongNumericType {
                                location: *location,
                                expected_type: numeric_type,
                                provided_type: texture_numeric_type,
                            });
                        }
                    }

                    if let Some(view_type) = requirement.image_view_type {
                        let texture_view_type = image_view_type_to_vulkano(texture.dimensions());

                        if view_type != texture_view_type {
                            return Err(AppearanceCreationError::WrongViewType {
                                location: *location,
                                expected_type: view_type,
                                provided_type: texture_view_type,
                            });
                        }
                    }
                }

                Descriptor::Buffer(buffer_id) => {
                    let buffer = vulkan
                        .buffers
                        .get(buffer_id.as_id(), &guard)
                        .ok_or(AppearanceCreationError::InvalidBufferId)?;
                    let types = &requirement.descriptor_types;
                    let buffer_type = buffer.descriptor_type();
                    if !types.contains(&buffer_type) {
                        return Err(AppearanceCreationError::WrongDescriptorType {
                            location: *location,
                            allowed_types: types.clone(),
                            provided_type: buffer_type,
                        });
                    }
                }

                Descriptor::Mvp => {
                    let types = &requirement.descriptor_types;
                    if !types.contains(&DescriptorType::UniformBuffer) {
                        return Err(AppearanceCreationError::WrongDescriptorType {
                            location: *location,
                            allowed_types: types.clone(),
                            provided_type: DescriptorType::UniformBuffer,
                        });
                    }
                }
            }
        }

        // TODO: Cache descriptor sets

        Ok(())
    }
}

impl<'a> GraphicsInterface<'a> {
    /// Returns the settings of the graphics backend.
    pub fn settings(&self) -> Graphics {
        *self.settings.read()
    }

    pub fn settings_mut<F: FnMut(&mut Graphics)>(&self, mut f: F) {
        let mut settings = self.settings.write();
        f(&mut settings)
    }

    fn send_settings(&self, settings: Graphics) {
        let _ = self.settings_sender.try_send(settings);
    }

    /// Sets the settings of this graphics backend
    pub fn set_settings(&self, settings: Graphics) {
        *self.settings.write() = settings;
        self.send_settings(settings);
    }

    /// Returns the current present mode of the game.
    pub fn present_mode(&self) -> PresentMode {
        self.settings.read().present_mode
    }

    /// Returns all the present modes this device supports.
    pub fn supported_present_modes(&self) -> Option<&'a [PresentMode]> {
        self.present_modes
    }

    /// Sets the present mode of the graphics backend. Returns an error in case the present mode is not supported.
    pub fn set_present_mode(&self, present_mode: PresentMode) -> Result<()> {
        if self
            .present_modes
            .ok_or(anyhow!(
                "Can not set present mode before window initialized."
            ))?
            .contains(&present_mode)
        {
            let mut settings = self.settings.write();
            settings.present_mode = present_mode;
            self.send_settings(*settings);
        } else {
            return Err(anyhow!("Present mode not supported."));
        };

        Ok(())
    }

    /// Returns the clear color of the window.
    pub fn clear_color(&self) -> Color {
        self.settings.read().clear_color
    }

    /// Sets the clear color of the window.
    pub fn set_clear_color(&self, clear_color: Color) {
        let mut settings = self.settings.write();
        settings.clear_color = clear_color;
        self.send_settings(*settings);
    }
}

/// Errors that originate from Vulkan and the backend is not responsible for.
#[derive(Error, Debug)]
pub enum VulkanError {
    /// Returns when an operation is not possible because there is not enough memory left.
    #[error("Not enough memory for this operation.")]
    OutOfHostMemory,

    /// Returns when there is not enough VRAM for a graphics operation.
    #[error("Not enough VRAM for this operation.")]
    OutOfDeviceMemory,

    /// The GPU device was lost, likely to a crash, driver reset or system instability.
    ///
    /// This might occur sometimes
    #[error("Lost access to the graphics device.")]
    DeviceLost,

    /// Your application has breached the boundaries of the amount of graphical objects
    /// it can render.
    #[error("Too many graphical objects to draw.")]
    TooManyObjects,

    /// Returns when the window and with it the surface unexpectedly gets closed.
    #[error("The window to present to has been lost.")]
    SurfaceLost,

    /// An unexpected error that might occur.
    #[error("An unexpected error occured: {0}")]
    Other(VulkanoError),
}

impl From<AllocateBufferError> for VulkanError {
    fn from(value: AllocateBufferError) -> Self {
        match value {
            AllocateBufferError::CreateBuffer(e) => e.into(),
            AllocateBufferError::BindMemory(e) => e.into(),
            AllocateBufferError::AllocateMemory(e) => {
                if let MemoryAllocatorError::AllocateDeviceMemory(e) = e {
                    e.unwrap().into()
                } else {
                    VulkanError::Other(vulkano::VulkanError::Unknown)
                }
            }
        }
    }
}

impl From<AllocateImageError> for VulkanError {
    fn from(value: AllocateImageError) -> Self {
        match value {
            AllocateImageError::CreateImage(e) => e.into(),
            AllocateImageError::BindMemory(e) => e.into(),
            AllocateImageError::AllocateMemory(e) => {
                if let MemoryAllocatorError::AllocateDeviceMemory(e) = e {
                    e.unwrap().into()
                } else {
                    VulkanError::Other(vulkano::VulkanError::Unknown)
                }
            }
        }
    }
}

impl From<VulkanoError> for VulkanError {
    fn from(value: VulkanoError) -> Self {
        match value {
            VulkanoError::OutOfHostMemory => Self::OutOfHostMemory,
            VulkanoError::OutOfDeviceMemory => Self::OutOfDeviceMemory,
            VulkanoError::DeviceLost => Self::DeviceLost,
            VulkanoError::TooManyObjects => Self::TooManyObjects,
            VulkanoError::SurfaceLost => Self::SurfaceLost,
            e => Self::Other(e),
        }
    }
}

impl From<SpirvBytesNotMultipleOf4> for ShaderError {
    fn from(_value: SpirvBytesNotMultipleOf4) -> Self {
        Self::InvalidSpirV
    }
}

/// Backend wide Graphics settings.
#[derive(Debug, Clone, Copy)]
pub struct Graphics {
    /// An option that determines something called "VSync".
    ///
    /// # Default
    ///
    /// - [`PresentMode::Fifo`]
    pub present_mode: PresentMode,

    /// The clear color of the window.
    ///
    /// Replaces the background with this color each frame.
    ///
    /// # Default
    ///
    /// - [`Color::BLACK`]
    pub clear_color: Color, // TODO: Clear(Color), Load, DontCare

    /// The amount of retries of creating a window surface to attempt before failing
    /// to create the backend.
    ///
    /// # Default
    ///
    /// - `20`
    pub window_handle_retries: usize,

    /// The maximum amount of frames, which can be drawn in parallel.
    ///
    /// # Default
    ///
    /// - `2`
    pub max_frames_in_flight: usize, // /// Time waited before each frame.
}

impl Default for Graphics {
    fn default() -> Self {
        Self::new()
    }
}

impl Graphics {
    /// Creates a new graphics settings instance.
    pub fn new() -> Self {
        Self {
            present_mode: PresentMode::Fifo,
            clear_color: Color::BLACK,
            window_handle_retries: 20,
            max_frames_in_flight: 2,
        }
    }
}

/// The presentation action to take when presenting images to the window.
///
/// In game engine terms this affects "VSync".
///
/// `Immediate` mode is the only one that does not have "VSync".
///
/// When designing in game graphics settings this is the setting that gets changed when users select the VSync option.
///
/// The vsync options may include higher latency than the other ones.
///
/// It is not recommended dynamically switching between those during the game, as they may cause visual artifacts or noticable changes.
#[derive(Clone, Copy, PartialEq, Eq, Debug, PartialOrd, Ord, Hash)]
#[non_exhaustive]
pub enum PresentMode {
    /// This one has no vsync and presents the image as soon as it is available.
    ///
    /// This may happen while the image is presenting, so it may cause tearing.
    ///
    /// This present mode has the lowest latency compared to every other mode, so this is the option for most fast paced games where latency matters.
    ///
    /// This present mode may not be available on every device.
    Immediate,

    /// This present mode has a waiting slot for the next image to be presented after the current one has finished presenting.
    /// This mode also does not block the drawing thread, drawing images, even when they will not get presented.
    ///
    /// This means there is no tearing and with just one waiting slot also not that much latency.
    ///
    /// This option is recommended if `Immediate` is not available and also for games that focus visual experience over latency, as this one does not have tearing.
    ///
    /// It may also not be available on every device.
    Mailbox,

    /// Means first in first out.
    ///
    /// This present mode is also known as "vsync on". It blocks the thread and only draws and presents images if the present buffer is finished drawing to the screen.
    ///
    /// It is guaranteed to be available on every device.
    Fifo,
}

impl From<PresentMode> for vulkano::swapchain::PresentMode {
    fn from(value: PresentMode) -> vulkano::swapchain::PresentMode {
        use vulkano::swapchain::PresentMode as Pm;
        match value {
            PresentMode::Immediate => Pm::Immediate,
            PresentMode::Mailbox => Pm::Mailbox,
            PresentMode::Fifo => Pm::Fifo,
        }
    }
}

impl From<vulkano::swapchain::PresentMode> for PresentMode {
    fn from(value: vulkano::swapchain::PresentMode) -> PresentMode {
        use vulkano::swapchain::PresentMode as Pm;
        match value {
            Pm::Immediate => PresentMode::Immediate,
            Pm::Mailbox => PresentMode::Mailbox,
            _ => PresentMode::Fifo,
        }
    }
}

#[inline]
pub(crate) fn format_to_vulkano(format: &Format) -> vulkano::format::Format {
    match format {
        Format::Rg4Unorm => vulkano::format::Format::R4G4_UNORM_PACK8,
        Format::Rgba4Unorm => vulkano::format::Format::R4G4B4A4_UNORM_PACK16,
        Format::R5G6B5Unorm => vulkano::format::Format::R5G6B5_UNORM_PACK16,
        Format::Rgb5A1Unorm => vulkano::format::Format::R5G5B5A1_UNORM_PACK16,
        Format::Sr8 => vulkano::format::Format::R8_SRGB,
        Format::Srg8 => vulkano::format::Format::R8G8_SRGB,
        Format::Srgb8 => vulkano::format::Format::R8G8B8_SRGB,
        Format::Srgba8 => vulkano::format::Format::R8G8B8A8_SRGB,
        Format::R8Unorm => vulkano::format::Format::R8_UNORM,
        Format::Rg8Unorm => vulkano::format::Format::R8G8_UNORM,
        Format::Rgb8Unorm => vulkano::format::Format::R8G8B8_UNORM,
        Format::Rgba8Unorm => vulkano::format::Format::R8G8B8A8_UNORM,
        Format::R8Uint => vulkano::format::Format::R8_UINT,
        Format::R8Sint => vulkano::format::Format::R8_SINT,
        Format::Rgba8Uint => vulkano::format::Format::R8G8B8A8_UINT,
        Format::Rgba8Sint => vulkano::format::Format::R8G8B8A8_SINT,
        Format::A2Rgb10Unorm => vulkano::format::Format::A2R10G10B10_UNORM_PACK32,
        Format::R16Float => vulkano::format::Format::R16_SFLOAT,
        Format::Rg16Float => vulkano::format::Format::R16G16_SFLOAT,
        Format::Rgba16Float => vulkano::format::Format::R16G16B16A16_SFLOAT,
        Format::R16Unorm => vulkano::format::Format::R16_UNORM,
        Format::Rg16Unorm => vulkano::format::Format::R16G16_UNORM,
        Format::Rgb16Unorm => vulkano::format::Format::R16G16B16_UNORM,
        Format::Rgba16Unorm => vulkano::format::Format::R16G16B16A16_UNORM,
        Format::R32Float => vulkano::format::Format::R32_SFLOAT,
        Format::Rg32Float => vulkano::format::Format::R32G32_SFLOAT,
        Format::Rgb32Float => vulkano::format::Format::R32G32B32_SFLOAT,
        Format::Rgba32Float => vulkano::format::Format::R32G32B32A32_SFLOAT,
        // Format::D32Float => vulkano::format::Format::D32_SFLOAT,
        // Format::D24UnormS8Uint => vulkano::format::Format::D24_UNORM_S8_UINT,
        Format::Bc1RgbUnormBlock => vulkano::format::Format::BC1_RGB_UNORM_BLOCK,
        Format::Bc1RgbSrgbBlock => vulkano::format::Format::BC1_RGB_SRGB_BLOCK,
        Format::Bc1RgbaUnormBlock => vulkano::format::Format::BC1_RGBA_UNORM_BLOCK,
        Format::Bc1RgbaSrgbBlock => vulkano::format::Format::BC1_RGBA_SRGB_BLOCK,
        Format::Bc2UnormBlock => vulkano::format::Format::BC2_UNORM_BLOCK,
        Format::Bc2SrgbBlock => vulkano::format::Format::BC2_SRGB_BLOCK,
        Format::Bc3UnormBlock => vulkano::format::Format::BC3_UNORM_BLOCK,
        Format::Bc3SrgbBlock => vulkano::format::Format::BC3_SRGB_BLOCK,
        Format::Bc4UnormBlock => vulkano::format::Format::BC4_UNORM_BLOCK,
        Format::Bc5UnormBlock => vulkano::format::Format::BC5_UNORM_BLOCK,
        Format::Bc7UnormBlock => vulkano::format::Format::BC7_UNORM_BLOCK,
        Format::Bc7SrgbBlock => vulkano::format::Format::BC7_SRGB_BLOCK,
        Format::Etc2Rgb8UnormBlock => vulkano::format::Format::ETC2_R8G8B8_UNORM_BLOCK,
        Format::Etc2Rgb8SrgbBlock => vulkano::format::Format::ETC2_R8G8B8_SRGB_BLOCK,
        Format::Etc2Rgb8A1UnormBlock => vulkano::format::Format::ETC2_R8G8B8A1_UNORM_BLOCK,
        Format::Etc2Rgb8A1SrgbBlock => vulkano::format::Format::ETC2_R8G8B8A1_SRGB_BLOCK,
        Format::Etc2Rgb8A8UnormBlock => vulkano::format::Format::ETC2_R8G8B8A8_UNORM_BLOCK,
        Format::Etc2Rgb8A8SrgbBlock => vulkano::format::Format::ETC2_R8G8B8A8_SRGB_BLOCK,
        Format::EacR11UnormBlock => vulkano::format::Format::EAC_R11_UNORM_BLOCK,
        Format::EacRg11UnormBlock => vulkano::format::Format::EAC_R11G11_UNORM_BLOCK,
        Format::Astc4x4UnormBlock => vulkano::format::Format::ASTC_4x4_UNORM_BLOCK,
        Format::Astc4x4SrgbBlock => vulkano::format::Format::ASTC_4x4_SRGB_BLOCK,
        Format::Astc5x4UnormBlock => vulkano::format::Format::ASTC_5x4_UNORM_BLOCK,
        Format::Astc5x4SrgbBlock => vulkano::format::Format::ASTC_5x4_SRGB_BLOCK,
        Format::Astc5x5UnormBlock => vulkano::format::Format::ASTC_5x5_UNORM_BLOCK,
        Format::Astc5x5SrgbBlock => vulkano::format::Format::ASTC_5x5_SRGB_BLOCK,
        Format::Astc6x5UnormBlock => vulkano::format::Format::ASTC_6x5_UNORM_BLOCK,
        Format::Astc6x5SrgbBlock => vulkano::format::Format::ASTC_6x5_SRGB_BLOCK,
        Format::Astc6x6UnormBlock => vulkano::format::Format::ASTC_6x6_UNORM_BLOCK,
        Format::Astc6x6SrgbBlock => vulkano::format::Format::ASTC_6x6_SRGB_BLOCK,
        Format::Astc8x5UnormBlock => vulkano::format::Format::ASTC_8x5_UNORM_BLOCK,
        Format::Astc8x5SrgbBlock => vulkano::format::Format::ASTC_8x5_SRGB_BLOCK,
        Format::Astc8x6UnormBlock => vulkano::format::Format::ASTC_8x6_UNORM_BLOCK,
        Format::Astc8x6SrgbBlock => vulkano::format::Format::ASTC_8x6_SRGB_BLOCK,
        Format::Astc8x8UnormBlock => vulkano::format::Format::ASTC_8x8_UNORM_BLOCK,
        Format::Astc8x8SrgbBlock => vulkano::format::Format::ASTC_8x8_SRGB_BLOCK,
        Format::Astc10x5UnormBlock => vulkano::format::Format::ASTC_10x5_UNORM_BLOCK,
        Format::Astc10x5SrgbBlock => vulkano::format::Format::ASTC_10x5_SRGB_BLOCK,
        Format::Astc10x6UnormBlock => vulkano::format::Format::ASTC_10x6_UNORM_BLOCK,
        Format::Astc10x6SrgbBlock => vulkano::format::Format::ASTC_10x6_SRGB_BLOCK,
        Format::Astc10x8UnormBlock => vulkano::format::Format::ASTC_10x8_UNORM_BLOCK,
        Format::Astc10x8SrgbBlock => vulkano::format::Format::ASTC_10x8_SRGB_BLOCK,
        Format::Astc10x10UnormBlock => vulkano::format::Format::ASTC_10x10_UNORM_BLOCK,
        Format::Astc10x10SrgbBlock => vulkano::format::Format::ASTC_10x10_SRGB_BLOCK,
        Format::Astc12x10UnormBlock => vulkano::format::Format::ASTC_12x10_UNORM_BLOCK,
        Format::Astc12x10SrgbBlock => vulkano::format::Format::ASTC_12x10_SRGB_BLOCK,
        Format::Astc12x12UnormBlock => vulkano::format::Format::ASTC_12x12_UNORM_BLOCK,
        Format::Astc12x12SrgbBlock => vulkano::format::Format::ASTC_12x12_SRGB_BLOCK,
    }
}

#[inline]
pub(crate) fn vertex_buffer_description_to_vulkano(
    description: VertexBufferDescription,
) -> vulkano::pipeline::graphics::vertex_input::VertexBufferDescription {
    vulkano::pipeline::graphics::vertex_input::VertexBufferDescription {
        members: description
            .members
            .into_iter()
            .map(|(k, v)| {
                let format = format_to_vulkano(&v.format);
                (
                    k,
                    vulkano::pipeline::graphics::vertex_input::VertexMemberInfo {
                        offset: v.offset,
                        format,
                        num_elements: v.num_elements,
                        stride: v.stride,
                    },
                )
            })
            .collect(),
        stride: description.stride,
        input_rate: vulkano::pipeline::graphics::vertex_input::VertexInputRate::Vertex,
    }
}
