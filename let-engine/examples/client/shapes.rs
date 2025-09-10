//! 3 Shapes, triangle, square and circle.

use graphics::VulkanTypes;
use let_engine::prelude::{graphics::buffer::BufferId, *};
use let_engine_core::circle;

fn main() {
    // Log messages
    simple_logger::SimpleLogger::new()
        .with_level(log::LevelFilter::Debug)
        .init()
        .unwrap();

    let window_builder = WindowBuilder::new()
        .inner_size(uvec2(1500, 535))
        .resizable(false)
        .title(env!("CARGO_CRATE_NAME"));

    let_engine::start(
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
    color_buffer: BufferId<Color>,
    view_cycle: usize,

    triangle: ObjectId,
    square: ObjectId,
    circle: ObjectId,
}

impl Game {
    /// Constructor for this scene.
    pub fn new(context: EngineContext) -> Self {
        {
            let root_view = context.scene.root_view_mut();
            *root_view.camera_mut() = Transform::with_size(Vec2::splat(1.0 / 500.0));
            root_view.set_scaling(CameraScaling::Expand);
        }

        // All shapes are going to share the same material and color.
        let default_material = context
            .graphics
            .load_material::<Vec2>(&Material::new_default())
            .unwrap();

        let builder = AppearanceBuilder::<VulkanTypes>::default().material(default_material);

        // Shape 1: Triangle
        let triangle_model = context.graphics.load_model(&model!(triangle)).unwrap();
        let triangle_buffer = context
            .graphics
            .load_buffer(&Buffer::from_data(
                buffer::BufferUsage::Uniform,
                BufferAccess::Fixed,
                Color::RED,
            ))
            .unwrap();
        let triangle_appearance = builder
            .clone()
            .model(triangle_model)
            .descriptors(&[
                (Location::new(0, 0), Descriptor::Mvp),
                (Location::new(1, 0), Descriptor::buffer(triangle_buffer)),
            ])
            .build(&context.graphics)
            .unwrap();
        let mut triangle = ObjectBuilder::new(triangle_appearance);
        triangle.transform.position = vec2(-2.0, 0.21); // move triangle to the left
        let triangle = context
            .scene
            .add_object(context.scene.root_layer_id(), triangle)
            .unwrap();

        // Shape 2: Square
        let square_model = context.graphics.load_model(&model!(square)).unwrap();
        let square_buffer = context
            .graphics
            .load_buffer(&Buffer::from_data(
                buffer::BufferUsage::Uniform,
                BufferAccess::Fixed,
                Color::GREEN,
            ))
            .unwrap();
        let square_appearance = builder
            .clone()
            .model(square_model)
            .descriptors(&[
                (Location::new(0, 0), Descriptor::Mvp),
                (Location::new(1, 0), Descriptor::buffer(square_buffer)),
            ])
            .build(&context.graphics)
            .unwrap();
        let square = ObjectBuilder::new(square_appearance);
        let square = context
            .scene
            .add_object(context.scene.root_layer_id(), square)
            .unwrap();

        // Shape 3: Circle
        let circle_model = context.graphics.load_model(&circle!(40)).unwrap();
        let circle_buffer = context
            .graphics
            .load_buffer(&Buffer::from_data(
                buffer::BufferUsage::Uniform,
                // We will only write to this object. 2 buffers is sufficient for small data
                BufferAccess::RingBuffer { buffers: 2 },
                Color::BLUE,
            ))
            .unwrap();
        let circle_appearance = builder
            .clone()
            .model(circle_model)
            .descriptors(&[
                (Location::new(0, 0), Descriptor::Mvp),
                (Location::new(1, 0), Descriptor::buffer(circle_buffer)),
            ])
            .build(&context.graphics)
            .unwrap();
        let mut circle = ObjectBuilder::new(circle_appearance);
        circle.transform.position.x = 2.0; // move circle to the right
        let circle = context
            .scene
            .add_object(context.scene.root_layer_id(), circle)
            .unwrap();

        Self {
            color_buffer: circle_buffer,
            view_cycle: 0,

            triangle,
            square,
            circle,
        }
    }
}

/// Implement the Game trait into the Game struct.
impl let_engine::Game for Game {
    // Exit when the X button on the window is pressed.
    fn window(&mut self, context: EngineContext, event: events::WindowEvent) {
        if let WindowEvent::CloseRequested = event {
            context.exit();
        }
    }

    fn input(&mut self, context: EngineContext, event: events::InputEvent) {
        if let InputEvent::KeyboardInput { input } = event
            && let ElementState::Pressed = input.state
        {
            match input.key {
                // Exit when the escape key is pressed.
                Key::Named(NamedKey::Escape) => {
                    context.exit();
                }
                Key::Named(NamedKey::Space) => {
                    self.view_cycle = (self.view_cycle + 1) % 4;
                    log::info!("Mode: {}", self.view_cycle + 1);

                    match self.view_cycle {
                        0 => {
                            context
                                .scene
                                .object_mut(self.circle)
                                .unwrap()
                                .appearance
                                .set_visible(true);
                        }
                        1 => {
                            context
                                .scene
                                .object_mut(self.triangle)
                                .unwrap()
                                .appearance
                                .set_visible(false);
                        }
                        2 => {
                            context
                                .scene
                                .object_mut(self.triangle)
                                .unwrap()
                                .appearance
                                .set_visible(true);
                            context
                                .scene
                                .object_mut(self.square)
                                .unwrap()
                                .appearance
                                .set_visible(false);
                        }
                        3 => {
                            context
                                .scene
                                .object_mut(self.square)
                                .unwrap()
                                .appearance
                                .set_visible(true);
                            context
                                .scene
                                .object_mut(self.circle)
                                .unwrap()
                                .appearance
                                .set_visible(false);
                        }
                        _ => unreachable!(),
                    }
                }
                _ => (),
            }
        }
    }

    // Gradually change color of circle
    fn update(&mut self, context: EngineContext) {
        let buffer = context.graphics.buffer(self.color_buffer).unwrap();
        buffer
            .write_data(|w| {
                *w = w.lerp(
                    Color::from_rgb(1.0, 0.3, 0.5),
                    (context.time.delta_time() * 0.04) as f32,
                )
            })
            .unwrap();
    }
}
