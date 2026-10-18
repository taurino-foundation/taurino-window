use std::{
    collections::{HashMap, HashSet},
    sync::{Arc, Mutex},
};

use anyhow::{Context, Result, anyhow, bail};
use global_hotkey::{
    GlobalHotKeyEvent, GlobalHotKeyManager, HotKeyState,
    hotkey::HotKey,
};
use tao::window::WindowId;

use crate::config::{
    AppConfig, ShortcutConfig, ShortcutStateFilter,
};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ShortcutStateFilter {
    Pressed,
    Released,
    Both,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShortcutConfig {
    pub shortcut: String,
    pub event: String,

    #[serde(default = "default_state")]
    pub state: ShortcutStateFilter,
}

fn default_state() -> ShortcutStateFilter {
    ShortcutStateFilter::Pressed
}






#[derive(Debug, Clone)]
pub enum ShortcutScope {
    App,
    Window {
        label: String,
        window_id: WindowId,
    },
}

#[derive(Debug, Clone)]
pub struct ShortcutMessage {
    pub event: String,
    pub shortcut: String,
    pub state: HotKeyState,
    pub scope: ShortcutScope,
}

#[derive(Clone)]
struct Entry {
    config: ShortcutConfig,
    hotkey: HotKey,
}

struct WindowRegistry {
    label: String,
    entries: HashMap<u32, Entry>,
}

struct Registry {
    // First logical registration system.
    app: HashMap<u32, Entry>,

    // Second logical registration system.
    windows: HashMap<WindowId, WindowRegistry>,

    // Configured shortcuts waiting for Tao window creation.
    pending: HashMap<String, Vec<ShortcutConfig>>,

    // Native OS registrations.
    native: HashMap<u32, HotKey>,

    focused: Option<WindowId>,
}

impl Registry {
    fn new() -> Self {
        Self {
            app: HashMap::new(),
            windows: HashMap::new(),
            pending: HashMap::new(),
            native: HashMap::new(),
            focused: None,
        }
    }
}

type EventSender =
    Arc<dyn Fn(ShortcutMessage) + Send + Sync + 'static>;

pub struct ShortcutManager {
    native: GlobalHotKeyManager,
    registry: Arc<Mutex<Registry>>,
    sender: EventSender,
}

impl ShortcutManager {
    pub fn new<F>(sender: F) -> Result<Self>
    where
        F: Fn(ShortcutMessage) + Send + Sync + 'static,
    {
        // Construct on the GUI event-loop thread.
        let native = GlobalHotKeyManager::new()?;
        let registry = Arc::new(Mutex::new(Registry::new()));
        let sender: EventSender = Arc::new(sender);

        let manager = Self {
            native,
            registry: Arc::clone(&registry),
            sender: Arc::clone(&sender),
        };

        // Install once for the application/process.
        GlobalHotKeyEvent::set_event_handler(Some(
            move |event| {
                Self::dispatch(&registry, &sender, event);
            },
        ));

        Ok(manager)
    }

    // Parse Python configuration and prepare both scopes.
    pub fn configure(&self, config: &AppConfig) -> Result<()> {
        for shortcut in &config.shortcuts {
            self.register_app(shortcut.clone())?;
        }

        let mut labels = HashSet::new();

        for window in &config.windows {
            if !labels.insert(&window.label) {
                bail!("Duplicate window label: {}", window.label);
            }

            let mut registry = self.registry.lock().unwrap();

            if registry.pending.contains_key(&window.label)
                || registry.windows.values().any(|w| w.label == window.label)
            {
                bail!("Window already configured: {}", window.label);
            }

            registry.pending.insert(
                window.label.clone(),
                window.shortcuts.clone(),
            );
        }

        Ok(())
    }

    fn ensure_native(
        &self,
        registry: &mut Registry,
        hotkey: HotKey,
    ) -> Result<()> {
        let id = hotkey.id();

        if !registry.native.contains_key(&id) {
            self.native.register(hotkey)
                .context("Failed to register OS hotkey")?;

            registry.native.insert(id, hotkey);
        }

        Ok(())
    }

    pub fn register_app(&self, config: ShortcutConfig) -> Result<()> {
        let hotkey: HotKey = config.shortcut.parse()
            .map_err(|e| anyhow!("Invalid shortcut: {e}"))?;

        let mut registry = self.registry.lock().unwrap();
        let id = hotkey.id();

        if registry.app.contains_key(&id) {
            bail!("App shortcut already exists: {}", config.shortcut);
        }

        self.ensure_native(&mut registry, hotkey)?;

        registry.app.insert(
            id,
            Entry { config, hotkey },
        );

        Ok(())
    }

    // Call after the Tao Window has been created.
    pub fn attach_window(
        &self,
        label: &str,
        window_id: WindowId,
    ) -> Result<()> {
        let mut registry = self.registry.lock().unwrap();

        if registry.windows.contains_key(&window_id) {
            bail!("WindowId already attached");
        }

        if registry.windows.values().any(|w| w.label == label) {
            bail!("Window label already attached: {label}");
        }

        // Keep pending configuration until setup succeeds.
        let configs = registry.pending.get(label)
            .cloned()
            .ok_or_else(|| anyhow!("Unknown window: {label}"))?;

        let mut entries = HashMap::new();
        let mut newly_registered = Vec::new();

        for config in configs {
            let hotkey: HotKey = match config.shortcut.parse() {
                Ok(hotkey) => hotkey,
                Err(error) => {
                    self.rollback(&mut registry, &newly_registered);
                    return Err(anyhow!("Invalid shortcut: {error}"));
                }
            };

            let id = hotkey.id();

            if entries.contains_key(&id) {
                self.rollback(&mut registry, &newly_registered);
                bail!("Duplicate shortcut in window {label}");
            }

            if !registry.native.contains_key(&id) {
                if let Err(error) = self.native.register(hotkey) {
                    self.rollback(&mut registry, &newly_registered);
                    return Err(error.into());
                }

                registry.native.insert(id, hotkey);
                newly_registered.push(hotkey);
            }

            entries.insert(id, Entry { config, hotkey });
        }

        registry.windows.insert(
            window_id,
            WindowRegistry {
                label: label.to_owned(),
                entries,
            },
        );

        registry.pending.remove(label);

        Ok(())
    }

    fn rollback(&self, registry: &mut Registry, hotkeys: &[HotKey]) {
        for hotkey in hotkeys {
            if self.native.unregister(*hotkey).is_ok() {
                registry.native.remove(&hotkey.id());
            }
        }
    }

    pub fn set_window_focus(
        &self,
        window_id: WindowId,
        focused: bool,
    ) {
        let mut registry = self.registry.lock().unwrap();

        if focused && registry.windows.contains_key(&window_id) {
            registry.focused = Some(window_id);
        } else if !focused && registry.focused == Some(window_id) {
            registry.focused = None;
        }
    }

    pub fn remove_window(&self, window_id: WindowId) -> Result<()> {
        let mut registry = self.registry.lock().unwrap();

        if registry.focused == Some(window_id) {
            registry.focused = None;
        }

        registry.windows.remove(&window_id);

        self.cleanup_native(&mut registry)
    }

    fn cleanup_native(&self, registry: &mut Registry) -> Result<()> {
        let mut used = HashSet::new();

        used.extend(registry.app.keys().copied());

        for window in registry.windows.values() {
            used.extend(window.entries.keys().copied());
        }

        let unused: Vec<HotKey> = registry.native
            .iter()
            .filter(|(id, _)| !used.contains(id))
            .map(|(_, hotkey)| *hotkey)
            .collect();

        for hotkey in unused {
            self.native.unregister(hotkey)?;
            registry.native.remove(&hotkey.id());
        }

        Ok(())
    }

    fn matches_state(
        filter: ShortcutStateFilter,
        state: HotKeyState,
    ) -> bool {
        match filter {
            ShortcutStateFilter::Pressed => state == HotKeyState::Pressed,
            ShortcutStateFilter::Released => state == HotKeyState::Released,
            ShortcutStateFilter::Both => true,
        }
    }

    fn dispatch(
        registry: &Arc<Mutex<Registry>>,
        sender: &EventSender,
        event: GlobalHotKeyEvent,
    ) {
        let mut messages = Vec::new();

        {
            let Ok(registry) = registry.lock() else {
                return;
            };

            // 1. Application shortcuts.
            if let Some(entry) = registry.app.get(&event.id) {
                if Self::matches_state(entry.config.state, event.state) {
                    messages.push(ShortcutMessage {
                        event: entry.config.event.clone(),
                        shortcut: entry.config.shortcut.clone(),
                        state: event.state,
                        scope: ShortcutScope::App,
                    });
                }
            }

            // 2. Focused window shortcuts.
            if let Some(window_id) = registry.focused {
                if let Some(window) = registry.windows.get(&window_id) {
                    if let Some(entry) = window.entries.get(&event.id) {
                        if Self::matches_state(entry.config.state, event.state) {
                            messages.push(ShortcutMessage {
                                event: entry.config.event.clone(),
                                shortcut: entry.config.shortcut.clone(),
                                state: event.state,
                                scope: ShortcutScope::Window {
                                    label: window.label.clone(),
                                    window_id,
                                },
                            });
                        }
                    }
                }
            }
        }

        // No mutex held while forwarding events.
        for message in messages {
            sender(message);
        }
    }

    pub fn is_app_registered(&self, shortcut: &str) -> bool {
        let Ok(hotkey) = shortcut.parse::<HotKey>() else {
            return false;
        };

        self.registry.lock()
            .map(|r| r.app.contains_key(&hotkey.id()))
            .unwrap_or(false)
    }

    pub fn is_window_registered(
        &self,
        window_id: WindowId,
        shortcut: &str,
    ) -> bool {
        let Ok(hotkey) = shortcut.parse::<HotKey>() else {
            return false;
        };

        self.registry.lock()
            .map(|r| {
                r.windows.get(&window_id)
                    .is_some_and(|w| w.entries.contains_key(&hotkey.id()))
            })
            .unwrap_or(false)
    }
}
