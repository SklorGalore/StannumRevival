//! Records from one RAW revision rewritten for the other (33 and 35).
//!
//! Fields that keep their meaning but change name (branch ratings, a
//! switched shunt's regulated bus) are copied across. Fields revision 35
//! added get PSS/E's defaults going up, and are dropped going down with a
//! note whenever they held something. Records with no place in the other
//! revision are rewritten as their nearest equivalent:
//!
//! - 35 system switching devices become 33 branches whose circuit starts
//!   with `@` (breaker) or `*` (switch), as revision 33 marks them; 33
//!   branches marked that way become switching devices going up.
//! - A 35 bus with several switched shunts keeps the first; revision 33
//!   allows one per bus, so the rest become fixed shunts at their BINIT.
//! - 35 loads' distributed generation is netted from the load.

use super::{Doc, Item, Notes, Rec, int, is_zero, layout, num, text, value_f64};
use crate::diff::tokenizer::tokenize;
use crate::diff::value::{Number, Value};
use std::collections::{HashMap, HashSet};

pub(crate) fn convert(doc: &mut Doc, to: i64, notes: &mut Notes) {
    let from = doc.revision;
    if from == to {
        return;
    }
    // Records the reader keeps as lines but this conversion changes.
    decode_three_winding(doc, from, notes);
    if from == 35 {
        decode_single_line(doc, "SYSTEM SWITCHING DEVICE", from, notes);
    }
    if to == 33 {
        down(doc, notes);
    } else {
        up(doc, notes);
    }
    impedance_tables(doc, from, to, notes);
    doc.revision = to;
    if let Some((_, rev)) = doc.ident.iter_mut().find(|(n, _)| n == "REV") {
        *rev = int(to);
    }
}

fn lost(rec: &Rec, fields: &[&str], notes: &mut Notes, what: &str) {
    if fields.iter().any(|f| !is_zero(rec.get(f))) {
        notes.add(what.to_string());
    }
}

/// Rating names: revision 35 has 12 per winding, 33 has three (A, B, C).
fn branch_ratings_down(rec: &mut Rec, notes: &mut Notes) {
    for (old, new) in [("RATE1", "RATEA"), ("RATE2", "RATEB"), ("RATE3", "RATEC")] {
        rec.copy(old, new);
    }
    let extra: Vec<String> = (4..=12).map(|i| format!("RATE{i}")).collect();
    let extra: Vec<&str> = extra.iter().map(String::as_str).collect();
    lost(
        rec,
        &extra,
        notes,
        "Branch ratings 4 to 12 dropped: revision 33 has three",
    );
}

fn winding_ratings_down(rec: &mut Rec, windings: usize, notes: &mut Notes) {
    for w in 1..=windings {
        for (i, letter) in ["A", "B", "C"].iter().enumerate() {
            rec.copy(&format!("RATE{w}-{}", i + 1), &format!("RAT{letter}{w}"));
        }
        let extra: Vec<String> = (4..=12).map(|i| format!("RATE{w}-{i}")).collect();
        let extra: Vec<&str> = extra.iter().map(String::as_str).collect();
        lost(
            rec,
            &extra,
            notes,
            "Transformer ratings 4 to 12 dropped: revision 33 has three",
        );
        lost(
            rec,
            &[&format!("NOD{w}")],
            notes,
            "Transformer controlled node (NOD) dropped: revision 33 has none",
        );
    }
}

fn winding_ratings_up(rec: &mut Rec, windings: usize) {
    for w in 1..=windings {
        for (i, letter) in ["A", "B", "C"].iter().enumerate() {
            rec.copy(&format!("RAT{letter}{w}"), &format!("RATE{w}-{}", i + 1));
        }
        rec.lines = None;
    }
}

fn windings(rec: &Rec) -> usize {
    if is_zero(rec.get("K")) { 2 } else { 3 }
}

fn key(i: f64, j: f64, ckt: &str) -> (i64, i64, String) {
    let (a, b) = (i.abs() as i64, j.abs() as i64);
    (a.min(b), a.max(b), ckt.trim().to_uppercase())
}

fn recs(items: Vec<Item>) -> (Vec<Rec>, Vec<Item>) {
    let mut out = Vec::new();
    let mut kept = Vec::new();
    for item in items {
        match item {
            Item::Rec(r) => out.push(r),
            other => kept.push(other),
        }
    }
    (out, kept)
}

