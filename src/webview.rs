#[cfg(windows)]
use crate::config::FocusState;
use crate::utils::{WebContextStore, WebviewBounds};
use std::{
    ops::Deref,
    path::PathBuf,
    rc::Rc,
    sync::{Arc, Mutex},
};
#[cfg(target_os = "macos")]
use taurino_core::dpi::LogicalSize;
#[cfg(target_os = "macos")]
use taurino_core::objc2::{MainThreadMarker, rc::Retained};
#[cfg(target_os = "macos")]
use taurino_core::objc2_app_kit::NSView;
#[cfg(windows)]
use taurino_core::tao::platform::windows::WindowExtWindows;
use taurino_core::{
    anyhow::{Result, anyhow},
    dpi::PhysicalSize,
    tao::{self, window::Window},
};
#[cfg(windows)]
use taurino_core::{
    log,
    webview2_com::{FocusChangedEventHandler, Microsoft::Web::WebView2::Win32::ICoreWebView2Controller},
};
// ------------------------------------------------------------
// WebView
// ------------------------------------------------------------
#[derive(Clone)]
pub struct WebView {
    label: String,
    id: u32,
    window_id: Arc<Mutex<u32>>,
    inner: Rc<taurino_core::wry::WebView>,
    context_store: WebContextStore,
    context_key: Option<PathBuf>,
    bounds: Arc<Mutex<Option<WebviewBounds>>>,
}
impl WebView {
    /// Übernimmt eine bereits erstellte native WebView.
    ///
    /// Der zugehörige Kontext und dessen Label-Registrierung müssen
    /// bereits vom aufrufenden Code eingerichtet worden sein.
    ///
    /// Weitere Handles derselben WebView über `clone()` erzeugen,
    /// nicht durch erneute Aufrufe von `new()` mit demselben `inner`.
    pub fn new(
        id: u32,
        label: impl Into<String>,
        window_id: Arc<Mutex<u32>>,
        inner: Rc<taurino_core::wry::WebView>,
        context_key: Option<PathBuf>,
        context_store: WebContextStore,
        bounds: Arc<Mutex<Option<WebviewBounds>>>,
    ) -> Self {
        Self {
            label: label.into(),
            id,
            window_id,
            inner,
            context_store,
            context_key,
            bounds,
        }
    }
    // --------------------------------------------------------
    // Identität
    // --------------------------------------------------------
    pub fn id(&self) -> u32 {
        self.id
    }
    pub fn label(&self) -> &str {
        &self.label
    }
    // --------------------------------------------------------
    // Fensterzuordnung
    // --------------------------------------------------------
    pub fn window_id(&self) -> u32 {
        *self.window_id.lock().expect("WebView window_id mutex is poisoned")
    }
    pub fn window_id_handle(&self) -> Arc<Mutex<u32>> {
        Arc::clone(&self.window_id)
    }
    /// Ändert ausschließlich die gespeicherte Fenster-ID.
    ///
    /// Die native WebView wird dadurch nicht verschoben.
    /// Für einen Fensterwechsel normalerweise `reparent()` verwenden.
    pub fn set_window_id(&self, window_id: u32) {
        *self.window_id.lock().expect("WebView window_id mutex is poisoned") = window_id;
    }
    // --------------------------------------------------------
    // Native WebView
    // --------------------------------------------------------
    /// Liefert den vorhandenen Rc, ohne ihn zu klonen.
    ///
    /// Ein extern erzeugter Rc-Klon darf nicht länger als alle
    /// WebView-Wrapper leben, wenn deren Drop die Kontextreferenz
    /// zuverlässig entfernen soll.
    pub fn inner(&self) -> &Rc<taurino_core::wry::WebView> {
        &self.inner
    }
    /// Eindeutiger Zugriff auf Wry, auch bei gleichnamigen Methoden.
    pub fn as_wry(&self) -> &taurino_core::wry::WebView {
        self.inner.as_ref()
    }
    /// Bestehender Low-Level-Zugriff aus deiner ursprünglichen API.
    ///
    /// Das Austauschen des Rc aktualisiert weder Kontextreferenzen
    /// noch Fensterzuordnung oder Bounds.
    #[deprecated(note = "Das Ersetzen von inner umgeht die Kontextverwaltung. \
                Für normale Wry-Aufrufe as_wry() verwenden.")]
    pub fn inner_mut(&mut self) -> &mut Rc<taurino_core::wry::WebView> {
        &mut self.inner
    }
    // --------------------------------------------------------
    // Kontext
    // --------------------------------------------------------
    /// Liefert den unveränderten Schlüssel für den Context-Store.
    pub fn context_key(&self) -> &Option<PathBuf> {
        &self.context_key
    }
    pub fn context_store(&self) -> &WebContextStore {
        &self.context_store
    }
    /// Klont den gemeinsam genutzten Store-Handle.
    pub fn context_store_handle(&self) -> WebContextStore {
        self.context_store.clone()
    }
    // --------------------------------------------------------
    // Gespeicherte Layout-Bounds
    // --------------------------------------------------------
    /// Liest die gespeicherten Bounds, ohne sie zu entfernen.
    ///
    /// Dies ist NICHT taurino_core::wry::WebView::bounds().
    /// WebviewBounds muss Clone implementieren.
    pub fn bounds(&self) -> Option<WebviewBounds> {
        self.bounds.lock().expect("WebView bounds mutex is poisoned").clone()
    }
    pub fn bounds_handle(&self) -> Arc<Mutex<Option<WebviewBounds>>> {
        Arc::clone(&self.bounds)
    }
    /// Ändert ausschließlich die gespeicherten Layout-Daten.
    ///
    /// Position und Größe der nativen WebView bleiben unverändert.
    pub fn set_cached_bounds(&self, bounds: Option<WebviewBounds>) {
        *self.bounds.lock().expect("WebView bounds mutex is poisoned") = bounds;
    }
    /// Entnimmt die gespeicherten Bounds.
    ///
    /// Danach enthalten alle Wrapper-Klone an dieser Stelle None.
    pub fn take_bounds(&self) -> Option<WebviewBounds> {
        self.bounds.lock().expect("WebView bounds mutex is poisoned").take()
    }
    pub fn clear_bounds(&self) {
        self.set_cached_bounds(None);
    }
    // --------------------------------------------------------
    // Tatsächliche native Geometrie
    // --------------------------------------------------------
    /// Fragt die aktuelle Geometrie direkt bei Wry ab.
    pub fn native_bounds(&self) -> Result<taurino_core::wry::Rect> {
        self.as_wry()
            .bounds()
            .map_err(|error| anyhow!("failed to read native bounds for webview '{}': {error}", self.label))
    }
    /// Ändert die native Geometrie.
    ///
    /// Die gespeicherten WebviewBounds werden nicht automatisch
    /// angepasst: deren Umrechnung gehört in deinen Layout-Code.
    pub fn set_window_bounds(&self, bounds: taurino_core::wry::Rect) -> Result<()> {
        self.as_wry()
            .set_bounds(bounds)
            .map_err(|error| anyhow!("failed to set native bounds for webview '{}': {error}", self.label))
    }
}
// ------------------------------------------------------------
// Trait-Implementierungen
// ------------------------------------------------------------
impl Deref for WebView {
    type Target = taurino_core::wry::WebView;
    fn deref(&self) -> &Self::Target {
        self.as_wry()
    }
}
impl AsRef<taurino_core::wry::WebView> for WebView {
    fn as_ref(&self) -> &taurino_core::wry::WebView {
        self.as_wry()
    }
}
impl Drop for WebView {
    fn drop(&mut self) {
        // Nur aufräumen, wenn dieser Wrapper den letzten starken
        // Rc auf die native WebView hält.
        //
        // Anders als Rc::get_mut berücksichtigt dieser Test nicht
        // zusätzlich vorhandene Weak-Referenzen.
        if Rc::strong_count(&self.inner) != 1 {
            return;
        }
        // Bei einem vergifteten Store keine weitere Panic auslösen.
        // In diesem Fehlerfall bleibt die Registrierung erhalten.
        //
        // Den Wrapper nicht droppen, während derselbe Thread
        // bereits den Context-Store-Lock hält.
        let Ok(mut context_store) = self.context_store.lock() else {
            return;
        };
        if let Some(web_context) = context_store.get_mut(&self.context_key) {
            web_context.referenced_by_webviews.remove(&self.label);
            // Linux/BSD: Kontext zur Wiederverwendung behalten.
            // Andere Plattformen: ungenutzten Kontext entfernen.
            #[cfg(not(any(
                target_os = "linux",
                target_os = "dragonfly",
                target_os = "freebsd",
                target_os = "netbsd",
                target_os = "openbsd"
            )))]
            if web_context.referenced_by_webviews.is_empty() {
                context_store.remove(&self.context_key);
            }
        }
    }
}
// ------------------------------------------------------------
// Freie Funktionen für bestehende Aufrufstellen
// ------------------------------------------------------------
#[cfg(target_os = "macos")]
pub fn reparent_native(webview: &WebView, target: &Arc<tao::window::Window>) -> Result<()> {
    use tao::platform::macos::WindowExtMacOS;
    use taurino_core::wry::WebViewExtMacOS;
    webview
        .inner()
        .reparent(target.ns_window() as _)
        .map_err(|e| anyhow!("reparent failed: {e}"))
}
#[cfg(windows)]
pub fn reparent_native(webview: &WebView, target: &Arc<tao::window::Window>) -> Result<()> {
    use taurino_core::wry::WebViewExtWindows;
    webview
        .inner()
        .reparent(target.hwnd())
        .map_err(|e| anyhow!("reparent failed: {e}"))
}
#[cfg(any(
    target_os = "linux",
    target_os = "dragonfly",
    target_os = "freebsd",
    target_os = "netbsd",
    target_os = "openbsd"
))]
pub fn reparent_native(webview: &WebView, target: &Arc<tao::window::Window>) -> Result<()> {
    use tao::platform::unix::WindowExtUnix;
    use taurino_core::wry::WebViewExtUnix;
    let container = target
        .default_vbox()
        .ok_or_else(|| anyhow!("target window has no default vbox"))?;
    webview
        .inner()
        .reparent(container)
        .map_err(|e| anyhow!("reparent failed: {e}"))
}
#[cfg(target_os = "macos")]
pub fn inner_size(window: &Window, webviews: &[WebView], has_children: bool) -> PhysicalSize<u32> {
    use taurino_core::wry::WebViewExtMacOS;
    if !has_children {
        if let Some(webview) = webviews.first() {
            let _main_thread =
                MainThreadMarker::new().expect("native view measurement must run on the macOS main thread");
            let native_webview = webview.as_wry().webview();
            // SAFETY:
            // Wry liefert seine WKWebView-Unterklasse zurück.
            // WKWebView ist auf macOS eine NSView-Unterklasse.
            // Der Zugriff erfolgt nach Prüfung auf dem Main Thread.
            let view = unsafe { Retained::cast_unchecked::<NSView>(native_webview) };
            let frame = view.frame();
            return LogicalSize::<f64>::new(frame.size.width, frame.size.height).to_physical(window.scale_factor());
        }
    }
    let size = window.inner_size();
    // Explizite Übernahme vermeidet eine Abhängigkeit davon,
    // ob taurino_core::dpi und Tao identische Typen reexportieren.
    PhysicalSize::new(size.width, size.height)
}
#[cfg(not(target_os = "macos"))]
pub fn inner_size(window: &Window, _webviews: &[WebView], _has_children: bool) -> PhysicalSize<u32> {
    let size = window.inner_size();
    PhysicalSize::new(size.width, size.height)
}
/// Used to prevent duplicated [`WindowEvent::Focused`] events,
/// and to track last focused webview in multi-webview mode for us to restore webview focuses
/// Used to prevent duplicated [`WindowEvent::Focused`] events,
/// and to track last focused webview in multi-webview mode for us to restore webview focuses
#[cfg(windows)]
pub fn add_focus_change_listeners(
    window_id: Arc<Mutex<u32>>,
    id: u32,
    focused_webview: Arc<Mutex<FocusState>>,
    label: String,
    controller: &ICoreWebView2Controller,
    token: &mut i64,
    on_focus_change: impl Fn(u32, u32, bool) + Send + Sync + 'static,
) {
    let label_ = label.clone();
    let window_id_ = window_id.clone();
    let focused_webview_ = focused_webview.clone();
    let on_focus_change = Arc::new(on_focus_change);
    let on_focus_change_got = on_focus_change.clone();
    if let Err(error) = unsafe {
        controller.add_GotFocus(
            &FocusChangedEventHandler::create(Box::new(move |_, _| {
                let mut focused_webview = focused_webview_.lock().unwrap();
                // when using multiwebview mode, we should check if the focus change is actually a "webview focus change"
                // instead of a window focus change (here we're patching window events, so we only care about the actual window changing focus)
                let already_focused = matches!(
                    *focused_webview,
                    FocusState::WindowFocused | FocusState::WebviewFocused { .. }
                );
                *focused_webview = FocusState::WebviewFocused {
                    webview_label: label_.clone(),
                };
                if !already_focused {
                    on_focus_change_got(*window_id_.lock().unwrap(), id, true);
                }
                Ok(())
            })),
            token,
        )
    } {
        log::error!(
            "Failed to attach WebView2 `add_GotFocus` handler, `WindowEvent::Focused` will not be sent: {error}"
        );
        return;
    }
    if let Err(error) = unsafe {
        controller.add_LostFocus(
            &FocusChangedEventHandler::create(Box::new(move |_, _| {
                use crate::config::FocusState;
                let mut focused_webview = focused_webview.lock().unwrap();
                // when using multiwebview mode, we should handle webview focus changes
                // so we check is the currently focused webview matches this webview's
                // (in this case, it means we lost the window focus)
                //
                // on multiwebview mode if we change focus to a different webview
                // we get the gotFocus event of the other webview before the lostFocus
                // so this check makes sense
                if let FocusState::WebviewFocused { ref webview_label } = *focused_webview {
                    let lost_window_focus = webview_label == &label;
                    if lost_window_focus {
                        // only reset when we lost window focus - otherwise some other webview is focused
                        *focused_webview = FocusState::Blured {
                            last_focused_webview_label: Some(label.clone()),
                        };
                        on_focus_change(*window_id.lock().unwrap(), id, false);
                    }
                }
                Ok(())
            })),
            token,
        )
    } {
        log::error!(
            "Failed to attach WebView2 `add_LostFocus` handler, `WindowEvent::Focused` will not be sent: {error}"
        );
    }
}
