//! Listening to a sample on the computer. Plain playback through the
//! default output; no verification, since resampling does not matter here.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{BufferSize, OutputCallbackInfo, StreamConfig};

use crate::output::TransferError;

/// A playing preview; playback stops when this is dropped.
pub struct Preview {
    _stream: cpal::Stream,
    position: Arc<AtomicUsize>,
    finished: Arc<AtomicBool>,
    frames: usize,
}

impl Preview {
    /// Whether the whole sample has been played.
    pub fn finished(&self) -> bool {
        self.finished.load(Ordering::Relaxed)
    }

    /// Frames played so far and in total.
    pub fn position(&self) -> (usize, usize) {
        (self.position.load(Ordering::Relaxed), self.frames)
    }
}

/// Plays 16-bit mono `pcm` at `sample_rate` on the default output.
pub fn preview(pcm: Arc<[i16]>, sample_rate: u32) -> Result<Preview, TransferError> {
    let host = crate::host()?;
    let device = host
        .default_output_device()
        .ok_or_else(|| TransferError::DeviceNotFound("default".into()))?;
    let config = StreamConfig {
        channels: 1,
        sample_rate,
        buffer_size: BufferSize::Default,
    };
    let position = Arc::new(AtomicUsize::new(0));
    let finished = Arc::new(AtomicBool::new(false));
    let frames = pcm.len();
    let stream = device.build_output_stream(
        config,
        {
            let position = position.clone();
            let finished = finished.clone();
            move |out: &mut [i16], _: &OutputCallbackInfo| {
                let start = position.load(Ordering::Relaxed);
                let n = fill(out, &pcm, start);
                position.store(start + n, Ordering::Relaxed);
                if start + n >= pcm.len() {
                    finished.store(true, Ordering::Relaxed);
                }
            }
        },
        |_| {},
        None,
    )?;
    stream.play()?;
    Ok(Preview {
        _stream: stream,
        position,
        finished,
        frames,
    })
}

/// Copies `pcm[start..]` into `out` and fills the rest with silence.
/// Returns the number of samples copied.
fn fill(out: &mut [i16], pcm: &[i16], start: usize) -> usize {
    let rest = pcm.get(start..).unwrap_or(&[]);
    let n = rest.len().min(out.len());
    out[..n].copy_from_slice(&rest[..n]);
    out[n..].fill(0);
    n
}

#[cfg(test)]
mod tests {
    use super::fill;

    #[test]
    fn fill_copies_then_silence() {
        let pcm = [1, 2, 3];
        let mut out = [9; 2];
        assert_eq!(fill(&mut out, &pcm, 0), 2);
        assert_eq!(out, [1, 2]);
        assert_eq!(fill(&mut out, &pcm, 2), 1);
        assert_eq!(out, [3, 0]);
        assert_eq!(fill(&mut out, &pcm, 5), 0);
        assert_eq!(out, [0, 0]);
    }
}
