//! Smoke test: the vendored SYRO library links and produces a stream.
//! Bit-exact comparison with the SDK example is in volcasm-core's
//! `syro_reference` tests.

use std::ptr;
use volcasm_syro_sys::*;

#[test]
fn encodes_short_sample() {
    // 0.1 s of a 440 Hz sine at 31250 Hz, 16-bit little endian.
    let mut pcm: Vec<u8> = (0..3125)
        .map(|n| {
            let t = n as f64 / 31250.0;
            ((t * 440.0 * std::f64::consts::TAU).sin() * 16000.0) as i16
        })
        .flat_map(i16::to_le_bytes)
        .collect();

    let mut data = SyroData {
        DataType: DataType_Sample_Liner,
        pData: pcm.as_mut_ptr(),
        Number: 0,
        Size: pcm.len() as u32,
        Quality: 16,
        Fs: 31250,
        SampleEndian: LittleEndian,
    };

    let mut handle: SyroHandle = ptr::null_mut();
    let mut frames: u32 = 0;
    let status = unsafe { SyroVolcaSample_Start(&mut handle, &mut data, 1, 0, &mut frames) };
    assert_eq!(status, Status_Success);
    assert!(frames > 0);

    // The frame count from Start overshoots the stream: the last frames return
    // Status_NoData (the SDK example ignores the status and repeats the
    // previous values). NoData must only appear as a contiguous tail.
    let mut nonzero = 0usize;
    let mut first_no_data = None;
    for i in 0..frames {
        let (mut left, mut right) = (0i16, 0i16);
        let status = unsafe { SyroVolcaSample_GetSample(handle, &mut left, &mut right) };
        match first_no_data {
            None if status == Status_NoData => first_no_data = Some(i),
            None => assert_eq!(status, Status_Success, "frame {i}"),
            Some(_) => assert_eq!(status, Status_NoData, "frame {i}"),
        }
        if left != 0 || right != 0 {
            nonzero += 1;
        }
    }
    assert!(nonzero > 0);
    assert!(first_no_data.is_some_and(|i| i > frames / 2));

    assert_eq!(unsafe { SyroVolcaSample_End(handle) }, Status_Success);
}

#[test]
fn sdk_constants() {
    assert_eq!(VOLCASAMPLE_NUM_OF_SAMPLE, 100);
    assert_eq!(VOLCASAMPLE_NUM_OF_PATTERN, 10);
    assert_eq!(VOLCASAMPLE_PATTERN_SIZE, 2624);
}
