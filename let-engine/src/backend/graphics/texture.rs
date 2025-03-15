//! Texture related options.

use anyhow::anyhow;
use let_engine_core::resources::{
    buffer::BufferAccess,
    texture::{AddressMode, Filter, LoadedTexture, Sampler, Texture, TextureSettings, ViewTypeDim},
    Format, SampledFormatType,
};
use smallvec::smallvec;

use std::sync::Arc;
use vulkano::{
    buffer::{Buffer, BufferCreateInfo, BufferUsage, Subbuffer},
    command_buffer::{
        allocator::StandardCommandBufferAllocator, AutoCommandBufferBuilder, BufferImageCopy,
        ClearColorImageInfo, CopyBufferToImageInfo, CopyImageInfo, CopyImageToBufferInfo,
        PrimaryAutoCommandBuffer, PrimaryCommandBufferAbstract,
    },
    descriptor_set::layout::DescriptorType,
    image::{
        sampler::{Filter as vkFilter, SamplerAddressMode, SamplerCreateInfo, SamplerMipmapMode},
        view::{ImageView, ImageViewCreateInfo, ImageViewType},
        Image, ImageCreateInfo, ImageType, ImageUsage,
    },
    memory::allocator::{AllocationCreateInfo, MemoryTypeFilter, StandardMemoryAllocator},
    sync::{GpuFuture, HostAccessError},
    Validated,
};

use parking_lot::Mutex;

/// A VRAM loaded instance of a texture.
#[derive(Clone)]
pub struct GpuTexture {
    inner_texture: Arc<Mutex<InnerTexture>>,

    // References to resources needed to upload data to the GPU.
    future: Arc<Mutex<Option<Box<dyn GpuFuture + Send>>>>,
    queues: Arc<Queues>,
    memory_allocator: Arc<StandardMemoryAllocator>,
    command_buffer_allocator: Arc<StandardCommandBufferAllocator>,
}

pub(crate) struct InnerTexture {
    staging: Option<Subbuffer<[u8]>>,

    settings: TextureSettings,
    dimensions: ViewTypeDim,
    view: Arc<ImageView>,

    write: Option<Arc<PrimaryAutoCommandBuffer>>,
}

impl InnerTexture {
    // TODO: Make sampler optional
    pub(crate) fn descriptor_type(&self) -> DescriptorType {
        DescriptorType::CombinedImageSampler
    }

    pub(crate) fn vk_sampler(&self) -> SamplerCreateInfo {
        sampler_to_vulkano(&self.settings.sampler, &self.format().sampled_format_type())
    }

    pub(crate) fn view(&self) -> &Arc<ImageView> {
        &self.view
    }

    /// Returns the dimensions of this texture.
    pub fn dimensions(&self) -> &ViewTypeDim {
        &self.dimensions
    }

    /// Returns the format this texture requires.
    pub fn format(&self) -> &Format {
        &self.settings.format
    }
}

