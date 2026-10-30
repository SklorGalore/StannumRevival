//! The schema: tables, fields, types, units and defaults, read from TOML.

use crate::value::Value;
use serde::Deserialize;
use std::collections::HashMap;
use std::hash::{BuildHasherDefault, Hasher};

/// FNV-1a: names are short, and opening a case looks up fields a million
/// times, where the default hasher's resistance to crafted keys costs more
/// than it buys (the keys come from the schema).
#[derive(Default)]
pub(crate) struct Fnv(u64);

impl Hasher for Fnv {
    fn finish(&self) -> u64 {
        self.0
    }
    fn write(&mut self, bytes: &[u8]) {
        let mut h = if self.0 == 0 {
            0xcbf29ce484222325
        } else {
            self.0
        };
        for b in bytes {
            h ^= u64::from(*b);
            h = h.wrapping_mul(0x100000001b3);
        }
        self.0 = h;
    }
}

pub(crate) type NameMap<V> = HashMap<String, V, BuildHasherDefault<Fnv>>;
use std::fmt;

/// A format version, `MAJOR.MINOR`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Version {
    pub major: u32,
    pub minor: u32,
}

impl Version {
    pub fn parse(s: &str) -> Option<Version> {
        let (a, b) = s.split_once('.')?;
        let ok = |p: &str| !p.is_empty() && p.chars().all(|c| c.is_ascii_digit());
        if !ok(a) || !ok(b) {
            return None;
        }
        Some(Version {
            major: a.parse().ok()?,
            minor: b.parse().ok()?,
        })
    }
}

impl fmt::Display for Version {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}", self.major, self.minor)
    }
}

/// One element of a list's items.
#[derive(Clone, Debug, PartialEq)]
pub enum ItemType {
    Int,
    Float,
    String,
    Status,
    Enum(Vec<String>),
    /// The id of a record in a table; soft when the file needn't have it.
    Ref {
        to: String,
        soft: bool,
    },
    Element,
}

#[derive(Clone, Debug, PartialEq)]
pub enum FieldType {
    Int,
    Float,
    Bool,
    Status,
    String,
    Enum(Vec<String>),
    /// The id of a record in table `to`. A soft reference may name a record
    /// the file doesn't have (areas, zones and owners often aren't listed).
    Ref {
        to: String,
        soft: bool,
    },
    /// A typed reference, `kind:id`.
    Element,
    /// A list of single values (one item type) or of tuples (several);
    /// tuples may leave off trailing items down to `min`.
    List {
        items: Vec<ItemType>,
        min: usize,
    },
}

#[derive(Clone, Debug)]
pub struct FieldDef {
    pub name: String,
    pub ty: FieldType,
    pub unit: Option<String>,
    pub required: bool,
    /// The value a missing field has; None for fields that are absent
    /// unless set ("none").
    pub default: Option<Value>,
    pub doc: String,
}

#[derive(Clone, Debug)]
pub struct TableDef {
    pub name: String,
    /// The kind typed references use for this table's records, such as
    /// `line` in `line:205:310:1`. Two tables may share one.
    pub kind: String,
    /// How a new record's id is made from its fields: `{field}` parts, and
    /// `{sort(a,b)}` for bus numbers in increasing order, joined by `:`.
    pub id: String,
    /// Fields that order the table's records; empty keeps them as written.
    pub order: Vec<String>,
    pub singleton: bool,
    pub doc: String,
    pub fields: Vec<FieldDef>,
    index: NameMap<usize>,
}

impl TableDef {
    pub fn field(&self, name: &str) -> Option<&FieldDef> {
        self.index.get(name).map(|&i| &self.fields[i])
    }

    pub fn position(&self, name: &str) -> Option<usize> {
        self.index.get(name).copied()
    }
}

#[derive(Clone, Debug)]
pub struct Schema {
    /// The header word: `stannum-case` or `stannum-lists`.
    pub format: String,
    pub version: Version,
    pub doc: String,
    pub tables: Vec<TableDef>,
    index: NameMap<usize>,
}

impl Schema {
    pub fn table(&self, name: &str) -> Option<&TableDef> {
        self.index.get(name).map(|&i| &self.tables[i])
    }

    pub fn position(&self, name: &str) -> Option<usize> {
        self.index.get(name).copied()
    }

    /// Tables whose records typed references of `kind` name.
    pub fn tables_of_kind<'a>(&'a self, kind: &'a str) -> impl Iterator<Item = &'a TableDef> {
        self.tables.iter().filter(move |t| t.kind == kind)
    }

