use let_engine_core::{draw::Draw, resources::resources};

use egui_winit_vulkano::{Gui, GuiConfig};
use winit::event_loop::ActiveEventLoop;

pub(crate) fn init(draw: &Draw, event_loop: &ActiveEventLoop) -> Gui {
    let vulkan = resources().unwrap().vulkan();
    Gui::new_with_subpass(
        event_loop,
        draw.surface.clone(),
        vulkan.queue.clone(),
        vulkan.subpass.clone(),
        draw.swapchain.image_format(),
        GuiConfig {
            allow_srgb_render_target: true,
            ..Default::default()
        },
    )
}
