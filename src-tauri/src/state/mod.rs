pub mod preferences;

use std::sync::{
    atomic::{AtomicBool, AtomicU64},
    Mutex,
};
use std::time::Instant;

use preferences::Preferences;

pub struct AppState {
    pub preferences: Mutex<Preferences>,
    pub preferences_update: Mutex<()>,
    pub shortcut_registered: AtomicBool,
    pub quitting: AtomicBool,
    pub started_at: Instant,
    pub startup_millis: AtomicU64,
}

impl AppState {
    pub fn new(preferences: Preferences, started_at: Instant) -> Self {
        Self {
            preferences: Mutex::new(preferences),
            preferences_update: Mutex::new(()),
            shortcut_registered: AtomicBool::new(false),
            quitting: AtomicBool::new(false),
            started_at,
            startup_millis: AtomicU64::new(0),
        }
    }
}
