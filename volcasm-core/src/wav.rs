//! Serialization of SYRO streams to WAV.

use crate::syro::STREAM_SAMPLE_RATE;

const HEADER_SIZE: usize = 44;
const CHANNELS: u16 = 2;
const BITS_PER_SAMPLE: u16 = 16;
const BLOCK_ALIGN: u16 = CHANNELS * BITS_PER_SAMPLE / 8;

/// Writes a stereo 16-bit PCM WAV at [`STREAM_SAMPLE_RATE`] with the same
/// 44-byte header as the SDK example (`RIFF`, `fmt ` of 16 bytes, `data`).
pub fn stream_to_wav(frames: &[[i16; 2]]) -> Vec<u8> {
    stream_to_wav_at(frames, STREAM_SAMPLE_RATE)
}

/// Same as [`stream_to_wav`] for a stream at `sample_rate` (a stream from
/// [`crate::syro::resample_stream`]).
pub fn stream_to_wav_at(frames: &[[i16; 2]], sample_rate: u32) -> Vec<u8> {
    let data_size = (frames.len() * BLOCK_ALIGN as usize) as u32;
    let mut wav = Vec::with_capacity(HEADER_SIZE + data_size as usize);
    wav.extend_from_slice(b"RIFF");
    wav.extend_from_slice(&(data_size + 0x24).to_le_bytes());
    wav.extend_from_slice(b"WAVEfmt ");
    wav.extend_from_slice(&16u32.to_le_bytes());
    wav.extend_from_slice(&1u16.to_le_bytes()); // PCM
    wav.extend_from_slice(&CHANNELS.to_le_bytes());
    wav.extend_from_slice(&sample_rate.to_le_bytes());
    wav.extend_from_slice(&(sample_rate * BLOCK_ALIGN as u32).to_le_bytes());
    wav.extend_from_slice(&BLOCK_ALIGN.to_le_bytes());
    wav.extend_from_slice(&BITS_PER_SAMPLE.to_le_bytes());
    wav.extend_from_slice(b"data");
    wav.extend_from_slice(&data_size.to_le_bytes());
    for [left, right] in frames {
        wav.extend_from_slice(&left.to_le_bytes());
        wav.extend_from_slice(&right.to_le_bytes());
    }
    wav
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn header_matches_sdk_example() {
        let wav = stream_to_wav(&[[1, -1], [0x1234, 0]]);
        let expected_header: [u8; HEADER_SIZE] = [
            b'R', b'I', b'F', b'F', 0x2c, 0x00, 0x00, 0x00, b'W', b'A', b'V', b'E', b'f', b'm',
            b't', b' ', 0x10, 0x00, 0x00, 0x00, 0x01, 0x00, 0x02, 0x00, 0x44, 0xAC, 0x00, 0x00,
            0x10, 0xB1, 0x02, 0x00, 0x04, 0x00, 0x10, 0x00, b'd', b'a', b't', b'a', 0x08, 0x00,
            0x00, 0x00,
        ];
        assert_eq!(wav[..HEADER_SIZE], expected_header);
        assert_eq!(wav[HEADER_SIZE..], [1, 0, 0xff, 0xff, 0x34, 0x12, 0, 0]);
    }
}
