//! The `.volcasm` project file: a ZIP archive with `manifest.json`,
//! `samples/NN.wav` (16-bit mono, device rate) and `patterns/NN.bin` (raw
//! 2624-byte SDK pattern structures).

use std::fmt;
use std::fs::File;
use std::io::{BufReader, BufWriter, Cursor, Read, Seek, Write};
use std::path::Path;

use serde::{Deserialize, Serialize};
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, DateTime, ZipArchive, ZipWriter};

use super::{Project, ProjectError, Slot};
use crate::device;
use crate::pattern::{Pattern, PatternError};
use crate::sample::wav;
use crate::syro::PATTERN_SIZE;

/// Value of the `format` field.
pub const FORMAT: &str = "volcasm";
/// Current format version. Readers reject newer versions.
pub const VERSION: u32 = 1;

const MANIFEST: &str = "manifest.json";
/// Upper bound for one archive member, against malformed or hostile files.
/// Larger than any sample the device can hold.
const MAX_MEMBER_SIZE: u64 = 64 * 1024 * 1024;

#[derive(Debug, Serialize, Deserialize)]
struct Manifest {
    format: String,
    version: u32,
    device: String,
    name: String,
    author: Option<String>,
    license: Option<String>,
    description: Option<String>,
    /// Preview tempo in BPM; optional, added after version 1 was defined.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    tempo: Option<u16>,
    /// Swing and reverb on Vosyr's scales, kept from imports; optional.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    swing: Option<u8>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    reverb: Option<u8>,
    slots: Vec<SlotEntry>,
    patterns: Vec<PatternEntry>,
}

#[derive(Debug, Serialize, Deserialize)]
struct SlotEntry {
    slot: usize,
    name: String,
    file: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct PatternEntry {
    /// Sequence number, 1-based as on the device.
    number: usize,
    file: String,
}

/// Error reading or writing a `.volcasm` file.
#[derive(Debug)]
pub enum FileError {
    Io(std::io::Error),
    Zip(zip::result::ZipError),
    Manifest(serde_json::Error),
    /// Not a Volcasm file (`format` field).
    NotVolcasm(String),
    /// Written by a newer Volcasm.
    UnsupportedVersion(u32),
    UnknownDevice(String),
    /// An archive member is missing or too large.
    Member {
        name: String,
        message: String,
    },
    Sample {
        slot: usize,
        message: String,
    },
    Pattern {
        number: usize,
        message: String,
    },
    /// A slot or pattern appears twice in the manifest.
    Duplicate(String),
    Project(ProjectError),
}

impl fmt::Display for FileError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(err) => write!(f, "{err}"),
            Self::Zip(err) => write!(f, "archive: {err}"),
            Self::Manifest(err) => write!(f, "manifest: {err}"),
            Self::NotVolcasm(format) => write!(f, "not a Volcasm project (format {format:?})"),
            Self::UnsupportedVersion(v) => write!(
                f,
                "format version {v} is newer than supported ({VERSION}); update Volcasm"
            ),
            Self::UnknownDevice(id) => write!(f, "unknown device {id:?}"),
            Self::Member { name, message } => write!(f, "{name}: {message}"),
            Self::Sample { slot, message } => write!(f, "slot {slot}: {message}"),
            Self::Pattern { number, message } => write!(f, "pattern {number}: {message}"),
            Self::Duplicate(what) => write!(f, "{what} appears twice"),
            Self::Project(err) => write!(f, "{err}"),
        }
    }
}

impl std::error::Error for FileError {}

impl From<std::io::Error> for FileError {
    fn from(err: std::io::Error) -> Self {
        Self::Io(err)
    }
}

impl From<zip::result::ZipError> for FileError {
    fn from(err: zip::result::ZipError) -> Self {
        Self::Zip(err)
    }
}

fn sample_file(slot: usize) -> String {
    format!("samples/{slot:02}.wav")
}

fn pattern_file(number: usize) -> String {
    format!("patterns/{number:02}.bin")
}

