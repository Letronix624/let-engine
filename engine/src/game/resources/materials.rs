//! Material related settings that determine the way the scene gets rendered.

use crate::{
    error::textures::*,
    resources::{Texture, Vulkan},
    Vertex as GameVertex,
};
use derive_builder::Builder;
use std::sync::Arc;

use vulkano::descriptor_set::{
    allocator::StandardDescriptorSetAllocator, PersistentDescriptorSet, WriteDescriptorSet,
};
use vulkano::pipeline::{
    cache::PipelineCache,
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

/// The way in which an object gets drawn using it's vertices and indices.
#[derive(Debug, Clone, Copy)]
pub enum Topology {
    /// Creates triangles using every 3 vertices for one triangle.
    TriangleList,
    /// Creates triangles using 3 vertices for the first triangle and every next triangle using the next vertex and the 2 vertices before that.
    TriangleStrip,
    /// Creates a line using every 2 vertices.
    LineList,
    /// Creates a line using the vertices as guiding points where to go next.
    LineStrip,
    /// Creates a pixel for every vertex.
    PointList,
}

/// A material holding the way an object should be drawn.
///
/// Takes some time.
#[derive(Clone, PartialEq)]
pub struct Material {
    pub(crate) pipeline: Arc<GraphicsPipeline>,
    pub(crate) descriptor: Option<Arc<PersistentDescriptorSet>>,
    pub(crate) texture: Option<Texture>,
    pub(crate) layer: u32,
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
    /// Makes a new material.
    pub(crate) fn new(
        settings: MaterialSettings,
        shaders: &Shaders,
        descriptor: Vec<WriteDescriptorSet>,
        vulkan: &Vulkan,
        pipeline_cache: Arc<PipelineCache>,
        subpass: Subpass,
        allocator: &StandardDescriptorSetAllocator,
    ) -> Self {
        let vs = &shaders.vertex;
        let fs = &shaders.fragment;

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
            .build_with_cache(pipeline_cache)
            .build(vulkan.device.clone())
            .unwrap();
        let descriptor = if !descriptor.is_empty() {
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

    /// Writes to the material changing the variables for the shaders.
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

    /// Sets the layer of the texture in case it has a texture with layers.
    pub fn set_layer(&mut self, id: u32) -> Result<(), Box<dyn std::error::Error>> {
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

    /// Goes to the next frame of the layer.
    ///
    /// Returns an error if it reached the limit.
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

    /// Goes to the last frame of the layer.
    ///
    /// Returns an error if the layer is 0.
    pub fn last_frame(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        if self.texture.is_some() {
            if self.layer == 0 {
                return Err(Box::new(TextureIDError));
            }
        } else {
            return Err(Box::new(NoTextureError));
        }
        self.layer -= 1;
        Ok(())
    }

    /// Returns the texture.
    pub fn get_texture(&self) -> Option<Texture> {
        self.texture.clone()
    }

    /// Sets the texture.
    pub fn set_texture(&mut self, texture: Option<Texture>) {
        self.texture = texture;
    }
}

/// Vertex and fragment shaders of a material
/// as well as the topology and line width, if the topology is set to LineList or LineStrip.
#[derive(Builder, Clone, Debug)]
pub struct MaterialSettings {
    #[builder(setter(into), default = "Topology::TriangleList")]
    pub topology: Topology,
    #[builder(setter(into), default = "1.0")]
    pub line_width: f32,
    #[builder(setter(into), default = "None")]
    pub texture: Option<Texture>,
    #[builder(setter(into), default = "0")]
    pub initial_layer: u32,
}

/// Holds compiled shaders in form of ShaderModules to use in a material.
#[derive(Clone, Debug, PartialEq)]
pub struct Shaders {
    pub(crate) vertex: Arc<ShaderModule>,
    pub(crate) fragment: Arc<ShaderModule>,
}

impl Shaders {
    /// Creates a shader from SpirV bytes.
    ///
    /// # Safety
    ///
    /// When loading those shaders the engine doesn't know if they are right.
    /// So when they are wrong I'm not sure what will happen. Make it right!
    pub(crate) unsafe fn from_bytes(
        vertex_bytes: &[u8],
        fragment_bytes: &[u8],
        vulkan: &Vulkan,
    ) -> Self {
        let vertex: Arc<ShaderModule> =
            unsafe { ShaderModule::from_bytes(vulkan.device.clone(), vertex_bytes).unwrap() };
        let fragment: Arc<ShaderModule> =
            unsafe { ShaderModule::from_bytes(vulkan.device.clone(), fragment_bytes).unwrap() };
        Self { vertex, fragment }
    }
}
