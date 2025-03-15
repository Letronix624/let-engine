use graphics::{
    buffer::GpuBuffer,
    material::{GpuMaterial, VulkanGraphicsShaders},
    model::GpuModel,
    GraphicsInterface, VulkanTypes,
};
//#![windows_subsystem = "windows"]
#[cfg(feature = "client")]
use let_engine::prelude::*;

#[cfg(feature = "client")]
use let_engine_widgets::labels::{Label, LabelCreateInfo, Labelifier};

#[cfg(feature = "client")]
use std::{
    f64::consts::{FRAC_PI_2, FRAC_PI_4},
    sync::Arc,
    time::{Duration, SystemTime},
};

// A const that contains the constant window resolution.
#[cfg(feature = "client")]
const RESOLUTION: UVec2 = uvec2(800, 600);

#[cfg(not(feature = "client"))]
fn main() {
    eprintln!("This example requires you to have the `client` feature enabled.");
}

#[cfg(feature = "client")]
fn main() {
    simple_logger::SimpleLogger::new()
        .with_level(log::LevelFilter::Debug)
        .init()
        .unwrap();

    // Describing the window.
    let window_builder = WindowBuilder::default()
        .resizable(false)
        .inner_size(RESOLUTION)
        .title("Pong 2");
    // Initialize the engine.
    let mut engine = Engine::<Game>::new(
        EngineSettings::default()
            .window(window_builder)
            // Do not update physics because there are no physics.
            .tick_system(
                TickSettingsBuilder::default()
                    .update_physics(false)
                    .tick_wait(Duration::from_secs_f64(1.0 / 20.0)) // 20 ticks per second
                    .build()
                    .unwrap(),
            ),
    )
    .unwrap();

    // Runs the game
    engine.start(Game::new);
}

#[cfg(feature = "client")]
struct Game {
    // We only keep the ui_view to keep it from dropping so it keeps existing.
    _ui_view: Arc<LayerView<VulkanTypes>>,

    labelifier: Labelifier<VulkanTypes>,

    left_paddle: Paddle,
    right_paddle: Paddle,
    ball: Ball,

