//! Material related settings that determine the way the scene gets rendered.

use anyhow::{Error, Result};
use foldhash::HashMap;
use let_engine_core::resources::{
    buffer::Location,
    material::{GraphicsShaders, MaterialSettings},
    model::Vertex,
};

use std::{hash::BuildHasher, sync::Arc};
use thiserror::Error;

use vulkano::{
    pipeline::graphics::vertex_input::{VertexDefinition, VertexInputRate, VertexInputState},
    shader::{
        spirv::bytes_to_words, DescriptorBindingRequirements, EntryPoint, ShaderModule,
        ShaderModuleCreateInfo,
    },
};

use super::{
    vertex_buffer_description_to_vulkano,
    vulkan::shaders::{default_shader, default_textured_shader},
    GraphicsInterface, VulkanError,
};

/// A material holding the way an object should be drawn.
#[derive(Clone)]
pub struct GpuMaterial {
    settings: MaterialSettings,
    shaders: VulkanGraphicsShaders,

    pub(crate) vertex_input_state: VertexInputState,
}

/// Errors that occur from material management.
#[derive(Debug, Error)]
pub enum GpuMaterialError {
    /// An unexpected error which could occur.
    #[error("{0}")]
    InvalidVertexType(Error),

    /// Error when attempting to create a default material.
    #[error("There was a problem creating the shader for this material: {0}")]
    Shader(ShaderError),

    /// An invalid combination of settings.
    ///
    /// Element `0` is a specified setting, which does not work with element `1`.
    #[error("Invalid combination of settings. {0} does not work with {1}.")]
    InvalidSettings(String, String),
}

impl PartialEq for GpuMaterial {
    fn eq(&self, other: &Self) -> bool {
        let vertex_input =
            eq_vertex_input_state(&self.vertex_input_state, &other.vertex_input_state);

        self.settings == other.settings && self.shaders.hash == other.shaders.hash && vertex_input
    }
}

impl Eq for GpuMaterial {}

impl std::fmt::Debug for GpuMaterial {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Material")
            .field("settings", &self.settings)
            .field("shaders", &self.shaders)
            .finish()
    }
}

impl std::hash::Hash for GpuMaterial {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.settings.hash(state);

        state.write_u64(self.shaders.hash);

        let bindings = self.vertex_input_state.bindings.iter();
        let attributes = self.vertex_input_state.attributes.iter();

        for (k, v) in bindings {
            state.write_u32(*k);

            state.write_u32(v.stride);
            match v.input_rate {
                VertexInputRate::Vertex => {
                    state.write_u8(0);
                }
                VertexInputRate::Instance { divisor } => {
                    state.write_u8(1);
                    state.write_u32(divisor);
                }
            }
        }

        for (k, v) in attributes {
            state.write_u32(*k);

            state.write_u32(v.binding);
            state.write_u32(v.offset);
            v.format.hash(state);
        }
    }
}

/// # Creation
///
/// Right now it produces an error when the shaders do not have a main function.
impl GpuMaterial {
    /// Creates a new material using the given shaders, settings and write operations.
    pub fn new<V: Vertex>(
        settings: MaterialSettings,
        shaders: VulkanGraphicsShaders,
    ) -> Result<Self, GpuMaterialError> {
        if settings.primitive_restart && settings.topology.is_list() {
            return Err(GpuMaterialError::InvalidSettings(
                // TODO: What is this?
                "`primitive_restart = true`".to_string(),
                format!("`topology = {:?}`", settings.topology),
            ));
        };

        let vertex_input_state = vertex_buffer_description_to_vulkano(V::description())
            .definition(&shaders.vertex)
            .map_err(|e| GpuMaterialError::InvalidVertexType(e.into()))?;

        Ok(Self {
            settings,
            shaders,
            vertex_input_state,
        })
    }

    /// Creates a new default material containing a simple shader that just applies the MVP matrix at binding 0, 0 and uses [`Vertex`] as the vertex type.
    pub fn new_default(interface: &GraphicsInterface) -> Result<Self, GpuMaterialError> {
        Self::new::<let_engine_core::resources::data::Vert>(
            MaterialSettings::default(),
            VulkanGraphicsShaders::new_default(interface).map_err(GpuMaterialError::Shader)?,
        )
    }

