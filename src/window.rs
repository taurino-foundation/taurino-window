/// Represents a native application window and its window-scoped resources.

///

/// `Window` is the engine-level wrapper around a Tao [`tao::window::Window`].

/// Besides the native window handle, it owns or references the resources whose

/// lifetime is associated with that window, including WebViews, window-local

/// menu state and platform-specific rendering or focus state.

///

/// The type forms the boundary between Taurino's window abstraction and the

/// underlying platform window supplied by Tao.

///

/// # Responsibilities

///

/// A `Window` is responsible for:

///

/// - identifying the window through its Taurino [`WindowId`] and human-readable

///   label;

/// - providing controlled access to the underlying Tao window;

/// - owning the [`WebViewManager`] associated with the window;

/// - tracking whether child WebViews are attached;

/// - retaining the window's menu association;

/// - exposing platform-independent window state and operations;

/// - maintaining platform-specific state required for native integration;

/// - coordinating destruction of resources that must be released before the

///   native window handle is dropped.

///

/// # Ownership and lifetime

///

/// The native Tao window is stored as:

///

/// ```text

/// Option<Arc<Tao>>

/// ```

///

/// rather than as an unconditional window value.

///

/// This allows the native handle to be explicitly removed during shutdown while

/// keeping the surrounding `Window` object alive long enough to complete

/// registry cleanup, event processing or other teardown operations.

///

/// Once the native handle has been removed, methods requiring access to the

/// native window return an error through [`Window::tao`].

///

/// External [`Arc`] clones of the Tao window or associated WebViews may extend

/// the lifetime of those resources beyond the lifetime of the references owned

/// directly by this wrapper.

///

/// Consequently, removing resources from `Window` does not necessarily imply

/// immediate destruction of every corresponding native object.

///

/// # WebViews

///

/// WebViews belonging to the window are managed through [`WebViewManager`].

///

/// `has_children` provides a lightweight atomic indication of whether child

/// WebViews currently participate in the window layout. Some platform-specific

/// behavior, such as focus handling or effective client-area calculation, may

/// depend on this state.

///

/// WebViews must be released before the locally owned Tao window handle during

/// explicit destruction so that resources depending on the native window do

/// not outlive their local owner.

///

/// # Menus

///

/// The `menu` field stores the menu association for this particular window.

///

/// It does not necessarily represent ownership of the complete native menu

/// lifecycle. In particular, an application-wide menu may remain owned by the

/// global menu manager even after this window is destroyed.

///

/// Therefore removing a menu reference from `Window` must not automatically be

/// interpreted as destroying an application-wide menu.

///

/// # Platform-specific state

///

/// Some state only exists on platforms where additional native behavior is

/// required.

///

/// On Windows this includes:

///

/// - a `softbuffer` surface used for transparent-window drawing;

/// - the configured background color;

/// - whether the window is transparent;

/// - explicit focus state used when child WebViews participate in focus

///   handling.

///

/// These resources are conditionally compiled and are not part of the runtime

/// representation on other platforms.

///

/// # Destruction

///

/// [`Window::destroy`] performs local resource teardown in dependency order.

///

/// In particular, dependent resources such as WebViews and the Windows drawing

/// surface are released before the locally owned Tao window handle is removed.

/// This preserves the native window while resources that depend on it are being

/// cleaned up.

///

/// `destroy()` does not itself represent the complete application-level window

/// close procedure. Manager registrations, menu integration and higher-level

/// close policy should be handled before calling it.

///

/// Likewise, [`Window::close`] only schedules a close request through the event

/// loop. It does not synchronously destroy the window.

///

/// # Threading

///

/// Native GUI resources are generally event-loop-thread-bound.

///

/// Although some fields use [`Arc`], [`Mutex`] or atomics for shared state,

/// those synchronization primitives do not make arbitrary native Tao or WebView

/// operations valid from background threads.

///

/// Operations affecting the native window should therefore normally be

/// performed through the application's GUI event-loop path.

///

/// # Locking

///

/// Mutable auxiliary state is protected by small, independent mutexes.

///

/// Locks should be held only while reading or replacing the corresponding

/// state. Native window calls, WebView destruction and other potentially

/// re-entrant operations should not be performed while unrelated state locks

/// are held.

///

/// [`lock_state`] converts poisoned mutexes into regular engine errors rather

/// than recovering the poisoned guard.

///

/// # Invariants

///

/// During normal operation:

///

/// - `inner` contains the native Tao window;

/// - `id` remains the stable Taurino identifier assigned at construction;

/// - `label` remains the application-visible window label;

/// - `webviews_manager` contains the WebViews owned by this window;

/// - `has_children` reflects whether child WebViews currently participate in

///   the window;

/// - platform-specific resources refer to the same native window represented by

///   `inner`.

///

/// After explicit destruction, `inner` may be `None`. Code must therefore not

/// assume that the existence of a `Window` wrapper implies that the native

/// window is still available.
use std::sync::{
    Arc, Mutex, MutexGuard,
    atomic::{AtomicBool, Ordering},
};

use taurino_core::{
    WebViewId, WindowExt, WindowId,
    anyhow::{Result, anyhow},
    dpi::{PhysicalPosition, PhysicalSize, Position, Size, Theme},
    image::Icon,
    muda::MenuId,
    tao::{self, event_loop::EventLoopProxy, window::Window as Tao},
};

#[cfg(any(
    target_os = "linux",
    target_os = "dragonfly",
    target_os = "freebsd",
    target_os = "netbsd",
    target_os = "openbsd"
))]
use taurino_core::gtk;

#[cfg(target_os = "android")]
use tao::platform::android::WindowExtAndroid;

#[cfg(target_os = "ios")]
use tao::platform::ios::WindowExtIOS;

#[cfg(target_os = "macos")]
use tao::platform::macos::WindowExtMacOS;

#[cfg(any(
    target_os = "linux",
    target_os = "dragonfly",
    target_os = "freebsd",
    target_os = "netbsd",
    target_os = "openbsd"
))]
use tao::platform::unix::WindowExtUnix;

#[cfg(windows)]
use tao::platform::windows::WindowExtWindows;

#[cfg(windows)]
use taurino_core::{arc_mut, softbuffer};

use taurino_core::raw_window_handle::{DisplayHandle, HandleError, HasDisplayHandle, HasWindowHandle, WindowHandle};

use taurino_menu::{
    WindowMenu,
    prelude::{
        CheckMenuItem, IconMenuItem, IsMenuItem, Menu, MenuItem, MenuItemKind, NativeIcon, PredefinedMenuItem, Submenu,
    },
};

#[cfg(windows)]
use crate::config::FocusState;

// Import path corrected: `TitleBarStyle` lives in `config`, not in `types`.

#[cfg(target_os = "macos")]
use crate::config::TitleBarStyle;

