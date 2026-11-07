//! Audio playback engine using Symphonia decoding and Rodio output,
//! with real-time waveform capture, 10-band biquad IIR Equalizer,
//! balance panning, accurate stream seeking, and FFT spectrum analysis.

use parking_lot::Mutex;
use rodio::{DeviceSinkBuilder, MixerDeviceSink, Player, Source};
use rustfft::{FftPlanner, num_complex::Complex};
use std::fs::File;
use std::num::{NonZeroU16, NonZeroU32};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use symphonia::core::audio::SampleBuffer;
use symphonia::core::codecs::{DecoderOptions, CODEC_TYPE_NULL};
use symphonia::core::errors::Error as SymphoniaError;
use symphonia::core::formats::{FormatOptions, SeekMode, SeekTo};
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::{MetadataOptions, StandardTagKey};
use symphonia::core::probe::Hint;
use symphonia::core::units::Time;

pub const SPECTRUM_BARS: usize = 19;
pub const WAVEFORM_SAMPLES: usize = 75;

// Winamp 10 standard EQ center frequencies (Hz)
pub const EQ_FREQUENCIES: [f32; 10] = [
    60.0, 170.0, 310.0, 600.0, 1000.0, 3000.0, 6000.0, 12000.0, 14000.0, 16000.0,
];

/// Classic Winamp built-in EQ presets (gains in dB for the 10 bands above).
pub const EQ_PRESETS: &[(&str, [f32; 10])] = &[
    ("Flat", [0.0; 10]),
    ("Classical", [0.0, 0.0, 0.0, 0.0, 0.0, 0.0, -7.0, -7.0, -7.0, -9.0]),
    ("Club", [0.0, 0.0, 8.0, 6.0, 5.0, 3.0, 0.0, 0.0, 3.0, 6.0]),
    ("Dance", [9.0, 7.0, 2.0, 0.0, 0.0, -5.0, -7.0, -7.0, 0.0, 0.0]),
    ("Full Bass", [-8.0, 9.0, 9.0, 5.0, 1.0, -4.0, -8.0, -10.0, -11.0, -11.0]),
    ("Full Bass & Treble", [7.0, 5.0, 0.0, -7.0, -5.0, 1.0, 8.0, 11.0, 12.0, 12.0]),
    ("Full Treble", [-9.0, -9.0, -9.0, -4.0, 2.0, 11.0, 16.0, 16.0, 16.0, 17.0]),
    ("Laptop / Headphones", [4.0, 11.0, 5.0, -3.0, -2.0, 1.0, 4.0, 9.0, 12.0, 14.0]),
    ("Large Hall", [10.0, 10.0, 5.0, 5.0, 0.0, -4.0, -6.0, -6.0, -4.0, -2.0]),
    ("Party", [7.0, 7.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 7.0, 7.0]),
    ("Pop", [-1.0, 4.0, 7.0, 8.0, 5.0, 0.0, -2.0, -2.0, -1.0, -1.0]),
    ("Reggae", [0.0, 0.0, 0.0, -5.0, 0.0, 6.0, 6.0, 0.0, 0.0, 0.0]),
    ("Rock", [8.0, 4.0, -5.0, -8.0, -3.0, 4.0, 8.0, 11.0, 11.0, 11.0]),
    ("Ska", [-2.0, -4.0, -4.0, 0.0, 4.0, 5.0, 8.0, 9.0, 11.0, 9.0]),
    ("Soft", [4.0, 1.0, 0.0, -2.0, 0.0, 4.0, 8.0, 9.0, 11.0, 12.0]),
    ("Soft Rock", [4.0, 4.0, 2.0, 0.0, -4.0, -5.0, -3.0, 0.0, 2.0, 8.0]),
    ("Techno", [8.0, 5.0, -5.0, -7.0, 0.0, 8.0, 9.0, 9.0, 8.0, 9.0]),
];

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PlaybackState {
    Stopped,
    Playing,
    Paused,
}

