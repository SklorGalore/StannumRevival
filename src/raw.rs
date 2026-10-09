//! PSS/E RAW conversion following MATPOWER 8.1 psse_read/parse/convert.
//! See NOTICE for upstream attribution and licensing.
use crate::{Branch, Bus, BusType, Case, Error, Generator};
use std::{collections::HashMap, path::Path};

/// Options for RAW import. Defaults to MATPOWER-style conversion with warnings.
#[derive(Clone, Debug, Default)]
pub struct RawOptions {
    /// Override revision detection (supported revisions: 23 through 33, and 35).
    pub revision: Option<u32>,
    /// Reject lossy approximations and ignored equipment instead of warning.
    pub strict: bool,
}

/// A source-located diagnostic about RAW conversion.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RawWarning {
    /// One-based source line, or the header line for case-wide diagnostics.
    pub line: usize,
    /// Description of the approximation or ignored input.
    pub message: String,
}

/// Converted case, effective revision, and import diagnostics.
#[derive(Clone, Debug, PartialEq)]
pub struct RawImport {
    /// Converted data; network validation occurs when solving.
    pub case: Case,
    /// Revision used after applying detection or the explicit override.
    pub revision: u32,
    /// Diagnostics; strict mode may still return non-lossy warnings.
    pub warnings: Vec<RawWarning>,
}

struct Context<'a> {
    options: &'a RawOptions,
    warnings: Vec<RawWarning>,
}
impl Context<'_> {
    fn warn(&mut self, line: usize, lossy: bool, message: impl Into<String>) -> Result<(), Error> {
        let message = message.into();
        if lossy && self.options.strict {
            log::error!("strict RAW conversion rejected line {line}: {message}");
            return Err(Error::Unsupported(format!("RAW line {line}: {message}")));
        }
        log::warn!("RAW conversion warning at line {line} (lossy={lossy}): {message}");
        self.warnings.push(RawWarning { line, message });
        Ok(())
    }
}

struct Record {
    line: usize,
    fields: Vec<String>,
    comment: String,
}
impl Record {
    fn error(&self, message: impl Into<String>) -> Error {
        Error::Parse {
            line: self.line,
            column: 1,
            message: message.into(),
        }
    }
    fn text(&self, i: usize) -> Result<&str, Error> {
        self.fields
            .get(i)
            .map(String::as_str)
            .ok_or_else(|| self.error(format!("missing RAW field {}", i + 1)))
    }
    fn num(&self, i: usize) -> Result<f64, Error> {
        let n = self
            .text(i)?
            .trim()
            .replace(['d', 'D'], "E")
            .parse::<f64>()
            .map_err(|_| self.error(format!("invalid number in RAW field {}", i + 1)))?;
        if !n.is_finite() {
            return Err(self.error("RAW numbers must be finite"));
        }
        Ok(n)
    }
    fn optional(&self, i: usize, default: f64) -> Result<f64, Error> {
        if self.fields.get(i).is_none_or(|s| s.trim().is_empty()) {
            Ok(default)
        } else {
            self.num(i)
        }
    }
    fn id(&self, i: usize) -> Result<u32, Error> {
        self.integer(i, false)
    }
    fn integer(&self, i: usize, absolute: bool) -> Result<u32, Error> {
        let n = self.num(i)?;
        let n = if absolute { n.abs() } else { n };
        if n < 0.0 || n > u32::MAX as f64 || n.fract() != 0.0 {
            return Err(self.error("invalid RAW integer identifier"));
        }
        Ok(n as u32)
    }
    fn status(&self, i: usize) -> Result<bool, Error> {
        match self.num(i)? {
            0.0 => Ok(false),
            1.0 => Ok(true),
            _ => Err(self.error("RAW status must be 0 or 1")),
        }
    }
}