/// Revision 35 to 33.
fn down(doc: &mut Doc, notes: &mut Notes) {
    let system = doc.items("SYSTEM-WIDE").len();
    if system > 0 {
        notes.add(format!(
            "System-wide data dropped ({system} records: solution settings and rating names): revision 33 has none"
        ));
        doc.sections.remove("SYSTEM-WIDE");
    }
    for rec in doc.recs_mut("LOAD") {
        if rec.number("DGENM") != 0.0 && !(is_zero(rec.get("DGENP")) && is_zero(rec.get("DGENQ"))) {
            let (p, q) = (
                rec.number("PL") - rec.number("DGENP"),
                rec.number("QL") - rec.number("DGENQ"),
            );
            rec.set("PL", num(p));
            rec.set("QL", num(q));
            notes.add("Load distributed generation netted from PL and QL: revision 33 has no place for it");
        }
        lost(
            rec,
            &["LOADTYPE"],
            notes,
            "Load type dropped: revision 33 has none",
        );
        rec.lines = None;
    }
    for rec in doc.recs_mut("GENERATOR") {
        lost(
            rec,
            &["NREG"],
            notes,
            "Generator regulated node (NREG) dropped: revision 33 has none",
        );
        lost(
            rec,
            &["BASLOD"],
            notes,
            "Generator base load flag (BASLOD) dropped: revision 33 has none",
        );
        rec.lines = None;
    }
    for rec in doc.recs_mut("BRANCH") {
        branch_ratings_down(rec, notes);
        lost(
            rec,
            &["NAME"],
            notes,
            "Branch names dropped: revision 33 has none",
        );
        rec.lines = None;
    }
    // Switching devices become marked branches.
    let owners: HashMap<i64, Value> = doc
        .items("BUS")
        .iter()
        .filter_map(|i| match i {
            Item::Rec(r) => Some((r.number("I") as i64, r.get("OWNER").cloned()?)),
            Item::Lines(_) => None,
        })
        .collect();
    let mut taken: HashSet<(i64, i64, String)> = doc
        .items("BRANCH")
        .iter()
        .filter_map(|i| match i {
            Item::Rec(r) => Some(key(r.number("I"), r.number("J"), r.text("CKT"))),
            Item::Lines(_) => None,
        })
        .collect();
    let (devices, _) = recs(
        doc.sections
            .remove("SYSTEM SWITCHING DEVICE")
            .unwrap_or_default(),
    );
    for d in devices {
        let (i, j) = (d.number("I"), d.number("J"));
        let mark = if d.number("STYPE") == 2.0 { '@' } else { '*' };
        let first = d.text("CKT").trim().chars().next().unwrap_or('1');
        let ckt = std::iter::once(first)
            .chain("123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ".chars())
            .map(|c| format!("{mark}{c}"))
            .find(|c| !taken.contains(&key(i, j, c)))
            .unwrap_or_else(|| format!("{mark}{first}"));
        taken.insert(key(i, j, &ckt));
        let owner = owners
            .get(&(i.abs() as i64))
            .cloned()
            .unwrap_or_else(|| int(1));
        let mut b = Rec::new(vec![]);
        for name in ["I", "J", "X", "STAT", "MET"] {
            b.copy_from(&d, name);
        }
        b.set("CKT", text(&ckt));
        for (old, new) in [("RATE1", "RATEA"), ("RATE2", "RATEB"), ("RATE3", "RATEC")] {
            if let Some(v) = d.get(old) {
                b.set(new, v.clone());
            }
        }
        b.set("O1", owner);
        for f in ["F1", "F2", "F3", "F4"] {
            b.set(f, num(1.0));
        }
        notes.add("System switching devices written as branches with an @ (breaker) or * (switch) circuit, keeping their status but not their normal status (NSTAT): revision 33 has no switching device section");
        lost(
            &d,
            &["NAME"],
            notes,
            "Switching device names dropped: revision 33 has none",
        );
        doc.items_mut("BRANCH").push(Item::Rec(b));
    }
    for rec in doc.recs_mut("TRANSFORMER") {
        let w = windings(rec);
        winding_ratings_down(rec, w, notes);
        rec.lines = None;
    }
    for rec in doc.recs_mut("TWO-TERMINAL DC") {
        lost(
            rec,
            &["NDR", "NDI"],
            notes,
            "Two-terminal DC controlled nodes (NDR, NDI) dropped: revision 33 has none",
        );
        rec.lines = None;
    }
    // One switched shunt per bus in revision 33.
    let (shunts, kept) = recs(doc.sections.remove("SWITCHED SHUNT").unwrap_or_default());
    let mut seen = HashSet::new();
    let mut fixed_ids: HashSet<(i64, String)> = doc
        .items("FIXED SHUNT")
        .iter()
        .filter_map(|i| match i {
            Item::Rec(r) => Some((r.number("I") as i64, r.text("ID").trim().to_string())),
            Item::Lines(_) => None,
        })
        .chain(doc.items("FIXED SHUNT").iter().flat_map(|i| {
            match i {
                // Kept lines, one record each: the ID is the second field.
                Item::Lines(lines) => data_lines(lines)
                    .into_iter()
                    .filter_map(|l| {
                        let t = tokenize(l).ok()?;
                        Some((t.first()?.parse().ok()?, t.get(1)?.trim().to_string()))
                    })
                    .collect::<Vec<_>>(),
                Item::Rec(_) => vec![],
            }
        }))
        .collect();
    let mut out = kept;
    for mut s in shunts {
        let bus = s.number("I") as i64;
        if seen.insert(bus) {
            s.copy("SWREG", "SWREM");
            lost(
                &s,
                &["NREG"],
                notes,
                "Switched shunt regulated node (NREG) dropped: revision 33 has none",
            );
            // Blocks that are out of service are left out.
            let mut blocks = Vec::new();
            for g in 1..=8 {
                let (st, n, b) = (format!("S{g}"), format!("N{g}"), format!("B{g}"));
                if s.get(&n).is_none() {
                    break;
                }
                if s.get(&st).is_some_and(|v| value_f64(v) == 0.0) {
                    notes.add("Switched shunt blocks out of service left out: revision 33 has no block status");
                    continue;
                }
                blocks.push((
                    s.get(&n).cloned().unwrap(),
                    s.get(&b).cloned().unwrap_or(num(0.0)),
                ));
            }
            if blocks.is_empty() {
                blocks.push((int(0), num(0.0)));
            }
            let mut r = Rec::new(vec![]);
            for name in [
                "I", "MODSW", "ADJM", "STAT", "VSWHI", "VSWLO", "SWREM", "RMPCT", "RMIDNT", "BINIT",
            ] {
                r.copy_from(&s, name);
            }
            for (g, (n, b)) in blocks.into_iter().enumerate() {
                r.set(&format!("N{}", g + 1), n);
                r.set(&format!("B{}", g + 1), b);
            }
            out.push(Item::Rec(r));
        } else {
            let mut id = s.text("ID").trim().to_string();
            if id.is_empty() || fixed_ids.contains(&(bus, id.clone())) {
                id = (1..)
                    .map(|n| format!("S{n}"))
                    .find(|c| !fixed_ids.contains(&(bus, c.clone())))
                    .unwrap();
            }
            fixed_ids.insert((bus, id.clone()));
            let mut f = Rec::new(vec![]);
            f.copy_from(&s, "I");
            f.set("ID", text(&id));
            f.set("STATUS", s.get("STAT").cloned().unwrap_or(int(1)));
            f.set("GL", num(0.0));
            f.set("BL", s.get("BINIT").cloned().unwrap_or(num(0.0)));
            notes.add("Extra switched shunts on a bus written as fixed shunts at BINIT: revision 33 allows one switched shunt per bus");
            doc.items_mut("FIXED SHUNT").push(Item::Rec(f));
        }
    }
    doc.sections.insert("SWITCHED SHUNT".into(), out);
    if !doc.tail.is_empty() {
        notes.add("Substation (node-breaker) data dropped: revision 33 has none");
        doc.tail.clear();
    }
}

