use winit::event::{ElementState, Event, ModifiersState, WindowEvent};
pub use winit::event::{MouseButton, VirtualKeyCode};

pub use hashbrown::HashSet;

pub struct Input {
    //pressed mouse buttons
    mouse_down: HashSet<MouseButton>,
    //pressed keyboard buttons
    keyboard_down: HashSet<u32>,
    //pressed keyboard modifiers
    keyboard_modifiers: ModifiersState,
    //mouse position
    //mouse to layer space position
}

impl Input {
    pub fn new() -> Self {
        Self {
            mouse_down: HashSet::new(),
            keyboard_down: HashSet::new(),
            keyboard_modifiers: ModifiersState::empty(),
        }
    }
    pub fn update<T: 'static>(&mut self, event: &Event<T>) {
        if let Event::WindowEvent { event, .. } = event {
            match event {
                WindowEvent::KeyboardInput { input, .. } => {
                    if input.state == ElementState::Pressed {
                        self.keyboard_down.insert(input.scancode);
                    } else {
                        self.keyboard_down.remove(&input.scancode);
                    }
                }
                WindowEvent::ModifiersChanged(modifiers) => {
                    self.keyboard_modifiers = *modifiers;
                }
                WindowEvent::MouseInput { state, button, .. } => {
                    if state == &ElementState::Pressed {
                        self.mouse_down.insert(button.clone());
                    } else {
                        self.mouse_down.remove(button);
                    }
                }
                _ => (),
            }
        }
    }

    pub fn is_down(&self, key: &u32) -> bool {
        self.keyboard_down.contains(key)
    }
    pub fn mouse_down(&self, button: &MouseButton) -> bool {
        self.mouse_down.contains(button)
    }

    pub fn shift(&self) -> bool {
        self.keyboard_modifiers.shift()
    }
    pub fn ctrl(&self) -> bool {
        self.keyboard_modifiers.ctrl()
    }
    pub fn alt(&self) -> bool {
        self.keyboard_modifiers.alt()
    }
    pub fn logo(&self) -> bool {
        self.keyboard_modifiers.logo()
    }
}
