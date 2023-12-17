//#![windows_subsystem = "windows"]
use let_engine::prelude::*;
use std::{
    f64::consts::{FRAC_PI_2, FRAC_PI_4},
    time::SystemTime,
};

// A const that contains the constant window resolution.
const RESOLUTION: Vec2 = vec2(800.0, 600.0);

fn main() {
    // Describing the window.
    let window_builder = WindowBuilder::default()
        .resizable(false)
        .inner_size(RESOLUTION)
        .title("Pong 2");
    let engine = Engine::new(
        EngineSettingsBuilder::default()
            .window_settings(window_builder)
            // Disable ticks.
            .tick_settings(TickSettingsBuilder::default().paused(true).build().unwrap())
            .build()
            .unwrap(),
    )
    .unwrap();

    let game = Game::new(engine.components());

    // Runs the game
    engine.start(game);
}

struct Game {
    /// Exits the program on true.
    exit: bool,

    left_paddle: Paddle,
    right_paddle: Paddle,
    ball: Ball,

    left_score: Label,
    right_score: Label,
}
impl Game {
    pub fn new(components: &Components) -> Self {
        let game_layer = components.scene().new_layer();
        let ui_layer = components.scene().new_layer();
        // limits the view to -1 to 1 max
        game_layer.set_camera_settings(CameraSettings::default().mode(CameraScaling::Limited));
        ui_layer.set_camera_settings(CameraSettings::default().mode(CameraScaling::Expand));

        // Make left paddle controlled with W for up and S for down.
        let left_paddle = Paddle::new(&game_layer, (VirtualKeyCode::W, VirtualKeyCode::S), -0.95);
        // The right paddle controlled with the arrow up and down keys.
        let right_paddle = Paddle::new(
            &game_layer,
            (VirtualKeyCode::Up, VirtualKeyCode::Down),
            0.95,
        );

        // Spawns a ball in the middle.
        let ball = Ball::new(game_layer.clone());

        // Loading the font for the score.
        let font = Font::from_bytes(include_bytes!("Px437_CL_Stingray_8x16.ttf"), components)
            .expect("Font is invalid.");

        // Making a default label for the left side.
        let mut left_score = Label::new(
            components,
            &font,
            LabelCreateInfo {
                appearance: Appearance::new().transform(Transform::default().size(vec2(0.5, 0.7))),
                text: "0".to_string(),
                align: directions::NO,
                transform: Transform::default().position(vec2(-0.55, 0.0)),
                scale: vec2(50.0, 50.0),
            },
        );
        // initialize this one to the ui
        left_score.init(&ui_layer);

        // Making a default label for the right side.
        let mut right_score = Label::new(
            components,
            &font,
            LabelCreateInfo {
                appearance: Appearance::new().transform(Transform::default().size(vec2(0.5, 0.7))),
                text: "0".to_string(),
                align: directions::NW,
                transform: Transform::default().position(vec2(0.55, 0.0)),
                scale: vec2(50.0, 50.0),
            },
        );
        right_score.init(&ui_layer);

        // Just the line in the middle.
        let mut middle_line = Object::new();
        // Make a custom model that is just a stippled line going from 1 to -1.
        middle_line.appearance.set_model(Model::Custom(
            ModelData::new(
                Data {
                    vertices: vec![
                        vert(0.0, 0.7),
                        vert(0.0, 0.3),
                        vert(0.0, -0.3),
                        vert(0.0, -0.7),
                    ],
                    indices: vec![0, 1, 2, 3],
                },
                components,
            )
            .unwrap(),
        ));
        // A description of how the line should look like.
        let line_material = MaterialSettingsBuilder::default()
            .line_width(10.0)
            .topology(Topology::LineList)
            .build()
            .unwrap();
        let line_material = Material::new(line_material, components).unwrap();
        middle_line.appearance.set_material(Some(line_material));
        middle_line.init(&ui_layer);

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

impl let_engine::Game for Game {
    fn update(&mut self, components: &Components) {
        // run the update functions of the pedals.
        self.left_paddle.update(components);
        self.right_paddle.update(components);
        self.ball.update(components);
        self.left_score
            .update_text(format!("{}", self.ball.wins[0]));
        self.right_score
            .update_text(format!("{}", self.ball.wins[1]));
    }
    fn event(&mut self, event: events::Event, _components: &Components) {
        match event {
            // Exit when the X button is pressed.
            Event::Window(WindowEvent::CloseRequested) => {
                self.exit = true;
            }
            Event::Input(InputEvent::KeyboardInput { input }) => {
                if input.state == ElementState::Pressed {
                    match input.keycode {
                        // Exit when the escape key is pressed.
                        Some(VirtualKeyCode::Escape) => self.exit = true,
                        // Troll the right paddle
                        Some(VirtualKeyCode::E) => {
                            self.right_paddle.shrink();
                        }
                        // Grow and show the right paddle whos boss.
                        Some(VirtualKeyCode::Q) => {
                            self.left_paddle.grow();
                        }
                        // Oh, so the left paddle thinks it's funny. I'll show it.
                        Some(VirtualKeyCode::Left) => {
                            self.left_paddle.shrink();
                        }
                        // I can grow too, noob.
                        Some(VirtualKeyCode::Right) => {
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

struct Paddle {
    controls: (VirtualKeyCode, VirtualKeyCode), //up/down
    object: Object,
    height: f32,
}

impl Paddle {
    pub fn new(layer: &Layer, controls: (VirtualKeyCode, VirtualKeyCode), x: f32) -> Self {
        let height = 0.05;
        let mut object = Object::new();
        object.transform = Transform {
            position: vec2(x, 0.0),
            size: vec2(0.015, height),
            ..Default::default()
        };

        // Make a collider that resembles the form of the paddle.
        object.set_collider(Some(ColliderBuilder::square(0.015, height).build()));

        // Initialize the object to the given layer.
        object.init(layer);
        Self {
            controls,
            object,
            height,
        }
    }
    pub fn update(&mut self, components: &Components) {
        // Turn the `True` and `False` of the input.key_down() into 1, 0 or -1.
        let shift = components.input().key_down(self.controls.0) as i32
            - components.input().key_down(self.controls.1) as i32;

        // Shift Y and clamp it between 0.51 so it doesn't go out of bounds.
        let y = &mut self.object.transform.position.y;
        *y -= shift as f32 * components.time().delta_time() as f32 * 1.3;
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

struct Ball {
    object: Object,
    layer: Layer,
    direction: Vec2,
    speed: f32,
    lifetime: SystemTime,
    new_round: SystemTime,
    pub wins: [u32; 2],
}

/// Ball logic.
impl Ball {
    pub fn new(layer: Layer) -> Self {
        let lifetime = SystemTime::now();
        let mut object = Object::new();
        object.transform.size = vec2(0.015, 0.015);
        object.init(&layer);

        Self {
            object,
            layer,
            direction: Self::random_direction(lifetime),
            speed: 1.0,
            lifetime,
            new_round: lifetime,
            wins: [0; 2],
        }
    }
    pub fn update(&mut self, components: &Components) {
        // Wait one second before starting the round.
        if self.new_round.elapsed().unwrap().as_secs() > 0 {
            let position = self.object.transform.position;

            let touching_paddle = self
                .layer
                .intersection_with_shape(Shape::square(0.02, 0.02), (position, 0.0))
                .is_some();
            let touching_floor =
                position.y < self.layer.side_to_world(directions::N, RESOLUTION).y + 0.015;
            let touching_roof =
                position.y > self.layer.side_to_world(directions::S, RESOLUTION).y - 0.015;
            let touching_wall = position.x.abs() > 1.0;

            if touching_paddle {
                self.rebound(position.x as f64);
                // It's getting faster with time.
                self.speed += 0.005;
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
                self.direction * components.time().delta_time() as f32 * self.speed;
            self.object.sync().unwrap();
        }
    }
    fn reset(&mut self) {
        self.new_round = SystemTime::now();
        self.object.transform.position = vec2(0.0, 0.0);
        self.direction = Self::random_direction(self.lifetime);
        self.object.sync().unwrap();
    }
    fn rebound(&mut self, x: f64) {
        // Random 0.0 to 1.0 value. Some math that makes a random direction.
        let random = (self.lifetime.elapsed().unwrap().as_secs_f64() * 135225.3)
            .sin()
            .copysign(-x);
        let direction = random.mul_add(FRAC_PI_2, FRAC_PI_4.copysign(-x)) - FRAC_PI_2;

        self.direction = Vec2::from_angle(direction as f32).normalize();
    }
    fn random_direction(lifetime: SystemTime) -> Vec2 {
        // Random -1.0 to 1.0 value. Some math that makes a random direction.
        let random = (lifetime.elapsed().unwrap().as_secs_f64() * 135225.3).sin();
        let direction = random.mul_add(FRAC_PI_2, FRAC_PI_4.copysign(random)) - FRAC_PI_2;
        Vec2::from_angle(direction as f32)
    }
}
