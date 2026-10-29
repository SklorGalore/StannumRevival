//! Tables read a page at a time.
//!
//! An 80,000-bus case has hundreds of thousands of rows per table, more than
//! the app should copy across and keep just to show the thirty in view. The
//! core keeps the rows; the app asks for the slice it shows, sorted, searched
//! and filtered here, with [`PageQuery`], and gets a [`Page`] back.
//!
//! Rows are read through their own `Serialize`, the same way [`Table`] sends
//! them, so a column's name in a query is its field's name in the table
//! (nested fields joined with dots, such as `worst.loading_pct`), and a
//! field's rounding applies to sorting and searching too.

use crate::table::Table;
use schemars::JsonSchema;
use serde::ser::{self, Impossible, Serializer};
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::fmt;

/// The page size when a query doesn't say.
const DEFAULT_LIMIT: usize = 200;
/// The most rows one page sends.
pub const MAX_LIMIT: usize = 5_000;

/// Which rows of a table to send, in what order.
#[derive(Clone, Debug, Default, Deserialize, Serialize, JsonSchema, PartialEq)]
pub struct PageQuery {
    /// Rows to skip in the sorted, filtered order.
    #[serde(default)]
    pub offset: usize,
    /// Rows to send: 200 when left out, at most 5,000.
    #[serde(default)]
    pub limit: Option<usize>,
    /// Sort by one field. Left out, rows keep the table's own order.
    #[serde(default)]
    pub sort: Option<Sort>,
    /// Keep rows where any field's text contains this, ignoring case.
    #[serde(default)]
    pub search: String,
    /// Keep rows passing every rule.
    #[serde(default)]
    pub rules: Vec<Rule>,
    /// And, when there are any, passing at least one of these.
    #[serde(default)]
    pub any: Vec<Rule>,
    /// An element id listed even when the search or rules would hide it, so
    /// the picked row stays in view.
    #[serde(default)]
    pub keep: Option<String>,
    /// An element id whose position in the sorted, filtered rows is sent
    /// back as `found`, so the app can scroll to it.
    #[serde(default)]
    pub find: Option<String>,
}

impl PageQuery {
    pub fn limit(&self) -> usize {
        self.limit.unwrap_or(DEFAULT_LIMIT).min(MAX_LIMIT)
    }

    /// The query without its window: two queries that only differ in
    /// offset and limit list the same rows in the same order.
    pub fn order_key(&self) -> String {
        let mut q = self.clone();
        q.offset = 0;
        q.limit = None;
        serde_json::to_string(&q).unwrap_or_default()
    }

    fn filters(&self) -> bool {
        !self.search.trim().is_empty()
            || self.rules.iter().any(Rule::ready)
            || self.any.iter().any(Rule::ready)
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema, PartialEq)]
pub struct Sort {
    pub field: String,
    #[serde(default)]
    pub descending: bool,
}

/// One filter rule: a field compared with a value. The same rules as the
/// app's filter chips (`src/lib/filters.ts`).
#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema, PartialEq)]
pub struct Rule {
    pub field: String,
    pub operator: Operator,
    #[serde(default)]
    pub value: String,
    #[serde(default)]
    pub upper: Option<String>,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Operator {
    Eq,
    Neq,
    Contains,
    Gt,
    Gte,
    Lt,
    Lte,
    Between,
    Missing,
    Present,
}

impl Rule {
    /// Whether the rule is complete enough to apply.
    pub fn ready(&self) -> bool {
        match self.operator {
            Operator::Missing | Operator::Present => true,
            Operator::Between => {
                !self.value.trim().is_empty()
                    && self.upper.as_deref().is_some_and(|u| !u.trim().is_empty())
            }
            _ => !self.value.trim().is_empty(),
        }
    }