    left_score_label: Label<VulkanTypes>,
    right_score_label: Label<VulkanTypes>,
}
#[cfg(feature = "client")]
impl Game {
    pub fn new(context: &EngineContext) -> Self {
        // First we get the root layer where the scene will be simulated on.
        let root_layer = context.scene.root_layer();
        // We also create a ui layer, the place where the text and middle line will be.
        let ui_layer = root_layer.new_layer();

        // next we set the view of the game scene to -1 to 1 max
        let root_view = context.scene.root_view();
        root_view.set_camera(Camera::default().scaling(CameraScaling::Limited));

        // When making UI, a recommended scaling mode is `Expand`, because it makes sure the UI is
        // the same size no matter how the window is scaled. Size of the transform is zoom here.
        let _ui_view = ui_layer
            .new_view(
                Camera::default()
                    .scaling(CameraScaling::Expand)
                    .transform(Transform::default().size(Vec2::splat(0.8))),
                &context.scene,
                uvec2(0, 0),
            )
            .unwrap();
        // The view will exist as long as this variable is kept. Dropping this eliminates the view.

        // Make left paddle controlled with W for up and S for down.
        let left_paddle = Paddle::new(
            root_layer,
            (Key::Character("w".into()), Key::Character("s".into())),
            -0.95,
            &context.graphics,
        );
        // The right paddle controlled with J and K. Weird controls, but 60% keyboard friendly
        let right_paddle = Paddle::new(
            root_layer,
            (Key::Character("k".into()), Key::Character("j".into())),
            0.95,
            &context.graphics,
        );

        // Spawns a ball in the middle.
        let ball = Ball::new(root_layer, &root_view, &context.graphics);

        let mut labelifier = Labelifier::new(&context.graphics).unwrap();

        // Loading the font for the score.
        let font = labelifier
            .font_from_slice(include_bytes!("Px437_CL_Stingray_8x16.ttf"))
            .expect("Font is invalid.");

        // Making a default label for the left side.
        let left_score_label = Label::new(
            LabelCreateInfo::default()
                .text("0")
                .align(Direction::No)
                .transform(Transform::default().position(vec2(-0.55, 0.0)))
                .scale(Vec2::splat(50.0))
                .font(font.clone()),
            &mut labelifier,
            &context.graphics,
        )
        .unwrap();

        // initialize this one to the ui
        NewObject::new(
            left_score_label
                .appearance()
                .build(&context.graphics)
                .unwrap(),
        )
        .init(&ui_layer)
        .unwrap();

        // Making a default label for the right side.
        let right_score_label = Label::new(
            LabelCreateInfo::default()
                .transform(Transform::default().position(vec2(0.55, 0.0)))
                .text("0")
                .align(Direction::Nw)
                .scale(vec2(50.0, 50.0))
                .font(font),
            &mut labelifier,
            &context.graphics,
        )
        .unwrap();

        NewObject::new(
            right_score_label
                .appearance()
                .build(&context.graphics)
                .unwrap(),
        )
        .init(&ui_layer)
        .unwrap();

        // Submit label creation task in the end.
        // labelifier.update().unwrap();
        // dbg!("Updated");

        /* Line in the middle */

        // Make a custom model that is just 2 lines.
        let vertices = vec![
            vert(0.0, 0.7),
            vert(0.0, 0.3),
            vert(0.0, -0.3),
            vert(0.0, -0.7),
        ];

        let middle_model = GpuModel::new(model!(vertices), &context.graphics).unwrap();

        // A description of how the line should look like.
        let line_material_settings = MaterialSettingsBuilder::default()
            .line_width(10.0)
            .topology(Topology::LineList)
            .build()
            .unwrap();

        let line_material = GpuMaterial::new::<Vert>(
            line_material_settings,
            VulkanGraphicsShaders::new_default(&context.graphics).unwrap(),
        )
        .unwrap();

        // The buffer is a Fixed Uniform here, because it's small and will never change.
        let middle_line_color = GpuBuffer::new(
            Buffer::from_data(BufferUsage::Uniform, BufferAccess::Fixed, Color::WHITE),
            &context.graphics,
        )
        .unwrap();

        let middle_line_appearance = AppearanceBuilder::default()
            .material(line_material)
            .model(middle_model)
            .descriptors(&[
                (Location::new(0, 0), Descriptor::Mvp),
                (Location::new(1, 0), Descriptor::buffer(middle_line_color)),
            ])
            .build(&context.graphics)
            .unwrap();

        // Add the line to the ui layer
        NewObject::new(middle_line_appearance)
            .init(&ui_layer)
            .unwrap();

        Self {
            _ui_view,
            labelifier,

            left_paddle,
            right_paddle,
            ball,

            left_score_label,
            right_score_label,
        }
    }
}

#[cfg(feature = "client")]
impl let_engine::Game for Game {
    fn update(&mut self, context: &EngineContext) {
        // run the update functions of the paddles.
        self.left_paddle.update(context);
        self.right_paddle.update(context);

        // If anyone has won after the ball has updated, modify the score counter
        if self.ball.update(&context.time) {
            dbg!("Win");
            // Update score labels
            self.left_score_label
                .update_text(format!("{}", self.ball.wins[0]))
                .unwrap();
            self.right_score_label
                .update_text(format!("{}", self.ball.wins[1]))
                .unwrap();

            // Update the labelifier each frame to make the score update.
            self.labelifier.update().unwrap();
        };
    }

    // Exit when the X button is pressed.
    fn window(&mut self, context: &EngineContext, event: events::WindowEvent) {
        if let WindowEvent::CloseRequested = event {
            context.exit();
        }
    }

