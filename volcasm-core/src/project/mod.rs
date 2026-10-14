//! A Volcasm project: sample slots and patterns for one device.

pub mod file;

use std::fmt;

use crate::device::DeviceProfile;
use crate::memory::MemoryUsage;
use crate::pattern::Pattern;
use crate::sample::Sample;
use crate::syro::SyroItem;

/// A filled sample slot.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Slot {
    pub name: String,
    /// Always 16-bit mono at the device rate (D11).
    pub sample: Sample,
}

/// What to send in a transfer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Target {
    /// The sample in slot `n` (0-based).
    Sample(usize),
    /// Erase slot `n` on the device.
    Erase(usize),
    /// Pattern `n` (0-based; sequence n+1).
    Pattern(usize),
}

/// Error when changing a project or preparing a transfer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProjectError {
    SlotOutOfRange(usize),
    PatternOutOfRange(usize),
    /// The sample is not at the device rate.
    SampleRate {
        expected: u32,
        actual: u32,
    },
    EmptySample,
    /// A transfer asks for the sample of an empty slot.
    EmptySlot(usize),
}

impl fmt::Display for ProjectError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SlotOutOfRange(n) => write!(f, "slot {n} out of range"),
            Self::PatternOutOfRange(n) => write!(f, "pattern {} out of range", n + 1),
            Self::SampleRate { expected, actual } => {
                write!(f, "sample rate {actual} Hz, expected {expected} Hz")
            }
            Self::EmptySample => write!(f, "sample has no frames"),
            Self::EmptySlot(n) => write!(f, "slot {n} is empty"),
        }
    }
}

impl std::error::Error for ProjectError {}

/// Tempo of a new project and of files without a tempo, in BPM.
pub const DEFAULT_TEMPO: u16 = 120;
/// Tempo range in BPM: the limits of the volca sample (56-240, also
/// Vosyr's limits, `docs/vsj-format.md` section 5).
pub const TEMPO_RANGE: std::ops::RangeInclusive<u16> = 56..=240;

/// Sample slots and patterns for one device.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Project {
    device: &'static DeviceProfile,
    pub name: String,
    pub author: Option<String>,
    pub license: Option<String>,
    pub description: Option<String>,
    /// Tempo for the preview, in BPM. Not sent to the device.
    pub tempo: u16,
    /// Swing as stored by Vosyr (0-75), kept from imports; not used yet.
    pub swing: Option<u8>,
    /// Reverb level as stored by Vosyr (0-127), kept from imports; not
    /// used yet.
    pub reverb: Option<u8>,
    slots: Vec<Option<Slot>>,
    patterns: Vec<Pattern>,
}

impl Project {
    /// An empty project: no samples, empty patterns.
    pub fn new(device: &'static DeviceProfile, name: impl Into<String>) -> Self {
        Self {
            device,
            name: name.into(),
            author: None,
            license: None,
            description: None,
            tempo: DEFAULT_TEMPO,
            swing: None,
            reverb: None,
            slots: vec![None; device.sample_slots as usize],
            patterns: vec![Pattern::default(); device.patterns as usize],
        }
    }

