//! Persistent user configuration for RetroAmp (~/.config/retroamp/config.json).

use serde::{Deserialize, Serialize};
use std::fs::OpenOptions;
use std::io::Write;
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

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
    /// Active skin: a bundled name ("nord_frost", "cyberpunk", "hifi_studio")
    /// or the path of an imported .wsz. None = embedded default skin.
    #[serde(default)]
    pub skin: Option<String>,
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
            skin: None,
        }
    }
}

pub fn config_path() -> PathBuf {
    let xdg_config_home = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .filter(|path| !path.as_os_str().is_empty() && path.is_absolute());
    let home_config = std::env::var_os("HOME")
        .map(PathBuf::from)
        .filter(|path| !path.as_os_str().is_empty())
        .map(|home| home.join(".config"));
    let base = xdg_config_home.or(home_config).unwrap_or_else(|| {
        // Avoid a shared /tmp config when neither standard per-user location is
        // available. The working-directory fallback remains local to this run.
        std::env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("."))
            .join(".config")
    });
    base.join("retroamp").join("config.json")
}

static TEMP_FILE_SEQUENCE: AtomicU64 = AtomicU64::new(0);

fn write_atomically(path: &Path, contents: &[u8]) -> std::io::Result<()> {
    let parent = path.parent().ok_or_else(|| {
        std::io::Error::new(std::io::ErrorKind::InvalidInput, "config path has no parent")
    })?;
    let filename = path.file_name().ok_or_else(|| {
        std::io::Error::new(std::io::ErrorKind::InvalidInput, "config path has no filename")
    })?;

    for _ in 0..128 {
        let sequence = TEMP_FILE_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let temp_path = parent.join(format!(
            ".{}.tmp-{}-{sequence}",
            filename.to_string_lossy(),
            std::process::id()
        ));
        let mut temp_file = match OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&temp_path)
        {
            Ok(file) => file,
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error),
        };

        if let Err(error) = temp_file.write_all(contents).and_then(|_| temp_file.sync_all()) {
            drop(temp_file);
            let _ = std::fs::remove_file(&temp_path);
            return Err(error);
        }
        drop(temp_file);

        if let Err(error) = std::fs::rename(&temp_path, path) {
            let _ = std::fs::remove_file(&temp_path);
            return Err(error);
        }
        return Ok(());
    }

    Err(std::io::Error::new(
        std::io::ErrorKind::AlreadyExists,
        "could not allocate a unique temporary config file",
    ))
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

    pub fn save(&self) -> bool {
        let path = config_path();
        if let Some(dir) = path.parent() {
            if let Err(error) = std::fs::create_dir_all(dir) {
                log::warn!("Could not create config dir {}: {error}", dir.display());
                return false;
            }
        }

        match serde_json::to_vec_pretty(self) {
            Ok(contents) => match write_atomically(&path, &contents) {
                Ok(()) => true,
                Err(error) => {
                    log::warn!("Could not save config {}: {error}", path.display());
                    false
                }
            },
            Err(error) => {
                log::warn!("Config serialize error: {error}");
                false
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    #[test]
    fn atomic_config_replacement_uses_private_permissions() {
        let sequence = TEMP_FILE_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!(
            "retroamp-config-test-{}-{sequence}",
            std::process::id()
        ));
        std::fs::create_dir(&dir).unwrap();
        let path = dir.join("config.json");
        std::fs::write(&path, b"old").unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();

        write_atomically(&path, b"new").unwrap();

        let mode = std::fs::metadata(&path).unwrap().permissions().mode() & 0o777;
        assert_eq!(std::fs::read(&path).unwrap(), b"new");
        let _ = std::fs::remove_dir_all(dir);
        assert_eq!(mode, 0o600);
    }
}
