mod resources;
use resources::Resources;
mod objects;
use objects::Object;

use std::{
    sync::Arc
};

pub struct AppInfo {
    AppName: &'static str,
    DiscordPresence: u64
}

#[derive(Clone)]
#[allow(dead_code)]
pub struct Game {
    pub objects: Vec<Arc<Object>>,
    pub resources: Resources,
}