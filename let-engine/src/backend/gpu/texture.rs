//! Texture related options.

use concurrent_slotmap::declare_key;
use let_engine_core::resources::{
    SampledFormatType,
    buffer::BufferAccess,
    texture::{AddressMode, Filter, LoadedTexture, Sampler, Texture, TextureSettings, ViewTypeDim},
};
use vulkano_taskgraph::{
    Id,
    command_buffer::{CopyBufferToImageInfo, CopyImageToBufferInfo},
    resource::{AccessTypes, HostAccessType, ImageLayoutType},
};

use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering::Relaxed},
};
use vulkano::{
    DeviceSize,
    buffer::{Buffer, BufferCreateInfo, BufferUsage},
    descriptor_set::layout::DescriptorType,
    image::{
        Image, ImageCreateInfo, ImageType, ImageUsage,
        sampler::{Filter as VkFilter, SamplerAddressMode, SamplerCreateInfo, SamplerMipmapMode},
        view::{ImageView, ImageViewCreateInfo, ImageViewType},
    },
    memory::allocator::{AllocationCreateInfo, DeviceLayout, MemoryTypeFilter},
    sync::HostAccessError,
};

enum GpuTextureInner {
    Fixed {
        image_id: Id<Image>,
        image_view: Arc<ImageView>,
    },
    Staged {
        image_id: Id<Image>,
        image_view: Arc<ImageView>,
        staging_id: Id<Buffer>,
    },
    RingBuffer {
        image_ids: Box<[Id<Image>]>,
        image_views: Box<[Arc<ImageView>]>,
        staging_id: Id<Buffer>,
        turn: AtomicUsize,
    },
}

/// A VRAM loaded instance of a texture.
pub struct GpuTexture {
    inner: GpuTextureInner,

    settings: TextureSettings,
    dimensions: ViewTypeDim,
}

declare_key! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub struct TextureId
}

impl TextureId {
    pub const TAG_BIT: u32 = 1 << 3;

    pub fn is_virtual(&self) -> bool {
        self.0.tag() & VIRTUAL_TAG_BIT != 0
    }
}

impl GpuTexture {
    pub(crate) fn new(texture: &Texture, vulkan: &Vulkan) -> Result<Self, GpuTextureError> {
        let settings = texture.settings();
        let dimensions = texture.dimensions();

        let inner = match texture.settings().access_pattern {
            BufferAccess::Fixed => {
                let (_, image_id, image_view) = Self::staged_write(vulkan, texture);

                GpuTextureInner::Fixed {
                    image_id,
                    image_view,
                }
            }
            BufferAccess::Staged => {
                let (staging_id, image_id, image_view) = Self::staged_write(vulkan, texture);

                GpuTextureInner::Staged {
                    image_id,
                    image_view,
                    staging_id,
                }
            }
            BufferAccess::RingBuffer { buffers } => Self::ring(vulkan, texture, buffers),
            other => return Err(GpuTextureError::UnsupportedAccess(other)),
        };

        vulkan.flag_taskgraph_to_be_rebuilt();

        Ok(Self {
            inner,
            settings: settings.clone(),
            dimensions: *dimensions,
        })
    }

    /// Creates a new image that can only be accessed on the GPU.
    ///
    /// `settings.access_pattern` will always be `Fixed`
    pub(crate) fn new_gpu_only(
        dimensions: ViewTypeDim,
        mut settings: TextureSettings,
        vulkan: &Vulkan,
    ) -> Result<Self, GpuTextureError> {
        settings.access_pattern = BufferAccess::Fixed;

        let format = format_to_vulkano(&settings.format);

        // Create new image with given dimensions and settings
        let image_id = vulkan
            .resources
            .create_image(
                &ImageCreateInfo {
                    image_type: image_type_to_vulkano(&dimensions),
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

        let image_view = ImageView::new(
            image_state.image(),
            &ImageViewCreateInfo {
                view_type,
                ..ImageViewCreateInfo::from_image(image_state.image())
            },
        )
        .unwrap();

        vulkan.flag_taskgraph_to_be_rebuilt();

        Ok(Self {
            inner: GpuTextureInner::Fixed {
                image_id,
                image_view,
            },
            settings,
            dimensions,
        })
    }

    /// Returns the settings of this texture.
    pub fn settings(&self) -> &TextureSettings {
        &self.settings
    }

    pub(crate) fn image_view(&self) -> &Arc<ImageView> {
        match &self.inner {
            GpuTextureInner::Fixed { image_view, .. }
            | GpuTextureInner::Staged { image_view, .. } => image_view,
            GpuTextureInner::RingBuffer {
                image_views, turn, ..
            } => &image_views[turn.load(Relaxed)],
        }
    }

    pub(crate) fn resources(&self) -> Vec<ResourceAccess> {
        let access_types = AccessTypes::COLOR_ATTACHMENT_READ;
        match &self.inner {
            GpuTextureInner::Fixed { image_id, .. } | GpuTextureInner::Staged { image_id, .. } => {
                vec![ResourceAccess::Image {
                    id: *image_id,
                    access_types,
                }]
            }
            GpuTextureInner::RingBuffer { image_ids, .. } => image_ids
                .iter()
                .map(|id| ResourceAccess::Image {
                    id: *id,
                    access_types,
                })
                .collect(),
        }
    }
}

impl GpuTexture {
    fn staged_write(vulkan: &Vulkan, texture: &Texture) -> (Id<Buffer>, Id<Image>, Arc<ImageView>) {
        let buffer_id = vulkan
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
                    texture.dimensions(),
                    texture.settings(),
                ) as DeviceSize)
                .unwrap(),
            )
            .unwrap();

