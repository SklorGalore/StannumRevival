//! A bus coordinates CSV, decoded the way Stannum reads one from disk.
#![no_main]

use libfuzzer_sys::fuzz_target;
use stannum_core::{geo, workspace::decode_text};

fuzz_target!(|bytes: &[u8]| {
    let _ = geo::parse(&decode_text(bytes));
});