#[derive(Clone, Debug)]
pub struct TrackMetadata {
    pub title: String,
    pub artist: String,
    pub path: PathBuf,
    pub duration: Duration,
    pub sample_rate: u32,
    pub channels: u16,
    pub bitrate_kbps: u32,
}

/// Shared visualization data passed from audio thread to GUI thread
pub struct VisData {
    pub waveform: [f32; WAVEFORM_SAMPLES],
    pub spectrum: [f32; SPECTRUM_BARS],
    pub spectrum_peaks: [f32; SPECTRUM_BARS],
}

impl Default for VisData {
    fn default() -> Self {
        Self {
            waveform: [0.0; WAVEFORM_SAMPLES],
            spectrum: [0.0; SPECTRUM_BARS],
            spectrum_peaks: [0.0; SPECTRUM_BARS],
        }
    }
}

/// 10-band Equalizer settings (-12.0dB to +12.0dB)
#[derive(Clone, Copy, Debug)]
pub struct EqSettings {
    pub enabled: bool,
    pub preamp: f32, // -12 to +12 dB
    pub bands: [f32; 10], // 60, 170, 310, 600, 1k, 3k, 6k, 12k, 14k, 16k
}

impl Default for EqSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            preamp: 0.0,
            bands: [0.0; 10],
        }
    }
}

/// Biquad Peaking EQ Filter for DSP
#[derive(Clone, Copy, Default)]
pub struct BiquadFilter {
    b0: f32,
    b1: f32,
    b2: f32,
    a1: f32,
    a2: f32,
    x1: f32,
    x2: f32,
    y1: f32,
    y2: f32,
}

impl BiquadFilter {
    pub fn new_peaking(gain_db: f32, f0: f32, fs: f32, q: f32) -> Self {
        if gain_db.abs() < 0.01 {
            return Self {
                b0: 1.0, b1: 0.0, b2: 0.0,
                a1: 0.0, a2: 0.0,
                x1: 0.0, x2: 0.0,
                y1: 0.0, y2: 0.0,
            };
        }

        let a = 10.0f32.powf(gain_db / 40.0);
        let w0 = 2.0 * std::f32::consts::PI * (f0 / fs).clamp(0.0001, 0.499);
        let alpha = w0.sin() / (2.0 * q);

        let b0 = 1.0 + alpha * a;
        let b1 = -2.0 * w0.cos();
        let b2 = 1.0 - alpha * a;
        let a0 = 1.0 + alpha / a;
        let a1 = -2.0 * w0.cos();
        let a2 = 1.0 - alpha / a;

        Self {
            b0: b0 / a0,
            b1: b1 / a0,
            b2: b2 / a0,
            a1: a1 / a0,
            a2: a2 / a0,
            x1: 0.0,
            x2: 0.0,
            y1: 0.0,
            y2: 0.0,
        }
    }

    #[inline]
    pub fn process(&mut self, x: f32) -> f32 {
        let y = self.b0 * x + self.b1 * self.x1 + self.b2 * self.x2 - self.a1 * self.y1 - self.a2 * self.y2;
        self.x2 = self.x1;
        self.x1 = x;
        self.y2 = self.y1;
        self.y1 = y;
        y
    }
}

pub struct AudioEngine {
    sink_handle: Option<MixerDeviceSink>,
    player: Option<Player>,
    pub state: PlaybackState,
    pub current_track: Option<TrackMetadata>,
    pub position: Duration,
    pub volume: f32,
    pub balance: f32,
    pub eq: EqSettings,
    shared_params: Arc<Mutex<AudioParams>>,
    vis_data: Arc<Mutex<VisData>>,
    mono_vis_buffer: Arc<Mutex<Vec<f32>>>,
    fft_planner: Arc<Mutex<FftPlanner<f32>>>,
    last_position_update: std::time::Instant,
    is_finished: Arc<AtomicBool>,
}

#[derive(Clone)]
pub struct AudioParams {
    pub volume: f32,
    pub balance: f32,
    pub eq: EqSettings,
    pub seek_request: Option<Duration>,
}

