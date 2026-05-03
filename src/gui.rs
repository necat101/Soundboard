use eframe::egui;
use std::sync::Arc;
use std::time::Duration;

use parking_lot::Mutex;

use crate::audio::{AudioEngine, PlaybackInfo};
use crate::config::AppConfig;
use crate::sound::{filter_sounds, FolderTab, SoundEntry};

const SEEK_JUMP: Duration = Duration::from_secs(5);

/// Main application state
pub struct SoundboardApp {
    engine: Arc<Mutex<Option<AudioEngine>>>,
    config: AppConfig,
    search_query: String,
    available_devices: Vec<String>,
    selected_device_idx: usize,
    status_message: String,
    needs_save: bool,
}

impl SoundboardApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        // Configure dark visuals with accent colors
        let mut visuals = egui::Visuals::dark();
        visuals.widgets.noninteractive.bg_fill = egui::Color32::from_rgb(25, 25, 35);
        visuals.widgets.inactive.bg_fill = egui::Color32::from_rgb(35, 35, 50);
        visuals.widgets.hovered.bg_fill = egui::Color32::from_rgb(55, 55, 80);
        visuals.widgets.active.bg_fill = egui::Color32::from_rgb(80, 60, 180);
        visuals.selection.bg_fill = egui::Color32::from_rgb(100, 70, 200);
        visuals.window_fill = egui::Color32::from_rgb(20, 20, 30);
        visuals.panel_fill = egui::Color32::from_rgb(20, 20, 30);
        visuals.extreme_bg_color = egui::Color32::from_rgb(15, 15, 22);
        visuals.faint_bg_color = egui::Color32::from_rgb(30, 30, 42);
        cc.egui_ctx.set_visuals(visuals);

        let mut style = (*cc.egui_ctx.style()).clone();
        style
            .text_styles
            .insert(egui::TextStyle::Heading, egui::FontId::proportional(22.0));
        style
            .text_styles
            .insert(egui::TextStyle::Body, egui::FontId::proportional(14.0));
        style
            .text_styles
            .insert(egui::TextStyle::Button, egui::FontId::proportional(14.0));
        style
            .text_styles
            .insert(egui::TextStyle::Small, egui::FontId::proportional(11.0));
        style.spacing.item_spacing = egui::vec2(8.0, 6.0);
        cc.egui_ctx.set_style(style);

        let config = AppConfig::load();
        let available_devices = AudioEngine::list_output_devices();

        let engine = match AudioEngine::new(config.output_device.as_deref()) {
            Ok(e) => {
                log::info!("Audio engine initialized: {}", e.device_name);
                Some(e)
            }
            Err(e) => {
                log::error!("Failed to init audio engine: {}", e);
                match AudioEngine::new(None) {
                    Ok(e) => Some(e),
                    Err(_) => None,
                }
            }
        };

        let selected_device_idx = if let Some(ref e) = engine {
            available_devices
                .iter()
                .position(|d| d == &e.device_name)
                .unwrap_or(0)
        } else {
            0
        };

        Self {
            engine: Arc::new(Mutex::new(engine)),
            config,
            search_query: String::new(),
            available_devices,
            selected_device_idx,
            status_message: String::new(),
            needs_save: false,
        }
    }

    fn play_sound(&self, sound: &SoundEntry) {
        let engine = self.engine.lock();
        if let Some(ref engine) = *engine {
            match engine.play(
                &sound.path,
                &sound.name,
                sound.volume,
                self.config.master_volume,
                self.config.play_locally,
                self.config.local_volume,
            ) {
                Ok(()) => log::info!("Playing: {}", sound.name),
                Err(e) => log::error!("Playback error: {}", e),
            }
        }
    }

    fn stop_all(&self) {
        let engine = self.engine.lock();
        if let Some(ref engine) = *engine {
            engine.stop_all();
        }
    }

    fn switch_device(&mut self, device_name: &str) {
        let mut engine_lock = self.engine.lock();
        match AudioEngine::new(Some(device_name)) {
            Ok(e) => {
                self.status_message = format!("Switched to: {}", e.device_name);
                self.config.output_device = Some(device_name.to_string());
                *engine_lock = Some(e);
                self.needs_save = true;
            }
            Err(e) => {
                self.status_message = format!("Error: {}", e);
                log::error!("Failed to switch device: {}", e);
            }
        }
    }

    fn seek_sound_to(&mut self, sound_name: &str, seconds: f64) {
        let result = {
            let engine = self.engine.lock();
            engine
                .as_ref()
                .map(|e| e.seek_by_name(sound_name, Duration::from_secs_f64(seconds.max(0.0))))
        };

        if let Some(Err(e)) = result {
            self.status_message = e;
        }
    }

    fn jump_sound(&mut self, sound_name: &str, forward: bool) {
        let result = {
            let engine = self.engine.lock();
            engine
                .as_ref()
                .map(|e| e.seek_relative_by_name(sound_name, SEEK_JUMP, forward))
        };

        if let Some(Err(e)) = result {
            self.status_message = e;
        }
    }

    fn format_duration(duration: Duration) -> String {
        let total = duration.as_secs();
        let seconds = total % 60;
        let minutes = (total / 60) % 60;
        let hours = total / 3600;

        if hours > 0 {
            format!("{}:{:02}:{:02}", hours, minutes, seconds)
        } else {
            format!("{}:{:02}", minutes, seconds)
        }
    }

    fn render_top_bar(&mut self, ui: &mut egui::Ui) {
        ui.vertical(|ui| {
            ui.horizontal_wrapped(|ui| {
                ui.label(
                    egui::RichText::new("Soundboard")
                        .size(20.0)
                        .color(egui::Color32::from_rgb(140, 100, 255))
                        .strong(),
                );
                ui.separator();

                ui.label("Output:");
                let current = {
                    let engine = self.engine.lock();
                    if let Some(ref e) = *engine {
                        e.device_name.clone()
                    } else if self.available_devices.is_empty() {
                        "No devices".to_string()
                    } else {
                        "Select...".to_string()
                    }
                };

                if let Some(idx) = self.available_devices.iter().position(|d| d == &current) {
                    self.selected_device_idx = idx;
                }

                let device_width = ui.available_width().clamp(180.0, 250.0);
                let device_response = egui::ComboBox::from_id_salt("device_selector")
                    .selected_text(&current)
                    .width(device_width)
                    .show_ui(ui, |ui| {
                        let mut changed = false;
                        for (i, dev) in self.available_devices.iter().enumerate() {
                            let is_cable = dev.to_lowercase().contains("cable");
                            let label = if is_cable {
                                egui::RichText::new(format!("Cable: {}", dev))
                                    .color(egui::Color32::from_rgb(100, 220, 100))
                            } else {
                                egui::RichText::new(dev.as_str())
                            };
                            if ui
                                .selectable_value(&mut self.selected_device_idx, i, label)
                                .changed()
                            {
                                changed = true;
                            }
                        }
                        changed
                    });

                if let Some(inner) = device_response.inner {
                    if inner {
                        if let Some(name) = self
                            .available_devices
                            .get(self.selected_device_idx)
                            .cloned()
                        {
                            self.switch_device(&name);
                        }
                    }
                }

                ui.separator();
                ui.label("Virtual Vol:");
                let vol_slider = ui.add_sized(
                    [120.0, 22.0],
                    egui::Slider::new(&mut self.config.master_volume, 0.0..=2.0)
                        .show_value(true)
                        .custom_formatter(|v, _| format!("{:.0}%", v * 100.0))
                        .clamping(egui::SliderClamping::Always),
                );
                if vol_slider.changed() {
                    self.needs_save = true;
                    if let Some(ref engine) = *self.engine.lock() {
                        engine.update_global_volumes(
                            self.config.master_volume,
                            self.config.local_volume,
                        );
                    }
                }

                ui.separator();
                if ui
                    .checkbox(&mut self.config.play_locally, "Echo to Speakers")
                    .changed()
                {
                    self.needs_save = true;
                }

                if self.config.play_locally {
                    ui.label("Local Vol:");
                    let lvol_slider = ui.add_sized(
                        [120.0, 22.0],
                        egui::Slider::new(&mut self.config.local_volume, 0.0..=2.0)
                            .show_value(true)
                            .custom_formatter(|v, _| format!("{:.0}%", v * 100.0))
                            .clamping(egui::SliderClamping::Always),
                    );
                    if lvol_slider.changed() {
                        self.needs_save = true;
                        if let Some(ref engine) = *self.engine.lock() {
                            engine.update_global_volumes(
                                self.config.master_volume,
                                self.config.local_volume,
                            );
                        }
                    }
                }

                ui.separator();
                if ui
                    .add(
                        egui::Button::new(
                            egui::RichText::new("Stop All")
                                .color(egui::Color32::from_rgb(255, 100, 100)),
                        )
                        .min_size(egui::vec2(86.0, 28.0)),
                    )
                    .clicked()
                {
                    self.stop_all();
                }
            });

            ui.add_space(6.0);
            ui.horizontal(|ui| {
                ui.label("Search:");
                ui.add_sized(
                    [ui.available_width(), 24.0],
                    egui::TextEdit::singleline(&mut self.search_query)
                        .hint_text("Search sounds..."),
                );
            });
        });
    }

    fn render_tabs(&mut self, ui: &mut egui::Ui) {
        egui::ScrollArea::horizontal()
            .id_salt("tabs_scroll")
            .auto_shrink([false, true])
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    let num_tabs = self.config.folders.len();
                    for i in 0..num_tabs {
                        let name = self.config.folders[i].name.clone();
                        let is_active = self.config.active_tab == i;

                        let btn = if is_active {
                            egui::Button::new(
                                egui::RichText::new(&name)
                                    .color(egui::Color32::WHITE)
                                    .strong(),
                            )
                            .fill(egui::Color32::from_rgb(80, 60, 180))
                            .corner_radius(egui::CornerRadius::same(6))
                        } else {
                            egui::Button::new(
                                egui::RichText::new(&name)
                                    .color(egui::Color32::from_rgb(180, 180, 200)),
                            )
                            .fill(egui::Color32::from_rgb(40, 40, 55))
                            .corner_radius(egui::CornerRadius::same(6))
                        };

                        let response = ui.add(btn.min_size(egui::vec2(80.0, 30.0)));
                        if response.clicked() {
                            self.config.active_tab = i;
                            self.needs_save = true;
                        }

                        response.context_menu(|ui| {
                            if ui.button("Refresh").clicked() {
                                self.config.folders[i].refresh();
                                self.needs_save = true;
                                ui.close_menu();
                            }
                            if ui.button("Remove Tab").clicked() {
                                self.config.folders.remove(i);
                                if self.config.active_tab >= self.config.folders.len()
                                    && !self.config.folders.is_empty()
                                {
                                    self.config.active_tab = self.config.folders.len() - 1;
                                }
                                self.needs_save = true;
                                ui.close_menu();
                            }
                        });
                    }

                    if ui
                        .add(
                            egui::Button::new(
                                egui::RichText::new(" + ")
                                    .size(16.0)
                                    .color(egui::Color32::from_rgb(100, 220, 100)),
                            )
                            .fill(egui::Color32::from_rgb(35, 50, 35))
                            .corner_radius(egui::CornerRadius::same(6))
                            .min_size(egui::vec2(40.0, 30.0)),
                        )
                        .clicked()
                    {
                        if let Some(folder) = rfd::FileDialog::new().pick_folder() {
                            let tab = FolderTab::from_directory(&folder);
                            log::info!("Added folder: {} ({} sounds)", tab.name, tab.sounds.len());
                            self.config.active_tab = self.config.folders.len();
                            self.config.folders.push(tab);
                            self.needs_save = true;
                        }
                    }
                });
            });
    }

    fn render_sound_grid(&mut self, ui: &mut egui::Ui) {
        if self.config.folders.is_empty() {
            ui.vertical_centered(|ui| {
                ui.add_space(80.0);
                ui.label(
                    egui::RichText::new("No folders added yet")
                        .size(18.0)
                        .color(egui::Color32::from_rgb(120, 120, 150)),
                );
                ui.add_space(10.0);
                ui.label(
                    egui::RichText::new("Click the + button above to add a folder of sounds")
                        .size(14.0)
                        .color(egui::Color32::from_rgb(90, 90, 110)),
                );
            });
            return;
        }

        let tab_idx = self
            .config
            .active_tab
            .min(self.config.folders.len().saturating_sub(1));

        let sounds: Vec<SoundEntry> = {
            let tab = &self.config.folders[tab_idx];
            filter_sounds(&tab.sounds, &self.search_query)
                .into_iter()
                .cloned()
                .collect()
        };

        if sounds.is_empty() {
            ui.vertical_centered(|ui| {
                ui.add_space(40.0);
                if self.search_query.is_empty() {
                    ui.label(
                        egui::RichText::new("No audio files found in this folder")
                            .color(egui::Color32::from_rgb(120, 120, 150)),
                    );
                } else {
                    ui.label(
                        egui::RichText::new(format!("No sounds matching '{}'", self.search_query))
                            .color(egui::Color32::from_rgb(120, 120, 150)),
                    );
                }
            });
            return;
        }

        let playback_infos: Vec<PlaybackInfo> = {
            let engine = self.engine.lock();
            engine
                .as_ref()
                .map(|e| e.playback_snapshot())
                .unwrap_or_default()
        };

        let spacing = 8.0_f32;
        let min_tile_width = 300.0_f32;
        let available_width = ui.available_width().max(1.0);
        let cols =
            (((available_width + spacing) / (min_tile_width + spacing)).floor() as usize).max(1);
        let tile_width = if cols == 1 {
            available_width
        } else {
            ((available_width - spacing * (cols.saturating_sub(1) as f32)) / cols as f32).floor()
        };

        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.spacing_mut().item_spacing = egui::vec2(spacing, spacing);
                ui.horizontal_wrapped(|ui| {
                    for sound in sounds.iter() {
                        let playback = playback_infos.iter().find(|info| info.name == sound.name);
                        ui.allocate_ui_with_layout(
                            egui::vec2(tile_width, 0.0),
                            egui::Layout::top_down(egui::Align::Min),
                            |ui| {
                                self.render_sound_tile(ui, sound, tab_idx, playback, tile_width);
                            },
                        );
                    }
                });
            });
    }

    fn render_sound_tile(
        &mut self,
        ui: &mut egui::Ui,
        sound: &SoundEntry,
        tab_idx: usize,
        playback: Option<&PlaybackInfo>,
        tile_width: f32,
    ) {
        let is_playing = playback.is_some();
        let bg_color = if is_playing {
            egui::Color32::from_rgb(45, 35, 80)
        } else {
            egui::Color32::from_rgb(30, 30, 45)
        };

        let frame = egui::Frame::new()
            .fill(bg_color)
            .corner_radius(egui::CornerRadius::same(8))
            .inner_margin(egui::Margin::same(12))
            .stroke(egui::Stroke::new(
                1.0,
                if is_playing {
                    egui::Color32::from_rgb(120, 80, 255)
                } else {
                    egui::Color32::from_rgb(50, 50, 70)
                },
            ));

        let inner_width = (tile_width - 24.0).max(120.0);
        frame.show(ui, |ui| {
            ui.set_min_width(inner_width);
            ui.set_max_width(inner_width);
            ui.vertical(|ui| {
                ui.set_min_width(inner_width);
                ui.set_max_width(inner_width);

                let name_color = if is_playing {
                    egui::Color32::from_rgb(180, 150, 255)
                } else {
                    egui::Color32::from_rgb(220, 220, 240)
                };

                ui.add_sized(
                    [inner_width, 20.0],
                    egui::Label::new(
                        egui::RichText::new(&sound.name)
                            .color(name_color)
                            .strong()
                            .size(13.0),
                    )
                    .truncate(),
                )
                .on_hover_text(&sound.name);

                ui.horizontal(|ui| {
                    let play_text = if is_playing { "Stop" } else { "Play" };
                    let play_color = if is_playing {
                        egui::Color32::from_rgb(255, 100, 100)
                    } else {
                        egui::Color32::from_rgb(100, 220, 100)
                    };

                    if ui
                        .add_sized(
                            [52.0, 26.0],
                            egui::Button::new(
                                egui::RichText::new(play_text).color(play_color).size(13.0),
                            ),
                        )
                        .clicked()
                    {
                        if is_playing {
                            let engine = self.engine.lock();
                            if let Some(ref e) = *engine {
                                e.stop_by_name(&sound.name);
                            }
                        } else {
                            self.play_sound(sound);
                        }
                    }

                    let sound_name = sound.name.clone();
                    let mut vol = sound.volume;
                    let slider_width = ui.available_width().max(64.0);
                    let slider = ui.add_sized(
                        [slider_width, 22.0],
                        egui::Slider::new(&mut vol, 0.0..=2.0)
                            .show_value(false)
                            .clamping(egui::SliderClamping::Always),
                    );
                    if slider.changed() {
                        if let Some(tab) = self.config.folders.get_mut(tab_idx) {
                            if let Some(s) = tab.sounds.iter_mut().find(|s| s.name == sound_name) {
                                s.volume = vol;
                                self.needs_save = true;
                            }
                        }
                        if let Some(ref engine) = *self.engine.lock() {
                            engine.update_sound_volume(
                                &sound_name,
                                vol,
                                self.config.master_volume,
                                self.config.local_volume,
                            );
                        }
                    }
                });

                if let Some(playback) = playback {
                    ui.add_space(4.0);
                    if let Some(duration) = playback.duration.filter(|d| !d.is_zero()) {
                        let duration_secs = duration.as_secs_f64();
                        let mut position_secs = playback.position.min(duration).as_secs_f64();
                        let time_label = format!(
                            "{} / {}",
                            Self::format_duration(playback.position.min(duration)),
                            Self::format_duration(duration)
                        );

                        ui.horizontal(|ui| {
                            if ui
                                .add_sized([32.0, 24.0], egui::Button::new("<<"))
                                .on_hover_text("Rewind 5 seconds")
                                .clicked()
                            {
                                self.jump_sound(&sound.name, false);
                            }

                            let progress_width =
                                (inner_width - 72.0 - ui.spacing().item_spacing.x * 2.0).max(80.0);
                            let response = ui
                                .add_sized(
                                    [progress_width, 22.0],
                                    egui::Slider::new(&mut position_secs, 0.0..=duration_secs)
                                        .show_value(false)
                                        .clamping(egui::SliderClamping::Always),
                                )
                                .on_hover_text("Drag to seek");
                            if response.changed() {
                                self.seek_sound_to(&sound.name, position_secs);
                            }

                            if ui
                                .add_sized([32.0, 24.0], egui::Button::new(">>"))
                                .on_hover_text("Fast forward 5 seconds")
                                .clicked()
                            {
                                self.jump_sound(&sound.name, true);
                            }
                        });

                        ui.add_sized(
                            [inner_width, 14.0],
                            egui::Label::new(
                                egui::RichText::new(time_label)
                                    .size(10.0)
                                    .color(egui::Color32::from_rgb(120, 120, 155)),
                            )
                            .truncate(),
                        );
                    } else {
                        ui.label(
                            egui::RichText::new(format!(
                                "{} / unknown duration",
                                Self::format_duration(playback.position)
                            ))
                            .size(10.0)
                            .color(egui::Color32::from_rgb(120, 120, 155)),
                        );
                    }
                }

                if let Some(ref hk) = sound.hotkey {
                    ui.label(
                        egui::RichText::new(format!("Key: {}", hk))
                            .size(10.0)
                            .color(egui::Color32::from_rgb(100, 100, 140)),
                    );
                }
            });
        });
    }

    fn render_status_bar(&self, ui: &mut egui::Ui) {
        ui.horizontal_wrapped(|ui| {
            let playing = {
                let engine = self.engine.lock();
                engine
                    .as_ref()
                    .map(|e| e.currently_playing())
                    .unwrap_or_default()
            };

            if playing.is_empty() {
                ui.label(
                    egui::RichText::new("Ready")
                        .size(11.0)
                        .color(egui::Color32::from_rgb(90, 90, 110)),
                );
            } else {
                ui.add(
                    egui::Label::new(
                        egui::RichText::new(format!("Playing: {}", playing.join(", ")))
                            .size(11.0)
                            .color(egui::Color32::from_rgb(140, 120, 255)),
                    )
                    .truncate(),
                );
            }

            if !self.status_message.is_empty() {
                ui.separator();
                ui.label(
                    egui::RichText::new(&self.status_message)
                        .size(11.0)
                        .color(egui::Color32::from_rgb(120, 120, 140)),
                );
            }

            let engine = self.engine.lock();
            if let Some(ref e) = *engine {
                ui.separator();
                let dev_display = if e.device_name.to_lowercase().contains("cable") {
                    format!("Cable: {}", e.device_name)
                } else {
                    e.device_name.clone()
                };
                ui.label(
                    egui::RichText::new(dev_display)
                        .size(11.0)
                        .color(egui::Color32::from_rgb(90, 130, 90)),
                );
            }
        });
    }
}

