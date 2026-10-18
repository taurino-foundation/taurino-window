use {
    crate::config::window_effects::{WindowEffect, WindowEffectState},
    anyhow::Result,
    dpi::{LogicalPosition, PhysicalPosition, PhysicalSize, PixelUnit},
    parking_lot::Mutex,
    serde::{Deserialize, Deserializer, Serialize, Serializer},
    serde_with::skip_serializing_none,
    std::{
        collections::{HashMap, HashSet},
        fmt::{self, Display},
        path::PathBuf,
        str::FromStr,
        sync::Arc,
    },
    taurino_tools::{Rect, Theme},
    url::Url,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PredefinedAction {
    Separator,
    Copy,
    Cut,
    Paste,
    SelectAll,
    Undo,
    Redo,
    Minimize,
    Maximize,
    Fullscreen,
    Hide,
    HideOthers,
    ShowAll,
    CloseWindow,
    Quit,
    Services,
    BringAllToFront,
    About,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageConfig {
    pub kind: String,
    pub path: Option<String>,
    pub data: Option<Vec<u8>>,
    pub rgba: Option<Vec<u8>>,
    pub width: Option<u32>,
    pub height: Option<u32>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AboutMetadataConfig {
    pub name: Option<String>,
    pub version: Option<String>,
    pub short_version: Option<String>,
    pub authors: Option<Vec<String>>,
    pub comments: Option<String>,
    pub copyright: Option<String>,
    pub license: Option<String>,
    pub website: Option<String>,
    pub website_label: Option<String>,
    pub credits: Option<String>,
    pub icon: Option<ImageConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum MenuItemConfig {
    Text {
        id: Option<String>,
        text: String,
        enabled: bool,
        accelerator: Option<String>,
    },
    Check {
        id: Option<String>,
        text: String,
        enabled: bool,
        checked: bool,
        accelerator: Option<String>,
    },
    Icon {
        id: Option<String>,
        text: String,
        enabled: bool,
        icon: Option<ImageConfig>,
        native_icon: Option<String>,
        accelerator: Option<String>,
    },
    Predefined {
        action: PredefinedAction,
        metadata: Option<AboutMetadataConfig>,
    },
    Submenu {
        id: Option<String>,
        text: String,
        enabled: bool,
        icon: Option<ImageConfig>,
        native_icon: Option<String>,
        items: Vec<MenuItemConfig>,
    },
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MenuConfig {
    pub id: Option<String>,
    #[serde(default)]
    pub items: Vec<MenuItemConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrayIconConfig {
    pub id: String,
    pub menu: Option<MenuConfig>,
    pub icon: Option<ImageConfig>,
    pub tooltip: Option<String>,
    pub title: Option<String>,
    pub temp_dir_path: Option<String>,
    #[serde(default)]
    pub icon_as_template: bool,
    #[serde(default = "default_true")]
    pub show_menu_on_left_click: bool,
    #[serde(default = "default_true")]
    pub visible: bool,
}

fn default_true() -> bool {
    true
}

/// Identifier of a webview.
#[derive(Debug, Clone, Copy, Hash, Eq, PartialEq, Ord, PartialOrd, serde::Serialize, serde::Deserialize)]
pub struct WebViewId(u32);

impl From<u32> for WebViewId {
    fn from(value: u32) -> Self {
        Self(value)
    }
}

impl WebViewId {
    pub fn get(self) -> u32 {
        self.0
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct WebviewBounds {
    pub x_rate: f32,
    pub y_rate: f32,
    pub width_rate: f32,
    pub height_rate: f32,
}

#[cfg(windows)]
#[derive(Debug, Serialize, Deserialize)]
pub enum FocusState {
    WindowFocused,
    WebviewFocused { webview_label: String },
    Blured { last_focused_webview_label: Option<String> },
}

#[cfg(windows)]
impl Default for FocusState {
    fn default() -> Self {
        Self::Blured {
            last_focused_webview_label: None,
        }
    }
}

/// A tuple struct of RGBA colors. Each value has minimum of 0 and maximum of 255.
#[derive(Debug, PartialEq, Eq, Serialize, Default, Clone, Copy)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Color(pub u8, pub u8, pub u8, pub u8);

impl From<Color> for (u8, u8, u8, u8) {
    fn from(value: Color) -> Self {
        (value.0, value.1, value.2, value.3)
    }
}

impl From<Color> for (u8, u8, u8) {
    fn from(value: Color) -> Self {
        (value.0, value.1, value.2)
    }
}

impl From<(u8, u8, u8, u8)> for Color {
    fn from(value: (u8, u8, u8, u8)) -> Self {
        Color(value.0, value.1, value.2, value.3)
    }
}

impl From<(u8, u8, u8)> for Color {
    fn from(value: (u8, u8, u8)) -> Self {
        Color(value.0, value.1, value.2, 255)
    }
}

impl From<Color> for [u8; 4] {
    fn from(value: Color) -> Self {
        [value.0, value.1, value.2, value.3]
    }
}

impl From<Color> for [u8; 3] {
    fn from(value: Color) -> Self {
        [value.0, value.1, value.2]
    }
}

impl From<[u8; 4]> for Color {
    fn from(value: [u8; 4]) -> Self {
        Color(value[0], value[1], value[2], value[3])
    }
}

impl From<[u8; 3]> for Color {
    fn from(value: [u8; 3]) -> Self {
        Color(value[0], value[1], value[2], 255)
    }
}

impl FromStr for Color {
    type Err = String;
    fn from_str(mut color: &str) -> Result<Self, Self::Err> {
        color = color.trim().strip_prefix('#').unwrap_or(color);
        let color = match color.len() {
            3 => color
                .chars()
                .flat_map(|c| std::iter::repeat_n(c, 2))
                .chain(std::iter::repeat_n('f', 2))
                .collect(),
            6 => format!("{color}FF"),
            8 => color.to_string(),
            _ => {
                return Err(
                    "Invalid hex color length, must be either 3, 6 or 8, for example: #fff, #ffffff, or #ffffffff"
                        .into(),
                );
            }
        };

        let r = u8::from_str_radix(&color[0..2], 16).map_err(|e| e.to_string())?;
        let g = u8::from_str_radix(&color[2..4], 16).map_err(|e| e.to_string())?;
        let b = u8::from_str_radix(&color[4..6], 16).map_err(|e| e.to_string())?;
        let a = u8::from_str_radix(&color[6..8], 16).map_err(|e| e.to_string())?;

        Ok(Color(r, g, b, a))
    }
}

fn default_alpha() -> u8 {
    255
}

#[derive(Deserialize)]
#[serde(untagged)]
enum InnerColor {
    /// Color hex string, for example: #fff, #ffffff, or #ffffffff.
    String(String),
    /// Array of RGB colors. Each value has minimum of 0 and maximum of 255.
    Rgb((u8, u8, u8)),
    /// Array of RGBA colors. Each value has minimum of 0 and maximum of 255.
    Rgba((u8, u8, u8, u8)),
    /// Object of red, green, blue, alpha color values. Each value has minimum of 0 and maximum of 255.
    RgbaObject {
        red: u8,
        green: u8,
        blue: u8,
        #[serde(default = "default_alpha")]
        alpha: u8,
    },
}

impl<'de> Deserialize<'de> for Color {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let color = InnerColor::deserialize(deserializer)?;
        let color = match color {
            InnerColor::String(string) => string.parse().map_err(serde::de::Error::custom)?,
            InnerColor::Rgb(rgb) => Color(rgb.0, rgb.1, rgb.2, 255),
            InnerColor::Rgba(rgb) => rgb.into(),
            InnerColor::RgbaObject {
                red,
                green,
                blue,
                alpha,
            } => Color(red, green, blue, alpha),
        };

        Ok(color)
    }
}

/// An URL to open on a Taurino webview window.
#[derive(PartialEq, Debug, Clone, Serialize)]
#[serde(untagged)]
#[non_exhaustive]
pub enum WebviewUrl {
    /// An external URL. Must use either the `http` or `https` schemes.
    External(Url),
    /// The path portion of an app URL.
    /// For instance, to load `tauri://localhost/users/john`,
    /// you can simply provide `users/john` in this configuration.
    App(PathBuf),
    /// A custom protocol url, for example, `doom://index.html`
    CustomProtocol(Url),
}

impl<'de> Deserialize<'de> for WebviewUrl {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum WebviewUrlDeserializer {
            Url(Url),
            Path(PathBuf),
        }

        match WebviewUrlDeserializer::deserialize(deserializer)? {
            WebviewUrlDeserializer::Url(u) => {
                if u.scheme() == "https" || u.scheme() == "http" {
                    Ok(Self::External(u))
                } else {
                    Ok(Self::CustomProtocol(u))
                }
            }
            WebviewUrlDeserializer::Path(p) => Ok(Self::App(p)),
        }
    }
}

impl fmt::Display for WebviewUrl {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::External(url) | Self::CustomProtocol(url) => {
                write!(f, "{url}")
            }
            Self::App(path) => write!(f, "{}", path.display()),
        }
    }
}

impl Default for WebviewUrl {
    fn default() -> Self {
        Self::App("index.html".into())
    }
}

impl WebviewUrl {
    pub fn is_about_blank(&self) -> bool {
        matches!(
            self,
            Self::CustomProtocol(url)
                if url.scheme() == "about" && url.path() == "blank"
        )
    }
}

#[skip_serializing_none]
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WebViewConfig {
    pub label: String,
    /// Whether the webview is a child of the window or not. Defaults to `false`.
    #[serde(default)]
    pub child: bool,
    #[serde(default)]
    pub auto_resize: bool,
    /// The window webview URL.
    #[serde(default)]
    pub url: WebviewUrl,
    /// The user agent for the webview
    #[serde(alias = "user-agent")]
    pub user_agent: Option<String>,
    /// Whether the drag and drop handlers used internally to generate [`DragDropEvent`]s are enabled on the webview. By default it is enabled.
    ///
    /// Disabling it is required to use HTML5 drag and drop on the frontend on Windows since we replace the drag drop handler of WebView2.
    ///
    /// Note: this setting maps to [`WebviewBuilder::disable_drag_drop_handler`], not [`WindowBuilder::drag_and_drop`].
    ///
    /// [`DragDropEvent`]: https://docs.rs/tauri/latest/tauri/enum.DragDropEvent.html
    /// [`WebviewBuilder::disable_drag_drop_handler`]: https://docs.rs/tauri/latest/tauri/webview/struct.WebviewBuilder.html#method.disable_drag_drop_handler
    /// [`WindowBuilder::drag_and_drop`]: https://docs.rs/tauri/latest/x86_64-pc-windows-msvc/tauri/window/struct.WindowBuilder.html#method.drag_and_drop
    #[serde(default = "default_true", alias = "drag-drop-enabled")]
    pub drag_drop_enabled: bool,
    /// Defines additional browser arguments on Windows.
    ///
    /// ## Warning
    ///
    /// Webview instances with different browser arguments must also have different [data directories](Self::data_directory).
    ///
    /// By default wry passes `--disable-features=msWebOOUI,msPdfOOUI,msSmartScreenProtection`
    /// so if you set this, you also need to disable these components by yourself if you want.
    #[serde(default, alias = "additional-browser-args")]
    pub additional_browser_args: Option<String>,
    /// Whether or not the webview should be launched in incognito  mode.
    ///
    /// ## Platform-specific:
    ///
    /// - **Android**: Unsupported.
    #[serde(default)]
    pub incognito: bool,
    /// The proxy URL for the WebView for all network requests.
    ///
    /// Must be either a `http://` or a `socks5://` URL.
    ///
    /// ## Platform-specific
    ///
    /// - **macOS**: Requires the `macos-proxy` feature flag and only compiles for macOS 14+.
    #[serde(alias = "proxy-url")]
    pub proxy_url: Option<Url>,
    /// Whether page zooming by hotkeys is enabled
    ///
    /// ## Platform-specific:
    ///
    /// - **Windows**: Controls WebView2's [`IsZoomControlEnabled`](https://learn.microsoft.com/en-us/microsoft-edge/webview2/reference/winrt/microsoft_web_webview2_core/corewebview2settings?view=webview2-winrt-1.0.2420.47#iszoomcontrolenabled) setting.
    /// - **MacOS / Linux**: Injects a polyfill that zooms in and out with `ctrl/command` + `-/=`,
    /// 20% in each step, ranging from 20% to 1000%. Requires `webview:allow-set-webview-zoom` permission
    ///
    /// - **Android / iOS**: Unsupported.
    #[serde(default, alias = "zoom-hotkeys-enabled")]
    pub zoom_hotkeys_enabled: bool,
    /// Whether browser extensions can be installed for the webview process
    ///
    /// ## Platform-specific:
    ///
    /// - **Windows**: Enables the WebView2 environment's [`AreBrowserExtensionsEnabled`](https://learn.microsoft.com/en-us/microsoft-edge/webview2/reference/winrt/microsoft_web_webview2_core/corewebview2environmentoptions?view=webview2-winrt-1.0.2739.15#arebrowserextensionsenabled)
    /// - **MacOS / Linux / iOS / Android** - Unsupported.
    #[serde(default, alias = "browser-extensions-enabled")]
    pub browser_extensions_enabled: bool,

    /// Sets whether the custom protocols should use `https://<scheme>.localhost` instead of the default `http://<scheme>.localhost` on Windows and Android. Defaults to `false`.
    ///
    /// ## Note
    ///
    /// Using a `https` scheme will NOT allow mixed content when trying to fetch `http` endpoints and therefore will not match the behavior of the `<scheme>://localhost` protocols used on macOS and Linux.
    ///
    /// ## Warning
    ///
    /// Changing this value between releases will change the IndexedDB, cookies and localstorage location and your app will not be able to access the old data.
    #[serde(default, alias = "use-https-scheme")]
    pub use_https_scheme: bool,
    /// Enable web inspector which is usually called browser devtools. Enabled by default.
    ///
    /// This API works in **debug** builds, but requires `devtools` feature flag to enable it in **release** builds.
    ///
    /// ## Platform-specific
    ///
    /// - macOS: This will call private functions on **macOS**.
    /// - Android: Open `chrome://inspect/#devices` in Chrome to get the devtools window. Wry's `WebView` devtools API isn't supported on Android.
    /// - iOS: Open Safari > Develop > [Your Device Name] > [Your WebView] to get the devtools window.
    pub devtools: Option<bool>,

    /// Change the default background throttling behaviour.
    ///
    /// By default, browsers use a suspend policy that will throttle timers and even unload
    /// the whole tab (view) to free resources after roughly 5 minutes when a view became
    /// minimized or hidden. This will pause all tasks until the documents visibility state
    /// changes back from hidden to visible by bringing the view back to the foreground.
    ///
    /// ## Platform-specific
    ///
    /// - **Linux / Windows / Android**: Unsupported. Workarounds like a pending WebLock transaction might suffice.
    /// - **iOS**: Supported since version 17.0+.
    /// - **macOS**: Supported since version 14.0+.
    ///
    /// see <https://github.com/tauri-apps/tauri/issues/5250#issuecomment-2569380578>
    #[serde(default, alias = "background-throttling")]
    pub background_throttling: Option<BackgroundThrottlingPolicy>,
    /// Whether we should disable JavaScript code execution on the webview or not.
    #[serde(default, alias = "javascript-disabled")]
    pub javascript_disabled: bool,
    /// on macOS and iOS there is a link preview on long pressing links, this is enabled by default.
    /// see https://docs.rs/objc2-web-kit/latest/objc2_web_kit/struct.WKWebView.html#method.allowsLinkPreview
    #[serde(default = "default_true", alias = "allow-link-preview")]
    pub allow_link_preview: bool,
    /// Allows disabling the input accessory view on iOS.
    ///
    /// The accessory view is the view that appears above the keyboard when a text input element is focused.
    /// It usually displays a view with "Done", "Next" buttons.
    #[serde(
        default,
        alias = "disable-input-accessory-view",
        alias = "disable_input_accessory_view"
    )]
    pub disable_input_accessory_view: bool,
    /// Set a custom path for the webview's data directory (localStorage, cache, etc.),
    /// **relative to the local data directory (`localDataDir()`), followed by the window label**.
    ///
    /// To set absolute paths, use [`WebviewWindowBuilder::data_directory`](https://docs.rs/tauri/2/tauri/webview/struct.WebviewWindowBuilder.html#method.data_directory)
    ///
    /// This path is not affected by the `app > appDirectoriesOverride` config.
    /// To keep the webview data in an overridden directory, leave this unset (the webview then uses the app local data directory)
    /// or resolve a path from `app.path().app_local_data_dir()` and set it with `WebviewWindowBuilder::data_directory`.
    ///
    /// #### Platform-specific:
    ///
    /// - **Windows**: WebViews with different values for settings like `additionalBrowserArgs`, `browserExtensionsEnabled` or `scrollBarStyle` must have different data directories.
    /// - **macOS / iOS**: Unsupported, use `dataStoreIdentifier` instead.
    /// - **Android**: Unsupported.
    #[serde(default, alias = "data-directory")]
    pub data_directory: Option<PathBuf>,
    /// Initialize the WebView with a custom data store identifier. This can be seen as a replacement for `dataDirectory` which is unavailable in WKWebView.
    ///
    /// See <https://developer.apple.com/documentation/webkit/wkwebsitedatastore/init(foridentifier:)?language=objc>
    ///
    /// The array must contain 16 u8 numbers.
    ///
    /// #### Platform-specific:
    ///
    /// - **iOS**: Supported since version 17.0+.
    /// - **macOS**: Supported since version 14.0+.
    /// - **Windows / Linux / Android**: Unsupported.
    #[serde(default, alias = "data-store-identifier")]
    pub data_store_identifier: Option<[u8; 16]>,

    /// Specifies the native scrollbar style to use with the webview.
    /// CSS styles that modify the scrollbar are applied on top of the native appearance configured here.
    ///
    /// Defaults to `default`, which is the browser default.
    ///
    /// ## Platform-specific
    ///
    /// - **Windows**:
    ///   - `fluentOverlay` requires WebView2 Runtime version 125.0.2535.41 or higher,
    ///     and does nothing on older versions.
    ///   - This option must be given the same value for all webviews that target the same data directory.
    /// - **Linux / Android / iOS / macOS**: Unsupported. Only supports `Default` and performs no operation.
    #[serde(default, alias = "scroll-bar-style")]
    pub scroll_bar_style: ScrollBarStyle,

    /// Whether to limit navigations to App-Bound Domains.
    ///
    /// This is required to enable Service Workers in WKWebView, which are otherwise
    /// unavailable. Defaults to `false`.
    ///
    /// When this is set to `true`, the webview can only navigate to the domains listed in the
    /// `WKAppBoundDomains` array of `src-tauri/Info.ios.plist`. Add `localhost` and every
    /// [registrable domain](https://developer.mozilla.org/en-US/docs/Glossary/Registrable_domain)
    /// this webview loads to that array:
    ///
    /// ```xml
    /// <plist>
    /// <dict>
    ///     <key>WKAppBoundDomains</key>
    ///     <array>
    ///         <string>localhost</string>
    ///         <string>aregistrabledomain.example</string>
    ///     </array>
    /// </dict>
    /// </plist>
    /// ```
    ///
    /// `localhost` must be listed if any webview with this option enabled opens a local webpage,
    /// makes any localhost call, or uses the isolation pattern, because Tauri serves the
    /// application webpage, the IPC protocol and the isolation pattern iframe from the
    /// `localhost` domain.
    ///
    /// Requests served through custom URI schemes are allowed as long as they use a registrable
    /// domain listed in the `WKAppBoundDomains` array, including requests to the `localhost`
    /// domain.
    ///
    /// An entire URI scheme can be listed by adding the protocol name followed by a colon, for
    /// example `stream:` for a custom `stream` scheme (see the
    /// [streaming example](https://github.com/tauri-apps/tauri/blob/dev/examples/streaming/main.rs)).
    /// This is not covered by Apple's
    /// [App-Bound Domains announcement](https://webkit.org/blog/10882/app-bound-domains/),
    /// so it may not be accepted during App Store review.
    ///
    /// See <https://webkit.org/blog/10882/app-bound-domains/> and
    /// <https://developer.apple.com/documentation/webkit/wkwebviewconfiguration/limitsnavigationstoappbounddomains>
    /// for the official documentation on App-Bound Domains.
    ///
    /// ## Platform-specific
    ///
    /// - **iOS**: Supported since version 14.0+.
    /// - **Linux / Windows / Android / macOS:** Unsupported.
    #[serde(default, alias = "limit-navigations-to-app-bound-domains")]
    pub limit_navigations_to_app_bound_domains: bool,
    /// The name of the Android activity to create for this window.
    #[serde(default, alias = "activity-name")]
    pub activity_name: Option<String>,
    /// The name of the Android activity that is creating this webview window.
    ///
    /// This is important to determine which stack the activity will belong to.
    #[serde(default, alias = "created-by-activity-name")]
    pub created_by_activity_name: Option<String>,

    /// Sets the identifier of the scene that is requesting the new scene,
    /// establishing a relationship between the two scenes.
    ///
    /// By default the system uses the foreground scene.
    #[serde(default, alias = "requested-by-scene-identifier")]
    pub requested_by_scene_identifier: Option<String>,
    /// Controls the WebView's browser-level general autofill behavior.
    ///
    /// **This option does not disable password or credit card autofill.**
    ///
    /// When set to `false`, the WebView will not automatically populate
    /// general form fields using previously stored data such as addresses
    /// or contact information.
    ///
    /// If not specified, this is `true` by default.
    ///
    /// ## Platform-specific
    ///
    /// - **Windows**: Supported. WebView2's autofill feature (called
    ///   "Suggestions") may not honor `autocomplete="off"` on input
    ///   elements in some cases.
    /// - **Linux / Android / iOS / macOS**: Unsupported and performs no
    ///   operation.
    #[serde(default = "default_true", alias = "general-autofill-enabled")]
    pub general_autofill_enabled: bool,

    /// Enables clipboard access for the page rendered on **Linux** and **Windows**.
    ///
    /// **macOS** doesn't provide such method and is always enabled by default,
    /// but you still need to add menu item accelerators to use shortcuts.
    #[serde(default = "default_true", alias = "enable-clipboard-access")]
    pub enable_clipboard_access: bool,
    /// Controls navigation behavior for this WebView.
    ///
    /// If omitted, no custom navigation handler is installed.
    #[serde(default)]
    pub navigation_policy: Option<NavigationPolicy>,

    /// Controls requests from `window.open`, `target="_blank"`, etc.
    #[serde(default)]
    pub new_window_policy: Option<NewWindowPolicy>,
    #[serde(default)]
    pub permission_request_policy: Option<PermissionRequestPolicy>,

    #[serde(default)]
    pub web_content_process_terminate_policy: Option<WebContentProcessTerminatePolicy>,
    #[serde(default)]
    pub bounds: Option<Rect>,
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum WebContentProcessTerminatePolicy {
    /// WebView automatisch neu laden.
    #[default]
    Reload,

    /// Nur loggen, nichts weiter machen.
    Ignore,
}

/// Background throttling policy.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub enum BackgroundThrottlingPolicy {
    /// A policy where background throttling is disabled
    Disabled,
    /// A policy where a web view that's not in a window fully suspends tasks. This is usually the default behavior in case no policy is set.
    Suspend,
    /// A policy where a web view that's not in a window limits processing, but does not fully suspend tasks.
    Throttle,
}

/// The scrollbar style to use in the webview.
///
/// ## Platform-specific
///
/// - **Windows**: This option must be given the same value for all webviews that target the same data directory.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[non_exhaustive]
pub enum ScrollBarStyle {
    #[default]
    /// The platform's native scrollbar, as rendered by the webview by default.
    ///
    /// This is the only supported value outside of Windows.
    Default,

    /// Fluent UI style overlay scrollbars. **Windows Only**
    ///
    /// Requires WebView2 Runtime version 125.0.2535.41 or higher, does nothing on older versions,
    /// see <https://learn.microsoft.com/en-us/microsoft-edge/webview2/release-notes/?tabs=dotnetcsharp#10253541>
    FluentOverlay,
}

impl Default for WebViewConfig {
    fn default() -> Self {
        Self {
            bounds: None,
            auto_resize: true,
            web_content_process_terminate_policy: None,
            enable_clipboard_access: false,
            label: "root".to_string(),
            child: false,
            url: WebviewUrl::default(),
            user_agent: None,
            drag_drop_enabled: true,
            additional_browser_args: None,
            incognito: false,
            proxy_url: None,
            zoom_hotkeys_enabled: false,
            browser_extensions_enabled: false,
            use_https_scheme: false,
            devtools: None,
            background_throttling: None,
            javascript_disabled: false,
            allow_link_preview: true,
            disable_input_accessory_view: false,
            data_directory: None,
            data_store_identifier: None,
            scroll_bar_style: ScrollBarStyle::default(),
            limit_navigations_to_app_bound_domains: false,
            activity_name: None,
            created_by_activity_name: None,
            requested_by_scene_identifier: None,
            general_autofill_enabled: true,
            navigation_policy: None,
            new_window_policy: None,
            permission_request_policy: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "type")]
pub enum NavigationPolicy {
    /// Allow every navigation.
    AllowAll,

    /// Deny every navigation.
    DenyAll,

    /// Only allow navigation to the same origin as the initial WebView URL.
    SameOrigin,

    /// Allow navigation only when one of the configured rules matches.
    AllowList { rules: Vec<NavigationRule> },

    /// Deny navigation when one of the configured rules matches.
    BlockList { rules: Vec<NavigationRule> },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NavigationRule {
    /// Optional URL scheme, e.g. "https".
    #[serde(default)]
    pub scheme: Option<String>,

    /// Hostname to match, e.g. "example.com".
    pub host: String,

    /// Optional port to match.
    #[serde(default)]
    pub port: Option<u16>,

    /// Whether subdomains should be accepted.
    ///
    /// Example:
    /// host = "example.com"
    /// include_subdomains = true
    ///
    /// Matches:
    /// - example.com
    /// - api.example.com
    /// - auth.api.example.com
    #[serde(default)]
    pub include_subdomains: bool,
}

impl NavigationPolicy {
    pub fn allows(&self, initial_url: Option<&Url>, target_url: &Url) -> bool {
        match self {
            Self::AllowAll => true,

            Self::DenyAll => false,

            Self::SameOrigin => {
                let Some(initial_url) = initial_url else {
                    return false;
                };

                same_origin(initial_url, target_url)
            }

            Self::AllowList { rules } => rules.iter().any(|rule| rule.matches(target_url)),

            Self::BlockList { rules } => !rules.iter().any(|rule| rule.matches(target_url)),
        }
    }
}

impl NavigationRule {
    pub fn matches(&self, url: &Url) -> bool {
        if let Some(expected_scheme) = &self.scheme {
            if !url.scheme().eq_ignore_ascii_case(expected_scheme) {
                return false;
            }
        }

        let Some(actual_host) = url.host_str() else {
            return false;
        };

        if !self.matches_host(actual_host) {
            return false;
        }

        if let Some(expected_port) = self.port {
            if url.port_or_known_default() != Some(expected_port) {
                return false;
            }
        }

        true
    }

    fn matches_host(&self, actual_host: &str) -> bool {
        if actual_host.eq_ignore_ascii_case(&self.host) {
            return true;
        }

        if !self.include_subdomains {
            return false;
        }

        let actual_host = actual_host.to_ascii_lowercase();
        let expected_host = self.host.to_ascii_lowercase();

        actual_host.ends_with(&format!(".{expected_host}"))
    }
}

fn same_origin(a: &Url, b: &Url) -> bool {
    a.scheme() == b.scheme() && a.host_str() == b.host_str() && a.port_or_known_default() == b.port_or_known_default()
}

// ============================================================================
// New window configuration
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "type")]
pub enum NewWindowPolicy {
    /// Let the WebView runtime handle the request normally.
    Allow,

    /// Reject every new-window request.
    Deny,

    /// Evaluate requests against configured rules.
    Rules {
        rules: Vec<NewWindowRule>,

        #[serde(default)]
        fallback: NewWindowAction,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NewWindowRule {
    /// Match condition.
    pub request: NewWindowRequestMatcher,

    /// Response returned when the request matches.
    pub response: NewWindowAction,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NewWindowRequestMatcher {
    /// Optional URL scheme, e.g. "https".
    #[serde(default)]
    pub scheme: Option<String>,

    /// Optional host, e.g. "example.com".
    #[serde(default)]
    pub host: Option<String>,

    /// Include subdomains when matching `host`.
    #[serde(default)]
    pub include_subdomains: bool,

    /// Optional port.
    #[serde(default)]
    pub port: Option<u16>,

    /// Optional path prefix.
    #[serde(default)]
    pub path_prefix: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", tag = "type")]
pub enum NewWindowAction {
    /// Allow WRY to create the requested window.
    Allow,

    /// Reject the request.
    #[default]
    Deny,

    /// Create a new engine-controlled window using the supplied configuration.
    Create { window: Box<WindowConfig> },
}

// ============================================================================
// Matching
// ============================================================================

impl NewWindowRequestMatcher {
    pub fn matches(&self, url: &Url) -> bool {
        if let Some(expected_scheme) = &self.scheme {
            if !url.scheme().eq_ignore_ascii_case(expected_scheme) {
                return false;
            }
        }

        if let Some(expected_host) = &self.host {
            let Some(actual_host) = url.host_str() else {
                return false;
            };

            if !host_matches(actual_host, expected_host, self.include_subdomains) {
                return false;
            }
        }

        if let Some(expected_port) = self.port {
            if url.port_or_known_default() != Some(expected_port) {
                return false;
            }
        }

        if let Some(path_prefix) = &self.path_prefix {
            if !url.path().starts_with(path_prefix) {
                return false;
            }
        }

        true
    }
}

fn host_matches(actual: &str, expected: &str, include_subdomains: bool) -> bool {
    if actual.eq_ignore_ascii_case(expected) {
        return true;
    }

    if !include_subdomains {
        return false;
    }

    let actual = actual.to_ascii_lowercase();
    let expected = expected.to_ascii_lowercase();

    actual.ends_with(&format!(".{expected}"))
}

// ============================================================================
// Policy evaluation
// ============================================================================

impl NewWindowPolicy {
    pub fn evaluate(&self, url: &Url) -> NewWindowAction {
        match self {
            Self::Allow => NewWindowAction::Allow,

            Self::Deny => NewWindowAction::Deny,

            Self::Rules { rules, fallback } => {
                for rule in rules {
                    if rule.request.matches(url) {
                        return rule.response.clone();
                    }
                }

                fallback.clone()
            }
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PermissionRequestPolicy {}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct InitializationScript {
    pub script: String,
    pub for_main_frame_only: bool,
}

#[derive(Debug)]
pub struct WebContext {
    pub inner: wry::WebContext,
    pub referenced_by_webviews: HashSet<String>,
    // on Linux the custom protocols are associated with the context
    // and you cannot register a URI scheme more than once
    pub registered_custom_protocols: HashSet<String>,
}

pub type WebContextStore = Arc<Mutex<HashMap<Option<PathBuf>, WebContext>>>;

/// A rectangular region in physical pixels.
#[derive(Clone, Copy, Debug, Serialize)]
pub struct PhysicalRect<P: dpi::Pixel, S: dpi::Pixel> {
    /// Rect position.
    pub position: dpi::PhysicalPosition<P>,
    /// Rect size.
    pub size: dpi::PhysicalSize<S>,
}

impl<P: dpi::Pixel, S: dpi::Pixel> Default for PhysicalRect<P, S> {
    fn default() -> Self {
        Self {
            position: (0, 0).into(),
            size: (0, 0).into(),
        }
    }
}

/// A rectangular region in logical pixels.
#[derive(Clone, Copy, Debug, Serialize)]
pub struct LogicalRect<P: dpi::Pixel, S: dpi::Pixel> {
    /// Rect position.
    pub position: dpi::LogicalPosition<P>,
    /// Rect size.
    pub size: dpi::LogicalSize<S>,
}

impl<P: dpi::Pixel, S: dpi::Pixel> Default for LogicalRect<P, S> {
    fn default() -> Self {
        Self {
            position: (0, 0).into(),
            size: (0, 0).into(),
        }
    }
}

#[derive(Debug, Clone, Copy, Hash, Eq, PartialEq, Ord, PartialOrd, serde::Serialize, serde::Deserialize)]
pub struct WindowId(u32);

impl From<u32> for WindowId {
    fn from(value: u32) -> Self {
        Self(value)
    }
}

impl WindowId {
    pub fn get(self) -> u32 {
        self.0
    }
}

/// Monitor descriptor.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Monitor {
    /// A human-readable name of the monitor.
    /// `None` if the monitor doesn't exist anymore.
    pub name: Option<String>,
    /// The monitor's resolution.
    pub size: PhysicalSize<u32>,
    /// The top-left corner position of the monitor relative to the larger full screen area.
    pub position: PhysicalPosition<i32>,
    /// The monitor's work_area.
    pub work_area: PhysicalRect<i32, u32>,
    /// Returns the scale factor that can be used to map logical pixels to physical pixels, and vice versa.
    pub scale_factor: f64,
}

impl Monitor {
    /// Returns a human-readable name of the monitor.
    /// Returns None if the monitor doesn't exist anymore.
    pub fn name(&self) -> Option<&String> {
        self.name.as_ref()
    }

    /// Returns the monitor's resolution.
    pub fn size(&self) -> &PhysicalSize<u32> {
        &self.size
    }

    /// Returns the top-left corner position of the monitor relative to the larger full screen area.
    pub fn position(&self) -> &PhysicalPosition<i32> {
        &self.position
    }

    /// Returns the monitor's work_area.
    pub fn work_area(&self) -> &PhysicalRect<i32, u32> {
        &self.work_area
    }

    /// Returns the scale factor that can be used to map logical pixels to physical pixels, and vice versa.
    pub fn scale_factor(&self) -> f64 {
        self.scale_factor
    }
}

/// Describes the appearance of the mouse cursor.
#[non_exhaustive]
#[derive(Debug, Default, Copy, Clone, PartialEq, Eq, Hash, Serialize)]
pub enum CursorIcon {
    /// The platform-dependent default cursor.
    #[default]
    Default,
    /// A simple crosshair.
    Crosshair,
    /// A hand (often used to indicate links in web browsers).
    Hand,
    /// Self explanatory.
    Arrow,
    /// Indicates something is to be moved.
    Move,
    /// Indicates text that may be selected or edited.
    Text,
    /// Program busy indicator.
    Wait,
    /// Help indicator (often rendered as a "?")
    Help,
    /// Progress indicator. Shows that processing is being done. But in contrast
    /// with "Wait" the user may still interact with the program. Often rendered
    /// as a spinning beach ball, or an arrow with a watch or hourglass.
    Progress,

    /// Cursor showing that something cannot be done.
    NotAllowed,
    ContextMenu,
    Cell,
    VerticalText,
    Alias,
    Copy,
    NoDrop,
    /// Indicates something can be grabbed.
    Grab,
    /// Indicates something is grabbed.
    Grabbing,
    AllScroll,
    ZoomIn,
    ZoomOut,

    /// Indicate that some edge is to be moved. For example, the 'SeResize' cursor
    /// is used when the movement starts from the south-east corner of the box.
    EResize,
    NResize,
    NeResize,
    NwResize,
    SResize,
    SeResize,
    SwResize,
    WResize,
    EwResize,
    NsResize,
    NeswResize,
    NwseResize,
    ColResize,
    RowResize,
}

impl<'de> Deserialize<'de> for CursorIcon {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        Ok(match s.to_lowercase().as_str() {
            "default" => CursorIcon::Default,
            "crosshair" => CursorIcon::Crosshair,
            "hand" => CursorIcon::Hand,
            "arrow" => CursorIcon::Arrow,
            "move" => CursorIcon::Move,
            "text" => CursorIcon::Text,
            "wait" => CursorIcon::Wait,
            "help" => CursorIcon::Help,
            "progress" => CursorIcon::Progress,
            "notallowed" => CursorIcon::NotAllowed,
            "contextmenu" => CursorIcon::ContextMenu,
            "cell" => CursorIcon::Cell,
            "verticaltext" => CursorIcon::VerticalText,
            "alias" => CursorIcon::Alias,
            "copy" => CursorIcon::Copy,
            "nodrop" => CursorIcon::NoDrop,
            "grab" => CursorIcon::Grab,
            "grabbing" => CursorIcon::Grabbing,
            "allscroll" => CursorIcon::AllScroll,
            "zoomin" => CursorIcon::ZoomIn,
            "zoomout" => CursorIcon::ZoomOut,
            "eresize" => CursorIcon::EResize,
            "nresize" => CursorIcon::NResize,
            "neresize" => CursorIcon::NeResize,
            "nwresize" => CursorIcon::NwResize,
            "sresize" => CursorIcon::SResize,
            "seresize" => CursorIcon::SeResize,
            "swresize" => CursorIcon::SwResize,
            "wresize" => CursorIcon::WResize,
            "ewresize" => CursorIcon::EwResize,
            "nsresize" => CursorIcon::NsResize,
            "neswresize" => CursorIcon::NeswResize,
            "nwseresize" => CursorIcon::NwseResize,
            "colresize" => CursorIcon::ColResize,
            "rowresize" => CursorIcon::RowResize,
            _ => CursorIcon::Default,
        })
    }
}

/// Window size constraints
#[derive(Clone, Copy, PartialEq, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WindowSizeConstraints {
    /// The minimum width a window can be, If this is `None`, the window will have no minimum width.
    ///
    /// The default is `None`.
    pub min_width: Option<PixelUnit>,
    /// The minimum height a window can be, If this is `None`, the window will have no minimum height.
    ///
    /// The default is `None`.
    pub min_height: Option<PixelUnit>,
    /// The maximum width a window can be, If this is `None`, the window will have no maximum width.
    ///
    /// The default is `None`.
    pub max_width: Option<PixelUnit>,
    /// The maximum height a window can be, If this is `None`, the window will have no maximum height.
    ///
    /// The default is `None`.
    pub max_height: Option<PixelUnit>,
}

/// Progress bar status.
#[derive(Debug, Clone, Copy, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ProgressBarStatus {
    /// Hide progress bar.
    None,
    /// Normal state.
    Normal,
    /// Indeterminate state. **Treated as Normal on Linux and macOS**
    Indeterminate,
    /// Paused state. **Treated as Normal on Linux**
    Paused,
    /// Error state. **Treated as Normal on Linux**
    Error,
}

/// Progress Bar State
#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProgressBarState {
    /// The progress bar status.
    pub status: Option<ProgressBarStatus>,
    /// The progress bar progress. This can be a value ranging from `0` to `100`
    pub progress: Option<u64>,
    /// The `.desktop` filename with the Unity desktop window manager, for example `myapp.desktop` **Linux Only**
    pub desktop_filename: Option<String>,
}

/// Type of user attention requested on a window.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(tag = "type")]
pub enum UserAttentionType {
    /// ## Platform-specific
    /// - **macOS:** Bounces the dock icon until the application is in focus.
    /// - **Windows:** Flashes both the window and the taskbar button until the application is in focus.
    Critical,
    /// ## Platform-specific
    /// - **macOS:** Bounces the dock icon once.
    /// - **Windows:** Flashes the taskbar button until the application is in focus.
    Informational,
}

/// Defines which device events (raw input from mice, keyboards and other HID devices that is not
/// bound to a specific window) the event loop should deliver to the application.
///
/// Listening to device events can be expensive, so the runtime filters them out by default
/// while the application has no focused window. See [`crate::Runtime::set_device_event_filter`].
///
/// ## Platform-specific
///
/// - **Linux / macOS / iOS / Android**: Unsupported, device events are always filtered out.
#[derive(Default, Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(tag = "type")]
pub enum DeviceEventFilter {
    /// Always filter out device events.
    Always,
    /// Filter out device events while the window is not focused.
    #[default]
    Unfocused,
    /// Report all device events regardless of window focus.
    Never,
}

/// Defines the orientation that a window resize will be performed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
pub enum ResizeDirection {
    East,
    North,
    NorthEast,
    NorthWest,
    South,
    SouthEast,
    SouthWest,
    West,
}

/// How the window title bar should be displayed on macOS.
#[derive(Debug, Clone, PartialEq, Eq, Copy, Default)]
#[non_exhaustive]
pub enum TitleBarStyle {
    /// A normal title bar.
    #[default]
    Visible,
    /// Makes the title bar transparent, so the window background color is shown instead.
    ///
    /// Useful if you don't need to have actual HTML under the title bar. This lets you avoid the caveats of using `TitleBarStyle::Overlay`. Will be more useful when Tauri lets you set a custom window background color.
    Transparent,
    /// Shows the title bar as a transparent overlay over the window's content.
    ///
    /// Keep in mind:
    /// - The height of the title bar is different on different OS versions, which can lead to window the controls and title not being where you don't expect.
    /// - You need to define a custom drag region to make your window draggable, however due to a limitation you can't drag the window when it's not in focus <https://github.com/tauri-apps/tauri/issues/4316>.
    /// - The color of the window title depends on the system theme.
    Overlay,
}

impl Serialize for TitleBarStyle {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.to_string().as_ref())
    }
}

impl<'de> Deserialize<'de> for TitleBarStyle {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        Ok(match s.to_lowercase().as_str() {
            "transparent" => Self::Transparent,
            "overlay" => Self::Overlay,
            _ => Self::Visible,
        })
    }
}

impl Display for TitleBarStyle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                Self::Visible => "Visible",
                Self::Transparent => "Transparent",
                Self::Overlay => "Overlay",
            }
        )
    }
}
/// Application's activation policy. Corresponds to NSApplicationActivationPolicy.
#[cfg(target_os = "macos")]
#[cfg_attr(docsrs, doc(cfg(target_os = "macos")))]
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
pub enum ActivationPolicy {
    /// Corresponds to NSApplicationActivationPolicyRegular.
    Regular,
    /// Corresponds to NSApplicationActivationPolicyAccessory.
    Accessory,
    /// Corresponds to NSApplicationActivationPolicyProhibited.
    Prohibited,
}

