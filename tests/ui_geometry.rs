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
    /// Tests run against the developer's real config location, so the harness
    /// keeps every save attempt out of reach unless a test opts in.
    pin_last_save: bool,
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
            pin_last_save: true,
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
        if self.pin_last_save {
            self.app.last_config_save = std::time::Instant::now();
        }
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
fn tab_focus_cycles_through_the_open_menu_without_escaping() {
    let mut app = OffscreenApp::new(295.0, 362.0);
    app.press_key(egui::Key::F10);
    app.press_key(egui::Key::Tab);

    let first = app
        .ctx
        .memory(|memory| memory.focused())
        .expect("an open menu should take keyboard focus");

    // egui hands focus to the next interested widget one frame after Tab, so
    // crossing the last row shows one frame with nothing focused before the
    // cycle lands back on the first row. Focus must never stay away longer.
    let mut blank_frames = 0;
    let mut wrapped = false;
    for _ in 0..80 {
        app.press_key(egui::Key::Tab);
        match app.ctx.memory(|memory| memory.focused()) {
            Some(id) if id == first => {
                wrapped = true;
                break;
            }
            Some(_) => blank_frames = 0,
            None => {
                blank_frames += 1;
                assert!(
                    blank_frames <= 1,
                    "Tab focus left the open menu for {blank_frames} frames"
                );
            }
        }
    }
    assert!(wrapped, "cycling Tab never returned to the first menu row");

    let response = app
        .ctx
        .read_response(first)
        .expect("the first menu row should still be allocated");
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
    // Rows and the grip share one `Ui`. When the height snaps another row on,
    // the grip must keep its identity, or the drag is dropped after one row.
    for (theme, row_height) in [
        (UiTheme::WinampRedrawn, 13.0),
        (UiTheme::ClassicWinamp, 13.0),
        (UiTheme::ModernHifi, 18.0),
        (UiTheme::CyberpunkTerminal, 14.0),
    ] {
        let mut app = OffscreenApp::new(295.0, 560.0);
        app.app.active_theme = theme;
        let start = 58.0 + 5.0 * row_height;
        app.app.playlist_state.height = start;
        app.app.playlist_state.height_unsnapped = start;
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

        let pos = app.app.playlist_state.position + vec2(120.0, start - 3.0);
        let button = |pos, pressed| Event::PointerButton {
            pos,
            button: PointerButton::Primary,
            pressed,
            modifiers: Modifiers::NONE,
        };
        app.update(vec![Event::PointerMoved(pos), button(pos, true)]);
        let distance = (2.0 * row_height) as i32;
        for delta in 1..=distance {
            app.update(vec![Event::PointerMoved(pos + vec2(0.0, delta as f32))]);
        }
        app.update(vec![button(pos + vec2(0.0, distance as f32), false)]);

        assert_eq!(
            app.app.playlist_state.height,
            start + 2.0 * row_height,
            "a drag past two row boundaries should add two rows in {theme:?}"
        );
    }
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

    // The chain walks background loads: it ends when nothing is pending.
    assert!(
        run_until(
            &mut app,
            |app| (app.pending_load.is_none(), app.playback_error.is_some()),
            |(no_pending, error)| no_pending && error,
            std::time::Duration::from_secs(5)
        ),
        "the exhausted chain must settle on an error"
    );
    assert_eq!(app.app.current_track_index, Some(0), "the selection returns to the chain origin");
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

/// Run idle frames until a copyable snapshot of the app satisfies `done`.
/// Worker threads deliver load outcomes and scan results between frames.
fn run_until<R: Copy>(
    app: &mut OffscreenApp,
    snapshot: impl Fn(&WinampApp) -> R,
    done: impl Fn(R) -> bool,
    limit: std::time::Duration,
) -> bool {
    let deadline = std::time::Instant::now() + limit;
    loop {
        if done(snapshot(&app.app)) {
            return true;
        }
        if std::time::Instant::now() >= deadline {
            return false;
        }
        std::thread::sleep(std::time::Duration::from_millis(15));
        app.update_output(vec![]);
    }
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

    assert!(
        app.app.play_track_index(0),
        "a failed load is still a kicked load"
    );
    assert!(
        run_until(
            &mut app,
            |app| app.playback_error.is_some(),
            |reported| reported,
            std::time::Duration::from_secs(5)
        ),
        "the background failure must surface in a notice"
    );
    let message = app.app.playback_error.as_deref().unwrap();
    assert!(message.contains("Unavailable test fixture"));
    assert!(message.contains("open a different file"));

    // The default host is only 295pt wide; the notice must fit inside it
    // instead of being centred on it and clipped on both sides.
    for _ in 0..3 {
        app.update(vec![]);
    }
    let notice = app
        .area("retroamp-playback-error")
        .expect("the failure must be reported on screen");
    assert!(
        app.bounds.contains_rect(notice),
        "notice {notice:?} is clipped by {:?}",
        app.bounds
    );
}

#[test]
fn refused_skin_is_reported_on_screen_and_a_good_one_clears_the_notice() {
    let mut app = OffscreenApp::new(295.0, 362.0);
    let good = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/skins/matrix.wsz");

    app.app.handle_opened_file("missing.wsz".into());
    let message = app
        .app
        .skin_error
        .as_deref()
        .expect("a refused skin must be reported, not only logged");
    assert!(message.contains("missing.wsz"), "{message}");

    for _ in 0..3 {
        app.update(vec![]);
    }
    let notice = app
        .area("retroamp-skin-error")
        .expect("the refused skin must be reported on screen");
    assert!(
        app.bounds.contains_rect(notice),
        "notice {notice:?} is clipped by {:?}",
        app.bounds
    );

    app.app.handle_opened_file(good);
    assert_eq!(app.app.skin_error, None, "loading a skin clears the notice");
}

/// The menu is a modal (focus stays inside it) but it must still open as a
/// dropdown under the titlebar logo, not over the centre of the player.
#[test]
fn queued_tracks_get_real_durations_without_blocking_the_ui() {
    // A 2-second WAV whose header the scanner reads without any audio device.
    let dir = std::env::temp_dir().join(format!(
        "retroamp-duration-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    let rate: u32 = 8000; // small on purpose: the probe reads the header only
    let path = dir.join("two-seconds.wav");
    let frames = 2 * rate as usize;
    let mut bytes = Vec::new();
    bytes.extend_from_slice(b"RIFF");
    // u32 sizes: `frames` is a usize, whose 8-byte `to_le_bytes` would
    // shift the whole header by four zero bytes.
    bytes.extend_from_slice(&((36 + frames * 2) as u32).to_le_bytes());
    bytes.extend_from_slice(b"WAVEfmt ");
    bytes.extend_from_slice(&16u32.to_le_bytes());
    bytes.extend_from_slice(&1u16.to_le_bytes()); // PCM
    bytes.extend_from_slice(&1u16.to_le_bytes()); // mono
    bytes.extend_from_slice(&rate.to_le_bytes());
    bytes.extend_from_slice(&(rate * 2).to_le_bytes());
    bytes.extend_from_slice(&2u16.to_le_bytes());
    bytes.extend_from_slice(&16u16.to_le_bytes());
    // The header claims two seconds of PCM, but the data chunk is empty:
    // the scanner can still read the duration from the header, while the
    // player fails at priming before any audio device is touched. That
    // keeps the test hermetic and exercises the scan, not the play path.
    bytes.extend_from_slice(b"data");
    bytes.extend_from_slice(&((frames * 2) as u32).to_le_bytes());
    // …and stop here: no samples follow the data header.
    std::fs::write(&path, &bytes).unwrap();

    let mut app = OffscreenApp::new(295.0, 362.0);
    app.app.open_paths(&[path.clone()]);
    assert_eq!(app.app.playlist_state.items.len(), 1);
    assert_eq!(app.app.playlist_state.items[0].duration_secs, 0);

    // The probe runs on a worker thread; idle frames pick the result up.
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    let mut delay = std::time::Duration::ZERO;
    while app.app.playlist_state.items[0].duration_secs == 0 {
        assert!(
            std::time::Instant::now() < deadline,
            "the scanned duration never arrived"
        );
        std::thread::sleep(std::time::Duration::from_millis(25));
        delay = OffscreenApp::repaint_delay(&app.update_output(vec![]));
    }
    assert_eq!(app.app.playlist_state.items[0].duration_secs, 2);
    assert!(
        delay <= std::time::Duration::from_millis(33),
        "a frame that applied a duration should prompt the next one; got {delay:?}"
    );

    // Playback priming runs in the background and reports its failure once settled.
    assert!(
        run_until(
            &mut app,
            |app| app.playback_error.is_some(),
            |reported| reported,
            std::time::Duration::from_secs(5),
        ),
        "the unplayable entry must be reported instead of silently queued"
    );

    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn command_line_arguments_fill_the_playlist_and_report_failures() {
    // Garbage content keeps this hermetic: metadata fails before any audio
    // device is touched.
    let unique = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let dir = std::env::temp_dir().join(format!("retroamp-args-{}-{unique}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    for name in ["zeta.mp3", "alpha.flac", "notes.txt"] {
        std::fs::write(dir.join(name), b"not audio").unwrap();
    }
    let skin = dir.join("fake.wsz");
    std::fs::write(&skin, b"not a zip archive").unwrap();

    let mut app = OffscreenApp::new(295.0, 362.0);
    app.app.open_paths(&[dir.clone(), skin]);

    let titles: Vec<&str> = app
        .app
        .playlist_state
        .items
        .iter()
        .map(|entry| entry.title.as_str())
        .collect();
    assert_eq!(
        titles,
        vec!["alpha", "zeta"],
        "folder tracks join sorted; non-audio files are skipped"
    );
    assert!(
        run_until(
            &mut app,
            |app| app.playback_error.is_some(),
            |reported| reported,
            std::time::Duration::from_secs(5)
        ),
        "the first unplayable track must be reported"
    );
    let error = app.app.playback_error.as_deref().unwrap();
    assert!(error.contains("alpha"), "{error}");
    let skin_error = app
        .app
        .skin_error
        .as_deref()
        .expect("a refused skin argument is reported");
    assert!(skin_error.contains("fake.wsz"), "{skin_error}");

    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn menu_drops_down_under_the_titlebar_logo() {
    let mut app = OffscreenApp::new(295.0, 362.0);
    app.press_key(egui::Key::F10);
    for _ in 0..3 {
        app.update(vec![]);
    }
    let menu = app
        .area("retroamp-main-menu")
        .expect("menu popup must be allocated");
    let expected = app.app.main_state.position + vec2(8.0, 20.0);
    assert_eq!(menu.min, expected, "the menu should open at the dropdown anchor");
    // Focus is still trapped by the modal: Tab never leaves it.
    app.press_key(egui::Key::Tab);
    assert!(app.app.main_state.menu_open);
}

#[test]
fn failed_config_saves_back_off_instead_of_retrying_forever() {
    let mut app = OffscreenApp::new(295.0, 362.0);
    // A regular file where the config directory should be: creating it fails.
    let blocker = std::env::temp_dir().join(format!(
        "retroamp-config-blocker-{}.tmp",
        std::process::id()
    ));
    std::fs::write(&blocker, b"not a directory").unwrap();

    let _lock = APP_INIT_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let _env = ScopedEnv::new(vec![(
        "XDG_CONFIG_HOME",
        Some(blocker.clone().into_os_string()),
    )]);
    app.pin_last_save = false;
    app.app.config_dirty = true;

    let mut delays = Vec::new();
    for _ in 0..12 {
        // Long past the due time, so every frame attempts a save and fails.
        app.app.last_config_save = std::time::Instant::now() - std::time::Duration::from_secs(600);
        delays.push(OffscreenApp::repaint_delay(&app.update_output(vec![])));
    }
    assert!(app.app.config_dirty, "the settings are still unsaved");
    assert_eq!(
        app.app.config_save_interval,
        std::time::Duration::from_secs(60),
        "each failure should double the wait, capped at a minute"
    );
    assert!(
        delays[11] >= std::time::Duration::from_secs(50),
        "an idle UI with an unwritable config should sleep, not wake at the save rate; got {:?}",
        delays
    );

    // Once the location is writable again the normal cadence returns.
    std::fs::remove_file(&blocker).unwrap();
    app.app.last_config_save = std::time::Instant::now() - std::time::Duration::from_secs(600);
    app.update(vec![]);
    assert!(!app.app.config_dirty, "a successful save clears the dirty flag");
    assert_eq!(app.app.config_save_interval, std::time::Duration::from_millis(800));

    drop(_env);
    drop(_lock);
    let _ = std::fs::remove_dir_all(&blocker);
}
