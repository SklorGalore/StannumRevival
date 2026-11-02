//! Reading and writing the text form.
//!
//! ```text
//! stannum-case 1.0
//! # A comment just above a record belongs to it.
//! bus 101 number=101 name="GEN A" kv=13.8 type=ref
//! ```
//!
//! Line 1 is the header, `format MAJOR.MINOR`. Every other line is blank,
//! a comment (`#` to the end of the line), or one record: the table name,
//! the record's id, then `name=value` fields separated by spaces. Values are
//! bare tokens, `"quoted strings"` with `\"`, `\\`, `\n`, `\t` and `\r`
//! escapes, or `[lists, of, values]`.

use crate::document::{Document, Record, Table, ref_target};
use crate::schema::{FieldType, ItemType, Schema, TableDef, Version};
use crate::value::{Value, bare_char, parse_float, write_str};
use std::collections::{HashMap, HashSet};
use std::fmt;

/// One problem with a file, at a line and column (both from 1).
#[derive(Clone, Debug, PartialEq)]
pub struct Issue {
    pub line: usize,
    pub column: usize,
    pub message: String,
}

impl fmt::Display for Issue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "line {}, column {}: {}",
            self.line, self.column, self.message
        )
    }
}

/// Why a file couldn't be read: every problem found, in line order (at
/// most [`MAX_ISSUES`]).
#[derive(Clone, Debug, PartialEq)]
pub struct ReadError {
    pub issues: Vec<Issue>,
}

pub const MAX_ISSUES: usize = 50;

impl fmt::Display for ReadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (i, issue) in self.issues.iter().enumerate() {
            if i > 0 {
                writeln!(f)?;
            }
            write!(f, "{issue}")?;
        }
        Ok(())
    }
}

impl std::error::Error for ReadError {}

fn issue(line: usize, column: usize, message: impl Into<String>) -> Issue {
    Issue {
        line,
        column,
        message: message.into(),
    }
}

// ---------------------------------------------------------------------------
// Tokens.

/// A value as written, before the schema gives it a type.
#[derive(Clone, Debug)]
enum Syntax {
    Bare(String),
    Quoted(String),
    List(Vec<(usize, Syntax)>),
}

struct Cursor<'a> {
    line: &'a str,
    at: usize,
    number: usize,
}

impl Cursor<'_> {
    fn peek(&self) -> Option<char> {
        self.line[self.at..].chars().next()
    }

    fn column(&self) -> usize {
        self.line[..self.at].chars().count() + 1
    }

    fn bump(&mut self) -> Option<char> {
        let c = self.peek()?;
        self.at += c.len_utf8();
        Some(c)
    }

    fn skip_spaces(&mut self) -> bool {
        let start = self.at;
        while matches!(self.peek(), Some(' ' | '\t')) {
            self.bump();
        }
        self.at > start
    }

    fn err(&self, message: impl Into<String>) -> Issue {
        issue(self.number, self.column(), message)
    }

    fn bare(&mut self) -> String {
        let start = self.at;
        while self.peek().is_some_and(bare_char) {
            self.bump();
        }
        self.line[start..self.at].to_string()
    }

    fn quoted(&mut self) -> Result<String, Issue> {
        self.bump(); // the opening quote
        let mut out = String::new();
        loop {
            match self.bump() {
                None => return Err(self.err("a quoted string isn't closed")),
                Some('"') => return Ok(out),
                Some('\\') => match self.bump() {
                    Some('"') => out.push('"'),
                    Some('\\') => out.push('\\'),
                    Some('n') => out.push('\n'),
                    Some('t') => out.push('\t'),
                    Some('r') => out.push('\r'),
                    _ => return Err(self.err("unknown escape in a quoted string")),
                },
                Some(c) => out.push(c),
            }
        }
    }

    fn value(&mut self, depth: usize) -> Result<Syntax, Issue> {
        match self.peek() {
            Some('"') => Ok(Syntax::Quoted(self.quoted()?)),
            Some('[') => {
                if depth >= 2 {
                    return Err(self.err("lists nest one level at most"));
                }
                self.bump();
                let mut items = Vec::new();
                self.skip_spaces();
                if self.peek() == Some(']') {
                    self.bump();
                    return Ok(Syntax::List(items));
                }
                loop {
                    self.skip_spaces();
                    let column = self.column();
                    items.push((column, self.value(depth + 1)?));
                    self.skip_spaces();
                    match self.bump() {
                        Some(',') => continue,
                        Some(']') => return Ok(Syntax::List(items)),
                        _ => return Err(self.err("expected `,` or `]` in a list")),
                    }
                }
            }
            Some(c) if bare_char(c) => Ok(Syntax::Bare(self.bare())),
            _ => Err(self.err("expected a value")),
        }
    }
}

