
// taurino-core/src/config_adapter.rs

use anyhow::{anyhow, bail, Result};

use crate::{
    builder::{
        CheckMenuItemBuilder,
        IconMenuItemBuilder,
        MenuItemBuilder,
    },
    image::Image,
    item::{
        Menu,
        MenuItemKind,
        PredefinedMenuItem,
        Submenu,
    },
    metadata::{AboutMetadata, NativeIcon},
    tray::{TrayIcon, TrayIconBuilder},
};

use config::menu::{
    AboutMetadataConfig,
    ImageConfig,
    MenuConfig,
    MenuItemConfig,
    PredefinedAction,
    TrayIconConfig,
};



pub fn parse_native_icon(value: String) -> Result<NativeIcon> {
    // NativeIcon already derives serde::Deserialize.
    Ok(serde_json::from_value(
        serde_json::Value::String(value),
    )?)
}

pub fn build_about(config: AboutMetadataConfig) -> Result<AboutMetadata<'static>> {
    Ok(AboutMetadata {
        name: config.name,
        version: config.version,
        short_version: config.short_version,
        authors: config.authors,
        comments: config.comments,
        copyright: config.copyright,
        license: config.license,
        website: config.website,
        website_label: config.website_label,
        credits: config.credits,
        icon: config.icon.map(load_image).transpose()?,
    })
}

pub fn build_predefined(
    action: PredefinedAction,
    metadata: Option<AboutMetadataConfig>,
) -> Result<MenuItemKind> {
    use PredefinedAction as A;

    let item = match action {
        A::Separator => PredefinedMenuItem::separator()?,
        A::Copy => PredefinedMenuItem::copy(None)?,
        A::Cut => PredefinedMenuItem::cut(None)?,
        A::Paste => PredefinedMenuItem::paste(None)?,
        A::SelectAll => PredefinedMenuItem::select_all(None)?,
        A::Undo => PredefinedMenuItem::undo(None)?,
        A::Redo => PredefinedMenuItem::redo(None)?,
        A::Minimize => PredefinedMenuItem::minimize(None)?,
        A::Maximize => PredefinedMenuItem::maximize(None)?,
        A::Fullscreen => PredefinedMenuItem::fullscreen(None)?,
        A::Hide => PredefinedMenuItem::hide(None)?,
        A::HideOthers => PredefinedMenuItem::hide_others(None)?,
        A::ShowAll => PredefinedMenuItem::show_all(None)?,
        A::CloseWindow => PredefinedMenuItem::close_window(None)?,
        A::Quit => PredefinedMenuItem::quit(None)?,
        A::Services => PredefinedMenuItem::services(None)?,
        A::BringAllToFront => {
            PredefinedMenuItem::bring_all_to_front()?
        }
        A::About => {
            let metadata = metadata
                .map(build_about)
                .transpose()?;

            PredefinedMenuItem::about(None, metadata)?
        }
    };

    Ok(MenuItemKind::Predefined(item))
}

pub fn build_menu_item(config: MenuItemConfig) -> Result<MenuItemKind> {
    match config {
        MenuItemConfig::Text {
            id,
            text,
            enabled,
            accelerator,
        } => {
            let mut builder = MenuItemBuilder::new(text)
                .enabled(enabled);

            if let Some(id) = id {
                builder = builder.id(id);
            }

            if let Some(accelerator) = accelerator {
                builder = builder.accelerator(accelerator);
            }

            Ok(MenuItemKind::MenuItem(builder.build()?))
        }

        MenuItemConfig::Check {
            id,
            text,
            enabled,
            checked,
            accelerator,
        } => {
            let mut builder = CheckMenuItemBuilder::new(text)
                .enabled(enabled)
                .checked(checked);

            if let Some(id) = id {
                builder = builder.id(id);
            }

            if let Some(accelerator) = accelerator {
                builder = builder.accelerator(accelerator);
            }

            Ok(MenuItemKind::Check(builder.build()?))
        }

        MenuItemConfig::Icon {
            id,
            text,
            enabled,
            icon,
            native_icon,
            accelerator,
        } => {
            if icon.is_some() && native_icon.is_some() {
                bail!("Icon item cannot specify both icon types");
            }

            let mut builder = IconMenuItemBuilder::new(text)
                .enabled(enabled);

            if let Some(id) = id {
                builder = builder.id(id);
            }

            if let Some(accelerator) = accelerator {
                builder = builder.accelerator(accelerator);
            }

            if let Some(icon) = icon {
                builder = builder.icon(load_image(icon)?);
            } else if let Some(native_icon) = native_icon {
                builder = builder.native_icon(
                    parse_native_icon(native_icon)?
                );
            }

            Ok(MenuItemKind::Icon(builder.build()?))
        }

        MenuItemConfig::Predefined { action, metadata } => {
            build_predefined(action, metadata)
        }

        MenuItemConfig::Submenu {
            id,
            text,
            enabled,
            icon,
            native_icon,
            items,
        } => {
            if icon.is_some() && native_icon.is_some() {
                bail!("Submenu cannot specify both icon types");
            }

            let submenu = if let Some(id) = id {
                Submenu::with_id(id, text, enabled)?
            } else {
                Submenu::new(text, enabled)?
            };

            if let Some(icon) = icon {
                submenu.set_icon(Some(load_image(icon)?))?;
            } else if let Some(native_icon) = native_icon {
                submenu.set_native_icon(Some(
                    parse_native_icon(native_icon)?
                ))?;
            }

            for item in items {
                submenu.append(&build_menu_item(item)?)?;
            }

            Ok(MenuItemKind::Submenu(submenu))
        }
    }
}

