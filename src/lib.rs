#[cfg(target_os = "macos")]
use crate::config::TitleBarStyle;
use crate::{
    config::{Color, WindowSizeConstraints},
    wrappers::TaoIcon,
};
use dpi::{LogicalPosition, LogicalSize, PhysicalSize, Size};
use std::fmt;
#[cfg(target_os = "macos")]
use tao::platform::macos::WindowBuilderExtMacOS;
#[cfg(any(
    target_os = "linux",
    target_os = "dragonfly",
    target_os = "freebsd",
    target_os = "netbsd",
    target_os = "openbsd"
))]
use tao::platform::unix::WindowBuilderExtUnix;
#[cfg(windows)]
use tao::platform::windows::WindowBuilderExtWindows;
use tao::window::{Fullscreen, Theme as TaoTheme, WindowBuilder as TaoWindowBuilder};
#[cfg(target_os = "macos")]
use taurino_core::dpi::Position;
#[cfg(any(
    target_os = "linux",
    target_os = "dragonfly",
    target_os = "freebsd",
    target_os = "netbsd",
    target_os = "openbsd"
))]
use taurino_core::gtk;
#[cfg(windows)]
use taurino_core::windows::Win32::Foundation::HWND;
use taurino_core::{
    anyhow,
    dpi::{self, Theme},
    image::Icon,
    tao,
};
pub mod config;
pub mod utils;
pub mod webview;
pub mod window;
pub mod wrappers;

/// Builder used to configure and create a native application window.
///
/// It wraps Tao's `WindowBuilder` and adds Taurino-specific configuration
/// such as centering, overflow prevention, and macOS tabbing identity.
#[derive(Clone, Default)]
pub struct WindowBuilder {
    /// Underlying Tao window builder that receives the platform configuration.
    pub inner: TaoWindowBuilder,

    /// Whether the window should be centered after creation.
    pub center: bool,

    /// Optional margin used to keep the window inside the monitor's working area.
    pub prevent_overflow: Option<Size>,

    /// macOS tabbing identifier used to group windows into native tabs.
    #[cfg(target_os = "macos")]
    pub tabbing_identifier: Option<String>,
}

impl std::fmt::Debug for WindowBuilder {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut s = f.debug_struct("WindowBuilder");
        s.field("inner", &self.inner)
            .field("center", &self.center)
            .field("prevent_overflow", &self.prevent_overflow);
        #[cfg(target_os = "macos")]
        {
            s.field("tabbing_identifier", &self.tabbing_identifier);
        }
        s.finish()
    }
}

impl WindowBuilder {
    /// Creates a new builder with sensible defaults.
    ///
    /// The builder starts focused, is titled "Taurino App", and applies
    /// platform-specific defaults (e.g. a visible title bar style on macOS
    /// and a window class name on Windows).
    pub fn new() -> Self {
        #[allow(unused_mut)]
        let mut builder = Self::default().focused(true);
        #[cfg(target_os = "macos")]
        {
            // TODO: find a proper way to prevent webview being pushed out of the window.
            // Workaround for issue: https://github.com/tauri-apps/tauri/issues/10225
            // The window requires `NSFullSizeContentViewWindowMask` flag to prevent devtools
            // pushing the content view out of the window.
            // By setting the default style to `TitleBarStyle::Visible` should fix the issue for most of the users.
            builder = builder.title_bar_style(TitleBarStyle::Visible);
        }
        builder = builder.title("Taurino App");
        #[cfg(windows)]
        {
            builder = builder.window_classname("Taurino Window");
        }
        builder
    }

    /// Marks the window to be centered after creation.
    ///
    /// The actual centering is performed later by the window creation logic.
    pub fn center(mut self) -> Self {
        self.center = true;
        self
    }

    /// Sets the initial outer position of the window in logical coordinates.
    pub fn position(mut self, x: f64, y: f64) -> Self {
        self.inner = self.inner.with_position(LogicalPosition::new(x, y));
        self
    }

    /// Sets the initial inner (client-area) size in logical coordinates.
    pub fn inner_size(mut self, width: f64, height: f64) -> Self {
        self.inner = self.inner.with_inner_size(LogicalSize::new(width, height));
        self
    }

