//! Sequence preview in the app: a `Sequencer` on the audio thread, fed with
//! the edited state through a channel.

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::{self, Sender};

use volcasm_audio::{Player, TransferError};
use volcasm_core::pattern::{PARTS, Pattern};
use volcasm_core::preview::Sequencer;

use crate::state::AppState;

enum Command {
    Pattern(Box<Pattern>),
    Tempo(f64),
    Samples(Vec<Option<Arc<[i16]>>>),
    Solo([bool; PARTS]),
}

/// A playing sequence preview. Stops when dropped.
pub struct SequencePreview {
    _player: Player,
    tx: Sender<Command>,
    /// Step playing now (1-16), 0 = none.
    step: Arc<AtomicUsize>,
    sent_pattern: Pattern,
    sent_tempo: u16,
    sent_solo: [bool; PARTS],
    sent_samples: u64,
}

fn samples(state: &AppState) -> Vec<Option<Arc<[i16]>>> {
    (0..state.slot_count())
        .map(|n| {
            state
                .project
                .slot(n)
                .map(|s| Arc::<[i16]>::from(s.sample.pcm.as_slice()))
        })
        .collect()
}

impl SequencePreview {
    /// Starts playing the current sequence of `state` from its first step.
    pub fn start(state: &AppState) -> Result<Self, TransferError> {
        let (tx, rx) = mpsc::channel::<Command>();
        let step = Arc::new(AtomicUsize::new(0));
        let pattern = *state.current_pattern();
        let tempo = state.project.tempo;
        let sample_rate = state.project.device().sample_rate;
        let initial = samples(state);
        let solo = state.solo;
        let shared_step = step.clone();
        let player = Player::start(move |rate| {
            let mut sequencer = Sequencer::new(rate, sample_rate);
            sequencer.set_pattern(pattern);
            sequencer.set_tempo(f64::from(tempo));
            sequencer.set_samples(initial);
            sequencer.set_solo(solo);
            move |out: &mut [[f32; 2]]| {
                while let Ok(command) = rx.try_recv() {
                    match command {
                        Command::Pattern(p) => sequencer.set_pattern(*p),
                        Command::Tempo(t) => sequencer.set_tempo(t),
                        Command::Samples(s) => sequencer.set_samples(s),
                        Command::Solo(s) => sequencer.set_solo(s),
                    }
                }
                sequencer.render(out);
                shared_step.store(sequencer.current_step().unwrap_or(0), Ordering::Relaxed);
            }
        })?;
        Ok(Self {
            _player: player,
            tx,
            step,
            sent_pattern: pattern,
            sent_tempo: tempo,
            sent_solo: solo,
            sent_samples: state.samples_generation,
        })
    }

    /// Sends whatever changed in `state` since the last call, so edits are
    /// heard while playing.
    pub fn sync(&mut self, state: &AppState) {
        let pattern = *state.current_pattern();
        if pattern != self.sent_pattern {
            self.sent_pattern = pattern;
            let _ = self.tx.send(Command::Pattern(Box::new(pattern)));
        }
        if state.project.tempo != self.sent_tempo {
            self.sent_tempo = state.project.tempo;
            let _ = self.tx.send(Command::Tempo(f64::from(self.sent_tempo)));
        }
        if state.solo != self.sent_solo {
            self.sent_solo = state.solo;
            let _ = self.tx.send(Command::Solo(self.sent_solo));
        }
        if state.samples_generation != self.sent_samples {
            self.sent_samples = state.samples_generation;
            let _ = self.tx.send(Command::Samples(samples(state)));
        }
    }

    /// Step playing now (1-16).
    pub fn step(&self) -> Option<usize> {
        match self.step.load(Ordering::Relaxed) {
            0 => None,
            n => Some(n),
        }
    }
}
