//! A PSS/E RAW file, read the way Stannum opens one: for power flow
//! (rs-power), for topology (nodal), for Records and for Case diff, then
//! shown in the views that read it straight away.
#![no_main]

use libfuzzer_sys::fuzz_target;
use stannum_core::{OpenCase, diff};
use std::path::Path;

fuzz_target!(|bytes: &[u8]| {
    let case = OpenCase::from_bytes(Path::new("fuzz.raw"), bytes);
    let _ = case.summary();
    let _ = case.topology_wire();
    let _ = case.diagram_network();
    let _ = case.bus_coordinates();
    if let Some(job) = case.records_job() {
        let _ = job.run();
    }
    let _ = diff::parse_bytes("fuzz.raw", bytes);
});
