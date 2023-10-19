//! General window stuff.
//!
//! Multiple structs to change the properties of a Window.
use crossbeam::atomic::AtomicCell;
use dpi::*;
use std::sync::Arc;
use vulkano::swapchain::Surface;
pub use winit::{
    dpi,
    window::{CursorGrabMode, CursorIcon, Icon, UserAttentionType, WindowLevel},
};
use winit::{
    error::ExternalError,
    window::{Fullscreen, WindowButtons},
};

/// A struct representing the window.
#[derive(Clone)]
pub struct Window {
    window: Arc<winit::window::Window>,
    clear_color: Arc<AtomicCell<[f32; 4]>>,
}

impl Window {
    pub fn new(surface: Arc<Surface>, clear_color: [f32; 4]) -> Self {
        Self {
            window: surface
                .object()
                .unwrap()
                .clone()
                .downcast::<winit::window::Window>()
                .unwrap(),
            clear_color: Arc::new(AtomicCell::new(clear_color)),
        }
    }

    /// Requests the window to be redrawn.
    #[inline]
    pub fn request_redraw(&self) {
        self.window.request_redraw();
    }

    /// Returns the inner size of the window.
    #[inline]
    pub fn inner_size(&self) -> PhysicalSize<u32> {
        self.window.inner_size()
    }

    /// Sets the size of the window. This unmaximizes the window in case it is.
    #[inline]
    pub fn set_inner_size(&self, size: impl Into<Size>) {
        self.window.set_inner_size(size);
    }

    /// Returns the outer size of the window.
    #[inline]
    pub fn outer_size(&self) -> PhysicalSize<u32> {
        self.window.outer_size()
    }

    /// Restricts the window to not go smaller than the given size.
    #[inline]
    pub fn set_min_inner_size(&self, size: Option<impl Into<Size>>) {
        self.window.set_min_inner_size(size);
    }

    /// Restricts the window to not go bigger than the given size.
    #[inline]
    pub fn set_max_inner_size(&self, size: Option<impl Into<Size>>) {
        self.window.set_max_inner_size(size);
    }

    /// Returns the increments in which the window gets resized.
    #[inline]
    pub fn resize_increments(&self) -> Option<PhysicalSize<u32>> {
        self.window.resize_increments()
    }

    /// Returns the increments in which the window gets resized.
    #[inline]
    pub fn set_resize_increments(&self, increments: Option<impl Into<Size>>) {
        self.window.set_resize_increments(increments);
    }

    /// Returns the title of the window.
    #[inline]
    pub fn title(&self) -> String {
        self.window.title()
    }

    /// Sets the title of the window.
    #[inline]
    pub fn set_title(&self, title: &str) {
        self.window.set_title(title)
    }

    /// Sets whether the window should have an alpha channel.
    #[inline]
    fn set_transparent(&self, transparent: bool) {
        self.window.set_transparent(transparent)
    }

    /// Sets whether the window should be visible.
    #[inline]
    pub fn set_visible(&self, visible: bool) {
        self.window.set_visible(visible)
    }

    /// Returns whether the window is visible.
    ///
    /// `None` means it can't be determined if the window is visible or not.
    #[inline]
    pub fn visible(&self) -> Option<bool> {
        self.window.is_visible()
    }

    /// Sets whether the window should be resizable.
    #[inline]
    pub fn set_resizable(&self, resizable: bool) {
        self.window.set_resizable(resizable)
    }

    /// Returns whether the window is resizable.
    #[inline]
    pub fn resizable(&self) -> bool {
        self.window.is_resizable()
    }

    /// Sets the enabled_buttons of the title bar.
    #[inline]
    pub fn set_enabled_buttons(&self, close: bool, minimize: bool, maximize: bool) {
        let mut buttons = WindowButtons::empty();
        if close {
            buttons.toggle(WindowButtons::CLOSE);
        };
        if minimize {
            buttons.toggle(WindowButtons::MINIMIZE);
        };
        if maximize {
            buttons.toggle(WindowButtons::MAXIMIZE);
        };
        self.window.set_enabled_buttons(buttons)
    }

    /// Returns the enabled buttons on the title bar.
    /// (bool, bool, bool) -> close, minimize, maximize buttons
    #[inline]
    pub fn enabled_buttons(&self) -> (bool, bool, bool) {
        let mut result = (false, false, false);
        let enabled = self.window.enabled_buttons();
        if enabled.contains(WindowButtons::CLOSE) {
            result.0 = true;
        };
        if enabled.contains(WindowButtons::MINIMIZE) {
            result.1 = true;
        };
        if enabled.contains(WindowButtons::MAXIMIZE) {
            result.2 = true;
        };
        result
    }