/// Writes `project` as a `.volcasm` archive.
pub fn write(project: &Project, writer: impl Write + Seek) -> Result<(), FileError> {
    let manifest = Manifest {
        format: FORMAT.into(),
        version: VERSION,
        device: project.device.id.into(),
        name: project.name.clone(),
        author: project.author.clone(),
        license: project.license.clone(),
        description: project.description.clone(),
        tempo: Some(project.tempo),
        swing: project.swing,
        reverb: project.reverb,
        slots: project
            .slots()
            .map(|(slot, s)| SlotEntry {
                slot,
                name: s.name.clone(),
                file: sample_file(slot),
            })
            .collect(),
        patterns: (1..=project.patterns.len())
            .map(|number| PatternEntry {
                number,
                file: pattern_file(number),
            })
            .collect(),
    };

    // Fixed timestamps keep the output reproducible.
    let options = SimpleFileOptions::default()
        .compression_method(CompressionMethod::Deflated)
        .last_modified_time(DateTime::default());
    let mut zip = ZipWriter::new(writer);
    zip.start_file(MANIFEST, options)?;
    let mut json = serde_json::to_vec_pretty(&manifest).map_err(FileError::Manifest)?;
    json.push(b'\n');
    zip.write_all(&json)?;
    for (slot, s) in project.slots() {
        let mut wav_bytes = Cursor::new(Vec::new());
        wav::write(&s.sample, &mut wav_bytes).map_err(|e| FileError::Sample {
            slot,
            message: e.to_string(),
        })?;
        zip.start_file(sample_file(slot), options)?;
        zip.write_all(wav_bytes.get_ref())?;
    }
    for (i, pattern) in project.patterns.iter().enumerate() {
        zip.start_file(pattern_file(i + 1), options)?;
        zip.write_all(&pattern.to_bytes())?;
    }
    zip.finish()?;
    Ok(())
}

/// Reads a `.volcasm` archive.
pub fn read(reader: impl Read + Seek) -> Result<Project, FileError> {
    let mut zip = ZipArchive::new(reader)?;
    let manifest: Manifest =
        serde_json::from_slice(&member(&mut zip, MANIFEST)?).map_err(FileError::Manifest)?;
    if manifest.format != FORMAT {
        return Err(FileError::NotVolcasm(manifest.format));
    }
    if manifest.version > VERSION {
        return Err(FileError::UnsupportedVersion(manifest.version));
    }
    let device =
        device::profile(&manifest.device).ok_or(FileError::UnknownDevice(manifest.device))?;

    let mut project = Project::new(device, manifest.name);
    project.author = manifest.author;
    project.license = manifest.license;
    project.description = manifest.description;
    project.swing = manifest.swing;
    project.reverb = manifest.reverb;
    if let Some(tempo) = manifest.tempo {
        project.tempo = tempo.clamp(*super::TEMPO_RANGE.start(), *super::TEMPO_RANGE.end());
    }

    for entry in manifest.slots {
        if project.slot(entry.slot).is_some() {
            return Err(FileError::Duplicate(format!("slot {}", entry.slot)));
        }
        let bytes = member(&mut zip, &entry.file)?;
        let sample = wav::read(Cursor::new(bytes)).map_err(|e| FileError::Sample {
            slot: entry.slot,
            message: e.to_string(),
        })?;
        let slot = Slot {
            name: entry.name,
            sample,
        };
        project
            .set_slot(entry.slot, slot)
            .map_err(FileError::Project)?;
    }

    let mut seen = vec![false; project.patterns.len()];
    for entry in manifest.patterns {
        let pattern_error = |message: String| FileError::Pattern {
            number: entry.number,
            message,
        };
        let index = entry
            .number
            .checked_sub(1)
            .filter(|&i| i < seen.len())
            .ok_or_else(|| pattern_error(format!("number out of range 1-{}", seen.len())))?;
        if std::mem::replace(&mut seen[index], true) {
            return Err(FileError::Duplicate(format!("pattern {}", entry.number)));
        }
        let bytes: [u8; PATTERN_SIZE] =
            member(&mut zip, &entry.file)?
                .try_into()
                .map_err(|b: Vec<u8>| {
                    pattern_error(format!("{} B, expected {PATTERN_SIZE} B", b.len()))
                })?;
        let pattern =
            Pattern::from_bytes(&bytes).map_err(|e: PatternError| pattern_error(e.to_string()))?;
        project
            .set_pattern(index, pattern)
            .map_err(FileError::Project)?;
    }
    Ok(project)
}

/// Saves `project` to `path`.
pub fn save(project: &Project, path: &Path) -> Result<(), FileError> {
    let mut writer = BufWriter::new(File::create(path)?);
    write(project, &mut writer)?;
    writer.flush()?;
    Ok(())
}

/// Loads a project from `path`.
pub fn load(path: &Path) -> Result<Project, FileError> {
    read(BufReader::new(File::open(path)?))
}

/// Reads archive member `name` completely, with a size limit.
fn member<R: Read + Seek>(zip: &mut ZipArchive<R>, name: &str) -> Result<Vec<u8>, FileError> {
    let member_error = |message: String| FileError::Member {
        name: name.to_owned(),
        message,
    };
    let file = zip.by_name(name).map_err(|e| member_error(e.to_string()))?;
    if file.size() > MAX_MEMBER_SIZE {
        return Err(member_error(format!("{} B is too large", file.size())));
    }
    let mut bytes = Vec::new();
    file.take(MAX_MEMBER_SIZE + 1).read_to_end(&mut bytes)?;
    if bytes.len() as u64 > MAX_MEMBER_SIZE {
        return Err(member_error("too large".into()));
    }
    Ok(bytes)
}