    /// Sets the minimum inner size of the window in logical coordinates.
    pub fn min_inner_size(mut self, min_width: f64, min_height: f64) -> Self {
        self.inner = self.inner.with_min_inner_size(LogicalSize::new(min_width, min_height));
        self
    }

    /// Sets the maximum inner size of the window in logical coordinates.
    pub fn max_inner_size(mut self, max_width: f64, max_height: f64) -> Self {
        self.inner = self.inner.with_max_inner_size(LogicalSize::new(max_width, max_height));
        self
    }

    /// Applies a complete set of min/max inner size constraints at once.
    pub fn inner_size_constraints(mut self, constraints: WindowSizeConstraints) -> Self {
        self.inner.window.inner_size_constraints = tao::window::WindowSizeConstraints {
            min_width: constraints.min_width,
            min_height: constraints.min_height,
            max_width: constraints.max_width,
            max_height: constraints.max_height,
        };
        self
    }

    /// Prevent the window from overflowing the working area (e.g. monitor size - taskbar size) on creation
    ///
    /// ## Platform-specific
    ///
    /// - **iOS / Android:** Unsupported.
    pub fn prevent_overflow(mut self) -> Self {
        self.prevent_overflow.replace(PhysicalSize::new(0, 0).into());
        self
    }

    /// Prevent the window from overflowing the working area (e.g. monitor size - taskbar size)
    /// on creation with a margin
    ///
    /// ## Platform-specific
    ///
    /// - **iOS / Android:** Unsupported.
    pub fn prevent_overflow_with_margin(mut self, margin: Size) -> Self {
        self.prevent_overflow.replace(margin);
        self
    }

    /// Enables or disables the resizable flag of the window.
    pub fn resizable(mut self, resizable: bool) -> Self {
        self.inner = self.inner.with_resizable(resizable);
        self
    }

    /// Enables or disables the native maximize action.
    pub fn maximizable(mut self, maximizable: bool) -> Self {
        self.inner = self.inner.with_maximizable(maximizable);
        self
    }

    /// Enables or disables the native minimize action.
    pub fn minimizable(mut self, minimizable: bool) -> Self {
        self.inner = self.inner.with_minimizable(minimizable);
        self
    }

    /// Enables or disables the native close action.
    pub fn closable(mut self, closable: bool) -> Self {
        self.inner = self.inner.with_closable(closable);
        self
    }

    /// Sets the initial native window title.
    pub fn title<S: Into<String>>(mut self, title: S) -> Self {
        self.inner = self.inner.with_title(title.into());
        self
    }

    /// Configures the window to start in borderless fullscreen mode or in
    /// windowed mode.
    pub fn fullscreen(mut self, fullscreen: bool) -> Self {
        self.inner = if fullscreen {
            self.inner.with_fullscreen(Some(Fullscreen::Borderless(None)))
        } else {
            self.inner.with_fullscreen(None)
        };
        self
    }

    /// Sets whether the window should receive focus on creation.
    pub fn focused(mut self, focused: bool) -> Self {
        self.inner = self.inner.with_focused(focused);
        self
    }

    /// Sets whether the window is allowed to be focused at all.
    pub fn focusable(mut self, focusable: bool) -> Self {
        self.inner = self.inner.with_focusable(focusable);
        self
    }

    /// Sets whether the window should start maximized.
    pub fn maximized(mut self, maximized: bool) -> Self {
        self.inner = self.inner.with_maximized(maximized);
        self
    }

    /// Sets whether the window should be visible immediately after creation.
    pub fn visible(mut self, visible: bool) -> Self {
        self.inner = self.inner.with_visible(visible);
        self
    }

    /// Enables or disables transparent window rendering.
    pub fn transparent(mut self, transparent: bool) -> Self {
        self.inner = self.inner.with_transparent(transparent);
        self
    }

    /// Enables or disables native window decorations (title bar and borders).
    pub fn decorations(mut self, decorations: bool) -> Self {
        self.inner = self.inner.with_decorations(decorations);
        self
    }