    /// Sets whether the window should be minimized.
    #[inline]
    pub fn set_minimized(&self, minimized: bool) {
        self.window.set_minimized(minimized)
    }

    /// Returns whether the window is minimized.
    ///
    /// `None` gets returned if it couldn'd be determined if the window is minimized.
    #[inline]
    pub fn minimized(&self) -> Option<bool> {
        self.window.is_minimized()
    }

    /// Sets whether the window should be maximized.
    #[inline]
    pub fn set_maximized(&self, maximized: bool) {
        self.window.set_maximized(maximized)
    }

    /// Returns whether the window is maximized.
    #[inline]
    pub fn maximized(&self) -> bool {
        self.window.is_maximized()
    }

    /// Sets whether the window should be in fullscreen.
    #[inline]
    pub fn set_fullscreen(&self, fullscreen: bool) {
        let fullscreen = if fullscreen {
            Some(Fullscreen::Borderless(None))
        } else {
            None
        };
        self.window.set_fullscreen(fullscreen)
    }

    /// Returns whether the window is fullscreen.
    #[inline]
    pub fn fullscreen(&self) -> bool {
        self.window.fullscreen().is_some()
    }

    /// Sets whether the window should have a title bar.
    #[inline]
    pub fn set_decorations(&self, decorations: bool) {
        self.window.set_decorations(decorations)
    }

    /// Returns whether the window has a title bar.
    #[inline]
    pub fn decorated(&self) -> bool {
        self.window.is_decorated()
    }

    /// Sets the window level.
    #[inline]
    pub fn set_window_level(&self, level: WindowLevel) {
        self.window.set_window_level(level)
    }

    /// Sets the window icon.
    #[inline]
    pub fn set_window_icon(&self, icon: Option<Icon>) {
        self.window.set_window_icon(icon);
    }

    /// Focuses the window.
    #[inline]
    pub fn focus(&self) {
        self.window.focus_window();
    }

    /// Returns true if the window is focused.
    #[inline]
    pub fn has_focus(&self) -> bool {
        self.window.has_focus()
    }

    /// Makes the window request for user attention with the given context.
    #[inline]
    pub fn request_user_attention(&self, request_type: Option<UserAttentionType>) {
        self.window.request_user_attention(request_type);
    }

    /// Sets the cursor icon to be the given variant.
    #[inline]
    pub fn set_cursor_icon(&self, cursor: CursorIcon) {
        self.window.set_cursor_icon(cursor);
    }

    /// Makes the window grab the cursor.
    /// Some variants don't work on some platforms.
    #[inline]
    pub fn set_cursor_grab(&self, mode: CursorGrabMode) -> Result<(), ExternalError> {
        self.window.set_cursor_grab(mode)
    }

    /// Makes the cursor invisible mostly just within the confines of the window.
    #[inline]
    pub fn set_cursor_visible(&self, mode: CursorGrabMode) -> Result<(), ExternalError> {
        self.window.set_cursor_grab(mode)
    }

    /// Drags the window with the left mouse button until it's released.
    #[inline]
    pub fn drag_window(&self) -> Result<(), ExternalError> {
        self.window.drag_window()
    }

    /// Modifies whether the window catches cursor events.
    /// If `true`, the window will catch the cursor events. If `false`, events are passed through the window such that any other window behind it receives them. By default hittest is enabled.
    #[inline]
    pub fn set_cursor_hittest(&self, hittest: bool) -> Result<(), ExternalError> {
        self.window.set_cursor_hittest(hittest)
    }

    /// Sets the clear color of the window.
    pub fn set_clear_color(&self, color: [f32; 4]) {
        self.set_transparent(color[3] != 1.0);
        self.clear_color.store(color);
    }

    pub fn clear_color(&self) -> [f32; 4] {
        self.clear_color.load()
    }
}

/// A builder describing the initial state of the window.
#[derive(Clone)]
#[must_use]
pub struct WindowBuilder {
    attributes: winit::window::WindowBuilder,
    pub(crate) clear_color: [f32; 4],
}

impl WindowBuilder {
    /// Creates a new window builder.
    pub fn new() -> Self {
        let attributes = winit::window::WindowBuilder::new().with_title("Game");
        Self {
            attributes,
            clear_color: [0.0, 0.0, 0.0, 1.0],
        }
    }

