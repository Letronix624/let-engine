//! Material related settings that determine the way the scene gets rendered.

use crate::{
    error::{
        draw::{ShaderError, VulkanError},
        textures::*,
    },
    prelude::{InstanceData, Texture, Vertex as GameVertex},
};

use anyhow::{anyhow, Error, Result};
use derive_builder::Builder;
use parking_lot::Mutex;
use std::sync::{Arc, Weak};

use vulkano::{
    descriptor_set::{DescriptorSet, WriteDescriptorSet},
    pipeline::{
        graphics::{
            input_assembly::{InputAssemblyState, PrimitiveTopology},
            rasterization::RasterizationState,
            vertex_input::{Vertex, VertexDefinition},
        },
        GraphicsPipeline, Pipeline,
    },
    render_pass::Subpass,
    shader::{spirv::bytes_to_words, ShaderModule, ShaderModuleCreateInfo},
};

use super::{vulkan::pipeline::create_pipeline, Loader, RESOURCES};
// pub use vulkano::pipeline::graphics::rasterization::LineStipple;

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

impl From<Topology> for PrimitiveTopology {
    fn from(value: Topology) -> Self {
        match value {
            Topology::TriangleList => PrimitiveTopology::TriangleList,
            Topology::TriangleStrip => PrimitiveTopology::TriangleStrip,
            Topology::LineList => PrimitiveTopology::LineList,
            Topology::LineStrip => PrimitiveTopology::LineStrip,
            Topology::PointList => PrimitiveTopology::PointList,
        }
    }
}

/// A material holding the way an object should be drawn.
///
/// It takes some time to make a new material.
#[derive(Clone)]
pub struct Material {
    pub(crate) pipeline: Arc<Mutex<Weak<GraphicsPipeline>>>,
    instanced: bool,
    pub(crate) descriptor: Option<Arc<DescriptorSet>>,
    texture: Option<Texture>,
    layer: u32,
    settings: MaterialSettings,
    shaders: Shaders,
}

impl PartialEq for Material {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.pipeline, &other.pipeline) && self.descriptor == other.descriptor
    }
}

impl std::fmt::Debug for Material {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Material")
            .field("instanced", &self.instanced)
            .field("texture", &self.texture)
            .field("layer", &self.layer)
            .finish()
    }
}
/// Making
///
/// Right now it produces an error when the shaders do not have a main function.
impl Material {
    pub(crate) fn from_pipeline(
        pipeline: &Arc<GraphicsPipeline>,
        instanced: bool,
        shaders: Shaders,
    ) -> Self {
        Self {
            pipeline: Arc::new(Mutex::new(Arc::downgrade(pipeline))),
            instanced,
            descriptor: None,
            texture: None,
            layer: 0,
            settings: MaterialSettings::default(),
            shaders,
        }
    }
    /// Creates a new material using the given shaders, settings and write operations.
    pub fn new_with_shaders(
        settings: MaterialSettings,
        texture: Option<Texture>,
        shaders: &Shaders,
        instanced: bool,
        writes: Vec<WriteDescriptorSet>,
    ) -> Result<Self, VulkanError> {
        let vs = &shaders.vertex;
        let fs = &shaders.fragment;
        let vertex = vs
            .entry_point(&shaders.entry_point)
            .ok_or(VulkanError::ShaderError)?;
        let fragment = fs
            .entry_point(&shaders.entry_point)
            .ok_or(VulkanError::ShaderError)?;

        let topology: PrimitiveTopology = settings.topology.into();

        // let line_stipple = settings.line_stripple.map(StateMode::Fixed);

        let resources = &RESOURCES;
        let mut loader = resources.loader().lock();
        let vulkan = resources.vulkan();
        let pipeline_cache = loader.pipeline_cache.clone();
        let subpass = Subpass::from(vulkan.render_pass.clone(), 0)
            .ok_or(VulkanError::Other(Error::msg("Failed to make subpass.")))?;

        let input_assembly = InputAssemblyState {
            topology,
            ..Default::default()
        };

        let vertex_input_state = if instanced {
            [GameVertex::per_vertex(), InstanceData::per_instance()]
                .definition(&vertex.info().input_interface)
        } else {
            [GameVertex::per_vertex()].definition(&vertex.info().input_interface)
        }
        .map_err(|e| VulkanError::Other(e.into()))?;

        let rasterisation_state = RasterizationState {
            line_width: settings.line_width,
            ..RasterizationState::default()
        };

        let pipeline = create_pipeline(
            &vulkan.device,
            vertex,
            fragment,
            input_assembly,
            subpass,
            vertex_input_state,
            rasterisation_state,
            Some(pipeline_cache),
        )
        .map_err(VulkanError::Other)?;

        loader.pipelines.push(pipeline.clone());

        let descriptor = if !writes.is_empty() {
            Some(DescriptorSet::new(
                loader.descriptor_set_allocator.clone(),
                pipeline
                    .layout()
                    .set_layouts()
                    .get(2) // on set 2
                    .ok_or(VulkanError::Other(Error::msg(
                        "Failed to get the second set of the pipeline layout.",
                    )))?
                    .clone(),
                writes.clone(),
                [],
            )?)
        } else {
            None
        };
        Ok(Self {
            pipeline: Arc::new(Mutex::new(Arc::downgrade(&pipeline))),
            descriptor,
            instanced,
            layer: settings.initial_layer,
            texture,
            settings,
            shaders: shaders.clone(),
        })
    }

