//! Playback of a SYRO stream through cpal, with verification of the audio
//! path before any data is sent.
//!
//! Sequence: stream start with silence (pre-roll) while the audio path is
//! verified, then the stream data, then silence until the device buffer has
//! played out. On Linux the running PipeWire graph is checked (device rate,
//! clock settings, volumes); the device is known only once the stream is
//! linked, so all checks happen during the pre-roll. On other systems the
//! stream is resampled to the device mix rate when it is not 44.1 kHz (with
//! a warning), so that the system does not convert it, and the mix rate is
//! checked again during the pre-roll and after the data.

use std::fmt;
use std::sync::atomic::{AtomicBool, AtomicU8, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{BufferSize, DeviceId, ErrorKind, OutputCallbackInfo, StreamConfig};

use volcasm_core::syro::ResampleError;

use crate::check::{self, Problem, Warning};
use crate::graph::GraphError;

/// How long the pre-roll waits for the audio path to verify before giving up.
const VERIFY_TIMEOUT: Duration = Duration::from_secs(3);
/// Interval between checks during the pre-roll.
const VERIFY_INTERVAL: Duration = Duration::from_millis(100);
/// Silence after the device delay, before the stream is closed.
const TAIL_MARGIN: Duration = Duration::from_millis(250);
/// Upper bound for the measured device delay.
const MAX_DELAY: Duration = Duration::from_secs(2);
/// Polling interval while the data is playing.
const POLL_INTERVAL: Duration = Duration::from_millis(20);

/// An audio output that can be passed to [`TransferOptions`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutputInfo {
    /// cpal device id, e.g. `pipewire:alsa_output....` on Linux.
    pub id: String,
    pub name: String,
}

/// Lists audio outputs.
pub fn list_outputs() -> Result<Vec<OutputInfo>, TransferError> {
    let host = crate::host()?;
    let mut outputs = Vec::new();
    for device in host.output_devices()? {
        outputs.push(OutputInfo {
            id: device.id()?.to_string(),
            name: device.description()?.name().to_owned(),
        });
    }
    Ok(outputs)
}

/// Transfer settings.
#[derive(Debug, Clone, Default)]
pub struct TransferOptions {
    /// Output id from [`list_outputs`]; the default output when `None`.
    pub device: Option<String>,
}

/// Transfer phase.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    /// Checking the audio path; only silence is played.
    Checking,
    /// Playing the stream data.
    Sending,
    /// Data sent; waiting for the device buffer to play out.
    Draining,
    /// Finished, refused, failed or cancelled; see [`Transfer::wait`].
    Done,
}

/// Outcome of a completed transfer.
#[derive(Debug, Clone, PartialEq)]
pub struct Report {
    pub warnings: Vec<Warning>,
    /// Buffer underruns while playing; any underrun corrupts the stream.
    pub xruns: usize,
    pub cancelled: bool,
}

/// Error that stopped a transfer.
#[derive(Debug)]
pub enum TransferError {
    /// The audio path would alter the stream; nothing was sent.
    Refused(Problem),
    /// The device rate changed while the data was playing.
    RateChanged(Problem),
    Graph(GraphError),
    Audio(cpal::Error),
    /// The requested output does not exist.
    DeviceNotFound(String),
    Resample(ResampleError),
}

impl fmt::Display for TransferError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Refused(problem) => write!(f, "transfer refused, nothing was sent: {problem}"),
            Self::RateChanged(problem) => {
                write!(f, "audio path changed during the transfer: {problem}")
            }
            Self::Graph(err) => write!(f, "{err}"),
            Self::Audio(err) => write!(f, "audio error: {err}"),
            Self::DeviceNotFound(id) => write!(f, "audio output not found: {id}"),
            Self::Resample(err) => write!(f, "{err}"),
        }
    }
}

impl std::error::Error for TransferError {}

impl From<cpal::Error> for TransferError {
    fn from(err: cpal::Error) -> Self {
        Self::Audio(err)
    }
}

