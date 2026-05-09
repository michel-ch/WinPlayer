#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use parking_lot::RwLock;
use std::sync::Arc;
use winplayer::data::Library;
use winplayer::playback::PlaybackController;
use winplayer::settings::Settings;
use winplayer::ui::App;

fn main() -> Result<(), eframe::Error> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    let settings = Arc::new(RwLock::new(Settings::load_or_default()));
    let library = Arc::new(Library::new());

    {
        let lib = library.clone();
        let roots = settings.read().scan.roots.clone();
        std::thread::Builder::new().name("library-scan".into()).spawn(move || {
            lib.scan(&roots);
        }).expect("spawn library-scan");
    }

    let playback = match PlaybackController::new(library.clone()) {
        Ok(p) => p,
        Err(e) => {
            log::error!("playback init failed: {e}");
            std::process::exit(1);
        }
    };

    let initial_volume = settings.read().playback.volume;
    playback.set_volume(initial_volume);

    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1200.0, 800.0])
            .with_min_inner_size([720.0, 480.0])
            .with_title("WinPlayer"),
        ..Default::default()
    };

    eframe::run_native(
        "WinPlayer",
        native_options,
        Box::new(move |cc| Ok(Box::new(App::new(cc, library, playback, settings)))),
    )
}
