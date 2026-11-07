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
  - 🎨 **Skin Selector Drawer**: Press `S` anytime to switch themes instantly, or drop any custom `.wsz` file onto the window.
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
- **Persistent configuration** (`~/.config/retroamp/config.json`): UI scale, theme, playlist size (rows), volume, shuffle/repeat, EQ/Playlist visibility, and visualizer mode are remembered across launches — changed automatically, saved on exit.
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
| `Up` / `Down` | Volume up / down |
| `Ctrl+D` | Toggle Double Size (2.0× scale) |
| `Ctrl+=` / `Ctrl+-` | Zoom in / out (1.0×–3.0×) |
| `Ctrl+0` | Reset zoom to 1.0× |

---

## 🚀 Running RetroAmp

### From App Launcher
Search for **RetroAmp** in your desktop application launcher / GNOME Shell.

### From Terminal
```bash
retroamp
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
