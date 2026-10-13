//! Sequence preview renderer. Rendering at 31 250 Hz (the sample rate)
//! with tempo 125 gives exactly 3750 frames per step.

use std::sync::Arc;

use volcasm_core::pattern::{Func, PARTS, Param, Pattern, Speed};
use volcasm_core::preview::Sequencer;

const RATE: u32 = 31_250;
const STEP: usize = 3750;

/// A sequencer with `samples` in slots 0, 1, ... and `pattern`.
fn sequencer(pattern: Pattern, samples: &[Vec<i16>]) -> Sequencer {
    let mut s = Sequencer::new(RATE, RATE);
    s.set_tempo(125.0);
    s.set_pattern(pattern);
    s.set_samples(
        samples
            .iter()
            .map(|pcm| Some(Arc::<[i16]>::from(pcm.as_slice())))
            .collect(),
    );
    s
}

fn render(s: &mut Sequencer, frames: usize) -> Vec<[f32; 2]> {
    let mut out = vec![[0.0; 2]; frames];
    s.render(&mut out);
    out
}

/// A single positive impulse followed by silence.
fn impulse() -> Vec<i16> {
    let mut pcm = vec![0; 100];
    pcm[0] = 32_767;
    pcm
}

/// Frames where the left or right channel is not silent.
fn onsets(out: &[[f32; 2]]) -> Vec<usize> {
    out.iter()
        .enumerate()
        .filter(|(_, f)| f[0].abs() > 1e-3 || f[1].abs() > 1e-3)
        .map(|(i, _)| i)
        .collect()
}

/// Part 0 plays sample 0 on `steps`.
fn pattern_with_steps(steps: u16) -> Pattern {
    let mut p = Pattern::default();
    p.part_mut(0).set_steps(steps);
    p
}

#[test]
fn steps_are_sixteenth_notes() {
    let mut s = sequencer(pattern_with_steps(0xffff), &[impulse()]);
    assert_eq!(s.step_frames(), STEP as f64);
    let out = render(&mut s, 4 * STEP);
    assert_eq!(onsets(&out), [0, STEP, 2 * STEP, 3 * STEP]);
}

#[test]
fn inactive_steps_are_skipped() {
    // Steps 1, 2 and 3 on, only 1 and 3 active: the sequence is two steps
    // long and alternates between them.
    let mut pattern = pattern_with_steps(0b111);
    pattern.set_active_steps(0b101);
    let mut s = sequencer(pattern, &[impulse()]);
    let mut seen = Vec::new();
    for _ in 0..4 {
        render(&mut s, STEP);
        seen.push(s.current_step().unwrap());
    }
    assert_eq!(seen, [1, 3, 1, 3]);
    let mut s = sequencer(pattern, &[impulse()]);
    assert_eq!(
        onsets(&render(&mut s, 4 * STEP)),
        [0, STEP, 2 * STEP, 3 * STEP]
    );
}

#[test]
fn no_active_step_is_silent() {
    let mut pattern = pattern_with_steps(0xffff);
    pattern.set_active_steps(0);
    let mut s = sequencer(pattern, &[impulse()]);
    assert!(onsets(&render(&mut s, 2 * STEP)).is_empty());
    assert_eq!(s.current_step(), None);
}

/// Part 0 plays a positive impulse, part 1 a negative one, both on step 1.
fn two_parts() -> (Pattern, Vec<Vec<i16>>) {
    let mut p = Pattern::default();
    p.part_mut(0).set_steps(1);
    p.part_mut(1).set_steps(1);
    p.part_mut(1).set_sample(1);
    let negative: Vec<i16> = impulse().iter().map(|&x| -x).collect();
    (p, vec![impulse(), negative])
}

#[test]
fn mute_and_solo() {
    let (pattern, samples) = two_parts();
    // Both parts: the impulses cancel.
    let mut s = sequencer(pattern, &samples);
    assert!(render(&mut s, 10)[0][0].abs() < 1e-6);

    let mut muted = pattern;
    muted.part_mut(1).set_muted(true);
    let mut s = sequencer(muted, &samples);
    assert!(render(&mut s, 10)[0][0] > 0.1);

    let mut s = sequencer(pattern, &samples);
    let mut solo = [false; PARTS];
    solo[1] = true;
    s.set_solo(solo);
    assert!(render(&mut s, 10)[0][0] < -0.1);
}

