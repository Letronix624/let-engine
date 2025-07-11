//! Texture related options.

use let_engine_core::resources::{
    buffer::BufferAccess,
    texture::{AddressMode, Filter, LoadedTexture, Sampler, Texture, TextureSettings, ViewTypeDim},
    SampledFormatType,
};
use vulkano_taskgraph::{
    command_buffer::{CopyBufferToImageInfo, CopyImageToBufferInfo},
    resource::{AccessTypes, HostAccessType, ImageLayoutType},
    Id,
};

use std::sync::Arc;
use vulkano::{
    buffer::{Buffer, BufferCreateInfo, BufferUsage},
    descriptor_set::layout::DescriptorType,
    image::{
        sampler::{Filter as VkFilter, SamplerAddressMode, SamplerCreateInfo, SamplerMipmapMode},
        view::{ImageView, ImageViewCreateInfo, ImageViewType},
        Image, ImageCreateInfo, ImageType, ImageUsage,
    },
    memory::allocator::{AllocationCreateInfo, DeviceLayout, MemoryTypeFilter},
    sync::HostAccessError,
    DeviceSize,
};

/// A VRAM loaded instance of a texture.
#[derive(Clone)]
pub struct GpuTexture {
    image_id: Id<Image>,
    view: Arc<ImageView>,
    settings: TextureSettings,
    dimensions: ViewTypeDim,

    staging: Option<Id<Buffer>>,
}

impl PartialEq for GpuTexture {
    fn eq(&self, other: &Self) -> bool {
        self.image_id == other.image_id
            && Arc::ptr_eq(&self.view, &other.view)
            && self.settings == other.settings
            && self.dimensions == other.dimensions
            && self.staging == other.staging
    }
}

impl GpuTexture {
    pub fn new(texture: &Texture) -> Result<Self, GpuTextureError> {
        let vulkan = VK.get().ok_or(GpuTextureError::BackendNotInitialized)?;

        let settings = texture.settings();

        let mip_levels = settings.mip_levels;

        let dimensions = texture.dimensions();

        let format = format_to_vulkano(&settings.format);

        let access = &settings.access_pattern;

        let buffer_id = Self::create_buffer(vulkan, settings, dimensions);

        let image_id = Self::create_image(vulkan, format, dimensions, mip_levels)?;

        let flight = vulkan.transfer_flight().unwrap();
        flight.wait(None).unwrap();

        // Write texture data into GPU buffer & copy to texture
        unsafe {
            vulkano_taskgraph::execute(
                vulkan.queues.transfer(),
                &vulkan.resources,
                vulkan.transfer_flight,
                |cb, ctx| {
                    let write: &mut [u8] = ctx.write_buffer(buffer_id, ..)?;

                    write.copy_from_slice(texture.data());

                    cb.copy_buffer_to_image(
                        &vulkano_taskgraph::command_buffer::CopyBufferToImageInfo {
                            src_buffer: buffer_id,
                            dst_image: image_id,
                            dst_image_layout: ImageLayoutType::Optimal,
                            ..Default::default()
                        },
                    )?;

                    Ok(())
                },
                [(buffer_id, HostAccessType::Write)],
                [(buffer_id, AccessTypes::COPY_TRANSFER_READ)],
                [(
                    image_id,
                    AccessTypes::COPY_TRANSFER_WRITE,
                    ImageLayoutType::Optimal,
                )],
            )
        }
        .unwrap();

        let view_type = image_view_type_to_vulkano(dimensions);

        let image_state = vulkan.resources.image(image_id).unwrap();

        let view = ImageView::new(
            image_state.image(),
            &ImageViewCreateInfo {
                view_type,
                ..ImageViewCreateInfo::from_image(image_state.image())
            },
        )
        .unwrap();

        vulkan.add_resource(super::vulkan::Resource::Image {
            id: image_id,
            access_types: AccessTypes::FRAGMENT_SHADER_SAMPLED_READ,
        });

        Ok(Self {
            image_id,
            view,
            settings: settings.clone(),
            dimensions: *dimensions,

            staging: match access {
                BufferAccess::Fixed => None,
                BufferAccess::Staged => Some(buffer_id),
                _ => unreachable!(),
            },
        })
    }