use crate::{
    config::{Color, CursorIcon, Monitor, ProgressBarState, ResizeDirection, UserAttentionType, WindowSizeConstraints},
    webview::{WebView, WebViewManager, inner_size},
    wrappers::{CursorIconWrapper, MonitorHandleWrapper, ProgressBarStateWrapper, UserAttentionTypeWrapper},
};

/// GUI-thread-bound wrapper. The WebViews make it non-Send/Sync.

///

/// WebViews and surface are placed before `inner` so that their local owners are

/// released before the local Tao handle during regular drop.

/// External clones can keep the respective resources alive further.

pub struct Window {
    /// Manages all WebViews associated with this window.

    ///

    /// WebViews are window-scoped resources and are released as part of the

    /// controlled window destruction sequence.
    pub webviews_manager: WebViewManager,

    /// Menu association currently attached to this window.

    ///

    /// The stored [`WindowMenu`] may represent either a window-local menu or a

    /// reference to an application-wide menu. Removing this value therefore

    /// does not necessarily mean that the underlying native menu should be

    /// destroyed.
    pub(crate) menu: Arc<Mutex<Option<WindowMenu>>>,

    /// Drawing surface used for transparent-window rendering on Windows.

    ///

    /// The surface depends on the native Tao window and must therefore be

    /// released before the locally owned native window handle during explicit

    /// destruction.
    #[cfg(windows)]
    pub surface: Arc<Mutex<Option<softbuffer::Surface<Arc<Tao>, Arc<Tao>>>>>,

    /// Stable application-visible label identifying this window.
    pub label: String,

    /// Taurino-level identifier assigned to this window.

    ///

    /// This is distinct from Tao's native [`tao::window::WindowId`].
    pub id: WindowId,

    /// Configured native background color on Windows.

    ///

    /// The value is shared with redraw handling for transparent windows.
    #[cfg(windows)]
    pub background_color: Arc<Mutex<Option<tao::window::RGBA>>>,

    /// Whether this window uses transparent native rendering on Windows.
    #[cfg(windows)]
    pub is_window_transparent: bool,

    /// Effective focus state for the window and its child WebViews on Windows.

    ///

    /// Tao's native focus state alone is insufficient when focus may reside in

    /// an embedded child WebView, so Taurino tracks the combined state

    /// explicitly.
    #[cfg(windows)]
    pub focused_webview: Arc<Mutex<FocusState>>,

    /// Indicates whether child WebViews are currently attached to the window.

    ///

    /// Atomic storage permits inexpensive reads from code paths that only need

    /// the current structural state without locking the complete WebView

    /// manager.
    pub has_children: AtomicBool,

    /// Locally owned reference to the underlying Tao window.

    ///

    /// `None` means that the native handle has already been taken or released

    /// as part of the destruction sequence. Methods requiring the native window

    /// must access it through [`Window::tao`] or one of the optional accessors.
    pub(crate) inner: Option<Arc<Tao>>,
}

impl Window {
    /// Creates a window wrapper and initializes its window-scoped platform resources.
    pub fn new(
        window: Tao,

        window_id: WindowId,

        window_label: String,

        menu: Option<WindowMenu>,

        #[cfg(windows)] background_color: Option<tao::window::RGBA>,

        #[cfg(windows)] is_window_transparent: bool,

        #[cfg(windows)] focused_webview: Arc<Mutex<FocusState>>,

        has_children: bool,

        webviews_manager: WebViewManager,
    ) -> Self {
        let inner = Arc::new(window);

        #[cfg(windows)]
        let surface = arc_mut(if is_window_transparent {
            if let Ok(context) = softbuffer::Context::new(inner.clone()) {
                if let Ok(mut surface) = softbuffer::Surface::new(&context, inner.clone()) {
                    use taurino_core::WindowExt;

                    inner.draw_surface(&mut surface, background_color);

                    Some(surface)
                } else {
                    None
                }
            } else {
                None
            }
        } else {
            None
        });

        Self {
            webviews_manager,

            menu: arc_mut(menu),

            id: window_id,

            label: window_label,

            #[cfg(windows)]
            background_color: arc_mut(background_color),

            #[cfg(windows)]
            is_window_transparent,

            #[cfg(windows)]
            surface,

            #[cfg(windows)]
            focused_webview,

            has_children: AtomicBool::new(has_children),

            inner: Some(inner),
        }
    }
}

impl Window {
    /// Returns the window label.

    /// Returns the application-visible label of this window.
    pub fn label(&self) -> &str {
        &self.label
    }

    /// Returns the internal u32 window ID (not the Tao WindowId).

    /// Returns the Taurino window identifier assigned to this window.
    pub fn id(&self) -> WindowId {
        self.id
    }

    /// Returns the native window or an error after it has been taken.

    /// Returns the underlying Tao window or an error if its native handle is no longer available.
    pub fn tao(&self) -> Result<&Arc<Tao>> {
        self.inner
            .as_ref()
            .ok_or_else(|| anyhow!("Window '{}' is not available", self.label))
    }

    /// Returns the underlying Tao window.

    ///

    /// Returns `None` if the native window has already been taken or destroyed.

    /// Returns the underlying Tao window reference when the native handle is still available.
    pub fn inner(&self) -> Option<&Arc<Tao>> {
        self.inner.as_ref()
    }

    /// Returns a cloned reference to the underlying Tao window.

    /// Returns a cloned shared reference to the underlying Tao window when available.
    pub fn inner_arc(&self) -> Option<Arc<Tao>> {
        self.inner.clone()
    }

    /// Returns whether this window currently has a menu.

    /// Returns whether this window currently has an attached menu reference.
    pub fn has_menu(&self) -> bool {
        self.menu.lock().map(|menu| menu.is_some()).unwrap_or(false)
    }

    /// Returns whether the menu attached to this window is application-wide.

    ///

    /// This is mainly relevant on macOS.

    /// Returns whether this window references an application-wide menu.
    pub fn has_app_wide_menu(&self) -> bool {
        self.menu
            .lock()
            .map(|menu| menu.as_ref().is_some_and(|menu| menu.is_app_wide))
            .unwrap_or(false)
    }

    /// Returns a clone of the menu attached to this window.

    /// Returns a cloned menu handle, or `None` if no menu is attached or the menu lock is poisoned.
    pub fn menu(&self) -> Option<Menu> {
        self.menu
            .lock()
            .ok()
            .and_then(|menu| menu.as_ref().map(|menu| menu.menu.clone()))
    }

    /// Removes the menu reference from this window and returns it.

    ///

    /// For an application-wide macOS menu this only removes this window's

    /// reference. Actual application-wide ownership should remain with the

    /// MenuManager.

    /// Removes and returns this window's stored menu reference without changing global menu ownership.
    pub fn take_window_menu(&self) -> Option<WindowMenu> {
        self.menu.lock().ok().and_then(|mut menu| menu.take())
    }

    /// Takes only the native handle; WebViews, surface and menu remain intact.

    /// For the complete close sequence use the manager and destroy().

    /// Removes and returns the native Tao window handle without destroying other window-scoped resources.
    pub fn take_inner_window(&mut self) -> Option<Arc<Tao>> {
        self.inner.take()
    }

