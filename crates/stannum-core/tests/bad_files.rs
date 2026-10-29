//! Bad files give errors, never panics. Each reader here is also fuzzed
//! (fuzz/README.md); inputs the fuzzers found problems with are kept here
//! so the normal test run keeps them fixed.

use stannum_core::{OpenCase, diff, geo, run_file, workspace::decode_text};
use std::path::Path;

/// Opens a file the way the app does and reads it into every view that
/// reads a case straight away, like the `raw` and `matpower` fuzz targets.
fn open(name: &str, bytes: &[u8]) -> OpenCase {
    let case = OpenCase::from_bytes(Path::new(name), bytes);
    let _ = case.summary();
    let _ = case.topology_wire();
    let _ = case.diagram_network();
    let _ = case.bus_coordinates();
    if let Some(job) = case.records_job() {
        let _ = job.run();
    }
    if name.ends_with(".raw") {
        let _ = diff::parse_bytes(name, bytes);
    }
    case
}

const FIVE_BUS: &str = include_str!("fixtures/five_bus.raw");
const CASE9: &str = include_str!("fixtures/case9.m");

/// Every prefix of a file at line ends, so each section is cut short once.
fn truncations(text: &str) -> impl Iterator<Item = &str> {
    text.match_indices('\n').map(move |(i, _)| &text[..i])
}

#[test]
fn raw_files_cut_short_never_panic() {
    for text in truncations(FIVE_BUS) {
        open("cut.raw", text.as_bytes());
    }
}

#[test]
fn matpower_files_cut_short_never_panic() {
    for text in truncations(CASE9) {
        open("cut.m", text.as_bytes());
    }
}

#[test]
fn files_that_are_not_cases_give_errors() {
    let junk: &[&[u8]] = &[
        b"",
        b"\n\n\n",
        b"\0\0\0\xff\xfe",
        b"PK\x03\x04 this is a zip file",
        b"0, 100.0, 33 / no buses\nQ\n",
        b"function mpc = x\nmpc.baseMVA = 100;\nmpc.bus = [1 3 0 0 0 0 1 1 0 230 1 1.1 0.9;\n",
    ];
    for bytes in junk {
        for name in ["junk.raw", "junk.m"] {
            let case = open(name, bytes);
            assert!(
                case.power.is_err() || bytes.starts_with(b"0, 100.0"),
                "{name} {bytes:?} read for power flow"
            );
        }
    }
}

#[test]
fn raw_values_out_of_range_never_panic() {
    // The five-bus case with one field of each bus record (a different field
    // on each line) replaced by a value out of any range a RAW field holds.
    let values = ["NaN", "inf", "-inf", "1e999", "-1", "4294967296", "''", "0"];
    for value in values {
        let mut text = String::new();
        let mut in_buses = false;
        for (i, line) in FIVE_BUS.lines().enumerate() {
            if line.trim_start().starts_with("0 /") {
                in_buses = line.contains("BEGIN BUS DATA");
                text.push_str(line);
                text.push('\n');
                continue;
            }
            if in_buses {
                let mut fields: Vec<&str> = line.split(',').collect();
                let at = i % fields.len();
                fields[at] = value;
                text.push_str(&fields.join(","));
            } else {
                text.push_str(line);
            }
            text.push('\n');
        }
        open("odd.raw", text.as_bytes());
    }
}

#[test]
fn bad_coordinate_files_give_errors_or_problems() {
    assert!(geo::parse("").is_err());
    assert!(geo::parse("bus,lat\n1,2").is_err());
    let file = geo::parse("bus,lat,lon\n1,NaN,0\n2,\"91\",0\n-3,0,0\n\"4,0,0\n5,1e999,0\n")
        .expect("the header is usable");
    assert!(file.places.is_empty());
    assert_eq!(file.problems.len(), 5);
    // Windows-1252 bytes, as the app decodes them from disk.
    let file = geo::parse(&decode_text(b"bus,lat,lon,substation\n1,0,0,Caf\xe9\n")).unwrap();
    assert_eq!(file.places[&1].substation.as_deref(), Some("Café"));
}

#[test]
fn run_file_numbers_read_back_exactly() {
    // Found by the run_file fuzz target: serde_json's default float parser
    // read 7.777777777777778e96, written for this number, one unit in the
    // last place lower, so a saved run file changed its own values.
    let steps = run_file::parse("open_case path=77777777777777777777e77\n").unwrap();
    let written = run_file::format(&steps, None, "").unwrap();
    let again = run_file::parse(&written).unwrap();
    assert_eq!(steps[0].args, again[0].args, "{written}");
}

/// Every field of every record in the RAW fixtures replaced, one at a time,
/// by each odd value. Too slow for every test run; run it with
/// `cargo test --release -p stannum-core --test bad_files -- --ignored`.
#[test]
#[ignore]
fn raw_field_sweep_never_panics() {
    let values = [
        "NaN",
        "inf",
        "-inf",
        "1e999",
        "-1",
        "-99999",
        "4294967296",
        "0",
        "''",
        "",
    ];
    let fixtures = [
        include_str!("fixtures/raw_export/five_bus_everything.raw"),
        include_str!("fixtures/five_bus_devices.raw"),
        include_str!("fixtures/five_bus_dc.raw"),
        include_str!("fixtures/five_bus_switches.raw"),
        include_str!("fixtures/five_bus_phase_shifter.raw"),
        include_str!("fixtures/five_bus_substations.raw"),
    ];
    for fixture in fixtures {
        let lines: Vec<&str> = fixture.lines().collect();
        for (at, line) in lines.iter().enumerate().skip(3) {
            if line.trim_start().starts_with("0 /") || line.trim_start().starts_with("@!") {
                continue;
            }
            let fields: Vec<&str> = line.split(',').collect();
            for f in 0..fields.len() {
                for value in values {
                    let mut changed = fields.clone();
                    changed[f] = value;
                    let changed = changed.join(",");
                    let mut text = lines.clone();
                    text[at] = &changed;
                    let text = text.join("\n");
                    let result = std::panic::catch_unwind(|| open("sweep.raw", text.as_bytes()));
                    assert!(
                        result.is_ok(),
                        "line {} field {f} = {value:?} panics:\n{changed}",
                        at + 1
                    );
                }
            }
        }
    }
}

/// The same sweep over every number in a MATPOWER case.
#[test]
#[ignore]
fn matpower_field_sweep_never_panics() {
    let values = [
        "NaN",
        "Inf",
        "-Inf",
        "1e999",
        "-1",
        "4294967296",
        "0",
        "'x'",
        ";",
    ];
    for fixture in [CASE9, include_str!("fixtures/vsc_grids/two_area.m")] {
        let lines: Vec<&str> = fixture.lines().collect();
        for (at, line) in lines.iter().enumerate() {
            let fields: Vec<&str> = line.split_whitespace().collect();
            for f in 0..fields.len() {
                if fields[f].trim_end_matches(';').parse::<f64>().is_err() {
                    continue;
                }
                for value in values {
                    let mut changed = fields.clone();
                    changed[f] = value;
                    let changed = changed.join("\t");
                    let mut text = lines.clone();
                    text[at] = &changed;
                    let text = text.join("\n");
                    let result = std::panic::catch_unwind(|| open("sweep.m", text.as_bytes()));
                    assert!(
                        result.is_ok(),
                        "line {} field {f} = {value:?} panics:\n{changed}",
                        at + 1
                    );
                }
            }
        }
    }
}
