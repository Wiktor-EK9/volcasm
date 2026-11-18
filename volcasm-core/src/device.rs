//! Device profiles: hardware limits kept in one place, so the data model and
//! file format do not hard-code them.

use std::fmt;

use crate::memory::MemoryLayout;
use volcasm_syro_sys as sys;

/// An image of the whole sample memory, sent in one transfer.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct MemoryImage(pub &'static [u8]);

impl fmt::Debug for MemoryImage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "MemoryImage({} B)", self.0.len())
    }
}

/// Hardware limits of one supported device.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DeviceProfile {
    /// Stable identifier, e.g. for project manifests.
    pub id: &'static str,
    /// Human-readable device name.
    pub name: &'static str,
    /// Number of sample slots (numbered from 0).
    pub sample_slots: u32,
    /// Number of patterns (sequences).
    pub patterns: u32,
    /// Native sample rate of stored samples (16-bit mono).
    pub sample_rate: u32,
    /// Sample memory layout.
    pub memory: MemoryLayout,
    /// The factory samples (factory reset).
    pub factory_samples: MemoryImage,
    /// No samples at all (erase all slots).
    pub no_samples: MemoryImage,
}

impl DeviceProfile {
    /// Longest total sample time that fits the memory, in seconds (16-bit
    /// mono at the device rate, ignoring block rounding).
    pub fn max_sample_seconds(&self) -> f64 {
        self.memory.capacity_bytes() as f64
            / crate::memory::BYTES_PER_FRAME as f64
            / f64::from(self.sample_rate)
    }
}

/// KORG volca sample (first generation).
///
/// Memory capacity was measured on hardware: 1019
/// blocks of 4096 B fit, the 1020th is uncertain.
pub const VOLCA_SAMPLE: DeviceProfile = DeviceProfile {
    id: "volca-sample",
    name: "volca sample",
    sample_slots: 100,
    patterns: 10,
    sample_rate: 31_250,
    memory: MemoryLayout {
        block_size: 4096,
        blocks: 1019,
    },
    // From KORG's SDK (`alldata`); the factory sequences are empty.
    factory_samples: MemoryImage(sys::ALLDATA_FACTORY),
    no_samples: MemoryImage(sys::ALLDATA_EMPTY),
};

/// All supported device profiles.
pub const PROFILES: [&DeviceProfile; 1] = [&VOLCA_SAMPLE];

/// The profile with identifier `id`.
pub fn profile(id: &str) -> Option<&'static DeviceProfile> {
    PROFILES.into_iter().find(|p| p.id == id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn volca_sample_holds_about_67_seconds() {
        // 1019 blocks × 4096 B / 2 B / 31 250 Hz.
        let seconds = VOLCA_SAMPLE.max_sample_seconds();
        assert!((seconds - 66.781_184).abs() < 1e-6, "{seconds}");
    }
}
