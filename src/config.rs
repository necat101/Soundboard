use std::fs;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::sound::FolderTab;

/// Persistent application configuration
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AppConfig {
    pub output_device: Option<String>,
    pub master_volume: f32, // Primary "Virtual" volume
    #[serde(default = "default_true")]
    pub play_locally: bool,
    #[serde(default = "default_volume")]
    pub local_volume: f32, // Local cloned output volume
    pub folders: Vec<FolderTab>,
    pub active_tab: usize,
    pub window_width: f32,
    pub window_height: f32,
}

fn default_true() -> bool { true }
fn default_volume() -> f32 { 1.0 }

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            output_device: None,
            master_volume: 1.0,
            play_locally: true,
            local_volume: 1.0,
            folders: Vec::new(),
            active_tab: 0,
            window_width: 1100.0,
            window_height: 700.0,
        }
    }
}

impl AppConfig {
    /// Get the config file path
    fn config_path() -> PathBuf {
        let config_dir = dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("soundboard");
        config_dir.join("config.json")
    }

    /// Load config from disk, or return default
    pub fn load() -> Self {
        let path = Self::config_path();
        if path.exists() {
            match fs::read_to_string(&path) {
                Ok(content) => match serde_json::from_str(&content) {
                    Ok(config) => {
                        log::info!("Loaded config from {}", path.display());
                        return config;
                    }
                    Err(e) => log::warn!("Failed to parse config: {}", e),
                },
                Err(e) => log::warn!("Failed to read config: {}", e),
            }
        }
        Self::default()
    }

    /// Save config to disk
    pub fn save(&self) {
        let path = Self::config_path();
        if let Some(parent) = path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        match serde_json::to_string_pretty(self) {
            Ok(json) => {
                if let Err(e) = fs::write(&path, json) {
                    log::error!("Failed to save config: {}", e);
                } else {
                    log::debug!("Config saved to {}", path.display());
                }
            }
            Err(e) => log::error!("Failed to serialize config: {}", e),
        }
    }
}
