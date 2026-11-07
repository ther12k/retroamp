//! "Winamp (Redrawn)" — a vector re-creation of the classic Winamp 2.x design.
//! Same geometry, colors, and beveled 90s look as the bitmap skin, but drawn
//! with egui primitives so it stays crisp at any zoom (1.5x, 2x, ...).

use crate::audio::{AudioEngine, PlaybackState, SPECTRUM_BARS, WAVEFORM_SAMPLES};
use crate::skin::{EQ_WIDTH, EQ_HEIGHT, MAIN_WIDTH, MAIN_HEIGHT, PL_DEFAULT_WIDTH};
use crate::ui::eq_window::{EqAction, EqWindowState};
use crate::ui::main_window::{MainAction, MainWindowState, VisMode};
use crate::ui::playlist_window::{PlaylistAction, PlaylistWindowState};
use egui::{Align2, Color32, FontFamily, FontId, Pos2, Rect, Sense, Stroke, Ui, Vec2};
use std::time::Duration;

// Classic palette
const TB_TOP: Color32 = Color32::from_rgb(46, 56, 92);
const TB_BOT: Color32 = Color32::from_rgb(24, 30, 52);
const CHASSIS_TOP: Color32 = Color32::from_rgb(41, 50, 82);
const CHASSIS_BOT: Color32 = Color32::from_rgb(24, 29, 48);
const EDGE_LIGHT: Color32 = Color32::from_rgb(84, 96, 134);
const EDGE_DARK: Color32 = Color32::from_rgb(8, 11, 20);
const LCD_BG: Color32 = Color32::from_rgb(0, 0, 0);
const LED_GREEN: Color32 = Color32::from_rgb(0, 232, 0);
const LED_DIM: Color32 = Color32::from_rgb(0, 96, 0);
const LED_AMBER: Color32 = Color32::from_rgb(255, 176, 0);
const BTN_TOP: Color32 = Color32::from_rgb(158, 164, 178);
const BTN_BOT: Color32 = Color32::from_rgb(104, 110, 126);
const BTN_GLYPH: Color32 = Color32::from_rgb(24, 28, 38);
const GOLD: Color32 = Color32::from_rgb(208, 178, 88);

fn vgrad(painter: &egui::Painter, rect: Rect, top: Color32, bot: Color32, rounding: f32) {
    painter.rect_filled(rect, rounding, top);
    let half = Rect::from_min_max(
        Pos2::new(rect.min.x, rect.center().y),
        rect.max,
    );
    painter.rect_filled(half, rounding, bot);
}

fn bevel(painter: &egui::Painter, rect: Rect, pressed: bool, rounding: f32) {
    vgrad(&painter, rect, if pressed { BTN_BOT } else { BTN_TOP }, if pressed { BTN_TOP } else { BTN_BOT }, rounding);
    painter.rect_stroke(rect, rounding, Stroke::new(1.0_f32, Color32::BLACK), egui::StrokeKind::Inside);
    let inner = rect.shrink(1.0);
    let (light, dark) = if pressed { (Color32::from_rgb(70, 76, 90), Color32::from_rgb(215, 220, 230)) } else { (Color32::from_rgb(215, 220, 230), Color32::from_rgb(70, 76, 90)) };
    painter.line_segment([Pos2::new(inner.min.x, inner.max.y), Pos2::new(inner.min.x, inner.min.y)], Stroke::new(1.0_f32, light));
    painter.line_segment([Pos2::new(inner.min.x, inner.min.y), Pos2::new(inner.max.x, inner.min.y)], Stroke::new(1.0_f32, light));
    painter.line_segment([Pos2::new(inner.max.x, inner.min.y), Pos2::new(inner.max.x, inner.max.y)], Stroke::new(1.0_f32, dark));
    painter.line_segment([Pos2::new(inner.min.x, inner.max.y), Pos2::new(inner.max.x, inner.max.y)], Stroke::new(1.0_f32, dark));
}

/// Small beveled button with a text label; returns click state.
fn text_btn(ui: &mut Ui, painter: &egui::Painter, rect: Rect, label: &str, size: f32, pressed: bool, glyph_color: Color32) -> bool {
    let resp = ui.allocate_rect(rect, Sense::click());
    bevel(&painter, rect, resp.is_pointer_button_down_on() || pressed, 2.0);
    painter.text(rect.center(), Align2::CENTER_CENTER, label, FontId::new(size, FontFamily::Proportional), if resp.is_pointer_button_down_on() { Color32::BLACK } else { glyph_color });
    resp.clicked()
}

fn chassis(painter: &egui::Painter, rect: Rect) {
    vgrad(&painter, rect, CHASSIS_TOP, CHASSIS_BOT, 0.0);
    painter.rect_stroke(rect, 0.0, Stroke::new(1.0_f32, EDGE_DARK), egui::StrokeKind::Inside);
    let inner = rect.shrink(1.0);
    painter.line_segment([Pos2::new(inner.min.x, inner.max.y), Pos2::new(inner.min.x, inner.min.y)], Stroke::new(1.0_f32, EDGE_LIGHT));
    painter.line_segment([Pos2::new(inner.min.x, inner.min.y), Pos2::new(inner.max.x, inner.min.y)], Stroke::new(1.0_f32, EDGE_LIGHT));
}

