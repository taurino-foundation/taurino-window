// Fenster-Wrapper auf Basis des hochgeladenen Moduls.
// Native GUI-Aufrufe auf dem zuständigen GUI-Thread ausführen.
// Die projektspezifischen Typen/Erweiterungen werden weiterhin vorausgesetzt.

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
use taurino_core::softbuffer;

use taurino_core::raw_window_handle::{DisplayHandle, HandleError, HasDisplayHandle, HasWindowHandle, WindowHandle};
use taurino_menu::{
    WindowMenu,
    prelude::{
        CheckMenuItem, IconMenuItem, IsMenuItem, Menu, MenuItem, MenuItemKind, NativeIcon, PredefinedMenuItem, Submenu,
    },
};

#[cfg(windows)]
use crate::config::FocusState;
// Importpfad korrigiert: `TitleBarStyle` liegt in `config`, nicht in `types`.
#[cfg(target_os = "macos")]
use crate::config::TitleBarStyle;
use crate::{
    config::{Color, CursorIcon, Monitor, ProgressBarState, ResizeDirection, UserAttentionType, WindowSizeConstraints},
    webview::{WebView, WebViewManager, inner_size},
    wrappers::{CursorIconWrapper, MonitorHandleWrapper, ProgressBarStateWrapper, UserAttentionTypeWrapper},
};
/// GUI-threadgebundener Wrapper. Die WebViews machen ihn nicht Send/Sync.
///
/// WebViews und Surface stehen vor `inner`, damit ihre lokalen Besitzer beim
/// regulären Drop vor dem lokalen Tao-Handle freigegeben werden.
/// Externe Klone können die jeweiligen Ressourcen weiterin am Leben halten.
pub struct Window {
    /// Native Menüanbindung und deren Lebensdauer verwaltet der MenuManager.
    /// Ein app-weites Menü darf beim Schließen dieses Fensters erhalten bleiben.
    pub(crate) menu: Arc<Mutex<Option<WindowMenu>>>,
    pub label: String,
    pub id: WindowId,
    pub webviews_manager: WebViewManager,
    #[cfg(windows)]
    pub background_color: Arc<Mutex<Option<tao::window::RGBA>>>,
    #[cfg(windows)]
    pub is_window_transparent: bool,
    #[cfg(windows)]
    pub surface: Arc<Mutex<Option<softbuffer::Surface<Arc<Tao>, Arc<Tao>>>>>,
    #[cfg(windows)]
    pub focused_webview: Arc<Mutex<FocusState>>,
    pub has_children: AtomicBool,
    pub(crate) inner: Option<Arc<Tao>>,
}

impl Window {
    pub fn new(
        id: WindowId,
        inner: Option<Arc<Tao>>,
        menu: Arc<Mutex<Option<WindowMenu>>>,
        webviews_manager: WebViewManager,
        label: String,
        #[cfg(windows)] background_color: Arc<Mutex<Option<tao::window::RGBA>>>,
        #[cfg(windows)] is_window_transparent: bool,
        #[cfg(windows)] surface: Arc<Mutex<Option<softbuffer::Surface<Arc<Tao>, Arc<Tao>>>>>,
        #[cfg(windows)] focused_webview: Arc<Mutex<FocusState>>,
        has_children: AtomicBool,
    ) -> Self {
        Self {
            id,
            inner,
            menu,
            webviews_manager,
            label,
            #[cfg(windows)]
            background_color,
            #[cfg(windows)]
            is_window_transparent,
            #[cfg(windows)]
            surface,
            #[cfg(windows)]
            focused_webview,
            has_children,
        }
    }
}

impl Window {
    /// Returns the window label.
    pub fn label(&self) -> &str {
        &self.label
    }

    /// Gibt die interne u32-Fenster-ID zurück (nicht die Tao-WindowId).
    pub fn id(&self) -> WindowId {
        self.id
    }
    /// Liefert das native Fenster oder einen Fehler nach dessen Entnahme.
    pub fn tao(&self) -> Result<&Arc<Tao>> {
        self.inner
            .as_ref()
            .ok_or_else(|| anyhow!("Window '{}' is not available", self.label))
    }

