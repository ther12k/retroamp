//! Cyberpunk Terminal UI/UX for RetroAmp.
//! CRT Phosphor Green scanlines, terminal brackets, monospace telemetry, and hacker aesthetics.

use crate::audio::{AudioEngine, PlaybackState};
use crate::skin::{MAIN_WIDTH, MAIN_HEIGHT, EQ_WIDTH, EQ_HEIGHT, PL_DEFAULT_WIDTH};
use crate::ui::eq_window::{EqAction, EqWindowState};
use crate::ui::main_window::{MainAction, MainWindowState, VisMode};
use crate::ui::playlist_window::{PlaylistAction, PlaylistWindowState};
use egui::{Color32, FontFamily, FontId, Pos2, Rect, Sense, Stroke, Ui, Vec2};
use std::time::Duration;

const CRT_BG: Color32 = Color32::from_rgb(6, 12, 8);
const CRT_BORDER: Color32 = Color32::from_rgb(0, 180, 80);
const PHOSPHOR_GREEN: Color32 = Color32::from_rgb(0, 255, 100);
const PHOSPHOR_DIM: Color32 = Color32::from_rgb(0, 150, 60);
const SCANLINE_COLOR: Color32 = Color32::from_rgba_premultiplied(0, 40, 20, 90);
const AMBER_ALERT: Color32 = Color32::from_rgb(255, 170, 0);

