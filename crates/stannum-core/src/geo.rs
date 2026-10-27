//! Bus coordinates for the map: a CSV side file beside the case.
//!
//! The file names buses by number, so one file serves every variant of a
//! case. Each row gives a latitude and longitude in decimal degrees (WGS84)
//! and, optionally, the substation the bus sits in:
//!
//! ```text
//! bus,latitude,longitude,substation
//! 1,30.2672,-97.7431,Austin North
//! 2,30.2672,-97.7431,Austin North
//! 5,29.7604,-95.3698,Houston
//! ```
//!
//! Columns are found by their header name, in any order and any case, and
//! extra columns are ignored. Rows that can't be read are skipped and
//! reported with their line number; the rest of the file still attaches.
//!
//! A bus the file leaves out is placed at the average of its neighbours that
//! have a place, working outward from the placed buses, and is marked as
//! estimated. Buses with no placed bus anywhere in their island stay without
//! coordinates.

use rs_power::Case;
use serde::Serialize;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Header names accepted for each column, compared lowercase with spaces,
/// dashes and underscores removed.
const BUS: &[&str] = &["bus", "busnumber", "busnum", "number", "busid", "i"];
const LAT: &[&str] = &["latitude", "lat"];
const LON: &[&str] = &["longitude", "lon", "long", "lng"];
const SUBSTATION: &[&str] = &["substation", "substationname", "sub", "subname", "site"];

/// How many problem lines a summary lists by name; the rest are counted.
const LISTED_PROBLEMS: usize = 20;

/// One row of the file.
#[derive(Clone, Debug, PartialEq)]
pub struct Place {
    pub latitude: f64,
    pub longitude: f64,
    pub substation: Option<String>,
}

/// A line of the file that was skipped, and why.
#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct Problem {
    pub line: usize,
    pub message: String,
}

/// A coordinates file as read, before it meets a case.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct CoordinateFile {
    /// Bus number to place. A bus listed twice keeps its last row.
    pub places: HashMap<u32, Place>,
    pub problems: Vec<Problem>,
}

fn normalise(header: &str) -> String {
    header
        .trim()
        .trim_matches('"')
        .chars()
        .filter(|c| !matches!(c, ' ' | '-' | '_'))
        .flat_map(char::to_lowercase)
        .collect()
}

/// Split one CSV line, honouring double quotes ("" inside quotes is a quote).
fn split(line: &str) -> Vec<String> {
    let mut fields = vec![];
    let mut field = String::new();
    let mut quoted = false;
    let mut chars = line.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '"' if quoted && chars.peek() == Some(&'"') => {
                field.push('"');
                chars.next();
            }
            '"' => quoted = !quoted,
            ',' if !quoted => fields.push(std::mem::take(&mut field)),
            _ => field.push(c),
        }
    }
    fields.push(field);
    fields.into_iter().map(|f| f.trim().to_string()).collect()
}

