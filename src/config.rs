use std::fs;
use std::path::PathBuf;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub trackpad_natural: bool,
    pub mouse_natural: bool,
    pub login_at_start: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            trackpad_natural: true,
            mouse_natural: false,
            login_at_start: false,
        }
    }
}

pub fn config_dir() -> Result<PathBuf> {
    let base = dirs::config_dir().context("no config dir")?;
    Ok(base.join("macuse"))
}

pub fn config_path() -> Result<PathBuf> {
    Ok(config_dir()?.join("config.json"))
}

pub fn load() -> Config {
    match config_path().and_then(|p| Ok(fs::read_to_string(p)?)) {
        Ok(s) => serde_json::from_str(&s).unwrap_or_default(),
        Err(_) => Config::default(),
    }
}

pub fn save(cfg: &Config) -> Result<()> {
    let dir = config_dir()?;
    fs::create_dir_all(&dir).context("create config dir")?;
    let path = config_path()?;
    let json = serde_json::to_string_pretty(cfg)?;
    fs::write(path, json).context("write config")?;
    Ok(())
}
