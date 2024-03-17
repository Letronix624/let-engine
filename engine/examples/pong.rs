//#![windows_subsystem = "windows"]
#[cfg(feature = "client")]
use let_engine::prelude::*;

#[cfg(feature = "client")]
use std::{
    f64::consts::{FRAC_PI_2, FRAC_PI_4},
    sync::Arc,
    time::{Duration, SystemTime},
};

// A const that contains the constant window resolution.
#[cfg(feature = "client")]
const RESOLUTION: Vec2 = vec2(800.0, 600.0);

#[cfg(not(feature = "client"))]
fn main() {
    eprintln!("This example requires you to have the `client` feature enabled.");
}

#[cfg(feature = "client")]
fn main() {
    // Describing the window.
    let window_builder = WindowBuilder::default()
        .resizable(false)
        .inner_size(RESOLUTION)
        .title("Pong 2");
    // Initialize the engine.
    let engine = Engine::new(
        EngineSettingsBuilder::default()
            .window_settings(window_builder)
            // Do not update physics because there are no physics.
            .tick_settings(
                TickSettingsBuilder::default()
                    .update_physics(false)
                    .tick_wait(Duration::from_secs_f64(1.0 / 20.0)) // 20 ticks per second
                    .build()
                    .unwrap(),
            )
            .build()
            .unwrap(),
    )
    .unwrap();

    // Initialize the game struct after the engine was initialized.
    let game = Game::new();

    // Runs the game
    engine.start(game);
}

#[cfg(feature = "client")]
struct Game {
    /// Exits the program on true.
    exit: bool,

    left_paddle: Paddle,
    right_paddle: Paddle,
    ball: Ball,

    left_score: Label<Object>,
    right_score: Label<Object>,
}
#[cfg(feature = "client")]
impl Game {
    pub fn new() -> Self {
        let game_layer = SCENE.new_layer();
        let ui_layer = SCENE.new_layer();
        // limits the view to -1 to 1 max
        game_layer.set_camera_settings(CameraSettings::default().mode(CameraScaling::Limited));
        ui_layer.set_camera_settings(
            CameraSettings::default()
                .mode(CameraScaling::Expand)
                .zoom(0.8),
        );

        // Make left paddle controlled with W for up and S for down.
        let left_paddle = Paddle::new(
            &game_layer,
            (Key::Character("w".into()), Key::Character("s".into())),
            -0.95,
        );
        // The right paddle controlled with J and K. Weird controls, but 60% keyboard friendly
        let right_paddle = Paddle::new(
            &game_layer,
            (Key::Character("k".into()), Key::Character("j".into())),
            0.95,
        );

        // Spawns a ball in the middle.
        let ball = Ball::new(&game_layer);

        // Loading the font for the score.
        let font = Font::from_slice(include_bytes!("Px437_CL_Stingray_8x16.ttf"))
            .expect("Font is invalid.");

        // Making a default label for the left side.
        let left_score = Label::new(
            &font,
            LabelCreateInfo {
                appearance: Appearance::new().transform(Transform::default().size(vec2(0.5, 0.7))),
                text: "0".to_string(),
                align: Direction::No,
                transform: Transform::default().position(vec2(-0.55, 0.0)),
                scale: vec2(50.0, 50.0),
            },
        );
        // initialize this one to the ui
        let left_score = left_score.init(&ui_layer).unwrap();

        // Making a default label for the right side.
        let right_score = Label::new(
            &font,
            LabelCreateInfo {
                appearance: Appearance::new().transform(Transform::default().size(vec2(0.5, 0.7))),
                text: "0".to_string(),
                align: Direction::Nw,
                transform: Transform::default().position(vec2(0.55, 0.0)),
                scale: vec2(50.0, 50.0),
            },
        );
        let right_score = right_score.init(&ui_layer).unwrap();

        // Just the line in the middle.
        let mut middle_line = NewObject::new();

        // Make a custom model that is just 2 lines.
        const MIDDLE_DATA: Data = Data::new_fixed(
            &[
                vert(0.0, 0.7),
                vert(0.0, 0.3),
                vert(0.0, -0.3),
                vert(0.0, -0.7),
            ],
            &[0, 1, 2, 3],
        );
        middle_line
            .appearance
            .set_model(Some(Model::Custom(ModelData::new(MIDDLE_DATA).unwrap())))
            .unwrap();
        // A description of how the line should look like.
        let line_material = MaterialSettingsBuilder::default()
            .line_width(10.0)
            .topology(Topology::LineList)
            .build()
            .unwrap();
        let line_material = Material::new(line_material, None).unwrap();
        middle_line.appearance.set_material(Some(line_material));
        middle_line.init(&ui_layer).unwrap();

        Self {
            exit: false,
            left_paddle,
            right_paddle,
            ball,

            left_score,
            right_score,
        }
    }
}