    /// Makes a new window builder using the one from the Winit crate.
    #[inline]
    pub fn from_winit_builder(builder: winit::window::WindowBuilder) -> Self {
        Self {
            attributes: builder,
            clear_color: [0.0, 0.0, 0.0, 1.0],
        }
    }

    /// Sets the inner size of the window.
    #[inline]
    pub fn inner_size(mut self, size: impl Into<Size>) -> Self {
        self.attributes = self.attributes.with_inner_size(size);
        self
    }

    /// Restricts the inner size of the window to not go past the given size.
    #[inline]
    pub fn max_inner_size(mut self, size: impl Into<Size>) -> Self {
        self.attributes = self.attributes.with_max_inner_size(size);
        self
    }

    /// Restricts the inner size of the window to not go below the given size.
    #[inline]
    pub fn min_inner_size(mut self, size: impl Into<Size>) -> Self {
        self.attributes = self.attributes.with_min_inner_size(size);
        self
    }

    /// Moves the window to the given position.
    ///
    /// Works on windows, mac and x11 but not wayland.
    #[inline]
    pub fn position(mut self, position: impl Into<Position>) -> Self {
        self.attributes = self.attributes.with_position(position);
        self
    }

    /// Makes the window resizable.
    #[inline]
    pub fn resizable(mut self, resizable: bool) -> Self {
        self.attributes = self.attributes.with_resizable(resizable);
        self
    }

    /// Enables the given buttons on the title bar.
    #[inline]
    pub fn enabled_buttons(mut self, close: bool, minimize: bool, maximize: bool) -> Self {
        let mut buttons = WindowButtons::empty();
        if close {
            buttons.toggle(WindowButtons::CLOSE);
        };
        if minimize {
            buttons.toggle(WindowButtons::MINIMIZE);
        };
        if maximize {
            buttons.toggle(WindowButtons::MAXIMIZE);
        };
        self.attributes = self.attributes.with_enabled_buttons(buttons);
        self
    }

    /// Sets the title of the window seen on the title bar.
    #[inline]
    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.attributes = self.attributes.with_title(title);
        self
    }

    // Add more modes.
    /// Sets the window to borderless fullscreen on the current monitor.
    #[inline]
    pub fn fullscreen(mut self, fullscreen: bool) -> Self {
        let fullscreen = if fullscreen {
            Some(Fullscreen::Borderless(None))
        } else {
            None
        };
        self.attributes = self.attributes.with_fullscreen(fullscreen);
        self
    }

    /// Request that the window is maximized upon creation.
    #[inline]
    pub fn maximized(mut self, maximized: bool) -> Self {
        self.attributes = self.attributes.with_maximized(maximized);
        self
    }

    /// Sets the window to be visible upon creation.
    #[inline]
    pub fn visible(mut self, visible: bool) -> Self {
        self.attributes = self.attributes.with_visible(visible);
        self
    }

    /// Sets the clear color of the window.
    pub fn clear_color(mut self, color: [f32; 4]) -> Self {
        self.attributes = self.attributes.with_transparent(color[3] != 1.0);
        self.clear_color = color;
        self
    }

    /// Gives the window a title bar and buttons.
    #[inline]
    pub fn decorations(mut self, decorations: bool) -> Self {
        self.attributes = self.attributes.with_decorations(decorations);
        self
    }

    /// The ordering of the window.
    #[inline]
    pub fn window_level(mut self, level: WindowLevel) -> Self {
        self.attributes = self.attributes.with_window_level(level);
        self
    }

    /// Sets the icon of the window application.
    #[inline]
    pub fn icon(mut self, icon: Option<Icon>) -> Self {
        self.attributes = self.attributes.with_window_icon(icon);
        self
    }

    /// Build window with resize increments hint.
    #[inline]
    pub fn resize_increments(mut self, increments: impl Into<Size>) -> Self {
        self.attributes = self.attributes.with_resize_increments(increments);
        self
    }

    /// Should the window be initially active?
    #[inline]
    pub fn active(mut self, active: bool) -> Self {
        self.attributes = self.attributes.with_active(active);
        self
    }
}

impl From<WindowBuilder> for winit::window::WindowBuilder {
    fn from(val: WindowBuilder) -> Self {
        val.attributes
    }
}

impl Default for WindowBuilder {
    fn default() -> Self {
        Self::new()
    }
}
