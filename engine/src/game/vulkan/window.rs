extern crate image;
extern crate vulkano;
use std::sync::Arc;
use vulkano::instance::Instance;
use vulkano::swapchain::Surface;
use vulkano_win::VkSurfaceBuild;
use winit::{event_loop::EventLoop, window::WindowBuilder};

pub fn create_window(
    instance: &Arc<Instance>,
    builder: WindowBuilder,
) -> (EventLoop<()>, Arc<Surface>) {
    // let icon: DynamicImage =
    //     image::load_from_memory(include_bytes!("../../assets/handsomesquidward.bmp")).unwrap();
    // let icondimension = (icon.height(), icon.width());
    // let iconbytes: Vec<u8> = icon.into_rgba8().into_raw();
    let event_loop = winit::event_loop::EventLoopBuilder::new().build();
    // let surface = WindowBuilder::new()
    //     .with_resizable(true)
    //     .with_title(TITLE)
    //     .with_min_inner_size(LogicalSize::new(200, 200))
    //     .with_inner_size(LogicalSize::new(WIDTH, HEIGHT))
    //     // .with_window_icon(Some(
    //     //     winit::window::Icon::from_rgba(iconbytes, icondimension.1, icondimension.0).unwrap(),
    //     // ))
    //     .with_always_on_top(true)
    //     .with_decorations(true)

    let surface = builder
        .build_vk_surface(&event_loop, instance.clone())
        .unwrap();

    (event_loop, surface)
}