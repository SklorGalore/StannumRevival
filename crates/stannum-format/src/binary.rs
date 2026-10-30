//! The binary form (`.stnb`): the same tables as the text, as columns.
//!
//! A `.stnb` file is a ZIP archive whose entries are stored, not
//! compressed, holding `manifest.json` and one Arrow IPC file per table:
//! `tables/<name>.arrow` for the schema's tables and `extensions/<name>.arrow`
//! for others. Each Arrow file has an `id` column, one column per schema
//! field with defaults filled in (null only where a field has no default
//! and is absent), a `_comment` column, and a text column for each `x.`
//! extension field. Rows are in canonical order. Arrow's own buffer
//! compression (Zstandard) is on unless the writer turns it off.
//!
//! Column types: `int` as int64, `float` as float64, `bool` and `status` as
//! bool (`on` is true), `enum` as a dictionary of strings, `string`, `ref`
//! and `element` as strings, and lists as Arrow lists: of the item type, or
//! of a struct with fields `0`, `1`, ... for tuples. Extension fields and
//! the fields of extension tables hold their value as it is written in a
//! text file.

use crate::document::{Document, Record, Table};
use crate::schema::{FieldDef, FieldType, ItemType, Schema, TableDef, Version};
use crate::text::{canonical_tables, check, check_value, kept_table, untyped_from_text};
use crate::value::Value;
use arrow_array::cast::AsArray;
use arrow_array::types::{
    Float32Type, Float64Type, Int8Type, Int16Type, Int32Type, Int64Type, UInt8Type, UInt16Type,
    UInt32Type, UInt64Type,
};
use arrow_array::{
    Array, ArrayRef, BooleanArray, DictionaryArray, Float64Array, Int64Array, ListArray,
    RecordBatch, StringArray, StructArray,
};
use arrow_buffer::{NullBuffer, OffsetBuffer};
use arrow_ipc::CompressionType;
use arrow_ipc::reader::FileReader;
use arrow_ipc::writer::{FileWriter, IpcWriteOptions};
use arrow_schema::{DataType, Field, Fields, Schema as ArrowSchema};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use std::io::{Cursor, Read, Write};
use std::sync::Arc;

/// The archive entry that describes the rest.
pub const MANIFEST: &str = "manifest.json";

/// How to write a `.stnb` file.
#[derive(Clone, Debug)]
pub struct BinaryOptions {
    /// Compress Arrow buffers with Zstandard. Off gives larger files that
    /// can be mapped from disk and read without decompressing.
    pub compress: bool,
    /// The program writing the file, kept in the manifest (`app`).
    pub app: Option<String>,
}

