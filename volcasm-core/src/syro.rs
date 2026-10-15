//! Safe wrapper over the KORG SYRO encoder for volca sample.

use std::fmt;
use std::ptr;

use volcasm_syro_sys as sys;

/// Sample rate of the generated SYRO stream (stereo, 16-bit).
pub const STREAM_SAMPLE_RATE: u32 = 44_100;

/// Size of a raw `VolcaSample_Pattern_Data` structure in bytes.
pub const PATTERN_SIZE: usize = sys::VOLCASAMPLE_PATTERN_SIZE as usize;

/// One unit of data to transfer in a SYRO stream.
#[derive(Debug, Clone, Copy)]
pub enum SyroItem<'a> {
    /// Uncompressed 16-bit mono sample for slot `slot`.
    Sample {
        slot: u32,
        pcm: &'a [i16],
        sample_rate: u32,
    },
    /// Sample compressed to `bits` (8-16) bits per sample.
    SampleCompressed {
        slot: u32,
        pcm: &'a [i16],
        sample_rate: u32,
        bits: u32,
    },
    /// Erase the sample in slot `slot`.
    Erase { slot: u32 },
    /// Raw `VolcaSample_Pattern_Data` for pattern `index` (0-based).
    Pattern {
        index: u32,
        data: &'a [u8; PATTERN_SIZE],
    },
    /// Whole sample memory image (`.alldata` from the SDK). Replaces all
    /// samples on the device; `all_sample_empty.alldata` erases them.
    All { data: &'a [u8] },
    /// Whole sample memory image compressed to `bits` (8-16) bits per sample.
    AllCompressed { data: &'a [u8], bits: u32 },
}

/// Error status returned by SYRO.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyroError {
    IllegalDataType,
    IllegalData,
    IllegalParameter,
    OutOfRangeNumber,
    OutOfRangeQuality,
    NotEnoughMemory,
    InvalidHandle,
    NoData,
    /// Status code not defined by the SDK header. The C enum is unsigned with
    /// GCC and signed with MSVC; i64 holds both.
    Unknown(i64),
    /// Sample is too large to describe in a `SyroData` (over 4 GiB).
    SampleTooLarge,
}

impl SyroError {
    fn from_status(status: sys::SyroStatus) -> Self {
        match status {
            sys::Status_IllegalDataType => Self::IllegalDataType,
            sys::Status_IllegalData => Self::IllegalData,
            sys::Status_IllegalParameter => Self::IllegalParameter,
            sys::Status_OutOfRange_Number => Self::OutOfRangeNumber,
            sys::Status_OutOfRange_Quality => Self::OutOfRangeQuality,
            sys::Status_NotEnoughMemory => Self::NotEnoughMemory,
            sys::Status_InvalidHandle => Self::InvalidHandle,
            sys::Status_NoData => Self::NoData,
            other => Self::Unknown(other.into()),
        }
    }
}

impl fmt::Display for SyroError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unknown(code) => write!(f, "SYRO error: unknown status {code}"),
            other => write!(f, "SYRO error: {other:?}"),
        }
    }
}

impl std::error::Error for SyroError {}

/// Owned buffer and metadata for one `SyroData` entry.
struct Entry {
    data_type: sys::SyroDataType,
    bytes: Vec<u8>,
    number: u32,
    quality: u32,
    fs: u32,
}

