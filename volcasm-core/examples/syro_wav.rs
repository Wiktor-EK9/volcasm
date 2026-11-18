//! Writes a SYRO stream WAV for playback into the volca sample SYNC IN.
//!
//! Usage: syro_wav [--rate <hz>] <out.wav> <item>...
//!   --rate <hz>                 resample the stream to <hz> as Volcasm does
//!                               for outputs not at 44.1 kHz
//!   s<slot>:<file.wav>          sample (16-bit mono WAV), slot 0-99
//!   s<slot>c<bits>:<file.wav>   compressed sample, bits 8-16
//!   e<slot>                     erase sample slot
//!   p<number>:<file>            raw pattern data (2624 B), pattern 1-10
//!   a:<file.alldata>            whole sample memory (replaces all samples)
//!   ac<bits>:<file.alldata>     same, compressed (bits 8-16)
//!
//! Input samples are not converted (see `import_sample`): the WAV must already be
//! 16-bit mono; its sample rate is passed to SYRO as is.

use std::error::Error;
use std::path::Path;
use std::process::ExitCode;

use volcasm_core::syro::{PATTERN_SIZE, STREAM_SAMPLE_RATE, SyroItem, encode, resample_stream};
use volcasm_core::wav::stream_to_wav_at;

enum Spec {
    Sample {
        slot: u32,
        bits: Option<u32>,
        pcm: Vec<i16>,
        sample_rate: u32,
    },
    Erase {
        slot: u32,
    },
    Pattern {
        index: u32,
        data: Box<[u8; PATTERN_SIZE]>,
    },
    All {
        data: Vec<u8>,
        bits: Option<u32>,
    },
}

impl Spec {
    fn item(&self) -> SyroItem<'_> {
        match self {
            Spec::Sample {
                slot,
                bits: None,
                pcm,
                sample_rate,
            } => SyroItem::Sample {
                slot: *slot,
                pcm,
                sample_rate: *sample_rate,
            },
            Spec::Sample {
                slot,
                bits: Some(bits),
                pcm,
                sample_rate,
            } => SyroItem::SampleCompressed {
                slot: *slot,
                pcm,
                sample_rate: *sample_rate,
                bits: *bits,
            },
            Spec::Erase { slot } => SyroItem::Erase { slot: *slot },
            Spec::Pattern { index, data } => SyroItem::Pattern {
                index: *index,
                data,
            },
            Spec::All { data, bits: None } => SyroItem::All { data },
            Spec::All {
                data,
                bits: Some(bits),
            } => SyroItem::AllCompressed { data, bits: *bits },
        }
    }
}

fn parse(arg: &str) -> Result<Spec, Box<dyn Error>> {
    let (head, path) = match arg.split_once(':') {
        Some((head, path)) => (head, Some(path)),
        None => (arg, None),
    };
    let kind = head.chars().next().ok_or("empty item")?;
    let rest = &head[1..];
    match (kind, path) {
        ('s', Some(path)) => {
            let (slot, bits) = match rest.split_once('c') {
                Some((slot, bits)) => (slot.parse()?, Some(bits.parse()?)),
                None => (rest.parse()?, None),
            };
            let (pcm, sample_rate) = read_pcm(Path::new(path))?;
            Ok(Spec::Sample {
                slot,
                bits,
                pcm,
                sample_rate,
            })
        }
        ('e', None) => Ok(Spec::Erase {
            slot: rest.parse()?,
        }),
        ('p', Some(path)) => {
            let number: u32 = rest.parse()?;
            let index = number.checked_sub(1).ok_or("pattern number is 1-10")?;
            let data: Box<[u8; PATTERN_SIZE]> = std::fs::read(path)?
                .into_boxed_slice()
                .try_into()
                .map_err(|_| format!("{path}: pattern must be {PATTERN_SIZE} bytes"))?;
            Ok(Spec::Pattern { index, data })
        }
        ('a', Some(path)) => Ok(Spec::All {
            data: std::fs::read(path)?,
            bits: match rest.strip_prefix('c') {
                Some(bits) => Some(bits.parse()?),
                None if rest.is_empty() => None,
                None => return Err(format!("invalid item: {arg}").into()),
            },
        }),
        _ => Err(format!("invalid item: {arg}").into()),
    }
}

fn read_pcm(path: &Path) -> Result<(Vec<i16>, u32), Box<dyn Error>> {
    let mut reader = hound::WavReader::open(path)?;
    let spec = reader.spec();
    if spec.channels != 1 || spec.bits_per_sample != 16 {
        return Err(format!("{}: expected 16-bit mono WAV", path.display()).into());
    }
    let pcm = reader.samples::<i16>().collect::<Result<_, _>>()?;
    Ok((pcm, spec.sample_rate))
}

fn run() -> Result<(), Box<dyn Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (rate, args) = match args.as_slice() {
        [flag, rate, rest @ ..] if flag == "--rate" => (rate.parse::<u32>()?, rest),
        rest => (STREAM_SAMPLE_RATE, rest),
    };
    let [out, items @ ..] = args else {
        return Err("usage: syro_wav [--rate <hz>] <out.wav> <item>...".into());
    };
    if items.is_empty() {
        return Err("no items given".into());
    }
    let specs = items
        .iter()
        .map(|arg| parse(arg))
        .collect::<Result<Vec<_>, _>>()?;
    let items: Vec<SyroItem<'_>> = specs.iter().map(Spec::item).collect();
    let frames = resample_stream(&encode(&items)?, rate)?;
    std::fs::write(out, stream_to_wav_at(&frames, rate))?;
    println!(
        "{out}: {} frames, {:.2} s at {rate} Hz",
        frames.len(),
        frames.len() as f64 / f64::from(rate)
    );
    Ok(())
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("error: {err}");
            ExitCode::FAILURE
        }
    }
}
