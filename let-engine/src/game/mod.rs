#[cfg(feature = "client")]
pub use let_engine_core::window;
#[cfg(feature = "client")]
use let_engine_core::{draw::Draw, resources::Resources};
#[cfg(feature = "client")]
use let_engine_core::{resources::RESOURCES, window::WINDOW};
#[cfg(all(feature = "egui", feature = "client"))]
mod egui;
#[cfg(feature = "client")]
pub mod events;
#[cfg(feature = "client")]
pub mod input;
pub mod settings;
mod tick_system;

use anyhow::Result;
use atomic_float::AtomicF64;
use crossbeam::atomic::AtomicCell;
use parking_lot::{Condvar, Mutex};
use smol::Timer;

#[cfg(feature = "client")]
use self::{
    events::{InputEvent, ScrollDelta},
    window::{Window, WindowBuilder},
};
pub use tick_system::*;

#[cfg(feature = "networking")]
pub mod networking;
#[cfg(feature = "networking")]
use networking::{GameClient, GameServer, RemoteMessage};
#[cfg(feature = "networking")]
use serde::{Deserialize, Serialize};

use std::marker::PhantomData;
use std::{
    sync::{atomic::Ordering, Arc},
    time::Duration,
    time::SystemTime,
};

// client feature
#[cfg(feature = "client")]
use crate::INPUT;
use crate::{SETTINGS, TIME};

#[cfg_attr(
    feature = "networking",
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
#[cfg_attr(
    not(feature = "networking"),
    doc = "
