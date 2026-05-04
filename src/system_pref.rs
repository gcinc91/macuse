use std::process::Command;

/// Lee `com.apple.swipescrolldirection` del NSGlobalDomain.
/// `true` = natural scrolling activado (default macOS).
pub fn is_natural_scrolling_enabled() -> bool {
    let out = Command::new("defaults")
        .args(["read", "-g", "com.apple.swipescrolldirection"])
        .output();

    match out {
        Ok(o) if o.status.success() => {
            let s = String::from_utf8_lossy(&o.stdout);
            let s = s.trim();
            // `defaults` devuelve "1" / "0" / "true" / "false"
            !matches!(s, "0" | "false")
        }
        _ => true, // default macOS = natural ON
    }
}