    fn input(&mut self, context: &EngineContext, event: events::InputEvent) {
        if let InputEvent::KeyboardInput { input } = event {
            if input.state == ElementState::Pressed {
                match input.key {
                    // Exit when the escape key is pressed.
                    Key::Named(NamedKey::Escape) => context.exit(),
                    Key::Character(e) => {
                        if e == *"e" {
                            // Troll the right paddle
                            self.right_paddle.shrink();
                        } else if e == *"q" {
                            // Grow and show the right paddle whos boss.
                            self.left_paddle.grow();
                        }
                    }
                    // Oh, so the left paddle thinks it's funny. I'll show it.
                    Key::Named(NamedKey::ArrowLeft) => {
                        self.left_paddle.shrink();
                    }
                    // I can grow too, noob.
                    Key::Named(NamedKey::ArrowRight) => {
                        self.right_paddle.grow();
                    }
                    _ => (),
                }
            }
        }
    }
}

#[cfg(feature = "client")]
struct Paddle {
    controls: (Key, Key), //up/down
    object: Object<VulkanTypes>,
    height: f32,
}

#[cfg(feature = "client")]
impl Paddle {
    pub fn new(
        layer: &Arc<Layer<VulkanTypes>>,
        controls: (Key, Key),
        x: f32,
        graphics_interface: &GraphicsInterface,
    ) -> Self {
        // Next we describe the appearance of the paddle.

        // Here we make the pedal square and give it a default material
        let model = GpuModel::new(model!(square), graphics_interface).unwrap();
        let material = GpuMaterial::new_default(graphics_interface).unwrap();

        // Make the paddle white
        let buffer = GpuBuffer::new(
            Buffer::from_data(BufferUsage::Uniform, BufferAccess::Fixed, Color::WHITE),
            graphics_interface,
        )
        .unwrap();

        let appearance = AppearanceBuilder::default()
            .model(model)
            .material(material)
            .descriptors(&[
                (Location::new(0, 0), Descriptor::Mvp),
                (Location::new(1, 0), Descriptor::buffer(buffer)),
            ])
            .build(graphics_interface)
            .unwrap();

        let height = 0.05;
        let mut object = NewObject::new(appearance);
        object.transform = Transform {
            position: vec2(x, 0.0),
            size: vec2(0.015, height),
            ..Default::default()
        };

        // Make a collider that resembles the form of the paddle.
        object.set_collider(Some(ColliderBuilder::square(0.015, height).build()));

        // Initialize the object to the given layer.
        let object = object.init(layer).unwrap();
        Self {
            controls,
            object,
            height,
        }
    }
    pub fn update(&mut self, context: &EngineContext) {
        // Turn the `True` and `False` of the input.key_down() into 1, 0 or -1.
        let shift = context.input.key_down(&self.controls.0) as i32
            - context.input.key_down(&self.controls.1) as i32;

        // Shift Y and clamp it between 0.51 so it doesn't go out of bounds.
        let y = &mut self.object.transform.position.y;
        *y -= shift as f32 * context.time.delta_time() as f32 * 1.3;
        *y = y.clamp(-0.70, 0.70);

        // Updates the object in the game.
        self.object.sync().unwrap();
    }
    /// To troll the opponent.
    pub fn shrink(&mut self) {
        self.resize(-0.001);
    }
    /// GROW BACK!
    pub fn grow(&mut self) {
        self.resize(0.001);
    }
    fn resize(&mut self, difference: f32) {
        self.height += difference;
        self.height = self.height.clamp(0.001, 0.7);
        self.object.transform.size.y = self.height;
        self.object
            .set_collider(Some(ColliderBuilder::square(0.015, self.height).build()));
        self.object.sync().unwrap();
    }
}

#[cfg(feature = "client")]
struct Ball {
    object: Object<VulkanTypes>,
    layer: Arc<Layer<VulkanTypes>>,
    view: Arc<LayerView<VulkanTypes>>,
    direction: Vec2,
    speed: f32,
    new_round: SystemTime,
    pub wins: [u32; 2],
    // bounce_sound: Sound,
}

