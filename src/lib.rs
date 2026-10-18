
#![cfg_attr(docsrs, feature(doc_cfg))]

// ============================================================
// Modules
// ============================================================

pub mod builder;
pub mod config;
pub mod events;
mod utils;
pub mod webview;
pub mod window;

mod platform_webview;
mod wrappers;

use crate::config::PhysicalRect;

// ============================================================
// Platform Modules
// ============================================================

#[cfg(windows)]
pub mod windows;

#[cfg(target_os = "macos")]
pub mod macos;

#[cfg(any(
    windows,
    target_os = "linux",
    target_os = "dragonfly",
    target_os = "freebsd",
    target_os = "netbsd",
    target_os = "openbsd",
))]
pub mod undecorated_resizing;

#[cfg(any(
    target_os = "linux",
    target_os = "dragonfly",
    target_os = "freebsd",
    target_os = "netbsd",
    target_os = "openbsd",
))]
pub mod linux;

// ============================================================
// Cross-Platform Low-Level Reexports
// ============================================================

pub use {
    anyhow,
    dpi,
    getrandom,
    log,
    parking_lot,
    raw_window_handle,
    serde,
    serde_json,
    serde_with,
    serialize_to_javascript,
    tao,
    taurino_tools,
    url,
    uuid,
    wry,
};

// ============================================================
// Windows Low-Level Reexports
// ============================================================

#[cfg(windows)]
pub use {
    once_cell,
    softbuffer,
    webview2_com,
    windows as windows_api,
};

// ============================================================
// macOS Low-Level Reexports
// ============================================================

#[cfg(target_os = "macos")]
pub use {
    block2,
    objc2,
    objc2_app_kit,
    objc2_foundation,
    objc2_web_kit,
    security_framework,
    x509_parser,
    x_win,
};

// ============================================================
// Linux / BSD Low-Level Reexports
// ============================================================

#[cfg(any(
    target_os = "linux",
    target_os = "dragonfly",
    target_os = "freebsd",
    target_os = "netbsd",
    target_os = "openbsd",
))]
pub use {
    gtk,
    webkit2gtk,
};




pub trait MonitorExt {
    /// Get the work area of this monitor
    ///
    /// ## Platform-specific:
    ///
    /// - **Android / iOS**: Unsupported.
    fn work_area(&self) -> PhysicalRect<i32, u32>;
}

pub trait WindowExt {
    /// Enable or disable the window
    ///
    /// ## Platform-specific:
    ///
    /// - **Android / iOS**: Unsupported.
    fn set_enabled(&self, enabled: bool);

    /// Whether the window is enabled or disabled.
    ///
    /// ## Platform-specific:
    ///
    /// - **Android / iOS**: Unsupported, always returns `true`.
    fn is_enabled(&self) -> bool;

    /// Center the window
    ///
    /// ## Platform-specific:
    ///
    /// - **Android / iOS**: Unsupported.
    fn center(&self) {}

    /// Clears the window surface. i.e make it transparent.
    #[cfg(windows)]
    fn draw_surface(
        &self,
        surface: &mut softbuffer::Surface<std::sync::Arc<tao::window::Window>, std::sync::Arc<tao::window::Window>>,
        background_color: Option<tao::window::RGBA>,
    );
}

/// The platform webview handle. Accessed with [`Webview#method.with_webview`];

pub struct PlatformWebview(platform_webview::Webview);

impl PlatformWebview {
    /// Returns [`webkit2gtk::WebView`] handle.
    #[cfg(any(
        target_os = "linux",
        target_os = "dragonfly",
        target_os = "freebsd",
        target_os = "netbsd",
        target_os = "openbsd"
    ))]
    #[cfg_attr(
        docsrs,
        doc(cfg(any(
            target_os = "linux",
            target_os = "dragonfly",
            target_os = "freebsd",
            target_os = "netbsd",
            target_os = "openbsd"
        )))
    )]
    pub fn inner(&self) -> webkit2gtk::WebView {
        self.0.clone()
    }

    /// Returns the WebView2 controller.
    #[cfg(windows)]
    #[cfg_attr(docsrs, doc(cfg(windows)))]
    pub fn controller(&self) -> webview2_com::Microsoft::Web::WebView2::Win32::ICoreWebView2Controller {
        self.0.controller.clone()
    }

    /// Returns the WebView2 environment.
    #[cfg(windows)]
    #[cfg_attr(docsrs, doc(cfg(windows)))]
    pub fn environment(&self) -> webview2_com::Microsoft::Web::WebView2::Win32::ICoreWebView2Environment {
        self.0.environment.clone()
    }

    /// Returns the [WKWebView] handle.
    ///
    /// [WKWebView]: https://developer.apple.com/documentation/webkit/wkwebview
    #[cfg(any(target_os = "macos", target_os = "ios"))]
    #[cfg_attr(docsrs, doc(cfg(any(target_os = "macos", target_os = "ios"))))]
    pub fn inner(&self) -> *mut std::ffi::c_void {
        self.0.webview
    }

    /// Returns WKWebView [controller] handle.
    ///
    /// [controller]: https://developer.apple.com/documentation/webkit/wkusercontentcontroller
    #[cfg(any(target_os = "macos", target_os = "ios"))]
    #[cfg_attr(docsrs, doc(cfg(any(target_os = "macos", target_os = "ios"))))]
    pub fn controller(&self) -> *mut std::ffi::c_void {
        self.0.manager
    }

    /// Returns [NSWindow] associated with the WKWebView webview.
    ///
    /// [NSWindow]: https://developer.apple.com/documentation/appkit/nswindow
    #[cfg(target_os = "macos")]
    #[cfg_attr(docsrs, doc(cfg(target_os = "macos")))]
    pub fn ns_window(&self) -> *mut std::ffi::c_void {
        self.0.ns_window
    }

    /// Returns [UIViewController] used by the WKWebView webview NSWindow.
    ///
    /// [UIViewController]: https://developer.apple.com/documentation/uikit/uiviewcontroller
    #[cfg(target_os = "ios")]
    #[cfg_attr(docsrs, doc(cfg(target_os = "ios")))]
    pub fn view_controller(&self) -> *mut std::ffi::c_void {
        self.0.view_controller
    }

    /// Returns handle for JNI execution.
    #[cfg(target_os = "android")]
    pub fn jni_handle(&self) -> wry::JniHandle {
        self.0
    }
}