    #[cfg(windows)]

    /// Returns the stored Windows background color.
    pub fn background_color(&self) -> Option<tao::window::RGBA> {
        *self.background_color.lock().unwrap()
    }

    #[cfg(windows)]

    /// Returns whether this Windows window is configured for transparent rendering.
    pub fn is_transparent(&self) -> bool {
        self.is_window_transparent
    }

    #[cfg(windows)]

    /// Returns the shared Windows focus state used by the window and its child WebViews.
    pub fn focused_webview(&self) -> &Arc<Mutex<FocusState>> {
        &self.focused_webview
    }
}

impl HasDisplayHandle for Window {
    /// Returns the raw display handle of the underlying native window.
    fn display_handle(&self) -> std::result::Result<DisplayHandle<'_>, HandleError> {
        self.inner.as_ref().ok_or(HandleError::Unavailable)?.display_handle()
    }
}

impl HasWindowHandle for Window {
    /// Returns the raw window handle of the underlying native window.
    fn window_handle(&self) -> std::result::Result<WindowHandle<'_>, HandleError> {
        self.inner.as_ref().ok_or(HandleError::Unavailable)?.window_handle()
    }
}

impl Window {
    // -------------------------------------------------------------------------

    // Getters

    // -------------------------------------------------------------------------

    /// Returns the scale factor that can be used to map logical pixels to physical pixels, and vice versa.

    /// Returns the native scale factor used to convert between logical and physical pixels.
    pub fn scale_factor(&self) -> Result<f64> {
        let inner = self.tao()?;

        Ok(inner.scale_factor())
    }

    /// Returns the position of the top-left hand corner of the window's client area relative to the top-left hand corner of the desktop.

    /// Returns the desktop-relative position of the window's client area.
    pub fn inner_position(&self) -> Result<PhysicalPosition<i32>> {
        let inner = self.tao()?;

        inner.inner_position().map_err(Into::into)
    }

    /// Returns the position of the top-left hand corner of the window relative to the top-left hand corner of the desktop.

    /// Returns the desktop-relative position of the complete native window.
    pub fn outer_position(&self) -> Result<PhysicalPosition<i32>> {
        let inner = self.tao()?;

        inner.outer_position().map_err(Into::into)
    }

    /// Returns the physical size of the window's client area.

    /// Returns the effective physical client-area size, accounting for child WebViews when required.
    pub fn inner_size(&self) -> Result<PhysicalSize<u32>> {
        let has_children = self.has_children();

        let inner = self.tao()?;

        Ok(inner_size(inner, &self.webviews_manager.webviews(), has_children))
    }

    /// Returns the physical size of the entire window.

    /// Returns the physical size of the complete native window.
    pub fn outer_size(&self) -> Result<PhysicalSize<u32>> {
        let inner = self.tao()?;

        Ok(inner.outer_size())
    }

    /// Gets the window's current fullscreen state.

    /// Returns whether the window is currently fullscreen.
    pub fn is_fullscreen(&self) -> Result<bool> {
        let inner = self.tao()?;

        Ok(inner.fullscreen().is_some())
    }

    /// Gets the window's current minimized state.

    /// Returns whether the window is currently minimized.
    pub fn is_minimized(&self) -> Result<bool> {
        let inner = self.tao()?;

        Ok(inner.is_minimized())
    }

    /// Gets the window's current maximized state.

    /// Returns whether the window is currently maximized.
    pub fn is_maximized(&self) -> Result<bool> {
        let inner = self.tao()?;

        Ok(inner.is_maximized())
    }

    /// Returns whether the window or, on Windows, one of its child WebViews currently owns focus.
    pub fn is_focused(&self) -> Result<bool> {
        let inner = self.tao()?;

        #[cfg(windows)]
        if self.has_children() {
            let state = lock_state(&self.focused_webview, "focused_webview")?;

            return Ok(matches!(
                &*state,
                FocusState::WindowFocused | FocusState::WebviewFocused { .. }
            ));
        }

        Ok(inner.is_focused())
    }

    /// Returns whether this window currently has child webviews.

    /// Returns whether the window currently contains child WebViews.
    pub fn has_children(&self) -> bool {
        self.has_children.load(Ordering::Acquire)
    }

    /// Sets whether this window has child webviews.

    /// Updates the atomic flag indicating whether the window contains child WebViews.
    pub fn set_has_child_webviews(&self, has_children: bool) {
        self.has_children.store(has_children, Ordering::Release);
    }

    /// Gets the window's current decoration state.

    /// Returns whether native window decorations are enabled.
    pub fn is_decorated(&self) -> Result<bool> {
        let inner = self.tao()?;

        Ok(inner.is_decorated())
    }

    /// Gets the window's current resizable state.

    /// Returns whether the window is currently resizable.
    pub fn is_resizable(&self) -> Result<bool> {
        let inner = self.tao()?;

        Ok(inner.is_resizable())
    }

    /// Gets the window's native maximize button state.

    /// Returns whether the native maximize action is enabled.
    pub fn is_maximizable(&self) -> Result<bool> {
        let inner = self.tao()?;

        Ok(inner.is_maximizable())
    }

    /// Gets the window's native minimize button state.

    /// Returns whether the native minimize action is enabled.
    pub fn is_minimizable(&self) -> Result<bool> {
        let inner = self.tao()?;

        Ok(inner.is_minimizable())
    }

    /// Gets the window's native close button state.

    /// Returns whether the native close action is enabled.
    pub fn is_closable(&self) -> Result<bool> {
        let inner = self.tao()?;

        Ok(inner.is_closable())
    }

    /// Gets the window's current visibility state.

    /// Returns whether the window is currently visible.
    pub fn is_visible(&self) -> Result<bool> {
        let inner = self.tao()?;

        Ok(inner.is_visible())
    }

    /// Whether the window is enabled or disabled.

    /// Returns whether the window currently accepts native user interaction.
    pub fn is_enabled(&self) -> Result<bool> {
        let inner = self.tao()?;

        Ok(inner.is_enabled())
    }

    /// Gets the window alwaysOnTop flag state.

    /// Returns whether the window is configured to stay above normal windows.
    pub fn is_always_on_top(&self) -> Result<bool> {
        let inner = self.tao()?;

        Ok(inner.is_always_on_top())
    }

    /// Gets the window's current title.

    /// Returns the current native window title.
    pub fn title(&self) -> Result<String> {
        let inner = self.tao()?;

        Ok(inner.title())
    }

    /// Returns the monitor on which the window currently resides.

    /// Returns the monitor that currently contains the window, if one can be determined.
    pub fn current_monitor(&self) -> Result<Option<Monitor>> {
        let inner = self.tao()?;

        Ok(inner.current_monitor().map(|m| MonitorHandleWrapper(m).into()))
    }

    /// Returns the primary monitor of the system.

