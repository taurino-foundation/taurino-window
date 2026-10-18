pub mod monitor;
pub mod window;
#[cfg(windows)]
use std::sync::Arc;

#[cfg(windows)]
use crate::{
    config::{FocusState, WebViewId, WindowId},
    events::{EventLoopMessage, EventLoopProxy, SynthesizedEvent, WebViewEvent},
};
#[cfg(windows)]
use dpi::PhysicalSize;
#[cfg(windows)]
use parking_lot::Mutex;
#[cfg(windows)]
use webview2_com::{
    ContainsFullScreenElementChangedEventHandler, FocusChangedEventHandler,
    Microsoft::Web::WebView2::Win32::ICoreWebView2Controller,
};
#[cfg(windows)]
use wry::{WebView, WebViewExtWindows};

#[cfg(windows)]
pub fn apply_shadow_correction(
    decorations: bool,
    window_size: &mut PhysicalSize<u32>, // oder was auch immer der Typ ist
) -> anyhow::Result<u32> {
    #[allow(unused_mut)]
    let mut shadow_width = 0;

    if decorations {
        use windows::Win32::UI::WindowsAndMessaging::{AdjustWindowRect, WS_OVERLAPPEDWINDOW};
        let mut rect = windows::Win32::Foundation::RECT::default();
        let result = unsafe { AdjustWindowRect(&mut rect, WS_OVERLAPPEDWINDOW, false) };
        if result.is_ok() {
            shadow_width = (rect.right - rect.left) as u32;
            // rect.bottom is made out of shadow, and we don't care about it
            window_size.height += -rect.top as u32;
        }
    }

    Ok(shadow_width)
}

/// Used to prevent duplicated [`WindowEvent::Focused`] events,
/// and to track last focused webview in multi-webview mode for us to restore webview focuses
/// Used to prevent duplicated [`WindowEvent::Focused`] events,
/// and to track last focused webview in multi-webview mode for us to restore webview focuses
#[cfg(windows)]
fn add_focus_change_listeners(
    window_id: Arc<Mutex<WindowId>>,
    id: WebViewId,
    proxy: EventLoopProxy,
    focused_webview: Arc<Mutex<FocusState>>,
    label: String,
    controller: &ICoreWebView2Controller,
    token: &mut i64,
) {
    let label_ = label.clone();
    let window_id_ = window_id.clone();
    let proxy_clone = proxy.clone();
    let focused_webview_ = focused_webview.clone();
    if let Err(error) = unsafe {
        controller.add_GotFocus(
            &FocusChangedEventHandler::create(Box::new(move |_, _| {
                let mut focused_webview = focused_webview_.lock();
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
                    let _ = proxy_clone.send_event(EventLoopMessage::WebViewEvent(
                        *window_id_.lock(),
                        id,
                        WebViewEvent::SynthesizedEvent(SynthesizedEvent::Focused(false)),
                    ));
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
                let mut focused_webview = focused_webview.lock();
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
                        let _ = proxy.send_event(EventLoopMessage::WebViewEvent(
                            *window_id.lock(),
                            id,
                            WebViewEvent::SynthesizedEvent(SynthesizedEvent::Focused(true)),
                        ));
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

#[cfg(windows)]
pub fn register_webview_events(
    _window_id: Arc<Mutex<WindowId>>,
    _webview_id: WebViewId,
    _webview_label: String,
    _focused_webview: Arc<Mutex<FocusState>>,
    _webview: &WebView,
    _proxy: EventLoopProxy,
) {
    let id = _window_id.clone();
    let controller = _webview.controller();
    let mut token = 0;

    add_focus_change_listeners(
        id.clone(),
        _webview_id,
        _proxy.clone(),
        _focused_webview,
        _webview_label,
        &controller,
        &mut token,
    );

    if let Ok(webview) = unsafe { controller.CoreWebView2() } {
        unsafe {
            let _ = webview.add_ContainsFullScreenElementChanged(
                &ContainsFullScreenElementChangedEventHandler::create(Box::new(move |sender, _| {
                    let mut contains_fullscreen_element = windows::core::BOOL::default();

                    sender
                        .ok_or_else(windows::core::Error::empty)?
                        .ContainsFullScreenElement(&mut contains_fullscreen_element)?;

                    let _ = _proxy.send_event(EventLoopMessage::FullscreenChanged(
                        *id.lock(),
                        contains_fullscreen_element.as_bool(),
                    ));

                    Ok(())
                })),
                &mut token,
            );
        }
    }
}
