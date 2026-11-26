# Building RetroAmp on Linux

On Ubuntu 24.04 (the CI runner), install a C toolchain, `pkg-config`, and the ALSA development package:

```sh
sudo apt-get update
sudo apt-get install --no-install-recommends build-essential pkg-config libasound2-dev
```

Install Rust with `rustup`. CI pins Rust 1.89.0 and uses the committed `Cargo.lock`. From the repository root:

```sh
rustup toolchain install 1.89.0 --profile minimal
cargo +1.89.0 test --locked
cargo +1.89.0 build --release --locked
```

The default tests run without a display or audio device. The playback smoke test is ignored by default; on a machine with working audio, point it at a local audio file with audible content in its first second:

```sh
RETROAMP_TEST_AUDIO=/absolute/path/to/audio cargo test --locked test_audio_playback_and_fft -- --ignored
```