impl GpuTexture {
    pub fn new(texture: &Texture, interface: &GraphicsInterface) -> Result<Self, GpuTextureError> {
        let vulkan = interface.vulkan.clone();

        let settings = texture.settings();

        // Settings Mip Levels 3/4
        let mip_levels = settings.mip_levels;

        let dimensions = texture.dimensions();

        let format = format_to_vulkano(&settings.format);

        let access = &settings.access_pattern;

        let src_buffer =
            Self::create_src_buffer(vulkan.memory_allocator.clone(), settings, dimensions)?;

        // Write texture data into GPU buffer
        src_buffer.write().unwrap().copy_from_slice(texture.data());

        let image = Self::create_image(
            vulkan.memory_allocator.clone(),
            format,
            dimensions,
            mip_levels,
        )?;

        let mut write = AutoCommandBufferBuilder::primary(
            vulkan.command_buffer_allocator.clone(),
            vulkan.queues.transfer_id(),
            vulkano::command_buffer::CommandBufferUsage::MultipleSubmit,
        )
        .map_err(|e| GpuTextureError::Other(e.into()))?;

        write
            .copy_buffer_to_image(CopyBufferToImageInfo::buffer_image(
                src_buffer.clone(),
                image.clone(),
            ))
            .map_err(|e| GpuTextureError::Other(e.into()))?;

        // Move buffer to image
        let write = write
            .build()
            .map_err(|e| GpuTextureError::Other(e.into()))?;

        {
            let mut vulkan_future = vulkan.future.lock();

            // Keep future and command buffer to reuse when the texture gets updated.
            let command_buffer_future = write
                .clone()
                .execute(vulkan.queues.get_transfer().clone())
                .map_err(|e| GpuTextureError::Other(e.into()))?
                .then_signal_semaphore_and_flush()
                .map_err(|e| GpuTextureError::Other(e.into()))?
                .boxed_send();

            if let Some(future) = vulkan_future.take() {
                *vulkan_future = Some(future.join(command_buffer_future).boxed_send());
            } else {
                *vulkan_future = Some(command_buffer_future.boxed_send());
            }
        }

        let view = Self::create_image_view(image.clone(), dimensions)?;

        let inner_texture = Arc::new(Mutex::new(InnerTexture {
            staging: match access {
                BufferAccess::Fixed => None,
                BufferAccess::Staged => Some(src_buffer),
                _ => unreachable!(),
            },
            view,
            settings: settings.clone(),
            dimensions: *dimensions,
            write: match access {
                BufferAccess::Staged => Some(write),
                _ => None,
            },
        }));

        Ok(Self {
            inner_texture,
            future: vulkan.future.clone(),
            queues: vulkan.queues.clone(),
            memory_allocator: vulkan.memory_allocator.clone(),
            command_buffer_allocator: vulkan.command_buffer_allocator.clone(),
        })
    }

    /// Creates a new image that can only be accessed on the GPU.
    ///
    /// `settings.access_pattern` will always be `Fixed`
    pub fn new_gpu_only(
        dimensions: ViewTypeDim,
        mut settings: TextureSettings,
        interface: &GraphicsInterface,
    ) -> Result<Self, GpuTextureError> {
        let vulkan = &interface.vulkan;

        settings.access_pattern = BufferAccess::Fixed;

        let future = vulkan.future.clone();
        let queues = vulkan.queues.clone();
        let memory_allocator = vulkan.memory_allocator.clone();
        let command_buffer_allocator = vulkan.command_buffer_allocator.clone();

        let format = format_to_vulkano(&settings.format);

        // Create new image with given dimensions and settings
        let image = Image::new(
            memory_allocator.clone(),
            ImageCreateInfo {
                image_type: Self::image_type(&dimensions),
                format,
                extent: dimensions.extent(),
                array_layers: dimensions.array_layers(),
                usage: ImageUsage::SAMPLED,
                mip_levels: settings.mip_levels,
                ..Default::default()
            },
            AllocationCreateInfo::default(),
        )
        .map_err(|e| GpuTextureError::Other(e.into()))?;

        let view = Self::create_image_view(image, &dimensions)?;

        Ok(Self {
            inner_texture: Arc::new(Mutex::new(InnerTexture {
                staging: None,
                settings,
                dimensions,
                view,
                write: None,
            })),
            future,
            queues,
            memory_allocator,
            command_buffer_allocator,
        })
    }

    /// Returns the settings of this texture.
    pub fn settings(&self) -> TextureSettings {
        self.inner_texture.lock().settings.clone()
    }

    /// Returns the dimensions of this texture.
    pub fn dimensions(&self) -> ViewTypeDim {
        self.inner_texture.lock().dimensions
    }

    /// Returns the format this texture requires.
    pub fn format(&self) -> Format {
        self.inner_texture.lock().settings.format
    }

    pub(crate) fn inner(&self) -> &Arc<Mutex<InnerTexture>> {
        &self.inner_texture
    }
}