    /// Whether a value passes the rule.
    pub fn matches(&self, actual: &Value) -> bool {
        match self.operator {
            Operator::Missing => return actual.is_null(),
            Operator::Present => return !actual.is_null(),
            _ => {}
        }
        match actual {
            Value::Null => false,
            Value::Num(x) => {
                let Some(v) = number(&self.value) else {
                    return false;
                };
                match self.operator {
                    Operator::Eq => *x == v,
                    Operator::Neq => *x != v,
                    Operator::Gt => *x > v,
                    Operator::Gte => *x >= v,
                    Operator::Lt => *x < v,
                    Operator::Lte => *x <= v,
                    Operator::Between => self
                        .upper
                        .as_deref()
                        .and_then(number)
                        .is_some_and(|u| u >= v && *x >= v && *x <= u),
                    _ => false,
                }
            }
            Value::Bool(_) | Value::Text(_) => {
                let text = actual.text();
                match self.operator {
                    Operator::Eq => text == self.value,
                    Operator::Neq => text != self.value,
                    Operator::Contains => text.to_lowercase().contains(&self.value.to_lowercase()),
                    _ => false,
                }
            }
        }
    }
}

fn number(text: &str) -> Option<f64> {
    let t = text.trim();
    if t.is_empty() {
        return None;
    }
    t.parse::<f64>().ok().filter(|x| x.is_finite())
}

/// One field's value, as sorting and filtering see it.
#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    Null,
    Bool(bool),
    Num(f64),
    Text(String),
}

impl Value {
    pub fn is_null(&self) -> bool {
        matches!(self, Value::Null)
    }

    /// As the app writes it (JavaScript's `String(value)`).
    pub fn text(&self) -> String {
        match self {
            Value::Null => String::new(),
            Value::Bool(b) => b.to_string(),
            Value::Num(x) => js_number(*x),
            Value::Text(t) => t.clone(),
        }
    }

    pub fn from_json(v: &serde_json::Value) -> Value {
        match v {
            serde_json::Value::Bool(b) => Value::Bool(*b),
            serde_json::Value::Number(n) => n.as_f64().map_or(Value::Null, Value::Num),
            serde_json::Value::String(s) => Value::Text(s.clone()),
            _ => Value::Null,
        }
    }
}

/// A number written the way JavaScript writes it, for whole numbers and
/// the usual decimals.
fn js_number(x: f64) -> String {
    if x.is_nan() {
        "NaN".into()
    } else if x.is_infinite() {
        if x > 0.0 { "Infinity" } else { "-Infinity" }.into()
    } else {
        // Rust writes the shortest text that reads back as the same f64,
        // like JavaScript, and whole numbers without ".0".
        format!("{x}")
    }
}

/// The order the app's tables sort in: missing values last, numbers by
/// value, and text in natural order ("bus 2" before "bus 10").
pub fn compare(a: &Value, b: &Value) -> Ordering {
    match (a, b) {
        (Value::Null, Value::Null) => Ordering::Equal,
        (Value::Null, _) => Ordering::Greater,
        (_, Value::Null) => Ordering::Less,
        (Value::Num(x), Value::Num(y)) => x.partial_cmp(y).unwrap_or(Ordering::Equal),
        _ => natural(&a.text(), &b.text()),
    }
}

/// Text compared with runs of digits read as numbers, ignoring case first.
pub fn natural(a: &str, b: &str) -> Ordering {
    fn chunks(s: &str) -> impl Iterator<Item = (bool, &str)> {
        let mut rest = s;
        std::iter::from_fn(move || {
            let first = rest.chars().next()?;
            let digit = first.is_ascii_digit();
            let end = rest
                .char_indices()
                .find(|(_, c)| c.is_ascii_digit() != digit)
                .map_or(rest.len(), |(i, _)| i);
            let (chunk, tail) = rest.split_at(end);
            rest = tail;
            Some((digit, chunk))
        })
    }
    let (mut x, mut y) = (chunks(a), chunks(b));
    loop {
        match (x.next(), y.next()) {
            (None, None) => return a.cmp(b),
            (None, Some(_)) => return Ordering::Less,
            (Some(_), None) => return Ordering::Greater,
            (Some((true, p)), Some((true, q))) => {
                let (p, q) = (p.trim_start_matches('0'), q.trim_start_matches('0'));
                let o = p.len().cmp(&q.len()).then_with(|| p.cmp(q));
                if o != Ordering::Equal {
                    return o;
                }
            }
            (Some((_, p)), Some((_, q))) => {
                let o = p.to_lowercase().cmp(&q.to_lowercase());
                if o != Ordering::Equal {
                    return o;
                }
            }
        }
    }
}

