use atomic_float::AtomicF64;
use crossbeam::{atomic::AtomicCell, channel::bounded};
use std::sync::atomic::Ordering::Relaxed;

use let_engine_core::{
    backend::{
        Backends,
        audio::AudioInterface,
        gpu::GpuBackend,
        networking::{NetEvent, NetworkingBackend},
    },
    objects::scenes::Scene,
};

use parking_lot::{Condvar, Mutex};

#[cfg(feature = "client")]
use crate::window::WindowBuilder;
use crate::{
    backend::DefaultBackends,
    settings,
    tick_system::{self, TickSystem},
};
#[cfg(feature = "client")]
use {
    self::events::ScrollDelta,
    crate::window::Window,
    crate::{events, input::Input},
    anyhow::Result,
    glam::{dvec2, uvec2, vec2},
    let_engine_core::backend::gpu::GpuInterfacer,
    std::sync::OnceLock,
    winit::application::ApplicationHandler,
    winit::event::MouseScrollDelta,
};

use std::{sync::Arc, time::Duration};
use std::{sync::atomic::AtomicBool, time::Instant};

type Connection<B> = <B as NetworkingBackend>::Connection;
type ClientMessage<'a, B> = <B as NetworkingBackend>::ClientEvent<'a>;
type ServerMessage<'a, B> = <B as NetworkingBackend>::ServerEvent<'a>;

/// The main event trait of the game engine.
///
/// All events emitted by backends or window updates with the client feature enabled get called in this trait.
///
/// Every trait has a default implementation that does nothing.
///
/// An event represents an update of the game state.
#[allow(unused_variables)]
pub trait Game<B: Backends = DefaultBackends>: Send + Sync + 'static {
    /// Runs before the frame is drawn.
    #[cfg(feature = "client")]
    fn update(&mut self, context: EngineContext<B>) {}

    /// Runs before `update` method
    #[cfg(feature = "egui")]
    fn egui(&mut self, context: EngineContext<B>, egui_context: egui::Context) {}

    /// Runs based on the configured tick settings of the engine.
    fn tick(&mut self, context: EngineContext<B>) {}

    /// Runs when the window is ready.
    #[cfg(feature = "client")]
    fn window_ready(&mut self, context: EngineContext<B>) {}

    /// Events emitted by the window system.
    #[cfg(feature = "client")]
    fn window(&mut self, context: EngineContext<B>, event: events::WindowEvent) {}

    /// Received input events.
    #[cfg(feature = "client")]
    fn input(&mut self, context: EngineContext<B>, event: events::InputEvent) {}

    /// An external networking event emitted by the set networking backends server.
    fn server_event(
        &mut self,
        context: EngineContext<B>,
        connection: Connection<B::Networking>,
        message: ServerMessage<B::Networking>,
    ) {
    }

    /// An external networking event emitted by the set networking backends client.
    fn client_event(&mut self, context: EngineContext<B>, message: ClientMessage<B::Networking>) {}

    /// The last event ever emitted by this trait.
    ///
    /// Symbolizes a halt of the game engine, which can be initiated by the contexts `exit` method
    /// or a Ctrl-C event.
    fn end(&mut self, context: EngineContext<B>) {}
}

/// The initial start method of the game engine.
///
/// Runs the game closure after the backends have started.
pub fn start<G: Game<B>, B: Backends + 'static>(
    settings: impl Into<settings::EngineSettings<B>>,
    game: impl FnOnce(EngineContext<B>) -> G,
) -> Result<(), EngineError<B>> {
    Engine::start(game, settings)
}

/// The struct that holds and executes all backends and the game state.
struct Engine<G, B = DefaultBackends>
where
    G: Game<B>,
    B: Backends,
{
    #[cfg(feature = "client")]
    gpu_backend: B::Gpu,
    #[cfg(feature = "client")]
    window_settings: WindowBuilder,

    #[allow(dead_code)]
    game: Arc<GameWrapper<G, B>>,
}

pub use let_engine_core::EngineError;

