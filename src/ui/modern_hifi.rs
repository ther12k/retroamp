//! Modern Audiophile Hi-Fi Studio UI/UX for RetroAmp.
//! Sleek brushed aluminum chassis, OLED display, modern vector typography and faders.

use crate::audio::{AudioEngine, PlaybackState};
use crate::skin::{MAIN_WIDTH, MAIN_HEIGHT, EQ_WIDTH, EQ_HEIGHT, PL_DEFAULT_WIDTH};
use crate::ui::eq_window::{EqAction, EqWindowState};
use crate::ui::main_window::{MainAction, MainWindowState, VisMode};
use crate::ui::playlist_window::{PlaylistAction, PlaylistWindowState};
use egui::{Color32, FontFamily, FontId, Pos2, Rect, Sense, Stroke, Ui, Vec2};
use std::time::Duration;

const BG_COLOR: Color32 = Color32::from_rgb(22, 24, 29);
const BORDER_COLOR: Color32 = Color32::from_rgb(45, 50, 60);
const OLED_BG: Color32 = Color32::from_rgb(10, 12, 16);
const ACCENT_CYAN: Color32 = Color32::from_rgb(0, 240, 255);
const ACCENT_ORANGE: Color32 = Color32::from_rgb(255, 140, 0);
const ACCENT_GREEN: Color32 = Color32::from_rgb(0, 255, 128);
const TEXT_MUTED: Color32 = Color32::from_rgb(130, 140, 160);
const BTN_BG: Color32 = Color32::from_rgb(32, 36, 44);
const BTN_HOVER: Color32 = Color32::from_rgb(42, 48, 60);

