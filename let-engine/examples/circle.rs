//! Simple circle scene.
#[cfg(feature = "client")]
use std::sync::Arc;

#[cfg(feature = "client")]
use let_engine::prelude::*;

#[cfg(not(feature = "client"))]
fn main() {
    eprintln!("This example requires you to have the `client` feature enabled.");
}

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

    // Here it initializes the game struct to be used with the engine run method.
    let game = Game::new();

    // Runs the game engine and makes a window.
    engine.start(game);
}

/// Makes a game struct containing
#[cfg(feature = "client")]
struct Game {
    /// the main layer, where the scene gets put inside,
    main_layer: Arc<Layer>,
    /// a variable that decides whether the program should close.
    exit: bool,
}

#[cfg(feature = "client")]
impl Game {
    /// Constructor for this scene.
    pub fn new() -> Self {
        Self {
            // Makes a base layer where you place your scene into.
            main_layer: SCENE.new_layer(),
            exit: false,
        }
    }
}

/// Implement the Game trait into the Game struct.
#[cfg(feature = "client")]
impl let_engine::Game for Game {
    fn start(&mut self) {
        // Makes the view zoomed out and not stretchy.
        self.main_layer.set_camera_settings(CameraSettings {
            zoom: 0.5,
            mode: CameraScaling::Expand,
        });
        // Makes the circle in the middle.
        let mut circle = NewObject::new();
        // Loads a circle model into the engine and sets the appearance of this object to it.
        circle
            .appearance
            .set_model(Some(Model::Custom(
                ModelData::new(make_circle!(30)).unwrap(),
            )))
            .unwrap();
        // Initializes the object to the layer
        circle.init(&self.main_layer).unwrap();
    }
    fn event(&mut self, event: Event) {
        match event {
            // Exit when the X button is pressed.
            Event::Window(WindowEvent::CloseRequested) => {
                self.exit = true;
            }
            Event::Input(InputEvent::KeyboardInput { input }) => {
                if input.state == ElementState::Pressed {
                    if let Key::Named(NamedKey::Escape) = input.key {
                        // Exit when the escape key is pressed.
                        self.exit = true;
                    }
                }
            }
            _ => (),
        };
    }
    /// Exits the program in case `self.exit` is true.
    fn exit(&self) -> bool {
        self.exit
    }
}
