use crate::data::{Library, LibraryStatus};
use crate::domain::Screen;
use crate::playback::PlaybackController;
use crate::settings::Settings;
use crate::ui::screens::albums::AlbumsCache;
use crate::ui::screens::all_songs::AllSongsState;
use crate::ui::screens::artists::ArtistsCache;
use crate::ui::screens::folders::FoldersState;
use crate::ui::screens::queue::QueueState;
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
    pub queue_state: QueueState,
    pub albums_cache: AlbumsCache,
    pub artists_cache: ArtistsCache,
}

impl App {
    pub fn new(
        cc: &eframe::CreationContext<'_>,
        library: Arc<Library>,
        playback: Arc<PlaybackController>,
        settings: Arc<RwLock<Settings>>,
    ) -> Self {
        crate::ui::fonts::install(&cc.egui_ctx);
        crate::ui::theme::install(&cc.egui_ctx);
        Self {
            library,
            playback,
            settings,
            screen: Screen::AllSongs,
            toasts: Toasts::new(),
            all_songs_state: AllSongsState::default(),
            folders_state: FoldersState::default(),
            queue_state: QueueState::default(),
            albums_cache: AlbumsCache::default(),
            artists_cache: ArtistsCache::default(),
        }
    }

    fn handle_shortcuts(&self, ctx: &egui::Context) {
        if ctx.wants_keyboard_input() {
            return;
        }
        ctx.input(|i| {
            if i.key_pressed(egui::Key::Space) {
                self.playback.play_pause();
            }
            if i.modifiers.command_only() {
                if i.key_pressed(egui::Key::ArrowRight) {
                    self.playback.next();
                }
                if i.key_pressed(egui::Key::ArrowLeft) {
                    self.playback.previous();
                }
            }
        });
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.handle_shortcuts(ctx);

        // Snapshot playback once per frame — Now Playing and the mini player
        // both need it, and a fresh read-lock per consumer is wasteful.
        let pb_snapshot = self.playback.snapshot();
        let scanning = self.library.status() == LibraryStatus::Scanning;

        // Only request a delayed repaint when something is actually changing:
        // playback running, library still indexing, or a transient toast on
        // screen. Idle UI lets egui go fully event-driven.
        let needs_tick = pb_snapshot.is_playing || scanning || self.toasts.has_active();
        if needs_tick {
            ctx.request_repaint_after(std::time::Duration::from_millis(80));
        }

        if self.screen.shows_chrome() {
            egui::TopBottomPanel::top("top_bar").show(ctx, |ui| {
                let count = self.library.len();
                crate::ui::components::top_bar::draw(
                    ui,
                    &mut self.screen,
                    count,
                    &mut self.all_songs_state.query,
                );
            });
        }

        if scanning {
            egui::TopBottomPanel::top("status_bar").show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.spinner();
                    ui.label(format!(
                        "Scanning library\u{2026} ({} songs so far)",
                        self.library.len()
                    ));
                });
            });
        }

        // Mini player hides on the Now Playing screen — that screen already
        // shows full transport, so two would just be duplicative.
        //
        // Notes on layout: the panel uses `exact_height` (not `min_height`)
        // and an explicit `Frame::none()` so the central panel above can't
        // bleed into it. A flexible min_height let a tall scroll area's
        // content visually overshadow the bar at the bottom of the window.
        if !matches!(self.screen, Screen::NowPlaying) {
            const MINI_HEIGHT: f32 = 72.0;
            let mini_frame = egui::Frame::none()
                .fill(crate::ui::theme::BG_ELEV)
                .stroke(egui::Stroke::NONE)
                .inner_margin(egui::Margin::symmetric(20.0, 10.0));
            egui::TopBottomPanel::bottom("mini_player")
                .resizable(false)
                .exact_height(MINI_HEIGHT)
                .frame(mini_frame)
                .show_separator_line(false)
                .show(ctx, |ui| {
                    // Hairline separator drawn manually at the top edge so
                    // it sits inside the panel, not on the boundary where
                    // egui's default separator can flicker against scroll
                    // content above.
                    let top = ui.max_rect().min.y;
                    let left = ui.max_rect().min.x;
                    let right = ui.max_rect().max.x;
                    ui.painter().hline(
                        left..=right,
                        top,
                        egui::Stroke::new(1.0, crate::ui::theme::BORDER_SOFT),
                    );
                    crate::ui::components::mini_player::draw(
                        ui,
                        &pb_snapshot,
                        &self.playback,
                        &mut self.screen,
                    );
                });
        }

        egui::CentralPanel::default().show(ctx, |ui| match self.screen.clone() {
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
                crate::ui::screens::albums::draw_list(
                    ui,
                    &self.library,
                    &mut self.albums_cache,
                    &mut self.screen,
                );
            }
            Screen::AlbumDetail(name) => {
                crate::ui::screens::albums::draw_detail(
                    ui,
                    &self.library,
                    &self.playback,
                    &mut self.albums_cache,
                    &name,
                    &mut self.screen,
                );
            }
            Screen::ArtistsList => {
                crate::ui::screens::artists::draw_list(
                    ui,
                    &self.library,
                    &mut self.artists_cache,
                    &mut self.screen,
                );
            }
            Screen::ArtistDetail(name) => {
                crate::ui::screens::artists::draw_detail(
                    ui,
                    &self.library,
                    &self.playback,
                    &mut self.artists_cache,
                    &name,
                    &mut self.screen,
                );
            }
            Screen::Folders => {
                crate::ui::screens::folders::draw(
                    ui,
                    &self.library,
                    &self.playback,
                    &mut self.folders_state,
                    &mut self.screen,
                );
            }
            Screen::Queue => {
                crate::ui::screens::queue::draw(
                    ui,
                    &self.playback,
                    &mut self.queue_state,
                    &mut self.screen,
                );
            }
            Screen::NowPlaying => {
                crate::ui::screens::now_playing::draw_with_state(
                    ui,
                    &pb_snapshot,
                    &self.playback,
                    &mut self.screen,
                );
            }
            Screen::Equalizer => {
                crate::ui::screens::equalizer::draw(
                    ui,
                    &self.settings,
                    &mut self.toasts,
                    &mut self.screen,
                );
            }
            Screen::Settings => {
                crate::ui::screens::settings::draw(
                    ui,
                    &self.settings,
                    &self.library,
                    &mut self.toasts,
                    &mut self.screen,
                );
            }
        });

        self.toasts.show(ctx);
    }
}

impl Drop for App {
    fn drop(&mut self) {
        // Best-effort graceful shutdown. Runs after eframe's loop exits and
        // before the controller / engine threads tear down.
        // 1. Persist the current volume (the only setting the user can
        //    change outside the Settings screen) and any in-flight edits.
        let snap = self.playback.snapshot();
        {
            let mut s = self.settings.write();
            s.playback.volume = snap.volume;
            s.playback.shuffle = snap.shuffle_enabled;
            s.playback.repeat = snap.repeat_mode;
        }
        if let Err(e) = self.settings.read().save() {
            log::warn!("on-exit settings save failed: {e}");
        }
        // 2. Persist last-played with the current playback position so a
        //    fresh launch can resume mid-track.
        if let Some(song) = &snap.current_song {
            let _ = crate::last_played::save(&crate::last_played::LastPlayed {
                path: song.path.clone(),
                position_ms: snap.current_position_ms,
            });
        }
        // 3. The playback layer currently exposes only a best-effort
        //    shutdown signal, not a join/ack API. Keep UI-owned persistence
        //    above this point so exit remains robust within the UI boundary.
        self.playback.shutdown();
    }
}