    /// Returns the underlying Tao window.
    ///
    /// Returns `None` if the native window has already been taken or destroyed.
    pub fn inner(&self) -> Option<&Arc<Tao>> {
        self.inner.as_ref()
    }

    /// Returns a cloned reference to the underlying Tao window.
    pub fn inner_arc(&self) -> Option<Arc<Tao>> {
        self.inner.clone()
    }

    /// Returns whether this window currently has a menu.
    pub fn has_menu(&self) -> bool {
        self.menu.lock().map(|menu| menu.is_some()).unwrap_or(false)
    }

    /// Returns whether the menu attached to this window is application-wide.
    ///
    /// This is mainly relevant on macOS.
    pub fn has_app_wide_menu(&self) -> bool {
        self.menu
            .lock()
            .map(|menu| menu.as_ref().is_some_and(|menu| menu.is_app_wide))
            .unwrap_or(false)
    }

    /// Returns a clone of the menu attached to this window.
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
    pub fn take_window_menu(&self) -> Option<WindowMenu> {
        self.menu.lock().ok().and_then(|mut menu| menu.take())
    }

    /// Entnimmt nur das native Handle; WebViews, Surface und Menü bleiben erhalten.
    /// Für den vollständigen Schließablauf den Manager und destroy() verwenden.
    pub fn take_inner_window(&mut self) -> Option<Arc<Tao>> {
        self.inner.take()
    }

    #[cfg(windows)]
    pub fn background_color(&self) -> Option<tao::window::RGBA> {
        *self.background_color.lock().unwrap()
    }

    #[cfg(windows)]
    pub fn is_transparent(&self) -> bool {
        self.is_window_transparent
    }

    #[cfg(windows)]
    pub fn focused_webview(&self) -> &Arc<Mutex<FocusState>> {
        &self.focused_webview
    }
}

impl HasDisplayHandle for Window {
    fn display_handle(&self) -> std::result::Result<DisplayHandle<'_>, HandleError> {
        self.inner.as_ref().ok_or(HandleError::Unavailable)?.display_handle()
    }
}

impl HasWindowHandle for Window {
    fn window_handle(&self) -> std::result::Result<WindowHandle<'_>, HandleError> {
        self.inner.as_ref().ok_or(HandleError::Unavailable)?.window_handle()
    }
}

impl Window {
    // -------------------------------------------------------------------------
    // Getters
    // -------------------------------------------------------------------------
    /// Returns the scale factor that can be used to map logical pixels to physical pixels, and vice versa.
    pub fn scale_factor(&self) -> Result<f64> {
        let inner = self.tao()?;
        Ok(inner.scale_factor())
    }

    /// Returns the position of the top-left hand corner of the window's client area relative to the top-left hand corner of the desktop.
    pub fn inner_position(&self) -> Result<PhysicalPosition<i32>> {
        let inner = self.tao()?;
        inner.inner_position().map_err(Into::into)
    }

    /// Returns the position of the top-left hand corner of the window relative to the top-left hand corner of the desktop.
    pub fn outer_position(&self) -> Result<PhysicalPosition<i32>> {
        let inner = self.tao()?;
        inner.outer_position().map_err(Into::into)
    }

    /// Returns the physical size of the window's client area.
    pub fn inner_size(&self) -> Result<PhysicalSize<u32>> {
        let has_children = self.has_children();
        let inner = self.tao()?;
        Ok(inner_size(inner, &self.webviews_manager.webviews(), has_children))
    }

    /// Returns the physical size of the entire window.
    pub fn outer_size(&self) -> Result<PhysicalSize<u32>> {
        let inner = self.tao()?;
        Ok(inner.outer_size())
    }

    /// Gets the window's current fullscreen state.
    pub fn is_fullscreen(&self) -> Result<bool> {
        let inner = self.tao()?;
        Ok(inner.fullscreen().is_some())
    }

