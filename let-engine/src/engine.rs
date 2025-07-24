use atomic_float::AtomicF64;
use crossbeam::channel::bounded;

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
    std::cell::OnceCell,
    winit::application::ApplicationHandler,
    winit::event::MouseScrollDelta,
};

use std::sync::atomic::AtomicBool;
use std::time::Instant;
use std::{
    sync::{atomic::Ordering, Arc},
    time::Duration,
};

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
    fn update(&mut self, context: &EngineContext<B>) {}

    /// Runs based on the configured tick settings of the engine.
    fn tick(&mut self, context: &EngineContext<B>) {}

    /// Runs when the window is ready.
    #[cfg(feature = "client")]
    fn window_ready(&mut self, context: &EngineContext<B>) {}

    /// Events captured in the window.
    #[cfg(feature = "client")]
    fn window(&mut self, context: &EngineContext<B>, event: events::WindowEvent) {}

    /// Received input events.
    #[cfg(feature = "client")]
    fn input(&mut self, context: &EngineContext<B>, event: events::InputEvent) {}

    // #[cfg(all(feature = "client", feature = "egui"))]
    // fn egui(&mut self, engine_context: &EngineContext<B>, egui_context: ) {}

    /// A network event received by the server.
    fn server_event(
        &mut self,
        context: &EngineContext<B>,
        connection: Connection<B::Networking>,
        message: ServerMessage<B::Networking>,
    ) {
    }

    /// A network event received by the client.
    fn client_event(&mut self, context: &EngineContext<B>, message: ClientMessage<B::Networking>) {}

    /// Runs last after the game has been stopped using the context's stop method.
    fn end(&mut self, context: &EngineContext<B>) {}
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
    context: EngineContext<B>,

    #[cfg(feature = "client")]
    graphics_backend: B::Graphics,
    #[cfg(feature = "client")]
    settings: EngineSettings<B>,

    game: Arc<Mutex<G>>,
}

pub use let_engine_core::EngineError;

