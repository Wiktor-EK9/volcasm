//! One part (track) of a pattern: sample, steps, function flags, knob
//! parameters and motion data. Encodings in `docs/vsj-format.md`, section 4.

use std::fmt;

use super::PartData;

/// Number of steps per part.
pub const STEPS: usize = 16;
/// Number of motion rows.
pub const MOTION_ROWS: usize = 14;

/// Bit 7 of a parameter byte. Vosyr and the KORG presets set it on every
/// parameter; the device ignores it except for SPEED, where it selects the
/// continuous mode (confirmed on hardware).
const HIGH_BIT: u8 = 0x80;
/// Bipolar values are stored as `64 + value`.
const BIPOLAR_CENTER: i16 = 64;
/// `FuncMemoryPart` bit 4: 1 = the part plays (README: "1=mute off").
const PLAY_BIT: u8 = 1 << 4;

/// Knob parameters other than SPEED, with their index in `Param[]` (SDK).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Param {
    Level = 0,
    Pan = 1,
    AmpEgAttack = 3,
    AmpEgDecay = 4,
    PitchEgInt = 5,
    PitchEgAttack = 6,
    PitchEgDecay = 7,
    StartPoint = 8,
    Length = 9,
    HiCut = 10,
}

/// Index of SPEED in `Param[]`.
const SPEED_INDEX: usize = 2;

impl Param {
    pub const ALL: [Param; 10] = [
        Param::Level,
        Param::Pan,
        Param::AmpEgAttack,
        Param::AmpEgDecay,
        Param::PitchEgInt,
        Param::PitchEgAttack,
        Param::PitchEgDecay,
        Param::StartPoint,
        Param::Length,
        Param::HiCut,
    ];

    /// PAN and PITCH EG INT are centered (-63..=63), the rest 0..=127.
    pub fn is_bipolar(self) -> bool {
        matches!(self, Param::Pan | Param::PitchEgInt)
    }

    /// Valid range of the value shown on the device.
    pub fn range(self) -> (i8, i8) {
        if self.is_bipolar() {
            (-63, 63)
        } else {
            (0, 127)
        }
    }
}

/// SPEED: continuous (-63..=63) or in semitones (-24..=24, FUNC+SPEED).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Speed {
    Continuous(i8),
    Semitones(i8),
}

/// Function flags in `FuncMemoryPart` (bits 0-3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Func {
    Motion = 0,
    Loop = 1,
    Reverb = 2,
    Reverse = 3,
}

/// Motion rows (`VOLCASAMPLE_MOTION_*`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MotionRow {
    LevelStart = 0,
    LevelEnd = 1,
    PanStart = 2,
    PanEnd = 3,
    SpeedStart = 4,
    SpeedEnd = 5,
    AmpEgAttack = 6,
    AmpEgDecay = 7,
    PitchEgInt = 8,
    PitchEgAttack = 9,
    PitchEgDecay = 10,
    StartPoint = 11,
    Length = 12,
    HiCut = 13,
}

impl MotionRow {
    /// The parameter a row records; `None` for the SPEED rows, whose
    /// encoding (raw 0..127 per the SDK README) is not observed yet.
    pub fn param(self) -> Option<Param> {
        Some(match self {
            Self::LevelStart | Self::LevelEnd => Param::Level,
            Self::PanStart | Self::PanEnd => Param::Pan,
            Self::SpeedStart | Self::SpeedEnd => return None,
            Self::AmpEgAttack => Param::AmpEgAttack,
            Self::AmpEgDecay => Param::AmpEgDecay,
            Self::PitchEgInt => Param::PitchEgInt,
            Self::PitchEgAttack => Param::PitchEgAttack,
            Self::PitchEgDecay => Param::PitchEgDecay,
            Self::StartPoint => Param::StartPoint,
            Self::Length => Param::Length,
            Self::HiCut => Param::HiCut,
        })
    }
}

/// A value outside the allowed range.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RangeError {
    pub what: &'static str,
    pub value: i32,
    pub min: i32,
    pub max: i32,
}

impl fmt::Display for RangeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} {} out of range {}..={}",
            self.what, self.value, self.min, self.max
        )
    }
}

impl std::error::Error for RangeError {}

/// Error setting a motion value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MotionError {
    Range(RangeError),
    /// SPEED rows are only available raw ([`Part::set_raw_motion`]).
    SpeedRow,
}

impl fmt::Display for MotionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Range(err) => write!(f, "{err}"),
            Self::SpeedRow => write!(f, "SPEED motion rows are only available raw"),
        }
    }
}

impl std::error::Error for MotionError {}

fn check(what: &'static str, value: i32, min: i32, max: i32) -> Result<(), RangeError> {
    if (min..=max).contains(&value) {
        Ok(())
    } else {
        Err(RangeError {
            what,
            value,
            min,
            max,
        })
    }
}

fn decode(param: Param, byte: u8) -> i8 {
    let value = i16::from(byte & !HIGH_BIT);
    if param.is_bipolar() {
        (value - BIPOLAR_CENTER) as i8
    } else {
        value as i8
    }
}