    /// Gets the window's current minimized state.
    pub fn is_minimized(&self) -> Result<bool> {
        let inner = self.tao()?;
        Ok(inner.is_minimized())
    }

    /// Gets the window's current maximized state.
    pub fn is_maximized(&self) -> Result<bool> {
        let inner = self.tao()?;
        Ok(inner.is_maximized())
    }

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
    pub fn has_children(&self) -> bool {
        self.has_children.load(Ordering::Acquire)
    }

    /// Sets whether this window has child webviews.
    pub fn set_has_child_webviews(&self, has_children: bool) {
        self.has_children.store(has_children, Ordering::Release);
    }

    /// Gets the window's current decoration state.
    pub fn is_decorated(&self) -> Result<bool> {
        let inner = self.tao()?;
        Ok(inner.is_decorated())
    }

    /// Gets the window's current resizable state.
    pub fn is_resizable(&self) -> Result<bool> {
        let inner = self.tao()?;
        Ok(inner.is_resizable())
    }

    /// Gets the window's native maximize button state.
    pub fn is_maximizable(&self) -> Result<bool> {
        let inner = self.tao()?;
        Ok(inner.is_maximizable())
    }

    /// Gets the window's native minimize button state.
    pub fn is_minimizable(&self) -> Result<bool> {
        let inner = self.tao()?;
        Ok(inner.is_minimizable())
    }

    /// Gets the window's native close button state.
    pub fn is_closable(&self) -> Result<bool> {
        let inner = self.tao()?;
        Ok(inner.is_closable())
    }

    /// Gets the window's current visibility state.
    pub fn is_visible(&self) -> Result<bool> {
        let inner = self.tao()?;
        Ok(inner.is_visible())
    }

    /// Whether the window is enabled or disabled.
    pub fn is_enabled(&self) -> Result<bool> {
        let inner = self.tao()?;
        Ok(inner.is_enabled())
    }

    /// Gets the window alwaysOnTop flag state.
    pub fn is_always_on_top(&self) -> Result<bool> {
        let inner = self.tao()?;
        Ok(inner.is_always_on_top())
    }

    /// Gets the window's current title.
    pub fn title(&self) -> Result<String> {
        let inner = self.tao()?;
        Ok(inner.title())
    }

    /// Returns the monitor on which the window currently resides.
    pub fn current_monitor(&self) -> Result<Option<Monitor>> {
        let inner = self.tao()?;
        Ok(inner.current_monitor().map(|m| MonitorHandleWrapper(m).into()))
    }

    /// Returns the primary monitor of the system.
    pub fn primary_monitor(&self) -> Result<Option<Monitor>> {
        let inner = self.tao()?;
        Ok(inner.primary_monitor().map(|m| MonitorHandleWrapper(m).into()))
    }

    /// Returns the monitor that contains the given point.
    pub fn monitor_from_point(&self, x: f64, y: f64) -> Result<Option<Monitor>> {
        let inner = self.tao()?;
        Ok(inner.monitor_from_point(x, y).map(|m| MonitorHandleWrapper(m).into()))
    }

    /// Returns the list of all the monitors available on the system.
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
    pub fn default_vbox(&self) -> Result<gtk::Box> {
        self.tao()?
            .default_vbox()
            .cloned()
            .ok_or_else(|| anyhow!("Window '{}' has no default GTK vbox", self.label))
    }

    /// Returns the name of the Android activity associated with this window.
    #[cfg(target_os = "android")]
    pub fn activity_name(&self) -> Result<String> {
        let inner = self.tao()?;
        Ok(inner.activity_name().to_string())
    }

    /// Returns the identifier of the UIScene tied to this UIWindow.
    #[cfg(target_os = "ios")]
    pub fn scene_identifier(&self) -> Result<String> {
        let inner = self.tao()?;
        Ok(inner.scene_identifier().to_string())
    }

