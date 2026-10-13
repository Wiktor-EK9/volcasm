//! `.volcasm` files: round trip, reproducible output and rejected input.

use std::io::{Cursor, Write};

use volcasm_core::device::VOLCA_SAMPLE;
use volcasm_core::pattern::{Func, Param, Pattern, Speed};
use volcasm_core::project::file::{self, FileError};
use volcasm_core::project::{Project, Slot};
use volcasm_core::sample::Sample;
use zip::ZipWriter;
use zip::write::SimpleFileOptions;

fn sample(seed: i16, frames: usize) -> Sample {
    Sample {
        sample_rate: 31_250,
        pcm: (0..frames).map(|i| (i as i16).wrapping_mul(seed)).collect(),
    }
}

fn project() -> Project {
    let mut p = Project::new(&VOLCA_SAMPLE, "Round trip");
    p.author = Some("Tester".into());
    p.license = Some("CC0-1.0".into());
    p.description = Some("Zażółć gęślą jaźń".into());
    p.tempo = 97;
    for (slot, seed, frames) in [(0, 3, 1000), (5, -7, 1), (99, 11, 40_000)] {
        p.set_slot(
            slot,
            Slot {
                name: format!("sample {slot}"),
                sample: sample(seed, frames),
            },
        )
        .unwrap();
    }
    let pattern = p.pattern_mut(3).unwrap();
    pattern.set_active_steps(0x7ffe);
    let part = pattern.part_mut(1);
    part.set_sample(99);
    part.set_steps(0x1111);
    part.set_param(Param::Pan, -20).unwrap();
    part.set_speed(Speed::Semitones(-12)).unwrap();
    part.set_func(Func::Loop, true);
    // An unknown field must survive too.
    pattern.data_mut().Padding2[5] = 0x42;
    p
}

fn to_bytes(project: &Project) -> Vec<u8> {
    let mut out = Cursor::new(Vec::new());
    file::write(project, &mut out).unwrap();
    out.into_inner()
}

#[test]
fn round_trip() {
    let original = project();
    let read = file::read(Cursor::new(to_bytes(&original))).unwrap();
    assert_eq!(read, original);
    assert_eq!(read.pattern(3).unwrap().data().Padding2[5], 0x42);
}

#[test]
fn output_is_reproducible() {
    assert_eq!(to_bytes(&project()), to_bytes(&project()));
}

#[test]
fn save_and_load() {
    let path = std::path::Path::new(env!("CARGO_TARGET_TMPDIR")).join("round_trip.volcasm");
    file::save(&project(), &path).unwrap();
    assert_eq!(file::load(&path).unwrap(), project());
}

#[test]
fn manifest_is_readable_json() {
    let bytes = to_bytes(&project());
    let mut zip = zip::ZipArchive::new(Cursor::new(bytes)).unwrap();
    let manifest: serde_json::Value =
        serde_json::from_reader(zip.by_name("manifest.json").unwrap()).unwrap();
    assert_eq!(manifest["format"], "volcasm");
    assert_eq!(manifest["version"], 1);
    assert_eq!(manifest["device"], "volca-sample");
    assert_eq!(manifest["slots"][2]["slot"], 99);
    assert_eq!(manifest["slots"][2]["file"], "samples/99.wav");
    assert_eq!(manifest["patterns"][9]["file"], "patterns/10.bin");
    assert_eq!(manifest["tempo"], 97);
}

/// Builds an archive from `members` (name, content).
fn archive(members: &[(&str, &[u8])]) -> Vec<u8> {
    let mut zip = ZipWriter::new(Cursor::new(Vec::new()));
    for (name, content) in members {
        zip.start_file(*name, SimpleFileOptions::default()).unwrap();
        zip.write_all(content).unwrap();
    }
    zip.finish().unwrap().into_inner()
}

fn manifest(fields: &str) -> String {
    format!(
        r#"{{"format":"volcasm","version":1,"device":"volca-sample","name":"x",
            "author":null,"license":null,"description":null{fields}}}"#
    )
}

fn read_err(members: &[(&str, &[u8])]) -> FileError {
    file::read(Cursor::new(archive(members))).unwrap_err()
}

fn wav_bytes(sample: &Sample) -> Vec<u8> {
    let mut out = Cursor::new(Vec::new());
    volcasm_core::sample::wav::write(sample, &mut out).unwrap();
    out.into_inner()
}

#[test]
fn rejects_foreign_newer_and_unknown_device() {
    let other = manifest(r#","slots":[],"patterns":[]"#).replace("\"volcasm\"", "\"other\"");
    assert!(matches!(
        read_err(&[("manifest.json", other.as_bytes())]),
        FileError::NotVolcasm(_)
    ));
    let newer = manifest(r#","slots":[],"patterns":[]"#).replace("\"version\":1", "\"version\":2");
    assert!(matches!(
        read_err(&[("manifest.json", newer.as_bytes())]),
        FileError::UnsupportedVersion(2)
    ));
    let device = manifest(r#","slots":[],"patterns":[]"#).replace("volca-sample", "volca-sample-2");
    assert!(matches!(
        read_err(&[("manifest.json", device.as_bytes())]),
        FileError::UnknownDevice(_)
    ));
    assert!(matches!(read_err(&[]), FileError::Member { .. }));
}

#[test]
fn missing_patterns_default_and_unknown_fields_are_ignored() {
    let m = manifest(r#","slots":[],"patterns":[],"future_field":42"#);
    let p = file::read(Cursor::new(archive(&[("manifest.json", m.as_bytes())]))).unwrap();
    assert_eq!(p, Project::new(&VOLCA_SAMPLE, "x"));
    assert_eq!(p.tempo, 120);
}

#[test]
fn rejects_bad_members() {
    let wav = wav_bytes(&sample(1, 10));
    let dup = manifest(
        r#","slots":[{"slot":1,"name":"a","file":"a.wav"},{"slot":1,"name":"b","file":"a.wav"}],"patterns":[]"#,
    );
    assert!(matches!(
        read_err(&[("manifest.json", dup.as_bytes()), ("a.wav", &wav)]),
        FileError::Duplicate(_)
    ));

    let missing = manifest(r#","slots":[{"slot":1,"name":"a","file":"a.wav"}],"patterns":[]"#);
    assert!(matches!(
        read_err(&[("manifest.json", missing.as_bytes())]),
        FileError::Member { .. }
    ));

    let mut wrong_rate = sample(1, 10);
    wrong_rate.sample_rate = 44_100;
    let rate_wav = wav_bytes(&wrong_rate);
    assert!(matches!(
        read_err(&[("manifest.json", missing.as_bytes()), ("a.wav", &rate_wav)]),
        FileError::Project(_)
    ));

    let pat = manifest(r#","slots":[],"patterns":[{"number":1,"file":"p.bin"}]"#);
    let mut bad = Pattern::default().to_bytes();
    bad[0] = 0;
    assert!(matches!(
        read_err(&[("manifest.json", pat.as_bytes()), ("p.bin", &bad)]),
        FileError::Pattern { number: 1, .. }
    ));
    assert!(matches!(
        read_err(&[("manifest.json", pat.as_bytes()), ("p.bin", &[0; 10])]),
        FileError::Pattern { number: 1, .. }
    ));
    let out_of_range = manifest(r#","slots":[],"patterns":[{"number":11,"file":"p.bin"}]"#);
    let good = Pattern::default().to_bytes();
    assert!(matches!(
        read_err(&[("manifest.json", out_of_range.as_bytes()), ("p.bin", &good)]),
        FileError::Pattern { number: 11, .. }
    ));
}
