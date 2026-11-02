//! Carrying what only a file holds onto a new document of the same case.
//!
//! A tool may rebuild a case from another form (Stannum converts to RAW to
//! solve and edit, then back). The rebuilt document has the network, but
//! not what only the original file held: record ids that differ from the
//! natural key (a renumbered bus keeps its id), comments, and extensions.
//! [`Document::carry_from`] puts them back, matching records by natural key.

use crate::document::{Document, Record};
use crate::schema::{FieldType, ItemType};
use crate::value::Value;
use std::collections::HashMap;

/// Per kind: the rebuilt document's id → the original file's id.
type IdMaps = HashMap<String, HashMap<String, String>>;

/// The fields an id template reads: `{a}` and `{sort(a,b)}` parts.
fn id_fields(template: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest = template;
    while let Some(open) = rest.find('{') {
        let Some(close) = rest[open..].find('}') else {
            break;
        };
        let inner = &rest[open + 1..open + close];
        let inner = inner
            .strip_prefix("sort(")
            .and_then(|s| s.strip_suffix(')'))
            .unwrap_or(inner);
        out.extend(inner.split(',').map(|f| f.trim().to_string()));
        rest = &rest[open + close + 1..];
    }
    out
}

/// Whether an extension is carried: `x.` fields and tables, except those a
/// converter writes itself (`x.psse`), which the rebuilt document already has.
fn carried(name: &str) -> bool {
    name.starts_with("x.") && !name.starts_with("x.psse.") && name != "x.psse"
}

impl Document {
    /// Take ids, comments and extensions from `source`, an earlier file of
    /// the same case, for records with the same natural key. References
    /// are renamed with the ids. Records only `source` has stay out; records
    /// only this document get their natural ids, made from the renamed ids
    /// they refer to.
    pub fn carry_from(&mut self, source: &Document) {
        // Natural key → id in the source, per table.
        let mut source_keys: HashMap<&str, HashMap<String, &Record>> = HashMap::new();
        for t in &source.tables {
            let keys = source_keys.entry(t.name.as_str()).or_default();
            for r in &t.records {
                if let Some(key) = source.default_id(&t.name, r) {
                    keys.insert(key, r);
                }
            }
        }
        // A table's ids may hold other records' ids (a load's holds its
        // bus's), so tables are matched once the tables their ids refer to
        // are, and references to a table are renamed once, right after.
        let mut settled: Vec<String> = Vec::new();
        loop {
            let ready: Vec<String> = self
                .tables
                .iter()
                .map(|t| t.name.clone())
                .filter(|name| !settled.contains(name))
                .filter(|name| {
                    self.schema.table(name).is_some_and(|def| {
                        id_fields(&def.id)
                            .iter()
                            .all(|f| match def.field(f).map(|f| &f.ty) {
                                Some(FieldType::Ref { to, .. }) => {
                                    settled.contains(to) || self.table(to).is_none()
                                }
                                _ => true,
                            })
                    })
                })
                .collect();
            if ready.is_empty() {
                break;
            }
            let mut renamed: IdMaps = HashMap::new();
            for name in &ready {
                let def = self.schema.table(name).unwrap();
                let empty = HashMap::new();
                let keys = source_keys.get(name.as_str()).unwrap_or(&empty);
                let i = self.tables.iter().position(|t| &t.name == name).unwrap();
                for j in 0..self.tables[i].records.len() {
                    let r = &self.tables[i].records[j];
                    let Some(key) = self.default_id(name, r) else {
                        continue;
                    };
                    // The source's id; a new record gets its natural id with
                    // the ids it refers to renamed.
                    let id = match keys.get(&key) {
                        Some(src) => src.id.clone(),
                        None => key,
                    };
                    if r.id != id {
                        let old = std::mem::replace(&mut self.tables[i].records[j].id, id.clone());
                        renamed.entry(def.kind.clone()).or_default().insert(old, id);
                    }
                }
            }
            self.rename_refs(&renamed);
            settled.extend(ready);
        }

        // Comments and extensions, by id.
        if self.preamble.is_empty() {
            self.preamble = source.preamble.clone();
        }
        for t in &mut self.tables {
            let Some(st) = source.table(&t.name) else {
                continue;
            };
            let by_id: HashMap<&str, &Record> =
                st.records.iter().map(|r| (r.id.as_str(), r)).collect();
            for r in &mut t.records {
                let Some(s) = by_id.get(r.id.as_str()) else {
                    continue;
                };
                if r.comments.is_empty() {
                    r.comments = s.comments.clone();
                }
                for (name, value) in &s.fields {
                    if carried(name) && r.get(name).is_none() {
                        r.fields.push((name.clone(), value.clone()));
                    }
                }
            }
        }
        for st in &source.tables {
            if carried(&st.name) && self.table(&st.name).is_none() {
                self.tables.push(st.clone());
            }
        }
    }

