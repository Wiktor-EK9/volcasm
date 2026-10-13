//! `samples/samples`: the sample slot index (`docs/vsj-format.md`, section 3).

use crate::{Error, check_marker, check_size};

/// Size of the index file (fixed buffer, rest zeros).
pub const INDEX_SIZE: usize = 65_535;
/// Number of slot records.
pub const SLOT_COUNT: usize = 100;
/// Size of one slot record.
pub const RECORD_SIZE: usize = 215;

const HEADER: &[u8; 4] = b"SMST";
const FOOTER: &[u8; 4] = b"SMED";
const RECORDS_OFFSET: usize = 4;
const FOOTER_OFFSET: usize = RECORDS_OFFSET + SLOT_COUNT * RECORD_SIZE;

const NAME: std::ops::Range<usize> = 0..100;
const USED: usize = 100;
const FILE_NAME: std::ops::Range<usize> = 111..211;

/// One slot record. Fields not understood yet stay in `raw`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SlotRecord {
    pub slot: usize,
    /// Display name (stored in mixed case, Vosyr shows it in upper case).
    pub name: String,
    /// Whether the slot holds a sample.
    pub used: bool,
    /// Name of the WAV file in `samples/`.
    pub file_name: String,
    /// The whole record, including unknown fields (+101..+111, +211..+215).
    pub raw: [u8; RECORD_SIZE],
}

/// The parsed index; `raw` is the file as read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SampleIndex {
    pub slots: Vec<SlotRecord>,
    pub raw: Vec<u8>,
}

impl SampleIndex {
    pub fn parse(raw: Vec<u8>) -> Result<Self, Error> {
        const FILE: &str = "samples/samples";
        check_size(FILE, &raw, INDEX_SIZE)?;
        check_marker(FILE, &raw, 0, HEADER)?;
        check_marker(FILE, &raw, FOOTER_OFFSET, FOOTER)?;
        let slots = raw[RECORDS_OFFSET..FOOTER_OFFSET]
            .as_chunks::<RECORD_SIZE>()
            .0
            .iter()
            .enumerate()
            .map(|(slot, &raw)| SlotRecord {
                slot,
                name: nul_terminated(&raw[NAME]),
                used: raw[USED] == 1,
                file_name: nul_terminated(&raw[FILE_NAME]),
                raw,
            })
            .collect();
        Ok(Self { slots, raw })
    }

    /// Slots that hold a sample.
    pub fn used(&self) -> impl Iterator<Item = &SlotRecord> {
        self.slots.iter().filter(|slot| slot.used)
    }
}

/// Text up to the first NUL. Only ASCII has been observed; other bytes are
/// decoded as UTF-8, lossy (the raw bytes stay in the record).
fn nul_terminated(bytes: &[u8]) -> String {
    let end = bytes.iter().position(|&b| b == 0).unwrap_or(bytes.len());
    String::from_utf8_lossy(&bytes[..end]).into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn empty_index() -> Vec<u8> {
        let mut raw = vec![0; INDEX_SIZE];
        raw[..4].copy_from_slice(HEADER);
        raw[FOOTER_OFFSET..FOOTER_OFFSET + 4].copy_from_slice(FOOTER);
        raw
    }

    #[test]
    fn parses_a_record() {
        let mut raw = empty_index();
        let rec = RECORDS_OFFSET + 3 * RECORD_SIZE;
        raw[rec..rec + 4].copy_from_slice(b"Kick");
        raw[rec + USED] = 1;
        raw[rec + FILE_NAME.start..rec + FILE_NAME.start + 7].copy_from_slice(b"sample1");
        let index = SampleIndex::parse(raw.clone()).unwrap();
        assert_eq!(index.slots.len(), SLOT_COUNT);
        let slot = &index.slots[3];
        assert_eq!((slot.name.as_str(), slot.used), ("Kick", true));
        assert_eq!(slot.file_name, "sample1");
        assert_eq!(index.used().count(), 1);
        assert_eq!(index.raw, raw);
    }

    #[test]
    fn rejects_bad_markers_and_size() {
        let mut raw = empty_index();
        raw[0] = b'X';
        assert!(matches!(SampleIndex::parse(raw), Err(Error::Marker { .. })));
        let mut raw = empty_index();
        raw[FOOTER_OFFSET] = b'X';
        assert!(matches!(SampleIndex::parse(raw), Err(Error::Marker { .. })));
        assert!(matches!(
            SampleIndex::parse(vec![0; 100]),
            Err(Error::Size { .. })
        ));
    }
}
