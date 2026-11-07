//! Winamp classic skin loader and sprite atlas.
//!
//! Supports classic 2.x skin assets:
//! - Default embedded assets
//! - .wsz / .zip skin archives
//! - Extracted skin directories
//! - viscolor.txt color palette

use egui::{Color32, ColorImage, Rect, Pos2, Vec2};
use std::collections::HashMap;
use std::io::Read as _;
use std::path::Path;

pub const MAIN_WIDTH: f32 = 275.0;
pub const MAIN_HEIGHT: f32 = 116.0;
pub const EQ_WIDTH: f32 = 275.0;
pub const EQ_HEIGHT: f32 = 116.0;
pub const PL_DEFAULT_WIDTH: f32 = 275.0;
pub const PL_DEFAULT_HEIGHT: f32 = 116.0;

/// Embedded default skin images
const RAW_MAIN: &[u8] = include_bytes!("../assets/skins/default/main.png");
const RAW_CBUTTONS: &[u8] = include_bytes!("../assets/skins/default/cbuttons.png");
const RAW_TITLEBAR: &[u8] = include_bytes!("../assets/skins/default/titlebar.png");
const RAW_NUMBERS: &[u8] = include_bytes!("../assets/skins/default/numbers.png");
const RAW_TEXT: &[u8] = include_bytes!("../assets/skins/default/text.png");
const RAW_VOLUME: &[u8] = include_bytes!("../assets/skins/default/volume.png");
const RAW_BALANCE: &[u8] = include_bytes!("../assets/skins/default/balance.png");
const RAW_POSBAR: &[u8] = include_bytes!("../assets/skins/default/posbar.png");
const RAW_PLAYPAUS: &[u8] = include_bytes!("../assets/skins/default/playpaus.png");
const RAW_SHUFREP: &[u8] = include_bytes!("../assets/skins/default/shufrep.png");
const RAW_MONOSTER: &[u8] = include_bytes!("../assets/skins/default/monoster.png");
const RAW_EQMAIN: &[u8] = include_bytes!("../assets/skins/default/eqmain.png");
const RAW_EQ_EX: &[u8] = include_bytes!("../assets/skins/default/eq_ex.png");
const RAW_PLEDIT: &[u8] = include_bytes!("../assets/skins/default/pledit.png");
const RAW_VISCOLOR: &str = include_str!("../assets/skins/default/viscolor.txt");

#[derive(Clone)]
pub struct Skin {
    pub name: String,
    pub main: ColorImage,
    pub cbuttons: ColorImage,
    pub titlebar: ColorImage,
    pub numbers: ColorImage,
    pub text: ColorImage,
    pub volume: ColorImage,
    pub balance: ColorImage,
    pub posbar: ColorImage,
    pub playpaus: ColorImage,
    pub shufrep: ColorImage,
    pub monoster: ColorImage,
    pub eqmain: ColorImage,
    pub eq_ex: ColorImage,
    pub pledit: ColorImage,
    pub vis_colors: [Color32; 24],
}

impl Skin {
    /// Load the default classic Winamp skin embedded in the binary.
    pub fn load_default() -> Self {
        Self {
            name: "Winamp (Classic)".to_string(),
            main: decode_image(RAW_MAIN).unwrap_or_else(fallback_image),
            cbuttons: decode_image(RAW_CBUTTONS).unwrap_or_else(fallback_image),
            titlebar: decode_image(RAW_TITLEBAR).unwrap_or_else(fallback_image),
            numbers: decode_image(RAW_NUMBERS).unwrap_or_else(fallback_image),
            text: decode_image(RAW_TEXT).unwrap_or_else(fallback_image),
            volume: decode_image(RAW_VOLUME).unwrap_or_else(fallback_image),
            balance: decode_image(RAW_BALANCE).unwrap_or_else(fallback_image),
            posbar: decode_image(RAW_POSBAR).unwrap_or_else(fallback_image),
            playpaus: decode_image(RAW_PLAYPAUS).unwrap_or_else(fallback_image),
            shufrep: decode_image(RAW_SHUFREP).unwrap_or_else(fallback_image),
            monoster: decode_image(RAW_MONOSTER).unwrap_or_else(fallback_image),
            eqmain: decode_image(RAW_EQMAIN).unwrap_or_else(fallback_image),
            eq_ex: decode_image(RAW_EQ_EX).unwrap_or_else(fallback_image),
            pledit: decode_image(RAW_PLEDIT).unwrap_or_else(fallback_image),
            vis_colors: parse_viscolor(RAW_VISCOLOR),
        }
    }