/// Read a coordinates CSV. Fails only when the header can't be used; bad
/// rows are listed in [`CoordinateFile::problems`].
pub fn parse(text: &str) -> Result<CoordinateFile, String> {
    let mut lines = text
        .lines()
        .enumerate()
        .map(|(i, l)| (i + 1, l.trim_start_matches('\u{feff}')))
        .filter(|(_, l)| !l.trim().is_empty() && !l.trim_start().starts_with('#'));
    let (_, header) = lines.next().ok_or("the file is empty")?;
    let header: Vec<String> = split(header).iter().map(|h| normalise(h)).collect();
    let find = |names: &[&str]| header.iter().position(|h| names.contains(&h.as_str()));
    let (Some(bus), Some(lat), Some(lon)) = (find(BUS), find(LAT), find(LON)) else {
        return Err(
            "the first line must name the columns, with at least bus, latitude and longitude"
                .into(),
        );
    };
    let substation = find(SUBSTATION);

    let mut file = CoordinateFile::default();
    for (line, text) in lines {
        let row = split(text);
        let cell = |i: usize| row.get(i).map(String::as_str).unwrap_or("");
        let problem = |message: String| Problem { line, message };
        let number = match cell(bus).parse::<f64>() {
            Ok(n) if n >= 1.0 && n.fract() == 0.0 && n <= u32::MAX as f64 => n as u32,
            _ => {
                file.problems
                    .push(problem(format!("\"{}\" isn't a bus number", cell(bus))));
                continue;
            }
        };
        let degrees = |i: usize, limit: f64, what: &str| match cell(i).parse::<f64>() {
            Ok(v) if v.is_finite() && v.abs() <= limit => Ok(v),
            Ok(v) if v.is_finite() => Err(format!(
                "bus {number}: {what} {v} is outside ±{limit} degrees"
            )),
            _ => Err(format!(
                "bus {number}: \"{}\" isn't a {what} in decimal degrees",
                cell(i)
            )),
        };
        match (
            degrees(lat, 90.0, "latitude"),
            degrees(lon, 180.0, "longitude"),
        ) {
            (Ok(latitude), Ok(longitude)) => {
                let substation = substation
                    .map(cell)
                    .filter(|s| !s.is_empty())
                    .map(str::to_string);
                file.places.insert(
                    number,
                    Place {
                        latitude,
                        longitude,
                        substation,
                    },
                );
            }
            (Err(e), _) | (_, Err(e)) => file.problems.push(problem(e)),
        }
    }
    Ok(file)
}

/// A coordinates file attached to the open case.
#[derive(Clone, Debug)]
pub struct LoadedCoordinates {
    pub path: PathBuf,
    pub file: CoordinateFile,
}

impl LoadedCoordinates {
    pub fn read(path: &Path) -> Result<Self, String> {
        let bytes = std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
        let file = parse(&crate::workspace::decode_text(&bytes))
            .map_err(|e| format!("{}: {e}", path.display()))?;
        Ok(Self {
            path: path.to_path_buf(),
            file,
        })
    }

    /// Where every bus of `case` goes, and how the file matched the case.
    pub fn resolve(&self, case: &Case) -> BusCoordinates {
        resolve(&self.file, case, &self.path)
    }
}

/// What the app shows about the attached coordinates file.
#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct CoordinatesSummary {
    pub path: String,
    pub name: String,
    /// Rows read from the file.
    pub rows: usize,
    /// Buses of the case placed by the file.
    pub from_file: usize,
    /// Buses placed from their neighbours.
    pub estimated: usize,
    /// Buses with no place at all (no placed bus in their island).
    pub missing: usize,
    /// Rows naming buses this case doesn't have.
    pub unknown_buses: usize,
    /// Lines skipped, with the reason; at most 20 are listed.
    pub problems: Vec<Problem>,
    /// Lines skipped in all.
    pub problem_count: usize,
}

/// Where each bus goes, as columns in the case's bus order.
#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct BusCoordinates {
    pub bus: Vec<u32>,
    /// Null for a bus with no place.
    pub latitude: Vec<Option<f64>>,
    pub longitude: Vec<Option<f64>>,
    /// "file", "estimated" or "none".
    pub source: Vec<&'static str>,
    /// The substation the file names, if any.
    pub substation: Vec<Option<String>>,
    pub summary: CoordinatesSummary,
}

impl BusCoordinates {
    /// The place of one bus: latitude, longitude, source and substation.
    pub fn of(&self, bus: u32) -> Option<(f64, f64, &'static str, Option<&str>)> {
        let i = self.bus.iter().position(|&b| b == bus)?;
        Some((
            self.latitude[i]?,
            self.longitude[i]?,
            self.source[i],
            self.substation[i].as_deref(),
        ))
    }
}