pub fn render_cyberpunk_main(
    ui: &mut Ui,
    state: &mut MainWindowState,
    audio: &mut AudioEngine,
) -> MainAction {
    let mut action = MainAction::None;
    let window_rect = Rect::from_min_size(state.position, Vec2::new(MAIN_WIDTH, MAIN_HEIGHT));

    // Dragging
    let titlebar_rect = Rect::from_min_size(state.position, Vec2::new(MAIN_WIDTH - 40.0, 16.0));
    let titlebar_resp = ui.allocate_rect(titlebar_rect, Sense::click_and_drag());
    if titlebar_resp.drag_started() {
        state.os_drag_started = true;
    }

    let painter = ui.painter_at(window_rect);

    // 1. CRT Monitor Chassis
    painter.rect_filled(window_rect, 2.0, CRT_BG);
    painter.rect_stroke(window_rect, 2.0, Stroke::new(1.5_f32, CRT_BORDER), egui::StrokeKind::Inside);

    // Subtle horizontal CRT scanlines
    for y in (window_rect.min.y as i32..window_rect.max.y as i32).step_by(3) {
        painter.line_segment(
            [Pos2::new(window_rect.min.x, y as f32), Pos2::new(window_rect.max.x, y as f32)],
            Stroke::new(1.0_f32, SCANLINE_COLOR),
        );
    }

    // 2. Terminal Titlebar
    painter.rect_filled(
        Rect::from_min_size(state.position, Vec2::new(MAIN_WIDTH, 16.0)),
        2.0,
        Color32::from_rgb(0, 40, 20),
    );

    // [≡] Main Menu button
    let menu_btn = Rect::from_min_size(state.position + Vec2::new(6.0, 2.0), Vec2::new(16.0, 12.0));
    let menu_resp = ui.allocate_rect(menu_btn, Sense::click());
    painter.text(menu_btn.center(), egui::Align2::CENTER_CENTER, "[≡]", FontId::new(8.0, FontFamily::Monospace), if menu_resp.hovered() || state.menu_open { Color32::WHITE } else { PHOSPHOR_GREEN });
    if menu_resp.clicked() {
        state.menu_open = !state.menu_open;
        state.menu_frames = 0;
    }

    painter.text(
        state.position + Vec2::new(26.0, 2.0),
        egui::Align2::LEFT_TOP,
        "// RETROAMP_CORE_v2.0 [ONLINE] //",
        FontId::new(9.0, FontFamily::Monospace),
        PHOSPHOR_GREEN,
    );

    // Theme Switcher Button ([S])
    let theme_btn = Rect::from_min_size(state.position + Vec2::new(MAIN_WIDTH - 48.0, 2.0), Vec2::new(14.0, 12.0));
    let theme_resp = ui.allocate_rect(theme_btn, Sense::click());
    painter.text(theme_btn.center(), egui::Align2::CENTER_CENTER, "[S]", FontId::new(8.0, FontFamily::Monospace), if theme_resp.hovered() { Color32::WHITE } else { PHOSPHOR_GREEN });
    if theme_resp.clicked() { action = MainAction::LoadSkin; }

    // Minimize [ - ]
    let min_btn = Rect::from_min_size(state.position + Vec2::new(MAIN_WIDTH - 30.0, 2.0), Vec2::new(14.0, 12.0));
    let min_resp = ui.allocate_rect(min_btn, Sense::click());
    painter.text(min_btn.center(), egui::Align2::CENTER_CENTER, "[-]", FontId::new(8.0, FontFamily::Monospace), if min_resp.hovered() { Color32::WHITE } else { PHOSPHOR_GREEN });
    if min_resp.clicked() { action = MainAction::Minimize; }

    // Close [ X ]
    let close_btn = Rect::from_min_size(state.position + Vec2::new(MAIN_WIDTH - 15.0, 2.0), Vec2::new(14.0, 12.0));
    let close_resp = ui.allocate_rect(close_btn, Sense::click());
    painter.text(close_btn.center(), egui::Align2::CENTER_CENTER, "[X]", FontId::new(8.0, FontFamily::Monospace), if close_resp.hovered() { Color32::RED } else { PHOSPHOR_GREEN });
    if close_resp.clicked() { action = MainAction::Close; }

    // 3. Terminal Prompt Display Area (x: 6, y: 20, w: 263, h: 48)
    let term_rect = Rect::from_min_size(state.position + Vec2::new(6.0, 20.0), Vec2::new(263.0, 48.0));
    painter.rect_filled(term_rect, 1.0, Color32::from_rgb(2, 8, 4));
    painter.rect_stroke(term_rect, 1.0, Stroke::new(1.0_f32, PHOSPHOR_DIM), egui::StrokeKind::Inside);

    // Track command line
    let title_line = if let Some(ref track) = audio.current_track {
        format!("> PLAY: {} [{}]", track.title, track.artist)
    } else {
        "> DAEMON: READY -- NO STREAM ATTACHED".to_string()
    };
    painter.text(term_rect.min + Vec2::new(6.0, 4.0), egui::Align2::LEFT_TOP, &title_line, FontId::new(9.5, FontFamily::Monospace), PHOSPHOR_GREEN);

    // Clock and Status
    let display_secs = if state.time_remaining_mode {
        if let Some(ref track) = audio.current_track {
            if track.duration > audio.position { (track.duration - audio.position).as_secs() } else { 0 }
        } else { 0 }
    } else {
        audio.position.as_secs()
    };
    let time_str = format!("TIME: [{:02}:{:02}]", display_secs / 60, display_secs % 60);
    let time_rect = Rect::from_min_size(term_rect.min + Vec2::new(6.0, 18.0), Vec2::new(85.0, 12.0));
    let time_resp = ui.allocate_rect(time_rect, Sense::click());
    if time_resp.clicked() { state.time_remaining_mode = !state.time_remaining_mode; }
    painter.text(time_rect.min, egui::Align2::LEFT_TOP, &time_str, FontId::new(10.0, FontFamily::Monospace), AMBER_ALERT);

    // Audio Telemetry
    let (srate, kbps) = if let Some(ref t) = audio.current_track {
        (t.sample_rate / 1000, t.bitrate_kbps)
    } else {
        (44, 320)
    };
    let status_line = format!("FREQ: {}kHz | BIT: {}k", srate, kbps);
    painter.text(term_rect.min + Vec2::new(6.0, 32.0), egui::Align2::LEFT_TOP, &status_line, FontId::new(9.0, FontFamily::Monospace), PHOSPHOR_DIM);

    // Oscilloscope / Matrix spectrum on the right
    let vis_rect = Rect::from_min_size(term_rect.min + Vec2::new(175.0, 18.0), Vec2::new(82.0, 26.0));
    let vis_resp = ui.allocate_rect(vis_rect, Sense::click());
    if vis_resp.clicked() {
        state.vis_mode = match state.vis_mode {
            VisMode::Spectrum => VisMode::Oscilloscope,
            VisMode::Oscilloscope => VisMode::Off,
            VisMode::Off => VisMode::Spectrum,
        };
    }

    if state.vis_mode == VisMode::Spectrum {
        let (_, spectrum, _) = audio.get_vis_data();
        let num_bars = 16;
        for i in 0..num_bars {
            let bx = vis_rect.min.x + (i as f32) * 5.0;
            let val = spectrum[i].clamp(0.0, 1.0);
            let h = (val * 22.0).round().max(1.0);
            let by = vis_rect.max.y - h;
            painter.rect_filled(Rect::from_min_size(Pos2::new(bx, by), Vec2::new(3.0, h)), 0.0, PHOSPHOR_GREEN);
        }
    } else if state.vis_mode == VisMode::Oscilloscope {
        let (waveform, _, _) = audio.get_vis_data();
        let mid_y = vis_rect.min.y + 13.0;
        let w_len = waveform.len().min(75);
        for i in 0..w_len - 1 {
            let x1 = vis_rect.min.x + (i as f32 / w_len as f32) * 80.0;
            let x2 = vis_rect.min.x + ((i + 1) as f32 / w_len as f32) * 80.0;
            let y1 = (mid_y - waveform[i] * 12.0).clamp(vis_rect.min.y, vis_rect.max.y);
            let y2 = (mid_y - waveform[i + 1] * 12.0).clamp(vis_rect.min.y, vis_rect.max.y);
            painter.line_segment([Pos2::new(x1, y1), Pos2::new(x2, y2)], Stroke::new(1.0_f32, PHOSPHOR_GREEN));
        }
    }

    // 4. Progress Bar (ASCII / Matrix style)
    let prog_rect = Rect::from_min_size(state.position + Vec2::new(6.0, 72.0), Vec2::new(263.0, 6.0));
    painter.rect_filled(prog_rect, 0.0, Color32::from_rgb(0, 20, 10));
    let progress_ratio = if let Some(ref track) = audio.current_track {
        if track.duration > Duration::ZERO { (audio.position.as_secs_f32() / track.duration.as_secs_f32()).clamp(0.0, 1.0) } else { 0.0 }
    } else { 0.0 };
    painter.rect_filled(Rect::from_min_size(prog_rect.min, Vec2::new(prog_rect.width() * progress_ratio, 6.0)), 0.0, PHOSPHOR_GREEN);
    let prog_resp = ui.allocate_rect(prog_rect, Sense::click_and_drag());
    if prog_resp.clicked() || prog_resp.dragged() {
        if let Some(mouse) = ui.input(|i| i.pointer.interact_pos()) {
            let r = ((mouse.x - prog_rect.min.x) / prog_rect.width()).clamp(0.0, 1.0);
            state.seek_target_ratio = Some(r);
        }
    }

    // 5. Sliders and Matrix Toggles (y: 81)
    let vol_text = format!("VOL: {:2.0}%", audio.volume * 100.0);
    painter.text(state.position + Vec2::new(6.0, 81.0), egui::Align2::LEFT_TOP, &vol_text, FontId::new(8.5, FontFamily::Monospace), PHOSPHOR_DIM);
    let vol_track = Rect::from_min_size(state.position + Vec2::new(56.0, 83.0), Vec2::new(50.0, 5.0));
    painter.rect_filled(vol_track, 0.0, Color32::from_rgb(0, 30, 15));
    painter.rect_filled(Rect::from_min_size(vol_track.min, Vec2::new(vol_track.width() * audio.volume, 5.0)), 0.0, PHOSPHOR_GREEN);
    let vol_resp = ui.allocate_rect(vol_track, Sense::click_and_drag());
    if vol_resp.dragged() || vol_resp.clicked() {
        if let Some(mouse) = ui.input(|i| i.pointer.interact_pos()) {
            let r = ((mouse.x - vol_track.min.x) / vol_track.width()).clamp(0.0, 1.0);
            audio.set_volume(r);
        }
    }

    // EQ / PL Buttons
    let eq_btn = Rect::from_min_size(state.position + Vec2::new(116.0, 80.0), Vec2::new(26.0, 11.0));
    let eq_resp = ui.allocate_rect(eq_btn, Sense::click());
    if eq_resp.clicked() { state.show_eq = !state.show_eq; }
    painter.text(eq_btn.center(), egui::Align2::CENTER_CENTER, if state.show_eq { "[EQ*]" } else { "[EQ]" }, FontId::new(8.0, FontFamily::Monospace), if state.show_eq { PHOSPHOR_GREEN } else { PHOSPHOR_DIM });

    let pl_btn = Rect::from_min_size(state.position + Vec2::new(146.0, 80.0), Vec2::new(26.0, 11.0));
    let pl_resp = ui.allocate_rect(pl_btn, Sense::click());
    if pl_resp.clicked() { state.show_playlist = !state.show_playlist; }
    painter.text(pl_btn.center(), egui::Align2::CENTER_CENTER, if state.show_playlist { "[PL*]" } else { "[PL]" }, FontId::new(8.0, FontFamily::Monospace), if state.show_playlist { PHOSPHOR_GREEN } else { PHOSPHOR_DIM });

    // Shuf / Rep
    let shuf_btn = Rect::from_min_size(state.position + Vec2::new(180.0, 80.0), Vec2::new(36.0, 11.0));
    let shuf_resp = ui.allocate_rect(shuf_btn, Sense::click());
    if shuf_resp.clicked() { state.shuffle = !state.shuffle; }
    painter.text(shuf_btn.center(), egui::Align2::CENTER_CENTER, if state.shuffle { "[SHF*]" } else { "[SHF]" }, FontId::new(8.0, FontFamily::Monospace), if state.shuffle { AMBER_ALERT } else { PHOSPHOR_DIM });

    let rep_btn = Rect::from_min_size(state.position + Vec2::new(220.0, 80.0), Vec2::new(36.0, 11.0));
    let rep_resp = ui.allocate_rect(rep_btn, Sense::click());
    if rep_resp.clicked() { state.repeat = !state.repeat; }
    painter.text(rep_btn.center(), egui::Align2::CENTER_CENTER, if state.repeat { "[REP*]" } else { "[REP]" }, FontId::new(8.0, FontFamily::Monospace), if state.repeat { AMBER_ALERT } else { PHOSPHOR_DIM });

    // 6. Hacker Terminal Command Buttons (y: 95)
    let term_cmds = [
        ("[<<]", MainAction::Prev),
        ("[PLAY]", MainAction::Play),
        ("[PAUS]", MainAction::Pause),
        ("[STOP]", MainAction::Stop),
        ("[>>]", MainAction::Next),
        ("[^EJ]", MainAction::Eject),
    ];

    for (idx, (cmd, act)) in term_cmds.iter().enumerate() {
        let bx = state.position.x + 6.0 + (idx as f32) * 44.0;
        let by = state.position.y + 94.0;
        let c_rect = Rect::from_min_size(Pos2::new(bx, by), Vec2::new(40.0, 16.0));
        let c_resp = ui.allocate_rect(c_rect, Sense::click());

        let is_active = (*cmd == "[PLAY]" && audio.state == PlaybackState::Playing) || (*cmd == "[PAUS]" && audio.state == PlaybackState::Paused);

        if c_resp.hovered() || is_active {
            painter.rect_filled(c_rect, 1.0, PHOSPHOR_GREEN);
            painter.text(c_rect.center(), egui::Align2::CENTER_CENTER, cmd, FontId::new(9.0, FontFamily::Monospace), Color32::BLACK);
        } else {
            painter.rect_stroke(c_rect, 1.0, Stroke::new(1.0_f32, PHOSPHOR_DIM), egui::StrokeKind::Inside);
            painter.text(c_rect.center(), egui::Align2::CENTER_CENTER, cmd, FontId::new(9.0, FontFamily::Monospace), PHOSPHOR_GREEN);
        }

        if c_resp.clicked() { action = *act; }
    }

    action
}

