pub mod resources;
use anyhow::Result;
use hashbrown::HashMap;
use resources::{GameFont, Resources, Texture};
pub mod objects;
pub use objects::{data::Data, Appearance, CameraOption, CameraScaling, Node, Object};
pub mod vulkan;
use vulkan::Vulkan;
use winit::{
    event::{Event, WindowEvent},
    event_loop::EventLoop,
    window::{Window, WindowBuilder},
};
mod draw;
use draw::Draw;
mod font_layout;
use font_layout::Labelifier;
pub mod materials;
use image::{load_from_memory_with_format, DynamicImage, ImageFormat as IFormat};
use parking_lot::Mutex;
use std::{sync::Arc, time::Instant};

use crate::{error::objects::*, error::textures::*, texture::*, AppInfo};

pub use self::objects::data::Vertex;

type AObject = Arc<Mutex<Object>>;
type NObject = Arc<Mutex<Node<AObject>>>;

/// This is what you create your whole game session with.
pub struct GameBuilder {
    window_builder: Option<WindowBuilder>,
    app_info: Option<AppInfo>,
    clear_background_color: [f32; 4],
}

impl GameBuilder {
    pub fn new() -> Self {
        Self {
            window_builder: None,
            app_info: None,
            clear_background_color: [0.0; 4],
        }
    }
    pub fn with_window_builder(mut self, window_builder: WindowBuilder) -> Self {
        self.window_builder = Some(window_builder);
        self
    }
    pub fn with_app_info(mut self, app_info: AppInfo) -> Self {
        self.app_info = Some(app_info);
        self
    }
    pub fn with_clear_background_clear_color(mut self, color: [f32; 4]) -> Self {
        self.clear_background_color = color;
        self
    }
    pub fn build(&mut self) -> (Game, EventLoop<()>) {
        let app_info = if let Some(app_info) = self.app_info {
            app_info
        } else {
            panic!("No app info");
        };

        let window_builder = if let Some(window_builder) = self.window_builder.clone() {
            window_builder
        } else {
            panic!("no window builder");
        };

        let clear_background_color = self.clear_background_color;

        let resources = Resources::new();
        let (shaders, vulkan, event_loop) = Vulkan::init(window_builder, app_info);
        let mut draw = Draw::setup(&vulkan);
        let labelifier = Labelifier::new(&vulkan, &mut draw);

        (
            Game {
                objects: vec![],
                objects_map: HashMap::new(),
                resources,
                labelifier,

                time: Instant::now(),
                delta_instant: Instant::now(),
                delta_time: 0.0,
                clear_background_color,

                app_info,
                draw,
                vulkan,
                shaders,
            },
            event_loop,
        )
    }
}

/// The struct that holds and executes all of the game data.
#[allow(dead_code)]
pub struct Game {
    objects: Vec<(NObject, Option<Arc<Mutex<Node<AObject>>>>)>,
    objects_map: HashMap<*const Mutex<Object>, NObject>,
    resources: Resources,
    labelifier: Labelifier,

    time: Instant,
    delta_instant: Instant,
    delta_time: f64,
    clear_background_color: [f32; 4],

    app_info: AppInfo,
    draw: Draw,
    vulkan: Vulkan,
    shaders: materials::Shaders,
}