impl GpuTexture {
    fn create_src_buffer(
        memory_allocator: Arc<StandardMemoryAllocator>,
        settings: &TextureSettings,
        dimensions: &ViewTypeDim,
    ) -> Result<Subbuffer<[u8]>, GpuTextureError> {
        // let access_pattern = match settings.access_pattern {
        //     BufferAccess::Fixed TODO: Implement
        // };

        Buffer::new_slice(
            memory_allocator,
            BufferCreateInfo {
                usage: BufferUsage::TRANSFER_SRC,
                ..Default::default()
            },
            AllocationCreateInfo {
                memory_type_filter: MemoryTypeFilter::PREFER_HOST
                    | MemoryTypeFilter::HOST_SEQUENTIAL_WRITE,
                ..Default::default()
            },
            Texture::calculate_buffer_size(dimensions, settings) as u64,
        )
        .map_err(Validated::unwrap)
        .map_err(|e| GpuTextureError::Other(e.into()))
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
        memory_allocator: Arc<StandardMemoryAllocator>,
        format: vulkano::format::Format,
        dimensions: &ViewTypeDim,
        mip_levels: u32,
    ) -> Result<Arc<Image>, GpuTextureError> {
        // Create new image with given dimensions and settings
        Image::new(
            memory_allocator,
            ImageCreateInfo {
                image_type: Self::image_type(dimensions),
                format,
                extent: dimensions.extent(),
                array_layers: dimensions.array_layers(),
                usage: ImageUsage::TRANSFER_DST | ImageUsage::SAMPLED,
                mip_levels,
                ..Default::default()
            },
            AllocationCreateInfo {
                memory_type_filter: MemoryTypeFilter::PREFER_DEVICE,
                ..Default::default()
            },
        )
        .map_err(|e| GpuTextureError::Other(e.into()))
    }

    fn create_staging_image(
        memory_allocator: Arc<StandardMemoryAllocator>,
        format: vulkano::format::Format,
        dimensions: &ViewTypeDim,
        mip_levels: u32,
    ) -> Result<Arc<Image>, GpuTextureError> {
        Image::new(
            memory_allocator,
            ImageCreateInfo {
                image_type: Self::image_type(dimensions),
                format,
                extent: dimensions.extent(),
                array_layers: dimensions.array_layers(),
                usage: ImageUsage::TRANSFER_DST | ImageUsage::TRANSFER_SRC,
                mip_levels,
                ..Default::default()
            },
            AllocationCreateInfo {
                memory_type_filter: MemoryTypeFilter::PREFER_HOST,
                ..AllocationCreateInfo::default()
            },
        )
        .map_err(|e| GpuTextureError::Other(e.into()))
    }

    fn create_image_view(
        image: Arc<Image>,
        dimensions: &ViewTypeDim,
    ) -> Result<Arc<ImageView>, GpuTextureError> {
        let view_type = image_view_type_to_vulkano(dimensions);

        ImageView::new(
            image.clone(),
            ImageViewCreateInfo {
                view_type,
                ..ImageViewCreateInfo::from_image(&image)
            },
        )
        .map_err(|e| GpuTextureError::Other(e.into()))
    }
}

impl LoadedTexture for GpuTexture {
    type Error = GpuTextureError;

    /// Reads the texture from the GPU. Speed depends on Access preference set in the settings.
    fn data(&self) -> Result<Vec<u8>, Self::Error> {
        let inner_texture = self.inner_texture.lock();

        let access = inner_texture.settings.access_pattern;
        if let BufferAccess::Fixed = access {
            return Err(GpuTextureError::UnsupportedAccess(access));
        }

        let guard = inner_texture
            .staging
            .as_ref()
            .unwrap()
            .read()
            .map_err(GpuTextureError::HostAccess)?;

        Ok(guard.to_vec())
    }

    /// Returns the dimensions of this texture.
    fn dimensions(&self) -> ViewTypeDim {
        self.dimensions()
    }