fn titlebar(painter: &egui::Painter, rect: Rect, title: &str) {
    vgrad(&painter, rect, TB_TOP, TB_BOT, 0.0);
    // embossed text
    painter.text(rect.center() + Vec2::new(0.5, 0.5), Align2::CENTER_CENTER, title, FontId::new(9.0, FontFamily::Proportional), Color32::from_rgb(10, 14, 24));
    painter.text(rect.center() + Vec2::new(-0.5, -0.5), Align2::CENTER_CENTER, title, FontId::new(9.0, FontFamily::Proportional), Color32::from_rgb(190, 200, 225));
    painter.line_segment([Pos2::new(rect.min.x, rect.max.y - 0.5), Pos2::new(rect.max.x, rect.max.y - 0.5)], Stroke::new(1.0_f32, EDGE_DARK));
}

// --- transport glyph drawing -------------------------------------------------

fn glyph_play(p: &egui::Painter, c: Pos2, s: f32, col: Color32) {
    p.add(egui::Shape::convex_polygon(
        vec![Pos2::new(c.x - s * 0.5, c.y - s * 0.7), Pos2::new(c.x - s * 0.5, c.y + s * 0.7), Pos2::new(c.x + s * 0.8, c.y)],
        col,
        Stroke::NONE,
    ));
}
fn glyph_pause(p: &egui::Painter, c: Pos2, s: f32, col: Color32) {
    for dx in [-0.45_f32, 0.15] {
        p.rect_filled(Rect::from_min_size(Pos2::new(c.x + dx * s, c.y - s * 0.65), Vec2::new(s * 0.3, s * 1.3)), 0.5, col);
    }
}
fn glyph_stop(p: &egui::Painter, c: Pos2, s: f32, col: Color32) {
    p.rect_filled(Rect::from_center_size(c, Vec2::new(s * 1.1, s * 1.1)), 0.5, col);
}
fn glyph_prev(p: &egui::Painter, c: Pos2, s: f32, col: Color32) {
    p.add(egui::Shape::convex_polygon(
        vec![Pos2::new(c.x + s * 0.6, c.y - s * 0.7), Pos2::new(c.x + s * 0.6, c.y + s * 0.7), Pos2::new(c.x - s * 0.1, c.y)],
        col, Stroke::NONE,
    ));
    p.rect_filled(Rect::from_min_size(Pos2::new(c.x - s * 0.65, c.y - s * 0.7), Vec2::new(s * 0.22, s * 1.4)), 0.0, col);
}
fn glyph_next(p: &egui::Painter, c: Pos2, s: f32, col: Color32) {
    p.add(egui::Shape::convex_polygon(
        vec![Pos2::new(c.x - s * 0.6, c.y - s * 0.7), Pos2::new(c.x - s * 0.6, c.y + s * 0.7), Pos2::new(c.x + s * 0.1, c.y)],
        col, Stroke::NONE,
    ));
    p.rect_filled(Rect::from_min_size(Pos2::new(c.x + s * 0.43, c.y - s * 0.7), Vec2::new(s * 0.22, s * 1.4)), 0.0, col);
}
fn glyph_eject(p: &egui::Painter, c: Pos2, s: f32, col: Color32) {
    p.add(egui::Shape::convex_polygon(
        vec![Pos2::new(c.x - s * 0.6, c.y - s * 0.1), Pos2::new(c.x + s * 0.6, c.y - s * 0.1), Pos2::new(c.x, c.y - s * 0.8)],
        col, Stroke::NONE,
    ));
    p.rect_filled(Rect::from_min_size(Pos2::new(c.x - s * 0.6, c.y + s * 0.25), Vec2::new(s * 1.2, s * 0.25)), 0.0, col);
}

// --- shared widgets -----------------------------------------------------------

fn slider_track(painter: &egui::Painter, rect: Rect) {
    painter.rect_filled(rect, rect.height() / 2.0, Color32::from_rgb(10, 13, 22));
    painter.rect_stroke(rect, rect.height() / 2.0, Stroke::new(1.0_f32, EDGE_DARK), egui::StrokeKind::Inside);
}

fn slider_thumb(painter: &egui::Painter, center: Pos2, size: Vec2) {
    let rect = Rect::from_center_size(center, size);
    vgrad(&painter, rect, BTN_TOP, BTN_BOT, 1.5);
    painter.rect_stroke(rect, 1.5, Stroke::new(1.0_f32, Color32::BLACK), egui::StrokeKind::Inside);
    painter.line_segment([Pos2::new(center.x, rect.min.y + 2.0), Pos2::new(center.x, rect.max.y - 2.0)], Stroke::new(1.0_f32, Color32::from_rgb(40, 46, 60)));
}