pub fn render_cyberpunk_eq(
    ui: &mut Ui,
    state: &mut EqWindowState,
    audio: &mut AudioEngine,
) -> EqAction {
    let mut action = EqAction::None;
    let window_rect = Rect::from_min_size(state.position, Vec2::new(EQ_WIDTH, EQ_HEIGHT));

    // Dragging
    let titlebar_rect = Rect::from_min_size(state.position, Vec2::new(EQ_WIDTH - 20.0, 16.0));
    let titlebar_resp = ui.allocate_rect(titlebar_rect, Sense::click_and_drag());
    if titlebar_resp.drag_started() {
        if let Some(mouse) = ui.input(|i| i.pointer.interact_pos()) {
            state.drag_offset = mouse - state.position;
            state.is_dragging = true;
        }
    }
    if state.is_dragging {
        if ui.input(|i| i.pointer.any_down()) {
            if let Some(mouse) = ui.input(|i| i.pointer.interact_pos()) {
                state.position = mouse - state.drag_offset;
            }
        } else {
            state.is_dragging = false;
        }
    }

    let painter = ui.painter_at(window_rect);

    // Chassis
    painter.rect_filled(window_rect, 2.0, CRT_BG);
    painter.rect_stroke(window_rect, 2.0, Stroke::new(1.5_f32, CRT_BORDER), egui::StrokeKind::Inside);

    // Titlebar
    painter.rect_filled(Rect::from_min_size(state.position, Vec2::new(EQ_WIDTH, 16.0)), 2.0, Color32::from_rgb(0, 40, 20));
    painter.text(
        state.position + Vec2::new(8.0, 2.0),
        egui::Align2::LEFT_TOP,
        "// DSP_TELEMETRY: 10-BAND EQ //",
        FontId::new(9.0, FontFamily::Monospace),
        PHOSPHOR_GREEN,
    );

    // Close [X]
    let close_btn = Rect::from_min_size(state.position + Vec2::new(EQ_WIDTH - 15.0, 2.0), Vec2::new(14.0, 12.0));
    let close_resp = ui.allocate_rect(close_btn, Sense::click());
    painter.text(close_btn.center(), egui::Align2::CENTER_CENTER, "[X]", FontId::new(8.0, FontFamily::Monospace), if close_resp.hovered() { Color32::RED } else { PHOSPHOR_GREEN });
    if close_resp.clicked() { action = EqAction::Close; }

    // ON Toggle
    let on_btn = Rect::from_min_size(state.position + Vec2::new(8.0, 20.0), Vec2::new(32.0, 12.0));
    let on_resp = ui.allocate_rect(on_btn, Sense::click());
    if on_resp.clicked() { audio.eq.enabled = !audio.eq.enabled; }
    painter.text(on_btn.center(), egui::Align2::CENTER_CENTER, if audio.eq.enabled { "[PWR:1]" } else { "[PWR:0]" }, FontId::new(8.0, FontFamily::Monospace), if audio.eq.enabled { PHOSPHOR_GREEN } else { PHOSPHOR_DIM });

    // [PRESETS] — classic Winamp preset list
    let presets_btn = Rect::from_min_size(state.position + Vec2::new(46.0, 20.0), Vec2::new(70.0, 12.0));
    let presets_resp = ui.allocate_rect(presets_btn, Sense::click());
    painter.text(presets_btn.center(), egui::Align2::CENTER_CENTER, "[PRESETS]", FontId::new(8.0, FontFamily::Monospace), if presets_resp.hovered() || state.show_presets { Color32::WHITE } else { PHOSPHOR_DIM });
    if presets_resp.clicked() {
        state.show_presets = !state.show_presets;
    }

    // Preamp slider (x: 10, y: 38, h: 62)
    let pre_rect = Rect::from_min_size(state.position + Vec2::new(14.0, 38.0), Vec2::new(12.0, 62.0));
    painter.rect_filled(pre_rect, 0.0, Color32::from_rgb(0, 20, 10));
    let pre_resp = ui.allocate_rect(pre_rect, Sense::click_and_drag());
    if pre_resp.dragged() || pre_resp.clicked() {
        if let Some(mouse) = ui.input(|i| i.pointer.interact_pos()) {
            let r = 1.0 - ((mouse.y - pre_rect.min.y) / pre_rect.height()).clamp(0.0, 1.0);
            audio.eq.preamp = r * 24.0 - 12.0;
        }
    }
    let pre_thumb_y = pre_rect.max.y - ((audio.eq.preamp + 12.0) / 24.0) * pre_rect.height();
    painter.rect_filled(Rect::from_center_size(Pos2::new(pre_rect.center().x, pre_thumb_y), Vec2::new(10.0, 4.0)), 0.0, AMBER_ALERT);
    painter.text(state.position + Vec2::new(20.0, 102.0), egui::Align2::CENTER_TOP, "PRE", FontId::new(7.0, FontFamily::Monospace), PHOSPHOR_DIM);

    // 10 Frequency Faders (x: 48..250)
    let band_xs = [50.0, 72.0, 94.0, 116.0, 138.0, 160.0, 182.0, 204.0, 226.0, 248.0];
    let freq_labels = ["60", "170", "310", "600", "1k", "3k", "6k", "12k", "14k", "16k"];

    for (i, (&bx, &lbl)) in band_xs.iter().zip(freq_labels.iter()).enumerate() {
        let b_rect = Rect::from_min_size(state.position + Vec2::new(bx, 38.0), Vec2::new(12.0, 62.0));
        painter.rect_filled(b_rect, 0.0, Color32::from_rgb(0, 20, 10));

        let b_resp = ui.allocate_rect(b_rect, Sense::click_and_drag());
        if b_resp.dragged() || b_resp.clicked() {
            if let Some(mouse) = ui.input(|i| i.pointer.interact_pos()) {
                let r = 1.0 - ((mouse.y - b_rect.min.y) / b_rect.height()).clamp(0.0, 1.0);
                audio.eq.bands[i] = r * 24.0 - 12.0;
            }
        }

        let thumb_y = b_rect.max.y - ((audio.eq.bands[i] + 12.0) / 24.0) * b_rect.height();
        painter.rect_filled(Rect::from_center_size(Pos2::new(b_rect.center().x, thumb_y), Vec2::new(10.0, 4.0)), 0.0, PHOSPHOR_GREEN);
        painter.text(state.position + Vec2::new(bx + 6.0, 102.0), egui::Align2::CENTER_TOP, lbl, FontId::new(7.0, FontFamily::Monospace), PHOSPHOR_DIM);
    }

    action
}

