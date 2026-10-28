//! Small cases built into Stannum, so there is something to open on first
//! run. Their files are in `crates/stannum-core/samples`; see the README
//! there for where each comes from.

use crate::error::Error;
use crate::workspace::{CaseSummary, Workspace};
use schemars::JsonSchema;
use serde::Serialize;
use std::path::{Path, PathBuf};

/// One file of a sample: its name on disk and its text.
struct File {
    name: &'static str,
    text: &'static str,
}

struct Sample {
    id: &'static str,
    title: &'static str,
    description: &'static str,
    format: &'static str,
    buses: usize,
    case: File,
    coordinates: Option<File>,
}

macro_rules! file {
    ($name:literal) => {
        File {
            name: $name,
            text: include_str!(concat!("../samples/", $name)),
        }
    };
}

const SAMPLES: &[Sample] = &[
    Sample {
        id: "case9",
        title: "Nine-bus case",
        description: "MATPOWER's case9: three generators, three loads. The classic first power flow.",
        format: "matpower",
        buses: 9,
        case: file!("case9.m"),
        coordinates: None,
    },
    Sample {
        id: "five_bus",
        title: "Five-bus case",
        description: "PSS/E RAW v35 with two areas, a transformer and a 69 kV spur, placed on a map.",
        format: "raw",
        buses: 5,
        case: file!("five_bus.raw"),
        coordinates: Some(file!("five_bus.coordinates.csv")),
    },
    Sample {
        id: "five_bus_devices",
        title: "Five-bus case with devices",
        description: "The five-bus case with a three-winding transformer, a breaker and switched shunts.",
        format: "raw",
        buses: 7,
        case: file!("five_bus_devices.raw"),
        coordinates: Some(file!("five_bus.coordinates.csv")),
    },
    Sample {
        id: "synthetic_200",
        title: "Synthetic 200-bus grid",
        description: "Made-up 345/115 kV grid from Stannum's synthetic case generator, with bus coordinates for the map.",
        format: "matpower",
        buses: 201,
        case: file!("synthetic_200.m"),
        coordinates: Some(file!("synthetic_200.coordinates.csv")),
    },
];

/// A case built into Stannum.
#[derive(Clone, Debug, Serialize, JsonSchema)]
pub struct SampleCase {
    /// What `open_sample` takes.
    pub id: &'static str,
    pub title: &'static str,
    pub description: &'static str,
    /// "matpower" or "raw".
    pub format: &'static str,
    /// How many buses it has, counting the star bus of a three-winding transformer.
    pub buses: usize,
    /// Whether it comes with bus coordinates for the map.
    pub has_coordinates: bool,
    /// The case file's name.
    pub file_name: &'static str,
}

/// Every built-in sample case.
pub fn list() -> Vec<SampleCase> {
    SAMPLES
        .iter()
        .map(|s| SampleCase {
            id: s.id,
            title: s.title,
            description: s.description,
            format: s.format,
            buses: s.buses,
            has_coordinates: s.coordinates.is_some(),
            file_name: s.case.name,
        })
        .collect()
}

/// Where samples are written when the caller names no folder.
pub fn default_dir() -> PathBuf {
    std::env::temp_dir().join("stannum-samples")
}

/// Write a sample's files into `dir` (made if missing), replacing any copy
/// there, and return the case file's path and the coordinates file's path.
pub fn write(id: &str, dir: &Path) -> Result<(PathBuf, Option<PathBuf>), Error> {
    let sample = SAMPLES.iter().find(|s| s.id == id).ok_or_else(|| {
        let known: Vec<_> = SAMPLES.iter().map(|s| s.id).collect();
        Error::invalid_arguments(format!(
            "Stannum has no sample called {id}. The samples are {}.",
            known.join(", ")
        ))
    })?;
    std::fs::create_dir_all(dir).map_err(|e| Error::file(dir, e))?;
    let put = |f: &File| -> Result<PathBuf, Error> {
        let path = dir.join(f.name);
        std::fs::write(&path, f.text).map_err(|e| Error::file(&path, e))?;
        Ok(path)
    };
    let case = put(&sample.case)?;
    let coordinates = sample.coordinates.as_ref().map(put).transpose()?;
    Ok((case, coordinates))
}

/// Write a sample into `dir` (or [`default_dir`]) and open it, with its bus
/// coordinates attached.
pub fn open(ws: &mut Workspace, id: &str, dir: Option<&Path>) -> Result<CaseSummary, Error> {
    let dir = dir.map(Path::to_path_buf).unwrap_or_else(default_dir);
    let (case, coordinates) = write(id, &dir)?;
    ws.open(&case)?;
    let open = ws.case_mut(None)?;
    if let Some(path) = coordinates {
        open.load_coordinates(&path)?;
    }
    Ok(open.summary())
}