fn encode(param: Param, value: i8) -> Result<u8, RangeError> {
    let (min, max) = param.range();
    check("parameter value", value.into(), min.into(), max.into())?;
    let raw = if param.is_bipolar() {
        i16::from(value) + BIPOLAR_CENTER
    } else {
        i16::from(value)
    };
    Ok(HIGH_BIT | raw as u8)
}

/// A part of a pattern. Same layout as the SDK structure.
#[repr(transparent)]
#[derive(Debug, Clone, Copy)]
pub struct Part(pub(super) PartData);

impl Part {
    /// The SDK structure, with all fields including reserved ones.
    pub fn data(&self) -> &PartData {
        &self.0
    }

    pub fn data_mut(&mut self) -> &mut PartData {
        &mut self.0
    }

    /// Sample slot played by the part.
    pub fn sample(&self) -> u16 {
        self.0.SampleNum
    }

    pub fn set_sample(&mut self, slot: u16) {
        self.0.SampleNum = slot;
    }

    /// Step on/off bits, bit n = step n+1.
    pub fn steps(&self) -> u16 {
        self.0.StepOn
    }

    pub fn set_steps(&mut self, bits: u16) {
        self.0.StepOn = bits;
    }

    /// Whether step `step` (1-16) is on.
    pub fn step(&self, step: usize) -> bool {
        self.0.StepOn & step_bit(step) != 0
    }

    pub fn set_step(&mut self, step: usize, on: bool) {
        if on {
            self.0.StepOn |= step_bit(step);
        } else {
            self.0.StepOn &= !step_bit(step);
        }
    }

    pub fn func(&self, func: Func) -> bool {
        self.0.FuncMemoryPart & (1 << func as u8) != 0
    }

    pub fn set_func(&mut self, func: Func, on: bool) {
        set_bit(&mut self.0.FuncMemoryPart, 1 << func as u8, on);
    }

    /// Muted parts have bit 4 of `FuncMemoryPart` cleared.
    pub fn muted(&self) -> bool {
        self.0.FuncMemoryPart & PLAY_BIT == 0
    }

    pub fn set_muted(&mut self, muted: bool) {
        set_bit(&mut self.0.FuncMemoryPart, PLAY_BIT, !muted);
    }

    /// Value of `param` as shown on the device.
    pub fn param(&self, param: Param) -> i8 {
        decode(param, self.0.Param[param as usize])
    }

    /// Sets `param`; stored with bit 7 set, like Vosyr.
    pub fn set_param(&mut self, param: Param, value: i8) -> Result<(), RangeError> {
        self.0.Param[param as usize] = encode(param, value)?;
        Ok(())
    }

    pub fn speed(&self) -> Speed {
        let byte = self.0.Param[SPEED_INDEX];
        let value = (i16::from(byte & !HIGH_BIT) - BIPOLAR_CENTER) as i8;
        if byte & HIGH_BIT != 0 {
            Speed::Continuous(value)
        } else {
            Speed::Semitones(value)
        }
    }

    pub fn set_speed(&mut self, speed: Speed) -> Result<(), RangeError> {
        self.0.Param[SPEED_INDEX] = match speed {
            Speed::Continuous(v) => {
                check("speed", v.into(), -63, 63)?;
                HIGH_BIT | (i16::from(v) + BIPOLAR_CENTER) as u8
            }
            Speed::Semitones(v) => {
                check("speed in semitones", v.into(), -24, 24)?;
                (i16::from(v) + BIPOLAR_CENTER) as u8
            }
        };
        Ok(())
    }

    /// Motion value of `row` at `step` (1-16), `None` when the step has no
    /// motion data. SPEED rows return `None` too; use [`Part::raw_motion`].
    pub fn motion(&self, row: MotionRow, step: usize) -> Option<i8> {
        let param = row.param()?;
        match self.raw_motion(row, step) {
            0 => None,
            byte => Some(decode(param, byte)),
        }
    }

    /// Sets or clears a motion value. Not available for the SPEED rows.
    pub fn set_motion(
        &mut self,
        row: MotionRow,
        step: usize,
        value: Option<i8>,
    ) -> Result<(), MotionError> {
        let param = row.param().ok_or(MotionError::SpeedRow)?;
        let byte = match value {
            Some(v) => encode(param, v).map_err(MotionError::Range)?,
            None => 0,
        };
        self.set_raw_motion(row, step, byte);
        Ok(())
    }

    /// Raw motion byte of `row` at `step` (1-16); 0 = no data.
    pub fn raw_motion(&self, row: MotionRow, step: usize) -> u8 {
        self.0.Motion[row as usize][step_index(step)]
    }

    pub fn set_raw_motion(&mut self, row: MotionRow, step: usize, byte: u8) {
        self.0.Motion[row as usize][step_index(step)] = byte;
    }
}

fn step_index(step: usize) -> usize {
    assert!((1..=STEPS).contains(&step), "step {step} out of range 1-16");
    step - 1
}

fn step_bit(step: usize) -> u16 {
    1 << step_index(step)
}

fn set_bit(byte: &mut u8, mask: u8, on: bool) {
    if on {
        *byte |= mask;
    } else {
        *byte &= !mask;
    }
}
