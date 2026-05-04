use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use crate::config::Config;
use crate::system_pref;

/// Estado compartido entre la UI (escritor) y el callback del event tap (lector).
/// Solo bools -> AtomicBool. Sin locks.
pub struct ScrollState {
    pub trackpad_natural: AtomicBool,
    pub mouse_natural: AtomicBool,
    pub system_natural: AtomicBool,
}

impl ScrollState {
    pub fn from_config(cfg: &Config) -> Arc<Self> {
        Arc::new(Self {
            trackpad_natural: AtomicBool::new(cfg.trackpad_natural),
            mouse_natural: AtomicBool::new(cfg.mouse_natural),
            system_natural: AtomicBool::new(system_pref::is_natural_scrolling_enabled()),
        })
    }

    pub fn refresh_system(&self) {
        self.system_natural
            .store(system_pref::is_natural_scrolling_enabled(), Ordering::Relaxed);
    }

    pub fn snapshot(&self) -> (bool, bool, bool) {
        (
            self.trackpad_natural.load(Ordering::Relaxed),
            self.mouse_natural.load(Ordering::Relaxed),
            self.system_natural.load(Ordering::Relaxed),
        )
    }
}