/// Enable prevent overflow with a margin
/// so that the window's size + this margin won't overflow the workarea
#[derive(Debug, PartialEq, Clone, Deserialize, Serialize, Default)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PreventOverflowMargin {
    /// Horizontal margin in physical pixels
    pub width: u32,
    /// Vertical margin in physical pixels
    pub height: u32,
}

/// Prevent overflow with a margin
#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum PreventOverflowConfig {
    /// Enable prevent overflow or not
    Enable(bool),
    /// Enable prevent overflow with a margin
    /// so that the window's size + this margin won't overflow the workarea
    Margin(PreventOverflowMargin),
}

#[skip_serializing_none]
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WindowConfig {
    /// The window identifier. It must be alphanumeric.
    #[serde(default = "default_window_label")]
    pub label: String,
    /// Whether Taurino should create this window at app startup or not.
    ///
    /// When this is set to `false` you must manually grab the config object via `app.config().app.windows`
    ///
    #[serde(default = "default_true")]
    pub create: bool,

    /// The horizontal position of the window's top left corner in logical pixels
    pub x: Option<f64>,
    /// The vertical position of the window's top left corner in logical pixels
    pub y: Option<f64>,

    pub webviews: Vec<WebViewConfig>,
    /// Whether or not the window starts centered or not.
    #[serde(default)]
    pub center: bool,
    /// The window width in logical pixels.
    #[serde(default = "default_width")]
    pub width: f64,
    /// The window height in logical pixels.
    #[serde(default = "default_height")]
    pub height: f64,
    /// The min window width in logical pixels.
    #[serde(alias = "min-width")]
    pub min_width: Option<f64>,
    /// The min window height in logical pixels.
    #[serde(alias = "min-height")]
    pub min_height: Option<f64>,
    /// The max window width in logical pixels.
    #[serde(alias = "max-width")]
    pub max_width: Option<f64>,
    /// The max window height in logical pixels.
    #[serde(alias = "max-height")]
    pub max_height: Option<f64>,

    /// Whether or not to prevent the window from overflowing the workarea
    ///
    /// ## Platform-specific
    ///
    /// - **iOS / Android:** Unsupported.
    #[serde(alias = "prevent-overflow")]
    pub prevent_overflow: Option<PreventOverflowConfig>,
    /// Whether the window is resizable or not. When resizable is set to false, native window's maximize button is automatically disabled.
    #[serde(default = "default_true")]
    pub resizable: bool,
    /// Whether the window's native maximize button is enabled or not.
    /// If resizable is set to false, this setting is ignored.
    ///
    /// ## Platform-specific
    ///
    /// - **macOS:** Disables the "zoom" button in the window titlebar, which is also used to enter fullscreen mode.
    /// - **Linux / iOS / Android:** Unsupported.
    #[serde(default = "default_true")]
    pub maximizable: bool,

    /// Whether the window's native minimize button is enabled or not.
    ///
    /// ## Platform-specific
    ///
    /// - **Linux / iOS / Android:** Unsupported.
    #[serde(default = "default_true")]
    pub minimizable: bool,
    /// Whether the window's native close button is enabled or not.
    ///
    /// ## Platform-specific
    ///
    /// - **Linux:** "GTK+ will do its best to convince the window manager not to show a close button.
    ///   Depending on the system, this function may not have any effect when called on a window that is already visible"
    /// - **iOS / Android:** Unsupported.
    #[serde(default = "default_true")]
    pub closable: bool,
    /// The window title.
    #[serde(default = "default_title")]
    pub title: String,
    /// Whether the window starts as fullscreen or not.
    #[serde(default)]
    pub fullscreen: bool,
    /// Whether the window will be initially focused or not.
    #[serde(default = "default_true")]
    pub focus: bool,
    /// Whether the window will be focusable or not.
    #[serde(default = "default_true")]
    pub focusable: bool,
    /// Whether the window is transparent or not.
    ///
    /// ## Platform-specific
    ///
    /// - **Windows**: Using `noRedirectionBitmap` can help avoid a white flash when creating a transparent window.
    #[serde(default)]
    pub transparent: bool,
    /// Whether the window is maximized or not.
    #[serde(default)]
    pub maximized: bool,
    /// Whether the window is visible or not.
    #[serde(default = "default_true")]
    pub visible: bool,
    /// Whether the window should have borders and bars.
    #[serde(default = "default_true")]
    pub decorations: bool,
    /// Whether the window should always be below other windows.
    #[serde(default, alias = "always-on-bottom")]
    pub always_on_bottom: bool,
    /// Whether the window should always be on top of other windows.
    #[serde(default, alias = "always-on-top")]
    pub always_on_top: bool,
    /// Whether the window should be visible on all workspaces or virtual desktops.
    ///
    /// ## Platform-specific
    ///
    /// - **Windows / iOS / Android:** Unsupported.
    #[serde(default, alias = "visible-on-all-workspaces")]
    pub visible_on_all_workspaces: bool,
    /// Prevents the window contents from being captured by other apps.
    #[serde(default, alias = "content-protected")]
    pub content_protected: bool,
    /// If `true`, hides the window icon from the taskbar on Windows and Linux.
    #[serde(default, alias = "skip-taskbar")]
    pub skip_taskbar: bool,
    /// The name of the window class created on Windows to create the window. **Windows only**.
    pub window_classname: Option<String>,
    /// This sets `WS_EX_NOREDIRECTIONBITMAP`.
    ///
    /// This can avoid the white flash that may appear before the webview content is rendered
    /// when using a transparent window. **Windows only**.
    #[serde(default, alias = "no-redirection-bitmap")]
    pub no_redirection_bitmap: bool,
    /// The initial window theme. Defaults to the system theme. Only implemented on Windows and macOS 10.14+.
    pub theme: Option<Theme>,
    /// The style of the macOS title bar.
    #[serde(default, alias = "title-bar-style")]
    pub title_bar_style: TitleBarStyle,
    /// The position of the window controls on macOS.
    ///
    /// Requires titleBarStyle: Overlay and decorations: true.
    #[serde(default, alias = "traffic-light-position")]
    pub traffic_light_position: Option<LogicalPosition<f64>>,
    /// If `true`, sets the window title to be hidden on macOS.
    #[serde(default, alias = "hidden-title")]
    pub hidden_title: bool,
    /// Whether clicking an inactive window also clicks through to the webview on macOS.
    #[serde(default, alias = "accept-first-mouse")]
    pub accept_first_mouse: bool,
    /// Defines the window [tabbing identifier] for macOS.
    ///
    /// Windows with matching tabbing identifiers will be grouped together.
    /// If the tabbing identifier is not set, automatic tabbing will be disabled.
    ///
    /// [tabbing identifier]: <https://developer.apple.com/documentation/appkit/nswindow/1644704-tabbingidentifier>
    #[serde(default, alias = "tabbing-identifier")]
    pub tabbing_identifier: Option<String>,

    /// Whether or not the window has shadow.
    ///
    /// ## Platform-specific
    ///
    /// - **Windows:**
    ///   - `false` has no effect on decorated window, shadow are always ON.
    ///   - `true` will make undecorated window have a 1px white border,
    /// and on Windows 11, it will have a rounded corners.
    /// - **Linux:** Unsupported.
    #[serde(default = "default_true")]
    pub shadow: bool,
    /// Window effects.
    ///
    /// Requires the window to be transparent.
    ///
    /// ## Platform-specific:
    ///
    /// - **Windows**: If using decorations or shadows, you may want to try this workaround <https://github.com/tauri-apps/tao/issues/72#issuecomment-975607891>
    /// - **Linux**: Unsupported
    #[serde(default, alias = "window-effects")]
    pub window_effects: Option<WindowEffectsConfig>,

    /// Sets the window associated with this label to be the parent of the window to be created.
    ///
    /// ## Platform-specific
    ///
    /// - **Windows**: This sets the passed parent as an owner window to the window to be created.
    ///   From [MSDN owned windows docs](https://docs.microsoft.com/en-us/windows/win32/winmsg/window-features#owned-windows):
    ///     - An owned window is always above its owner in the z-order.
    ///     - The system automatically destroys an owned window when its owner is destroyed.
    ///     - An owned window is hidden when its owner is minimized.
    /// - **Linux**: This makes the new window transient for parent, see <https://docs.gtk.org/gtk3/method.Window.set_transient_for.html>
    /// - **macOS**: This adds the window as a child of parent, see <https://developer.apple.com/documentation/appkit/nswindow/1419152-addchildwindow?language=objc>
    pub parent: Option<String>,

    /// Set the window and webview background color.
    ///
    /// ## Platform-specific:
    ///
    /// - **Windows**: alpha channel is ignored for the window layer.
    /// - **Windows**: On Windows 7, alpha channel is ignored for the webview layer.
    /// - **Windows**: On Windows 8 and newer, if alpha channel is not `0`, it will be ignored for the webview layer.
    #[serde(alias = "background-color")]
    pub background_color: Option<Color>,

    #[serde(default = "default_true")]
    pub enable_drag_drop: bool,
}

