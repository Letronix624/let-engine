//! Simple circle scene.
#[cfg(feature = "client")]
use std::sync::Arc;

#[cfg(feature = "client")]
use graphics::{buffer::GpuBuffer, material::GpuMaterial, model::GpuModel, VulkanTypes};
#[cfg(feature = "client")]
use let_engine::prelude::*;
use let_engine_core::make_circle;

#[cfg(not(feature = "client"))]
fn main() {
    eprintln!("This example requires you to have the `client` feature enabled.");
}

#[cfg(feature = "client")]
fn main() {
    // Log messages

    #[cfg(feature = "vulkan_debug")]
    simple_logger::SimpleLogger::new()
        .with_level(log::LevelFilter::Debug)
        .init()
        .unwrap();

    // First you make a builder containing the description of the window.

    let window_builder = WindowBuilder::new().inner_size(uvec2(1280, 720));
    // Then you start the engine allowing you to load resources and layers.
    let mut engine =
        Engine::<Game>::new(EngineSettings::default().window(window_builder).graphics(
            graphics::Graphics {
                present_mode: graphics::PresentMode::Fifo,
                ..Default::default()
            },
        ))
        .unwrap();

    // Here it initializes the game struct to be used with the engine run method,
    // runs the game engine and makes a window.
    engine.start(Game::new);
}

/// Makes a game struct containing
#[cfg(feature = "client")]
struct Game {
    /// the view perspective to draw
    _root_view: Arc<LayerView<VulkanTypes>>,
}

#[cfg(feature = "client")]
impl Game {
    /// Constructor for this scene.
    pub fn new(context: &EngineContext) -> Self {
        // First we get the root layer where the scene will be simulated on.
        let root_layer = context.scene.root_layer().clone();

        // The view will exist as long as this variable is kept. Dropping this eliminates the view.
        let root_view = context.scene.root_view();
        // next we set the view of the game scene zoomed out and not stretchy.
        root_view.set_camera(Camera {
            transform: Transform::default().size(Vec2::splat(0.5)),
            scaling: CameraScaling::Expand,
        });

        // Loads a circle model into the engine.
        let circle_model = GpuModel::new(&make_circle!(20)).unwrap();

        let default_material = GpuMaterial::new_default().unwrap();

        let color_buffer = GpuBuffer::new(Buffer::from_data(
            buffer::BufferUsage::Uniform,
            BufferAccess::Fixed,
            Color::from_rgb(1.0, 0.3, 0.5),
        ))
        .unwrap();

        let circle_appearance = AppearanceBuilder::<VulkanTypes>::default()
            .model(circle_model)
            .material(default_material)
            .descriptors(&[
                (Location::new(0, 0), Descriptor::Mvp),
                (
                    Location::new(1, 0),
                    Descriptor::buffer(color_buffer.clone()),
                ),
            ])
            .build()
            .unwrap();

        // Makes the circle in the middle.
        let circle = NewObject::new(circle_appearance);

        // Initializes the object to the layer
        circle.init(&root_layer).unwrap();

        Self {
            // color_buffer,
            // Makes a base layer where you place your scene into.
            _root_view: root_view,
        }
    }
}

/// Implement the Game trait into the Game struct.
#[cfg(feature = "client")]
impl let_engine::Game for Game {
    // Exit when the X button on the window is pressed.
    fn window(&mut self, context: &EngineContext, event: events::WindowEvent) {
        if let WindowEvent::CloseRequested = event {
            context.exit();
        }
    }

    // Exit when the escape key is pressed.
    fn input(&mut self, context: &EngineContext, event: events::InputEvent) {
        if let InputEvent::KeyboardInput { input } = event {
            if let ElementState::Pressed = input.state {
                if let Key::Named(NamedKey::Escape) = input.key {
                    context.exit();
                }
            }
        }
    }
}
