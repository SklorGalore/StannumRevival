//! A Stannum case (.stn or .stnb) or lists (.stnl) file: read against its
//! schema, written in canonical form, and that form must read back to
//! itself; a case read from either form must also survive the other. A
//! case is then opened the way Stannum opens one (converted to RAW 35).
#![no_main]

use libfuzzer_sys::fuzz_target;
use stannum_core::OpenCase;
use std::path::Path;

fuzz_target!(|bytes: &[u8]| {
    if let Ok(doc) = stannum_format::read_case_binary(bytes)
        && let Ok(text) = stannum_format::write(&doc)
    {
        let again = stannum_format::read_case(&text).expect("text written from .stnb reads");
        assert_eq!(stannum_format::write(&again).unwrap(), text, "canonical text is stable");
    }
    if let Ok(text) = std::str::from_utf8(bytes) {
        let schema = stannum_format::schema_for(text).unwrap_or(stannum_format::case_schema());
        if let Ok(doc) = stannum_format::read(text, schema)
            && let Ok(out) = stannum_format::write(&doc)
        {
            let again = stannum_format::read(&out, schema).expect("canonical text reads back");
            assert_eq!(stannum_format::write(&again).unwrap(), out, "canonical text is stable");
            if schema.format == "stannum-case" {
                let binary = stannum_format::write_binary(&again, &Default::default())
                    .expect("a case that reads writes as .stnb");
                let back = stannum_format::read_case_binary(&binary).expect(".stnb reads back");
                assert_eq!(stannum_format::write(&back).unwrap(), out, ".stnb keeps the case");
            }
        }
    }
    let case = OpenCase::from_bytes(Path::new("fuzz.stn"), bytes);
    let _ = case.summary();
    let _ = case.diagram_network();
    if let Some(job) = case.records_job() {
        let _ = job.run();
    }
});