    /// Returns the current window theme.
    pub fn theme(&self) -> Result<Theme> {
        Ok(map_theme_from_tao(self.tao()?.theme()))
    }

    // -------------------------------------------------------------------------
    // Setters
    // -------------------------------------------------------------------------
    /// Centers the window.
    pub fn center(&self) -> Result<()> {
        let inner = self.tao()?;
        inner.center();
        Ok(())
    }

    /// Requests user attention to the window.
    pub fn request_user_attention(&self, request_type: Option<UserAttentionType>) -> Result<()> {
        let inner = self.tao()?;
        inner.request_user_attention(request_type.map(|r| UserAttentionTypeWrapper::from(r).0));
        Ok(())
    }

    /// Updates the window resizable flag.
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
    pub fn set_enabled(&self, enabled: bool) -> Result<()> {
        let inner = self.tao()?;
        inner.set_enabled(enabled);
        Ok(())
    }

    /// Updates the window's native maximize button state.
    pub fn set_maximizable(&self, maximizable: bool) -> Result<()> {
        let inner = self.tao()?;
        inner.set_maximizable(maximizable);
        Ok(())
    }

    /// Updates the window's native minimize button state.
    pub fn set_minimizable(&self, minimizable: bool) -> Result<()> {
        let inner = self.tao()?;
        inner.set_minimizable(minimizable);
        Ok(())
    }

    /// Updates the window's native close button state.
    pub fn set_closable(&self, closable: bool) -> Result<()> {
        let inner = self.tao()?;
        inner.set_closable(closable);
        Ok(())
    }

    /// Updates the window title.
    pub fn set_title<S: Into<String>>(&self, title: S) -> Result<()> {
        let inner = self.tao()?;
        inner.set_title(&title.into());
        Ok(())
    }

    /// Maximizes the window.
    pub fn maximize(&self) -> Result<()> {
        let inner = self.tao()?;
        inner.set_maximized(true);
        Ok(())
    }

    /// Unmaximizes the window.
    pub fn unmaximize(&self) -> Result<()> {
        let inner = self.tao()?;
        inner.set_maximized(false);
        Ok(())
    }

    /// Minimizes the window.
    pub fn minimize(&self) -> Result<()> {
        let inner = self.tao()?;
        inner.set_minimized(true);
        Ok(())
    }

    /// Unminimizes the window.
    pub fn unminimize(&self) -> Result<()> {
        let inner = self.tao()?;
        inner.set_minimized(false);
        Ok(())
    }

    /// Shows the window.
    pub fn show(&self) -> Result<()> {
        let inner = self.tao()?;
        inner.set_visible(true);
        Ok(())
    }

    /// Hides the window.
    pub fn hide(&self) -> Result<()> {
        let inner = self.tao()?;
        inner.set_visible(false);
        Ok(())
    }

    /// Sendet einen Schließwunsch als eigenes Ereignis an die Event-Loop.
    ///
    /// `make_event` erzeugt das Schließereignis deiner Anwendung. Die Event-Loop
    /// muss es verarbeiten, gegebenenfalls ein Veto prüfen, Menüanbindungen und
    /// Registrierungen entfernen und erst anschließend `destroy()` aufrufen.
    /// `Ok(())` bedeutet nur: Das Ereignis wurde erfolgreich eingereiht.
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

    /// Gibt die lokalen WebView-Handles, die geteilte Surface und das lokale
    /// native Fenster-Handle frei. Sendet kein Schließereignis und prüft kein Veto.
    ///
    /// Vorher im Manager die native Menüanbindung lösen und die Fenster-/WebView-
    /// Registrierungen bereinigen. Das app-weite Menü nicht pauschal zerstören.
    /// Externe WebView-/Tao-Klone werden durch diese Methode nicht invalidiert;
    /// insbesondere ist die sofortige Zerstörung des OS-Fensters nicht garantiert.
    /// Nicht aufrufen, während der aufrufende Thread den Surface-/Context-Lock hält.
    pub fn destroy(&mut self) -> Result<()> {
        // Bei einem Lock-Fehler ist noch keine Ressource aus diesem Wrapper entfernt.
        #[cfg(windows)]
        let surface = {
            let mut guard = lock_state(&self.surface, "surface")?;
            guard.take()
        };

        // Abhängige Ressourcen freigeben, solange self.inner noch existiert.
        self.webviews_manager.clear();
        self.set_has_child_webviews(false);
        #[cfg(windows)]
        drop(surface);
        drop(self.inner.take());
        Ok(())
    }

