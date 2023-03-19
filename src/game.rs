pub mod resources;
use anyhow::Result;
use hashbrown::HashMap;
use resources::Resources;
pub mod objects;
pub use objects::{data::Data, Appearance, Node, Object};
pub mod vulkan;
use rusttype::{point, PositionedGlyph};
use vulkan::Vulkan;
use winit::{
    event_loop::EventLoop,
    window::{Window, WindowBuilder},
};
mod draw;
use draw::Draw;
mod font_layout;

use parking_lot::Mutex;
use std::sync::Arc;

use crate::{errors::*, AppInfo};

use self::objects::data::Vertex;

/// This is what you create your whole game session with.
pub struct GameBuilder {
    window_builder: Option<WindowBuilder>,
    app_info: Option<AppInfo>,
    //resources: Resources,
}

impl GameBuilder {
    pub fn new() -> Self {
        Self {
            window_builder: None,
            app_info: None,
            //resources: Resources::new(),
        }
    }
    // pub fn with_resources(mut self, resources: Resources) -> Self {
    //     self.resources = resources;
    //     self
    // }
    pub fn with_window_builder(mut self, window_builder: WindowBuilder) -> Self {
        self.window_builder = Some(window_builder);
        self
    }
    pub fn with_app_info(mut self, app_info: AppInfo) -> Self {
        self.app_info = Some(app_info);
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

        let resources = Resources::new();
        let (vulkan, event_loop) = Vulkan::init(window_builder, app_info);
        let draw = Draw::setup(&vulkan);

        (
            Game {
                objects: vec![],
                objects_map: HashMap::new(),
                resources,
                app_info,
                vulkan,
                draw,
            },
            event_loop,
        )
    }
}

/// The struct that holds and executes all of the game data.
#[allow(dead_code)]
pub struct Game {
    objects: Vec<Arc<Mutex<Node<Arc<Mutex<Object>>>>>>,
    objects_map: HashMap<*const Mutex<Object>, Arc<Mutex<Node<Arc<Mutex<Object>>>>>>,
    //main_camera
    resources: Resources,
    app_info: AppInfo,
    draw: Draw,
    vulkan: Vulkan,
}

impl Game {
    pub fn update(&mut self) {
        self.draw
            .redrawevent(&mut self.vulkan, self.objects.clone());
    }
    pub fn recreate_swapchain(&mut self) {
        self.draw.recreate_swapchain = true;
    }
    pub fn add_object(&mut self, object: &Arc<Mutex<Object>>) -> Result<(), ObjectExistsError> {
        if Self::contains_object(&self, object) {
            return Err(ObjectExistsError);
        }

        let node = Arc::new(Mutex::new(Node {
            object: object.clone(),
            parent: None,
            children: vec![],
        }));
        self.objects.push(node.clone());

        self.objects_map.insert(Arc::as_ptr(&object), node.clone());
        Ok(())
    }
    pub fn add_child_object(
        &mut self,
        parent: &Arc<Mutex<Object>>,
        object: &Arc<Mutex<Object>>,
    ) -> Result<(), Box<dyn std::error::Error>> {

        if Self::contains_object(&self, object) {
            return Err(Box::new(ObjectExistsError));
        }

        let parent = if let Some(parent) = self.objects_map.get(&Arc::as_ptr(parent)) {
            parent.clone()
        } else {
            return Err(Box::new(NoParentError));
        };

        let node = Arc::new(Mutex::new(Node {
            object: object.clone(),
            parent: Some(Arc::downgrade(&parent)),
            children: vec![],
        }));

        parent.lock().children.push(node.clone());

        self.objects_map.insert(Arc::as_ptr(&object), node);
        Ok(())
    }
    pub fn contains_object(&self, object: &Arc<Mutex<Object>>) -> bool {
        self.objects_map.contains_key(&Arc::as_ptr(object))
    }
    pub fn remove_object(&mut self, object: &Arc<Mutex<Object>>) -> Result<(), NoObjectError> {
        let node: Arc<Mutex<Node<Arc<Mutex<Object>>>>>;
        if let Some(obj) = self.objects_map.remove(&Arc::as_ptr(object)) {
            node = obj.clone();
        } else {
            return Err(NoObjectError);
        }
        let objectguard = node.lock();
        if let Some(parent) = &objectguard.parent {
            let parent = parent.clone().upgrade().unwrap();

            parent.lock().remove_child(&node);
        } else {
            if let Some(index) = self
                .objects
                .clone()
                .into_iter()
                .position(|x| Arc::ptr_eq(&x, &node))
            {
                self.objects.remove(index);
            }
        }
        Ok(())
    }