    /// Returns the system's primary monitor, if one is available.
    pub fn primary_monitor(&self) -> Result<Option<Monitor>> {
        let inner = self.tao()?;

        Ok(inner.primary_monitor().map(|m| MonitorHandleWrapper(m).into()))
    }

    /// Returns the monitor that contains the given point.

    /// Returns the monitor containing the supplied desktop coordinate, if any.
    pub fn monitor_from_point(&self, x: f64, y: f64) -> Result<Option<Monitor>> {
        let inner = self.tao()?;

        Ok(inner.monitor_from_point(x, y).map(|m| MonitorHandleWrapper(m).into()))
    }

    /// Returns the list of all the monitors available on the system.

    /// Returns all monitors currently reported by the windowing system.
    pub fn available_monitors(&self) -> Result<Vec<Monitor>> {
        let inner = self.tao()?;

        Ok(inner
            .available_monitors()
            .map(|m| MonitorHandleWrapper(m).into())
            .collect())
    }

    /// Returns the `ApplicationWindow` from gtk crate that is used by this window.

    #[cfg(any(
        target_os = "linux",
        target_os = "dragonfly",
        target_os = "freebsd",
        target_os = "netbsd",
        target_os = "openbsd"
    ))]

    /// Returns the underlying GTK application window used by this Tao window.
    pub fn gtk_window(&self) -> Result<gtk::ApplicationWindow> {
        let inner = self.tao()?;

        Ok(inner.gtk_window().clone())
    }

    /// Returns the vertical [`gtk::Box`] that is added by default as the sole child of this window.

    #[cfg(any(
        target_os = "linux",
        target_os = "dragonfly",
        target_os = "freebsd",
        target_os = "netbsd",
        target_os = "openbsd"
    ))]

    /// Returns the default GTK vertical box owned by this window.
    pub fn default_vbox(&self) -> Result<gtk::Box> {
        self.tao()?
            .default_vbox()
            .cloned()
            .ok_or_else(|| anyhow!("Window '{}' has no default GTK vbox", self.label))
    }

    /// Returns the name of the Android activity associated with this window.

    #[cfg(target_os = "android")]

    /// Returns the Android activity name associated with this window.
    pub fn activity_name(&self) -> Result<String> {
        let inner = self.tao()?;

        Ok(inner.activity_name().to_string())
    }

    /// Returns the identifier of the UIScene tied to this UIWindow.

    #[cfg(target_os = "ios")]

    /// Returns the iOS scene identifier associated with this window.
    pub fn scene_identifier(&self) -> Result<String> {
        let inner = self.tao()?;

        Ok(inner.scene_identifier().to_string())
    }

    /// Returns the current window theme.

    /// Returns the current native window theme mapped to Taurino's theme type.
    pub fn theme(&self) -> Result<Theme> {
        Ok(map_theme_from_tao(self.tao()?.theme()))
    }

    // -------------------------------------------------------------------------

    // Setters

    // -------------------------------------------------------------------------

    /// Centers the window.

    /// Centers the window using the platform window manager.
    pub fn center(&self) -> Result<()> {
        let inner = self.tao()?;

        inner.center();

        Ok(())
    }

    /// Requests user attention to the window.

    /// Requests platform-specific user attention for this window.
    pub fn request_user_attention(&self, request_type: Option<UserAttentionType>) -> Result<()> {
        let inner = self.tao()?;

        inner.request_user_attention(request_type.map(|r| UserAttentionTypeWrapper::from(r).0));

        Ok(())
    }

    /// Updates the window resizable flag.

    /// Enables or disables resizing and updates undecorated resize handling on Windows.
    pub fn set_resizable(&self, resizable: bool) -> Result<()> {
        let inner = self.tao()?;

        inner.set_resizable(resizable);

        #[cfg(windows)]
        {
            if !resizable {
                use taurino_core::undecorated_resizing;

                undecorated_resizing::detach_resize_handler(inner.hwnd());
            } else if !inner.is_decorated() {
                use taurino_core::undecorated_resizing;

                undecorated_resizing::attach_resize_handler(inner.hwnd(), inner.has_undecorated_shadow());
            }
        }

        Ok(())
    }

    /// Enable or disable the window.

    /// Enables or disables native user interaction with the window.
    pub fn set_enabled(&self, enabled: bool) -> Result<()> {
        let inner = self.tao()?;

        inner.set_enabled(enabled);

        Ok(())
    }

    /// Updates the window's native maximize button state.

    /// Enables or disables the native maximize action.
    pub fn set_maximizable(&self, maximizable: bool) -> Result<()> {
        let inner = self.tao()?;

        inner.set_maximizable(maximizable);

        Ok(())
    }

    /// Updates the window's native minimize button state.

    /// Enables or disables the native minimize action.
    pub fn set_minimizable(&self, minimizable: bool) -> Result<()> {
        let inner = self.tao()?;

        inner.set_minimizable(minimizable);

        Ok(())
    }

    /// Updates the window's native close button state.

    /// Enables or disables the native close action.
    pub fn set_closable(&self, closable: bool) -> Result<()> {
        let inner = self.tao()?;

        inner.set_closable(closable);

        Ok(())
    }

    /// Updates the window title.

    /// Updates the native window title.
    pub fn set_title<S: Into<String>>(&self, title: S) -> Result<()> {
        let inner = self.tao()?;

        inner.set_title(&title.into());

        Ok(())
    }

    /// Maximizes the window.

    /// Maximizes the window.
    pub fn maximize(&self) -> Result<()> {
        let inner = self.tao()?;

        inner.set_maximized(true);

        Ok(())
    }

    /// Unmaximizes the window.

    /// Restores the window from the maximized state.
    pub fn unmaximize(&self) -> Result<()> {
        let inner = self.tao()?;

        inner.set_maximized(false);

        Ok(())
    }

    /// Minimizes the window.

    /// Minimizes the window.
    pub fn minimize(&self) -> Result<()> {
        let inner = self.tao()?;

        inner.set_minimized(true);

        Ok(())
    }

    /// Unminimizes the window.

    /// Restores the window from the minimized state.
    pub fn unminimize(&self) -> Result<()> {
        let inner = self.tao()?;

        inner.set_minimized(false);

        Ok(())
    }

    /// Shows the window.

    /// Makes the window visible.
    pub fn show(&self) -> Result<()> {
        let inner = self.tao()?;

        inner.set_visible(true);

        Ok(())
    }

    /// Hides the window.

    /// Hides the window without destroying it.
    pub fn hide(&self) -> Result<()> {
        let inner = self.tao()?;

        inner.set_visible(false);

        Ok(())
    }

    /// Sends a close request as a custom event to the event loop.

    ///

    /// `make_event` creates your application's close event. The event loop

    /// must process it, check for a veto if applicable, remove menu bindings and

    /// registrations, and only then call `destroy()`.

    /// `Ok(())` only means: the event was successfully enqueued.

    /// Queues an application-defined close event for this window on the event loop.
    pub fn close<T: 'static>(
        &self,

        proxy: &EventLoopProxy<T>,

        make_event: impl FnOnce(tao::window::WindowId) -> T,
    ) -> Result<()> {
        let id = self.tao()?.id();

        proxy
            .send_event(make_event(id))
            .map_err(|_| anyhow!("Cannot request close for '{}': event loop is closed", self.label))
    }

    /// Releases the local WebView handles, the shared surface and the local

    /// native window handle. Does not send a close event and does not check for a veto.

    ///

    /// Beforehand, detach the native menu binding in the manager and clean up the

    /// window/WebView registrations. Do not destroy the app-wide menu wholesale.

    /// External WebView/Tao clones are not invalidated by this method;

    /// in particular, immediate destruction of the OS window is not guaranteed.

    /// Do not call while the calling thread holds the surface/context lock.

    /// Releases this wrapper's WebViews and platform resources before dropping its native window handle.
    pub fn destroy(&mut self) -> Result<()> {
        // On a lock error, no resource has yet been removed from this wrapper.

        #[cfg(windows)]
        let surface = {
            let mut guard = lock_state(&self.surface, "surface")?;

            guard.take()
        };

        // Release dependent resources while self.inner still exists.

        self.webviews_manager.clear();

        self.set_has_child_webviews(false);

        #[cfg(windows)]
        drop(surface);

        drop(self.inner.take());

        Ok(())
    }

    /// Updates the decorations flag.

    /// Enables or disables native decorations and updates undecorated resize handling on Windows.
    pub fn set_decorations(&self, decorations: bool) -> Result<()> {
        let inner = self.tao()?;

        inner.set_decorations(decorations);

        #[cfg(windows)]
        {
            use taurino_core::undecorated_resizing;

            if decorations {
                undecorated_resizing::detach_resize_handler(inner.hwnd());
            } else if inner.is_resizable() {
                undecorated_resizing::attach_resize_handler(inner.hwnd(), inner.has_undecorated_shadow());
            }
        }

        Ok(())
    }

    /// Toggles the native window shadow on Windows/macOS.

    /// Enables or disables the native window shadow on supported platforms.
    pub fn set_shadow(&self, enable: bool) -> Result<()> {
        let inner = self.tao()?;

        #[cfg(windows)]
        {
            inner.set_undecorated_shadow(enable);

            return Ok(());
        }

        #[cfg(target_os = "macos")]
        {
            inner.set_has_shadow(enable);

            return Ok(());
        }

        #[cfg(not(any(windows, target_os = "macos")))]
        {
            let _ = (inner, enable);

            Err(anyhow!("set_shadow is not supported on this platform"))
        }
    }

    /// Updates the window alwaysOnBottom flag.

    /// Configures whether the window stays below normal windows.
    pub fn set_always_on_bottom(&self, always_on_bottom: bool) -> Result<()> {
        let inner = self.tao()?;

        inner.set_always_on_bottom(always_on_bottom);

        Ok(())
    }

    /// Updates the window alwaysOnTop flag.

    /// Configures whether the window stays above normal windows.
    pub fn set_always_on_top(&self, always_on_top: bool) -> Result<()> {
        let inner = self.tao()?;

        inner.set_always_on_top(always_on_top);

        Ok(())
    }

    /// Updates the window visibleOnAllWorkspaces flag.

    /// Configures whether the window is visible on all workspaces.
    pub fn set_visible_on_all_workspaces(&self, visible_on_all_workspaces: bool) -> Result<()> {
        let inner = self.tao()?;

        inner.set_visible_on_all_workspaces(visible_on_all_workspaces);

        Ok(())
    }

    /// Changes the background color. On Windows a redraw is subsequently

    /// requested; the actual surface drawing occurs in the event handler.

    /// Updates the native background color and requests a redraw on Windows.
    pub fn set_background_color(&self, color: Option<Color>) -> Result<()> {
        let inner = self.tao()?;

        let rgba: Option<tao::window::RGBA> = color.map(Into::into);

        #[cfg(windows)]
        {
            *lock_state(&self.background_color, "background_color")? = rgba;
        }

        // No color lock is held during the native call.

        inner.set_background_color(rgba);

        #[cfg(windows)]
        inner.request_redraw();

        Ok(())
    }

    /// Prevents the window contents from being captured by other apps.

    /// Enables or disables protection against capture of the window contents by other applications.
    pub fn set_content_protected(&self, protected: bool) -> Result<()> {
        self.tao()?.set_content_protection(protected);

        Ok(())
    }

    /// Resizes the window.

    /// Sets the window's inner size.
    pub fn set_size(&self, size: Size) -> Result<()> {
        self.tao()?.set_inner_size(size);

        Ok(())
    }

    /// Updates the window min inner size.

    /// Sets or clears the minimum inner size constraint.
    pub fn set_min_size(&self, size: Option<Size>) -> Result<()> {
        let inner = self.tao()?;

        inner.set_min_inner_size(size);

        Ok(())
    }

    /// Updates the window max inner size.

    /// Sets or clears the maximum inner size constraint.
    pub fn set_max_size(&self, size: Option<Size>) -> Result<()> {
        let inner = self.tao()?;

        inner.set_max_inner_size(size);

        Ok(())
    }

    /// Sets this window's minimum inner width.

    /// Applies the configured minimum and maximum inner size constraints.
    pub fn set_size_constraints(&self, constraints: WindowSizeConstraints) -> Result<()> {
        self.tao()?
            .set_inner_size_constraints(tao::window::WindowSizeConstraints {
                min_width: constraints.min_width,

                min_height: constraints.min_height,

                max_width: constraints.max_width,

                max_height: constraints.max_height,
            });

        Ok(())
    }

    /// Updates the window position.

    /// Moves the window to the supplied outer desktop position.
    pub fn set_position(&self, position: Position) -> Result<()> {
        let inner = self.tao()?;

        inner.set_outer_position(position);

        Ok(())
    }

    /// Updates the window fullscreen state.

    /// Enables or disables borderless fullscreen mode.
    pub fn set_fullscreen(&self, fullscreen: bool) -> Result<()> {
        let inner = self.tao()?;

        if fullscreen {
            inner.set_fullscreen(Some(tao::window::Fullscreen::Borderless(None)));
        } else {
            inner.set_fullscreen(None);
        }

        Ok(())
    }

    /// Sets the window as fullscreen on the monitor that contains the given physical position.

    /// Enables borderless fullscreen on the monitor containing the supplied physical position.
    pub fn set_fullscreen_on_monitor(&self, position: PhysicalPosition<f64>) -> Result<()> {
        let inner = self.tao()?;

        let monitor = inner
            .monitor_from_point(position.x, position.y)
            .ok_or_else(|| anyhow!("No monitor contains position ({}, {})", position.x, position.y))?;

        inner.set_fullscreen(Some(tao::window::Fullscreen::Borderless(Some(monitor))));

        Ok(())
    }

    #[cfg(target_os = "macos")]

    /// Enables or disables macOS simple fullscreen mode.
    pub fn set_simple_fullscreen(&self, enable: bool) -> Result<()> {
        let inner = self.tao()?;

        if inner.simple_fullscreen() == enable {
            return Ok(());
        }

        if !inner.set_simple_fullscreen(enable) {
            return Err(anyhow!("Could not change simple fullscreen for '{}'", self.label));
        }

        Ok(())
    }

    /// Bring the window to front and focus.

    /// Brings the window to the foreground and requests keyboard focus.
    pub fn set_focus(&self) -> Result<()> {
        let inner = self.tao()?;

        inner.set_focus();

        Ok(())
    }

    /// Sets whether the window can be focused.

    /// Configures whether the window is allowed to receive focus.
    pub fn set_focusable(&self, focusable: bool) -> Result<()> {
        let inner = self.tao()?;

        inner.set_focusable(focusable);

        Ok(())
    }

    /// Updates the window icon.

    /// Converts and applies the supplied image as the native window icon.
    pub fn set_icon(&self, icon: Icon<'_>) -> Result<()> {
        let tao_icon = to_tao_icon(icon)?;

        self.tao()?.set_window_icon(Some(tao_icon));

        Ok(())
    }

    /// Whether to hide the window icon from the taskbar or not.

    /// Configures whether the window is omitted from the taskbar on supported platforms.
    pub fn set_skip_taskbar(&self, skip: bool) -> Result<()> {
        let inner = self.tao()?;

        #[cfg(any(
            windows,
            target_os = "linux",
            target_os = "dragonfly",
            target_os = "freebsd",
            target_os = "netbsd",
            target_os = "openbsd"
        ))]
        {
            inner.set_skip_taskbar(skip)?;

            return Ok(());
        }

        #[cfg(not(any(
            windows,
            target_os = "linux",
            target_os = "dragonfly",
            target_os = "freebsd",
            target_os = "netbsd",
            target_os = "openbsd"
        )))]
        {
            let _ = (inner, skip);

            Err(anyhow!("set_skip_taskbar is not supported on this platform"))
        }
    }

    /// Grabs the cursor, preventing it from leaving the window.

    /// Grabs or releases the cursor within the window.
    pub fn set_cursor_grab(&self, grab: bool) -> Result<()> {
        self.tao()?.set_cursor_grab(grab)?;

        Ok(())
    }

    /// Modifies the cursor's visibility.

    /// Shows or hides the cursor while it is over the window.
    pub fn set_cursor_visible(&self, visible: bool) -> Result<()> {
        let inner = self.tao()?;

        inner.set_cursor_visible(visible);

        Ok(())
    }

    /// Modifies the cursor icon of the window.

    /// Changes the native cursor icon used by the window.
    pub fn set_cursor_icon(&self, icon: CursorIcon) -> Result<()> {
        let inner = self.tao()?;

        inner.set_cursor_icon(CursorIconWrapper::from(icon).0);

        Ok(())
    }

    /// Changes the position of the cursor in window coordinates.

    /// Moves the cursor to the supplied position in window coordinates.
    pub fn set_cursor_position<Pos: Into<Position>>(&self, position: Pos) -> Result<()> {
        let inner = self.tao()?;

        Ok(inner.set_cursor_position(position)?)
    }

    /// Ignores the window cursor events.

    /// Configures whether the window ignores cursor input events.
    pub fn set_ignore_cursor_events(&self, ignore: bool) -> Result<()> {
        let inner = self.tao()?;

        Ok(inner.set_ignore_cursor_events(ignore)?)
    }

    /// Starts dragging the window.

    /// Starts a native interactive window drag operation.
    pub fn start_dragging(&self) -> Result<()> {
        let inner = self.tao()?;

        Ok(inner.drag_window()?)
    }

    /// Starts resize-dragging the window.

    /// Starts a native interactive resize operation in the requested direction.
    pub fn start_resize_dragging(&self, direction: ResizeDirection) -> Result<()> {
        let result = self.tao()?.drag_resize_window(match direction {
            ResizeDirection::East => tao::window::ResizeDirection::East,

            ResizeDirection::North => tao::window::ResizeDirection::North,

            ResizeDirection::NorthEast => tao::window::ResizeDirection::NorthEast,

            ResizeDirection::NorthWest => tao::window::ResizeDirection::NorthWest,

            ResizeDirection::South => tao::window::ResizeDirection::South,

            ResizeDirection::SouthEast => tao::window::ResizeDirection::SouthEast,

            ResizeDirection::SouthWest => tao::window::ResizeDirection::SouthWest,

            ResizeDirection::West => tao::window::ResizeDirection::West,
        });

        Ok(result?)
    }

    #[cfg(any(
        target_os = "linux",
        target_os = "dragonfly",
        target_os = "freebsd",
        target_os = "netbsd",
        target_os = "openbsd"
    ))]

    /// Returns the underlying GTK application window wrapped in `GtkWindow`.
    pub fn gtk_window_wrapped(&self) -> Result<GtkWindow> {
        self.gtk_window().map(GtkWindow)
    }

    #[cfg(any(
        target_os = "linux",
        target_os = "dragonfly",
        target_os = "freebsd",
        target_os = "netbsd",
        target_os = "openbsd"
    ))]

    /// Returns the default GTK vertical box wrapped in `GtkBox`.
    pub fn default_vbox_wrapped(&self) -> Result<GtkBox> {
        self.default_vbox().map(GtkBox)
    }

    /// Sets a badge counter on the platforms supported here.

    /// On Windows, no silent success is pretended.

    /// Sets or clears the application badge count using the platform-specific implementation.
    pub fn set_badge_count(&self, count: Option<i64>, desktop_filename: Option<String>) -> Result<()> {
        let inner = self.tao()?;

        #[cfg(target_os = "ios")]
        {
            let _ = desktop_filename;

            inner.set_badge_count(count.map_or(0, |x| x.clamp(i32::MIN as i64, i32::MAX as i64) as i32));

            return Ok(());
        }

        #[cfg(target_os = "macos")]
        {
            let _ = desktop_filename;

            inner.set_badge_label(count.map(|x| x.to_string()));

            return Ok(());
        }

        #[cfg(any(
            target_os = "linux",
            target_os = "dragonfly",
            target_os = "freebsd",
            target_os = "netbsd",
            target_os = "openbsd"
        ))]
        {
            inner.set_badge_count(count, desktop_filename);

            return Ok(());
        }

        #[cfg(not(any(
            target_os = "ios",
            target_os = "macos",
            target_os = "linux",
            target_os = "dragonfly",
            target_os = "freebsd",
            target_os = "netbsd",
            target_os = "openbsd"
        )))]
        {
            let _ = (inner, count, desktop_filename);

            Err(anyhow!("set_badge_count is not supported on this platform"))
        }
    }

    /// Sets the badge count on the taskbar **macOS only**.

    #[cfg(target_os = "macos")]

    /// Sets or clears the macOS Dock badge label.
    pub fn set_badge_label(&self, label: Option<String>) -> Result<()> {
        self.tao()?.set_badge_label(label);

        Ok(())
    }

    /// On Windows, sets an overlay icon; None removes it.

    /// Sets or clears the Windows taskbar overlay icon.
    pub fn set_overlay_icon(&self, icon: Option<Icon<'_>>) -> Result<()> {
        let inner = self.tao()?;

        #[cfg(windows)]
        {
            let icon = icon.map(to_tao_icon).transpose()?;

            inner.set_overlay_icon(icon.as_ref());

            return Ok(());
        }

        #[cfg(not(windows))]
        {
            let _ = (inner, icon);

            Err(anyhow!("set_overlay_icon is only supported on Windows"))
        }
    }

    /// Sets the taskbar progress state.

    /// Updates the platform taskbar or dock progress indicator.
    pub fn set_progress_bar(&self, progress_state: ProgressBarState) -> Result<()> {
        self.tao()?
            .set_progress_bar(ProgressBarStateWrapper::from(progress_state).0);

        Ok(())
    }

    /// Sets the title bar style. Available on macOS only.

    ///

    // Mapping of the three variants intentionally taken from the template.

    // The enum definition and the intended layout semantics are not available.

    #[cfg(target_os = "macos")]

    /// Applies the selected macOS title-bar transparency and content layout behavior.
    pub fn set_title_bar_style(&self, style: TitleBarStyle) -> Result<()> {
        let inner = self.tao()?;

        match style {
            TitleBarStyle::Visible => {
                inner.set_titlebar_transparent(false);

                inner.set_fullsize_content_view(true);
            }

            TitleBarStyle::Transparent => {
                inner.set_titlebar_transparent(true);

                inner.set_fullsize_content_view(false);
            }

            TitleBarStyle::Overlay => {
                inner.set_titlebar_transparent(true);

                inner.set_fullsize_content_view(true);
            }

            #[allow(unreachable_patterns)]
            _ => return Err(anyhow!("Unsupported title bar style")),
        }

        Ok(())
    }

    /// Moves the window buttons under macOS.

    /// Moves the macOS traffic-light window controls to the supplied inset position.
    pub fn set_traffic_light_position(&self, position: Position) -> Result<()> {
        let inner = self.tao()?;

        #[cfg(target_os = "macos")]
        {
            inner.set_traffic_light_inset(position);

            return Ok(());
        }

        #[cfg(not(target_os = "macos"))]
        {
            let _ = (inner, position);

            Err(anyhow!("set_traffic_light_position is only supported on macOS"))
        }
    }

    /// Sets the preferred native window theme or restores system theme selection.
    pub fn set_theme(&self, theme: Option<Theme>) -> Result<()> {
        let inner = self.tao()?;

        inner.set_theme(theme.map(map_theme));

        Ok(())
    }
}

