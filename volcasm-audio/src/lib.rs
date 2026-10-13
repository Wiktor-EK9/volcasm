//! Audio input/output for Volcasm.
//!
//! Output goes through PipeWire (cpal). Before any SYRO data is sent, the
//! PipeWire graph is checked so that the stream reaches the device at
//! 44.1 kHz without resampling (D4, D18).

pub mod check;
pub mod graph;
pub mod output;
pub mod preview;

pub use output::{Canceller, OutputInfo, Phase, Report, Transfer, TransferError, TransferOptions};
pub use output::{list_outputs, start};
pub use preview::{Preview, preview};
