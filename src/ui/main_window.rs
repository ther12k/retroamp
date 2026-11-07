//! Main Winamp Window UI implementation.

use crate::audio::{AudioEngine, PlaybackState, SPECTRUM_BARS, WAVEFORM_SAMPLES};
use crate::skin::{Skin, Sprites, MAIN_WIDTH, MAIN_HEIGHT};
use crate::ui::textures::SkinTextures;
use egui::{Color32, Pos2, Rect, Sense, Ui, Vec2};
use std::time::Duration;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum VisMode {
    Spectrum,
    Oscilloscope,
    Off,
}

pub struct MainWindowState {
    pub position: Pos2,
    pub time_remaining_mode: bool,
    pub vis_mode: VisMode,
    pub marquee_offset: usize,
    pub last_marquee_tick: std::time::Instant,
    pub shuffle: bool,
    pub repeat: bool,
    pub show_eq: bool,
    pub show_playlist: bool,
    pub is_dragging: bool,
    pub drag_offset: Vec2,
    /// Set when the skinned titlebar drag starts; consumed by the app to hand
    /// the move off to the window manager (native, lag-free).
    pub os_drag_started: bool,
    pub volume_dragging: bool,
    pub balance_dragging: bool,
    pub posbar_dragging: bool,
    pub seek_target_ratio: Option<f32>,
    /// Classic Winamp-style main menu (opened from the titlebar logo / ☰).
    pub menu_open: bool,
    /// Frames since the menu opened (debounces outside-click auto-close).
    pub menu_frames: u32,
}