/// Maps a Taurino theme value to Tao's native window theme.
pub fn map_theme(theme: Theme) -> taurino_core::tao::window::Theme {
    match theme {
        Theme::Light => taurino_core::tao::window::Theme::Light,

        Theme::Dark => taurino_core::tao::window::Theme::Dark,

        _ => taurino_core::tao::window::Theme::Light,
    }
}

#[cfg(any(
    target_os = "linux",
    target_os = "dragonfly",
    target_os = "freebsd",
    target_os = "netbsd",
    target_os = "openbsd"
))]

pub struct GtkWindow(pub gtk::ApplicationWindow);

#[cfg(any(
    target_os = "linux",
    target_os = "dragonfly",
    target_os = "freebsd",
    target_os = "netbsd",
    target_os = "openbsd"
))]

pub struct GtkBox(pub gtk::Box);

// -----------------------------------------------------------------------------

// window/menu_ops.rs  (or append directly in builder.rs)

// -----------------------------------------------------------------------------

impl Window {
    // -------------------------------------------------------------------------

    // internal helper

    // -------------------------------------------------------------------------

    /// Clones the menu bound to this window, if present.

    /// Otherwise error – this keeps the API symmetric to `tao()`.

    #[inline]

    /// Returns the attached menu or an error when this window has no menu.
    pub fn menu_or_err(&self) -> Result<Menu> {
        self.try_menu()?
            .ok_or_else(|| anyhow!("Window '{}' has no menu attached", self.label))
    }

