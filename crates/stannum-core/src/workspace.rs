//! The open case shared by every Stannum module.
//!
//! Opening a file parses it once per engine: rs-power for power flow,
//! nodal for topology. Either engine may decline a file (for example nodal
//! only reads RAW revision 35); the other modules keep working and the
//! frontend shows why a module is unavailable.

use crate::diff::{self, Comparison};
use crate::power::{self, PowerCase, SolveSettings};
use nodal::topology::{Analysis, GraphData, Path as TopologyPath, Topology, TopologyOptions};
use rs_power::{PowerFlowResult, PowerFlowSensitivity};
use serde::Serialize;
use std::path::{Path, PathBuf};
use std::time::Instant;

#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum CaseFormat {
    Matpower,
    Raw,
}

pub struct TopologyState {
    pub case: nodal::model::Case,
    pub topology: Topology,
    pub merge_zero_impedance: bool,
}

pub struct OpenCase {
    pub path: PathBuf,
    pub name: String,
    pub format: CaseFormat,
    pub power: Result<PowerCase, String>,
    pub topology: Result<TopologyState, String>,
    pub solved: Option<(PowerFlowResult, SolveSettings)>,
    pub sensitivity: Option<(PowerFlowSensitivity, power::ModelChoice)>,
}

#[derive(Default)]
pub struct Workspace {
    pub case: Option<OpenCase>,
    pub comparison: Option<Comparison>,
}

#[derive(Serialize, Debug, Clone)]
pub struct ModuleStatus<T: Serialize> {
    pub available: bool,
    pub error: Option<String>,
    pub info: Option<T>,
}

impl<T: Serialize> ModuleStatus<T> {
    fn from<S>(r: &Result<S, String>, info: impl FnOnce(&S) -> T) -> Self {
        match r {
            Ok(s) => Self {
                available: true,
                error: None,
                info: Some(info(s)),
            },
            Err(e) => Self {
                available: false,
                error: Some(e.clone()),
                info: None,
            },
        }
    }
}

#[derive(Serialize, Debug, Clone)]
pub struct PowerInfo {
    pub counts: power::CaseCounts,
    pub warnings: Vec<power::Warning>,
}

#[derive(Serialize, Debug, Clone)]
pub struct TopologyInfo {
    pub revision: i32,
    pub buses: usize,
    pub equipment: usize,
    pub titles: Vec<String>,
}

#[derive(Serialize, Debug, Clone)]
pub struct CaseSummary {
    pub path: String,
    pub name: String,
    pub format: CaseFormat,
    pub power: ModuleStatus<PowerInfo>,
    pub topology: ModuleStatus<TopologyInfo>,
    /// RAW files can be used as a comparison baseline.
    pub diffable: bool,
}

#[derive(Serialize)]
pub struct TopologySnapshot<'a> {
    pub case: &'a nodal::model::Case,
    pub analysis: Analysis,
    pub graph: GraphData,
}

#[derive(Serialize)]
pub struct TopologyView {
    pub analysis: Analysis,
    pub graph: GraphData,
}

/// UTF-8 first, then Windows-1252 (the encoding PSS/E exports use).
pub fn decode_text(bytes: &[u8]) -> String {
    match std::str::from_utf8(bytes) {
        Ok(s) => s.to_string(),
        Err(_) => encoding_rs::WINDOWS_1252
            .decode_without_bom_handling(bytes)
            .0
            .into_owned(),
    }
}

fn elapsed_ms(start: Instant) -> f64 {
    start.elapsed().as_secs_f64() * 1000.0
}