    pub fn load_font_bytes(&mut self, name: &str, data: &[u8]) {
        self.resources.add_font_bytes(name, data);
    }
    pub fn unload_font(&mut self, name: &str) {
        self.resources.remove_font(name);
    }
    pub fn unload_texture(&mut self, name: &str) {
        self.resources.remove_texture(name);
    }
    pub fn unload_sound(&mut self, name: &str) {
        self.resources.remove_sound(name);
    }
    pub fn load_sound(&mut self, name: &str, sound: &[u8]) {
        self.resources.add_sound(name, sound);
    }
    pub fn load_texture(&mut self, name: &str, texture: Vec<u8>, width: u32, height: u32) {
        self.resources.add_texture(name, texture, width, height);
        self.draw.update_textures(&self.vulkan, &self.resources);
    }
    pub fn get_window(&self) -> &Window {
        self.vulkan
            .surface
            .object()
            .unwrap()
            .downcast_ref::<Window>()
            .unwrap()
    }
    pub fn get_font_data(
        // make sepparate rs for this. add this to font_layout. ooga
        &mut self,
        font: &str,
        text: &str,
        size: f32,
        color: [f32; 4],
    ) -> Appearance {
        let fontname = font;
        let font = self.resources.fonts.get(font).unwrap().clone();

        let glyphs: Vec<PositionedGlyph> = font
            .0
            .layout(
                text, //text,
                rusttype::Scale::uniform(size),
                point(0.0, font.0.v_metrics(rusttype::Scale::uniform(size)).ascent),
            )
            .collect();

        self.resources.update_cache(fontname, glyphs.clone());

        let dimensions: [u32; 2] = [1000; 2];

        let mut indices: Vec<u16> = vec![];

        let vertices: Vec<Vertex> = glyphs
            .clone()
            .iter()
            .flat_map(|g| {
                if let Ok(Some((uv_rect, screen_rect))) = self.resources.cache.rect_for(font.1, g) {
                    let gl_rect = rusttype::Rect {
                        min: point(
                            (screen_rect.min.x as f32 / dimensions[0] as f32 - 0.5) * 2.0,
                            (screen_rect.min.y as f32 / dimensions[1] as f32 - 0.5) * 2.0,
                        ),
                        max: point(
                            (screen_rect.max.x as f32 / dimensions[0] as f32 - 0.5) * 2.0,
                            (screen_rect.max.y as f32 / dimensions[1] as f32 - 0.5) * 2.0,
                        ),
                    };
                    indices.extend([0, 1, 2, 2, 3, 0]);
                    vec![
                        Vertex {
                            position: [gl_rect.min.x, gl_rect.max.y],
                            tex_position: [uv_rect.min.x, uv_rect.max.y],
                        },
                        Vertex {
                            position: [gl_rect.min.x, gl_rect.min.y],
                            tex_position: [uv_rect.min.x, uv_rect.min.y],
                        },
                        Vertex {
                            position: [gl_rect.max.x, gl_rect.min.y],
                            tex_position: [uv_rect.max.x, uv_rect.min.y],
                        },
                        Vertex {
                            position: [gl_rect.max.x, gl_rect.min.y],
                            tex_position: [uv_rect.max.x, uv_rect.min.y],
                        },
                        Vertex {
                            position: [gl_rect.max.x, gl_rect.max.y],
                            tex_position: [uv_rect.max.x, uv_rect.max.y],
                        },
                        Vertex {
                            position: [gl_rect.min.x, gl_rect.max.y],
                            tex_position: [uv_rect.min.x, uv_rect.max.y],
                        },
                    ]
                    .into_iter()
                } else {
                    vec![].into_iter()
                }
            })
            .collect();
        self.draw.update_font_objects(&self.vulkan, &self.resources);
        let object = Appearance {
            texture: Some("fontatlas".to_string()),
            data: Data {
                vertices: vertices,
                indices: indices,
            },
            //data: Data::square(),
            color,
            material: 2,
            ..Appearance::empty()
        };
        //self.textobjects.push(object.clone());
        object
    }
}