impl Default for BinaryOptions {
    fn default() -> Self {
        BinaryOptions {
            compress: true,
            app: None,
        }
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Manifest {
    pub format: String,
    pub version: String,
    /// SHA-256 of the schema file the writer used, for information.
    pub schema_sha256: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub app: Option<String>,
    /// Comment lines at the top of the file, as in text.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub preamble: Vec<String>,
    pub tables: Vec<ManifestTable>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct ManifestTable {
    pub name: String,
    pub path: String,
    pub rows: usize,
}

/// Whether bytes look like a `.stnb` file: a ZIP archive that starts with
/// its manifest, as writers put it.
pub fn is_binary(bytes: &[u8]) -> bool {
    bytes.starts_with(b"PK\x03\x04")
        && bytes.get(30..30 + MANIFEST.len()) == Some(MANIFEST.as_bytes())
}

fn schema_sha256(schema: &Schema) -> String {
    let text = if schema.format == "stannum-case" {
        include_str!("../schema/stannum-case.toml")
    } else {
        include_str!("../schema/stannum-lists.toml")
    };
    Sha256::digest(text.as_bytes())
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

// ---------------------------------------------------------------------------
// Types.

fn item_data_type(item: &ItemType) -> DataType {
    match item {
        ItemType::Int => DataType::Int64,
        ItemType::Float => DataType::Float64,
        ItemType::Status => DataType::Boolean,
        ItemType::String | ItemType::Enum(_) | ItemType::Ref { .. } | ItemType::Element => {
            DataType::Utf8
        }
    }
}

fn tuple_fields(items: &[ItemType], min: usize) -> Fields {
    items
        .iter()
        .enumerate()
        .map(|(i, it)| Field::new(i.to_string(), item_data_type(it), i >= min))
        .collect()
}

fn data_type(ty: &FieldType) -> DataType {
    match ty {
        FieldType::Int => DataType::Int64,
        FieldType::Float => DataType::Float64,
        FieldType::Bool | FieldType::Status => DataType::Boolean,
        FieldType::String | FieldType::Ref { .. } | FieldType::Element => DataType::Utf8,
        FieldType::Enum(_) => {
            DataType::Dictionary(Box::new(DataType::Int32), Box::new(DataType::Utf8))
        }
        FieldType::List { items, min } => {
            let item = if items.len() == 1 {
                Field::new("item", item_data_type(&items[0]), false)
            } else {
                Field::new("item", DataType::Struct(tuple_fields(items, *min)), false)
            };
            DataType::List(Arc::new(item))
        }
    }
}

// ---------------------------------------------------------------------------
// Writing.

/// Scalars of one item type, as an Arrow array.
fn scalar_array(values: &[Option<&Value>], ty: &DataType) -> ArrayRef {
    match ty {
        DataType::Int64 => Arc::new(Int64Array::from_iter(
            values.iter().map(|v| v.and_then(|v| v.as_i64())),
        )),
        DataType::Float64 => Arc::new(Float64Array::from_iter(
            values.iter().map(|v| v.and_then(|v| v.as_f64())),
        )),
        DataType::Boolean => Arc::new(BooleanArray::from_iter(values.iter().map(|v| {
            v.and_then(|v| match v {
                Value::Bool(b) => Some(*b),
                other => other.as_str().map(|s| s == "on"),
            })
        }))),
        DataType::Dictionary(_, _) => Arc::new(DictionaryArray::<Int32Type>::from_iter(
            values.iter().map(|v| v.and_then(|v| v.as_str())),
        )),
        _ => Arc::new(StringArray::from_iter(
            values.iter().map(|v| v.and_then(|v| v.as_str())),
        )),
    }
}

fn column(values: &[Option<&Value>], f: &FieldDef) -> Result<ArrayRef, String> {
    let FieldType::List { items, min } = &f.ty else {
        return Ok(scalar_array(values, &data_type(&f.ty)));
    };
    let mut offsets = vec![0i32];
    let mut flat: Vec<&Value> = Vec::new();
    for v in values {
        if let Some(list) = v.and_then(|v| v.as_list()) {
            flat.extend(list);
        }
        offsets.push(flat.len() as i32);
    }
    let nulls = NullBuffer::from_iter(values.iter().map(|v| v.is_some()));
    let (item_field, child): (Field, ArrayRef) = if items.len() == 1 {
        let ty = item_data_type(&items[0]);
        let flat: Vec<Option<&Value>> = flat.into_iter().map(Some).collect();
        (
            Field::new("item", ty.clone(), false),
            scalar_array(&flat, &ty),
        )
    } else {
        let fields = tuple_fields(items, *min);
        let children: Vec<ArrayRef> = items
            .iter()
            .enumerate()
            .map(|(i, it)| {
                let parts: Vec<Option<&Value>> = flat
                    .iter()
                    .map(|t| t.as_list().and_then(|t| t.get(i)))
                    .collect();
                scalar_array(&parts, &item_data_type(it))
            })
            .collect();
        let child = StructArray::try_new(fields.clone(), children, None)
            .map_err(|e| format!("{}: {e}", f.name))?;
        (
            Field::new("item", DataType::Struct(fields), false),
            Arc::new(child),
        )
    };
    let list = ListArray::try_new(
        Arc::new(item_field),
        OffsetBuffer::new(offsets.into()),
        child,
        (nulls.null_count() > 0).then_some(nulls),
    )
    .map_err(|e| format!("{}: {e}", f.name))?;
    Ok(Arc::new(list))
}

fn text_column(values: &[Option<String>]) -> ArrayRef {
    Arc::new(StringArray::from_iter(values.iter().map(|v| v.as_deref())))
}

/// One table as an Arrow record batch.
fn batch(doc: &Document, table: &Table, records: &[&Record]) -> Result<RecordBatch, String> {
    let def = doc.schema.table(&table.name);
    let mut fields = vec![Field::new("id", DataType::Utf8, false)];
    let ids: Vec<Option<String>> = records.iter().map(|r| Some(r.id.clone())).collect();
    let mut columns = vec![text_column(&ids)];
    let known = |name: &str| def.is_some_and(|d| d.field(name).is_some());
    if let Some(def) = def {
        for f in &def.fields {
            let values: Vec<Option<&Value>> = records
                .iter()
                .map(|r| r.get(&f.name).or(f.default.as_ref()))
                .collect();
            if let Some(nan) = values.iter().position(|v| v.is_some_and(has_nan)) {
                return Err(format!(
                    "{} {}: {} is NaN",
                    table.name, records[nan].id, f.name
                ));
            }
            let nullable = values.iter().any(Option::is_none);
            fields.push(Field::new(&f.name, data_type(&f.ty), nullable));
            columns.push(column(&values, f).map_err(|e| format!("{} {}", table.name, e))?);
        }
    }
    let comments: Vec<Option<String>> = records
        .iter()
        .map(|r| (!r.comments.is_empty()).then(|| r.comments.join("\n")))
        .collect();
    fields.push(Field::new("_comment", DataType::Utf8, true));
    columns.push(text_column(&comments));
    // Fields the schema doesn't know, by name, as text.
    let other: BTreeSet<&str> = records
        .iter()
        .flat_map(|r| r.fields.iter().map(|(n, _)| n.as_str()))
        .filter(|n| !known(n))
        .collect();
    for name in other {
        let values: Vec<Option<String>> = records
            .iter()
            .map(|r| r.get(name).map(Value::to_text))
            .collect();
        fields.push(Field::new(name, DataType::Utf8, true));
        columns.push(text_column(&values));
    }
    RecordBatch::try_new(Arc::new(ArrowSchema::new(fields)), columns).map_err(|e| e.to_string())
}

fn has_nan(v: &Value) -> bool {
    match v {
        Value::Float(f) => f.is_nan(),
        Value::List(l) => l.iter().any(has_nan),
        _ => false,
    }
}

/// Write a document as a `.stnb` file.
pub fn write_binary(doc: &Document, options: &BinaryOptions) -> Result<Vec<u8>, String> {
    let schema = doc.schema;
    let err = |e: &dyn std::fmt::Display| e.to_string();
    let mut ipc = IpcWriteOptions::default();
    if options.compress {
        ipc = ipc
            .try_with_compression(Some(CompressionType::ZSTD))
            .map_err(|e| err(&e))?;
    }
    let mut entries = Vec::new();
    for (table, records) in canonical_tables(doc) {
        let batch = batch(doc, table, &records)?;
        let mut buf = Vec::new();
        {
            let mut w = FileWriter::try_new_with_options(&mut buf, &batch.schema(), ipc.clone())
                .map_err(|e| err(&e))?;
            w.write(&batch).map_err(|e| err(&e))?;
            w.finish().map_err(|e| err(&e))?;
        }
        let folder = if schema.table(&table.name).is_some() {
            "tables"
        } else {
            "extensions"
        };
        entries.push((
            ManifestTable {
                name: table.name.clone(),
                path: format!("{folder}/{}.arrow", table.name),
                rows: records.len(),
            },
            buf,
        ));
    }
    let manifest = Manifest {
        format: schema.format.clone(),
        version: schema.version.to_string(),
        schema_sha256: schema_sha256(schema),
        app: options.app.clone(),
        preamble: doc.preamble.clone(),
        tables: entries
            .iter()
            .map(|(t, _)| ManifestTable {
                name: t.name.clone(),
                path: t.path.clone(),
                rows: t.rows,
            })
            .collect(),
    };
    let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
    // Stored, and aligned so a reader can map each Arrow file in place.
    let stored = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Stored)
        .last_modified_time(zip::DateTime::default());
    zip.start_file(MANIFEST, stored).map_err(|e| err(&e))?;
    let json = serde_json::to_string_pretty(&manifest).map_err(|e| err(&e))?;
    zip.write_all(json.as_bytes()).map_err(|e| err(&e))?;
    for (t, buf) in &entries {
        zip.start_file(&t.path, stored.with_alignment(64))
            .map_err(|e| err(&e))?;
        zip.write_all(buf).map_err(|e| err(&e))?;
    }
    Ok(zip.finish().map_err(|e| err(&e))?.into_inner())
}

// ---------------------------------------------------------------------------
// Reading.

/// Every row of an Arrow column as a value, typed by the array alone:
/// integers, floats, bools, strings, lists, and structs as tuples (trailing
/// nulls left off).
fn values_of(array: &dyn Array) -> Result<Vec<Option<Value>>, String> {
    macro_rules! ints {
        ($t:ty) => {
            array
                .as_primitive::<$t>()
                .iter()
                .map(|v| v.map(|v| Value::Int(v as i64)))
                .collect()
        };
    }
    Ok(match array.data_type() {
        DataType::Int8 => ints!(Int8Type),
        DataType::Int16 => ints!(Int16Type),
        DataType::Int32 => ints!(Int32Type),
        DataType::Int64 => ints!(Int64Type),
        DataType::UInt8 => ints!(UInt8Type),
        DataType::UInt16 => ints!(UInt16Type),
        DataType::UInt32 => ints!(UInt32Type),
        DataType::UInt64 => array
            .as_primitive::<UInt64Type>()
            .iter()
            .map(|v| {
                v.map(|v| {
                    i64::try_from(v)
                        .map(Value::Int)
                        .map_err(|_| "an integer is too large".to_string())
                })
                .transpose()
            })
            .collect::<Result<_, _>>()?,
        DataType::Float32 => array
            .as_primitive::<Float32Type>()
            .iter()
            .map(|v| v.map(|v| Value::Float(v as f64)))
            .collect(),
        DataType::Float64 => array
            .as_primitive::<Float64Type>()
            .iter()
            .map(|v| v.map(|v| Value::Float(v + 0.0)))
            .collect(),
        DataType::Boolean => array
            .as_boolean()
            .iter()
            .map(|v| v.map(Value::Bool))
            .collect(),
        DataType::Utf8 => array
            .as_string::<i32>()
            .iter()
            .map(|v| v.map(|s| Value::Str(s.to_string())))
            .collect(),
        DataType::LargeUtf8 => array
            .as_string::<i64>()
            .iter()
            .map(|v| v.map(|s| Value::Str(s.to_string())))
            .collect(),
        DataType::Utf8View => array
            .as_string_view()
            .iter()
            .map(|v| v.map(|s| Value::Str(s.to_string())))
            .collect(),
        DataType::Dictionary(_, _) => {
            let dict = array.as_any_dictionary();
            let words = values_of(dict.values().as_ref())?;
            let keys = dict.normalized_keys();
            (0..array.len())
                .map(|i| {
                    if array.is_null(i) {
                        None
                    } else {
                        words.get(keys[i]).cloned().flatten()
                    }
                })
                .collect()
        }
        DataType::List(_) | DataType::LargeList(_) => {
            let (items, offsets): (Vec<Option<Value>>, Vec<usize>) = match array.data_type() {
                DataType::List(_) => {
                    let l = array.as_list::<i32>();
                    (
                        values_of(l.values().as_ref())?,
                        l.value_offsets().iter().map(|&o| o as usize).collect(),
                    )
                }
                _ => {
                    let l = array.as_list::<i64>();
                    (
                        values_of(l.values().as_ref())?,
                        l.value_offsets().iter().map(|&o| o as usize).collect(),
                    )
                }
            };
            (0..array.len())
                .map(|i| {
                    if array.is_null(i) {
                        return Ok(None);
                    }
                    items[offsets[i]..offsets[i + 1]]
                        .iter()
                        .map(|v| v.clone().ok_or_else(|| "a list holds a null".to_string()))
                        .collect::<Result<Vec<_>, _>>()
                        .map(|l| Some(Value::List(l)))
                })
                .collect::<Result<_, String>>()?
        }
        DataType::Struct(_) => {
            let s = array.as_struct();
            let children: Vec<Vec<Option<Value>>> = s
                .columns()
                .iter()
                .map(|c| values_of(c.as_ref()))
                .collect::<Result<_, _>>()?;
            (0..array.len())
                .map(|i| {
                    if array.is_null(i) {
                        return Ok(None);
                    }
                    let mut tuple: Vec<Option<Value>> =
                        children.iter().map(|c| c[i].clone()).collect();
                    while tuple.last().is_some_and(Option::is_none) {
                        tuple.pop();
                    }
                    tuple
                        .into_iter()
                        .map(|v| v.ok_or_else(|| "a tuple has a gap".to_string()))
                        .collect::<Result<Vec<_>, _>>()
                        .map(|t| Some(Value::List(t)))
                })
                .collect::<Result<_, String>>()?
        }
        other => return Err(format!("columns of type {other} aren't read")),
    })
}

/// A value from a column, as the schema types it: the shapes the text
/// reader gives (status as `on`/`off`, ints in float fields as floats).
fn coerce(v: Value, ty: &FieldType) -> Result<Value, String> {
    let item = |v: Value, it: &ItemType| -> Result<Value, String> {
        let ty = match it {
            ItemType::Int => FieldType::Int,
            ItemType::Float => FieldType::Float,
            ItemType::Status => FieldType::Status,
            _ => FieldType::String,
        };
        coerce(v, &ty)
    };
    Ok(match (ty, v) {
        (FieldType::Int, v @ Value::Int(_)) => v,
        (FieldType::Float, Value::Float(f)) => Value::Float(f),
        (FieldType::Float, Value::Int(i)) => Value::Float(i as f64),
        (FieldType::Bool, v @ Value::Bool(_)) => v,
        (FieldType::Status, Value::Bool(b)) => Value::Str(if b { "on" } else { "off" }.into()),
        (FieldType::Status, v @ Value::Str(_)) => v,
        (
            FieldType::String | FieldType::Ref { .. } | FieldType::Element | FieldType::Enum(_),
            v @ Value::Str(_),
        ) => v,
        (FieldType::List { items, .. }, Value::List(list)) => Value::List(
            list.into_iter()
                .map(|v| match (items.len(), v) {
                    (1, v) => item(v, &items[0]),
                    (_, Value::List(t)) => Ok(Value::List(
                        t.into_iter()
                            .zip(items)
                            .map(|(p, it)| item(p, it))
                            .collect::<Result<_, _>>()?,
                    )),
                    _ => Err("expected a tuple".to_string()),
                })
                .collect::<Result<_, _>>()?,
        ),
        (ty, v) => {
            return Err(format!(
                "a {} where the schema has {}",
                kind(&v),
                type_name(ty)
            ));
        }
    })
}

fn kind(v: &Value) -> &'static str {
    match v {
        Value::Int(_) => "whole number",
        Value::Float(_) => "number",
        Value::Bool(_) => "true/false",
        Value::Str(_) | Value::Raw(_) => "string",
        Value::List(_) => "list",
    }
}

fn type_name(ty: &FieldType) -> &'static str {
    match ty {
        FieldType::Int => "a whole number",
        FieldType::Float => "a number",
        FieldType::Bool => "true/false",
        FieldType::Status => "on/off",
        FieldType::List { .. } => "a list",
        _ => "a string",
    }
}

/// Why a `.stnb` file couldn't be read: every problem found, as
/// `table row N: ...` lines.
#[derive(Clone, Debug, PartialEq)]
pub struct BinaryError {
    pub problems: Vec<String>,
}

impl std::fmt::Display for BinaryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.problems.join("\n"))
    }
}