/// Rows a page can be cut from.
pub trait Rows {
    fn row_count(&self) -> usize;
    /// One field of one row; `Null` for a field the row doesn't have.
    fn value(&self, row: usize, field: &str) -> Value;
    /// Whether any field's text contains `needle` (already lower case).
    fn contains(&self, row: usize, needle: &str) -> bool;
    /// The row's element id, for `keep` and `find`.
    fn element(&self, row: usize) -> Option<String>;
}

/// The rows a query lists, as indexes into the table in the order shown,
/// and where `find` is among them.
#[derive(Clone, Debug, Default)]
pub struct Order {
    pub indexes: Vec<usize>,
    pub found: Option<usize>,
}

/// Filter and sort `rows` as `query` asks. `extra` filters further (such
/// as the Power flow tables' bus rules); a row must pass it as well.
pub fn order(rows: &dyn Rows, query: &PageQuery, extra: Option<&dyn Fn(usize) -> bool>) -> Order {
    let needle = query.search.trim().to_lowercase();
    let rules: Vec<&Rule> = query.rules.iter().filter(|r| r.ready()).collect();
    let any: Vec<&Rule> = query.any.iter().filter(|r| r.ready()).collect();
    let keep = query.keep.as_deref();
    let filtering = query.filters() || extra.is_some();
    let mut indexes: Vec<usize> = (0..rows.row_count())
        .filter(|&i| {
            if !filtering {
                return true;
            }
            let passes = (needle.is_empty() || rows.contains(i, &needle))
                && rules.iter().all(|r| r.matches(&rows.value(i, &r.field)))
                && (any.is_empty() || any.iter().any(|r| r.matches(&rows.value(i, &r.field))))
                && extra.is_none_or(|f| f(i));
            passes || keep.is_some_and(|k| rows.element(i).as_deref() == Some(k))
        })
        .collect();
    if let Some(sort) = &query.sort {
        let mut keyed: Vec<(usize, Value)> = indexes
            .iter()
            .map(|&i| (i, rows.value(i, &sort.field)))
            .collect();
        // Stable, so ties keep the table's order.
        // Missing values stay last in both directions.
        keyed.sort_by(|a, b| {
            let o = compare(&a.1, &b.1);
            let missing = a.1.is_null() || b.1.is_null();
            if sort.descending && !missing {
                o.reverse()
            } else {
                o
            }
        });
        indexes = keyed.into_iter().map(|(i, _)| i).collect();
    }
    let found = query.find.as_deref().and_then(|f| {
        indexes
            .iter()
            .position(|&i| rows.element(i).as_deref() == Some(f))
    });
    Order { indexes, found }
}

/// One page of a table.
#[derive(Clone, Debug, Serialize, JsonSchema)]
#[schemars(rename = "PageOf{R}")]
pub struct Page<R: Serialize + JsonSchema> {
    /// Rows in the whole table.
    pub total: usize,
    /// Rows passing the search and rules (and the kept row).
    pub matched: usize,
    /// Where this page starts among the matched rows.
    pub offset: usize,
    /// Each row's position in the whole table, in the order sent.
    pub positions: Vec<usize>,
    pub rows: Table<R>,
    /// Where `find` is among the matched rows, if it is there.
    pub found: Option<usize>,
}

impl<R: Serialize + JsonSchema + Clone> Page<R> {
    /// Cut the page `query` asks for from `rows`, listed in `order`.
    pub fn cut(rows: &[R], order: &Order, query: &PageQuery) -> Self {
        let start = query.offset.min(order.indexes.len());
        let end = (start + query.limit()).min(order.indexes.len());
        let positions = order.indexes[start..end].to_vec();
        Page {
            total: rows.len(),
            matched: order.indexes.len(),
            offset: start,
            rows: positions.iter().map(|&i| rows[i].clone()).collect(),
            positions,
            found: order.found,
        }
    }
}

