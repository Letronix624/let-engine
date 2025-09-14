//! General window stuff.
//!
//! Multiple structs to change the properties of a Window.
use glam::{IVec2, UVec2, Vec2, uvec2, vec2};
use std::sync::Arc;
pub use winit::window::{CursorGrabMode, CursorIcon, Icon, UserAttentionType, WindowLevel};
use winit::{dpi::*, error::ExternalError, window::WindowButtons};

/// A struct representing the window.
#[derive(Debug, Clone)]
pub struct Window {
    window: Arc<winit::window::Window>,
}

impl Window {
    pub(crate) fn new(window: Arc<winit::window::Window>) -> Self {
        Self { window }
    }

    #[inline]
    pub(crate) fn pre_present_notify(&self) {
        self.window.pre_present_notify();
    }

    /// Requests the window to be redrawn.
    #[inline]
    pub fn request_redraw(&self) {
        self.window.request_redraw();
    }

    /// Returns the inner size of the window in pixels.
    #[inline]
    pub fn inner_size(&self) -> UVec2 {
        Size(self.window.inner_size().into()).into()
    }

    /// Sets the size of the window in pixels. This unmaximizes the window in case it is.
    #[inline]
    pub fn set_inner_size(&self, size: UVec2) {
        let _ = self.window.request_inner_size(Size::from_uvec2(size));
    }

    /// Returns the outer size of the window in pixels.
    #[inline]
    pub fn outer_size(&self) -> UVec2 {
        Size(self.window.outer_size().into()).into()
    }

    /// Restricts the window to not go smaller than the given size in pixels.
    #[inline]
    pub fn set_min_inner_size(&self, size: Option<UVec2>) {
        self.window.set_min_inner_size(size.map(Size::from_uvec2));
    }

    /// Restricts the window to not go bigger than the given size in pixels.
    #[inline]
    pub fn set_max_inner_size(&self, size: Option<UVec2>) {
        self.window.set_max_inner_size(size.map(Size::from_uvec2));
    }

    /// Returns the increments in which the window gets resized in pixels.
    #[inline]
    pub fn resize_increments(&self) -> Option<UVec2> {
        self.window
            .resize_increments()
            .map(|x| Size(x.into()).into())
    }