/// Revision 33 to 35.
fn up(doc: &mut Doc, notes: &mut Notes) {
    for rec in doc.recs_mut("LOAD") {
        rec.lines = None;
    }
    for rec in doc.recs_mut("GENERATOR") {
        rec.lines = None;
    }
    // Marked branches are switching devices in revision 35.
    let (branches, kept) = recs(doc.sections.remove("BRANCH").unwrap_or_default());
    let mut out = kept;
    let mut taken: HashSet<(i64, i64, String)> = HashSet::new();
    for mut b in branches {
        let ckt = b.text("CKT").trim().to_string();
        if let Some(mark) = ckt.chars().next().filter(|c| matches!(c, '@' | '*')) {
            let mut d = Rec::new(vec![]);
            for name in ["I", "J", "X", "STAT", "MET"] {
                d.copy_from(&b, name);
            }
            // The mark becomes the device type; the circuit loses it unless
            // that would repeat another device's.
            let (i, j) = (b.number("I"), b.number("J"));
            let bare = ckt[1..].trim().to_string();
            let ckt = if !bare.is_empty() && !taken.contains(&key(i, j, &bare)) {
                format!("{bare:<2}")
            } else {
                ckt
            };
            taken.insert(key(i, j, &ckt));
            d.set("CKT", text(&ckt));
            for (old, new) in [("RATEA", "RATE1"), ("RATEB", "RATE2"), ("RATEC", "RATE3")] {
                if let Some(v) = b.get(old) {
                    d.set(new, v.clone());
                }
            }
            d.copy_from_as(&b, "STAT", "NSTAT");
            d.set("STYPE", int(if mark == '@' { 2 } else { 3 }));
            lost(
                &b,
                &["R", "B", "GI", "BI", "GJ", "BJ", "LEN"],
                notes,
                "Resistance, charging and line shunts of @ and * branches dropped: revision 35 switching devices have only a reactance",
            );
            notes.add("Branches with an @ (breaker) or * (switch) circuit written as system switching devices");
            doc.items_mut("SYSTEM SWITCHING DEVICE").push(Item::Rec(d));
            continue;
        }
        for (old, new) in [("RATEA", "RATE1"), ("RATEB", "RATE2"), ("RATEC", "RATE3")] {
            b.copy(old, new);
        }
        b.lines = None;
        out.push(Item::Rec(b));
    }
    doc.sections.insert("BRANCH".into(), out);
    for rec in doc.recs_mut("TRANSFORMER") {
        let w = windings(rec);
        winding_ratings_up(rec, w);
    }
    for rec in doc.recs_mut("TWO-TERMINAL DC") {
        rec.lines = None;
    }
    for rec in doc.recs_mut("SWITCHED SHUNT") {
        rec.copy("SWREM", "SWREG");
        // N, B blocks gain an in-service status.
        let mut blocks = Vec::new();
        for g in 1..=8 {
            let Some(n) = rec.get(&format!("N{g}")).cloned() else {
                break;
            };
            blocks.push((n, rec.get(&format!("B{g}")).cloned().unwrap_or(num(0.0))));
        }
        for (g, (n, b)) in blocks.into_iter().enumerate() {
            rec.set(&format!("S{}", g + 1), int(1));
            rec.set(&format!("N{}", g + 1), n);
            rec.set(&format!("B{}", g + 1), b);
        }
        rec.set("ID", text("1 "));
        rec.lines = None;
    }
}

