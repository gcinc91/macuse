use std::env;
use std::fs;
use std::path::PathBuf;
use std::process::Command;

use anyhow::{Context, Result};

const LABEL: &str = "com.macuse.app";

fn plist_path() -> Result<PathBuf> {
    let home = dirs::home_dir().context("no home dir")?;
    Ok(home.join("Library/LaunchAgents").join(format!("{LABEL}.plist")))
}

fn binary_path() -> Result<PathBuf> {
    let exe = env::current_exe().context("current_exe")?;
    Ok(exe.canonicalize().unwrap_or(exe))
}

fn plist_contents(bin: &str) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>{LABEL}</string>
    <key>ProgramArguments</key>
    <array>
        <string>{bin}</string>
    </array>
    <key>RunAtLoad</key>
    <true/>
    <key>ProcessType</key>
    <string>Interactive</string>
    <key>KeepAlive</key>
    <false/>
</dict>
</plist>
"#
    )
}

pub fn is_installed() -> bool {
    plist_path().map(|p| p.exists()).unwrap_or(false)
}

pub fn install() -> Result<()> {
    let bin = binary_path()?;
    let bin_str = bin.to_string_lossy();
    let path = plist_path()?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).context("create LaunchAgents dir")?;
    }
    fs::write(&path, plist_contents(&bin_str)).context("write plist")?;
    let _ = Command::new("launchctl")
        .args(["unload", path.to_string_lossy().as_ref()])
        .status();
    Command::new("launchctl")
        .args(["load", "-w", path.to_string_lossy().as_ref()])
        .status()
        .context("launchctl load")?;
    Ok(())
}

pub fn uninstall() -> Result<()> {
    let path = plist_path()?;
    if path.exists() {
        let _ = Command::new("launchctl")
            .args(["unload", "-w", path.to_string_lossy().as_ref()])
            .status();
        fs::remove_file(&path).context("remove plist")?;
    }
    Ok(())
}
