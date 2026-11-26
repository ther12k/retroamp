//! Winamp classic skin loader and sprite atlas.
//!
//! Supports classic 2.x skin assets:
//! - Default embedded assets
//! - .wsz / .zip skin archives
//! - Extracted skin directories
//! - viscolor.txt color palette

use egui::{Color32, ColorImage, Rect, Pos2, Vec2};
use std::collections::HashMap;
use std::io::{Cursor, Read, Seek};
use std::path::Path;

const MAX_ARCHIVE_BYTES: u64 = 64 * 1024 * 1024;
const MAX_ARCHIVE_ENTRIES: usize = 128;
const MAX_ENTRY_BYTES: u64 = 8 * 1024 * 1024;
const MAX_TOTAL_ENTRY_BYTES: u64 = 32 * 1024 * 1024;
const MAX_IMAGE_DIMENSION: u32 = 2048;
const MAX_IMAGE_DECODE_BYTES: u64 = 64 * 1024 * 1024;

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
const RAW_NORD_FROST: &[u8] = include_bytes!("../assets/skins/nord_frost.wsz");

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

    /// Load a skin from a .wsz or .zip archive file with bounded reads.
    pub fn load_wsz<P: AsRef<Path>>(path: P) -> Result<Self, String> {
        let path_ref = path.as_ref();
        let name = path_ref
            .file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| "Custom Skin".to_string());

        let file = std::fs::File::open(path_ref).map_err(|error| error.to_string())?;
        if file
            .metadata()
            .map_err(|error| error.to_string())?
            .len()
            > MAX_ARCHIVE_BYTES
        {
            return Err("Skin archive exceeds the size limit".to_string());
        }
        Self::load_archive(name, file)
    }

    /// Load the bundled Nord Frost skin from the executable's embedded bytes.
    pub fn load_nord_frost() -> Result<Self, String> {
        Self::load_archive("nord_frost".to_string(), Cursor::new(RAW_NORD_FROST))
    }

    fn load_archive<R: Read + Seek>(name: String, reader: R) -> Result<Self, String> {
        const KNOWN_ASSETS: &[&str] = &[
            "MAIN.BMP",
            "CBUTTONS.BMP",
            "TITLEBAR.BMP",
            "NUMBERS.BMP",
            "TEXT.BMP",
            "VOLUME.BMP",
            "BALANCE.BMP",
            "PANBAR.BMP",
            "POSBAR.BMP",
            "PLAYPAUS.BMP",
            "SHUFREP.BMP",
            "MONOSTER.BMP",
            "EQMAIN.BMP",
            "EQ_EX.BMP",
            "PLEDIT.BMP",
            "VISCOLOR.TXT",
        ];

        let mut archive = zip::ZipArchive::new(reader).map_err(|error| error.to_string())?;
        if archive.len() > MAX_ARCHIVE_ENTRIES {
            return Err("Skin archive contains too many entries".to_string());
        }

        let mut files: HashMap<String, Vec<u8>> = HashMap::new();
        let mut total_declared = 0u64;
        let mut total_read = 0u64;
        for index in 0..archive.len() {
            // zip 2.6.1 exposes the final entry for a duplicate exact name at a
            // single index, so inspect names by index and open only recognized assets.
            let filename = archive
                .name_for_index(index)
                .ok_or_else(|| format!("Invalid skin archive entry {index}"))?
                .to_ascii_uppercase()
                .replace('\\', "/");
            if filename.ends_with('/') {
                continue;
            }

            let base_name = filename.rsplit('/').next().unwrap_or(&filename).to_string();
            if !KNOWN_ASSETS.contains(&base_name.as_str()) {
                continue;
            }
            let essential = matches!(
                base_name.as_str(),
                "MAIN.BMP" | "CBUTTONS.BMP" | "TITLEBAR.BMP" | "EQMAIN.BMP" | "PLEDIT.BMP"
            );

            // Read metadata without decoding first so size limits still apply to
            // recognized files even when their compression method is unsupported.
            let raw_entry = match archive.by_index_raw(index) {
                Ok(entry) => entry,
                Err(error) if essential => {
                    return Err(format!("Could not inspect skin asset {base_name}: {error}"));
                }
                Err(_) => continue,
            };
            if !raw_entry.is_file() {
                continue;
            }
            let declared_size = raw_entry.size();
            drop(raw_entry);

            if declared_size > MAX_ENTRY_BYTES {
                return Err(format!("Skin asset {base_name} exceeds the size limit"));
            }
            total_declared = total_declared
                .checked_add(declared_size)
                .ok_or_else(|| "Skin archive size overflow".to_string())?;
            if total_declared > MAX_TOTAL_ENTRY_BYTES {
                return Err("Skin archive exceeds the uncompressed size limit".to_string());
            }

            let mut entry = match archive.by_index(index) {
                Ok(entry) => entry,
                Err(error) if essential => {
                    return Err(format!("Could not read skin asset {base_name}: {error}"));
                }
                Err(_) => continue,
            };
            let mut data = Vec::with_capacity((declared_size as usize).min(1024 * 1024));
            let read_result = entry
                .by_ref()
                .take(MAX_ENTRY_BYTES + 1)
                .read_to_end(&mut data);
            let actual_size = data.len() as u64;
            total_read = total_read
                .checked_add(actual_size)
                .ok_or_else(|| "Skin archive size overflow".to_string())?;
            if total_read > MAX_TOTAL_ENTRY_BYTES {
                return Err("Skin archive exceeds the uncompressed size limit".to_string());
            }
            if let Err(error) = read_result {
                if essential {
                    return Err(format!("Could not read skin asset {base_name}: {error}"));
                }
                continue;
            }
            if actual_size != declared_size || actual_size > MAX_ENTRY_BYTES {
                if essential {
                    return Err(format!("Invalid size for skin asset {base_name}"));
                }
                continue;
            }
            if files.insert(base_name.clone(), data).is_some() {
                return Err(format!("Skin archive contains duplicate asset {base_name}"));
            }
        }

        let mut skin = Self::load_default();
        skin.name = name;
        let mut has_decoded_essential = false;
        apply_image(&files, "MAIN.BMP", &mut skin.main, true, &mut has_decoded_essential)?;
        apply_image(
            &files,
            "CBUTTONS.BMP",
            &mut skin.cbuttons,
            true,
            &mut has_decoded_essential,
        )?;
        apply_image(
            &files,
            "TITLEBAR.BMP",
            &mut skin.titlebar,
            true,
            &mut has_decoded_essential,
        )?;
        apply_image(&files, "NUMBERS.BMP", &mut skin.numbers, false, &mut has_decoded_essential)?;
        apply_image(&files, "TEXT.BMP", &mut skin.text, false, &mut has_decoded_essential)?;
        apply_image(&files, "VOLUME.BMP", &mut skin.volume, false, &mut has_decoded_essential)?;
        let balance_asset = if files.contains_key("BALANCE.BMP") {
            "BALANCE.BMP"
        } else {
            "PANBAR.BMP"
        };
        apply_image(&files, balance_asset, &mut skin.balance, false, &mut has_decoded_essential)?;
        apply_image(&files, "POSBAR.BMP", &mut skin.posbar, false, &mut has_decoded_essential)?;
        apply_image(&files, "PLAYPAUS.BMP", &mut skin.playpaus, false, &mut has_decoded_essential)?;
        apply_image(&files, "SHUFREP.BMP", &mut skin.shufrep, false, &mut has_decoded_essential)?;
        apply_image(&files, "MONOSTER.BMP", &mut skin.monoster, false, &mut has_decoded_essential)?;
        apply_image(&files, "EQMAIN.BMP", &mut skin.eqmain, true, &mut has_decoded_essential)?;
        apply_image(&files, "EQ_EX.BMP", &mut skin.eq_ex, false, &mut has_decoded_essential)?;
        apply_image(&files, "PLEDIT.BMP", &mut skin.pledit, true, &mut has_decoded_essential)?;

        if !has_decoded_essential {
            return Err("Archive does not contain decodable classic Winamp skin bitmaps".to_string());
        }
        if let Some(data) = files.get("VISCOLOR.TXT") {
            if let Ok(vis_text) = std::str::from_utf8(data) {
                skin.vis_colors = parse_viscolor(vis_text);
            }
        }

        Ok(skin)
    }
}

