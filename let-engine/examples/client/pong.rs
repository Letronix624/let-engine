//! Simple pong game
//!
//! # Controls
//! - w: left paddle up
//! - s: left paddle down
//!
//! - k: left paddle up
//! - j: left paddle down

use audio::{
    gen_square_wave,
    sound::static_sound::{StaticSoundData, StaticSoundSettings},
};
use gpu::VulkanTypes;

use let_engine::prelude::*;
use let_engine_core::backend::audio::{AudioInterface, DefaultAudioBackend};

use let_engine_widgets::labels::{Label, LabelCreateInfo, Labelifier};

use std::{
    f64::consts::{FRAC_PI_2, FRAC_PI_4},
    time::{Duration, SystemTime},
};

// A const that contains the constant window resolution.
const RESOLUTION: UVec2 = uvec2(800, 600);

struct PongBackends;

impl core_backend::Backends for PongBackends {
    type Gpu = gpu::DefaultGpuBackend;

    type Kira = DefaultAudioBackend;

    type Networking = ();
}

type EngineContext<'a> = let_engine::EngineContext<'a, (), PongBackends>;

fn main() {
    simple_logger::SimpleLogger::new()
        .with_level(log::LevelFilter::Debug)
        .init()
        .unwrap();

    // Describing the window.
    let window_builder = WindowBuilder::default()
        .resizable(false)
        .inner_size(RESOLUTION)
        .title(env!("CARGO_CRATE_NAME"));
    // Initialize the engine.
    let_engine::start::<Game, (), PongBackends>(
        EngineSettings::default()
            .window(window_builder)
            .tick_system(
                TickSettingsBuilder::default()
                    .tick_wait(Duration::from_secs_f64(1.0 / 20.0)) // 20 ticks per second
                    .build()
                    .unwrap(),
            ),
        Game::new,
    )
    .unwrap();
}

struct Game {
    labelifier: Labelifier<VulkanTypes>,

    left_paddle: Paddle,
    right_paddle: Paddle,
    ball: Ball,

    left_score_label: Label<VulkanTypes>,
    right_score_label: Label<VulkanTypes>,
}

