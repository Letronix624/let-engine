use crate::backend::DefaultBackends;
use crate::input::Input;
use crate::prelude::EngineSettings;
use crate::tick_system::TickSystem;
#[cfg(feature = "client")]
use anyhow::Result;
use atomic_float::AtomicF64;
use glam::{dvec2, uvec2, vec2};
use let_engine_core::backend::audio::{self, AudioInterface};
use let_engine_core::backend::graphics::GraphicsBackend;
use let_engine_core::backend::Backends;
use let_engine_core::objects::scenes::Scene;
use parking_lot::{Condvar, Mutex};
use winit::application::ApplicationHandler;
use winit::event::MouseScrollDelta;

use crate::{events, settings, tick_system};

#[cfg(feature = "client")]
use self::events::ScrollDelta;
#[cfg(feature = "client")]
use crate::window::Window;

use std::cell::OnceCell;
use std::sync::atomic::AtomicBool;
use std::time::Instant;
use std::{
    sync::{atomic::Ordering, Arc},
    time::Duration,
};

#[cfg_attr(
    all(feature = "client"),
    doc = "
Represents the game application with essential methods for a game's lifetime.
# Usage
```
use let_engine::prelude::*;
struct Game {
    exit: bool,
}
impl let_engine::Game<()> for Game {
    fn exit(&self) -> bool {
       // exits the program in case self.exit is true
       self.exit
    }

    async fn update(&mut self) {
        // runs every frame or every engine loop update.
        //...
    }
}
```
        "
)]
#[allow(unused_variables)]
pub trait Game<B: Backends = DefaultBackends>: Send + Sync + 'static {
    /// Runs right after the `start` function was called for the Engine.
    fn start(&mut self, context: &EngineContext<B>) {}

    /// Runs every frame.
    #[cfg(feature = "client")]
    fn update(&mut self, context: &EngineContext<B>) {}

    /// Runs based on the configured tick settings of the engine.
    fn tick(&mut self, context: &EngineContext<B>) {}

    /// Events captured in the window.
    #[cfg(feature = "client")]
    fn window(&mut self, context: &EngineContext<B>, event: events::WindowEvent) {}

    /// Received input events.
    #[cfg(feature = "client")]
    fn input(&mut self, context: &EngineContext<B>, event: events::InputEvent) {}

    // #[cfg(all(feature = "client", feature = "egui"))]
    // fn egui(&mut self, engine_context: &EngineContext<B>, egui_context: ) {}

    // /// A network event coming from the server or client, receiving a user specified message format.
    // fn net_event(
    //     &mut self,
    //     context: &EngineContext<B>,
    //     connection: networking::Connection,
    //     message: RemoteMessage<<B::Networking as NetworkingBackend>::Msg>,
    // ) {
    // }

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
    event_loop: Option<winit::event_loop::EventLoop<()>>,

    #[cfg(feature = "client")]
    graphics_backend: B::Graphics,

    // server: Option<GameServer<B::Networking>>,
    // client: Option<GameClient<B::Networking>>,
    game: Option<Arc<Mutex<G>>>,

    settings: EngineSettings<B>,
}

use let_engine_core::EngineError;