    /// Makes a new default material.
    pub fn new(
        settings: MaterialSettings,
        texture: Option<Texture>,
    ) -> Result<Material, VulkanError> {
        let resources = &RESOURCES;
        let shaders = resources.vulkan().clone().default_shaders;
        Self::new_with_shaders(settings, texture, &shaders, false, vec![])
    }

    /// Makes a new default material.
    pub fn new_instanced(
        settings: MaterialSettings,
        texture: Option<Texture>,
    ) -> Result<Material, VulkanError> {
        let resources = &RESOURCES;
        let shaders = resources.vulkan().clone().default_instance_shaders;
        Self::new_with_shaders(settings, texture, &shaders, true, vec![])
    }

    /// Creates a simple material made just for showing a texture.
    pub fn new_default_textured(texture: &Texture) -> Material {
        let resources = &RESOURCES;
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

    /// Creates a default material instance with a texture.
    pub fn new_default_textured_instance(texture: &Texture) -> Material {
        let resources = &RESOURCES;
        let default = if texture.layers() == 1 {
            resources.vulkan().textured_instance_material.clone()
        } else {
            resources.vulkan().texture_array_instance_material.clone()
        };
        Material {
            texture: Some(texture.clone()),
            ..default
        }
    }

    /// Returns the graphics pipeline, but in case it is out of date reloads it from the beginning.
    ///
    /// ## How the system works
    ///
    /// A material contains a graphics pipeline that because this game engine is not working with a dynamic viewport goes
    /// out of date as soon as the window size has changed.
    ///
    /// Every material only has a weak pointer to the pipeline they are working with. That means there is also a strong pointer
    /// somewhere. The loader struct contains a vec of arcs with all the graphics pipelines that get cleared on window resize
    /// making the weak pointer invalid and return a `None`. This function returns the Some if the weak pointer or remakes the
    /// whole pipeline returning it instead.
    pub(crate) fn get_pipeline_or_recreate(
        &self,
        loader: &mut Loader,
    ) -> Result<Arc<GraphicsPipeline>> {
        if let Some(pipeline) = self.pipeline.lock().upgrade() {
            return Ok(pipeline);
        }
        let vulkan = RESOURCES.vulkan();
        let vertex = self
            .shaders
            .vertex
            .entry_point(&self.shaders.entry_point)
            .ok_or(anyhow!("Entry point changed during runtime."))?;
        let fragment = self
            .shaders
            .fragment
            .entry_point(&self.shaders.entry_point)
            .ok_or(anyhow!("Entry point changed during runtime."))?;

        let subpass = Subpass::from(vulkan.render_pass.clone(), 0)
            .ok_or(anyhow!("Failed to create subpass from the render pass."))?;

        let input_assembly = InputAssemblyState {
            topology: self.settings.topology.into(),
            ..Default::default()
        };

        let vertex_input_state = if self.instanced {
            [GameVertex::per_vertex(), InstanceData::per_instance()]
                .definition(&vertex.info().input_interface)
        } else {
            [GameVertex::per_vertex()].definition(&vertex.info().input_interface)
        }?;

        let rasterisation_state = RasterizationState {
            line_width: self.settings.line_width,
            ..RasterizationState::default()
        };

        let pipeline = create_pipeline(
            &vulkan.device,
            vertex,
            fragment,
            input_assembly,
            subpass,
            vertex_input_state,
            rasterisation_state,
            Some(loader.pipeline_cache.clone()),
        )?;

        loader.pipelines.push(pipeline.clone());
        *self.pipeline.lock() = Arc::downgrade(&pipeline);
        Ok(pipeline)
    }
}
impl Material {
    /// Writes to the material changing the variables for the shaders.
    ///
    /// # Safety
    /// The program will crash in case in case the data input here is not as the shader wants it.
    pub unsafe fn write(&mut self, descriptor: Vec<WriteDescriptorSet>) -> Result<()> {
        let resources = &RESOURCES;
        let mut loader = resources.loader().lock();
        self.descriptor = Some(DescriptorSet::new(
            loader.descriptor_set_allocator.clone(),
            self.get_pipeline_or_recreate(&mut loader)?
                .layout()
                .set_layouts()
                .get(1)
                .ok_or(Error::msg(
                    "Could not obtain the second set layout of this write.",
                ))?
                .clone(),
            descriptor,
            [],
        )?);
        Ok(())
    }