impl Default for MainWindowState {
    fn default() -> Self {
        Self {
            position: Pos2::new(10.0, 10.0),
            time_remaining_mode: false,
            vis_mode: VisMode::Spectrum,
            marquee_offset: 0,
            last_marquee_tick: std::time::Instant::now(),
            shuffle: false,
            repeat: false,
            show_eq: true,
            show_playlist: true,
            is_dragging: false,
            drag_offset: Vec2::ZERO,
            os_drag_started: false,
            volume_dragging: false,
            balance_dragging: false,
            posbar_dragging: false,
            seek_target_ratio: None,
            menu_open: false,
            menu_frames: 0,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum MainAction {
    None,
    Prev,
    Play,
    Pause,
    Stop,
    Next,
    Eject,
    ToggleEq,
    TogglePlaylist,
    LoadSkin,
    Minimize,
    Close,
}

pub fn render_main_window(
    ui: &mut Ui,
    state: &mut MainWindowState,
    audio: &mut AudioEngine,
    skin: &Skin,
    textures: &SkinTextures,
) -> MainAction {
    let mut action = MainAction::None;
    let window_rect = Rect::from_min_size(state.position, Vec2::new(MAIN_WIDTH, MAIN_HEIGHT));

    // Dragging the skinned titlebar starts a native WM window move
    // (borderless window, no system chrome). Docked panels travel along
    // because they stay glued to the main panel.
    let titlebar_rect = Rect::from_min_size(state.position, Vec2::new(MAIN_WIDTH - 30.0, 14.0));
    let titlebar_resp = ui.allocate_rect(titlebar_rect, Sense::click_and_drag());
    if titlebar_resp.drag_started() {
        state.os_drag_started = true;
    }

    let painter = ui.painter_at(window_rect);

    // 1. Draw Main Window Background (MAIN.BMP)
    painter.image(
        textures.main.id(),
        window_rect,
        Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
        Color32::WHITE,
    );

    // 2. Draw Active Titlebar Strip (TITLEBAR.BMP, 275x14)
    let tb_rect = Rect::from_min_size(state.position, Vec2::new(MAIN_WIDTH, 14.0));
    painter.image(
        textures.titlebar.id(),
        tb_rect,
        Sprites::TITLEBAR_ACTIVE.to_uv(344.0, 87.0),
        Color32::WHITE,
    );

    // Classic Winamp menu button: click the logo in the top-left titlebar
    let menu_rect = Rect::from_min_size(state.position + Vec2::new(11.0, 3.0), Vec2::new(26.0, 9.0));
    let menu_resp = ui.allocate_rect(menu_rect, Sense::click());
    if menu_resp.clicked() {
        state.menu_open = !state.menu_open;
        state.menu_frames = 0;
    }

    // Minimize button (x: 244, y: 3, 9x9)
    let min_rect = Rect::from_min_size(state.position + Vec2::new(244.0, 3.0), Vec2::new(9.0, 9.0));
    let min_resp = ui.allocate_rect(min_rect, Sense::click());
    let min_sprite = if min_resp.is_pointer_button_down_on() { Sprites::MINIMIZE_PRESSED } else { Sprites::MINIMIZE_NORMAL };
    painter.image(textures.titlebar.id(), min_rect, min_sprite.to_uv(344.0, 87.0), Color32::WHITE);
    if min_resp.clicked() {
        action = MainAction::Minimize;
    }

    // Close button (x: 264, y: 3, 9x9)
    let close_rect = Rect::from_min_size(state.position + Vec2::new(264.0, 3.0), Vec2::new(9.0, 9.0));
    let close_resp = ui.allocate_rect(close_rect, Sense::click());
    let close_sprite = if close_resp.is_pointer_button_down_on() { Sprites::CLOSE_PRESSED } else { Sprites::CLOSE_NORMAL };
    painter.image(
        textures.titlebar.id(),
        close_rect,
        close_sprite.to_uv(344.0, 87.0),
        Color32::WHITE,
    );
    if close_resp.clicked() {
        action = MainAction::Close;
    }

    // 3. Playback Status Indicator (PLAYPAUS.BMP, x: 24, y: 28, 9x9)
    let playpaus_rect = Rect::from_min_size(state.position + Vec2::new(24.0, 28.0), Vec2::new(9.0, 9.0));
    let status_sprite = match audio.state {
        PlaybackState::Playing => Sprites::INDICATOR_PLAY,
        PlaybackState::Paused => Sprites::INDICATOR_PAUSE,
        PlaybackState::Stopped => Sprites::INDICATOR_STOP,
    };
    painter.image(
        textures.playpaus.id(),
        playpaus_rect,
        status_sprite.to_uv(42.0, 9.0),
        Color32::WHITE,
    );

    // 4. Time Display (NUMBERS.BMP, digits x: 36..100, y: 26..39)
    let time_area = Rect::from_min_size(state.position + Vec2::new(34.0, 24.0), Vec2::new(66.0, 15.0));
    let time_resp = ui.allocate_rect(time_area, Sense::click());
    if time_resp.clicked() {
        state.time_remaining_mode = !state.time_remaining_mode;
    }

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

    let mins = (display_secs / 60).min(99);
    let secs = display_secs % 60;

    let min_tens = (mins / 10) as usize;
    let min_ones = (mins % 10) as usize;
    let sec_tens = (secs / 10) as usize;
    let sec_ones = (secs % 10) as usize;

    // Minus sign if remaining mode
    if state.time_remaining_mode {
        let minus_rect = Rect::from_min_size(state.position + Vec2::new(36.0, 26.0), Vec2::new(9.0, 13.0));
        painter.image(
            textures.numbers.id(),
            minus_rect,
            Sprites::NUMBER_MINUS.to_uv(99.0, 13.0),
            Color32::WHITE,
        );
    }

    // Digits
    let digit_positions = [
        (48.0, min_tens),
        (60.0, min_ones),
        (78.0, sec_tens),
        (90.0, sec_ones),
    ];
    for (x_offset, digit) in digit_positions {
        let digit_rect = Rect::from_min_size(state.position + Vec2::new(x_offset, 26.0), Vec2::new(9.0, 13.0));
        let sprite = Sprites::number_rect(digit);
        painter.image(
            textures.numbers.id(),
            digit_rect,
            sprite.to_uv(99.0, 13.0),
            Color32::WHITE,
        );
    }

    // 5. Visualizer Area (x: 24, y: 43, 76x16)
    let vis_rect = Rect::from_min_size(state.position + Vec2::new(24.0, 43.0), Vec2::new(76.0, 16.0));
    let vis_resp = ui.allocate_rect(vis_rect, Sense::click());
    if vis_resp.clicked() {
        state.vis_mode = match state.vis_mode {
            VisMode::Spectrum => VisMode::Oscilloscope,
            VisMode::Oscilloscope => VisMode::Off,
            VisMode::Off => VisMode::Spectrum,
        };
    }

    // Render background of visualizer (viscolor[0])
    painter.rect_filled(vis_rect, 0.0, skin.vis_colors[0]);

    if state.vis_mode == VisMode::Spectrum {
        let (_, spectrum, peaks) = audio.get_vis_data();
        let bar_width = 3.0;
        let bar_gap = 1.0;

        for i in 0..SPECTRUM_BARS {
            let x = vis_rect.min.x + (i as f32) * (bar_width + bar_gap);
            if x + bar_width > vis_rect.max.x {
                break;
            }

            let height_ratio = spectrum[i].clamp(0.0, 1.0);
            let bar_height = (height_ratio * 15.0).round();

            // Draw vertical bar colored by frequency band palette
            for h in 0..(bar_height as usize) {
                let y = vis_rect.max.y - 1.0 - (h as f32);
                let color_idx = (2 + (15 - h.min(15))).min(17);
                painter.rect_filled(
                    Rect::from_min_size(Pos2::new(x, y), Vec2::new(bar_width, 1.0)),
                    0.0,
                    skin.vis_colors[color_idx],
                );
            }

            // Draw peak line (viscolor[23])
            let peak_h = (peaks[i] * 15.0).round();
            if peak_h > 0.0 {
                let peak_y = (vis_rect.max.y - 1.0 - peak_h).max(vis_rect.min.y);
                painter.rect_filled(
                    Rect::from_min_size(Pos2::new(x, peak_y), Vec2::new(bar_width, 1.0)),
                    0.0,
                    skin.vis_colors[23],
                );
            }
        }
    } else if state.vis_mode == VisMode::Oscilloscope {
        let (waveform, _, _) = audio.get_vis_data();
        let center_y = vis_rect.min.y + 8.0;

        for i in 0..WAVEFORM_SAMPLES.min(75) {
            let x = vis_rect.min.x + (i as f32);
            let y = (center_y - waveform[i] * 7.0).clamp(vis_rect.min.y, vis_rect.max.y - 1.0);
            painter.rect_filled(
                Rect::from_min_size(Pos2::new(x, y), Vec2::new(1.0, 1.0)),
                0.0,
                skin.vis_colors[18],
            );
        }
    }

    // 6. Track Title / Marquee Display (TEXT.BMP, x: 112, y: 27, 153x6)
    let marquee_rect = Rect::from_min_size(state.position + Vec2::new(112.0, 27.0), Vec2::new(153.0, 6.0));
    painter.rect_filled(marquee_rect, 0.0, Color32::BLACK);

    let title_text = if let Some(ref track) = audio.current_track {
        format!("{}. {} ({} kbps)   ***   WINAMP LINUX   ***   ", 1, track.title, track.bitrate_kbps)
    } else {
        "WINAMP (R) 5.92 LINUX RUST PORT - IT REALLY WHIPS THE LLAMA'S ASS!   ***   ".to_string()
    };

    if state.last_marquee_tick.elapsed() >= Duration::from_millis(140) {
        state.marquee_offset = (state.marquee_offset + 1) % title_text.len().max(1);
        state.last_marquee_tick = std::time::Instant::now();
    }

    // Render characters from marquee using TEXT.BMP bitmap font
    let visible_chars = 30;
    let chars: Vec<char> = title_text.chars().collect();
    for i in 0..visible_chars {
        let char_idx = (state.marquee_offset + i) % chars.len();
        let ch = chars[char_idx];
        let ch_rect = Rect::from_min_size(
            state.position + Vec2::new(112.0 + (i as f32) * 5.0, 27.0),
            Vec2::new(5.0, 6.0),
        );
        let sprite = Sprites::char_rect(ch);
        painter.image(
            textures.text.id(),
            ch_rect,
            sprite.to_uv(155.0, 74.0),
            Color32::WHITE,
        );
    }

    // 7. Bitrate and Sample Rate (x: 111, y: 43)
    let (bitrate, srate) = if let Some(ref track) = audio.current_track {
        (track.bitrate_kbps, track.sample_rate / 1000)
    } else {
        (128, 44)
    };
    render_text_str(
        &painter,
        textures.text.id(),
        state.position + Vec2::new(111.0, 43.0),
        &format!("{:3}", bitrate),
    );
    render_text_str(
        &painter,
        textures.text.id(),
        state.position + Vec2::new(156.0, 43.0),
        &format!("{:2}", srate),
    );

    // Mono / Stereo indicators (MONOSTER.BMP, x: 212, y: 41, 29x12)
    let stereo_rect = Rect::from_min_size(state.position + Vec2::new(239.0, 41.0), Vec2::new(29.0, 12.0));
    let is_stereo = audio.current_track.as_ref().map(|t| t.channels >= 2).unwrap_or(true);
    let stereo_sprite = if is_stereo { Sprites::STEREO_ACTIVE } else { Sprites::STEREO_INACTIVE };
    painter.image(
        textures.monoster.id(),
        stereo_rect,
        stereo_sprite.to_uv(58.0, 24.0),
        Color32::WHITE,
    );

    // 8. Volume Slider (VOLUME.BMP, x: 107, y: 57, 68x13)
    let vol_frame_idx = ((audio.volume * 27.0).round() as usize).min(27);
    let vol_rect = Rect::from_min_size(state.position + Vec2::new(107.0, 57.0), Vec2::new(68.0, 13.0));
    painter.image(
        textures.volume.id(),
        vol_rect,
        Sprites::volume_frame(vol_frame_idx).to_uv(68.0, 433.0),
        Color32::WHITE,
    );
    let vol_resp = ui.allocate_rect(vol_rect, Sense::click_and_drag());
    if vol_resp.dragged() || vol_resp.clicked() {
        if let Some(mouse) = ui.input(|i| i.pointer.interact_pos()) {
            let ratio = ((mouse.x - vol_rect.min.x) / vol_rect.width()).clamp(0.0, 1.0);
            audio.set_volume(ratio);
        }
    }
    // Volume thumb
    let thumb_x = vol_rect.min.x + audio.volume * (vol_rect.width() - 14.0);
    let vol_thumb_rect = Rect::from_min_size(Pos2::new(thumb_x, vol_rect.min.y + 1.0), Vec2::new(14.0, 11.0));
    let vol_thumb_sprite = if vol_resp.is_pointer_button_down_on() { Sprites::VOLUME_THUMB_PRESSED } else { Sprites::VOLUME_THUMB_NORMAL };
    painter.image(
        textures.volume.id(),
        vol_thumb_rect,
        vol_thumb_sprite.to_uv(68.0, 433.0),
        Color32::WHITE,
    );

    // 9. Balance Slider (BALANCE.BMP, x: 177, y: 57, 38x13)
    let bal_ratio = (audio.balance + 1.0) * 0.5;
    let bal_frame_idx = ((bal_ratio * 27.0).round() as usize).min(27);
    let bal_rect = Rect::from_min_size(state.position + Vec2::new(177.0, 57.0), Vec2::new(38.0, 13.0));
    painter.image(
        textures.balance.id(),
        bal_rect,
        Sprites::balance_frame(bal_frame_idx).to_uv(68.0, 433.0),
        Color32::WHITE,
    );
    let bal_resp = ui.allocate_rect(bal_rect, Sense::click_and_drag());
    if bal_resp.dragged() || bal_resp.clicked() {
        if let Some(mouse) = ui.input(|i| i.pointer.interact_pos()) {
            let ratio = ((mouse.x - bal_rect.min.x) / bal_rect.width()).clamp(0.0, 1.0);
            audio.set_balance(ratio * 2.0 - 1.0);
        }
    }
    let bal_thumb_x = bal_rect.min.x + bal_ratio * (bal_rect.width() - 14.0);
    let bal_thumb_rect = Rect::from_min_size(Pos2::new(bal_thumb_x, bal_rect.min.y + 1.0), Vec2::new(14.0, 11.0));
    painter.image(
        textures.balance.id(),
        bal_thumb_rect,
        Sprites::BALANCE_THUMB_NORMAL.to_uv(68.0, 433.0),
        Color32::WHITE,
    );

    // EQ and Playlist Toggle Buttons (x: 219, y: 58 and x: 242, y: 58)
    let eq_toggle_rect = Rect::from_min_size(state.position + Vec2::new(219.0, 58.0), Vec2::new(23.0, 12.0));
    let eq_resp = ui.allocate_rect(eq_toggle_rect, Sense::click());
    if eq_resp.clicked() {
        state.show_eq = !state.show_eq;
        action = MainAction::ToggleEq;
    }
    let eq_sprite = match (state.show_eq, eq_resp.is_pointer_button_down_on()) {
        (true, false) => Sprites::EQ_TOGGLE_ON,
        (true, true) => Sprites::EQ_TOGGLE_ON_PRESSED,
        (false, false) => Sprites::EQ_TOGGLE_OFF,
        (false, true) => Sprites::EQ_TOGGLE_OFF_PRESSED,
    };
    painter.image(textures.shufrep.id(), eq_toggle_rect, eq_sprite.to_uv(92.0, 85.0), Color32::WHITE);

    let pl_toggle_rect = Rect::from_min_size(state.position + Vec2::new(242.0, 58.0), Vec2::new(23.0, 12.0));
    let pl_resp = ui.allocate_rect(pl_toggle_rect, Sense::click());
    if pl_resp.clicked() {
        state.show_playlist = !state.show_playlist;
        action = MainAction::TogglePlaylist;
    }
    let pl_sprite = match (state.show_playlist, pl_resp.is_pointer_button_down_on()) {
        (true, false) => Sprites::PL_TOGGLE_ON,
        (true, true) => Sprites::PL_TOGGLE_ON_PRESSED,
        (false, false) => Sprites::PL_TOGGLE_OFF,
        (false, true) => Sprites::PL_TOGGLE_OFF_PRESSED,
    };
    painter.image(textures.shufrep.id(), pl_toggle_rect, pl_sprite.to_uv(92.0, 85.0), Color32::WHITE);

    // 10. Posbar / Seek Bar (POSBAR.BMP, x: 16, y: 72, 248x10)
    let posbar_rect = Rect::from_min_size(state.position + Vec2::new(16.0, 72.0), Vec2::new(248.0, 10.0));
    painter.image(
        textures.posbar.id(),
        posbar_rect,
        Sprites::POSBAR_TRACK.to_uv(307.0, 10.0),
        Color32::WHITE,
    );
    let posbar_resp = ui.allocate_rect(posbar_rect, Sense::click_and_drag());

    let progress_ratio = if let Some(ref track) = audio.current_track {
        if track.duration > Duration::ZERO {
            (audio.position.as_secs_f32() / track.duration.as_secs_f32()).clamp(0.0, 1.0)
        } else {
            0.0
        }
    } else {
        0.0
    };

    if posbar_resp.dragged() || posbar_resp.clicked() {
        if let Some(mouse) = ui.input(|i| i.pointer.interact_pos()) {
            let ratio = ((mouse.x - posbar_rect.min.x) / posbar_rect.width()).clamp(0.0, 1.0);
            state.seek_target_ratio = Some(ratio);
        }
    }

    let thumb_pos_x = posbar_rect.min.x + progress_ratio * (posbar_rect.width() - 29.0);
    let pos_thumb_rect = Rect::from_min_size(Pos2::new(thumb_pos_x, posbar_rect.min.y), Vec2::new(29.0, 10.0));
    let pos_thumb_sprite = if posbar_resp.is_pointer_button_down_on() {
        Sprites::POSBAR_THUMB_PRESSED
    } else {
        Sprites::POSBAR_THUMB_NORMAL
    };
    painter.image(
        textures.posbar.id(),
        pos_thumb_rect,
        pos_thumb_sprite.to_uv(307.0, 10.0),
        Color32::WHITE,
    );

    // 11. Transport Buttons (CBUTTONS.BMP, y: 88, each 23x18, eject 22x16)
    let prev_rect = Rect::from_min_size(state.position + Vec2::new(16.0, 88.0), Vec2::new(23.0, 18.0));
    let prev_resp = ui.allocate_rect(prev_rect, Sense::click());
    let prev_sprite = if prev_resp.is_pointer_button_down_on() { Sprites::PREV_PRESSED } else { Sprites::PREV_NORMAL };
    painter.image(textures.cbuttons.id(), prev_rect, prev_sprite.to_uv(136.0, 36.0), Color32::WHITE);
    if prev_resp.clicked() { action = MainAction::Prev; }

    let play_rect = Rect::from_min_size(state.position + Vec2::new(39.0, 88.0), Vec2::new(23.0, 18.0));
    let play_resp = ui.allocate_rect(play_rect, Sense::click());
    let play_sprite = if play_resp.is_pointer_button_down_on() { Sprites::PLAY_PRESSED } else { Sprites::PLAY_NORMAL };
    painter.image(textures.cbuttons.id(), play_rect, play_sprite.to_uv(136.0, 36.0), Color32::WHITE);
    if play_resp.clicked() { action = MainAction::Play; }

    let pause_rect = Rect::from_min_size(state.position + Vec2::new(62.0, 88.0), Vec2::new(23.0, 18.0));
    let pause_resp = ui.allocate_rect(pause_rect, Sense::click());
    let pause_sprite = if pause_resp.is_pointer_button_down_on() { Sprites::PAUSE_PRESSED } else { Sprites::PAUSE_NORMAL };
    painter.image(textures.cbuttons.id(), pause_rect, pause_sprite.to_uv(136.0, 36.0), Color32::WHITE);
    if pause_resp.clicked() { action = MainAction::Pause; }

    let stop_rect = Rect::from_min_size(state.position + Vec2::new(85.0, 88.0), Vec2::new(23.0, 18.0));
    let stop_resp = ui.allocate_rect(stop_rect, Sense::click());
    let stop_sprite = if stop_resp.is_pointer_button_down_on() { Sprites::STOP_PRESSED } else { Sprites::STOP_NORMAL };
    painter.image(textures.cbuttons.id(), stop_rect, stop_sprite.to_uv(136.0, 36.0), Color32::WHITE);
    if stop_resp.clicked() { action = MainAction::Stop; }

    let next_rect = Rect::from_min_size(state.position + Vec2::new(108.0, 88.0), Vec2::new(23.0, 18.0));
    let next_resp = ui.allocate_rect(next_rect, Sense::click());
    let next_sprite = if next_resp.is_pointer_button_down_on() { Sprites::NEXT_PRESSED } else { Sprites::NEXT_NORMAL };
    painter.image(textures.cbuttons.id(), next_rect, next_sprite.to_uv(136.0, 36.0), Color32::WHITE);
    if next_resp.clicked() { action = MainAction::Next; }

    let eject_rect = Rect::from_min_size(state.position + Vec2::new(136.0, 89.0), Vec2::new(22.0, 16.0));
    let eject_resp = ui.allocate_rect(eject_rect, Sense::click());
    let eject_sprite = if eject_resp.is_pointer_button_down_on() { Sprites::EJECT_PRESSED } else { Sprites::EJECT_NORMAL };
    painter.image(textures.cbuttons.id(), eject_rect, eject_sprite.to_uv(136.0, 36.0), Color32::WHITE);
    if eject_resp.clicked() { action = MainAction::Eject; }

    // 12. Shuffle & Repeat Buttons (SHUFREP.BMP)
    let shuffle_rect = Rect::from_min_size(state.position + Vec2::new(164.0, 89.0), Vec2::new(47.0, 15.0));
    let shuf_resp = ui.allocate_rect(shuffle_rect, Sense::click());
    if shuf_resp.clicked() {
        state.shuffle = !state.shuffle;
    }
    let shuf_sprite = match (state.shuffle, shuf_resp.is_pointer_button_down_on()) {
        (true, false) => Sprites::SHUFFLE_ON,
        (true, true) => Sprites::SHUFFLE_ON_PRESSED,
        (false, false) => Sprites::SHUFFLE_OFF,
        (false, true) => Sprites::SHUFFLE_OFF_PRESSED,
    };
    painter.image(textures.shufrep.id(), shuffle_rect, shuf_sprite.to_uv(92.0, 85.0), Color32::WHITE);

    let repeat_rect = Rect::from_min_size(state.position + Vec2::new(210.0, 89.0), Vec2::new(28.0, 15.0));
    let rep_resp = ui.allocate_rect(repeat_rect, Sense::click());
    if rep_resp.clicked() {
        state.repeat = !state.repeat;
    }
    let rep_sprite = match (state.repeat, rep_resp.is_pointer_button_down_on()) {
        (true, false) => Sprites::REPEAT_ON,
        (true, true) => Sprites::REPEAT_ON_PRESSED,
        (false, false) => Sprites::REPEAT_OFF,
        (false, true) => Sprites::REPEAT_OFF_PRESSED,
    };
    painter.image(textures.shufrep.id(), repeat_rect, rep_sprite.to_uv(92.0, 85.0), Color32::WHITE);

    action
}

fn render_text_str(painter: &egui::Painter, texture_id: egui::TextureId, start_pos: Pos2, text: &str) {
    for (i, ch) in text.chars().enumerate() {
        let ch_rect = Rect::from_min_size(start_pos + Vec2::new((i as f32) * 5.0, 0.0), Vec2::new(5.0, 6.0));
        let sprite = Sprites::char_rect(ch);
        painter.image(
            texture_id,
            ch_rect,
            sprite.to_uv(155.0, 74.0),
            Color32::WHITE,
        );
    }
}
