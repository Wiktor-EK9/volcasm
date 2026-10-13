//! WAV files in the device format (16-bit mono PCM), as stored in projects.

use std::io::{Read, Seek, Write};

use super::Sample;

/// Reads a 16-bit mono PCM WAV. Other formats are rejected; use
/// [`super::import`] to convert them.
pub fn read(reader: impl Read) -> Result<Sample, hound::Error> {
    let mut reader = hound::WavReader::new(reader)?;
    let spec = reader.spec();
    if spec.channels != 1
        || spec.bits_per_sample != 16
        || spec.sample_format != hound::SampleFormat::Int
    {
        return Err(hound::Error::Unsupported);
    }
    let pcm = reader.samples::<i16>().collect::<Result<_, _>>()?;
    Ok(Sample {
        sample_rate: spec.sample_rate,
        pcm,
    })
}

/// Writes `sample` as a 16-bit mono PCM WAV.
pub fn write(sample: &Sample, writer: impl Write + Seek) -> Result<(), hound::Error> {
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: sample.sample_rate,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut writer = hound::WavWriter::new(writer, spec)?;
    let mut samples = writer.get_i16_writer(sample.pcm.len() as u32);
    for &s in &sample.pcm {
        samples.write_sample(s);
    }
    samples.flush()?;
    writer.finalize()
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use super::*;

    #[test]
    fn round_trip() {
        let sample = Sample {
            sample_rate: 31_250,
            pcm: vec![0, i16::MIN, i16::MAX, -1, 1],
        };
        let mut buf = Cursor::new(Vec::new());
        write(&sample, &mut buf).unwrap();
        buf.set_position(0);
        assert_eq!(read(buf).unwrap(), sample);
    }
}
