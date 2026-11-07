use std::path::Path;
use retroamp::audio::{AudioEngine, BiquadFilter, PlaybackState};
use retroamp::skin::Skin;

#[test]
fn test_embedded_skin_loads() {
    let skin = Skin::load_default();
    assert_eq!(skin.name, "Winamp (Classic)");
    assert_eq!(skin.main.size, [275, 116]);
    assert_eq!(skin.cbuttons.size, [136, 36]);
    assert_eq!(skin.titlebar.size, [344, 87]);
    assert_eq!(skin.numbers.size, [99, 13]);
    assert_eq!(skin.text.size, [155, 74]);
    assert_eq!(skin.volume.size, [68, 433]);
    assert_eq!(skin.eqmain.size, [275, 315]);
    assert_eq!(skin.pledit.size, [280, 186]);
    assert_eq!(skin.vis_colors.len(), 24);
}

#[test]
fn test_skin_loader_safety_and_validation() {
    // 1. Loading non-existent file must return Err
    assert!(Skin::load_wsz("non_existent_file.wsz").is_err());

    // 2. Loading custom matrix.wsz must succeed
    let wsz_path = Path::new("assets/skins/matrix.wsz");
    if wsz_path.exists() {
        let skin = Skin::load_wsz(wsz_path).expect("Failed to load matrix.wsz");
        assert_eq!(skin.name, "matrix");
        assert_eq!(skin.main.size, [275, 116]);
    }
}

#[test]
fn test_biquad_eq_dsp_processing() {
    let fs = 44100.0;
    let f0 = 1000.0;

    // Create a +6dB peaking filter at 1kHz
    let mut boost_filter = BiquadFilter::new_peaking(6.0, f0, fs, 1.414);
    // Create a -6dB peaking filter at 1kHz
    let mut cut_filter = BiquadFilter::new_peaking(-6.0, f0, fs, 1.414);

    let mut boost_energy = 0.0f32;
    let mut cut_energy = 0.0f32;

    // Generate 1000 samples of a 1kHz sine wave
    for i in 0..1000 {
        let t = i as f32 / fs;
        let x = (2.0 * std::f32::consts::PI * f0 * t).sin();

        let y_boost = boost_filter.process(x);
        let y_cut = cut_filter.process(x);

        // Discard initial transient (first 200 samples)
        if i >= 200 {
            boost_energy += y_boost * y_boost;
            cut_energy += y_cut * y_cut;
        }
    }

    // Boosted signal must have significantly more energy than cut signal (~4x power difference for 12dB spread)
    assert!(
        boost_energy > cut_energy * 2.5,
        "Boost energy ({boost_energy}) should be significantly greater than cut energy ({cut_energy})"
    );
}

#[test]
fn test_audio_playback_and_fft() {
    let mut audio = AudioEngine::new();
    let mp3_path = Path::new("/mnt/data/The WIRED CD - Rip. Sample. Mash. Share/01 - Beastie Boys - Now Get Busy.mp3");

    if mp3_path.exists() {
        audio.play_file(mp3_path).expect("Failed to play MP3");
        assert!(audio.is_playing());
        assert!(audio.current_track.is_some());

        let track = audio.current_track.as_ref().unwrap();
        assert_eq!(track.sample_rate, 44100);
        assert_eq!(track.channels, 2);
        assert!(track.duration.as_secs() > 0);

        // Let audio stream run for 400ms to verify FFT and visualization buffer
        std::thread::sleep(std::time::Duration::from_millis(400));
        let finished = audio.update();
        assert!(!finished, "Song should not finish immediately");

        let (_waveform, spectrum, _peaks) = audio.get_vis_data();
        let total_energy: f32 = spectrum.iter().sum();

        assert!(
            total_energy > 0.01,
            "FFT spectrum energy should be non-zero during playback"
        );

        // Test seeking
        audio.seek(std::time::Duration::from_secs(10));
        assert_eq!(audio.position.as_secs(), 10);

        audio.pause();
        assert_eq!(audio.state, PlaybackState::Paused);

        audio.resume();
        assert_eq!(audio.state, PlaybackState::Playing);

        audio.stop();
        assert_eq!(audio.state, PlaybackState::Stopped);
    }
}