pub fn render_cyberpunk_playlist(
    ui: &mut Ui,
    state: &mut PlaylistWindowState,
    current_playing_index: Option<usize>,
    _audio: &AudioEngine,
) -> PlaylistAction {
    let mut action = PlaylistAction::None;
    let window_rect = Rect::from_min_size(state.position, Vec2::new(PL_DEFAULT_WIDTH, state.height));

    // Dragging
    let titlebar_rect = Rect::from_min_size(state.position, Vec2::new(PL_DEFAULT_WIDTH - 20.0, 16.0));
    let titlebar_resp = ui.allocate_rect(titlebar_rect, Sense::click_and_drag());
    if titlebar_resp.drag_started() {
        if let Some(mouse) = ui.input(|i| i.pointer.interact_pos()) {
            state.drag_offset = mouse - state.position;
            state.is_dragging = true;
        }
    }
    if state.is_dragging {
        if ui.input(|i| i.pointer.any_down()) {
            if let Some(mouse) = ui.input(|i| i.pointer.interact_pos()) {
                state.position = mouse - state.drag_offset;
            }
        } else {
            state.is_dragging = false;
        }
    }

    let painter = ui.painter_at(window_rect);

    // Chassis
    painter.rect_filled(window_rect, 2.0, CRT_BG);
    painter.rect_stroke(window_rect, 2.0, Stroke::new(1.5_f32, CRT_BORDER), egui::StrokeKind::Inside);

    // Titlebar
    painter.rect_filled(Rect::from_min_size(state.position, Vec2::new(PL_DEFAULT_WIDTH, 16.0)), 2.0, Color32::from_rgb(0, 40, 20));
    let count_str = format!("// QUEUE_INDEX: {} ITEMS //", state.items.len());
    painter.text(
        state.position + Vec2::new(8.0, 2.0),
        egui::Align2::LEFT_TOP,
        &count_str,
        FontId::new(9.0, FontFamily::Monospace),
        PHOSPHOR_GREEN,
    );

    // Close [X]
    let close_btn = Rect::from_min_size(state.position + Vec2::new(PL_DEFAULT_WIDTH - 15.0, 2.0), Vec2::new(14.0, 12.0));
    let close_resp = ui.allocate_rect(close_btn, Sense::click());
    painter.text(close_btn.center(), egui::Align2::CENTER_CENTER, "[X]", FontId::new(8.0, FontFamily::Monospace), if close_resp.hovered() { Color32::RED } else { PHOSPHOR_GREEN });
    if close_resp.clicked() { action = PlaylistAction::Close; }

    // List Area
    let list_area = Rect::from_min_max(
        state.position + Vec2::new(6.0, 18.0),
        state.position + Vec2::new(PL_DEFAULT_WIDTH - 6.0, state.height - 24.0),
    );
    painter.rect_filled(list_area, 0.0, Color32::from_rgb(2, 8, 4));
    painter.rect_stroke(list_area, 0.0, Stroke::new(1.0_f32, PHOSPHOR_DIM), egui::StrokeKind::Inside);

    let line_height = 14.0;
    let visible_lines = ((list_area.height() / line_height).floor() as usize).max(1);

    // Mouse wheel scrolling
    if ui.rect_contains_pointer(list_area) {
        let scroll_delta = ui.input(|i| i.raw_scroll_delta.y);
        if scroll_delta > 0.0 && state.scroll_offset > 0 {
            state.scroll_offset = state.scroll_offset.saturating_sub(1);
        } else if scroll_delta < 0.0 && state.scroll_offset + visible_lines < state.items.len() {
            state.scroll_offset = (state.scroll_offset + 1).min(state.items.len().saturating_sub(visible_lines));
        }
    }

    for (i, entry_idx) in (state.scroll_offset..state.items.len()).take(visible_lines).enumerate() {
        let entry = &state.items[entry_idx];
        let y = list_area.min.y + 2.0 + (i as f32) * line_height;
        let item_rect = Rect::from_min_size(Pos2::new(list_area.min.x + 2.0, y), Vec2::new(list_area.width() - 4.0, line_height));

        let is_selected = state.selected_index == Some(entry_idx);
        let is_playing = current_playing_index == Some(entry_idx);

        if is_selected {
            painter.rect_filled(item_rect, 0.0, Color32::from_rgb(0, 60, 30));
        }

        let item_resp = ui.allocate_rect(item_rect, Sense::click());
        if item_resp.clicked() { state.selected_index = Some(entry_idx); }
        if item_resp.double_clicked() { action = PlaylistAction::PlayIndex(entry_idx); }

        let prefix = if is_playing { ">" } else { " " };
        let track_line = format!("{} {:02}. {}", prefix, entry_idx + 1, entry.title);
        let dur_str = format!("{:02}:{:02}", entry.duration_secs / 60, entry.duration_secs % 60);

        painter.text(
            item_rect.min + Vec2::new(2.0, 1.0),
            egui::Align2::LEFT_TOP,
            &track_line,
            FontId::new(9.0, FontFamily::Monospace),
            if is_playing { Color32::WHITE } else if is_selected { PHOSPHOR_GREEN } else { PHOSPHOR_DIM },
        );

        painter.text(
            item_rect.max - Vec2::new(4.0, 13.0),
            egui::Align2::RIGHT_TOP,
            &dur_str,
            FontId::new(9.0, FontFamily::Monospace),
            if is_playing { Color32::WHITE } else { PHOSPHOR_DIM },
        );
    }

    // Command buttons: [+ADD] [-DEL] [CLEAR]
    let add_rect = Rect::from_min_size(state.position + Vec2::new(6.0, state.height - 20.0), Vec2::new(48.0, 14.0));
    let add_resp = ui.allocate_rect(add_rect, Sense::click());
    painter.text(add_rect.center(), egui::Align2::CENTER_CENTER, "[+ADD]", FontId::new(8.5, FontFamily::Monospace), if add_resp.hovered() { Color32::WHITE } else { PHOSPHOR_GREEN });
    if add_resp.clicked() { action = PlaylistAction::AddFiles; }

    let rem_rect = Rect::from_min_size(state.position + Vec2::new(58.0, state.height - 20.0), Vec2::new(48.0, 14.0));
    let rem_resp = ui.allocate_rect(rem_rect, Sense::click());
    painter.text(rem_rect.center(), egui::Align2::CENTER_CENTER, "[-DEL]", FontId::new(8.5, FontFamily::Monospace), if rem_resp.hovered() { Color32::WHITE } else { PHOSPHOR_GREEN });
    if rem_resp.clicked() { action = PlaylistAction::RemoveSelected; }

    let clr_rect = Rect::from_min_size(state.position + Vec2::new(110.0, state.height - 20.0), Vec2::new(54.0, 14.0));
    let clr_resp = ui.allocate_rect(clr_rect, Sense::click());
    painter.text(clr_rect.center(), egui::Align2::CENTER_CENTER, "[CLEAR]", FontId::new(8.5, FontFamily::Monospace), if clr_resp.hovered() { Color32::WHITE } else { PHOSPHOR_GREEN });
    if clr_resp.clicked() { action = PlaylistAction::Clear; }

    // Resize grip at bottom edge (smooth: relative drag accumulation)
    let grip_rect = Rect::from_min_max(
        state.position + Vec2::new(13.0, state.height - 7.0),
        state.position + Vec2::new(PL_DEFAULT_WIDTH - 21.0, state.height),
    );
    let grip_resp = ui.allocate_rect(grip_rect, Sense::drag());
    grip_resp.clone().on_hover_cursor(egui::CursorIcon::ResizeVertical);
    if grip_resp.dragged() {
        const CHROME: f32 = 58.0;
        const ROW_H: f32 = 14.0;
        let rows = ((state.height - CHROME) / ROW_H).round();
        let new_rows = (rows + grip_resp.drag_delta().y / ROW_H).clamp(2.0, 64.0);
        state.height = CHROME + new_rows * ROW_H;
    }

    action
}
