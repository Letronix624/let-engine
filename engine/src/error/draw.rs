//! Redraw errors

use thiserror::Error;
use vulkano::{shader::spirv::SpirvBytesNotMultipleOf4, Validated, VulkanError as VulkanoError};

/// Errors that originate from Vulkan.
#[derive(Error, Debug)]
pub enum VulkanError {
    #[error("The swapchain is out of date and needs to be updated.")]
    SwapchainOutOfDate,
    #[error("Failed to flush future:\n{0}")]
    FlushFutureError(String),
    #[error("A Validated error:\n{0}")]
    Validated(Validated<VulkanoError>),
}

impl From<Validated<VulkanoError>> for VulkanError {
    fn from(value: Validated<VulkanoError>) -> Self {
        Self::Validated(value)
    }
}

/// Errors that occur from the creation of Shaders.
#[derive(Error, Debug)]
pub enum ShaderError {
    #[error("The given entry point to those shaders is not present in the given shaders.")]
    ShaderEntryPoint,
    #[error("The provided bytes are not SpirV.")]
    InvalidSpirV,
    #[error("Something happened and the shader can not be made.: {0:?}")]
    Other(VulkanError),
}

impl From<Validated<VulkanoError>> for ShaderError {
    fn from(value: Validated<VulkanoError>) -> Self {
        Self::Other(value.into())
    }
}

impl From<SpirvBytesNotMultipleOf4> for ShaderError {
    fn from(_value: SpirvBytesNotMultipleOf4) -> Self {
        Self::InvalidSpirV
    }
}