impl<G: Game<B>, B: Backends + 'static> Engine<G, B> {
    /// Starts the game engine with the given game.
    pub fn start(
        game: impl FnOnce(EngineContext<B>) -> G,
        settings: impl Into<settings::EngineSettings<B>>,
    ) -> Result<(), EngineError<B>> {
        let settings: settings::EngineSettings<B> = settings.into();

        #[cfg(feature = "client")]
        let event_loop = winit::event_loop::EventLoop::new().unwrap();

        // Gpu backend
        #[cfg(feature = "client")]
        let (gpu_backend, gpu_interface) =
            B::Gpu::new(settings.gpu, &event_loop).map_err(EngineError::GpuBackend)?;

        // Audio backend
        let audio_interface =
            AudioInterface::new(settings.audio).map_err(EngineError::AudioBackend)?;

        // Networking backend
        let (net_send, net_recv) = bounded(0);
        let (game_send, game_recv) = bounded(0);

        let networking_settings = settings.networking;
        std::thread::Builder::new()
            .name("let-engine-networking-backend".to_string())
            .spawn(move || {
                let mut networking_backend = match B::Networking::new(networking_settings) {
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
            gpu_interface,
            audio_interface,
            server,
            client,
        ));

        {
            let game = game.clone();
            ctrlc::set_handler(move || {
                game.exit.store(true, Relaxed);
            })
            .unwrap();
        }

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
                gpu_backend,
                #[cfg(feature = "client")]
                window_settings: settings.window,
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
            .create_window(self.window_settings.clone().into())
            .unwrap()
            .into();

        let size = window.inner_size();
        *self.game.scene.lock().root_view_mut().extent_mut() = uvec2(size.width, size.height);

        self.game.window.set(Window::new(window.clone())).unwrap();

        self.gpu_backend.init_window(event_loop, &window);

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
        event_loop: &winit::event_loop::ActiveEventLoop,
        _window_id: winit::window::WindowId,
        event: winit::event::WindowEvent,
    ) {
        use winit::event::WindowEvent;

        let window_size = {
            let window = self.game.window.get().unwrap();
            window.inner_size().as_vec2()
        };

        self.game.input.lock().update(&event, window_size);

        #[cfg(feature = "egui")]
        if self.gpu_backend.update_egui(&event) {
            return;
        }

        let window_event = match event {
            WindowEvent::Resized(size) => {
                let size = uvec2(size.width, size.height);
                self.gpu_backend.resize_event(size);
                *self.game.scene.lock().root_view_mut().extent_mut() = size;
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
                let framerate_limit = self.game.time.framerate_limit();

                if framerate_limit == Duration::ZERO {
                    event_loop.set_control_flow(winit::event_loop::ControlFlow::Poll);
                } else {
                    event_loop.set_control_flow(winit::event_loop::ControlFlow::wait_duration(
                        framerate_limit,
                    ));
                }

                #[cfg(feature = "egui")]
                self.game.egui(self.gpu_backend.draw_egui());

                self.game.update();

                self.gpu_backend
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
    }

    fn new_events(
        &mut self,
        _event_loop: &winit::event_loop::ActiveEventLoop,
        cause: winit::event::StartCause,
    ) {
        use winit::event::StartCause;

        match cause {
            StartCause::ResumeTimeReached { .. } | StartCause::Poll => {
                if let Some(window) = self.game.window.get() {
                    window.request_redraw();
                }
            }
            _ => (),
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
    pub gpu: <B::Gpu as GpuBackend>::Interface,
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

    pub(super) scene: Mutex<Scene<<B::Gpu as GpuBackend>::LoadedTypes>>,

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
        #[cfg(feature = "client")] gpu: <B::Gpu as GpuBackend>::Interface,
        audio: AudioInterface<<B as Backends>::Kira>,
        server: <B::Networking as NetworkingBackend>::ServerInterface,
        client: <B::Networking as NetworkingBackend>::ClientInterface,
    ) -> Self {
        let exit: AtomicBool = false.into();

        let time = Time::default();
        #[cfg(feature = "client")]
        let input = Mutex::new(Input::default());
        let scene = Mutex::new(Scene::default());
        #[cfg(feature = "client")]
        let window = OnceLock::new();
        let backends = BackendInterfaces::<B> {
            tick_system: Arc::new(TickSystem::new(tick_settings)),
            #[cfg(feature = "client")]
            gpu,
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
        scene: &'a mut Scene<<B::Gpu as GpuBackend>::LoadedTypes>,
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
    #[cfg(feature = "egui")]
    pub fn egui(&self, egui_context: egui::Context) {
        let mut scene = self.scene.lock();
        let mut input = self.input.lock();
        let context = self.context(&mut scene, &mut input);
        self.game.lock().egui(context, egui_context);
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
type GpuInterface<'a, B> = <<<B as Backends>::Gpu as GpuBackend>::Interface as GpuInterfacer<
    <<B as Backends>::Gpu as GpuBackend>::LoadedTypes,
>>::Interface<'a>;

/// The context of the game engine. It's the direct interface between the engine and game state
/// updates.
///
/// It contains timing, the scene, an interface to each backend and if feature `client` is
/// enabled, input, the window as well as the gpu backend.
pub struct EngineContext<'a, B = DefaultBackends>
where
    B: Backends,
    B::Gpu: 'a,
{
    exit: &'a AtomicBool,
    pub time: &'a Time,
    pub scene: &'a mut Scene<<B::Gpu as GpuBackend>::LoadedTypes>,
    #[cfg(feature = "client")]
    pub input: &'a mut Input,
    #[cfg(feature = "client")]
    pub(super) window: &'a OnceLock<Window>,
    #[cfg(feature = "client")]
    pub gpu: GpuInterface<'a, B>,
    pub audio: &'a AudioInterface<B::Kira>,
    pub server: &'a <B::Networking as NetworkingBackend>::ServerInterface,
    pub client: &'a <B::Networking as NetworkingBackend>::ClientInterface,
}

impl<'a, B: Backends> EngineContext<'a, B> {
    fn new(
        exit: &'a AtomicBool,
        time: &'a Time,
        scene: &'a mut Scene<<B::Gpu as GpuBackend>::LoadedTypes>,
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
            gpu: backends.gpu.interface(),
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

    #[cfg(feature = "client")]
    framerate_limit: AtomicCell<Duration>,

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
            #[cfg(feature = "client")]
            framerate_limit: AtomicCell::new(Duration::ZERO),
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

    /// Sets the framerate limit as waiting time between frames.
    ///
    /// This should be able to be changed by the user in case they have a device with limited power capacity like a laptop with a battery.
    ///
    /// Setting the duration to no wait time at all will turn off the limit.
    #[cfg(feature = "client")]
    #[inline]
    pub fn set_framerate_limit(&self, limit: Duration) {
        self.framerate_limit.store(limit);
    }

    #[cfg(feature = "client")]
    #[inline]
    pub fn framerate_limit(&self) -> Duration {
        self.framerate_limit.load()
    }

    /// Sets the cap for the max frames per second the game should be able to output.
    ///
    /// This method is the same as setting the `set_framerate_limit` of this setting to `1.0 / cap` in seconds.
    ///
    /// Warns when `fps` is not normal or smaller than `0`
    #[cfg(feature = "client")]
    #[inline]
    pub fn set_fps_limit(&self, fps: f64) {
        if !fps.is_normal() || fps < 0.0 {
            log::warn!("Invalid FPS value: {}. Not changing FPS", fps);
            return;
        }

        self.set_framerate_limit(Duration::from_secs_f64(1.0 / fps));
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
        }
        impl Game {
            pub fn new() -> Self {
                Self { number: 0 }
            }
        }

        impl crate::Game for Game {
            fn tick(&mut self, context: EngineContext) {
                self.number += 1;
                if self.number > 62 {
                    context.exit();
                }
            }
        }

        crate::start(EngineSettings::default(), |_| Game::new())?;

        Ok(())
    }
}
