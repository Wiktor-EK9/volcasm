//! Plays a SYRO stream WAV (from `syro_wav`) to the volca sample SYNC IN
//! after checking that the device runs at 44.1 kHz. `--status` needs
//! PipeWire (Linux).
//!
//! Usage:
//!   play_syro --list                      list audio outputs
//!   play_syro --status                    show PipeWire clock and output rates
//!   play_syro [--device <id>] <stream.wav>
//!
//! Press Enter while playing to cancel.

use std::error::Error;
use std::io::BufRead;
use std::process::ExitCode;
use std::time::Duration;

use volcasm_audio::check::STREAM_RATE;
use volcasm_audio::{Phase, TransferOptions, list_outputs};

fn read_stream(path: &str) -> Result<Vec<[i16; 2]>, Box<dyn Error>> {
    let mut reader = hound::WavReader::open(path)?;
    let spec = reader.spec();
    if spec.channels != 2 || spec.bits_per_sample != 16 || spec.sample_rate != STREAM_RATE {
        return Err(format!("{path}: expected a 16-bit stereo {STREAM_RATE} Hz WAV").into());
    }
    let samples = reader.samples::<i16>().collect::<Result<Vec<_>, _>>()?;
    Ok(samples.as_chunks::<2>().0.to_vec())
}

fn run() -> Result<(), Box<dyn Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let usage = "usage: play_syro --list | --status | [--device <id>] <stream.wav>";
    let (device, path) = match args.as_slice() {
        [flag] if flag == "--list" => {
            for output in list_outputs()? {
                println!("{}\n    {}", output.name, output.id);
            }
            return Ok(());
        }
        [flag] if flag == "--status" => return status(),
        [flag, id, path] if flag == "--device" => (Some(id.clone()), path),
        [path] if !path.starts_with('-') => (None, path),
        _ => return Err(usage.into()),
    };

    let frames = read_stream(path)?;
    println!(
        "{path}: {} frames, {:.1} s",
        frames.len(),
        frames.len() as f64 / f64::from(STREAM_RATE)
    );
    let transfer = volcasm_audio::start(frames, TransferOptions { device });
    let canceller = transfer.canceller();
    std::thread::spawn(move || {
        if std::io::stdin().lock().lines().next().is_some() {
            canceller.cancel();
        }
    });

    let mut last_phase = None;
    while transfer.phase() != Phase::Done {
        let phase = transfer.phase();
        if last_phase != Some(phase) {
            if last_phase == Some(Phase::Sending) {
                println!();
            }
            if phase == Phase::Sending {
                for warning in transfer.warnings() {
                    println!("warning: {warning}");
                }
            }
            println!("{phase:?}");
            last_phase = Some(phase);
        }
        if phase == Phase::Sending {
            let (sent, total) = transfer.progress();
            print!("\r{:5.1}%", sent as f64 * 100.0 / total.max(1) as f64);
            std::io::Write::flush(&mut std::io::stdout())?;
        }
        std::thread::sleep(Duration::from_millis(200));
    }
    let report = transfer.wait()?;
    if report.cancelled {
        println!("cancelled");
    } else {
        println!("done");
    }
    if report.xruns > 0 {
        println!(
            "warning: {} buffer underruns; the stream is likely corrupted",
            report.xruns
        );
    }
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

/// Prints the PipeWire clock settings and the rates of the outputs.
#[cfg(target_os = "linux")]
fn status() -> Result<(), Box<dyn Error>> {
    use volcasm_audio::{check, graph};

    let snapshot = graph::snapshot()?;
    println!("{:?}", snapshot.clock);
    match check::clock_reachable(&snapshot.clock) {
        Ok(()) => println!("clock: can run at {STREAM_RATE} Hz"),
        Err(problem) => println!("clock: {problem}"),
    }
    for node in &snapshot.nodes {
        if node.prop("media.class") != Some("Audio/Sink") {
            continue;
        }
        let support = match node.supports_rate(STREAM_RATE) {
            Some(true) => "supports",
            Some(false) => "does NOT support",
            None => "unknown support for",
        };
        println!(
            "{}: {support} {STREAM_RATE} Hz, rates {:?}, current {:?}",
            node.prop("node.description").unwrap_or("?"),
            node.rates,
            node.format.map(|f| f.rate)
        );
    }
    Ok(())
}

#[cfg(not(target_os = "linux"))]
fn status() -> Result<(), Box<dyn Error>> {
    Err("--status needs PipeWire (Linux)".into())
}