    /// Only `true` if a menu is present **and** it is not app-wide.

    /// Such menus may be manipulated by the window itself without disturbing the

    /// global MenuManager (macOS).

    /// Returns whether this window has a menu that is not application-wide.
    pub fn has_local_menu(&self) -> bool {
        self.menu
            .lock()
            .map(|m| m.as_ref().is_some_and(|wm| !wm.is_app_wide))
            .unwrap_or(false)
    }

    // -------------------------------------------------------------------------

    // Search / traversal (delegates to Menu)

    // -------------------------------------------------------------------------

    /// Direct children of the menu.

    /// Returns the direct top-level items of this window's menu.
    pub fn menu_items(&self) -> Result<Vec<MenuItemKind>> {
        self.menu_or_err()?.items()
    }

    /// Direct match only among the top-level entries.

    /// Returns the top-level menu item with the supplied identifier, if present.
    pub fn get_menu_item_by_id(&self, id: &MenuId) -> Result<Option<MenuItemKind>> {
        self.menu_or_err()?.get(id)
    }

    /// Recursive search in the entire menu tree.

    /// Recursively searches the complete menu tree for the supplied identifier.
    pub fn find_menu_item(&self, id: &MenuId) -> Result<Option<MenuItemKind>> {
        self.menu_or_err()?.find(id)
    }