    pub fn parse(text: &str) -> Result<Schema, String> {
        let raw: RawSchema = toml::from_str(text).map_err(|e| e.to_string())?;
        let version = Version::parse(&raw.version).ok_or("bad schema version")?;
        let mut tables = Vec::new();
        for t in raw.table {
            let mut fields = Vec::new();
            for f in t.fields {
                let ty = field_type(&f).map_err(|e| format!("{}.{}: {e}", t.name, f.name))?;
                let default = match &f.default {
                    Some(v) => Some(
                        default_value(v, &ty)
                            .ok_or_else(|| format!("{}.{}: bad default", t.name, f.name))?,
                    ),
                    None => None,
                };
                fields.push(FieldDef {
                    name: f.name,
                    ty,
                    unit: f.unit,
                    required: f.required,
                    default,
                    doc: f.doc,
                });
            }
            let index = fields
                .iter()
                .enumerate()
                .map(|(i, f)| (f.name.clone(), i))
                .collect();
            tables.push(TableDef {
                name: t.name,
                kind: t.kind,
                id: t.id,
                order: t.order,
                singleton: t.singleton,
                doc: t.doc,
                fields,
                index,
            });
        }
        let index = tables
            .iter()
            .enumerate()
            .map(|(i, t)| (t.name.clone(), i))
            .collect();
        let schema = Schema {
            format: raw.format,
            version,
            doc: raw.doc,
            tables,
            index,
        };
        for t in &schema.tables {
            for f in &t.fields {
                if let FieldType::Ref { to, .. } = &f.ty
                    && schema.table(to).is_none()
                {
                    return Err(format!(
                        "{}.{} refers to unknown table {to}",
                        t.name, f.name
                    ));
                }
            }
        }
        Ok(schema)
    }
}

#[derive(Deserialize)]
struct RawSchema {
    format: String,
    version: String,
    doc: String,
    table: Vec<RawTable>,
}

#[derive(Deserialize)]
struct RawTable {
    name: String,
    kind: String,
    id: String,
    #[serde(default)]
    order: Vec<String>,
    #[serde(default)]
    singleton: bool,
    doc: String,
    fields: Vec<RawField>,
}

#[derive(Deserialize)]
struct RawField {
    name: String,
    #[serde(rename = "type")]
    ty: String,
    unit: Option<String>,
    #[serde(default)]
    values: Vec<String>,
    to: Option<String>,
    #[serde(default)]
    soft: bool,
    #[serde(default)]
    item: Vec<String>,
    item_min: Option<usize>,
    #[serde(default)]
    required: bool,
    default: Option<toml::Value>,
    doc: String,
}

fn item_type(s: &str) -> Result<ItemType, String> {
    Ok(match s {
        "int" => ItemType::Int,
        "float" => ItemType::Float,
        "string" => ItemType::String,
        "status" => ItemType::Status,
        "element" => ItemType::Element,
        _ => {
            if let Some(to) = s.strip_prefix("ref:") {
                ItemType::Ref {
                    to: to.to_string(),
                    soft: false,
                }
            } else if let Some(to) = s.strip_prefix("softref:") {
                ItemType::Ref {
                    to: to.to_string(),
                    soft: true,
                }
            } else if let Some(values) = s.strip_prefix("enum:") {
                ItemType::Enum(values.split('|').map(str::to_string).collect())
            } else {
                return Err(format!("unknown item type {s}"));
            }
        }
    })
}

fn field_type(f: &RawField) -> Result<FieldType, String> {
    Ok(match f.ty.as_str() {
        "int" => FieldType::Int,
        "float" => FieldType::Float,
        "bool" => FieldType::Bool,
        "status" => FieldType::Status,
        "string" => FieldType::String,
        "element" => FieldType::Element,
        "enum" => {
            if f.values.is_empty() {
                return Err("enum without values".into());
            }
            FieldType::Enum(f.values.clone())
        }
        "ref" => FieldType::Ref {
            to: f.to.clone().ok_or("ref without `to`")?,
            soft: f.soft,
        },
        "list" => {
            let items = f
                .item
                .iter()
                .map(|s| item_type(s))
                .collect::<Result<Vec<_>, _>>()?;
            if items.is_empty() {
                return Err("list without `item`".into());
            }
            let min = f.item_min.unwrap_or(items.len());
            FieldType::List { items, min }
        }
        other => return Err(format!("unknown type {other}")),
    })
}

fn default_value(v: &toml::Value, ty: &FieldType) -> Option<Value> {
    Some(match (ty, v) {
        (FieldType::Int, toml::Value::Integer(i)) => Value::Int(*i),
        (FieldType::Float, toml::Value::Float(f)) => Value::Float(*f),
        (FieldType::Bool, toml::Value::Boolean(b)) => Value::Bool(*b),
        (
            FieldType::Status
            | FieldType::String
            | FieldType::Enum(_)
            | FieldType::Ref { .. }
            | FieldType::Element,
            toml::Value::String(s),
        ) => Value::Str(s.clone()),
        (FieldType::List { items, .. }, toml::Value::Array(a)) => Value::List(
            a.iter()
                .map(|x| match (items.first(), x) {
                    (Some(ItemType::String), toml::Value::String(s)) => Some(Value::Str(s.clone())),
                    (Some(ItemType::Int), toml::Value::Integer(i)) => Some(Value::Int(*i)),
                    (Some(ItemType::Float), toml::Value::Float(f)) => Some(Value::Float(*f)),
                    _ => None,
                })
                .collect::<Option<Vec<_>>>()?,
        ),
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    #[test]
    fn built_in_schemas_load() {
        let case = crate::case_schema();
        assert_eq!(case.format, "stannum-case");
        assert!(case.table("bus").unwrap().field("vm_pu").is_some());
        assert_eq!(case.tables_of_kind("transformer").count(), 2);
        let lists = crate::lists_schema();
        assert_eq!(lists.format, "stannum-lists");
    }
}
