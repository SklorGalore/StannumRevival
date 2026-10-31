//! Compact results files (`.stnr`): every finding of a contingency run as
//! one Arrow IPC stream, zstd compressed, with names dictionary encoded.
//!
//! The stream is read batch by batch, so a reader never needs the whole
//! run in memory, and any Arrow reader opens it: pyarrow
//! (`pa.ipc.open_stream`), polars (`pl.read_ipc_stream`), pandas through
//! pyarrow, DuckDB through its arrow extension. The schema's metadata
//! carries the format version, the case, the run's summary and settings.
//!
//! One row per finding, in rank order: each branch overload, voltage
//! problem and interface as the run reported it, one `not_converged` row
//! for an outage whose AC solve failed, and one `ok` row for an outage
//! with nothing to report, so the file says which outages were studied.

use crate::contingency::{ContingencyResults, ContingencyRow};
use arrow_array::builder::{Float32Builder, StringDictionaryBuilder, UInt32Builder};
use arrow_array::types::Int32Type;
use arrow_array::{ArrayRef, RecordBatch};
use arrow_ipc::CompressionType;
use arrow_ipc::writer::{IpcWriteOptions, StreamWriter};
use arrow_schema::{DataType, Field, Schema};
use serde::Serialize;
use std::collections::HashMap;
use std::io::Write;
use std::sync::Arc;

/// The format and version, in the schema metadata as `stannum.format`.
pub const RESULTS_FORMAT: &str = "stannum-results 0.1";

/// Rows per record batch: small enough to stream, large enough to compress.
const BATCH_ROWS: usize = 65_536;

/// What [`write_contingency_results`] wrote.
#[derive(Clone, Debug, Serialize, schemars::JsonSchema)]
pub struct ResultsExport {
    /// Rows written: one per finding, plus one per outage with none.
    pub rows: usize,
    /// Outages in the file.
    pub outages: usize,
    /// Rows that are violations (not `ok`).
    pub violations: usize,
    /// File size.
    pub bytes: u64,
}

type Dict = StringDictionaryBuilder<Int32Type>;

/// The columns, in schema order.
const COLUMNS: &[(&str, ColumnType)] = &[
    ("rank", ColumnType::U32),
    ("category", ColumnType::Text),
    ("contingency", ColumnType::Text),
    ("outage", ColumnType::Text),
    ("description", ColumnType::Text),
    ("solved", ColumnType::Text),
    ("type", ColumnType::Text),
    ("element", ColumnType::Text),
    ("element_id", ColumnType::Text),
    ("from_bus", ColumnType::U32),
    ("to_bus", ColumnType::U32),
    ("bus", ColumnType::U32),
    ("value_pct", ColumnType::F64),
    ("before_pct", ColumnType::F64),
    ("limit_pct", ColumnType::F64),
    ("mw", ColumnType::F64),
    ("before_mw", ColumnType::F64),
    ("vm_pu", ColumnType::F64),
    ("before_vm_pu", ColumnType::F64),
    ("vmin_pu", ColumnType::F64),
    ("vmax_pu", ColumnType::F64),
    ("note", ColumnType::Text),
];

#[derive(Clone, Copy)]
enum ColumnType {
    U32,
    F64,
    Text,
}

/// The stream's schema, with `metadata`.
fn schema(metadata: HashMap<String, String>) -> Schema {
    let fields: Vec<Field> = COLUMNS
        .iter()
        .map(|&(name, t)| {
            let data_type = match t {
                ColumnType::U32 => DataType::UInt32,
                ColumnType::F64 => DataType::Float32,
                ColumnType::Text => {
                    DataType::Dictionary(Box::new(DataType::Int32), Box::new(DataType::Utf8))
                }
            };
            // rank, contingency, solved and type are always there.
            let required = matches!(name, "rank" | "contingency" | "solved" | "type");
            Field::new(name, data_type, !required)
        })
        .collect();
    Schema::new_with_metadata(fields, metadata)
}

/// One batch being built.
struct Batch {
    rank: UInt32Builder,
    category: Dict,
    contingency: Dict,
    outage: Dict,
    description: Dict,
    solved: Dict,
    kind: Dict,
    element: Dict,
    element_id: Dict,
    from_bus: UInt32Builder,
    to_bus: UInt32Builder,
    bus: UInt32Builder,
    value_pct: Float32Builder,
    before_pct: Float32Builder,
    limit_pct: Float32Builder,
    mw: Float32Builder,
    before_mw: Float32Builder,
    vm_pu: Float32Builder,
    before_vm_pu: Float32Builder,
    vmin_pu: Float32Builder,
    vmax_pu: Float32Builder,
    note: Dict,
    len: usize,
}