    /// Creates a new image that can only be accessed on the GPU.
    ///
    /// `settings.access_pattern` will always be `Fixed`
    pub fn new_gpu_only(
        dimensions: ViewTypeDim,
        mut settings: TextureSettings,
    ) -> Result<Self, GpuTextureError> {
        let vulkan = VK.get().ok_or(GpuTextureError::BackendNotInitialized)?;

        settings.access_pattern = BufferAccess::Fixed;

        let format = format_to_vulkano(&settings.format);

        // Create new image with given dimensions and settings
        let image_id = vulkan
            .resources
            .create_image(
                &ImageCreateInfo {
                    image_type: Self::image_type(&dimensions),
                    format,
                    extent: dimensions.extent(),
                    array_layers: dimensions.array_layers(),
                    usage: ImageUsage::SAMPLED,
                    mip_levels: settings.mip_levels,
                    ..Default::default()
                },
                &AllocationCreateInfo::default(),
            )
            .unwrap(); // TODO: unwraps

        let image_state = vulkan.resources.image(image_id).unwrap();

        let view_type = image_view_type_to_vulkano(&dimensions);

        let view = ImageView::new(
            image_state.image(),
            &ImageViewCreateInfo {
                view_type,
                ..ImageViewCreateInfo::from_image(image_state.image())
            },
        )
        .unwrap();

        vulkan.add_resource(super::vulkan::Resource::Image {
            id: image_id,
            access_types: AccessTypes::FRAGMENT_SHADER_SAMPLED_READ,
        });

        Ok(Self {
            image_id,
            view,
            settings,
            dimensions,
            staging: None,
        })
    }

    /// Returns the settings of this texture.
    pub fn settings(&self) -> &TextureSettings {
        &self.settings
    }
}

impl GpuTexture {
    fn create_buffer(
        vulkan: &Vulkan,
        settings: &TextureSettings,
        dimensions: &ViewTypeDim,
    ) -> Id<Buffer> {
        vulkan
            .resources
            .create_buffer(
                &BufferCreateInfo {
                    usage: BufferUsage::TRANSFER_SRC | BufferUsage::TRANSFER_DST,
                    ..Default::default()
                },
                &AllocationCreateInfo {
                    memory_type_filter: MemoryTypeFilter::PREFER_HOST
                        | MemoryTypeFilter::HOST_SEQUENTIAL_WRITE,
                    ..Default::default()
                },
                DeviceLayout::new_unsized::<[u8]>(Texture::calculate_buffer_size(
                    dimensions, settings,
                ) as DeviceSize)
                .unwrap(),
            )
            .unwrap()
    }

    fn image_type(dimensions: &ViewTypeDim) -> ImageType {
        match dimensions {
            ViewTypeDim::D1 { .. } => ImageType::Dim1d,
            ViewTypeDim::D2 { .. } => ImageType::Dim2d,
            ViewTypeDim::D3 { .. } => ImageType::Dim3d,
            ViewTypeDim::CubeMap { .. } => ImageType::Dim2d,
            ViewTypeDim::D1Array { .. } => ImageType::Dim1d,
            ViewTypeDim::D2Array { .. } => ImageType::Dim2d,
            ViewTypeDim::CubeArray { .. } => ImageType::Dim2d,
        }
    }

    fn create_image(
        vulkan: &Vulkan,
        format: vulkano::format::Format,
        dimensions: &ViewTypeDim,
        mip_levels: u32,
    ) -> Result<Id<Image>, GpuTextureError> {
        let usage = ImageUsage::TRANSFER_SRC | ImageUsage::TRANSFER_DST | ImageUsage::SAMPLED;

        // Create new image with given dimensions and settings
        vulkan
            .resources
            .create_image(
                &ImageCreateInfo {
                    image_type: Self::image_type(dimensions),
                    format,
                    extent: dimensions.extent(),
                    array_layers: dimensions.array_layers(),
                    usage,
                    mip_levels,
                    ..Default::default()
                },
                &AllocationCreateInfo {
                    memory_type_filter: MemoryTypeFilter::PREFER_DEVICE,
                    ..Default::default()
                },
            )
            .map_err(|e| GpuTextureError::Other(e.unwrap().into()))
    }

    pub(crate) fn image_view(&self) -> &Arc<ImageView> {
        &self.view
    }

