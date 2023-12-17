//! Events from the event loop.

use std::path::PathBuf;

use crate::prelude::*;
#[cfg(feature = "egui")]
use egui_winit_vulkano::egui::Context;
pub use winit::event::{ElementState, MouseButton, VirtualKeyCode};

/// Describes an event coming from the event loop.
#[derive(Debug, Clone)]
pub enum Event {
    /// The EGUI context. This is only available if you enable the egui feature.
    #[cfg(feature = "egui")]
    Egui(Context),
    /// Events that happened to the window.
    Window(WindowEvent),
    /// Input events.
    Input(InputEvent),
    /// The last event to be called in this loop.
    /// This is the "do on quit" event.
    Destroyed,
}

/// An event coming with window context.
#[derive(Debug, Clone)]
pub enum WindowEvent {
    /// In case the window has been resized the new size is given here.
    Resized(PhysicalSize<u32>),
    /// The window has been requested to close.
    /// Happens when the X button gets pressed on the title bar, the X gets pressed in the task bar, the Alt f4 combination gets pressed or any other ways to request a close to the window.
    CloseRequested,
    /// The window no more.
    Destroyed,
    /// A file is getting hovered over the window.
    ///
    /// This event happens separately for every file in case a multitude of files have been dragged in.
    HoveredFile(PathBuf),
    /// The file has been dropped inside the window.
    ///
    /// This event happens separately for every file in case a multitude of files have been dragged in.
    DroppedFile(PathBuf),
    /// In case files have been hovered over the window but have left the window without getting dropped in.
    ///
    /// This event gets called once, no matter how many files were hovered over the window.
    HoveredFileCancelled,
    /// `True` if the window was focused.
    /// `False` if it lost focus.
    Focused(bool),
    /// The cursor has entered the window.
    CursorEntered,
    /// The cursor has left the window.
    CursorLeft,
    /// THe cursor has moved on the window.
    ///
    /// Cursor position in pixels relative the the top left corner of the screen.
    CursorMoved(PhysicalPosition<f64>),
    /// Mouse scroll event on the window.
    MouseWheel(ScrollDelta),
}

/// An event coming from device input.
#[derive(Debug, Clone)]
pub enum InputEvent {
    /// Raw mouse motion in delta.
    ///
    /// The given units may be different depending on the device.
    MouseMotion(Vec2),
    /// Mouse scroll event.
    MouseWheel(ScrollDelta),
    /// A mouse button was pressed.
    MouseInput(MouseButton, ElementState),
    /// A unicode character was received by the window.
    ReceivedCharacter(char),
    /// Input by the keyboard.
    KeyboardInput { input: KeyboardInput },
    /// The modifiers were changed.
    /// Gets called when either shift, ctrl, alt or the super key get pressed.
    ///
    /// The changes can be taken from the [INPUT](input::Input) struct.
    ModifiersChanged,
}

/// The delta of a mouse scroll.
#[derive(Debug, Clone)]
pub enum ScrollDelta {
    LineDelta(Vec2),
    /// A scroll with exact pixels to be moved.
    PixelDelta(PhysicalPosition<f64>),
}

/// Input received from the keyboard.
#[derive(Debug, Clone)]
pub struct KeyboardInput {
    /// Hardware dependent scancode.
    ///
    /// Does not change when you change the keyboard map.
    /// Only keeps track of the physical keyboard key scancodes.
    pub scancode: u32,
    /// The meaning of the key.
    pub keycode: Option<VirtualKeyCode>,
    pub state: ElementState,
}
