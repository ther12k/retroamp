//! Main Winamp Application with window snapping, keyboard shortcuts,
//! audio playback coordination, and skin switching.

use crate::audio::{AudioEngine, PlaybackState};
use crate::config::AppConfig;
use crate::skin::{Skin, MAIN_WIDTH, MAIN_HEIGHT, EQ_WIDTH, EQ_HEIGHT, PL_DEFAULT_WIDTH};
use crate::ui::classic_vector::{render_vector_eq, render_vector_main, render_vector_playlist};
use crate::ui::cyberpunk::{render_cyberpunk_eq, render_cyberpunk_main, render_cyberpunk_playlist};
use crate::ui::eq_window::{render_eq_window, EqAction, EqWindowState};
use crate::ui::main_window::{render_main_window, MainAction, MainWindowState, VisMode};
use crate::ui::modern_hifi::{render_hifi_eq, render_hifi_main, render_hifi_playlist};
use crate::ui::playlist_window::{render_playlist_window, PlaylistAction, PlaylistEntry, PlaylistWindowState};
use crate::ui::textures::SkinTextures;
use eframe::App;
use egui::{Color32, Context, Key};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum UiTheme {
    /// Vector re-creation of the classic design — crisp at any zoom. Default.
    WinampRedrawn,
    /// The original bitmap skin (best at exactly 1x / 2x zoom).
    ClassicWinamp,
    ModernHifi,
    CyberpunkTerminal,
}

impl UiTheme {
    pub fn as_str(self) -> &'static str {
        match self {
            UiTheme::WinampRedrawn => "redrawn",
            UiTheme::ClassicWinamp => "classic",
            UiTheme::ModernHifi => "hifi",
            UiTheme::CyberpunkTerminal => "cyberpunk",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s {
            "redrawn" | "vector" | "winamp" => UiTheme::WinampRedrawn,
            "hifi" | "studio" => UiTheme::ModernHifi,
            "cyberpunk" | "terminal" => UiTheme::CyberpunkTerminal,
            _ => UiTheme::ClassicWinamp,
        }
    }
}

pub struct WinampApp {
    pub skin: Skin,
    pub textures: Option<SkinTextures>,
    pub audio: AudioEngine,
    pub main_state: MainWindowState,
    pub eq_state: EqWindowState,
    pub playlist_state: PlaylistWindowState,
    pub current_track_index: Option<usize>,
    pub snap_distance: f32,
    pub eq_docked: bool,
    pub pl_docked: bool,
    /// UI scale (Winamp-style "double size"): 1.0 = normal, 2.0 = double.
    /// Also settable at launch with the WINAMP_SCALE env var (e.g. =2).
    pub ui_scale: f32,
    pub show_skin_picker: bool,
    pub show_about: bool,
    pub active_theme: UiTheme,
    /// Last-saved configuration (change detection + persistence).
    pub config_cache: AppConfig,
    pub config_dirty: bool,
    pub last_config_save: Instant,
}

/// Colors so popup dialogs blend with the active theme's look.
#[derive(Clone, Copy)]
pub struct MenuPalette {
    pub frame_fill: Color32,
    pub frame_stroke: Color32,
    pub section: Color32,
    pub text: Color32,
    pub hover_bg: Color32,
    pub hover_text: Color32,
    pub accent: Color32,
}

impl MenuPalette {
    pub fn for_theme(theme: UiTheme) -> Self {
        match theme {
            // 1998-authentic: classic Windows 95 popup menu over the retro skin.
            UiTheme::WinampRedrawn | UiTheme::ClassicWinamp => Self {
                frame_fill: Color32::from_rgb(192, 192, 192),
                frame_stroke: Color32::BLACK,
                section: Color32::from_rgb(90, 90, 90),
                text: Color32::BLACK,
                hover_bg: Color32::from_rgb(0, 0, 128),
                hover_text: Color32::WHITE,
                accent: Color32::from_rgb(0, 0, 128),
            },
            // OLED glass with cyan highlight.
            UiTheme::ModernHifi => Self {
                frame_fill: Color32::from_rgb(12, 14, 18),
                frame_stroke: Color32::from_rgb(60, 70, 90),
                section: Color32::from_rgb(100, 110, 130),
                text: Color32::from_rgb(225, 230, 240),
                hover_bg: Color32::from_rgb(0, 240, 255),
                hover_text: Color32::from_rgb(0, 10, 16),
                accent: Color32::from_rgb(0, 200, 255),
            },
            // CRT phosphor terminal.
            UiTheme::CyberpunkTerminal => Self {
                frame_fill: Color32::from_rgb(2, 10, 5),
                frame_stroke: Color32::from_rgb(0, 180, 80),
                section: Color32::from_rgb(0, 150, 60),
                text: Color32::from_rgb(0, 255, 100),
                hover_bg: Color32::from_rgb(0, 255, 100),
                hover_text: Color32::from_rgb(0, 12, 5),
                accent: Color32::from_rgb(0, 255, 100),
            },
        }
    }
}

/// One selectable menu row, painted in the theme palette.
fn menu_item(ui: &mut egui::Ui, p: &MenuPalette, label: &str, checked: bool) -> bool {
    let width = ui.available_width();
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(width, 16.0), egui::Sense::click());
    if resp.hovered() || resp.is_pointer_button_down_on() {
        ui.painter().rect_filled(rect, 2.0, p.hover_bg);
    }
    let text = if checked { format!("✓  {label}") } else { format!("    {label}") };
    ui.painter().text(
        rect.min + egui::vec2(8.0, rect.height() / 2.0),
        egui::Align2::LEFT_CENTER,
        &text,
        egui::FontId::new(10.5, egui::FontFamily::Proportional),
        if resp.hovered() { p.hover_text } else { p.text },
    );
    resp.clicked()
}