impl Game {
    pub fn new(mut context: EngineContext) -> Result<Game, ()> {
        // First we create a ui layer, the place where the text and middle line will be.
        let ui_layer = context
            .scene
            .add_layer(context.scene.root_layer_id())
            .unwrap();

        // next we set the view of the game scene to -1 to 1 max
        {
            let root_view = context.scene.root_view_mut();
            root_view.scaling = CameraScaling::Box;
        }

        // When making UI, a recommended scaling mode is `Expand`, because it makes sure the UI is
        // the same size no matter how the window is scaled. Size of the transform is zoom here.
        let _ui_view = context
            .scene
            .add_view(
                ui_layer,
                Transform::default(),
                CameraScaling::Expand,
                DrawTarget::Window,
                None,
            )
            .unwrap();

        // The view will exist as long as this variable is kept. Dropping this eliminates the view.

        // Make left paddle controlled with W for up and S for down.
        let left_paddle = Paddle::new(
            (Key::Character("w".into()), Key::Character("s".into())),
            -0.95,
            &mut context,
        );
        // The right paddle controlled with J and K. Weird controls, but 60% keyboard friendly
        let right_paddle = Paddle::new(
            (Key::Character("k".into()), Key::Character("j".into())),
            0.95,
            &mut context,
        );

        // Spawns a ball in the middle.
        let ball = Ball::new(&mut context);

        let mut labelifier = Labelifier::new(&context.gpu).unwrap();

        // Loading the font for the score.
        let font = labelifier
            .font_from_slice(include_bytes!("../assets/Px437_CL_Stingray_8x16.ttf"))
            .expect("Font is invalid.");

        // Making a default label for the left side.
        let left_score_label = Label::new(
            LabelCreateInfo::default()
                .text("0")
                .align(Direction::No)
                .transform(Transform::with_position(vec2(
                    RESOLUTION.x as f32 * -0.55,
                    80.0,
                )))
                .extent(RESOLUTION / uvec2(2, 1))
                .scale(Vec2::splat(50.0))
                .font(font),
            &mut labelifier,
            &context.gpu,
        )
        .unwrap();

        // initialize this one to the ui
        context.scene.add_object(
            ui_layer,
            ObjectBuilder::new(left_score_label.appearance().build(&context.gpu).unwrap()),
        );

        // Making a default label for the right side.
        let right_score_label = Label::new(
            LabelCreateInfo::default()
                .text("0")
                .align(Direction::Nw)
                .transform(Transform::with_position(vec2(
                    RESOLUTION.x as f32 * 0.55,
                    80.0,
                )))
                .extent(RESOLUTION / uvec2(2, 1))
                .scale(Vec2::splat(50.0))
                .font(font),
            &mut labelifier,
            &context.gpu,
        )
        .unwrap();

        context
            .scene
            .add_object(
                ui_layer,
                ObjectBuilder::new(right_score_label.appearance().build(&context.gpu).unwrap()),
            )
            .unwrap();

        // Submit label creation task in the end.
        labelifier.update(&context.gpu).unwrap();

        /* Line in the middle */

        // Make a custom model that is just 2 lines.
        let vertices = vec![
            vec2(0.0, RESOLUTION.y as f32 * 0.8),
            vec2(0.0, RESOLUTION.y as f32 * 0.1),
            vec2(0.0, RESOLUTION.y as f32 * -0.1),
            vec2(0.0, RESOLUTION.y as f32 * -0.8),
        ];

        let middle_model = context.gpu.load_model(&model!(vertices)).unwrap();

        // A description of how the line should look like.
        let line_material_settings = MaterialSettingsBuilder::default()
            .line_width(10.0)
            .topology(Topology::LineList)
            .build()
            .unwrap();

        let line_material = context
            .gpu
            .load_material::<Vec2>(&Material::new(
                line_material_settings,
                GraphicsShaders::new_default(),
            ))
            .unwrap();

        // The buffer is a Fixed Uniform here, because it's small and will never change.
        let middle_line_color = context
            .gpu
            .load_buffer(&Buffer::from_data(
                BufferUsage::Uniform,
                BufferAccess::Fixed,
                Color::WHITE,
            ))
            .unwrap();

        let middle_line_appearance = AppearanceBuilder::default()
            .material(line_material)
            .model(middle_model)
            .descriptors(&[
                (Location::new(0, 0), Descriptor::Mvp),
                (Location::new(1, 0), Descriptor::buffer(middle_line_color)),
            ])
            .build(&context.gpu)
            .unwrap();

        // Add the line to the ui layer
        context
            .scene
            .add_object(ui_layer, ObjectBuilder::new(middle_line_appearance))
            .unwrap();

        Ok(Self {
            labelifier,

            left_paddle,
            right_paddle,
            ball,

            left_score_label,
            right_score_label,
        })
    }
}

impl let_engine::Game<PongBackends> for Game {
    fn update(&mut self, mut context: EngineContext) -> Result<(), ()> {
        // run the update functions of the paddles.
        self.left_paddle.update(&mut context);
        self.right_paddle.update(&mut context);

        // If anyone has won after the ball has updated, modify the score counter
        if self.ball.update(context.time, &mut context) {
            // Update score labels
            self.left_score_label
                .update_text(format!("{}", self.ball.wins[0]))
                .unwrap();
            self.right_score_label
                .update_text(format!("{}", self.ball.wins[1]))
                .unwrap();

            // Update the labelifier each frame to make the score update.
            self.labelifier.update(&context.gpu).unwrap();
        };
        Ok(())
    }

    // Exit when the X button is pressed.
    fn window(&mut self, context: EngineContext, event: events::WindowEvent) -> Result<(), ()> {
        if let WindowEvent::CloseRequested = event {
            context.exit();
        }
        Ok(())
    }

