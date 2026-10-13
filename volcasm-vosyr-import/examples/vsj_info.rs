//! Prints the content of a Vosyr project: sample slots and sequences.
//!
//! Usage: vsj_info <project.vsj>

use std::error::Error;
use std::path::Path;
use std::process::ExitCode;

use volcasm_vosyr_import::load;

/// FuncMemoryPart bits (SDK): MOTION, LOOP, REVERB, REVERSE; bit 4 set means
/// the part plays (not muted).
fn flags(func: u8) -> String {
    let mut out: Vec<&str> = [(0, "motion"), (1, "loop"), (2, "reverb"), (3, "reverse")]
        .iter()
        .filter(|(bit, _)| func & (1 << bit) != 0)
        .map(|&(_, name)| name)
        .collect();
    if func & 0x10 == 0 {
        out.push("MUTE");
    }
    out.join(",")
}

fn steps(bits: u16) -> String {
    (0..16)
        .map(|i| if bits & (1 << i) != 0 { 'x' } else { '.' })
        .collect()
}

fn run() -> Result<(), Box<dyn Error>> {
    let path = std::env::args()
        .nth(1)
        .ok_or("usage: vsj_info <project.vsj>")?;
    let project = load(Path::new(&path))?;
    println!("project {} ({})", project.name, project.dir.display());

    println!("\nsamples:");
    let mut total_frames = 0;
    for slot in project.index.used() {
        let length = match project.load_sample(slot.slot) {
            Ok(sample) => {
                total_frames += sample.frames();
                format!("{:.3} s", sample.duration())
            }
            Err(err) => format!("not loaded: {err}"),
        };
        println!("  {:2} {:30} {length}", slot.slot, slot.name);
    }
    println!(
        "  {} used, {:.3} s loaded",
        project.index.used().count(),
        total_frames as f64 / 31_250.0
    );

    for (n, sequence) in project.sequences.iter().enumerate() {
        println!(
            "\nsequence {:2}  active steps {}",
            n + 1,
            steps(sequence.ActiveStep)
        );
        for (i, part) in sequence.Part.iter().enumerate() {
            let motion = part.Motion.iter().flatten().filter(|&&v| v != 0).count();
            println!(
                "  P{:<2} sample {:2}  {}  {:02x?} {}{}",
                i + 1,
                part.SampleNum,
                steps(part.StepOn),
                part.Param,
                flags(part.FuncMemoryPart),
                if motion > 0 {
                    format!(" ({motion} motion values)")
                } else {
                    String::new()
                }
            );
        }
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
