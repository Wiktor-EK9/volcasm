//! Small DSP blocks for the preview: attack–decay envelope, low-pass
//! filter and reverb.

/// Attack–decay envelope, 0..1.
#[derive(Debug, Clone, Copy)]
pub struct Envelope {
    attack: f64,
    /// `None` = hold after the attack.
    decay: Option<f64>,
    time: f64,
}

impl Envelope {
    /// Times in seconds.
    pub fn new(attack: f64, decay: Option<f64>) -> Self {
        Self {
            attack: attack.max(0.0),
            decay,
            time: 0.0,
        }
    }

    /// Value at the current time, then advances by `dt` seconds.
    pub fn next(&mut self, dt: f64) -> f64 {
        let t = self.time;
        self.time += dt;
        if t < self.attack {
            return t / self.attack;
        }
        // A zero attack starts at full level.
        match self.decay {
            None => 1.0,
            Some(decay) => (1.0 - (t - self.attack) / decay.max(1e-6)).max(0.0),
        }
    }
}

/// State-variable low-pass filter (12 dB/octave).
#[derive(Debug, Clone, Copy, Default)]
pub struct LowPass {
    low: f64,
    band: f64,
}

impl LowPass {
    pub fn process(&mut self, input: f64, cutoff_hz: f64, sample_rate: f64) -> f64 {
        let f = (2.0 * (std::f64::consts::PI * cutoff_hz / sample_rate).sin()).min(1.0);
        let q = 0.7;
        self.low += f * self.band;
        let high = input - self.low - q * self.band;
        self.band += f * high;
        self.low
    }
}

/// Comb filter with damping (Freeverb style).
#[derive(Debug, Clone)]
struct Comb {
    buffer: Vec<f64>,
    index: usize,
    store: f64,
}

impl Comb {
    fn new(len: usize) -> Self {
        Self {
            buffer: vec![0.0; len.max(1)],
            index: 0,
            store: 0.0,
        }
    }

    fn process(&mut self, input: f64, feedback: f64, damp: f64) -> f64 {
        let out = self.buffer[self.index];
        self.store = out * (1.0 - damp) + self.store * damp;
        self.buffer[self.index] = input + self.store * feedback;
        self.index = (self.index + 1) % self.buffer.len();
        out
    }
}

#[derive(Debug, Clone)]
struct AllPass {
    buffer: Vec<f64>,
    index: usize,
}

impl AllPass {
    fn new(len: usize) -> Self {
        Self {
            buffer: vec![0.0; len.max(1)],
            index: 0,
        }
    }

    fn process(&mut self, input: f64) -> f64 {
        let delayed = self.buffer[self.index];
        let out = delayed - input;
        self.buffer[self.index] = input + delayed * 0.5;
        self.index = (self.index + 1) % self.buffer.len();
        out
    }
}

/// A small Schroeder/Freeverb-style stereo reverb (own implementation of
/// the classic structure: parallel combs, then all-passes).
#[derive(Debug, Clone)]
pub struct Reverb {
    combs: [Vec<Comb>; 2],
    allpasses: [Vec<AllPass>; 2],
}

/// Delay lengths at 44.1 kHz (the classic Freeverb tuning).
const COMB_TUNING: [usize; 4] = [1116, 1188, 1277, 1356];
const ALLPASS_TUNING: [usize; 2] = [556, 441];
const STEREO_SPREAD: usize = 23;

impl Reverb {
    pub fn new(sample_rate: f64) -> Self {
        let scale = |n: usize| (n as f64 * sample_rate / 44_100.0).round() as usize;
        let side = |spread: usize| {
            (
                COMB_TUNING
                    .iter()
                    .map(|&n| Comb::new(scale(n + spread)))
                    .collect(),
                ALLPASS_TUNING
                    .iter()
                    .map(|&n| AllPass::new(scale(n + spread)))
                    .collect(),
            )
        };
        let (left_combs, left_ap) = side(0);
        let (right_combs, right_ap) = side(STEREO_SPREAD);
        Self {
            combs: [left_combs, right_combs],
            allpasses: [left_ap, right_ap],
        }
    }

    /// Processes one mono input sample into a stereo wet signal.
    pub fn process(&mut self, input: f64) -> [f64; 2] {
        let mut out = [0.0; 2];
        for ((combs, allpasses), out) in
            self.combs.iter_mut().zip(&mut self.allpasses).zip(&mut out)
        {
            let mut sum: f64 = combs
                .iter_mut()
                .map(|c| c.process(input * 0.25, 0.84, 0.2))
                .sum();
            for ap in allpasses {
                sum = ap.process(sum);
            }
            *out = sum;
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn envelope_attack_then_hold_or_decay() {
        let mut hold = Envelope::new(0.01, None);
        assert_eq!(hold.next(0.005), 0.0);
        assert!((hold.next(0.005) - 0.5).abs() < 1e-9);
        assert_eq!(hold.next(1.0), 1.0);
        assert_eq!(hold.next(0.0), 1.0);
        let mut decay = Envelope::new(0.0, Some(0.1));
        decay.next(0.05);
        assert!((decay.next(0.0) - 0.5).abs() < 1e-6);
        decay.next(1.0);
        assert_eq!(decay.next(0.0), 0.0);
    }

    #[test]
    fn low_pass_attenuates_high_frequencies() {
        let rate = 48_000.0;
        let rms = |freq: f64| {
            let mut f = LowPass::default();
            let n = 48_000;
            let sum: f64 = (0..n)
                .map(|i| {
                    let x = (2.0 * std::f64::consts::PI * freq * i as f64 / rate).sin();
                    let y = f.process(x, 500.0, rate);
                    if i > n / 2 { y * y } else { 0.0 }
                })
                .sum();
            (sum / (n / 2) as f64).sqrt()
        };
        assert!(rms(100.0) > 0.6);
        assert!(rms(8_000.0) < 0.05);
    }

    #[test]
    fn reverb_has_a_tail() {
        let mut r = Reverb::new(48_000.0);
        r.process(1.0);
        let tail: f64 = (0..48_000).map(|_| r.process(0.0)[0].abs()).sum();
        assert!(tail > 0.1);
        assert!(r.process(0.0)[0].is_finite());
    }
}
