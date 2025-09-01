//! Simple circle scene.
//!
//! # Controls
//! - Scroll: change number of vertices

use graphics::VulkanTypes;
use let_engine::prelude::{graphics::model::ModelId, *};
use let_engine_core::circle;

const MAX_DEGREE: usize = 1000;

fn main() {
    // Log messages
    simple_logger::SimpleLogger::new()
        .with_level(log::LevelFilter::Debug)
        .init()
        .unwrap();

    // First you make a builder containing the description of the window.

    let window_builder = WindowBuilder::new()
        .inner_size(uvec2(1280, 720))
        .title(env!("CARGO_CRATE_NAME"));

    // Now we run the engine
    Engine::<Game>::start(
        Game::new,
        EngineSettings::default()
            .window(window_builder)
            .graphics(graphics::Graphics {
                present_mode: graphics::PresentMode::Fifo,
                ..Default::default()
            }),
    )
    .unwrap();
}

/// Makes a game struct containing
struct Game {
    model: ModelId<Vec2>,
    degree: u32,
}

impl Game {
    /// Constructor for this scene.
    pub fn new(context: EngineContext) -> Self {
        {
            let root_view = context.scene.root_view_mut();

            // next we set the view of the game scene zoomed out and not stretchy.
            *root_view.camera_mut() = Transform::with_size(Vec2::splat(2.0));
            root_view.set_scaling(CameraScaling::Circle);
        }

        // First we get the root layer where the scene will be simulated on.
        let root_layer = context.scene.root_layer();

        // Create a "circle" model with a default degree (amount of corners) of 15.
        let degree = 15;
        let mut circle_model = circle!(degree, BufferAccess::Staged);

        // Raise maximum vertices and indices for growable model
        circle_model.set_max_vertices(MAX_DEGREE + 1);
        circle_model.set_max_indices(MAX_DEGREE * 3);

        // Load circle model to the GPU.
        let circle_model = context.graphics.load_model(&circle_model).unwrap();

        let default_material = context
            .graphics
            .load_material::<Vec2>(&Material::new_default())
            .unwrap();

        let color_buffer = context
            .graphics
            .load_buffer(&Buffer::from_data(
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
                (Location::new(1, 0), Descriptor::buffer(color_buffer)),
            ])
            .build(&context.graphics)
            .unwrap();

        // Makes the circle in the middle.
        let circle = ObjectBuilder::new(circle_appearance);

        // Initializes the object to the layer
        context.scene.add_object(root_layer.id(), circle).unwrap();

        Self {
            model: circle_model,
            degree,
        }
    }
}

/// Implement the Game trait into the Game struct.
impl let_engine::Game for Game {
    // Exit when the X button on the window is pressed.
    fn window(&mut self, context: EngineContext, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => context.exit(),
            WindowEvent::MouseWheel(ScrollDelta::LineDelta(delta)) => {
                if delta.y > 0.0 {
                    if self.degree < MAX_DEGREE as u32 {
                        self.degree += 1;
                        log::info!("(+) Corners: {}", self.degree);
                    }
                } else if self.degree > 2 {
                    self.degree -= 1;
                    log::info!("(-) Corners: {}", self.degree);
                }

                let new_model = circle!(self.degree);

                let model = context.graphics.model(self.model).unwrap();

                model.write_model(&new_model).unwrap();
            }
            _ => (),
        }
    }

    // Exit when the escape key is pressed.
    fn input(&mut self, context: EngineContext, event: InputEvent) {
        if let InputEvent::KeyboardInput { input } = event {
            if let ElementState::Pressed = input.state {
                if let Key::Named(NamedKey::Escape) = input.key {
                    context.exit();
                }
            }
        }
    }
}