impl<G: Game<B>, B: Backends + 'static> Engine<G, B>
where
    <B::Kira as audio::AudioBackend>::Settings: Default,
    <B::Kira as audio::AudioBackend>::Error: std::fmt::Debug,
{
    /// Starts the game engine with the given game.
    pub fn start(
        game: impl FnOnce(&EngineContext<B>) -> G,
        settings: impl Into<settings::EngineSettings<B>>,
    ) -> Result<(), EngineError<B>> {
        let settings: settings::EngineSettings<B> = settings.into();

        #[cfg(feature = "client")]
        let event_loop = winit::event_loop::EventLoop::new().unwrap();

        // Graphics backend
        #[cfg(feature = "client")]
        let graphics_backend = B::Graphics::new(&settings.graphics, &event_loop)
            .map_err(EngineError::GraphicsBackend)?;

        // Audio backend
        let audio_interface =
            AudioInterface::new(&settings.audio).map_err(EngineError::AudioBackend)?;

        // Networking backend
        let (net_send, net_recv) = bounded(1);
        let (game_send, game_recv) = bounded(1);

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
                let (game, context): (Arc<Mutex<G>>, EngineContext<B>) = game_recv.recv().unwrap();

                loop {
                    networking_backend
                        .receive(|message| {
                            let mut game = game.lock();
                            match message {
                                NetEvent::Server { connection, event } => {
                                    game.server_event(&context, connection, event)
                                }
                                NetEvent::Client { event } => game.client_event(&context, event),
                                NetEvent::Error(e) => todo!("handle error: {e}"),
                            };
                        })
                        .unwrap();
                }
            })
            .unwrap();

        let result = net_recv.recv().unwrap();

        let (server, client) = result.map_err(EngineError::NetworkingBackend)?;

        let context = EngineContext::new(
            #[cfg(feature = "client")]
            graphics_backend.interface().clone(),
            audio_interface,
            settings.tick_system.clone(),
            server,
            client,
        );

        let game = Arc::new(Mutex::new(game(&context)));

        game_send.send((game.clone(), context.clone())).unwrap();

        #[cfg(not(feature = "client"))]
        {
            tick_system::run(context.clone(), game.clone());
            game.lock().end(&context);
            use let_engine_core::backend::networking::{ClientInterface, ServerInterface};
            let _ = context.server.stop();
            let _ = context.client.disconnect();
        }

        #[cfg(feature = "client")]
        {
            let mut engine = Self {
                context,
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
        let game = self.game.clone();

        let window: Arc<winit::window::Window> = event_loop
            .create_window(self.settings.window.clone().into())
            .unwrap()
            .into();

        self.context
            .window
            .set(Window::new(window.clone()))
            .unwrap();

        self.graphics_backend
            .init_window(&window, &self.context.scene);

        game.lock().window_ready(&self.context);

        let context_clone = self.context.clone();

        // Start backend threads
        std::thread::Builder::new()
            .name("let-engine-tick-system".to_string())
            .spawn(move || {
                let context = context_clone;
                tick_system::run(context, game);
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

        self.context.update(&event);

        let window_event = match event {
            WindowEvent::Resized(size) => {
                let size = uvec2(size.width, size.height);
                self.graphics_backend.resize_event(size);
                self.context.scene.root_view().set_extent(size);
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
                self.game.lock().input(
                    &self.context,
                    events::InputEvent::KeyboardInput {
                        input: events::KeyboardInput {
                            physical_key: event.physical_key,
                            key: event.logical_key,
                            text: event.text,
                            key_location: event.location,
                            state: event.state,
                            repeat: event.repeat,
                        },
                    },
                );
                return;
            }
            WindowEvent::ModifiersChanged(_) => {
                self.game
                    .lock()
                    .input(&self.context, events::InputEvent::ModifiersChanged);
                return;
            }
            WindowEvent::MouseInput { state, button, .. } => {
                self.game
                    .lock()
                    .input(&self.context, events::InputEvent::MouseInput(button, state));
                return;
            }
            WindowEvent::MouseWheel { delta, .. } => events::WindowEvent::MouseWheel(match delta {
                MouseScrollDelta::LineDelta(x, y) => ScrollDelta::LineDelta(vec2(x, y)),
                MouseScrollDelta::PixelDelta(delta) => {
                    ScrollDelta::PixelDelta(dvec2(delta.x, delta.y))
                }
            }),
            WindowEvent::RedrawRequested => {
                let window = self.context.window().unwrap();

                self.game.lock().update(&self.context);
                self.graphics_backend
                    .draw(|| {
                        window.pre_present_notify();
                    })
                    .unwrap(); // TODO: Error handling events
                self.context.time.update();

                return;
            }
            _ => return,
        };

        self.game.lock().window(&self.context, window_event);
    }

    fn about_to_wait(&mut self, event_loop: &winit::event_loop::ActiveEventLoop) {
        if self.context.exiting() {
            event_loop.exit();
        }

        if let Some(window) = self.context.window() {
            window.request_redraw();
        }
    }

    fn exiting(&mut self, _event_loop: &winit::event_loop::ActiveEventLoop) {
        self.game.lock().end(&self.context);

        // TODO: handle error
        use let_engine_core::backend::networking::{ClientInterface, ServerInterface};
        let _ = self.context.server.stop();
        let _ = self.context.client.disconnect();
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
                self.game.lock().input(
                    &self.context,
                    events::InputEvent::MouseMotion(glam::vec2(delta.0 as f32, delta.1 as f32)),
                );
            }
            DeviceEvent::MouseWheel { delta } => {
                self.game.lock().input(
                    &self.context,
                    events::InputEvent::MouseWheel(match delta {
                        MouseScrollDelta::LineDelta(x, y) => {
                            ScrollDelta::LineDelta(glam::vec2(x, y))
                        }
                        MouseScrollDelta::PixelDelta(delta) => {
                            ScrollDelta::PixelDelta(dvec2(delta.x, delta.y))
                        }
                    }),
                );
            }
            _ => (),
        }
    }
}

/// Context interface connecting the event loop to the engine.
///
/// Allows receiving timing information, stored inputs and access to settings.
pub struct EngineContext<B = DefaultBackends>
where
    B: Backends,
{
    exit: Arc<AtomicBool>,
    pub time: Arc<Time>,
    pub scene: Arc<Scene<<B::Graphics as GraphicsBackend>::LoadedTypes>>,
    pub(super) tick_system: Arc<TickSystem>,
    #[cfg(feature = "client")]
    pub input: Arc<Input>,
    #[cfg(feature = "client")]
    pub(super) window: OnceCell<Window>,
    #[cfg(feature = "client")]
    pub graphics: <B::Graphics as GraphicsBackend>::Interface,
    pub audio: AudioInterface<B::Kira>,
    pub server: <B::Networking as NetworkingBackend>::ServerInterface,
    pub client: <B::Networking as NetworkingBackend>::ClientInterface,
}

unsafe impl<B: Backends> Send for EngineContext<B> {}

unsafe impl<B: Backends> Sync for EngineContext<B> {}