    /// Sets the layer of the texture in case it has a texture with layers.
    pub fn set_layer(&mut self, id: u32) -> Result<(), TextureError> {
        if let Some(texture) = &self.texture {
            if id > texture.layers() - 1 {
                return Err(TextureError::Layer(format!(
                    "Given: {}, Highest: {}",
                    id,
                    texture.layers() - 1
                )));
            }
        } else {
            return Err(TextureError::NoTexture);
        }
        self.layer = id;
        Ok(())
    }

    /// Returns the layer of the texture in case the material is textured.
    pub fn layer(&self) -> u32 {
        self.layer
    }

    /// Goes to the next frame of the texture.
    ///
    /// Returns an error if it reached the limit.
    pub fn next_frame(&mut self) -> Result<(), TextureError> {
        if let Some(texture) = &self.texture {
            if texture.layers() <= self.layer + 1 {
                return Err(TextureError::Layer(
                    "You are already at the last frame.".to_string(),
                ));
            }
        } else {
            return Err(TextureError::NoTexture);
        }
        self.layer += 1;
        Ok(())
    }

    /// Goes back a frame of the texture.
    ///
    /// Returns an error if the layer is already on 0.
    pub fn last_frame(&mut self) -> Result<(), TextureError> {
        if self.texture.is_some() {
            if self.layer == 0 {
                return Err(TextureError::Layer(
                    "You are already on the first frame".to_string(),
                ));
            }
        } else {
            return Err(TextureError::NoTexture);
        }
        self.layer -= 1;
        Ok(())
    }

    /// Returns the texture.
    pub fn texture(&self) -> Option<Texture> {
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
    /// If the texture has multiple layers this is the layer it starts at.
    #[builder(setter(into), default = "0")]
    pub initial_layer: u32,
}

impl Default for MaterialSettings {
    fn default() -> Self {
        Self {
            topology: Topology::TriangleList,
            line_width: 1.0,
            initial_layer: 0,
        }
    }
}

/// Holds compiled shaders in form of ShaderModules to use in a material.
#[derive(Clone, Debug, PartialEq)]
pub struct Shaders {
    pub(crate) vertex: Arc<ShaderModule>,
    pub(crate) fragment: Arc<ShaderModule>,
    entry_point: Box<str>,
}

impl Shaders {
    /// Creates a shader from SpirV bytes.
    ///
    /// # Safety
    ///
    /// When loading those shaders the engine does not know if they are right.
    /// So when they are wrong I am not sure what will happen. Make it right!
    pub unsafe fn from_bytes(
        vertex_bytes: &[u8],
        fragment_bytes: &[u8],
        entry_point: &str,
    ) -> Result<Self, ShaderError> {
        let resources = &RESOURCES;
        let device = resources.vulkan().clone().device;
        let vertex_words = bytes_to_words(vertex_bytes)?;
        let fragment_words = bytes_to_words(fragment_bytes)?;
        let vertex: Arc<ShaderModule> = unsafe {
            ShaderModule::new(device.clone(), ShaderModuleCreateInfo::new(&vertex_words))?
        };
        let fragment: Arc<ShaderModule> = unsafe {
            ShaderModule::new(device.clone(), ShaderModuleCreateInfo::new(&fragment_words))?
        };
        vertex
            .entry_point(entry_point)
            .ok_or(ShaderError::ShaderEntryPoint)?;
        fragment
            .entry_point(entry_point)
            .ok_or(ShaderError::ShaderEntryPoint)?;
        Ok(Self {
            vertex,
            fragment,
            entry_point: entry_point.into(),
        })
    }
    pub fn from_modules(
        vertex: Arc<ShaderModule>,
        fragment: Arc<ShaderModule>,
        entry_point: impl Into<Box<str>>,
    ) -> Self {
        Self {
            vertex,
            fragment,
            entry_point: entry_point.into(),
        }
    }
}
