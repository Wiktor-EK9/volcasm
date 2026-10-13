//! Edits on samples already in the device format (D11).

use std::fmt;
use std::ops::Range;

use rubato::audioadapter_buffers::direct::SequentialSliceOfSlices;
use rubato::{Async, FixedAsync, Resampler, SincInterpolationParameters};

use super::Sample;

/// Pitch range in semitones (two octaves each way).
pub const PITCH_RANGE: std::ops::RangeInclusive<i32> = -24..=24;
/// Chunk size for the pitch resampler.
const PITCH_CHUNK: usize = 1024;

/// Returns frames `range` of `sample`, or `None` if the range is empty or
/// out of bounds.
pub fn trim(sample: &Sample, range: Range<usize>) -> Option<Sample> {
    if range.is_empty() {
        return None;
    }
    Some(Sample {
        sample_rate: sample.sample_rate,
        pcm: sample.pcm.get(range)?.to_vec(),
    })
}

/// Removes silence before the first and after the last frame whose level
/// reaches `threshold` (absolute 16-bit value). A sample that never reaches
/// it is returned unchanged.
pub fn trim_silence(sample: &Sample, threshold: i16) -> Sample {
    let loud = |s: &i16| i32::from(*s).abs() >= i32::from(threshold);
    match (
        sample.pcm.iter().position(loud),
        sample.pcm.iter().rposition(loud),
    ) {
        (Some(first), Some(last)) => Sample {
            sample_rate: sample.sample_rate,
            pcm: sample.pcm[first..=last].to_vec(),
        },
        _ => sample.clone(),
    }
}

/// Scales `sample` so that its peak reaches full scale (32767), rounding to
/// the nearest value. Silent and already full-scale samples are left as is.
/// Returns the applied gain.
pub fn normalize(sample: &mut Sample) -> f64 {
    let peak = sample
        .pcm
        .iter()
        .map(|&s| i32::from(s).abs())
        .max()
        .unwrap_or(0);
    if peak == 0 || peak >= i32::from(i16::MAX) {
        return 1.0;
    }
    let gain = f64::from(i16::MAX) / f64::from(peak);
    for s in &mut sample.pcm {
        *s = (f64::from(*s) * gain)
            .round()
            .clamp(f64::from(i16::MIN), f64::from(i16::MAX)) as i16;
    }
    gain
}

/// Error changing the pitch.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PitchError {
    OutOfRange(i32),
    Resample(String),
}

impl fmt::Display for PitchError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::OutOfRange(n) => write!(
                f,
                "pitch {n} out of range {}..={} semitones",
                PITCH_RANGE.start(),
                PITCH_RANGE.end()
            ),
            Self::Resample(msg) => write!(f, "resampling failed: {msg}"),
        }
    }
}

impl std::error::Error for PitchError {}

/// Length after shifting `frames` by `semitones`: playback runs
/// `2^(semitones/12)` times faster, so the sample gets that much shorter.
pub fn pitched_frames(frames: usize, semitones: i32) -> usize {
    ((frames as f64 / 2f64.powf(f64::from(semitones) / 12.0)).ceil() as usize).max(1)
}

