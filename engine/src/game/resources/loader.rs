use super::{materials, Vulkan};
use std::sync::Arc;
use vulkano::{
    buffer::{allocator::*, BufferContents, BufferUsage},
    command_buffer::{
        allocator::StandardCommandBufferAllocator, AutoCommandBufferBuilder, CommandBufferUsage,
        PrimaryCommandBufferAbstract,
    },
    descriptor_set::{
        allocator::StandardDescriptorSetAllocator, PersistentDescriptorSet, WriteDescriptorSet,
    },
    format::Format,
    image::{
        view::{ImageView, ImageViewCreateInfo},
        ImageDimensions, ImageViewType, ImmutableImage, MipmapsCount,
    },
    memory::allocator::StandardMemoryAllocator,
    pipeline::{cache::PipelineCache, Pipeline},
    render_pass::Subpass,
    sampler::Sampler,
};

use super::textures::{Format as tFormat, TextureSettings};

/// Loads thing to the gpu.
pub(crate) struct Loader {
    pub memory_allocator: Arc<StandardMemoryAllocator>,
    pub vertex_buffer_allocator: SubbufferAllocator,
    pub index_buffer_allocator: SubbufferAllocator,
    pub object_buffer_allocator: SubbufferAllocator,
    pub descriptor_set_allocator: StandardDescriptorSetAllocator,
    pub command_buffer_allocator: StandardCommandBufferAllocator,
    pub pipeline_cache: Arc<PipelineCache>,
}

impl Loader {
    /// Initializes the loader
    pub fn init(vulkan: &Vulkan) -> Self {
        let memory_allocator =
            Arc::new(StandardMemoryAllocator::new_default(vulkan.device.clone()));

        let vertex_buffer_allocator: SubbufferAllocator = SubbufferAllocator::new(
            memory_allocator.clone(),
            SubbufferAllocatorCreateInfo {
                buffer_usage: BufferUsage::VERTEX_BUFFER,
                ..Default::default()
            },
        );

        let index_buffer_allocator: SubbufferAllocator = SubbufferAllocator::new(
            memory_allocator.clone(),
            SubbufferAllocatorCreateInfo {
                buffer_usage: BufferUsage::INDEX_BUFFER,
                ..Default::default()
            },
        );

        let object_buffer_allocator: SubbufferAllocator = SubbufferAllocator::new(
            memory_allocator.clone(),
            SubbufferAllocatorCreateInfo {
                buffer_usage: BufferUsage::UNIFORM_BUFFER,
                ..Default::default()
            },
        );

        let descriptor_set_allocator = StandardDescriptorSetAllocator::new(vulkan.device.clone());

        let command_buffer_allocator =
            StandardCommandBufferAllocator::new(vulkan.device.clone(), Default::default());

        let pipeline_cache = PipelineCache::empty(vulkan.device.clone()).unwrap();

        Self {
            memory_allocator,
            vertex_buffer_allocator,
            index_buffer_allocator,
            object_buffer_allocator,
            descriptor_set_allocator,
            command_buffer_allocator,
            pipeline_cache,
        }
    }

    /// Loads a material to the gpu.
    pub fn load_material(
        &self,
        vulkan: &Vulkan,
        shaders: &materials::Shaders,
        settings: materials::MaterialSettings,
        descriptor_bindings: Vec<WriteDescriptorSet>,
    ) -> materials::Material {
        let subpass = Subpass::from(vulkan.render_pass.clone(), 0).unwrap();
        materials::Material::new(
            settings,
            shaders,
            descriptor_bindings,
            vulkan,
            self.pipeline_cache.clone(),
            subpass,
            &self.descriptor_set_allocator,
        )
    }

    /// Loads a texture to the GPU.
    pub fn load_texture(
        &self,
        vulkan: &Vulkan,
        data: &[u8],
        dimensions: (u32, u32),
        layers: u32,
        format: tFormat,
        settings: TextureSettings,
    ) -> Arc<PersistentDescriptorSet> {
        let mut uploads = AutoCommandBufferBuilder::primary(
            &self.command_buffer_allocator,
            vulkan.queue.queue_family_index(),
            CommandBufferUsage::OneTimeSubmit,
        )
        .unwrap();

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

        let image = ImmutableImage::from_iter(
            &self.memory_allocator,
            data.to_vec(),
            ImageDimensions::Dim2d {
                width: dimensions.0,
                height: dimensions.1,
                array_layers: layers,
            },
            MipmapsCount::One,
            format,
            &mut uploads,
        )
        .unwrap();

        let set_layout;

        let texture_view = ImageView::new(
            image.clone(),
            ImageViewCreateInfo {
                view_type: if layers == 1 {
                    set_layout = vulkan
                        .textured_material
                        .pipeline
                        .layout()
                        .set_layouts()
                        .get(1)
                        .unwrap()
                        .clone();
                    ImageViewType::Dim2d
                } else {
                    set_layout = vulkan
                        .texture_array_material
                        .pipeline
                        .layout()
                        .set_layouts()
                        .get(1)
                        .unwrap()
                        .clone();
                    ImageViewType::Dim2dArray
                },
                ..ImageViewCreateInfo::from_image(&image)
            },
        )
        .unwrap();

        let samplercreateinfo = settings.sampler.to_vulkano();

        let sampler = Sampler::new(vulkan.device.clone(), samplercreateinfo).unwrap();

        let set = PersistentDescriptorSet::new(
            &self.descriptor_set_allocator,
            set_layout,
            [WriteDescriptorSet::image_view_sampler(
                0,
                texture_view,
                sampler,
            )],
        )
        .unwrap();

        let _ = uploads
            .build()
            .unwrap()
            .execute(vulkan.queue.clone())
            .unwrap();
        set
    }
    /// Makes a descriptor write.
    pub fn write_descriptor<T: BufferContents>(
        &self,
        descriptor: T,
        set: u32,
    ) -> WriteDescriptorSet {
        let buf = self.object_buffer_allocator.allocate_sized().unwrap();
        *buf.write().unwrap() = descriptor;
        WriteDescriptorSet::buffer(set, buf)
    }
}