fn default_window_label() -> String {
    "main".to_string()
}

fn default_width() -> f64 {
    800.
}

fn default_height() -> f64 {
    600.
}

fn default_title() -> String {
    "Taurino App".to_string()
}

/// The window effects configuration object
#[skip_serializing_none]
#[derive(Debug, PartialEq, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WindowEffectsConfig {
    /// List of Window effects to apply to the Window.
    ///
    /// Generally, conflicting effects will apply the first one and ignore the rest but
    /// on macOS you can specify one Liquid Glass style and one Visual Effect material at the same time
    /// to make Tauri fallback to the latter on macOS 15 and below.
    pub effects: Vec<WindowEffect>,
    /// Window effect state **macOS Only**. Ignored for Liquid Glass Effects.
    pub state: Option<WindowEffectState>,
    /// Window effect corner radius **macOS Only**
    pub radius: Option<f64>,
    /// Window effect color.
    ///
    /// ## Platform-specific
    ///
    /// - **Windows**: Affects [`WindowEffect::Blur`] and [`WindowEffect::Acrylic`] only
    /// on Windows 10 v1903+. Doesn't have any effect on Windows 7 or Windows 11.
    /// - **macOS**: Only affects Liquid Glass effects.
    pub color: Option<Color>,
    /// Enables interactive glass behavior, which adds a visual response to user interactions.
    ///
    /// **macOS 27.0+**. Only affects Liquid Glass effects.
    #[serde(default)]
    pub interactive: bool,
}