impl<G: Game<B>, B: Backends + 'static> Engine<G, B>
where
    <B::Kira as audio::Backend>::Settings: Default,
    <B::Kira as audio::Backend>::Error: std::fmt::Debug,
{
    /// Initializes the game engine with the given settings ready to be launched using the `start` method.
    ///
    /// This function can only be called one time. Attempting to make a second one of those will return an error.
    pub fn new(settings: impl Into<settings::EngineSettings<B>>) -> Result<Self, EngineError<B>> {
        let settings: settings::EngineSettings<B> = settings.into();

        #[cfg(feature = "client")]
        let event_loop = winit::event_loop::EventLoop::new().unwrap();

        #[cfg(feature = "client")]
        let graphics_backend = B::Graphics::new(&settings.graphics, &event_loop)
            .map_err(EngineError::GraphicsBackend)?;

        #[cfg(feature = "client")]
        let audio_interface =
            AudioInterface::new(&settings.audio).map_err(EngineError::AudioBackend)?;

        let context = EngineContext::new(
            graphics_backend.interface().clone(),
            audio_interface,
            settings.tick_system.clone(),
        );

        Ok(Self {
            context,
            #[cfg(feature = "client")]
            graphics_backend,
            // #[cfg(feature = "client")]
            // audio_backend,
            #[cfg(feature = "client")]
            event_loop: Some(event_loop),
            // server: None,
            // client: None,
            game: None,
            settings,
        })
    }

    /// Server side start function running all the methods of the given game object as documented in the [trait](Game).
    #[cfg(not(feature = "client"))]
    pub fn start(mut self, game: impl FnOnce(&EngineContext<B>) -> G) {
        let game = Arc::new(smol::lock::Mutex::new(game(&self.context)));

        game.lock().await.start(&self.context);

        let game_clone = game.clone();
        let context_clone = self.context.clone();
        smol::spawn(async move {
            let game = game_clone;
            let context = context_clone;
            tick_system::run(context, game).await;
        })
        .detach();

        // loop: check exit and break, if networking is active future both at the same time and a timer future.
        // if the timeout is reached roll the loop again

        loop {
            if self.context.exiting() {
                break;
            }

            use futures::future::{select, Either};
            use smol::Timer;

            let server = self.server.as_ref().map(|s| s.messages.1.clone());
            let client = self.client.as_ref().map(|c| c.messages.1.clone());

            let result = match (server, client) {
                (Some(server), None) => {
                    match select(
                        Box::pin(server.recv()),
                        Timer::after(Duration::from_millis(50)),
                    )
                    .await
                    {
                        Either::Left((left, _)) => left,
                        Either::Right(_) => {
                            continue;
                        }
                    }
                }
                (None, Some(client)) => {
                    match select(
                        Box::pin(client.recv()),
                        Timer::after(Duration::from_millis(50)),
                    )
                    .await
                    {
                        Either::Left((left, _)) => left,
                        Either::Right(_) => {
                            continue;
                        }
                    }
                }
                (Some(server), Some(client)) => {
                    match select(
                        Box::pin(smol::future::race(server.recv(), client.recv())),
                        Timer::after(Duration::from_millis(50)),
                    )
                    .await
                    {
                        Either::Left((left, _)) => left,
                        Either::Right(_) => {
                            continue;
                        }
                    }
                }
                (None, None) => {
                    Timer::after(Duration::from_millis(50)).await;
                    continue;
                }
            };

            if let Ok((connection, message)) = result {
                game.lock()
                    .await
                    .net_event(&self.context, connection, message);
            }
        }

        // Gracefully shutdown both server and client if open.
        if let Some(server) = self.server {
            let _ = server.stop().await;
        }
        if let Some(client) = self.client {
            let _ = client.disconnect().await;
        }
    }

    #[cfg(feature = "client")]
    pub fn start(&mut self, game: impl FnOnce(&EngineContext<B>) -> G) {
        self.game = Some(Arc::new(Mutex::new(game(&self.context))));
        let event_loop = std::mem::take(&mut self.event_loop).unwrap();

        event_loop.run_app(self).unwrap();
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
        let Some(game) = self.game.clone() else {
            return;
        };

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

        game.lock().start(&self.context);

        let game_clone = Arc::clone(&game);
        let context_clone = self.context.clone();
        std::thread::spawn(|| {
            let game = game_clone;
            let context = context_clone;
            tick_system::run(context, game);
        });
    }

    fn window_event(
        &mut self,
        _event_loop: &winit::event_loop::ActiveEventLoop,
        _window_id: winit::window::WindowId,
        event: winit::event::WindowEvent,
    ) {
        use winit::event::WindowEvent;

        let Some(game) = self.game.clone() else {
            return;
        };

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
                game.lock().input(
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
                game.lock()
                    .input(&self.context, events::InputEvent::ModifiersChanged);
                return;
            }
            WindowEvent::MouseInput { state, button, .. } => {
                game.lock()
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

                game.lock().update(&self.context);
                self.graphics_backend
                    .update(|| {
                        window.pre_present_notify();
                    })
                    .unwrap(); // TODO: Error handling events
                self.context.time.update();

                return;
            }
            _ => return,
        };

        game.lock().window(&self.context, window_event);
    }

    fn about_to_wait(&mut self, event_loop: &winit::event_loop::ActiveEventLoop) {
        if self.context.exiting() {
            // if let Some(server) = &mut self.server {
            //     server.stop().await.unwrap();
            // }
            event_loop.exit();
        }

        // {
        //     if let Some(server) = &mut self.server {
        //         smol::block_on(async {
        //             let messages = server.receive_messages().await;
        //             for message in messages {
        //                 game.lock()
        //                     .await
        //                     .net_event(&self.context, message.0, message.1);
        //             }
        //         });
        //     }
        //     if let Some(client) = &mut self.client {
        //         smol::block_on(async {
        //             let messages = client.receive_messages().await;
        //             for message in messages {
        //                 game.lock()
        //                     .await
        //                     .net_event(&self.context, message.0, message.1);
        //             }
        //         });
        //     }
        // }

        if let Some(window) = self.context.window() {
            window.request_redraw();
        }
    }

    fn exiting(&mut self, _event_loop: &winit::event_loop::ActiveEventLoop) {
        let Some(game) = self.game.as_mut() else {
            return;
        };
        // {
        //     // Gracefully shutdown both server and client if open.
        //     if let Some(server) = &mut self.server {
        //         smol::block_on(async {
        //             let _ = server.stop().await;
        //         });
        //     }
        //     if let Some(client) = &mut self.client {
        //         smol::block_on(async {
        //             let _ = client.disconnect().await;
        //         });
        //     }
        // }
        game.lock().end(&self.context);
    }

    fn device_event(
        &mut self,
        _event_loop: &winit::event_loop::ActiveEventLoop,
        _device_id: winit::event::DeviceId,
        event: winit::event::DeviceEvent,
    ) {
        use winit::event::DeviceEvent;
        let Some(game) = self.game.as_mut() else {
            return;
        };
        match event {
            DeviceEvent::MouseMotion { delta } => {
                game.lock().input(
                    &self.context,
                    events::InputEvent::MouseMotion(glam::vec2(delta.0 as f32, delta.1 as f32)),
                );
            }
            DeviceEvent::MouseWheel { delta } => {
                game.lock().input(
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
    pub input: Arc<Input>,
    pub scene: Arc<Scene<<B::Graphics as GraphicsBackend>::LoadedTypes>>,
    pub(super) tick_system: Arc<TickSystem>,
    pub(super) window: OnceCell<Window>,

    pub graphics: <B::Graphics as GraphicsBackend>::Interface,
    pub audio: AudioInterface<B::Kira>,
    // pub networking: Arc<networking::Networking>,
    // pub networking: <<B as Backends<Msg>>::Networking as Backend>::Interface,
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
            input: self.input.clone(),
            scene: self.scene.clone(),
            tick_system: self.tick_system.clone(),
            window: self.window.clone(),
            graphics: self.graphics.clone(),
            audio: self.audio.clone(),
        }
    }
}

impl<B: Backends> EngineContext<B> {
    fn new(
        graphics: <B::Graphics as GraphicsBackend>::Interface,
        audio: AudioInterface<B::Kira>,
        tick_system: tick_system::TickSettings,
    ) -> Self {
        let scene = Arc::new(Scene::new());

        Self {
            exit: Arc::new(false.into()),
            time: Time::default().into(),
            input: Input::default().into(),
            scene,
            tick_system: Arc::new(TickSystem::new(tick_system)),
            window: OnceCell::new(),
            graphics,
            audio,
            // networking,
        }
    }

    /// Returns the window in case it is initialized.
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
        let engine = Engine::new(EngineSettings::default())?;

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
            fn exit(&self) -> bool {
                self.exit
            }
            async fn tick(&mut self) {
                self.number += 1;
                if self.number > 62 {
                    self.exit = true;
                }
            }
        }

        engine.start(Game::new());
        Ok(())
    }
}