    pub(crate) fn vk_sampler(&self) -> vulkano::image::sampler::SamplerCreateInfo {
        sampler_to_vulkano(
            &self.settings.sampler,
            &self.settings.format.sampled_format_type(),
        )
    }

    // TODO: Make sampler optional
    pub(crate) fn descriptor_type(&self) -> DescriptorType {
        DescriptorType::CombinedImageSampler
    }
}

impl LoadedTexture for GpuTexture {
    type Error = GpuTextureError;

    /// Reads the texture from the GPU. Speed depends on Access preference set in the settings.
    fn data<F: FnOnce(&[u8])>(&self, f: F) -> Result<(), Self::Error> {
        let access = self.settings.access_pattern;
        let Some(staging_id) = self.staging else {
            return Err(GpuTextureError::UnsupportedAccess(access));
        };

        let vulkan = VK.get().ok_or(GpuTextureError::BackendNotInitialized)?;

        let queue = vulkan.queues.transfer();
        let flight = vulkan.transfer_flight().unwrap();

        flight.wait(None).unwrap();

        // Task 1: image -> staging
        unsafe {
            vulkano_taskgraph::execute(
                queue,
                &vulkan.resources,
                vulkan.transfer_flight,
                |cb, _| {
                    cb.copy_image_to_buffer(&CopyImageToBufferInfo {
                        src_image: self.image_id,
                        dst_buffer: staging_id,
                        ..Default::default()
                    })?;
                    Ok(())
                },
                [],
                [(staging_id, AccessTypes::COPY_TRANSFER_WRITE)],
                [(
                    self.image_id,
                    AccessTypes::COPY_TRANSFER_READ,
                    ImageLayoutType::Optimal,
                )],
            )
        }
        .unwrap();

        flight.wait(None).unwrap();

        // Task 2: read staging
        unsafe {
            vulkano_taskgraph::execute(
                queue,
                &vulkan.resources,
                vulkan.transfer_flight,
                |_, ctx| {
                    let read: &[u8] = ctx.read_buffer(staging_id, ..)?;

                    f(read);

                    Ok(())
                },
                [(staging_id, HostAccessType::Read)],
                [],
                [],
            )
        }
        .unwrap();

        Ok(())
    }

    /// Returns the dimensions of this texture.
    fn dimensions(&self) -> &ViewTypeDim {
        &self.dimensions
    }

    /// Writes the modified bytes to the GPU. Speed depends on Access preference set in settings.
    fn write_data<F: FnOnce(&mut [u8])>(&self, f: F) -> Result<(), Self::Error> {
        let Some(staging_id) = self.staging else {
            return Err(GpuTextureError::UnsupportedAccess(
                self.settings.access_pattern,
            ));
        };

        let vulkan = VK.get().ok_or(GpuTextureError::BackendNotInitialized)?;
        let queue = vulkan.queues.transfer();

        let flight = vulkan.transfer_flight().unwrap();

        flight.wait(None).unwrap();

        unsafe {
            vulkano_taskgraph::execute(
                queue,
                &vulkan.resources,
                vulkan.transfer_flight,
                |cb, ctx| {
                    let write: &mut [u8] = ctx.write_buffer(staging_id, ..)?;
                    f(write);

                    cb.copy_buffer_to_image(&CopyBufferToImageInfo {
                        src_buffer: staging_id,
                        dst_image: self.image_id,
                        ..Default::default()
                    })?;

                    Ok(())
                },
                [(staging_id, HostAccessType::Write)],
                [
                    (staging_id, AccessTypes::COPY_TRANSFER_WRITE),
                    (staging_id, AccessTypes::COPY_TRANSFER_READ),
                ],
                [(
                    self.image_id,
                    AccessTypes::COPY_TRANSFER_WRITE,
                    ImageLayoutType::Optimal,
                )],
            )
        }
        .unwrap();

        Ok(())
    }
}

// Texture based errors.

use thiserror::Error;

use super::{
    format_to_vulkano,
    vulkan::{Vulkan, VK},
    VulkanError,
};

/// Errors that occur from the GPU texture.
#[derive(Error, Debug)]
pub enum GpuTextureError {
    /// Returns when attempting to create a texture,
    /// but the engine has not been started with [`Engine::start`](crate::Engine::start),
    /// or the backend has closed down.
    #[error("Can not create texture: Engine not initialized.")]
    BackendNotInitialized,

