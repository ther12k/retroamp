//! Playlist Editor Window UI implementation (PLEDIT.BMP).

use crate::audio::AudioEngine;
use crate::skin::{Skin, PL_DEFAULT_WIDTH, PL_DEFAULT_HEIGHT, SpriteRect, Sprites};
use crate::ui::textures::SkinTextures;
use egui::{Color32, Pos2, Rect, Sense, Ui, Vec2};
use std::path::PathBuf;

pub struct PlaylistEntry {
    pub title: String,
    pub path: PathBuf,
    pub duration_secs: u64,
}

pub struct PlaylistWindowState {
    pub position: Pos2,
    pub height: f32,
    pub selected_index: Option<usize>,
    pub scroll_offset: usize,
    pub items: Vec<PlaylistEntry>,
    pub is_dragging: bool,
    pub drag_offset: Vec2,
}

impl Default for PlaylistWindowState {
    fn default() -> Self {
        Self {
            position: Pos2::new(10.0, 242.0), // docked below EQ (126 + 116 = 242)
            height: PL_DEFAULT_HEIGHT,
            selected_index: None,
            scroll_offset: 0,
            items: Vec::new(),
            is_dragging: false,
            drag_offset: Vec2::ZERO,
        }
    }
}

pub enum PlaylistAction {
    None,
    PlayIndex(usize),
    AddFiles,
    RemoveSelected,
    Clear,
    Close,
}

