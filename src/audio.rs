use std::io::BufReader;
use std::fs::File;
use std::path::Path;
use std::sync::Arc;
use std::time::Instant;

use cpal::traits::{DeviceTrait, HostTrait};
use parking_lot::Mutex;
use rodio::{Decoder, OutputStream, OutputStreamHandle, Sink};

/// Represents a currently playing sound
pub struct PlayingSound {
    pub name: String,
    pub sink: Sink,
    pub local_sink: Option<Sink>,
    pub base_volume: f32,
}

/// Core audio engine handling device output and playback
pub struct AudioEngine {
    _stream: OutputStream,
    stream_handle: OutputStreamHandle,
    _local_stream: Option<OutputStream>,
    local_stream_handle: Option<OutputStreamHandle>,
    pub playing: Arc<Mutex<Vec<PlayingSound>>>,
    pub device_name: String,
}

impl AudioEngine {
    /// Create a new AudioEngine targeting a specific output device by name.
    /// If `device_name` is None, uses the default output device.
    pub fn new(device_name: Option<&str>) -> Result<Self, String> {
        let host = cpal::default_host();

        let device = if let Some(name) = device_name {
            host.output_devices()
                .map_err(|e| format!("Failed to enumerate output devices: {}", e))?
                .find(|d| {
                    d.name()
                        .map(|n| n.to_lowercase().contains(&name.to_lowercase()))
                        .unwrap_or(false)
                })
                .ok_or_else(|| format!("Output device '{}' not found", name))?
        } else {
            host.default_output_device()
                .ok_or_else(|| "No default output device found".to_string())?
        };

        let actual_name = device.name().unwrap_or_else(|_| "Unknown".into());
        log::info!("Using output device: {}", actual_name);

        let mut local_stream = None;
        let mut local_stream_handle = None;

        // Try to create a local stream to the default device if we are targeting a specific device
        if let Some(def_dev) = host.default_output_device() {
            let def_name = def_dev.name().unwrap_or_default();
            if def_name != actual_name {
                if let Ok((ls, lh)) = OutputStream::try_from_device(&def_dev) {
                    local_stream = Some(ls);
                    local_stream_handle = Some(lh);
                    log::info!("Local output device initialized: {}", def_name);
                }
            }
        }

        let (stream, stream_handle) = OutputStream::try_from_device(&device)
            .map_err(|e| format!("Failed to create output stream: {}", e))?;

        Ok(Self {
            _stream: stream,
            stream_handle,
            _local_stream: local_stream,
            local_stream_handle,
            playing: Arc::new(Mutex::new(Vec::new())),
            device_name: actual_name,
        })
    }

    /// Play an audio file, returning Ok on success
    pub fn play(&self, path: &Path, name: &str, base_volume: f32, master_volume: f32, play_locally: bool, local_volume: f32) -> Result<(), String> {
        let file = File::open(path)
            .map_err(|e| format!("Cannot open file '{}': {}", path.display(), e))?;
        let reader = BufReader::new(file);
        let source = Decoder::new(reader)
            .map_err(|e| format!("Cannot decode '{}': {}", path.display(), e))?;

        let sink = Sink::try_new(&self.stream_handle)
            .map_err(|e| format!("Cannot create sink: {}", e))?;

        sink.set_volume(base_volume * master_volume);
        sink.append(source);

        let mut local_sink = None;
        if play_locally {
            if let Some(ref lh) = self.local_stream_handle {
                if let Ok(lsink) = Sink::try_new(lh) {
                    lsink.set_volume(base_volume * local_volume);
                    if let Ok(file2) = File::open(path) {
                        if let Ok(source2) = Decoder::new(BufReader::new(file2)) {
                            lsink.append(source2);
                            local_sink = Some(lsink);
                        }
                    }
                }
            }
        }

        let mut playing = self.playing.lock();
        // Clean up finished sounds
        playing.retain(|p| !p.sink.empty());
        playing.push(PlayingSound {
            name: name.to_string(),
            sink,
            local_sink,
            base_volume,
        });

        Ok(())
    }

    /// Stop all currently playing sounds
    pub fn stop_all(&self) {
        let mut playing = self.playing.lock();
        for p in playing.drain(..) {
            p.sink.stop();
            if let Some(ls) = p.local_sink { ls.stop(); }
        }
    }

    /// Stop a specific sound by name (first match)
    pub fn stop_by_name(&self, name: &str) {
        let mut playing = self.playing.lock();
        if let Some(idx) = playing.iter().position(|p| p.name == name) {
            let p = playing.remove(idx);
            p.sink.stop();
            if let Some(ls) = p.local_sink { ls.stop(); }
        }
    }

    pub fn update_global_volumes(&self, master_volume: f32, local_volume: f32) {
        let playing = self.playing.lock();
        for p in playing.iter() {
            p.sink.set_volume(p.base_volume * master_volume);
            if let Some(ref ls) = &p.local_sink {
                ls.set_volume(p.base_volume * local_volume);
            }
        }
    }

    pub fn update_sound_volume(&self, name: &str, new_base_volume: f32, master_volume: f32, local_volume: f32) {
        let mut playing = self.playing.lock();
        for p in playing.iter_mut().filter(|p| p.name == name) {
            p.base_volume = new_base_volume;
            p.sink.set_volume(new_base_volume * master_volume);
            if let Some(ref ls) = p.local_sink {
                ls.set_volume(new_base_volume * local_volume);
            }
        }
    }

    /// Get names of currently playing sounds (cleaning up finished ones)
    pub fn currently_playing(&self) -> Vec<String> {
        let mut playing = self.playing.lock();
        playing.retain(|p| !p.sink.empty());
        playing.iter().map(|p| p.name.clone()).collect()
    }

    /// List all available output devices
    pub fn list_output_devices() -> Vec<String> {
        let host = cpal::default_host();
        host.output_devices()
            .map(|devices| {
                devices
                    .filter_map(|d| d.name().ok())
                    .collect()
            })
            .unwrap_or_default()
    }
}
