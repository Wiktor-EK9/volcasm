//! Loader tests on the Vosyr projects in `tests/fixtures/vosyr`. Expected
//! values come from the Vosyr screenshots and test notes stored next to the
//! projects (`docs/vsj-format.md`).

use std::path::{Path, PathBuf};

use volcasm_core::pattern::PartData;
use volcasm_vosyr_import::{Error, SEQUENCE_COUNT, VSJ_SIZE, VosyrProject, load};

fn fixture(rel: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../tests/fixtures/vosyr")
        .join(rel)
}

fn project(rel: &str) -> VosyrProject {
    load(&fixture(rel)).unwrap()
}

/// Knob values as shown by Vosyr, in its UI order: LEVEL, PAN, SPEED,
/// START POINT, LENGTH, AMP EG ATTACK, AMP EG DECAY, PITCH EG INT,
/// PITCH EG ATTACK, PITCH EG DECAY, HI CUT. Decoded from `Param[]` (SDK
/// order) with the +128 / +192 encoding; SPEED in continuous mode.
fn ui_params(part: &PartData) -> [i32; 11] {
    let p = part.Param.map(i32::from);
    let unsigned = |v: i32| v - 128;
    let bipolar = |v: i32| v - 192;
    [
        unsigned(p[0]),  // LEVEL
        bipolar(p[1]),   // PAN
        bipolar(p[2]),   // SPEED
        unsigned(p[8]),  // START POINT
        unsigned(p[9]),  // LENGTH
        unsigned(p[3]),  // AMP EG ATTACK
        unsigned(p[4]),  // AMP EG DECAY
        bipolar(p[5]),   // PITCH EG INT
        unsigned(p[6]),  // PITCH EG ATTACK
        unsigned(p[7]),  // PITCH EG DECAY
        unsigned(p[10]), // HI CUT
    ]
}

/// Steps (1-16) that are on in `bits`.
fn steps(bits: u16) -> Vec<u32> {
    (0..16)
        .filter(|i| bits & (1 << i) != 0)
        .map(|i| i + 1)
        .collect()
}

#[test]
fn test1_slots() {
    let p = project("Test1/Test1.vsj");
    assert_eq!(p.name, "Test1");
    let used: Vec<_> = p.index.used().collect();
    assert_eq!(used.len(), 25);
    assert_eq!(
        used.iter().map(|s| s.slot).collect::<Vec<_>>(),
        (0..25).collect::<Vec<_>>()
    );
    // Names as on the SAMPLES screen (shown in upper case there).
    assert_eq!(used[0].name, "GS_BLU_A_SynthBass_02");
    assert_eq!(used[0].file_name, "sample1791829490708");
    assert_eq!(used[7].name, "GS_BLU_Openhat_05");
    assert_eq!(used[16].name, "GS_BLU_Kick_02");
    assert_eq!(used[24].name, "GS_BLU_A#m_Synth_Stab");
    assert!(
        p.index.slots[25..]
            .iter()
            .all(|s| !s.used && s.name.is_empty())
    );
}

#[test]
fn test1_s1_p1_matches_screenshot() {
    let p = project("Test1/Test1.vsj");
    let s1 = &p.sequences[0];
    // ACTIVE STEP 13 off.
    assert_eq!(s1.ActiveStep, 0xefff);
    let p1 = &s1.Part[0];
    assert_eq!(p1.SampleNum, 0);
    assert_eq!(steps(p1.StepOn), [4, 7, 8, 11, 12, 14, 15]);
    assert_eq!(ui_params(p1), [127, 0, 0, 75, 127, 0, 31, -38, 0, 28, 64]);
}

#[test]
fn test1_s4_p8_matches_screenshot() {
    let p = project("Test1/Test1.vsj");
    let p8 = &p.sequences[3].Part[7];
    assert_eq!(p8.SampleNum, 0);
    assert_eq!(steps(p8.StepOn), [1, 3, 5, 8, 9, 11, 13]);
    assert_eq!(ui_params(p8), [127, 13, -59, 36, 50, 0, 37, 34, 77, 23, 21]);
}