impl OpenCase {
    pub fn from_bytes(path: &Path, bytes: &[u8]) -> Self {
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| path.display().to_string());
        let format = match path.extension().and_then(|e| e.to_str()) {
            Some(e) if e.eq_ignore_ascii_case("m") => CaseFormat::Matpower,
            _ => CaseFormat::Raw,
        };
        let text = decode_text(bytes);
        let power = power::load(&text, format == CaseFormat::Matpower);
        let topology = match format {
            CaseFormat::Matpower => {
                Err("Topology analysis reads PSS/E RAW revision 35 files.".to_string())
            }
            CaseFormat::Raw => nodal::parse_raw(bytes)
                .map(|case| TopologyState {
                    topology: Topology::new(&case),
                    case,
                    merge_zero_impedance: false,
                })
                .map_err(|d| d.to_string()),
        };
        Self {
            path: path.to_path_buf(),
            name,
            format,
            power,
            topology,
            solved: None,
            sensitivity: None,
        }
    }

    pub fn summary(&self) -> CaseSummary {
        CaseSummary {
            path: self.path.display().to_string(),
            name: self.name.clone(),
            format: self.format,
            power: ModuleStatus::from(&self.power, |p| PowerInfo {
                counts: power::counts(&p.case),
                warnings: p.warnings.clone(),
            }),
            topology: ModuleStatus::from(&self.topology, |t| TopologyInfo {
                revision: t.case.revision,
                buses: t.case.buses.len(),
                equipment: t.case.equipment.len(),
                titles: t.case.titles.clone(),
            }),
            diffable: self.format == CaseFormat::Raw,
        }
    }

    fn power_case(&self) -> Result<&PowerCase, String> {
        self.power.as_ref().map_err(|e| e.clone())
    }

    pub fn solve(&mut self, settings: SolveSettings) -> Result<power::SolveReport, String> {
        let power = self.power_case()?;
        let start = Instant::now();
        let result =
            rs_power::solve(&power.case, &settings.options()).map_err(|e| e.to_string())?;
        let report = power::report(&result, &power.elements, &settings, elapsed_ms(start));
        self.solved = Some((result, settings));
        Ok(report)
    }

    pub fn sensitivities(
        &mut self,
        settings: SolveSettings,
    ) -> Result<power::SensitivitySummary, String> {
        let case = &self.power_case()?.case;
        let start = Instant::now();
        let s = rs_power::calculate_sensitivities(case, &settings.options())
            .map_err(|e| e.to_string())?;
        let summary = power::sensitivity_summary(&s, settings.model, elapsed_ms(start));
        self.sensitivity = Some((s, settings.model));
        Ok(summary)
    }

    pub fn responses(
        &self,
        column: usize,
        quantity: &str,
        engineering: bool,
    ) -> Result<power::ResponseTable, String> {
        let (s, _) = self
            .sensitivity
            .as_ref()
            .ok_or("Calculate sensitivities first.")?;
        let q = power::Quantity::parse(quantity).ok_or("Unknown quantity")?;
        power::responses(s, column, q, engineering)
    }

    /// Write the input case, or the latest solved case, as MATPOWER.
    pub fn export_matpower(&self, path: &Path, solved: bool) -> Result<(), String> {
        let case = if solved {
            &self
                .solved
                .as_ref()
                .ok_or("Solve the case before exporting results.")?
                .0
                .case
        } else {
            &self.power_case()?.case
        };
        rs_power::save_matpower(path, case).map_err(|e| e.to_string())
    }

    /// The case as buses, generators and branches for the one-line diagram.
    pub fn diagram_network(&self) -> Result<crate::diagram::CaseNetwork, String> {
        Ok(crate::diagram::network(self.power_case()?))
    }

    fn topology_state(&self) -> Result<&TopologyState, String> {
        self.topology.as_ref().map_err(|e| e.clone())
    }

    pub fn topology_snapshot(&self) -> Result<TopologySnapshot<'_>, String> {
        let t = self.topology_state()?;
        Ok(TopologySnapshot {
            case: &t.case,
            analysis: t.topology.analyze(&t.case),
            graph: t.topology.graph_data(),
        })
    }

    pub fn set_merge_zero_impedance(&mut self, enabled: bool) -> Result<TopologyView, String> {
        let t = self.topology.as_mut().map_err(|e| e.clone())?;
        t.topology = Topology::with_options(
            &t.case,
            TopologyOptions {
                merge_zero_impedance: enabled,
            },
        );
        t.merge_zero_impedance = enabled;
        Ok(TopologyView {
            analysis: t.topology.analyze(&t.case),
            graph: t.topology.graph_data(),
        })
    }

    pub fn shortest_path(&self, from: i32, to: i32) -> Result<Option<TopologyPath>, String> {
        let t = self.topology_state()?;
        t.topology
            .shortest_path(&t.case, from, to)
            .map_err(|d| d.to_string())
    }
}

impl Workspace {
    pub fn open(&mut self, path: &Path) -> Result<CaseSummary, String> {
        let bytes = std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
        let case = OpenCase::from_bytes(path, &bytes);
        if case.power.is_err() && case.topology.is_err() && case.format == CaseFormat::Matpower {
            return Err(case.power.err().unwrap_or_default());
        }
        let summary = case.summary();
        self.case = Some(case);
        Ok(summary)
    }

    pub fn close(&mut self) {
        self.case = None;
    }

    pub fn current(&self) -> Result<&OpenCase, String> {
        self.case
            .as_ref()
            .ok_or_else(|| "Open a case first.".into())
    }

    pub fn current_mut(&mut self) -> Result<&mut OpenCase, String> {
        self.case
            .as_mut()
            .ok_or_else(|| "Open a case first.".into())
    }

    pub fn compare(
        &mut self,
        baseline: &Path,
        comparison: &Path,
    ) -> Result<diff::DiffSummary, String> {
        // Clear first so a failure never leaves stale results on screen.
        self.comparison = None;
        let before = diff::parse_path(baseline).map_err(|e| e.to_string())?;
        let after = diff::parse_path(comparison).map_err(|e| e.to_string())?;
        let result = diff::compare_cases(before, after);
        let summary = result.summary();
        self.comparison = Some(result);
        Ok(summary)
    }

    pub fn comparison(&self) -> Result<&Comparison, String> {
        self.comparison
            .as_ref()
            .ok_or_else(|| "Compare two files first.".into())
    }
}
