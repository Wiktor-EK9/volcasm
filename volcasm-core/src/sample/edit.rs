//! Edits on samples already in the device format (D11).

use std::ops::Range;

use super::Sample;

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

#[cfg(test)]
mod tests {
    use super::*;

    fn sample(pcm: &[i16]) -> Sample {
        Sample {
            sample_rate: 31_250,
            pcm: pcm.to_vec(),
        }
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
