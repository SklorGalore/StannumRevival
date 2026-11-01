//! Compare the latest solve with the voltages saved in the case file.
//!
//! A RAW or MATPOWER file saved by another tool after a solve holds that
//! tool's voltage at every bus. Comparing them with Stannum's own solve is
//! the quickest way for anyone to check Stannum against their tool of record
//! on their own case.

use crate::{Error, OpenCase};
use rs_power::BusType;
use serde::Serialize;
use std::collections::HashMap;

/// One bus: the file's voltage, the solved one and the difference.
#[derive(Clone, Debug, Serialize, schemars::JsonSchema)]
pub struct FileVoltageRow {
    pub bus: u32,
    pub name: String,
    pub base_kv: f64,
    pub area: u32,
    /// Voltage magnitude in the file, p.u.
    pub file_vm: f64,
    /// Solved voltage magnitude, p.u.
    pub solved_vm: f64,
    /// Solved minus file, p.u.
    pub dvm: f64,
    /// Voltage angle in the file, degrees.
    pub file_va: f64,
    /// Solved voltage angle, degrees.
    pub solved_va: f64,
    /// Solved minus file, degrees, after lining up the reference bus.
    pub dva: f64,
}

/// How the latest solve compares with the voltages saved in the file.
#[derive(Clone, Debug, Serialize, schemars::JsonSchema)]
pub struct FileVoltageComparison {
    /// Buses in both the file and the solve, in service.
    pub compared: usize,
    /// Mean |solved - file| voltage magnitude, p.u.
    pub mean_dvm: f64,
    /// Largest |solved - file| voltage magnitude, p.u.
    pub max_dvm: f64,
    /// The bus with the largest magnitude difference.
    pub max_dvm_bus: Option<u32>,
    /// Largest |solved - file| angle, degrees, after lining up the reference bus.
    pub max_dva: f64,
    pub max_dva_bus: Option<u32>,
    /// Buses within 0.001 p.u. of the file's magnitude.
    pub within_0_001: usize,
    /// Buses within 0.01 p.u. of the file's magnitude.
    pub within_0_01: usize,
    /// The bus whose angle both sides were lined up on (the file's first
    /// reference bus); none when the file has none in the solve.
    pub reference_bus: Option<u32>,
    /// True when the solve includes study edits, so it is not the file's case.
    pub with_study_edits: bool,
    /// The largest differences first, at most `limit` rows.
    pub rows: Vec<FileVoltageRow>,
}

impl OpenCase {
    /// Compare the latest solve with the voltages saved in the file.
    pub fn compare_file_voltages(&self, limit: usize) -> Result<FileVoltageComparison, Error> {
        let report = self
            .report
            .as_ref()
            .ok_or_else(|| Error::not_ready("Solve the power flow first."))?;
        if !report.converged {
            return Err(Error::not_ready(
                "The last solve didn't converge; solve again before comparing.",
            ));
        }
        let power = self.power.as_ref().map_err(|e| Error::from(e.clone()))?;
        let buses = &power.case.buses;
        let unsolved = buses
            .iter()
            .all(|b| (b.vm - 1.0).abs() < 1e-9 && b.va.abs() < 1e-9);
        if buses.is_empty() || unsolved {
            return Err(Error::not_ready(
                "The file holds no solved voltages (every bus is at 1 p.u. and 0 degrees), so there is nothing to compare with.",
            ));
        }
        let file: HashMap<u32, (f64, f64, BusType)> =
            buses.iter().map(|b| (b.id, (b.vm, b.va, b.kind))).collect();
        let reference = buses
            .iter()
            .find(|b| b.kind == BusType::Reference && report.buses.iter().any(|r| r.id == b.id))
            .map(|b| b.id);
        let offset = reference
            .and_then(|id| {
                let solved = report.buses.iter().find(|r| r.id == id)?;
                Some(solved.va - file[&id].1)
            })
            .unwrap_or(0.0);
        let mut rows = Vec::new();
        for r in report.buses.iter() {
            let Some(&(fvm, fva, kind)) = file.get(&r.id) else {
                continue;
            };
            if kind == BusType::Isolated || r.kind == "Isolated" {
                continue;
            }
            rows.push(FileVoltageRow {
                bus: r.id,
                name: r.name.clone(),
                base_kv: r.base_kv,
                area: r.area,
                file_vm: fvm,
                solved_vm: r.vm,
                dvm: r.vm - fvm,
                file_va: fva,
                solved_va: r.va,
                dva: r.va - fva - offset,
            });
        }
        let compared = rows.len();
        let mean_dvm = if compared == 0 {
            0.0
        } else {
            rows.iter().map(|r| r.dvm.abs()).sum::<f64>() / compared as f64
        };
        let worst = |key: fn(&FileVoltageRow) -> f64| {
            rows.iter()
                .max_by(|a, b| key(a).abs().total_cmp(&key(b).abs()))
                .map(|r| (key(r).abs(), r.bus))
        };
        let (max_dvm, max_dvm_bus) = worst(|r| r.dvm).map_or((0.0, None), |(d, b)| (d, Some(b)));
        let (max_dva, max_dva_bus) = worst(|r| r.dva).map_or((0.0, None), |(d, b)| (d, Some(b)));
        let within_0_001 = rows.iter().filter(|r| r.dvm.abs() <= 0.001).count();
        let within_0_01 = rows.iter().filter(|r| r.dvm.abs() <= 0.01).count();
        rows.sort_by(|a, b| {
            b.dvm
                .abs()
                .total_cmp(&a.dvm.abs())
                .then(b.dva.abs().total_cmp(&a.dva.abs()))
        });
        rows.truncate(limit);
        log::info!(
            "Compared {compared} bus voltages with the file: mean {mean_dvm:.5} p.u., largest {max_dvm:.4} p.u.{}, angle {max_dva:.3} deg",
            max_dvm_bus
                .map(|b| format!(" at bus {b}"))
                .unwrap_or_default()
        );
        Ok(FileVoltageComparison {
            compared,
            mean_dvm,
            max_dvm,
            max_dvm_bus,
            max_dva,
            max_dva_bus,
            within_0_001,
            within_0_01,
            reference_bus: reference,
            with_study_edits: !self.edits.is_empty(),
            rows,
        })
    }
}