fn draw_visualizer(painter: &egui::Painter, rect: Rect, audio: &AudioEngine, mode: VisMode) {
    painter.rect_filled(rect, 0.0, LCD_BG);
    painter.rect_stroke(rect, 0.0, Stroke::new(1.0_f32, Color32::from_rgb(20, 26, 14)), egui::StrokeKind::Inside);
    match mode {
        VisMode::Spectrum => {
            let (_, spectrum, peaks) = audio.get_vis_data();
            let bw = 3.0;
            let gap = 1.0;
            for i in 0..SPECTRUM_BARS {
                let x = rect.min.x + i as f32 * (bw + gap);
                if x + bw > rect.max.x { break; }
                let h = (spectrum[i].clamp(0.0, 1.0) * (rect.height() - 1.0)).round();
                for step in 0..(h as i32) {
                    let y = rect.max.y - 1.0 - step as f32;
                    let t = step as f32 / (rect.height() - 2.0).max(1.0);
                    let col = Color32::from_rgb(0, (150.0 + t * 105.0) as u8, 0);
                    painter.rect_filled(Rect::from_min_size(Pos2::new(x, y), Vec2::new(bw, 1.0)), 0.0, col);
                }
                let ph = (peaks[i] * (rect.height() - 1.0)).round();
                if ph > 0.0 {
                    painter.rect_filled(Rect::from_min_size(Pos2::new(x, (rect.max.y - 1.0 - ph).max(rect.min.y)), Vec2::new(bw, 1.0)), 0.0, Color32::from_rgb(190, 255, 190));
                }
            }
        }
        VisMode::Oscilloscope => {
            let (wave, _, _) = audio.get_vis_data();
            let mid = rect.center().y;
            for i in 0..WAVEFORM_SAMPLES.min(rect.width() as usize) {
                let x = rect.min.x + i as f32;
                let y = (mid - wave[i] * (rect.height() / 2.0 - 1.0)).clamp(rect.min.y, rect.max.y - 1.0);
                painter.rect_filled(Rect::from_min_size(Pos2::new(x, y), Vec2::new(1.0, 1.0)), 0.0, LED_GREEN);
            }
        }
        VisMode::Off => {}
    }
}

// ============================================================================
// MAIN WINDOW
// ============================================================================

