//! Conversion between raw pattern bytes (as stored by Vosyr and sent by SYRO)
//! and the SDK structure `VolcaSample_Pattern_Data`.

use std::fmt;

pub use sys::{VolcaSample_Part_Data as PartData, VolcaSample_Pattern_Data as PatternData};
use volcasm_syro_sys as sys;

use crate::syro::PATTERN_SIZE;

// The raw format is little endian and the SDK structure uses native fields.
#[cfg(target_endian = "big")]
compile_error!("pattern conversion assumes a little-endian target");

// Every byte of the structure belongs to a field (no implicit padding): see
// the field sizes in `volcasm-syro-sys/tests/layout.rs`.
const _: () = assert!(size_of::<PatternData>() == PATTERN_SIZE);

/// Invalid raw pattern data.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PatternError {
    Header(u32),
    DevCode(u16),
    Footer(u32),
}

impl fmt::Display for PatternError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Header(v) => write!(f, "invalid pattern header {v:#010x}"),
            Self::DevCode(v) => write!(f, "invalid pattern device code {v:#06x}"),
            Self::Footer(v) => write!(f, "invalid pattern footer {v:#010x}"),
        }
    }
}

impl std::error::Error for PatternError {}

/// Reads raw pattern bytes into the SDK structure and checks the header,
/// device code and footer.
pub fn from_bytes(bytes: &[u8; PATTERN_SIZE]) -> Result<PatternData, PatternError> {
    // SAFETY: `PatternData` is `repr(C)`, consists only of integer fields and
    // arrays of them (any bit pattern is valid) and is exactly PATTERN_SIZE
    // bytes long; `read_unaligned` imposes no alignment requirement.
    let data: PatternData = unsafe { bytes.as_ptr().cast::<PatternData>().read_unaligned() };
    if data.Header != sys::VOLCASAMPLE_PATTERN_HEADER {
        return Err(PatternError::Header(data.Header));
    }
    if u32::from(data.DevCode) != sys::VOLCASAMPLE_PATTERN_DEVCODE {
        return Err(PatternError::DevCode(data.DevCode));
    }
    if data.Footer != sys::VOLCASAMPLE_PATTERN_FOOTER {
        return Err(PatternError::Footer(data.Footer));
    }
    Ok(data)
}

/// Writes the SDK structure as raw pattern bytes.
pub fn to_bytes(data: &PatternData) -> [u8; PATTERN_SIZE] {
    // SAFETY: same size, and the structure has no padding bytes, so every
    // byte read is initialized.
    unsafe {
        std::ptr::from_ref(data)
            .cast::<[u8; PATTERN_SIZE]>()
            .read_unaligned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(rel: &str) -> [u8; PATTERN_SIZE] {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../tests/fixtures/vosyr")
            .join(rel);
        std::fs::read(path).unwrap().try_into().unwrap()
    }

    #[test]
    fn round_trip() {
        for rel in ["Test1/sequences/sequence01", "Test2/sequences/sequence09"] {
            let bytes = fixture(rel);
            assert_eq!(to_bytes(&from_bytes(&bytes).unwrap()), bytes, "{rel}");
        }
    }

    #[test]
    fn reads_fields() {
        let data = from_bytes(&fixture("Test1/sequences/sequence01")).unwrap();
        assert_eq!(data.ActiveStep, 0xefff);
        assert_eq!(data.Part[0].SampleNum, 0);
    }

    #[test]
    fn rejects_invalid_markers() {
        let good = fixture("Test1/sequences/sequence01");
        let mut bad = good;
        bad[0] = b'X';
        assert!(matches!(from_bytes(&bad), Err(PatternError::Header(_))));
        let mut bad = good;
        bad[4] = 0;
        assert!(matches!(from_bytes(&bad), Err(PatternError::DevCode(_))));
        let mut bad = good;
        bad[PATTERN_SIZE - 1] = 0;
        assert!(matches!(from_bytes(&bad), Err(PatternError::Footer(_))));
    }
}