/// Ball logic.
#[cfg(feature = "client")]
impl Ball {
    pub fn new(
        layer: &Arc<Layer<VulkanTypes>>,
        view: &Arc<LayerView<VulkanTypes>>,
        graphics_interface: &GraphicsInterface,
    ) -> Self {
        let lifetime = SystemTime::now();

        let model = GpuModel::new(model!(square), graphics_interface).unwrap();
        let material = GpuMaterial::new_default(graphics_interface).unwrap();

        // Make the ball white
        let buffer = GpuBuffer::new(
            Buffer::from_data(BufferUsage::Uniform, BufferAccess::Fixed, Color::WHITE),
            graphics_interface,
        )
        .unwrap();

        let appearance = AppearanceBuilder::default()
            .model(model)
            .material(material)
            .descriptors(&[
                (Location::new(0, 0), Descriptor::Mvp),
                (Location::new(1, 0), Descriptor::buffer(buffer)),
            ])
            .build(graphics_interface)
            .unwrap();

        let mut object = NewObject::new(appearance);
        object.transform.size = vec2(0.015, 0.015);

        let object = object.init(layer).unwrap();

        // // make a sound to play when bouncing.
        // let bounce_sound = Sound::new(
        //     SoundData::gen_square_wave(777.0, 0.03),
        //     SoundSettings::default().volume(0.05),
        // );

        Self {
            object,
            layer: layer.clone(),
            view: view.clone(),
            direction: vec2(1.0, 0.0),
            speed: 1.1,
            new_round: lifetime,
            wins: [0; 2],
            // bounce_sound,
        }
    }

    /// Updates the ball and returns true if the ball has touched the wall
    pub fn update(&mut self, time: &Time) -> bool {
        // Wait one second before starting the round.
        if self.new_round.elapsed().unwrap().as_secs() > 0 {
            let position = self.object.transform.position;

            // Check if the ball is touching a paddle.
            let touching_paddle = self
                .layer
                .intersection_with_shape(Shape::square(0.02, 0.02), (position, 0.0))
                .is_some();

            // Check if the top side or bottom side are touched by checking if the ball position is below or above the screen edges +- the ball size.
            let touching_floor = position.y < self.view.side_to_world(vec2(0.0, 1.0)).y + 0.015;
            let touching_roof = position.y > self.view.side_to_world(vec2(0.0, -1.0)).y - 0.015;
            let touching_wall = position.x.abs() > 1.0;

            if touching_paddle
                && (self.direction.x.is_sign_negative()
                    == self.object.transform.position.x.is_sign_negative())
            {
                self.rebound(position.x as f64, time);
                // It's getting faster with time.
                self.speed += 0.03;
            } else if touching_floor || touching_roof {
                self.direction.y *= -1.0;
            } else if touching_wall {
                // Right wins increase by 1 in case the X is negative.
                if position.x.is_sign_negative() {
                    self.wins[1] += 1;
                } else {
                    self.wins[0] += 1;
                }
                self.reset(time);
                return true;
            }

            // Calculate new ball position
            self.object.transform.position +=
                self.direction * time.delta_time() as f32 * self.speed;

            // Apply new ball position
            self.object.sync().unwrap();
            // self.bounce_sound.update(Tween::default()).unwrap();
        }

        false
    }

    fn reset(&mut self, time: &Time) {
        self.new_round = SystemTime::now();
        self.object.transform.position = vec2(0.0, 0.0);
        self.direction = Self::random_direction(time);
        self.speed = 1.1;
        self.object.sync().unwrap();
    }

    fn rebound(&mut self, x: f64, time: &Time) {
        // Random 0.0 to 1.0 value. Some math that makes a random direction.
        let random = (time.time() * 135225.3).sin().copysign(-x);
        let direction = random.mul_add(FRAC_PI_2, FRAC_PI_4.copysign(-x)) - FRAC_PI_2;

        self.direction = Vec2::from_angle(direction as f32).normalize();

        // // play the bounce sound.
        // self.bounce_sound.play().unwrap();
    }

    fn random_direction(time: &Time) -> Vec2 {
        // Random -1.0 to 1.0 value. Some math that makes a random direction.
        let random = (time.time() * 135225.3).sin();
        let direction = random.mul_add(FRAC_PI_2, FRAC_PI_4.copysign(random)) - FRAC_PI_2;
        Vec2::from_angle(direction as f32)
    }
}