// Commas and whitespace may be mixed. A comma after a whitespace-separated
// value terminates that value; successive commas represent omitted fields.
fn record(line: usize, input: &str) -> Result<Record, Error> {
    let mut r = Record {
        line,
        fields: vec![],
        comment: String::new(),
    };
    let mut chars = input.chars().peekable();
    let mut after_comma = true;
    while let Some(&c) = chars.peek() {
        if c.is_whitespace() {
            chars.next();
            continue;
        }
        if c == '@' && chars.clone().nth(1) == Some('!') {
            chars.next();
            chars.next();
            r.comment = chars.collect();
            break;
        }
        if c == '/' {
            chars.next();
            r.comment = chars.collect();
            break;
        }
        if c == ',' {
            if after_comma {
                r.fields.push(String::new());
            }
            chars.next();
            after_comma = true;
            continue;
        }
        let mut field = String::new();
        if c == '\'' || c == '"' {
            chars.next();
            loop {
                let ch = chars
                    .next()
                    .ok_or_else(|| r.error("unterminated RAW string"))?;
                if ch == c {
                    if chars.peek() == Some(&c) {
                        chars.next();
                        field.push(c);
                    } else {
                        break;
                    }
                } else {
                    field.push(ch);
                }
            }
            if chars
                .peek()
                .is_some_and(|ch| !ch.is_whitespace() && !matches!(ch, ',' | '/'))
            {
                return Err(r.error("expected separator after quoted RAW field"));
            }
        } else {
            while let Some(&ch) = chars.peek() {
                if ch.is_whitespace()
                    || matches!(ch, ',' | '/')
                    || (ch == '@' && chars.clone().nth(1) == Some('!'))
                {
                    break;
                }
                let ch = chars.next().unwrap();
                if matches!(ch, '\'' | '"') {
                    return Err(r.error("unexpected quote in RAW field"));
                }
                field.push(ch);
            }
        }
        r.fields.push(field);
        after_comma = false;
    }
    if after_comma && !r.fields.is_empty() {
        r.fields.push(String::new());
    }
    Ok(r)
}

fn section_names(rev: u32) -> Vec<&'static str> {
    let mut names = vec!["bus"];
    if rev >= 24 {
        names.push("load");
    }
    if rev >= 31 {
        names.push("fixed shunt");
    }
    names.extend(["generator", "branch"]);
    if rev >= 35 {
        names.push("system switching device");
    }
    names.extend([
        if rev <= 27 {
            "transformer adjustment"
        } else {
            "transformer"
        },
        "area",
        "two-terminal DC",
    ]);
    if rev >= 29 {
        names.push("voltage source converter");
    }
    if rev <= 30 {
        names.push("switched shunt");
    }
    names.extend([
        "impedance correction",
        "multi-terminal DC",
        "multi-section line",
        "zone",
        "inter-area transfer",
    ]);
    if rev >= 25 {
        names.push("owner");
    }
    if rev >= 26 {
        names.push("FACTS control device");
    }
    if rev >= 31 {
        names.push("switched shunt");
    }
    if rev >= 32 {
        names.push("GNE device");
    }
    if rev >= 33 {
        names.push("induction machine");
    }
    names
}

/// Load RAW using MATPOWER-style conversions. Use `load_raw_with_options` to
/// obtain warnings about approximations and ignored data.
pub fn load_raw(path: impl AsRef<Path>) -> Result<Case, Error> {
    Ok(load_raw_with_options(path, &RawOptions::default())?.case)
}
/// Load a RAW file with revision control and conversion diagnostics.
///
/// Invalid UTF-8 bytes are replaced with the Unicode replacement character.
/// Returns [`Error::Io`] on read failure, or errors from [`parse_raw_with_options`].
pub fn load_raw_with_options(
    path: impl AsRef<Path>,
    options: &RawOptions,
) -> Result<RawImport, Error> {
    let path = path.as_ref();
    log::info!(
        "loading PSS/E RAW case from {} (strict={})",
        path.display(),
        options.strict
    );
    let bytes = std::fs::read(path)?;
    log::debug!("read {} bytes from {}", bytes.len(), path.display());
    parse_raw_with_options(&String::from_utf8_lossy(&bytes), options)
}
/// Parse RAW revisions 23–33 and 35 using MATPOWER-style conversions. Warnings are
/// available through `parse_raw_with_options`; this convenience API drops them.
pub fn parse_raw(input: &str) -> Result<Case, Error> {
    Ok(parse_raw_with_options(input, &RawOptions::default())?.case)
}

