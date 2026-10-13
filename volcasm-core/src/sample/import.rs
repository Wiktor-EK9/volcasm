//! Import of audio files (WAV, AIFF, FLAC, MP3, Ogg Vorbis) into the device
//! format.
//!
//! Steps: decode, downmix to mono (mean of all channels), resample to the
//! target rate, convert to 16-bit with rounding and clipping (no dither).
//! Steps that are not needed are skipped, so 16-bit mono material already at
//! the target rate passes through unchanged, bit for bit.

use std::fmt;
use std::fs::File;
use std::path::Path;

use rubato::audioadapter_buffers::direct::SequentialSliceOfSlices;
use rubato::{Fft, FixedSync, Resampler};
use symphonia::core::codecs::audio::AudioDecoderOptions;
use symphonia::core::errors::Error as DecodeError;
use symphonia::core::formats::probe::Hint;
use symphonia::core::formats::{FormatOptions, TrackType};
use symphonia::core::io::{MediaSource, MediaSourceStream};
use symphonia::core::meta::MetadataOptions;

use super::Sample;

/// Chunk size for the FFT resampler.
const RESAMPLER_CHUNK: usize = 1024;

/// Error while importing an audio file.
#[derive(Debug)]
pub enum ImportError {
    Io(std::io::Error),
    /// The container or codec could not be read.
    Decode(DecodeError),
    /// The file has no audio track.
    NoAudioTrack,
    /// The decoded audio has no frames.
    Empty,
    /// Resampler setup or processing failed.
    Resample(String),
}

impl fmt::Display for ImportError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(err) => write!(f, "I/O error: {err}"),
            Self::Decode(err) => write!(f, "decode error: {err}"),
            Self::NoAudioTrack => write!(f, "no audio track"),
            Self::Empty => write!(f, "no audio data"),
            Self::Resample(msg) => write!(f, "resampling failed: {msg}"),
        }
    }
}

impl std::error::Error for ImportError {}

impl From<std::io::Error> for ImportError {
    fn from(err: std::io::Error) -> Self {
        Self::Io(err)
    }
}

impl From<DecodeError> for ImportError {
    fn from(err: DecodeError) -> Self {
        Self::Decode(err)
    }
}

/// Decoded audio: interleaved samples in [-1, 1).
struct Decoded {
    sample_rate: u32,
    channels: usize,
    interleaved: Vec<f64>,
}

/// Imports the audio file at `path` and converts it to 16-bit mono at
/// `sample_rate`.
pub fn import_file(path: &Path, sample_rate: u32) -> Result<Sample, ImportError> {
    let mut hint = Hint::new();
    if let Some(ext) = path.extension().and_then(|ext| ext.to_str()) {
        hint.with_extension(ext);
    }
    import(Box::new(File::open(path)?), &hint, sample_rate)
}

/// Imports audio from `source` (format detected from its content, helped by
/// `hint`) and converts it to 16-bit mono at `sample_rate`.
pub fn import(
    source: Box<dyn MediaSource>,
    hint: &Hint,
    sample_rate: u32,
) -> Result<Sample, ImportError> {
    let decoded = decode(source, hint)?;
    let mono = downmix(&decoded.interleaved, decoded.channels);
    let mono = if decoded.sample_rate == sample_rate {
        mono
    } else {
        resample(&mono, decoded.sample_rate, sample_rate)?
    };
    Ok(Sample {
        sample_rate,
        pcm: mono.iter().map(|&x| to_i16(x)).collect(),
    })
}