    pub fn device(&self) -> &'static DeviceProfile {
        self.device
    }

    pub fn slot(&self, n: usize) -> Option<&Slot> {
        self.slots.get(n)?.as_ref()
    }

    /// Filled slots with their numbers.
    pub fn slots(&self) -> impl Iterator<Item = (usize, &Slot)> {
        self.slots
            .iter()
            .enumerate()
            .filter_map(|(n, slot)| Some((n, slot.as_ref()?)))
    }

    /// Puts `slot` into slot `n`. The sample must be at the device rate.
    pub fn set_slot(&mut self, n: usize, slot: Slot) -> Result<(), ProjectError> {
        if n >= self.slots.len() {
            return Err(ProjectError::SlotOutOfRange(n));
        }
        if slot.sample.sample_rate != self.device.sample_rate {
            return Err(ProjectError::SampleRate {
                expected: self.device.sample_rate,
                actual: slot.sample.sample_rate,
            });
        }
        if slot.sample.pcm.is_empty() {
            return Err(ProjectError::EmptySample);
        }
        self.slots[n] = Some(slot);
        Ok(())
    }

    /// Empties slot `n` and returns what it held.
    pub fn clear_slot(&mut self, n: usize) -> Result<Option<Slot>, ProjectError> {
        let slot = self
            .slots
            .get_mut(n)
            .ok_or(ProjectError::SlotOutOfRange(n))?;
        Ok(slot.take())
    }

    pub fn patterns(&self) -> &[Pattern] {
        &self.patterns
    }

    pub fn pattern(&self, n: usize) -> Option<&Pattern> {
        self.patterns.get(n)
    }

    pub fn pattern_mut(&mut self, n: usize) -> Option<&mut Pattern> {
        self.patterns.get_mut(n)
    }

    pub fn set_pattern(&mut self, n: usize, pattern: Pattern) -> Result<(), ProjectError> {
        *self
            .patterns
            .get_mut(n)
            .ok_or(ProjectError::PatternOutOfRange(n))? = pattern;
        Ok(())
    }

    /// Total duration of all samples, in seconds.
    pub fn total_duration(&self) -> f64 {
        // fold from +0.0: an empty f64 sum would be -0.0 ("-0.00" on screen).
        self.slots()
            .map(|(_, slot)| slot.sample.duration())
            .fold(0.0, |a, b| a + b)
    }

    /// Device memory taken by all samples.
    pub fn memory_usage(&self) -> MemoryUsage {
        self.device
            .memory
            .usage(self.slots().map(|(_, slot)| slot.sample.frames()))
    }

    /// SYRO items for `targets`, in that order. Samples are sent compressed
    /// to `bits` (8-16) bits when given.
    pub fn syro_items(
        &self,
        targets: &[Target],
        bits: Option<u32>,
    ) -> Result<Vec<SyroItem<'_>>, ProjectError> {
        targets
            .iter()
            .map(|&target| {
                Ok(match target {
                    Target::Sample(n) => {
                        if n >= self.slots.len() {
                            return Err(ProjectError::SlotOutOfRange(n));
                        }
                        let slot = self.slot(n).ok_or(ProjectError::EmptySlot(n))?;
                        let (slot_number, pcm) = (n as u32, slot.sample.pcm.as_slice());
                        let sample_rate = slot.sample.sample_rate;
                        match bits {
                            None => SyroItem::Sample {
                                slot: slot_number,
                                pcm,
                                sample_rate,
                            },
                            Some(bits) => SyroItem::SampleCompressed {
                                slot: slot_number,
                                pcm,
                                sample_rate,
                                bits,
                            },
                        }
                    }
                    Target::Erase(n) => {
                        if n >= self.slots.len() {
                            return Err(ProjectError::SlotOutOfRange(n));
                        }
                        SyroItem::Erase { slot: n as u32 }
                    }
                    Target::Pattern(n) => SyroItem::Pattern {
                        index: n as u32,
                        data: self
                            .pattern_raw(n)
                            .ok_or(ProjectError::PatternOutOfRange(n))?,
                    },
                })
            })
            .collect()
    }

    /// Raw bytes of pattern `n`, as SYRO takes them.
    fn pattern_raw(&self, n: usize) -> Option<&[u8; crate::syro::PATTERN_SIZE]> {
        let pattern = self.patterns.get(n)?;
        // SAFETY: `PatternData` is exactly PATTERN_SIZE bytes without padding
        // (see `pattern`), and u8 has no alignment requirement.
        Some(unsafe { &*std::ptr::from_ref(pattern.data()).cast() })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::device::VOLCA_SAMPLE;

    fn sample(frames: usize) -> Sample {
        Sample {
            sample_rate: 31_250,
            pcm: vec![100; frames],
        }
    }

    #[test]
    fn new_project_is_empty() {
        let p = Project::new(&VOLCA_SAMPLE, "x");
        assert!(p.total_duration().is_sign_positive());
        assert_eq!(p.slots().count(), 0);
        assert_eq!(p.patterns().len(), 10);
        assert_eq!(p.memory_usage().used_blocks, 0);
    }

    #[test]
    fn slots_are_checked() {
        let mut p = Project::new(&VOLCA_SAMPLE, "x");
        let slot = |s| Slot {
            name: "s".into(),
            sample: s,
        };
        assert_eq!(
            p.set_slot(100, slot(sample(1))),
            Err(ProjectError::SlotOutOfRange(100))
        );
        let mut wrong_rate = sample(1);
        wrong_rate.sample_rate = 44_100;
        assert!(matches!(
            p.set_slot(0, slot(wrong_rate)),
            Err(ProjectError::SampleRate { .. })
        ));
        assert_eq!(
            p.set_slot(0, slot(sample(0))),
            Err(ProjectError::EmptySample)
        );
        p.set_slot(7, slot(sample(4097))).unwrap();
        assert_eq!(p.memory_usage().used_blocks, 3);
        assert_eq!(p.total_duration(), 4097.0 / 31_250.0);
        assert!(p.clear_slot(7).unwrap().is_some());
        assert_eq!(p.slots().count(), 0);
    }

    #[test]
    fn syro_items_follow_targets() {
        let mut p = Project::new(&VOLCA_SAMPLE, "x");
        p.set_slot(
            3,
            Slot {
                name: "a".into(),
                sample: sample(10),
            },
        )
        .unwrap();
        let items = p
            .syro_items(
                &[Target::Sample(3), Target::Erase(4), Target::Pattern(9)],
                Some(12),
            )
            .unwrap();
        assert!(matches!(
            items[0],
            SyroItem::SampleCompressed {
                slot: 3,
                bits: 12,
                ..
            }
        ));
        assert!(matches!(items[1], SyroItem::Erase { slot: 4 }));
        match items[2] {
            SyroItem::Pattern { index, data } => {
                assert_eq!(index, 9);
                assert_eq!(*data, Pattern::default().to_bytes());
            }
            _ => panic!(),
        }
        assert!(matches!(
            p.syro_items(&[Target::Sample(5)], None),
            Err(ProjectError::EmptySlot(5))
        ));
        assert!(matches!(
            p.syro_items(&[Target::Pattern(10)], None),
            Err(ProjectError::PatternOutOfRange(10))
        ));
    }
}