    /// Load a skin from a .wsz or .zip archive file safely.
    pub fn load_wsz<P: AsRef<Path>>(path: P) -> Result<Self, String> {
        let path_ref = path.as_ref();
        let name = path_ref
            .file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| "Custom Skin".to_string());

        let file = std::fs::File::open(path_ref).map_err(|e| e.to_string())?;
        let mut archive = zip::ZipArchive::new(file).map_err(|e| e.to_string())?;

        const KNOWN_ASSETS: &[&str] = &[
            "MAIN.BMP", "CBUTTONS.BMP", "TITLEBAR.BMP", "NUMBERS.BMP", "TEXT.BMP",
            "VOLUME.BMP", "BALANCE.BMP", "PANBAR.BMP", "POSBAR.BMP", "PLAYPAUS.BMP",
            "SHUFREP.BMP", "MONOSTER.BMP", "EQMAIN.BMP", "EQ_EX.BMP", "PLEDIT.BMP",
            "VISCOLOR.TXT"
        ];
        const MAX_ENTRY_BYTES: u64 = 8 * 1024 * 1024; // 8MB safety ceiling

        let mut files: HashMap<String, Vec<u8>> = HashMap::new();
        let max_entries = archive.len().min(128);

        for i in 0..max_entries {
            if let Ok(mut entry) = archive.by_index(i) {
                if entry.is_file() {
                    let filename = entry.name().to_ascii_uppercase().replace('\\', "/");
                    let base_name = filename.rsplit('/').next().unwrap_or(&filename).to_string();

                    if KNOWN_ASSETS.contains(&base_name.as_str()) {
                        if entry.size() <= MAX_ENTRY_BYTES {
                            let mut data = Vec::with_capacity((entry.size() as usize).min(1024 * 1024));
                            if entry.by_ref().take(MAX_ENTRY_BYTES).read_to_end(&mut data).is_ok() {
                                files.insert(base_name, data);
                            }
                        }
                    }
                }
            }
        }

        // Must contain at least one essential skin file
        let has_essential = files.contains_key("MAIN.BMP")
            || files.contains_key("CBUTTONS.BMP")
            || files.contains_key("TITLEBAR.BMP")
            || files.contains_key("EQMAIN.BMP")
            || files.contains_key("PLEDIT.BMP");

        if !has_essential {
            return Err("Archive does not contain classic Winamp skin bitmaps".to_string());
        }

        let mut skin = Self::load_default();
        skin.name = name;

        if let Some(data) = files.get("MAIN.BMP") {
            if let Some(img) = decode_image(data) { skin.main = img; }
        }
        if let Some(data) = files.get("CBUTTONS.BMP") {
            if let Some(img) = decode_image(data) { skin.cbuttons = img; }
        }
        if let Some(data) = files.get("TITLEBAR.BMP") {
            if let Some(img) = decode_image(data) { skin.titlebar = img; }
        }
        if let Some(data) = files.get("NUMBERS.BMP") {
            if let Some(img) = decode_image(data) { skin.numbers = img; }
        }
        if let Some(data) = files.get("TEXT.BMP") {
            if let Some(img) = decode_image(data) { skin.text = img; }
        }
        if let Some(data) = files.get("VOLUME.BMP") {
            if let Some(img) = decode_image(data) { skin.volume = img; }
        }
        if let Some(data) = files.get("BALANCE.BMP").or_else(|| files.get("PANBAR.BMP")) {
            if let Some(img) = decode_image(data) { skin.balance = img; }
        }
        if let Some(data) = files.get("POSBAR.BMP") {
            if let Some(img) = decode_image(data) { skin.posbar = img; }
        }
        if let Some(data) = files.get("PLAYPAUS.BMP") {
            if let Some(img) = decode_image(data) { skin.playpaus = img; }
        }
        if let Some(data) = files.get("SHUFREP.BMP") {
            if let Some(img) = decode_image(data) { skin.shufrep = img; }
        }
        if let Some(data) = files.get("MONOSTER.BMP") {
            if let Some(img) = decode_image(data) { skin.monoster = img; }
        }
        if let Some(data) = files.get("EQMAIN.BMP") {
            if let Some(img) = decode_image(data) { skin.eqmain = img; }
        }
        if let Some(data) = files.get("EQ_EX.BMP") {
            if let Some(img) = decode_image(data) { skin.eq_ex = img; }
        }
        if let Some(data) = files.get("PLEDIT.BMP") {
            if let Some(img) = decode_image(data) { skin.pledit = img; }
        }
        if let Some(data) = files.get("VISCOLOR.TXT") {
            if let Ok(vis_text) = std::str::from_utf8(data) {
                skin.vis_colors = parse_viscolor(vis_text);
            }
        }

