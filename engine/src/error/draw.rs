//! Redraw errors

use thiserror::Error;

#[derive(Error, Debug)]
pub enum RedrawError {
    #[error("The swapchain is out of date and needs to be updated.")]
    SwapchainOutOfDate,
    #[error("Failed to flush future:\n{0}")]
    FlushFutureError(String),
    #[error("A Vulkan error:\n{0}")]
    VulkanError(String),
}

#[derive(Error, Debug)]
#[error("The swapchain failed to be recreated:\n{0}")]
pub struct SwapchainRecreationError(pub String);
