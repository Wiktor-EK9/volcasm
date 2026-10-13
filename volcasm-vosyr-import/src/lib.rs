//! Read-only loader for Vosyr `.vsj` projects (clean-room, D9).
//!
//! A project is a directory: `<name>.vsj`, `samples/samples` (slot index),
//! `samples/sample<number>` (WAV files) and `sequences/sequence01..10` (raw
//! SDK pattern structures). Only fields confirmed in `docs/vsj-format.md`
//! are interpreted; everything else is kept byte for byte. The `.vsj` file
//! itself has a variable layout and is kept raw (stage 11).

pub mod convert;
pub mod index;
pub mod project;

use std::fmt;
use std::path::PathBuf;

use volcasm_core::pattern::PatternError;

pub use index::{SampleIndex, SlotRecord};
pub use project::{SEQUENCE_COUNT, VSJ_SIZE, VosyrProject, load};

/// Error while loading a Vosyr project.
#[derive(Debug)]
pub enum Error {
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
    /// A file has an unexpected size.
    Size {
        file: String,
        expected: usize,
        actual: usize,
    },
    /// A file lacks an expected marker.
    Marker {
        file: String,
        offset: usize,
        expected: [u8; 4],
    },
    /// A sequence file is not a valid pattern structure.
    Pattern { file: String, source: PatternError },
    /// The slot number is not 0-99.
    SlotOutOfRange(usize),
    /// The slot holds no sample.
    SlotEmpty(usize),
    /// The sample file cannot be read or is not in the device format.
    Sample {
        slot: usize,
        path: PathBuf,
        message: String,
    },
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io { path, source } => write!(f, "{}: {source}", path.display()),
            Self::Size {
                file,
                expected,
                actual,
            } => write!(f, "{file}: size {actual} B, expected {expected} B"),
            Self::Marker {
                file,
                offset,
                expected,
            } => write!(
                f,
                "{file}: missing marker {:?} at offset {offset}",
                String::from_utf8_lossy(expected)
            ),
            Self::Pattern { file, source } => write!(f, "{file}: {source}"),
            Self::SlotOutOfRange(slot) => write!(f, "slot {slot} out of range 0-99"),
            Self::SlotEmpty(slot) => write!(f, "slot {slot} is empty"),
            Self::Sample {
                slot,
                path,
                message,
            } => write!(f, "slot {slot} ({}): {message}", path.display()),
        }
    }
}

impl std::error::Error for Error {}

fn check_size(file: &str, data: &[u8], expected: usize) -> Result<(), Error> {
    if data.len() == expected {
        Ok(())
    } else {
        Err(Error::Size {
            file: file.to_owned(),
            expected,
            actual: data.len(),
        })
    }
}

fn check_marker(file: &str, data: &[u8], offset: usize, expected: &[u8; 4]) -> Result<(), Error> {
    if data.get(offset..offset + 4) == Some(expected) {
        Ok(())
    } else {
        Err(Error::Marker {
            file: file.to_owned(),
            offset,
            expected: *expected,
        })
    }
}
