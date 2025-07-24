//! Texture test featuring my cat Rusty.
//!
//! Press space to bitshift random pixels to make an interesting effect.

use graphics::{buffer::GpuBuffer, material::GpuMaterial, model::GpuModel, VulkanTypes};
use image::ImageBuffer;
use let_engine::prelude::graphics::texture::GpuTexture;
use let_engine::prelude::*;

static RES: UVec2 = uvec2(1122, 821);

fn main() {
    // Log messages
    simple_logger::SimpleLogger::new()
        .with_level(log::LevelFilter::Debug)
        .init()
        .unwrap();

    // First you make a builder containing the description of the window.

    let window_builder = WindowBuilder::new()
        .inner_size(RES)
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
    texture: GpuTexture,
}

impl Game {
    /// Constructor for this scene.
    pub fn new(context: &EngineContext) -> Self {
        // First we get the root layer where the scene will be simulated on.
        let root_layer = context.scene.root_layer().clone();

        // The view will exist as long as this variable is kept. Dropping this eliminates the view.
        let root_view = context.scene.root_view();

        // next we set the view of the game scene zoomed out and not stretchy.
        root_view.set_scaling(CameraScaling::Expand);

        // A square model with textured vertices.
        let model = GpuModel::new(&Model::new_indexed(
            vec![
                tvert(1.0, 1.0, 1.0, 1.0),
                tvert(1.0, -1.0, 1.0, -1.0),
                tvert(-1.0, 1.0, -1.0, 1.0),
                tvert(-1.0, -1.0, -1.0, -1.0),
            ],
            vec![0, 1, 2, 2, 1, 3],
            BufferAccess::Fixed,
        ))
        .unwrap();

        let texture = Texture::from_bytes(
            include_bytes!("../assets/example-texture.png").to_vec(),
            ImageFormat::Png,
            TextureSettingsBuilder::default()
                .format(Format::Rgba8Unorm)
                .access_pattern(BufferAccess::Staged) // `BufferAccess::Staged` to make the texture mutable
                .unwrap()
                .build()
                .unwrap(),
        )
        .unwrap();

        // Load the texture to the GPU
        let gpu_texture = GpuTexture::new(&texture).unwrap();

        let default_material = GpuMaterial::new_default_textured().unwrap();

        let color_buffer = GpuBuffer::new(&Buffer::from_data(
            buffer::BufferUsage::Uniform,
            BufferAccess::Fixed,
            Color::WHITE,
        ))
        .unwrap();

        let dim = texture.dimensions().extent();

        let appearance = AppearanceBuilder::<VulkanTypes>::default()
            .model(model)
            .material(default_material)
            .transform(Transform::with_size(vec2(dim[0] as f32, dim[1] as f32)))
            .descriptors(&[
                (Location::new(0, 0), Descriptor::Mvp),
                (
                    Location::new(1, 0),
                    Descriptor::buffer(color_buffer.clone()),
                ),
                (
                    Location::new(2, 0),
                    Descriptor::Texture(gpu_texture.clone()),
                ),
            ])
            .build()
            .unwrap();

        let object = NewObject::new(appearance);

        // Initializes the object to the layer
        object.init(&root_layer).unwrap();

        Self {
            texture: gpu_texture,
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
                    // Edit texture when space is pressed.
                    Key::Named(NamedKey::Space) => {
                        // Write data to the texture
                        self.texture
                            .write_data(|data| {
                                let mut buffer: ImageBuffer<image::Rgba<u8>, &mut [u8]> =
                                    ImageBuffer::from_raw(RES.x, RES.y, data).unwrap();

                                const PIXELS: usize = 100000;

                                log::info!("Shifting {PIXELS} pixels");
                                for _ in 0..PIXELS {
                                    let c = uvec2(
                                        rand::random_range(0..RES.x),
                                        rand::random_range(0..RES.y),
                                    );

                                    let pixel = buffer.get_pixel_mut(c.x, c.y);
                                    pixel.0[0..2].iter_mut().for_each(|rgb: &mut u8| {
                                        *rgb = rgb.rotate_left(1);
                                    });
                                }
                            })
                            .unwrap();
                    }
                    _ => (),
                }
            }
        }
    }
}