    /// When resizing the wrong view type format was used.
    #[error("Wrong view type used. Can not resize from {0:?} to {1:?}.")]
    InvalidViewType(ImageViewType, ImageViewType),

    /// Returns when the used format is not supported for use on the device.
    #[error("This format is not supported on your device.")]
    FormatNotSupported,

    /// Returns when the access operation is not supported with the currently set access setting.
    #[error("Requested access operation not possible with current access setting: {0:?}")]
    UnsupportedAccess(BufferAccess),

    /// Returns if there was an error attempting to read or write a texture from and to the GPU.
    #[error("{0}")]
    HostAccess(HostAccessError),

    /// If the texture for some other reason can not be made.
    #[error("There was an error loading this texture: {0}")]
    Other(VulkanError),
}

fn sampler_to_vulkano<'a>(
    sampler: &Sampler,
    format_type: &SampledFormatType,
) -> SamplerCreateInfo<'a> {
    SamplerCreateInfo {
        mag_filter: match sampler.mag_filter {
            Filter::Nearest => VkFilter::Nearest,
            Filter::Linear => VkFilter::Linear,
        },
        min_filter: match sampler.mag_filter {
            Filter::Nearest => VkFilter::Nearest,
            Filter::Linear => VkFilter::Linear,
        },
        mipmap_mode: match sampler.mag_filter {
            Filter::Nearest => SamplerMipmapMode::Nearest,
            Filter::Linear => SamplerMipmapMode::Linear,
        },

        // improvable.
        address_mode: [
            match sampler.address_mode[0] {
                AddressMode::Repeat => SamplerAddressMode::Repeat,
                AddressMode::Mirrored => SamplerAddressMode::MirroredRepeat,
                AddressMode::ClampToEdge => SamplerAddressMode::ClampToEdge,
                AddressMode::ClampToBorder => SamplerAddressMode::ClampToBorder,
            },
            match sampler.address_mode[1] {
                AddressMode::Repeat => SamplerAddressMode::Repeat,
                AddressMode::Mirrored => SamplerAddressMode::MirroredRepeat,
                AddressMode::ClampToEdge => SamplerAddressMode::ClampToEdge,
                AddressMode::ClampToBorder => SamplerAddressMode::ClampToBorder,
            },
            match sampler.address_mode[2] {
                AddressMode::Repeat => SamplerAddressMode::Repeat,
                AddressMode::Mirrored => SamplerAddressMode::MirroredRepeat,
                AddressMode::ClampToEdge => SamplerAddressMode::ClampToEdge,
                AddressMode::ClampToBorder => SamplerAddressMode::ClampToBorder,
            },
        ],
        border_color: match sampler.border_color {
            let_engine_core::resources::texture::BorderColor::Transparent => match format_type {
                SampledFormatType::Float => {
                    vulkano::image::sampler::BorderColor::FloatTransparentBlack
                }
                SampledFormatType::Int => vulkano::image::sampler::BorderColor::IntTransparentBlack,
            },
            let_engine_core::resources::texture::BorderColor::Black => match format_type {
                SampledFormatType::Float => vulkano::image::sampler::BorderColor::FloatOpaqueBlack,
                SampledFormatType::Int => vulkano::image::sampler::BorderColor::IntOpaqueBlack,
            },
            let_engine_core::resources::texture::BorderColor::White => match format_type {
                SampledFormatType::Float => vulkano::image::sampler::BorderColor::FloatOpaqueWhite,
                SampledFormatType::Int => vulkano::image::sampler::BorderColor::IntOpaqueWhite,
            },
        },
        ..Default::default()
    }
}

pub(crate) fn image_view_type_to_vulkano(dimensions: &ViewTypeDim) -> ImageViewType {
    match dimensions {
        ViewTypeDim::D1 { .. } => ImageViewType::Dim1d,
        ViewTypeDim::D2 { .. } => ImageViewType::Dim2d,
        ViewTypeDim::D3 { .. } => ImageViewType::Dim3d,
        ViewTypeDim::CubeMap { .. } => ImageViewType::Cube,
        ViewTypeDim::D1Array { .. } => ImageViewType::Dim1dArray,
        ViewTypeDim::D2Array { .. } => ImageViewType::Dim2dArray,
        ViewTypeDim::CubeArray { .. } => ImageViewType::CubeArray,
    }
}