    /// Resizes the texture leaving the original image on the top left and zeroed data in the place out of bounds of the original image.
    ///
    /// In case the target dimensions are smaller than the current dimensions, the data will be cut out.
    fn resize(&self, new_dimensions: ViewTypeDim) -> anyhow::Result<(), Self::Error> {
        let mut inner = self.inner_texture.lock();

        if image_view_type_to_vulkano(&new_dimensions)
            != image_view_type_to_vulkano(&inner.dimensions)
        {
            return Err(GpuTextureError::InvalidViewType(
                image_view_type_to_vulkano(&new_dimensions),
                image_view_type_to_vulkano(&inner.dimensions),
            ));
        }

        let settings = &inner.settings;

        if let BufferAccess::Fixed = settings.access_pattern {
            return Err(GpuTextureError::UnsupportedAccess(settings.access_pattern));
        }

        let format = format_to_vulkano(&settings.format);

        // Create staging image
        let staging_image = Self::create_staging_image(
            self.memory_allocator.clone(),
            format,
            &new_dimensions,
            settings.mip_levels,
        )?;

        // Create new buffer and image
        let new_buffer =
            Self::create_src_buffer(self.memory_allocator.clone(), settings, &new_dimensions)?;
        let new_image = Self::create_image(
            self.memory_allocator.clone(),
            format,
            &new_dimensions,
            settings.mip_levels,
        )?;

        let mut vulkan_future = self.future.lock();

        let Some(future) = vulkan_future.take() else {
            return Err(GpuTextureError::Other(anyhow!(
                "Unexpected missing future."
            )));
        };

        // Create command buffer and execute
        let mut command_buffer = AutoCommandBufferBuilder::primary(
            self.command_buffer_allocator.clone(),
            self.queues.transfer_id(),
            vulkano::command_buffer::CommandBufferUsage::OneTimeSubmit,
        )
        .map_err(|e| GpuTextureError::Other(e.into()))?;

        let extent = inner.dimensions.extent();

        let region = BufferImageCopy {
            image_subresource: staging_image.subresource_layers(),
            image_extent: extent,
            buffer_image_height: extent[1],
            buffer_row_length: extent[0],
            ..Default::default()
        };

        // Clear image, copy old buffer to new staging image, staging image to new buffer and staging image to image.
        command_buffer
            .clear_color_image(ClearColorImageInfo::image(staging_image.clone()))
            .map_err(|e| GpuTextureError::Other(e.into()))?
            .copy_buffer_to_image(CopyBufferToImageInfo {
                regions: smallvec![region],
                ..CopyBufferToImageInfo::buffer_image(
                    inner.staging.as_ref().unwrap().clone(),
                    staging_image.clone(),
                )
            })
            .map_err(|e| GpuTextureError::Other(e.into()))?
            .copy_image_to_buffer(CopyImageToBufferInfo::image_buffer(
                staging_image.clone(),
                new_buffer.clone(),
            ))
            .map_err(|e| GpuTextureError::Other(e.into()))?
            .copy_image(CopyImageInfo::images(
                staging_image.clone(),
                new_image.clone(),
            ))
            .map_err(|e| GpuTextureError::Other(e.into()))?;

        let command_buffer = command_buffer
            .build()
            .map_err(|e| GpuTextureError::Other(e.into()))?;

        let future = future
            .then_execute(self.queues.get_transfer().clone(), command_buffer)
            .map_err(|e| GpuTextureError::Other(e.into()))?
            .boxed_send();

        let mut write = AutoCommandBufferBuilder::primary(
            self.command_buffer_allocator.clone(),
            self.queues.transfer_id(),
            vulkano::command_buffer::CommandBufferUsage::MultipleSubmit,
        )
        .map_err(|e| GpuTextureError::Other(e.into()))?;

        *vulkan_future = Some(future);

        std::mem::drop(vulkan_future);

        // Create new command buffer for updating the texture.
        write
            .copy_buffer_to_image(CopyBufferToImageInfo::buffer_image(
                new_buffer.clone(),
                new_image.clone(),
            ))
            .map_err(|e| GpuTextureError::Other(e.into()))?;

        let view = Self::create_image_view(new_image, &new_dimensions)?;

        let write = write
            .build()
            .map_err(|e| GpuTextureError::Other(e.into()))?;

        inner.staging = Some(new_buffer);
        inner.dimensions = new_dimensions;
        inner.view = view;

        inner.write = Some(write);

        Ok(())
    }