pub fn render_vector_main(ui: &mut Ui, state: &mut MainWindowState, audio: &mut AudioEngine) -> MainAction {
    let mut action = MainAction::None;
    let window_rect = Rect::from_min_size(state.position, Vec2::new(MAIN_WIDTH, MAIN_HEIGHT));
    let painter = ui.painter_at(window_rect);
    let pos = state.position;

    // Titlebar: drag + menu hotspot + window buttons
    let tb = Rect::from_min_size(pos, Vec2::new(MAIN_WIDTH, 14.0));
    let titlebar_resp = ui.allocate_rect(Rect::from_min_max(tb.min, Pos2::new(tb.max.x - 40.0, tb.max.y)), Sense::click_and_drag());
    if titlebar_resp.drag_started() { state.os_drag_started = true; }
    titlebar(&painter, tb, "W I N A M P");

    let menu_rect = Rect::from_min_size(pos + Vec2::new(10.0, 2.0), Vec2::new(26.0, 10.0));
    let menu_resp = ui.allocate_rect(menu_rect, Sense::click());
    if menu_resp.clicked() { state.menu_open = !state.menu_open; state.menu_frames = 0; }

    // Minimize / Close (vector: _ and x glyphs on beveled mini-buttons)
    let min_rect = Rect::from_min_size(pos + Vec2::new(243.0, 2.0), Vec2::new(10.0, 10.0));
    let min_resp = ui.allocate_rect(min_rect, Sense::click());
    bevel(&painter, min_rect, min_resp.is_pointer_button_down_on(), 1.0);
    painter.line_segment([Pos2::new(min_rect.min.x + 2.5, min_rect.max.y - 3.0), Pos2::new(min_rect.max.x - 2.5, min_rect.max.y - 3.0)], Stroke::new(1.2_f32, BTN_GLYPH));
    if min_resp.clicked() { action = MainAction::Minimize; }

    let close_rect = Rect::from_min_size(pos + Vec2::new(263.0, 2.0), Vec2::new(10.0, 10.0));
    let close_resp = ui.allocate_rect(close_rect, Sense::click());
    bevel(&painter, close_rect, close_resp.is_pointer_button_down_on(), 1.0);
    painter.line_segment([Pos2::new(close_rect.min.x + 2.5, close_rect.min.y + 2.5), Pos2::new(close_rect.max.x - 2.5, close_rect.max.y - 2.5)], Stroke::new(1.2_f32, BTN_GLYPH));
    painter.line_segment([Pos2::new(close_rect.max.x - 2.5, close_rect.min.y + 2.5), Pos2::new(close_rect.min.x + 2.5, close_rect.max.y - 2.5)], Stroke::new(1.2_f32, BTN_GLYPH));
    if close_resp.clicked() { action = MainAction::Close; }

    chassis(&painter, Rect::from_min_max(Pos2::new(window_rect.min.x, tb.max.y), window_rect.max));

    // Playback indicator (24,28)
    let ind_c = pos + Vec2::new(28.5, 32.5);
    match audio.state {
        PlaybackState::Playing => glyph_play(&painter, ind_c, 5.0, LED_GREEN),
        PlaybackState::Paused => glyph_pause(&painter, ind_c, 5.0, LED_GREEN),
        PlaybackState::Stopped => glyph_stop(&painter, ind_c, 4.0, LED_DIM),
    }

    // LCD display plate (34..250, 22..40) behind time+marquee
    let lcd = Rect::from_min_size(pos + Vec2::new(34.0, 22.0), Vec2::new(216.0, 19.0));
    painter.rect_filled(lcd, 0.0, LCD_BG);

    // Time digits (36,24) big mono, click toggles remaining mode
    let time_area = Rect::from_min_size(pos + Vec2::new(36.0, 22.0), Vec2::new(64.0, 19.0));
    let time_resp = ui.allocate_rect(time_area, Sense::click());
    if time_resp.clicked() { state.time_remaining_mode = !state.time_remaining_mode; }
    let secs = if state.time_remaining_mode {
        audio.current_track.as_ref().map(|t| if t.duration > audio.position { (t.duration - audio.position).as_secs() } else { 0 }).unwrap_or(0)
    } else {
        audio.position.as_secs()
    };
    let (mins, secs) = ((secs / 60).min(99), secs % 60);
    let time_str = if state.time_remaining_mode { format!("-{:02}:{:02}", mins, secs) } else { format!("{:02}:{:02}", mins, secs) };
    painter.text(time_area.min + Vec2::new(2.0, 1.0), Align2::LEFT_TOP, &time_str, FontId::new(15.0, FontFamily::Monospace), LED_GREEN);

    // Marquee (112,24 .. 246) green mono, clipped
    let mq = Rect::from_min_max(pos + Vec2::new(104.0, 24.0), Pos2::new(lcd.max.x - 4.0, lcd.max.y - 3.0));
    painter.rect_filled(mq, 0.0, LCD_BG);
    let title_text = if let Some(t) = &audio.current_track {
        format!("{}. {} ***   ", 1, t.title)
    } else {
        "WINAMP REDRAWN - IT REALLY WHIPS THE LLAMA'S ASS!   ***   ".to_string()
    };
    if state.last_marquee_tick.elapsed() >= Duration::from_millis(160) {
        state.marquee_offset = (state.marquee_offset + 1) % title_text.chars().count().max(1);
        state.last_marquee_tick = std::time::Instant::now();
    }
    let chars: Vec<char> = title_text.chars().collect();
    let visible = ((mq.width() / 5.4) as usize).min(40);
    let mut mx = mq.min.x;
    for i in 0..visible {
        let ch = chars[(state.marquee_offset + i) % chars.len()];
        painter.text(Pos2::new(mx, mq.min.y + 1.0), Align2::LEFT_TOP, ch.to_string(), FontId::new(7.0, FontFamily::Monospace), LED_GREEN);
        mx += 5.4;
    }

    // Bitrate / kHz badges (111,43)(156,43)
    let (kbps, khz) = audio.current_track.as_ref().map(|t| (t.bitrate_kbps, t.sample_rate / 1000)).unwrap_or((128, 44));
    for (x, w, text) in [(110.0, 24.0, format!("{:3}", kbps)), (155.0, 17.0, format!("{:2}", khz))] {
        let r = Rect::from_min_size(pos + Vec2::new(x, 42.0), Vec2::new(w, 10.0));
        painter.rect_filled(r, 0.0, LCD_BG);
        painter.text(r.min + Vec2::new(2.0, 0.5), Align2::LEFT_TOP, &text, FontId::new(8.0, FontFamily::Monospace), LED_GREEN);
    }
    painter.text(pos + Vec2::new(136.0, 43.0), Align2::LEFT_TOP, "kbps", FontId::new(7.0, FontFamily::Proportional), LED_DIM);
    painter.text(pos + Vec2::new(174.0, 43.0), Align2::LEFT_TOP, "kHz", FontId::new(7.0, FontFamily::Proportional), LED_DIM);

    // Mono / Stereo indicators
    let stereo = audio.current_track.as_ref().map(|t| t.channels >= 2).unwrap_or(true);
    painter.text(pos + Vec2::new(213.0, 42.0), Align2::LEFT_TOP, "MONO", FontId::new(8.0, FontFamily::Proportional), if !stereo { LED_GREEN } else { LED_DIM });
    painter.text(pos + Vec2::new(240.0, 42.0), Align2::LEFT_TOP, "STEREO", FontId::new(8.0, FontFamily::Proportional), if stereo { LED_GREEN } else { LED_DIM });

    // Visualizer (24,53,76x16)
    let vis_rect = Rect::from_min_size(pos + Vec2::new(24.0, 53.0), Vec2::new(76.0, 16.0));
    let vis_resp = ui.allocate_rect(vis_rect, Sense::click());
    if vis_resp.clicked() {
        state.vis_mode = match state.vis_mode { VisMode::Spectrum => VisMode::Oscilloscope, VisMode::Oscilloscope => VisMode::Off, VisMode::Off => VisMode::Spectrum };
    }
    draw_visualizer(&painter, vis_rect, audio, state.vis_mode);

    // Volume (107,57,68x13) & Balance (177,57,38x13)
    let vol_rect = Rect::from_min_size(pos + Vec2::new(107.0, 57.0), Vec2::new(68.0, 11.0));
    slider_track(&painter, vol_rect);
    let vol_resp = ui.allocate_rect(vol_rect, Sense::click_and_drag());
    if vol_resp.dragged() || vol_resp.clicked() {
        if let Some(m) = ui.input(|i| i.pointer.interact_pos()) {
            audio.set_volume(((m.x - vol_rect.min.x) / vol_rect.width()).clamp(0.0, 1.0));
        }
    }
    slider_thumb(&painter, Pos2::new(vol_rect.min.x + audio.volume * (vol_rect.width() - 12.0) + 6.0, vol_rect.center().y), Vec2::new(12.0, 9.0));

    let bal_rect = Rect::from_min_size(pos + Vec2::new(177.0, 57.0), Vec2::new(38.0, 11.0));
    slider_track(&painter, bal_rect);
    let bal_resp = ui.allocate_rect(bal_rect, Sense::click_and_drag());
    if bal_resp.dragged() || bal_resp.clicked() {
        if let Some(m) = ui.input(|i| i.pointer.interact_pos()) {
            audio.set_balance((((m.x - bal_rect.min.x) / bal_rect.width()).clamp(0.0, 1.0)) * 2.0 - 1.0);
        }
    }
    slider_thumb(&painter, Pos2::new(bal_rect.min.x + ((audio.balance + 1.0) / 2.0) * (bal_rect.width() - 12.0) + 6.0, bal_rect.center().y), Vec2::new(12.0, 9.0));

    // EQ / PL toggles
    let eq_btn = Rect::from_min_size(pos + Vec2::new(219.0, 56.0), Vec2::new(23.0, 12.0));
    if text_btn(ui, &painter, eq_btn, "EQ", 8.0, state.show_eq, if state.show_eq { LED_GREEN } else { BTN_GLYPH }) { state.show_eq = !state.show_eq; action = MainAction::ToggleEq; }
    let pl_btn = Rect::from_min_size(pos + Vec2::new(245.0, 56.0), Vec2::new(23.0, 12.0));
    if text_btn(ui, &painter, pl_btn, "PL", 8.0, state.show_playlist, if state.show_playlist { LED_GREEN } else { BTN_GLYPH }) { state.show_playlist = !state.show_playlist; action = MainAction::TogglePlaylist; }

    // Seek bar (16,72,248x8)
    let seek_rect = Rect::from_min_size(pos + Vec2::new(16.0, 72.0), Vec2::new(248.0, 8.0));
    slider_track(&painter, seek_rect);
    let progress = audio.current_track.as_ref()
        .filter(|t| t.duration > Duration::ZERO)
        .map(|t| (audio.position.as_secs_f32() / t.duration.as_secs_f32()).clamp(0.0, 1.0))
        .unwrap_or(0.0);
    let seek_resp = ui.allocate_rect(seek_rect, Sense::click_and_drag());
    if seek_resp.dragged() || seek_resp.clicked() {
        if let Some(m) = ui.input(|i| i.pointer.interact_pos()) {
            state.seek_target_ratio = Some(((m.x - seek_rect.min.x) / seek_rect.width()).clamp(0.0, 1.0));
        }
    }
    slider_thumb(&painter, Pos2::new(seek_rect.min.x + progress * (seek_rect.width() - 16.0) + 8.0, seek_rect.center().y), Vec2::new(16.0, 8.0));

    // Transport buttons (y: 84)
    let transport: [(&str, MainAction, f32); 6] = [
        ("prev", MainAction::Prev, 16.0), ("play", MainAction::Play, 39.0), ("pause", MainAction::Pause, 62.0),
        ("stop", MainAction::Stop, 85.0), ("next", MainAction::Next, 108.0), ("eject", MainAction::Eject, 136.0),
    ];
    for (name, act, x) in transport {
        let size = if name == "eject" { Vec2::new(22.0, 16.0) } else { Vec2::new(23.0, 18.0) };
        let r = Rect::from_min_size(pos + Vec2::new(x, if name == "eject" { 86.0 } else { 84.0 }), size);
        let resp = ui.allocate_rect(r, Sense::click());
        bevel(&painter, r, resp.is_pointer_button_down_on(), 2.0);
        let c = r.center();
        match name {
            "prev" => glyph_prev(&painter, c, 6.0, BTN_GLYPH),
            "play" => glyph_play(&painter, c, 6.0, BTN_GLYPH),
            "pause" => glyph_pause(&painter, c, 6.0, BTN_GLYPH),
            "stop" => glyph_stop(&painter, c, 5.0, BTN_GLYPH),
            "next" => glyph_next(&painter, c, 6.0, BTN_GLYPH),
            _ => glyph_eject(&painter, c, 6.0, BTN_GLYPH),
        }
        if resp.clicked() { action = act; }
    }

    // Shuffle (164,84,47x15) Repeat (210,84,28x15)
    let shuf_rect = Rect::from_min_size(pos + Vec2::new(164.0, 84.0), Vec2::new(47.0, 15.0));
    let shuf_resp = ui.allocate_rect(shuf_rect, Sense::click());
    if shuf_resp.clicked() { state.shuffle = !state.shuffle; }
    bevel(&painter, shuf_rect, shuf_resp.is_pointer_button_down_on(), 2.0);
    painter.text(shuf_rect.min + Vec2::new(4.0, 3.5), Align2::LEFT_TOP, "SHUFFLE", FontId::new(7.0, FontFamily::Proportional), BTN_GLYPH);
    painter.circle_filled(shuf_rect.max - Vec2::new(5.0, 4.5), 2.0, if state.shuffle { LED_GREEN } else { LED_DIM });

    let rep_rect = Rect::from_min_size(pos + Vec2::new(214.0, 84.0), Vec2::new(28.0, 15.0));
    let rep_resp = ui.allocate_rect(rep_rect, Sense::click());
    if rep_resp.clicked() { state.repeat = !state.repeat; }
    bevel(&painter, rep_rect, rep_resp.is_pointer_button_down_on(), 2.0);
    painter.circle_filled(rep_rect.center() + Vec2::new(-6.0, 0.0), 3.0, if state.repeat { LED_GREEN } else { LED_DIM });
    painter.circle_filled(rep_rect.center() + Vec2::new(0.5, 0.0), 3.0, if state.repeat { LED_DIM } else { LED_DIM });

    action
}

