use atomic_float::AtomicF64;
use crossbeam::{atomic::AtomicCell, channel::bounded};
use std::sync::atomic::Ordering::Relaxed;

use let_engine_core::{
    backend::{
        audio::{self, AudioInterface},
        graphics::GraphicsBackend,
        networking::{NetEvent, NetworkingBackend},
        Backends,
    },
    objects::scenes::Scene,
};

use parking_lot::{Condvar, Mutex};

use crate::{
    backend::DefaultBackends,
    settings,
    tick_system::{self, TickSystem},
};
#[cfg(feature = "client")]
use {
    self::events::ScrollDelta,
    crate::window::Window,
    crate::{events, input::Input, prelude::EngineSettings},
    anyhow::Result,
    glam::{dvec2, uvec2, vec2},
    let_engine_core::backend::graphics::GraphicsInterfacer,
    std::sync::OnceLock,
    winit::application::ApplicationHandler,
    winit::event::MouseScrollDelta,
};

use std::{sync::atomic::AtomicBool, time::Instant};
use std::{sync::Arc, time::Duration};

type Connection<B> = <B as NetworkingBackend>::Connection;
type ClientMessage<'a, B> = <B as NetworkingBackend>::ClientEvent<'a>;
type ServerMessage<'a, B> = <B as NetworkingBackend>::ServerEvent<'a>;

#[cfg_attr(
    all(feature = "client"),
    doc = "
Represents the game application with essential methods for a game's lifetime.
# Usage
```
use let_engine::prelude::*;
struct Game;
impl let_engine::Game<DefaultBackends> for Game {
    fn update(&mut self) {
        // runs every frame or every engine loop update.
        //...
    }
}
```
        "
)]
#[allow(unused_variables)]
pub trait Game<B: Backends = DefaultBackends>: Send + Sync + 'static {
    /// Runs every frame.
    #[cfg(feature = "client")]
    fn update(&mut self, context: EngineContext<B>) {}

    /// Runs based on the configured tick settings of the engine.
    fn tick(&mut self, context: EngineContext<B>) {}

    /// Runs when the window is ready.
    #[cfg(feature = "client")]
    fn window_ready(&mut self, context: EngineContext<B>) {}

    /// Events captured in the window.
    #[cfg(feature = "client")]
    fn window(&mut self, context: EngineContext<B>, event: events::WindowEvent) {}

    /// Received input events.
    #[cfg(feature = "client")]
    fn input(&mut self, context: EngineContext<B>, event: events::InputEvent) {}

    // #[cfg(all(feature = "client", feature = "egui"))]
    // fn egui(&mut self, engine_context: &EngineContext<B>, egui_context: ) {}

    /// A network event received by the server.
    fn server_event(
        &mut self,
        context: EngineContext<B>,
        connection: Connection<B::Networking>,
        message: ServerMessage<B::Networking>,
    ) {
    }

    /// A network event received by the client.
    fn client_event(&mut self, context: EngineContext<B>, message: ClientMessage<B::Networking>) {}

    /// Runs last after the game has been stopped using the context's stop method.
    fn end(&mut self, context: EngineContext<B>) {}
}

/// The struct that holds and executes all of the game data.
///
/// Generic `Msg` that requires to be serde serialisable and deserialisable, is the message that can be sent/received from a remote
/// to be interpreted in the `net_event` function of `game`.
pub struct Engine<G, B = DefaultBackends>
where
    G: Game<B>,
    B: Backends,
{
    #[cfg(feature = "client")]
    graphics_backend: B::Graphics,
    #[cfg(feature = "client")]
    settings: EngineSettings<B>,

    #[allow(dead_code)]
    game: Arc<GameWrapper<G, B>>,
}

pub use let_engine_core::EngineError;