    /// Writes the modified bytes to the GPU. Speed depends on Access preference set in settings.
    fn write_data<F: FnMut(&mut [u8])>(&self, mut f: F) -> Result<(), Self::Error> {
        let inner = self.inner_texture.lock();

        if let BufferAccess::Fixed = inner.settings.access_pattern {
            return Err(GpuTextureError::UnsupportedAccess(
                inner.settings.access_pattern,
            ));
        }

        {
            let mut guard = inner
                .staging
                .as_ref()
                .unwrap()
                .write()
                .map_err(GpuTextureError::HostAccess)?;

            f(&mut guard);
        }

        let mut vulkan_future = self.future.lock();

        if let Some(future) = vulkan_future.take() {
            let future = future
                .then_execute(
                    self.queues.get_transfer().clone(),
                    inner.write.as_ref().unwrap().clone(),
                )
                .map_err(|e| GpuTextureError::Other(e.into()))?
                .boxed_send();
            *vulkan_future = Some(future);
        };

        Ok(())
    }

    // /// Scales the object appearance to how many pixels represent 1 according to the texture applied and returns it.
    // ///
    // /// Using 1000 works best in Expand camera mode for best quality.
    // pub fn auto_scaled(mut self, pixels_per_unit: f32) -> Result<Self, TextureError> {
    //     self.auto_scale(pixels_per_unit)?;
    //     Ok(self)
    // }

    // /// Scales the object appearance to how many pixels represent 1 according to the texture applied.
    // ///
    // /// Using 1000 works best in Expand camera mode for best quality.
    // pub fn auto_scale(&mut self, pixels_per_unit: f32) -> Result<(), TextureError> {
    //     let dimensions;
    //     if let Some(material) = &self.instance.material {
    //         dimensions = if let Some(texture) = material.texture() {
    //             texture.dimensions()
    //         } else {
    //             return Err(TextureError::NoTexture);
    //         };
    //     } else {
    //         return Err(TextureError::NoTexture);
    //     };

    //     self.transform.size = vec2(
    //         dimensions.0 as f32 / pixels_per_unit,
    //         dimensions.1 as f32 / pixels_per_unit,
    //     );

    //     Ok(())
    // }
}

// Texture based errors.

use thiserror::Error;

use super::{format_to_vulkano, vulkan::Queues, GraphicsInterface};

/// Errors that occur from the GPU texture.
#[derive(Error, Debug)]
pub enum GpuTextureError {
    /// When resizing the wrong view type format was used.
    #[error("Wrong view type used. Can not resize from {0:?} to {1:?}.")]
    InvalidViewType(ImageViewType, ImageViewType),

    /// Returns when the access operation is not supported with the currently set access setting.
    #[error("Requested access operation not possible with current access setting: {0:?}")]
    UnsupportedAccess(BufferAccess),

    /// Returns if there was an error attempting to read or write a texture from and to the GPU.
    #[error("{0}")]
    HostAccess(HostAccessError),

    /// If the texture for some reason can not be made.
    #[error("There was an error loading this texture: {0}")]
    Other(anyhow::Error),
}

fn sampler_to_vulkano(sampler: &Sampler, format_type: &SampledFormatType) -> SamplerCreateInfo {
    SamplerCreateInfo {
        mag_filter: match sampler.mag_filter {
            Filter::Nearest => vkFilter::Nearest,
            Filter::Linear => vkFilter::Linear,
        },
        min_filter: match sampler.mag_filter {
            Filter::Nearest => vkFilter::Nearest,
            Filter::Linear => vkFilter::Linear,
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