    /// Creates a new default material containing a simple shader that just applies the MVP matrix at binding 0, 0 and uses [`TVertex`] as the vertex type.
    pub fn new_default_textured(interface: &GraphicsInterface) -> Result<Self, GpuMaterialError> {
        Self::new::<let_engine_core::resources::data::TVert>(
            MaterialSettings::default(),
            VulkanGraphicsShaders::new_default_textured(interface)
                .map_err(GpuMaterialError::Shader)?,
        )
    }

    /// Returns the material settings of this type.
    pub fn settings(&self) -> &MaterialSettings {
        &self.settings
    }

    /// Returns the graphics shaders of this material.
    pub fn graphics_shaders(&self) -> &VulkanGraphicsShaders {
        &self.shaders
    }
}

// texture
// impl GpuMaterial {
// /// Sets the layer of the texture in case it has a texture with layers.
// pub fn set_layer(&mut self, id: u32) -> Result<(), TextureError> {
//     if let Some(texture) = &self.texture {
//         if id > texture.layers() - 1 {
//             return Err(TextureError::Layer(format!(
//                 "Given: {}, Highest: {}",
//                 id,
//                 texture.layers() - 1
//             )));
//         }
//     } else {
//         return Err(TextureError::NoTexture);
//     }
//     self.layer = id;
//     Ok(())
// }

// /// Returns the layer of the texture in case the material is textured.
// pub fn layer(&self) -> u32 {
//     self.layer
// }

// /// Goes to the next frame of the texture.
// ///
// /// Returns an error if it reached the limit.
// pub fn next_frame(&mut self) -> Result<(), TextureError> {
//     if let Some(texture) = &self.texture {
//         if texture.layers() <= self.layer + 1 {
//             return Err(TextureError::Layer(
//                 "You are already at the last frame.".to_string(),
//             ));
//         }
//     } else {
//         return Err(TextureError::NoTexture);
//     }
//     self.layer += 1;
//     Ok(())
// }

// /// Goes back a frame of the texture.
// ///
// /// Returns an error if the layer is already on 0.
// pub fn last_frame(&mut self) -> Result<(), TextureError> {
//     if self.texture.is_some() {
//         if self.layer == 0 {
//             return Err(TextureError::Layer(
//                 "You are already on the first frame".to_string(),
//             ));
//         }
//     } else {
//         return Err(TextureError::NoTexture);
//     }
//     self.layer -= 1;
//     Ok(())
// }

// /// Returns the texture.
// pub fn texture(&self) -> Option<Texture> {
//     self.texture.clone()
// }

// /// Sets the texture.
// pub fn set_texture(&mut self, texture: Option<Texture>) {
//     self.texture = texture;
// }
// }

/// Holds compiled shaders in form of ShaderModules to use in a material.
#[derive(Clone, Debug)]
pub struct VulkanGraphicsShaders {
    pub(crate) vertex: EntryPoint,
    pub(crate) fragment: Option<EntryPoint>,
    pub(crate) requirements: HashMap<Location, DescriptorBindingRequirements>,
    pub(crate) hash: u64,
}

impl PartialEq for VulkanGraphicsShaders {
    fn eq(&self, other: &Self) -> bool {
        self.hash == other.hash
    }
}

