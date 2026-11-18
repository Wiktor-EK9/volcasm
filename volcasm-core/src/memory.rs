//! Sample memory budget.
//!
//! Samples are stored in fixed-size blocks: each non-empty sample takes
//! `ceil(bytes / block_size)` blocks, with no other per-sample cost. This
//! model was measured on hardware.

/// Block-based sample memory of a device.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MemoryLayout {
    /// Block size in bytes.
    pub block_size: u32,
    /// Number of blocks available for samples.
    pub blocks: u32,
}

/// Bytes per stored sample frame (16-bit mono).
pub const BYTES_PER_FRAME: u64 = 2;

impl MemoryLayout {
    /// Total sample capacity in bytes.
    pub fn capacity_bytes(&self) -> u64 {
        u64::from(self.block_size) * u64::from(self.blocks)
    }

    /// Blocks taken by a sample of `frames` frames (0 for an empty slot).
    pub fn blocks_for(&self, frames: usize) -> u64 {
        (frames as u64 * BYTES_PER_FRAME).div_ceil(u64::from(self.block_size))
    }

    /// Memory usage of samples with the given frame counts.
    pub fn usage(&self, frames: impl IntoIterator<Item = usize>) -> MemoryUsage {
        MemoryUsage {
            used_blocks: frames.into_iter().map(|n| self.blocks_for(n)).sum(),
            total_blocks: u64::from(self.blocks),
        }
    }
}

/// Blocks used by a set of samples versus device capacity.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MemoryUsage {
    pub used_blocks: u64,
    pub total_blocks: u64,
}

impl MemoryUsage {
    /// Whether all samples fit on the device.
    pub fn fits(&self) -> bool {
        self.used_blocks <= self.total_blocks
    }

    /// Blocks left; 0 when over capacity.
    pub fn free_blocks(&self) -> u64 {
        self.total_blocks.saturating_sub(self.used_blocks)
    }

    /// Blocks over capacity; 0 when everything fits.
    pub fn excess_blocks(&self) -> u64 {
        self.used_blocks.saturating_sub(self.total_blocks)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::device::VOLCA_SAMPLE;

    const LAYOUT: MemoryLayout = VOLCA_SAMPLE.memory;

    #[test]
    fn block_rounding() {
        assert_eq!(LAYOUT.blocks_for(0), 0);
        assert_eq!(LAYOUT.blocks_for(1), 1);
        assert_eq!(LAYOUT.blocks_for(2048), 1);
        assert_eq!(LAYOUT.blocks_for(2049), 2);
    }

    /// Hardware memory test (2026-10-13). Read
    /// conservatively: the last slot shown on the display is taken as the
    /// first one that did not fit.
    #[test]
    fn hardware_fill_experiments() {
        // 102 350 B and 102 352 B fillers both take 25 blocks.
        assert_eq!(LAYOUT.blocks_for(51_175), 25);
        assert_eq!(LAYOUT.blocks_for(51_176), 25);
        // 2000 B probes take one block each.
        assert_eq!(LAYOUT.blocks_for(1000), 1);

        // Experiment 1: 40 fillers, then probes in slots 40..=58 were stored.
        let exp1 = [51_175].repeat(40).into_iter().chain([1000].repeat(19));
        assert_eq!(LAYOUT.usage(exp1).used_blocks, 1019);
        // Experiment 2: 39 fillers, then probes in slots 39..=82 were stored.
        let exp2 = [51_176].repeat(39).into_iter().chain([1000].repeat(44));
        let usage = LAYOUT.usage(exp2);
        assert_eq!(usage.used_blocks, 1019);
        assert!(usage.fits());
        assert_eq!(usage.free_blocks(), 0);
        // One more probe did not fit.
        let over = LAYOUT.usage([51_176].repeat(39).into_iter().chain([1000].repeat(45)));
        assert!(!over.fits());
        assert_eq!(over.excess_blocks(), 1);
    }
}
