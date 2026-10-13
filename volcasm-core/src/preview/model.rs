//! How knob values map to sound in the preview. The volca sample's engine
//! is not documented; everything here is an assumption unless marked
//! confirmed (`docs/hardware.md`, section 4). Calibration changes only this
//! file.

/// Steps per beat: the 16 steps are sixteenth notes. Certain.
pub const STEPS_PER_BEAT: f64 = 4.0;

/// Playback rate factor for SPEED in semitones. Confirmed: +12 is an
/// octave (hardware test `t3_speed`).
pub fn semitone_ratio(semitones: f64) -> f64 {
    2f64.powf(semitones / 12.0)
}

/// Semitones per unit of continuous SPEED. HYPOTHESIS: the full range
/// ±63 spans ±1 octave (in `t3_speed`, continuous +12 sounded only a little
/// higher).
pub const CONTINUOUS_SPEED_SEMITONES_PER_UNIT: f64 = 12.0 / 63.0;

/// START POINT 0..127 as a fraction of the sample. HYPOTHESIS: linear.
pub fn start_fraction(value: i8) -> f64 {
    f64::from(value) / 127.0
}

/// LENGTH 0..127 as a fraction of the rest after the start point.
/// HYPOTHESIS: linear.
pub fn length_fraction(value: i8) -> f64 {
    f64::from(value) / 127.0
}

/// LEVEL 0..127 as linear gain. HYPOTHESIS.
pub fn level_gain(value: f64) -> f64 {
    value / 127.0
}

/// PAN -63..63 as (left, right) gains, equal power. HYPOTHESIS.
pub fn pan_gains(value: f64) -> (f64, f64) {
    let angle = (value + 63.0) / 126.0 * std::f64::consts::FRAC_PI_2;
    (angle.cos(), angle.sin())
}

/// Attack time for a knob value 0..127, in seconds. HYPOTHESIS: quadratic
/// from 0 (instant) to 4 s.
pub fn attack_seconds(value: i8) -> f64 {
    let t = f64::from(value) / 127.0;
    t * t * 4.0
}

/// Decay time for a knob value 0..127, in seconds. HYPOTHESIS: quadratic
/// from 1 ms to 4 s.
pub fn decay_seconds(value: i8) -> f64 {
    0.001 + attack_seconds(value)
}

/// Whether AMP EG DECAY holds the level (no decay). HYPOTHESIS: the
/// maximum, which is also the default.
pub fn decay_holds(value: i8) -> bool {
    value == i8::MAX
}

/// PITCH EG INT -63..63 as semitones at the envelope peak. HYPOTHESIS:
/// ±63 = ±2 octaves.
pub fn pitch_eg_semitones(value: f64) -> f64 {
    value / 63.0 * 24.0
}

/// HI CUT 0..127 as a low-pass cutoff in Hz; `None` = open. HYPOTHESIS:
/// exponential from 100 Hz to 16 kHz, open at 127.
pub fn hi_cut_hz(value: f64) -> Option<f64> {
    (value < 127.0).then(|| 100.0 * 160f64.powf(value / 127.0))
}

/// Reverb send level of parts with REVERB on. HYPOTHESIS (the device's
/// reverb level is a global setting not read from projects yet).
pub const REVERB_SEND: f64 = 0.35;

/// Mix gain before the soft limiter, so ten parts at full level do not
/// clip hard.
pub const MASTER_GAIN: f64 = 0.5;