    /// Sets the increments in which the window gets resized in pixels.
    #[inline]
    pub fn set_resize_increments(&self, increments: Option<UVec2>) {
        self.window
            .set_resize_increments(increments.map(Size::from_uvec2));
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

    /// Sets whether the window should be visible.
    #[inline]
    pub fn set_visible(&self, visible: bool) {
        self.window.set_visible(visible)
    }

    /// Returns whether the window is visible.
    ///
    /// `None` means it can not be determined if the window is visible or not.
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
    /// `None` gets returned if it could not be determined if the window is minimized.
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
    pub fn set_fullscreen(&self, fullscreen: Option<Fullscreen>) {
        self.window.set_fullscreen(fullscreen.map(|x| x.into()))
    }

    /// Returns whether the window is fullscreen.
    #[inline]
    pub fn fullscreen(&self) -> Option<Fullscreen> {
        self.window.fullscreen().map(|x| x.into())
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
    pub fn set_cursor(&self, cursor: CursorIcon) {
        self.window.set_cursor(cursor);
    }

    /// Makes the window grab the cursor.
    /// Some variants do not work on some platforms.
    #[inline]
    pub fn set_cursor_grab(&self, mode: CursorGrabMode) -> Result<(), ExternalError> {
        self.window.set_cursor_grab(mode)
    }

    /// Makes the cursor invisible mostly just within the confines of the window.
    #[inline]
    pub fn set_cursor_visible(&self, visible: bool) {
        self.window.set_cursor_visible(visible)
    }

    /// Drags the window with the left mouse button until it's released.
    #[inline]
    pub fn drag_window(&self) -> Result<(), ExternalError> {
        self.window.drag_window()
    }

    /// Modifies whether the window catches cursor events.
    /// If `true`, the window will catch the cursor events. If `false`, events are passed through the window such that any
    /// other window behind it receives them. By default hittest is enabled.
    #[inline]
    pub fn set_cursor_hittest(&self, hittest: bool) -> Result<(), ExternalError> {
        self.window.set_cursor_hittest(hittest)
    }

    /// Sets if the window can be transparent or not.
    pub fn set_transparent(&self, transparent: bool) {
        self.window.set_transparent(transparent);
    }

    /// Returns all the monitors that are available.
    pub fn monitors(&self) -> Vec<Monitor> {
        self.window
            .available_monitors()
            .map(|handle| Monitor { handle })
            .collect()
    }

    /// Returns the current monitor where the window is inside right now if it can.
    pub fn currect_monitor(&self) -> Option<Monitor> {
        self.window
            .current_monitor()
            .map(|handle| Monitor { handle })
    }
}

/// A builder describing the initial state of the window.
#[derive(Clone, Debug)]
#[must_use]
pub struct WindowBuilder(winit::window::WindowAttributes);

impl WindowBuilder {
    /// Creates a new window builder.
    pub fn new() -> Self {
        let attributes = winit::window::WindowAttributes::default().with_title("Game");
        Self(attributes)
    }

    /// Makes a new window builder using the one from the Winit crate.
    #[inline]
    pub fn from_winit_attributes(attributes: winit::window::WindowAttributes) -> Self {
        Self(attributes)
    }

    /// Sets the inner size of the window in pixels.
    #[inline]
    pub fn inner_size(mut self, size: UVec2) -> Self {
        self.0 = self.0.with_inner_size(Size::from(size));
        self
    }

    /// Restricts the inner size of the window to not go past the given size in pixels.
    #[inline]
    pub fn max_inner_size(mut self, size: UVec2) -> Self {
        self.0 = self.0.with_max_inner_size(Size::from(size));
        self
    }

    /// Restricts the inner size of the window to not go below the given size in pixels.
    #[inline]
    pub fn min_inner_size(mut self, size: UVec2) -> Self {
        self.0 = self.0.with_min_inner_size(Size::from(size));
        self
    }

    /// Moves the window to the given position in pixels.
    ///
    /// Works on windows, mac and x11 but not on others.
    #[inline]
    pub fn position(mut self, position: IVec2) -> Self {
        let position = Position::Physical(PhysicalPosition {
            x: position.x,
            y: position.y,
        });
        self.0 = self.0.with_position(position);
        self
    }

    /// Makes the window resizable.
    #[inline]
    pub fn resizable(mut self, resizable: bool) -> Self {
        self.0 = self.0.with_resizable(resizable);
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
        self.0 = self.0.with_enabled_buttons(buttons);
        self
    }

    /// Sets the title of the window seen on the title bar.
    #[inline]
    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.0 = self.0.with_title(title);
        self
    }

    // Add more modes.
    /// Sets the window to borderless fullscreen on the current monitor.
    #[inline]
    pub fn fullscreen(mut self, fullscreen: Option<Fullscreen>) -> Self {
        self.0 = self.0.with_fullscreen(fullscreen.map(|x| x.into()));
        self
    }

    /// Request that the window is maximized upon creation.
    #[inline]
    pub fn maximized(mut self, maximized: bool) -> Self {
        self.0 = self.0.with_maximized(maximized);
        self
    }

    /// Sets the window to be visible upon creation.
    #[inline]
    pub fn visible(mut self, visible: bool) -> Self {
        self.0.visible = visible;
        self
    }

    /// Sets if the window can be seen through.
    pub fn transparent(mut self, transparent: bool) -> Self {
        self.0 = self.0.with_transparent(transparent);
        self
    }

    /// Gives the window a title bar and buttons.
    #[inline]
    pub fn decorations(mut self, decorations: bool) -> Self {
        self.0 = self.0.with_decorations(decorations);
        self
    }

    /// The ordering of the window.
    #[inline]
    pub fn window_level(mut self, level: WindowLevel) -> Self {
        self.0 = self.0.with_window_level(level);
        self
    }

    /// Sets the icon of the window application.
    #[inline]
    pub fn icon(mut self, icon: Option<Icon>) -> Self {
        self.0 = self.0.with_window_icon(icon);
        self
    }

    /// Build window with resize increments hint in pixels.
    #[inline]
    pub fn resize_increments(mut self, increments: UVec2) -> Self {
        self.0 = self.0.with_resize_increments(Size::from(increments));
        self
    }

    /// Should the window be initially active?
    #[inline]
    pub fn active(mut self, active: bool) -> Self {
        self.0 = self.0.with_active(active);
        self
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Fullscreen {
    Exclusive(VideoModeHandle),
    Borderless(Option<Monitor>),
}

impl From<Fullscreen> for winit::window::Fullscreen {
    fn from(value: Fullscreen) -> Self {
        match value {
            Fullscreen::Exclusive(mode) => winit::window::Fullscreen::Exclusive(mode.video_mode),
            Fullscreen::Borderless(monitor) => {
                winit::window::Fullscreen::Borderless(monitor.map(|x| x.handle))
            }
        }
    }
}

impl From<winit::window::Fullscreen> for Fullscreen {
    fn from(value: winit::window::Fullscreen) -> Self {
        match value {
            winit::window::Fullscreen::Exclusive(video_mode) => {
                Fullscreen::Exclusive(VideoModeHandle { video_mode })
            }
            winit::window::Fullscreen::Borderless(handle) => {
                Fullscreen::Borderless(handle.map(|handle| Monitor { handle }))
            }
        }
    }
}

/// A representation of a monitor.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Monitor {
    handle: winit::monitor::MonitorHandle,
}

impl Monitor {
    /// Returns the name of the monitor in case it it still connected.
    pub fn name(&self) -> Option<String> {
        self.handle.name()
    }

    /// Returns the resolution of the monitor.
    pub fn size(&self) -> Vec2 {
        let size = self.handle.size();
        vec2(size.width as f32, size.height as f32)
    }

    /// Returns the position of the top left corner of the monitor relative to the larger full screen  area.
    pub fn position(&self) -> Vec2 {
        let position = self.handle.position();
        vec2(position.x as f32, position.y as f32)
    }

    /// Returns the refresh rate of the monitor in case it is still connected.
    pub fn refresh_rate(&self) -> Option<u32> {
        self.handle.refresh_rate_millihertz()
    }

    /// Returns the scaling factor of the monitor.
    pub fn scale_factor(&self) -> f64 {
        self.handle.scale_factor()
    }

    /// Returns all the video modes of this monitor.
    pub fn video_modes(&self) -> Vec<VideoModeHandle> {
        self.handle
            .video_modes()
            .map(|video_mode| VideoModeHandle { video_mode })
            .collect()
    }
}

/// Exclusive fullscreen video modes for specific monitors.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct VideoModeHandle {
    video_mode: winit::monitor::VideoModeHandle,
}

impl VideoModeHandle {
    /// Returns the resolution of this video mode.
    pub fn size(&self) -> Vec2 {
        let size = self.video_mode.size();
        vec2(size.width as f32, size.height as f32)
    }

    /// Returns the refresh rate of this monitor in millihertz
    pub fn refresh_rate(&self) -> u32 {
        self.video_mode.refresh_rate_millihertz()
    }
}

impl From<WindowBuilder> for winit::window::WindowAttributes {
    fn from(val: WindowBuilder) -> Self {
        val.0
    }
}

impl From<Arc<winit::window::Window>> for Window {
    fn from(value: Arc<winit::window::Window>) -> Self {
        Self { window: value }
    }
}

impl Default for WindowBuilder {
    fn default() -> Self {
        Self::new()
    }
}

struct Size(winit::dpi::Size);

impl Size {
    fn from_uvec2(uvec2: UVec2) -> Self {
        uvec2.into()
    }
}

impl From<UVec2> for Size {
    fn from(value: UVec2) -> Self {
        Size(winit::dpi::Size::Physical(PhysicalSize {
            width: value.x,
            height: value.y,
        }))
    }
}

impl From<Size> for UVec2 {
    fn from(value: Size) -> Self {
        uvec2(
            value.0.to_physical(1.0).width,
            value.0.to_physical(1.0).height,
        )
    }
}

impl From<Size> for winit::dpi::Size {
    fn from(value: Size) -> winit::dpi::Size {
        value.0
    }
}
