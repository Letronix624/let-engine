use super::Vulkan;
use anyhow::{Error, Result};
use std::sync::Arc;
use vulkano::{
    buffer::{allocator::*, Buffer, BufferContents, BufferCreateInfo, BufferUsage, Subbuffer},
    command_buffer::{
        allocator::{StandardCommandBufferAllocator, StandardCommandBufferAllocatorCreateInfo},
        AutoCommandBufferBuilder, CommandBufferUsage, CopyBufferToImageInfo,
    },
    descriptor_set::{
        allocator::{StandardDescriptorSetAllocator, StandardDescriptorSetAllocatorCreateInfo},
        DescriptorSet, WriteDescriptorSet,
    },
    format::Format,
    image::{
        sampler::Sampler,
        view::{ImageView, ImageViewCreateInfo, ImageViewType},
        Image, ImageCreateInfo, ImageType, ImageUsage,
    },
    memory::allocator::{AllocationCreateInfo, MemoryTypeFilter, StandardMemoryAllocator},
    pipeline::{
        cache::{PipelineCache, PipelineCacheCreateInfo},
        GraphicsPipeline, Pipeline,
    },
    DeviceSize,
};

use super::textures::{Format as tFormat, TextureSettings};

/// Loads thing to the gpu.
pub struct Loader {
    pub memory_allocator: Arc<StandardMemoryAllocator>,
    pub vertex_buffer_allocator: SubbufferAllocator,
    pub index_buffer_allocator: SubbufferAllocator,
    pub object_buffer_allocator: SubbufferAllocator,
    pub instance_buffer_allocator: SubbufferAllocator,
    pub descriptor_set_allocator: Arc<StandardDescriptorSetAllocator>,
    pub command_buffer_allocator: Arc<StandardCommandBufferAllocator>,
    pub pipeline_cache: Arc<PipelineCache>,
    pub pipelines: Vec<Arc<GraphicsPipeline>>,
}

impl Loader {
    /// Initializes the loader
    pub fn init(vulkan: &Vulkan, pipelines: Vec<Arc<GraphicsPipeline>>) -> Result<Self> {
        let memory_allocator =
            Arc::new(StandardMemoryAllocator::new_default(vulkan.device.clone()));

        let vertex_buffer_allocator: SubbufferAllocator = SubbufferAllocator::new(
            memory_allocator.clone(),
            SubbufferAllocatorCreateInfo {
                buffer_usage: BufferUsage::VERTEX_BUFFER,
                memory_type_filter: MemoryTypeFilter::HOST_SEQUENTIAL_WRITE
                    | MemoryTypeFilter::PREFER_DEVICE,
                ..Default::default()
            },
        );

        let index_buffer_allocator: SubbufferAllocator = SubbufferAllocator::new(
            memory_allocator.clone(),
            SubbufferAllocatorCreateInfo {
                buffer_usage: BufferUsage::INDEX_BUFFER,
                memory_type_filter: MemoryTypeFilter::HOST_SEQUENTIAL_WRITE
                    | MemoryTypeFilter::PREFER_DEVICE,
                ..Default::default()
            },
        );

        let object_buffer_allocator: SubbufferAllocator = SubbufferAllocator::new(
            memory_allocator.clone(),
            SubbufferAllocatorCreateInfo {
                buffer_usage: BufferUsage::UNIFORM_BUFFER,
                memory_type_filter: MemoryTypeFilter::HOST_SEQUENTIAL_WRITE,
                ..Default::default()
            },
        );

        let instance_buffer_allocator: SubbufferAllocator = SubbufferAllocator::new(
            memory_allocator.clone(),
            SubbufferAllocatorCreateInfo {
                buffer_usage: BufferUsage::VERTEX_BUFFER,
                memory_type_filter: MemoryTypeFilter::HOST_SEQUENTIAL_WRITE,
                ..Default::default()
            },
        );

        let descriptor_set_allocator = StandardDescriptorSetAllocator::new(
            vulkan.device.clone(),
            StandardDescriptorSetAllocatorCreateInfo::default(),
        )
        .into();

        let command_buffer_allocator = StandardCommandBufferAllocator::new(
            vulkan.device.clone(),
            StandardCommandBufferAllocatorCreateInfo {
                secondary_buffer_count: 2,
                ..Default::default()
            },
        )
        .into();

        let pipeline_cache = unsafe {
            PipelineCache::new(vulkan.device.clone(), PipelineCacheCreateInfo::default())?
        };

        Ok(Self {
            memory_allocator,
            vertex_buffer_allocator,
            index_buffer_allocator,
            object_buffer_allocator,
            instance_buffer_allocator,
            descriptor_set_allocator,
            command_buffer_allocator,
            pipeline_cache,
            pipelines,
        })
    }