struct Line {
    table: String,
    id: String,
    /// Field name, its column, its value.
    fields: Vec<(String, usize, Syntax)>,
    comment: Option<String>,
}

fn comment_text(s: &str) -> String {
    let s = s.strip_prefix('#').unwrap_or(s);
    s.strip_prefix(' ').unwrap_or(s).trim_end().to_string()
}

fn record_line(text: &str, number: usize) -> Result<Line, Issue> {
    let mut c = Cursor {
        line: text,
        at: 0,
        number,
    };
    c.skip_spaces();
    let table = c.bare();
    if table.is_empty() {
        return Err(c.err("expected a table name"));
    }
    if !c.skip_spaces() {
        return Err(c.err("expected a space and the record's id after the table name"));
    }
    let id = match c.peek() {
        Some('"') => c.quoted()?,
        _ => c.bare(),
    };
    if id.is_empty() {
        return Err(c.err("expected the record's id"));
    }
    let mut fields = Vec::new();
    let mut comment = None;
    loop {
        let spaced = c.skip_spaces();
        match c.peek() {
            None => break,
            Some('#') => {
                comment = Some(comment_text(&text[c.at..]));
                break;
            }
            _ if !spaced => return Err(c.err("expected a space between fields")),
            _ => {}
        }
        let column = c.column();
        let name = c.bare();
        if name.is_empty() || c.bump() != Some('=') {
            return Err(issue(number, column, "expected `name=value`"));
        }
        let value = c.value(0)?;
        fields.push((name, column, value));
    }
    Ok(Line {
        table,
        id,
        fields,
        comment,
    })
}

// ---------------------------------------------------------------------------
// Types.

fn word_in(s: &str, values: &[String]) -> Result<Value, String> {
    if values.iter().any(|v| v == s) {
        Ok(Value::Str(s.to_string()))
    } else {
        Err(format!("expected one of {}, not `{s}`", values.join(", ")))
    }
}

fn element(s: &str) -> Result<Value, String> {
    match s.split_once(':') {
        Some((kind, id)) if !kind.is_empty() && !id.is_empty() => Ok(Value::Str(s.to_string())),
        _ => Err(format!(
            "expected a typed reference such as `line:205:310:1`, not `{s}`"
        )),
    }
}

fn scalar(syntax: &Syntax, ty: &FieldType) -> Result<Value, String> {
    let (text, quoted) = match syntax {
        Syntax::Bare(s) => (s.as_str(), false),
        Syntax::Quoted(s) => (s.as_str(), true),
        Syntax::List(_) => return Err("expected a single value, not a list".into()),
    };
    let want = |what: &str| format!("expected {what}, not `{text}`");
    match ty {
        FieldType::String | FieldType::Ref { .. } => Ok(Value::Str(text.to_string())),
        _ if quoted => Err(format!(
            "`\"{text}\"` is quoted; this field takes a bare value"
        )),
        FieldType::Int => text
            .parse()
            .map(Value::Int)
            .map_err(|_| want("a whole number")),
        FieldType::Float => parse_float(text)
            .map(Value::Float)
            .ok_or_else(|| want("a number")),
        FieldType::Bool => match text {
            "true" => Ok(Value::Bool(true)),
            "false" => Ok(Value::Bool(false)),
            _ => Err(want("`true` or `false`")),
        },
        FieldType::Status => word_in(text, &["on".into(), "off".into()]),
        FieldType::Enum(values) => word_in(text, values),
        FieldType::Element => element(text),
        FieldType::List { .. } => Err("expected a list".into()),
    }
}

