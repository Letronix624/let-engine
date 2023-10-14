//! Material related settings that determine the way the scene gets rendered.

use crate::error::textures::*;
use crate::prelude::{Format, Texture, TextureSettings, Vertex as GameVertex};

use derive_builder::Builder;
use image::ImageFormat;
use std::sync::Arc;

use vulkano::descriptor_set::{PersistentDescriptorSet, WriteDescriptorSet};
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

use super::Resources;

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
/// It takes some time to make a new material.
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
/// Making
impl Material {
    /// Creates a new material using the given shaders, settings and write operations.
    pub fn new_with_shaders(
        settings: MaterialSettings,
        shaders: &Shaders,
        descriptor: Vec<WriteDescriptorSet>,
        resources: &Resources,
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

        let loader = resources.loader().lock();
        let vulkan = resources.vulkan();
        let pipeline_cache = loader.pipeline_cache.clone();
        let subpass = Subpass::from(vulkan.render_pass.clone(), 0).unwrap();
        let allocator = &loader.descriptor_set_allocator;

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

    /// Makes a new default material.
    pub fn new(&self, settings: MaterialSettings, resources: &Resources) -> Material {
        let shaders = &resources.vulkan().default_shaders;
        Self::new_with_shaders(settings, shaders, vec![], resources)
    }

    /// Simplification of making a texture and putting it into a material.
    pub fn new_from_texture(
        texture: &[u8],
        format: ImageFormat,
        layers: u32,
        settings: TextureSettings,
        resources: &Resources,
    ) -> Result<Material, InvalidFormatError> {
        let texture = Texture::from_bytes(texture, format, layers, settings, resources)?;

        Ok(Self::new_default_textured(&texture, resources))
    }

    /// Simplification of making a texture from raw and putting it into a material.
    pub fn new_from_raw_texture(
        texture: Vec<u8>,
        format: Format,
        dimensions: (u32, u32),
        layers: u32,
        settings: TextureSettings,
        resources: &Resources,
    ) -> Material {
        let texture = Texture::from_raw(&texture, dimensions, format, layers, settings, resources);
        Self::new_default_textured(&texture, resources)
    }

    /// Creates a simple material made just for showing a texture.
    pub fn new_default_textured(texture: &Texture, resources: &Resources) -> Material {
        let default = if texture.layers() == 1 {
            resources.vulkan().textured_material.clone()
        } else {
            resources.vulkan().texture_array_material.clone()
        };
        Material {
            texture: Some(texture.clone()),
            ..default
        }
    }
}
impl Material {
    /// Writes to the material changing the variables for the shaders.
    pub fn write(
        &mut self,
        descriptor: Vec<WriteDescriptorSet>,
        resources: &Resources,
        // allocator: &StandardDescriptorSetAllocator,
    ) {
        let loader = resources.loader().lock();
        self.descriptor = Some(
            PersistentDescriptorSet::new(
                &loader.descriptor_set_allocator,
                self.pipeline.layout().set_layouts().get(1).unwrap().clone(),
                descriptor,
            )
            .unwrap(),
        );
    }

    /// Sets the layer of the texture in case it has a texture with layers.
    pub fn set_layer(&mut self, id: u32) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(texture) = &self.texture {
            if id > texture.layers() - 1 {
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
            if texture.layers() <= self.layer + 1 {
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
    /// The usage way of the vertices and indices given in the model.
    #[builder(setter(into), default = "Topology::TriangleList")]
    pub topology: Topology,
    /// The width of the line in case the topology was set to something with lines.
    #[builder(setter(into), default = "1.0")]
    pub line_width: f32,
    /// The optional texture of the material.
    #[builder(setter(into), default = "None")]
    pub texture: Option<Texture>,
    /// If the texture has multiple layers this is the layer it starts at.
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
    pub unsafe fn from_bytes(
        vertex_bytes: &[u8],
        fragment_bytes: &[u8],
        resources: &Resources,
    ) -> Self {
        let device = &resources.vulkan().device;
        let vertex: Arc<ShaderModule> =
            unsafe { ShaderModule::from_bytes(device.clone(), vertex_bytes).unwrap() };
        let fragment: Arc<ShaderModule> =
            unsafe { ShaderModule::from_bytes(device.clone(), fragment_bytes).unwrap() };
        Self { vertex, fragment }
    }
}