// ============================================================================
// EQUALIZER
// ============================================================================

pub fn render_vector_eq(ui: &mut Ui, state: &mut EqWindowState, audio: &mut AudioEngine) -> EqAction {
    let mut action = EqAction::None;
    let window_rect = Rect::from_min_size(state.position, Vec2::new(EQ_WIDTH, EQ_HEIGHT));
    let painter = ui.painter_at(window_rect);
    let pos = state.position;

    let tb = Rect::from_min_size(pos, Vec2::new(EQ_WIDTH, 14.0));
    let titlebar_resp = ui.allocate_rect(Rect::from_min_max(tb.min, Pos2::new(tb.max.x - 20.0, tb.max.y)), Sense::click_and_drag());
    if titlebar_resp.drag_started() {
        if let Some(m) = ui.input(|i| i.pointer.interact_pos()) { state.drag_offset = m - state.position; state.is_dragging = true; }
    }
    if state.is_dragging {
        if ui.input(|i| i.pointer.any_down()) {
            if let Some(m) = ui.input(|i| i.pointer.interact_pos()) { state.position = m - state.drag_offset; }
        } else { state.is_dragging = false; }
    }
    titlebar(&painter, tb, "W I N A M P   E Q U A L I Z E R");

    let close_rect = Rect::from_min_size(pos + Vec2::new(263.0, 2.0), Vec2::new(10.0, 10.0));
    let close_resp = ui.allocate_rect(close_rect, Sense::click());
    bevel(&painter, close_rect, close_resp.is_pointer_button_down_on(), 1.0);
    painter.line_segment([Pos2::new(close_rect.min.x + 2.5, close_rect.min.y + 2.5), Pos2::new(close_rect.max.x - 2.5, close_rect.max.y - 2.5)], Stroke::new(1.2_f32, BTN_GLYPH));
    painter.line_segment([Pos2::new(close_rect.max.x - 2.5, close_rect.min.y + 2.5), Pos2::new(close_rect.min.x + 2.5, close_rect.max.y - 2.5)], Stroke::new(1.2_f32, BTN_GLYPH));
    if close_resp.clicked() { action = EqAction::Close; }

    chassis(&painter, Rect::from_min_max(Pos2::new(window_rect.min.x, tb.max.y), window_rect.max));

    // ON button (14,18)
    let on_rect = Rect::from_min_size(pos + Vec2::new(14.0, 18.0), Vec2::new(26.0, 13.0));
    let on_resp = ui.allocate_rect(on_rect, Sense::click());
    if on_resp.clicked() { audio.eq.enabled = !audio.eq.enabled; }
    bevel(&painter, on_rect, on_resp.is_pointer_button_down_on(), 2.0);
    painter.text(on_rect.center(), Align2::CENTER_CENTER, "ON", FontId::new(8.5, FontFamily::Proportional), if audio.eq.enabled { LED_GREEN } else { BTN_GLYPH });

    // AUTO (cosmetic, disabled look)
    let auto_rect = Rect::from_min_size(pos + Vec2::new(44.0, 18.0), Vec2::new(32.0, 13.0));
    bevel(&painter, auto_rect, false, 2.0);
    painter.text(auto_rect.center(), Align2::CENTER_CENTER, "AUTO", FontId::new(8.0, FontFamily::Proportional), Color32::from_rgb(90, 96, 110));

    // PRESETS (204,18)
    let presets_rect = Rect::from_min_size(pos + Vec2::new(204.0, 18.0), Vec2::new(58.0, 13.0));
    let presets_resp = ui.allocate_rect(presets_rect, Sense::click());
    if presets_resp.clicked() { state.show_presets = !state.show_presets; }
    bevel(&painter, presets_rect, presets_resp.is_pointer_button_down_on() || state.show_presets, 2.0);
    painter.text(presets_rect.center(), Align2::CENTER_CENTER, "PRESETS", FontId::new(8.0, FontFamily::Proportional), BTN_GLYPH);

    // Graph (86,17,113x19)
    let graph = Rect::from_min_size(pos + Vec2::new(86.0, 17.0), Vec2::new(113.0, 19.0));
    painter.rect_filled(graph, 0.0, LCD_BG);
    painter.rect_stroke(graph, 0.0, Stroke::new(1.0_f32, EDGE_DARK), egui::StrokeKind::Inside);
    if audio.eq.enabled {
        let mut pts = Vec::with_capacity(114);
        for step in 0..114 {
            let ratio = step as f32 / 113.0;
            let b_idx = (ratio * 9.0).floor() as usize;
            let t = ratio * 9.0 - b_idx as f32;
            let b_next = (b_idx + 1).min(9);
            let db = audio.eq.bands[b_idx] * (1.0 - t) + audio.eq.bands[b_next] * t + audio.eq.preamp * 0.4;
            let y = (graph.max.y - 1.0 - ((db + 12.0) / 24.0) * (graph.height() - 2.0)).clamp(graph.min.y + 1.0, graph.max.y - 1.0);
            pts.push(Pos2::new(graph.min.x + ratio * graph.width(), y));
        }
        for w in pts.windows(2) {
            painter.line_segment([w[0], w[1]], Stroke::new(1.5_f32, LED_GREEN));
        }
    }

    // Preamp slider (21,38,14x63)
    let pre_rect = Rect::from_min_size(pos + Vec2::new(21.0, 38.0), Vec2::new(14.0, 63.0));
    painter.rect_filled(pre_rect, 0.0, Color32::from_rgb(13, 16, 28));
    painter.rect_stroke(pre_rect, 0.0, Stroke::new(1.0_f32, EDGE_DARK), egui::StrokeKind::Inside);
    let pre_resp = ui.allocate_rect(pre_rect, Sense::click_and_drag());
    if pre_resp.dragged() || pre_resp.clicked() {
        if let Some(m) = ui.input(|i| i.pointer.interact_pos()) {
            audio.eq.preamp = (1.0 - ((m.y - pre_rect.min.y) / pre_rect.height()).clamp(0.0, 1.0)) * 24.0 - 12.0;
        }
    }
    let pre_ty = pre_rect.max.y - 5.5 - ((audio.eq.preamp + 12.0) / 24.0) * (pre_rect.height() - 11.0);
    slider_thumb(&painter, Pos2::new(pre_rect.center().x, pre_ty), Vec2::new(12.0, 11.0));

    // 10 band sliders (x: 78 + i*18)
    let labels = ["60", "170", "310", "600", "1K", "3K", "6K", "12K", "14K", "16K"];
    for (i, lbl) in labels.iter().enumerate() {
        let bx = 78.0 + i as f32 * 18.0;
        let b_rect = Rect::from_min_size(pos + Vec2::new(bx, 38.0), Vec2::new(14.0, 63.0));
        painter.rect_filled(b_rect, 0.0, Color32::from_rgb(13, 16, 28));
        painter.rect_stroke(b_rect, 0.0, Stroke::new(1.0_f32, EDGE_DARK), egui::StrokeKind::Inside);

        // dashed 0 dB rail
        let mid_y = b_rect.center().y;
        let mut dx = b_rect.min.x + 1.5;
        while dx < b_rect.max.x - 1.5 {
            painter.line_segment([Pos2::new(dx, mid_y), Pos2::new((dx + 2.0).min(b_rect.max.x - 1.5), mid_y)], Stroke::new(1.0_f32, Color32::from_rgb(55, 64, 96)));
            dx += 4.0;
        }

        let b_resp = ui.allocate_rect(b_rect, Sense::click_and_drag());
        if b_resp.dragged() || b_resp.clicked() {
            if let Some(m) = ui.input(|i| i.pointer.interact_pos()) {
                audio.eq.bands[i] = (1.0 - ((m.y - b_rect.min.y) / b_rect.height()).clamp(0.0, 1.0)) * 24.0 - 12.0;
            }
        }
        let ty = b_rect.max.y - 5.5 - ((audio.eq.bands[i] + 12.0) / 24.0) * (b_rect.height() - 11.0);
        slider_thumb(&painter, Pos2::new(b_rect.center().x, ty), Vec2::new(12.0, 11.0));

        painter.text(Pos2::new(b_rect.center().x, pos.y + 104.0), Align2::CENTER_TOP, lbl, FontId::new(6.5, FontFamily::Proportional), Color32::from_rgb(170, 178, 200));
    }
    painter.text(Pos2::new(pre_rect.center().x, pos.y + 104.0), Align2::CENTER_TOP, "PRE", FontId::new(6.5, FontFamily::Proportional), Color32::from_rgb(170, 178, 200));

    // dB scale labels
    painter.text(pos + Vec2::new(52.0, 24.0), Align2::LEFT_TOP, "+12db", FontId::new(7.0, FontFamily::Monospace), Color32::from_rgb(150, 158, 180));
    painter.text(pos + Vec2::new(52.0, 61.0), Align2::LEFT_TOP, "0db", FontId::new(7.0, FontFamily::Monospace), Color32::from_rgb(150, 158, 180));
    painter.text(pos + Vec2::new(52.0, 92.0), Align2::LEFT_TOP, "-12db", FontId::new(7.0, FontFamily::Monospace), Color32::from_rgb(150, 158, 180));

    action
}