fn item_field_type(item: &ItemType) -> FieldType {
    match item {
        ItemType::Int => FieldType::Int,
        ItemType::Float => FieldType::Float,
        ItemType::String => FieldType::String,
        ItemType::Status => FieldType::Status,
        ItemType::Enum(v) => FieldType::Enum(v.clone()),
        ItemType::Ref { to, soft } => FieldType::Ref {
            to: to.clone(),
            soft: *soft,
        },
        ItemType::Element => FieldType::Element,
    }
}

/// A value typed by the schema; a list item's own column comes with its error.
fn typed(syntax: &Syntax, ty: &FieldType, column: usize) -> Result<Value, (usize, String)> {
    let FieldType::List { items, min } = ty else {
        return scalar(syntax, ty).map_err(|e| (column, e));
    };
    let Syntax::List(list) = syntax else {
        return Err((column, "expected a list in `[...]`".into()));
    };
    let mut out = Vec::new();
    for (col, item) in list {
        if items.len() == 1 {
            out.push(scalar(item, &item_field_type(&items[0])).map_err(|e| (*col, e))?);
            continue;
        }
        let Syntax::List(parts) = item else {
            return Err((
                *col,
                format!("expected a tuple of {} values in `[...]`", items.len()),
            ));
        };
        if parts.len() < *min || parts.len() > items.len() {
            let n = if min == &items.len() {
                format!("{min}")
            } else {
                format!("{min} to {}", items.len())
            };
            return Err((
                *col,
                format!("expected {n} values in each `[...]`, not {}", parts.len()),
            ));
        }
        let mut tuple = Vec::new();
        for ((pc, part), it) in parts.iter().zip(items) {
            tuple.push(scalar(part, &item_field_type(it)).map_err(|e| (*pc, e))?);
        }
        out.push(Value::List(tuple));
    }
    Ok(Value::List(out))
}

/// Tables a reader keeps without knowing them: extensions (`x.`) and the
/// names reserved for sequence and dynamics data (`seq.`, `dyn.`).
pub(crate) fn kept_table(name: &str) -> bool {
    ["x.", "seq.", "dyn."].iter().any(|p| name.starts_with(p))
}

/// A value written as text, for a field the schema doesn't know: as the
/// text reader would keep it. None when it isn't one value.
pub(crate) fn untyped_from_text(text: &str) -> Option<Value> {
    let mut c = Cursor {
        line: text,
        at: 0,
        number: 0,
    };
    let syntax = c.value(0).ok()?;
    (c.at == text.len()).then(|| untyped(&syntax))
}

/// What the text reader checks beyond a value's type: enum words, typed
/// references, tuple lengths. For values that arrive already typed.
pub(crate) fn check_value(value: &Value, ty: &FieldType) -> Result<(), String> {
    let word = |v: &Value, values: &[String]| match v.as_str() {
        Some(s) => word_in(s, values).map(|_| ()),
        None => Err("expected a word".to_string()),
    };
    let item = |v: &Value, it: &ItemType| match it {
        ItemType::Enum(values) => word(v, values),
        ItemType::Status => word(v, &["on".into(), "off".into()]),
        ItemType::Element => element(v.as_str().unwrap_or_default()).map(|_| ()),
        _ => Ok(()),
    };
    match ty {
        FieldType::Enum(values) => word(value, values),
        FieldType::Status => word(value, &["on".into(), "off".into()]),
        FieldType::Element => element(value.as_str().unwrap_or_default()).map(|_| ()),
        FieldType::Float => match value {
            Value::Float(f) if f.is_nan() => Err("NaN isn't a number here".into()),
            _ => Ok(()),
        },
        FieldType::List { items, min } => {
            for v in value.as_list().unwrap_or_default() {
                if items.len() == 1 {
                    item(v, &items[0])?;
                    continue;
                }
                let parts = v.as_list().unwrap_or_default();
                if parts.len() < *min || parts.len() > items.len() {
                    return Err(format!(
                        "expected {min} to {} values in each tuple, not {}",
                        items.len(),
                        parts.len()
                    ));
                }
                for (p, it) in parts.iter().zip(items) {
                    item(p, it)?;
                }
            }
            Ok(())
        }
        _ => Ok(()),
    }
}

