//! Sequence preview: an approximate model of the volca sample's playback,
//! rendered offline or in real time. Deterministic and free of I/O.
//!
//! The device's sound engine is not documented, so this is a preview, not
//! an emulation; the assumptions live in [`model`].

mod dsp;
pub mod model;

use std::sync::Arc;

use crate::pattern::{Func, MotionRow, PARTS, Param, Part, Pattern, STEPS, Speed};
use dsp::{Envelope, LowPass, Reverb};

/// One part's sound for one step.
struct Voice {
    sample: Arc<[i16]>,
    /// Region of the sample, frames `start..end`.
    start: usize,
    len: usize,
    /// Position in the region, in sample frames.
    pos: f64,
    reverse: bool,
    looped: bool,
    /// Sample frames per output frame at speed 0.
    base_rate: f64,
    /// Start and end values over the step (motion interpolates).
    speed: (f64, f64),
    level: (f64, f64),
    pan: (f64, f64),
    pitch_int: f64,
    amp_env: Envelope,
    pitch_env: Envelope,
    cutoff: Option<f64>,
    filter: LowPass,
    reverb: bool,
    /// Output frames since the trigger.
    age: f64,
    step_frames: f64,
}

impl Voice {
    /// Next output frame: (left, right, reverb send). `None` when finished.
    fn next(&mut self, out_rate: f64) -> Option<(f64, f64, f64)> {
        if self.len == 0 {
            return None;
        }
        if self.pos >= self.len as f64 {
            if !self.looped {
                return None;
            }
            self.pos %= self.len as f64;
        }
        let i = self.pos.floor() as usize;
        let frac = self.pos - i as f64;
        let at = |k: usize| -> f64 {
            let k = k.min(self.len - 1);
            let index = if self.reverse {
                self.start + self.len - 1 - k
            } else {
                self.start + k
            };
            f64::from(self.sample[index]) / 32_768.0
        };
        let next_index = if i + 1 < self.len {
            i + 1
        } else if self.looped {
            0
        } else {
            i
        };
        let mut x = at(i) * (1.0 - frac) + at(next_index) * frac;

        let dt = 1.0 / out_rate;
        let t = (self.age / self.step_frames).min(1.0);
        let lerp = |(a, b): (f64, f64)| a + (b - a) * t;
        let amp = self.amp_env.next(dt) * model::level_gain(lerp(self.level));
        let pitch = lerp(self.speed) + self.pitch_int * self.pitch_env.next(dt);
        self.pos += self.base_rate * model::semitone_ratio(pitch);
        self.age += 1.0;

        if let Some(cutoff) = self.cutoff {
            x = self.filter.process(x, cutoff, out_rate);
        }
        let x = x * amp;
        let (l, r) = model::pan_gains(lerp(self.pan));
        let send = if self.reverb {
            x * model::REVERB_SEND
        } else {
            0.0
        };
        Some((x * l, x * r, send))
    }
}

/// Plays one pattern in a loop.
pub struct Sequencer {
    out_rate: f64,
    sample_rate: f64,
    tempo: f64,
    pattern: Pattern,
    samples: Vec<Option<Arc<[i16]>>>,
    solo: [bool; PARTS],
    voices: [Option<Voice>; PARTS],
    reverb: Reverb,
    frames_to_next_step: f64,
    /// Step index (0-15) to look at next.
    next_step: usize,
    current_step: Option<usize>,
}

impl Sequencer {
    /// `out_rate`: output sample rate; `sample_rate`: rate of the samples
    /// (the device rate).
    pub fn new(out_rate: u32, sample_rate: u32) -> Self {
        Self {
            out_rate: f64::from(out_rate),
            sample_rate: f64::from(sample_rate),
            tempo: 120.0,
            pattern: Pattern::default(),
            samples: Vec::new(),
            solo: [false; PARTS],
            voices: std::array::from_fn(|_| None),
            reverb: Reverb::new(f64::from(out_rate)),
            frames_to_next_step: 0.0,
            next_step: 0,
            current_step: None,
        }
    }

    pub fn set_pattern(&mut self, pattern: Pattern) {
        self.pattern = pattern;
    }

    pub fn set_tempo(&mut self, bpm: f64) {
        self.tempo = bpm.max(1.0);
    }

    /// Sample of each slot (index = slot number).
    pub fn set_samples(&mut self, samples: Vec<Option<Arc<[i16]>>>) {
        self.samples = samples;
    }

    /// Soloed parts; when any is soloed, only those play.
    pub fn set_solo(&mut self, solo: [bool; PARTS]) {
        self.solo = solo;
    }

    /// Restarts from the first active step and silences all voices.
    pub fn reset(&mut self) {
        self.frames_to_next_step = 0.0;
        self.next_step = 0;
        self.current_step = None;
        self.voices = std::array::from_fn(|_| None);
        self.reverb = Reverb::new(self.out_rate);
    }

