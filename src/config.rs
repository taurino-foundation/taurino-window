// Copyright 2019-2024 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

use std::{fmt::Display, str::FromStr};

use taurino_core::{
    dpi::{PhysicalPosition, PhysicalRect, PhysicalSize, PixelUnit},
    serde::{Deserialize, Deserializer, Serialize, Serializer},
};

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(crate = "taurino_core::serde")]
pub struct InitializationScript {
    pub script: String,
    pub for_main_frame_only: bool,
}

/// Monitor descriptor.
#[derive(Debug, Clone, Serialize)]
#[serde(crate = "taurino_core::serde")]
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
#[serde(crate = "taurino_core::serde")]
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
#[serde(crate = "taurino_core::serde")]
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
#[serde(crate = "taurino_core::serde")]
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
#[serde(crate = "taurino_core::serde")]
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
#[serde(crate = "taurino_core::serde")]
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
#[serde(crate = "taurino_core::serde")]
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
#[serde(crate = "taurino_core::serde")]
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
#[serde(crate = "taurino_core::serde")]
pub enum ActivationPolicy {
    /// Corresponds to NSApplicationActivationPolicyRegular.
    Regular,
    /// Corresponds to NSApplicationActivationPolicyAccessory.
    Accessory,
    /// Corresponds to NSApplicationActivationPolicyProhibited.
    Prohibited,
}

/// A tuple struct of RGBA colors. Each value has minimum of 0 and maximum of 255.
#[derive(Debug, PartialEq, Eq, Serialize, Default, Clone, Copy)]
#[serde(crate = "taurino_core::serde")]
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
#[serde(crate = "taurino_core::serde")]
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
            InnerColor::String(string) => string.parse().map_err(taurino_core::serde::de::Error::custom)?,
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

#[cfg(windows)]
#[derive(Debug, Serialize, Deserialize)]
#[serde(crate = "taurino_core::serde")]
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