#[test]
fn test1_s1_sample_numbers_match_screenshot() {
    let p = project("Test1/Test1.vsj");
    let samples: Vec<u16> = p.sequences[0].Part.iter().map(|p| p.SampleNum).collect();
    assert_eq!(samples, [0, 8, 16, 7, 16, 12, 19, 24, 14, 1]);
}

/// Needs the Test1 sample files, which are not in the repository (D12).
#[test]
#[ignore = "needs the third-party Test1 samples on disk"]
fn test1_total_duration_matches_ui() {
    let p = project("Test1/Test1.vsj");
    let frames: usize = p
        .index
        .used()
        .map(|s| p.load_sample(s.slot).unwrap().frames())
        .sum();
    // Frame count measured independently with Python's `wave` module:
    // 57.5049 s. Vosyr shows "DURATION: 57.51S", consistent with rounding
    // up (hypothesis, docs/vsj-format.md section 2).
    assert_eq!(frames, 1_797_029);
    let centiseconds = (frames as f64 / 31_250.0 * 100.0).ceil();
    assert_eq!(centiseconds, 5751.0);
}

#[test]
fn test2_part_functions_and_extremes() {
    let p = project("Test2/Test2.vsj");
    let s1 = &p.sequences[0];
    let func: Vec<u8> = s1.Part.iter().map(|p| p.FuncMemoryPart).collect();
    // P1 MOTION, P2 LOOP, P3 REVERB, P4 REVERSE, P5 MUTE, others default.
    assert_eq!(
        func,
        [0x11, 0x12, 0x14, 0x18, 0x00, 0x10, 0x10, 0x10, 0x10, 0x10]
    );
    // P6: START POINT 5.
    assert_eq!(ui_params(&s1.Part[5])[3], 5);
    // P9: sample 24, values alternating minimum and maximum.
    let p9 = &s1.Part[8];
    assert_eq!(p9.SampleNum, 24);
    assert_eq!(
        ui_params(p9),
        [127, -63, 63, 0, 127, 0, 127, -63, 127, 0, 127]
    );
    // ACTIVE STEP: S2 without steps 1 and 16, S3 without step 8.
    assert_eq!(p.sequences[1].ActiveStep, 0x7ffe);
    assert_eq!(p.sequences[2].ActiveStep, 0xff7f);
}

#[test]
fn test2_motion() {
    let p = project("Test2/Test2.vsj");
    let s1 = &p.sequences[0];
    // P7: LEVEL 69 on step 9 (row 0 = LEVEL start).
    assert_eq!(s1.Part[6].Motion[0][8], 128 + 69);
    // P8: AMP EG ATTACK 45 on step 6 (row 6), HI CUT 68 on step 14 (row 13).
    assert_eq!(s1.Part[7].Motion[6][5], 128 + 45);
    assert_eq!(s1.Part[7].Motion[13][13], 128 + 68);
}

#[test]
fn test2_samples_load() {
    let p = project("Test2/Test2.vsj");
    let used: Vec<_> = p.index.used().map(|s| s.slot).collect();
    assert!(!used.is_empty());
    for slot in used {
        let sample = p.load_sample(slot).unwrap();
        assert_eq!(sample.sample_rate, 31_250);
        assert!(sample.frames() > 0);
    }
}

#[test]
fn test3_slot_changes() {
    let used = |rel: &str| -> Vec<(usize, String, String)> {
        project(rel)
            .index
            .used()
            .map(|s| (s.slot, s.name.clone(), s.file_name.clone()))
            .collect()
    };
    assert!(used("Test3/B1/B1.vsj").is_empty());
    assert_eq!(
        used("Test3/B6/B6.vsj"),
        [(0, "Samp3".into(), "sample1791835382293".into())]
    );
    assert_eq!(
        used("Test3/B7/B7.vsj"),
        [(5, "Samp3".into(), "sample1791835520531".into())]
    );
    // Renamed: same file, new name.
    assert_eq!(
        used("Test3/B7r/B7r.vsj"),
        [(5, "Renamed123".into(), "sample1791835520531".into())]
    );
    assert_eq!(used("Test3/B8/B8.vsj").len(), 2);
    // Removed slot 5: the whole record is zeroed.
    let b8r = project("Test3/B8r/B8r.vsj");
    assert_eq!(b8r.index.used().map(|s| s.slot).collect::<Vec<_>>(), [0]);
    assert_eq!(b8r.index.slots[5].raw, [0; 215]);
    assert!(matches!(b8r.load_sample(5), Err(Error::SlotEmpty(5))));
    assert!(b8r.load_sample(0).is_ok());
}