Represents the game application with essential methods for a game's lifetime.
# Usage
```
use let_engine::prelude::*;
struct Game {
    exit: bool,
}
impl let_engine::Game for Game {
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
#[allow(async_fn_in_trait)]
pub trait Game<#[cfg(feature = "networking")] Msg> {
    #[cfg_attr(
        feature = "client",
        doc = "Runs right before the first frame is drawn and the window gets displayed, initializing the instance."
    )]
    #[cfg_attr(
        not(feature = "client"),
        doc = "Runs right after the `start` function was called for the Engine."
    )]
    async fn start(&mut self) {}
    #[cfg_attr(feature = "client", doc = "Runs before the frame is drawn.")]
    #[cfg(feature = "client")]
    async fn update(&mut self) {}
    /// Runs after the frame is drawn.
    #[cfg(feature = "client")]
    async fn frame_update(&mut self) {}
    /// Runs based on the configured tick settings of the engine.
    fn tick(&mut self) -> impl std::future::Future<Output = ()> + std::marker::Send {
        async {}
    }
    /// Handles engine and window events.
    #[allow(unused_variables)]
    #[cfg(feature = "client")]
    async fn event(&mut self, event: events::Event) {}
    /// A network event coming from the server or client, receiving a user specified message format.
    #[cfg(feature = "networking")]
    #[allow(unused_variables)]
    async fn net_event(&mut self, connection: networking::Connection, message: RemoteMessage<Msg>) {
    }
    #[cfg_attr(
        feature = "client",
        doc = "If true exits the program, stopping the loop and closing the window."
    )]
    #[cfg_attr(
        not(feature = "client"),
        doc = "If true exits the program, stopping the loop."
    )]
    fn exit(&self) -> bool;
}

macro_rules! impl_engine_features {
    { impl Engine $implementations:tt } => {
        #[cfg(not(feature = "networking"))]
        impl<G: Game + Send + Sync + 'static> Engine<G> $implementations

        #[cfg(feature = "networking")]
        impl<G: Game<Msg> + Send + Sync + 'static , Msg> Engine<G, Msg>
        where
            for<'a> Msg: Send + Sync + Serialize + Deserialize<'a> + Clone + 'static $implementations
    };
}

/// The struct that holds and executes all of the game data.
///
/// Generic `Msg` that requires to be serde serialisable and deserialisable, is the message that can be sent/received from a remote
/// to be interpreted in the `net_event` function of `game`.
#[cfg(feature = "networking")]
pub struct Engine<G, Msg>
where
    G: Game<Msg> + Send + Sync + 'static,
    for<'a> Msg: Send + Sync + Serialize + Deserialize<'a> + Clone + 'static,
{
    #[cfg(all(feature = "egui", feature = "client"))]
    gui: egui_winit_vulkano::Gui,
    tick_system: Option<TickSystem<G, Msg>>,
    #[cfg(feature = "client")]
    event_loop: Option<winit::event_loop::EventLoop<()>>,

    #[cfg(feature = "client")]
    draw: Draw,
    server: Option<GameServer<Msg>>,
    client: Option<GameClient<Msg>>,
    _game: PhantomData<G>,
}

/// The struct that holds and executes all of the game data.
#[cfg(not(feature = "networking"))]
pub struct Engine<G>
where
    G: Game + Send + Sync + 'static,
{
    #[cfg(all(feature = "egui", feature = "client"))]
    gui: egui_winit_vulkano::Gui,
    tick_system: Option<TickSystem<G>>,
    #[cfg(feature = "client")]
    event_loop: Option<winit::event_loop::EventLoop<()>>,

    #[cfg(feature = "client")]
    draw: Draw,
    _game: PhantomData<G>,
}

/// Makes sure the engine struct only gets constructed a single time.
static INIT: parking_lot::Once = parking_lot::Once::new();

use let_engine_core::EngineError;

impl_engine_features! {

    impl Engine
    {
        /// Initializes the game engine with the given settings ready to be launched using the `start` method.
        ///
        /// This function can only be called one time. Attempting to make a second one of those will return an error.
        pub fn new(settings: impl Into<settings::EngineSettings>) -> Result<Self, EngineError> {
            if INIT.state() == parking_lot::OnceState::New {
                #[cfg(feature = "client")]
                let event_loop = winit::event_loop::EventLoopBuilder::new()
                    .build()
                    .map_err(|e| EngineError::Other(e.into()))?;
                #[cfg(feature = "client")]
                let resources = Resources::new(&event_loop)?;
                #[cfg(feature = "client")]
                RESOURCES.get_or_init(|| resources);
                INIT.call_once(|| {});
                let settings = settings.into();
                SETTINGS.tick_system.set(settings.tick_settings);
                let tick_system = Some(TickSystem::new());

                #[cfg(feature = "client")]
                let draw = Draw::setup(
                    settings.window_settings,
                    &event_loop,
                    SETTINGS.graphics.clone(),
                )
                .map_err(EngineError::DrawingBackendError)?;
                #[cfg(feature = "client")]
                WINDOW.get_or_init(|| draw.window().clone());

                #[cfg(all(feature = "egui", feature = "client"))]
                let gui = egui::init(&draw, &event_loop);

                Ok(Self {
                    #[cfg(all(feature = "egui", feature = "client"))]
                    gui,
                    tick_system,
                    #[cfg(feature = "client")]
                    event_loop: Some(event_loop),
                    #[cfg(feature = "client")]
                    draw,
                    #[cfg(feature = "networking")]
                    server: None,
                    #[cfg(feature = "networking")]
                    client: None,
                    _game: PhantomData
                })
            } else {
                Err(EngineError::EngineInitialized)
            }
        }

        /// Returns the window of the game.
        #[cfg(feature = "client")]
        pub fn get_window(&self) -> &Window {
            self.draw.window()
        }

        /// Server side start function running all the methods of the given game object as documented in the [trait](Game).
        // #[cfg(not(feature = "client"))]
        pub fn start(mut self, game: G) {


            smol::block_on(async {
                let game = Arc::new(smol::lock::Mutex::new(game));

                game.lock().await.start().await;
                let tick_system = std::mem::take(&mut self.tick_system);
                if let Some(tick_system) = tick_system {
                    let game_clone = Arc::clone(&game);
                    smol::spawn(async {
                        let mut tick_system = tick_system;
                        let game = game_clone;
                        tick_system.run(game).await;
                    });
                }
                loop {
                    if game.lock().await.exit() {
                        break;
                    }

                    #[cfg(feature = "networking")]
                    {
                        use futures::future::{Either, select};

                        let server = self.server.as_ref().map(|s| s.messages.1.clone());
                        let client = self.client.as_ref().map(|c| c.messages.1.clone());

                        match (server, client) {
                            (Some(server), None) => {
                                select(
                                    Box::pin(server.recv()),
                                    Timer::after(Duration::from_millis(50))
                                );
                            }
                            (None, Some(client)) => {
                                select(
                                    Box::pin(client.recv()),
                                    Timer::after(Duration::from_millis(50))
                                );
                            }
                            (Some(server), Some(client)) => {
                                smol::future::race(server.recv(), client.recv());
                            }
                            (None, None) => continue
                        };

                        // server_receiver.race();

                    }
                }
            })
        }

        #[cfg(feature = "client")]
        pub fn start(&mut self, game: G) {
            use let_engine_core::draw::VulkanError;
            use winit::event::{DeviceEvent, Event, MouseScrollDelta, StartCause, WindowEvent};
            let game = Arc::new(smol::lock::Mutex::new(game));

            let event_loop = std::mem::take(&mut self.event_loop).unwrap();

            event_loop
                .run(move |event, control_flow| {
                    smol::block_on(async {
                        INPUT.update(&event, self.get_window().inner_size());
                        if game.lock().await.exit() {
                            #[cfg(feature = "networking")]
                            if let Some(server) = &mut self.server {
                                server.stop().await.unwrap();
                            }
                            control_flow.exit();
                        }

                        #[cfg(feature = "networking")]
                        {
                            if let Some(server) = &mut self.server {
                                let messages = server.receive_messages().await;
                                for message in messages {
                                    game.lock().await.net_event(message.0, message.1).await;
                                }
                            }
                            if let Some(client) = &mut self.client {
                                let messages = client.receive_messages().await;
                                for message in messages {
                                    game.lock().await.net_event(message.0, message.1).await;
                                }
                            }
                        }

                        match event {
                            Event::WindowEvent { event, .. } => {
                                #[cfg(feature = "egui")]
                                self.gui.update(&event);
                                let event = match event {
                                    WindowEvent::Resized(size) => {
                                        self.draw.mark_swapchain_outdated();
                                        events::Event::Window(events::WindowEvent::Resized(size))
                                    }
                                    WindowEvent::CloseRequested => {
                                        events::Event::Window(events::WindowEvent::CloseRequested)
                                    }
                                    WindowEvent::CursorEntered { .. } => {
                                        events::Event::Window(events::WindowEvent::CursorEntered)
                                    }
                                    WindowEvent::CursorLeft { .. } => {
                                        events::Event::Window(events::WindowEvent::CursorLeft)
                                    }
                                    WindowEvent::CursorMoved { position, .. } => events::Event::Window(
                                        events::WindowEvent::CursorMoved(position),
                                    ),
                                    WindowEvent::Destroyed => {
                                        events::Event::Window(events::WindowEvent::Destroyed)
                                    }
                                    WindowEvent::HoveredFile(file) => {
                                        events::Event::Window(events::WindowEvent::HoveredFile(file))
                                    }
                                    WindowEvent::DroppedFile(file) => {
                                        events::Event::Window(events::WindowEvent::DroppedFile(file))
                                    }
                                    WindowEvent::HoveredFileCancelled => {
                                        events::Event::Window(events::WindowEvent::HoveredFileCancelled)
                                    }
                                    WindowEvent::Focused(focused) => {
                                        events::Event::Window(events::WindowEvent::Focused(focused))
                                    }
                                    WindowEvent::KeyboardInput { event, .. } => {
                                        events::Event::Input(InputEvent::KeyboardInput {
                                            input: events::KeyboardInput {
                                                physical_key: event.physical_key,
                                                key: event.logical_key,
                                                text: event.text,
                                                key_location: event.location,
                                                state: event.state,
                                                repeat: event.repeat,
                                            },
                                        })
                                    }
                                    WindowEvent::ModifiersChanged(_) => {
                                        events::Event::Input(InputEvent::ModifiersChanged)
                                    }
                                    WindowEvent::MouseInput { state, button, .. } => {
                                        events::Event::Input(InputEvent::MouseInput(button, state))
                                    }
                                    WindowEvent::MouseWheel { delta, .. } => events::Event::Window(
                                        events::WindowEvent::MouseWheel(match delta {
                                            MouseScrollDelta::LineDelta(x, y) => {
                                                ScrollDelta::LineDelta(glam::vec2(x, y))
                                            }
                                            MouseScrollDelta::PixelDelta(x) => {
                                                ScrollDelta::PixelDelta(x)
                                            }
                                        }),
                                    ),
                                    WindowEvent::RedrawRequested => {

                                        // fps limit logic
                                        let start_time = SystemTime::now();

                                        // redraw
                                        match self.draw.redraw_event(
                                            #[cfg(feature = "egui")]
                                            &mut self.gui,
                                        ) {
                                            Err(VulkanError::SwapchainOutOfDate) => {
                                                self.draw.mark_swapchain_outdated();
                                            }
                                            Err(e) => panic!("{e}"),
                                            _ => (),
                                        };

                                        // sleeps the required time to hit the framerate limit.
                                        spin_sleep::native_sleep(
                                            SETTINGS
                                                .graphics
                                                .framerate_limit()
                                                .saturating_sub(start_time.elapsed().unwrap() * 2),
                                        );
                                        TIME.update();
                                        game.lock().await.frame_update().await;
                                        events::Event::Destroyed
                                    }
                                    _ => events::Event::Destroyed,
                                };
                                // destroy event can not be called here so I did the most lazy approach possible.
                                if let events::Event::Destroyed = event {
                                } else {
                                    game.lock().await.event(event).await;
                                }
                            }
                            Event::DeviceEvent { event, .. } => match event {
                                DeviceEvent::MouseMotion { delta } => {
                                    game.lock().await
                                        .event(events::Event::Input(InputEvent::MouseMotion(glam::vec2(
                                            delta.0 as f32,
                                            delta.1 as f32,
                                        )))).await;
                                }
                                DeviceEvent::MouseWheel { delta } => {
                                    game.lock().await
                                        .event(events::Event::Input(InputEvent::MouseWheel(
                                            match delta {
                                                MouseScrollDelta::LineDelta(x, y) => {
                                                    ScrollDelta::LineDelta(glam::vec2(x, y))
                                                }
                                                MouseScrollDelta::PixelDelta(delta) => {
                                                    ScrollDelta::PixelDelta(delta)
                                                }
                                            },
                                        ))).await;
                                }
                                _ => (),
                            },
                            Event::AboutToWait => {
                                #[cfg(feature = "egui")]
                                {
                                    let mut context = egui_winit_vulkano::egui::Context::default();
                                    self.gui.immediate_ui(|gui| {
                                        context = gui.context()
                                    });
                                    game.lock().await.event(events::Event::Egui(context)).await;
                                }

                                game.lock().await.update().await;
                                self.get_window().request_redraw();
                            }
                            Event::LoopExiting => {
                                game.lock().await.event(events::Event::Destroyed).await;
                            }
                            Event::MemoryWarning => {
                                game.lock().await.event(events::Event::LowMemory).await;
                            }
                            Event::NewEvents(StartCause::Init) => {
                                #[cfg(feature = "egui")]
                                {
                                    let mut context = egui_winit_vulkano::egui::Context::default();
                                    self.gui.immediate_ui(|gui| {
                                        context = gui.context()
                                    });
                                    game.lock().await.event(events::Event::Egui(context)).await;
                                }
                                match self.draw.redraw_event(
                                    #[cfg(feature = "egui")]
                                    &mut self.gui,
                                ) {
                                    Err(VulkanError::SwapchainOutOfDate) => {
                                        self.draw.mark_swapchain_outdated();
                                    }
                                    Err(e) => panic!("{e}"),
                                    _ => (),
                                };
                                game.lock().await.start().await;
                                self.get_window().initialize();

                                let tick_system = std::mem::take(&mut self.tick_system);
                                if let Some(tick_system) = tick_system {
                                    let game_clone = Arc::clone(&game);
                                    smol::spawn(async {
                                        let mut tick_system = tick_system;
                                        let game = game_clone;
                                        tick_system.run(game).await;
                                    });
                                }
                            }
                            _ => (),
                        }
                });
            })
            .unwrap();
        }
    }
}

#[cfg(feature = "networking")]
impl_engine_features! {
    impl Engine {
        /// Hosts a server in this engine struct with the given ip and port to accept users and send/receive messages.
        pub fn new_server(&mut self, addr: std::net::SocketAddr) -> Result<GameServer<Msg>> {
            let server = GameServer::<Msg>::new(addr)?;
            self.server = Some(server.clone());
            Ok(server)
        }

        /// Hosts a server in this engine struct with the given port to accept clients from the same device and send/receive messages.
        pub fn new_local_server(&mut self, port: u16) -> Result<GameServer<Msg>> {
            let addr = std::net::SocketAddrV4::new(std::net::Ipv4Addr::new(127, 0, 0, 1), port);
            let server = GameServer::<Msg>::new(addr.into())?;
            self.server = Some(server.clone());
            Ok(server)
        }

        /// Hosts a server in this engine struct with the given port to accept users from anywhere and send/receive messages.
        ///
        /// Allows users from the local network to join the game using the given port and allows users from around the world to join
        /// if this port is forwarded in your network.
        pub fn new_public_server(&mut self, port: u16) -> Result<GameServer<Msg>> {
            let addr = local_ip_addr::get_local_ip_address()?;
            use std::str::FromStr;
            let addr = std::net::SocketAddr::new(std::net::Ipv4Addr::from_str(&addr)?.into(), port);
            let server = GameServer::<Msg>::new(addr)?;
            self.server = Some(server.clone());
            Ok(server)
        }

        /// Creates a client that you can connect to the given server address with it's `connect` function.
        ///
        /// Pipes the remote messages created by that client to the games `net_event` function.
        pub fn new_client(&mut self, addr: std::net::SocketAddr) -> Result<GameClient<Msg>> {
            let client = GameClient::<Msg>::new(addr)?;
            self.client = Some(client.clone());
            Ok(client)
        }
    }
}

/// Holds the timings of the engine like runtime and delta time.
pub struct Time {
    /// Time since engine start.
    time: SystemTime,
    time_scale: AtomicF64,
    #[cfg(feature = "client")]
    delta_instant: AtomicCell<SystemTime>,
    #[cfg(feature = "client")]
    delta_time: AtomicF64,
    pub(crate) zero_cvar: (Mutex<()>, Condvar),
}

impl Default for Time {
    fn default() -> Self {
        Self {
            time: SystemTime::now(),
            time_scale: AtomicF64::new(1.0f64),
            #[cfg(feature = "client")]
            delta_instant: AtomicCell::new(SystemTime::now()),
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
        self.delta_time.store(
            self.delta_instant.load().elapsed().unwrap().as_secs_f64(),
            Ordering::Release,
        );
        self.delta_instant.store(SystemTime::now());
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
        1.0 / self.delta_time.load(Ordering::Acquire)
    }

    /// Returns the time since start of the engine game session.
    #[inline]
    pub fn time(&self) -> f64 {
        self.time.elapsed().unwrap().as_secs_f64()
    }

    /// Returns the time scale of the game
    #[inline]
    pub fn scale(&self) -> f64 {
        self.time_scale.load(Ordering::Acquire)
    }

    /// Sets the time scale of the game.
    ///
    /// Panics if the given time scale is negative.
    #[inline]
    pub fn set_scale(&self, time_scale: f64) {
        if time_scale.is_sign_negative() {
            panic!("A negative time scale was given.");
        }
        self.time_scale.store(time_scale, Ordering::Release);
        if time_scale != 0.0 {
            self.zero_cvar.1.notify_all();
        }
    }

    /// Sleeps the given duration times the time scale of the game engine.
    #[inline]
    pub fn sleep(&self, duration: Duration) {
        spin_sleep::sleep(duration.mul_f64(self.time_scale.load(Ordering::Acquire)));
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
