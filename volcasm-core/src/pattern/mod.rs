//! Patterns (sequences): the SDK structure `VolcaSample_Pattern_Data`, its
//! raw bytes (as stored by Vosyr and sent by SYRO) and a typed API.

mod part;

pub use part::{Func, MOTION_ROWS, MotionError, MotionRow, Param, Part, RangeError, STEPS, Speed};

use std::fmt;

pub use sys::{VolcaSample_Part_Data as PartData, VolcaSample_Pattern_Data as PatternData};
use volcasm_syro_sys as sys;

use crate::syro::PATTERN_SIZE;

// The raw format is little endian and the SDK structure uses native fields.
#[cfg(target_endian = "big")]
compile_error!("pattern conversion assumes a little-endian target");

// Every byte of the structure belongs to a field (no implicit padding): see
// the field sizes in `volcasm-syro-sys/tests/layout.rs`.
const _: () = assert!(size_of::<PatternData>() == PATTERN_SIZE);

/// Invalid raw pattern data.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PatternError {
    Header(u32),
    DevCode(u16),
    Footer(u32),
}

impl fmt::Display for PatternError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Header(v) => write!(f, "invalid pattern header {v:#010x}"),
            Self::DevCode(v) => write!(f, "invalid pattern device code {v:#06x}"),
            Self::Footer(v) => write!(f, "invalid pattern footer {v:#010x}"),
        }
    }
}

impl std::error::Error for PatternError {}

/// Reads raw pattern bytes into the SDK structure and checks the header,
/// device code and footer.
pub fn from_bytes(bytes: &[u8; PATTERN_SIZE]) -> Result<PatternData, PatternError> {
    // SAFETY: `PatternData` is `repr(C)`, consists only of integer fields and
    // arrays of them (any bit pattern is valid) and is exactly PATTERN_SIZE
    // bytes long; `read_unaligned` imposes no alignment requirement.
    let data: PatternData = unsafe { bytes.as_ptr().cast::<PatternData>().read_unaligned() };
    if data.Header != sys::VOLCASAMPLE_PATTERN_HEADER {
        return Err(PatternError::Header(data.Header));
    }
    if u32::from(data.DevCode) != sys::VOLCASAMPLE_PATTERN_DEVCODE {
        return Err(PatternError::DevCode(data.DevCode));
    }
    if data.Footer != sys::VOLCASAMPLE_PATTERN_FOOTER {
        return Err(PatternError::Footer(data.Footer));
    }
    Ok(data)
}

/// Writes the SDK structure as raw pattern bytes.
pub fn to_bytes(data: &PatternData) -> [u8; PATTERN_SIZE] {
    // SAFETY: same size, and the structure has no padding bytes, so every
    // byte read is initialized.
    unsafe {
        std::ptr::from_ref(data)
            .cast::<[u8; PATTERN_SIZE]>()
            .read_unaligned()
    }
}

/// Number of parts per pattern.
pub const PARTS: usize = 10;

/// Values Vosyr and the KORG presets store in fields the SDK calls
/// `Reserved` and `Padding1` (identical in all 120 fixture sequences and
/// in the presets); meaning unknown.
const FACTORY_RESERVED: [u8; 2] = [0xff, 0x00];
const FACTORY_PADDING1: [u8; 22] = [
    0x7f, 0xff, 0xff, 0xff, 0xf7, 0xff, 0xff, 0x7f, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
    0xff, 0xff, 0xff, 0xef, 0xff, 0xff,
];
/// `Padding2`: 0xff in all fixtures.
const FACTORY_PADDING2: u8 = 0xff;
/// Default `Param[]`: `VolcaSample_Pattern_Init` values with bit 7 set, as
/// Vosyr stores them.
const DEFAULT_PARAMS: [u8; 11] = [
    0xff, 0xc0, 0xc0, 0x80, 0xff, 0xc0, 0x80, 0xff, 0x80, 0xff, 0xff,
];

/// A pattern. Holds the complete SDK structure, so fields without an API
/// (reserved, padding) are kept as they are.
#[derive(Clone, Copy)]
pub struct Pattern(PatternData);

impl Default for Pattern {
    /// An empty pattern as Vosyr creates it (`Test3/B1`): no steps, sample
    /// 0, default knobs, parts playing, all steps active.
    fn default() -> Self {
        // SAFETY: all fields are integers or arrays of them; zero is valid.
        let mut data: PatternData = unsafe { std::mem::zeroed() };
        data.Header = sys::VOLCASAMPLE_PATTERN_HEADER;
        data.DevCode = sys::VOLCASAMPLE_PATTERN_DEVCODE as u16;
        data.Reserved = FACTORY_RESERVED;
        data.ActiveStep = 0xffff;
        data.Padding1 = FACTORY_PADDING1;
        for part in &mut data.Part {
            part.Level = 127;
            part.Param = DEFAULT_PARAMS;
            part.FuncMemoryPart = sys::VOLCASAMPLE_FUNC_MUTE as u8;
        }
        data.Padding2 = [FACTORY_PADDING2; 28];
        data.Footer = sys::VOLCASAMPLE_PATTERN_FOOTER;
        Self(data)
    }
}

