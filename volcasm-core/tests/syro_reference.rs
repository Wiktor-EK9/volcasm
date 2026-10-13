//! Bit-exact comparison of the SYRO stream WAV with the unmodified SDK
//! example (`syro_volcasample_example`) for the same input.

use std::path::{Path, PathBuf};
use std::process::Command;

use volcasm_core::syro::{PATTERN_SIZE, SyroError, SyroItem, encode};
use volcasm_core::wav::stream_to_wav;
use volcasm_syro_sys::REFERENCE_EXAMPLE_BIN;

const PATTERN_HEADER: &[u8; 4] = b"PTST";
const PATTERN_FOOTER: &[u8; 4] = b"PTED";
const PATTERN_DEVCODE: u16 = 0x33b8;

fn fixture(rel: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../tests/fixtures/vosyr")
        .join(rel)
}

/// Own recordings from Test2 (16-bit mono 31 250 Hz).
fn sample_a() -> PathBuf {
    fixture("Test2/samples/sample1791833706583")
}

fn sample_b() -> PathBuf {
    fixture("Test2/samples/sample1791833712136")
}

fn read_pcm(path: &Path) -> (Vec<i16>, u32) {
    let mut reader = hound::WavReader::open(path).unwrap();
    let spec = reader.spec();
    assert_eq!((spec.channels, spec.bits_per_sample), (1, 16), "{path:?}");
    let pcm = reader.samples::<i16>().map(Result::unwrap).collect();
    (pcm, spec.sample_rate)
}

fn read_pattern(path: &Path) -> [u8; PATTERN_SIZE] {
    let data: [u8; PATTERN_SIZE] = std::fs::read(path).unwrap().try_into().unwrap();
    assert_eq!(&data[0..4], PATTERN_HEADER);
    assert_eq!(u16::from_le_bytes([data[4], data[5]]), PATTERN_DEVCODE);
    assert_eq!(&data[PATTERN_SIZE - 4..], PATTERN_FOOTER);
    data
}

/// Runs the SDK example with `args` (its own syntax, e.g. `p1:file`) and
/// returns the generated WAV.
fn reference_wav(name: &str, args: &[String]) -> Vec<u8> {
    let out = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("{name}.wav"));
    let _ = std::fs::remove_file(&out);
    let output = Command::new(REFERENCE_EXAMPLE_BIN)
        .arg(&out)
        .args(args)
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        output.status.success() && stdout.contains("Complete to convert"),
        "SDK example failed:\n{stdout}"
    );
    std::fs::read(&out).unwrap()
}

fn assert_same_wav(ours: &[u8], reference: &[u8]) {
    assert_eq!(ours.len(), reference.len(), "WAV size differs");
    if let Some(pos) = ours.iter().zip(reference).position(|(a, b)| a != b) {
        panic!("WAV differs at byte {pos} of {}", ours.len());
    }
}

fn arg(prefix: &str, path: &Path) -> String {
    format!("{prefix}:{}", path.display())
}

#[test]
fn pattern_matches_sdk_example() {
    let path = fixture("Test1/sequences/sequence01");
    let data = read_pattern(&path);
    let item = SyroItem::Pattern {
        index: 0,
        data: &data,
    };
    let ours = stream_to_wav(&encode(&[item]).unwrap());
    let reference = reference_wav("pattern_p1", &[arg("p1", &path)]);
    assert_same_wav(&ours, &reference);
}

#[test]
fn sample_matches_sdk_example() {
    let (pcm, sample_rate) = read_pcm(&sample_a());
    assert_eq!(sample_rate, 31_250);
    let item = SyroItem::Sample {
        slot: 0,
        pcm: &pcm,
        sample_rate,
    };
    let ours = stream_to_wav(&encode(&[item]).unwrap());
    let reference = reference_wav("sample_s0", &[arg("s0", &sample_a())]);
    assert_same_wav(&ours, &reference);
}

#[test]
fn compressed_sample_matches_sdk_example() {
    let (pcm, sample_rate) = read_pcm(&sample_b());
    let item = SyroItem::SampleCompressed {
        slot: 42,
        pcm: &pcm,
        sample_rate,
        bits: 8,
    };
    let ours = stream_to_wav(&encode(&[item]).unwrap());
    let reference = reference_wav("sample_s42c8", &[arg("s42c8", &sample_b())]);
    assert_same_wav(&ours, &reference);
}

#[test]
fn erase_matches_sdk_example() {
    let ours = stream_to_wav(&encode(&[SyroItem::Erase { slot: 5 }]).unwrap());
    let reference = reference_wav("erase_e5", &["e5:".to_string()]);
    assert_same_wav(&ours, &reference);
}

#[test]
fn mixed_stream_matches_sdk_example() {
    let (pcm_a, fs_a) = read_pcm(&sample_a());
    let (pcm_b, fs_b) = read_pcm(&sample_b());
    let pattern_path = fixture("Test2/sequences/sequence09");
    let pattern = read_pattern(&pattern_path);
    let items = [
        SyroItem::Sample {
            slot: 3,
            pcm: &pcm_a,
            sample_rate: fs_a,
        },
        SyroItem::SampleCompressed {
            slot: 99,
            pcm: &pcm_b,
            sample_rate: fs_b,
            bits: 12,
        },
        SyroItem::Erase { slot: 7 },
        SyroItem::Pattern {
            index: 9,
            data: &pattern,
        },
    ];
    let ours = stream_to_wav(&encode(&items).unwrap());
    let reference = reference_wav(
        "mixed",
        &[
            arg("s3", &sample_a()),
            arg("s99c12", &sample_b()),
            "e7:".to_string(),
            arg("p10", &pattern_path),
        ],
    );
    assert_same_wav(&ours, &reference);
}

#[test]
fn out_of_range_slot_is_rejected() {
    let err = encode(&[SyroItem::Erase { slot: 100 }]).unwrap_err();
    assert_eq!(err, SyroError::OutOfRangeNumber);
}