#[cfg(feature = "client")]
impl let_engine::Game for Game {
    fn update(&mut self) {
        // run the update functions of the paddles.
        self.left_paddle.update();
        self.right_paddle.update();
        self.ball.update();
        self.left_score
            .update_text(format!("{}", self.ball.wins[0]));
        self.right_score
            .update_text(format!("{}", self.ball.wins[1]));
    }
    fn event(&mut self, event: events::Event) {
        match event {
            // Exit when the X button is pressed.
            Event::Window(WindowEvent::CloseRequested) => {
                self.exit = true;
            }
            Event::Input(InputEvent::KeyboardInput { input }) => {
                if input.state == ElementState::Pressed {
                    match input.keycode {
                        // Exit when the escape key is pressed.
                        Key::Named(NamedKey::Escape) => self.exit = true,
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
            _ => (),
        }
    }
    fn exit(&self) -> bool {
        self.exit
    }
}

#[cfg(feature = "client")]
struct Paddle {
    controls: (Key, Key), //up/down
    object: Object,
    height: f32,
}

#[cfg(feature = "client")]
impl Paddle {
    pub fn new(layer: &Arc<Layer>, controls: (Key, Key), x: f32) -> Self {
        let height = 0.05;
        let mut object = NewObject::new();
        object.transform = Transform {
            position: vec2(x, 0.0),
            size: vec2(0.015, height),
            ..Default::default()
        };
        object.appearance = Appearance::default().model(Some(Model::Square)).unwrap();

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
    pub fn update(&mut self) {
        // Turn the `True` and `False` of the input.key_down() into 1, 0 or -1.
        let shift =
            INPUT.key_down(&self.controls.0) as i32 - INPUT.key_down(&self.controls.1) as i32;

        // Shift Y and clamp it between 0.51 so it doesn't go out of bounds.
        let y = &mut self.object.transform.position.y;
        *y -= shift as f32 * TIME.delta_time() as f32 * 1.3;
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
    object: Object,
    layer: Arc<Layer>,
    direction: Vec2,
    speed: f32,
    new_round: SystemTime,
    pub wins: [u32; 2],

    bounce_sound: Sound,
}

/// Ball logic.
#[cfg(feature = "client")]
impl Ball {
    pub fn new(layer: &Arc<Layer>) -> Self {
        let lifetime = SystemTime::now();
        let mut object = NewObject::new();
        object.transform.size = vec2(0.015, 0.015);
        object.appearance = Appearance::default().model(Some(Model::Square)).unwrap();
        let object = object.init(layer).unwrap();
        // make a sound to play when bouncing.
        let mut bounce_sound = Sound::new(
            SoundData::gen_square_wave(777.0, 0.03),
            SoundSettings::default().volume(0.05),
        );
        bounce_sound.bind_to_object(Some(&object));

        Self {
            object,
            layer: layer.clone(),
            direction: Self::random_direction(),
            speed: 1.1,
            new_round: lifetime,
            wins: [0; 2],
            bounce_sound,
        }
    }
    pub fn update(&mut self) {
        // Wait one second before starting the round.
        if self.new_round.elapsed().unwrap().as_secs() > 0 {
            let position = self.object.transform.position;

            // Check if the ball is touching a paddle.
            let touching_paddle = self
                .layer
                .intersection_with_shape(Shape::square(0.02, 0.02), (position, 0.0))
                .is_some();
            // Check if the top side or bottom side are touched by checking if the ball position is below or above the screen edges +- the ball size.
            let touching_floor = position.y < self.layer.side_to_world(vec2(0.0, 1.0)).y + 0.015;
            let touching_roof = position.y > self.layer.side_to_world(vec2(0.0, -1.0)).y - 0.015;
            let touching_wall = position.x.abs() > 1.0;

            if touching_paddle
                && (self.direction.x.is_sign_negative()
                    == self.object.transform.position.x.is_sign_negative())
            {
                self.rebound(position.x as f64);
                // It's getting faster with time.
                self.speed += 0.02;
            } else if touching_floor {
                self.direction.y = self.direction.y.abs();
            } else if touching_roof {
                self.direction.y = -self.direction.y.abs();
            } else if touching_wall {
                // Right wins increase by 1 in case the X is negative.
                if position.x.is_sign_negative() {
                    self.wins[1] += 1;
                } else {
                    self.wins[0] += 1;
                }
                self.reset();
                return;
            }

            self.object.transform.position +=
                self.direction * TIME.delta_time() as f32 * self.speed;
            self.object.sync().unwrap();
            self.bounce_sound.update(Tween::default()).unwrap();
        }
    }
    fn reset(&mut self) {
        self.new_round = SystemTime::now();
        self.object.transform.position = vec2(0.0, 0.0);
        self.direction = Self::random_direction();
        self.speed = 1.1;
        self.object.sync().unwrap();
    }
    fn rebound(&mut self, x: f64) {
        // Random 0.0 to 1.0 value. Some math that makes a random direction.
        let random = (TIME.time() * 135225.3).sin().copysign(-x);
        let direction = random.mul_add(FRAC_PI_2, FRAC_PI_4.copysign(-x)) - FRAC_PI_2;

        self.direction = Vec2::from_angle(direction as f32).normalize();

        // play the bounce sound.
        self.bounce_sound.play().unwrap();
    }
    fn random_direction() -> Vec2 {
        // Random -1.0 to 1.0 value. Some math that makes a random direction.
        let random = (TIME.time() * 135225.3).sin();
        let direction = random.mul_add(FRAC_PI_2, FRAC_PI_4.copysign(random)) - FRAC_PI_2;
        Vec2::from_angle(direction as f32)
    }
}