impl AudioEngine {
    pub fn new() -> Self {
        let sink_handle = match DeviceSinkBuilder::open_default_sink() {
            Ok(handle) => Some(handle),
            Err(e) => {
                log::warn!("Failed to open default sound device: {e}");
                None
            }
        };

        let shared_params = Arc::new(Mutex::new(AudioParams {
            volume: 0.8,
            balance: 0.0,
            eq: EqSettings::default(),
            seek_request: None,
        }));

        Self {
            sink_handle,
            player: None,
            state: PlaybackState::Stopped,
            current_track: None,
            position: Duration::ZERO,
            volume: 0.8,
            balance: 0.0,
            eq: EqSettings::default(),
            shared_params,
            vis_data: Arc::new(Mutex::new(VisData::default())),
            mono_vis_buffer: Arc::new(Mutex::new(Vec::with_capacity(2048))),
            fft_planner: Arc::new(Mutex::new(FftPlanner::new())),
            last_position_update: std::time::Instant::now(),
            is_finished: Arc::new(AtomicBool::new(false)),
        }
    }

    pub fn play_file<P: AsRef<Path>>(&mut self, path: P) -> Result<(), String> {
        let path_buf = path.as_ref().to_path_buf();
        self.stop();

        if self.sink_handle.is_none() {
            if let Ok(handle) = DeviceSinkBuilder::open_default_sink() {
                self.sink_handle = Some(handle);
            }
        }

        let sink_handle = self.sink_handle.as_ref().ok_or("No audio output device available")?;

        // Read metadata using Symphonia
        let metadata = extract_metadata(&path_buf)?;

        // Open audio file for decoding
        let file = File::open(&path_buf).map_err(|e| e.to_string())?;
        let mss = MediaSourceStream::new(Box::new(file), Default::default());

        let mut hint = Hint::new();
        if let Some(ext) = path_buf.extension().and_then(|e| e.to_str()) {
            hint.with_extension(ext);
        }

        let meta_opts: MetadataOptions = Default::default();
        let fmt_opts: FormatOptions = Default::default();
        let probed = symphonia::default::get_probe()
            .format(&hint, mss, &fmt_opts, &meta_opts)
            .map_err(|e| format!("Audio format probe error: {e}"))?;

        let format = probed.format;
        let track = format
            .tracks()
            .iter()
            .find(|t| t.codec_params.codec != CODEC_TYPE_NULL)
            .ok_or("No audio track found")?;

        let track_id = track.id;
        let dec_opts: DecoderOptions = Default::default();
        let decoder = symphonia::default::get_codecs()
            .make(&track.codec_params, &dec_opts)
            .map_err(|e| format!("Codec initialization error: {e}"))?;

        let sample_rate = track.codec_params.sample_rate.unwrap_or(44100);
        let channels = track.codec_params.channels.map(|c| c.count() as u16).unwrap_or(2);

        self.is_finished.store(false, Ordering::SeqCst);

        // Update shared params
        {
            let mut p = self.shared_params.lock();
            p.volume = self.volume;
            p.balance = self.balance;
            p.eq = self.eq;
            p.seek_request = None;
        }

        let audio_source = SymphoniaSource::new(
            format,
            decoder,
            track_id,
            sample_rate,
            channels,
            self.shared_params.clone(),
            self.mono_vis_buffer.clone(),
            self.is_finished.clone(),
        );

        let player = Player::connect_new(sink_handle.mixer());
        player.set_volume(self.volume);
        player.append(audio_source);
        player.play();

        self.player = Some(player);
        self.state = PlaybackState::Playing;
        self.current_track = Some(metadata);
        self.position = Duration::ZERO;
        self.last_position_update = std::time::Instant::now();

        Ok(())
    }

    pub fn pause(&mut self) {
        if let Some(ref player) = self.player {
            if self.state == PlaybackState::Playing {
                player.pause();
                self.state = PlaybackState::Paused;
            }
        }
    }

