//! Raw FFI bindings to the KORG SYRO library for volca sample.
//!
//! The SDK is vendored in `vendor/volcasample` (BSD-3-Clause, see `COPYING`).

#![allow(non_camel_case_types, non_snake_case, non_upper_case_globals)]

include!(concat!(env!("OUT_DIR"), "/bindings.rs"));

/// Path to the unmodified SDK example binary (`syro_volcasample_example`),
/// used as the reference for bit-exact stream comparisons.
#[cfg(feature = "reference-example")]
pub const REFERENCE_EXAMPLE_BIN: &str = env!("VOLCASM_SYRO_EXAMPLE_BIN");
