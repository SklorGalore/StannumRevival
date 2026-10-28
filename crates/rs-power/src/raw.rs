//! PSS/E RAW conversion following MATPOWER 8.1 psse_read/parse/convert.
//! See NOTICE for upstream attribution and licensing.
use crate::{
    Area, AreaLoad, Asymmetric, Branch, Bus, BusType, Case, Error, FlowControls, Generator,
    LccControl, LccConverter, LccEnd, LccLine, MtdcBus, MtdcConverter, MtdcLink, MtdcSystem,
    PhaseShifter, ReactiveTap, ShuntMode, SwitchedShunt, TapControl, VoltageControls, VscAcControl,
    VscConverter, VscDcControl, VscLine,
};
use std::{
    collections::{HashMap, HashSet},
    path::Path,
};

/// Options for RAW import. Defaults to MATPOWER-style conversion with warnings.
#[derive(Clone, Debug, Default)]
pub struct RawOptions {
    /// Override revision detection (supported revisions: 23 through 35).
    pub revision: Option<u32>,
    /// Reject lossy approximations and ignored equipment instead of warning.
    pub strict: bool,
    /// Also convert voltage source converter (VSC) and multi-terminal DC
    /// lines into `case.dcline` rows. MATPOWER skips both, so this is off by
    /// default to keep conversions identical to MATPOWER 8.1.
    pub all_dc_lines: bool,
    /// Give line-commutated converters (two-terminal and multi-terminal DC
    /// rows) negative reactive limits, because they absorb reactive power.
    /// MATPOWER writes the same estimates as positive, supplied limits, so
    /// this is off by default to keep conversions identical to MATPOWER 8.1.
    pub lcc_absorbs_reactive: bool,
    /// Keep generators' remote voltage regulation (IREG, shared by RMPCT) in
    /// [`Generator::regulated_bus`] and [`Generator::reactive_share`].
    /// MATPOWER applies every setpoint at the generator's own bus, so this
    /// is off by default to keep conversions identical to MATPOWER 8.1.
    pub remote_regulation: bool,
    /// Keep loads' constant-current (IP, IQ) and constant-impedance (YP, YQ)
    /// parts in [`Bus::pd_i`], [`Bus::qd_i`], [`Bus::pd_z`] and
    /// [`Bus::qd_z`], so power flow varies them with voltage. MATPOWER
    /// converts them to constant power at the bus voltage in the file, so
    /// this is off by default to keep conversions identical to MATPOWER 8.1.
    pub zip_loads: bool,
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
    /// The RAW record behind each entry of `case.branches`, in the same order.
    pub branch_sources: Vec<BranchSource>,
    /// The RAW record behind each entry of `case.generators`, in the same order.
    pub generator_sources: Vec<GeneratorSource>,
    /// The name of the DC line behind each `case.dcline` row, in row order.
    /// A multi-terminal system gives several rows that share its name.
    pub dc_line_names: Vec<String>,
    /// The RAW section behind each `case.dcline` row, in row order.
    pub dc_line_kinds: Vec<DcLineKind>,
    /// Every two-terminal DC line with its full RAW data, for the detailed
    /// DC model ([`crate::DcModel::Detailed`]). Each names its `case.dcline` row.
    pub lcc_lines: Vec<LccLine>,
    /// Every VSC DC line with one DC-voltage and one power converter, with
    /// its full RAW data, for the detailed DC model. Only filled when
    /// [`RawOptions::all_dc_lines`] imports VSC rows.
    pub vsc_lines: Vec<VscLine>,
    /// Switched shunts in control (MODSW 1 to 5, in service, with a valid
    /// band and blocks, and the device they watch found) and transformer
    /// windings with voltage-control
    /// taps (COD 1), for [`crate::PowerFlowOptions::voltage_controls`].
    /// Revisions 30 and later; MATPOWER freezes both, as the import still
    /// does in `case`.
    pub voltage_controls: VoltageControls,
    /// Transformer windings with phase shifters in active power flow
    /// control (COD 3, symmetric, and COD 5, asymmetric) and taps in
    /// reactive power flow control (COD 2), for
    /// [`crate::PowerFlowOptions::flow_controls`]. Revisions 30 and later;
    /// `case` keeps every angle and ratio as the file gives it.
    pub flow_controls: FlowControls,
    /// Every multi-terminal DC system with its full RAW data, for the
    /// detailed DC model. Each converter names its `case.dcline` row. Only
    /// filled when [`RawOptions::all_dc_lines`] imports multi-terminal rows.
    pub mtdc: Vec<MtdcSystem>,
    /// The substations of revision 34-35 substation data, with their
    /// latitude and longitude and the buses their nodes sit at, in file
    /// order. Empty for earlier revisions and files without that data.
    pub substations: Vec<RawSubstation>,
    /// The area data records, in file order, for
    /// [`crate::AreaInterchange::areas`]. An `ISW` naming no bus leaves the
    /// area without a slack bus.
    pub areas: Vec<Area>,
    /// In-service loads whose `AREA` differs from their bus's area, for
    /// [`crate::AreaInterchange::loads`]. Their power is already in the
    /// bus's load in `case`.
    pub area_loads: Vec<AreaLoad>,
    /// Revision 34-35 system switching devices, in file order. Each also
    /// solves as a branch of `case`.
    pub switching_devices: Vec<SwitchingDevice>,
}

/// A substation record of revision 34-35 RAW data.
#[derive(Clone, Debug, PartialEq)]
pub struct RawSubstation {
    /// Substation number (IS).
    pub number: u32,
    /// Substation name, trimmed; may be empty.
    pub name: String,
    /// Latitude and longitude in decimal degrees (LATI, LONG), when the
    /// record gives a usable place. PSS/E writes 0, 0 for "not set", which
    /// reads as no place.
    pub place: Option<(f64, f64)>,
    /// Buses of the case with a node in this substation, in node order,
    /// each once.
    pub buses: Vec<u32>,
    /// One-based source line of the substation record.
    pub line: usize,
}

/// Which RAW section a `case.dcline` row came from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DcLineKind {
    /// A two-terminal (line-commutated) DC line.
    TwoTerminal,
    /// A voltage source converter DC line.
    VoltageSourceConverter,
    /// One converter of a multi-terminal DC system, paired with the converter
    /// that holds the system's DC voltage.
    MultiTerminal,
}

/// Which RAW section a branch came from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BranchKind {
    /// A non-transformer branch record.
    Line,
    /// A two- or three-winding transformer record.
    Transformer,
    /// A system switching device record (revisions 34-35).
    Switch,
}

/// What a system switching device is, from its STYPE field.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SwitchType {
    /// STYPE 1, PSS/E's default: a generic connector.
    Generic,
    /// STYPE 2: a circuit breaker.
    Breaker,
    /// STYPE 3: a disconnect switch.
    Disconnector,
}

/// A system switching device of revision 34-35 RAW data. It solves as a
/// lossless branch of reactance X (`case.branches[branch]`), in service when
/// closed (STAT 1).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SwitchingDevice {
    /// Row in `case.branches` and `branch_sources`.
    pub branch: usize,
    pub kind: SwitchType,
    /// Device name, trimmed; may be empty.
    pub name: String,
    /// The normal status (NSTAT) is open.
    pub normally_open: bool,
}

/// The RAW record a converted branch came from. MATPOWER rows have none.
///
/// A three-winding transformer becomes three branches to a star bus; each
/// keeps the transformer's buses and circuit, with `winding` set to 1, 2 or 3.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BranchSource {
    pub kind: BranchKind,
    /// Terminal buses as written (I, J, and K for three-winding transformers).
    pub buses: Vec<u32>,
    /// Circuit identifier, trimmed; `"1"` when the field is blank.
    pub circuit: String,
    /// Winding of a three-winding transformer; 0 otherwise.
    pub winding: u8,
    /// One-based line of the record's first row.
    pub line: usize,
}