/// Every file is kept byte for byte.
#[test]
fn raw_round_trip() {
    for rel in [
        "Test1/Test1.vsj",
        "Test2/Test2.vsj",
        "Test3/B1/B1.vsj",
        "Test3/B5/B5.vsj",
        "Test3/B8/B8.vsj",
    ] {
        let p = project(rel);
        let path = fixture(rel);
        assert_eq!(p.vsj, std::fs::read(&path).unwrap(), "{rel}");
        assert_eq!(p.vsj.len(), VSJ_SIZE);
        let index = std::fs::read(p.dir.join("samples/samples")).unwrap();
        assert_eq!(p.index.raw, index, "{rel}");
        for (slot, record) in p.index.slots.iter().enumerate() {
            assert_eq!(&index[4 + 215 * slot..4 + 215 * (slot + 1)], record.raw);
        }
        assert_eq!(p.sequences.len(), SEQUENCE_COUNT);
        for n in 1..=SEQUENCE_COUNT {
            let file = std::fs::read(p.dir.join(format!("sequences/sequence{n:02}"))).unwrap();
            assert_eq!(p.sequence_bytes(n).as_slice(), file, "{rel} sequence {n}");
        }
    }
}

/// Copies a fixture project to a temporary directory for corruption tests.
fn copy_project(rel_dir: &str, name: &str) -> PathBuf {
    let src = fixture(rel_dir);
    let dst = Path::new(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _ = std::fs::remove_dir_all(&dst);
    for sub in ["", "samples", "sequences"] {
        std::fs::create_dir_all(dst.join(sub)).unwrap();
        for entry in std::fs::read_dir(src.join(sub)).unwrap() {
            let entry = entry.unwrap();
            if entry.file_type().unwrap().is_file() {
                std::fs::copy(entry.path(), dst.join(sub).join(entry.file_name())).unwrap();
            }
        }
    }
    dst
}

#[test]
fn corrupt_projects_are_rejected() {
    let dir = copy_project("Test3/B6", "corrupt_header");
    let vsj = dir.join("B6.vsj");
    let mut data = std::fs::read(&vsj).unwrap();
    data[0] = b'X';
    std::fs::write(&vsj, &data).unwrap();
    assert!(matches!(load(&vsj), Err(Error::Marker { .. })));

    let dir = copy_project("Test3/B6", "corrupt_size");
    std::fs::write(dir.join("samples/samples"), [0u8; 10]).unwrap();
    assert!(matches!(load(&dir.join("B6.vsj")), Err(Error::Size { .. })));

    let dir = copy_project("Test3/B6", "missing_sequence");
    std::fs::remove_file(dir.join("sequences/sequence07")).unwrap();
    assert!(matches!(load(&dir.join("B6.vsj")), Err(Error::Io { .. })));

    let dir = copy_project("Test3/B6", "bad_pattern");
    let seq = dir.join("sequences/sequence03");
    let mut data = std::fs::read(&seq).unwrap();
    data[2620] = 0;
    std::fs::write(&seq, &data).unwrap();
    assert!(matches!(
        load(&dir.join("B6.vsj")),
        Err(Error::Pattern { .. })
    ));

    let dir = copy_project("Test3/B6", "missing_sample");
    std::fs::remove_file(dir.join("samples/sample1791835382293")).unwrap();
    let p = load(&dir.join("B6.vsj")).unwrap();
    assert!(matches!(
        p.load_sample(0),
        Err(Error::Sample { slot: 0, .. })
    ));
    assert!(matches!(
        p.load_sample(100),
        Err(Error::SlotOutOfRange(100))
    ));
}
