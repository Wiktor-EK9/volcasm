//! Layout of the SDK pattern structures (must match VOLCASAMPLE_PATTERN_SIZE).

use std::mem::{offset_of, size_of};
use volcasm_syro_sys::*;

#[test]
fn pattern_struct_is_2624_bytes() {
    assert_eq!(size_of::<VolcaSample_Pattern_Data>(), 2624);
    assert_eq!(
        size_of::<VolcaSample_Pattern_Data>(),
        VOLCASAMPLE_PATTERN_SIZE as usize
    );
}

#[test]
fn part_struct_is_256_bytes() {
    assert_eq!(size_of::<VolcaSample_Part_Data>(), 256);
}

#[test]
fn pattern_field_offsets() {
    assert_eq!(offset_of!(VolcaSample_Pattern_Data, ActiveStep), 8);
    assert_eq!(offset_of!(VolcaSample_Pattern_Data, Part), 0x20);
    assert_eq!(offset_of!(VolcaSample_Pattern_Data, Footer), 2620);
    assert_eq!(offset_of!(VolcaSample_Part_Data, Param), 9);
    assert_eq!(offset_of!(VolcaSample_Part_Data, FuncMemoryPart), 20);
    assert_eq!(offset_of!(VolcaSample_Part_Data, Motion), 32);
}