    /// All entries in the tree, depth-first.

    /// Returns all menu items in depth-first traversal order.
    pub fn all_menu_items(&self) -> Result<Vec<MenuItemKind>> {
        self.menu_or_err()?.all_items()
    }

    /// Visits every entry in the tree.

    /// Visits every item in this window's menu tree with the supplied callback.
    pub fn visit_menu_items<F>(&self, visitor: F) -> Result<()>
    where
        F: FnMut(&MenuItemKind) -> Result<()>,
    {
        self.menu_or_err()?.visit(visitor)
    }

    // -------------------------------------------------------------------------

    // Typed getters (delegates to Menu)

    // -------------------------------------------------------------------------

    /// Returns the regular menu item with the supplied identifier, if present.
    pub fn get_menu_item(&self, id: &MenuId) -> Result<Option<MenuItem>> {
        self.menu_or_err()?.get_menu_item(id)
    }

    /// Returns the submenu with the supplied identifier, if present.
    pub fn get_submenu(&self, id: &MenuId) -> Result<Option<Submenu>> {
        self.menu_or_err()?.get_submenu(id)
    }

    /// Returns the check-menu item with the supplied identifier, if present.
    pub fn get_check_menu_item(&self, id: &MenuId) -> Result<Option<CheckMenuItem>> {
        self.menu_or_err()?.get_check(id)
    }