pub fn render_hifi_main(
    ui: &mut Ui,
    state: &mut MainWindowState,
    audio: &mut AudioEngine,
) -> MainAction {
    let mut action = MainAction::None;
    let window_rect = Rect::from_min_size(state.position, Vec2::new(MAIN_WIDTH, MAIN_HEIGHT));

    // Handle native window dragging
    let titlebar_rect = Rect::from_min_size(state.position, Vec2::new(MAIN_WIDTH - 40.0, 18.0));
    let titlebar_resp = ui.allocate_rect(titlebar_rect, Sense::click_and_drag());
    if titlebar_resp.drag_started() {
        state.os_drag_started = true;
    }

    let painter = ui.painter_at(window_rect);

    // 1. Chassis Background (Brushed dark aluminum)
    painter.rect_filled(window_rect, 4.0, BG_COLOR);
    painter.rect_stroke(window_rect, 4.0, Stroke::new(1.0_f32, BORDER_COLOR), egui::StrokeKind::Inside);

    // 2. Titlebar
    painter.rect_filled(
        Rect::from_min_size(state.position, Vec2::new(MAIN_WIDTH, 18.0)),
        4.0,
        Color32::from_rgb(16, 18, 22),
    );

    // ☰ Main Menu button (left of title)
    let menu_rect = Rect::from_min_size(state.position + Vec2::new(6.0, 2.0), Vec2::new(14.0, 14.0));
    let menu_resp = ui.allocate_rect(menu_rect, Sense::click());
    if menu_resp.hovered() || state.menu_open {
        painter.rect_filled(menu_rect, 3.0, BTN_HOVER);
    }
    painter.text(
        menu_rect.center(),
        egui::Align2::CENTER_CENTER,
        "☰",
        FontId::new(11.0, FontFamily::Proportional),
        Color32::WHITE,
    );
    if menu_resp.clicked() {
        state.menu_open = !state.menu_open;
        state.menu_frames = 0;
    }

    // Title text
    painter.text(
        state.position + Vec2::new(24.0, 4.0),
        egui::Align2::LEFT_TOP,
        "RETROAMP // HI-FI STUDIO",
        FontId::new(9.0, FontFamily::Proportional),
        Color32::from_rgb(170, 180, 200),
    );

    // Theme Switcher button ('🎨')
    let theme_btn_rect = Rect::from_min_size(state.position + Vec2::new(MAIN_WIDTH - 52.0, 3.0), Vec2::new(14.0, 12.0));
    let theme_resp = ui.allocate_rect(theme_btn_rect, Sense::click());
    if theme_resp.hovered() {
        painter.rect_filled(theme_btn_rect, 2.0, BTN_HOVER);
    }
    painter.text(
        theme_btn_rect.center(),
        egui::Align2::CENTER_CENTER,
        "🎨",
        FontId::new(9.0, FontFamily::Proportional),
        Color32::WHITE,
    );
    if theme_resp.clicked() {
        action = MainAction::LoadSkin;
    }

    // Minimize button (orange dot)
    let min_rect = Rect::from_min_size(state.position + Vec2::new(MAIN_WIDTH - 32.0, 4.0), Vec2::new(10.0, 10.0));
    let min_resp = ui.allocate_rect(min_rect, Sense::click());
    painter.circle_filled(min_rect.center(), 4.0, if min_resp.hovered() { Color32::from_rgb(255, 180, 40) } else { ACCENT_ORANGE });
    if min_resp.clicked() {
        action = MainAction::Minimize;
    }

    // Close button (red dot)
    let close_rect = Rect::from_min_size(state.position + Vec2::new(MAIN_WIDTH - 18.0, 4.0), Vec2::new(10.0, 10.0));
    let close_resp = ui.allocate_rect(close_rect, Sense::click());
    painter.circle_filled(close_rect.center(), 4.0, if close_resp.hovered() { Color32::from_rgb(255, 80, 80) } else { Color32::from_rgb(230, 40, 40) });
    if close_resp.clicked() {
        action = MainAction::Close;
    }

    // 3. OLED Glass Display (x: 8, y: 22, w: 259, h: 44)
    let oled_rect = Rect::from_min_size(state.position + Vec2::new(8.0, 22.0), Vec2::new(259.0, 44.0));
    painter.rect_filled(oled_rect, 3.0, OLED_BG);
    painter.rect_stroke(oled_rect, 3.0, Stroke::new(1.0_f32, Color32::from_rgb(30, 36, 46)), egui::StrokeKind::Inside);

    // Track Title (Marquee)
    let title_text = if let Some(ref track) = audio.current_track {
        format!("{}  •  {}", track.title, track.artist)
    } else {
        "RetroAmp Studio Edition • Hi-Fi Audio Engine".to_string()
    };
    painter.text(
        oled_rect.min + Vec2::new(8.0, 4.0),
        egui::Align2::LEFT_TOP,
        &title_text,
        FontId::new(10.0, FontFamily::Proportional),
        Color32::WHITE,
    );

    // Time Display
    let display_secs = if state.time_remaining_mode {
        if let Some(ref track) = audio.current_track {
            if track.duration > audio.position {
                (track.duration - audio.position).as_secs()
            } else {
                0
            }
        } else {
            0
        }
    } else {
        audio.position.as_secs()
    };
    let time_str = format!("{:02}:{:02}", display_secs / 60, display_secs % 60);
    let time_rect = Rect::from_min_size(oled_rect.min + Vec2::new(8.0, 18.0), Vec2::new(45.0, 14.0));
    let time_resp = ui.allocate_rect(time_rect, Sense::click());
    if time_resp.clicked() {
        state.time_remaining_mode = !state.time_remaining_mode;
    }
    painter.text(
        time_rect.min,
        egui::Align2::LEFT_TOP,
        &time_str,
        FontId::new(13.0, FontFamily::Monospace),
        ACCENT_CYAN,
    );

    // Audio status badge pills (e.g. [FLAC 44.1k] [320k] [ST])
    let (srate, kbps) = if let Some(ref t) = audio.current_track {
        (t.sample_rate / 1000, t.bitrate_kbps)
    } else {
        (44, 320)
    };
    let meta_str = format!("{}kHz • {}kbps • ST", srate, kbps);
    painter.text(
        oled_rect.min + Vec2::new(60.0, 20.0),
        egui::Align2::LEFT_TOP,
        &meta_str,
        FontId::new(9.0, FontFamily::Monospace),
        TEXT_MUTED,
    );

    // Visualizer Area inside OLED (right side: x: 175..250, y: 16..38)
    let vis_rect = Rect::from_min_size(oled_rect.min + Vec2::new(172.0, 16.0), Vec2::new(80.0, 24.0));
    let vis_resp = ui.allocate_rect(vis_rect, Sense::click());
    if vis_resp.clicked() {
        state.vis_mode = match state.vis_mode {
            VisMode::Spectrum => VisMode::Oscilloscope,
            VisMode::Oscilloscope => VisMode::Off,
            VisMode::Off => VisMode::Spectrum,
        };
    }

    if state.vis_mode == VisMode::Spectrum {
        let (_, spectrum, peaks) = audio.get_vis_data();
        let bar_width = 3.0;
        let bar_gap = 1.0;
        let num_bars = 18;

        for i in 0..num_bars {
            let bx = vis_rect.min.x + (i as f32) * (bar_width + bar_gap);
            let val = spectrum[i].clamp(0.0, 1.0);
            let bar_h = (val * 20.0).round().max(1.0);
            let by = vis_rect.max.y - bar_h;

            let bar_rect = Rect::from_min_size(Pos2::new(bx, by), Vec2::new(bar_width, bar_h));
            // Gradient: cyan to violet
            let ratio = i as f32 / num_bars as f32;
            let col = Color32::from_rgb(
                (0.0 + ratio * 180.0) as u8,
                (240.0 - ratio * 100.0) as u8,
                255,
            );
            painter.rect_filled(bar_rect, 1.0, col);

            // Peak cap
            let peak_h = (peaks[i] * 20.0).round();
            if peak_h > 1.0 {
                let peak_y = vis_rect.max.y - peak_h;
                painter.rect_filled(
                    Rect::from_min_size(Pos2::new(bx, peak_y), Vec2::new(bar_width, 1.0)),
                    0.0,
                    Color32::WHITE,
                );
            }
        }
    } else if state.vis_mode == VisMode::Oscilloscope {
        let (waveform, _, _) = audio.get_vis_data();
        let mid_y = vis_rect.min.y + 12.0;
        let w_len = waveform.len().min(75);
        for i in 0..w_len - 1 {
            let x1 = vis_rect.min.x + (i as f32 / w_len as f32) * 78.0;
            let x2 = vis_rect.min.x + ((i + 1) as f32 / w_len as f32) * 78.0;
            let y1 = (mid_y - waveform[i] * 10.0).clamp(vis_rect.min.y, vis_rect.max.y);
            let y2 = (mid_y - waveform[i + 1] * 10.0).clamp(vis_rect.min.y, vis_rect.max.y);
            painter.line_segment([Pos2::new(x1, y1), Pos2::new(x2, y2)], Stroke::new(1.0_f32, ACCENT_CYAN));
        }
    }

    // 4. Seek Scrubber Track (x: 8, y: 70, w: 259, h: 6)
    let seek_track_rect = Rect::from_min_size(state.position + Vec2::new(8.0, 70.0), Vec2::new(259.0, 6.0));
    painter.rect_filled(seek_track_rect, 3.0, Color32::from_rgb(14, 16, 20));

    let progress_ratio = if let Some(ref track) = audio.current_track {
        if track.duration > Duration::ZERO {
            (audio.position.as_secs_f32() / track.duration.as_secs_f32()).clamp(0.0, 1.0)
        } else {
            0.0
        }
    } else {
        0.0
    };

    let filled_rect = Rect::from_min_size(seek_track_rect.min, Vec2::new(seek_track_rect.width() * progress_ratio, 6.0));
    painter.rect_filled(filled_rect, 3.0, ACCENT_CYAN);

    let seek_resp = ui.allocate_rect(seek_track_rect, Sense::click_and_drag());
    if seek_resp.clicked() || seek_resp.dragged() {
        if let Some(mouse) = ui.input(|i| i.pointer.interact_pos()) {
            let r = ((mouse.x - seek_track_rect.min.x) / seek_track_rect.width()).clamp(0.0, 1.0);
            state.seek_target_ratio = Some(r);
        }
    }

    // Knob
    let thumb_center = Pos2::new(seek_track_rect.min.x + progress_ratio * seek_track_rect.width(), seek_track_rect.center().y);
    painter.circle_filled(thumb_center, 4.0, Color32::WHITE);

    // 5. Volume Slider & Balance Row (x: 8, y: 80)
    let vol_track = Rect::from_min_size(state.position + Vec2::new(8.0, 81.0), Vec2::new(54.0, 5.0));
    painter.rect_filled(vol_track, 2.5, Color32::from_rgb(14, 16, 20));
    let vol_fill = Rect::from_min_size(vol_track.min, Vec2::new(vol_track.width() * audio.volume, 5.0));
    painter.rect_filled(vol_fill, 2.5, ACCENT_GREEN);
    let vol_resp = ui.allocate_rect(vol_track, Sense::click_and_drag());
    if vol_resp.dragged() || vol_resp.clicked() {
        if let Some(mouse) = ui.input(|i| i.pointer.interact_pos()) {
            let r = ((mouse.x - vol_track.min.x) / vol_track.width()).clamp(0.0, 1.0);
            audio.set_volume(r);
        }
    }

    // EQ / PL Window Toggles
    let eq_btn = Rect::from_min_size(state.position + Vec2::new(68.0, 78.0), Vec2::new(20.0, 11.0));
    let eq_resp = ui.allocate_rect(eq_btn, Sense::click());
    if eq_resp.clicked() {
        state.show_eq = !state.show_eq;
    }
    let eq_col = if state.show_eq { ACCENT_CYAN } else { BTN_BG };
    painter.rect_filled(eq_btn, 2.0, eq_col);
    painter.text(eq_btn.center(), egui::Align2::CENTER_CENTER, "EQ", FontId::new(8.0, FontFamily::Proportional), if state.show_eq { Color32::BLACK } else { TEXT_MUTED });

    let pl_btn = Rect::from_min_size(state.position + Vec2::new(92.0, 78.0), Vec2::new(20.0, 11.0));
    let pl_resp = ui.allocate_rect(pl_btn, Sense::click());
    if pl_resp.clicked() {
        state.show_playlist = !state.show_playlist;
    }
    let pl_col = if state.show_playlist { ACCENT_CYAN } else { BTN_BG };
    painter.rect_filled(pl_btn, 2.0, pl_col);
    painter.text(pl_btn.center(), egui::Align2::CENTER_CENTER, "PL", FontId::new(8.0, FontFamily::Proportional), if state.show_playlist { Color32::BLACK } else { TEXT_MUTED });

    // Shuffle & Repeat Toggles
    let shuf_btn = Rect::from_min_size(state.position + Vec2::new(118.0, 78.0), Vec2::new(32.0, 11.0));
    let shuf_resp = ui.allocate_rect(shuf_btn, Sense::click());
    if shuf_resp.clicked() { state.shuffle = !state.shuffle; }
    painter.rect_filled(shuf_btn, 2.0, if state.shuffle { ACCENT_ORANGE } else { BTN_BG });
    painter.text(shuf_btn.center(), egui::Align2::CENTER_CENTER, "SHUF", FontId::new(8.0, FontFamily::Proportional), if state.shuffle { Color32::BLACK } else { TEXT_MUTED });

    let rep_btn = Rect::from_min_size(state.position + Vec2::new(154.0, 78.0), Vec2::new(28.0, 11.0));
    let rep_resp = ui.allocate_rect(rep_btn, Sense::click());
    if rep_resp.clicked() { state.repeat = !state.repeat; }
    painter.rect_filled(rep_btn, 2.0, if state.repeat { ACCENT_ORANGE } else { BTN_BG });
    painter.text(rep_btn.center(), egui::Align2::CENTER_CENTER, "REP", FontId::new(8.0, FontFamily::Proportional), if state.repeat { Color32::BLACK } else { TEXT_MUTED });

    // 6. Tactile Transport Buttons (y: 93, each 26x18)
    let btn_symbols = [
        ("⏮", MainAction::Prev),
        ("▶", MainAction::Play),
        ("⏸", MainAction::Pause),
        ("⏹", MainAction::Stop),
        ("⏭", MainAction::Next),
        ("⏏", MainAction::Eject),
    ];

    for (idx, (sym, act)) in btn_symbols.iter().enumerate() {
        let bx = state.position.x + 8.0 + (idx as f32) * 32.0;
        let by = state.position.y + 93.0;
        let b_rect = Rect::from_min_size(Pos2::new(bx, by), Vec2::new(28.0, 18.0));
        let b_resp = ui.allocate_rect(b_rect, Sense::click());

        let is_active_play = *sym == "▶" && audio.state == PlaybackState::Playing;
        let is_active_pause = *sym == "⏸" && audio.state == PlaybackState::Paused;

        let bg = if b_resp.is_pointer_button_down_on() {
            Color32::from_rgb(55, 62, 75)
        } else if is_active_play {
            Color32::from_rgb(0, 180, 100)
        } else if is_active_pause {
            Color32::from_rgb(220, 120, 0)
        } else if b_resp.hovered() {
            BTN_HOVER
        } else {
            BTN_BG
        };

        painter.rect_filled(b_rect, 3.0, bg);
        painter.rect_stroke(b_rect, 3.0, Stroke::new(1.0_f32, Color32::from_rgb(45, 52, 65)), egui::StrokeKind::Inside);
        painter.text(
            b_rect.center(),
            egui::Align2::CENTER_CENTER,
            sym,
            FontId::new(12.0, FontFamily::Proportional),
            Color32::WHITE,
        );

        if b_resp.clicked() {
            action = *act;
        }
    }

    action
}

