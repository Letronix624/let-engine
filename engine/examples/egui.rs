//! Simple circle scene with egui.
//!
//! Requires the egui feature to be enabled.
//! Runnable with `cargo run --features=egui --example egui`
#![allow(unused_imports)]
use std::{cell::RefCell, sync::Arc};

use egui_demo_lib::DemoWindows;
use let_engine::prelude::*;

thread_local! {
    static DEMO_APP: RefCell<DemoWindows> = RefCell::new(DemoWindows::default());
}

#[cfg(any(not(feature = "egui"), not(feature = "client")))]
fn main() {
    eprintln!("This example requires you to have the `egui` and `client` feature enabled.");
}

#[cfg(feature = "egui")]
#[cfg(feature = "client")]
fn main() {
    // First you make a builder containing the description of the window.
    let window_builder = WindowBuilder::new().inner_size(vec2(1280.0, 720.0));
    // Then you start the engine allowing you to load resources and layers.
    let engine = Engine::new(
        EngineSettingsBuilder::default()
            .window_settings(window_builder)
            .build()
            .unwrap(),
    )
    .unwrap();

    let game = Game::new();

    // Runs the game
    engine.start(game);
}

#[cfg(feature = "egui")]
struct Game {
    layer: Arc<Layer>,
    exit: bool,
}

#[cfg(feature = "egui")]
impl Game {
    pub fn new() -> Self {
        Self {
            // Makes a base layer where you place your scene into.
            layer: SCENE.new_layer(),
            exit: false,
        }
    }
}

#[cfg(feature = "egui")]
impl let_engine::Game for Game {
    fn start(&mut self) {
        // Makes the view zoomed out and not stretchy.
        self.layer.set_camera_settings(CameraSettings {
            zoom: 0.5,
            mode: CameraScaling::Linear,
        });

        // Makes the circle in the middle.
        let mut circle = Object::new();
        // Loads a circle model into the engine and sets the appearance of this object to it.
        circle
            .appearance
            .set_model(Model::Custom(ModelData::new(make_circle!(30)).unwrap()));
        // Initializes the object to the layer
        circle.init(&self.layer);
    }
    fn event(&mut self, event: events::Event) {
        match event {
            // Exit when the X button is pressed.
            Event::Window(WindowEvent::CloseRequested) => {
                self.exit = true;
            }
            Event::Input(InputEvent::KeyboardInput { input }) => {
                if input.state == ElementState::Pressed {
                    if let Some(VirtualKeyCode::Escape) = input.keycode {
                        // Exit when the escape key is pressed.
                        self.exit = true;
                    }
                }
            }
            Event::Egui(ctx) => {
                // Use the egui context to make a gui.
                DEMO_APP.with_borrow_mut(|app| app.ui(&ctx));
            }
            _ => (),
        };
    }
    fn exit(&self) -> bool {
        self.exit
    }
}
