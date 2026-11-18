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

/// Chunk size (input frames) for [`resample_stream`].
const RESAMPLE_CHUNK: usize = 1024;

/// Error resampling a stream.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResampleError(String);

impl fmt::Display for ResampleError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "resampling the stream failed: {}", self.0)
    }
}

impl std::error::Error for ResampleError {}

/// Converts a stream from [`encode`] (stereo, [`STREAM_SAMPLE_RATE`]) to
/// `rate`, for outputs whose shared mixer runs at another rate: the
/// conversion is done here with a known resampler instead of by the system.
///
/// rubato's synchronous FFT resampler, processed in chunks so that long
/// streams do not need a floating point copy. The resampler delay is
/// trimmed (output aligned with the input) and the length is
/// `ceil(len * rate / 44100)`. Samples are rounded and clipped to 16 bits.
/// A stream already at `rate` is returned unchanged.
pub fn resample_stream(frames: &[[i16; 2]], rate: u32) -> Result<Vec<[i16; 2]>, ResampleError> {
    use rubato::audioadapter_buffers::direct::SequentialSliceOfSlices;
    use rubato::{Fft, FixedSync, Resampler};

    if rate == STREAM_SAMPLE_RATE {
        return Ok(frames.to_vec());
    }
    let err = |e: &dyn std::error::Error| ResampleError(e.to_string());
    let mut resampler = Fft::<f32>::new(
        STREAM_SAMPLE_RATE as usize,
        rate as usize,
        RESAMPLE_CHUNK,
        2,
        FixedSync::Input,
    )
    .map_err(|e| err(&e))?;

    let expected = (frames.len() as u64 * u64::from(rate)).div_ceil(u64::from(STREAM_SAMPLE_RATE));
    let expected = usize::try_from(expected).map_err(|e| err(&e))?;
    let mut out = Vec::with_capacity(expected);
    let mut skip = resampler.output_delay();
    let mut pos = 0;
    let (mut left, mut right) = (Vec::new(), Vec::new());
    while out.len() < expected {
        // After the input ends, zeros flush the resampler.
        let need = resampler.input_frames_next();
        let chunk = &frames[pos.min(frames.len())..(pos + need).min(frames.len())];
        pos += need;
        left.clear();
        right.clear();
        for [l, r] in chunk {
            left.push(f32::from(*l) / 32_768.0);
            right.push(f32::from(*r) / 32_768.0);
        }
        left.resize(need, 0.0);
        right.resize(need, 0.0);
        let channels = [left.as_slice(), right.as_slice()];
        let input = SequentialSliceOfSlices::new(&channels, 2, need).map_err(|e| err(&e))?;
        let block = resampler.process(&input, None).map_err(|e| err(&e))?;
        let data = block.take_data();
        let produced = data.as_chunks::<2>().0;
        let start = skip.min(produced.len());
        skip -= start;
        for [l, r] in &produced[start..] {
            if out.len() == expected {
                break;
            }
            out.push([to_i16(*l), to_i16(*r)]);
        }
    }
    Ok(out)
}

/// Converts a sample in [-1, 1) to 16 bits with rounding and clipping.
fn to_i16(x: f32) -> i16 {
    (x * 32_768.0)
        .round()
        .clamp(f32::from(i16::MIN), f32::from(i16::MAX)) as i16
}

#[cfg(test)]
mod resample_tests {
    use super::*;

    #[test]
    fn same_rate_is_unchanged() {
        let frames = [[1, -1], [i16::MAX, i16::MIN]];
        assert_eq!(
            resample_stream(&frames, STREAM_SAMPLE_RATE).unwrap(),
            frames
        );
    }

    #[test]
    fn length_follows_ratio() {
        for len in [0, 1, 1023, 1024, 1025, 44_100, 100_000] {
            let out = resample_stream(&vec![[1000, -1000]; len], 48_000).unwrap();
            assert_eq!(out.len(), (len * 48_000).div_ceil(44_100), "input {len}");
        }
    }

    #[test]
    fn tone_is_kept_and_aligned() {
        // 1 kHz on the left, 3 kHz on the right, half scale.
        let tone = |f: f64, t: f64| {
            (0.5 * (2.0 * std::f64::consts::PI * f * t).sin() * 32_768.0).round() as i16
        };
        let frames: Vec<[i16; 2]> = (0..44_100)
            .map(|n| {
                let t = n as f64 / 44_100.0;
                [tone(1000.0, t), tone(3000.0, t)]
            })
            .collect();
        let out = resample_stream(&frames, 48_000).unwrap();
        // Away from the edges, each output frame equals the tone at its time.
        for (n, [l, r]) in out.iter().enumerate().skip(1000).take(46_000) {
            let t = n as f64 / 48_000.0;
            assert!(
                (i32::from(*l) - i32::from(tone(1000.0, t))).abs() <= 4,
                "left {n}"
            );
            assert!(
                (i32::from(*r) - i32::from(tone(3000.0, t))).abs() <= 4,
                "right {n}"
            );
        }
    }

    #[test]
    fn syro_stream_does_not_clip() {
        // Pseudo-random sample: dense data in every QAM symbol.
        let mut x: u32 = 1;
        let pcm: Vec<i16> = (0..20_000)
            .map(|_| {
                x = x.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
                (x >> 16) as i16
            })
            .collect();
        let frames = encode(&[SyroItem::SampleCompressed {
            slot: 0,
            pcm: &pcm,
            sample_rate: 31_250,
            bits: 16,
        }])
        .unwrap();
        let peak = |f: &[[i16; 2]]| {
            f.iter()
                .flatten()
                .map(|s| i32::from(*s).abs())
                .max()
                .unwrap()
        };
        let out = resample_stream(&frames, 48_000).unwrap();
        // Overshoot of the band-limited signal must stay below full scale.
        assert!(
            peak(&out) < i32::from(i16::MAX),
            "{} -> {}",
            peak(&frames),
            peak(&out)
        );
    }
}