pub fn render_hifi_eq(
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

    // Chassis Background
    painter.rect_filled(window_rect, 4.0, BG_COLOR);
    painter.rect_stroke(window_rect, 4.0, Stroke::new(1.0_f32, BORDER_COLOR), egui::StrokeKind::Inside);

    // Titlebar
    painter.rect_filled(Rect::from_min_size(state.position, Vec2::new(EQ_WIDTH, 16.0)), 4.0, Color32::from_rgb(16, 18, 22));
    painter.text(
        state.position + Vec2::new(10.0, 3.0),
        egui::Align2::LEFT_TOP,
        "STUDIO EQUALIZER // 10-BAND DSP",
        FontId::new(9.0, FontFamily::Proportional),
        Color32::from_rgb(170, 180, 200),
    );

    // Close button
    let close_rect = Rect::from_min_size(state.position + Vec2::new(EQ_WIDTH - 16.0, 4.0), Vec2::new(10.0, 10.0));
    let close_resp = ui.allocate_rect(close_rect, Sense::click());
    painter.circle_filled(close_rect.center(), 4.0, if close_resp.hovered() { Color32::from_rgb(255, 80, 80) } else { Color32::from_rgb(230, 40, 40) });
    if close_resp.clicked() { action = EqAction::Close; }

    // EQ ON Toggle
    let on_btn = Rect::from_min_size(state.position + Vec2::new(10.0, 22.0), Vec2::new(28.0, 14.0));
    let on_resp = ui.allocate_rect(on_btn, Sense::click());
    if on_resp.clicked() { audio.eq.enabled = !audio.eq.enabled; }
    painter.rect_filled(on_btn, 2.0, if audio.eq.enabled { ACCENT_GREEN } else { BTN_BG });
    painter.text(on_btn.center(), egui::Align2::CENTER_CENTER, "ON", FontId::new(9.0, FontFamily::Proportional), if audio.eq.enabled { Color32::BLACK } else { TEXT_MUTED });

    // PRESETS button — opens the classic Winamp preset list
    let presets_btn = Rect::from_min_size(state.position + Vec2::new(44.0, 22.0), Vec2::new(64.0, 14.0));
    let presets_resp = ui.allocate_rect(presets_btn, Sense::click());
    painter.rect_filled(presets_btn, 2.0, if presets_resp.hovered() || state.show_presets { BTN_HOVER } else { BTN_BG });
    painter.text(presets_btn.center(), egui::Align2::CENTER_CENTER, "PRESETS", FontId::new(8.5, FontFamily::Proportional), if state.show_presets { ACCENT_CYAN } else { Color32::WHITE });
    if presets_resp.clicked() {
        state.show_presets = !state.show_presets;
    }

    // Studio Curve Graph (x: 48, y: 20, w: 218, h: 22)
    let graph_rect = Rect::from_min_size(state.position + Vec2::new(48.0, 20.0), Vec2::new(218.0, 22.0));
    painter.rect_filled(graph_rect, 3.0, OLED_BG);
    painter.rect_stroke(graph_rect, 3.0, Stroke::new(1.0_f32, Color32::from_rgb(30, 36, 46)), egui::StrokeKind::Inside);

    if audio.eq.enabled {
        let center_y = graph_rect.center().y;
        let mut pts = Vec::with_capacity(100);
        for step in 0..100 {
            let ratio = step as f32 / 99.0;
            let b_idx = (ratio * 9.0).floor() as usize;
            let t = ratio * 9.0 - b_idx as f32;
            let b_next = (b_idx + 1).min(9);
            let db = audio.eq.bands[b_idx] * (1.0 - t) + audio.eq.bands[b_next] * t + audio.eq.preamp * 0.4;
            let y = (center_y - (db / 12.0) * 9.0).clamp(graph_rect.min.y + 1.0, graph_rect.max.y - 1.0);
            pts.push(Pos2::new(graph_rect.min.x + ratio * graph_rect.width(), y));
        }
        for w in pts.windows(2) {
            painter.line_segment([w[0], w[1]], Stroke::new(1.5_f32, ACCENT_GREEN));
        }
    }

    // 10 Frequency Band Sliders (y: 48..105, height: 52)
    let band_xs = [48.0, 70.0, 92.0, 114.0, 136.0, 158.0, 180.0, 202.0, 224.0, 246.0];
    let freq_labels = ["60", "170", "310", "600", "1k", "3k", "6k", "12k", "14k", "16k"];

    // Preamp slider (x: 16)
    let pre_rect = Rect::from_min_size(state.position + Vec2::new(16.0, 48.0), Vec2::new(12.0, 52.0));
    painter.rect_filled(pre_rect, 2.0, Color32::from_rgb(14, 16, 20));
    let pre_resp = ui.allocate_rect(pre_rect, Sense::click_and_drag());
    if pre_resp.dragged() || pre_resp.clicked() {
        if let Some(mouse) = ui.input(|i| i.pointer.interact_pos()) {
            let r = 1.0 - ((mouse.y - pre_rect.min.y) / pre_rect.height()).clamp(0.0, 1.0);
            audio.eq.preamp = r * 24.0 - 12.0;
        }
    }
    let pre_thumb_y = pre_rect.max.y - ((audio.eq.preamp + 12.0) / 24.0) * pre_rect.height();
    painter.rect_filled(Rect::from_center_size(Pos2::new(pre_rect.center().x, pre_thumb_y), Vec2::new(10.0, 6.0)), 2.0, ACCENT_ORANGE);
    painter.text(state.position + Vec2::new(22.0, 103.0), egui::Align2::CENTER_TOP, "PRE", FontId::new(7.0, FontFamily::Proportional), TEXT_MUTED);

    for (i, (&bx, &lbl)) in band_xs.iter().zip(freq_labels.iter()).enumerate() {
        let b_rect = Rect::from_min_size(state.position + Vec2::new(bx, 48.0), Vec2::new(12.0, 52.0));
        painter.rect_filled(b_rect, 2.0, Color32::from_rgb(14, 16, 20));

        let b_resp = ui.allocate_rect(b_rect, Sense::click_and_drag());
        if b_resp.dragged() || b_resp.clicked() {
            if let Some(mouse) = ui.input(|i| i.pointer.interact_pos()) {
                let r = 1.0 - ((mouse.y - b_rect.min.y) / b_rect.height()).clamp(0.0, 1.0);
                audio.eq.bands[i] = r * 24.0 - 12.0;
            }
        }

        let thumb_y = b_rect.max.y - ((audio.eq.bands[i] + 12.0) / 24.0) * b_rect.height();
        painter.rect_filled(Rect::from_center_size(Pos2::new(b_rect.center().x, thumb_y), Vec2::new(10.0, 6.0)), 2.0, ACCENT_CYAN);
        painter.text(state.position + Vec2::new(bx + 6.0, 103.0), egui::Align2::CENTER_TOP, lbl, FontId::new(7.0, FontFamily::Proportional), TEXT_MUTED);
    }

    action
}