        let format = format_to_vulkano(&texture.settings().format);
        let image_id = vulkan
            .resources
            .create_image(
                &ImageCreateInfo {
                    image_type: image_type_to_vulkano(texture.dimensions()),
                    format,
                    extent: texture.dimensions().extent(),
                    array_layers: texture.dimensions().array_layers(),
                    usage: ImageUsage::TRANSFER_SRC
                        | ImageUsage::TRANSFER_DST
                        | ImageUsage::SAMPLED,
                    mip_levels: texture.settings().mip_levels,
                    ..Default::default()
                },
                &AllocationCreateInfo {
                    memory_type_filter: MemoryTypeFilter::PREFER_DEVICE,
                    ..Default::default()
                },
            )
            .map_err(|e| GpuTextureError::Other(e.unwrap().into()))
            .unwrap();

        // Write texture data into GPU buffer & copy to texture
        vulkan.wait_transfer();
        unsafe {
            vulkano_taskgraph::execute(
                vulkan.queues.transfer(),
                &vulkan.resources,
                vulkan.transfer_flight,
                |cb, ctx| {
                    let write: &mut [u8] = ctx.write_buffer(buffer_id, ..)?;

                    write.copy_from_slice(texture.as_bytes());

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

        let image_state = vulkan.resources.image(image_id).unwrap();

        let image_view = ImageView::new(
            image_state.image(),
            &ImageViewCreateInfo {
                view_type: image_view_type_to_vulkano(texture.dimensions()),
                ..ImageViewCreateInfo::from_image(image_state.image())
            },
        )
        .unwrap();

        (buffer_id, image_id, image_view)
    }

    fn ring(vulkan: &Vulkan, texture: &Texture, buffers: usize) -> GpuTextureInner {
        let staging_id = vulkan
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
                    texture.dimensions(),
                    texture.settings(),
                ) as DeviceSize)
                .unwrap(),
            )
            .unwrap();

        let format = format_to_vulkano(&texture.settings().format);

        let mut image_ids = Vec::with_capacity(buffers);
        let mut image_views = Vec::with_capacity(buffers);
        for _ in 0..buffers {
            let image_id = vulkan
                .resources
                .create_image(
                    &ImageCreateInfo {
                        image_type: image_type_to_vulkano(texture.dimensions()),
                        format,
                        extent: texture.dimensions().extent(),
                        array_layers: texture.dimensions().array_layers(),
                        usage: ImageUsage::TRANSFER_DST | ImageUsage::SAMPLED,
                        mip_levels: texture.settings().mip_levels,
                        ..Default::default()
                    },
                    &AllocationCreateInfo {
                        memory_type_filter: MemoryTypeFilter::PREFER_DEVICE,
                        ..Default::default()
                    },
                )
                .map_err(|e| GpuTextureError::Other(e.unwrap().into()))
                .unwrap();
            let image_state = vulkan.resources.image(image_id).unwrap();

            let image_view = ImageView::new(
                image_state.image(),
                &ImageViewCreateInfo {
                    view_type: image_view_type_to_vulkano(texture.dimensions()),
                    ..ImageViewCreateInfo::from_image(image_state.image())
                },
            )
            .unwrap();
            image_ids.push(image_id);
            image_views.push(image_view);
        }

        vulkan.wait_transfer();
        unsafe {
            vulkano_taskgraph::execute(
                vulkan.queues.transfer(),
                &vulkan.resources,
                vulkan.transfer_flight,
                |cb, ctx| {
                    let write: &mut [u8] = ctx.write_buffer(staging_id, ..)?;

                    write.copy_from_slice(texture.as_bytes());

                    for image_id in image_ids.iter() {
                        cb.copy_buffer_to_image(
                            &vulkano_taskgraph::command_buffer::CopyBufferToImageInfo {
                                src_buffer: staging_id,
                                dst_image: *image_id,
                                dst_image_layout: ImageLayoutType::Optimal,
                                ..Default::default()
                            },
                        )?;
                    }

                    Ok(())
                },
                [(staging_id, HostAccessType::Write)],
                [(staging_id, AccessTypes::COPY_TRANSFER_READ)],
                image_ids.iter().map(|id| {
                    (
                        *id,
                        AccessTypes::COPY_TRANSFER_WRITE,
                        ImageLayoutType::Optimal,
                    )
                }),
            )
        }
        .unwrap();