/// Shifts the pitch by `semitones` by resampling (varispeed): higher means
/// shorter. The sample rate stays the same. 0 returns the sample unchanged.
pub fn pitch(sample: &Sample, semitones: i32) -> Result<Sample, PitchError> {
    if !PITCH_RANGE.contains(&semitones) {
        return Err(PitchError::OutOfRange(semitones));
    }
    if semitones == 0 || sample.pcm.is_empty() {
        return Ok(sample.clone());
    }
    let err = |e: &dyn std::error::Error| PitchError::Resample(e.to_string());
    let ratio = 1.0 / 2f64.powf(f64::from(semitones) / 12.0);
    let mut resampler = Async::<f64>::new_sinc(
        ratio,
        1.0,
        &SincInterpolationParameters::default(),
        PITCH_CHUNK,
        1,
        FixedAsync::Input,
    )
    .map_err(|e| err(&e))?;
    let input: Vec<f64> = sample
        .pcm
        .iter()
        .map(|&s| f64::from(s) / 32_768.0)
        .collect();
    let channels = [input.as_slice()];
    let adapter = SequentialSliceOfSlices::new(&channels, 1, input.len()).map_err(|e| err(&e))?;
    let mut output = resampler
        .process_all(&adapter, input.len(), None)
        .map_err(|e| err(&e))?
        .take_data();
    output.resize(pitched_frames(sample.pcm.len(), semitones), 0.0);
    Ok(Sample {
        sample_rate: sample.sample_rate,
        pcm: output
            .iter()
            .map(|&x| {
                (x * 32_768.0)
                    .round()
                    .clamp(f64::from(i16::MIN), f64::from(i16::MAX)) as i16
            })
            .collect(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample(pcm: &[i16]) -> Sample {
        Sample {
            sample_rate: 31_250,
            pcm: pcm.to_vec(),
        }
    }

    /// 0.4 s of a sine at `freq` Hz, amplitude 0.5.
    fn sine(freq: f64) -> Sample {
        Sample {
            sample_rate: 31_250,
            pcm: (0..12_500)
                .map(|i| {
                    (0.5 * (2.0 * std::f64::consts::PI * freq * f64::from(i) / 31_250.0).sin()
                        * 32_768.0)
                        .round() as i16
                })
                .collect(),
        }
    }

    /// Peak and zero-crossing frequency of the middle half.
    fn measure(sample: &Sample) -> (f64, f64) {
        let n = sample.pcm.len();
        let mid = &sample.pcm[n / 4..3 * n / 4];
        let peak = mid.iter().map(|&s| f64::from(s).abs()).fold(0.0, f64::max);
        let crossings = mid.windows(2).filter(|w| (w[0] < 0) != (w[1] < 0)).count();
        (peak, crossings as f64 / 2.0 / (mid.len() as f64 / 31_250.0))
    }

    #[test]
    fn pitch_changes_frequency_and_length() {
        for (semitones, freq) in [
            (12, 2000.0),
            (-12, 500.0),
            (7, 1000.0 * 2f64.powf(7.0 / 12.0)),
        ] {
            let out = pitch(&sine(1000.0), semitones).unwrap();
            assert_eq!(out.sample_rate, 31_250);
            assert_eq!(out.frames(), pitched_frames(12_500, semitones));
            let (peak, measured) = measure(&out);
            assert!(
                (measured - freq).abs() < freq * 0.01,
                "{semitones}: {measured} Hz"
            );
            assert!(
                (peak - 16_384.0).abs() < 16_384.0 * 0.01,
                "{semitones}: peak {peak}"
            );
        }
        assert_eq!(pitched_frames(12_500, 12), 6250);
        assert_eq!(pitched_frames(12_500, -12), 25_000);
    }

    #[test]
    fn pitch_zero_is_identity_and_range_is_checked() {
        let s = sine(440.0);
        assert_eq!(pitch(&s, 0).unwrap(), s);
        assert_eq!(pitch(&s, 25), Err(PitchError::OutOfRange(25)));
        assert_eq!(pitch(&s, -25), Err(PitchError::OutOfRange(-25)));
    }

    #[test]
    fn pitch_of_tiny_samples() {
        let s = sample(&[1000, -1000, 500]);
        assert!(pitch(&s, 24).unwrap().frames() >= 1);
        assert_eq!(pitch(&s, -24).unwrap().frames(), 12);
    }

    #[test]
    fn trim_silence_keeps_the_loud_part() {
        let s = sample(&[0, 3, -2, 500, 10, -800, 4, 0]);
        assert_eq!(trim_silence(&s, 100).pcm, [500, 10, -800]);
        assert_eq!(trim_silence(&s, 1000), s);
    }

    #[test]
    fn trim_takes_range() {
        let s = sample(&[1, 2, 3, 4, 5]);
        assert_eq!(trim(&s, 1..4).unwrap().pcm, [2, 3, 4]);
        assert_eq!(trim(&s, 0..5).unwrap(), s);
    }

    #[test]
    fn trim_rejects_empty_or_out_of_bounds() {
        let s = sample(&[1, 2, 3]);
        assert_eq!(trim(&s, 2..2), None);
        assert_eq!(trim(&s, 1..4), None);
    }

    #[test]
    fn normalize_scales_peak_to_full_scale() {
        let mut s = sample(&[0, 1000, -2000, 500]);
        let gain = normalize(&mut s);
        assert_eq!(gain, 32767.0 / 2000.0);
        assert_eq!(s.pcm, [0, 16384, -32767, 8192]);
    }

    #[test]
    fn normalize_keeps_silence_and_full_scale() {
        let mut silent = sample(&[0, 0]);
        assert_eq!(normalize(&mut silent), 1.0);
        assert_eq!(silent.pcm, [0, 0]);
        let mut full = sample(&[-32768, 100]);
        assert_eq!(normalize(&mut full), 1.0);
        assert_eq!(full.pcm, [-32768, 100]);
    }
}