pub fn render_playlist_window(
    ui: &mut Ui,
    state: &mut PlaylistWindowState,
    current_playing_index: Option<usize>,
    _audio: &AudioEngine,
    _skin: &Skin,
    textures: &SkinTextures,
) -> PlaylistAction {
    let mut action = PlaylistAction::None;
    let window_rect = Rect::from_min_size(state.position, Vec2::new(PL_DEFAULT_WIDTH, state.height));

    // Handle dragging
    let titlebar_rect = Rect::from_min_size(state.position, Vec2::new(PL_DEFAULT_WIDTH - 20.0, 14.0));
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

    // 1. Draw Playlist Frame borders (PLEDIT.BMP)
    // Top bar assembly: left corner(25) + title(100) + fill(25 x N) + right corner(25) = 275
    let top_left_rect = Rect::from_min_size(state.position, Vec2::new(25.0, 20.0));
    painter.image(
        textures.pledit.id(),
        top_left_rect,
        Sprites::PL_TOP_LEFT.to_uv(280.0, 186.0),
        Color32::WHITE,
    );

    let title_rect = Rect::from_min_size(state.position + Vec2::new(25.0, 0.0), Vec2::new(100.0, 20.0));
    painter.image(
        textures.pledit.id(),
        title_rect,
        Sprites::PL_TITLE.to_uv(280.0, 186.0),
        Color32::WHITE,
    );

    // Repeatable fill between title (ends at x=125) and right corner (starts at x=250)
    let fill_start = 125.0;
    let fill_end = PL_DEFAULT_WIDTH - 25.0;
    let mut fx = fill_start;
    while fx < fill_end {
        let w = (25.0f32).min(fill_end - fx);
        let fill_rect = Rect::from_min_size(state.position + Vec2::new(fx, 0.0), Vec2::new(w, 20.0));
        painter.image(
            textures.pledit.id(),
            fill_rect,
            SpriteRect::new(127.0, 0.0, w, 20.0).to_uv(280.0, 186.0),
            Color32::WHITE,
        );
        fx += w;
    }

    let top_right_rect = Rect::from_min_size(state.position + Vec2::new(PL_DEFAULT_WIDTH - 25.0, 0.0), Vec2::new(25.0, 20.0));
    painter.image(
        textures.pledit.id(),
        top_right_rect,
        Sprites::PL_TOP_RIGHT.to_uv(280.0, 186.0),
        Color32::WHITE,
    );

    // Close button (x: 264, y: 3, 9x9)
    let close_rect = Rect::from_min_size(state.position + Vec2::new(264.0, 3.0), Vec2::new(9.0, 9.0));
    let close_resp = ui.allocate_rect(close_rect, Sense::click());
    let close_sprite = if close_resp.is_pointer_button_down_on() {
        Sprites::PL_CLOSE_PRESSED
    } else {
        Sprites::PL_CLOSE_NORMAL
    };
    painter.image(textures.pledit.id(), close_rect, close_sprite.to_uv(280.0, 186.0), Color32::WHITE);
    if close_resp.clicked() {
        action = PlaylistAction::Close;
    }

    // Side Borders (tiled vertically between y: 20 and height - 38)
    let middle_h = (state.height - 58.0).max(0.0);
    let mut y_curr = 20.0;
    while y_curr < 20.0 + middle_h {
        let piece_h = (29.0f32).min(20.0 + middle_h - y_curr);
        let left_border_rect = Rect::from_min_size(state.position + Vec2::new(0.0, y_curr), Vec2::new(12.0, piece_h));
        painter.image(
            textures.pledit.id(),
            left_border_rect,
            SpriteRect::new(0.0, 42.0, 12.0, piece_h).to_uv(280.0, 186.0),
            Color32::WHITE,
        );

        let right_border_rect = Rect::from_min_size(state.position + Vec2::new(PL_DEFAULT_WIDTH - 20.0, y_curr), Vec2::new(20.0, piece_h));
        painter.image(
            textures.pledit.id(),
            right_border_rect,
            SpriteRect::new(31.0, 42.0, 20.0, piece_h).to_uv(280.0, 186.0),
            Color32::WHITE,
        );
        y_curr += piece_h;
    }

    // Bottom bar: left (125px) + right (150px) = 275px
    let bottom_left_rect = Rect::from_min_size(
        state.position + Vec2::new(0.0, state.height - 38.0),
        Vec2::new(125.0, 38.0),
    );
    painter.image(
        textures.pledit.id(),
        bottom_left_rect,
        Sprites::PL_BOTTOM_LEFT.to_uv(280.0, 186.0),
        Color32::WHITE,
    );

    let bottom_right_rect = Rect::from_min_size(
        state.position + Vec2::new(125.0, state.height - 38.0),
        Vec2::new(150.0, 38.0),
    );
    painter.image(
        textures.pledit.id(),
        bottom_right_rect,
        Sprites::PL_BOTTOM_RIGHT.to_uv(280.0, 186.0),
        Color32::WHITE,
    );

    // 2. Playlist Center Background
    let list_area = Rect::from_min_max(
        state.position + Vec2::new(12.0, 20.0),
        state.position + Vec2::new(PL_DEFAULT_WIDTH - 20.0, state.height - 38.0),
    );
    painter.rect_filled(list_area, 0.0, Color32::from_rgb(0, 0, 0));

    // Mouse wheel scrolling for playlist
    let line_height = 13.0;
    let visible_lines = ((list_area.height() / line_height).floor() as usize).max(1);

    if ui.rect_contains_pointer(list_area) {
        let scroll_delta = ui.input(|i| i.raw_scroll_delta.y);
        if scroll_delta > 0.0 && state.scroll_offset > 0 {
            state.scroll_offset = state.scroll_offset.saturating_sub(1);
        } else if scroll_delta < 0.0 && state.scroll_offset + visible_lines < state.items.len() {
            state.scroll_offset = (state.scroll_offset + 1).min(state.items.len().saturating_sub(visible_lines));
        }
    }

    // Ensure scroll offset is within valid range
    if state.scroll_offset + visible_lines > state.items.len() {
        state.scroll_offset = state.items.len().saturating_sub(visible_lines);
    }

    // 3. Render Playlist Items

    for (i, entry_idx) in (state.scroll_offset..state.items.len()).take(visible_lines).enumerate() {
        let entry = &state.items[entry_idx];
        let y = list_area.min.y + (i as f32) * line_height;
        let item_rect = Rect::from_min_size(Pos2::new(list_area.min.x, y), Vec2::new(list_area.width(), line_height));

        let is_selected = state.selected_index == Some(entry_idx);
        let is_playing = current_playing_index == Some(entry_idx);

        if is_selected {
            painter.rect_filled(item_rect, 0.0, Color32::from_rgb(0, 0, 140));
        }

        let item_resp = ui.allocate_rect(item_rect, Sense::click());
        if item_resp.clicked() {
            state.selected_index = Some(entry_idx);
        }
        if item_resp.double_clicked() {
            action = PlaylistAction::PlayIndex(entry_idx);
        }

        // Text formatting: "1. Artist - Title" and right-aligned "03:45"
        let line_text = format!("{}. {}", entry_idx + 1, entry.title);
        let dur_text = format!("{:02}:{:02}", entry.duration_secs / 60, entry.duration_secs % 60);

        // Render with Winamp font
        let _text_color = if is_playing {
            Color32::from_rgb(0, 255, 64)
        } else if is_selected {
            Color32::WHITE
        } else {
            Color32::from_rgb(0, 200, 0)
        };

        let mut x = item_rect.min.x + 2.0;
        for ch in line_text.chars().take(34) {
            let ch_rect = Rect::from_min_size(Pos2::new(x, y + 3.0), Vec2::new(5.0, 6.0));
            let sprite = if is_selected {
                Sprites::char_rect_selected(ch)
            } else {
                Sprites::char_rect(ch)
            };
            painter.image(textures.text.id(), ch_rect, sprite.to_uv(155.0, 74.0), Color32::WHITE);
            x += 5.0;
        }

        let mut dur_x = item_rect.max.x - 30.0;
        for ch in dur_text.chars() {
            let ch_rect = Rect::from_min_size(Pos2::new(dur_x, y + 3.0), Vec2::new(5.0, 6.0));
            let sprite = if is_selected {
                Sprites::char_rect_selected(ch)
            } else {
                Sprites::char_rect(ch)
            };
            painter.image(textures.text.id(), ch_rect, sprite.to_uv(155.0, 74.0), Color32::WHITE);
            dur_x += 5.0;
        }
    }

    // 5. Bottom-edge resize grip (like original Winamp: drag to change the
    //    number of visible track rows; snaps to 13px row increments).
    {
        let grip_rect = Rect::from_min_max(
            state.position + Vec2::new(13.0, state.height - 7.0),
            state.position + Vec2::new(PL_DEFAULT_WIDTH - 21.0, state.height),
        );
        let grip_resp = ui.allocate_rect(grip_rect, Sense::drag());
        grip_resp.clone().on_hover_cursor(egui::CursorIcon::ResizeVertical);

        // Smooth resize: accumulate *relative* pointer movement and snap the
        // running total to row increments. (Absolute pointer math jitters,
        // because the host window resizes underneath the cursor mid-drag.)
        if grip_resp.dragged() {
            const CHROME: f32 = 58.0;
            const ROW_H: f32 = 13.0;
            let rows = ((state.height - CHROME) / ROW_H).round();
            let new_rows = (rows + grip_resp.drag_delta().y / ROW_H).clamp(2.0, 64.0);
            state.height = CHROME + new_rows * ROW_H;
        }
    }

    // 4. Bottom Action Buttons (+ ADD, - REM, SEL, MISC)
    // ADD File Button (+) at bottom left: x: 14, y: height - 30, 22x18
    let add_rect = Rect::from_min_size(state.position + Vec2::new(14.0, state.height - 30.0), Vec2::new(22.0, 18.0));
    let add_resp = ui.allocate_rect(add_rect, Sense::click());
    if add_resp.clicked() {
        action = PlaylistAction::AddFiles;
    }

    // REM Button (-) at x: 44, y: height - 30, 22x18
    let rem_rect = Rect::from_min_size(state.position + Vec2::new(44.0, state.height - 30.0), Vec2::new(22.0, 18.0));
    let rem_resp = ui.allocate_rect(rem_rect, Sense::click());
    if rem_resp.clicked() {
        action = PlaylistAction::RemoveSelected;
    }

    // Total Duration Display at bottom right (above MIN/SEC labels)
    let total_secs: u64 = state.items.iter().map(|it| it.duration_secs).sum();
    let total_str = format!("{:02}:{:02}", total_secs / 60, total_secs % 60);
    let mut tx = state.position.x + PL_DEFAULT_WIDTH - 82.0;
    let ty = state.position.y + state.height - 31.0;
    for ch in total_str.chars() {
        let ch_rect = Rect::from_min_size(Pos2::new(tx, ty), Vec2::new(5.0, 6.0));
        let sprite = Sprites::char_rect(ch);
        painter.image(textures.text.id(), ch_rect, sprite.to_uv(155.0, 74.0), Color32::from_rgb(0, 255, 64));
        tx += 5.0;
    }

    action
}