/// The RAW record a converted generator came from.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GeneratorSource {
    pub bus: u32,
    /// Machine identifier, trimmed; `"1"` when the field is blank.
    pub id: String,
    /// One-based source line.
    pub line: usize,
}

/// A RAW identifier field: trimmed, with PSS/E's default of "1" when blank.
fn identifier(r: &Record, i: usize) -> String {
    match r.fields.get(i).map(|s| s.trim()) {
        Some(s) if !s.is_empty() => s.to_string(),
        _ => "1".to_string(),
    }
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
        // One line per record would flood the log (Texas 2k has 1,099 of the
        // same); callers summarise the list, which is returned in full.
        log::debug!("RAW conversion warning at line {line} (lossy={lossy}): {message}");
        self.warnings.push(RawWarning { line, message });
        Ok(())
    }
}

struct Record {
    line: usize,
    fields: Vec<String>,
    /// Indexes of the fields written in quotes.
    quoted: Vec<usize>,
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
        quoted: vec![],
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
            r.quoted.push(r.fields.len());
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
    if rev >= 34 {
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
/// Parse RAW revisions 23–35 using MATPOWER-style conversions. Warnings are
/// available through `parse_raw_with_options`; this convenience API drops them.
pub fn parse_raw(input: &str) -> Result<Case, Error> {
    Ok(parse_raw_with_options(input, &RawOptions::default())?.case)
}

/// Parse RAW revisions 23–35 with conversion diagnostics.
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
    if !(23..=35).contains(&rev) {
        return Err(Error::Unsupported(format!(
            "RAW revision {rev}; supported revisions are 23–35"
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
    let mut system_wide = rev >= 34;
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
    let mut branch_sources = Vec::new();
    let mut generator_sources = Vec::new();
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
            pd_i: 0.0,
            qd_i: 0.0,
            pd_z: 0.0,
            qd_z: 0.0,
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
    let mut area_loads = Vec::new();
    for r in section("load") {
        let b = &mut case.buses[bus_index(r, 0)?];
        if r.status(2)? {
            let ip = r.optional(7, 0.0)?;
            let iq = r.optional(8, 0.0)?;
            let yp = r.optional(9, 0.0)?;
            let yq = r.optional(10, 0.0)?;
            let area = r.optional(3, b.area as f64)?;
            if area > 0.0 && area.fract() == 0.0 && area as u32 != b.area {
                let pl = r.num(5)?;
                area_loads.push(if options.zip_loads {
                    AreaLoad {
                        bus: b.id,
                        area: area as u32,
                        p: pl,
                        p_i: ip,
                        p_z: yp,
                    }
                } else {
                    AreaLoad {
                        bus: b.id,
                        area: area as u32,
                        p: pl + ip * b.vm + yp * b.vm * b.vm,
                        p_i: 0.0,
                        p_z: 0.0,
                    }
                });
            }
            b.pd += r.num(5)?;
            b.qd += r.num(6)?;
            if options.zip_loads {
                // PSS/E YQ is positive for capacitive load; `qd_z` is
                // consumption, positive for inductive load.
                b.pd_i += ip;
                b.qd_i += iq;
                b.pd_z += yp;
                b.qd_z -= yq;
            } else {
                if [ip, iq, yp, yq].iter().any(|n| *n != 0.0) {
                    ctx.warn(
                        r.line,
                        true,
                        "ZIP load converted to constant power at the input bus voltage",
                    )?;
                }
                b.pd += ip * b.vm + yp * b.vm * b.vm;
                b.qd += iq * b.vm - yq * b.vm * b.vm;
            }
        }
    }
    for r in section("fixed shunt") {
        let b = &mut case.buses[bus_index(r, 0)?];
        if r.status(2)? {
            b.gs += r.num(3)?;
            b.bs += r.num(4)?;
        }
    }
    let mut voltage_controls = VoltageControls::default();
    let mut flow_controls = FlowControls::default();
    // Switched shunts whose watched VSC converter or switched shunt is
    // found once every record has been read.
    let mut pending_shunts = Vec::new();
    for r in section("switched shunt") {
        let index = bus_index(r, 0)?;
        let control = if rev >= 30 {
            switched_shunt(rev, r, &indices, &case)?
        } else {
            None
        };
        let b = &mut case.buses[index];
        let modsw = r.optional(if rev >= 35 { 2 } else { 1 }, 0.0)?;
        if let Some(shunt) = control {
            pending_shunts.push(shunt);
        } else if modsw == 6.0 {
            ctx.warn(
                r.line,
                true,
                "switched shunt frozen at BINIT; MODSW 6 watches a FACTS device, and FACTS devices are not modelled",
            )?;
        } else if modsw != 0.0 {
            ctx.warn(
                r.line,
                true,
                "switched shunt frozen at BINIT; its switching control is not supported, or its band, blocks or controlled bus are not valid",
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
        let shift = usize::from(rev >= 35);
        let ireg = r.optional(7, 0.0)?;
        let regulated_bus = if ireg != 0.0 && ireg != r.num(0)? {
            if !options.remote_regulation {
                ctx.warn(
                    r.line,
                    true,
                    "remote generator regulation ignored; voltage setpoint applied locally",
                )?;
                None
            } else if let Some(&i) = indices.get(&r.id(7)?) {
                Some(case.buses[i].id)
            } else {
                ctx.warn(
                    r.line,
                    true,
                    "generator regulates an unknown bus; voltage setpoint applied locally",
                )?;
                None
            }
        } else {
            None
        };
        let reactive_share = if options.remote_regulation {
            r.optional(15 + shift, 100.0)?
        } else {
            100.0
        };
        if r.optional(26 + shift, 0.0)? != 0.0 {
            ctx.warn(
                r.line,
                true,
                "wind-machine control ignored; supplied generator limits retained",
            )?;
        }
        generator_sources.push(GeneratorSource {
            bus: r.id(0)?,
            id: identifier(r, 1),
            line: r.line,
        });
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
            regulated_bus,
            reactive_share,
        });
    }
    for r in section("branch") {
        let from = r.id(0)?;
        let to = r.integer(1, true)?;
        let online = r.status(if rev >= 34 {
            23
        } else if rev <= 27 {
            15
        } else {
            13
        })?;
        let shunt = if rev >= 34 {
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
        branch_sources.push(BranchSource {
            kind: BranchKind::Line,
            buses: vec![from, to],
            circuit: identifier(r, 2),
            winding: 0,
            line: r.line,
        });
        case.branches.push(Branch {
            from,
            to,
            r: r.num(3)?,
            x: r.num(4)?,
            b: r.num(5)?,
            rate_a: r.num(if rev >= 34 { 7 } else { 6 })?,
            rate_b: r.num(if rev >= 34 { 8 } else { 7 })?,
            rate_c: r.num(if rev >= 34 { 9 } else { 8 })?,
            tap: if rev <= 27 { r.optional(9, 0.0)? } else { 0.0 },
            shift: if rev <= 27 { r.optional(10, 0.0)? } else { 0.0 },
            online,
            angmin: -360.0,
            angmax: 360.0,
            extra: vec![],
        });
    }
    convert_transformers(
        rev,
        section("transformer"),
        &mut case,
        &mut branch_sources,
        &indices,
        &mut ctx,
        &mut voltage_controls.taps,
        &mut flow_controls,
    )?;
    // After the transformers, so branch rows of files that have none keep
    // the numbers they had before switching devices were read.
    let switching_devices = convert_switching_devices(
        section("system switching device"),
        &mut case,
        &mut branch_sources,
        &indices,
        &mut ctx,
    )?;
    let (mut dc_line_names, lcc_lines) = convert_dc(
        rev,
        section("two-terminal DC"),
        &mut case,
        &indices,
        &mut ctx,
    )?;
    let mut dc_line_kinds = vec![DcLineKind::TwoTerminal; dc_line_names.len()];
    let mut vsc_lines = vec![];
    let mut mtdc = vec![];
    if options.all_dc_lines {
        let (vsc_rows, vsc_details) = convert_vsc(
            section("voltage source converter"),
            &case,
            &indices,
            &mut ctx,
        )?;
        for (k, detail) in vsc_details.into_iter().enumerate() {
            if let Some(mut detail) = detail {
                detail.row = dc_line_names.len() + k;
                vsc_lines.push(detail);
            }
        }
        let (mt_rows, systems) =
            convert_multi_terminal(rev, section("multi-terminal DC"), &case, &indices, &mut ctx)?;
        let mt_base = dc_line_names.len() + vsc_rows.len();
        for mut system in systems {
            for c in &mut system.converters {
                c.row = c.row.map(|r| r + mt_base);
            }
            mtdc.push(system);
        }
        for (kind, rows) in [
            (DcLineKind::VoltageSourceConverter, vsc_rows),
            (DcLineKind::MultiTerminal, mt_rows),
        ] {
            for (name, row) in rows {
                dc_line_names.push(name);
                dc_line_kinds.push(kind);
                case.dcline.get_or_insert_with(Vec::new).push(row);
            }
        }
    }
    voltage_controls.switched_shunts = resolve_switched_shunts(
        pending_shunts,
        &case,
        &dc_line_names,
        &dc_line_kinds,
        &mut ctx,
    )?;
    if options.lcc_absorbs_reactive
        && let Some(lines) = &mut case.dcline
    {
        for (line, kind) in lines.iter_mut().zip(&dc_line_kinds) {
            if *kind != DcLineKind::VoltageSourceConverter {
                // [QMIN, QMAX] supplied becomes [-QMAX, -QMIN] absorbed, per end.
                for col in [11, 13] {
                    (line[col], line[col + 1]) = (-line[col + 1], -line[col]);
                }
            }
        }
    }
    let mut areas = Vec::new();
    for r in section("area") {
        let isw = r.optional(1, 0.0)?;
        let slack_bus = if isw == 0.0 {
            None
        } else if let Some(&i) = indices.get(&r.id(1)?) {
            Some(case.buses[i].id)
        } else {
            ctx.warn(
                r.line,
                false,
                "area slack bus (ISW) is not a bus in the case; the area holds no interchange",
            )?;
            None
        };
        areas.push(Area {
            number: r.id(0)?,
            slack_bus,
            desired_mw: r.optional(2, 0.0)?,
            tolerance_mw: r.optional(3, 10.0)?,
            name: r.fields.get(4).map_or("", |s| s.trim()).to_string(),
        });
    }
    for (i, rows) in sections.iter().enumerate().take(names.len()) {
        let name = names[i];
        if !matches!(
            name,
            "bus"
                | "load"
                | "fixed shunt"
                | "switched shunt"
                | "generator"
                | "branch"
                | "system switching device"
                | "transformer"
                | "two-terminal DC"
                | "area"
        ) && !(options.all_dc_lines
            && matches!(name, "voltage source converter" | "multi-terminal DC"))
            && let Some(r) = rows.first()
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
    // Sections past the revision's known list. In revisions 34-35 they hold
    // the substation data: each substation's record and its nodes, then its
    // switching devices and terminals, each block ended by a 0. Substations
    // keep their place and the buses their nodes sit at; the rest of the
    // node-breaker detail, and anything else, is reported once in total.
    let extra = sections.get(names.len()..).unwrap_or(&[]);
    let (substations, read) = if rev >= 34 {
        convert_substations(extra, &indices, &mut ctx)?
    } else {
        Default::default()
    };
    let skipped: Vec<&Record> = extra
        .iter()
        .flatten()
        .filter(|r| !read.contains(&r.line))
        .collect();
    if let Some(r) = skipped.first() {
        ctx.warn(
            r.line,
            true,
            if substations.is_empty() {
                format!("skipped {} records of additional section data", skipped.len())
            } else {
                format!(
                    "skipped {} records of substation node-breaker data (read {} substations' places and buses)",
                    skipped.len(),
                    substations.len()
                )
            },
        )?;
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
        branch_sources,
        generator_sources,
        dc_line_names,
        dc_line_kinds,
        lcc_lines,
        vsc_lines,
        voltage_controls,
        flow_controls,
        mtdc,
        substations,
        areas,
        area_loads,
        switching_devices,
    })
}

/// PSS/E's default switching device reactance, in per unit.
const SWITCH_X: f64 = 0.0001;

/// Revision 34-35 system switching devices:
/// `I, J, 'CKT', X, RATE1..RATE12, STAT, NSTAT, MET, STYPE, 'NAME'`. Each
/// becomes a lossless branch of reactance X (PSS/E's 0.0001 when 0 or
/// blank), in service when STAT is 1.
fn convert_switching_devices(
    rows: &[Record],
    case: &mut Case,
    branch_sources: &mut Vec<BranchSource>,
    indices: &HashMap<u32, usize>,
    ctx: &mut Context,
) -> Result<Vec<SwitchingDevice>, Error> {
    let mut devices = Vec::with_capacity(rows.len());
    for r in rows {
        let from = r.id(0)?;
        let to = r.integer(1, true)?;
        for id in [from, to] {
            if !indices.contains_key(&id) {
                ctx.warn(
                    r.line,
                    false,
                    format!("switching device references unknown bus {id}; device retained"),
                )?;
            }
        }
        let x = r.optional(3, SWITCH_X)?;
        let status = |i: usize| -> Result<bool, Error> {
            if r.fields.get(i).is_none_or(|s| s.trim().is_empty()) {
                Ok(true)
            } else {
                r.status(i)
            }
        };
        let online = status(16)?;
        let normally_open = !status(17)?;
        let kind = match r.optional(19, 1.0)? {
            2.0 => SwitchType::Breaker,
            3.0 => SwitchType::Disconnector,
            _ => SwitchType::Generic,
        };
        let name = r
            .fields
            .get(20)
            .map(|s| s.trim().to_string())
            .unwrap_or_default();
        devices.push(SwitchingDevice {
            branch: case.branches.len(),
            kind,
            name,
            normally_open,
        });
        branch_sources.push(BranchSource {
            kind: BranchKind::Switch,
            buses: vec![from, to],
            circuit: identifier(r, 2),
            winding: 0,
            line: r.line,
        });
        case.branches.push(Branch {
            from,
            to,
            r: 0.0,
            x: if x == 0.0 { SWITCH_X } else { x },
            b: 0.0,
            rate_a: r.optional(4, 0.0)?,
            rate_b: r.optional(5, 0.0)?,
            rate_c: r.optional(6, 0.0)?,
            tap: 0.0,
            shift: 0.0,
            online,
            angmin: -360.0,
            angmax: 360.0,
            extra: vec![],
        });
    }
    Ok(devices)
}

/// Substation records from the sections after the known ones. A substation
/// record starts its section and is followed there by its node records; the
/// switching device and terminal blocks that follow start with numbers, while
/// a substation record's second field is its quoted name, which is how the
/// next substation is told apart. Empty blocks, and the 0 that ends the
/// data, give empty sections. Returns the substations and the lines read.
fn convert_substations(
    extra: &[Vec<Record>],
    indices: &HashMap<u32, usize>,
    ctx: &mut Context,
) -> Result<(Vec<RawSubstation>, HashSet<usize>), Error> {
    let mut substations: Vec<RawSubstation> = vec![];
    let mut read = HashSet::new();
    for rows in extra {
        let Some(first) = rows.first() else { continue };
        let named = first.quoted.contains(&1)
            || first
                .fields
                .get(1)
                .is_some_and(|f| f.trim().replace(['d', 'D'], "E").parse::<f64>().is_err());
        if !named {
            continue;
        }
        let number = match first.id(0) {
            Ok(n) if n > 0 => n,
            _ => {
                ctx.warn(
                    first.line,
                    false,
                    "skipped a substation record without a valid number",
                )?;
                continue;
            }
        };
        let lat = first.optional(2, 0.0);
        let lon = first.optional(3, 0.0);
        let place = match (lat, lon) {
            (Ok(lat), Ok(lon)) if lat == 0.0 && lon == 0.0 => None,
            (Ok(lat), Ok(lon)) if lat.abs() <= 90.0 && lon.abs() <= 180.0 => Some((lat, lon)),
            _ => {
                ctx.warn(
                    first.line,
                    false,
                    format!("substation {number} has no usable latitude and longitude"),
                )?;
                None
            }
        };
        let mut buses = vec![];
        read.extend(rows.iter().map(|r| r.line));
        for node in &rows[1..] {
            // Node records: NI, 'NAME', I, STATUS, VM, VA.
            match node.id(2) {
                Ok(bus) if indices.contains_key(&bus) => {
                    if !buses.contains(&bus) {
                        buses.push(bus);
                    }
                }
                _ => ctx.warn(
                    node.line,
                    false,
                    format!("substation {number} node names a bus the case doesn't have"),
                )?,
            }
        }
        substations.push(RawSubstation {
            number,
            name: first.fields[1].trim().to_string(),
            place,
            buses,
            line: first.line,
        });
    }
    Ok((substations, read))
}

/// A switched shunt control read from its record, before the VSC
/// converter or switched shunt it watches (MODSW 4 and 5) is found.
struct PendingShunt {
    shunt: SwitchedShunt,
    /// The record's shunt ID (revision 35), "1" before.
    id: String,
    watches: Watches,
}

enum Watches {
    /// Its own control needs nothing else (MODSW 1 to 3).
    Nothing,
    /// The converter at `bus` of the VSC DC line named `name` (MODSW 4).
    Converter { name: String, bus: u32 },
    /// A switched shunt at `bus`, the one with ID `id` when given (MODSW 5).
    Shunt { bus: u32, id: Option<String> },
}

/// The control of a switched shunt record (revision 30 or later), if it has
/// one that the solver can apply.
fn switched_shunt(
    rev: u32,
    r: &Record,
    indices: &HashMap<u32, usize>,
    case: &Case,
) -> Result<Option<PendingShunt>, Error> {
    // Field positions: MODSW, STAT (none before 32), VSWHI, VSWLO, SWREM,
    // RMIDNT, BINIT, first N.
    let [modsw, stat, vhi, vlo, rem, rmidnt, binit, first] = match rev {
        35.. => [2, 4, 5, 6, 7, 10, 11, 12],
        32..=34 => [1, 3, 4, 5, 6, 8, 9, 10],
        _ => [1, usize::MAX, 2, 3, 4, 6, 7, 8],
    };
    let modsw = r.optional(modsw, 0.0)?;
    if !matches!(modsw, 1.0..=5.0) || modsw.fract() != 0.0 {
        return Ok(None);
    }
    if stat != usize::MAX && !r.status(stat)? {
        return Ok(None);
    }
    let (vmax, vmin) = (r.optional(vhi, 1.0)?, r.optional(vlo, 1.0)?);
    let band_ok = if modsw <= 2.0 {
        vmin > 0.0 && vmax >= vmin
    } else {
        // Fractions of the watched device's reactive range.
        vmin >= 0.0 && vmax >= vmin && vmax <= 1.0
    };
    if !band_ok {
        return Ok(None);
    }
    // Revision 35 records carry S, N, B per block; earlier ones N, B.
    let width = if rev >= 35 { 3 } else { 2 };
    let mut blocks = Vec::new();
    let mut at = first;
    while at + width - 1 < r.fields.len() && blocks.len() < 8 {
        let (status, n, b) = if rev >= 35 {
            (
                r.optional(at, 1.0)?,
                r.optional(at + 1, 0.0)?,
                r.optional(at + 2, 0.0)?,
            )
        } else {
            (1.0, r.optional(at, 0.0)?, r.optional(at + 1, 0.0)?)
        };
        if status != 0.0 && n >= 1.0 && n.fract() == 0.0 && b != 0.0 {
            blocks.push((n as u32, b));
        }
        at += width;
    }
    if blocks.is_empty() {
        return Ok(None);
    }
    let own = r.id(0)?;
    let remote = r.optional(rem, 0.0)?.abs();
    // The bus SWREM names, its own bus when 0; `None` when it is not in the case.
    let named = if remote == 0.0 || remote == f64::from(own) {
        Some(own)
    } else {
        indices.get(&(remote as u32)).map(|&i| case.buses[i].id)
    };
    let name = r
        .fields
        .get(rmidnt)
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());
    let mut watches = Watches::Nothing;
    let (mode, regulated_bus) = match modsw as u8 {
        1 | 2 => {
            let mode = if modsw == 1.0 {
                ShuntMode::Discrete
            } else {
                ShuntMode::Continuous
            };
            (mode, named.filter(|&b| b != own))
        }
        3 => {
            let Some(bus) = named else { return Ok(None) };
            (ShuntMode::PlantReactive { bus }, None)
        }
        4 => {
            let (Some(bus), Some(name)) = (named, name) else {
                return Ok(None);
            };
            watches = Watches::Converter { name, bus };
            // The row is filled in once the VSC lines are read.
            (ShuntMode::ConverterReactive { row: 0, bus }, None)
        }
        _ => {
            let Some(bus) = named else { return Ok(None) };
            // Revision 35 allows several shunts at a bus; RMIDNT, when
            // given, is taken as the ID of the one watched.
            watches = Watches::Shunt {
                bus,
                id: name.filter(|_| rev >= 35),
            };
            (ShuntMode::ShuntSetting { shunt: 0 }, None)
        }
    };
    Ok(Some(PendingShunt {
        shunt: SwitchedShunt {
            bus: own,
            mode,
            regulated_bus,
            vmax,
            vmin,
            initial: r.num(binit)?,
            blocks,
            line: r.line,
        },
        id: if rev >= 35 {
            identifier(r, 1)
        } else {
            "1".into()
        },
        watches,
    }))
}

/// Find the VSC converter or switched shunt each MODSW 4 or 5 shunt watches.
/// Shunts whose device is missing stay frozen at BINIT, with a warning.
fn resolve_switched_shunts(
    pending: Vec<PendingShunt>,
    case: &Case,
    dc_line_names: &[String],
    dc_line_kinds: &[DcLineKind],
    ctx: &mut Context<'_>,
) -> Result<Vec<SwitchedShunt>, Error> {
    let mut kept = Vec::with_capacity(pending.len());
    for mut p in pending {
        if let Watches::Converter { name, bus } = &p.watches {
            let row = (case.dcline.iter().flatten().enumerate()).position(|(r, line)| {
                dc_line_kinds.get(r) == Some(&DcLineKind::VoltageSourceConverter)
                    && dc_line_names.get(r).is_some_and(|n| n.trim() == name)
                    && (line[0] == f64::from(*bus) || line[1] == f64::from(*bus))
            });
            let Some(row) = row else {
                ctx.warn(
                    p.shunt.line,
                    true,
                    format!(
                        "switched shunt frozen at BINIT; it watches the converter at bus {bus} of VSC DC line {name}, which is not in the case (VSC lines are read only with all DC lines on)"
                    ),
                )?;
                continue;
            };
            p.shunt.mode = ShuntMode::ConverterReactive { row, bus: *bus };
        }
        kept.push(p);
    }
    let found: Vec<Option<usize>> = (0..kept.len())
        .map(|k| {
            let Watches::Shunt { bus, id } = &kept[k].watches else {
                return Some(k);
            };
            (0..kept.len()).find(|&j| {
                j != k
                    && kept[j].shunt.bus == *bus
                    && id.as_ref().is_none_or(|id| kept[j].id == *id)
            })
        })
        .collect();
    // A shunt watching a shunt that drops out drops out too.
    let mut alive: Vec<bool> = found.iter().map(Option::is_some).collect();
    loop {
        let mut changed = false;
        for k in 0..kept.len() {
            if alive[k] && found[k].is_some_and(|j| j != k && !alive[j]) {
                alive[k] = false;
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }
    let mut shunts = Vec::with_capacity(kept.len());
    // New positions, as frozen shunts drop out of the list.
    let mut position = vec![usize::MAX; kept.len()];
    for (k, p) in kept.iter().enumerate() {
        if alive[k] {
            position[k] = shunts.len();
            shunts.push(p.shunt.clone());
        } else if let Watches::Shunt { bus, .. } = &p.watches {
            ctx.warn(
                p.shunt.line,
                true,
                format!(
                    "switched shunt frozen at BINIT; it watches a switched shunt at bus {bus} that is not in the case or not in control"
                ),
            )?;
        }
    }
    for (k, p) in kept.iter().enumerate() {
        if let (true, Watches::Shunt { .. }, Some(j)) = (alive[k], &p.watches, found[k]) {
            shunts[position[k]].mode = ShuntMode::ShuntSetting { shunt: position[j] };
        }
    }
    Ok(shunts)
}

/// The voltage-control tap of a transformer winding record (COD 1), if any.
/// `ratio_index` is the winding's WINDV field and `cod` its COD field.
#[allow(clippy::too_many_arguments)]
fn tap_control(
    rev: u32,
    w: &Record,
    cod: usize,
    branch: usize,
    own_bus: u32,
    tap: f64,
    indices: &HashMap<u32, usize>,
    case: &Case,
) -> Result<Option<TapControl>, Error> {
    if w.optional(cod, 0.0)? != 1.0 {
        return Ok(None);
    }
    // CONT, (NODE in revision 35), RMA, RMI, VMA, VMI, NTP.
    let skip = usize::from(rev >= 35);
    let cont = w.optional(cod + 1, 0.0)?;
    let [rma, rmi, vma, vmi, ntp] = [2, 3, 4, 5, 6].map(|k| cod + skip + k);
    let (ratio_max, ratio_min) = (w.optional(rma, 1.1)?, w.optional(rmi, 0.9)?);
    let (vmax, vmin) = (w.optional(vma, 1.1)?, w.optional(vmi, 0.9)?);
    let positions = w.optional(ntp, 33.0)?;
    let ratio = w.num(0)?;
    let Some(&bus) = indices.get(&(cont.abs() as u32)) else {
        return Ok(None);
    };
    if cont == 0.0
        || ratio <= 0.0
        || tap <= 0.0
        || ratio_max <= ratio_min
        || !(vmin > 0.0 && vmax >= vmin)
        || positions < 2.0
        || positions.fract() != 0.0
    {
        return Ok(None);
    }
    let regulated_bus = case.buses[bus].id;
    Ok(Some(TapControl {
        branch,
        regulated_bus,
        // A negative CONT puts a bus that is not a terminal on the winding's
        // own side; the winding's own bus is on its own side too.
        raises_voltage: regulated_bus == own_bus || cont < 0.0,
        vmax,
        vmin,
        ratio,
        ratio_max,
        ratio_min,
        positions: positions as u32,
        tap_per_ratio: tap / ratio,
        line: w.line,
    }))
}

/// Add a transformer winding record's flow control, if it has one: a
/// reactive flow tap (COD 2) or a phase shifter (COD 3 or 5).
fn flow_control(
    rev: u32,
    w: &Record,
    controls: [usize; 3],
    branch: usize,
    tap: f64,
    ctx: &mut Context<'_>,
    flow: &mut FlowControls,
) -> Result<(), Error> {
    if let Some(t) = reactive_tap(rev, w, controls[0], branch, tap, ctx)? {
        flow.reactive_taps.push(t);
    }
    if let Some(p) = phase_shifter(rev, w, controls, branch, tap, ctx)? {
        flow.phase_shifters.push(p);
    }
    Ok(())
}

/// The reactive power flow tap of a transformer winding record (COD 2), if
/// any. `cod` is the record's COD field and `tap` the winding's branch tap.
fn reactive_tap(
    rev: u32,
    w: &Record,
    cod: usize,
    branch: usize,
    tap: f64,
    ctx: &mut Context<'_>,
) -> Result<Option<ReactiveTap>, Error> {
    if rev < 30 || w.optional(cod, 0.0)? != 2.0 {
        return Ok(None);
    }
    // CONT, (NODE in revision 35), RMA, RMI, VMA, VMI, NTP.
    let skip = usize::from(rev >= 35);
    let [rma, rmi, vma, vmi, ntp] = [2, 3, 4, 5, 6].map(|k| cod + skip + k);
    let (ratio_max, ratio_min) = (w.optional(rma, 1.1)?, w.optional(rmi, 0.9)?);
    let (mvar_max, mvar_min) = (w.optional(vma, 0.0)?, w.optional(vmi, 0.0)?);
    let positions = w.optional(ntp, 33.0)?;
    let ratio = w.num(0)?;
    if ratio <= 0.0
        || tap <= 0.0
        || ratio_max <= ratio_min
        || positions < 2.0
        || positions.fract() != 0.0
    {
        ctx.warn(
            w.line,
            true,
            "reactive flow tap with an invalid ratio range or tap positions held at its ratio",
        )?;
        return Ok(None);
    }
    if mvar_max < mvar_min {
        ctx.warn(
            w.line,
            false,
            "reactive flow tap limits given high-to-low (VMA < VMI); using them as a band",
        )?;
    }
    Ok(Some(ReactiveTap {
        branch,
        mvar_max,
        mvar_min,
        ratio,
        ratio_max,
        ratio_min,
        positions: positions as u32,
        tap_per_ratio: tap / ratio,
        line: w.line,
    }))
}

/// The phase shifter of a transformer winding record in active power flow
/// control (COD 3 or 5), if any. `controls` holds the record's COD, TAB and
/// CNXA fields; `tap` is the winding's branch tap.
fn phase_shifter(
    rev: u32,
    w: &Record,
    controls: [usize; 3],
    branch: usize,
    tap: f64,
    ctx: &mut Context<'_>,
) -> Result<Option<PhaseShifter>, Error> {
    let cod = controls[0];
    let mode = w.optional(cod, 0.0)?;
    if rev < 30 || !(mode == 3.0 || mode == 5.0) {
        return Ok(None);
    }
    // CONT, (NODE in revision 35), RMA, RMI, VMA, VMI.
    let skip = usize::from(rev >= 35);
    let [rma, rmi, vma, vmi] = [2, 3, 4, 5].map(|k| cod + skip + k);
    let (angle_max, angle_min) = (w.optional(rma, 0.0)?, w.optional(rmi, 0.0)?);
    let (mw_max, mw_min) = (w.optional(vma, 0.0)?, w.optional(vmi, 0.0)?);
    let angle = w.num(2)?;
    if angle_max == angle_min {
        ctx.warn(
            w.line,
            true,
            "phase shifter with no angle range (RMA = RMI) held at its angle",
        )?;
        return Ok(None);
    }
    if mw_max < mw_min {
        ctx.warn(
            w.line,
            false,
            "phase shifter flow limits given high-to-low (VMA < VMI); using them as a band",
        )?;
    }
    let asymmetric = if mode == 5.0 {
        // CNXA is normalized into -90..=90.
        let mut alpha = w.optional(controls[2], 0.0)?;
        alpha = (alpha + 90.0).rem_euclid(180.0) - 90.0;
        if alpha.abs() < 1.0 || tap <= 0.0 {
            ctx.warn(
                w.line,
                true,
                "asymmetric phase shifter with a connection angle of zero cannot shift; held at its angle",
            )?;
            return Ok(None);
        }
        Some(Asymmetric {
            connection_angle: if alpha == -90.0 { 90.0 } else { alpha },
            tap,
        })
    } else {
        None
    };
    Ok(Some(PhaseShifter {
        branch,
        mw_max,
        mw_min,
        angle_max,
        angle_min,
        angle,
        asymmetric,
        line: w.line,
    }))
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

#[allow(clippy::too_many_arguments)]
fn convert_transformers(
    rev: u32,
    rows: &[Record],
    case: &mut Case,
    sources: &mut Vec<BranchSource>,
    indices: &HashMap<u32, usize>,
    ctx: &mut Context<'_>,
    taps: &mut Vec<TapControl>,
    flow: &mut FlowControls,
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
            // COD, TAB and CNXA on each winding record. Revision 34 widened the
            // ratings to twelve and revision 35 added NODE after CONT.
            let controls = match rev {
                ..=33 => [6, 13, 16],
                34 => [15, 22, 25],
                _ => [15, 23, 26],
            };
            for w in ws.iter().take(if kind == 0 { 1 } else { 3 }) {
                for i in controls {
                    let value = w.optional(i, 0.0)?;
                    // Voltage control (COD ±1) goes to `voltage_controls`
                    // and phase shifters (COD ±3, ±5) to `flow_controls`,
                    // which also read the connection angle (CNXA).
                    let modelled = (i == controls[0]
                        && [1.0, 2.0, 3.0, 5.0].contains(&value.abs()))
                        || (i == controls[2] && w.optional(controls[0], 0.0)?.abs() == 5.0);
                    if value != 0.0 && !modelled {
                        ctx.warn(w.line, true, "transformer reactive, power or impedance-correction controls and connection angles ignored; supplied tap retained")?;
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
            let source = |winding: u8| -> Result<BranchSource, Error> {
                Ok(BranchSource {
                    kind: BranchKind::Transformer,
                    buses: (0..if kind == 0 { 2 } else { 3 })
                        .map(|i| r.id(i))
                        .collect::<Result<_, _>>()?,
                    circuit: identifier(r, 3),
                    winding,
                    line: r.line,
                })
            };
            if kind == 0 {
                sources.push(source(0)?);
                let f = &case.buses[buses[0]];
                let t = &case.buses[buses[1]];
                let zi = impedance(z, 0, cz, ws[0].num(1)?, f, case.base_mva)?;
                let tap = winding(&ws[0], cw, f)? / winding(&ws[1], cw, t)?;
                if rev >= 30
                    && let Some(control) = tap_control(
                        rev,
                        &ws[0],
                        controls[0],
                        case.branches.len(),
                        f.id,
                        tap,
                        indices,
                        case,
                    )?
                {
                    taps.push(control);
                }
                flow_control(rev, &ws[0], controls, case.branches.len(), tap, ctx, flow)?;
                let (f, t) = (&case.buses[buses[0]], &case.buses[buses[1]]);
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
                    sources.push(source(i as u8 + 1)?);
                    let b = &case.buses[buses[i]];
                    let online = status != 0 && status != [4, 2, 3][i];
                    let tap = winding(&ws[i], cw, b)?;
                    if rev >= 30
                        && let Some(control) = tap_control(
                            rev,
                            &ws[i],
                            controls[0],
                            case.branches.len(),
                            b.id,
                            tap,
                            indices,
                            case,
                        )?
                    {
                        taps.push(control);
                    }
                    flow_control(rev, &ws[i], controls, case.branches.len(), tap, ctx, flow)?;
                    let b = &case.buses[buses[i]];
                    case.branches.push(transformer_branch(
                        r, &ws[i], b.id, id, star_z[i], tap, online,
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
                    pd_i: 0.0,
                    qd_i: 0.0,
                    pd_z: 0.0,
                    qd_z: 0.0,
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
    rev: u32,
    rows: &[Record],
    case: &mut Case,
    indices: &HashMap<u32, usize>,
    ctx: &mut Context<'_>,
) -> Result<(Vec<String>, Vec<LccLine>), Error> {
    if rows.is_empty() {
        return Ok((Vec::new(), Vec::new()));
    }
    let mut dc = vec![];
    let mut names = vec![];
    let mut lcc = vec![];
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
        let detail = lcc_line(rev, dc.len(), r, rect, inv)?;
        let mut ignored = vec![];
        if detail.min_compounded_kv != 0.0 {
            ignored.push("DCVMIN");
        }
        for c in [&detail.rectifier, &detail.inverter] {
            if c.capacitor != 0.0 && !ignored.contains(&"XCAP") {
                ignored.push("XCAP");
            }
            if c.commutating_bus != 0 && !ignored.contains(&"IC") {
                ignored.push("IC");
            }
            if c.transformer.is_some() && !ignored.contains(&"IF/IT/ID") {
                ignored.push("IF/IT/ID");
            }
        }
        if !ignored.is_empty() {
            ctx.warn(
                r.line,
                false,
                format!(
                    "the detailed model of DC line {} ignores {}",
                    detail.name,
                    ignored.join(", ")
                ),
            )?;
        }
        dc.push(line);
        names.push(identifier(r, 0));
        lcc.push(detail);
    }
    ctx.warn(
        rows[0].line,
        true,
        "in the simplified DC model, two-terminal DC lines use MATPOWER's lossless approximation and estimated reactive limits",
    )?;
    case.dcline = Some(dc);
    Ok((names, lcc))
}

/// Full RAW data of one two-terminal DC line, which becomes `case.dcline` row `row`.
fn lcc_line(
    rev: u32,
    row: usize,
    r: &Record,
    rect: &Record,
    inv: &Record,
) -> Result<LccLine, Error> {
    let control = match r.id(1)? {
        1 => LccControl::Power,
        2 => LccControl::Current,
        _ => LccControl::Blocked,
    };
    let text = |rec: &Record, i: usize| {
        rec.fields
            .get(i)
            .map(|s| s.trim().trim_matches(['\'', '"']).trim().to_string())
            .unwrap_or_default()
    };
    let converter = |c: &Record| -> Result<LccConverter, Error> {
        // Revision 34 adds the node number ND after IC.
        let shift = usize::from(rev >= 34);
        let (from, to) = (c.optional(13 + shift, 0.0)?, c.optional(14 + shift, 0.0)?);
        let transformer = (from != 0.0 && to != 0.0).then(|| {
            let id = text(c, 15 + shift);
            (
                from.abs() as u32,
                to.abs() as u32,
                if id.is_empty() { "1".into() } else { id },
            )
        });
        let bridges = c.num(1)?;
        if bridges < 1.0 || bridges.fract() != 0.0 || bridges > f64::from(u32::MAX) {
            return Err(c.error("DC converter bridge count must be a positive integer"));
        }
        Ok(LccConverter {
            bus: c.id(0)?,
            bridges: bridges as u32,
            angle_max: c.num(2)?,
            angle_min: c.num(3)?,
            resistance: c.num(4)?,
            reactance: c.num(5)?,
            base_kv: c.num(6)?,
            ratio: c.optional(7, 1.0)?,
            tap: c.optional(8, 1.0)?,
            tap_max: c.optional(9, 1.5)?,
            tap_min: c.optional(10, 0.51)?,
            tap_step: c.optional(11, 0.00625)?,
            commutating_bus: c.optional(12, 0.0)?.abs() as u32,
            transformer,
            capacitor: c.optional(16 + shift, 0.0)?,
        })
    };
    let meter = text(r, 8);
    Ok(LccLine {
        row,
        name: identifier(r, 0),
        control,
        resistance: r.num(2)?,
        setpoint: r.num(3)?,
        scheduled_kv: r.num(4)?,
        mode_switch_kv: r.optional(5, 0.0)?,
        compounding_resistance: r.optional(6, 0.0)?,
        current_margin: r.optional(7, 0.0)?,
        metered_end: if meter.eq_ignore_ascii_case("R") {
            LccEnd::Rectifier
        } else {
            LccEnd::Inverter
        },
        min_compounded_kv: r.optional(9, 0.0)?,
        ccc_iterations: r.optional(10, 20.0)?.max(0.0) as u32,
        ccc_acceleration: r.optional(11, 1.0)?,
        rectifier: converter(rect)?,
        inverter: converter(inv)?,
    })
}

/// Voltage magnitude of a DC terminal's AC bus, used as its voltage setpoint.
fn terminal_vm(
    r: &Record,
    bus: u32,
    case: &Case,
    indices: &HashMap<u32, usize>,
) -> Result<f64, Error> {
    indices
        .get(&bus)
        .map(|i| case.buses[*i].vm)
        .ok_or_else(|| r.error("unknown DC terminal bus"))
}

/// Reactive limits of a line-commutated converter carrying `p` MW, estimated
/// from its firing angles the same way as two-terminal DC lines.
fn commutated_q_limits(p: f64, angmx: f64, angmn: f64) -> (f64, f64) {
    let (min, max) = (angmn.to_radians(), angmx.to_radians());
    (
        (p * min.tan()).abs(),
        (p * ((max.cos() + 0.5) * 0.5).acos().tan()).abs(),
    )
}

/// One converter end of a VSC line: its AC bus, voltage setpoint and reactive range.
struct VscEnd {
    bus: u32,
    vg: f64,
    qmin: f64,
    qmax: f64,
}

/// A converted `dcline` row with its RAW line name.
type NamedRow = (String, Vec<f64>);

/// Each VSC DC line becomes one lossless `dcline` row. The MW-controlled
/// converter sets the transfer and the DC-voltage converter takes the other
/// end. A converter in AC voltage control holds ACSET within MINQ..MAXQ; one
/// in power factor control gets a fixed reactive output at that factor.
fn convert_vsc(
    rows: &[Record],
    case: &Case,
    indices: &HashMap<u32, usize>,
    ctx: &mut Context<'_>,
) -> Result<(Vec<NamedRow>, Vec<Option<VscLine>>), Error> {
    if rows.is_empty() {
        return Ok((Vec::new(), Vec::new()));
    }
    let mut out = vec![];
    let mut details = vec![];
    for group in rows.chunks(3) {
        let r = &group[0];
        if group.len() != 3 {
            return Err(r.error("truncated voltage source converter DC line"));
        }
        let name = identifier(r, 0);
        let in_service = r.optional(1, 1.0)? != 0.0;
        let ends = [&group[1], &group[2]];
        let mut types = [0; 2];
        for (i, c) in ends.iter().enumerate() {
            types[i] = c.id(1)?;
            if types[i] > 2 {
                return Err(c.error("VSC converter TYPE must be 0, 1 or 2"));
            }
        }
        // The MW-controlled converter's DCSET is positive when it feeds power
        // into its AC bus, so that converter is the receiving end.
        let (mut sending, mut receiving, mut p) = (0, 1, 0.0);
        if let Some(k) = types.iter().position(|&t| t == 2) {
            let dcset = ends[k].num(3)?;
            p = dcset.abs();
            (sending, receiving) = if dcset >= 0.0 { (1 - k, k) } else { (k, 1 - k) };
        } else if types.iter().all(|&t| t != 0) {
            ctx.warn(
                r.line,
                true,
                format!("VSC DC line {name} has no MW-controlled converter; scheduled at 0 MW"),
            )?;
        }
        let terminal = |c: &Record, p: f64| -> Result<VscEnd, Error> {
            let bus = c.id(0)?;
            let vm = terminal_vm(c, bus, case, indices)?;
            let mode = c.optional(2, 1.0)?;
            let acset = c.optional(4, 1.0)?;
            if mode == 2.0 {
                let pf = acset.clamp(-1.0, 1.0);
                if pf == 0.0 {
                    return Err(c.error("VSC power factor setpoint must not be zero"));
                }
                let q = p * pf.abs().acos().tan() * pf.signum();
                Ok(VscEnd {
                    bus,
                    vg: vm,
                    qmin: q,
                    qmax: q,
                })
            } else {
                let (qmax, qmin) = (c.optional(11, 9999.0)?, c.optional(12, -9999.0)?);
                if qmin > qmax {
                    return Err(c.error("VSC MINQ exceeds MAXQ"));
                }
                if acset <= 0.0 {
                    return Err(c.error("VSC AC voltage setpoint must be positive"));
                }
                Ok(VscEnd {
                    bus,
                    vg: acset,
                    qmin,
                    qmax,
                })
            }
        };
        let from = terminal(ends[sending], p)?;
        let to = terminal(ends[receiving], p)?;
        let mut line = vec![0.0; 17];
        line[0] = from.bus as f64;
        line[1] = to.bus as f64;
        line[2] = f64::from(in_service && types.iter().all(|&t| t != 0));
        line[3] = p;
        line[4] = p;
        line[7] = from.vg;
        line[8] = to.vg;
        line[9] = 0.85 * p;
        line[10] = 1.15 * p;
        line[11] = from.qmin;
        line[12] = from.qmax;
        line[13] = to.qmin;
        line[14] = to.qmax;
        if line.iter().any(|n| !n.is_finite()) {
            return Err(r.error("nonfinite converted VSC DC line"));
        }
        let detail = if types.contains(&1) && types.contains(&2) {
            let converter = |c: &Record, t: u32| -> Result<VscConverter, Error> {
                Ok(VscConverter {
                    bus: c.id(0)?,
                    dc_control: if t == 1 {
                        VscDcControl::DcVoltage
                    } else {
                        VscDcControl::Power
                    },
                    ac_control: if c.optional(2, 1.0)? == 2.0 {
                        VscAcControl::PowerFactor
                    } else {
                        VscAcControl::Voltage
                    },
                    dc_setpoint: c.num(3)?,
                    ac_setpoint: c.optional(4, 1.0)?,
                    loss_kw: c.optional(5, 0.0)?,
                    loss_kw_per_amp: c.optional(6, 0.0)?,
                    min_loss_kw: c.optional(7, 0.0)?,
                    smax_mva: c.optional(8, 0.0)?,
                    imax_amps: c.optional(9, 0.0)?,
                    power_weighting: c.optional(10, 1.0)?,
                    qmax: c.optional(11, 9999.0)?,
                    qmin: c.optional(12, -9999.0)?,
                    remote_bus: c.optional(13, 0.0)?.abs() as u32,
                    remote_percent: c.optional(14, 100.0)?,
                })
            };
            let detail = VscLine {
                row: 0,
                name: name.clone(),
                in_service,
                resistance: r.num(2)?,
                converters: [converter(ends[0], types[0])?, converter(ends[1], types[1])?],
                from: sending,
            };
            if detail
                .converters
                .iter()
                .any(|c| c.remote_bus != 0 && c.remote_bus != c.bus)
            {
                ctx.warn(
                    r.line,
                    false,
                    format!(
                        "the detailed model of VSC DC line {name} regulates its own AC bus, not REMOT"
                    ),
                )?;
            }
            Some(detail)
        } else {
            None
        };
        out.push((name, line));
        details.push(detail);
    }
    ctx.warn(
        rows[0].line,
        true,
        "in the simplified DC model, VSC DC lines are lossless, regulate only their own AC bus, and ignore converter MVA and current limits",
    )?;
    Ok((out, details))
}

/// Each multi-terminal DC system becomes one lossless `dcline` row per
/// scheduled converter, linking it to the converter that holds the DC
/// voltage (VCONV), which then picks up the balance of the system. The DC
/// buses and DC links between converters are not modelled there. The full
/// data of each system is returned too, for the detailed model, with each
/// converter's row counted from the first row returned here.
fn convert_multi_terminal(
    rev: u32,
    rows: &[Record],
    case: &Case,
    indices: &HashMap<u32, usize>,
    ctx: &mut Context<'_>,
) -> Result<(Vec<NamedRow>, Vec<MtdcSystem>), Error> {
    if rows.is_empty() {
        return Ok((Vec::new(), Vec::new()));
    }
    let text = |rec: &Record, i: usize| {
        rec.fields
            .get(i)
            .map(|s| s.trim().trim_matches(['\'', '"']).trim().to_string())
            .unwrap_or_default()
    };
    let mut out = vec![];
    let mut systems = vec![];
    let mut i = 0;
    while i < rows.len() {
        let h = &rows[i];
        let name = identifier(h, 0);
        let (nconv, ndcbs, ndcln) = (h.id(1)? as usize, h.id(2)? as usize, h.id(3)? as usize);
        let end = i + 1 + nconv + ndcbs + ndcln;
        if end > rows.len() {
            return Err(h.error("truncated multi-terminal DC line"));
        }
        let mode = h.id(4)?;
        if mode > 2 {
            return Err(h.error("multi-terminal DC control mode must be 0, 1 or 2"));
        }
        let vconv = h.integer(5, true)?;
        let vconvn = h.optional(7, 0.0)?.abs() as u32;
        let converters = &rows[i + 1..i + 1 + nconv];
        let dc_buses = &rows[i + 1 + nconv..i + 1 + nconv + ndcbs];
        let dc_links = &rows[i + 1 + nconv + ndcbs..end];
        i = end;
        let Some(holder) = converters
            .iter()
            .find(|c| c.integer(0, true).ok() == Some(vconv))
        else {
            return Err(h.error(format!(
                "multi-terminal DC line {name} has no converter at its voltage-controlling bus {vconv}"
            )));
        };
        // In current mode SETVL is amps; the DC voltage is the holder's SETVL in kV.
        let kv = holder.num(12)?.abs();
        let holder_bus = vconv;
        let holder_vm = terminal_vm(holder, holder_bus, case, indices)?;
        let mut pairs = std::collections::HashSet::new();
        let mut system = MtdcSystem {
            name: name.clone(),
            control: match mode {
                1 => LccControl::Power,
                2 => LccControl::Current,
                _ => LccControl::Blocked,
            },
            voltage_bus: vconv,
            mode_switch_kv: h.optional(6, 0.0)?,
            negative_voltage_bus: vconvn,
            converters: Vec::with_capacity(nconv),
            buses: Vec::with_capacity(ndcbs),
            links: Vec::with_capacity(ndcln),
        };
        for c in converters {
            let bus = c.integer(0, true)?;
            let bridges = c.num(1)?;
            if bridges < 1.0 || bridges.fract() != 0.0 || bridges > f64::from(u32::MAX) {
                return Err(c.error("DC converter bridge count must be a positive integer"));
            }
            let code = c.optional(15, 1.0)?;
            let mut detail = MtdcConverter {
                converter: LccConverter {
                    bus,
                    bridges: bridges as u32,
                    angle_max: c.num(2)?,
                    angle_min: c.num(3)?,
                    resistance: c.num(4)?,
                    reactance: c.num(5)?,
                    base_kv: c.num(6)?,
                    ratio: c.optional(7, 1.0)?,
                    tap: c.optional(8, 1.0)?,
                    tap_max: c.optional(9, 1.5)?,
                    tap_min: c.optional(10, 0.51)?,
                    tap_step: c.optional(11, 0.00625)?,
                    commutating_bus: 0,
                    transformer: None,
                    capacitor: 0.0,
                },
                setpoint: c.num(12)?,
                participation: c.optional(13, 1.0)?,
                margin: c.optional(14, 0.0)?,
                code: code.clamp(f64::from(i32::MIN), f64::from(i32::MAX)) as i32,
                row: None,
            };
            if bus == vconv || (vconvn != 0 && bus == vconvn) {
                system.converters.push(detail);
                continue;
            }
            let setvl = c.num(12)?;
            let p = match mode {
                1 => setvl.abs(),
                2 => setvl.abs() * kv / 1000.0,
                _ => 0.0,
            };
            let vm = terminal_vm(c, bus, case, indices)?;
            // A positive SETVL marks a rectifier, which sends power to the holder.
            let rectifier = setvl >= 0.0;
            let (from, to) = if rectifier {
                (bus, holder_bus)
            } else {
                (holder_bus, bus)
            };
            let (from_vm, to_vm) = if rectifier {
                (vm, holder_vm)
            } else {
                (holder_vm, vm)
            };
            let own = commutated_q_limits(p, c.num(2)?, c.num(3)?);
            let held = commutated_q_limits(p, holder.num(2)?, holder.num(3)?);
            let (qf, qt) = if rectifier { (own, held) } else { (held, own) };
            let mut line = vec![0.0; 17];
            line[0] = from as f64;
            line[1] = to as f64;
            line[2] = f64::from(mode != 0);
            line[3] = p;
            line[4] = p;
            line[7] = from_vm;
            line[8] = to_vm;
            line[9] = 0.85 * p;
            line[10] = 1.15 * p;
            line[11] = qf.0;
            line[12] = qf.1;
            line[13] = qt.0;
            line[14] = qt.1;
            if line.iter().any(|n| !n.is_finite()) {
                return Err(c.error("nonfinite converted multi-terminal DC converter"));
            }
            // Two converters on the same AC bus would share an id; number repeats.
            let key = (from.min(to), from.max(to));
            let mut row_name = name.clone();
            let mut n = 1;
            while !pairs.insert((key, row_name.clone())) {
                n += 1;
                row_name = format!("{name}#{n}");
            }
            detail.row = Some(out.len());
            system.converters.push(detail);
            out.push((row_name, line));
        }
        for b in dc_buses {
            system.buses.push(MtdcBus {
                number: b.id(0)?,
                ac_bus: b.optional(1, 0.0)?.abs() as u32,
                second: b.optional(5, 0.0)?.abs() as u32,
                name: text(b, 4),
                ground_resistance: b.optional(6, 0.0)?,
                area: b.optional(2, 1.0)?.abs() as u32,
                zone: b.optional(3, 1.0)?.abs() as u32,
                owner: b.optional(7, 1.0)?.abs() as u32,
            });
        }
        for l in dc_links {
            // Revision 31 added the metered end MET before RDC.
            let shift = usize::from(rev >= 31);
            let circuit = text(l, 2);
            system.links.push(MtdcLink {
                from: l.id(0)?,
                to: l.integer(1, true)?,
                circuit: if circuit.is_empty() {
                    "1".into()
                } else {
                    circuit
                },
                metered_end: if shift == 1 && l.optional(3, 1.0)? >= 2.0 {
                    2
                } else {
                    1
                },
                resistance: l.num(3 + shift)?,
                inductance: l.optional(4 + shift, 0.0)?,
            });
        }
        if vconvn != 0 {
            ctx.warn(
                h.line,
                true,
                format!("in the simplified DC model, multi-terminal DC line {name}: negative-pole voltage-controlling converter {vconvn} is held at 0 MW"),
            )?;
        }
        let mut ignored = vec![];
        if system.converters.iter().any(|c| c.margin != 0.0) {
            ignored.push("DCPF and MARG order sharing");
        }
        if system.buses.iter().any(|b| b.ground_resistance != 0.0) {
            ignored.push("RGRND");
        }
        if !ignored.is_empty() {
            ctx.warn(
                h.line,
                false,
                format!(
                    "the detailed model of multi-terminal DC line {name} ignores {}",
                    ignored.join(", ")
                ),
            )?;
        }
        systems.push(system);
    }
    ctx.warn(
        rows[0].line,
        true,
        "in the simplified DC model, multi-terminal DC lines are lossless, ignore their DC network, and link each converter to the DC voltage converter",
    )?;
    Ok((out, systems))
}