/// Rows of a struct type, read through their `Serialize`. `element` names
/// the field holding the element id, or builds one (buses have none).
pub struct StructRows<'a, R> {
    pub rows: &'a [R],
    pub element: fn(&R) -> Option<String>,
}

impl<R: Serialize> Rows for StructRows<'_, R> {
    fn row_count(&self) -> usize {
        self.rows.len()
    }

    fn value(&self, row: usize, field: &str) -> Value {
        let mut found = Value::Null;
        visit(&self.rows[row], &mut |path, v| {
            if path == field {
                found = v.to_owned();
            }
        });
        found
    }

    fn contains(&self, row: usize, needle: &str) -> bool {
        let mut hit = false;
        visit(&self.rows[row], &mut |_, v| {
            if !hit {
                hit = match v {
                    Leaf::Null => false,
                    Leaf::Text(t) => t.to_lowercase().contains(needle),
                    other => other.to_owned().text().to_lowercase().contains(needle),
                };
            }
        });
        hit
    }

    fn element(&self, row: usize) -> Option<String> {
        (self.element)(&self.rows[row])
    }
}

/// One field's value as the row serializes it, borrowed.
#[derive(Clone, Copy, Debug)]
pub enum Leaf<'a> {
    Null,
    Bool(bool),
    Num(f64),
    Text(&'a str),
}

impl Leaf<'_> {
    fn to_owned(self) -> Value {
        match self {
            Leaf::Null => Value::Null,
            Leaf::Bool(b) => Value::Bool(b),
            Leaf::Num(x) => Value::Num(x),
            Leaf::Text(t) => Value::Text(t.to_string()),
        }
    }
}

/// Call `f` with every field of `row` as a dotted path and its value.
/// Nested structs add to the path; items of a list are numbered.
pub fn visit<R: Serialize + ?Sized>(row: &R, f: &mut dyn FnMut(&str, Leaf<'_>)) {
    let mut path = String::new();
    let _ = row.serialize(Walk { path: &mut path, f });
}

struct Walk<'a, 'f> {
    path: &'a mut String,
    f: &'a mut (dyn FnMut(&str, Leaf<'_>) + 'f),
}

#[derive(Debug)]
struct Stop;

impl fmt::Display for Stop {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("stop")
    }
}

impl std::error::Error for Stop {}

impl ser::Error for Stop {
    fn custom<T: fmt::Display>(_: T) -> Self {
        Stop
    }
}

impl Walk<'_, '_> {
    fn leaf(self, v: Leaf<'_>) -> Result<(), Stop> {
        (self.f)(self.path, v);
        Ok(())
    }

    /// Visit `value` under `path.name`.
    fn child<T: ?Sized + Serialize>(&mut self, name: &str, value: &T) -> Result<(), Stop> {
        let len = self.path.len();
        if !self.path.is_empty() {
            self.path.push('.');
        }
        self.path.push_str(name);
        let r = value.serialize(Walk {
            path: self.path,
            f: self.f,
        });
        self.path.truncate(len);
        r
    }
}

struct Items<'a, 'f> {
    walk: Walk<'a, 'f>,
    next: usize,
}

impl ser::SerializeSeq for Items<'_, '_> {
    type Ok = ();
    type Error = Stop;
    fn serialize_element<T: ?Sized + Serialize>(&mut self, value: &T) -> Result<(), Stop> {
        let name = self.next.to_string();
        self.next += 1;
        self.walk.child(&name, value)
    }
    fn end(self) -> Result<(), Stop> {
        Ok(())
    }
}

impl ser::SerializeTuple for Items<'_, '_> {
    type Ok = ();
    type Error = Stop;
    fn serialize_element<T: ?Sized + Serialize>(&mut self, value: &T) -> Result<(), Stop> {
        ser::SerializeSeq::serialize_element(self, value)
    }
    fn end(self) -> Result<(), Stop> {
        Ok(())
    }
}