#[test]
fn reverse_plays_backwards() {
    let ramp: Vec<i16> = (1..=50).map(|i| i * 100).collect();
    let mut p = pattern_with_steps(1);
    p.part_mut(0).set_func(Func::Reverse, true);
    let mut s = sequencer(p, &[ramp]);
    let out = render(&mut s, 50);
    assert!(out[0][0] > out[10][0] && out[10][0] > out[40][0] && out[40][0] > 0.0);
}

#[test]
fn loop_repeats_the_sample() {
    let short = vec![10_000i16; 20];
    let mut s = sequencer(pattern_with_steps(1), std::slice::from_ref(&short));
    let out = render(&mut s, 100);
    assert!(out[10][0] > 0.0 && out[50][0] == 0.0);

    let mut p = pattern_with_steps(1);
    p.part_mut(0).set_func(Func::Loop, true);
    let mut s = sequencer(p, &[short]);
    let out = render(&mut s, 100);
    assert!(out[50][0] > 0.0 && out[90][0] > 0.0);
}

#[test]
fn pan_extremes() {
    let mut p = pattern_with_steps(1);
    p.part_mut(0).set_param(Param::Pan, -63).unwrap();
    let mut s = sequencer(p, &[impulse()]);
    let out = render(&mut s, 1);
    assert!(out[0][0] > 0.1 && out[0][1].abs() < 1e-6);
}

/// Zero crossings per second of the first `frames` of the left channel.
fn frequency(out: &[[f32; 2]], frames: usize) -> f64 {
    let crossings = out[..frames]
        .windows(2)
        .filter(|w| (w[0][0] < 0.0) != (w[1][0] < 0.0))
        .count();
    crossings as f64 / 2.0 / (frames as f64 / f64::from(RATE))
}

#[test]
fn speed_in_semitones_changes_pitch() {
    let sine: Vec<i16> = (0..31_250)
        .map(|i| (8000.0 * (2.0 * std::f64::consts::PI * 500.0 * i as f64 / 31_250.0).sin()) as i16)
        .collect();
    let mut p = pattern_with_steps(1);
    let mut s = sequencer(p, std::slice::from_ref(&sine));
    let base = frequency(&render(&mut s, 3000), 3000);
    p.part_mut(0).set_speed(Speed::Semitones(12)).unwrap();
    let mut s = sequencer(p, &[sine]);
    let octave = frequency(&render(&mut s, 3000), 3000);
    assert!((base - 500.0).abs() < 15.0, "{base}");
    assert!((octave - 1000.0).abs() < 25.0, "{octave}");
}

#[test]
fn ten_full_parts_do_not_clip() {
    let loud = vec![i16::MAX; 5000];
    let mut p = Pattern::default();
    for i in 0..PARTS {
        p.part_mut(i).set_steps(0xffff);
        p.part_mut(i).set_func(Func::Reverb, true);
    }
    let mut s = sequencer(p, &[loud]);
    let out = render(&mut s, 2 * STEP);
    // The soft limiter reaches full scale at most, never beyond.
    assert!(
        out.iter()
            .flatten()
            .all(|x| x.abs() <= 1.0 && x.is_finite())
    );
}

#[test]
fn rendering_is_deterministic() {
    let mut p = Pattern::from_bytes(
        &std::fs::read(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../tests/fixtures/vosyr/Test1/sequences/sequence01"),
        )
        .unwrap()
        .try_into()
        .unwrap(),
    )
    .unwrap();
    p.part_mut(0).set_func(Func::Reverb, true);
    let samples: Vec<Vec<i16>> = (0..25).map(|n| vec![(n * 1000) as i16; 3000]).collect();
    let a = render(&mut sequencer(p, &samples), 4 * STEP);
    let b = render(&mut sequencer(p, &samples), 4 * STEP);
    assert_eq!(a, b);
}

#[test]
fn missing_samples_are_silent() {
    let mut p = pattern_with_steps(0xffff);
    p.part_mut(0).set_sample(42);
    let mut s = sequencer(p, &[impulse()]);
    assert!(onsets(&render(&mut s, 2 * STEP)).is_empty());
}