/// The values of one finding past the outage's own columns.
#[derive(Default)]
struct Finding<'a> {
    kind: &'a str,
    element: Option<&'a str>,
    element_id: Option<&'a str>,
    from_bus: Option<u32>,
    to_bus: Option<u32>,
    bus: Option<u32>,
    value_pct: Option<f32>,
    before_pct: Option<f32>,
    limit_pct: Option<f32>,
    mw: Option<f32>,
    before_mw: Option<f32>,
    vm_pu: Option<f32>,
    before_vm_pu: Option<f32>,
    vmin_pu: Option<f32>,
    vmax_pu: Option<f32>,
    note: Option<&'a str>,
}

fn text(b: &mut Dict, v: Option<&str>) {
    match v {
        Some(v) => {
            b.append_value(v);
        }
        None => b.append_null(),
    }
}

/// Some for a non-empty string.
fn some(v: &str) -> Option<&str> {
    (!v.is_empty()).then_some(v)
}

impl Batch {
    fn new() -> Self {
        Self {
            rank: UInt32Builder::new(),
            category: Dict::new(),
            contingency: Dict::new(),
            outage: Dict::new(),
            description: Dict::new(),
            solved: Dict::new(),
            kind: Dict::new(),
            element: Dict::new(),
            element_id: Dict::new(),
            from_bus: UInt32Builder::new(),
            to_bus: UInt32Builder::new(),
            bus: UInt32Builder::new(),
            value_pct: Float32Builder::new(),
            before_pct: Float32Builder::new(),
            limit_pct: Float32Builder::new(),
            mw: Float32Builder::new(),
            before_mw: Float32Builder::new(),
            vm_pu: Float32Builder::new(),
            before_vm_pu: Float32Builder::new(),
            vmin_pu: Float32Builder::new(),
            vmax_pu: Float32Builder::new(),
            note: Dict::new(),
            len: 0,
        }
    }

    fn push(&mut self, row: &ContingencyRow, solved: &str, f: Finding) {
        self.rank.append_value(row.rank as u32);
        text(&mut self.category, row.category.as_deref());
        self.contingency.append_value(&row.name);
        let outage = if row.kind == "base" {
            None
        } else {
            some(&row.element)
        };
        text(&mut self.outage, outage);
        text(&mut self.description, row.description.as_deref());
        self.solved.append_value(solved);
        self.kind.append_value(f.kind);
        text(&mut self.element, f.element);
        text(&mut self.element_id, f.element_id);
        self.from_bus.append_option(f.from_bus);
        self.to_bus.append_option(f.to_bus);
        self.bus.append_option(f.bus);
        self.value_pct.append_option(f.value_pct);
        self.before_pct.append_option(f.before_pct);
        self.limit_pct.append_option(f.limit_pct);
        self.mw.append_option(f.mw);
        self.before_mw.append_option(f.before_mw);
        self.vm_pu.append_option(f.vm_pu);
        self.before_vm_pu.append_option(f.before_vm_pu);
        self.vmin_pu.append_option(f.vmin_pu);
        self.vmax_pu.append_option(f.vmax_pu);
        text(&mut self.note, f.note);
        self.len += 1;
    }

    fn finish(&mut self, schema: &Arc<Schema>) -> Result<RecordBatch, arrow_schema::ArrowError> {
        let columns: Vec<ArrayRef> = vec![
            Arc::new(self.rank.finish()),
            Arc::new(self.category.finish()),
            Arc::new(self.contingency.finish()),
            Arc::new(self.outage.finish()),
            Arc::new(self.description.finish()),
            Arc::new(self.solved.finish()),
            Arc::new(self.kind.finish()),
            Arc::new(self.element.finish()),
            Arc::new(self.element_id.finish()),
            Arc::new(self.from_bus.finish()),
            Arc::new(self.to_bus.finish()),
            Arc::new(self.bus.finish()),
            Arc::new(self.value_pct.finish()),
            Arc::new(self.before_pct.finish()),
            Arc::new(self.limit_pct.finish()),
            Arc::new(self.mw.finish()),
            Arc::new(self.before_mw.finish()),
            Arc::new(self.vm_pu.finish()),
            Arc::new(self.before_vm_pu.finish()),
            Arc::new(self.vmin_pu.finish()),
            Arc::new(self.vmax_pu.finish()),
            Arc::new(self.note.finish()),
        ];
        self.len = 0;
        RecordBatch::try_new(schema.clone(), columns)
    }
}

fn bus_name(name: &str, number: u32) -> String {
    let name = name.trim();
    if name.is_empty() {
        number.to_string()
    } else {
        format!("{name} {number}")
    }
}

/// Counts what goes through to the file, so its size is known at the end.
struct Counted<W> {
    inner: W,
    bytes: u64,
}