fn resolve(file: &CoordinateFile, case: &Case, path: &Path) -> BusCoordinates {
    let n = case.buses.len();
    let index: HashMap<u32, usize> = case
        .buses
        .iter()
        .enumerate()
        .map(|(i, b)| (b.id, i))
        .collect();
    let mut place: Vec<Option<(f64, f64)>> = vec![None; n];
    let mut source = vec!["none"; n];
    let mut substation = vec![None; n];
    for (i, b) in case.buses.iter().enumerate() {
        if let Some(p) = file.places.get(&b.id) {
            place[i] = Some((p.latitude, p.longitude));
            source[i] = "file";
            substation[i] = p.substation.clone();
        }
    }
    let from_file = source.iter().filter(|s| **s == "file").count();

    // Neighbours by branch, in or out of service: geography doesn't change
    // with switching.
    let mut neighbours: Vec<Vec<usize>> = vec![vec![]; n];
    for br in &case.branches {
        if let (Some(&f), Some(&t)) = (index.get(&br.from), index.get(&br.to))
            && f != t
        {
            neighbours[f].push(t);
            neighbours[t].push(f);
        }
    }
    // Ring by ring outward: each round places every bus with a placed
    // neighbour at their average, so a bus's estimate only uses buses
    // closer to the file's places than it is.
    let mut frontier: Vec<usize> = (0..n)
        .filter(|&i| place[i].is_none() && neighbours[i].iter().any(|&j| place[j].is_some()))
        .collect();
    while !frontier.is_empty() {
        let placed: Vec<(usize, (f64, f64))> = frontier
            .iter()
            .map(|&i| {
                let known: Vec<(f64, f64)> =
                    neighbours[i].iter().filter_map(|&j| place[j]).collect();
                let k = known.len() as f64;
                let lat = known.iter().map(|p| p.0).sum::<f64>() / k;
                let lon = known.iter().map(|p| p.1).sum::<f64>() / k;
                (i, (lat, lon))
            })
            .collect();
        for &(i, p) in &placed {
            place[i] = Some(p);
            source[i] = "estimated";
        }
        let mut next: Vec<usize> = placed
            .iter()
            .flat_map(|&(i, _)| neighbours[i].iter().copied())
            .filter(|&j| place[j].is_none())
            .collect();
        next.sort_unstable();
        next.dedup();
        frontier = next;
    }

    let estimated = source.iter().filter(|s| **s == "estimated").count();
    let unknown_buses = file
        .places
        .keys()
        .filter(|b| !index.contains_key(b))
        .count();
    let summary = CoordinatesSummary {
        path: path.display().to_string(),
        name: path
            .file_name()
            .map(|f| f.to_string_lossy().into_owned())
            .unwrap_or_default(),
        rows: file.places.len(),
        from_file,
        estimated,
        missing: n - from_file - estimated,
        unknown_buses,
        problems: file
            .problems
            .iter()
            .take(LISTED_PROBLEMS)
            .cloned()
            .collect(),
        problem_count: file.problems.len(),
    };
    BusCoordinates {
        bus: case.buses.iter().map(|b| b.id).collect(),
        latitude: place.iter().map(|p| p.map(|p| p.0)).collect(),
        longitude: place.iter().map(|p| p.map(|p| p.1)).collect(),
        source,
        substation,
        summary,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_columns_by_header_in_any_order() {
        let f = parse(
            "\u{feff}Longitude, Sub-Station ,BUS_NUMBER,Lat,notes\n\
             -97.5,\"North, Main\",1,30.25,x\n\
             # a comment\n\
             \n\
             -95.0,,2,29.5\n",
        )
        .unwrap();
        assert!(f.problems.is_empty(), "{:?}", f.problems);
        assert_eq!(
            f.places[&1],
            Place {
                latitude: 30.25,
                longitude: -97.5,
                substation: Some("North, Main".into())
            }
        );
        assert_eq!(f.places[&2].substation, None);
    }

    #[test]
    fn bad_rows_are_listed_and_skipped() {
        let f = parse("bus,lat,lon\n1,30,-97\nx,30,-97\n3,95,-97\n4,30,east\n").unwrap();
        assert_eq!(f.places.len(), 1);
        let lines: Vec<usize> = f.problems.iter().map(|p| p.line).collect();
        assert_eq!(lines, [3, 4, 5]);
        assert!(f.problems[1].message.contains("outside ±90"));
    }

    #[test]
    fn needs_the_three_columns() {
        assert!(parse("bus,lat\n1,30\n").is_err());
        assert!(parse("").is_err());
    }
}