impl<B> Clone for EngineContext<B>
where
    B: Backends,
{
    fn clone(&self) -> Self {
        Self {
            exit: self.exit.clone(),
            time: self.time.clone(),
            scene: self.scene.clone(),
            tick_system: self.tick_system.clone(),
            #[cfg(feature = "client")]
            input: self.input.clone(),
            #[cfg(feature = "client")]
            window: self.window.clone(),
            #[cfg(feature = "client")]
            graphics: self.graphics.clone(),
            audio: self.audio.clone(),
            server: self.server.clone(),
            client: self.client.clone(),
        }
    }
}

impl<B: Backends> EngineContext<B> {
    fn new(
        #[cfg(feature = "client")] graphics: <B::Graphics as GraphicsBackend>::Interface,
        audio: AudioInterface<B::Kira>,
        tick_system: tick_system::TickSettings,
        server: <B::Networking as NetworkingBackend>::ServerInterface,
        client: <B::Networking as NetworkingBackend>::ClientInterface,
    ) -> Self {
        let scene = Arc::new(Scene::new());
        let exit: Arc<AtomicBool> = Arc::new(false.into());

        {
            let exit = exit.clone();
            let _ = ctrlc::set_handler(move || exit.store(true, Ordering::Release));
        }

        Self {
            exit,
            time: Time::default().into(),
            scene,
            tick_system: Arc::new(TickSystem::new(tick_system)),
            #[cfg(feature = "client")]
            input: Input::default().into(),
            #[cfg(feature = "client")]
            window: OnceCell::new(),
            #[cfg(feature = "client")]
            graphics,
            audio,
            server,
            client,
        }
    }

    /// Returns the window in case it is initialized.
    #[cfg(feature = "client")]
    pub fn window(&self) -> Option<&Window> {
        self.window.get()
    }

    /// Stops the game and lastly runs the exit function of `Game`.
    pub fn exit(&self) {
        self.exit.store(true, Ordering::Relaxed);
    }

    /// Returns true if the loop is exiting.
    pub fn exiting(&self) -> bool {
        self.exit.load(Ordering::Relaxed)
    }

    #[cfg(feature = "client")]
    fn update(&self, window_event: &winit::event::WindowEvent) {
        if let Some(window) = self.window() {
            self.input
                .update(window_event, window.inner_size().as_vec2());
        }
    }
}

/// Holds the timings of the engine like runtime and delta time.
pub struct Time {
    /// Time since engine start.
    time: Instant,
    time_scale: AtomicF64,
    #[cfg(feature = "client")]
    delta_instant: crossbeam::atomic::AtomicCell<Instant>,
    #[cfg(feature = "client")]
    delta_time: AtomicF64,
    pub(crate) zero_cvar: (Mutex<()>, Condvar),
}

impl Default for Time {
    fn default() -> Self {
        Self {
            time: Instant::now(),
            time_scale: AtomicF64::new(1.0f64),
            #[cfg(feature = "client")]
            delta_instant: crossbeam::atomic::AtomicCell::new(Instant::now()),
            #[cfg(feature = "client")]
            delta_time: AtomicF64::new(0.0f64),
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
        self.delta_time.store(delta, Ordering::Release);
    }

    /// Returns the time it took to execute last iteration.
    #[inline]
    #[cfg(feature = "client")]
    pub fn delta_time(&self) -> f64 {
        self.delta_time.load(Ordering::Acquire) * self.scale()
    }

    /// Returns the delta time of the update iteration that does not scale with the time scale.
    #[inline]
    #[cfg(feature = "client")]
    pub fn unscaled_delta_time(&self) -> f64 {
        self.delta_time.load(Ordering::Acquire)
    }

    /// Returns the frames per second.
    #[inline]
    #[cfg(feature = "client")]
    pub fn fps(&self) -> f64 {
        1.0 / self.delta_time.load(Ordering::Relaxed)
    }

    /// Returns the time since start of the engine game session.
    #[inline]
    pub fn time(&self) -> f64 {
        self.time.elapsed().as_secs_f64()
    }

    /// Returns the time scale of the game
    #[inline]
    pub fn scale(&self) -> f64 {
        self.time_scale.load(Ordering::Relaxed)
    }

    /// Sets the time scale of the game.
    ///
    /// Panics if the given time scale is negative.
    #[inline]
    pub fn set_scale(&self, time_scale: f64) {
        if time_scale.is_sign_negative() {
            panic!("A negative time scale was given.");
        }
        self.time_scale.store(time_scale, Ordering::Relaxed);
        if time_scale != 0.0 {
            self.zero_cvar.1.notify_all();
        }
    }

    /// Sleeps the given duration times the time scale of the game engine.
    #[inline]
    pub fn sleep(&self, duration: Duration) {
        spin_sleep::sleep(duration.mul_f64(self.time_scale.load(Ordering::Relaxed)));
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
