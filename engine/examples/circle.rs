//! Simple circle scene.
use let_engine::prelude::*;

let_engine::let_engine!();

fn main() {
    // First you make a builder containing the description of the window.
    let window_builder = WindowBuilder::new()
        .inner_size(vec2(1280.0, 720.0));
    // Then you start the engine allowing you to load resources and layers.
    let engine = start_engine!(window_builder);

    // Makes a base layer where you place your scene into.
    let layer = SCENE.new_layer();
    // Makes the view zoomed out and not stretchy.
    layer.set_camera_settings(CameraSettings { zoom: 0.5, mode: CameraScaling::Linear });

    // Makes the circle in the middle.
    let mut circle = Object::new();
    // Loads a circle model into the engine and sets the appearance of this object to it.
    circle.appearance.set_model(Some(model!(make_circle!(30))));
    // Initializes the object to the layer
    circle.init(&layer);

    // Runs the loop
    engine.run_loop(move |event, control_flow| {
        match event {
            // Exit when the X button is pressed.
            Event::Window(WindowEvent::CloseRequested) => {
                control_flow.set_exit();
            }
            Event::Input(InputEvent::KeyboardInput { input }) => {
                if input.state == ElementState::Pressed {
                    if let Some(VirtualKeyCode::Escape) = input.keycode {
                        // Exit when the escape key is pressed.
                        control_flow.set_exit();
                    }
                }
            }
            _ => (),
        };              
    })
}