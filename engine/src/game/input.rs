//! The default input system by the engine.

use crate::objects::scenes::Layer;
use std::{
    collections::HashSet,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};
pub use winit::event::MouseButton;
use winit::event::{ElementState, Event, WindowEvent};
pub use winit::keyboard::*;

use crossbeam::atomic::AtomicCell;
use glam::f32::{vec2, Vec2};
use parking_lot::Mutex;

/// Holds the input information to be used in game.
///
/// Updates each frame.
#[derive(Clone)]
pub struct Input {
    //pressed keyboard keycodes.
    keys_down: Arc<Mutex<HashSet<Key>>>,
    //pressed keyboard modifiers
    keyboard_modifiers: Arc<Mutex<ModifiersState>>,
    //pressed mouse buttons
    mouse_down: Arc<Mutex<HashSet<MouseButton>>>,
    //mouse position
    cursor_position: Arc<AtomicCell<Vec2>>,
    cursor_inside: Arc<AtomicBool>,
    //dimensions of the window
    dimensions: Arc<AtomicCell<Vec2>>, // lazylock future
}

impl Input {
    pub(crate) fn new() -> Self {
        Self {
            keys_down: Arc::new(Mutex::new(HashSet::new())),
            keyboard_modifiers: Arc::new(Mutex::new(ModifiersState::empty())),
            mouse_down: Arc::new(Mutex::new(HashSet::new())),
            cursor_position: Arc::new(AtomicCell::new(vec2(0.0, 0.0))),
            cursor_inside: Arc::new(AtomicBool::new(false)),
            dimensions: Arc::new(AtomicCell::new(vec2(0.0, 0.0))),
        }
    }
    /// Updates the input with the event.
    pub(crate) fn update<T: 'static>(&self, event: &Event<T>, dimensions: Vec2) {
        self.dimensions.store(dimensions);
        if let Event::WindowEvent { event, .. } = event {
            match event {
                WindowEvent::KeyboardInput { event, .. } => {
                    if event.state == ElementState::Pressed {
                        self.keys_down.lock().insert(event.logical_key.clone());
                    } else {
                        self.keys_down.lock().remove(&event.logical_key);
                    }
                }
                WindowEvent::ModifiersChanged(modifiers) => {
                    *self.keyboard_modifiers.lock() = modifiers.state();
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
                        (position.x as f32 / dimensions.x) * 2.0 - 1.0,
                        (position.y as f32 / dimensions.y) * 2.0 - 1.0,
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

    /// Returns true if the given keycode is pressed on the keyboard.
    pub fn key_down(&self, key: &Key) -> bool {
        self.keys_down.lock().contains(key)
    }

    /// Returns true if the given mouse button is pressed.
    pub fn mouse_down(&self, button: &MouseButton) -> bool {
        self.mouse_down.lock().contains(button)
    }

    /// Returns the cursor position going from -1.0 to 1.0 x and y.
    pub fn cursor_position(&self) -> Vec2 {
        let cp = self.cursor_position.load();
        vec2(cp[0], cp[1])
    }

    /// Returns the cursor position going from -1.0 to 1.0 x and y scaled with the inserted layers scaling properties.
    pub fn scaled_cursor(&self, layer: &Layer) -> Vec2 {
        let dimensions = layer.camera_scaling().scale(self.dimensions.load());
        let cp = self.cursor_position.load();
        vec2(cp[0], cp[1]) * dimensions
    }

    /// Returns the cursor position in layer world space.
    pub fn cursor_to_world(&self, layer: &Layer) -> Vec2 {
        let dims = self.dimensions.load();
        let dimensions = layer.camera_scaling().scale(dims);
        let cp = self.cursor_position.load();
        let cam = layer.camera_transform().position;
        let zoom = 1.0 / layer.zoom();
        vec2(
            cp[0] * (dimensions.x * zoom) + cam[0] * 2.0,
            cp[1] * (dimensions.y * zoom) + cam[1] * 2.0,
        )
    }

    /// Returns true if shift is pressed on the keyboard.
    pub fn shift(&self) -> bool {
        self.keyboard_modifiers.lock().shift_key()
    }

    /// Returns true if ctrl is pressed on the keyboard.
    pub fn ctrl(&self) -> bool {
        self.keyboard_modifiers.lock().control_key()
    }

    /// Returns true if alt is pressed on the keyboard.
    pub fn alt(&self) -> bool {
        self.keyboard_modifiers.lock().alt_key()
    }

    /// Returns true if the super key is pressed on the keyboard.
    ///
    /// Super key also means "Windows" key on Windows or "Command" key on Mac.
    pub fn super_key(&self) -> bool {
        self.keyboard_modifiers.lock().super_key()
    }

    /// Returns true if the cursor is located in the window.
    pub fn cursor_inside(&self) -> bool {
        self.cursor_inside.load(Ordering::Acquire)
    }
}

impl Default for Input {
    fn default() -> Self {
        Self::new()
    }
}