impl std::error::Error for BinaryError {}

fn one(problem: impl Into<String>) -> BinaryError {
    BinaryError {
        problems: vec![problem.into()],
    }
}

/// The manifest of a `.stnb` file.
pub fn read_manifest(bytes: &[u8]) -> Result<Manifest, BinaryError> {
    let mut zip = zip::ZipArchive::new(Cursor::new(bytes))
        .map_err(|e| one(format!("not a .stnb file: {e}")))?;
    read_manifest_in(&mut zip)
}

fn read_manifest_in(zip: &mut zip::ZipArchive<Cursor<&[u8]>>) -> Result<Manifest, BinaryError> {
    let mut text = String::new();
    zip.by_name(MANIFEST)
        .map_err(|_| one("not a .stnb file: it has no manifest.json"))?
        .read_to_string(&mut text)
        .map_err(|e| one(format!("manifest.json: {e}")))?;
    serde_json::from_str(&text).map_err(|e| one(format!("manifest.json: {e}")))
}

/// Read a `.stnb` file against `schema`, with the same checks as text.
pub fn read_binary(bytes: &[u8], schema: &'static Schema) -> Result<Document, BinaryError> {
    let mut zip = zip::ZipArchive::new(Cursor::new(bytes))
        .map_err(|e| one(format!("not a .stnb file: {e}")))?;
    let manifest = read_manifest_in(&mut zip)?;
    if manifest.format != schema.format {
        return Err(one(format!(
            "this file is {}, not {}",
            manifest.format, schema.format
        )));
    }
    let version = Version::parse(&manifest.version)
        .ok_or_else(|| one(format!("manifest.json: bad version `{}`", manifest.version)))?;
    let ours = schema.version;
    if version.major > ours.major || (version.major == 0 && version > ours) {
        return Err(one(format!(
            "this file is {} {version}, newer than the {ours} this Stannum reads; open it in a newer Stannum",
            schema.format
        )));
    }
    // Unknown names are mistakes unless the file is newer (or extensions).
    let strict = version.major == ours.major && version.minor <= ours.minor;
    let mut doc = Document::new(schema);
    doc.version = version;
    doc.preamble = manifest.preamble.clone();
    let mut problems = Vec::new();
    // Every entry's bytes first; then the tables are decoded side by side.
    let mut entries = Vec::new();
    for t in &manifest.tables {
        let def = schema.table(&t.name);
        if def.is_none() && strict && !kept_table(&t.name) {
            problems.push(format!("unknown table `{}`", t.name));
            continue;
        }
        let mut buf = Vec::new();
        match zip.by_name(&t.path) {
            Ok(mut f) => {
                if let Err(e) = f.read_to_end(&mut buf) {
                    problems.push(format!("{}: {e}", t.path));
                    continue;
                }
            }
            Err(_) => {
                problems.push(format!(
                    "the manifest lists {} but the file has no such entry",
                    t.path
                ));
                continue;
            }
        }
        entries.push((t, def, buf));
    }
    let decoded: Vec<Result<Vec<Record>, Vec<String>>> = std::thread::scope(|s| {
        let tasks: Vec<_> = entries
            .iter()
            .map(|(t, def, buf)| {
                // Small tables aren't worth a thread.
                if buf.len() < 16 * 1024 {
                    Err(read_table(buf, &t.name, *def, strict))
                } else {
                    Ok(s.spawn(move || read_table(buf, &t.name, *def, strict)))
                }
            })
            .collect();
        tasks
            .into_iter()
            .map(|task| match task {
                Err(done) => done,
                Ok(handle) => handle
                    .join()
                    .unwrap_or_else(|e| std::panic::resume_unwind(e)),
            })
            .collect()
    });
    // Rows get line numbers in file order, so the shared checks can say
    // which row they mean: (table name, row) by line.
    let mut rows: Vec<(&str, usize)> = vec![("", 0)];
    for ((t, _, _), result) in entries.iter().zip(decoded) {
        match result {
            Ok(mut records) => {
                for (row, r) in records.iter_mut().enumerate() {
                    r.line = rows.len();
                    rows.push((&t.name, row + 1));
                }
                if records.len() != t.rows {
                    problems.push(format!(
                        "{}: the manifest says {} rows, the table has {}",
                        t.name,
                        t.rows,
                        records.len()
                    ));
                }
                doc.tables.push(Table {
                    name: t.name.clone(),
                    records,
                });
            }
            Err(mut p) => problems.append(&mut p),
        }
    }
    if problems.is_empty() {
        let mut issues = Vec::new();
        check(&doc, &mut issues);
        for i in issues {
            let (table, row) = &rows[i.line.min(rows.len() - 1)];
            let message = i.message.replace(" on line ", " on row ");
            problems.push(format!("{table} row {row}: {message}"));
        }
    }
    // Same-id messages name a line; give them the row in that table.
    let fix = |p: String| -> String {
        if let Some((head, n)) = p.rsplit_once(" on row ")
            && let Ok(n) = n.parse::<usize>()
            && let Some((_, row)) = rows.get(n)
        {
            return format!("{head} on row {row}");
        }
        p
    };
    let problems: Vec<String> = problems.into_iter().map(fix).collect();
    if problems.is_empty() {
        Ok(doc)
    } else {
        Err(BinaryError {
            problems: problems.into_iter().take(crate::text::MAX_ISSUES).collect(),
        })
    }
}

