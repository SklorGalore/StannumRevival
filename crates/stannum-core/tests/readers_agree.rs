//! The three RAW readers see the same case.
//!
//! A RAW file is read three times: by rs-power for power flow (revisions
//! 23-35), by nodal for topology (35 only) and by case-diff's parser for
//! Records and Case diff (33 and 35). Each reader that can read a fixture
//! must count the same buses, generators, branches, switches and loads, apart
//! from the differences listed in [`KNOWN`], each with its reason. A new
//! difference fails here before it shows up as two modules disagreeing.

use stannum_core::diff::parse_bytes;
use stannum_core::power;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// What each reader counts; None where it doesn't count that thing.
#[derive(Debug, Default)]
struct Counts {
    buses: Option<usize>,
    generators: Option<usize>,
    /// Lines plus transformers, a three-winding transformer once.
    branches: Option<usize>,
    switches: Option<usize>,
    loads: Option<usize>,
}

fn rs_power(text: &str) -> Option<Counts> {
    let p = power::load(text, false).ok()?;
    // Three-winding transformers get an internal star bus each.
    let names = power::bus_names(&p.case);
    let star = names.iter().filter(|n| n.starts_with("STR_PT_XF_")).count();
    let mut branches: Vec<&str> = p
        .elements
        .branches
        .iter()
        .map(|b| b.element.as_str())
        .collect();
    branches.sort_unstable();
    branches.dedup();
    Some(Counts {
        buses: Some(p.case.buses.len() - star),
        generators: Some(p.elements.generators.len()),
        branches: Some(branches.len()),
        // Power flow has no switching devices and sums loads per bus.
        switches: None,
        loads: None,
    })
}

fn nodal(bytes: &[u8]) -> Option<Counts> {
    use nodal::model::EquipmentKind;
    let c = nodal::parse_raw(bytes).ok()?;
    let of = |k: EquipmentKind| c.equipment.iter().filter(|e| e.kind == k).count();
    Some(Counts {
        buses: Some(c.buses.len()),
        generators: Some(c.generators.len()),
        branches: Some(of(EquipmentKind::Line) + of(EquipmentKind::Transformer)),
        switches: Some(of(EquipmentKind::Switch)),
        loads: Some(c.loads.len()),
    })
}

fn case_diff(name: &str, bytes: &[u8]) -> Option<Counts> {
    let c = parse_bytes(name, bytes).ok()?;
    let n = |section: &str| {
        c.sections
            .iter()
            .find(|(s, _)| s == section)
            .map_or(0, |(_, r)| r.len())
    };
    Some(Counts {
        buses: Some(n("BUS")),
        generators: Some(n("GENERATOR")),
        branches: Some(n("BRANCH") + n("TRANSFORMER")),
        switches: Some(n("SYSTEM SWITCHING DEVICE")),
        loads: Some(n("LOAD")),
    })
}

/// One count, read from a reader's [`Counts`].
type Quantity = fn(&Counts) -> Option<usize>;

/// Differences that are known and accepted: (fixture, what, why). `what` is
/// a count, or "read by <reader>" for a file a reader should read but can't.
const KNOWN: &[(&str, &str, &str)] = &[
    (
        "five_bus_substations.raw",
        "read by case-diff",
        "case-diff's schema has no SUBSTATION section (RAW 35 node-breaker data), so Records and Case diff can't read a file that has one",
    ),
    (
        "dc_modes.raw",
        "read by case-diff",
        "bus records leave out trailing fields (11 of 13); rs-power fills defaults, case-diff wants every field",
    ),
    (
        "revision_33.raw",
        "read by case-diff",
        "bus records leave out trailing fields (11 of 13)",
    ),
    (
        "transformers.raw",
        "read by case-diff",
        "bus records leave out trailing fields (11 of 13)",
    ),
];

/// The RAW revision in the file's first line.
fn revision(text: &str) -> Option<u32> {
    let first = text.lines().next()?;
    let first = first.split('/').next()?;
    first.split(',').nth(2)?.trim().parse().ok()
}

fn fixtures() -> Vec<PathBuf> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut out = vec![];
    for dir in [
        root.join("tests/fixtures"),
        root.join("../rs-power/tests/fixtures/raw"),
        root.join("../nodal"),
        root.join("../nodal/tests"),
    ] {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for e in entries.flatten() {
            let p = e.path();
            if p.extension()
                .and_then(|x| x.to_str())
                .is_some_and(|x| x.eq_ignore_ascii_case("raw"))
            {
                out.push(p);
            }
        }
    }
    out.sort();
    out
}

#[test]
fn the_raw_readers_count_the_same_case() {
    let mut problems = vec![];
    let mut table = String::new();
    let mut compared = 0;
    for path in fixtures() {
        let name = path.file_name().unwrap().to_string_lossy().into_owned();
        let bytes = std::fs::read(&path).unwrap();
        let text = String::from_utf8_lossy(&bytes);
        let rev = revision(&text);
        let all = [
            (
                "rs-power",
                (23..=35).contains(&rev.unwrap_or(0)),
                rs_power(&text),
            ),
            ("nodal", rev == Some(35), nodal(&bytes)),
            (
                "case-diff",
                matches!(rev, Some(33 | 35)),
                case_diff(&name, &bytes),
            ),
        ];
        // A reader that should read the file must.
        for (reader, should, counts) in &all {
            let what = format!("read by {reader}");
            let known = KNOWN.iter().any(|(f, w, _)| *f == name && *w == what);
            match (*should, counts.is_some(), known) {
                (true, false, false) => problems.push(format!("{name}: {reader} can't read it")),
                (_, true, true) => {
                    problems.push(format!("{name}: {what} works now; take it off KNOWN"))
                }
                _ => {}
            }
        }
        let readers: Vec<(&str, Counts)> = all
            .into_iter()
            .filter_map(|(r, _, c)| c.map(|c| (r, c)))
            .collect();
        let quantities: [(&str, Quantity); 5] = [
            ("buses", |c| c.buses),
            ("generators", |c| c.generators),
            ("branches", |c| c.branches),
            ("switches", |c| c.switches),
            ("loads", |c| c.loads),
        ];
        for (what, get) in quantities {
            let seen: BTreeMap<&str, usize> = readers
                .iter()
                .filter_map(|(r, c)| get(c).map(|n| (*r, n)))
                .collect();
            table.push_str(&format!("{name:<45} {what:<11} {seen:?}\n"));
            if seen.len() < 2 {
                continue;
            }
            compared += 1;
            let agree = seen.values().all(|n| Some(n) == seen.values().next());
            let known = KNOWN.iter().any(|(f, w, _)| *f == name && *w == what);
            match (agree, known) {
                (false, false) => problems.push(format!("{name}: {what} {seen:?}")),
                (true, true) => {
                    problems.push(format!("{name}: {what} now agree; take it off KNOWN"))
                }
                _ => {}
            }
        }
    }
    println!("{table}");
    assert!(
        compared > 20,
        "too few fixtures read by two readers:\n{table}"
    );
    assert!(problems.is_empty(), "{}", problems.join("\n"));
}