impl eframe::App for SoundboardApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            self.stop_all();
        }

        egui::TopBottomPanel::top("top_bar")
            .frame(
                egui::Frame::new()
                    .fill(egui::Color32::from_rgb(18, 18, 28))
                    .inner_margin(egui::Margin::same(10))
                    .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(40, 40, 60))),
            )
            .show(ctx, |ui| {
                self.render_top_bar(ui);
            });

        egui::TopBottomPanel::bottom("status_bar")
            .frame(
                egui::Frame::new()
                    .fill(egui::Color32::from_rgb(15, 15, 22))
                    .inner_margin(egui::Margin::symmetric(10, 5))
                    .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(35, 35, 50))),
            )
            .show(ctx, |ui| {
                self.render_status_bar(ui);
            });

        egui::CentralPanel::default()
            .frame(
                egui::Frame::new()
                    .fill(egui::Color32::from_rgb(20, 20, 30))
                    .inner_margin(egui::Margin::same(12)),
            )
            .show(ctx, |ui| {
                self.render_tabs(ui);
                ui.add_space(8.0);
                ui.separator();
                ui.add_space(8.0);
                self.render_sound_grid(ui);
            });

        if self.needs_save {
            self.config.save();
            self.needs_save = false;
        }

        let has_playing_sounds = {
            let engine = self.engine.lock();
            engine
                .as_ref()
                .map(|e| !e.currently_playing().is_empty())
                .unwrap_or(false)
        };
        if has_playing_sounds {
            ctx.request_repaint_after(Duration::from_millis(100));
        }
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        self.config.save();
        log::info!("Config saved on exit");
    }
}