impl fmt::Debug for Pattern {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Pattern")
            .field("active_steps", &format_args!("{:#06x}", self.0.ActiveStep))
            .field(
                "parts",
                &self
                    .parts()
                    .map(|p| (p.sample(), p.steps()))
                    .collect::<Vec<_>>(),
            )
            .finish()
    }
}

impl PartialEq for Pattern {
    fn eq(&self, other: &Self) -> bool {
        self.to_bytes() == other.to_bytes()
    }
}

impl Eq for Pattern {}

/// A pattern that the device would not accept as is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PatternIssue {
    /// A part plays a slot the device does not have.
    SampleOutOfRange { part: usize, sample: u16 },
    /// `Level` must be 127 (SDK README).
    Level { part: usize, level: u8 },
    /// `Accent` must be 0 (SDK README).
    Accent { part: usize, accent: u16 },
    /// At least one step must be active (SDK README).
    NoActiveStep,
}

impl fmt::Display for PatternIssue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SampleOutOfRange { part, sample } => {
                write!(f, "part {}: sample {sample} out of range", part + 1)
            }
            Self::Level { part, level } => {
                write!(f, "part {}: level {level}, must be 127", part + 1)
            }
            Self::Accent { part, accent } => {
                write!(f, "part {}: accent {accent}, must be 0", part + 1)
            }
            Self::NoActiveStep => write!(f, "no active step"),
        }
    }
}

impl Pattern {
    /// Reads raw pattern bytes (header, device code and footer checked).
    pub fn from_bytes(bytes: &[u8; PATTERN_SIZE]) -> Result<Self, PatternError> {
        from_bytes(bytes).map(Self)
    }

    pub fn to_bytes(&self) -> [u8; PATTERN_SIZE] {
        to_bytes(&self.0)
    }

    /// The SDK structure, with all fields.
    pub fn data(&self) -> &PatternData {
        &self.0
    }

    pub fn data_mut(&mut self) -> &mut PatternData {
        &mut self.0
    }

    /// Active step bits (ACTIVE STEP), bit n = step n+1.
    pub fn active_steps(&self) -> u16 {
        self.0.ActiveStep
    }

    pub fn set_active_steps(&mut self, bits: u16) {
        self.0.ActiveStep = bits;
    }

    /// Part `index` (0-9).
    pub fn part(&self, index: usize) -> &Part {
        // SAFETY: `Part` is `repr(transparent)` over `PartData`.
        unsafe { &*std::ptr::from_ref(&self.0.Part[index]).cast::<Part>() }
    }

    pub fn part_mut(&mut self, index: usize) -> &mut Part {
        // SAFETY: `Part` is `repr(transparent)` over `PartData`.
        unsafe { &mut *std::ptr::from_mut(&mut self.0.Part[index]).cast::<Part>() }
    }

    pub fn parts(&self) -> impl Iterator<Item = &Part> {
        (0..PARTS).map(|i| self.part(i))
    }

