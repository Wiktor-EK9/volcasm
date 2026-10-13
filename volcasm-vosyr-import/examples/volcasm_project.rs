//! Works with `.volcasm` project files.
//!
//! Usage:
//!   volcasm_project import <project.vsj> <out.volcasm>
//!   volcasm_project info <file.volcasm>
//!   volcasm_project stream <file.volcasm> <out.wav> <target>... [--bits <8-16>]
//!
//! Targets: `s<slot>` (sample), `e<slot>` (erase), `p<1-10>` (pattern),
//! `samples` (all filled slots), `patterns` (all ten). The stream WAV can be
//! played with `play_syro`.

use std::error::Error;
use std::path::Path;
use std::process::ExitCode;

use volcasm_core::pattern::Func;
use volcasm_core::project::{Project, Target, file};
use volcasm_core::syro::encode;
use volcasm_core::wav::stream_to_wav;

fn info(project: &Project) {
    println!("{} ({})", project.name, project.device().name);
    for (label, value) in [
        ("author", &project.author),
        ("license", &project.license),
        ("description", &project.description),
    ] {
        if let Some(value) = value {
            println!("{label}: {value}");
        }
    }
    for (n, slot) in project.slots() {
        println!("  s{n:<2} {:30} {:.3} s", slot.name, slot.sample.duration());
    }
    let memory = project.memory_usage();
    println!(
        "memory: {} of {} blocks{}",
        memory.used_blocks,
        memory.total_blocks,
        if memory.fits() { "" } else { " (does NOT fit)" }
    );
    for (i, pattern) in project.patterns().iter().enumerate() {
        let used: Vec<String> = pattern
            .parts()
            .enumerate()
            .filter(|(_, part)| part.steps() != 0)
            .map(|(p, part)| {
                let motion = if part.func(Func::Motion) { "m" } else { "" };
                format!("P{}:s{}{motion}", p + 1, part.sample())
            })
            .collect();
        let issues = pattern.issues(project.device().sample_slots);
        println!(
            "  p{:<2} {}{}",
            i + 1,
            if used.is_empty() {
                "empty".into()
            } else {
                used.join(" ")
            },
            if issues.is_empty() {
                String::new()
            } else {
                format!("  issues: {issues:?}")
            }
        );
    }
}

fn targets(project: &Project, args: &[String]) -> Result<Vec<Target>, Box<dyn Error>> {
    let mut out = Vec::new();
    for arg in args {
        match arg.as_str() {
            "samples" => out.extend(project.slots().map(|(n, _)| Target::Sample(n))),
            "patterns" => out.extend((0..project.patterns().len()).map(Target::Pattern)),
            _ => {
                let (kind, number) = arg.split_at(1);
                let number: usize = number
                    .parse()
                    .map_err(|_| format!("invalid target {arg}"))?;
                out.push(match kind {
                    "s" => Target::Sample(number),
                    "e" => Target::Erase(number),
                    "p" => Target::Pattern(number.checked_sub(1).ok_or("patterns are 1-10")?),
                    _ => return Err(format!("invalid target {arg}").into()),
                });
            }
        }
    }
    Ok(out)
}

fn run() -> Result<(), Box<dyn Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.as_slice() {
        [cmd, vsj, out] if cmd == "import" => {
            let (project, errors) = volcasm_vosyr_import::load(Path::new(vsj))?.to_project();
            for err in &errors {
                eprintln!("warning: {err}");
            }
            file::save(&project, Path::new(out))?;
            info(&project);
            println!("saved {out}");
        }
        [cmd, path] if cmd == "info" => info(&file::load(Path::new(path))?),
        [cmd, path, out, rest @ ..] if cmd == "stream" => {
            let project = file::load(Path::new(path))?;
            let (bits, rest) = match rest {
                [targets @ .., flag, bits] if flag == "--bits" => (Some(bits.parse()?), targets),
                _ => (None, rest),
            };
            let targets = targets(&project, rest)?;
            if targets.is_empty() {
                return Err("no targets".into());
            }
            let frames = encode(&project.syro_items(&targets, bits)?)?;
            std::fs::write(out, stream_to_wav(&frames))?;
            println!(
                "{out}: {} items, {:.1} s at 44.1 kHz",
                targets.len(),
                frames.len() as f64 / 44_100.0
            );
        }
        _ => {
            return Err(
                "usage: volcasm_project import <vsj> <out.volcasm> | info <file> | \
                        stream <file> <out.wav> <target>... [--bits N]"
                    .into(),
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