impl Rec {
    fn copy_from(&mut self, other: &Rec, name: &str) {
        self.copy_from_as(other, name, name);
    }

    fn copy_from_as(&mut self, other: &Rec, name: &str, to: &str) {
        if let Some(v) = other.get(name) {
            self.set(to, v.clone());
        }
    }
}

fn data_lines(lines: &[String]) -> Vec<&String> {
    lines
        .iter()
        .filter(|l| {
            let t = l.trim_start();
            !(t.is_empty() || t.starts_with("@!") || t.starts_with('/'))
        })
        .collect()
}

fn decode_tokens(tokens: &[String], names: &super::LineLayout) -> Option<super::Fields> {
    if tokens.len() != names.len() {
        return None;
    }
    tokens
        .iter()
        .zip(names)
        .map(|(t, (name, kind))| {
            let v = match kind.as_str() {
                "string" => Value::Text(t.clone()),
                "integer" => Value::Number(Number::parse_integer(t)?),
                _ => Value::Number(Number::parse_number(t)?),
            };
            Some((name.clone(), v))
        })
        .collect()
}

/// One-line records the reader keeps as lines (its layout for them is
/// from file headers only), as fields.
fn decode_single_line(doc: &mut Doc, section: &str, from: i64, notes: &mut Notes) {
    let Some(layout) = layout(section, from, false) else {
        return;
    };
    let [names] = layout.lines.as_slice() else {
        return;
    };
    let items = std::mem::take(doc.items_mut(section));
    let mut out = Vec::new();
    for item in items {
        let Item::Lines(lines) = item else {
            out.push(item);
            continue;
        };
        for line in data_lines(&lines) {
            match tokenize(line).ok().and_then(|t| decode_tokens(&t, names)) {
                Some(fields) => out.push(Item::Rec(Rec::new(fields))),
                None => {
                    notes.add(format!(
                        "A {section} record that doesn't match the expected layout was left out"
                    ));
                }
            }
        }
    }
    *doc.items_mut(section) = out;
}

