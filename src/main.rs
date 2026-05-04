//! macuse - natural scrolling independiente para trackpad y raton.

mod config;
mod log;
mod login_item;
mod permissions;
mod scroll;
mod system_pref;
mod ui;

use cocoa::appkit::{NSApp, NSApplication};

use crate::scroll::{tap, ScrollState};

fn main() {
    mlog!("--- macuse start, exe={:?}", std::env::current_exe().ok());

    let cfg = config::load();
    mlog!("config cargada: trackpad={} mouse={} login={}",
        cfg.trackpad_natural, cfg.mouse_natural, cfg.login_at_start);
    let state = ScrollState::from_config(&cfg);
    mlog!("system natural scrolling = {}",
        crate::system_pref::is_natural_scrolling_enabled());

    let trusted_initial = permissions::is_trusted();
    mlog!("AX trusted (initial) = {}", trusted_initial);
    let trusted_now = if trusted_initial {
        true
    } else {
        permissions::prompt_trust();
        let t = permissions::is_trusted();
        mlog!("AX trusted (after prompt) = {}", t);
        t
    };

    let started_tap = if trusted_now {
        match tap::start(state.clone()) {
            Ok(t) => {
                mlog!("event tap iniciado OK");
                Some(t)
            }
            Err(e) => {
                mlog!("event tap fallo: {e}");
                None
            }
        }
    } else {
        mlog!("sin permiso AX, tap no iniciado (banner visible)");
        None
    };

    unsafe {
        let app = NSApp();
        let _delegate = ui::build(state, started_tap);
        mlog!("UI lista, entrando en run loop");
        app.run();
    }
}