fn decode(source: Box<dyn MediaSource>, hint: &Hint) -> Result<Decoded, ImportError> {
    let stream = MediaSourceStream::new(source, Default::default());
    let mut format = symphonia::default::get_probe().probe(
        hint,
        stream,
        FormatOptions::default(),
        MetadataOptions::default(),
    )?;
    let track = format
        .default_track(TrackType::Audio)
        .ok_or(ImportError::NoAudioTrack)?;
    let track_id = track.id;
    let params = track
        .codec_params
        .as_ref()
        .and_then(|params| params.audio())
        .ok_or(ImportError::NoAudioTrack)?;
    let mut decoder = symphonia::default::get_codecs()
        .make_audio_decoder(params, &AudioDecoderOptions::default())?;

    let mut interleaved = Vec::new();
    let mut packet_samples: Vec<f64> = Vec::new();
    let mut spec = None;
    while let Some(packet) = format.next_packet()? {
        if packet.track_id != track_id {
            continue;
        }
        let buf = match decoder.decode(&packet) {
            Ok(buf) => buf,
            // A corrupt packet is skipped, as in the symphonia examples.
            Err(DecodeError::DecodeError(_)) => continue,
            Err(err) => return Err(err.into()),
        };
        let buf_spec = (buf.spec().rate(), buf.spec().channels().count());
        if *spec.get_or_insert(buf_spec) != buf_spec {
            return Err(DecodeError::Unsupported("audio format changes within the stream").into());
        }
        buf.copy_to_vec_interleaved(&mut packet_samples);
        interleaved.extend_from_slice(&packet_samples);
    }

    let (sample_rate, channels) = spec.ok_or(ImportError::Empty)?;
    if channels == 0 || interleaved.is_empty() {
        return Err(ImportError::Empty);
    }
    Ok(Decoded {
        sample_rate,
        channels,
        interleaved,
    })
}

/// Mean of all channels. Mono input is returned as is.
fn downmix(interleaved: &[f64], channels: usize) -> Vec<f64> {
    if channels == 1 {
        return interleaved.to_vec();
    }
    interleaved
        .chunks_exact(channels)
        .map(|frame| frame.iter().sum::<f64>() / channels as f64)
        .collect()
}

/// Resamples a whole mono clip with rubato's synchronous FFT resampler. The
/// resampler delay is trimmed, so the output is aligned with the input.
fn resample(mono: &[f64], from: u32, to: u32) -> Result<Vec<f64>, ImportError> {
    let err = |e: &dyn std::error::Error| ImportError::Resample(e.to_string());
    let mut resampler = Fft::<f64>::new(
        from as usize,
        to as usize,
        RESAMPLER_CHUNK,
        1,
        FixedSync::Input,
    )
    .map_err(|e| err(&e))?;
    let channels = [mono];
    let input = SequentialSliceOfSlices::new(&channels, 1, mono.len()).map_err(|e| err(&e))?;
    let output = resampler
        .process_all(&input, mono.len(), None)
        .map_err(|e| err(&e))?;
    Ok(output.take_data())
}

/// Converts a sample in [-1, 1) to 16-bit with rounding and clipping. Exact
/// for values that came from 16-bit input.
fn to_i16(x: f64) -> i16 {
    (x * 32_768.0)
        .round()
        .clamp(f64::from(i16::MIN), f64::from(i16::MAX)) as i16
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn to_i16_is_exact_for_16_bit_values() {
        for s in [i16::MIN, -12_345, -1, 0, 1, 12_345, i16::MAX] {
            assert_eq!(to_i16(f64::from(s) / 32_768.0), s);
        }
    }

    #[test]
    fn to_i16_rounds_and_clips() {
        assert_eq!(to_i16(1.0), i16::MAX);
        assert_eq!(to_i16(-1.5), i16::MIN);
        assert_eq!(to_i16(0.6 / 32_768.0), 1);
        assert_eq!(to_i16(-0.6 / 32_768.0), -1);
    }

    #[test]
    fn resample_length_follows_ratio() {
        for len in [1, 10, 1023, 1024, 1025, 44_100] {
            let out = resample(&vec![0.25; len], 44_100, 31_250).unwrap();
            assert_eq!(out.len(), (len * 31_250).div_ceil(44_100), "input {len}");
        }
    }

    #[test]
    fn downmix_averages_channels() {
        assert_eq!(downmix(&[0.5, 0.0, -0.25, 0.25], 2), [0.25, 0.0]);
        assert_eq!(downmix(&[0.1, 0.2], 1), [0.1, 0.2]);
    }
}