    /// Problems that would make the device reject or misplay the pattern,
    /// for a device with `sample_slots` slots.
    pub fn issues(&self, sample_slots: u32) -> Vec<PatternIssue> {
        let mut issues = Vec::new();
        if self.0.ActiveStep == 0 {
            issues.push(PatternIssue::NoActiveStep);
        }
        for (part, data) in self.0.Part.iter().enumerate() {
            if u32::from(data.SampleNum) >= sample_slots {
                issues.push(PatternIssue::SampleOutOfRange {
                    part,
                    sample: data.SampleNum,
                });
            }
            if data.Level != 127 {
                issues.push(PatternIssue::Level {
                    part,
                    level: data.Level,
                });
            }
            if data.Accent != 0 {
                issues.push(PatternIssue::Accent {
                    part,
                    accent: data.Accent,
                });
            }
        }
        issues
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(rel: &str) -> [u8; PATTERN_SIZE] {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../tests/fixtures/vosyr")
            .join(rel);
        std::fs::read(path).unwrap().try_into().unwrap()
    }

    #[test]
    fn round_trip() {
        for rel in ["Test1/sequences/sequence01", "Test2/sequences/sequence09"] {
            let bytes = fixture(rel);
            assert_eq!(to_bytes(&from_bytes(&bytes).unwrap()), bytes, "{rel}");
        }
    }

    #[test]
    fn reads_fields() {
        let data = from_bytes(&fixture("Test1/sequences/sequence01")).unwrap();
        assert_eq!(data.ActiveStep, 0xefff);
        assert_eq!(data.Part[0].SampleNum, 0);
    }

    #[test]
    fn default_is_vosyr_empty_pattern() {
        let empty = fixture("../vosyr/Test3/B1/sequences/sequence01");
        assert_eq!(Pattern::default().to_bytes(), empty);
        assert!(Pattern::default().issues(100).is_empty());
    }

    fn test1_s1() -> Pattern {
        Pattern::from_bytes(&fixture("Test1/sequences/sequence01")).unwrap()
    }

    #[test]
    fn typed_params_match_screenshot() {
        // Test1 S1 P1 as shown by Vosyr.
        let pattern = test1_s1();
        let p1 = pattern.part(0);
        let values: Vec<i8> = Param::ALL.iter().map(|&p| p1.param(p)).collect();
        // LEVEL, PAN, AMP ATK, AMP DEC, PITCH INT, PITCH ATK, PITCH DEC,
        // START, LENGTH, HI CUT.
        assert_eq!(values, [127, 0, 0, 31, -38, 0, 28, 75, 127, 64]);
        assert_eq!(p1.speed(), Speed::Continuous(0));
        assert!(p1.step(4) && p1.step(15) && !p1.step(1));
        assert!(!p1.muted());
    }

    #[test]
    fn edits_round_trip_and_keep_other_bytes() {
        let original = test1_s1();
        let mut pattern = original;
        let p = pattern.part_mut(2);
        let old_pan = p.param(Param::Pan);
        p.set_param(Param::Pan, -63).unwrap();
        assert_eq!(p.param(Param::Pan), -63);
        assert_eq!(p.data().Param[1], 0x81);
        p.set_param(Param::Pan, old_pan).unwrap();
        assert_eq!(pattern, original);

        let p = pattern.part_mut(0);
        p.set_func(Func::Reverb, true);
        p.set_muted(true);
        assert_eq!(p.data().FuncMemoryPart, 0x04);
        p.set_func(Func::Reverb, false);
        p.set_muted(false);
        assert_eq!(pattern, original);
    }

    #[test]
    fn param_ranges() {
        let mut pattern = Pattern::default();
        let p = pattern.part_mut(0);
        assert!(p.set_param(Param::Level, -1).is_err());
        assert!(p.set_param(Param::Pan, -64).is_err());
        assert!(p.set_param(Param::Pan, 63).is_ok());
        assert!(p.set_speed(Speed::Semitones(25)).is_err());
        assert!(p.set_speed(Speed::Continuous(-64)).is_err());
    }

    #[test]
    fn speed_modes_match_hardware_test() {
        // Bytes from a hardware test of the SPEED encoding.
        let mut pattern = Pattern::default();
        let p = pattern.part_mut(0);
        p.set_speed(Speed::Semitones(12)).unwrap();
        assert_eq!(p.data().Param[2], 0x4c);
        p.set_speed(Speed::Continuous(12)).unwrap();
        assert_eq!(p.data().Param[2], 0xcc);
        assert_eq!(p.speed(), Speed::Continuous(12));
        p.data_mut().Param[2] = 0x40;
        assert_eq!(p.speed(), Speed::Semitones(0));
    }

    #[test]
    fn bit7_is_ignored_on_read() {
        // Raw VolcaSample_Pattern_Init PAN 0x7f plays right +63 (hardware test).
        let mut pattern = Pattern::default();
        pattern.part_mut(0).data_mut().Param[1] = 0x7f;
        assert_eq!(pattern.part(0).param(Param::Pan), 63);
    }

    #[test]
    fn motion_matches_test2() {
        let pattern = Pattern::from_bytes(&fixture("Test2/sequences/sequence01")).unwrap();
        assert_eq!(pattern.part(6).motion(MotionRow::LevelStart, 9), Some(69));
        assert_eq!(pattern.part(6).motion(MotionRow::LevelStart, 8), None);
        assert_eq!(pattern.part(7).motion(MotionRow::AmpEgAttack, 6), Some(45));
        assert_eq!(pattern.part(7).motion(MotionRow::HiCut, 14), Some(68));

        let mut edited = Pattern::default();
        let p = edited.part_mut(6);
        p.set_motion(MotionRow::LevelStart, 9, Some(69)).unwrap();
        assert_eq!(p.raw_motion(MotionRow::LevelStart, 9), 197);
        assert_eq!(
            p.set_motion(MotionRow::SpeedStart, 1, Some(0)),
            Err(MotionError::SpeedRow)
        );
    }

    #[test]
    fn issues_are_reported() {
        let mut pattern = Pattern::default();
        pattern.set_active_steps(0);
        pattern.part_mut(3).set_sample(100);
        pattern.part_mut(4).data_mut().Level = 100;
        assert_eq!(
            pattern.issues(100),
            [
                PatternIssue::NoActiveStep,
                PatternIssue::SampleOutOfRange {
                    part: 3,
                    sample: 100
                },
                PatternIssue::Level {
                    part: 4,
                    level: 100
                },
            ]
        );
    }

    #[test]
    fn rejects_invalid_markers() {
        let good = fixture("Test1/sequences/sequence01");
        let mut bad = good;
        bad[0] = b'X';
        assert!(matches!(from_bytes(&bad), Err(PatternError::Header(_))));
        let mut bad = good;
        bad[4] = 0;
        assert!(matches!(from_bytes(&bad), Err(PatternError::DevCode(_))));
        let mut bad = good;
        bad[PATTERN_SIZE - 1] = 0;
        assert!(matches!(from_bytes(&bad), Err(PatternError::Footer(_))));
    }
}
