//! Row tables sent as columns.
//!
//! A large case has hundreds of thousands of rows, and as JSON objects every
//! row repeats its field names. [`Table`] holds rows like a `Vec` but
//! serializes as `count` plus one array per field, all `count` long, which
//! cuts a solve report to about a third (62 MB to 23 MB on a 78,000-bus
//! case). The columns come from the row type's own `Serialize`, so a field
//! added to a row can't be left out, and a field's `serialize_with` (such as
//! [`mw`] or [`pu`] rounding) applies to its column too.
//!
//! The frontend turns columns back into rows with `fromColumns`
//! (`src/lib/analysis/wire.ts`); Python keeps them as NumPy columns.

use schemars::{JsonSchema, Schema, SchemaGenerator};
use serde::Serialize;
use serde::ser::{self, Impossible, SerializeMap, SerializeStruct, Serializer};
use serde_json::{Map, Value, json};
use std::borrow::Cow;
use std::fmt;
use std::ops::{Deref, DerefMut};

/// Rows of one kind, sent as columns. Reads and changes like a `Vec<R>`.
#[derive(Clone, Debug, PartialEq)]
pub struct Table<R>(pub Vec<R>);

impl<R> Default for Table<R> {
    fn default() -> Self {
        Self(Vec::new())
    }
}

impl<R> Deref for Table<R> {
    type Target = Vec<R>;
    fn deref(&self) -> &Vec<R> {
        &self.0
    }
}

impl<R> DerefMut for Table<R> {
    fn deref_mut(&mut self) -> &mut Vec<R> {
        &mut self.0
    }
}

impl<R> From<Vec<R>> for Table<R> {
    fn from(rows: Vec<R>) -> Self {
        Self(rows)
    }
}

impl<R> FromIterator<R> for Table<R> {
    fn from_iter<I: IntoIterator<Item = R>>(iter: I) -> Self {
        Self(iter.into_iter().collect())
    }
}

impl<R> IntoIterator for Table<R> {
    type Item = R;
    type IntoIter = std::vec::IntoIter<R>;
    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}

impl<'a, R> IntoIterator for &'a Table<R> {
    type Item = &'a R;
    type IntoIter = std::slice::Iter<'a, R>;
    fn into_iter(self) -> Self::IntoIter {
        self.0.iter()
    }
}

/// The row type's field names, as serde writes them.
fn fields<R: JsonSchema>() -> Vec<String> {
    let schema = R::json_schema(&mut SchemaGenerator::default());
    schema
        .get("properties")
        .and_then(Value::as_object)
        .map(|p| p.keys().cloned().collect())
        .unwrap_or_default()
}

impl<R: Serialize + JsonSchema> Serialize for Table<R> {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        columns(&self.0, s)
    }
}

/// Serialize rows as columns: `count`, then one array per field of `R`.
pub fn columns<R: Serialize + JsonSchema, S: Serializer>(
    rows: &[R],
    s: S,
) -> Result<S::Ok, S::Error> {
    let names = fields::<R>();
    let mut map = s.serialize_map(Some(names.len() + 1))?;
    map.serialize_entry("count", &rows.len())?;
    for name in &names {
        map.serialize_entry(name, &Column { rows, name })?;
    }
    map.end()
}

impl<R: JsonSchema> JsonSchema for Table<R> {
    fn schema_name() -> Cow<'static, str> {
        format!("{}Columns", R::schema_name()).into()
    }

    fn json_schema(generator: &mut SchemaGenerator) -> Schema {
        let row = R::json_schema(generator);
        let mut properties = Map::new();
        properties.insert(
            "count".into(),
            json!({ "type": "integer", "format": "uint", "minimum": 0 }),
        );
        if let Some(fields) = row.get("properties").and_then(Value::as_object) {
            for (name, field) in fields {
                properties.insert(name.clone(), json!({ "type": "array", "items": field }));
            }
        }
        let required: Vec<_> = properties.keys().cloned().collect();
        json!({
            "type": "object",
            "description": format!(
                "{} rows as columns: one array per field, each `count` long.",
                R::schema_name()
            ),
            "properties": properties,
            "required": required,
        })
        .try_into()
        .expect("an object is a schema")
    }
}

/// One field of every row, as an array.
struct Column<'a, R> {
    rows: &'a [R],
    name: &'a str,
}

impl<R: Serialize> Serialize for Column<'_, R> {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.collect_seq(self.rows.iter().map(|row| Cell {
            row,
            name: self.name,
        }))
    }
}

/// One field of one row: the row is serialized with a serializer that
/// passes only that field on, so nothing else is written or copied.
struct Cell<'a, R> {
    row: &'a R,
    name: &'a str,
}

impl<R: Serialize> Serialize for Cell<'_, R> {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        let mut pick = Pick {
            name: self.name,
            out: Some(s),
            done: None,
        };
        if let Err(e) = self.row.serialize(&mut pick) {
            return Err(ser::Error::custom(e));
        }
        match (pick.done, pick.out) {
            (Some(done), _) => done,
            // A field the row skipped this time (skip_serializing_if).
            (None, Some(out)) => out.serialize_none(),
            (None, None) => unreachable!("the output is used only for the field"),
        }
    }
}

struct Pick<'a, S: Serializer> {
    name: &'a str,
    out: Option<S>,
    done: Option<Result<S::Ok, S::Error>>,
}

#[derive(Debug)]
struct NotARow(String);

impl fmt::Display for NotARow {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for NotARow {}

impl ser::Error for NotARow {
    fn custom<T: fmt::Display>(msg: T) -> Self {
        Self(msg.to_string())
    }
}

fn not_a_row<T>() -> Result<T, NotARow> {
    Err(NotARow("a table row must serialize as a struct".into()))
}

impl<S: Serializer> SerializeStruct for &mut Pick<'_, S> {
    type Ok = ();
    type Error = NotARow;

