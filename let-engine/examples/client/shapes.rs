//! 3 Shapes, triangle, square and circle.

use graphics::{buffer::GpuBuffer, material::GpuMaterial, model::GpuModel, VulkanTypes};
use let_engine::prelude::*;
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
    color_buffer: GpuBuffer<Color>,
    view_cycle: usize,

    triangle: Object<VulkanTypes>,
    square: Object<VulkanTypes>,
    circle: Object<VulkanTypes>,
}

impl Game {
    /// Constructor for this scene.
    pub fn new(context: &EngineContext) -> Self {
        let root_layer = context.scene.root_layer().clone();

        let root_view = context.scene.root_view();
        root_view.set_camera(Transform::with_size(Vec2::splat(1.0 / 500.0)));
        root_view.set_scaling(CameraScaling::Expand);

        // All shapes are going to share the same material and color.
        let default_material = GpuMaterial::new_default().unwrap();

        let builder = AppearanceBuilder::<VulkanTypes>::default().material(default_material);

        // Shape 1: Triangle
        let triangle_model = GpuModel::new(&model!(triangle)).unwrap();
        let triangle_buffer = GpuBuffer::new(&Buffer::from_data(
            buffer::BufferUsage::Uniform,
            // We will only write to this object.
            BufferAccess::Pinned(PreferOperation::Write),
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
            .build()
            .unwrap();
        let mut triangle = NewObject::new(triangle_appearance);
        triangle.transform.position = vec2(-2.0, 0.21); // move triangle to the left
        let triangle = triangle.init(&root_layer).unwrap();

        // Shape 2: Square
        let square_model = GpuModel::new(&model!(square)).unwrap();
        let square_buffer = GpuBuffer::new(&Buffer::from_data(
            buffer::BufferUsage::Uniform,
            // We will only write to this object.
            BufferAccess::Pinned(PreferOperation::Write),
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
            .build()
            .unwrap();
        let square = NewObject::new(square_appearance);
        let square = square.init(&root_layer).unwrap();

        // Shape 3: Circle
        let circle_model = GpuModel::new(&circle!(40)).unwrap();
        let circle_buffer = GpuBuffer::new(&Buffer::from_data(
            buffer::BufferUsage::Uniform,
            // We will only write to this object.
            BufferAccess::Pinned(PreferOperation::Write),
            Color::BLUE,
        ))
        .unwrap();
        let circle_appearance = builder
            .clone()
            .model(circle_model)
            .descriptors(&[
                (Location::new(0, 0), Descriptor::Mvp),
                (
                    Location::new(1, 0),
                    Descriptor::buffer(circle_buffer.clone()),
                ),
            ])
            .build()
            .unwrap();
        let mut circle = NewObject::new(circle_appearance);
        circle.transform.position.x = 2.0; // move circle to the right
        let circle = circle.init(&root_layer).unwrap();

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
    fn window(&mut self, context: &EngineContext, event: events::WindowEvent) {
        if let WindowEvent::CloseRequested = event {
            context.exit();
        }
    }

    fn input(&mut self, context: &EngineContext, event: events::InputEvent) {
        if let InputEvent::KeyboardInput { input } = event {
            if let ElementState::Pressed = input.state {
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
                                self.circle.appearance.set_visible(true);
                                self.circle.sync().unwrap();
                            }
                            1 => {
                                self.triangle.appearance.set_visible(false);
                                self.triangle.sync().unwrap();
                            }
                            2 => {
                                self.triangle.appearance.set_visible(true);
                                self.square.appearance.set_visible(false);
                                self.triangle.sync().unwrap();
                                self.square.sync().unwrap();
                            }
                            3 => {
                                self.square.appearance.set_visible(true);
                                self.circle.appearance.set_visible(false);
                                self.square.sync().unwrap();
                                self.circle.sync().unwrap();
                            }
                            _ => unreachable!(),
                        }
                    }
                    _ => (),
                }
            }
        }
    }

    fn update(&mut self, context: &EngineContext<DefaultBackends>) {
        self.color_buffer
            .write_data(|w| {
                *w = w.lerp(
                    Color::from_rgb(1.0, 0.3, 0.5),
                    (context.time.delta_time() * 0.03) as f32,
                )
            })
            .unwrap();
    }
}