    pub fn resume(&mut self) {
        if let Some(ref player) = self.player {
            if self.state == PlaybackState::Paused {
                player.play();
                self.state = PlaybackState::Playing;
                self.last_position_update = std::time::Instant::now();
            }
        }
    }

    pub fn stop(&mut self) {
        if let Some(ref player) = self.player {
            player.stop();
        }
        self.player = None;
        self.state = PlaybackState::Stopped;
        self.position = Duration::ZERO;
        self.is_finished.store(false, Ordering::SeqCst);

        // Clear visualization
        let mut vis = self.vis_data.lock();
        *vis = VisData::default();
        self.mono_vis_buffer.lock().clear();
    }

    pub fn seek(&mut self, pos: Duration) {
        self.position = pos;
        let mut p = self.shared_params.lock();
        p.seek_request = Some(pos);
        self.last_position_update = std::time::Instant::now();
    }

    pub fn set_volume(&mut self, vol: f32) {
        self.volume = vol.clamp(0.0, 1.0);
        if let Some(ref player) = self.player {
            player.set_volume(self.volume);
        }
        let mut p = self.shared_params.lock();
        p.volume = self.volume;
    }

    pub fn set_balance(&mut self, bal: f32) {
        self.balance = bal.clamp(-1.0, 1.0);
        let mut p = self.shared_params.lock();
        p.balance = self.balance;
    }

    pub fn update_eq(&mut self, eq: EqSettings) {
        self.eq = eq;
        let mut p = self.shared_params.lock();
        p.eq = self.eq;
    }

    pub fn is_playing(&self) -> bool {
        self.state == PlaybackState::Playing
    }

    /// Check if stream completed and update playback time
    pub fn update(&mut self) -> bool {
        let mut track_finished = false;

        if self.state == PlaybackState::Playing {
            // Check stream EOF
            if self.is_finished.load(Ordering::SeqCst) {
                self.stop();
                track_finished = true;
            } else {
                let now = std::time::Instant::now();
                let elapsed = now.duration_since(self.last_position_update);
                self.last_position_update = now;
                self.position += elapsed;
            }
        }

        self.process_visualization();
        track_finished
    }

    fn process_visualization(&self) {
        let samples = {
            let buf = self.mono_vis_buffer.lock();
            buf.clone()
        };

        let mut vis = self.vis_data.lock();

        if samples.is_empty() || self.state != PlaybackState::Playing {
            // Decay spectrum smoothly when idle or stopped
            for i in 0..SPECTRUM_BARS {
                vis.spectrum[i] = (vis.spectrum[i] * 0.85).max(0.0);
                vis.spectrum_peaks[i] = (vis.spectrum_peaks[i] - 0.04).max(0.0);
            }
            // Decay waveform to flat center
            for i in 0..WAVEFORM_SAMPLES {
                vis.waveform[i] *= 0.8;
            }
            return;
        }

        // 1. Waveform for oscilloscope (75 mono samples)
        let chunk_size = (samples.len() / WAVEFORM_SAMPLES).max(1);
        for i in 0..WAVEFORM_SAMPLES {
            let idx = (i * chunk_size).min(samples.len() - 1);
            let target = samples[idx].clamp(-1.0, 1.0);
            vis.waveform[i] = vis.waveform[i] * 0.3 + target * 0.7;
        }

        // 2. FFT Spectrum Analysis (512 points of downmixed mono)
        let fft_size = 512;
        if samples.len() >= fft_size {
            let mut input: Vec<Complex<f32>> = Vec::with_capacity(fft_size);
            for (i, &s) in samples.iter().take(fft_size).enumerate() {
                // Apply Hann window
                let window = 0.5 * (1.0 - (2.0 * std::f32::consts::PI * i as f32 / (fft_size as f32 - 1.0)).cos());
                input.push(Complex::new(s * window, 0.0));
            }

            let mut planner = self.fft_planner.lock();
            let fft = planner.plan_fft_forward(fft_size);
            fft.process(&mut input);

            let mut magnitudes = [0.0f32; 256];
            for i in 0..256 {
                magnitudes[i] = (input[i].norm() / 16.0).clamp(0.0, 1.0);
            }

            // Map into 19 Winamp frequency bands (logarithmic grouping)
            let band_indices: [(usize, usize); SPECTRUM_BARS] = [
                (1, 2),    // ~80 Hz
                (2, 3),    // ~170 Hz
                (3, 4),    // ~250 Hz
                (4, 6),    // ~370 Hz
                (6, 8),    // ~550 Hz
                (8, 11),   // ~750 Hz
                (11, 15),  // ~1.1 kHz
                (15, 20),  // ~1.5 kHz
                (20, 26),  // ~2.0 kHz
                (26, 34),  // ~2.8 kHz
                (34, 44),  // ~3.8 kHz
                (44, 56),  // ~5.0 kHz
                (56, 72),  // ~6.5 kHz
                (72, 92),  // ~8.5 kHz
                (92, 116), // ~11 kHz
                (116, 145),// ~13.5 kHz
                (145, 178),// ~16 kHz
                (178, 215),// ~18.5 kHz
                (215, 255),// ~21 kHz
            ];

            for (b, &(start, end)) in band_indices.iter().enumerate() {
                let mut sum = 0.0;
                let count = (end - start).max(1) as f32;
                for i in start..end {
                    sum += magnitudes[i];
                }
                let avg = (sum / count) * 1.8;
                let val = avg.clamp(0.0, 1.0);

                vis.spectrum[b] = vis.spectrum[b] * 0.4 + val * 0.6;
                if vis.spectrum[b] >= vis.spectrum_peaks[b] {
                    vis.spectrum_peaks[b] = vis.spectrum[b];
                } else {
                    vis.spectrum_peaks[b] = (vis.spectrum_peaks[b] - 0.03).max(0.0);
                }
            }
        }
    }

