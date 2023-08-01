use super::Layer;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use winit::dpi::PhysicalSize;
use winit::event::{ElementState, Event, ModifiersState, WindowEvent};
pub use winit::event::{MouseButton, VirtualKeyCode};

use crossbeam::atomic::AtomicCell;
use glam::f32::{vec2, Vec2};
use hashbrown::HashSet;
use parking_lot::Mutex;

#[derive(Clone)]
pub struct Input {
    //pressed keyboard buttons
    keyboard_down: Arc<Mutex<HashSet<u32>>>,
    //pressed keyboard modifiers
    keyboard_modifiers: Arc<Mutex<ModifiersState>>,
    //pressed mouse buttons
    mouse_down: Arc<Mutex<HashSet<MouseButton>>>,
    //mouse position
    cursor_position: Arc<AtomicCell<Vec2>>,
    cursor_inside: Arc<AtomicBool>,
    //dimensions of the window
    dimensions: Arc<AtomicCell<(f32, f32)>>, // lazylock future
}

impl Input {
    pub fn update<T: 'static>(&mut self, event: &Event<T>, dimensions: PhysicalSize<u32>) {
        self.dimensions
            .store((dimensions.width as f32, dimensions.height as f32));
        if let Event::WindowEvent { event, .. } = event {
            match event {
                WindowEvent::KeyboardInput { input, .. } => {
                    if input.state == ElementState::Pressed {
                        self.keyboard_down.lock().insert(input.scancode);
                    } else {
                        self.keyboard_down.lock().remove(&input.scancode);
                    }
                }
                WindowEvent::ModifiersChanged(modifiers) => {
                    *self.keyboard_modifiers.lock() = *modifiers;
                }
                WindowEvent::MouseInput { state, button, .. } => {
                    if state == &ElementState::Pressed {
                        self.mouse_down.lock().insert(*button);
                    } else {
                        self.mouse_down.lock().remove(button);
                    }
                }
                WindowEvent::CursorMoved { position, .. } => {
                    self.cursor_position.store(vec2(
                        (position.x as f32 / dimensions.width as f32) * 2.0 - 1.0,
                        (position.y as f32 / dimensions.height as f32) * 2.0 - 1.0,
                    ));
                }
                WindowEvent::CursorEntered { .. } => {
                    self.cursor_inside.store(true, Ordering::Release)
                }
                WindowEvent::CursorLeft { .. } => {
                    self.cursor_inside.store(false, Ordering::Release)
                }
                _ => (),
            }
        }
    }

    pub fn is_down(&self, key: &u32) -> bool {
        self.keyboard_down.lock().contains(key)
    }
    pub fn mouse_down(&self, button: &MouseButton) -> bool {
        self.mouse_down.lock().contains(button)
    }
    pub fn cursor_position(&self) -> Vec2 {
        let cp = self.cursor_position.load();
        vec2(cp[0], cp[1])
    }
    pub fn scaled_cursor(&self, layer: &Layer) -> Vec2 {
        let (width, height) = super::objects::scale(layer.camera_scaling(), self.dimensions.load());
        let cp = self.cursor_position.load();
        vec2(cp[0] * width, cp[1] * height)
    }
    pub fn cursor_to_world(&self, layer: &Layer) -> Vec2 {
        let dims = self.dimensions.load();
        let (width, height) = super::objects::scale(layer.camera_scaling(), dims);
        let cp = self.cursor_position.load();
        let cam = layer.camera_position();
        let zoom = 1.0 / layer.zoom();
        vec2(
            cp[0] * (width * zoom) + cam[0] * 2.0,
            cp[1] * (height * zoom) + cam[1] * 2.0,
        )
    }

    pub fn shift(&self) -> bool {
        self.keyboard_modifiers.lock().shift()
    }
    pub fn ctrl(&self) -> bool {
        self.keyboard_modifiers.lock().ctrl()
    }
    pub fn alt(&self) -> bool {
        self.keyboard_modifiers.lock().alt()
    }
    pub fn logo(&self) -> bool {
        self.keyboard_modifiers.lock().logo()
    }
    pub fn cursor_inside(&self) -> bool {
        self.cursor_inside.load(Ordering::Acquire)
    }
}

impl Default for Input {
    fn default() -> Self {
        Self {
            keyboard_down: Arc::new(Mutex::new(HashSet::new())),
            keyboard_modifiers: Arc::new(Mutex::new(ModifiersState::empty())),
            mouse_down: Arc::new(Mutex::new(HashSet::new())),
            cursor_position: Arc::new(AtomicCell::new(vec2(0.0, 0.0))),
            cursor_inside: Arc::new(AtomicBool::new(false)),
            dimensions: Arc::new(AtomicCell::new((0.0, 0.0))),
        }
    }
}