        Ok(skin)
    }
}

/// Helper function to decode images from BMP or PNG data
pub fn decode_image(data: &[u8]) -> Option<ColorImage> {
    if let Ok(img) = image::load_from_memory(data) {
        if img.width() > 2048 || img.height() > 2048 {
            return None;
        }
        let rgba = img.to_rgba8();
        let size = [rgba.width() as usize, rgba.height() as usize];
        let pixels = rgba.into_raw();
        Some(ColorImage::from_rgba_unmultiplied(size, &pixels))
    } else {
        None
    }
}

fn fallback_image() -> ColorImage {
    ColorImage::new([1, 1], Color32::from_rgb(30, 30, 30))
}

/// Parse viscolor.txt
pub fn parse_viscolor(content: &str) -> [Color32; 24] {
    let mut colors = [
        Color32::from_rgb(0, 0, 0),       // 0: background
        Color32::from_rgb(24, 33, 41),    // 1: dots
        // 2..17: 16 spectrum analyzer colors (bottom to top)
        Color32::from_rgb(239, 49, 16),
        Color32::from_rgb(206, 41, 16),
        Color32::from_rgb(214, 90, 0),
        Color32::from_rgb(214, 102, 0),
        Color32::from_rgb(214, 115, 0),
        Color32::from_rgb(198, 123, 8),
        Color32::from_rgb(173, 132, 8),
        Color32::from_rgb(156, 140, 8),
        Color32::from_rgb(132, 148, 16),
        Color32::from_rgb(115, 156, 16),
        Color32::from_rgb(90, 165, 16),
        Color32::from_rgb(74, 173, 16),
        Color32::from_rgb(49, 181, 16),
        Color32::from_rgb(24, 189, 24),
        Color32::from_rgb(0, 198, 24),
        Color32::from_rgb(132, 132, 132),
        // 18..23: 5 oscilloscope waveform colors (center to edge)
        Color32::from_rgb(255, 255, 255),
        Color32::from_rgb(214, 214, 222),
        Color32::from_rgb(181, 189, 189),
        Color32::from_rgb(160, 170, 175),
        Color32::from_rgb(148, 156, 165),
        Color32::from_rgb(150, 150, 150),
    ];

    let mut idx = 0;
    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('/') || line.starts_with(';') {
            continue;
        }
        let parts: Vec<&str> = line.split([',', ' ', '\t']).filter(|p| !p.is_empty()).collect();
        if parts.len() >= 3 && idx < 24 {
            if let (Ok(r), Ok(g), Ok(b)) = (parts[0].parse::<u8>(), parts[1].parse::<u8>(), parts[2].parse::<u8>()) {
                colors[idx] = Color32::from_rgb(r, g, b);
                idx += 1;
            }
        }
    }

    colors
}

/// UV / Sub-texture coordinates in an image
#[derive(Copy, Clone, Debug)]
pub struct SpriteRect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl SpriteRect {
    pub const fn new(x: f32, y: f32, w: f32, h: f32) -> Self {
        Self { x, y, w, h }
    }

    pub fn to_uv(&self, img_w: f32, img_h: f32) -> Rect {
        Rect::from_min_max(
            Pos2::new(self.x / img_w, self.y / img_h),
            Pos2::new((self.x + self.w) / img_w, (self.y + self.h) / img_h),
        )
    }

    pub fn size(&self) -> Vec2 {
        Vec2::new(self.w, self.h)
    }
}