/// Parse RAW revisions 23–33 and 35 with conversion diagnostics.
///
/// Revision detection falls back to 23 when neither header nor comment identifies
/// a revision. [`RawOptions::revision`] overrides detection. Strict mode rejects
/// lossy conversions with [`Error::Unsupported`]; malformed records return
/// source-located [`Error::Parse`] errors. Unsupported revisions are rejected.
/// The resulting case still needs network validation by [`crate::solve`].
pub fn parse_raw_with_options(input: &str, options: &RawOptions) -> Result<RawImport, Error> {
    log::debug!(
        "parsing PSS/E RAW case ({} bytes, strict={}, revision override={:?})",
        input.len(),
        options.strict,
        options.revision
    );
    let normalized = input
        .trim_start_matches('\u{feff}')
        .replace("\r\n", "\n")
        .replace('\r', "\n");
    let mut lines = normalized.lines().enumerate();
    let (header_index, header_text) = lines
        .find(|(_, text)| !text.trim().is_empty() && !text.trim_start().starts_with("@!"))
        .ok_or_else(|| Error::InvalidCase("empty RAW file".into()))?;
    let header_line = header_index + 1;
    let header = record(header_line, header_text)?;
    let mut ctx = Context {
        options,
        warnings: vec![],
    };
    if header.num(0)? != 0.0 {
        ctx.warn(
            header_line,
            true,
            "IC is nonzero: change cases are not applied as incremental updates",
        )?;
    }
    let detected = if header.fields.get(2).is_some_and(|s| !s.is_empty()) {
        Some(header.id(2)?)
    } else {
        ["PSS/E-", "PSS(tm)E-"].iter().find_map(|marker| {
            header.comment.split_once(marker).and_then(|(_, tail)| {
                tail.chars()
                    .take_while(char::is_ascii_digit)
                    .collect::<String>()
                    .parse()
                    .ok()
            })
        })
    };
    let rev = options
        .revision
        .or(detected.filter(|rev| *rev != 0))
        .unwrap_or(23);
    if !((23..=33).contains(&rev) || rev == 35) {
        return Err(Error::Unsupported(format!(
            "RAW revision {rev}; supported revisions are 23–33 and 35"
        )));
    }
    log::debug!("selected RAW revision {rev} (detected={detected:?})");
    if options.revision.is_some() {
        ctx.warn(
            header_line,
            false,
            format!("explicitly using RAW revision {rev}"),
        )?;
    } else if detected.is_none_or(|rev| rev == 0) {
        ctx.warn(header_line, false, "unknown revision; using revision 23")?;
    }
    let base_mva = header.num(1)?;
    if base_mva <= 0.0 {
        return Err(header.error("base MVA must be positive"));
    }
    for _ in 0..2 {
        lines
            .next()
            .ok_or_else(|| header.error("missing RAW title records"))?;
    }
    let names = section_names(rev);
    let mut sections: Vec<Vec<Record>> = vec![];
    let mut current = vec![];
    let mut system_wide = rev >= 35;
    for (i, text) in lines {
        // Q terminates input, including any trailing sequence data.
        if text
            .trim()
            .split('/')
            .next()
            .is_some_and(|s| s.trim().eq_ignore_ascii_case("Q"))
        {
            if !current.is_empty() {
                sections.push(std::mem::take(&mut current));
            }
            break;
        }
        if text.trim_start().starts_with("@!") {
            continue;
        }
        let r = record(i + 1, text)?;
        if r.fields.is_empty() {
            continue;
        }
        if r.fields.len() == 1 && r.fields[0] == "0" {
            if system_wide {
                system_wide = false;
                continue;
            }
            sections.push(std::mem::take(&mut current));
        } else if !system_wide {
            current.push(r);
        }
    }
    if !current.is_empty() {
        return Err(current[0].error("unterminated RAW section"));
    }
    let required = names.iter().position(|s| *s == "branch").unwrap() + 1;
    if sections.len() < required {
        return Err(header.error("missing required RAW sections through branch data"));
    }
    log::debug!("parsed {} RAW sections", sections.len());
    let section = |name: &str| -> &[Record] {
        names
            .iter()
            .position(|s| *s == name)
            .and_then(|i| sections.get(i))
            .map(Vec::as_slice)
            .unwrap_or(&[])
    };
    let mut case = Case {
        base_mva,
        buses: vec![],
        generators: vec![],
        branches: vec![],
        dcline: None,
        gencost: None,
        labels: Default::default(),
    };
    let bus_rows = section("bus");
    let have_limits = rev >= 31
        && bus_rows.iter().all(|r| {
            [9, 10]
                .iter()
                .all(|i| r.fields.get(*i).is_some_and(|s| !s.is_empty()))
        });
    if !have_limits {
        ctx.warn(
            1,
            false,
            "using default bus voltage limits: VMIN=0.9, VMAX=1.1",
        )?;
    }
    let mut buses = vec![];
    for r in bus_rows {
        let (kind, name, kv, area, zone, vm, va) = if rev == 23 {
            (1, 9, 10, 6, 11, 7, 8)
        } else if rev <= 30 {
            (3, 1, 2, 6, 7, 8, 9)
        } else {
            (3, 1, 2, 4, 5, 7, 8)
        };
        let id = r.id(0)?;
        if id == 0 {
            return Err(r.error("bus ID must be positive"));
        }
        let b = Bus {
            id,
            kind: match r.id(kind)? {
                1 => BusType::Pq,
                2 => BusType::Pv,
                3 => BusType::Reference,
                4 => BusType::Isolated,
                _ => return Err(r.error("invalid bus type")),
            },
            base_kv: r.num(kv)?,
            area: r.id(area)?,
            zone: r.id(zone)?,
            vm: r.num(vm)?,
            va: r.num(va)?,
            vmax: if have_limits { r.num(9)? } else { 1.1 },
            vmin: if have_limits { r.num(10)? } else { 0.9 },
            pd: if rev == 23 { r.num(2)? } else { 0.0 },
            qd: if rev == 23 { r.num(3)? } else { 0.0 },
            gs: if rev <= 30 { r.num(4)? } else { 0.0 },
            bs: if rev <= 30 { r.num(5)? } else { 0.0 },
            extra: vec![],
        };
        buses.push((b, r.text(name)?.to_owned()));
    }
    buses.sort_by_key(|(b, _)| b.id);
    let mut indices = HashMap::new();
    let mut bus_names = vec![];
    for (b, name) in buses {
        if indices.insert(b.id, case.buses.len()).is_some() {
            return Err(header.error("duplicate RAW bus ID"));
        }
        bus_names.push(name);
        case.buses.push(b);
    }
    case.labels.insert("bus_name".into(), bus_names);
    let bus_index = |r: &Record, field| -> Result<usize, Error> {
        indices
            .get(&r.id(field)?)
            .copied()
            .ok_or_else(|| r.error("unknown bus reference"))
    };
    for r in section("load") {
        let b = &mut case.buses[bus_index(r, 0)?];
        if r.status(2)? {
            let ip = r.optional(7, 0.0)?;
            let iq = r.optional(8, 0.0)?;
            let yp = r.optional(9, 0.0)?;
            let yq = r.optional(10, 0.0)?;
            if [ip, iq, yp, yq].iter().any(|n| *n != 0.0) {
                ctx.warn(
                    r.line,
                    true,
                    "ZIP load converted to constant power at the input bus voltage",
                )?;
            }
            b.pd += r.num(5)? + ip * b.vm + yp * b.vm * b.vm;
            b.qd += r.num(6)? + iq * b.vm - yq * b.vm * b.vm;
        }
    }
    for r in section("fixed shunt") {
        let b = &mut case.buses[bus_index(r, 0)?];
        if r.status(2)? {
            b.gs += r.num(3)?;
            b.bs += r.num(4)?;
        }
    }
    for r in section("switched shunt") {
        let b = &mut case.buses[bus_index(r, 0)?];
        if r.optional(1, 0.0)? != 0.0 {
            ctx.warn(
                r.line,
                true,
                "switched shunt frozen at BINIT; switching controls ignored",
            )?;
        }
        if rev >= 32 && !r.status(if rev >= 35 { 4 } else { 3 })? {
            ctx.warn(
                r.line,
                true,
                "switched shunt status ignored when applying BINIT, matching MATPOWER",
            )?;
        }
        let col = if rev >= 35 {
            11
        } else if rev <= 27 {
            5
        } else if rev <= 29 {
            6
        } else if rev <= 31 {
            7
        } else {
            9
        };
        b.bs += r.num(col)?;
    }
    for r in section("generator") {
        bus_index(r, 0)?;
        if r.optional(7, 0.0)? != 0.0 && r.optional(7, 0.0)? != r.num(0)? {
            ctx.warn(
                r.line,
                true,
                "remote generator regulation ignored; voltage setpoint applied locally",
            )?;
        }
        let shift = usize::from(rev >= 35);
        if r.optional(26 + shift, 0.0)? != 0.0 {
            ctx.warn(
                r.line,
                true,
                "wind-machine control ignored; supplied generator limits retained",
            )?;
        }
        case.generators.push(Generator {
            bus: r.id(0)?,
            pg: r.num(2)?,
            qg: r.num(3)?,
            qmax: r.num(4)?,
            qmin: r.num(5)?,
            vg: r.num(6)?,
            mbase: r.num(8 + shift)?,
            online: r.status(14 + shift)?,
            pmax: r.num(16 + shift)?,
            pmin: r.num(17 + shift)?,
            extra: vec![0.0; 11],
        });
    }
    for r in section("branch") {
        let from = r.id(0)?;
        let to = r.integer(1, true)?;
        let online = r.status(if rev >= 35 {
            23
        } else if rev <= 27 {
            15
        } else {
            13
        })?;
        let shunt = if rev >= 35 {
            19
        } else if rev <= 27 {
            11
        } else {
            9
        };
        for (id, col) in [(from, shunt), (to, shunt + 2)] {
            if let Some(&i) = indices.get(&id) {
                if online {
                    case.buses[i].gs += r.optional(col, 0.0)? * base_mva;
                    case.buses[i].bs += r.optional(col + 1, 0.0)? * base_mva;
                }
            } else {
                ctx.warn(
                    r.line,
                    false,
                    format!("branch references unknown bus {id}; branch retained"),
                )?;
            }
        }
        case.branches.push(Branch {
            from,
            to,
            r: r.num(3)?,
            x: r.num(4)?,
            b: r.num(5)?,
            rate_a: r.num(if rev >= 35 { 7 } else { 6 })?,
            rate_b: r.num(if rev >= 35 { 8 } else { 7 })?,
            rate_c: r.num(if rev >= 35 { 9 } else { 8 })?,
            tap: if rev <= 27 { r.optional(9, 0.0)? } else { 0.0 },
            shift: if rev <= 27 { r.optional(10, 0.0)? } else { 0.0 },
            online,
            angmin: -360.0,
            angmax: 360.0,
            extra: vec![],
        });
    }
    convert_transformers(section("transformer"), &mut case, &indices, &mut ctx)?;
    convert_dc(section("two-terminal DC"), &mut case, &indices, &mut ctx)?;
    for (i, rows) in sections.iter().enumerate() {
        let name = names.get(i).copied().unwrap_or("additional section");
        if !matches!(
            name,
            "bus"
                | "load"
                | "fixed shunt"
                | "switched shunt"
                | "generator"
                | "branch"
                | "transformer"
                | "two-terminal DC"
                | "area"
        ) && let Some(r) = rows.first()
        {
            ctx.warn(
                r.line,
                !matches!(
                    name,
                    "zone" | "owner" | "multi-section line" | "inter-area transfer"
                ),
                format!("skipped {} records of {name} data", rows.len()),
            )?;
        }
    }
    log::info!(
        "converted RAW revision {rev}: {} buses, {} generators, {} branches, {} warnings",
        case.buses.len(),
        case.generators.len(),
        case.branches.len(),
        ctx.warnings.len()
    );
    Ok(RawImport {
        case,
        revision: rev,
        warnings: ctx.warnings,
    })
}

