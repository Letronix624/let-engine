extern crate image;
extern crate vulkano;
use image::DynamicImage;
use std::sync::Arc;
use vulkano::instance::Instance;
use vulkano::swapchain::Surface;
use vulkano_win::VkSurfaceBuild;
use winit::dpi::LogicalSize;
use winit::{event_loop::EventLoop, window::{WindowBuilder, Fullscreen}};

pub struct Window {
    event_loop: EventLoop<()>,
    surface: Surface,
    // resizable: bool,
    // min_size: [u32; 2],
    // max_size: [u32; 2],
    // position: [i32; 2],
    // title: String,
    // visible: bool,
    // decorations: bool,

}
impl Window {
    pub fn create_window(instance: &Arc<Instance>, builder: WindowBuilder) -> Self {
        // let icon: DynamicImage =
        //     image::load_from_memory(include_bytes!("../../assets/handsomesquidward.bmp")).unwrap();
        let icondimension = (icon.height(), icon.width());
        let iconbytes: Vec<u8> = icon.into_rgba8().into_raw();
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
            .with_visible(false)
            .build_vk_surface(&event_loop, instance.clone())
            .unwrap();

        Self {
            event_loop,
            surface
        }
    }
}

