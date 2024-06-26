//! Simple circle scene.
#[cfg(all(feature = "client", not(feature = "networking")))]
use std::sync::Arc;

#[cfg(all(feature = "client", not(feature = "networking")))]
use let_engine::prelude::*;

#[cfg(any(not(feature = "client"), feature = "networking"))]
fn main() {
    eprintln!(
        "This example requires you to have the `client` feature enabled and `networking` disabled."
    );
}

#[cfg(all(feature = "client", not(feature = "networking")))]
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
#[cfg(all(feature = "client", not(feature = "networking")))]
struct Game {
    /// the main layer, where the scene gets put inside,
    main_layer: Arc<Layer>,
    /// a variable that decides whether the program should close.
    exit: bool,
}

#[cfg(all(feature = "client", not(feature = "networking")))]
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
#[cfg(all(feature = "client", not(feature = "networking")))]
impl let_engine::Game for Game {
    async fn start(&mut self) {
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
    async fn event(&mut self, event: Event) {
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