pub fn build_menu(config: MenuConfig) -> Result<Menu> {
    let menu = match config.id {
        Some(id) => Menu::with_id(id)?,
        None => Menu::new()?,
    };

    for item in config.items {
        menu.append(&build_menu_item(item)?)?;
    }

    Ok(menu)
}

pub fn build_tray_icon(config: TrayIconConfig) -> Result<TrayIcon> {
    if config.id.trim().is_empty() {
        bail!("Tray icon ID cannot be empty");
    }

    let mut builder = TrayIconBuilder::with_id(config.id);

    if let Some(menu) = config.menu {
        let menu = build_menu(menu)?;
        builder = builder.menu(&menu);
    }

    if let Some(icon) = config.icon {
        builder = builder.icon(load_image(icon)?);
    }

    if let Some(tooltip) = config.tooltip {
        builder = builder.tooltip(tooltip);
    }

    if let Some(title) = config.title {
        builder = builder.title(title);
    }

    if let Some(path) = config.temp_dir_path {
        builder = builder.temp_dir_path(path);
    }

    builder = builder
        .icon_as_template(config.icon_as_template)
        .show_menu_on_left_click(config.show_menu_on_left_click);

    let tray = builder.build()?;
    tray.set_visible(config.visible)?;

    Ok(tray)
}