    /// The step (1-16) playing now.
    pub fn current_step(&self) -> Option<usize> {
        self.current_step
    }

    /// Output frames per step: steps are sixteenth notes.
    pub fn step_frames(&self) -> f64 {
        self.out_rate * 60.0 / self.tempo / model::STEPS_PER_BEAT
    }

    /// Renders stereo frames in -1..1.
    pub fn render(&mut self, out: &mut [[f32; 2]]) {
        for frame in out {
            if self.frames_to_next_step <= 0.0 {
                self.advance();
                self.frames_to_next_step += self.step_frames();
            }
            self.frames_to_next_step -= 1.0;

            let (mut l, mut r, mut send) = (0.0, 0.0, 0.0);
            for voice in &mut self.voices {
                match voice.as_mut().and_then(|v| v.next(self.out_rate)) {
                    Some((vl, vr, vs)) => {
                        l += vl;
                        r += vr;
                        send += vs;
                    }
                    None => *voice = None,
                }
            }
            let [wl, wr] = self.reverb.process(send);
            let limit = |x: f64| (x * model::MASTER_GAIN).tanh() as f32;
            *frame = [limit(l + wl), limit(r + wr)];
        }
    }

    /// Plays the next active step. ACTIVE STEP off means the step is skipped
    /// and the sequence gets shorter (confirmed on the device).
    fn advance(&mut self) {
        let active = self.pattern.active_steps();
        if active == 0 {
            self.current_step = None;
            return;
        }
        while active & (1 << self.next_step) == 0 {
            self.next_step = (self.next_step + 1) % STEPS;
        }
        let step = self.next_step + 1;
        self.next_step = (self.next_step + 1) % STEPS;
        self.current_step = Some(step);

        let any_solo = self.solo.iter().any(|&s| s);
        for i in 0..PARTS {
            let part = *self.pattern.part(i);
            let audible = if any_solo {
                self.solo[i]
            } else {
                !part.muted()
            };
            if audible && part.step(step) {
                self.voices[i] = self.voice(&part, step);
            }
        }
    }

    fn voice(&self, part: &Part, step: usize) -> Option<Voice> {
        let sample = self.samples.get(usize::from(part.sample()))?.clone()?;
        let motion = part.func(Func::Motion);
        // A motion value replaces the knob on its step.
        let value = |row: MotionRow, param: Param| -> f64 {
            let m = if motion { part.motion(row, step) } else { None };
            f64::from(m.unwrap_or_else(|| part.param(param)))
        };
        let pair = |start: MotionRow, end: MotionRow, param: Param| {
            let a = value(start, param);
            let b = if motion && part.motion(end, step).is_some() {
                value(end, param)
            } else {
                a
            };
            (a, b)
        };

        let start = (model::start_fraction(value(MotionRow::StartPoint, Param::StartPoint) as i8)
            * sample.len() as f64) as usize;
        let start = start.min(sample.len().saturating_sub(1));
        let rest = sample.len() - start;
        let len = ((model::length_fraction(value(MotionRow::Length, Param::Length) as i8)
            * rest as f64)
            .round() as usize)
            .clamp(1, rest);

        // SPEED motion is not used: its encoding is unconfirmed.
        let speed = match part.speed() {
            Speed::Semitones(n) => f64::from(n),
            Speed::Continuous(v) => f64::from(v) * model::CONTINUOUS_SPEED_SEMITONES_PER_UNIT,
        };
        let env = |attack: Param, attack_row: MotionRow, decay: Param, decay_row: MotionRow| {
            let a = value(attack_row, attack) as i8;
            let d = value(decay_row, decay) as i8;
            Envelope::new(
                model::attack_seconds(a),
                (!model::decay_holds(d)).then(|| model::decay_seconds(d)),
            )
        };

        Some(Voice {
            start,
            len,
            pos: 0.0,
            reverse: part.func(Func::Reverse),
            looped: part.func(Func::Loop),
            base_rate: self.sample_rate / self.out_rate,
            speed: (speed, speed),
            level: pair(MotionRow::LevelStart, MotionRow::LevelEnd, Param::Level),
            pan: pair(MotionRow::PanStart, MotionRow::PanEnd, Param::Pan),
            pitch_int: model::pitch_eg_semitones(value(MotionRow::PitchEgInt, Param::PitchEgInt)),
            amp_env: env(
                Param::AmpEgAttack,
                MotionRow::AmpEgAttack,
                Param::AmpEgDecay,
                MotionRow::AmpEgDecay,
            ),
            pitch_env: env(
                Param::PitchEgAttack,
                MotionRow::PitchEgAttack,
                Param::PitchEgDecay,
                MotionRow::PitchEgDecay,
            ),
            cutoff: model::hi_cut_hz(value(MotionRow::HiCut, Param::HiCut)),
            filter: LowPass::default(),
            reverb: part.func(Func::Reverb),
            age: 0.0,
            step_frames: self.step_frames(),
            sample,
        })
    }
}
