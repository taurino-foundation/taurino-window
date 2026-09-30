use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
    sync::{Arc, Mutex},
};

use taurino_core::wry::WebContext as WryWebContext;

#[derive(Debug, Clone)]
pub struct WebviewBounds {
    pub x_rate: f32,
    pub y_rate: f32,
    pub width_rate: f32,
    pub height_rate: f32,
}

#[derive(Debug)]
pub struct WebContext {
    pub inner: WryWebContext,
    pub referenced_by_webviews: HashSet<String>,
    // on Linux the custom protocols are associated with the context
    // and you cannot register a URI scheme more than once
    pub registered_custom_protocols: HashSet<String>,
}

pub type WebContextStore = Arc<Mutex<HashMap<Option<PathBuf>, WebContext>>>;
