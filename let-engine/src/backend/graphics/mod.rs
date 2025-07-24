//! Default graphics backend made with `Vulkano`

use std::{cell::OnceCell, collections::BTreeMap, sync::Arc};

use anyhow::{anyhow, Context, Result};
use buffer::{DrawableBuffer, GpuBuffer};
use crossbeam::channel::{bounded, Receiver, Sender};
use draw::Draw;
use glam::UVec2;
use let_engine_core::{
    backend::graphics::{GraphicsBackend, Loaded},
    objects::{scenes::Scene, Color, Descriptor},
    resources::{
        buffer::Location,
        data::Data,
        model::{Vertex, VertexBufferDescription},
        texture::LoadedTexture,
        Format,
    },
};
use material::{eq_vertex_input_state, GpuMaterial, ShaderError, VulkanGraphicsShaders};
use model::{DrawableModel, GpuModel};
use parking_lot::RwLock;
use texture::{image_view_type_to_vulkano, GpuTexture};
use thiserror::Error;
use vulkan::{Vulkan, VK};
use vulkano::{
    buffer::AllocateBufferError,
    descriptor_set::layout::DescriptorType,
    format::NumericType,
    image::{view::ImageViewType, AllocateImageError},
    memory::allocator::MemoryAllocatorError,
    pipeline::graphics::vertex_input::VertexDefinition,
    shader::spirv::SpirvBytesNotMultipleOf4,
    LoadingError, VulkanError as VulkanoError,
};

use winit::raw_window_handle::HasDisplayHandle;

pub use vulkano::DeviceSize;

pub mod buffer;
pub mod material;
pub mod model;
pub mod texture;

mod draw;
mod vulkan;

#[derive(Debug)]
pub struct DefaultGraphicsBackend {
    draw: OnceCell<Draw>,
    interface: GraphicsInterface,
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
    type Interface = GraphicsInterface;

    type LoadedTypes = VulkanTypes;

    fn new(settings: &Self::Settings, handle: impl HasDisplayHandle) -> Result<Self, Self::Error> {
        // Initialize backend in case it is not already initialized.
        if VK.get().is_none() {
            let vulkan = Vulkan::init(&handle, settings)?;
            let _ = VK.set(vulkan);
        }

        let interface = GraphicsInterface::new(*settings);

        Ok(Self {
            draw: OnceCell::new(),
            interface,
        })
    }

    fn init_window(
        &mut self,
        window: &Arc<
            impl winit::raw_window_handle::HasWindowHandle
                + HasDisplayHandle
                + std::any::Any
                + Send
                + Sync,
        >,
        scene: &Arc<Scene<Self::LoadedTypes>>,
    ) {
        // TODO: Remove unwraps
        let draw = Draw::new(self.interface.clone(), window, scene.clone()).unwrap();

        let _ = self.draw.set(draw);
    }

    fn interface(&self) -> &GraphicsInterface {
        &self.interface
    }

