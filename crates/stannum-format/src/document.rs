//! A file's contents: tables of records.

use crate::schema::{FieldType, Schema, TableDef, Version};
use crate::value::Value;

/// One record: its id, fields in the order they were read or set, and the
/// comment lines just above it.
#[derive(Clone, Debug, PartialEq)]
pub struct Record {
    pub id: String,
    pub fields: Vec<(String, Value)>,
    pub comments: Vec<String>,
    /// One-based line it was read from; 0 when made in code.
    pub line: usize,
}

impl Record {
    pub fn new(id: impl Into<String>) -> Record {
        Record {
            id: id.into(),
            fields: Vec::new(),
            comments: Vec::new(),
            line: 0,
        }
    }

    pub fn get(&self, name: &str) -> Option<&Value> {
        self.fields.iter().find(|(n, _)| n == name).map(|(_, v)| v)
    }

    pub fn set(&mut self, name: &str, value: Value) {
        match self.fields.iter_mut().find(|(n, _)| n == name) {
            Some((_, v)) => *v = value,
            None => self.fields.push((name.to_string(), value)),
        }
    }

    /// Set a field, builder style.
    pub fn with(mut self, name: &str, value: Value) -> Record {
        self.set(name, value);
        self
    }

    pub fn remove(&mut self, name: &str) -> Option<Value> {
        let i = self.fields.iter().position(|(n, _)| n == name)?;
        Some(self.fields.remove(i).1)
    }

    pub fn float(&self, name: &str) -> Option<f64> {
        self.get(name).and_then(Value::as_f64)
    }

    pub fn int(&self, name: &str) -> Option<i64> {
        self.get(name).and_then(Value::as_i64)
    }

    pub fn str(&self, name: &str) -> Option<&str> {
        self.get(name).and_then(Value::as_str)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Table {
    pub name: String,
    pub records: Vec<Record>,
}

/// A case or lists file.
#[derive(Clone, Debug)]
pub struct Document {
    pub schema: &'static Schema,
    /// The version the file was written in. A newer minor version than the
    /// schema's (same major) reads, keeping what this build doesn't know.
    pub version: Version,
    /// Comment lines at the top of the file, after the header.
    pub preamble: Vec<String>,
    /// Tables in the order they were read or made.
    pub tables: Vec<Table>,
}

impl Document {
    pub fn new(schema: &'static Schema) -> Document {
        Document {
            schema,
            version: schema.version,
            preamble: Vec::new(),
            tables: Vec::new(),
        }
    }

    /// The version a writer puts in the header: the schema's, or the file's
    /// when it is a newer minor version whose extra fields were kept.
    pub fn written_version(&self) -> Version {
        let ours = self.schema.version;
        if self.version.major == ours.major && self.version > ours {
            self.version
        } else {
            ours
        }
    }

    pub fn table(&self, name: &str) -> Option<&Table> {
        self.tables.iter().find(|t| t.name == name)
    }

    /// A table's records; empty when the file has none.
    pub fn records(&self, name: &str) -> &[Record] {
        self.table(name).map_or(&[], |t| t.records.as_slice())
    }

    pub fn table_mut(&mut self, name: &str) -> &mut Table {
        let i = match self.tables.iter().position(|t| t.name == name) {
            Some(i) => i,
            None => {
                self.tables.push(Table {
                    name: name.to_string(),
                    records: Vec::new(),
                });
                self.tables.len() - 1
            }
        };
        &mut self.tables[i]
    }

    pub fn push(&mut self, table: &str, record: Record) {
        self.table_mut(table).records.push(record);
    }

    /// A field's value, or its default when the record leaves it out.
    pub fn value<'a>(&'a self, table: &str, record: &'a Record, field: &str) -> Option<&'a Value> {
        record.get(field).or_else(|| {
            self.schema
                .table(table)
                .and_then(|t| t.field(field))
                .and_then(|f| f.default.as_ref())
        })
    }

    /// The id a new record of `table` gets from its fields (the schema's id
    /// template), or None when a field the template needs is missing.
    pub fn default_id(&self, table: &str, record: &Record) -> Option<String> {
        let def = self.schema.table(table)?;
        make_id(def, |name| {
            self.value(table, record, name).map(|v| match v {
                Value::Str(s) | Value::Raw(s) => s.clone(),
                other => other.to_text(),
            })
        })
    }
}

/// Fill an id template: `{field}` and `{sort(a,b,...)}` parts, the latter
/// written in increasing (natural) order and joined by `:`.
pub(crate) fn make_id(def: &TableDef, get: impl Fn(&str) -> Option<String>) -> Option<String> {
    let mut out = String::new();
    let mut rest = def.id.as_str();
    while let Some(open) = rest.find('{') {
        out.push_str(&rest[..open]);
        let close = rest[open..].find('}')? + open;
        let inner = &rest[open + 1..close];
        if let Some(args) = inner
            .strip_prefix("sort(")
            .and_then(|s| s.strip_suffix(')'))
        {
            let mut parts: Vec<Value> = args
                .split(',')
                .map(|a| get(a.trim()).map(Value::Str))
                .collect::<Option<_>>()?;
            parts.sort_by(|a, b| a.sort_cmp(b));
            let parts: Vec<&str> = parts.iter().filter_map(Value::as_str).collect();
            out.push_str(&parts.join(":"));
        } else {
            out.push_str(&get(inner)?);
        }
        rest = &rest[close + 1..];
    }
    out.push_str(rest);
    Some(out)
}

/// Whether a field holds references, for checking them.
pub(crate) fn ref_target(ty: &FieldType) -> Option<(&str, bool)> {
    match ty {
        FieldType::Ref { to, soft } => Some((to, *soft)),
        _ => None,
    }
}