impl<G: Game<B>, B: Backends + 'static> Engine<G, B>
where
    <B::Kira as audio::AudioBackend>::Settings: Default,
    <B::Kira as audio::AudioBackend>::Error: std::fmt::Debug,
{
    /// Starts the game engine with the given game.
    pub fn start(
        game: impl FnOnce(EngineContext<B>) -> G,
        settings: impl Into<settings::EngineSettings<B>>,
    ) -> Result<(), EngineError<B>> {
        let settings: settings::EngineSettings<B> = settings.into();

        #[cfg(feature = "client")]
        let event_loop = winit::event_loop::EventLoop::new().unwrap();

        // Graphics backend
        #[cfg(feature = "client")]
        let (graphics_backend, graphics_interface) =
            B::Graphics::new(&settings.graphics, &event_loop)
                .map_err(EngineError::GraphicsBackend)?;

        // Audio backend
        let audio_interface =
            AudioInterface::new(&settings.audio).map_err(EngineError::AudioBackend)?;

        // Networking backend
        let (net_send, net_recv) = bounded(0);
        let (game_send, game_recv) = bounded(0);

        let networking_settings = settings.networking.clone();
        std::thread::Builder::new()
            .name("let-engine-networking-backend".to_string())
            .spawn(move || {
                let mut networking_backend = match B::Networking::new(&networking_settings) {
                    Ok(n) => {
                        net_send
                            .send(Ok((
                                n.server_interface().clone(),
                                n.client_interface().clone(),
                            )))
                            .unwrap();
                        n
                    }
                    Err(e) => {
                        net_send.send(Err(e)).unwrap();
                        return;
                    }
                };
                let game: Arc<GameWrapper<G, B>> = game_recv.recv().unwrap();

                loop {
                    networking_backend
                        .receive(|message| {
                            match message {
                                NetEvent::Server { connection, event } => {
                                    game.server_event(connection, event)
                                }
                                NetEvent::Client { event } => game.client_event(event),
                                NetEvent::Error(e) => todo!("handle error: {e}"),
                            };
                        })
                        .unwrap();
                }
            })
            .unwrap();

        let result = net_recv.recv().unwrap();

        let (server, client) = result.map_err(EngineError::NetworkingBackend)?;

        let game = Arc::new(GameWrapper::new(
            game,
            settings.tick_system.clone(),
            #[cfg(feature = "client")]
            graphics_interface,
            audio_interface,
            server,
            client,
        ));

        game_send.send(game.clone()).unwrap();

        #[cfg(not(feature = "client"))]
        {
            tick_system::run(game.clone());
            game.end();
            use let_engine_core::backend::networking::{ClientInterface, ServerInterface};
            let _ = game.backends.server.stop();
            let _ = game.backends.client.disconnect();
        }

        #[cfg(feature = "client")]
        {
            let mut engine = Self {
                #[cfg(feature = "client")]
                graphics_backend,
                #[cfg(feature = "client")]
                settings,
                game,
            };

            event_loop.run_app(&mut engine).unwrap();
        }

        Ok(())
    }
}

