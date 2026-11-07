pub mod skin;
pub mod audio;
pub mod config;
pub mod ui;

use eframe::NativeOptions;
use ui::WinampApp;

const ICON_RAW: &[u8] = include_bytes!("../assets/icons/retroamp-48.png");

/// Decode the embedded icon so the taskbar/alt-tab shows the RetroAmp logo
/// even before the desktop entry is matched.
fn load_window_icon() -> Option<egui::IconData> {
    let img = image::load_from_memory(ICON_RAW).ok()?.into_rgba8();
    let (w, h) = (img.width(), img.height());
    Some(egui::IconData {
        rgba: img.into_raw(),
        width: w,
        height: h,
    })
}

fn main() -> eframe::Result<()> {
    env_logger::init();

    let mut viewport = egui::ViewportBuilder::default()
        .with_title("RetroAmp")
        .with_app_id("retroamp")
        .with_inner_size(egui::vec2(295.0, 368.0))
        .with_min_inner_size(egui::vec2(275.0, 136.0))
        .with_resizable(false)
        // Borderless: the skin is the window chrome, and gaps are see-through
        .with_decorations(false)
        .with_transparent(true);

    if let Some(icon) = load_window_icon() {
        viewport = viewport.with_icon(icon);
    }

    let options = NativeOptions {
        viewport,
        ..Default::default()
    };

    eframe::run_native(
        "RetroAmp",
        options,
        Box::new(|cc| Ok(Box::new(WinampApp::new(cc)))),
    )
}