fn impedance(
    z: &Record,
    offset: usize,
    code: u32,
    nominal: f64,
    bus: &Bus,
    base: f64,
) -> Result<(f64, f64), Error> {
    let mut r = z.num(offset)?;
    let mut x = z.num(offset + 1)?;
    if code != 1 {
        let mva = z.num(offset + 2)?;
        if mva <= 0.0 || bus.base_kv <= 0.0 {
            return Err(z.error("positive transformer MVA and bus kV bases required"));
        }
        if code == 3 {
            r *= 1e-6 / mva;
            if x < r.abs() {
                return Err(z.error("transformer impedance magnitude is smaller than resistance"));
            }
            x = (x * x - r * r).sqrt();
        }
        // MATPOWER 8.1 defaults to use_winding_baseV=1, including NOMV=0.
        let factor = nominal.powi(2) / mva / (bus.base_kv.powi(2) / base);
        r *= factor;
        x *= factor;
    }
    if !r.is_finite() || !x.is_finite() {
        return Err(z.error("nonfinite converted transformer impedance"));
    }
    Ok((r, x))
}

fn winding(w: &Record, code: u32, bus: &Bus) -> Result<f64, Error> {
    let mut v = w.num(0)?;
    if code != 1 {
        if bus.base_kv <= 0.0 {
            return Err(w.error("positive bus base kV required"));
        }
        v /= bus.base_kv;
        if code == 3 {
            v *= w.num(1)?;
        }
    }
    if v < 0.0 || !v.is_finite() {
        return Err(w.error("invalid winding voltage"));
    }
    Ok(v)
}