impl From<ResampleError> for TransferError {
    fn from(err: ResampleError) -> Self {
        Self::Resample(err)
    }
}

impl From<GraphError> for TransferError {
    fn from(err: GraphError) -> Self {
        Self::Graph(err)
    }
}

/// State shared between the control thread, the audio callback and the
/// [`Transfer`] handle.
#[derive(Default)]
struct Shared {
    /// Set once the audio path is verified: the callback starts sending data.
    release: AtomicBool,
    cancel: AtomicBool,
    /// Data frames handed to the device so far.
    written: AtomicUsize,
    /// Data frames in total (changes if the stream is resampled).
    total: AtomicUsize,
    /// Set by the callback after the last data frame (or on cancel).
    done: AtomicBool,
    /// Largest observed callback-to-playback delay, in nanoseconds.
    delay_ns: AtomicU64,
    xruns: AtomicUsize,
    phase: AtomicU8,
    /// First stream error other than an underrun.
    error: Mutex<Option<cpal::Error>>,
    warnings: Mutex<Vec<Warning>>,
}

/// A running transfer.
pub struct Transfer {
    shared: Arc<Shared>,
    thread: JoinHandle<Result<Report, TransferError>>,
}

impl Transfer {
    /// Data frames sent so far and in total, at the output rate.
    pub fn progress(&self) -> (usize, usize) {
        (
            self.shared.written.load(Ordering::Relaxed),
            self.shared.total.load(Ordering::Relaxed),
        )
    }

    pub fn phase(&self) -> Phase {
        match self.shared.phase.load(Ordering::Relaxed) {
            0 => Phase::Checking,
            1 => Phase::Sending,
            2 => Phase::Draining,
            _ => Phase::Done,
        }
    }

    /// Warnings found when the graph was verified.
    pub fn warnings(&self) -> Vec<Warning> {
        self.shared.warnings.lock().unwrap().clone()
    }

    /// Stops sending data; the rest of the stream is replaced by silence.
    pub fn cancel(&self) {
        self.shared.cancel.store(true, Ordering::Relaxed);
    }

    /// A handle that cancels this transfer from another thread.
    pub fn canceller(&self) -> Canceller {
        Canceller(self.shared.clone())
    }

    /// Waits for the transfer to end.
    pub fn wait(self) -> Result<Report, TransferError> {
        self.thread.join().expect("transfer thread panicked")
    }
}

/// Cancels a [`Transfer`] from another thread.
#[derive(Clone)]
pub struct Canceller(Arc<Shared>);

impl Canceller {
    pub fn cancel(&self) {
        self.0.cancel.store(true, Ordering::Relaxed);
    }
}

/// Starts playing `frames` (a SYRO stream, 44.1 kHz stereo) in a background
/// thread.
pub fn start(frames: Vec<[i16; 2]>, options: TransferOptions) -> Transfer {
    let shared = Arc::new(Shared::default());
    shared.total.store(frames.len(), Ordering::Relaxed);
    let thread = thread::spawn({
        let shared = shared.clone();
        move || {
            let result = run(frames, &options, &shared);
            set_phase(&shared, Phase::Done);
            result
        }
    });
    Transfer { shared, thread }
}

fn set_phase(shared: &Shared, phase: Phase) {
    shared.phase.store(phase as u8, Ordering::Relaxed);
}