    fn draw(&mut self, pre_present_notify: impl FnOnce()) -> Result<(), Self::Error> {
        if let Some(draw) = self.draw.get_mut() {
            draw.redraw_event(pre_present_notify).map_err(|e| e.into())
        } else {
            Ok(())
        }
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
    type Buffer<B: Data> = GpuBuffer<B>;
    type DrawableBuffer = DrawableBuffer;
    type Model<V: Vertex> = GpuModel<V>;
    type DrawableModel = DrawableModel;
    type Texture = GpuTexture;

    type AppearanceCreationError = AppearanceCreationError;

    fn initialize_appearance(
        material: &GpuMaterial,
        model: &DrawableModel,
        descriptors: &BTreeMap<Location, Descriptor<VulkanTypes>>,
    ) -> Result<(), AppearanceCreationError> {
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
                Descriptor::Texture(texture) => {
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

                    if let Some(format) = requirement.image_format {
                        if format != texture_format {
                            return Err(AppearanceCreationError::WrongTextureFormat {
                                location: *location,
                                expected_format: format,
                                provided_format: texture_format,
                            });
                        }
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

                Descriptor::Buffer(buffer) => {
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
    fn draw_buffer<B: Data>(buffer: Self::Buffer<B>) -> Self::DrawableBuffer {
        DrawableBuffer::from_buffer(buffer)
    }

    fn draw_model<V: Vertex>(model: Self::Model<V>) -> Self::DrawableModel {
        DrawableModel::from_model(model)
    }
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
    #[error("Shader requires format {expected_format:?} at texture location {location:?}, but got {provided_format:?}.")]
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
}

#[derive(Debug, Clone)]
pub struct GraphicsInterface {
    settings: Arc<RwLock<Graphics>>,
    settings_channels: (Sender<Graphics>, Receiver<Graphics>),

    // Gets written to in swapchain.rs
    available_present_modes: Arc<RwLock<Vec<PresentMode>>>,
}

impl let_engine_core::backend::graphics::GraphicsInterface<VulkanTypes> for GraphicsInterface {
    fn load_material<V: let_engine_core::resources::model::Vertex>(
        &self,
        material: &let_engine_core::resources::material::Material,
    ) -> Result<GpuMaterial> {
        let shaders =
            unsafe { VulkanGraphicsShaders::from_bytes(material.graphics_shaders.clone())? };

        GpuMaterial::new::<V>(material.settings.clone(), shaders).context("hello")
    }

    fn load_buffer<B: Data>(
        &self,
        buffer: &let_engine_core::resources::buffer::Buffer<B>,
    ) -> Result<GpuBuffer<B>> {
        GpuBuffer::new(buffer).context("failed to load buffer")
    }

    fn load_model<V: let_engine_core::resources::model::Vertex>(
        &self,
        model: &let_engine_core::resources::model::Model<V>,
    ) -> Result<GpuModel<V>> {
        GpuModel::new(model).context("failed to load model")
    }

    fn load_texture(
        &self,
        texture: &let_engine_core::resources::texture::Texture,
    ) -> Result<GpuTexture> {
        GpuTexture::new(texture).context("failed to load texture")
    }
}

impl GraphicsInterface {
    fn new(settings: Graphics) -> Self {
        Self {
            settings: Arc::new(RwLock::new(settings)),
            settings_channels: bounded(3),
            available_present_modes: Arc::new(RwLock::new(vec![])),
        }
    }

    /// Returns the settings of the graphics backend.
    pub fn settings(&self) -> Graphics {
        *self.settings.read()
    }

    fn send_settings(&self, settings: Graphics) {
        let _ = self.settings_channels.0.try_send(settings);
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
    pub fn supported_present_modes(&self) -> Vec<PresentMode> {
        self.available_present_modes.read().clone()
    }

    /// Sets the present mode of the graphics backend. Returns an error in case the present mode is not supported.
    pub fn set_present_mode(&self, present_mode: PresentMode) -> Result<()> {
        if self.available_present_modes.read().contains(&present_mode) {
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

    // /// Sets the framerate limit as waiting time between frames.
    // ///
    // /// This should be able to be changed by the user in case they have a device with limited power capacity like a laptop with a battery.
    // ///
    // /// Setting the duration to no wait time at all will turn off the limit.
    // pub fn set_framerate_limit(&self, limit: Duration) {
    //     *self.framerate_limit.lock() = limit;
    // }

    // /// Sets the cap for the max frames per second the game should be able to output.
    // ///
    // /// This method is the same as setting the `set_framerate_limit` of this setting to `1.0 / cap` in seconds.
    // ///
    // /// Turns off the framerate cap if 0 was given.
    // pub fn set_fps_cap(&self, cap: u64) {
    //     if cap == 0 {
    //         self.set_framerate_limit(Duration::from_secs(cap));
    //         return;
    //     }
    //     self.set_framerate_limit(Duration::from_secs_f64(1.0 / cap as f64));
    // }
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
    pub clear_color: Color, // TODO

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
                                     // pub framerate_limit: Duration,
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
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
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
