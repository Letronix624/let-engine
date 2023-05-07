use super::{objects::data::Vertex as GameVertex, Texture, Vulkan};
use crate::error::textures::*;
use derive_builder::Builder;
use std::sync::Arc;

use vulkano::descriptor_set::{
    allocator::StandardDescriptorSetAllocator, PersistentDescriptorSet, WriteDescriptorSet,
};
use vulkano::pipeline::{
    graphics::{
        color_blend::ColorBlendState,
        input_assembly::{InputAssemblyState, PrimitiveTopology},
        rasterization::RasterizationState,
        vertex_input::Vertex,
        viewport::ViewportState,
    },
    GraphicsPipeline, Pipeline, StateMode,
};
use vulkano::render_pass::Subpass;
use vulkano::shader::ShaderModule;

#[derive(Clone, Copy)]
pub enum Topology {
    TriangleList,
    TriangleStrip,
    LineList,
    LineStrip,
    PointList,
}

#[derive(Clone, PartialEq)]
pub struct Material {
    pub pipeline: Arc<GraphicsPipeline>,
    pub descriptor: Option<Arc<PersistentDescriptorSet>>,
    pub texture: Option<Arc<Texture>>,
    pub layer: u32,
}

impl std::fmt::Debug for Material {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Material")
            .field("texture", &self.texture)
            .field("layer", &self.layer)
            .finish()
    }
}

impl Material {
    pub fn new(
        settings: MaterialSettings,
        descriptor: Vec<WriteDescriptorSet>,
        vulkan: &Vulkan,
        subpass: Subpass,
        allocator: &StandardDescriptorSetAllocator,
    ) -> Self {
        let vs = &settings.shaders.vertex;
        let fs = &settings.shaders.fragment;

        let topology: PrimitiveTopology = match settings.topology {
            Topology::TriangleList => PrimitiveTopology::TriangleList,
            Topology::TriangleStrip => PrimitiveTopology::TriangleStrip,
            Topology::LineList => PrimitiveTopology::LineList,
            Topology::LineStrip => PrimitiveTopology::LineStrip,
            Topology::PointList => PrimitiveTopology::PointList,
        };

        let pipeline: Arc<GraphicsPipeline> = GraphicsPipeline::start()
            .vertex_input_state(GameVertex::per_vertex())
            .input_assembly_state(InputAssemblyState::new().topology(topology))
            .rasterization_state(RasterizationState {
                line_width: StateMode::Fixed(settings.line_width),
                ..RasterizationState::new()
            })
            .vertex_shader(vs.entry_point("main").unwrap(), ())
            .viewport_state(ViewportState::viewport_dynamic_scissor_irrelevant())
            .fragment_shader(fs.entry_point("main").unwrap(), ())
            .color_blend_state(ColorBlendState::new(subpass.num_color_attachments()).blend_alpha())
            .render_pass(subpass)
            .build(vulkan.device.clone())
            .unwrap();
        let descriptor = if descriptor.len() != 0 {
            Some(
                PersistentDescriptorSet::new(
                    allocator,
                    pipeline
                        .layout()
                        .set_layouts()
                        .get(2) // on set 2
                        .unwrap()
                        .clone(),
                    descriptor,
                )
                .unwrap(),
            )
        } else {
            None
        };
        Self {
            pipeline,
            descriptor,
            layer: settings.initial_layer,
            texture: settings.texture,
        }
    }
    pub fn write(
        &mut self,
        descriptor: Vec<WriteDescriptorSet>,
        allocator: &StandardDescriptorSetAllocator,
    ) {
        self.descriptor = Some(
            PersistentDescriptorSet::new(
                allocator,
                self.pipeline.layout().set_layouts().get(1).unwrap().clone(),
                descriptor,
            )
            .unwrap(),
        );
    }
    pub fn pipeline(&self) -> &Arc<GraphicsPipeline> {
        &self.pipeline
    }
    pub fn layer(&mut self, id: u32) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(texture) = &self.texture {
            if id > texture.layers - 1 {
                return Err(Box::new(TextureIDError));
            }
        } else {
            return Err(Box::new(NoTextureError));
        }
        self.layer = id;
        Ok(())
    }
    pub fn get_layer(&self) -> u32 {
        self.layer
    }
    pub fn next_frame(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(texture) = &self.texture {
            if texture.layers <= self.layer + 1 {
                return Err(Box::new(TextureIDError));
            }
        } else {
            return Err(Box::new(NoTextureError));
        }
        self.layer += 1;
        Ok(())
    }
    pub fn last_frame(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(_) = &self.texture {
            if self.layer == 0 {
                return Err(Box::new(TextureIDError));
            }
        } else {
            return Err(Box::new(NoTextureError));
        }
        self.layer -= 1;
        Ok(())
    }
}

/// Vertex and fragment shaders of a material
/// as well as the topology and line width, if the topology is set to LineList or LineStrip.
#[derive(Builder)]
pub struct MaterialSettings {
    #[builder(setter(into))]
    pub shaders: Shaders,
    #[builder(setter(into), default = "Topology::TriangleList")]
    pub topology: Topology,
    #[builder(setter(into), default = "1.0")]
    pub line_width: f32,
    #[builder(setter(into), default = "None")]
    pub texture: Option<Arc<Texture>>,
    #[builder(setter(into), default = "0")]
    pub initial_layer: u32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Shaders {
    pub vertex: Arc<ShaderModule>,
    pub fragment: Arc<ShaderModule>,
}

impl Shaders {
    pub unsafe fn from_bytes(vertex_bytes: &[u8], fragment_bytes: &[u8], vulkan: &Vulkan) -> Self {
        let vertex: Arc<ShaderModule> =
            unsafe { ShaderModule::from_bytes(vulkan.device.clone(), vertex_bytes).unwrap() };
        let fragment: Arc<ShaderModule> =
            unsafe { ShaderModule::from_bytes(vulkan.device.clone(), fragment_bytes).unwrap() };
        Self { vertex, fragment }
    }
}