fn run(
    frames: Vec<[i16; 2]>,
    options: &TransferOptions,
    shared: &Arc<Shared>,
) -> Result<Report, TransferError> {
    set_phase(shared, Phase::Checking);

    let host = crate::host()?;
    let device = match &options.device {
        Some(id) => host
            .device_by_id(&id.parse::<DeviceId>()?)
            .ok_or_else(|| TransferError::DeviceNotFound(id.clone()))?,
        None => host
            .default_output_device()
            .ok_or_else(|| TransferError::DeviceNotFound("default".into()))?,
    };
    let rate = output_rate(&device)?;
    let frames = volcasm_core::syro::resample_stream(&frames, rate)?;
    shared.total.store(frames.len(), Ordering::Relaxed);
    let config = StreamConfig {
        channels: 2,
        sample_rate: rate,
        buffer_size: BufferSize::Default,
    };
    let mut feeder = Feeder::new(frames, shared.clone());
    let stream = device.build_output_stream(
        config,
        {
            let shared = shared.clone();
            move |data: &mut [i16], info: &OutputCallbackInfo| {
                let ts = info.timestamp();
                if let Some(delay) = ts.playback.checked_duration_since(ts.callback) {
                    shared
                        .delay_ns
                        .fetch_max(delay.as_nanos() as u64, Ordering::Relaxed);
                }
                feeder.fill(data);
            }
        },
        {
            let shared = shared.clone();
            move |err: cpal::Error| {
                if err.kind() == ErrorKind::Xrun {
                    shared.xruns.fetch_add(1, Ordering::Relaxed);
                } else {
                    shared.error.lock().unwrap().get_or_insert(err);
                }
            }
        },
        None,
    )?;
    stream.play()?;

    // Pre-roll: only silence is played until the audio path checks out.
    let deadline = Instant::now() + VERIFY_TIMEOUT;
    let warnings = loop {
        take_stream_error(shared)?;
        match verify(&device, rate)? {
            Ok(warnings) => break warnings,
            Err(problem) if problem.is_transient() && Instant::now() < deadline => {
                thread::sleep(VERIFY_INTERVAL);
            }
            Err(problem) => return Err(TransferError::Refused(problem)),
        }
    };
    *shared.warnings.lock().unwrap() = warnings.clone();
    if shared.cancel.load(Ordering::Relaxed) {
        return Ok(report(shared, warnings, true));
    }

    set_phase(shared, Phase::Sending);
    shared.release.store(true, Ordering::Relaxed);
    while !shared.done.load(Ordering::Relaxed) {
        take_stream_error(shared)?;
        thread::sleep(POLL_INTERVAL);
    }

    // The rate must not have changed while the data was playing. Checked
    // before the stream is closed: its PipeWire node goes away with it.
    set_phase(shared, Phase::Draining);
    let after = verify(&device, rate)?;
    let delay = Duration::from_nanos(shared.delay_ns.load(Ordering::Relaxed)).min(MAX_DELAY);
    thread::sleep(delay + TAIL_MARGIN);
    drop(stream);
    take_stream_error(shared)?;
    after.map_err(TransferError::RateChanged)?;

    let cancelled = shared.cancel.load(Ordering::Relaxed);
    Ok(report(shared, warnings, cancelled))
}

/// Rate of the stream sent to `device`. On Linux always 44.1 kHz: the
/// PipeWire graph is checked and its resampling is accepted only on devices
/// without 44.1 kHz.
#[cfg(target_os = "linux")]
fn output_rate(_device: &cpal::Device) -> Result<u32, TransferError> {
    Ok(check::STREAM_RATE)
}

/// Rate of the stream sent to `device`: the shared-mode mix rate, so that
/// WASAPI does not convert the stream.
#[cfg(not(target_os = "linux"))]
fn output_rate(device: &cpal::Device) -> Result<u32, TransferError> {
    Ok(device.default_output_config()?.sample_rate())
}

/// Checks the PipeWire graph around our running stream.
#[cfg(target_os = "linux")]
fn verify(
    _device: &cpal::Device,
    _rate: u32,
) -> Result<Result<Vec<Warning>, Problem>, TransferError> {
    let group = format!("cpal-{}", std::process::id());
    let is_ours = |node: &crate::graph::NodeState| {
        node.prop("node.group") == Some(group.as_str())
            && node.prop("media.class") == Some("Stream/Output/Audio")
    };
    Ok(check::verify(&crate::graph::snapshot()?, is_ours))
}