impl VulkanGraphicsShaders {
    /// Creates a shader from SpirV bytes.
    ///
    /// # Safety
    ///
    /// When loading those shaders the engine does not know if they are right.
    /// So when they are wrong I am not sure what will happen. Make it right!
    pub unsafe fn from_bytes(
        shaders: GraphicsShaders,
        interface: &GraphicsInterface,
    ) -> Result<Self, ShaderError> {
        let device = interface.vulkan.device.clone();

        let hash = *{
            let hasher = foldhash::fast::RandomState::default();
            &hasher.hash_one(shaders.clone())
        };

        let vertex_words =
            bytes_to_words(&shaders.vertex_bytes).map_err(|_| ShaderError::InvalidSpirV)?;

        let vertex: Arc<ShaderModule> = unsafe {
            ShaderModule::new(device.clone(), ShaderModuleCreateInfo::new(&vertex_words))
                .map_err(|x| ShaderError::Other(x.into()))?
        };

        let vertex = vertex
            .entry_point_with_execution(
                &shaders.vertex_entry_point,
                vulkano::shader::spirv::ExecutionModel::Vertex,
            )
            .ok_or(ShaderError::EntryPoint)?;

        let fragment = if let Some(frag) = shaders.fragment_bytes {
            let words = bytes_to_words(&frag).map_err(|_| ShaderError::InvalidSpirV)?;
            let fragment = unsafe {
                ShaderModule::new(device, ShaderModuleCreateInfo::new(&words))
                    .map_err(|x| ShaderError::Other(x.into()))?
            };
            let Some(entry_point) = &shaders.fragment_entry_point else {
                return Err(ShaderError::NoFragmentEntry);
            };
            Some(
                fragment
                    .entry_point_with_execution(
                        entry_point,
                        vulkano::shader::spirv::ExecutionModel::Fragment,
                    )
                    .ok_or(ShaderError::EntryPoint)?,
            )
        } else {
            None
        };

        let requirements = Self::requirements(&vertex, fragment.as_ref())
            .map_err(|e| ShaderError::IncompatibleDescriptor(e.to_string()))?;

        Ok(Self {
            vertex,
            fragment,
            requirements,
            hash,
        })
    }

    pub fn new_default(interface: &GraphicsInterface) -> Result<Self, ShaderError> {
        unsafe { Self::from_bytes(default_shader(), interface) }
    }

    pub fn new_default_textured(interface: &GraphicsInterface) -> Result<Self, ShaderError> {
        unsafe { Self::from_bytes(default_textured_shader(), interface) }
    }

    /// Returns a single hashmap of locations and descriptor binding requirements for their corresponding bindings.
    ///
    /// Returns an error in case the shaders are not compatible with each other.
    /// When both shaders require a different element for the same set and binding.
    fn requirements(
        vertex: &EntryPoint,
        fragment: Option<&EntryPoint>,
    ) -> Result<HashMap<Location, DescriptorBindingRequirements>> {
        let mut map: HashMap<Location, DescriptorBindingRequirements> = vertex
            .info()
            .descriptor_binding_requirements
            .iter()
            .map(|(k, v)| ((*k).into(), v.clone()))
            .collect();

        if let Some(fragment) = fragment {
            for (location, requirement) in fragment.info().descriptor_binding_requirements.iter() {
                let location: Location = (*location).into();

                if let Some(other_requirement) = map.get_mut(&location) {
                    other_requirement.merge(requirement)?;
                } else {
                    map.insert(location, requirement.clone());
                }
            }
        }

        Ok(map)
    }
}

// TODO: Comment
/// Errors that occur from the creation of Shaders.
#[derive(thiserror::Error, Debug)]
pub enum ShaderError {
    #[error("The given entry point to those shaders is not present in the given shaders.")]
    EntryPoint,

    #[error("The provided bytes are not SpirV.")]
    InvalidSpirV,

    #[error("No entry point provided to the fragment shader.")]
    NoFragmentEntry,

    #[error("The shaders are not compatible with eachother: {0}")]
    IncompatibleDescriptor(String),

    #[error("Something happened and the shader can not be made.: {0:?}")]
    Other(VulkanError),
}

pub(crate) fn eq_vertex_input_state(state1: &VertexInputState, state2: &VertexInputState) -> bool {
    let bindings = &state1.bindings;
    let other_bindings = &state2.bindings;

    if bindings.len() != other_bindings.len() {
        return false;
    }

    for (k, v) in bindings {
        if let Some(element) = other_bindings.get(k) {
            if v.stride != element.stride || v.input_rate != element.input_rate {
                return false;
            }
        } else {
            return false;
        }
    }

    let attributes = &state1.attributes;
    let other_attributes = &state2.attributes;

    if attributes.len() != other_attributes.len() {
        return false;
    }

    for (k, v) in attributes {
        if let Some(element) = other_attributes.get(k) {
            if v.binding != element.binding
                || v.offset != element.offset
                || v.format != element.format
            {
                return false;
            }
        } else {
            return false;
        }
    }

    true
}
