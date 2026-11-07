//! Persistent user configuration for RetroAmp (~/.config/retroamp/config.json).

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AppConfig {
    /// UI zoom: 1.0 = normal, 1.5, 2.0 (double size), up to 3.0.
    pub ui_scale: f32,
    /// Active theme: "classic" | "hifi" | "cyberpunk".
    pub theme: String,
    /// Visible playlist track rows (drives playlist height).
    pub playlist_rows: f32,
    /// Playback volume 0.0..=1.0.
    pub volume: f32,
    pub shuffle: bool,
    pub repeat: bool,
    pub show_eq: bool,
    pub show_playlist: bool,
    /// Visualizer mode: true = spectrum, false = oscilloscope.
    pub vis_spectrum: bool,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            ui_scale: 1.0,
            theme: "redrawn".to_string(),
            playlist_rows: 4.0,
            volume: 0.8,
            shuffle: false,
            repeat: false,
            show_eq: true,
            show_playlist: true,
            vis_spectrum: true,
        }
    }
}

pub fn config_path() -> PathBuf {
    let base = std::env::var("XDG_CONFIG_HOME")
        .ok()
        .filter(|s| !s.is_empty())
        .or_else(|| std::env::var("HOME").ok().map(|h| format!("{h}/.config")))
        .unwrap_or_else(|| "/tmp".to_string());
    PathBuf::from(base).join("retroamp").join("config.json")
}

impl AppConfig {
    pub fn load() -> Self {
        let path = config_path();
        match std::fs::read_to_string(&path) {
            Ok(text) => match serde_json::from_str(&text) {
                Ok(cfg) => {
                    log::info!("Loaded config from {}", path.display());
                    cfg
                }
                Err(e) => {
                    log::warn!("Config parse error ({}), using defaults", e);
                    Self::default()
                }
            },
            Err(_) => Self::default(),
        }
    }

    pub fn save(&self) {
        let path = config_path();
        if let Some(dir) = path.parent() {
            if let Err(e) = std::fs::create_dir_all(dir) {
                log::warn!("Could not create config dir {}: {e}", dir.display());
                return;
            }
        }
        match serde_json::to_string_pretty(self) {
            Ok(text) => {
                if let Err(e) = std::fs::write(&path, text) {
                    log::warn!("Could not save config {}: {e}", path.display());
                }
            }
            Err(e) => log::warn!("Config serialize error: {e}"),
        }
    }
}