    fn input(&mut self, context: EngineContext, event: events::InputEvent) -> Result<(), ()> {
        if let InputEvent::KeyboardInput { input } = event
            && input.state == ElementState::Pressed
        {
            match input.key {
                // Exit when the escape key is pressed.
                Key::Named(NamedKey::Escape) => context.exit(),
                Key::Character(e) => {
                    if e == *"e" {
                        // Troll the right paddle
                        self.right_paddle.shrink(context.scene);
                    } else if e == *"q" {
                        // Grow and show the right paddle whos boss.
                        self.left_paddle.grow(context.scene);
                    }
                }
                // Oh, so the left paddle thinks it's funny. I'll show it.
                Key::Named(NamedKey::ArrowLeft) => {
                    self.left_paddle.shrink(context.scene);
                }
                // I can grow too, noob.
                Key::Named(NamedKey::ArrowRight) => {
                    self.right_paddle.grow(context.scene);
                }
                _ => (),
            }
        }
        Ok(())
    }
}

struct Paddle {
    controls: (Key, Key), //up/down
    object: ObjectId,
    height: f32,
}

impl Paddle {
    pub fn new(controls: (Key, Key), x: f32, context: &mut EngineContext) -> Self {
        // Next we describe the appearance of the paddle.

        // Here we make the pedal square and give it a default material
        let model = context.gpu.load_model(&model!(square)).unwrap();
        let material = context
            .gpu
            .load_material::<Vec2>(&Material::new_default())
            .unwrap();

        // Make the paddle white
        let buffer = context
            .gpu
            .load_buffer(&Buffer::from_data(
                BufferUsage::Uniform,
                BufferAccess::Fixed,
                Color::WHITE,
            ))
            .unwrap();

        let appearance = AppearanceBuilder::default()
            .model(model)
            .material(material)
            .descriptors(&[
                (Location::new(0, 0), Descriptor::Mvp),
                (Location::new(1, 0), Descriptor::buffer(buffer)),
            ])
            .build(&context.gpu)
            .unwrap();

        let height = 0.05;
        let mut object = ObjectBuilder::new(appearance);
        object.transform = Transform {
            position: vec2(x, 0.0),
            size: vec2(0.015, height),
            ..Default::default()
        };

        // Make a collider that resembles the form of the paddle.
        object.set_collider(Some(ColliderBuilder::square(0.015, height).build()));

        // Initialize the object to the given layer.
        let object = context
            .scene
            .add_object(context.scene.root_layer_id(), object)
            .unwrap();
        Self {
            controls,
            object,
            height,
        }
    }
    pub fn update(&mut self, context: &mut EngineContext) {
        // Turn the `True` and `False` of the input.key_down() into 1, 0 or -1.
        let shift = context.input.key_down(&self.controls.0) as i32
            - context.input.key_down(&self.controls.1) as i32;

        let object = context.scene.object_mut(self.object).unwrap();

        // Shift Y and clamp it between 0.51 so it doesn't go out of bounds.
        let y = &mut object.transform.position.y;
        *y -= shift as f32 * context.time.delta_time() as f32 * 1.3;
        *y = y.clamp(-0.70, 0.70);
    }
    /// To troll the opponent.
    pub fn shrink(&mut self, scene: &mut Scene<VulkanTypes>) {
        self.resize(-0.001, scene);
    }
    /// GROW BACK!
    pub fn grow(&mut self, scene: &mut Scene<VulkanTypes>) {
        self.resize(0.001, scene);
    }
    fn resize(&mut self, difference: f32, scene: &mut Scene<VulkanTypes>) {
        self.height += difference;
        self.height = self.height.clamp(0.001, 0.7);
        let object = scene.object_mut(self.object).unwrap();
        object.transform.size.y = self.height;
        object.set_collider(Some(ColliderBuilder::square(0.015, self.height).build()));
    }
}

struct Ball {
    object_id: ObjectId,
    direction: Vec2,
    speed: f32,
    new_round: SystemTime,
    pub wins: [u32; 2],
    bounce_sound: StaticSoundData,
}