fn transformer_branch(
    r: &Record,
    w: &Record,
    from: u32,
    to: u32,
    z: (f64, f64),
    tap: f64,
    online: bool,
) -> Result<Branch, Error> {
    if !tap.is_finite() {
        return Err(r.error("nonfinite transformer tap ratio"));
    }
    Ok(Branch {
        from,
        to,
        r: z.0,
        x: z.1,
        b: 0.0,
        rate_a: w.num(3)?,
        rate_b: w.num(4)?,
        rate_c: w.num(5)?,
        tap,
        shift: w.num(2)?,
        online,
        angmin: -360.0,
        angmax: 360.0,
        extra: vec![],
    })
}

fn convert_transformers(
    rows: &[Record],
    case: &mut Case,
    indices: &HashMap<u32, usize>,
    ctx: &mut Context<'_>,
) -> Result<(), Error> {
    let mut groups = [vec![], vec![]];
    let mut pos = 0;
    while pos < rows.len() {
        let three = rows[pos].id(2)? != 0;
        let n = if three { 5 } else { 4 };
        let group = rows
            .get(pos..pos + n)
            .ok_or_else(|| rows[pos].error("truncated transformer"))?;
        groups[usize::from(three)].push(group);
        pos += n;
    }
    let max = case
        .buses
        .iter()
        .map(|b| u64::from(b.id))
        .max()
        .unwrap_or(0);
    let mut next = 1_u64;
    while next < max + 1 {
        next *= 10;
    }
    let mut star_count = 0;
    for (kind, groups) in groups.iter().enumerate() {
        for group in groups {
            let r = &group[0];
            let z = &group[1];
            let ws = &group[2..];
            let cw = r.id(4)?;
            let cz = r.id(5)?;
            if !(1..=3).contains(&cw) || !(1..=3).contains(&cz) {
                return Err(r.error("transformer CW and CZ must be 1, 2 or 3"));
            }
            let status = r.id(11)?;
            if status > if kind == 0 { 1 } else { 4 } {
                return Err(r.error("invalid transformer status"));
            }
            if r.optional(7, 0.0)? != 0.0 || r.optional(8, 0.0)? != 0.0 {
                ctx.warn(
                    r.line,
                    true,
                    "transformer magnetizing admittance ignored, matching MATPOWER",
                )?;
            }
            for w in ws.iter().take(if kind == 0 { 1 } else { 3 }) {
                for i in [6, 13, 16] {
                    if w.optional(i, 0.0)? != 0.0 {
                        ctx.warn(w.line, true, "transformer controls, correction tables and connection angles ignored; supplied tap retained")?;
                        break;
                    }
                }
            }
            if kind == 1 {
                next += 1;
            }
            let mut buses = vec![];
            for i in 0..if kind == 0 { 2 } else { 3 } {
                if let Some(&index) = indices.get(&r.id(i)?) {
                    buses.push(index);
                }
            }
            if buses.len() != if kind == 0 { 2 } else { 3 } {
                ctx.warn(
                    r.line,
                    true,
                    "transformer with unknown bus reference skipped",
                )?;
                continue;
            }
            if kind == 0 {
                let f = &case.buses[buses[0]];
                let t = &case.buses[buses[1]];
                let zi = impedance(z, 0, cz, ws[0].num(1)?, f, case.base_mva)?;
                let tap = winding(&ws[0], cw, f)? / winding(&ws[1], cw, t)?;
                case.branches.push(transformer_branch(
                    r,
                    &ws[0],
                    f.id,
                    t.id,
                    zi,
                    tap,
                    status == 1,
                )?);
            } else {
                let id =
                    u32::try_from(next).map_err(|_| r.error("star bus ID exceeds u32 range"))?;
                let mut zs = vec![];
                for i in 0..3 {
                    zs.push(impedance(
                        z,
                        3 * i,
                        cz,
                        ws[i].num(1)?,
                        &case.buses[buses[i]],
                        case.base_mva,
                    )?);
                }
                let star_z = [
                    (
                        (zs[0].0 + zs[2].0 - zs[1].0) / 2.0,
                        (zs[0].1 + zs[2].1 - zs[1].1) / 2.0,
                    ),
                    (
                        (zs[0].0 + zs[1].0 - zs[2].0) / 2.0,
                        (zs[0].1 + zs[1].1 - zs[2].1) / 2.0,
                    ),
                    (
                        (zs[1].0 + zs[2].0 - zs[0].0) / 2.0,
                        (zs[1].1 + zs[2].1 - zs[0].1) / 2.0,
                    ),
                ];
                for i in 0..3 {
                    let b = &case.buses[buses[i]];
                    let online = status != 0 && status != [4, 2, 3][i];
                    case.branches.push(transformer_branch(
                        r,
                        &ws[i],
                        b.id,
                        id,
                        star_z[i],
                        winding(&ws[i], cw, b)?,
                        online,
                    )?);
                }
                let b = &case.buses[buses[0]];
                case.buses.push(Bus {
                    id,
                    kind: if status == 0 {
                        BusType::Isolated
                    } else {
                        BusType::Pq
                    },
                    pd: 0.0,
                    qd: 0.0,
                    gs: 0.0,
                    bs: 0.0,
                    area: b.area,
                    zone: b.zone,
                    vm: z.num(9)?,
                    va: z.num(10)?,
                    base_kv: 1.0,
                    vmax: 1.1,
                    vmin: 0.9,
                    extra: vec![],
                });
                star_count += 1;
                case.labels
                    .get_mut("bus_name")
                    .unwrap()
                    .push(format!("STR_PT_XF_{star_count:<2}"));
                ctx.warn(
                    r.line,
                    false,
                    format!("added star bus {id} for three-winding transformer"),
                )?;
            }
        }
    }
    Ok(())
}