impl ser::SerializeStruct for Walk<'_, '_> {
    type Ok = ();
    type Error = Stop;
    fn serialize_field<T: ?Sized + Serialize>(
        &mut self,
        key: &'static str,
        value: &T,
    ) -> Result<(), Stop> {
        self.child(key, value)
    }
    fn end(self) -> Result<(), Stop> {
        Ok(())
    }
}

impl<'a, 'f> Serializer for Walk<'a, 'f> {
    type Ok = ();
    type Error = Stop;
    type SerializeSeq = Items<'a, 'f>;
    type SerializeTuple = Items<'a, 'f>;
    type SerializeTupleStruct = Impossible<(), Stop>;
    type SerializeTupleVariant = Impossible<(), Stop>;
    type SerializeMap = Impossible<(), Stop>;
    type SerializeStruct = Self;
    type SerializeStructVariant = Impossible<(), Stop>;

    fn serialize_bool(self, v: bool) -> Result<(), Stop> {
        self.leaf(Leaf::Bool(v))
    }
    fn serialize_i8(self, v: i8) -> Result<(), Stop> {
        self.leaf(Leaf::Num(v.into()))
    }
    fn serialize_i16(self, v: i16) -> Result<(), Stop> {
        self.leaf(Leaf::Num(v.into()))
    }
    fn serialize_i32(self, v: i32) -> Result<(), Stop> {
        self.leaf(Leaf::Num(v.into()))
    }
    fn serialize_i64(self, v: i64) -> Result<(), Stop> {
        self.leaf(Leaf::Num(v as f64))
    }
    fn serialize_u8(self, v: u8) -> Result<(), Stop> {
        self.leaf(Leaf::Num(v.into()))
    }
    fn serialize_u16(self, v: u16) -> Result<(), Stop> {
        self.leaf(Leaf::Num(v.into()))
    }
    fn serialize_u32(self, v: u32) -> Result<(), Stop> {
        self.leaf(Leaf::Num(v.into()))
    }
    fn serialize_u64(self, v: u64) -> Result<(), Stop> {
        self.leaf(Leaf::Num(v as f64))
    }
    fn serialize_f32(self, v: f32) -> Result<(), Stop> {
        self.leaf(Leaf::Num(v.into()))
    }
    fn serialize_f64(self, v: f64) -> Result<(), Stop> {
        // JSON has no NaN or infinity: they are sent as null.
        self.leaf(if v.is_finite() {
            Leaf::Num(v)
        } else {
            Leaf::Null
        })
    }
    fn serialize_char(self, v: char) -> Result<(), Stop> {
        let mut b = [0; 4];
        self.leaf(Leaf::Text(v.encode_utf8(&mut b)))
    }
    fn serialize_str(self, v: &str) -> Result<(), Stop> {
        self.leaf(Leaf::Text(v))
    }
    fn serialize_bytes(self, _: &[u8]) -> Result<(), Stop> {
        self.leaf(Leaf::Null)
    }
    fn serialize_none(self) -> Result<(), Stop> {
        self.leaf(Leaf::Null)
    }
    fn serialize_some<T: ?Sized + Serialize>(self, value: &T) -> Result<(), Stop> {
        value.serialize(self)
    }
    fn serialize_unit(self) -> Result<(), Stop> {
        self.leaf(Leaf::Null)
    }
    fn serialize_unit_struct(self, _: &'static str) -> Result<(), Stop> {
        self.leaf(Leaf::Null)
    }
    fn serialize_unit_variant(
        self,
        _: &'static str,
        _: u32,
        variant: &'static str,
    ) -> Result<(), Stop> {
        self.leaf(Leaf::Text(variant))
    }
    fn serialize_newtype_struct<T: ?Sized + Serialize>(
        self,
        _: &'static str,
        value: &T,
    ) -> Result<(), Stop> {
        value.serialize(self)
    }
    fn serialize_newtype_variant<T: ?Sized + Serialize>(
        self,
        _: &'static str,
        _: u32,
        _: &'static str,
        _: &T,
    ) -> Result<(), Stop> {
        Ok(())
    }
    fn serialize_seq(self, _: Option<usize>) -> Result<Items<'a, 'f>, Stop> {
        Ok(Items {
            walk: self,
            next: 0,
        })
    }
    fn serialize_tuple(self, _: usize) -> Result<Items<'a, 'f>, Stop> {
        Ok(Items {
            walk: self,
            next: 0,
        })
    }
    fn serialize_tuple_struct(
        self,
        _: &'static str,
        _: usize,
    ) -> Result<Self::SerializeTupleStruct, Stop> {
        Err(Stop)
    }
    fn serialize_tuple_variant(
        self,
        _: &'static str,
        _: u32,
        _: &'static str,
        _: usize,
    ) -> Result<Self::SerializeTupleVariant, Stop> {
        Err(Stop)
    }
    fn serialize_map(self, _: Option<usize>) -> Result<Self::SerializeMap, Stop> {
        Err(Stop)
    }
    fn serialize_struct(self, _: &'static str, _: usize) -> Result<Self, Stop> {
        Ok(self)
    }
    fn serialize_struct_variant(
        self,
        _: &'static str,
        _: u32,
        _: &'static str,
        _: usize,
    ) -> Result<Self::SerializeStructVariant, Stop> {
        Err(Stop)
    }
}

