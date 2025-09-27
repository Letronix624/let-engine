//! Simple circle scene.
//!
//! # Controls
//! - Scroll: increase / decrease number of sides

use let_engine::prelude::*;

use gpu::{VulkanTypes, model::ModelId};

// Limit of corners
const MAX_SIDES: usize = 1000;

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
    let_engine::start(
        EngineSettings::default()
            .window(window_builder)
            .gpu(gpu::GpuSettings {
                present_mode: gpu::PresentMode::Fifo,
                ..Default::default()
            }),
        Game::new,
    )
    .unwrap();
}

/// Makes a game struct containing
struct Game {
    model: ModelId<Vec2>,
    sides: u32,
}

impl Game {
    /// Constructor for this scene.
    pub fn new(context: EngineContext) -> Result<Self, ()> {
        {
            let root_view = context.scene.root_view_mut();

            // next we set the view of the game scene zoomed out and not stretchy.
            root_view.transform = Transform::with_size(Vec2::splat(2.0));
            root_view.scaling = CameraScaling::Circle;
        }

        // First we get the root layer where the scene will be simulated on.
        let root_layer = context.scene.root_layer();

        // Create a "circle" model with a default amount of sides.
        let sides = 15;
        let mut circle_model = circle!(sides, BufferAccess::Pinned(PreferOperation::Write));

        // Raise maximum vertices and indices for growable model

        // 1 vertex per side including `+ 1` for the center vertex
        circle_model.set_max_vertices(MAX_SIDES + 1);
        // Each side is 1 vertex, so 3 corners.
        circle_model.set_max_indices(MAX_SIDES * 3);

        // Load circle model to the GPU.
        let circle_model = context.gpu.load_model(&circle_model).unwrap();

        let default_material = context
            .gpu
            .load_material::<Vec2>(&Material::new_default())
            .unwrap();

        let color_buffer = context
            .gpu
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
            .build(&context.gpu)
            .unwrap();

        // Makes the circle in the middle.
        let circle = ObjectBuilder::new(circle_appearance);

        // Initializes the object to the layer
        context.scene.add_object(root_layer.id(), circle).unwrap();

        Ok(Self {
            model: circle_model,
            sides,
        })
    }
}

/// Implement the Game trait into the Game struct.
impl let_engine::Game for Game {
    // Exit when the X button on the window is pressed.
    fn window(&mut self, context: EngineContext, event: WindowEvent) -> Result<(), ()> {
        match event {
            WindowEvent::CloseRequested => context.exit(),
            WindowEvent::MouseWheel(ScrollDelta::LineDelta(delta)) => {
                // Add or subtract side depending on the delta of the scroll
                if delta.y > 0.0 {
                    if self.sides < MAX_SIDES as u32 {
                        self.sides += 1;
                        log::info!("(+) Corners: {}", self.sides);
                    }
                } else if self.sides > 2 {
                    self.sides -= 1;
                    log::info!("(-) Corners: {}", self.sides);
                }

                // Generate new circle model and write it to the GPU
                let new_model = circle!(self.sides);
                // Index model from backend index implementation directly
                context.gpu[self.model].write_model(&new_model).unwrap();
            }
            _ => (),
        }
        Ok(())
    }

    // Exit when the escape key is pressed.
    fn input(&mut self, context: EngineContext, event: InputEvent) -> Result<(), ()> {
        if let InputEvent::KeyboardInput { input } = event
            && let ElementState::Pressed = input.state
            && let Key::Named(NamedKey::Escape) = input.key
        {
            context.exit();
        }
        Ok(())
    }
}