#[allow(deprecated)]
mod window_effects {
    use super::*;

    #[derive(Debug, PartialEq, Eq, Clone, Copy, Deserialize, Serialize)]
    #[serde(rename_all = "camelCase")]
    #[non_exhaustive]
    /// Platform-specific window effects
    pub enum WindowEffect {
        /// A default material appropriate for the view's effectiveAppearance. **macOS 10.14-**
        #[deprecated(note = "You should instead choose an appropriate semantic material.")]
        AppearanceBased,
        /// **macOS 10.14-**
        #[deprecated(note = "Use a semantic material instead.")]
        Light,
        /// **macOS 10.14-**
        #[deprecated(note = "Use a semantic material instead.")]
        Dark,
        /// **macOS 10.14-**
        #[deprecated(note = "Use a semantic material instead.")]
        MediumLight,
        /// **macOS 10.14-**
        #[deprecated(note = "Use a semantic material instead.")]
        UltraDark,
        /// **macOS 10.10+**
        Titlebar,
        /// **macOS 10.10+**
        Selection,
        /// **macOS 10.11+**
        Menu,
        /// **macOS 10.11+**
        Popover,
        /// **macOS 10.11+**
        Sidebar,
        /// **macOS 10.14+**
        HeaderView,
        /// **macOS 10.14+**
        Sheet,
        /// **macOS 10.14+**
        WindowBackground,
        /// **macOS 10.14+**
        HudWindow,
        /// **macOS 10.14+**
        FullScreenUI,
        /// **macOS 10.14+**
        Tooltip,
        /// **macOS 10.14+**
        ContentBackground,
        /// **macOS 10.14+**
        UnderWindowBackground,
        /// **macOS 10.14+**
        UnderPageBackground,
        /// **macOS 26.0+**
        LiquidGlassRegular,
        /// **macOS 26.0+**
        LiquidGlassClear,
        /// Mica effect that matches the system dark preference **Windows 11 Only**
        Mica,
        /// Mica effect with dark mode but only if dark mode is enabled on the system **Windows 11 Only**
        MicaDark,
        /// Mica effect with light mode **Windows 11 Only**
        MicaLight,
        /// Tabbed effect that matches the system dark preference **Windows 11 Only**
        Tabbed,
        /// Tabbed effect with dark mode but only if dark mode is enabled on the system **Windows 11 Only**
        TabbedDark,
        /// Tabbed effect with light mode **Windows 11 Only**
        TabbedLight,
        /// **Windows 7/10/11(22H1) Only**
        ///
        /// ## Notes
        ///
        /// This effect has bad performance when resizing/dragging the window on Windows 11 build 22621.
        Blur,
        /// **Windows 10/11 Only**
        ///
        /// ## Notes
        ///
        /// This effect has bad performance when resizing/dragging the window on Windows 10 v1903+ and Windows 11 build 22000.
        Acrylic,
    }