    /// Returns the icon-menu item with the supplied identifier, if present.
    pub fn get_icon_menu_item(&self, id: &MenuId) -> Result<Option<IconMenuItem>> {
        self.menu_or_err()?.get_icon(id)
    }

    /// Returns the predefined menu item with the supplied identifier, if present.
    pub fn get_predefined_menu_item(&self, id: &MenuId) -> Result<Option<PredefinedMenuItem>> {
        self.menu_or_err()?.get_predefined(id)
    }

    // -------------------------------------------------------------------------

    // Mutation (delegates to Menu)

    // -------------------------------------------------------------------------

    /// Appends one item to the end of this window's menu.
    pub fn append_menu_item(&self, item: &dyn IsMenuItem) -> Result<()> {
        self.menu_or_err()?.append(item)
    }

    /// Appends multiple items to the end of this window's menu.
    pub fn append_menu_items(&self, items: &[&dyn IsMenuItem]) -> Result<()> {
        self.menu_or_err()?.append_items(items)
    }

    /// Inserts one item at the beginning of this window's menu.
    pub fn prepend_menu_item(&self, item: &dyn IsMenuItem) -> Result<()> {
        self.menu_or_err()?.prepend(item)
    }

    /// Inserts a menu item at the supplied position.
    pub fn insert_menu_item(&self, item: &dyn IsMenuItem, position: usize) -> Result<()> {
        self.menu_or_err()?.insert(item, position)
    }

    /// Removes the supplied item from this window's menu.
    pub fn remove_menu_item(&self, item: &dyn IsMenuItem) -> Result<()> {
        self.menu_or_err()?.remove(item)
    }

    // -------------------------------------------------------------------------

    // Submenu helpers: directly access a submenu of the window

    // -------------------------------------------------------------------------

    /// Appends an item to a submenu of the window.

    /// Appends an item to the submenu identified by `submenu_id`.
    pub fn append_to_submenu(&self, submenu_id: &MenuId, item: &dyn IsMenuItem) -> Result<()> {
        let submenu = self
            .get_submenu(submenu_id)?
            .ok_or_else(|| anyhow!("Submenu {:?} not found", submenu_id))?;

        submenu.append(item)
    }

    /// Set iconized submenu (macOS/Windows).

    /// Sets or clears the native icon of the specified submenu.
    pub fn set_submenu_native_icon(&self, submenu_id: &MenuId, icon: Option<NativeIcon>) -> Result<()> {
        let submenu = self
            .get_submenu(submenu_id)?
            .ok_or_else(|| anyhow!("Submenu {:?} not found", submenu_id))?;

        submenu.set_native_icon(icon)
    }

    #[cfg(windows)]

    /// Redraws only the matching transparent Windows window.

    /// Late events after taking the native handle are ignored.

    /// Redraws the matching transparent Windows surface when a valid redraw is required.
    pub fn redraw_requested(&self, window_id: tao::window::WindowId) -> Result<()> {
        let Some(inner) = self.inner.as_ref() else {
            return Ok(());
        };

        if inner.id() != window_id || !self.is_window_transparent {
            return Ok(());
        }

        let size = inner.inner_size();

        if size.width == 0 || size.height == 0 {
            return Ok(());
        }

        // Take a snapshot and release the color lock before the surface lock.

        let color = *lock_state(&self.background_color, "background_color")?;

        let mut guard = lock_state(&self.surface, "surface")?;

        if let Some(surface) = guard.as_mut() {
            // Existing project helper; its implementation is not available.

            // It must handle resizing/presentation and must not re-lock the same

            // surface mutex.

            inner.draw_surface(surface, color);
        }

        Ok(())
    }
}

/// Converts Taurino RGBA icon data into a Tao native window icon.
fn to_tao_icon(icon: taurino_core::image::Icon<'_>) -> Result<taurino_core::tao::window::Icon> {
    taurino_core::tao::window::Icon::from_rgba(icon.rgba.into_owned(), icon.width, icon.height)
        .map_err(|error| anyhow!("failed to create Tao icon from RGBA data: {error}"))
}

/// Maps Tao's native window theme to Taurino's theme type.
pub fn map_theme_from_tao(theme: taurino_core::tao::window::Theme) -> Theme {
    match theme {
        taurino_core::tao::window::Theme::Light => Theme::Light,

        taurino_core::tao::window::Theme::Dark => Theme::Dark,

        #[allow(unreachable_patterns)]
        _ => Theme::Light,
    }
}

// -----------------------------------------------------------------------------

// Additional accessors without changing the native window mapping

// -----------------------------------------------------------------------------

impl Window {
    /// Tao ID, not the u32 ID from the registry or from WebView::window_id().

    /// Returns the native Tao identifier of this window.
    pub fn tao_window_id(&self) -> Result<tao::window::WindowId> {
        Ok(self.tao()?.id())
    }

    /// Returns whether this wrapper still owns a native Tao window handle.
    pub fn is_tao_window_available(&self) -> bool {
        self.inner.is_some()
    }

    /// Returns all WebViews currently managed by this window.
    pub fn webviews(&self) -> &[WebView] {
        &self.webviews_manager.webviews()
    }

    /// Returns the WebView with the supplied Taurino identifier, if present.
    pub fn webview(&self, id: WebViewId) -> Option<&WebView> {
        self.webviews_manager.get_by_id(id)
    }

    /// Returns the WebView with the supplied label, if present.
    pub fn webview_by_label(&self, label: &str) -> Option<&WebView> {
        self.webviews_manager.get_by_label(label)
    }

    /// Requests a redraw; does not draw synchronously at this call site.

    /// Queues a redraw request for the native window.
    pub fn request_redraw(&self) -> Result<()> {
        self.tao()?.request_redraw();

        Ok(())
    }

    /// In contrast to menu(), a poisoned mutex is reported as an error.

    /// Returns a cloned menu handle and reports a poisoned menu lock as an error.
    pub fn try_menu(&self) -> Result<Option<Menu>> {
        let guard = lock_state(&self.menu, "menu")?;

        Ok(guard.as_ref().map(|menu| menu.menu.clone()))
    }

    /// Takes only the stored reference; does not detach the native menu binding.

    /// Removes and returns the stored menu reference while reporting lock poisoning as an error.
    pub fn try_take_window_menu(&self) -> Result<Option<WindowMenu>> {
        Ok(lock_state(&self.menu, "menu")?.take())
    }

    #[cfg(windows)]

    /// Returns the stored Windows background color while reporting lock poisoning as an error.
    pub fn try_background_color(&self) -> Result<Option<tao::window::RGBA>> {
        Ok(*lock_state(&self.background_color, "background_color")?)
    }
}

/// Converts PoisonError into a custom error without taking over its guard.

/// Locks window state and converts mutex poisoning into an engine error.
pub fn lock_state<'a, T>(mutex: &'a Mutex<T>, name: &str) -> Result<MutexGuard<'a, T>> {
    mutex.lock().map_err(|_| anyhow!("Window {name} mutex is poisoned"))
}
