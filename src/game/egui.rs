use super::Vulkan;
use egui_winit_vulkano::{Gui, GuiConfig};
use winit::event_loop::EventLoop;

pub fn init(event_loop: &EventLoop<()>, vulkan: &Vulkan) -> Gui {
    Gui::new_with_subpass(
        event_loop,
        vulkan.surface.clone(),
        vulkan.queue.clone(),
        vulkan.subpass.clone(),
        GuiConfig {
            preferred_format: Some(vulkano::format::Format::R8G8B8A8_UNORM),
            ..Default::default()
        },
    )
}