/// Classic Winamp sprite locations
pub struct Sprites;

impl Sprites {
    // Transport buttons in CBUTTONS.BMP (each button is 23x18, eject is 22x16)
    // Row 0: Normal, Row 1: Pressed
    pub const PREV_NORMAL: SpriteRect = SpriteRect::new(0.0, 0.0, 23.0, 18.0);
    pub const PREV_PRESSED: SpriteRect = SpriteRect::new(0.0, 18.0, 23.0, 18.0);
    pub const PLAY_NORMAL: SpriteRect = SpriteRect::new(23.0, 0.0, 23.0, 18.0);
    pub const PLAY_PRESSED: SpriteRect = SpriteRect::new(23.0, 18.0, 23.0, 18.0);
    pub const PAUSE_NORMAL: SpriteRect = SpriteRect::new(46.0, 0.0, 23.0, 18.0);
    pub const PAUSE_PRESSED: SpriteRect = SpriteRect::new(46.0, 18.0, 23.0, 18.0);
    pub const STOP_NORMAL: SpriteRect = SpriteRect::new(69.0, 0.0, 23.0, 18.0);
    pub const STOP_PRESSED: SpriteRect = SpriteRect::new(69.0, 18.0, 23.0, 18.0);
    pub const NEXT_NORMAL: SpriteRect = SpriteRect::new(92.0, 0.0, 23.0, 18.0);
    pub const NEXT_PRESSED: SpriteRect = SpriteRect::new(92.0, 18.0, 23.0, 18.0);
    pub const EJECT_NORMAL: SpriteRect = SpriteRect::new(114.0, 0.0, 22.0, 16.0);
    pub const EJECT_PRESSED: SpriteRect = SpriteRect::new(114.0, 16.0, 22.0, 16.0);

    // Titlebar in TITLEBAR.BMP
    pub const TITLEBAR_ACTIVE: SpriteRect = SpriteRect::new(27.0, 0.0, 275.0, 14.0);
    pub const TITLEBAR_INACTIVE: SpriteRect = SpriteRect::new(27.0, 15.0, 275.0, 14.0);
    pub const MINIMIZE_NORMAL: SpriteRect = SpriteRect::new(9.0, 0.0, 9.0, 9.0);
    pub const MINIMIZE_PRESSED: SpriteRect = SpriteRect::new(9.0, 9.0, 9.0, 9.0);
    pub const CLOSE_NORMAL: SpriteRect = SpriteRect::new(18.0, 0.0, 9.0, 9.0);
    pub const CLOSE_PRESSED: SpriteRect = SpriteRect::new(18.0, 9.0, 9.0, 9.0);
    pub const SHADE_NORMAL: SpriteRect = SpriteRect::new(0.0, 0.0, 9.0, 9.0);
    pub const SHADE_PRESSED: SpriteRect = SpriteRect::new(0.0, 9.0, 9.0, 9.0);

    // Numbers in NUMBERS.BMP (0-9 are 9x13 each, offset x = digit * 9; blank is at x = 90)
    pub fn number_rect(digit: usize) -> SpriteRect {
        let x = if digit < 10 { (digit as f32) * 9.0 } else { 90.0 };
        SpriteRect::new(x, 0.0, 9.0, 13.0)
    }
    pub const NUMBER_MINUS: SpriteRect = SpriteRect::new(90.0, 0.0, 9.0, 13.0);

    // Play/Pause/Stop indicator in PLAYPAUS.BMP
    pub const INDICATOR_PLAY: SpriteRect = SpriteRect::new(0.0, 0.0, 9.0, 9.0);
    pub const INDICATOR_PAUSE: SpriteRect = SpriteRect::new(9.0, 0.0, 9.0, 9.0);
    pub const INDICATOR_STOP: SpriteRect = SpriteRect::new(18.0, 0.0, 9.0, 9.0);

    // Mono / Stereo in MONOSTER.BMP
    pub const STEREO_ACTIVE: SpriteRect = SpriteRect::new(0.0, 0.0, 29.0, 12.0);
    pub const STEREO_INACTIVE: SpriteRect = SpriteRect::new(0.0, 12.0, 29.0, 12.0);
    pub const MONO_ACTIVE: SpriteRect = SpriteRect::new(29.0, 0.0, 29.0, 12.0);
    pub const MONO_INACTIVE: SpriteRect = SpriteRect::new(29.0, 12.0, 29.0, 12.0);