/// Rows made into records on one thread at a time.
const ROWS: usize = 4096;

enum Col<'a> {
    Field(&'a FieldDef),
    Comment,
    Text,
    Skip,
}

/// Records from rows `first_row..` of a table: `cols` holds those rows of
/// every column.
// Rows index into every column at once.
#[allow(clippy::needless_range_loop)]
fn build_rows(
    name: &str,
    kinds: &[Col],
    names: &[&str],
    id_col: usize,
    mut cols: Vec<&mut [Option<Value>]>,
    first_row: usize,
) -> (Vec<Record>, Vec<String>) {
    let n = cols.first().map_or(0, |c| c.len());
    let mut records = Vec::with_capacity(n);
    let mut problems = Vec::new();
    for i in 0..n {
        let row = first_row + i;
        let at = |c: &str| format!("{name} row {}: {c}", row + 1);
        let id = match &cols[id_col][i] {
            Some(Value::Str(s)) if !s.is_empty() => s.clone(),
            _ => {
                problems.push(at("no id"));
                String::new()
            }
        };
        let mut r = Record::new(id);
        for (c, kind) in kinds.iter().enumerate() {
            let Some(v) = cols[c][i].take() else {
                continue;
            };
            match kind {
                Col::Skip => {}
                Col::Comment => {
                    if let Value::Str(s) = v {
                        r.comments = s.split('\n').map(str::to_string).collect();
                    }
                }
                // Defaults are filled in on disk; a record holds what differs,
                // as one read from text does.
                Col::Field(f) if f.default.as_ref() == Some(&v) => {}
                Col::Field(f) => {
                    match coerce(v, &f.ty).and_then(|v| check_value(&v, &f.ty).map(|_| v)) {
                        Ok(v) if f.default.as_ref() == Some(&v) => {}
                        Ok(v) => r.fields.push((f.name.clone(), v)),
                        Err(e) => problems.push(at(&format!("{}: {e}", f.name))),
                    }
                }
                Col::Text => match v.as_str().and_then(untyped_from_text) {
                    Some(v) => r.fields.push((names[c].to_string(), v)),
                    None => problems.push(at(&format!("{}: not a value as text", names[c]))),
                },
            }
        }
        records.push(r);
    }
    (records, problems)
}

