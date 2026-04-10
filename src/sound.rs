use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use walkdir::WalkDir;

/// Supported audio file extensions
const AUDIO_EXTENSIONS: &[&str] = &["mp3", "wav", "ogg", "flac", "aac", "m4a"];

/// A single sound entry
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SoundEntry {
    pub name: String,
    pub path: PathBuf,
    pub volume: f32,
    pub hotkey: Option<String>,
}

/// A folder tab containing sounds from a directory
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FolderTab {
    pub name: String,
    pub directory: PathBuf,
    pub sounds: Vec<SoundEntry>,
}

impl FolderTab {
    /// Scan a directory and create a FolderTab from its audio files
    pub fn from_directory(dir: &Path) -> Self {
        let name = dir
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "Untitled".into());

        let mut sounds = Vec::new();

        for entry in WalkDir::new(dir)
            .max_depth(3)
            .follow_links(true)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            let path = entry.path();
            if path.is_file() {
                if let Some(ext) = path.extension() {
                    let ext_lower = ext.to_string_lossy().to_lowercase();
                    if AUDIO_EXTENSIONS.contains(&ext_lower.as_str()) {
                        let sound_name = path
                            .file_stem()
                            .map(|s| s.to_string_lossy().to_string())
                            .unwrap_or_else(|| "Unknown".into());

                        sounds.push(SoundEntry {
                            name: sound_name,
                            path: path.to_path_buf(),
                            volume: 1.0,
                            hotkey: None,
                        });
                    }
                }
            }
        }

        // Sort by name for consistent ordering
        sounds.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));

        Self {
            name,
            directory: dir.to_path_buf(),
            sounds,
        }
    }

    /// Refresh the folder tab by re-scanning the directory
    pub fn refresh(&mut self) {
        let fresh = Self::from_directory(&self.directory);
        // Preserve per-sound settings for existing sounds
        for new_sound in &fresh.sounds {
            if let Some(existing) = self.sounds.iter().find(|s| s.path == new_sound.path) {
                // keep old volume / hotkey
                let _ = existing; // used below via clone
            }
        }

        let old_sounds = std::mem::take(&mut self.sounds);
        self.sounds = fresh.sounds;

        for sound in &mut self.sounds {
            if let Some(old) = old_sounds.iter().find(|s| s.path == sound.path) {
                sound.volume = old.volume;
                sound.hotkey = old.hotkey.clone();
            }
        }
    }
}

/// Filter sounds by search query
pub fn filter_sounds<'a>(sounds: &'a [SoundEntry], query: &str) -> Vec<&'a SoundEntry> {
    if query.is_empty() {
        return sounds.iter().collect();
    }
    let q = query.to_lowercase();
    sounds
        .iter()
        .filter(|s| s.name.to_lowercase().contains(&q))
        .collect()
}