    /// Updates the decorations flag.
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

    /// Schaltet den nativen Fensterschatten auf Windows/macOS um.
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
    pub fn set_always_on_bottom(&self, always_on_bottom: bool) -> Result<()> {
        let inner = self.tao()?;
        inner.set_always_on_bottom(always_on_bottom);
        Ok(())
    }

    /// Updates the window alwaysOnTop flag.
    pub fn set_always_on_top(&self, always_on_top: bool) -> Result<()> {
        let inner = self.tao()?;
        inner.set_always_on_top(always_on_top);
        Ok(())
    }

    /// Updates the window visibleOnAllWorkspaces flag.
    pub fn set_visible_on_all_workspaces(&self, visible_on_all_workspaces: bool) -> Result<()> {
        let inner = self.tao()?;
        inner.set_visible_on_all_workspaces(visible_on_all_workspaces);
        Ok(())
    }

    /// Ändert die Hintergrundfarbe. Unter Windows wird anschließend ein Redraw
    /// angefordert; die tatsächliche Surface-Zeichnung erfolgt im Event-Handler.
    pub fn set_background_color(&self, color: Option<Color>) -> Result<()> {
        let inner = self.tao()?;
        let rgba: Option<tao::window::RGBA> = color.map(Into::into);
        #[cfg(windows)]
        {
            *lock_state(&self.background_color, "background_color")? = rgba;
        }
        // Kein Farblock bleibt während des nativen Aufrufs gehalten.
        inner.set_background_color(rgba);
        #[cfg(windows)]
        inner.request_redraw();
        Ok(())
    }

    /// Prevents the window contents from being captured by other apps.
    pub fn set_content_protected(&self, protected: bool) -> Result<()> {
        self.tao()?.set_content_protection(protected);
        Ok(())
    }

    /// Resizes the window.
    pub fn set_size(&self, size: Size) -> Result<()> {
        self.tao()?.set_inner_size(size);
        Ok(())
    }

    /// Updates the window min inner size.
    pub fn set_min_size(&self, size: Option<Size>) -> Result<()> {
        let inner = self.tao()?;
        inner.set_min_inner_size(size);
        Ok(())
    }

    /// Updates the window max inner size.
    pub fn set_max_size(&self, size: Option<Size>) -> Result<()> {
        let inner = self.tao()?;
        inner.set_max_inner_size(size);
        Ok(())
    }

    /// Sets this window's minimum inner width.
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
    pub fn set_position(&self, position: Position) -> Result<()> {
        let inner = self.tao()?;
        inner.set_outer_position(position);
        Ok(())
    }

    /// Updates the window fullscreen state.
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
    pub fn set_fullscreen_on_monitor(&self, position: PhysicalPosition<f64>) -> Result<()> {
        let inner = self.tao()?;
        let monitor = inner
            .monitor_from_point(position.x, position.y)
            .ok_or_else(|| anyhow!("No monitor contains position ({}, {})", position.x, position.y))?;
        inner.set_fullscreen(Some(tao::window::Fullscreen::Borderless(Some(monitor))));
        Ok(())
    }

    #[cfg(target_os = "macos")]
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
    pub fn set_focus(&self) -> Result<()> {
        let inner = self.tao()?;
        inner.set_focus();
        Ok(())
    }

    /// Sets whether the window can be focused.
    pub fn set_focusable(&self, focusable: bool) -> Result<()> {
        let inner = self.tao()?;
        inner.set_focusable(focusable);
        Ok(())
    }

