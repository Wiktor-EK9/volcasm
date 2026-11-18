//! The project directory: `.vsj`, slot index, sequences and sample files.

use std::fs::File;
use std::path::{Path, PathBuf};

use volcasm_core::device::VOLCA_SAMPLE;
use volcasm_core::pattern::{self, PatternData};
use volcasm_core::sample::{Sample, wav};
use volcasm_core::syro::PATTERN_SIZE;

use crate::index::SampleIndex;
use crate::{Error, check_marker, check_size};

/// Size of the `.vsj` file (fixed buffer).
pub const VSJ_SIZE: usize = 65_535;
/// Number of sequence files.
pub const SEQUENCE_COUNT: usize = 10;

const VSJ_HEADER: &[u8; 4] = b"PRST";

/// A loaded Vosyr project.
#[derive(Debug, Clone)]
pub struct VosyrProject {
    /// Project directory.
    pub dir: PathBuf,
    /// Project name (`.vsj` file stem).
    pub name: String,
    /// The `.vsj` file as read.
    pub vsj: Vec<u8>,
    pub index: SampleIndex,
    /// Sequences 1-10 (`sequence01..10`), validated SDK pattern structures.
    pub sequences: Vec<PatternData>,
}

/// Loads the project whose `.vsj` file is at `vsj_path`. Sample audio is
/// loaded separately with [`VosyrProject::load_sample`].
pub fn load(vsj_path: &Path) -> Result<VosyrProject, Error> {
    let dir = vsj_path.parent().unwrap_or(Path::new(".")).to_path_buf();
    let name = vsj_path
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();

    let vsj = read(vsj_path)?;
    let vsj_file = format!("{name}.vsj");
    check_size(&vsj_file, &vsj, VSJ_SIZE)?;
    check_marker(&vsj_file, &vsj, 0, VSJ_HEADER)?;

    let index = SampleIndex::parse(read(&dir.join("samples/samples"))?)?;

    let sequences = (1..=SEQUENCE_COUNT)
        .map(|n| {
            let file = format!("sequences/sequence{n:02}");
            let bytes = read(&dir.join(&file))?;
            check_size(&file, &bytes, PATTERN_SIZE)?;
            let bytes: [u8; PATTERN_SIZE] = bytes.try_into().expect("size checked");
            pattern::from_bytes(&bytes).map_err(|source| Error::Pattern { file, source })
        })
        .collect::<Result<_, _>>()?;

    Ok(VosyrProject {
        dir,
        name,
        vsj,
        index,
        sequences,
    })
}

impl VosyrProject {
    /// Tempo, swing and reverb from the `.vsj` file.
    pub fn settings(&self) -> Result<crate::GlobalSettings, Error> {
        crate::settings::parse(&self.vsj)
    }

    /// Raw bytes of sequence `n` (1-10), identical to the file.
    pub fn sequence_bytes(&self, n: usize) -> [u8; PATTERN_SIZE] {
        pattern::to_bytes(&self.sequences[n - 1])
    }

    /// Path of the WAV file of `slot`.
    pub fn sample_path(&self, slot: usize) -> Result<PathBuf, Error> {
        let record = self
            .index
            .slots
            .get(slot)
            .ok_or(Error::SlotOutOfRange(slot))?;
        if !record.used {
            return Err(Error::SlotEmpty(slot));
        }
        Ok(self.dir.join("samples").join(&record.file_name))
    }

    /// Loads the sample of `slot`. Vosyr stores samples already converted,
    /// so anything other than 16-bit mono at the device rate is an error.
    pub fn load_sample(&self, slot: usize) -> Result<Sample, Error> {
        let path = self.sample_path(slot)?;
        let sample_error = |message: String| Error::Sample {
            slot,
            path: path.clone(),
            message,
        };
        let file = File::open(&path).map_err(|e| sample_error(e.to_string()))?;
        let sample = wav::read(file).map_err(|e| sample_error(e.to_string()))?;
        if sample.sample_rate != VOLCA_SAMPLE.sample_rate {
            return Err(sample_error(format!(
                "sample rate {} Hz, expected {} Hz",
                sample.sample_rate, VOLCA_SAMPLE.sample_rate
            )));
        }
        Ok(sample)
    }
}

fn read(path: &Path) -> Result<Vec<u8>, Error> {
    std::fs::read(path).map_err(|source| Error::Io {
        path: path.to_path_buf(),
        source,
    })
}