fn untyped(syntax: &Syntax) -> Value {
    match syntax {
        Syntax::Bare(s) => Value::Raw(s.clone()),
        Syntax::Quoted(s) => Value::Str(s.clone()),
        Syntax::List(items) => Value::List(items.iter().map(|(_, s)| untyped(s)).collect()),
    }
}

// ---------------------------------------------------------------------------
// Reading.

/// Lines read on one thread at a time.
const LINES: usize = 8192;

/// One line of a case file, read on its own.
enum Event {
    Blank,
    Comment(String),
    /// A line that isn't a record.
    Bad(Issue),
    Record {
        table: String,
        /// None when the table is unknown.
        record: Option<Record>,
        /// The comment at the end of the line.
        comment: Option<String>,
        issues: Vec<Issue>,
    },
}

/// Read a line (`number`, one-based) without what comes before it: comments
/// and the table's place are settled by the caller. `strict` refuses names
/// the schema doesn't have (files no newer than it).
fn line_event(raw: &str, number: usize, schema: &'static Schema, strict: bool) -> Event {
    let line = raw.trim_end_matches('\r');
    let trimmed = line.trim_start();
    if trimmed.is_empty() {
        return Event::Blank;
    }
    if trimmed.starts_with('#') {
        return Event::Comment(comment_text(trimmed));
    }
    let parsed = match record_line(line, number) {
        Ok(l) => l,
        Err(e) => return Event::Bad(e),
    };
    let mut issues = Vec::new();
    let def = schema.table(&parsed.table);
    if def.is_none() && !kept_table(&parsed.table) && strict {
        issues.push(issue(
            number,
            1,
            format!("unknown table `{}`", parsed.table),
        ));
        return Event::Record {
            table: parsed.table,
            record: None,
            comment: None,
            issues,
        };
    }
    let mut record = Record::new(parsed.id);
    record.line = number;
    let mut names: HashSet<&str> = HashSet::new();
    for (name, column, syntax) in &parsed.fields {
        if !names.insert(name) {
            issues.push(issue(number, *column, format!("`{name}` is given twice")));
            continue;
        }
        let field = def.and_then(|d| d.field(name));
        let value = match field {
            Some(f) => match typed(syntax, &f.ty, column + name.len() + 1) {
                Ok(v) => v,
                Err((col, e)) => {
                    issues.push(issue(number, col, format!("{name}: {e}")));
                    // Kept untyped so it isn't also reported missing;
                    // the file is refused either way.
                    untyped(syntax)
                }
            },
            None => {
                let extension = name.starts_with("x.");
                if def.is_some() && !extension && strict {
                    issues.push(issue(
                        number,
                        *column,
                        format!("`{}` has no field `{name}`", parsed.table),
                    ));
                    continue;
                }
                untyped(syntax)
            }
        };
        record.fields.push((name.clone(), value));
    }
    Event::Record {
        table: parsed.table,
        record: Some(record),
        comment: parsed.comment,
        issues,
    }
}

