//! Material related settings that determine the way the scene gets rendered.

use anyhow::{Error, Result};
use foldhash::HashMap;
use let_engine_core::resources::{
    buffer::Location,
    material::{GraphicsShaders, Material, MaterialSettings},
    model::Vertex,
};

use std::sync::Arc;
use thiserror::Error;

use vulkano::{
    pipeline::graphics::vertex_input::{VertexDefinition, VertexInputState},
    shader::{
        DescriptorBindingRequirements, EntryPoint, ShaderModule, ShaderModuleCreateInfo,
        spirv::bytes_to_words,
    },
};

use crate::backend::gpu::vulkan::VIRTUAL_TAG_BIT;

use super::{VulkanError, vertex_buffer_description_to_vulkano, vulkan::Vulkan};

/// A material holding the way an object should be drawn.
#[derive(Clone)]
pub struct GpuMaterial {
    settings: MaterialSettings,
    shaders: VulkanGraphicsShaders,

    pub(crate) vertex_input_state: VertexInputState,
}

concurrent_slotmap::declare_key! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub struct MaterialId
}

impl MaterialId {
    pub const TAG_BIT: u32 = 1 << 6;

    pub fn is_virtual(&self) -> bool {
        self.0.tag() & VIRTUAL_TAG_BIT != 0
    }
}

/// Errors that occur from material management.
#[derive(Debug, Error)]
pub enum GpuMaterialError {
    /// An unexpected error which could occur.
    #[error(transparent)]
    InvalidVertexType(Error),

    /// Error when attempting to create a default material.
    #[error("There was a problem creating the shader for this material: {0}")]
    Shader(ShaderError),

    /// An invalid combination of settings.
    ///
    /// Element `0` is a specified setting, which does not work with element `1`.
    #[error("Invalid combination of settings. {0} does not work with {1}.")]
    InvalidSettings(&'static str, String),
}

impl std::fmt::Debug for GpuMaterial {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Material")
            .field("settings", &self.settings)
            .field("shaders", &self.shaders)
            .finish()
    }
}

/// # Creation
///
/// Right now it produces an error when the shaders do not have a main function.
impl GpuMaterial {
    /// Creates a new material using the given shaders, settings and write operations.
    pub(crate) fn new<V: Vertex>(
        material: &Material,
        vulkan: &Vulkan,
    ) -> Result<Self, GpuMaterialError> {
        let settings = material.settings.clone();
        let shaders = unsafe {
            VulkanGraphicsShaders::from_bytes(material.graphics_shaders.clone(), vulkan)
                .map_err(GpuMaterialError::Shader)?
        };

        if settings.primitive_restart && settings.topology.is_list() {
            return Err(GpuMaterialError::InvalidSettings(
                "`primitive_restart = true`",
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

    /// Returns the material settings of this type.
    pub fn settings(&self) -> &MaterialSettings {
        &self.settings
    }

    /// Returns the graphics shaders of this material.
    pub(crate) fn graphics_shaders(&self) -> &VulkanGraphicsShaders {
        &self.shaders
    }
}

/// Holds compiled shaders in form of ShaderModules to use in a material.
#[derive(Clone, Debug)]
pub(crate) struct VulkanGraphicsShaders {
    pub vertex: EntryPoint,
    pub fragment: Option<EntryPoint>,
    pub requirements: HashMap<Location, DescriptorBindingRequirements>,
}

impl VulkanGraphicsShaders {
    /// Creates a shader from SpirV bytes.
    ///
    /// # Safety
    ///
    /// When loading those shaders the engine does not know if they are right.
    pub unsafe fn from_bytes(
        shaders: GraphicsShaders,
        vulkan: &Vulkan,
    ) -> Result<Self, ShaderError> {
        let device = vulkan.device.clone();

        let vertex_words =
            bytes_to_words(&shaders.vertex_bytes).map_err(|_| ShaderError::InvalidSpirV)?;

        let vertex: Arc<ShaderModule> = unsafe {
            ShaderModule::new(&device, &ShaderModuleCreateInfo::new(&vertex_words))
                .map_err(|x| ShaderError::Other(x.unwrap().into()))?
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
                ShaderModule::new(&device, &ShaderModuleCreateInfo::new(&words))
                    .map_err(|x| ShaderError::Other(x.unwrap().into()))?
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
        })
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
    /// Returns when attempting to create a shader,
    /// but the engine has not been started with [`Engine::start`](crate::Engine::start),
    /// or the backend has closed down.
    #[error("Can not create shader: Engine not initialized.")]
    BackendNotInitialized,

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