#[doc(hidden)]
#[cfg(feature = "client")]
impl<G, B> ApplicationHandler for Engine<G, B>
where
    G: Game<B>,
    B: Backends + 'static,
{
    fn resumed(&mut self, event_loop: &winit::event_loop::ActiveEventLoop) {
        let window: Arc<winit::window::Window> = event_loop
            .create_window(self.settings.window.clone().into())
            .unwrap()
            .into();

        let size = window.inner_size();
        self.game
            .scene
            .lock()
            .root_view()
            .set_extent(uvec2(size.width, size.height));

        self.game.window.set(Window::new(window.clone())).unwrap();

        self.graphics_backend.init_window(&window);

        self.game.window_ready();

        // Start backend threads
        let game_clone = self.game.clone();
        std::thread::Builder::new()
            .name("let-engine-tick-system".to_string())
            .spawn(move || {
                tick_system::run(game_clone);
            })
            .unwrap();
    }

    fn window_event(
        &mut self,
        _event_loop: &winit::event_loop::ActiveEventLoop,
        _window_id: winit::window::WindowId,
        event: winit::event::WindowEvent,
    ) {
        use winit::event::WindowEvent;

        let window_size = {
            let window = self.game.window.get().unwrap();
            window.inner_size().as_vec2()
        };

        self.game.input.lock().update(&event, window_size);

        let window_event = match event {
            WindowEvent::Resized(size) => {
                let size = uvec2(size.width, size.height);
                self.graphics_backend.resize_event(size);
                self.game.scene.lock().root_view().set_extent(size);
                events::WindowEvent::Resized(size)
            }
            WindowEvent::CloseRequested => events::WindowEvent::CloseRequested,
            WindowEvent::CursorEntered { .. } => events::WindowEvent::CursorEntered,
            WindowEvent::CursorLeft { .. } => events::WindowEvent::CursorLeft,
            WindowEvent::CursorMoved { position, .. } => {
                events::WindowEvent::CursorMoved(dvec2(position.x, position.y))
            }
            WindowEvent::Destroyed => events::WindowEvent::Destroyed,
            WindowEvent::HoveredFile(file) => events::WindowEvent::HoveredFile(file),
            WindowEvent::DroppedFile(file) => events::WindowEvent::DroppedFile(file),
            WindowEvent::HoveredFileCancelled => events::WindowEvent::HoveredFileCancelled,
            WindowEvent::Focused(focused) => events::WindowEvent::Focused(focused),
            WindowEvent::KeyboardInput { event, .. } => {
                self.game.input(events::InputEvent::KeyboardInput {
                    input: events::KeyboardInput {
                        physical_key: event.physical_key,
                        key: event.logical_key,
                        text: event.text,
                        key_location: event.location,
                        state: event.state,
                        repeat: event.repeat,
                    },
                });
                return;
            }
            WindowEvent::ModifiersChanged(_) => {
                self.game.input(events::InputEvent::ModifiersChanged);
                return;
            }
            WindowEvent::MouseInput { state, button, .. } => {
                self.game
                    .input(events::InputEvent::MouseInput(button, state));
                return;
            }
            WindowEvent::MouseWheel { delta, .. } => events::WindowEvent::MouseWheel(match delta {
                MouseScrollDelta::LineDelta(x, y) => ScrollDelta::LineDelta(vec2(x, y)),
                MouseScrollDelta::PixelDelta(delta) => {
                    ScrollDelta::PixelDelta(dvec2(delta.x, delta.y))
                }
            }),
            WindowEvent::RedrawRequested => {
                self.game.update();

                self.graphics_backend
                    .draw(&self.game.scene.lock(), || {
                        let window = self.game.window.get().unwrap();
                        window.pre_present_notify();
                    })
                    .unwrap(); // TODO: Error handling events
                self.game.time.update();

                return;
            }
            _ => return,
        };

        self.game.window(window_event);
    }

    fn about_to_wait(&mut self, event_loop: &winit::event_loop::ActiveEventLoop) {
        if self.game.exit.load(Relaxed) {
            event_loop.exit();
        }

        if let Some(window) = self.game.window.get() {
            window.request_redraw();
        }
    }

    #[inline]
    fn exiting(&mut self, _event_loop: &winit::event_loop::ActiveEventLoop) {
        self.game.end();
    }

    fn device_event(
        &mut self,
        _event_loop: &winit::event_loop::ActiveEventLoop,
        _device_id: winit::event::DeviceId,
        event: winit::event::DeviceEvent,
    ) {
        use winit::event::DeviceEvent;
        match event {
            DeviceEvent::MouseMotion { delta } => {
                self.game.input(events::InputEvent::MouseMotion(glam::vec2(
                    delta.0 as f32,
                    delta.1 as f32,
                )));
            }
            DeviceEvent::MouseWheel { delta } => {
                self.game.input(events::InputEvent::MouseWheel(match delta {
                    MouseScrollDelta::LineDelta(x, y) => ScrollDelta::LineDelta(glam::vec2(x, y)),
                    MouseScrollDelta::PixelDelta(delta) => {
                        ScrollDelta::PixelDelta(dvec2(delta.x, delta.y))
                    }
                }));
            }
            _ => (),
        }
    }
}