    /// Window effect state **macOS only**
    ///
    /// <https://developer.apple.com/documentation/appkit/nsvisualeffectview/state>
    #[derive(Debug, PartialEq, Eq, Clone, Copy, Deserialize, Serialize)]
    #[serde(rename_all = "camelCase")]
    pub enum WindowEffectState {
        /// Make window effect state follow the window's active state
        FollowsWindowActiveState,
        /// Make window effect state always active
        Active,
        /// Make window effect state always inactive
        Inactive,
    }
}

impl Default for WindowConfig {
    fn default() -> Self {
        Self {
            label: default_window_label(),
            create: true,

            x: None,
            y: None,

            webviews: Vec::new(),
            enable_drag_drop: false,
            center: false,
            width: default_width(),
            height: default_height(),

            min_width: None,
            min_height: None,
            max_width: None,
            max_height: None,

            prevent_overflow: None,

            resizable: true,
            maximizable: true,
            minimizable: true,
            closable: true,

            title: default_title(),

            fullscreen: false,
            focus: true,
            focusable: true,
            transparent: false,
            maximized: false,
            visible: true,
            decorations: true,

            always_on_bottom: false,
            always_on_top: false,
            visible_on_all_workspaces: false,
            content_protected: false,
            skip_taskbar: false,

            window_classname: None,
            no_redirection_bitmap: false,

            theme: None,
            title_bar_style: TitleBarStyle::default(),
            traffic_light_position: None,
            hidden_title: false,
            accept_first_mouse: false,
            tabbing_identifier: None,

            shadow: true,
            window_effects: None,

            parent: None,
            background_color: None,
        }
    }
}
