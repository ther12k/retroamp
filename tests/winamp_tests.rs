#[cfg(feature = "egui-ui")]
use std::path::Path;
use retroamp::audio::{AudioEngine, BiquadFilter, PlaybackState};
#[cfg(feature = "egui-ui")]
use retroamp::skin::Skin;

#[cfg(feature = "egui-ui")]
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

#[cfg(feature = "egui-ui")]
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
#[ignore = "requires audio output and a RETROAMP_TEST_AUDIO fixture with an audible opening"]
fn test_audio_playback_and_fft() {
    use std::time::{Duration, Instant};

    let path = std::env::var_os("RETROAMP_TEST_AUDIO").expect("set RETROAMP_TEST_AUDIO");
    let mut audio = AudioEngine::new();
    audio.play_file(std::path::PathBuf::from(path)).expect("Failed to play fixture");
    assert!(audio.is_playing());

    let track = audio.current_track.as_ref().unwrap();
    assert!(track.sample_rate > 0 && track.channels > 0 && !track.duration.is_zero());
    let seek_to = track.duration / 2;

    // Output backends need not deliver the first decoded buffer at the same latency.
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        std::thread::sleep(Duration::from_millis(40));
        assert!(!audio.update(), "Fixture ended before producing FFT data");
        if audio.get_vis_data().1.iter().sum::<f32>() > 0.01 {
            break;
        }
        assert!(Instant::now() < deadline, "No FFT energy within five seconds");
    }

    audio.seek(seek_to);
    assert_eq!(audio.position, seek_to);
    audio.pause();
    assert_eq!(audio.state, PlaybackState::Paused);
    audio.resume();
    assert_eq!(audio.state, PlaybackState::Playing);
    audio.stop();
    assert_eq!(audio.state, PlaybackState::Stopped);
}
