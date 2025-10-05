//! The default input system by the engine.

use let_engine_core::{backend::gpu::Loaded, camera::CameraScaling, scenes::LayerView};
use std::collections::HashSet;
pub use winit::event::MouseButton;
use winit::event::{ElementState, WindowEvent};
pub use winit::keyboard::*;

use glam::f32::{Vec2, vec2};

/// Holds the input information to be used in game.
///
/// Updates each frame.
pub struct Input {
    //pressed keyboard keycodes.
    keys_down: HashSet<Key>,
    //pressed keyboard modifiers
    keyboard_modifiers: ModifiersState,
    //pressed mouse buttons
    mouse_down: HashSet<MouseButton>,
    //mouse position
    cursor_position: Vec2,
    cursor_inside: bool,
    //dimensions of the window
    dimensions: Vec2,
}

impl Input {
    pub(crate) fn new() -> Self {
        Self {
            keys_down: HashSet::new(),
            keyboard_modifiers: ModifiersState::empty(),
            mouse_down: HashSet::new(),
            cursor_position: vec2(0.0, 0.0),
            cursor_inside: false,
            dimensions: vec2(0.0, 0.0),
        }
    }
    /// Updates the input with the event.
    pub(crate) fn update(&mut self, event: &WindowEvent) {
        match event {
            WindowEvent::Resized(size) => {
                self.dimensions = vec2(size.width as f32, size.height as f32);
            }
            WindowEvent::KeyboardInput { event, .. } => {
                if event.state == ElementState::Pressed {
                    self.keys_down.insert(event.logical_key.clone());
                } else {
                    self.keys_down.remove(&event.logical_key);
                }
            }
            WindowEvent::ModifiersChanged(modifiers) => {
                self.keyboard_modifiers = modifiers.state();
            }
            WindowEvent::MouseInput { state, button, .. } => {
                if state == &ElementState::Pressed {
                    self.mouse_down.insert(*button);
                } else {
                    self.mouse_down.remove(button);
                }
            }
            WindowEvent::CursorMoved { position, .. } => {
                self.cursor_position = vec2(
                    (position.x as f32 / self.dimensions.x) * 2.0 - 1.0,
                    (position.y as f32 / self.dimensions.y) * 2.0 - 1.0,
                );
            }
            WindowEvent::CursorEntered { .. } => self.cursor_inside = true,
            WindowEvent::CursorLeft { .. } => self.cursor_inside = false,
            _ => (),
        }
    }

    /// Returns true if the given keycode is pressed on the keyboard.
    pub fn key_down(&self, key: &Key) -> bool {
        self.keys_down.contains(key)
    }

    /// Returns all the pressed keys in a HashSet
    pub fn pressed_keys(&self) -> HashSet<Key> {
        self.keys_down.clone()
    }

    /// Returns true if the given mouse button is pressed.
    pub fn mouse_down(&self, button: &MouseButton) -> bool {
        self.mouse_down.contains(button)
    }

    /// Returns the cursor position going from -1.0 to 1.0 x and y.
    pub fn cursor_position(&self) -> Vec2 {
        self.cursor_position
    }

    /// Returns the cursor position going from -1.0 to 1.0 x and y scaled with the inserted layers scaling properties.
    pub fn scaled_cursor(&self, scaling: CameraScaling) -> Vec2 {
        let dimensions = scaling.scale(self.dimensions);
        let cp = self.cursor_position;
        vec2(cp[0], cp[1]) * dimensions
    }

    /// Returns the cursor position in layer world space.
    pub fn cursor_to_world<T: Loaded>(&self, view: &LayerView<T>) -> Vec2 {
        view.screen_to_world(self.cursor_position, self.dimensions)
    }

    /// Returns true if shift is pressed on the keyboard.
    pub fn shift(&self) -> bool {
        self.keyboard_modifiers.shift_key()
    }

    /// Returns true if ctrl is pressed on the keyboard.
    pub fn ctrl(&self) -> bool {
        self.keyboard_modifiers.control_key()
    }

    /// Returns true if alt is pressed on the keyboard.
    pub fn alt(&self) -> bool {
        self.keyboard_modifiers.alt_key()
    }

    /// Returns true if the super key is pressed on the keyboard.
    ///
    /// Super key also means "Windows" key on Windows or "Command" key on Mac.
    pub fn super_key(&self) -> bool {
        self.keyboard_modifiers.super_key()
    }

    /// Returns true if the cursor is located in the window.
    pub fn cursor_inside(&self) -> bool {
        self.cursor_inside
    }
}

impl Default for Input {
    fn default() -> Self {
        Self::new()
    }
}