    // Shuffle & Repeat in SHUFREP.BMP
    // Shuffle: 47x15, Repeat: 28x15
    pub const SHUFFLE_OFF: SpriteRect = SpriteRect::new(28.0, 0.0, 47.0, 15.0);
    pub const SHUFFLE_OFF_PRESSED: SpriteRect = SpriteRect::new(28.0, 15.0, 47.0, 15.0);
    pub const SHUFFLE_ON: SpriteRect = SpriteRect::new(28.0, 30.0, 47.0, 15.0);
    pub const SHUFFLE_ON_PRESSED: SpriteRect = SpriteRect::new(28.0, 45.0, 47.0, 15.0);

    pub const REPEAT_OFF: SpriteRect = SpriteRect::new(0.0, 0.0, 28.0, 15.0);
    pub const REPEAT_OFF_PRESSED: SpriteRect = SpriteRect::new(0.0, 15.0, 28.0, 15.0);
    pub const REPEAT_ON: SpriteRect = SpriteRect::new(0.0, 30.0, 28.0, 15.0);
    pub const REPEAT_ON_PRESSED: SpriteRect = SpriteRect::new(0.0, 45.0, 28.0, 15.0);

    // EQ and Playlist toggle buttons in SHUFREP.BMP
    pub const EQ_TOGGLE_OFF: SpriteRect = SpriteRect::new(0.0, 61.0, 23.0, 12.0);
    pub const EQ_TOGGLE_OFF_PRESSED: SpriteRect = SpriteRect::new(0.0, 73.0, 23.0, 12.0);
    pub const EQ_TOGGLE_ON: SpriteRect = SpriteRect::new(46.0, 61.0, 23.0, 12.0);
    pub const EQ_TOGGLE_ON_PRESSED: SpriteRect = SpriteRect::new(46.0, 73.0, 23.0, 12.0);

    pub const PL_TOGGLE_OFF: SpriteRect = SpriteRect::new(23.0, 61.0, 23.0, 12.0);
    pub const PL_TOGGLE_OFF_PRESSED: SpriteRect = SpriteRect::new(23.0, 73.0, 23.0, 12.0);
    pub const PL_TOGGLE_ON: SpriteRect = SpriteRect::new(69.0, 61.0, 23.0, 12.0);
    pub const PL_TOGGLE_ON_PRESSED: SpriteRect = SpriteRect::new(69.0, 73.0, 23.0, 12.0);

    // Equalizer Window Titlebar in EQMAIN.BMP (canonical Webamp coordinates)
    pub const EQ_TITLEBAR_ACTIVE: SpriteRect = SpriteRect::new(0.0, 134.0, 275.0, 14.0);
    pub const EQ_TITLEBAR_INACTIVE: SpriteRect = SpriteRect::new(0.0, 149.0, 275.0, 14.0);
    pub const EQ_CLOSE_NORMAL: SpriteRect = SpriteRect::new(0.0, 116.0, 9.0, 9.0);
    pub const EQ_CLOSE_PRESSED: SpriteRect = SpriteRect::new(0.0, 125.0, 9.0, 9.0);

    // Playlist Window Assembly in PLEDIT.BMP (canonical Webamp coordinates)
    // Top bar: left(25) + title(100) + fill(25 x N) + right(25) = 275
    pub const PL_TOP_LEFT: SpriteRect = SpriteRect::new(0.0, 0.0, 25.0, 20.0);
    pub const PL_TITLE: SpriteRect = SpriteRect::new(26.0, 0.0, 100.0, 20.0);
    pub const PL_TOP_FILL: SpriteRect = SpriteRect::new(127.0, 0.0, 25.0, 20.0);
    pub const PL_TOP_RIGHT: SpriteRect = SpriteRect::new(153.0, 0.0, 25.0, 20.0);
    pub const PL_LEFT_BORDER: SpriteRect = SpriteRect::new(0.0, 42.0, 12.0, 29.0);
    pub const PL_RIGHT_BORDER: SpriteRect = SpriteRect::new(31.0, 42.0, 20.0, 29.0);
    pub const PL_BOTTOM_LEFT: SpriteRect = SpriteRect::new(0.0, 72.0, 125.0, 38.0);
    pub const PL_BOTTOM_RIGHT: SpriteRect = SpriteRect::new(126.0, 72.0, 150.0, 38.0);
    pub const PL_CLOSE_NORMAL: SpriteRect = SpriteRect::new(52.0, 42.0, 9.0, 9.0);
    pub const PL_CLOSE_PRESSED: SpriteRect = SpriteRect::new(52.0, 42.0, 9.0, 9.0);