fn read_table(
    buf: &[u8],
    name: &str,
    def: Option<&TableDef>,
    strict: bool,
) -> Result<Vec<Record>, Vec<String>> {
    let reader =
        FileReader::try_new(Cursor::new(buf), None).map_err(|e| vec![format!("{name}: {e}")])?;
    let arrow_schema = reader.schema();
    let mut columns: Vec<Vec<Option<Value>>> =
        arrow_schema.fields().iter().map(|_| Vec::new()).collect();
    for batch in reader {
        let batch = batch.map_err(|e| vec![format!("{name}: {e}")])?;
        // Large tables' columns are converted side by side.
        let threads = batch.num_rows() >= ROWS;
        let converted: Vec<Result<Vec<Option<Value>>, String>> = std::thread::scope(|s| {
            let tasks: Vec<_> = batch
                .columns()
                .iter()
                .map(|c| {
                    if threads {
                        Ok(s.spawn(move || values_of(c.as_ref())))
                    } else {
                        Err(values_of(c.as_ref()))
                    }
                })
                .collect();
            tasks
                .into_iter()
                .map(|t| match t {
                    Err(done) => done,
                    Ok(h) => h.join().unwrap_or_else(|e| std::panic::resume_unwind(e)),
                })
                .collect()
        });
        for (i, v) in converted.into_iter().enumerate() {
            let mut v =
                v.map_err(|e| vec![format!("{name}.{}: {e}", arrow_schema.field(i).name())])?;
            columns[i].append(&mut v);
        }
    }
    let names: Vec<&str> = arrow_schema
        .fields()
        .iter()
        .map(|f| f.name().as_str())
        .collect();
    let Some(id_col) = names.iter().position(|n| *n == "id") else {
        return Err(vec![format!("{name}: no `id` column")]);
    };
    let n = columns[id_col].len();
    let mut problems = Vec::new();
    let kinds: Vec<Col> = names
        .iter()
        .enumerate()
        .map(|(i, c)| match def.and_then(|d| d.field(c)) {
            _ if i == id_col => Col::Skip,
            Some(f) => Col::Field(f),
            None if *c == "_comment" => Col::Comment,
            None => {
                if def.is_some() && strict && !c.starts_with("x.") {
                    problems.push(format!("`{name}` has no field `{c}`"));
                }
                Col::Text
            }
        })
        .collect();
    if !problems.is_empty() {
        return Err(problems);
    }
    if columns.iter().any(|c| c.len() != n) {
        return Err(vec![format!("{name}: columns of different lengths")]);
    }
    // Rows are made into records in chunks, side by side.
    let mut chunks: Vec<std::slice::ChunksMut<Option<Value>>> =
        columns.iter_mut().map(|c| c.chunks_mut(ROWS)).collect();
    let parts: Vec<Vec<&mut [Option<Value>]>> = (0..n.div_ceil(ROWS))
        .map(|_| chunks.iter_mut().filter_map(Iterator::next).collect())
        .collect();
    let built: Vec<(Vec<Record>, Vec<String>)> = std::thread::scope(|s| {
        let kinds = &kinds;
        let names = &names;
        let tasks: Vec<_> = parts
            .into_iter()
            .enumerate()
            .map(|(k, cols)| {
                s.spawn(move || build_rows(name, kinds, names, id_col, cols, k * ROWS))
            })
            .collect();
        tasks
            .into_iter()
            .map(|t| t.join().unwrap_or_else(|e| std::panic::resume_unwind(e)))
            .collect()
    });
    let mut records = Vec::with_capacity(n);
    for (mut part, mut p) in built {
        records.append(&mut part);
        problems.append(&mut p);
    }
    if problems.is_empty() {
        Ok(records)
    } else {
        Err(problems)
    }
}