/// The distinct values of one field, sorted, missing last: the choices a
/// filter offers for that field. At most `limit`.
pub fn distinct(rows: &dyn Rows, field: &str, limit: usize) -> Vec<serde_json::Value> {
    let mut seen: Vec<Value> = Vec::new();
    let mut keys = std::collections::HashSet::new();
    for i in 0..rows.row_count() {
        let v = rows.value(i, field);
        let key = match &v {
            Value::Null => "\0null".to_string(),
            Value::Num(x) => format!("n{x}"),
            Value::Bool(b) => format!("b{b}"),
            Value::Text(t) => format!("t{t}"),
        };
        if keys.insert(key) {
            seen.push(v);
        }
    }
    seen.sort_by(compare);
    seen.into_iter()
        .take(limit)
        .map(|v| match v {
            Value::Null => serde_json::Value::Null,
            Value::Bool(b) => b.into(),
            Value::Num(x) => x.into(),
            Value::Text(t) => t.into(),
        })
        .collect()
}

/// A Power flow table's rows: buses, generators or branches of one result.
pub mod results {
    use super::*;
    use crate::power::{BranchRow, BusRow, GeneratorRow, SolveReport};
    use std::collections::HashSet;

    pub fn bus_element(b: &BusRow) -> Option<String> {
        Some(format!("bus:{}", b.id))
    }

    pub fn buses(report: &SolveReport) -> StructRows<'_, BusRow> {
        StructRows {
            rows: &report.buses,
            element: bus_element,
        }
    }

    pub fn generators(report: &SolveReport) -> StructRows<'_, GeneratorRow> {
        StructRows {
            rows: &report.generators,
            element: |g| Some(g.element.clone()),
        }
    }

    pub fn branches(report: &SolveReport) -> StructRows<'_, BranchRow> {
        StructRows {
            rows: &report.branches,
            element: |b| Some(b.element.clone()),
        }
    }

    /// A bus filter field (`src/lib/analysis/busFilters.ts`): area, base_kv,
    /// kind, number, name or vm. A blank name reads as missing.
    pub fn bus_field(report: &SolveReport, row: usize, field: &str) -> Value {
        match field {
            "number" => Value::Num(report.buses[row].id.into()),
            "name" => {
                let name = report.buses[row].name.trim();
                if name.is_empty() {
                    Value::Null
                } else {
                    Value::Text(name.into())
                }
            }
            "area" | "base_kv" | "kind" | "vm" => buses(report).value(row, field),
            _ => Value::Null,
        }
    }

    /// The buses passing every finished rule, or None when no rule applies.
    pub fn passing(report: &SolveReport, rules: &[Rule]) -> Option<HashSet<u32>> {
        let ready: Vec<&Rule> = rules.iter().filter(|r| r.ready()).collect();
        if ready.is_empty() {
            return None;
        }
        Some(
            (0..report.buses.len())
                .filter(|&i| {
                    ready
                        .iter()
                        .all(|r| r.matches(&bus_field(report, i, &r.field)))
                })
                .map(|i| report.buses[i].id)
                .collect(),
        )
    }

    /// The distinct values of a bus filter field.
    pub fn bus_values(report: &SolveReport, field: &str, limit: usize) -> Vec<serde_json::Value> {
        struct Field<'a>(&'a SolveReport, &'a str);
        impl Rows for Field<'_> {
            fn row_count(&self) -> usize {
                self.0.buses.len()
            }
            fn value(&self, row: usize, _: &str) -> Value {
                bus_field(self.0, row, self.1)
            }
            fn contains(&self, _: usize, _: &str) -> bool {
                false
            }
            fn element(&self, _: usize) -> Option<String> {
                None
            }
        }
        distinct(&Field(report, field), field, limit)
    }
}