/// Ball logic.
impl Ball {
    pub fn new(context: &mut EngineContext) -> Self {
        let lifetime = SystemTime::now();

        let model = context.gpu.load_model(&model!(square)).unwrap();
        let material = context
            .gpu
            .load_material::<Vec2>(&Material::new_default())
            .unwrap();

        // Make the ball white
        let buffer = context
            .gpu
            .load_buffer(&Buffer::from_data(
                BufferUsage::Uniform,
                BufferAccess::Fixed,
                Color::WHITE,
            ))
            .unwrap();

        let appearance = AppearanceBuilder::default()
            .model(model)
            .material(material)
            .descriptors(&[
                (Location::new(0, 0), Descriptor::Mvp),
                (Location::new(1, 0), Descriptor::buffer(buffer)),
            ])
            .build(&context.gpu)
            .unwrap();

        let mut object = ObjectBuilder::new(appearance);
        object.transform.size = vec2(0.015, 0.015);

        let object = context
            .scene
            .add_object(context.scene.root_layer_id(), object)
            .unwrap();

        // make a sound to play when bouncing.
        let bounce_sound = gen_square_wave(
            777.0,
            Duration::from_millis(30),
            StaticSoundSettings::new().volume(-10.0),
        );

        Self {
            object_id: object,
            direction: vec2(1.0, 0.0),
            speed: 1.1,
            new_round: lifetime,
            wins: [0; 2],
            bounce_sound,
        }
    }

    /// Updates the ball and returns true if the ball has touched the wall
    pub fn update(&mut self, time: &Time, context: &mut EngineContext) -> bool {
        // Wait one second before starting the round.
        if self.new_round.elapsed().unwrap().as_secs() > 0 {
            let object = context.scene.object(self.object_id).unwrap();
            let layer = context.scene.root_layer();
            let view = context.scene.root_view();

            let position = object.transform.position;

            // Check if the ball is touching a paddle.
            let touching_paddle = !layer
                .intersections_with_shape(Shape::square(0.02, 0.02), (position, 0.0))
                .is_empty();

            let dimensions = context.window().unwrap().inner_size();

            // Check if the top side or bottom side are touched by checking if the ball position is below or above the screen edges +- the ball size.
            let touching_floor = position.y
                < view
                    .screen_to_world(vec2(0.0, -1.0), dimensions.as_vec2())
                    .y
                    + 0.015;
            let touching_roof =
                position.y > view.screen_to_world(vec2(0.0, 1.0), dimensions.as_vec2()).y - 0.015;
            let touching_wall = position.x.abs() > 1.0;

            if touching_paddle
                && (self.direction.x.is_sign_negative()
                    == object.transform.position.x.is_sign_negative())
            {
                self.rebound(position.x as f64, context.audio);
                // It's getting faster with time.
                self.speed += 0.03;
            } else if touching_roof {
                self.direction.y *= -self.direction.y.signum();
            } else if touching_floor {
                self.direction.y *= self.direction.y.signum();
            } else if touching_wall {
                // Right wins increase by 1 in case the X is negative.
                if position.x.is_sign_positive() {
                    self.wins[0] += 1;
                    log::info!("Left scored!")
                } else {
                    self.wins[1] += 1;
                    log::info!("Right scored!")
                }
                self.reset(context.scene);
                return true;
            }

            // Calculate new ball position
            context
                .scene
                .object_mut(self.object_id)
                .unwrap()
                .transform
                .position += self.direction * time.delta_time() as f32 * self.speed;

            // self.bounce_sound.update(Tween::default()).unwrap();
        }

        false
    }

    fn reset(&mut self, scene: &mut Scene<VulkanTypes>) {
        self.new_round = SystemTime::now();

        let object = scene.object_mut(self.object_id).unwrap();
        object.transform.position = vec2(0.0, 0.0);

        self.direction = Self::random_direction();
        self.speed = 1.1;
    }

    fn rebound(&mut self, x: f64, audio_interface: &AudioInterface<DefaultAudioBackend>) {
        // Random 0.0 to 1.0 value. Some math that makes a random direction.
        let random = (rand::random_range(0.0..1.0) as f64).copysign(-x);
        let direction = random.mul_add(FRAC_PI_2, FRAC_PI_4.copysign(-x)) - FRAC_PI_2;

        self.direction = Vec2::from_angle(direction as f32).normalize();

        // play the bounce sound.
        audio_interface.play(self.bounce_sound.clone()).unwrap();
    }

    fn random_direction() -> Vec2 {
        let random = rand::random_range(-1.0..1.0) as f64;

        let direction = random.mul_add(FRAC_PI_2, FRAC_PI_4.copysign(random)) - FRAC_PI_2;
        Vec2::from_angle(direction as f32).normalize()
    }
}