    fn serialize_field<T: ?Sized + Serialize>(
        &mut self,
        key: &'static str,
        value: &T,
    ) -> Result<(), NotARow> {
        if key == self.name
            && let Some(out) = self.out.take()
        {
            self.done = Some(value.serialize(out));
        }
        Ok(())
    }

    fn end(self) -> Result<(), NotARow> {
        Ok(())
    }
}

macro_rules! refuse {
    ($($method:ident($($arg:ty),*) -> $ret:ty;)*) => {
        $(fn $method(self, $(_: $arg),*) -> Result<$ret, NotARow> { not_a_row() })*
    };
}

impl<S: Serializer> Serializer for &mut Pick<'_, S> {
    type Ok = ();
    type Error = NotARow;
    type SerializeSeq = Impossible<(), NotARow>;
    type SerializeTuple = Impossible<(), NotARow>;
    type SerializeTupleStruct = Impossible<(), NotARow>;
    type SerializeTupleVariant = Impossible<(), NotARow>;
    type SerializeMap = Impossible<(), NotARow>;
    type SerializeStruct = Self;
    type SerializeStructVariant = Impossible<(), NotARow>;

    fn serialize_struct(self, _: &'static str, _: usize) -> Result<Self, NotARow> {
        Ok(self)
    }

    refuse! {
        serialize_bool(bool) -> ();
        serialize_i8(i8) -> ();
        serialize_i16(i16) -> ();
        serialize_i32(i32) -> ();
        serialize_i64(i64) -> ();
        serialize_u8(u8) -> ();
        serialize_u16(u16) -> ();
        serialize_u32(u32) -> ();
        serialize_u64(u64) -> ();
        serialize_f32(f32) -> ();
        serialize_f64(f64) -> ();
        serialize_char(char) -> ();
        serialize_str(&str) -> ();
        serialize_bytes(&[u8]) -> ();
        serialize_none() -> ();
        serialize_unit() -> ();
        serialize_unit_struct(&'static str) -> ();
        serialize_unit_variant(&'static str, u32, &'static str) -> ();
        serialize_seq(Option<usize>) -> Self::SerializeSeq;
        serialize_tuple(usize) -> Self::SerializeTuple;
        serialize_tuple_struct(&'static str, usize) -> Self::SerializeTupleStruct;
        serialize_tuple_variant(&'static str, u32, &'static str, usize) -> Self::SerializeTupleVariant;
        serialize_map(Option<usize>) -> Self::SerializeMap;
        serialize_struct_variant(&'static str, u32, &'static str, usize) -> Self::SerializeStructVariant;
    }

    fn serialize_some<T: ?Sized + Serialize>(self, _: &T) -> Result<(), NotARow> {
        not_a_row()
    }

    fn serialize_newtype_struct<T: ?Sized + Serialize>(
        self,
        _: &'static str,
        _: &T,
    ) -> Result<(), NotARow> {
        not_a_row()
    }

    fn serialize_newtype_variant<T: ?Sized + Serialize>(
        self,
        _: &'static str,
        _: u32,
        _: &'static str,
        _: &T,
    ) -> Result<(), NotARow> {
        not_a_row()
    }
}

fn round(x: f64, places: i32) -> f64 {
    if !x.is_finite() {
        return x;
    }
    let k = 10f64.powi(places);
    let r = (x * k).round() / k;
    // Keep a tiny value tiny rather than rounding it to zero, so
    // "outside limits" checks (violation > 0) still hold.
    if r == 0.0 && x != 0.0 { x } else { r }
}

/// For `#[serde(serialize_with)]` on MW, MVAr, kV and degree fields: 0.1 kW
/// is far below anything shown.
pub fn mw<S: Serializer>(x: &f64, s: S) -> Result<S::Ok, S::Error> {
    s.serialize_f64(round(*x, 4))
}

/// For per-unit voltage fields.
pub fn pu<S: Serializer>(x: &f64, s: S) -> Result<S::Ok, S::Error> {
    s.serialize_f64(round(*x, 6))
}

/// For percentages that may be missing.
pub fn pct<S: Serializer>(x: &Option<f64>, s: S) -> Result<S::Ok, S::Error> {
    match x {
        Some(x) => s.serialize_f64(round(*x, 3)),
        None => s.serialize_none(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Serialize, schemars::JsonSchema)]
    struct Row {
        id: u32,
        name: String,
        #[serde(serialize_with = "mw")]
        p: f64,
        #[serde(skip_serializing_if = "Option::is_none")]
        note: Option<String>,
        #[serde(skip)]
        #[allow(dead_code)]
        hidden: u8,
    }

    #[test]
    fn rows_become_columns() {
        let t: Table<Row> = vec![
            Row {
                id: 1,
                name: "a".into(),
                p: 1.234_567_89,
                note: None,
                hidden: 0,
            },
            Row {
                id: 2,
                name: "b".into(),
                p: -0.5,
                note: Some("x".into()),
                hidden: 0,
            },
        ]
        .into();
        let v = serde_json::to_value(&t).unwrap();
        assert_eq!(
            v,
            json!({
                "count": 2,
                "id": [1, 2],
                "name": ["a", "b"],
                "p": [1.2346, -0.5],
                "note": [null, "x"],
            })
        );
    }

    #[test]
    fn an_empty_table_still_lists_its_fields() {
        let v = serde_json::to_value(Table::<Row>::default()).unwrap();
        assert_eq!(v["count"], 0);
        assert_eq!(v["id"], json!([]));
        assert!(v.get("hidden").is_none());
    }
}