/// A Records table's rows. Fields are column positions ("0", "1", ...),
/// plus "line" and "element".
impl Rows for crate::records::RecordTable {
    fn row_count(&self) -> usize {
        self.rows.len()
    }

    fn value(&self, row: usize, field: &str) -> Value {
        let r = &self.rows[row];
        match field {
            "line" => Value::Num(r.line as f64),
            "element" => r.element.clone().map_or(Value::Null, Value::Text),
            _ => field
                .parse::<usize>()
                .ok()
                .and_then(|i| r.cells.get(i))
                .map_or(Value::Null, Value::from_json),
        }
    }

    fn contains(&self, row: usize, needle: &str) -> bool {
        let r = &self.rows[row];
        (r.line > 0 && r.line.to_string().contains(needle))
            || r.cells.iter().any(|c| {
                let v = Value::from_json(c);
                !v.is_null() && v.text().to_lowercase().contains(needle)
            })
    }

    fn element(&self, row: usize) -> Option<String> {
        self.rows[row].element.clone()
    }
}

/// How a Records column filters: by number, by text, or by picking one of
/// a few values.
#[derive(Clone, Copy, Debug, Serialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum FieldType {
    Number,
    Text,
    Category,
}

/// Columns with this many distinct values or fewer filter by picking a value.
pub const CATEGORY_LIMIT: usize = 40;