#[derive(Clone)]
pub(crate) struct BackendInterfaces<B: Backends> {
    pub tick_system: Arc<TickSystem>,
    #[cfg(feature = "client")]
    pub graphics: <B::Graphics as GraphicsBackend>::Interface,
    pub audio: AudioInterface<B::Kira>,
    pub server: <B::Networking as NetworkingBackend>::ServerInterface,
    pub client: <B::Networking as NetworkingBackend>::ClientInterface,
}

unsafe impl<B: Backends> Send for BackendInterfaces<B> {}
unsafe impl<B: Backends> Sync for BackendInterfaces<B> {}

pub(crate) struct GameWrapper<G, B>
where
    G: Game<B>,
    B: Backends,
{
    pub(super) game: Mutex<G>,
    pub(super) exit: AtomicBool,

    pub(super) time: Time,
    #[cfg(feature = "client")]
    pub(super) input: Mutex<Input>,

    pub(super) scene: Mutex<Scene<<B::Graphics as GraphicsBackend>::LoadedTypes>>,

    #[cfg(feature = "client")]
    pub(super) window: OnceLock<Window>,

    pub(super) backends: BackendInterfaces<B>,
}

impl<G, B> GameWrapper<G, B>
where
    G: Game<B>,
    B: Backends,
{
    // TODO: define settings inside of here.
    pub fn new(
        game: impl FnOnce(EngineContext<B>) -> G,
        tick_settings: tick_system::TickSettings,
        #[cfg(feature = "client")] graphics: <B::Graphics as GraphicsBackend>::Interface,
        audio: AudioInterface<<B as Backends>::Kira>,
        server: <B::Networking as NetworkingBackend>::ServerInterface,
        client: <B::Networking as NetworkingBackend>::ClientInterface,
    ) -> Self {
        let exit = false.into();
        let time = Time::default();
        #[cfg(feature = "client")]
        let input = Mutex::new(Input::default());
        let scene = Mutex::new(Scene::default());
        #[cfg(feature = "client")]
        let window = OnceLock::new();
        let backends = BackendInterfaces::<B> {
            tick_system: Arc::new(TickSystem::new(tick_settings)),
            #[cfg(feature = "client")]
            graphics,
            audio,
            server,
            client,
        };

        let game = {
            let mut scene = scene.lock();
            #[cfg(feature = "client")]
            let mut input = input.lock();

            let context = EngineContext::<B>::new(
                &exit,
                &time,
                &mut scene,
                #[cfg(feature = "client")]
                &mut input,
                #[cfg(feature = "client")]
                &window,
                &backends,
            );

            Mutex::new(game(context))
        };

        Self {
            game,
            exit,
            time,
            #[cfg(feature = "client")]
            input,
            scene,
            #[cfg(feature = "client")]
            window,
            backends,
        }
    }

    pub fn context<'a>(
        &'a self,
        scene: &'a mut Scene<<B::Graphics as GraphicsBackend>::LoadedTypes>,
        #[cfg(feature = "client")] input: &'a mut Input,
    ) -> EngineContext<'a, B> {
        EngineContext::new(
            &self.exit,
            &self.time,
            scene,
            #[cfg(feature = "client")]
            input,
            #[cfg(feature = "client")]
            &self.window,
            &self.backends,
        )
    }

    #[inline]
    #[cfg(feature = "client")]
    pub fn update(&self) {
        let mut scene = self.scene.lock();
        let mut input = self.input.lock();
        let context = self.context(&mut scene, &mut input);
        self.game.lock().update(context);
    }

    #[inline]
    pub fn tick(&self) {
        let mut scene = self.scene.lock();
        #[cfg(feature = "client")]
        let mut input = self.input.lock();
        let context = self.context(
            &mut scene,
            #[cfg(feature = "client")]
            &mut input,
        );
        self.game.lock().tick(context);
    }

    #[cfg(feature = "client")]
    #[inline]
    pub fn window_ready(&self) {
        let mut scene = self.scene.lock();
        let mut input = self.input.lock();
        let context = self.context(&mut scene, &mut input);
        self.game.lock().window_ready(context);
    }

    #[cfg(feature = "client")]
    #[inline]
    pub fn window(&self, event: events::WindowEvent) {
        let mut scene = self.scene.lock();
        let mut input = self.input.lock();
        let context = self.context(&mut scene, &mut input);
        self.game.lock().window(context, event);
    }

    #[cfg(feature = "client")]
    #[inline]
    pub fn input(&self, event: events::InputEvent) {
        let mut scene = self.scene.lock();
        let mut input = self.input.lock();
        let context = self.context(&mut scene, &mut input);
        self.game.lock().input(context, event);
    }

    #[inline]
    pub fn server_event(
        &self,
        connection: <<B as Backends>::Networking as NetworkingBackend>::Connection,
        message: <<B as Backends>::Networking as NetworkingBackend>::ServerEvent<'_>,
    ) {
        let mut scene = self.scene.lock();
        #[cfg(feature = "client")]
        let mut input = self.input.lock();
        let context = self.context(
            &mut scene,
            #[cfg(feature = "client")]
            &mut input,
        );
        self.game.lock().server_event(context, connection, message);
    }

    #[inline]
    pub fn client_event(
        &self,
        message: <<B as Backends>::Networking as NetworkingBackend>::ClientEvent<'_>,
    ) {
        let mut scene = self.scene.lock();
        #[cfg(feature = "client")]
        let mut input = self.input.lock();
        let context = self.context(
            &mut scene,
            #[cfg(feature = "client")]
            &mut input,
        );
        self.game.lock().client_event(context, message);
    }

    #[inline]
    pub fn end(&self) {
        // TODO: handle error
        use let_engine_core::backend::networking::{ClientInterface, ServerInterface};
        let _ = self.backends.server.stop();
        let _ = self.backends.client.disconnect();

        let mut scene = self.scene.lock();
        #[cfg(feature = "client")]
        let mut input = self.input.lock();
        let context = self.context(
            &mut scene,
            #[cfg(feature = "client")]
            &mut input,
        );
        self.game.lock().end(context);
    }
}

