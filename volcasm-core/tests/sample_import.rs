//! Import of synthetic fixtures (`tests/fixtures/synthetic/generate.sh`):
//! 0.2 s of a 1 kHz sine at amplitude 0.5, stereo files with silence in the
//! right channel.

use std::fs::File;
use std::path::{Path, PathBuf};

use volcasm_core::device::VOLCA_SAMPLE;
use volcasm_core::sample::{Sample, import, wav};

const RATE: u32 = VOLCA_SAMPLE.sample_rate;
const DURATION: f64 = 0.2;
const FREQUENCY: f64 = 1000.0;
/// Peak of the sine (amplitude 0.5) in 16-bit.
const MONO_PEAK: f64 = 16_384.0;

fn synthetic(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../tests/fixtures/synthetic")
        .join(name)
}

fn import(name: &str) -> Sample {
    let sample = import::import_file(&synthetic(name), RATE).unwrap();
    assert_eq!(sample.sample_rate, RATE);
    sample
}

/// Peak and zero-crossing frequency of the middle half (away from the edges,
/// where resampler and codec transients live).
fn measure(sample: &Sample) -> (f64, f64) {
    let n = sample.frames();
    let middle = &sample.pcm[n / 4..3 * n / 4];
    let peak = middle
        .iter()
        .map(|&s| f64::from(s).abs())
        .fold(0.0, f64::max);
    let crossings = middle
        .windows(2)
        .filter(|w| (w[0] < 0) != (w[1] < 0))
        .count();
    let seconds = middle.len() as f64 / f64::from(RATE);
    (peak, crossings as f64 / 2.0 / seconds)
}

fn assert_close(actual: f64, expected: f64, tolerance: f64, what: &str) {
    assert!(
        (actual - expected).abs() <= expected * tolerance,
        "{what}: {actual}, expected {expected} ± {}%",
        tolerance * 100.0
    );
}

/// Checks length, level (within `level_tolerance`) and pitch of an imported
/// sine with `peak`.
fn assert_sine(
    name: &str,
    peak: f64,
    level_tolerance: f64,
    frames: std::ops::RangeInclusive<usize>,
) {
    let sample = import(name);
    assert!(
        frames.contains(&sample.frames()),
        "{name}: {} frames, expected {frames:?}",
        sample.frames()
    );
    let (measured_peak, frequency) = measure(&sample);
    assert_close(
        measured_peak,
        peak,
        level_tolerance,
        &format!("{name} peak"),
    );
    assert_close(frequency, FREQUENCY, 0.01, &format!("{name} frequency"));
}

/// Lossless sources: exactly `ceil(0.2 s * 31 250 Hz)` frames.
const EXACT: std::ops::RangeInclusive<usize> = 6250..=6250;
/// Lossy sources: encoder delay or padding, up to 50 ms.
const LOSSY_FRAMES: std::ops::RangeInclusive<usize> = 6250..=6250 + 1563;
/// Level tolerance for lossless sources.
const LOSSLESS_LEVEL: f64 = 0.01;
/// Lossy codecs change the level themselves: ffmpeg decodes the MP3 fixture
/// to a peak of 15 567 (-5%) and the Vorbis one to about 16 612 (+1.4%).
const LOSSY_LEVEL: f64 = 0.06;

#[test]
fn wav_stereo_24_bit_is_downmixed_and_resampled() {
    // Sine in one channel, silence in the other: half the level after the
    // downmix.
    assert_sine(
        "sine_44100_stereo_s24.wav",
        MONO_PEAK / 2.0,
        LOSSLESS_LEVEL,
        EXACT,
    );
}

#[test]
fn wav_float_48k_is_resampled() {
    assert_sine("sine_48000_mono_f32.wav", MONO_PEAK, LOSSLESS_LEVEL, EXACT);
}

#[test]
fn aiff_is_imported() {
    assert_sine("sine_44100_mono_s16.aiff", MONO_PEAK, LOSSLESS_LEVEL, EXACT);
}

#[test]
fn flac_is_imported() {
    assert_sine("sine_44100_mono.flac", MONO_PEAK, LOSSLESS_LEVEL, EXACT);
}

#[test]
fn mp3_is_imported() {
    assert_sine("sine_44100_mono.mp3", MONO_PEAK, LOSSY_LEVEL, LOSSY_FRAMES);
}

#[test]
fn m4a_aac_is_imported() {
    assert_sine("sine_44100_mono.m4a", MONO_PEAK, LOSSY_LEVEL, LOSSY_FRAMES);
}

/// Whole songs often carry cover art in an ID3v2 tag larger than the 1 MB
/// that symphonia scans for a format; without the ID3v2 reader such files
/// failed with "no suitable format reader found".
#[test]
fn mp3_after_large_id3_tag_is_imported() {
    const PAYLOAD: usize = 2 * 1024 * 1024;
    let mut frame = b"volcasm\0".to_vec();
    frame.resize(PAYLOAD, 0);
    let mut tag = b"PRIV".to_vec();
    tag.extend_from_slice(&(frame.len() as u32).to_be_bytes());
    tag.extend_from_slice(&[0, 0]);
    tag.extend_from_slice(&frame);
    // ID3v2.3 header; the tag size is "synchsafe" (7 bits per byte).
    let size = tag.len() as u32;
    let mut file = b"ID3\x03\x00\x00".to_vec();
    file.extend((0..4).rev().map(|i| ((size >> (7 * i)) & 0x7f) as u8));
    file.extend_from_slice(&tag);
    file.extend_from_slice(&std::fs::read(synthetic("sine_44100_mono.mp3")).unwrap());

    let path = std::env::temp_dir().join(format!("volcasm-id3-{}.mp3", std::process::id()));
    std::fs::write(&path, &file).unwrap();
    let result = import::import_file(&path, RATE);
    std::fs::remove_file(&path).unwrap();
    let sample = result.unwrap();
    assert!(
        LOSSY_FRAMES.contains(&sample.frames()),
        "{} frames",
        sample.frames()
    );
    let (peak, frequency) = measure(&sample);
    assert_close(peak, MONO_PEAK, LOSSY_LEVEL, "peak");
    assert_close(frequency, FREQUENCY, 0.01, "frequency");
}

#[test]
fn ogg_vorbis_is_imported() {
    assert_sine(
        "sine_44100_stereo.ogg",
        MONO_PEAK / 2.0,
        LOSSY_LEVEL,
        LOSSY_FRAMES,
    );
}

#[test]
fn device_format_wav_passes_through_bit_exact() {
    for path in [
        synthetic("sine_31250_mono_s16.wav"),
        // Own recording from Test2 (D12).
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../tests/fixtures/vosyr/Test2/samples/sample1791833706583"),
    ] {
        let original = wav::read(File::open(&path).unwrap()).unwrap();
        assert_eq!(original.sample_rate, RATE);
        let imported = import::import_file(&path, RATE).unwrap();
        assert_eq!(imported, original, "{path:?}");
    }
}

#[test]
fn passthrough_sine_has_expected_shape() {
    let sample = import("sine_31250_mono_s16.wav");
    assert_eq!(sample.frames(), (DURATION * f64::from(RATE)) as usize);
    let (peak, frequency) = measure(&sample);
    assert_close(peak, MONO_PEAK, 0.001, "peak");
    assert_close(frequency, FREQUENCY, 0.01, "frequency");
}

#[test]
fn non_audio_file_is_rejected() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../tests/fixtures/vosyr/Test1/sequences/sequence01");
    assert!(import::import_file(&path, RATE).is_err());
}