    pub fn get_vis_data(&self) -> ( [f32; WAVEFORM_SAMPLES], [f32; SPECTRUM_BARS], [f32; SPECTRUM_BARS] ) {
        let vis = self.vis_data.lock();
        (vis.waveform, vis.spectrum, vis.spectrum_peaks)
    }
}

/// Helper: Extract ID3 and format metadata from audio file
fn extract_metadata(path: &Path) -> Result<TrackMetadata, String> {
    let file = File::open(path).map_err(|e| e.to_string())?;
    let mss = MediaSourceStream::new(Box::new(file), Default::default());

    let mut hint = Hint::new();
    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
        hint.with_extension(ext);
    }

    let meta_opts: MetadataOptions = Default::default();
    let fmt_opts: FormatOptions = Default::default();
    let mut probed = symphonia::default::get_probe()
        .format(&hint, mss, &fmt_opts, &meta_opts)
        .map_err(|e| e.to_string())?;

    let format = probed.format;
    let track = format
        .tracks()
        .iter()
        .find(|t| t.codec_params.codec != CODEC_TYPE_NULL)
        .ok_or("No audio track")?;

    let sample_rate = track.codec_params.sample_rate.unwrap_or(44100);
    let channels = track.codec_params.channels.map(|c| c.count() as u16).unwrap_or(2);

    let duration = if let Some(n_frames) = track.codec_params.n_frames {
        Duration::from_secs_f64(n_frames as f64 / sample_rate as f64)
    } else {
        let size = std::fs::metadata(path).map(|m| m.len()).unwrap_or(0);
        let bitrate_bps = 128_000;
        Duration::from_secs_f64((size as f64 * 8.0) / bitrate_bps as f64)
    };

    let mut title = path
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "Unknown Track".to_string());
    let mut artist = "Unknown Artist".to_string();

    // Check tags from probed metadata
    if let Some(metadata) = probed.metadata.get().as_ref().and_then(|m| m.current()) {
        for tag in metadata.tags() {
            if let Some(std_key) = tag.std_key {
                match std_key {
                    StandardTagKey::TrackTitle => title = tag.value.to_string(),
                    StandardTagKey::Artist => artist = tag.value.to_string(),
                    _ => {}
                }
            }
        }
    }

    let bitrate_kbps = if let Some(n_frames) = track.codec_params.n_frames {
        let size = std::fs::metadata(path).map(|m| m.len()).unwrap_or(0);
        let dur_secs = n_frames as f64 / sample_rate as f64;
        if dur_secs > 0.0 {
            ((size as f64 * 8.0) / (dur_secs * 1000.0)).round() as u32
        } else {
            128
        }
    } else {
        128
    };

    Ok(TrackMetadata {
        title,
        artist,
        path: path.to_path_buf(),
        duration,
        sample_rate,
        channels,
        bitrate_kbps,
    })
}

