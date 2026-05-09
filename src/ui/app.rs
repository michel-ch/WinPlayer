use crate::data::{Library, LibraryStatus};
use crate::domain::Screen;
use crate::playback::PlaybackController;
use crate::settings::Settings;
use crate::ui::screens::all_songs::AllSongsState;
use crate::ui::screens::folders::FoldersState;
use crate::ui::toasts::Toasts;
use parking_lot::RwLock;
use std::sync::Arc;

pub struct App {
    pub library: Arc<Library>,
    pub playback: Arc<PlaybackController>,
    pub settings: Arc<RwLock<Settings>>,
    pub screen: Screen,
    pub toasts: Toasts,
    pub all_songs_state: AllSongsState,
    pub folders_state: FoldersState,
}

impl App {
    pub fn new(
        cc: &eframe::CreationContext<'_>,
        library: Arc<Library>,
        playback: Arc<PlaybackController>,
        settings: Arc<RwLock<Settings>>,
    ) -> Self {
        crate::ui::fonts::install(&cc.egui_ctx);
        Self {
            library, playback, settings,
            screen: Screen::AllSongs,
            toasts: Toasts::new(),
            all_songs_state: AllSongsState::default(),
            folders_state: FoldersState::default(),
        }
    }

    fn handle_shortcuts(&self, ctx: &egui::Context) {
        if ctx.wants_keyboard_input() { return; }
        ctx.input(|i| {
            if i.key_pressed(egui::Key::Space) {
                self.playback.play_pause();
            }
            if i.modifiers.command_only() {
                if i.key_pressed(egui::Key::ArrowRight) { self.playback.next(); }
                if i.key_pressed(egui::Key::ArrowLeft)  { self.playback.previous(); }
            }
        });
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.handle_shortcuts(ctx);
        ctx.request_repaint_after(std::time::Duration::from_millis(50));

        if self.screen.shows_chrome() {
            egui::TopBottomPanel::top("top_bar").show(ctx, |ui| {
                let count = self.library.len();
                crate::ui::components::top_bar::draw(ui, &mut self.screen, count, &mut self.all_songs_state.query);
            });
        }

        if self.library.status() == LibraryStatus::Scanning {
            egui::TopBottomPanel::top("status_bar").show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.spinner();
                    ui.label(format!("Scanning library\u{2026} ({} songs so far)", self.library.len()));
                });
            });
        }

        let visuals = ctx.style().visuals.clone();
        let mini_frame = egui::Frame::side_top_panel(&ctx.style())
            .fill(visuals.window_fill)
            .stroke(egui::Stroke::new(1.0, visuals.widgets.noninteractive.bg_stroke.color));
        egui::TopBottomPanel::bottom("mini_player")
            .min_height(60.0)
            .frame(mini_frame)
            .show(ctx, |ui| {
                let state = self.playback.snapshot();
                crate::ui::components::mini_player::draw(ui, &state, &self.playback, &mut self.screen);
            });

        egui::CentralPanel::default().show(ctx, |ui| {
            match self.screen.clone() {
                Screen::AllSongs => {
                    let threshold = self.settings.read().renumber.threshold;
                    crate::ui::screens::all_songs::draw(
                        ui,
                        &self.library,
                        &self.playback,
                        &mut self.all_songs_state,
                        &mut self.toasts,
                        threshold,
                        &mut self.screen,
                    );
                }
                Screen::AlbumsList => {
                    crate::ui::screens::albums::draw_list(ui, &self.library, &mut self.screen);
                }
                Screen::AlbumDetail(name) => {
                    crate::ui::screens::albums::draw_detail(ui, &self.library, &self.playback, &name, &mut self.screen);
                }
                Screen::ArtistsList => {
                    crate::ui::screens::artists::draw_list(ui, &self.library, &mut self.screen);
                }
                Screen::ArtistDetail(name) => {
                    crate::ui::screens::artists::draw_detail(ui, &self.library, &self.playback, &name, &mut self.screen);
                }
                Screen::Folders => {
                    crate::ui::screens::folders::draw(ui, &self.library, &self.playback, &mut self.folders_state, &mut self.screen);
                }
                Screen::Queue => {
                    crate::ui::screens::queue::draw(ui, &self.playback, &mut self.screen);
                }
                Screen::NowPlaying => {
                    crate::ui::screens::now_playing::draw(ui, &self.playback, &mut self.screen);
                }
                Screen::Equalizer => {
                    crate::ui::screens::equalizer::draw(ui, &self.settings, &mut self.screen);
                }
                Screen::Settings => {
                    crate::ui::screens::settings::draw(ui, &self.settings, &self.library, &mut self.toasts, &mut self.screen);
                }
            }
        });

        self.toasts.show(ctx);
    }
}