fn convert_dc(
    rows: &[Record],
    case: &mut Case,
    indices: &HashMap<u32, usize>,
    ctx: &mut Context<'_>,
) -> Result<(), Error> {
    if rows.is_empty() {
        return Ok(());
    }
    let mut dc = vec![];
    for group in rows.chunks(3) {
        let r = &group[0];
        if group.len() != 3 {
            return Err(r.error("truncated two-terminal DC line"));
        }
        let rect = &group[1];
        let inv = &group[2];
        let mode = r.id(1)?;
        if mode > 2 {
            return Err(r.error("DC control mode must be 0, 1 or 2"));
        }
        let p = match mode {
            1 => r.num(3)?.abs(),
            2 => r.num(3)?.abs() * r.num(4)? / 1000.0,
            _ => 0.0,
        };
        let mut line = vec![0.0; 17];
        line[0] = rect.id(0)? as f64;
        line[1] = inv.id(0)? as f64;
        line[2] = f64::from(mode != 0);
        line[3] = p;
        line[4] = p;
        line[9] = 0.85 * p;
        line[10] = 1.15 * p;
        for (i, end) in [rect, inv].iter().enumerate() {
            let index = indices
                .get(&end.id(0)?)
                .ok_or_else(|| end.error("unknown DC terminal bus"))?;
            line[7 + i] = case.buses[*index].vm;
            let min = end.num(3)?.to_radians();
            let max = end.num(2)?.to_radians();
            line[11 + 2 * i] = (p * min.tan()).abs();
            line[12 + 2 * i] = (p * ((max.cos() + 0.5) * 0.5).acos().tan()).abs();
        }
        if line.iter().any(|n| !n.is_finite()) {
            return Err(r.error("nonfinite converted DC line"));
        }
        dc.push(line);
    }
    ctx.warn(
        rows[0].line,
        true,
        "two-terminal DC lines use MATPOWER's lossless approximation and estimated reactive limits",
    )?;
    case.dcline = Some(dc);
    Ok(())
}
