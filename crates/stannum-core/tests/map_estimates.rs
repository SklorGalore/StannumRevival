//! Buses the coordinates leave out are placed from line lengths: hide some
//! substations of a case whose places are known and see how close the
//! estimates land.

use stannum_core::geo::{self, CoordinateFile, LoadedCoordinates, Place};
use std::collections::HashMap;
use std::path::PathBuf;

fn sample(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("samples")
        .join(name)
}

fn km(a: (f64, f64), b: (f64, f64)) -> f64 {
    let c = ((a.0 + b.0) / 2.0).to_radians().cos();
    ((a.0 - b.0) * 111.195).hypot((a.1 - b.1) * 111.195 * c)
}

/// Mean and worst error (km) over the hidden buses that got a place.
fn errors(truth: &HashMap<u32, Place>, hidden: &[u32], got: &geo::BusCoordinates) -> (f64, f64) {
    let mut e = Vec::new();
    for b in hidden {
        let (lat, lon, source, _) = got.of(*b).expect("hidden bus placed");
        assert_eq!(source, "estimated");
        let t = &truth[b];
        e.push(km((lat, lon), (t.latitude, t.longitude)));
    }
    let mean = e.iter().sum::<f64>() / e.len() as f64;
    (mean, e.iter().copied().fold(0.0, f64::max))
}

/// Hides every `every`th substation of a MATPOWER case and its coordinates
/// file, and returns (hidden buses, mean and worst error fitted, the same
/// with averages only), in km.
fn hide_and_place(
    case: &std::path::Path,
    coords: &std::path::Path,
    every: usize,
) -> (usize, f64, f64, f64, f64) {
    let text = std::fs::read_to_string(case).unwrap();
    let power = stannum_core::power::load(&text, true).unwrap();
    let truth = LoadedCoordinates::read(coords).unwrap().file;
    // Hide whole substations, all their buses together.
    let mut subs: Vec<&str> = truth
        .places
        .values()
        .filter_map(|p| p.substation.as_deref())
        .collect();
    subs.sort_unstable();
    subs.dedup();
    let hide: std::collections::HashSet<&str> = subs.iter().step_by(every).copied().collect();
    let mut kept = CoordinateFile::default();
    let mut hidden = Vec::new();
    for (bus, p) in &truth.places {
        if hide.contains(p.substation.as_deref().unwrap()) {
            hidden.push(*bus);
        } else {
            kept.places.insert(*bus, p.clone());
        }
    }
    hidden.sort_unstable();
    let file = LoadedCoordinates {
        path: "kept.csv".into(),
        file: kept,
    };
    let spans = geo::spans(&power.elements.branches, &power.branch_lengths);
    let fitted = geo::resolve(Some(&file), &HashMap::new(), &power.case, &spans, "case").unwrap();
    assert_eq!(fitted.summary.estimated_from, "reactance");
    assert_eq!(fitted.summary.estimated, hidden.len());

    // Without voltages there's no reactance in ohms: averages only.
    let mut flat = power.case.clone();
    for b in &mut flat.buses {
        b.base_kv = 0.0;
    }
    let averaged = geo::resolve(Some(&file), &HashMap::new(), &flat, &spans, "case").unwrap();
    assert_eq!(averaged.summary.estimated_from, "neighbours");

    let (fit_mean, fit_worst) = errors(&truth.places, &hidden, &fitted);
    let (avg_mean, avg_worst) = errors(&truth.places, &hidden, &averaged);
    println!(
        "{} of {} buses hidden: lengths mean {fit_mean:.1} km, worst {fit_worst:.1} km; \
         averages mean {avg_mean:.1} km, worst {avg_worst:.1} km",
        hidden.len(),
        truth.places.len(),
    );
    (hidden.len(), fit_mean, fit_worst, avg_mean, avg_worst)
}

#[test]
fn line_reactance_places_hidden_substations_closer_than_averages() {
    let (_, fit, _, avg, _) = hide_and_place(
        &sample("synthetic_200.m"),
        &sample("synthetic_200.coordinates.csv"),
        4,
    );
    assert!(fit < avg / 2.0, "{fit} vs {avg}");
}

/// The same on any case: `STANNUM_MAP_CASE=case.m` beside `case.coords.csv`
/// (or `STANNUM_MAP_COORDS`), `STANNUM_MAP_EVERY` (default 4).
#[test]
#[ignore]
fn hide_and_place_any_case() {
    let case = PathBuf::from(std::env::var("STANNUM_MAP_CASE").expect("STANNUM_MAP_CASE"));
    let coords = std::env::var("STANNUM_MAP_COORDS")
        .map(PathBuf::from)
        .unwrap_or_else(|_| case.with_extension("coords.csv"));
    let every = std::env::var("STANNUM_MAP_EVERY").map_or(4, |v| v.parse().unwrap());
    hide_and_place(&case, &coords, every);
}
