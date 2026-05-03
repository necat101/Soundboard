use std::fs::File;
use std::io::BufReader;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use cpal::traits::{DeviceTrait, HostTrait};
use parking_lot::Mutex;
use rodio::{Decoder, OutputStream, OutputStreamHandle, Sink, Source};

/// Playback state that the UI can render without touching rodio internals.
#[derive(Clone, Debug)]
pub struct PlaybackInfo {
    pub name: String,
    pub position: Duration,
    pub duration: Option<Duration>,
}

/// Represents a currently playing sound
pub struct PlayingSound {
    pub name: String,
    pub sink: Sink,
    pub local_sink: Option<Sink>,
    pub base_volume: f32,
    pub duration: Option<Duration>,
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
    fn estimate_duration(path: &Path) -> Option<Duration> {
        let file = File::open(path).ok()?;
        let source = Decoder::new(BufReader::new(file)).ok()?;
        let sample_rate = source.sample_rate() as f64;
        let channels = source.channels() as f64;

        if sample_rate <= 0.0 || channels <= 0.0 {
            return None;
        }

        let sample_count = source.count() as f64;
        let seconds = sample_count / sample_rate / channels;
        if seconds.is_finite() && seconds > 0.0 {
            Some(Duration::from_secs_f64(seconds))
        } else {
            None
        }
    }

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
    pub fn play(
        &self,
        path: &Path,
        name: &str,
        base_volume: f32,
        master_volume: f32,
        play_locally: bool,
        local_volume: f32,
    ) -> Result<(), String> {
        let file = File::open(path)
            .map_err(|e| format!("Cannot open file '{}': {}", path.display(), e))?;
        let reader = BufReader::new(file);
        let source = Decoder::new(reader)
            .map_err(|e| format!("Cannot decode '{}': {}", path.display(), e))?;
        let duration = source
            .total_duration()
            .or_else(|| Self::estimate_duration(path));

        let sink =
            Sink::try_new(&self.stream_handle).map_err(|e| format!("Cannot create sink: {}", e))?;

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
            duration,
        });

        Ok(())
    }

    fn seek_playing_sound(sound: &PlayingSound, position: Duration) -> Result<(), String> {
        let target = sound
            .duration
            .map(|duration| position.min(duration))
            .unwrap_or(position);

        sound
            .sink
            .try_seek(target)
            .map_err(|e| format!("Cannot seek '{}': {}", sound.name, e))?;

        if let Some(ref local_sink) = sound.local_sink {
            if let Err(e) = local_sink.try_seek(target) {
                log::warn!("Failed to seek local copy of '{}': {}", sound.name, e);
            }
        }

        Ok(())
    }

    /// Seek a currently playing sound to an exact position.
    pub fn seek_by_name(&self, name: &str, position: Duration) -> Result<(), String> {
        let playing = self.playing.lock();
        let mut found = false;
        let mut first_error = None;

        for sound in playing.iter().filter(|p| p.name == name) {
            found = true;
            if let Err(e) = Self::seek_playing_sound(sound, position) {
                first_error.get_or_insert(e);
            }
        }

        if !found {
            return Err(format!("Sound '{}' is not playing", name));
        }

        if let Some(error) = first_error {
            Err(error)
        } else {
            Ok(())
        }
    }

    /// Move a currently playing sound forward or backward by a fixed amount.
    pub fn seek_relative_by_name(
        &self,
        name: &str,
        delta: Duration,
        forward: bool,
    ) -> Result<(), String> {
        let playing = self.playing.lock();
        let mut found = false;
        let mut first_error = None;

        for sound in playing.iter().filter(|p| p.name == name) {
            found = true;
            let current = sound.sink.get_pos();
            let target = if forward {
                current.checked_add(delta).unwrap_or(Duration::MAX)
            } else {
                current.saturating_sub(delta)
            };

            if let Err(e) = Self::seek_playing_sound(sound, target) {
                first_error.get_or_insert(e);
            }
        }

        if !found {
            return Err(format!("Sound '{}' is not playing", name));
        }

        if let Some(error) = first_error {
            Err(error)
        } else {
            Ok(())
        }
    }

    /// Stop all currently playing sounds
    pub fn stop_all(&self) {
        let mut playing = self.playing.lock();
        for p in playing.drain(..) {
            p.sink.stop();
            if let Some(ls) = p.local_sink {
                ls.stop();
            }
        }
    }

    /// Stop a specific sound by name (first match)
    pub fn stop_by_name(&self, name: &str) {
        let mut playing = self.playing.lock();
        if let Some(idx) = playing.iter().position(|p| p.name == name) {
            let p = playing.remove(idx);
            p.sink.stop();
            if let Some(ls) = p.local_sink {
                ls.stop();
            }
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

    pub fn update_sound_volume(
        &self,
        name: &str,
        new_base_volume: f32,
        master_volume: f32,
        local_volume: f32,
    ) {
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

    /// Get current playback positions for active sounds.
    pub fn playback_snapshot(&self) -> Vec<PlaybackInfo> {
        let mut playing = self.playing.lock();
        playing.retain(|p| !p.sink.empty());
        playing
            .iter()
            .map(|p| PlaybackInfo {
                name: p.name.clone(),
                position: p.sink.get_pos(),
                duration: p.duration,
            })
            .collect()
    }

    /// List all available output devices
    pub fn list_output_devices() -> Vec<String> {
        let host = cpal::default_host();
        host.output_devices()
            .map(|devices| devices.filter_map(|d| d.name().ok()).collect())
            .unwrap_or_default()
    }
}