#[cfg(feature = "client")]
type GraphicsInterface<'a, B> =
    <<<B as Backends>::Graphics as GraphicsBackend>::Interface as GraphicsInterfacer<
        <<B as Backends>::Graphics as GraphicsBackend>::LoadedTypes,
    >>::Interface<'a>;

/// Context interface connecting the event loop to the engine.
///
/// Allows receiving timing information, stored inputs and access to settings.
pub struct EngineContext<'a, B = DefaultBackends>
where
    B: Backends,
    B::Graphics: 'a,
{
    exit: &'a AtomicBool,
    pub time: &'a Time,
    pub scene: &'a mut Scene<<B::Graphics as GraphicsBackend>::LoadedTypes>,
    #[cfg(feature = "client")]
    pub input: &'a mut Input,
    #[cfg(feature = "client")]
    pub(super) window: &'a OnceLock<Window>,
    #[cfg(feature = "client")]
    pub graphics: GraphicsInterface<'a, B>,
    pub audio: &'a AudioInterface<B::Kira>,
    pub server: &'a <B::Networking as NetworkingBackend>::ServerInterface,
    pub client: &'a <B::Networking as NetworkingBackend>::ClientInterface,
}

impl<'a, B: Backends> EngineContext<'a, B> {
    fn new(
        exit: &'a AtomicBool,
        time: &'a Time,
        scene: &'a mut Scene<<B::Graphics as GraphicsBackend>::LoadedTypes>,
        #[cfg(feature = "client")] input: &'a mut Input,
        #[cfg(feature = "client")] window: &'a OnceLock<Window>,
        backends: &'a BackendInterfaces<B>,
    ) -> Self {
        Self {
            exit,
            time,
            scene,
            #[cfg(feature = "client")]
            input,
            #[cfg(feature = "client")]
            window,
            #[cfg(feature = "client")]
            graphics: backends.graphics.interface(),
            audio: &backends.audio,
            server: &backends.server,
            client: &backends.client,
        }
    }