    /// Configures whether the window stays below normal windows.
    pub fn always_on_bottom(mut self, always_on_bottom: bool) -> Self {
        self.inner = self.inner.with_always_on_bottom(always_on_bottom);
        self
    }

    /// Configures whether the window stays above normal windows.
    pub fn always_on_top(mut self, always_on_top: bool) -> Self {
        self.inner = self.inner.with_always_on_top(always_on_top);
        self
    }

    /// Configures whether the window is visible on all workspaces.
    pub fn visible_on_all_workspaces(mut self, visible_on_all_workspaces: bool) -> Self {
        self.inner = self.inner.with_visible_on_all_workspaces(visible_on_all_workspaces);
        self
    }

    /// Enables or disables protection against capture of the window contents by other apps.
    pub fn content_protected(mut self, protected: bool) -> Self {
        self.inner = self.inner.with_content_protection(protected);
        self
    }

    /// Enables or disables the native window shadow on supported platforms.
    ///
    /// On Windows this maps to the undecorated shadow; on macOS this maps to
    /// the native window shadow flag.
    pub fn shadow(#[allow(unused_mut)] mut self, _enable: bool) -> Self {
        #[cfg(windows)]
        {
            self.inner = self.inner.with_undecorated_shadow(_enable);
        }
        #[cfg(target_os = "macos")]
        {
            self.inner = self.inner.with_has_shadow(_enable);
        }
        self
    }

    /// Sets the native owner window on Windows.
    #[cfg(windows)]
    pub fn owner(mut self, owner: HWND) -> Self {
        self.inner = self.inner.with_owner_window(owner.0 as _);
        self
    }

    /// Sets the native parent window on Windows.
    #[cfg(windows)]
    pub fn parent(mut self, parent: HWND) -> Self {
        self.inner = self.inner.with_parent_window(parent.0 as _);
        self
    }

    /// Sets the native parent window on macOS.
    #[cfg(target_os = "macos")]
    pub fn parent(mut self, parent: *mut std::ffi::c_void) -> Self {
        self.inner = self.inner.with_parent_window(parent);
        self
    }