/// A dim section caption (FILE / PLAYBACK / VIEW / ...).
fn menu_section(ui: &mut egui::Ui, p: &MenuPalette, title: &str) {
    ui.add_space(3.0);
    ui.painter().text(
        ui.cursor().min + egui::vec2(8.0, 6.0),
        egui::Align2::LEFT_CENTER,
        title,
        egui::FontId::new(8.5, egui::FontFamily::Proportional),
        p.section,
    );
    ui.add_space(13.0);
}

impl WinampApp {
    pub fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        let _skin = Skin::load_default();
        let mut audio = AudioEngine::new();
        let mut playlist_state = PlaylistWindowState::default();

        // Check if sample tracks exist in /mnt/data/The WIRED CD - Rip. Sample. Mash. Share
        let sample_dir = Path::new("/mnt/data/The WIRED CD - Rip. Sample. Mash. Share");
        if sample_dir.is_dir() {
            if let Ok(entries) = std::fs::read_dir(sample_dir) {
                let mut paths: Vec<PathBuf> = entries
                    .flatten()
                    .map(|e| e.path())
                    .filter(|p| {
                        p.extension()
                            .and_then(|ext| ext.to_str())
                            .map(|ext| ext.eq_ignore_ascii_case("mp3") || ext.eq_ignore_ascii_case("wav") || ext.eq_ignore_ascii_case("ogg") || ext.eq_ignore_ascii_case("flac"))
                            .unwrap_or(false)
                    })
                    .collect();
                paths.sort();

                for path in paths {
                    let title = path
                        .file_stem()
                        .map(|s| s.to_string_lossy().to_string())
                        .unwrap_or_else(|| "Track".to_string());
                    playlist_state.items.push(PlaylistEntry {
                        title,
                        path,
                        duration_secs: 180,
                    });
                }
            }
        }

        // Load persisted configuration, then let env vars override at launch.
        let cfg = AppConfig::load();

        // Theme: env (RETROAMP_SKIN) wins over saved config.
        let mut initial_theme = UiTheme::from_str(&cfg.theme);
        if let Ok(skin_name) = std::env::var("RETROAMP_SKIN").or_else(|_| std::env::var("WINAMP_SKIN")) {
            initial_theme = match skin_name.to_lowercase().as_str() {
                "hifi" | "studio" | "silver" => UiTheme::ModernHifi,
                "cyberpunk" | "terminal" | "matrix" => UiTheme::CyberpunkTerminal,
                "nord" | "frost" | "catppuccin" => UiTheme::ClassicWinamp, // nord uses the classic renderer
                _ => initial_theme,
            };
        }
        let initial_skin = match initial_theme {
            UiTheme::ModernHifi => Skin::load_wsz("assets/skins/hifi_studio.wsz").unwrap_or_else(|_| Skin::load_default()),
            UiTheme::CyberpunkTerminal => Skin::load_wsz("assets/skins/cyberpunk.wsz").unwrap_or_else(|_| Skin::load_default()),
            UiTheme::WinampRedrawn | UiTheme::ClassicWinamp => Skin::load_default(),
        };

        // Scale: env override, else saved config.
        let ui_scale = std::env::var("RETROAMP_SCALE")
            .or_else(|_| std::env::var("WINAMP_SCALE"))
            .ok()
            .and_then(|s| s.trim().parse::<f32>().ok())
            .map(|s| s.clamp(1.0, 3.0))
            .unwrap_or(cfg.ui_scale.clamp(1.0, 3.0));

        // Apply persisted playlist size (13px rows + 58px chrome, classic metric).
        playlist_state.height = 58.0 + cfg.playlist_rows.clamp(2.0, 64.0) * 13.0;

        // Apply persisted playback/UX toggles.
        audio.set_volume(cfg.volume.clamp(0.0, 1.0));
        let vis_mode = if cfg.vis_spectrum { VisMode::Spectrum } else { VisMode::Oscilloscope };
        let mut main_state = MainWindowState::default();
        main_state.shuffle = cfg.shuffle;
        main_state.repeat = cfg.repeat;
        main_state.show_eq = cfg.show_eq;
        main_state.show_playlist = cfg.show_playlist;
        main_state.vis_mode = vis_mode;

        // Config cache reflects *applied* values (env overrides included) so we
        // don't immediately flag the just-loaded config as dirty.
        let config_cache = AppConfig {
            ui_scale,
            theme: initial_theme.as_str().to_string(),
            playlist_rows: cfg.playlist_rows.clamp(2.0, 64.0),
            volume: cfg.volume.clamp(0.0, 1.0),
            shuffle: cfg.shuffle,
            repeat: cfg.repeat,
            show_eq: cfg.show_eq,
            show_playlist: cfg.show_playlist,
            vis_spectrum: cfg.vis_spectrum,
        };

