//! Audio input/output for Volcasm.
//!
//! Output goes through cpal: PipeWire on Linux, WASAPI on Windows. Before
//! any SYRO data is sent, the audio path is checked so that the stream is
//! not altered on the way. On Windows a device mix rate other than
//! 44.1 kHz is handled by resampling the stream in Volcasm.

pub mod check;
pub mod graph;
pub mod output;
pub mod player;
pub mod preview;
pub mod recorder;

pub use output::{Canceller, OutputInfo, Phase, Report, Transfer, TransferError, TransferOptions};
pub use output::{list_outputs, start};
pub use player::Player;
pub use preview::{Preview, preview};
pub use recorder::{Recorder, Recording, list_inputs};

/// The audio host: PipeWire on Linux (its graph is checked before a
/// transfer), the system default elsewhere.
fn host() -> Result<cpal::Host, TransferError> {
    #[cfg(target_os = "linux")]
    return Ok(cpal::host_from_id(cpal::HostId::PipeWire)?);
    #[cfg(not(target_os = "linux"))]
    Ok(cpal::default_host())
}