    /// Sets the transient parent window on Linux/BSD (GTK).
    #[cfg(any(
        target_os = "linux",
        target_os = "dragonfly",
        target_os = "freebsd",
        target_os = "netbsd",
        target_os = "openbsd"
    ))]
    pub fn transient_for(mut self, parent: &impl gtk::glib::IsA<gtk::Window>) -> Self {
        self.inner = self.inner.with_transient_for(parent);
        self
    }

    /// Enables or disables native drag-and-drop on Windows.
    #[cfg(windows)]
    pub fn drag_and_drop(mut self, enabled: bool) -> Self {
        self.inner = self.inner.with_drag_and_drop(enabled);
        self
    }

    /// Applies the selected macOS title-bar transparency and content layout behavior.
    ///
    /// The three variants control title-bar transparency and fullsize content
    /// view independently to achieve visible, transparent or overlay styles.
    #[cfg(target_os = "macos")]
    pub fn title_bar_style(mut self, style: TitleBarStyle) -> Self {
        match style {
            TitleBarStyle::Visible => {
                self.inner = self.inner.with_titlebar_transparent(false);
                // Fixes rendering issue when resizing window with devtools open (https://github.com/tauri-apps/tauri/issues/3914)
                self.inner = self.inner.with_fullsize_content_view(true);
            }
            TitleBarStyle::Transparent => {
                self.inner = self.inner.with_titlebar_transparent(true);
                self.inner = self.inner.with_fullsize_content_view(false);
            }
            TitleBarStyle::Overlay => {
                self.inner = self.inner.with_titlebar_transparent(true);
                self.inner = self.inner.with_fullsize_content_view(true);
            }
            #[allow(unreachable_patterns)]
            unknown => {
                eprintln!("unknown title bar style applied: {unknown}");
            }
        }
        self
    }

    /// Moves the macOS traffic-light window controls to the supplied inset position.
    #[cfg(target_os = "macos")]
    pub fn traffic_light_position<P: Into<Position>>(mut self, position: P) -> Self {
        self.inner = self.inner.with_traffic_light_inset(position.into());
        self
    }

    /// Hides or shows the macOS window title while keeping the title bar.
    #[cfg(target_os = "macos")]
    pub fn hidden_title(mut self, hidden: bool) -> Self {
        self.inner = self.inner.with_title_hidden(hidden);
        self
    }

    /// Sets the macOS tabbing identifier used to group windows into native tabs.
    #[cfg(target_os = "macos")]
    pub fn tabbing_identifier(mut self, identifier: &str) -> Self {
        self.inner = self.inner.with_tabbing_identifier(identifier);
        self.tabbing_identifier.replace(identifier.into());
        self
    }

    /// Converts and applies the supplied image as the initial native window icon.
    pub fn icon(mut self, icon: Icon) -> anyhow::Result<Self> {
        self.inner = self.inner.with_window_icon(Some(TaoIcon::try_from(icon)?.0));
        Ok(self)
    }

    /// Sets the initial native background color of the window.
    pub fn background_color(mut self, color: Color) -> Self {
        self.inner = self.inner.with_background_color(color.into());
        self
    }

    /// Configures whether the window is omitted from the taskbar on supported platforms.
    #[cfg(any(
        windows,
        target_os = "linux",
        target_os = "dragonfly",
        target_os = "freebsd",
        target_os = "netbsd",
        target_os = "openbsd"
    ))]
    pub fn skip_taskbar(mut self, skip: bool) -> Self {
        self.inner = self.inner.with_skip_taskbar(skip);
        self
    }

    /// No-op on macOS, iOS and Android, where taskbar skipping is unsupported.
    #[cfg(any(target_os = "macos", target_os = "ios", target_os = "android"))]
    pub fn skip_taskbar(self, _skip: bool) -> Self {
        self
    }

    /// Sets the preferred native window theme or restores system theme selection.
    pub fn theme(mut self, theme: Option<Theme>) -> Self {
        self.inner = self.inner.with_theme(if let Some(t) = theme {
            match t {
                Theme::Dark => Some(TaoTheme::Dark),
                _ => Some(TaoTheme::Light),
            }
        } else {
            None
        });
        self
    }

    /// Returns whether an initial window icon has been configured.
    pub fn has_icon(&self) -> bool {
        self.inner.window.window_icon.is_some()
    }

    /// Returns the currently configured preferred theme, if any.
    pub fn get_theme(&self) -> Option<Theme> {
        self.inner.window.preferred_theme.map(|theme| match theme {
            TaoTheme::Dark => Theme::Dark,
            _ => Theme::Light,
        })
    }

    /// Sets the Windows window class name.
    #[cfg(windows)]
    pub fn window_classname<S: Into<String>>(mut self, window_classname: S) -> Self {
        self.inner = self.inner.with_window_classname(window_classname);
        self
    }

    /// No-op on non-Windows platforms, where a window class name is not applicable.
    #[cfg(not(windows))]
    pub fn window_classname<S: Into<String>>(self, _window_classname: S) -> Self {
        self
    }

    /// Enables or disables the no-redirection-bitmap flag on Windows.
    ///
    /// This is a no-op on other platforms.
    pub fn no_redirection_bitmap(#[allow(unused_mut)] mut self, _enable: bool) -> Self {
        #[cfg(windows)]
        {
            self.inner = self.inner.with_no_redirection_bitmap(_enable);
        }
        self
    }

    /// Sets the Android activity name that should host the window.
    #[cfg(target_os = "android")]
    pub fn activity_name<S: Into<String>>(mut self, class_name: S) -> Self {
        self.inner = self.inner.with_activity_name(class_name.into());
        self
    }

    /// Sets the Android activity name that created the window.
    #[cfg(target_os = "android")]
    pub fn created_by_activity_name<S: Into<String>>(mut self, class_name: S) -> Self {
        self.inner = self.inner.with_created_by_activity_name(class_name.into());
        self
    }

    /// Sets the iOS scene identifier that requested the window.
    #[cfg(target_os = "ios")]
    pub fn requested_by_scene_identifier<S: Into<String>>(mut self, identifier: S) -> Self {
        self.inner = self.inner.with_requesting_scene_identifier(identifier.into());
        self
    }
}