        Self {
            skin: initial_skin,
            textures: None,
            audio,
            main_state,
            eq_state: EqWindowState::default(),
            playlist_state,
            current_track_index: None,
            snap_distance: 12.0,
            eq_docked: true,
            pl_docked: true,
            ui_scale,
            show_skin_picker: false,
            show_about: false,
            active_theme: initial_theme,
            config_cache,
            config_dirty: false,
            last_config_save: Instant::now(),
        }
    }

    /// Snapshot of every user-adjustable setting that gets persisted.
    fn config_snapshot(&self) -> AppConfig {
        AppConfig {
            ui_scale: self.ui_scale,
            theme: self.active_theme.as_str().to_string(),
            playlist_rows: ((self.playlist_state.height - 58.0) / 13.0).round().clamp(2.0, 64.0),
            volume: self.audio.volume,
            shuffle: self.main_state.shuffle,
            repeat: self.main_state.repeat,
            show_eq: self.main_state.show_eq,
            show_playlist: self.main_state.show_playlist,
            vis_spectrum: self.main_state.vis_mode == VisMode::Spectrum,
        }
    }

    pub fn play_track_index(&mut self, index: usize) {
        if index < self.playlist_state.items.len() {
            let path = self.playlist_state.items[index].path.clone();
            match self.audio.play_file(&path) {
                Ok(()) => {
                    self.current_track_index = Some(index);
                    if let Some(ref track) = self.audio.current_track {
                        self.playlist_state.items[index].duration_secs = track.duration.as_secs();
                    }
                }
                Err(e) => {
                    log::error!("Error playing audio file {}: {e}", path.display());
                }
            }
        }
    }

    pub fn play_next(&mut self) {
        if self.playlist_state.items.is_empty() {
            return;
        }

        let next_idx = if self.main_state.shuffle {
            fastrand::usize(..self.playlist_state.items.len())
        } else if let Some(curr) = self.current_track_index {
            (curr + 1) % self.playlist_state.items.len()
        } else {
            0
        };

        self.play_track_index(next_idx);
    }

    pub fn play_prev(&mut self) {
        if self.playlist_state.items.is_empty() {
            return;
        }

        let prev_idx = if let Some(curr) = self.current_track_index {
            if curr > 0 { curr - 1 } else { self.playlist_state.items.len() - 1 }
        } else {
            0
        };

        self.play_track_index(prev_idx);
    }

    pub fn open_file_dialog(&mut self) {
        if let Some(paths) = rfd::FileDialog::new()
            .add_filter("Audio & Skins", &["mp3", "flac", "wav", "ogg", "aac", "wsz", "zip"])
            .set_directory("/mnt/data")
            .pick_files()
        {
            for path in paths {
                self.handle_opened_file(path);
            }
        }
    }

    pub fn handle_opened_file(&mut self, path: PathBuf) {
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();

        if ext == "wsz" || ext == "zip" {
            // Load custom Winamp skin
            match Skin::load_wsz(&path) {
                Ok(new_skin) => {
                    self.skin = new_skin;
                    if let Some(ref mut textures) = self.textures {
                        textures.update(&self.skin);
                    }
                    log::info!("Loaded skin: {}", self.skin.name);
                }
                Err(e) => {
                    log::error!("Failed to load skin {}: {e}", path.display());
                }
            }
        } else {
            // Add track to playlist
            let title = path
                .file_stem()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_else(|| "Track".to_string());

            let new_idx = self.playlist_state.items.len();
            self.playlist_state.items.push(PlaylistEntry {
                title,
                path: path.clone(),
                duration_secs: 0,
            });

            // If stopped, start playing this track
            if self.audio.state == PlaybackState::Stopped {
                self.play_track_index(new_idx);
            }
        }
    }

    /// Snap windows together and track bidirectional magnetic docking status
    fn apply_window_docking(&mut self) {
        let main_top = self.main_state.position.y;
        let main_bottom = self.main_state.position.y + MAIN_HEIGHT;
        let main_left = self.main_state.position.x;
        let main_right = self.main_state.position.x + MAIN_WIDTH;

        // 1. Equalizer Window Docking
        if self.main_state.show_eq {
            // Snapping to Bottom of Main
            let near_main_bottom = (self.eq_state.position.y - main_bottom).abs() < self.snap_distance;
            let near_main_top = (self.eq_state.position.y + EQ_HEIGHT - main_top).abs() < self.snap_distance;
            let near_main_left = (self.eq_state.position.x - main_left).abs() < self.snap_distance;
            let near_main_right = (self.eq_state.position.x - main_right).abs() < self.snap_distance;

            if near_main_bottom && near_main_left {
                self.eq_state.position.x = main_left;
                self.eq_state.position.y = main_bottom;
                self.eq_docked = true;
            } else if near_main_top && near_main_left {
                self.eq_state.position.x = main_left;
                self.eq_state.position.y = main_top - EQ_HEIGHT;
                self.eq_docked = true;
            } else if near_main_right && (self.eq_state.position.y - main_top).abs() < self.snap_distance {
                self.eq_state.position.x = main_right;
                self.eq_state.position.y = main_top;
                self.eq_docked = true;
            } else if self.eq_docked {
                // Keep sticky alignment unless user dragged significantly away
                let dist_x = (self.eq_state.position.x - main_left).abs();
                let dist_y = (self.eq_state.position.y - main_bottom).abs();
                if dist_x < self.snap_distance * 1.5 && dist_y < self.snap_distance * 1.5 {
                    self.eq_state.position.x = main_left;
                    self.eq_state.position.y = main_bottom;
                } else {
                    self.eq_docked = false;
                }
            } else {
                self.eq_docked = false;
            }
        } else {
            self.eq_docked = false;
        }

        // 2. Playlist Window Docking
        if self.main_state.show_playlist {
            // Target can be either bottom of EQ (if EQ is docked below Main) or bottom of Main
            let eq_bottom = self.eq_state.position.y + EQ_HEIGHT;
            let eq_left = self.eq_state.position.x;

            let target_top = if self.main_state.show_eq && self.eq_docked {
                eq_bottom
            } else {
                main_bottom
            };
            let target_left = if self.main_state.show_eq && self.eq_docked {
                eq_left
            } else {
                main_left
            };

            let near_target_top = (self.playlist_state.position.y - target_top).abs() < self.snap_distance;
            let near_target_left = (self.playlist_state.position.x - target_left).abs() < self.snap_distance;

            // Also check docking directly below Main if EQ is undocked/elsewhere
            let near_main_bottom = (self.playlist_state.position.y - main_bottom).abs() < self.snap_distance;
            let near_main_left = (self.playlist_state.position.x - main_left).abs() < self.snap_distance;

            // Also check docking to the right of Main
            let near_main_right = (self.playlist_state.position.x - main_right).abs() < self.snap_distance;
            let near_main_top_align = (self.playlist_state.position.y - main_top).abs() < self.snap_distance;

            if near_target_top && near_target_left {
                self.playlist_state.position.x = target_left;
                self.playlist_state.position.y = target_top;
                self.pl_docked = true;
            } else if near_main_bottom && near_main_left {
                self.playlist_state.position.x = main_left;
                self.playlist_state.position.y = main_bottom;
                self.pl_docked = true;
            } else if near_main_right && near_main_top_align {
                self.playlist_state.position.x = main_right;
                self.playlist_state.position.y = main_top;
                self.pl_docked = true;
            } else if self.pl_docked {
                let dist_x = (self.playlist_state.position.x - target_left).abs();
                let dist_y = (self.playlist_state.position.y - target_top).abs();
                if dist_x < self.snap_distance * 1.5 && dist_y < self.snap_distance * 1.5 {
                    self.playlist_state.position.x = target_left;
                    self.playlist_state.position.y = target_top;
                } else {
                    self.pl_docked = false;
                }
            } else {
                self.pl_docked = false;
            }
        } else {
            self.pl_docked = false;
        }
    }

    /// Handle Winamp keyboard shortcuts
    fn handle_keyboard_shortcuts(&mut self, ctx: &Context) {
        let mut do_prev = false;
        let mut do_play = false;
        let mut do_pause = false;
        let mut do_stop = false;
        let mut do_next = false;
        let mut do_open = false;
        let mut vol_delta = 0.0f32;
        let mut zoom_action: Option<f32> = None;

        ctx.input(|i| {
            if i.key_pressed(Key::Z) { do_prev = true; }
            if i.key_pressed(Key::X) { do_play = true; }
            if i.key_pressed(Key::C) { do_pause = true; }
            if i.key_pressed(Key::V) { do_stop = true; }
            if i.key_pressed(Key::B) { do_next = true; }
            if i.key_pressed(Key::L) { do_open = true; }
            if i.key_pressed(Key::S) && !i.modifiers.ctrl { self.show_skin_picker = !self.show_skin_picker; }
            if i.key_pressed(Key::ArrowUp) { vol_delta += 0.05; }
            if i.key_pressed(Key::ArrowDown) { vol_delta -= 0.05; }

            // Winamp-style scaling: Ctrl+D = double size toggle,
            // Ctrl+= / Ctrl+- = fine steps, Ctrl+0 = reset.
            if i.modifiers.ctrl && !i.modifiers.shift {
                if i.key_pressed(Key::D) {
                    zoom_action = Some(if self.ui_scale >= 1.99 { 1.0 } else { 2.0 });
                } else if i.key_pressed(Key::Plus) || i.key_pressed(Key::Equals) {
                    zoom_action = Some((self.ui_scale + 0.25).min(3.0));
                } else if i.key_pressed(Key::Minus) {
                    zoom_action = Some((self.ui_scale - 0.25).max(1.0));
                } else if i.key_pressed(Key::Num0) {
                    zoom_action = Some(1.0);
                }
            }
        });

        if let Some(new_scale) = zoom_action {
            self.ui_scale = new_scale;
        }
        if do_prev { self.play_prev(); }
        if do_play {
            if let Some(idx) = self.current_track_index {
                self.play_track_index(idx);
            } else if !self.playlist_state.items.is_empty() {
                self.play_track_index(0);
            }
        }
        if do_pause {
            if self.audio.state == PlaybackState::Playing {
                self.audio.pause();
            } else if self.audio.state == PlaybackState::Paused {
                self.audio.resume();
            }
        }
        if do_stop { self.audio.stop(); }
        if do_next { self.play_next(); }
        if do_open { self.open_file_dialog(); }
        if vol_delta != 0.0 {
            let new_vol = (self.audio.volume + vol_delta).clamp(0.0, 1.0);
            self.audio.set_volume(new_vol);
        }
    }
}

