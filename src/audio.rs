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
use symphonia::core::units::{Time, TimeBase};

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
        let shared_params = Arc::new(Mutex::new(AudioParams {
            volume: 0.8,
            balance: 0.0,
            eq: EqSettings::default(),
            seek_request: None,
        }));

        Self {
            sink_handle: None,
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
        let cancellation = AtomicBool::new(false);
        self.play_file_with_cancellation(path, &cancellation)
    }

    /// Play a file while allowing a caller on another thread to cancel source
    /// priming. Cancellation is checked around packet reads/decodes, but cannot
    /// interrupt a read or decode already in progress.
    pub fn play_file_with_cancellation<P: AsRef<Path>>(
        &mut self,
        path: P,
        cancellation: &AtomicBool,
    ) -> Result<(), String> {
        let path_buf = path.as_ref().to_path_buf();
        self.stop();

        if self.sink_handle.is_none() {
            if let Ok(handle) = DeviceSinkBuilder::open_default_sink() {
                self.sink_handle = Some(handle);
            }
        }

        let sink_handle = self.sink_handle.as_ref().ok_or("No audio output device available")?;

        // Read metadata using Symphonia
        let mut metadata = extract_metadata(&path_buf)?;

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

        let is_finished = Arc::new(AtomicBool::new(false));
        let shared_params = Arc::new(Mutex::new(AudioParams {
            volume: self.volume,
            balance: self.balance,
            eq: self.eq,
            seek_request: None,
        }));
        let mono_vis_buffer = Arc::new(Mutex::new(Vec::with_capacity(2048)));

        let audio_source = SymphoniaSource::new(
            format,
            decoder,
            track_id,
            sample_rate,
            channels,
            shared_params.clone(),
            mono_vis_buffer.clone(),
            is_finished.clone(),
            cancellation,
        )?;

        if audio_source.current_frame_samples.is_empty() {
            return Err("Audio track contains no decodable samples".into());
        }
        metadata.channels = audio_source.channels().get();
        metadata.sample_rate = audio_source.sample_rate().get();

        let player = Player::connect_new(sink_handle.mixer());
        player.set_volume(self.volume);
        player.append(audio_source);
        player.play();

        // Publish the new source's shared state only after priming succeeds.
        self.is_finished = is_finished;
        self.shared_params = shared_params;
        self.mono_vis_buffer = mono_vis_buffer;
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
        if self.player.is_none() {
            return;
        }
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
            if self.is_finished.load(Ordering::SeqCst)
                && self.player.as_ref().is_some_and(Player::empty)
            {
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

/// Rodio caps input spans at 32,768 samples. Keep each span on an interleaved
/// frame boundary before Rodio applies that cap.
const MAX_SOURCE_SPAN_SAMPLES: usize = 32_768;
const MAX_PRIMING_PACKETS: usize = 32;

#[derive(Clone, Copy, PartialEq, Eq)]
enum PacketLoad {
    SamplesReady,
    Skipped,
    End,
}

/// Custom Rodio `Source` that streams packets from Symphonia decoder,
/// applies DSP Equalization & Balance panning, and feeds mono samples to the visualizer.
struct SymphoniaSource {
    format: Box<dyn symphonia::core::formats::FormatReader>,
    decoder: Box<dyn symphonia::core::codecs::Decoder>,
    track_id: u32,
    sample_rate: u32,
    channels: u16,
    /// True once the channel count has been adopted from a decoded packet
    /// (container headers can be missing or wrong; the stream is the truth).
    channels_confirmed: bool,
    time_base: Option<TimeBase>,
    seek_target_ts: Option<u64>,
    stream_ended: bool,
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
    fn new(
        format: Box<dyn symphonia::core::formats::FormatReader>,
        decoder: Box<dyn symphonia::core::codecs::Decoder>,
        track_id: u32,
        sample_rate: u32,
        channels: u16,
        shared_params: Arc<Mutex<AudioParams>>,
        mono_vis_buffer: Arc<Mutex<Vec<f32>>>,
        is_finished: Arc<AtomicBool>,
        cancellation: &AtomicBool,
    ) -> Result<Self, String> {
        Self::new_with_priming_budget(
            format,
            decoder,
            track_id,
            sample_rate,
            channels,
            shared_params,
            mono_vis_buffer,
            is_finished,
            cancellation,
            MAX_PRIMING_PACKETS,
        )
    }

    fn new_with_priming_budget(
        format: Box<dyn symphonia::core::formats::FormatReader>,
        decoder: Box<dyn symphonia::core::codecs::Decoder>,
        track_id: u32,
        sample_rate: u32,
        channels: u16,
        shared_params: Arc<Mutex<AudioParams>>,
        mono_vis_buffer: Arc<Mutex<Vec<f32>>>,
        is_finished: Arc<AtomicBool>,
        cancellation: &AtomicBool,
        priming_packet_budget: usize,
    ) -> Result<Self, String> {
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

        let time_base = format.tracks().iter().find(|track| track.id == track_id)
            .and_then(|track| track.codec_params.time_base)
            .filter(|base| base.numer != 0 && base.denom != 0);
        let mut source = Self {
            format,
            decoder,
            track_id,
            sample_rate,
            channels,
            channels_confirmed: false,
            time_base,
            seek_target_ts: None,
            stream_ended: false,
            current_frame_samples: Vec::with_capacity(4096),
            sample_index: 0,
            shared_params,
            mono_vis_buffer,
            is_finished,
            eq_filters: filters,
            last_applied_eq: initial_eq,
        };
        source.prime_first_packet(priming_packet_budget, cancellation)?;
        Ok(source)
    }

    fn check_and_update_dsp(&mut self) -> bool {
        let (seek_request, eq) = {
            let mut params = self.shared_params.lock();
            (params.seek_request.take(), params.eq)
        };
        let mut reset_filters = false;
        let mut seek_succeeded = false;
        if let Some(target_pos) = seek_request {
            let seek_to = SeekTo::Time {
                time: Time::from(target_pos.as_secs_f64()),
                track_id: Some(self.track_id),
            };
            match self.format.seek(SeekMode::Accurate, seek_to) {
                Ok(seeked) => {
                    self.decoder.reset();
                    self.current_frame_samples.clear();
                    self.sample_index = 0;
                    self.stream_ended = false;
                    self.is_finished.store(false, Ordering::SeqCst);
                    self.mono_vis_buffer.lock().clear();
                    self.seek_target_ts = (seeked.actual_ts < seeked.required_ts)
                        .then_some(seeked.required_ts);
                    reset_filters = true;
                    seek_succeeded = true;
                }
                Err(e) => log::warn!("Seek failed: {e}"),
            }
        }

        // Reset filter history after a seek as well as when EQ settings change.
        if reset_filters || eq.enabled != self.last_applied_eq.enabled || eq.bands != self.last_applied_eq.bands {
            let fs = self.sample_rate as f32;
            for ch in 0..2 {
                for b in 0..10 {
                    let gain = if eq.enabled { eq.bands[b] } else { 0.0 };
                    self.eq_filters[ch][b] = BiquadFilter::new_peaking(gain, EQ_FREQUENCIES[b], fs, 1.414);
                }
            }
            self.last_applied_eq = eq;
        }
        seek_succeeded
    }

    fn prime_first_packet(
        &mut self,
        packet_budget: usize,
        cancellation: &AtomicBool,
    ) -> Result<(), String> {
        for _ in 0..packet_budget {
            if cancellation.load(Ordering::SeqCst) {
                return Err("Audio source priming cancelled".into());
            }

            let load = self.load_next_packet();
            if cancellation.load(Ordering::SeqCst) {
                return Err("Audio source priming cancelled".into());
            }
            match load {
                PacketLoad::SamplesReady => return Ok(()),
                PacketLoad::Skipped => {}
                PacketLoad::End => return Err("Audio track contains no decodable samples".into()),
            }
        }

        if cancellation.load(Ordering::SeqCst) {
            Err("Audio source priming cancelled".into())
        } else {
            Err(format!(
                "Audio source priming exhausted its packet budget ({packet_budget} packets)"
            ))
        }
    }

    fn load_next_packet(&mut self) -> PacketLoad {
        self.check_and_update_dsp();
        if self.stream_ended {
            return PacketLoad::End;
        }

        self.current_frame_samples.clear();
        self.sample_index = 0;

        let packet = match self.format.next_packet() {
            Ok(packet) => packet,
            Err(SymphoniaError::IoError(e)) if e.kind() == std::io::ErrorKind::UnexpectedEof => {
                self.stream_ended = true;
                self.is_finished.store(true, Ordering::SeqCst);
                return PacketLoad::End;
            }
            Err(_) => {
                self.stream_ended = true;
                self.is_finished.store(true, Ordering::SeqCst);
                return PacketLoad::End;
            }
        };

        if packet.track_id() != self.track_id {
            return PacketLoad::Skipped;
        }

        let decoded = match self.decoder.decode(&packet) {
            Ok(decoded) => decoded,
            Err(SymphoniaError::DecodeError(_)) => return PacketLoad::Skipped,
            Err(_) => {
                self.stream_ended = true;
                self.is_finished.store(true, Ordering::SeqCst);
                return PacketLoad::End;
            }
        };

        // The decoder-reported layout is authoritative: container headers can
        // be missing or disagree with the actual stream.
        let spec = *decoded.spec();
        let spec_channels = spec.channels.count();
        if spec_channels == 0 || spec.rate == 0 {
            return PacketLoad::Skipped;
        }
        if self.channels_confirmed
            && (spec_channels != usize::from(self.channels) || spec.rate != self.sample_rate)
        {
            log::warn!("Stopping audio after a mid-stream format change");
            self.stream_ended = true;
            self.is_finished.store(true, Ordering::SeqCst);
            return PacketLoad::End;
        }

        let mut sample_buf = SampleBuffer::<f32>::new(decoded.capacity() as u64, spec);
        sample_buf.copy_interleaved_ref(decoded);
        let mut raw_samples = sample_buf.samples();
        if let Some(target) = self.seek_target_ts {
            if let Some(base) = self.time_base {
                let ticks = target.saturating_sub(packet.ts());
                let frames = (u128::from(ticks) * u128::from(base.numer) * u128::from(spec.rate))
                    .div_ceil(u128::from(base.denom));
                if frames >= (raw_samples.len() / spec_channels) as u128 {
                    return PacketLoad::Skipped;
                }
                raw_samples = &raw_samples[frames as usize * spec_channels..];
            }
            self.seek_target_ts = None;
        }

        let complete_sample_count = raw_samples.len() / spec_channels * spec_channels;
        if complete_sample_count != raw_samples.len() {
            log::warn!("Stopping audio after an incomplete interleaved frame");
            self.stream_ended = true;
            self.is_finished.store(true, Ordering::SeqCst);
            raw_samples = &raw_samples[..complete_sample_count];
        }
        if !self.channels_confirmed && !raw_samples.is_empty() {
            self.channels = spec_channels as u16;
            if self.sample_rate != spec.rate {
                self.sample_rate = spec.rate;
                for filters in &mut self.eq_filters {
                    for (band, filter) in filters.iter_mut().enumerate() {
                        *filter = BiquadFilter::new_peaking(
                            self.last_applied_eq.bands[band],
                            EQ_FREQUENCIES[band],
                            spec.rate as f32,
                            1.414,
                        );
                    }
                }
            }
            self.channels_confirmed = true;
        }
        if raw_samples.is_empty() {
            return PacketLoad::Skipped;
        }

        let channels = self.channels as usize;
        let params = self.shared_params.lock().clone();
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
        for frame_chunk in raw_samples.chunks_exact(channels) {
            let mut frame_sum = 0.0f32;
            for (ch, &raw) in frame_chunk.iter().enumerate() {
                let mut sample = raw * preamp_gain;
                if params.eq.enabled && ch < 2 {
                    for b in 0..10 {
                        sample = self.eq_filters[ch][b].process(sample);
                    }
                }
                if channels >= 2 {
                    sample *= match ch {
                        0 => pan_l,
                        1 => pan_r,
                        _ => 1.0,
                    };
                }
                self.current_frame_samples.push(sample);
                frame_sum += sample;
            }
            mono_vis_chunk.push(frame_sum / frame_chunk.len() as f32);
        }

        if !mono_vis_chunk.is_empty() {
            let mut vbuf = self.mono_vis_buffer.lock();
            vbuf.extend_from_slice(&mono_vis_chunk);
            if vbuf.len() > 2048 {
                let excess = vbuf.len() - 2048;
                vbuf.drain(0..excess);
            }
        }

        if self.current_frame_samples.is_empty() {
            PacketLoad::Skipped
        } else {
            PacketLoad::SamplesReady
        }
    }

    fn load_next_audio_span(&mut self) -> bool {
        loop {
            match self.load_next_packet() {
                PacketLoad::SamplesReady => return true,
                PacketLoad::Skipped => continue,
                PacketLoad::End => return false,
            }
        }
    }
}

impl Iterator for SymphoniaSource {
    type Item = f32;

    fn next(&mut self) -> Option<Self::Item> {
        let seek_pending = self.sample_index == 0
            && !self.current_frame_samples.is_empty()
            && self.shared_params.lock().seek_request.is_some();
        if seek_pending && self.check_and_update_dsp() && !self.load_next_audio_span() {
            return None;
        }

        if self.sample_index >= self.current_frame_samples.len() && !self.load_next_audio_span() {
            return None;
        }

        let sample = self.current_frame_samples[self.sample_index];
        self.sample_index += 1;

        // Keep the next span visible before Rodio asks for it. In particular,
        // Some(0) is reserved for EOF rather than a packet boundary.
        if self.sample_index == self.current_frame_samples.len() {
            self.load_next_audio_span();
        }

        Some(sample)
    }
}

impl Source for SymphoniaSource {
    fn current_span_len(&self) -> Option<usize> {
        let sample_count = self.current_frame_samples.len();
        if self.sample_index >= sample_count {
            return Some(0);
        }

        let channels = usize::from(self.channels.max(1));
        let span_limit = (MAX_SOURCE_SPAN_SAMPLES / channels) * channels;
        let span_limit = span_limit.max(channels);
        let span_start = (self.sample_index / span_limit) * span_limit;
        Some((sample_count - span_start).min(span_limit))
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

#[cfg(test)]
mod framing_tests {
    use super::*;

    /// Build a minimal 16-bit PCM WAV with the given channel count.
    fn wav(channels: u16, frames: usize) -> Vec<u8> {
        let mut samples = Vec::with_capacity(channels as usize * frames);
        for frame in 0..frames {
            let sample = if frame % 2 == 0 { 800 } else { -800 };
            samples.extend(std::iter::repeat_n(sample, channels as usize));
        }
        wav_with_interleaved_samples(channels, &samples)
    }

    fn wav_with_interleaved_samples(channels: u16, samples: &[i16]) -> Vec<u8> {
        assert_eq!(samples.len() % channels as usize, 0);
        let data_len = (samples.len() * 2) as u32;
        let mut wav: Vec<u8> = Vec::with_capacity(44 + data_len as usize);
        wav.extend_from_slice(b"RIFF");
        wav.extend_from_slice(&(36 + data_len).to_le_bytes());
        wav.extend_from_slice(b"WAVE");
        wav.extend_from_slice(b"fmt ");
        wav.extend_from_slice(&16u32.to_le_bytes());
        wav.extend_from_slice(&1u16.to_le_bytes()); // PCM
        wav.extend_from_slice(&channels.to_le_bytes());
        wav.extend_from_slice(&44_100u32.to_le_bytes());
        wav.extend_from_slice(&(44_100 * channels as u32 * 2).to_le_bytes());
        wav.extend_from_slice(&(channels * 2).to_le_bytes());
        wav.extend_from_slice(&16u16.to_le_bytes());
        wav.extend_from_slice(b"data");
        wav.extend_from_slice(&data_len.to_le_bytes());
        for sample in samples {
            wav.extend_from_slice(&sample.to_le_bytes());
        }
        wav
    }

    /// Decode a file through the player's actual source, the same way
    /// `AudioEngine::play_file` constructs it. `claimed_channels` lets tests
    /// simulate a container header that lies (or says nothing).
    fn wav_source(wav: Vec<u8>, claimed_channels: u16) -> SymphoniaSource {
        let cancellation = AtomicBool::new(false);
        wav_source_with_options(
            wav,
            claimed_channels,
            0.0,
            MAX_PRIMING_PACKETS,
            &cancellation,
        )
        .unwrap()
    }

    fn wav_source_with_options(
        wav: Vec<u8>,
        claimed_channels: u16,
        balance: f32,
        packet_budget: usize,
        cancellation: &AtomicBool,
    ) -> Result<SymphoniaSource, String> {
        let mut hint = Hint::new();
        hint.with_extension("wav");
        let input = MediaSourceStream::new(Box::new(std::io::Cursor::new(wav)), Default::default());
        let format = symphonia::default::get_probe()
            .format(&hint, input, &FormatOptions::default(), &MetadataOptions::default())
            .unwrap()
            .format;
        let track = format.default_track().unwrap();
        let id = track.id;
        let params = &track.codec_params;
        let sample_rate = params.sample_rate.unwrap_or(44100);
        let decoder = symphonia::default::get_codecs()
            .make(params, &DecoderOptions::default())
            .unwrap();
        SymphoniaSource::new_with_priming_budget(
            format,
            decoder,
            id,
            sample_rate,
            claimed_channels,
            Arc::new(Mutex::new(AudioParams {
                volume: 1.0,
                balance,
                eq: EqSettings::default(),
                seek_request: None,
            })),
            Arc::new(Mutex::new(Vec::new())),
            Arc::new(AtomicBool::new(false)),
            cancellation,
            packet_budget,
        )
    }

    fn decode_through_source(wav: Vec<u8>, claimed_channels: u16) -> (u16, usize) {
        let source = wav_source(wav, claimed_channels);
        (source.channels().get(), source.count())
    }

    #[test]
    fn seek_starts_at_requested_pcm_frame() {
        let mut source = wav_source(wav(2, 2400), 2);
        let initial_span = source.current_span_len().unwrap();
        assert_eq!(source.by_ref().take(initial_span).count(), initial_span);
        // 10 ms × 44,100 Hz = frame 441, the first negative sample here.
        source.shared_params.lock().seek_request = Some(Duration::from_millis(10));
        let samples: Vec<_> = source.collect();
        assert_eq!(samples.len(), (2400 - 441) * 2);
        assert!(samples[0] < 0.0);
    }

    #[test]
    fn engine_waits_for_the_player_queue_to_drain() {
        let source = wav_source(wav(2, 800), 2);
        let (mixer, output) = rodio::mixer::mixer(source.channels(), source.sample_rate());
        let mut engine = AudioEngine::new();
        engine.is_finished = source.is_finished.clone();
        let player = Player::connect_new(&mixer);
        player.append(source);
        engine.player = Some(player);
        engine.state = PlaybackState::Playing;
        assert!(!engine.update());

        // Advance the mixer without opening a hardware device.
        let _ = output.take(4_096).count();
        assert!(engine.update());
        assert_eq!(engine.state, PlaybackState::Stopped);
        assert!(!engine.update(), "EOF is reported once");
    }

    #[test]
    fn long_surround_packet_uses_bounded_frame_aligned_spans() {
        let mut source = wav_source(wav(6, 8192), 6);
        // Symphonia's WAV reader splits this fixture into smaller packets;
        // expand the primed packet to reproduce a 6 × 8192-sample span.
        source.current_frame_samples.resize(8192 * 6, 0.0);
        source.sample_index = 0;
        source.stream_ended = true;
        assert_eq!(source.current_span_len(), Some(32_766));
        let mut total_samples = 0;

        loop {
            let span_len = source.current_span_len().unwrap();
            if span_len == 0 {
                break;
            }
            assert!(span_len <= MAX_SOURCE_SPAN_SAMPLES);
            assert_eq!(span_len % 6, 0, "each span must end on a complete 5.1 frame");
            let read = source.by_ref().take(span_len).count();
            assert_eq!(read, span_len, "Iterator::next must deliver every sample in the span");
            total_samples += read;
        }

        assert_eq!(total_samples, 8192 * 6);

        let mut long_source = wav_source(wav(6, 8192), 6);
        long_source.current_frame_samples = (0..8192)
            .flat_map(|_| [0.1, 0.2, 0.3, 0.4, 0.5, 0.6])
            .collect();
        long_source.sample_index = 0;
        long_source.stream_ended = true;
        let stereo_output: Vec<_> = rodio::source::UniformSourceIterator::new(
            long_source,
            NonZeroU16::new(2).unwrap(),
            NonZeroU32::new(44_100).unwrap(),
        )
        .collect();
        assert_eq!(stereo_output.len(), 8192 * 2);
        for frame in stereo_output.chunks_exact(2) {
            assert!((frame[0] - 0.1).abs() < 1e-5);
            assert!((frame[1] - 0.2).abs() < 1e-5);
        }
    }

    #[test]
    fn balance_pans_front_pair_of_multichannel_audio_only() {
        let values = [4000, 8000, 12000, 16000, 20000, 24000];
        let cancellation = AtomicBool::new(false);
        let source = wav_source_with_options(
            wav_with_interleaved_samples(6, &values),
            6,
            -1.0,
            MAX_PRIMING_PACKETS,
            &cancellation,
        )
        .unwrap();
        let samples: Vec<_> = source.collect();

        assert_eq!(samples.len(), 6);
        assert!((samples[0] - values[0] as f32 / 32768.0).abs() < 1e-5);
        assert_eq!(samples[1], 0.0);
        for channel in 2..6 {
            assert!(
                (samples[channel] - values[channel] as f32 / 32768.0).abs() < 1e-5,
                "non-front channel {channel} must remain intact"
            );
        }
    }

    #[test]
    fn priming_reports_budget_exhaustion_and_cancellation() {
        let not_cancelled = AtomicBool::new(false);
        let budget_error = wav_source_with_options(wav(2, 8), 2, 0.0, 0, &not_cancelled)
            .err()
            .expect("zero packet budget must fail explicitly");
        assert!(budget_error.contains("packet budget"), "{budget_error}");

        let cancelled = AtomicBool::new(true);
        let cancel_error = wav_source_with_options(
            wav(2, 8),
            2,
            0.0,
            MAX_PRIMING_PACKETS,
            &cancelled,
        )
        .err()
        .expect("cancellation must fail explicitly");
        assert!(cancel_error.contains("cancelled"), "{cancel_error}");
    }

    #[test]
    fn stereo_wav_preserves_frame_count() {
        let (channels, samples) = decode_through_source(wav(2, 2400), 2);
        assert_eq!((channels, samples), (2, 4800), "2ch × 2400 frames must emit 4800 samples");
    }

    #[test]
    fn mono_wav_preserves_channel_layout() {
        let (channels, samples) = decode_through_source(wav(1, 1000), 1);
        assert_eq!((channels, samples), (1, 1000), "mono must report 1 channel and emit one sample per frame");
    }

    #[test]
    fn surround_wav_preserves_frame_count() {
        let (channels, samples) = decode_through_source(wav(6, 800), 6);
        assert_eq!((channels, samples), (6, 4800), "5.1 × 800 frames must emit all 6 samples per frame");
    }

    #[test]
    fn channel_count_adopted_from_decoded_stream() {
        // An MP4/AAC mono file often probes with no channel count, so the
        // source starts as a guessed "stereo" — the decoded stream must win.
        let (channels, samples) = decode_through_source(wav(1, 1000), 2);
        assert_eq!((channels, samples), (1, 1000), "source must self-correct to the decoder's layout");
    }
}
