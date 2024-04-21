use let_engine_core::{draw::Draw, resources::resources};

use egui_winit_vulkano::{Gui, GuiConfig};
use winit::event_loop::EventLoop;

pub(crate) fn init(draw: &Draw, event_loop: &EventLoop<()>) -> Gui {
    let vulkan = resources().unwrap().vulkan();
    Gui::new_with_subpass(
        event_loop,
        draw.surface.clone(),
        vulkan.queue.clone(),
        vulkan.subpass.clone(),
        vulkano::format::Format::R8G8B8A8_UNORM,
        GuiConfig::default(),
    )
}