/// Custom Rodio `Source` that streams packets from Symphonia decoder,
/// applies DSP Equalization & Balance panning, and feeds mono samples to the visualizer.
struct SymphoniaSource {
    format: Box<dyn symphonia::core::formats::FormatReader>,
    decoder: Box<dyn symphonia::core::codecs::Decoder>,
    track_id: u32,
    sample_rate: u32,
    channels: u16,
    current_frame_samples: Vec<f32>,
    sample_index: usize,
    shared_params: Arc<Mutex<AudioParams>>,
    mono_vis_buffer: Arc<Mutex<Vec<f32>>>,
    is_finished: Arc<AtomicBool>,
    // 10 Biquad filters per channel (up to 2 channels)
    eq_filters: [[BiquadFilter; 10]; 2],
    last_applied_eq: EqSettings,
}

impl SymphoniaSource {
    pub fn new(
        format: Box<dyn symphonia::core::formats::FormatReader>,
        decoder: Box<dyn symphonia::core::codecs::Decoder>,
        track_id: u32,
        sample_rate: u32,
        channels: u16,
        shared_params: Arc<Mutex<AudioParams>>,
        mono_vis_buffer: Arc<Mutex<Vec<f32>>>,
        is_finished: Arc<AtomicBool>,
    ) -> Self {
        let initial_eq = shared_params.lock().eq;
        let mut filters = [[BiquadFilter::default(); 10]; 2];

        // Initialize biquad filters for each band and channel
        for ch in 0..2 {
            for b in 0..10 {
                filters[ch][b] = BiquadFilter::new_peaking(
                    initial_eq.bands[b],
                    EQ_FREQUENCIES[b],
                    sample_rate as f32,
                    1.414,
                );
            }
        }

        Self {
            format,
            decoder,
            track_id,
            sample_rate,
            channels,
            current_frame_samples: Vec::with_capacity(4096),
            sample_index: 0,
            shared_params,
            mono_vis_buffer,
            is_finished,
            eq_filters: filters,
            last_applied_eq: initial_eq,
        }
    }

    fn check_and_update_dsp(&mut self) {
        let params = self.shared_params.lock().clone();

        // Check if seek was requested
        if let Some(target_pos) = params.seek_request {
            let time = Time::from(target_pos.as_secs_f64());
            let seek_to = SeekTo::Time {
                time,
                track_id: Some(self.track_id),
            };
            if let Ok(_) = self.format.seek(SeekMode::Accurate, seek_to) {
                self.decoder.reset();
                self.current_frame_samples.clear();
                self.sample_index = 0;
            }
            // Clear request
            self.shared_params.lock().seek_request = None;
        }

        // Reconfigure EQ filters if settings changed
        let eq = params.eq;
        if eq.enabled != self.last_applied_eq.enabled || eq.bands != self.last_applied_eq.bands {
            let fs = self.sample_rate as f32;
            for ch in 0..2 {
                for b in 0..10 {
                    let gain = if eq.enabled { eq.bands[b] } else { 0.0 };
                    self.eq_filters[ch][b] = BiquadFilter::new_peaking(gain, EQ_FREQUENCIES[b], fs, 1.414);
                }
            }
            self.last_applied_eq = eq;
        }
    }
}

impl Iterator for SymphoniaSource {
    type Item = f32;