    /// Updates the window icon.
    pub fn set_icon(&self, icon: Icon<'_>) -> Result<()> {
        let tao_icon = to_tao_icon(icon)?;
        self.tao()?.set_window_icon(Some(tao_icon));
        Ok(())
    }

    /// Whether to hide the window icon from the taskbar or not.
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
    pub fn set_cursor_grab(&self, grab: bool) -> Result<()> {
        self.tao()?.set_cursor_grab(grab)?;
        Ok(())
    }

    /// Modifies the cursor's visibility.
    pub fn set_cursor_visible(&self, visible: bool) -> Result<()> {
        let inner = self.tao()?;
        inner.set_cursor_visible(visible);
        Ok(())
    }

    /// Modifies the cursor icon of the window.
    pub fn set_cursor_icon(&self, icon: CursorIcon) -> Result<()> {
        let inner = self.tao()?;
        inner.set_cursor_icon(CursorIconWrapper::from(icon).0);
        Ok(())
    }

    /// Changes the position of the cursor in window coordinates.
    pub fn set_cursor_position<Pos: Into<Position>>(&self, position: Pos) -> Result<()> {
        let inner = self.tao()?;
        Ok(inner.set_cursor_position(position)?)
    }

    /// Ignores the window cursor events.
    pub fn set_ignore_cursor_events(&self, ignore: bool) -> Result<()> {
        let inner = self.tao()?;
        Ok(inner.set_ignore_cursor_events(ignore)?)
    }

    /// Starts dragging the window.
    pub fn start_dragging(&self) -> Result<()> {
        let inner = self.tao()?;
        Ok(inner.drag_window()?)
    }

    /// Starts resize-dragging the window.
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
    pub fn default_vbox_wrapped(&self) -> Result<GtkBox> {
        self.default_vbox().map(GtkBox)
    }

    /// Setzt einen Badge-Zähler auf den hier unterstützten Plattformen.
    /// Unter Windows wird dafür kein stiller Erfolg vorgetäuscht.
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
    pub fn set_badge_label(&self, label: Option<String>) -> Result<()> {
        self.tao()?.set_badge_label(label);
        Ok(())
    }

    /// Setzt unter Windows ein Overlay-Icon; None entfernt es.
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
    pub fn set_progress_bar(&self, progress_state: ProgressBarState) -> Result<()> {
        self.tao()?
            .set_progress_bar(ProgressBarStateWrapper::from(progress_state).0);
        Ok(())
    }

    /// Sets the title bar style. Available on macOS only.
    ///
    // Mapping der drei Varianten bewusst aus der Vorlage übernommen.
    // Die Enum-Definition und die beabsichtigte Layout-Semantik liegen nicht vor.
    #[cfg(target_os = "macos")]
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

    /// Verschiebt die Fensterknöpfe unter macOS.
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

    pub fn set_theme(&self, theme: Option<Theme>) -> Result<()> {
        let inner = self.tao()?;
        inner.set_theme(theme.map(map_theme));
        Ok(())
    }
}

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
// window/menu_ops.rs  (oder direkt in builder.rs anhängen)
// -----------------------------------------------------------------------------
impl Window {
    // -------------------------------------------------------------------------
    // interner Helfer
    // -------------------------------------------------------------------------
    /// Klont das an dieses Fenster gebundene Menu, falls vorhanden.
    /// Andernfalls Fehler – so bleibt die API symmetrisch zu `tao()`.
    #[inline]
    pub fn menu_or_err(&self) -> Result<Menu> {
        self.try_menu()?
            .ok_or_else(|| anyhow!("Window '{}' has no menu attached", self.label))
    }

    /// Nur `true`, wenn ein Menü vorhanden ist **und** es nicht app-weit ist.
    /// Solche Menüs darf das Fenster selbst manipulieren, ohne den
    /// globalen MenuManager (macOS) zu stören.
    pub fn has_local_menu(&self) -> bool {
        self.menu
            .lock()
            .map(|m| m.as_ref().is_some_and(|wm| !wm.is_app_wide))
            .unwrap_or(false)
    }