impl Game {
    pub fn update<T: 'static>(&mut self, event: &Event<T>) {
        match event {
            Event::WindowEvent {
                event: WindowEvent::Resized(_),
                ..
            } => {
                self.draw.recreate_swapchain = true;
            }
            Event::RedrawEventsCleared => {
                self.draw.redrawevent(
                    &mut self.vulkan,
                    self.objects.clone(),
                    self.clear_background_color,
                );
                self.labelifier.update(&self.vulkan, &mut self.draw);
                self.delta_time = self.delta_instant.elapsed().as_secs_f64();
                self.delta_instant = Instant::now();
            }
            _ => (),
        }
    }
    pub fn new_layer(&mut self) -> AObject {
        let object = Arc::new(Mutex::new(Object::new()));

        let node = Arc::new(Mutex::new(Node {
            object: object.clone(),
            parent: None,
            children: vec![],
        }));
        self.objects.push((node.clone(), None));

        self.objects_map.insert(Arc::as_ptr(&object), node.clone());
        object
    }
    pub fn load_texture_from_raw(
        &mut self,
        texture: Vec<u8>,
        format: Format,
        dimensions: (u32, u32),
        layers: u32,
        settings: TextureSettings,
    ) -> Arc<Texture> {
        Texture::new(
            texture.clone(),
            dimensions,
            layers,
            self.draw
                .load_texture(&self.vulkan, texture, dimensions, layers, format, settings),
            1,
        )
    }

    pub unsafe fn new_shader_from_raw(
        // loading things all temporary. Will get sepparated to their own things soon.
        &self,
        vertex_bytes: &[u8],
        fragment_bytes: &[u8],
    ) -> materials::Shaders {
        unsafe { materials::Shaders::from_bytes(vertex_bytes, fragment_bytes, &self.vulkan) }
    }

    pub fn default_shader(&self) -> &materials::Shaders {
        &self.shaders
    }

    pub fn new_material(&mut self, settings: materials::MaterialSettings) -> materials::Material {
        self.draw.load_material(&self.vulkan, settings)
    }
    pub fn load_texture(
        &mut self,
        texture: &[u8],
        format: ImageFormat,
        layers: u32,
        settings: TextureSettings,
    ) -> Result<Arc<Texture>, InvalidFormatError> {
        let image_format = match format {
            ImageFormat::PNG => IFormat::Png,
            ImageFormat::JPG => IFormat::Jpeg,
            ImageFormat::BMP => IFormat::Bmp,
            ImageFormat::TIFF => IFormat::Tiff,
            ImageFormat::WebP => IFormat::WebP,
            ImageFormat::TGA => IFormat::Tga,
        };
        let image = match load_from_memory_with_format(texture, image_format) {
            Err(_) => return Err(InvalidFormatError),
            Ok(v) => v,
        };

        let mut dimensions: (u32, u32);

        let mut format = Format::RGBA8;

        let image: Vec<u8> = match image {
            DynamicImage::ImageLuma8(image) => {
                format = Format::R8;
                dimensions = image.dimensions();
                image.into_vec()
            }
            DynamicImage::ImageLumaA8(_) => {
                let image = image.to_rgba8();
                dimensions = image.dimensions();
                image.into_vec()
            }
            DynamicImage::ImageLuma16(_) => {
                let image = image.to_luma8();
                dimensions = image.dimensions();
                format = Format::R8;
                image.into_vec()
            }
            DynamicImage::ImageLumaA16(_) => {
                let image = image.to_rgba16();
                dimensions = image.dimensions();
                format = Format::RGBA16;
                u16tou8vec(image.into_vec())
            }
            DynamicImage::ImageRgb8(_) => {
                let image = image.to_rgba8();
                dimensions = image.dimensions();
                image.into_vec()
            }
            DynamicImage::ImageRgba8(image) => {
                dimensions = image.dimensions();
                image.into_vec()
            }
            DynamicImage::ImageRgb16(_) => {
                let image = image.to_rgba16();
                dimensions = image.dimensions();
                format = Format::RGBA16;
                u16tou8vec(image.into_vec())
            }
            DynamicImage::ImageRgba16(image) => {
                format = Format::RGBA16;
                dimensions = image.dimensions();
                u16tou8vec(image.into_vec())
            }
            DynamicImage::ImageRgb32F(_) => {
                let image = image.to_rgba16();
                dimensions = image.dimensions();
                format = Format::RGBA16;
                u16tou8vec(image.into_vec())
            }
            DynamicImage::ImageRgba32F(_) => {
                let image = image.to_rgba16();
                dimensions = image.dimensions();
                format = Format::RGBA16;
                u16tou8vec(image.into_vec())
            }
            _ => {
                let image = image.to_rgba8();
                dimensions = image.dimensions();
                image.into_vec()
            }
        };

        dimensions.1 = dimensions.1 / layers;

        Ok(Self::load_texture_from_raw(
            self, image, format, dimensions, layers, settings,
        ))
    }
    pub fn load_font(&mut self, data: &[u8]) -> Arc<GameFont> {
        self.resources.load_font(data)
    }
    pub fn set_camera(
        &mut self,
        layer: &AObject,
        camera: &AObject,
    ) -> Result<(), Box<dyn std::error::Error>> {
        {
            let mut camera = camera.lock();

            if let None = camera.camera {
                camera.camera = Some(CameraOption::new())
            }
        }

        if let Some(layer) = self.objects_map.get(&Arc::as_ptr(layer)) {
            if let Some(index) = self.objects.iter().position(|x| Arc::ptr_eq(&x.0, layer)) {
                if let Some(camera) = self.objects_map.get(&Arc::as_ptr(camera)) {
                    self.objects[index].1 = Some(camera.clone())
                } else {
                    return Err(Box::new(NoObjectError));
                }
            }
        } else {
            return Err(Box::new(NoLayerError));
        }

        Ok(())
    }
    pub fn add_object(
        &mut self,
        parent: &AObject,
        initial_object: Object,
    ) -> Result<AObject, NoParentError> {
        let object = Arc::new(Mutex::new(initial_object));

        let parent = if let Some(parent) = self.objects_map.get(&Arc::as_ptr(parent)) {
            parent.clone()
        } else {
            return Err(NoParentError);
        };

        let node = Arc::new(Mutex::new(Node {
            object: object.clone(),
            parent: Some(Arc::downgrade(&parent)),
            children: vec![],
        }));

        parent.lock().children.push(node.clone());

        self.objects_map.insert(Arc::as_ptr(&object), node);
        Ok(object)
    }
    pub fn contains_object(&self, object: &AObject) -> bool {
        self.objects_map.contains_key(&Arc::as_ptr(object))
    }
    pub fn remove_object(&mut self, object: &AObject) -> Result<(), NoObjectError> {
        let node: NObject;
        if let Some(obj) = self.objects_map.remove(&Arc::as_ptr(object)) {
            node = obj.clone();
        } else {
            return Err(NoObjectError);
        }
        let mut objectguard = node.lock();

        objectguard.remove_children(&mut self.objects_map);

        if let Some(parent) = &objectguard.parent {
            let parent = parent.clone().upgrade().unwrap();
            let mut parent = parent.lock();
            parent.remove_child(&node);
        } else {
            if let Some(index) = self
                .objects
                .clone()
                .into_iter()
                .position(|x| Arc::ptr_eq(&x.0, &node))
            {
                self.objects.remove(index);
            }
        }
        Ok(())
    }

    pub fn time(&self) -> f64 {
        self.time.elapsed().as_secs_f64()
    }

    pub fn delta_time(&self) -> f64 {
        self.delta_time
    }

    pub fn fps(&self) -> f64 {
        1.0 / self.delta_time
    }
    pub fn set_clear_background_color(&mut self, color: [f32; 4]) {
        self.clear_background_color = color;
    }
    pub fn get_window(&self) -> &Window {
        self.vulkan
            .surface
            .object()
            .unwrap()
            .downcast_ref::<Window>()
            .unwrap()
    }
    pub fn queue_to_label(
        &mut self,
        object: &AObject,
        font: &Arc<GameFont>,
        text: &str,
        scale: f32,
        align: [f32; 2],
    ) {
        self.labelifier
            .queue(object.clone(), font, text.to_string(), scale, align);
    }
}

fn u16tou8vec(data: Vec<u16>) -> Vec<u8> {
    data.iter()
        .flat_map(|&u16_value| {
            let high_byte = ((u16_value >> 8) & 0xff) as u8;
            let low_byte = (u16_value & 0xff) as u8;
            vec![high_byte, low_byte]
        })
        .collect()
}