impl App for WinampApp {
    /// Transparent clear color: pixels not covered by the skinned windows are
    /// see-through to the desktop (requires `with_transparent(true)`).
    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        Color32::TRANSPARENT.to_normalized_gamma_f32()
    }

    /// Save the final configuration on graceful shutdown.
    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        self.config_snapshot().save();
    }

    fn update(&mut self, ctx: &Context, _frame: &mut eframe::Frame) {
        ctx.request_repaint();

        // Apply UI scale (auto-fit below works in points, so it composes
        // with the zoom: the window grows physically as the skin grows).
        ctx.set_zoom_factor(self.ui_scale);

        // Initialize textures on first frame
        if self.textures.is_none() {
            self.textures = Some(SkinTextures::new(ctx, &self.skin));
        }

        // Update audio engine (positions, FFT calculation)
        let track_finished = self.audio.update();
        if track_finished {
            if self.main_state.repeat {
                if let Some(idx) = self.current_track_index {
                    self.play_track_index(idx);
                }
            } else {
                self.play_next();
            }
        }

        // Sync DSP Equalizer settings
        self.audio.update_eq(self.audio.eq);

        // Process seek bar requests
        if let Some(ratio) = self.main_state.seek_target_ratio.take() {
            if let Some(ref track) = self.audio.current_track {
                if track.duration > Duration::ZERO {
                    let target_secs = track.duration.as_secs_f64() * (ratio as f64);
                    self.audio.seek(Duration::from_secs_f64(target_secs));
                }
            }
        }

        // Keyboard shortcuts
        self.handle_keyboard_shortcuts(ctx);

        // Handle dropped files (drag & drop)
        let dropped_paths: Vec<PathBuf> = ctx.input(|i| {
            i.raw.dropped_files.iter().filter_map(|f| f.path.clone()).collect()
        });
        for path in dropped_paths {
            self.handle_opened_file(path);
        }

        let old_eq_pos = self.eq_state.position;
        let textures = self.textures.take().unwrap();

        let mut main_action = MainAction::None;
        let mut eq_action = EqAction::None;
        let mut pl_action = PlaylistAction::None;

        // Root container: no fill — the skin panels float directly on the
        // desktop (transparent window), like real Winamp.
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE)
            .show(ctx, |ui| {
                match self.active_theme {
                    UiTheme::WinampRedrawn => {
                        main_action = render_vector_main(ui, &mut self.main_state, &mut self.audio);
                        if self.main_state.show_eq {
                            eq_action = render_vector_eq(ui, &mut self.eq_state, &mut self.audio);
                        }
                        if self.main_state.show_playlist {
                            pl_action = render_vector_playlist(ui, &mut self.playlist_state, self.current_track_index, &self.audio);
                        }
                    }
                    UiTheme::ModernHifi => {
                        main_action = render_hifi_main(ui, &mut self.main_state, &mut self.audio);
                        if self.main_state.show_eq {
                            eq_action = render_hifi_eq(ui, &mut self.eq_state, &mut self.audio);
                        }
                        if self.main_state.show_playlist {
                            pl_action = render_hifi_playlist(ui, &mut self.playlist_state, self.current_track_index, &self.audio);
                        }
                    }
                    UiTheme::CyberpunkTerminal => {
                        main_action = render_cyberpunk_main(ui, &mut self.main_state, &mut self.audio);
                        if self.main_state.show_eq {
                            eq_action = render_cyberpunk_eq(ui, &mut self.eq_state, &mut self.audio);
                        }
                        if self.main_state.show_playlist {
                            pl_action = render_cyberpunk_playlist(ui, &mut self.playlist_state, self.current_track_index, &self.audio);
                        }
                    }
                    UiTheme::ClassicWinamp => {
                        // 1. Render Main Window
                        main_action = render_main_window(
                            ui,
                            &mut self.main_state,
                            &mut self.audio,
                            &self.skin,
                            &textures,
                        );

                        // 2. Render Equalizer Window (if visible)
                        if self.main_state.show_eq {
                            eq_action = render_eq_window(
                                ui,
                                &mut self.eq_state,
                                &mut self.audio,
                                &self.skin,
                                &textures,
                            );
                        }

                        // 3. Render Playlist Window (if visible)
                        if self.main_state.show_playlist {
                            pl_action = render_playlist_window(
                                ui,
                                &mut self.playlist_state,
                                self.current_track_index,
                                &self.audio,
                                &self.skin,
                                &textures,
                            );
                        }
                    }
                }
            });

        // Restore textures
        self.textures = Some(textures);

        // Sticky window movement: if Equalizer was moved and Playlist is docked to it, carry Playlist along
        let eq_delta = self.eq_state.position - old_eq_pos;
        if eq_delta != egui::Vec2::ZERO && self.pl_docked && self.eq_docked {
            self.playlist_state.position += eq_delta;
        }

        // Apply magnetic snapping / sticky docking
        self.apply_window_docking();

        // --- Borderless (skin-chrome) window management ----------------------
        // 1) Dragging the main skinned titlebar hands the move off to the
        //    window manager (native interactive move — perfectly smooth,
        //    no per-frame position feedback lag). Docked panels travel with
        //    the window because they are glued to the stack.
        if self.main_state.os_drag_started {
            self.main_state.os_drag_started = false;
            ctx.send_viewport_cmd(egui::ViewportCommand::StartDrag);
        }

        // 2) Keep panels inside the window.
        let min_pos = egui::Pos2::new(10.0, 10.0);
        self.main_state.position = self.main_state.position.max(min_pos);
        if self.main_state.show_eq {
            self.eq_state.position = self.eq_state.position.max(min_pos);
        }
        if self.main_state.show_playlist {
            self.playlist_state.position = self.playlist_state.position.max(min_pos);
        }

        // 3) Auto-fit the borderless window to the visible stack so no
        //    empty background shows (like real Winamp's fixed-size windows).
        let mut w = self.main_state.position.x + MAIN_WIDTH;
        let mut h = self.main_state.position.y + MAIN_HEIGHT;
        if self.main_state.show_eq {
            w = w.max(self.eq_state.position.x + EQ_WIDTH);
            h = h.max(self.eq_state.position.y + EQ_HEIGHT);
        }
        if self.main_state.show_playlist {
            w = w.max(self.playlist_state.position.x + PL_DEFAULT_WIDTH);
            h = h.max(self.playlist_state.position.y + self.playlist_state.height);
        }
        let needed = egui::vec2(w + 10.0, h + 10.0);
        let current = ctx.input(|i| i.screen_rect().size());
        if (needed.x - current.x).abs() > 0.5 || (needed.y - current.y).abs() > 0.5 {
            ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(needed));
        }

        // --- RetroAmp Main Menu (classic Winamp-style) ------------------------
        if self.main_state.menu_open {
            self.main_state.menu_frames += 1;
            let mut menu_action: Option<MainAction> = None;
            let mut close_menu = false;
            let mut menu_zoom: Option<f32> = None;
            let mut menu_theme: Option<UiTheme> = None;
            let p = MenuPalette::for_theme(self.active_theme);

            let menu_pos = self.main_state.position + egui::vec2(8.0, 20.0);
            let menu_win = egui::Window::new("retroamp_menu")
                .id(egui::Id::new("retroamp-main-menu"))
                .title_bar(false)
                .resizable(false)
                .collapsible(false)
                .fixed_pos(menu_pos)
                .frame(
                    egui::Frame::window(&ctx.style())
                        .fill(p.frame_fill)
                        .stroke(egui::Stroke::new(1.0_f32, p.frame_stroke))
                        .inner_margin(egui::Margin::symmetric(4, 4)),
                )
                .show(ctx, |ui| {
                    ui.set_min_width(210.0);
                    ui.set_max_width(210.0);

                    menu_section(ui, &p, "FILE");
                    if menu_item(ui, &p, "Open File(s)…   (L)", false) { menu_action = Some(MainAction::Eject); close_menu = true; }
                    if menu_item(ui, &p, "Open Skin / Themes…   (S)", false) { menu_action = Some(MainAction::LoadSkin); close_menu = true; }
                    if menu_item(ui, &p, "Exit", false) { menu_action = Some(MainAction::Close); close_menu = true; }

                    ui.separator();
                    menu_section(ui, &p, "PLAYBACK");
                    if menu_item(ui, &p, "Previous   (Z)", false) { menu_action = Some(MainAction::Prev); close_menu = true; }
                    if menu_item(ui, &p, "Play   (X)", false) { menu_action = Some(MainAction::Play); close_menu = true; }
                    if menu_item(ui, &p, "Pause / Resume   (C)", false) { menu_action = Some(MainAction::Pause); close_menu = true; }
                    if menu_item(ui, &p, "Stop   (V)", false) { menu_action = Some(MainAction::Stop); close_menu = true; }
                    if menu_item(ui, &p, "Next   (B)", false) { menu_action = Some(MainAction::Next); close_menu = true; }
                    if menu_item(ui, &p, "Shuffle", self.main_state.shuffle) {
                        self.main_state.shuffle = !self.main_state.shuffle;
                        close_menu = true;
                    }
                    if menu_item(ui, &p, "Repeat", self.main_state.repeat) {
                        self.main_state.repeat = !self.main_state.repeat;
                        close_menu = true;
                    }

                    ui.separator();
                    menu_section(ui, &p, "VIEW");
                    if menu_item(ui, &p, "Equalizer", self.main_state.show_eq) {
                        self.main_state.show_eq = !self.main_state.show_eq;
                        close_menu = true;
                    }
                    if menu_item(ui, &p, "Playlist Editor", self.main_state.show_playlist) {
                        self.main_state.show_playlist = !self.main_state.show_playlist;
                        close_menu = true;
                    }
                    for (label, val) in [("Scale 100%", 1.0), ("Scale 150%", 1.5), ("Scale 200%", 2.0), ("Scale 300%", 3.0)] {
                        if menu_item(ui, &p, label, (self.ui_scale - val).abs() < 0.01) {
                            menu_zoom = Some(val);
                            close_menu = true;
                        }
                    }

                    ui.separator();
                    menu_section(ui, &p, "THEMES");
                    if menu_item(ui, &p, "Winamp (Redrawn)", self.active_theme == UiTheme::WinampRedrawn) {
                        menu_theme = Some(UiTheme::WinampRedrawn);
                        close_menu = true;
                    }
                    if menu_item(ui, &p, "Winamp (Bitmap 1:1)", self.active_theme == UiTheme::ClassicWinamp) {
                        menu_theme = Some(UiTheme::ClassicWinamp);
                        close_menu = true;
                    }
                    if menu_item(ui, &p, "Hi-Fi Studio", self.active_theme == UiTheme::ModernHifi) {
                        menu_theme = Some(UiTheme::ModernHifi);
                        close_menu = true;
                    }
                    if menu_item(ui, &p, "Cyberpunk Terminal", self.active_theme == UiTheme::CyberpunkTerminal) {
                        menu_theme = Some(UiTheme::CyberpunkTerminal);
                        close_menu = true;
                    }

                    ui.separator();
                    menu_section(ui, &p, "HELP");
                    if menu_item(ui, &p, "About RetroAmp…", false) {
                        self.show_about = true;
                        close_menu = true;
                    }
                });

            // Close on click outside (after the opening click has settled)
            if let Some(r) = &menu_win {
                if r.response.clicked_elsewhere() && self.main_state.menu_frames > 2 {
                    close_menu = true;
                }
            }
            if ctx.input(|i| i.key_pressed(Key::Escape)) {
                close_menu = true;
            }

            if close_menu {
                self.main_state.menu_open = false;
            }
            if let Some(z) = menu_zoom {
                self.ui_scale = z;
            }
            if let Some(theme) = menu_theme {
                self.active_theme = theme;
                // Load matching skin assets for the classic engine
                let skin_result = match theme {
                    UiTheme::ModernHifi => Skin::load_wsz("assets/skins/hifi_studio.wsz"),
                    UiTheme::CyberpunkTerminal => Skin::load_wsz("assets/skins/cyberpunk.wsz"),
                    UiTheme::WinampRedrawn | UiTheme::ClassicWinamp => Ok(Skin::load_default()),
                };
                if let Ok(new_skin) = skin_result {
                    self.skin = new_skin;
                    if let Some(ref mut textures) = self.textures {
                        textures.update(&self.skin);
                    }
                }
            }
            if let Some(a) = menu_action {
                main_action = a;
            }
        }

        // --- About dialog ------------------------------------------------------
        if self.show_about {
            let p = MenuPalette::for_theme(self.active_theme);
            egui::Window::new("About RetroAmp")
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
                .frame(
                    egui::Frame::window(&ctx.style())
                        .fill(p.frame_fill)
                        .stroke(egui::Stroke::new(1.0_f32, p.frame_stroke))
                        .inner_margin(egui::Margin::symmetric(10, 10)),
                )
                .show(ctx, |ui| {
                    ui.set_min_width(280.0);
                    ui.vertical_centered(|ui| {
                        ui.label(egui::RichText::new("⚡ RetroAmp").size(22.0).strong().color(p.accent));
                        ui.label(egui::RichText::new("v0.1.0 — Linux Edition").weak().color(p.section));
                    });
                    ui.separator();
                    ui.label(egui::RichText::new("Modular retro multimedia player in pure Rust.").color(p.text));
                    ui.label(egui::RichText::new("Skin engine, 10-band biquad EQ, FFT visualizer,").color(p.text));
                    ui.label(egui::RichText::new("borderless transparent skinned windows.").color(p.text));
                    ui.separator();
                    ui.label(egui::RichText::new("Built with: egui · symphonia · rodio · rustfft").weak().color(p.section));
                    ui.label(egui::RichText::new("Reverse-engineered with craft-anything; an homage to Winamp.").weak().color(p.section));
                    ui.separator();
                    if menu_item(ui, &p, "Close", false) {
                        self.show_about = false;
                    }
                });
        }

        // --- EQ Presets popup (classic Winamp preset list) ---------------------
        if self.eq_state.show_presets && self.main_state.show_eq {
            let p = MenuPalette::for_theme(self.active_theme);
            let mut close_presets = false;
            let mut applied_preset: Option<[f32; 10]> = None;

            let presets_pos = self.eq_state.position + egui::vec2(196.0, 30.0);
            let presets_win = egui::Window::new("retroamp_eq_presets")
                .id(egui::Id::new("retroamp-eq-presets"))
                .title_bar(false)
                .resizable(false)
                .collapsible(false)
                .fixed_pos(presets_pos)
                .frame(
                    egui::Frame::window(&ctx.style())
                        .fill(p.frame_fill)
                        .stroke(egui::Stroke::new(1.0_f32, p.frame_stroke))
                        .inner_margin(egui::Margin::symmetric(4, 4)),
                )
                .show(ctx, |ui| {
                    ui.set_min_width(170.0);
                    ui.set_max_width(170.0);
                    menu_section(ui, &p, "EQUALIZER PRESETS");
                    egui::ScrollArea::vertical().max_height(238.0).show(ui, |ui| {
                        for (name, bands) in crate::audio::EQ_PRESETS {
                            let active = self.audio.eq.enabled && self.audio.eq.bands[..] == bands[..];
                            if menu_item(ui, &p, name, active) {
                                applied_preset = Some(*bands);
                                close_presets = true;
                            }
                        }
                    });
                });

            if let Some(r) = &presets_win {
                if r.response.clicked_elsewhere() {
                    close_presets = true;
                }
            }
            if ctx.input(|i| i.key_pressed(Key::Escape)) {
                close_presets = true;
            }

            if close_presets {
                self.eq_state.show_presets = false;
            }
            if let Some(bands) = applied_preset {
                self.audio.eq.bands = bands;
            }
        }

        // --- Persist configuration (throttled change detection) ----------------
        let current_cfg = self.config_snapshot();
        if current_cfg != self.config_cache {
            self.config_dirty = true;
        }
        if self.config_dirty && self.last_config_save.elapsed() > Duration::from_millis(800) {
            current_cfg.save();
            self.config_cache = current_cfg;
            self.config_dirty = false;
            self.last_config_save = Instant::now();
        }

        // Process Main actions
        match main_action {
            MainAction::Play => {
                if self.audio.state == PlaybackState::Paused {
                    self.audio.resume();
                } else if let Some(idx) = self.current_track_index {
                    self.play_track_index(idx);
                } else if !self.playlist_state.items.is_empty() {
                    self.play_track_index(0);
                }
            }
            MainAction::Pause => {
                if self.audio.state == PlaybackState::Playing {
                    self.audio.pause();
                } else if self.audio.state == PlaybackState::Paused {
                    self.audio.resume();
                }
            }
            MainAction::Stop => {
                self.audio.stop();
            }
            MainAction::Prev => self.play_prev(),
            MainAction::Next => self.play_next(),
            MainAction::Eject => self.open_file_dialog(),
            MainAction::ToggleEq => {},
            MainAction::TogglePlaylist => {},
            MainAction::LoadSkin => { self.show_skin_picker = true; }
            MainAction::Minimize => {
                ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(true));
            }
            MainAction::Close => {
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
            MainAction::None => {}
        }

        // Process EQ actions
        if let EqAction::Close = eq_action {
            self.main_state.show_eq = false;
        }

        // Process Playlist actions
        match pl_action {
            PlaylistAction::PlayIndex(idx) => self.play_track_index(idx),
            PlaylistAction::AddFiles => self.open_file_dialog(),
            PlaylistAction::RemoveSelected => {
                if let Some(idx) = self.playlist_state.selected_index {
                    if idx < self.playlist_state.items.len() {
                        self.playlist_state.items.remove(idx);

                        // Safely adjust current track index
                        if let Some(curr) = self.current_track_index {
                            if curr == idx {
                                self.audio.stop();
                                self.current_track_index = if self.playlist_state.items.is_empty() {
                                    None
                                } else {
                                    Some(idx.min(self.playlist_state.items.len() - 1))
                                };
                            } else if curr > idx {
                                self.current_track_index = Some(curr - 1);
                            }
                        }

                        if self.playlist_state.items.is_empty() {
                            self.playlist_state.selected_index = None;
                        } else {
                            self.playlist_state.selected_index = Some(idx.min(self.playlist_state.items.len() - 1));
                        }
                    }
                }
            }
            PlaylistAction::Clear => {
                self.playlist_state.items.clear();
                self.playlist_state.selected_index = None;
                self.current_track_index = None;
                self.audio.stop();
            }
            PlaylistAction::Close => {
                self.main_state.show_playlist = false;
            }
            PlaylistAction::None => {}
        }

        // Render Skin Switcher Dialog if open
        if self.show_skin_picker {
            let p = MenuPalette::for_theme(self.active_theme);
            let mut close_picker = false;
            let mut switch_skin_path = None;
            let mut switch_to_classic = false;

            egui::Window::new("🎨 RetroAmp Theme Picker")
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
                .frame(
                    egui::Frame::window(&ctx.style())
                        .fill(p.frame_fill)
                        .stroke(egui::Stroke::new(1.0_f32, p.frame_stroke))
                        .inner_margin(egui::Margin::symmetric(8, 8)),
                )
                .show(ctx, |ui| {
                    ui.set_min_width(280.0);
                    ui.label(egui::RichText::new("Choose an iconic skin or load custom:").color(p.section));

                    if menu_item(ui, &p, "⭐  Winamp (Redrawn) — crisp at any zoom", self.active_theme == UiTheme::WinampRedrawn) {
                        self.active_theme = UiTheme::WinampRedrawn;
                        switch_to_classic = true;
                        close_picker = true;
                    }
                    if menu_item(ui, &p, "🖥  Winamp (Bitmap 1:1)", self.active_theme == UiTheme::ClassicWinamp && self.skin.name.contains("Winamp")) {
                        self.active_theme = UiTheme::ClassicWinamp;
                        switch_to_classic = true;
                        close_picker = true;
                    }
                    if menu_item(ui, &p, "🟢  Cyberpunk Terminal (Tokyo Phosphor)", self.active_theme == UiTheme::CyberpunkTerminal) {
                        self.active_theme = UiTheme::CyberpunkTerminal;
                        switch_skin_path = Some(PathBuf::from("assets/skins/cyberpunk.wsz"));
                        close_picker = true;
                    }
                    if menu_item(ui, &p, "📻  Hi-Fi Studio (Audiophile Aluminum)", self.active_theme == UiTheme::ModernHifi) {
                        self.active_theme = UiTheme::ModernHifi;
                        switch_skin_path = Some(PathBuf::from("assets/skins/hifi_studio.wsz"));
                        close_picker = true;
                    }
                    if menu_item(ui, &p, "❄️  Nord Frost (Slate & Ice Pastel)", self.active_theme == UiTheme::ClassicWinamp && self.skin.name == "nord_frost") {
                        self.active_theme = UiTheme::ClassicWinamp;
                        switch_skin_path = Some(PathBuf::from("assets/skins/nord_frost.wsz"));
                        close_picker = true;
                    }

                    ui.separator();
                    if menu_item(ui, &p, "📂  Load Custom .wsz Skin File...", false) {
                        self.active_theme = UiTheme::ClassicWinamp;
                        close_picker = true;
                        self.open_file_dialog();
                    }

                    if menu_item(ui, &p, "Close   (Esc)", false) || ctx.input(|i| i.key_pressed(Key::Escape)) {
                        close_picker = true;
                    }
                });

            if switch_to_classic {
                self.skin = Skin::load_default();
                if let Some(ref mut textures) = self.textures {
                    textures.update(&self.skin);
                }
            } else if let Some(path) = switch_skin_path {
                if let Ok(new_skin) = Skin::load_wsz(&path) {
                    self.skin = new_skin;
                    if let Some(ref mut textures) = self.textures {
                        textures.update(&self.skin);
                    }
                }
            }

            if close_picker {
                self.show_skin_picker = false;
            }
        }
    }
}
