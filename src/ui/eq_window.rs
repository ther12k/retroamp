//! Equalizer Window UI implementation (EQMAIN.BMP).

use crate::audio::AudioEngine;
use crate::skin::{Skin, EQ_WIDTH, EQ_HEIGHT, SpriteRect};
use crate::ui::textures::SkinTextures;
use egui::{Color32, Pos2, Rect, Sense, Stroke, Ui, Vec2};

pub struct EqWindowState {
    pub position: Pos2,
    pub is_dragging: bool,
    pub drag_offset: Vec2,
    pub active_slider: Option<usize>, // 0: Preamp, 1..=10: Bands
    /// Preset picker popup (shared by every theme's EQ renderer).
    pub show_presets: bool,
}

impl Default for EqWindowState {
    fn default() -> Self {
        Self {
            position: Pos2::new(10.0, 126.0), // default docked right below main window (10 + 116 = 126)
            is_dragging: false,
            drag_offset: Vec2::ZERO,
            active_slider: None,
            show_presets: false,
        }
    }
}

pub enum EqAction {
    None,
    Close,
}

pub fn render_eq_window(
    ui: &mut Ui,
    state: &mut EqWindowState,
    audio: &mut AudioEngine,
    _skin: &Skin,
    textures: &SkinTextures,
) -> EqAction {
    let mut action = EqAction::None;
    let window_rect = Rect::from_min_size(state.position, Vec2::new(EQ_WIDTH, EQ_HEIGHT));

    // Dragging handling
    let titlebar_rect = Rect::from_min_size(state.position, Vec2::new(EQ_WIDTH - 20.0, 14.0));
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

    // 1. Draw EQ Window Background (EQMAIN.BMP, 275x116 from top of texture)
    let eq_uv = Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 116.0 / 315.0));
    painter.image(textures.eqmain.id(), window_rect, eq_uv, Color32::WHITE);

    // 2. Draw Active Titlebar Strip (EQMAIN.BMP, 275x14 at y: 134)
    let eq_tb_rect = Rect::from_min_size(state.position, Vec2::new(EQ_WIDTH, 14.0));
    painter.image(
        textures.eqmain.id(),
        eq_tb_rect,
        crate::skin::Sprites::EQ_TITLEBAR_ACTIVE.to_uv(275.0, 315.0),
        Color32::WHITE,
    );

    // Close button (x: 264, y: 3, 9x9)
    let close_rect = Rect::from_min_size(state.position + Vec2::new(264.0, 3.0), Vec2::new(9.0, 9.0));
    let close_resp = ui.allocate_rect(close_rect, Sense::click());
    let close_sprite = if close_resp.is_pointer_button_down_on() {
        crate::skin::Sprites::EQ_CLOSE_PRESSED
    } else {
        crate::skin::Sprites::EQ_CLOSE_NORMAL
    };
    painter.image(textures.eqmain.id(), close_rect, close_sprite.to_uv(275.0, 315.0), Color32::WHITE);
    if close_resp.clicked() {
        action = EqAction::Close;
    }

    // 3. EQ ON Button (x: 14, y: 18, 25x12)
    let on_rect = Rect::from_min_size(state.position + Vec2::new(14.0, 18.0), Vec2::new(25.0, 12.0));
    let on_resp = ui.allocate_rect(on_rect, Sense::click());
    if on_resp.clicked() {
        audio.eq.enabled = !audio.eq.enabled;
    }
    // ON button sprite in EQMAIN.BMP
    let on_sprite = if audio.eq.enabled {
        SpriteRect::new(10.0, 119.0, 25.0, 12.0)
    } else {
        SpriteRect::new(69.0, 119.0, 25.0, 12.0)
    };
    painter.image(textures.eqmain.id(), on_rect, on_sprite.to_uv(275.0, 315.0), Color32::WHITE);

    // PRESETS button (x: 202, y: 18, 60x12) — opens the classic preset list
    let presets_rect = Rect::from_min_size(state.position + Vec2::new(202.0, 18.0), Vec2::new(60.0, 12.0));
    let presets_resp = ui.allocate_rect(presets_rect, Sense::click());
    painter.rect_filled(presets_rect, 1.0, if presets_resp.hovered() || state.show_presets {
        Color32::from_rgb(70, 80, 110)
    } else {
        Color32::from_rgb(40, 48, 70)
    });
    painter.rect_stroke(presets_rect, 1.0, Stroke::new(1.0_f32, Color32::BLACK), egui::StrokeKind::Inside);
    painter.text(
        presets_rect.center(),
        egui::Align2::CENTER_CENTER,
        "PRESETS",
        egui::FontId::new(7.5, egui::FontFamily::Proportional),
        Color32::from_rgb(210, 215, 230),
    );
    if presets_resp.clicked() {
        state.show_presets = !state.show_presets;
    }

    // 4. Equalizer Graph (x: 86, y: 17, 113x19)
    let graph_rect = Rect::from_min_size(state.position + Vec2::new(86.0, 17.0), Vec2::new(113.0, 19.0));
    painter.rect_filled(graph_rect, 0.0, Color32::from_rgb(0, 0, 0));

    // Draw equalizer response curve in graph
    if audio.eq.enabled {
        let center_y = graph_rect.min.y + 9.5;
        let mut points = Vec::with_capacity(113);
        for x_step in 0..113 {
            let ratio = x_step as f32 / 112.0;
            // Interpolate across 10 bands
            let band_f = ratio * 9.0;
            let b0 = (band_f.floor() as usize).min(9);
            let b1 = (b0 + 1).min(9);
            let t = band_f - b0 as f32;
            let db = audio.eq.bands[b0] * (1.0 - t) + audio.eq.bands[b1] * t + audio.eq.preamp * 0.5;
            let y = (center_y - (db / 12.0) * 8.0).clamp(graph_rect.min.y + 1.0, graph_rect.max.y - 2.0);
            points.push(Pos2::new(graph_rect.min.x + x_step as f32, y));
        }

        for w in points.windows(2) {
            painter.line_segment([w[0], w[1]], (1.0, Color32::from_rgb(0, 255, 64)));
        }
    }

    // 5. Preamp Slider (x: 21, y: 38, track 14x63)
    let preamp_rect = Rect::from_min_size(state.position + Vec2::new(21.0, 38.0), Vec2::new(14.0, 63.0));
    let preamp_resp = ui.allocate_rect(preamp_rect, Sense::click_and_drag());
    if preamp_resp.dragged() || preamp_resp.clicked() {
        if let Some(mouse) = ui.input(|i| i.pointer.interact_pos()) {
            let ratio = 1.0 - ((mouse.y - preamp_rect.min.y) / preamp_rect.height()).clamp(0.0, 1.0);
            audio.eq.preamp = ratio * 24.0 - 12.0; // -12dB to +12dB
        }
    }
    // Preamp Thumb (11x11, sprite at x: 0, y: 164 in EQMAIN.BMP)
    let preamp_thumb_ratio = (audio.eq.preamp + 12.0) / 24.0;
    let preamp_thumb_y = preamp_rect.max.y - 11.0 - preamp_thumb_ratio * (preamp_rect.height() - 11.0);
    let preamp_thumb_rect = Rect::from_min_size(Pos2::new(preamp_rect.min.x + 1.5, preamp_thumb_y), Vec2::new(11.0, 11.0));
    painter.image(
        textures.eqmain.id(),
        preamp_thumb_rect,
        SpriteRect::new(0.0, 164.0, 11.0, 11.0).to_uv(275.0, 315.0),
        Color32::WHITE,
    );

    // 6. 10 Frequency Band Sliders (x: 78, 96, 114, 132, 150, 168, 186, 204, 222, 240; y: 38)
    let band_xs = [78.0, 96.0, 114.0, 132.0, 150.0, 168.0, 186.0, 204.0, 222.0, 240.0];
    for (i, &bx) in band_xs.iter().enumerate() {
        let band_rect = Rect::from_min_size(state.position + Vec2::new(bx, 38.0), Vec2::new(14.0, 63.0));
        let band_resp = ui.allocate_rect(band_rect, Sense::click_and_drag());
        if band_resp.dragged() || band_resp.clicked() {
            if let Some(mouse) = ui.input(|i| i.pointer.interact_pos()) {
                let ratio = 1.0 - ((mouse.y - band_rect.min.y) / band_rect.height()).clamp(0.0, 1.0);
                audio.eq.bands[i] = ratio * 24.0 - 12.0;
            }
        }

        // Thumb position
        let thumb_ratio = (audio.eq.bands[i] + 12.0) / 24.0;
        let thumb_y = band_rect.max.y - 11.0 - thumb_ratio * (band_rect.height() - 11.0);
        let thumb_rect = Rect::from_min_size(Pos2::new(band_rect.min.x + 1.5, thumb_y), Vec2::new(11.0, 11.0));
        painter.image(
            textures.eqmain.id(),
            thumb_rect,
            SpriteRect::new(0.0, 164.0, 11.0, 11.0).to_uv(275.0, 315.0),
            Color32::WHITE,
        );
    }

    action
}