    /// Returns the window in case it is initialized.
    #[cfg(feature = "client")]
    pub fn window(&self) -> Option<&Window> {
        self.window.get()
    }

    /// Stops the game and lastly runs the exit function of `Game`.
    pub fn exit(&self) {
        self.exit.store(true, Relaxed);
    }

    /// Returns true if the loop is exiting.
    pub fn exiting(&self) -> bool {
        self.exit.load(Relaxed)
    }
}

/// Holds the timings of the engine like runtime and delta time.
pub struct Time {
    /// Time since engine start.
    time: AtomicCell<Instant>,
    time_scale: AtomicF64,

    #[cfg(feature = "client")]
    delta_instant: AtomicCell<Instant>,
    /// Rendering delta time
    #[cfg(feature = "client")]
    delta_time: AtomicF64,

    /// Notification to time dependent tick systems that the time scale has reached zero, so stopped.
    pub(crate) zero_cvar: (Mutex<()>, Condvar),
}

impl Default for Time {
    fn default() -> Self {
        Self {
            time: AtomicCell::new(Instant::now()),
            time_scale: 1.0.into(),
            #[cfg(feature = "client")]
            delta_instant: AtomicCell::new(Instant::now()),
            #[cfg(feature = "client")]
            delta_time: 0.0.into(),
            zero_cvar: (Mutex::new(()), Condvar::new()),
        }
    }
}

impl Time {
    /// Updates the time data on frame redraw.
    #[inline]
    #[cfg(feature = "client")]
    pub(crate) fn update(&self) {
        let now = Instant::now();
        let last = self.delta_instant.swap(now);
        let delta = now.duration_since(last).as_secs_f64();
        self.delta_time.store(delta, Relaxed);
    }

    /// Returns the time it took to execute last iteration.
    #[inline]
    #[cfg(feature = "client")]
    pub fn delta_time(&self) -> f64 {
        self.delta_time.load(Relaxed) * self.scale()
    }

    /// Returns the delta time of the update iteration that does not scale with the time scale.
    #[inline]
    #[cfg(feature = "client")]
    pub fn unscaled_delta_time(&self) -> f64 {
        self.delta_time.load(Relaxed)
    }

    /// Returns the frames per second.
    #[inline]
    #[cfg(feature = "client")]
    pub fn fps(&self) -> f64 {
        1.0 / self.delta_time.load(Relaxed)
    }

    /// Returns the time since start of the engine game session.
    #[inline]
    pub fn time(&self) -> f64 {
        self.time.load().elapsed().as_secs_f64()
    }

    /// Returns the time scale of the game
    #[inline]
    pub fn scale(&self) -> f64 {
        self.time_scale.load(Relaxed)
    }

    /// Sets the time scale of the game.
    ///
    /// Panics if the given time scale is negative.
    #[inline]
    pub fn set_scale(&self, time_scale: f64) {
        if time_scale.is_sign_negative() {
            panic!("A negative time scale was given.");
        }
        self.time_scale.store(time_scale, Relaxed);
        if time_scale != 0.0 {
            self.zero_cvar.1.notify_all();
        }
    }

    /// Sleeps the given duration times the time scale of the game engine.
    #[inline]
    pub fn sleep(&self, duration: Duration) {
        spin_sleep::sleep(duration.mul_f64(self.time_scale.load(Relaxed)));
    }
}

#[cfg(not(feature = "client"))]
#[cfg(test)]
mod tests {
    use crate::prelude::*;

    #[test]
    fn start_engine() -> anyhow::Result<()> {
        struct Game {
            number: u32,
            exit: bool,
        }
        impl Game {
            pub fn new() -> Self {
                Self {
                    number: 0,
                    exit: false,
                }
            }
        }

        impl crate::Game for Game {
            fn tick(&mut self, _context: &EngineContext) {
                self.number += 1;
                if self.number > 62 {
                    self.exit = true;
                }
            }
        }

        Engine::start(|_| Game::new(), EngineSettings::default())?;

        Ok(())
    }
}