/// Read a file against `schema`. Every problem is reported, with its line
/// and column; nothing is returned unless the file is entirely good.
pub fn read(text: &str, schema: &'static Schema) -> Result<Document, ReadError> {
    let mut issues = Vec::new();
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let mut lines = text.split('\n').enumerate();
    let header = lines
        .next()
        .map(|(_, l)| l.trim_end_matches('\r'))
        .unwrap_or("");
    let version = match header.split_once(' ') {
        Some((format, version)) if format == schema.format => {
            match Version::parse(version.trim()) {
                Some(v) => v,
                None => {
                    return Err(ReadError {
                        issues: vec![issue(
                            1,
                            format.len() + 2,
                            format!("expected a version such as `{}`", schema.version),
                        )],
                    });
                }
            }
        }
        _ => {
            return Err(ReadError {
                issues: vec![issue(
                    1,
                    1,
                    format!(
                        "the first line must be `{} {}`",
                        schema.format, schema.version
                    ),
                )],
            });
        }
    };
    let ours = schema.version;
    if version.major > ours.major || (version.major == 0 && version > ours) {
        return Err(ReadError {
            issues: vec![issue(
                1,
                1,
                format!(
                    "this file is {} {version}, newer than the {ours} this Stannum reads; open it in a newer Stannum",
                    schema.format
                ),
            )],
        });
    }
    let mut doc = Document::new(schema);
    doc.version = version;
    // Unknown names are mistakes unless the file is a newer minor version.
    let strict = version <= ours;
    // Lines are read in chunks side by side; comments, the preamble and
    // table order are then settled in file order.
    let rest: Vec<(usize, &str)> = lines.collect();
    let events: Vec<Vec<Event>> = std::thread::scope(|s| {
        let tasks: Vec<_> = rest
            .chunks(LINES)
            .map(|chunk| {
                let read = move || {
                    chunk
                        .iter()
                        .map(|(i, raw)| line_event(raw, i + 1, schema, strict))
                        .collect()
                };
                if rest.len() > LINES {
                    Ok(s.spawn(read))
                } else {
                    Err(read())
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
    let mut pending: Vec<String> = Vec::new();
    let mut seen_record = false;
    // The tables whose first record has been read.
    let mut started: HashSet<String> = HashSet::new();
    for event in events.into_iter().flatten() {
        match event {
            Event::Blank => {
                if !seen_record {
                    doc.preamble.append(&mut pending);
                } else {
                    pending.clear();
                }
            }
            Event::Comment(c) => pending.push(c),
            Event::Bad(e) => {
                issues.push(e);
                pending.clear();
            }
            Event::Record {
                table,
                record,
                comment,
                issues: mut found,
            } => {
                seen_record = true;
                // A `# table` line just above a table's first record marks
                // the table; it isn't the record's comment.
                if !started.contains(&table) {
                    started.insert(table.clone());
                    if pending.first().is_some_and(|c| c == &table) {
                        pending.remove(0);
                    }
                }
                issues.append(&mut found);
                let comments = std::mem::take(&mut pending);
                if let Some(mut record) = record {
                    record.comments = comments;
                    record.comments.extend(comment);
                    doc.table_mut(&table).records.push(record);
                }
            }
        }
    }
    check(&doc, &mut issues);
    if issues.is_empty() {
        Ok(doc)
    } else {
        issues.sort_by_key(|i| (i.line, i.column));
        issues.truncate(MAX_ISSUES);
        Err(ReadError { issues })
    }
}

/// Ids unique per kind, required fields present, references resolved.
pub(crate) fn check(doc: &Document, issues: &mut Vec<Issue>) {
    let schema = doc.schema;
    let records = doc.tables.iter().map(|t| t.records.len()).sum();
    let mut ids: HashMap<(&str, &str), usize, std::hash::BuildHasherDefault<crate::schema::Fnv>> =
        HashMap::with_capacity_and_hasher(records, Default::default());
    for table in &doc.tables {
        let Some(def) = schema.table(&table.name) else {
            continue;
        };
        if def.singleton && table.records.len() > 1 {
            issues.push(issue(
                table.records[1].line,
                1,
                format!("a file has one `{}` record", def.name),
            ));
        }
        for r in &table.records {
            if let Some(first) = ids.insert((def.kind.as_str(), r.id.as_str()), r.line) {
                issues.push(issue(
                    r.line,
                    def.name.len() + 2,
                    format!("{} id `{}` is already used on line {first}", def.kind, r.id),
                ));
            }
            for f in def.fields.iter().filter(|f| f.required) {
                if r.get(&f.name).is_none() {
                    issues.push(issue(
                        r.line,
                        1,
                        format!("`{}` {} has no `{}`", def.name, r.id, f.name),
                    ));
                }
            }
        }
    }
    let exists = |table: &str, id: &str| {
        schema
            .table(table)
            .is_some_and(|t| ids.contains_key(&(t.kind.as_str(), id)))
    };
    // References are checked table by table, large tables side by side.
    let refs = |table: &Table| -> Vec<Issue> {
        let mut issues = Vec::new();
        let Some(def) = schema.table(&table.name) else {
            return issues;
        };
        for r in &table.records {
            for (name, value) in &r.fields {
                let Some(f) = def.field(name) else { continue };
                let missing =
                    |to: &str, id: &str| format!("{name}: no `{to}` record has the id `{id}`");
                if let Some((to, soft)) = ref_target(&f.ty) {
                    if let Some(id) = value.as_str()
                        && !soft
                        && !exists(to, id)
                    {
                        issues.push(issue(r.line, 1, missing(to, id)));
                    }
                } else if let FieldType::List { items, .. } = &f.ty {
                    for (pos, it) in items.iter().enumerate() {
                        let ItemType::Ref { to, soft: false } = it else {
                            continue;
                        };
                        for v in value.as_list().unwrap_or_default() {
                            let item = if items.len() == 1 {
                                Some(v)
                            } else {
                                v.as_list().and_then(|t| t.get(pos))
                            };
                            if let Some(id) = item.and_then(Value::as_str)
                                && !exists(to, id)
                            {
                                issues.push(issue(r.line, 1, missing(to, id)));
                            }
                        }
                    }
                }
            }
        }
        issues
    };
    let found: Vec<Vec<Issue>> = std::thread::scope(|s| {
        let refs = &refs;
        let tasks: Vec<_> = doc
            .tables
            .iter()
            .map(|t| {
                if t.records.len() >= 4096 {
                    Ok(s.spawn(move || refs(t)))
                } else {
                    Err(refs(t))
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
    issues.extend(found.into_iter().flatten());
}

// ---------------------------------------------------------------------------
// Writing.

/// Write a document in canonical form: the schema's tables in schema order,
/// then other tables by name; records in each table's natural-key order;
/// known fields in schema order, left out when they equal their default;
/// other fields after them by name. Two writers given the same document
/// write the same bytes.
///
/// Fails on a NaN, which has no text.
pub fn write(doc: &Document) -> Result<String, String> {
    let schema = doc.schema;
    let mut out = format!("{} {}\n", schema.format, doc.written_version());
    for c in &doc.preamble {
        push_comment(&mut out, c);
    }
    for (table, records) in canonical_tables(doc) {
        let def = schema.table(&table.name);
        out.push('\n');
        push_comment(&mut out, &table.name);
        for r in records {
            write_record(&mut out, &table.name, def, r)?;
        }
    }
    Ok(out)
}

/// The tables and records in canonical order: the schema's tables in
/// schema order, then other tables by name; records by the table's order
/// fields, then id. Empty tables are left out.
pub(crate) fn canonical_tables(doc: &Document) -> Vec<(&Table, Vec<&Record>)> {
    let schema = doc.schema;
    let mut out = Vec::new();
    let mut tables: Vec<&Table> = doc
        .tables
        .iter()
        .filter(|t| !t.records.is_empty())
        .collect();
    tables.sort_by(|a, b| {
        let key = |t: &Table| {
            (
                schema.position(&t.name).unwrap_or(usize::MAX),
                t.name.clone(),
            )
        };
        key(a).cmp(&key(b))
    });
    for table in tables {
        let mut records: Vec<&Record> = table.records.iter().collect();
        if let Some(def) = schema.table(&table.name)
            && !def.order.is_empty()
        {
            records.sort_by(|a, b| {
                for f in &def.order {
                    let (x, y) = (doc.value(&def.name, a, f), doc.value(&def.name, b, f));
                    let ord = match (x, y) {
                        (Some(x), Some(y)) => x.sort_cmp(y),
                        (x, y) => x.is_some().cmp(&y.is_some()),
                    };
                    if ord != std::cmp::Ordering::Equal {
                        return ord;
                    }
                }
                Value::Str(a.id.clone()).sort_cmp(&Value::Str(b.id.clone()))
            });
        }
        out.push((table, records));
    }
    out
}

fn push_comment(out: &mut String, c: &str) {
    for line in c.split('\n') {
        if line.is_empty() {
            out.push_str("#\n");
        } else {
            out.push_str("# ");
            out.push_str(line);
            out.push('\n');
        }
    }
}

fn has_nan(v: &Value) -> bool {
    match v {
        Value::Float(f) => f.is_nan(),
        Value::List(l) => l.iter().any(has_nan),
        _ => false,
    }
}

fn write_record(
    out: &mut String,
    table: &str,
    def: Option<&TableDef>,
    r: &Record,
) -> Result<(), String> {
    for c in &r.comments {
        push_comment(out, c);
    }
    out.push_str(table);
    out.push(' ');
    write_str(&r.id, out);
    let mut known: Vec<(usize, &str, &Value)> = Vec::new();
    let mut other: Vec<(&str, &Value)> = Vec::new();
    for (name, value) in &r.fields {
        if has_nan(value) {
            return Err(format!(
                "{table} {}: {name} is NaN, which has no text",
                r.id
            ));
        }
        match def.and_then(|d| d.position(name)) {
            Some(i) => {
                let f = &def.expect("def").fields[i];
                if f.default.as_ref() != Some(value) {
                    known.push((i, name, value));
                }
            }
            None => other.push((name, value)),
        }
    }
    known.sort_by_key(|(i, _, _)| *i);
    other.sort_by(|a, b| a.0.cmp(b.0));
    for (name, value) in known.iter().map(|(_, n, v)| (*n, *v)).chain(other) {
        out.push(' ');
        out.push_str(name);
        out.push('=');
        value.write(out);
    }
    out.push('\n');
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{read_case, read_lists};

    fn messages(text: &str) -> Vec<String> {
        read_case(text)
            .unwrap_err()
            .issues
            .iter()
            .map(|i| i.to_string())
            .collect()
    }

    #[test]
    fn reads_and_writes_canonically() {
        let text = "stannum-case 1.0\n\
            bus 205 number=205 kv=230 name=\"HILL 230\" vm_pu=1.0113\n\
            bus 101 number=101 type=ref   kv=13.8\n\
            # Rebuilt 2024.\n\
            line 101:205:1 to=205 from=101 x_pu=0.1 status=on rate1_mva=400 # trailing\n";
        let doc = read_case(text).unwrap();
        let out = write(&doc).unwrap();
        assert_eq!(
            out,
            "stannum-case 1.0\n\n# bus\n\
             bus 101 number=101 kv=13.8 type=ref\n\
             bus 205 number=205 name=\"HILL 230\" kv=230 vm_pu=1.0113\n\n# line\n\
             # Rebuilt 2024.\n# trailing\n\
             line 101:205:1 from=101 to=205 x_pu=0.1 rate1_mva=400\n"
        );
        // Canonical text reads back to the same text.
        assert_eq!(write(&read_case(&out).unwrap()).unwrap(), out);
    }

    #[test]
    fn lists_and_tuples() {
        let text = "stannum-case 1.0\nbus 1 number=1\nswitched_shunt 1:1 bus=1 blocks=[[2, 20], [1, -30.5, off]]\n";
        let doc = read_case(text).unwrap();
        let blocks = doc.records("switched_shunt")[0].get("blocks").unwrap();
        assert_eq!(blocks.to_text(), "[[2, 20], [1, -30.5, off]]");
        assert!(
            messages("stannum-case 1.0\nbus 1 number=1\nswitched_shunt 1:1 bus=1 blocks=[[2]]\n")
                [0]
            .contains("expected 2 to 3 values")
        );
    }

    #[test]
    fn problems_have_lines_and_columns() {
        let m = messages(
            "stannum-case 1.0\nbus 1 number=x\nbus 1 number=1\nload 7:1 bus=7\nbus 2 number=2 colour=red\nbus 3 type=bogus\n",
        );
        assert_eq!(
            m,
            [
                "line 2, column 14: number: expected a whole number, not `x`",
                "line 3, column 5: bus id `1` is already used on line 2",
                "line 4, column 1: bus: no `bus` record has the id `7`",
                "line 5, column 16: `bus` has no field `colour`",
                "line 6, column 1: `bus` 3 has no `number`",
                "line 6, column 12: type: expected one of pq, pv, ref, isolated, not `bogus`",
            ]
        );
    }

    #[test]
    fn header_and_versions() {
        assert!(messages("bus 1 number=1\n")[0].contains("first line must be `stannum-case 1.0`"));
        assert!(messages("stannum-case 2.0\n")[0].contains("newer than the 1.0"));
        assert!(read_case("stannum-case 1.0\n").is_ok());
        // Preview files still open; a newer minor keeps what it adds.
        assert!(read_case("stannum-case 0.1\n").is_ok());
        let doc = read_case("stannum-case 1.3\nbus 1 number=1 later=2\n").unwrap();
        assert!(write(&doc).unwrap().starts_with("stannum-case 1.3\n"));
        assert!(
            messages("stannum-case 1.0\nbus 1 number=1 later=2\n")[0].contains("no field `later`")
        );
        assert!(
            write(&read_case("stannum-case 0.1\n").unwrap())
                .unwrap()
                .starts_with("stannum-case 1.0\n")
        );
        assert!(read_lists("stannum-case 1.0\n").is_err());
    }

    #[test]
    fn extensions_and_unknown_tables_are_kept() {
        let text = "stannum-case 1.0\nbus 1 number=1 x.psse.cw=1 x.note=\"hand edit\"\nx.psse.gne G1 lines=[\"a, b\", \"c\"] n=0010\n";
        let doc = read_case(text).unwrap();
        let out = write(&doc).unwrap();
        assert!(out.contains("bus 1 number=1 x.note=\"hand edit\" x.psse.cw=1\n"));
        assert!(out.contains("x.psse.gne G1 lines=[\"a, b\", c] n=0010\n"));
        assert!(messages("stannum-case 1.0\nfoo 1\n")[0].contains("unknown table `foo`"));
    }

    #[test]
    fn default_ids_follow_the_template() {
        let doc = read_case("stannum-case 1.0\nbus 310 number=310\nbus 205 number=205\n").unwrap();
        let line = Record::new("")
            .with("from", Value::Str("310".into()))
            .with("to", Value::Str("205".into()))
            .with("ckt", Value::Str("2".into()));
        assert_eq!(doc.default_id("line", &line).as_deref(), Some("205:310:2"));
        let load = Record::new("").with("bus", Value::Str("310".into()));
        assert_eq!(doc.default_id("load", &load).as_deref(), Some("310:1"));
    }

    #[test]
    fn preamble_and_table_markers() {
        let text = "stannum-case 1.0\n# Summer peak 2027.\n\n# bus\nbus 1 number=1\n\n# A loose comment.\n\n# bus\n# Own comment.\nbus 2 number=2\n";
        let doc = read_case(text).unwrap();
        assert_eq!(doc.preamble, ["Summer peak 2027."]);
        assert_eq!(doc.records("bus")[0].comments, Vec::<String>::new());
        assert_eq!(doc.records("bus")[1].comments, ["bus", "Own comment."]);
    }
}