pub fn render_hifi_playlist(
    ui: &mut Ui,
    state: &mut PlaylistWindowState,
    current_playing_index: Option<usize>,
    _audio: &AudioEngine,
) -> PlaylistAction {
    let mut action = PlaylistAction::None;
    let window_rect = Rect::from_min_size(state.position, Vec2::new(PL_DEFAULT_WIDTH, state.height));

    // Handle dragging
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

    // Chassis Background
    painter.rect_filled(window_rect, 4.0, BG_COLOR);
    painter.rect_stroke(window_rect, 4.0, Stroke::new(1.0_f32, BORDER_COLOR), egui::StrokeKind::Inside);

    // Titlebar
    painter.rect_filled(Rect::from_min_size(state.position, Vec2::new(PL_DEFAULT_WIDTH, 16.0)), 4.0, Color32::from_rgb(16, 18, 22));
    let count_str = format!("PLAYLIST QUEUE // {} TRACKS", state.items.len());
    painter.text(
        state.position + Vec2::new(10.0, 3.0),
        egui::Align2::LEFT_TOP,
        &count_str,
        FontId::new(9.0, FontFamily::Proportional),
        Color32::from_rgb(170, 180, 200),
    );

    // Close button
    let close_rect = Rect::from_min_size(state.position + Vec2::new(PL_DEFAULT_WIDTH - 16.0, 4.0), Vec2::new(10.0, 10.0));
    let close_resp = ui.allocate_rect(close_rect, Sense::click());
    painter.circle_filled(close_rect.center(), 4.0, if close_resp.hovered() { Color32::from_rgb(255, 80, 80) } else { Color32::from_rgb(230, 40, 40) });
    if close_resp.clicked() { action = PlaylistAction::Close; }

    // Playlist Queue List Area
    let list_area = Rect::from_min_max(
        state.position + Vec2::new(8.0, 20.0),
        state.position + Vec2::new(PL_DEFAULT_WIDTH - 8.0, state.height - 30.0),
    );
    painter.rect_filled(list_area, 3.0, OLED_BG);
    painter.rect_stroke(list_area, 3.0, Stroke::new(1.0_f32, Color32::from_rgb(30, 36, 46)), egui::StrokeKind::Inside);

    let line_height = 18.0;
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
        let item_rect = Rect::from_min_size(Pos2::new(list_area.min.x + 2.0, y), Vec2::new(list_area.width() - 4.0, line_height - 2.0));

        let is_selected = state.selected_index == Some(entry_idx);
        let is_playing = current_playing_index == Some(entry_idx);

        if is_selected {
            painter.rect_filled(item_rect, 2.0, Color32::from_rgb(30, 42, 60));
        } else if is_playing {
            painter.rect_filled(item_rect, 2.0, Color32::from_rgb(18, 32, 28));
        }

        let item_resp = ui.allocate_rect(item_rect, Sense::click());
        if item_resp.clicked() { state.selected_index = Some(entry_idx); }
        if item_resp.double_clicked() { action = PlaylistAction::PlayIndex(entry_idx); }

        // Track index pill
        let num_str = format!("{:02}", entry_idx + 1);
        painter.text(
            item_rect.min + Vec2::new(4.0, 2.0),
            egui::Align2::LEFT_TOP,
            &num_str,
            FontId::new(10.0, FontFamily::Monospace),
            if is_playing { ACCENT_GREEN } else { TEXT_MUTED },
        );

        // Title text
        painter.text(
            item_rect.min + Vec2::new(26.0, 2.0),
            egui::Align2::LEFT_TOP,
            &entry.title,
            FontId::new(10.0, FontFamily::Proportional),
            if is_playing { ACCENT_GREEN } else { Color32::WHITE },
        );

        // Duration
        let dur_str = format!("{:02}:{:02}", entry.duration_secs / 60, entry.duration_secs % 60);
        painter.text(
            item_rect.max - Vec2::new(6.0, 14.0),
            egui::Align2::RIGHT_TOP,
            &dur_str,
            FontId::new(10.0, FontFamily::Monospace),
            TEXT_MUTED,
        );
    }

    // Bottom Action Buttons: [+ ADD] [- REM] [CLEAR]
    let add_rect = Rect::from_min_size(state.position + Vec2::new(8.0, state.height - 24.0), Vec2::new(50.0, 16.0));
    let add_resp = ui.allocate_rect(add_rect, Sense::click());
    painter.rect_filled(add_rect, 2.0, if add_resp.hovered() { BTN_HOVER } else { BTN_BG });
    painter.text(add_rect.center(), egui::Align2::CENTER_CENTER, "+ ADD", FontId::new(9.0, FontFamily::Proportional), Color32::WHITE);
    if add_resp.clicked() { action = PlaylistAction::AddFiles; }

    let rem_rect = Rect::from_min_size(state.position + Vec2::new(62.0, state.height - 24.0), Vec2::new(50.0, 16.0));
    let rem_resp = ui.allocate_rect(rem_rect, Sense::click());
    painter.rect_filled(rem_rect, 2.0, if rem_resp.hovered() { BTN_HOVER } else { BTN_BG });
    painter.text(rem_rect.center(), egui::Align2::CENTER_CENTER, "- REM", FontId::new(9.0, FontFamily::Proportional), Color32::WHITE);
    if rem_resp.clicked() { action = PlaylistAction::RemoveSelected; }

    let clr_rect = Rect::from_min_size(state.position + Vec2::new(116.0, state.height - 24.0), Vec2::new(50.0, 16.0));
    let clr_resp = ui.allocate_rect(clr_rect, Sense::click());
    painter.rect_filled(clr_rect, 2.0, if clr_resp.hovered() { BTN_HOVER } else { BTN_BG });
    painter.text(clr_rect.center(), egui::Align2::CENTER_CENTER, "CLEAR", FontId::new(9.0, FontFamily::Proportional), Color32::WHITE);
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
        const ROW_H: f32 = 18.0;
        let rows = ((state.height - CHROME) / ROW_H).round();
        let new_rows = (rows + grip_resp.drag_delta().y / ROW_H).clamp(2.0, 64.0);
        state.height = CHROME + new_rows * ROW_H;
    }

    action
}