// ============================================================================
// PLAYLIST
// ============================================================================

pub fn render_vector_playlist(
    ui: &mut Ui,
    state: &mut PlaylistWindowState,
    current_playing_index: Option<usize>,
    _audio: &AudioEngine,
) -> PlaylistAction {
    let mut action = PlaylistAction::None;
    let window_rect = Rect::from_min_size(state.position, Vec2::new(PL_DEFAULT_WIDTH, state.height));
    let painter = ui.painter_at(window_rect);
    let pos = state.position;

    let tb = Rect::from_min_size(pos, Vec2::new(PL_DEFAULT_WIDTH, 14.0));
    let titlebar_resp = ui.allocate_rect(Rect::from_min_max(tb.min, Pos2::new(tb.max.x - 20.0, tb.max.y)), Sense::click_and_drag());
    if titlebar_resp.drag_started() {
        if let Some(m) = ui.input(|i| i.pointer.interact_pos()) { state.drag_offset = m - state.position; state.is_dragging = true; }
    }
    if state.is_dragging {
        if ui.input(|i| i.pointer.any_down()) {
            if let Some(m) = ui.input(|i| i.pointer.interact_pos()) { state.position = m - state.drag_offset; }
        } else { state.is_dragging = false; }
    }
    titlebar(&painter, tb, "W I N A M P   P L A Y L I S T");

    let close_rect = Rect::from_min_size(pos + Vec2::new(263.0, 2.0), Vec2::new(10.0, 10.0));
    let close_resp = ui.allocate_rect(close_rect, Sense::click());
    bevel(&painter, close_rect, close_resp.is_pointer_button_down_on(), 1.0);
    painter.line_segment([Pos2::new(close_rect.min.x + 2.5, close_rect.min.y + 2.5), Pos2::new(close_rect.max.x - 2.5, close_rect.max.y - 2.5)], Stroke::new(1.2_f32, BTN_GLYPH));
    painter.line_segment([Pos2::new(close_rect.max.x - 2.5, close_rect.min.y + 2.5), Pos2::new(close_rect.min.x + 2.5, close_rect.max.y - 2.5)], Stroke::new(1.2_f32, BTN_GLYPH));
    if close_resp.clicked() { action = PlaylistAction::Close; }

    chassis(&painter, Rect::from_min_max(Pos2::new(window_rect.min.x, tb.max.y), window_rect.max));

    // List area
    let list = Rect::from_min_max(pos + Vec2::new(8.0, 17.0), pos + Vec2::new(PL_DEFAULT_WIDTH - 8.0, state.height - 24.0));
    painter.rect_filled(list, 0.0, LCD_BG);
    painter.rect_stroke(list, 0.0, Stroke::new(1.0_f32, Color32::from_rgb(16, 20, 12)), egui::StrokeKind::Inside);

    let row_h = 13.0;
    let visible = ((list.height() / row_h).floor() as usize).max(1);
    if ui.rect_contains_pointer(list) {
        let d = ui.input(|i| i.raw_scroll_delta.y);
        if d > 0.0 && state.scroll_offset > 0 {
            state.scroll_offset = state.scroll_offset.saturating_sub(1);
        } else if d < 0.0 && state.scroll_offset + visible < state.items.len() {
            state.scroll_offset = (state.scroll_offset + 1).min(state.items.len().saturating_sub(visible));
        }
    }

    for (i, entry_idx) in (state.scroll_offset..state.items.len()).take(visible).enumerate() {
        let entry = &state.items[entry_idx];
        let y = list.min.y + (i as f32) * row_h;
        let row = Rect::from_min_size(Pos2::new(list.min.x + 1.0, y), Vec2::new(list.width() - 2.0, row_h));
        let selected = state.selected_index == Some(entry_idx);
        let playing = current_playing_index == Some(entry_idx);

        if selected {
            painter.rect_filled(row, 0.0, Color32::from_rgb(0, 0, 128));
        }
        let resp = ui.allocate_rect(row, Sense::click());
        if resp.clicked() { state.selected_index = Some(entry_idx); }
        if resp.double_clicked() { action = PlaylistAction::PlayIndex(entry_idx); }

        let col = if playing { Color32::from_rgb(0, 255, 64) } else if selected { Color32::WHITE } else { LED_GREEN };
        let text = format!("{:2}. {}", entry_idx + 1, entry.title);
        painter.text(row.min + Vec2::new(4.0, 2.0), Align2::LEFT_TOP, &text, FontId::new(9.0, FontFamily::Monospace), col);
        let dur = format!("{:02}:{:02}", entry.duration_secs / 60, entry.duration_secs % 60);
        painter.text(Pos2::new(row.max.x - 6.0, row.min.y + 2.0), Align2::RIGHT_TOP, &dur, FontId::new(9.0, FontFamily::Monospace), col);
    }

    // Bottom buttons: ADD REM SEL MISC
    let btns = [("ADD", 14.0), ("REM", 44.0), ("SEL", 74.0), ("MISC", 104.0)];
    for (label, x) in btns {
        let r = Rect::from_min_size(pos + Vec2::new(x, state.height - 22.0), Vec2::new(24.0, 15.0));
        let clicked = text_btn(ui, &painter, r, label, 7.5, false, BTN_GLYPH);
        match label {
            "ADD" if clicked => action = PlaylistAction::AddFiles,
            "REM" if clicked => action = PlaylistAction::RemoveSelected,
            "MISC" if clicked => action = PlaylistAction::Clear,
            _ => {}
        }
    }

    // Total time (right side)
    let total: u64 = state.items.iter().map(|it| it.duration_secs).sum();
    let total_str = format!("{:02}:{:02}", total / 60, total % 60);
    let t_rect = Rect::from_min_size(pos + Vec2::new(160.0, state.height - 20.0), Vec2::new(48.0, 12.0));
    painter.rect_filled(t_rect, 0.0, LCD_BG);
    painter.text(t_rect.min + Vec2::new(4.0, 0.5), Align2::LEFT_TOP, &total_str, FontId::new(8.0, FontFamily::Monospace), LED_GREEN);

    // Resize grip (13px rows to match the config metric)
    let grip = Rect::from_min_max(
        pos + Vec2::new(13.0, state.height - 7.0),
        pos + Vec2::new(PL_DEFAULT_WIDTH - 21.0, state.height),
    );
    let grip_resp = ui.allocate_rect(grip, Sense::drag());
    grip_resp.clone().on_hover_cursor(egui::CursorIcon::ResizeVertical);
    if grip_resp.dragged() {
        const CHROME: f32 = 58.0;
        const ROW_H: f32 = 13.0;
        let rows = ((state.height - CHROME) / ROW_H).round();
        state.height = CHROME + (rows + grip_resp.drag_delta().y / ROW_H).clamp(2.0, 64.0) * ROW_H;
    }

    action
}
