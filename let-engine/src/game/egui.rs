use crate::{resources::RESOURCES, Draw, EVENT_LOOP};

use egui_winit_vulkano::{Gui, GuiConfig};

pub(crate) fn init(draw: &Draw) -> Gui {
    let vulkan = RESOURCES.vulkan();
    EVENT_LOOP.with_borrow(|event_loop| {
        Gui::new_with_subpass(
            event_loop.get().unwrap(),
            draw.surface.clone(),
            vulkan.queue.clone(),
            vulkan.subpass.clone(),
            vulkano::format::Format::R8G8B8A8_UNORM,
            GuiConfig::default(),
        )
    })
}