    // -------------------------------------------------------------------------
    // Suche / Traversierung (delegiert an Menu)
    // -------------------------------------------------------------------------
    /// Direkte Kinder des Menüs.
    pub fn menu_items(&self) -> Result<Vec<MenuItemKind>> {
        self.menu_or_err()?.items()
    }

    /// Direkter Treffer nur unter den Top-Level-Einträgen.
    pub fn get_menu_item_by_id(&self, id: &MenuId) -> Result<Option<MenuItemKind>> {
        self.menu_or_err()?.get(id)
    }

    /// Rekursive Suche im gesamten Menübaum.
    pub fn find_menu_item(&self, id: &MenuId) -> Result<Option<MenuItemKind>> {
        self.menu_or_err()?.find(id)
    }

    /// Alle Einträge im Baum, depth-first.
    pub fn all_menu_items(&self) -> Result<Vec<MenuItemKind>> {
        self.menu_or_err()?.all_items()
    }

    /// Besucht jeden Eintrag im Baum.
    pub fn visit_menu_items<F>(&self, visitor: F) -> Result<()>
    where
        F: FnMut(&MenuItemKind) -> Result<()>,
    {
        self.menu_or_err()?.visit(visitor)
    }

    // -------------------------------------------------------------------------
    // Typisierte Getter (delegiert an Menu)
    // -------------------------------------------------------------------------
    pub fn get_menu_item(&self, id: &MenuId) -> Result<Option<MenuItem>> {
        self.menu_or_err()?.get_menu_item(id)
    }

    pub fn get_submenu(&self, id: &MenuId) -> Result<Option<Submenu>> {
        self.menu_or_err()?.get_submenu(id)
    }

    pub fn get_check_menu_item(&self, id: &MenuId) -> Result<Option<CheckMenuItem>> {
        self.menu_or_err()?.get_check(id)
    }

    pub fn get_icon_menu_item(&self, id: &MenuId) -> Result<Option<IconMenuItem>> {
        self.menu_or_err()?.get_icon(id)
    }

    pub fn get_predefined_menu_item(&self, id: &MenuId) -> Result<Option<PredefinedMenuItem>> {
        self.menu_or_err()?.get_predefined(id)
    }

    // -------------------------------------------------------------------------
    // Mutation (delegiert an Menu)
    // -------------------------------------------------------------------------
    pub fn append_menu_item(&self, item: &dyn IsMenuItem) -> Result<()> {
        self.menu_or_err()?.append(item)
    }

    pub fn append_menu_items(&self, items: &[&dyn IsMenuItem]) -> Result<()> {
        self.menu_or_err()?.append_items(items)
    }

    pub fn prepend_menu_item(&self, item: &dyn IsMenuItem) -> Result<()> {
        self.menu_or_err()?.prepend(item)
    }

    pub fn insert_menu_item(&self, item: &dyn IsMenuItem, position: usize) -> Result<()> {
        self.menu_or_err()?.insert(item, position)
    }

    pub fn remove_menu_item(&self, item: &dyn IsMenuItem) -> Result<()> {
        self.menu_or_err()?.remove(item)
    }

    // -------------------------------------------------------------------------
    // Submenu-Helfer: direkt auf ein Submenu des Fensters zugreifen
    // -------------------------------------------------------------------------
    /// Hängt ein Item an ein Submenu des Fensters.
    pub fn append_to_submenu(&self, submenu_id: &MenuId, item: &dyn IsMenuItem) -> Result<()> {
        let submenu = self
            .get_submenu(submenu_id)?
            .ok_or_else(|| anyhow!("Submenu {:?} not found", submenu_id))?;
        submenu.append(item)
    }