/// Three-winding transformers, which the reader keeps as lines, as fields
/// in the schema's three-winding layout.
fn decode_three_winding(doc: &mut Doc, from: i64, notes: &mut Notes) {
    let Some(layout) = layout("TRANSFORMER", from, true) else {
        return;
    };
    for item in doc.items_mut("TRANSFORMER") {
        let Item::Lines(lines) = item else { continue };
        let data = data_lines(lines);
        let decoded = (data.len() == layout.lines.len())
            .then(|| {
                let mut fields = Vec::new();
                for (line, names) in data.iter().zip(&layout.lines) {
                    fields.extend(decode_tokens(&tokenize(line).ok()?, names)?);
                }
                Some(fields)
            })
            .flatten();
        match decoded {
            Some(fields) => *item = Item::Rec(Rec::new(fields)),
            None => notes.add(
                "A three-winding transformer that doesn't match the expected layout was written unchanged; check it",
            ),
        }
    }
}

/// Impedance correction tables: revision 33 lists up to 11 (T, F) points
/// on one line per table; revision 35 lists (T, Re F, Im F) points, six to
/// a line, each table ended by 0, 0, 0.
fn impedance_tables(doc: &mut Doc, from: i64, to: i64, notes: &mut Notes) {
    let Some(items) = doc.sections.get_mut("IMPEDANCE CORRECTION") else {
        return;
    };
    type Point = (String, String, String);
    let zero = |s: &str| s.parse::<f64>().is_ok_and(|v| v == 0.0);
    let mut tables: Vec<(String, Vec<Point>)> = Vec::new();
    for item in items.drain(..) {
        let Item::Lines(lines) = &item else { continue };
        let data = data_lines(lines);
        if from == 33 {
            for line in data {
                let Ok(tokens) = tokenize(line) else { continue };
                let Some((table, rest)) = tokens.split_first() else {
                    continue;
                };
                let points = rest
                    .chunks(2)
                    .map(|p| {
                        let t = p[0].clone();
                        let f = p.get(1).cloned().unwrap_or_else(|| "0.0".into());
                        (t, f, "0.0".to_string())
                    })
                    .take_while(|(t, f, _)| !(zero(t) && zero(f)))
                    .collect();
                tables.push((table.clone(), points));
            }
        } else {
            let tokens: Vec<String> = data
                .iter()
                .filter_map(|l| tokenize(l).ok())
                .flatten()
                .collect();
            let mut at = 0;
            while at < tokens.len() {
                let table = tokens[at].clone();
                at += 1;
                let mut points = Vec::new();
                while at < tokens.len() {
                    let p: Vec<String> = tokens[at..(at + 3).min(tokens.len())].to_vec();
                    at += 3;
                    let get = |i: usize| p.get(i).cloned().unwrap_or_else(|| "0.0".into());
                    let (t, re, im) = (get(0), get(1), get(2));
                    if zero(&t) && zero(&re) && zero(&im) {
                        break;
                    }
                    points.push((t, re, im));
                }
                tables.push((table, points));
            }
        }
    }
    let mut out = Vec::new();
    for (table, mut points) in tables {
        let lines = if to == 33 {
            if points.iter().any(|(_, _, im)| !zero(im)) {
                notes.add("Imaginary parts of impedance correction factors dropped: revision 33 factors are real");
            }
            if points.len() > 11 {
                notes.add("Impedance correction points after the 11th dropped: revision 33 tables hold 11");
                points.truncate(11);
            }
            let mut tokens = vec![table];
            for k in 0..11 {
                match points.get(k) {
                    Some((t, re, _)) => tokens.extend([t.clone(), re.clone()]),
                    None => tokens.extend(["0.0".to_string(), "0.0".to_string()]),
                }
            }
            vec![tokens.join(",")]
        } else {
            let mut triples: Vec<String> = points
                .iter()
                .map(|(t, re, im)| format!("{t},{re},{im}"))
                .collect();
            triples.push("0.0,0.0,0.0".into());
            triples
                .chunks(6)
                .enumerate()
                .map(|(i, chunk)| {
                    let body = chunk.join(",");
                    if i == 0 {
                        format!("{table},{body}")
                    } else {
                        body
                    }
                })
                .collect()
        };
        notes.add(format!(
            "Impedance correction tables rewritten for revision {to} (layout from the PSS/E manual; no example file to check it against)"
        ));
        out.push(Item::Lines(lines));
    }
    *items = out;
}
