//! Events from the event loop.

use std::path::PathBuf;

use crate::prelude::*;
use glam::DVec2;
pub use winit::event::{ElementState, MouseButton};
pub use winit::keyboard::*;

/// An event coming with window context.
#[derive(Debug, Clone)]
pub enum WindowEvent {
    /// In case the window has been resized the new size is given here.
    Resized(UVec2),

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

    /// The cursor has moved on the window.
    ///
    /// Cursor position in pixels relative the the top left corner of the screen.
    CursorMoved(DVec2),

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
    PixelDelta(DVec2),
}

/// Input received from the keyboard.
#[derive(Debug, Clone)]
pub struct KeyboardInput {
    /// Layout independent key position based on the US QWERTY layout.
    ///
    /// May not apply to certain keyboards that implements functions for manually changing key positions.
    pub physical_key: PhysicalKey,

    /// A representation of the pressed or released key.
    ///
    /// Affected by the current modifiers.
    pub key: Key,

    /// Contains the text which is produced by this keypress.
    ///
    /// Returns `None`, if the key can not be interpreted as text.
    pub text: Option<SmolStr>,

    /// The location of this key on the keyboard, if the same key appears more than once.
    ///
    /// For example right and left shift, or the numbers which appear above the alphabetical characters or the keypad.
    pub key_location: KeyLocation,

    /// Pressed or released,
    pub state: ElementState,

    /// True if this key comes from a repeat event.
    ///
    /// On most operating systems, holding down a key makes that key repeat multiple times.
    pub repeat: bool,
}