    /// Loads a texture to the GPU.
    pub fn load_texture(
        &mut self,
        vulkan: &Vulkan,
        data: Arc<[u8]>,
        dimensions: (u32, u32),
        layers: u32,
        format: tFormat,
        settings: TextureSettings,
    ) -> Result<Arc<DescriptorSet>> {
        if dimensions.0 * dimensions.1 * format as u32 > data.len() as u32 {
            return Err(Error::msg(
                "The size of the texture is smaller than the provided texture dimensions.",
            ));
        }

        let mut uploads = AutoCommandBufferBuilder::primary(
            self.command_buffer_allocator.clone(),
            vulkan.queue.queue_family_index(),
            CommandBufferUsage::OneTimeSubmit,
        )?;

        let format = if settings.srgb {
            match format {
                tFormat::R8 => Format::R8_SRGB,
                tFormat::RGBA8 => Format::R8G8B8A8_SRGB,
                tFormat::RGBA16 => Format::R16G16B16A16_UNORM,
            }
        } else {
            match format {
                tFormat::R8 => Format::R8_UNORM,
                tFormat::RGBA8 => Format::R8G8B8A8_UNORM,
                tFormat::RGBA16 => Format::R16G16B16A16_UNORM,
            }
        };

        let upload_buffer: Subbuffer<[u8]> = Buffer::new_slice(
            self.memory_allocator.clone(),
            BufferCreateInfo {
                usage: BufferUsage::TRANSFER_SRC,
                ..Default::default()
            },
            AllocationCreateInfo {
                memory_type_filter: MemoryTypeFilter::PREFER_HOST
                    | MemoryTypeFilter::HOST_SEQUENTIAL_WRITE,
                ..Default::default()
            },
            format.block_size()
                * [dimensions.0, dimensions.1, 1]
                    .into_iter()
                    .map(|e| e as DeviceSize)
                    .product::<DeviceSize>()
                * layers as DeviceSize,
        )?;
        upload_buffer.write()?.copy_from_slice(&data);

        let image = Image::new(
            self.memory_allocator.clone(),
            ImageCreateInfo {
                image_type: ImageType::Dim2d,
                format,
                extent: [dimensions.0, dimensions.1, 1],
                array_layers: layers,
                usage: ImageUsage::TRANSFER_DST | ImageUsage::SAMPLED,
                ..Default::default()
            },
            AllocationCreateInfo::default(),
        )?;

        uploads.copy_buffer_to_image(CopyBufferToImageInfo::buffer_image(
            upload_buffer,
            image.clone(),
        ))?;

        let set_layout;

        let texture_view = ImageView::new(
            image.clone(),
            ImageViewCreateInfo {
                view_type: if layers <= 1 {
                    set_layout = vulkan
                        .textured_material
                        .get_pipeline_or_recreate(self)?
                        .layout()
                        .set_layouts()
                        .get(1)
                        .ok_or(Error::msg(
                            "failed to get second set of the texture layout.",
                        ))?
                        .clone();
                    ImageViewType::Dim2d
                } else {
                    set_layout = vulkan
                        .texture_array_material
                        .get_pipeline_or_recreate(self)?
                        .layout()
                        .set_layouts()
                        .get(1)
                        .ok_or(Error::msg(
                            "failed to get second set of the texture array layout.",
                        ))?
                        .clone();
                    ImageViewType::Dim2dArray
                },
                ..ImageViewCreateInfo::from_image(&image)
            },
        )?;

        let samplercreateinfo = settings.sampler.to_vulkano();

        let sampler = Sampler::new(vulkan.device.clone(), samplercreateinfo)?;

        let set = DescriptorSet::new(
            self.descriptor_set_allocator.clone(),
            set_layout,
            [WriteDescriptorSet::image_view_sampler(
                0,
                texture_view,
                sampler,
            )],
            [],
        )?;

        // Upload to gpu.
        let _ = uploads.build()?.execute(vulkan.queue.clone())?;
        Ok(set)
    }
    /// Makes a descriptor write.
    pub fn write_descriptor<T: BufferContents>(
        &self,
        descriptor: T,
        set: u32,
    ) -> Result<WriteDescriptorSet> {
        let buf = self.object_buffer_allocator.allocate_sized()?;
        *buf.write()? = descriptor;
        Ok(WriteDescriptorSet::buffer(set, buf))
    }
}