impl<W: Write> Write for Counted<W> {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let n = self.inner.write(buf)?;
        self.bytes += n as u64;
        Ok(n)
    }
    fn flush(&mut self) -> std::io::Result<()> {
        self.inner.flush()
    }
}

/// Write a contingency run as a `.stnr` stream. `case` names the case the
/// run was made on (kept in the metadata).
pub fn write_contingency_results(
    results: &ContingencyResults,
    case: &str,
    out: impl Write,
) -> Result<ResultsExport, String> {
    let err = |e: arrow_schema::ArrowError| e.to_string();
    let metadata = HashMap::from([
        ("stannum.format".to_string(), RESULTS_FORMAT.to_string()),
        ("stannum.kind".to_string(), "contingency".to_string()),
        ("stannum.case".to_string(), case.to_string()),
        (
            "stannum.summary".to_string(),
            serde_json::to_string(&results.summary).unwrap_or_default(),
        ),
        (
            "stannum.settings".to_string(),
            serde_json::to_string(results.settings()).unwrap_or_default(),
        ),
        (
            "stannum.created_by".to_string(),
            format!("stannum-core {}", env!("CARGO_PKG_VERSION")),
        ),
    ]);
    let schema = Arc::new(schema(metadata));
    let options = IpcWriteOptions::default()
        .try_with_compression(Some(CompressionType::ZSTD))
        .map_err(err)?;
    let mut writer = StreamWriter::try_new_with_options(
        Counted {
            inner: std::io::BufWriter::new(out),
            bytes: 0,
        },
        &schema,
        options,
    )
    .map_err(err)?;
    let mut written = ResultsExport {
        rows: 0,
        outages: 0,
        violations: 0,
        bytes: 0,
    };
    let mut batch = Batch::new();
    for (row, found) in results.rows.iter().zip(&results.violations) {
        let solved = match (row.checked, row.converged) {
            ("ac", Some(false)) => "ac_not_converged",
            ("ac", _) => "ac",
            _ => "linear",
        };
        written.outages += 1;
        let before = batch.len;
        if row.converged == Some(false) {
            batch.push(
                row,
                solved,
                Finding {
                    kind: "not_converged",
                    note: row.note.as_deref(),
                    ..Default::default()
                },
            );
        } else {
            for o in &found.overloads {
                batch.push(
                    row,
                    solved,
                    Finding {
                        kind: "branch",
                        element: Some(&o.name),
                        element_id: Some(&o.element),
                        from_bus: Some(o.from),
                        to_bus: Some(o.to),
                        value_pct: Some(o.pct as f32),
                        before_pct: Some(o.base_pct as f32),
                        limit_pct: Some(o.limit_pct as f32),
                        mw: Some(o.mw as f32),
                        before_mw: Some(o.base_mw as f32),
                        ..Default::default()
                    },
                );
            }
            for v in &found.voltage_problems {
                let name = bus_name(&v.name, v.bus);
                let id = format!("bus:{}", v.bus);
                batch.push(
                    row,
                    solved,
                    Finding {
                        kind: "voltage",
                        element: Some(&name),
                        element_id: Some(&id),
                        bus: Some(v.bus),
                        vm_pu: Some(v.vm as f32),
                        before_vm_pu: Some(v.base_vm as f32),
                        vmin_pu: Some(v.vmin as f32),
                        vmax_pu: Some(v.vmax as f32),
                        ..Default::default()
                    },
                );
            }
            for i in &found.interfaces {
                let id = format!("interface:{}", i.name);
                batch.push(
                    row,
                    solved,
                    Finding {
                        kind: "interface",
                        element: Some(&i.name),
                        element_id: Some(&id),
                        value_pct: i.pct.map(|v| v as f32),
                        before_pct: i.base_pct.map(|v| v as f32),
                        limit_pct: Some(i.limit_pct as f32),
                        mw: Some(i.mw as f32),
                        before_mw: Some(i.base_mw as f32),
                        ..Default::default()
                    },
                );
            }
        }
        let found_here = batch.len - before;
        if found_here == 0 {
            batch.push(
                row,
                solved,
                Finding {
                    kind: "ok",
                    note: row.note.as_deref(),
                    ..Default::default()
                },
            );
        } else {
            written.violations += found_here;
        }
        written.rows += batch.len - before;
        if batch.len >= BATCH_ROWS {
            writer
                .write(&batch.finish(&schema).map_err(err)?)
                .map_err(err)?;
        }
    }
    if batch.len > 0 || written.rows == 0 {
        writer
            .write(&batch.finish(&schema).map_err(err)?)
            .map_err(err)?;
    }
    writer.finish().map_err(err)?;
    let mut inner = writer.into_inner().map_err(err)?;
    inner.flush().map_err(|e| e.to_string())?;
    written.bytes = inner.bytes;
    Ok(written)
}