fn bounded_image_limits() -> image::Limits {
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(MAX_IMAGE_DIMENSION);
    limits.max_image_height = Some(MAX_IMAGE_DIMENSION);
    limits.max_alloc = Some(MAX_IMAGE_DECODE_BYTES);
    limits
}

fn apply_image(
    files: &HashMap<String, Vec<u8>>,
    asset: &str,
    target: &mut ColorImage,
    essential: bool,
    has_decoded_essential: &mut bool,
) -> Result<(), String> {
    if let Some(data) = files.get(asset) {
        if let Some(image) = decode_image(data) {
            *target = image;
            if essential {
                *has_decoded_essential = true;
            }
        } else if essential {
            return Err(format!("Could not decode skin asset {asset}"));
        }
    }
    Ok(())
}

/// Helper function to decode bounded BMP or PNG data.
pub fn decode_image(data: &[u8]) -> Option<ColorImage> {
    if data.is_empty() || data.len() as u64 > MAX_ENTRY_BYTES {
        return None;
    }

    let mut header_reader = image::ImageReader::new(Cursor::new(data))
        .with_guessed_format()
        .ok()?;
    header_reader.limits(bounded_image_limits());
    let (header_w, header_h) = header_reader.into_dimensions().ok()?;
    if header_w == 0 || header_h == 0 || header_w > MAX_IMAGE_DIMENSION || header_h > MAX_IMAGE_DIMENSION {
        return None;
    }

    let mut reader = image::ImageReader::new(Cursor::new(data))
        .with_guessed_format()
        .ok()?;
    reader.limits(bounded_image_limits());
    let img = reader.decode().ok()?;
    if img.width() != header_w || img.height() != header_h {
        return None;
    }

    let rgba = img.to_rgba8();
    let size = [rgba.width() as usize, rgba.height() as usize];
    let pixels = rgba.into_raw();
    Some(ColorImage::from_rgba_unmultiplied(size, &pixels))
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

#[cfg(test)]
mod tests {
    use super::Skin;
    use std::io::{Cursor, Write};

    fn make_archive(entries: &[(&str, &[u8])]) -> Vec<u8> {
        let mut archive = zip::ZipWriter::new(Cursor::new(Vec::new()));
        let options = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Stored);
        for (name, data) in entries {
            archive.start_file(*name, options).unwrap();
            archive.write_all(data).unwrap();
        }
        archive.finish().unwrap().into_inner()
    }

    fn patch_compression_method(bytes: &mut [u8], name: &[u8], method: u16) {
        let mut local_header_found = false;
        for offset in 0..bytes.len().saturating_sub(3) {
            if bytes.get(offset..offset + 4) != Some(b"PK\x03\x04") || offset + 30 > bytes.len() {
                continue;
            }
            let name_len = u16::from_le_bytes([bytes[offset + 26], bytes[offset + 27]]) as usize;
            let name_start = offset + 30;
            if bytes.get(name_start..name_start + name_len) == Some(name) {
                bytes[offset + 8..offset + 10].copy_from_slice(&method.to_le_bytes());
                local_header_found = true;
            }
        }

        let mut central_header_found = false;
        for offset in 0..bytes.len().saturating_sub(3) {
            if bytes.get(offset..offset + 4) != Some(b"PK\x01\x02") || offset + 46 > bytes.len() {
                continue;
            }
            let name_len = u16::from_le_bytes([bytes[offset + 28], bytes[offset + 29]]) as usize;
            let name_start = offset + 46;
            if bytes.get(name_start..name_start + name_len) == Some(name) {
                bytes[offset + 10..offset + 12].copy_from_slice(&method.to_le_bytes());
                central_header_found = true;
            }
        }
        assert!(local_header_found && central_header_found);
    }

    fn tiny_png() -> Vec<u8> {
        let image = image::GrayImage::new(2, 2);
        let mut png = Cursor::new(Vec::new());
        image.write_to(&mut png, image::ImageFormat::Png).unwrap();
        png.into_inner()
    }

    #[test]
    fn skips_unknown_entry_with_unsupported_compression() {
        let png = tiny_png();
        let mut bytes = make_archive(&[("MAIN.BMP", &png), ("README.txt", b"skin notes")]);
        // Method 12 is BZIP2, which is not enabled for this crate.
        patch_compression_method(&mut bytes, b"README.txt", 12);

        let skin = Skin::load_archive("test".to_string(), Cursor::new(bytes)).unwrap();
        assert_eq!(skin.main.size, [2, 2]);
    }

    #[test]
    fn invalid_optional_sprites_keep_embedded_fallbacks() {
        let png = tiny_png();
        let mut bytes = make_archive(&[
            ("MAIN.BMP", &png),
            ("VOLUME.BMP", &png),
            ("NUMBERS.BMP", b"not an image"),
        ]);
        patch_compression_method(&mut bytes, b"VOLUME.BMP", 12);

        let skin = Skin::load_archive("test".to_string(), Cursor::new(bytes)).unwrap();
        let default = Skin::load_default();
        assert_eq!(skin.volume, default.volume);
        assert_eq!(skin.numbers, default.numbers);
    }

    #[test]
    fn optional_sprites_do_not_replace_the_essential_bitmap_requirement() {
        let bytes = make_archive(&[("VOLUME.BMP", b"not an image")]);
        assert!(Skin::load_archive("test".to_string(), Cursor::new(bytes)).is_err());
    }
}