    fn next(&mut self) -> Option<Self::Item> {
        while self.sample_index >= self.current_frame_samples.len() {
            self.check_and_update_dsp();

            let packet = match self.format.next_packet() {
                Ok(packet) => packet,
                Err(SymphoniaError::IoError(e)) if e.kind() == std::io::ErrorKind::UnexpectedEof => {
                    self.is_finished.store(true, Ordering::SeqCst);
                    return None;
                }
                Err(_) => {
                    self.is_finished.store(true, Ordering::SeqCst);
                    return None;
                }
            };

            if packet.track_id() != self.track_id {
                continue;
            }

            match self.decoder.decode(&packet) {
                Ok(decoded) => {
                    self.current_frame_samples.clear();
                    self.sample_index = 0;

                    // Convert decoded audio using Symphonia's SampleBuffer (handles S8, S16, S24, S32, F32, F64)
                    let mut sample_buf = SampleBuffer::<f32>::new(decoded.capacity() as u64, *decoded.spec());
                    sample_buf.copy_interleaved_ref(decoded);
                    let raw_samples = sample_buf.samples();

                    let params = self.shared_params.lock().clone();
                    let channels = self.channels as usize;

                    // Panning coefficients
                    let (pan_l, pan_r) = if params.balance < 0.0 {
                        (1.0, 1.0 + params.balance)
                    } else if params.balance > 0.0 {
                        (1.0 - params.balance, 1.0)
                    } else {
                        (1.0, 1.0)
                    };

                    let preamp_gain = if params.eq.enabled {
                        10.0f32.powf(params.eq.preamp / 20.0)
                    } else {
                        1.0
                    };

                    let mut mono_vis_chunk = Vec::with_capacity(raw_samples.len() / channels);

                    // Process each frame through DSP (Preamp -> 10-band EQ -> Balance Panning)
                    for frame_chunk in raw_samples.chunks(channels) {
                        let mut processed_frame = [0.0f32; 2];

                        for ch in 0..channels.min(2) {
                            let mut sample = frame_chunk[ch] * preamp_gain;

                            // 10-band Equalizer filtering
                            if params.eq.enabled {
                                for b in 0..10 {
                                    sample = self.eq_filters[ch][b].process(sample);
                                }
                            }

                            // Balance panning
                            let pan = if ch == 0 { pan_l } else { pan_r };
                            sample *= pan;

                            processed_frame[ch] = sample;
                            self.current_frame_samples.push(sample);
                        }

                        // Mono downmix for visualizer (0.5 * (L + R))
                        let mono_sample = if channels >= 2 {
                            0.5 * (processed_frame[0] + processed_frame[1])
                        } else {
                            processed_frame[0]
                        };
                        mono_vis_chunk.push(mono_sample);
                    }

                    // Feed mono samples into visualization buffer
                    if !mono_vis_chunk.is_empty() {
                        let mut vbuf = self.mono_vis_buffer.lock();
                        vbuf.extend_from_slice(&mono_vis_chunk);
                        if vbuf.len() > 2048 {
                            let excess = vbuf.len() - 2048;
                            vbuf.drain(0..excess);
                        }
                    }
                }
                Err(SymphoniaError::DecodeError(_)) => continue,
                Err(_) => {
                    self.is_finished.store(true, Ordering::SeqCst);
                    return None;
                }
            }
        }

        let sample = self.current_frame_samples[self.sample_index];
        self.sample_index += 1;
        Some(sample)
    }
}

impl Source for SymphoniaSource {
    fn current_span_len(&self) -> Option<usize> {
        Some(self.current_frame_samples.len() - self.sample_index)
    }

    fn channels(&self) -> NonZeroU16 {
        NonZeroU16::new(self.channels).unwrap_or(NonZeroU16::new(2).unwrap())
    }

    fn sample_rate(&self) -> NonZeroU32 {
        NonZeroU32::new(self.sample_rate).unwrap_or(NonZeroU32::new(44100).unwrap())
    }

    fn total_duration(&self) -> Option<Duration> {
        None
    }
}