impl Entry {
    fn new(item: &SyroItem<'_>) -> Result<Self, SyroError> {
        let pcm_bytes = |pcm: &[i16]| -> Result<Vec<u8>, SyroError> {
            u32::try_from(pcm.len() * 2).map_err(|_| SyroError::SampleTooLarge)?;
            Ok(pcm.iter().flat_map(|s| s.to_le_bytes()).collect())
        };
        Ok(match *item {
            SyroItem::Sample {
                slot,
                pcm,
                sample_rate,
            } => Self {
                data_type: sys::DataType_Sample_Liner,
                bytes: pcm_bytes(pcm)?,
                number: slot,
                quality: 16,
                fs: sample_rate,
            },
            SyroItem::SampleCompressed {
                slot,
                pcm,
                sample_rate,
                bits,
            } => Self {
                data_type: sys::DataType_Sample_Compress,
                bytes: pcm_bytes(pcm)?,
                number: slot,
                quality: bits,
                fs: sample_rate,
            },
            SyroItem::Erase { slot } => Self {
                data_type: sys::DataType_Sample_Erase,
                bytes: Vec::new(),
                number: slot,
                quality: 16,
                fs: 0,
            },
            SyroItem::Pattern { index, data } => Self {
                data_type: sys::DataType_Pattern,
                bytes: data.to_vec(),
                number: index,
                quality: 16,
                fs: 0,
            },
            SyroItem::All { data } => Self::all(sys::DataType_Sample_All, data, 16)?,
            SyroItem::AllCompressed { data, bits } => {
                Self::all(sys::DataType_Sample_AllCompress, data, bits)?
            }
        })
    }

    fn all(data_type: sys::SyroDataType, data: &[u8], quality: u32) -> Result<Self, SyroError> {
        u32::try_from(data.len()).map_err(|_| SyroError::SampleTooLarge)?;
        Ok(Self {
            data_type,
            bytes: data.to_vec(),
            number: 0,
            quality,
            fs: 0,
        })
    }

    fn as_syro_data(&mut self) -> sys::SyroData {
        sys::SyroData {
            DataType: self.data_type,
            pData: if self.bytes.is_empty() {
                ptr::null_mut()
            } else {
                self.bytes.as_mut_ptr()
            },
            Number: self.number,
            Size: self.bytes.len() as u32,
            Quality: self.quality,
            Fs: self.fs,
            SampleEndian: sys::LittleEndian,
        }
    }
}

/// Encodes `items` into a stereo SYRO stream at [`STREAM_SAMPLE_RATE`].
///
/// Returns one `[left, right]` frame per stream frame. The length is the
/// frame count reported by `SyroVolcaSample_Start`. SYRO stops producing data
/// slightly before that count (`Status_NoData`); like the SDK example, the
/// last produced frame is repeated for the remainder.
pub fn encode(items: &[SyroItem<'_>]) -> Result<Vec<[i16; 2]>, SyroError> {
    let mut entries = items
        .iter()
        .map(Entry::new)
        .collect::<Result<Vec<_>, _>>()?;
    let mut data: Vec<sys::SyroData> = entries.iter_mut().map(Entry::as_syro_data).collect();
    let num_of_data = i32::try_from(data.len()).map_err(|_| SyroError::IllegalParameter)?;

    let mut handle: sys::SyroHandle = ptr::null_mut();
    let mut frames: u32 = 0;
    // SAFETY: `data` and the buffers it points into (`entries`) outlive the
    // handle, which is released by `End` below on every path.
    let status = unsafe {
        sys::SyroVolcaSample_Start(&mut handle, data.as_mut_ptr(), num_of_data, 0, &mut frames)
    };
    if status != sys::Status_Success {
        return Err(SyroError::from_status(status));
    }

    let mut out = Vec::with_capacity(frames as usize);
    let (mut left, mut right) = (0i16, 0i16);
    let mut result = Ok(());
    for _ in 0..frames {
        // SAFETY: `handle` is valid between `Start` and `End`.
        let status = unsafe { sys::SyroVolcaSample_GetSample(handle, &mut left, &mut right) };
        if status != sys::Status_Success && status != sys::Status_NoData {
            result = Err(SyroError::from_status(status));
            break;
        }
        out.push([left, right]);
    }

    // SAFETY: `handle` came from a successful `Start` and is ended once.
    let status = unsafe { sys::SyroVolcaSample_End(handle) };
    drop(entries);
    result?;
    if status != sys::Status_Success {
        return Err(SyroError::from_status(status));
    }
    Ok(out)
}