    /// Iconnisiertes Submenu setzen (macOS/Windows).
    pub fn set_submenu_native_icon(&self, submenu_id: &MenuId, icon: Option<NativeIcon>) -> Result<()> {
        let submenu = self
            .get_submenu(submenu_id)?
            .ok_or_else(|| anyhow!("Submenu {:?} not found", submenu_id))?;
        submenu.set_native_icon(icon)
    }

    #[cfg(windows)]
    /// Zeichnet nur das passende, transparente Windows-Fenster neu.
    /// Späte Ereignisse nach Entnahme des nativen Handles werden ignoriert.
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
        // Snapshot aufnehmen und Farblock vor dem Surface-Lock freigeben.
        let color = *lock_state(&self.background_color, "background_color")?;
        let mut guard = lock_state(&self.surface, "surface")?;
        if let Some(surface) = guard.as_mut() {
            // Bestehender Projekt-Helfer; seine Implementierung liegt nicht vor.
            // Er muss Größenanpassung/Präsentation übernehmen und darf denselben
            // Surface-Mutex nicht erneut sperren.
            inner.draw_surface(surface, color);
        }
        Ok(())
    }
}

fn to_tao_icon(icon: taurino_core::image::Icon<'_>) -> Result<taurino_core::tao::window::Icon> {
    taurino_core::tao::window::Icon::from_rgba(icon.rgba.into_owned(), icon.width, icon.height)
        .map_err(|error| anyhow!("failed to create Tao icon from RGBA data: {error}"))
}

pub fn map_theme_from_tao(theme: taurino_core::tao::window::Theme) -> Theme {
    match theme {
        taurino_core::tao::window::Theme::Light => Theme::Light,
        taurino_core::tao::window::Theme::Dark => Theme::Dark,
        #[allow(unreachable_patterns)]
        _ => Theme::Light,
    }
}

// -----------------------------------------------------------------------------
// Ergänzende Zugriffe ohne Änderung der nativen Fensterzuordnung
// -----------------------------------------------------------------------------

impl Window {
    /// Tao-ID, nicht die u32-ID aus der Registry oder aus WebView::window_id().
    pub fn native_id(&self) -> Result<tao::window::WindowId> {
        Ok(self.tao()?.id())
    }

    pub fn is_available(&self) -> bool {
        self.inner.is_some()
    }

    pub fn webviews(&self) -> &[WebView] {
        &self.webviews_manager.webviews()
    }

    pub fn webview(&self, id: WebViewId) -> Option<&WebView> {
        self.webviews_manager
            .webviews()
            .iter()
            .find(|webview| webview.id() == id)
    }

    pub fn webview_by_label(&self, label: &str) -> Option<&WebView> {
        self.webviews_manager
            .webviews()
            .iter()
            .find(|webview| webview.label() == label)
    }

    /// Fordert Zeichnen an; zeichnet nicht synchron an dieser Aufrufstelle.
    pub fn request_redraw(&self) -> Result<()> {
        self.tao()?.request_redraw();
        Ok(())
    }

    /// Im Gegensatz zu menu() wird ein vergifteter Mutex als Fehler gemeldet.
    pub fn try_menu(&self) -> Result<Option<Menu>> {
        let guard = lock_state(&self.menu, "menu")?;
        Ok(guard.as_ref().map(|menu| menu.menu.clone()))
    }

    /// Entnimmt nur die gespeicherte Referenz; löst keine native Menüanbindung.
    pub fn try_take_window_menu(&self) -> Result<Option<WindowMenu>> {
        Ok(lock_state(&self.menu, "menu")?.take())
    }

    #[cfg(windows)]
    pub fn try_background_color(&self) -> Result<Option<tao::window::RGBA>> {
        Ok(*lock_state(&self.background_color, "background_color")?)
    }
}

/// Wandelt PoisonError ohne Übernahme seines Guards in einen eigenen Fehler um.
fn lock_state<'a, T>(mutex: &'a Mutex<T>, name: &str) -> Result<MutexGuard<'a, T>> {
    mutex.lock().map_err(|_| anyhow!("Window {name} mutex is poisoned"))
}