/// Checks the shared-mode mix rate of the device against the `rate` the
/// stream was prepared for: WASAPI converts any other rate without notice.
#[cfg(not(target_os = "linux"))]
fn verify(
    device: &cpal::Device,
    rate: u32,
) -> Result<Result<Vec<Warning>, Problem>, TransferError> {
    let name = device.description()?.name().to_owned();
    let mix_rate = device.default_output_config()?.sample_rate();
    Ok(check::verify_mix_rate(&name, mix_rate, rate))
}

fn report(shared: &Shared, warnings: Vec<Warning>, cancelled: bool) -> Report {
    Report {
        warnings,
        xruns: shared.xruns.load(Ordering::Relaxed),
        cancelled,
    }
}

fn take_stream_error(shared: &Shared) -> Result<(), TransferError> {
    match shared.error.lock().unwrap().take() {
        Some(err) => Err(err.into()),
        None => Ok(()),
    }
}

/// Fills the device buffer: silence until released, then the stream data,
/// then silence. Cancelling switches to silence at once.
struct Feeder {
    frames: Vec<[i16; 2]>,
    pos: usize,
    shared: Arc<Shared>,
}

impl Feeder {
    fn new(frames: Vec<[i16; 2]>, shared: Arc<Shared>) -> Self {
        Self {
            frames,
            pos: 0,
            shared,
        }
    }

    /// Fills interleaved stereo `out`.
    fn fill(&mut self, out: &mut [i16]) {
        let released = self.shared.release.load(Ordering::Relaxed);
        let cancelled = self.shared.cancel.load(Ordering::Relaxed);
        let mut written = 0;
        if released && !cancelled {
            let remaining = &self.frames[self.pos..];
            for (dst, src) in out.as_chunks_mut::<2>().0.iter_mut().zip(remaining) {
                *dst = *src;
                written += 1;
            }
            self.pos += written;
            self.shared.written.store(self.pos, Ordering::Relaxed);
        }
        out[written * 2..].fill(0);
        if released && (cancelled || self.pos == self.frames.len()) {
            self.shared.done.store(true, Ordering::Relaxed);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn feeder(frames: &[[i16; 2]]) -> (Feeder, Arc<Shared>) {
        let shared = Arc::new(Shared::default());
        (Feeder::new(frames.to_vec(), shared.clone()), shared)
    }

    const DATA: [[i16; 2]; 5] = [[1, -1], [2, -2], [3, -3], [4, -4], [5, -5]];

    #[test]
    fn silence_until_released() {
        let (mut f, shared) = feeder(&DATA);
        let mut buf = [7i16; 6];
        f.fill(&mut buf);
        assert_eq!(buf, [0; 6]);
        assert_eq!(shared.written.load(Ordering::Relaxed), 0);
        assert!(!shared.done.load(Ordering::Relaxed));
    }

    #[test]
    fn data_then_silence() {
        let (mut f, shared) = feeder(&DATA);
        shared.release.store(true, Ordering::Relaxed);
        let mut buf = [7i16; 6];
        f.fill(&mut buf);
        assert_eq!(buf, [1, -1, 2, -2, 3, -3]);
        assert!(!shared.done.load(Ordering::Relaxed));
        f.fill(&mut buf);
        assert_eq!(buf, [4, -4, 5, -5, 0, 0]);
        assert_eq!(shared.written.load(Ordering::Relaxed), 5);
        assert!(shared.done.load(Ordering::Relaxed));
        f.fill(&mut buf);
        assert_eq!(buf, [0; 6]);
    }

    #[test]
    fn cancel_switches_to_silence() {
        let (mut f, shared) = feeder(&DATA);
        shared.release.store(true, Ordering::Relaxed);
        let mut buf = [7i16; 4];
        f.fill(&mut buf);
        shared.cancel.store(true, Ordering::Relaxed);
        f.fill(&mut buf);
        assert_eq!(buf, [0; 4]);
        assert_eq!(shared.written.load(Ordering::Relaxed), 2);
        assert!(shared.done.load(Ordering::Relaxed));
    }
}