        GpuTextureInner::RingBuffer {
            image_ids: image_ids.into_boxed_slice(),
            image_views: image_views.into_boxed_slice(),
            staging_id,
            turn: 0.into(),
        }
    }

    pub(crate) fn vk_sampler(&self) -> vulkano::image::sampler::SamplerCreateInfo<'_> {
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

    /// Reads the texture from the GPU.
    ///
    /// Only possible for staged textures.
    fn data<F: FnOnce(&[u8])>(&self, f: F) -> Result<(), Self::Error> {
        let GpuTextureInner::Staged {
            image_id,
            staging_id,
            ..
        } = &self.inner
        else {
            return Err(GpuTextureError::UnsupportedAccess(
                self.settings.access_pattern,
            ));
        };

        let vulkan = VK.get().unwrap();

        let queue = vulkan.queues.transfer();

        // Task 1: image -> staging
        vulkan.wait_transfer();
        unsafe {
            vulkano_taskgraph::execute(
                queue,
                &vulkan.resources,
                vulkan.transfer_flight,
                |cb, _| {
                    cb.copy_image_to_buffer(&CopyImageToBufferInfo {
                        src_image: *image_id,
                        dst_buffer: *staging_id,
                        ..Default::default()
                    })?;
                    Ok(())
                },
                [],
                [(*staging_id, AccessTypes::COPY_TRANSFER_WRITE)],
                [(
                    *image_id,
                    AccessTypes::COPY_TRANSFER_READ,
                    ImageLayoutType::Optimal,
                )],
            )
        }
        .unwrap();

        // Task 2: read staging
        vulkan.wait_transfer();
        unsafe {
            vulkano_taskgraph::execute(
                queue,
                &vulkan.resources,
                vulkan.transfer_flight,
                |_, ctx| {
                    let read: &[u8] = ctx.read_buffer(*staging_id, ..)?;

                    f(read);

                    Ok(())
                },
                [(*staging_id, HostAccessType::Read)],
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
        let vulkan = VK.get().unwrap();
        let queue = vulkan.queues.transfer();

        match &self.inner {
            GpuTextureInner::Staged {
                image_id,
                staging_id,
                ..
            } => {
                vulkan.wait_transfer();
                vulkan.graphics_flight().unwrap().wait_idle().unwrap();
                unsafe {
                    vulkano_taskgraph::execute(
                        queue,
                        &vulkan.resources,
                        vulkan.transfer_flight,
                        |cb, ctx| {
                            let write: &mut [u8] = ctx.write_buffer(*staging_id, ..)?;
                            f(write);

                            cb.copy_buffer_to_image(&CopyBufferToImageInfo {
                                src_buffer: *staging_id,
                                dst_image: *image_id,
                                ..Default::default()
                            })?;

                            Ok(())
                        },
                        [(*staging_id, HostAccessType::Write)],
                        [
                            (*staging_id, AccessTypes::COPY_TRANSFER_WRITE),
                            (*staging_id, AccessTypes::COPY_TRANSFER_READ),
                        ],
                        [(
                            *image_id,
                            AccessTypes::COPY_TRANSFER_WRITE,
                            ImageLayoutType::Optimal,
                        )],
                    )
                }
                .unwrap();
            }
            GpuTextureInner::RingBuffer {
                image_ids,
                staging_id,
                turn,
                ..
            } => {
                let index = (turn.load(Relaxed) + 1) % image_ids.len();
                let image_id = image_ids[index];

                vulkan.wait_transfer();

                unsafe {
                    vulkano_taskgraph::execute(
                        queue,
                        &vulkan.resources,
                        vulkan.transfer_flight,
                        |cb, ctx| {
                            let write: &mut [u8] = ctx.write_buffer(*staging_id, ..)?;
                            f(write);

                            cb.copy_buffer_to_image(&CopyBufferToImageInfo {
                                src_buffer: *staging_id,
                                dst_image: image_id,
                                ..Default::default()
                            })?;

                            Ok(())
                        },
                        [(*staging_id, HostAccessType::Write)],
                        [
                            (*staging_id, AccessTypes::COPY_TRANSFER_WRITE),
                            (*staging_id, AccessTypes::COPY_TRANSFER_READ),
                        ],
                        [(
                            image_id,
                            AccessTypes::COPY_TRANSFER_WRITE,
                            ImageLayoutType::Optimal,
                        )],
                    )
                }
                .unwrap();

                turn.store(index, Relaxed);
            }
            _ => {
                return Err(GpuTextureError::UnsupportedAccess(
                    self.settings.access_pattern,
                ));
            }
        }

        vulkan.wait_transfer();

        Ok(())
    }
}

// Texture based errors.

use thiserror::Error;

use crate::backend::gpu::vulkan::{ResourceAccess, VIRTUAL_TAG_BIT};

use super::{
    VulkanError, format_to_vulkano,
    vulkan::{VK, Vulkan},
};

/// Errors that occur from the GPU texture.
#[derive(Error, Debug)]
pub enum GpuTextureError {
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
    #[error(transparent)]
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

fn image_type_to_vulkano(dimensions: &ViewTypeDim) -> ImageType {
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
