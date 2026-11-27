# RetroAmp (Linux Edition)

<p align="center">
  <img src="assets/icons/retroamp-256.png" width="128" height="128" alt="RetroAmp Icon">
</p>

> A modern, modular retro multimedia player for Linux built with **Rust**, inspired by classic Winamp and engineered using clean-room reverse-engineering workflows guided by [storytold/craft-anything](https://github.com/storytold/craft-anything).

---

## ⚡ Highlights & Features

- **Iconic Modular Architecture**: Preserves the exact 3-piece snapped layout: **Main Player**, **10-Band Graphic Equalizer**, and **Playlist Editor**.
- **Alternative UI/UX Themes**:
  - ⭐ **Winamp (Classic)**: The authentic 1998 Nullsoft retro experience.
  - 🟢 **Cyberpunk Terminal**: Neon phosphor green & cyan CRT glow for Linux hacker workstations.
  - 📻 **Hi-Fi Studio**: Audiophile brushed aluminum with amber vacuum-fluorescent display.
  - ❄️ **Nord Frost**: Slate polar blue and pastel ice tones that blend with modern Linux desktop environments.
  - 🎨 **Skin Selector Drawer**: Press `S` to switch themes instantly, or drop any custom `.wsz` file onto the window. A skin that cannot be loaded is reported in a notice; nothing is applied silently.
- **Classic Main Menu** (`☰` in the titlebar, or click the logo in Classic theme): File / Playback (with Shuffle & Repeat) / View (EQ, Playlist, Double Size, Zoom) / Themes / Help — with an About dialog.
- **Clean-Room Implementation in Pure Rust**: Built with safe Rust using `egui`/`eframe` with hardware-accelerated immediate-mode GUI rendering.
- **Native Borderless & Transparent Chrome**:
  - No OS window titlebars: the skins *are* the chrome.
  - Real window manager dragging via native `ViewportCommand::StartDrag`.
  - Transparent see-through background between and around floating windows.
- **Linux Best-Practice Audio Engine**:
  - High-performance decoding via `symphonia` for **MP3, FLAC, WAV, OGG Vorbis, and AAC** (handles all sample bit depths: 8/16/24/32-bit and float).
  - Output powered by ALSA / PulseAudio / PipeWire via `rodio`/`cpal`.
  - **Real 10-band biquad IIR Equalizer** (60 Hz–16 kHz peaking filters, ±12 dB + preamp gain) processing live samples.
  - Real-time 512-point FFT spectrum analyzer (19 logarithmic frequency bands with peak falloff) and oscilloscope with true mono downmix.
  - True stream seeking, real ID3 title/artist tags, and EOF-driven playlist advancement with Shuffle and Repeat.
- **Full Customizability**:
  - **Playlist resizing**: Drag the bottom edge to grow/shrink track row capacity.
  - **Winamp-style scaling** — `Ctrl+D` toggles double size, `Ctrl+=`/`Ctrl+-` fine-zoom, `Ctrl+0` resets, or pick 100–300% from the menu's VIEW section. Launch big with `RETROAMP_SCALE=2 retroamp`.
- **Persistent configuration** (`~/.config/retroamp/config.json`): UI scale, theme, playlist size (rows), volume, shuffle/repeat, EQ/Playlist visibility, visualizer mode, and the active classic skin (bundled Nord Frost or a custom skin path) are remembered across launches and saved atomically.
- **Smooth native-feel interactions**: playlist bottom-edge resize uses relative drag accumulation (no cursor rubber-banding), and the main titlebar drag is a window-manager move.

---

## ⌨️ Keyboard Shortcuts

| Key | Action |
|-----|--------|
| `Z` | Previous track |
| `X` | Play |
| `C` | Pause / Unpause |
| `V` | Stop |
| `B` | Next track |
| `L` | Open audio file or `.wsz` skin |
| `S` | **Open Skin / Theme Selector** |
| EQ → PRESETS | Apply classic Winamp EQ presets (Rock, Pop, Classical, Dance, Full Bass…) |
| `☰` / titlebar logo | **Main Menu** (File · Playback · View · Scale · Themes · Help) — themed to match the skin |
| `F10` | Open / close the main menu |
| `Up` / `Down` | Volume up / down |
| `Ctrl+D` | Toggle Double Size (2.0× scale) |
| `Ctrl+=` / `Ctrl+-` | Zoom in / out (1.0×–3.0×) |
| `Ctrl+0` | Reset zoom to 1.0× |

All shortcuts except `F10` are ignored while the main menu, the EQ preset list, the skin picker or the About dialog is open; close it first (`Esc`).

### Desktop requirements

Opening files and skins goes through the XDG desktop portal: install `xdg-desktop-portal` and a frontend such as `xdg-desktop-portal-gtk` or `xdg-desktop-portal-gnome`. On a desktop without it, the Open dialog cannot appear; audio files can still be dragged onto the window. A track or skin that cannot be read is reported in a notice at the bottom of the window.

---

## GPUI experiment (Linux)

This branch has two frontends. A normal `cargo run --locked` still starts the egui player. To try the GPUI player instead, from this checkout:

```bash
cargo run --locked --no-default-features --features gpui-ui --bin retroamp-gpui
# Or start with a playlist:
cargo run --locked --no-default-features --features gpui-ui --bin retroamp-gpui -- track1.flac track2.mp3
```

Use a Linux desktop with a working Vulkan presentation surface, audio output, and XDG desktop portal for the Open dialog (`xdg-desktop-portal` plus a frontend such as `xdg-desktop-portal-gtk` or `xdg-desktop-portal-gnome`). Keep `--locked`: GPUI 0.2.2 pulls in `xattr` 0.2.3, which uses a Linux constant removed in libc 0.2.190, so the manifest pins `libc` to `=0.2.189` whenever `gpui-ui` is enabled and the lockfile holds `mio` at 1.2.4. A dependency update that cannot satisfy the pin stops with a resolution error instead of producing a lockfile that fails to compile only under this feature.

**Open…** (`L` or `Ctrl+O`) replaces the queue with the selected audio files. Without a desktop portal, the player reports that the file picker is unavailable; tracks can then only be passed as command-line arguments. Click a row to play it, or use Previous / Play / Pause / Stop / Next. `Z` / `X` / `C` / `V` / `B` control previous / play / pause-resume / stop / next; Up/Down adjusts volume. Tab/Shift+Tab moves focus, and Enter or Space activates a focused control. Track loading runs off the UI thread. Choosing another track or pressing Stop marks a pending load for cancellation; the token is checked between packet reads/decodes and cannot interrupt an operation already in progress. At EOF, unplayable tracks are skipped until a playable one is found. The GPUI target uses the same `AudioEngine` as the egui target, draws its own controls, and does not compile RetroAmp's bitmap skin loader. It does not yet implement the skinned windows, EQ editor, or saved preferences. The classic assets remain in this repository; binary distribution remains paused.

The GPUI view renders only the visible playlist rows and polls the audio engine about 30 times per second during playback, not at idle. In a local X11 idle check on Intel RPL-P graphics, the updated egui build used a median 0.0% of one logical core and 89.9 MiB RSS; GPUI used 1.2% and 138.2 MiB. These are medians of three ten-second samples after a five-second warm-up: debug builds with the same private silent audio sink and empty queues; egui used Redrawn at 1.5× with EQ hidden. The windows and backends differ. This measures idle process cost, **not** toolkit drawing speed. Compare release builds with the same track, controls, and update rate before attributing a difference to the toolkit.

An end-to-end audio check (tones in queue order, broken entry skipped) lives in `scripts/gpui-smoke.py`; see `scripts/README.md`.

## 🚀 Running RetroAmp

### From App Launcher
Search for **RetroAmp** in your desktop application launcher / GNOME Shell.

### From Terminal
```bash
retroamp
```
With tracks or a music folder (the first track starts, the rest queue; `.wsz` files load as skins):
```bash
retroamp track.flac ~/music
```
Or with an initial theme:
```bash
RETROAMP_SKIN=cyberpunk retroamp
RETROAMP_SKIN=hifi retroamp
RETROAMP_SKIN=nord retroamp
```
With scaling:
```bash
RETROAMP_SCALE=2 retroamp
```

### Build & Run from Source
```bash
cargo run --release
cargo test
```

See [docs/BUILD.md](docs/BUILD.md) for build prerequisites.

## Artwork and redistribution status

The MIT license in [LICENSE](LICENSE) is intended to cover the RetroAmp software source only. It does not grant rights to the original Winamp installer or its installer artwork. The embedded classic skin artwork, including MAIN, EQMAIN, and PLEDIT, came from the original Winamp installer; permission to redistribute it has not been verified. The provenance and redistribution permissions of the other bundled skins, icons, and artwork have also not been verified. The repository remains public, but do not distribute binaries containing these assets until the applicable rights are verified or the assets are replaced with properly licensed artwork. This notice is not clearance or a license.