/// Each Records column's filter type: number columns by number, and other
/// columns with few distinct values by picking one.
pub fn record_field_types(table: &crate::records::RecordTable) -> Vec<FieldType> {
    use crate::records::ColumnKind;
    table
        .columns
        .iter()
        .enumerate()
        .map(|(i, c)| {
            if matches!(c.kind, ColumnKind::Number) {
                return FieldType::Number;
            }
            let mut seen = std::collections::HashSet::new();
            for r in &table.rows {
                seen.insert(r.cells.get(i).map(|v| v.to_string()).unwrap_or_default());
                if seen.len() > CATEGORY_LIMIT {
                    break;
                }
            }
            if seen.len() <= CATEGORY_LIMIT {
                FieldType::Category
            } else if matches!(c.kind, ColumnKind::Text) {
                FieldType::Text
            } else {
                FieldType::Number
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Serialize;

    #[derive(Clone, Serialize, JsonSchema)]
    struct Worst {
        loading_pct: Option<f64>,
    }

    #[derive(Clone, Serialize, JsonSchema)]
    struct Row {
        element: String,
        name: String,
        kv: f64,
        on: bool,
        worst: Option<Worst>,
    }

    fn rows() -> Vec<Row> {
        let r = |e: &str, n: &str, kv: f64, w: Option<f64>| Row {
            element: e.into(),
            name: n.into(),
            kv,
            on: true,
            worst: w.map(|x| Worst {
                loading_pct: Some(x),
            }),
        };
        vec![
            r("bus:10", "North 10", 345.0, Some(120.0)),
            r("bus:2", "North 2", 138.0, None),
            r("bus:3", "South", 345.0, Some(80.0)),
            r("bus:4", "", 69.0, Some(101.5)),
        ]
    }

    fn table(rows: &[Row]) -> StructRows<'_, Row> {
        StructRows {
            rows,
            element: |r| Some(r.element.clone()),
        }
    }

    fn elements(rows: &[Row], q: &PageQuery) -> Vec<String> {
        let o = order(&table(rows), q, None);
        o.indexes.iter().map(|&i| rows[i].element.clone()).collect()
    }

    #[test]
    fn nested_fields_are_dotted() {
        let r = rows();
        assert_eq!(table(&r).value(0, "worst.loading_pct"), Value::Num(120.0));
        assert_eq!(table(&r).value(1, "worst.loading_pct"), Value::Null);
        assert_eq!(table(&r).value(0, "on"), Value::Bool(true));
    }

    #[test]
    fn sorts_with_missing_last_and_text_in_natural_order() {
        let r = rows();
        let mut q = PageQuery {
            sort: Some(Sort {
                field: "worst.loading_pct".into(),
                descending: false,
            }),
            ..Default::default()
        };
        assert_eq!(elements(&r, &q), ["bus:3", "bus:4", "bus:10", "bus:2"]);
        // Descending too: the largest first, still missing last.
        q.sort.as_mut().unwrap().descending = true;
        assert_eq!(elements(&r, &q), ["bus:10", "bus:4", "bus:3", "bus:2"]);
        q.sort = Some(Sort {
            field: "name".into(),
            descending: false,
        });
        assert_eq!(elements(&r, &q), ["bus:4", "bus:2", "bus:10", "bus:3"]);
        q.sort = Some(Sort {
            field: "kv".into(),
            descending: true,
        });
        // Ties keep the table's order.
        assert_eq!(elements(&r, &q), ["bus:10", "bus:3", "bus:2", "bus:4"]);
    }

    #[test]
    fn searches_rules_keep_and_find() {
        let r = rows();
        let mut q = PageQuery {
            search: "north".into(),
            find: Some("bus:2".into()),
            ..Default::default()
        };
        let o = order(&table(&r), &q, None);
        assert_eq!(o.indexes, [0, 1]);
        assert_eq!(o.found, Some(1));
        // Numbers are searched as they are written.
        q.search = "34".into();
        assert_eq!(elements(&r, &q), ["bus:10", "bus:3"]);
        q.search = String::new();
        q.rules = vec![Rule {
            field: "kv".into(),
            operator: Operator::Between,
            value: "100".into(),
            upper: Some("200".into()),
        }];
        q.keep = Some("bus:4".into());
        assert_eq!(elements(&r, &q), ["bus:2", "bus:4"]);
        // An unfinished rule doesn't filter.
        q.rules[0].upper = None;
        assert_eq!(elements(&r, &q).len(), 4);
        // Any one of these.
        q.keep = None;
        q.any = vec![
            Rule {
                field: "kv".into(),
                operator: Operator::Lt,
                value: "100".into(),
                upper: None,
            },
            Rule {
                field: "worst.loading_pct".into(),
                operator: Operator::Gt,
                value: "110".into(),
                upper: None,
            },
        ];
        assert_eq!(elements(&r, &q), ["bus:10", "bus:4"]);
    }

    #[test]
    fn a_page_is_cut_from_the_order() {
        let r = rows();
        let q = PageQuery {
            offset: 1,
            limit: Some(2),
            sort: Some(Sort {
                field: "element".into(),
                descending: false,
            }),
            ..Default::default()
        };
        let page = Page::cut(&r, &order(&table(&r), &q, None), &q);
        assert_eq!((page.total, page.matched, page.offset), (4, 4, 1));
        assert_eq!(page.positions, [2, 3]);
        let v = serde_json::to_value(&page).unwrap();
        assert_eq!(v["rows"]["element"], serde_json::json!(["bus:3", "bus:4"]));
    }

    #[test]
    fn distinct_values_are_sorted() {
        let r = rows();
        assert_eq!(
            distinct(&table(&r), "kv", 10),
            [serde_json::json!(69.0), 138.0.into(), 345.0.into()]
        );
    }

    #[test]
    fn natural_order() {
        assert_eq!(natural("bus 2", "bus 10"), Ordering::Less);
        assert_eq!(natural("B", "a"), Ordering::Greater);
        assert_eq!(natural("a", "A"), Ordering::Greater);
    }
}
