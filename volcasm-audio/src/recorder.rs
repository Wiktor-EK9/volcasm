//! Recording from an audio input (e.g. the volca's output into a line
//! input) for loading into a slot.

use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::mpsc::{self, Receiver};

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{DeviceId, HostId, InputCallbackInfo};

use crate::output::{OutputInfo, TransferError};

/// Lists PipeWire audio inputs (same shape as outputs). Outputs appear too,
/// marked "(monitor)": recording one captures what the computer plays.
pub fn list_inputs() -> Result<Vec<OutputInfo>, TransferError> {
    let host = cpal::host_from_id(HostId::PipeWire)?;
    let mut inputs = Vec::new();
    for device in host.input_devices()? {
        let mut name = device.description()?.name().to_owned();
        if device.supports_output() {
            name.push_str(" (monitor)");
        }
        inputs.push(OutputInfo {
            id: device.id()?.to_string(),
            name,
        });
    }
    Ok(inputs)
}

/// Recorded audio: interleaved samples in -1..1.
#[derive(Debug, Clone, PartialEq)]
pub struct Recording {
    pub rate: u32,
    pub channels: usize,
    pub interleaved: Vec<f32>,
}

/// A running recording. Audio is collected by [`Recorder::poll`]; the
/// stream stops when the recorder is finished or dropped.
pub struct Recorder {
    _stream: cpal::Stream,
    rx: Receiver<Vec<f32>>,
    buffer: Buffer,
    /// Peak level since the last [`Recorder::level`] call, as f32 bits.
    peak: Arc<AtomicU32>,
}

impl Recorder {
    /// Starts recording from `device` (an id from [`list_inputs`]; the
    /// default input when `None`), keeping at most `max_seconds`.
    pub fn start(device: Option<&str>, max_seconds: f64) -> Result<Self, TransferError> {
        let host = cpal::host_from_id(HostId::PipeWire)?;
        let device = match device {
            Some(id) => host
                .device_by_id(&id.parse::<DeviceId>()?)
                .ok_or_else(|| TransferError::DeviceNotFound(id.to_owned()))?,
            None => host
                .default_input_device()
                .ok_or_else(|| TransferError::DeviceNotFound("default input".into()))?,
        };
        let config = device.default_input_config()?.config();
        let rate = config.sample_rate;
        let channels = usize::from(config.channels.max(1));
        let (tx, rx) = mpsc::channel();
        let peak = Arc::new(AtomicU32::new(0));
        let shared_peak = peak.clone();
        let stream = device.build_input_stream(
            config,
            move |data: &[f32], _: &InputCallbackInfo| {
                let max = data.iter().fold(0f32, |m, x| m.max(x.abs()));
                shared_peak.fetch_max(max.to_bits(), Ordering::Relaxed);
                let _ = tx.send(data.to_vec());
            },
            |_| {},
            None,
        )?;
        stream.play()?;
        Ok(Self {
            _stream: stream,
            rx,
            buffer: Buffer::new(rate, channels, max_seconds),
            peak,
        })
    }

    /// Collects the audio received so far. Call regularly.
    pub fn poll(&mut self) {
        while let Ok(chunk) = self.rx.try_recv() {
            self.buffer.push(&chunk);
        }
    }

    /// Seconds recorded.
    pub fn seconds(&self) -> f64 {
        self.buffer.seconds()
    }

    /// Whether the maximum length is reached.
    pub fn is_full(&self) -> bool {
        self.buffer.is_full()
    }

    /// Peak level (0..1) since the last call.
    pub fn level(&self) -> f32 {
        f32::from_bits(self.peak.swap(0, Ordering::Relaxed))
    }

    /// Stops and returns the recording.
    pub fn finish(mut self) -> Recording {
        self.poll();
        self.buffer.into_recording()
    }
}

/// Collected audio with a length limit (frames, not samples).
struct Buffer {
    rate: u32,
    channels: usize,
    max_samples: usize,
    samples: Vec<f32>,
}

impl Buffer {
    fn new(rate: u32, channels: usize, max_seconds: f64) -> Self {
        let max_frames = (f64::from(rate) * max_seconds.max(0.0)) as usize;
        Self {
            rate,
            channels,
            max_samples: max_frames * channels,
            samples: Vec::new(),
        }
    }

    fn push(&mut self, chunk: &[f32]) {
        let room = self.max_samples.saturating_sub(self.samples.len());
        // Whole frames only.
        let take = chunk.len().min(room) / self.channels * self.channels;
        self.samples.extend_from_slice(&chunk[..take]);
    }

    fn seconds(&self) -> f64 {
        (self.samples.len() / self.channels) as f64 / f64::from(self.rate)
    }

    fn is_full(&self) -> bool {
        self.samples.len() + self.channels > self.max_samples
    }

    fn into_recording(self) -> Recording {
        Recording {
            rate: self.rate,
            channels: self.channels,
            interleaved: self.samples,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Buffer;

    #[test]
    fn buffer_keeps_whole_frames_up_to_the_limit() {
        // 2 channels, 4 frames per second, 1 second.
        let mut b = Buffer::new(4, 2, 1.0);
        b.push(&[0.1, 0.2, 0.3]);
        assert_eq!(b.samples.len(), 2);
        b.push(&[0.0; 10]);
        assert_eq!(b.samples.len(), 8);
        assert!(b.is_full());
        assert_eq!(b.seconds(), 1.0);
        let r = b.into_recording();
        assert_eq!((r.rate, r.channels, r.interleaved.len()), (4, 2, 8));
    }
}
