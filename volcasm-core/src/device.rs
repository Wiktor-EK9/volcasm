//! Device profiles: hardware limits kept in one place, so the data model and
//! file format do not hard-code them (D13).

use crate::memory::MemoryLayout;

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
}

/// KORG volca sample (first generation).
///
/// Memory capacity was measured on hardware (`docs/hardware.md`): 1019
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
};
