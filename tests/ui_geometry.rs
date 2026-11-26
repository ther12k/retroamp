//! Offscreen UI geometry regression tests.
//!
//! These reproduce the three interaction defects found in the 2026-11-20
//! release review, using egui's kittest-style offscreen harness
//! (`CreationContext::_new_kittest`): no window system or audio hardware
//! is needed, the real `WinampApp::update` runs, and popup geometry is read
//! from `ctx.memory`.

use eframe::App;
use egui::{Context, Event, Id, Modifiers, PointerButton, Pos2, RawInput, Rect, vec2};
use retroamp::ui::WinampApp;
use retroamp::ui::app::UiTheme;
use std::ffi::OsString;
use std::sync::Mutex;

static APP_INIT_LOCK: Mutex<()> = Mutex::new(());

struct ScopedEnv(Vec<(&'static str, Option<OsString>)>);

impl ScopedEnv {
    fn new(overrides: Vec<(&'static str, Option<OsString>)>) -> Self {
        let previous = overrides
            .iter()
            .map(|(key, _)| (*key, std::env::var_os(key)))
            .collect();
        for (key, value) in &overrides {
            unsafe {
                if let Some(value) = value {
                    std::env::set_var(key, value);
                } else {
                    std::env::remove_var(key);
                }
            }
        }
        Self(previous)
    }
}

impl Drop for ScopedEnv {
    fn drop(&mut self) {
        for (key, value) in &self.0 {
            unsafe {
                if let Some(value) = value {
                    std::env::set_var(key, value);
                } else {
                    std::env::remove_var(key);
                }
            }
        }
    }
}

struct OffscreenApp {
    app: WinampApp,
    ctx: Context,
    frame: eframe::Frame,
    bounds: Rect,
}

impl OffscreenApp {
    fn new(width: f32, height: f32) -> Self {
        let ctx = Context::default();
        let mut app = {
            let _guard = APP_INIT_LOCK
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            WinampApp::new(&eframe::CreationContext::_new_kittest(ctx.clone()))
        };
        // Pin the default theme/scale so the tests are hermetic regardless of
        // the developer machine's persisted config.json.
        app.active_theme = UiTheme::WinampRedrawn;
        app.ui_scale = 1.0;
        app.main_state.show_eq = true;
        app.main_state.show_playlist = true;
        app.playlist_state = Default::default();
        app.playlist_state.height = 58.0 + 4.0 * 13.0;
        app.playlist_state.height_unsnapped = app.playlist_state.height;
        Self {
            app,
            ctx,
            frame: eframe::Frame::_new_kittest(),
            bounds: Rect::from_min_size(Pos2::ZERO, vec2(width, height)),
        }
    }

    fn update(&mut self, events: Vec<Event>) {
        let _ = self.update_output(events);
    }

    fn update_output(&mut self, events: Vec<Event>) -> egui::FullOutput {
        let input = RawInput {
            screen_rect: Some(self.bounds),
            focused: true,
            events,
            ..Default::default()
        };
        self.app.last_config_save = std::time::Instant::now();
        self.ctx
            .run(input, |ctx| self.app.update(ctx, &mut self.frame))
    }

    fn click(&mut self, pos: Pos2) {
        let _ = self.click_output(pos);
    }

    fn click_output(&mut self, pos: Pos2) -> egui::FullOutput {
        let button = |pressed| Event::PointerButton {
            pos,
            button: PointerButton::Primary,
            pressed,
            modifiers: Modifiers::NONE,
        };
        self.update(vec![Event::PointerMoved(pos), button(true)]);
        self.update_output(vec![button(false)])
    }

    fn press_key(&mut self, key: egui::Key) -> egui::FullOutput {
        self.update_output(vec![Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Modifiers::NONE,
        }])
    }

    fn sends_close(output: &egui::FullOutput) -> bool {
        output
            .viewport_output
            .get(&egui::ViewportId::ROOT)
            .is_some_and(|viewport| {
                viewport
                    .commands
                    .iter()
                    .any(|command| matches!(command, egui::ViewportCommand::Close))
            })
    }

    fn area(&self, id: &str) -> Option<Rect> {
        self.ctx.memory(|mem| mem.area_rect(Id::new(id)))
    }

    fn repaint_delay(output: &egui::FullOutput) -> std::time::Duration {
        output.viewport_output[&egui::ViewportId::ROOT].repaint_delay
    }
}

/// The main menu lists ~20 items (~520pt tall) but the default window stack
/// is only 362pt — the popup must scroll instead of spilling past the host.
#[test]
fn menu_fits_the_default_host() {
    for height in [362.0, 136.0] {
        let mut app = OffscreenApp::new(295.0, height);
        if height < 362.0 {
            app.app.main_state.show_eq = false;
            app.app.main_state.show_playlist = false;
        }
        app.update(vec![Event::Key {
            key: egui::Key::F10,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Modifiers::NONE,
        }]);
        for _ in 0..3 {
            app.update(vec![]);
        }
        assert!(app.app.main_state.menu_open);
        let menu = app
            .area("retroamp-main-menu")
            .expect("menu popup must be allocated");
        assert!(
            app.bounds.contains_rect(menu),
            "menu {menu:?} exceeds {:?}",
            app.bounds
        );
    }
}

/// The click that opens the presets popup must not also dismiss it.
#[test]
fn f10_tab_enter_activates_a_menu_item_instead_of_the_background_close_button() {
    let mut app = OffscreenApp::new(295.0, 362.0);
    app.press_key(egui::Key::F10);
    assert!(app.app.main_state.menu_open);

    app.press_key(egui::Key::Tab);
    let output = app.press_key(egui::Key::Enter);

    assert!(app.app.show_skin_picker, "Tab + Enter should activate Open Skin / Themes");
    assert!(!app.app.main_state.menu_open, "activating a menu item should close the menu");
    assert!(
        !OffscreenApp::sends_close(&output),
        "keyboard focus must not reach the destructive background close control"
    );
}

#[test]
fn menu_rows_can_be_activated_with_tab_and_enter() {
    let mut app = OffscreenApp::new(295.0, 362.0);
    app.app.main_state.shuffle = false;
    app.press_key(egui::Key::F10);

    // Focus begins on Open File(s); Shuffle is the ninth menu row.
    for _ in 0..8 {
        app.press_key(egui::Key::Tab);
    }
    app.press_key(egui::Key::Enter);

    assert!(app.app.main_state.shuffle, "Enter should toggle the focused Shuffle row");
    assert!(!app.app.main_state.menu_open);
}

#[test]
fn tab_focus_wraps_inside_the_open_menu() {
    let mut app = OffscreenApp::new(295.0, 362.0);
    app.press_key(egui::Key::F10);

    // Cycle through all 21 selectable rows. Focus wraps to the first menu row
    // rather than escaping to a background control.
    for _ in 0..21 {
        app.press_key(egui::Key::Tab);
    }

    let focused = app
        .ctx
        .memory(|memory| memory.focused())
        .expect("an open menu should retain keyboard focus");
    let response = app
        .ctx
        .read_response(focused)
        .expect("focused menu row should still be allocated");
    assert_eq!(response.layer_id.order, egui::Order::Foreground);
}

#[test]
fn menu_escape_and_outside_click_behaviors_are_preserved() {
    let mut app = OffscreenApp::new(295.0, 362.0);
    app.press_key(egui::Key::F10);
    app.press_key(egui::Key::Escape);
    assert!(!app.app.main_state.menu_open, "Escape should close the menu");

    app.press_key(egui::Key::F10);
    app.update(vec![]);
    app.update(vec![]);
    let background_close = app.app.main_state.position + vec2(269.0, 6.0);
    let output = app.click_output(background_close);

    assert!(!app.app.main_state.menu_open, "an outside click should close the menu");
    assert!(
        !OffscreenApp::sends_close(&output),
        "the outside click must not pass through to the background close button"
    );
}

#[test]
fn presets_stay_open_after_the_opening_click() {
    let mut app = OffscreenApp::new(295.0, 362.0);
    for _ in 0..3 {
        app.update(vec![]);
    }
    let pos = app.app.eq_state.position + vec2(230.0, 24.0);
    app.click(pos);
    assert!(
        app.area("retroamp-eq-presets").is_some(),
        "the click must reach the Presets button"
    );
    assert!(
        app.app.eq_state.show_presets,
        "opening click should not dismiss the popup"
    );
    for _ in 0..3 {
        app.update(vec![]);
    }
    let area = app.area("retroamp-eq-presets").unwrap();
    assert!(
        app.bounds.contains_rect(area),
        "presets {area:?} exceed {:?}",
        app.bounds
    );
    app.click(area.min + vec2(40.0, 49.0)); // Classical (second row)
    assert!(!app.app.eq_state.show_presets);
    assert_eq!(app.app.audio.eq.bands, retroamp::audio::EQ_PRESETS[1].1);
}

/// Slow resize drags (sub-row pointer deltas per frame) must accumulate.
#[test]
fn slow_resize_accumulates_pointer_movement() {
    let mut app = OffscreenApp::new(295.0, 362.0);
    for _ in 0..3 {
        app.update(vec![]);
    }
    let start = app.app.playlist_state.height;
    let pos = app.app.playlist_state.position + vec2(120.0, start - 3.0);
    let button = |pos, pressed| Event::PointerButton {
        pos,
        button: PointerButton::Primary,
        pressed,
        modifiers: Modifiers::NONE,
    };
    app.update(vec![Event::PointerMoved(pos), button(pos, true)]);
    for delta in 1..=26 {
        app.update(vec![Event::PointerMoved(pos + vec2(0.0, delta as f32))]);
    }
    app.update(vec![button(pos + vec2(0.0, 26.0), false)]);
    let end = app.app.playlist_state.height;
    assert_eq!(end, start + 26.0, "a 26-point drag should add two rows");
}

#[test]
fn populated_playlist_resize_keeps_capture_when_visible_row_count_changes() {
    let mut app = OffscreenApp::new(295.0, 480.0);
    app.app.active_theme = UiTheme::ClassicWinamp;
    app.app.playlist_state.height = 58.0 + 5.0 * 13.0;
    app.app.playlist_state.height_unsnapped = app.app.playlist_state.height;
    app.app.playlist_state.items = (0..10)
        .map(|index| retroamp::ui::playlist_window::PlaylistEntry {
            title: format!("Track {index}"),
            path: std::path::PathBuf::from(format!("track-{index}.wav")),
            duration_secs: 180,
        })
        .collect();
    for _ in 0..3 {
        app.update(vec![]);
    }

    let start = app.app.playlist_state.height;
    let pos = app.app.playlist_state.position + vec2(120.0, start - 3.0);
    let button = |pos, pressed| Event::PointerButton {
        pos,
        button: PointerButton::Primary,
        pressed,
        modifiers: Modifiers::NONE,
    };
    app.update(vec![Event::PointerMoved(pos), button(pos, true)]);
    for delta in 1..=26 {
        app.update(vec![Event::PointerMoved(pos + vec2(0.0, delta as f32))]);
    }
    app.update(vec![button(pos + vec2(0.0, 26.0), false)]);

    assert_eq!(app.app.playlist_state.height, start + 26.0);
}

#[test]
fn imported_wsz_switches_to_the_bitmap_renderer() {
    let mut app = OffscreenApp::new(295.0, 362.0);
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/skins/matrix.wsz");
    app.app.handle_opened_file(path);
    assert_eq!(app.app.active_theme, UiTheme::ClassicWinamp);
    assert_eq!(app.app.skin.name, "matrix");
    let source = app.app.skin_source.clone().unwrap();
    assert!(std::path::Path::new(&source).is_absolute());

    app.app.handle_opened_file("missing.wsz".into());
    assert_eq!(app.app.skin.name, "matrix");
    assert_eq!(app.app.skin_source.as_deref(), Some(source.as_str()));
}

#[test]
fn eof_auto_advance_stops_after_failed_entries_without_changing_manual_index() {
    let mut app = OffscreenApp::new(295.0, 362.0);
    app.app.main_state.shuffle = false;
    app.app.main_state.repeat = false;
    app.app.playlist_state.items = (0..3)
        .map(|index| retroamp::ui::playlist_window::PlaylistEntry {
            title: format!("Unavailable {index}"),
            path: std::env::temp_dir().join(format!(
                "retroamp-missing-track-{}-{index}.wav",
                std::process::id()
            )),
            duration_secs: 0,
        })
        .collect();
    app.app.current_track_index = Some(0);

    app.app.play_next_auto();

    assert_eq!(app.app.current_track_index, Some(0));
    assert_eq!(app.app.audio.state, retroamp::audio::PlaybackState::Stopped);
}

#[test]
fn missing_saved_custom_skin_path_survives_exit_config_snapshot() {
    let unique = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let config_home = std::env::temp_dir().join(format!(
        "retroamp-skin-config-{}-{unique}",
        std::process::id()
    ));
    let config_dir = config_home.join("retroamp");
    std::fs::create_dir_all(&config_dir).unwrap();
    let missing_skin = config_home.join("temporarily-missing.wsz");
    assert!(!missing_skin.exists());

    let mut config = retroamp::config::AppConfig::default();
    config.theme = "classic".to_string();
    config.skin = Some(missing_skin.to_string_lossy().into_owned());
    std::fs::write(
        config_dir.join("config.json"),
        serde_json::to_vec(&config).unwrap(),
    )
    .unwrap();

    let _app_lock = APP_INIT_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let _env = ScopedEnv::new(vec![
        ("XDG_CONFIG_HOME", Some(config_home.clone().into_os_string())),
        ("RETROAMP_SKIN", None),
        ("WINAMP_SKIN", None),
    ]);
    let ctx = Context::default();
    let creation_context = eframe::CreationContext::_new_kittest(ctx);
    let mut app = WinampApp::new(&creation_context);

    assert_eq!(app.skin_source.as_deref(), missing_skin.to_str());
    app.on_exit(None);

    let saved: serde_json::Value = serde_json::from_slice(
        &std::fs::read(config_dir.join("config.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(saved["skin"], missing_skin.to_string_lossy().as_ref());

    drop(app);
    drop(_env);
    drop(_app_lock);
    std::fs::remove_dir_all(config_home).unwrap();
}

#[test]
fn idle_ui_sleeps_and_playback_requests_a_steady_refresh() {
    let mut app = OffscreenApp::new(295.0, 362.0);
    // The first pass uploads the bundled texture set and legitimately requests
    // a follow-up frame. Measure once the initial GPU resources have settled.
    for _ in 0..3 {
        app.update_output(vec![]);
    }
    let idle = app.update_output(vec![]);
    assert!(
        OffscreenApp::repaint_delay(&idle) >= std::time::Duration::from_millis(700),
        "an idle player should not continuously request animation frames; got {:?}",
        OffscreenApp::repaint_delay(&idle)
    );

    app.app.audio.state = retroamp::audio::PlaybackState::Playing;
    let playing = app.update_output(vec![]);
    let delay = OffscreenApp::repaint_delay(&playing);
    assert!(
        (std::time::Duration::from_millis(15)..=std::time::Duration::from_millis(33))
            .contains(&delay),
        "the visualizer should schedule an approximately 30 fps refresh; got {delay:?}"
    );
}

#[test]
fn failed_track_attempt_shows_the_title_and_recovery_step() {
    let mut app = OffscreenApp::new(295.0, 362.0);
    app.app.playlist_state.items = vec![retroamp::ui::playlist_window::PlaylistEntry {
        title: "Unavailable test fixture".into(),
        path: std::env::temp_dir().join(format!(
            "retroamp-playback-error-{}.wav",
            std::process::id()
        )),
        duration_secs: 0,
    }];

    assert!(!app.app.play_track_index(0));
    let message = app.app.playback_error.as_deref().unwrap();
    assert!(message.contains("Unavailable test fixture"));
    assert!(message.contains("open a different file"));
}