    // Volume bar in VOLUME.BMP: 28 frames, each 68x13, y = frame * 15; thumb is 14x11
    pub fn volume_frame(frame: usize) -> SpriteRect {
        let f = frame.min(27);
        SpriteRect::new(0.0, (f * 15) as f32, 68.0, 13.0)
    }
    pub const VOLUME_THUMB_NORMAL: SpriteRect = SpriteRect::new(15.0, 422.0, 14.0, 11.0);
    pub const VOLUME_THUMB_PRESSED: SpriteRect = SpriteRect::new(0.0, 422.0, 14.0, 11.0);

    // Balance bar in BALANCE.BMP: 28 frames, each 38x13 (or 68x13 in modern), thumb 14x11
    pub fn balance_frame(frame: usize) -> SpriteRect {
        let f = frame.min(27);
        SpriteRect::new(0.0, (f * 15) as f32, 38.0, 13.0)
    }
    pub const BALANCE_THUMB_NORMAL: SpriteRect = SpriteRect::new(15.0, 422.0, 14.0, 11.0);
    pub const BALANCE_THUMB_PRESSED: SpriteRect = SpriteRect::new(0.0, 422.0, 14.0, 11.0);

    // Seek bar / Posbar in POSBAR.BMP: Track is 248x10, Thumb is 29x10
    pub const POSBAR_TRACK: SpriteRect = SpriteRect::new(0.0, 0.0, 248.0, 10.0);
    pub const POSBAR_THUMB_NORMAL: SpriteRect = SpriteRect::new(248.0, 0.0, 29.0, 10.0);
    pub const POSBAR_THUMB_PRESSED: SpriteRect = SpriteRect::new(278.0, 0.0, 29.0, 10.0);

    // Font character rect in TEXT.BMP (5x6 pixels per char, 31 chars per row)
    pub fn char_rect(ch: char) -> SpriteRect {
        let (col, row) = match ch.to_ascii_uppercase() {
            'A'..='Z' => ((ch.to_ascii_uppercase() as u8 - b'A') as f32, 0.0),
            '\"' => (26.0, 0.0),
            '@' => (27.0, 0.0),
            '0'..='9' => ((ch as u8 - b'0') as f32, 1.0),
            '.' => (11.0, 1.0),
            ':' => (12.0, 1.0),
            '(' => (13.0, 1.0),
            ')' => (14.0, 1.0),
            '-' => (15.0, 1.0),
            '\'' => (16.0, 1.0),
            '!' => (17.0, 1.0),
            '_' => (18.0, 1.0),
            '+' => (19.0, 1.0),
            '\\' => (20.0, 1.0),
            '/' => (21.0, 1.0),
            '[' => (22.0, 1.0),
            ']' => (23.0, 1.0),
            '^' => (24.0, 1.0),
            '&' => (25.0, 1.0),
            '%' => (26.0, 1.0),
            ',' => (27.0, 1.0),
            '=' => (28.0, 1.0),
            '$' => (29.0, 1.0),
            '#' => (30.0, 1.0),
            ' ' => (30.0, 0.0), // blank space
            '?' => (3.0, 2.0),
            '*' => (4.0, 2.0),
            _ => (30.0, 0.0), // blank column
        };

        SpriteRect::new(col * 5.0, row * 6.0, 5.0, 6.0)
    }

    /// Selected font character on blue background (Row 3, 4, 5 in TEXT.BMP)
    pub fn char_rect_selected(ch: char) -> SpriteRect {
        let mut rect = Self::char_rect(ch);
        rect.y += 18.0; // 3 rows down
        rect
    }
}
