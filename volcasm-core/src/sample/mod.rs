//! Samples in the device format (16-bit mono at the native rate) and their
//! processing: import from audio files, trimming, normalization.

pub mod edit;
pub mod import;
pub mod wav;

/// A sample in the device format: 16-bit mono PCM at `sample_rate`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Sample {
    pub sample_rate: u32,
    pub pcm: Vec<i16>,
}

impl Sample {
    /// Number of frames.
    pub fn frames(&self) -> usize {
        self.pcm.len()
    }

    /// Duration in seconds.
    pub fn duration(&self) -> f64 {
        self.pcm.len() as f64 / f64::from(self.sample_rate)
    }
}
