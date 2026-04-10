mod audio;
mod config;
mod gui;
mod sound;

use eframe::egui;
use gui::SoundboardApp;

fn main() -> eframe::Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"))
        .format_timestamp_millis()
        .init();

    log::info!("Starting Soundboard v{}", env!("CARGO_PKG_VERSION"));

    let config = config::AppConfig::load();

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Soundboard")
            .with_inner_size([config.window_width, config.window_height])
            .with_min_inner_size([600.0, 400.0]),
        ..Default::default()
    };

    eframe::run_native(
        "Soundboard",
        options,
        Box::new(|cc| Ok(Box::new(SoundboardApp::new(cc)))),
    )
}
