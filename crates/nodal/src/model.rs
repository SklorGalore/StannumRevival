use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Diagnostic {
    pub severity: String,
    pub section: String,
    pub line: usize,
    pub message: String,
}

impl std::fmt::Display for Diagnostic {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} · line {}: {}", self.section, self.line, self.message)
    }
}

impl std::error::Error for Diagnostic {}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceRecord {
    pub line: usize,
    pub section: String,
    pub fields: BTreeMap<String, String>,
    pub raw_lines: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Bus {
    pub number: i32,
    pub name: String,
    pub base_kv: f64,
    pub bus_type: i32,
    pub area: Option<i32>,
    pub zone: Option<i32>,
    pub voltage_pu: Option<f64>,
    pub source: SourceRecord,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum EquipmentKind {
    Line,
    Transformer,
    Switch,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Equipment {
    pub id: String,
    pub kind: EquipmentKind,
    pub name: String,
    pub circuit: String,
    pub buses: Vec<i32>,
    pub status: i32,
    pub active_terminals: Vec<bool>,
    pub source: SourceRecord,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Injection {
    pub id: String,
    pub bus: i32,
    pub status: i32,
    pub p_mw: f64,
    pub q_mvar: f64,
    pub source: SourceRecord,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Case {
    pub revision: i32,
    pub base_mva: f64,
    pub frequency_hz: f64,
    pub zero_impedance_threshold: f64,
    pub titles: Vec<String>,
    pub encoding: String,
    pub buses: Vec<Bus>,
    pub equipment: Vec<Equipment>,
    pub loads: Vec<Injection>,
    pub generators: Vec<Injection>,
    pub diagnostics: Vec<Diagnostic>,
    pub skipped_sections: BTreeMap<String, usize>,
}
