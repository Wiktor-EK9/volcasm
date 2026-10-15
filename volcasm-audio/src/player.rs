//! Continuous stereo playback of a render function (sequence preview).
//! Plain playback on the default output; resampling by the audio system
//! does not matter here.

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{OutputCallbackInfo, SampleFormat};

use crate::output::TransferError;

/// A running player; playback stops when it is dropped.
pub struct Player {
    _stream: cpal::Stream,
    rate: u32,
}

impl Player {
    /// Starts playing. `make` receives the output sample rate and returns
    /// the render function, which fills stereo frames in -1..1 and runs on
    /// the audio thread.
    pub fn start<F, R>(make: F) -> Result<Self, TransferError>
    where
        F: FnOnce(u32) -> R,
        R: FnMut(&mut [[f32; 2]]) + Send + 'static,
    {
        let host = crate::host()?;
        let device = host
            .default_output_device()
            .ok_or_else(|| TransferError::DeviceNotFound("default".into()))?;
        let supported = device.default_output_config()?;
        let rate = supported.sample_rate();
        let mut config = supported.config();
        // f32 is converted by the audio system as needed.
        config.channels = config.channels.max(1);
        let channels = usize::from(config.channels);
        let mut render = make(rate);
        let mut frames: Vec<[f32; 2]> = Vec::new();
        let stream = device.build_output_stream_raw(
            config,
            SampleFormat::F32,
            move |data: &mut cpal::Data, _: &OutputCallbackInfo| {
                let Some(out) = data.as_slice_mut::<f32>() else {
                    return;
                };
                let n = out.len() / channels;
                frames.resize(n, [0.0; 2]);
                render(&mut frames);
                for (dst, src) in out.chunks_exact_mut(channels).zip(&frames) {
                    match dst {
                        [mono] => *mono = (src[0] + src[1]) * 0.5,
                        [l, r, rest @ ..] => {
                            *l = src[0];
                            *r = src[1];
                            rest.fill(0.0);
                        }
                        [] => {}
                    }
                }
            },
            |_| {},
            None,
        )?;
        stream.play()?;
        Ok(Self {
            _stream: stream,
            rate,
        })
    }

    /// Output sample rate.
    pub fn rate(&self) -> u32 {
        self.rate
    }
}
