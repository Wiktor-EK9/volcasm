//! Global settings in the `.vsj` file.
//!
//! Layout, recognized from 25 projects:
//! ```text
//! 0     4 B    "PRST"
//! 4     8 B    constants (u32 4854, u32 251)
//! 12    100 B  one byte per (sequence, part), UI state
//! 112   100 NUL-terminated strings, one per (sequence, part), UI state;
//!              these make the rest of the file move
//! t     1 B    sequence shown in Vosyr (0-based)
//! t+1   10 B   0x7f in all projects
//! t+11  1 B    tempo (BPM)
//! t+612 1 B    swing
//! t+613 1 B    reverb
//! t+714 100 B  0x40 in all projects
//! ```
//! Bytes not listed are zero in all projects.

use crate::Error;

const ENTRIES: usize = 100;
const FLAGS_OFFSET: usize = 12;
const STRINGS_OFFSET: usize = FLAGS_OFFSET + ENTRIES;
/// Longest string seen is 3 characters; anything much longer means the
/// layout is not what we know.
const MAX_STRING: usize = 64;

const TEMPO: usize = 11;
const SWING: usize = 612;
const REVERB: usize = 613;
/// End of the last known field (the 100 × 0x40 block).
const TAIL_LEN: usize = 814;

/// Global settings of a Vosyr project.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GlobalSettings {
    /// Tempo in BPM (Vosyr allows 56-240).
    pub tempo: u8,
    /// Swing, 0-75 in Vosyr.
    pub swing: u8,
    /// Reverb level, 0-127 in Vosyr.
    pub reverb: u8,
    /// Sequence shown when the project was saved (0-based).
    pub shown_sequence: u8,
}

/// Reads the global settings from a `.vsj` file. Fails, rather than guess,
/// when the file does not follow the known layout.
pub fn parse(vsj: &[u8]) -> Result<GlobalSettings, Error> {
    let layout = |message: String| Error::Layout(message);
    let mut pos = STRINGS_OFFSET;
    for entry in 0..ENTRIES {
        let rest = vsj
            .get(pos..)
            .ok_or_else(|| layout(format!("string {entry} beyond the end of the file")))?;
        let len = rest
            .iter()
            .take(MAX_STRING + 1)
            .position(|&b| b == 0)
            .ok_or_else(|| layout(format!("string {entry} at offset {pos} is not terminated")))?;
        if !rest[..len].iter().all(|b| b.is_ascii_graphic()) {
            return Err(layout(format!(
                "string {entry} at offset {pos} is not text"
            )));
        }
        pos += len + 1;
    }
    let tail = vsj
        .get(pos..pos + TAIL_LEN)
        .ok_or_else(|| layout(format!("settings block at offset {pos} is truncated")))?;
    Ok(GlobalSettings {
        tempo: tail[TEMPO],
        swing: tail[SWING],
        reverb: tail[REVERB],
        shown_sequence: tail[0],
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// An empty project's settings block with `strings` in the variable part.
    fn vsj(strings: &[&str], tempo: u8) -> Vec<u8> {
        let mut d = b"PRST".to_vec();
        d.resize(STRINGS_OFFSET, 0);
        for i in 0..ENTRIES {
            d.extend_from_slice(strings.get(i).unwrap_or(&"").as_bytes());
            d.push(0);
        }
        let t = d.len();
        d.resize(t + TAIL_LEN, 0);
        d[t + TEMPO] = tempo;
        d.resize(65_535, 0);
        d
    }

    #[test]
    fn strings_shift_the_settings() {
        let s = parse(&vsj(&["", "0|1", "13"], 97)).unwrap();
        assert_eq!(s.tempo, 97);
        assert_eq!(parse(&vsj(&[], 125)).unwrap().tempo, 125);
    }

    #[test]
    fn unknown_layout_is_an_error() {
        let mut d = vsj(&[], 125);
        d[STRINGS_OFFSET] = 0xff;
        assert!(matches!(parse(&d), Err(Error::Layout(_))));
        let long = "1".repeat(MAX_STRING + 1);
        assert!(matches!(parse(&vsj(&[&long], 125)), Err(Error::Layout(_))));
        assert!(matches!(parse(&d[..300]), Err(Error::Layout(_))));
    }
}