/* 

# taurino/config/menu.py

from __future__ import annotations

from enum import StrEnum
from pathlib import Path
from typing import Annotated, Literal, Self, TypeAlias

from pydantic import BaseModel, ConfigDict, Field, model_validator


class Config(BaseModel):
    model_config = ConfigDict(
        frozen=True,
        extra="forbid",
        validate_default=True,
    )


class NativeIcon(StrEnum):
    ADD = "Add"
    ADVANCED = "Advanced"
    BLUETOOTH = "Bluetooth"
    BOOKMARKS = "Bookmarks"
    CAUTION = "Caution"
    COLOR_PANEL = "ColorPanel"
    COLUMN_VIEW = "ColumnView"
    COMPUTER = "Computer"
    ENTER_FULL_SCREEN = "EnterFullScreen"
    EVERYONE = "Everyone"
    EXIT_FULL_SCREEN = "ExitFullScreen"
    FLOW_VIEW = "FlowView"
    FOLDER = "Folder"
    FOLDER_BURNABLE = "FolderBurnable"
    FOLDER_SMART = "FolderSmart"
    FOLLOW_LINK_FREESTANDING = "FollowLinkFreestanding"
    FONT_PANEL = "FontPanel"
    GO_LEFT = "GoLeft"
    GO_RIGHT = "GoRight"
    HOME = "Home"
    ICHAT_THEATER = "IChatTheater"
    ICON_VIEW = "IconView"
    INFO = "Info"
    INVALID_DATA_FREESTANDING = "InvalidDataFreestanding"
    LEFT_FACING_TRIANGLE = "LeftFacingTriangle"
    LIST_VIEW = "ListView"
    LOCK_LOCKED = "LockLocked"
    LOCK_UNLOCKED = "LockUnlocked"
    MENU_MIXED_STATE = "MenuMixedState"
    MENU_ON_STATE = "MenuOnState"
    MOBILE_ME = "MobileMe"
    MULTIPLE_DOCUMENTS = "MultipleDocuments"
    NETWORK = "Network"
    PATH = "Path"
    PREFERENCES_GENERAL = "PreferencesGeneral"
    QUICK_LOOK = "QuickLook"
    REFRESH_FREESTANDING = "RefreshFreestanding"
    REFRESH = "Refresh"
    REMOVE = "Remove"
    REVEAL_FREESTANDING = "RevealFreestanding"
    RIGHT_FACING_TRIANGLE = "RightFacingTriangle"
    SHARE = "Share"
    SLIDESHOW = "Slideshow"
    SMART_BADGE = "SmartBadge"
    STATUS_AVAILABLE = "StatusAvailable"
    STATUS_NONE = "StatusNone"
    STATUS_PARTIALLY_AVAILABLE = "StatusPartiallyAvailable"
    STATUS_UNAVAILABLE = "StatusUnavailable"
    STOP_PROGRESS_FREESTANDING = "StopProgressFreestanding"
    STOP_PROGRESS = "StopProgress"
    TRASH_EMPTY = "TrashEmpty"
    TRASH_FULL = "TrashFull"
    USER = "User"


class PredefinedAction(StrEnum):
    SEPARATOR = "separator"
    COPY = "copy"
    CUT = "cut"
    PASTE = "paste"
    SELECT_ALL = "select_all"
    UNDO = "undo"
    REDO = "redo"
    MINIMIZE = "minimize"
    MAXIMIZE = "maximize"
    FULLSCREEN = "fullscreen"
    HIDE = "hide"
    HIDE_OTHERS = "hide_others"
    SHOW_ALL = "show_all"
    CLOSE_WINDOW = "close_window"
    QUIT = "quit"
    SERVICES = "services"
    BRING_ALL_TO_FRONT = "bring_all_to_front"
    ABOUT = "about"


class ImageConfig(Config):
    kind: Literal["path", "bytes", "rgba"]
    path: str | None = None
    data: bytes | None = None
    rgba: bytes | None = None
    width: int | None = Field(default=None, ge=1, le=4294967295)
    height: int | None = Field(default=None, ge=1, le=4294967295)

    @model_validator(mode="after")
    def validate_source(self) -> Self:
        if self.kind == "path":
            if not self.path or self.data is not None or self.rgba is not None:
                raise ValueError("Path image requires only path")
        elif self.kind == "bytes":
            if self.data is None or self.path is not None or self.rgba is not None:
                raise ValueError("Bytes image requires only data")
        elif self.kind == "rgba":
            if self.rgba is None or self.width is None or self.height is None:
                raise ValueError("RGBA image requires rgba, width and height")
            if len(self.rgba) != self.width * self.height * 4:
                raise ValueError("Invalid RGBA buffer length")
        return self


ImageInput: TypeAlias = ImageConfig | str | Path | bytes


def image_config(value: ImageInput) -> ImageConfig:
    if isinstance(value, ImageConfig):
        return value
    if isinstance(value, (str, Path)):
        return ImageConfig(kind="path", path=str(value))
    if isinstance(value, bytes):
        return ImageConfig(kind="bytes", data=value)
    raise TypeError(f"Unsupported image type: {type(value)!r}")


class AboutMetadata(Config):
    name: str | None = None
    version: str | None = None
    short_version: str | None = None
    authors: tuple[str, ...] | None = None
    comments: str | None = None
    copyright: str | None = None
    license: str | None = None
    website: str | None = None
    website_label: str | None = None
    credits: str | None = None
    icon: ImageConfig | None = None


class TextItemConfig(Config):
    kind: Literal["text"] = "text"
    id: str | None = None
    text: str = Field(min_length=1)
    enabled: bool = True
    accelerator: str | None = None


class CheckItemConfig(Config):
    kind: Literal["check"] = "check"
    id: str | None = None
    text: str = Field(min_length=1)
    enabled: bool = True
    checked: bool = True
    accelerator: str | None = None


class IconItemConfig(Config):
    kind: Literal["icon"] = "icon"
    id: str | None = None
    text: str = Field(min_length=1)
    enabled: bool = True
    icon: ImageConfig | None = None
    native_icon: NativeIcon | None = None
    accelerator: str | None = None

    @model_validator(mode="after")
    def validate_icon(self) -> Self:
        if self.icon is not None and self.native_icon is not None:
            raise ValueError("Use icon or native_icon, not both")
        return self


class PredefinedItemConfig(Config):
    kind: Literal["predefined"] = "predefined"
    action: PredefinedAction
    metadata: AboutMetadata | None = None


class SubmenuConfig(Config):
    kind: Literal["submenu"] = "submenu"
    id: str | None = None
    text: str = Field(min_length=1)
    enabled: bool = True
    icon: ImageConfig | None = None
    native_icon: NativeIcon | None = None
    items: tuple[MenuItemConfig, ...] = ()

    @model_validator(mode="after")
    def validate_icon(self) -> Self:
        if self.icon is not None and self.native_icon is not None:
            raise ValueError("Use icon or native_icon, not both")
        return self


MenuItemConfig: TypeAlias = Annotated[
    TextItemConfig
    | CheckItemConfig
    | IconItemConfig
    | PredefinedItemConfig
    | SubmenuConfig,
    Field(discriminator="kind"),
]

SubmenuConfig.model_rebuild()


class MenuConfig(Config):
    id: str | None = None
    items: tuple[MenuItemConfig, ...] = ()


class TrayIconConfig(Config):
    id: str = Field(min_length=1)
    menu: MenuConfig | None = None
    icon: ImageConfig | None = None
    tooltip: str | None = None
    title: str | None = None
    temp_dir_path: str | None = None
    icon_as_template: bool = False
    show_menu_on_left_click: bool = True
    visible: bool = True


class FluentBuilder:
    def _update(self, **changes) -> Self:
        self._config = self._config.model_copy(update=changes)
        # Revalidate because Pydantic model_copy(update=...) does not validate.
        self._config = type(self._config).model_validate(
            self._config.model_dump(mode="python", round_trip=True)
        )
        return self

    def build(self):
        return self._config.model_copy(deep=True)


class MenuItemBuilder(FluentBuilder):
    def __init__(self, text: str, id: str | None = None):
        self._config = TextItemConfig(id=id, text=text)

    @classmethod
    def with_id(cls, id: str, text: str) -> Self:
        return cls(text, id)

    def id(self, value: str) -> Self:
        return self._update(id=value)

    def enabled(self, value: bool) -> Self:
        return self._update(enabled=value)

    def accelerator(self, value: str | None) -> Self:
        return self._update(accelerator=value)


class CheckMenuItemBuilder(FluentBuilder):
    def __init__(self, text: str, id: str | None = None):
        self._config = CheckItemConfig(id=id, text=text)

    @classmethod
    def with_id(cls, id: str, text: str) -> Self:
        return cls(text, id)

    def id(self, value: str) -> Self:
        return self._update(id=value)

    def enabled(self, value: bool) -> Self:
        return self._update(enabled=value)

    def checked(self, value: bool) -> Self:
        return self._update(checked=value)

    def accelerator(self, value: str | None) -> Self:
        return self._update(accelerator=value)


class IconMenuItemBuilder(FluentBuilder):
    def __init__(self, text: str, id: str | None = None):
        self._config = IconItemConfig(id=id, text=text)

    @classmethod
    def with_id(cls, id: str, text: str) -> Self:
        return cls(text, id)

    def id(self, value: str) -> Self:
        return self._update(id=value)

    def enabled(self, value: bool) -> Self:
        return self._update(enabled=value)

    def accelerator(self, value: str | None) -> Self:
        return self._update(accelerator=value)

    def icon(self, value: ImageInput) -> Self:
        return self._update(
            icon=image_config(value),
            native_icon=None,
        )

    def native_icon(self, value: NativeIcon) -> Self:
        return self._update(
            icon=None,
            native_icon=value,
        )


class MenuContainerBuilder(FluentBuilder):
    def _append(self, item: MenuItemConfig) -> Self:
        return self._update(items=(*self._config.items, item))

    def item(self, item: MenuItemConfig | FluentBuilder) -> Self:
        if isinstance(item, FluentBuilder):
            item = item.build()
        return self._append(item)

    def items(self, *items: MenuItemConfig | FluentBuilder) -> Self:
        for item in items:
            self.item(item)
        return self

    def text(self, id: str, text: str) -> Self:
        return self._append(TextItemConfig(id=id, text=text))

    def text_item(self, id: str, text: str) -> Self:
        return self.text(id, text)

    def check(self, id: str, text: str, checked: bool = True) -> Self:
        return self._append(
            CheckItemConfig(id=id, text=text, checked=checked)
        )

    def icon(self, id: str, text: str, icon: ImageInput) -> Self:
        return self._append(
            IconItemConfig(id=id, text=text, icon=image_config(icon))
        )

    def native_icon(self, id: str, text: str, icon: NativeIcon) -> Self:
        return self._append(
            IconItemConfig(id=id, text=text, native_icon=icon)
        )

    def submenu(self, submenu: SubmenuBuilder | SubmenuConfig) -> Self:
        if isinstance(submenu, SubmenuBuilder):
            submenu = submenu.build()
        return self._append(submenu)

    def predefined(
        self,
        action: PredefinedAction,
        metadata: AboutMetadata | None = None,
    ) -> Self:
        return self._append(
            PredefinedItemConfig(action=action, metadata=metadata)
        )

    def separator(self) -> Self:
        return self.predefined(PredefinedAction.SEPARATOR)

    def about(self, metadata: AboutMetadata | None = None) -> Self:
        return self.predefined(PredefinedAction.ABOUT, metadata)


# Generate named fluent methods for predefined operations.
def _predefined_method(action: PredefinedAction):
    def method(self: MenuContainerBuilder) -> Self:
        return self.predefined(action)
    method.__name__ = action.value
    return method


for _action in PredefinedAction:
    if _action not in (PredefinedAction.SEPARATOR, PredefinedAction.ABOUT):
        setattr(MenuContainerBuilder, _action.value, _predefined_method(_action))


class MenuBuilder(MenuContainerBuilder):
    def __init__(self, id: str | None = None):
        self._config = MenuConfig(id=id)

    @classmethod
    def with_id(cls, id: str) -> Self:
        return cls(id)

    def id(self, value: str) -> Self:
        return self._update(id=value)


class SubmenuBuilder(MenuContainerBuilder):
    def __init__(self, text: str, id: str | None = None):
        self._config = SubmenuConfig(id=id, text=text)

    @classmethod
    def with_id(cls, id: str, text: str) -> Self:
        return cls(text, id)

    def id(self, value: str) -> Self:
        return self._update(id=value)

    def enabled(self, value: bool) -> Self:
        return self._update(enabled=value)

    def submenu_icon(self, icon: ImageInput) -> Self:
        return self._update(
            icon=image_config(icon),
            native_icon=None,
        )

    def submenu_native_icon(self, icon: NativeIcon) -> Self:
        return self._update(
            icon=None,
            native_icon=icon,
        )


class TrayIconBuilder(FluentBuilder):
    def __init__(self, id: str):
        self._config = TrayIconConfig(id=id)

    @classmethod
    def with_id(cls, id: str) -> Self:
        return cls(id)

    def menu(self, value: MenuBuilder | MenuConfig | None) -> Self:
        if isinstance(value, MenuBuilder):
            value = value.build()
        return self._update(menu=value)

    def icon(self, value: ImageInput | None) -> Self:
        return self._update(
            icon=image_config(value) if value is not None else None
        )

    def tooltip(self, value: str | None) -> Self:
        return self._update(tooltip=value)

    def title(self, value: str | None) -> Self:
        return self._update(title=value)

    def temp_dir_path(self, value: str | Path | None) -> Self:
        return self._update(
            temp_dir_path=str(value) if value is not None else None
        )

    def icon_as_template(self, value: bool) -> Self:
        return self._update(icon_as_template=value)

    def show_menu_on_left_click(self, value: bool) -> Self:
        return self._update(show_menu_on_left_click=value)

    def visible(self, value: bool) -> Self:
        return self._update(visible=value)

    @property
    def id(self) -> str:
        return self._config.id


# Example
if __name__ == "__main__":
    tray = (
        TrayIconBuilder("main")
        .icon("assets/tray.png")
        .tooltip("Taurino Application")
        .title("Taurino")
        .menu(
            MenuBuilder("main-menu")
            .text("open", "Open")
            .item(
                CheckMenuItemBuilder("Notifications", "notifications")
                .checked(True)
                .enabled(True)
            )
            .separator()
            .submenu(
                SubmenuBuilder("Settings", "settings")
                .text_item("preferences", "Preferences")
                .check("autostart", "Start automatically", False)
                .separator()
                .about(AboutMetadata(name="Taurino", version="0.1.0"))
            )
            .text("quit", "Quit")
        )
        .show_menu_on_left_click(True)
        .build()
    )

    # JSON-compatible representation; bytes encoded as base64 by Pydantic.
    payload = tray.model_dump(mode="json")
    print(payload)




*/