    /// Rename references by the maps: ref fields, typed references, and
    /// lists of them.
    fn rename_refs(&mut self, maps: &IdMaps) {
        if maps.is_empty() {
            return;
        }
        let schema = self.schema;
        let kind_of = |table: &str| schema.table(table).map(|t| t.kind.as_str());
        let rename_element = |v: &mut Value| {
            if let Value::Str(s) = v
                && let Some((kind, id)) = s.split_once(':')
                && let Some(new) = maps.get(kind).and_then(|m| m.get(id))
            {
                *s = format!("{kind}:{new}");
            }
        };
        for t in &mut self.tables {
            let Some(def) = schema.table(&t.name) else {
                continue;
            };
            for r in &mut t.records {
                for (name, value) in &mut r.fields {
                    let Some(f) = def.field(name) else {
                        continue;
                    };
                    match &f.ty {
                        FieldType::Ref { to, .. } => {
                            if let (Some(kind), Value::Str(s)) = (kind_of(to), &mut *value)
                                && let Some(new) = maps.get(kind).and_then(|m| m.get(s.as_str()))
                            {
                                *s = new.clone();
                            }
                        }
                        FieldType::Element => rename_element(value),
                        FieldType::List { items, .. } => {
                            let Value::List(list) = value else {
                                continue;
                            };
                            for entry in list.iter_mut() {
                                let parts: Vec<&mut Value> = match entry {
                                    Value::List(tuple) if items.len() > 1 => {
                                        tuple.iter_mut().collect()
                                    }
                                    other => vec![other],
                                };
                                for (item, part) in items.iter().zip(parts) {
                                    match item {
                                        ItemType::Ref { to, .. } => {
                                            if let (Some(kind), Value::Str(s)) =
                                                (kind_of(to), &mut *part)
                                                && let Some(new) =
                                                    maps.get(kind).and_then(|m| m.get(s.as_str()))
                                            {
                                                *s = new.clone();
                                            }
                                        }
                                        ItemType::Element => rename_element(part),
                                        _ => {}
                                    }
                                }
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::{read_case, write};

    #[test]
    fn ids_comments_and_extensions_come_back() {
        let source = read_case(
            "stannum-case 1.0\n# Planning case.\n\n\
             # Renumbered from 101 in 2026.\n\
             bus north number=105 kv=138 x.note=\"keep\"\n\
             bus 200 number=200 kv=138\n\
             bus 300 number=300 kv=138\n\
             area 1 number=1 slack_bus=north\n\
             load north:1 bus=north p_mw=10\n\
             line tie from=north to=200 x_pu=0.1\n\
             line 200:300:1 from=200 to=300 x_pu=0.1\n\
             x.planning.project P1 buses=[105]\n",
        )
        .unwrap();
        // As rebuilt from RAW: natural ids, no comments, a load changed and
        // the 200-300 line gone.
        let mut rebuilt = read_case(
            "stannum-case 1.0\n\
             bus 105 number=105 kv=138\n\
             bus 200 number=200 kv=138\n\
             bus 300 number=300 kv=138\n\
             area 1 number=1 slack_bus=105\n\
             load 105:1 bus=105 p_mw=12\n\
             line 105:200:1 from=105 to=200 x_pu=0.1\n\
             line 105:300:1 from=105 to=300 x_pu=0.2\n",
        )
        .unwrap();
        rebuilt.carry_from(&source);
        let text = write(&rebuilt).unwrap();
        assert_eq!(
            text,
            "stannum-case 1.0\n# Planning case.\n\n\
             # area\narea 1 number=1 slack_bus=north\n\n\
             # bus\n# Renumbered from 101 in 2026.\n\
             bus north number=105 kv=138 x.note=keep\n\
             bus 200 number=200 kv=138\nbus 300 number=300 kv=138\n\n\
             # load\nload north:1 bus=north p_mw=12\n\n\
             # line\nline tie from=north to=200 x_pu=0.1\nline 300:north:1 from=north to=300 x_pu=0.2\n\n\
             # x.planning.project\nx.planning.project P1 buses=[105]\n"
        );
    }
}
