//! Converts an audio file to the volca sample format and writes it as WAV.
//!
//! Usage: import_sample <input> <out.wav> [--trim <start>..<end>] [--normalize]
//!
//! `--trim` takes frame numbers of the converted sample (31 250 Hz). Prints
//! the length and the memory blocks the sample takes on the device. The
//! output can be sent with `syro_wav`, e.g. `s0:<out.wav>`.

use std::error::Error;
use std::fs::File;
use std::io::BufWriter;
use std::path::Path;
use std::process::ExitCode;

use volcasm_core::device::VOLCA_SAMPLE;
use volcasm_core::sample::{edit, import, wav};

fn run() -> Result<(), Box<dyn Error>> {
    let mut args = std::env::args().skip(1);
    let usage = "usage: import_sample <input> <out.wav> [--trim <start>..<end>] [--normalize]";
    let (Some(input), Some(output)) = (args.next(), args.next()) else {
        return Err(usage.into());
    };
    let mut sample = import::import_file(Path::new(&input), VOLCA_SAMPLE.sample_rate)?;
    println!("imported: {} frames", sample.frames());

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--trim" => {
                let range = args.next().ok_or(usage)?;
                let (start, end) = range.split_once("..").ok_or(usage)?;
                let range = start.parse()?..end.parse()?;
                sample = edit::trim(&sample, range.clone())
                    .ok_or_else(|| format!("trim range {range:?} out of 0..{}", sample.frames()))?;
                println!("trimmed:  {} frames", sample.frames());
            }
            "--normalize" => {
                let gain = edit::normalize(&mut sample);
                println!("normalized: gain {gain:.3}");
            }
            _ => return Err(usage.into()),
        }
    }

    wav::write(&sample, BufWriter::new(File::create(&output)?))?;
    let memory = VOLCA_SAMPLE.memory;
    println!(
        "{output}: {} frames, {:.3} s, {} of {} memory blocks",
        sample.frames(),
        sample.duration(),
        memory.blocks_for(sample.frames()),
        memory.blocks
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
