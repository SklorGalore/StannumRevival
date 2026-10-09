use crate::{Branch, Bus, BusType, Case, Error, Generator};
use std::{collections::HashMap, path::Path};

#[derive(Clone, Debug, PartialEq)]
enum Kind {
    Word(String),
    Number(f64),
    String(String),
    Symbol(char),
    Newline,
}
#[derive(Clone, Debug)]
struct Token {
    kind: Kind,
    line: usize,
    column: usize,
    end_column: usize,
}
impl Token {
    fn error(&self, message: impl Into<String>) -> Error {
        Error::Parse {
            line: self.line,
            column: self.column,
            message: message.into(),
        }
    }
}

fn lex(input: &str) -> Result<Vec<Token>, Error> {
    log::trace!("lexing MATPOWER input ({} bytes)", input.len());
    let chars: Vec<char> = input.chars().collect();
    let (mut i, mut line, mut col) = (0, 1, 1);
    let mut out = Vec::new();
    while i < chars.len() {
        let c = chars[i];
        let start = Token {
            kind: Kind::Newline,
            line,
            column: col,
            end_column: col + 1,
        };
        if c == '%'
            || (c == '.' && chars.get(i + 1) == Some(&'.') && chars.get(i + 2) == Some(&'.'))
        {
            let continuation = c == '.';
            while i < chars.len() && chars[i] != '\n' {
                i += 1;
                col += 1;
            }
            if continuation && i < chars.len() {
                i += 1;
                line += 1;
                col = 1;
            }
            continue;
        }
        if c == '\n' {
            out.push(start);
            i += 1;
            line += 1;
            col = 1;
            continue;
        }
        if c.is_whitespace() {
            i += 1;
            col += 1;
            continue;
        }
        let begin = i;
        let kind = if c == '\'' {
            i += 1;
            let mut s = String::new();
            loop {
                if i >= chars.len() || chars[i] == '\n' {
                    return Err(start.error("unterminated string"));
                }
                if chars[i] == '\'' {
                    i += 1;
                    if chars.get(i) == Some(&'\'') {
                        s.push('\'');
                        i += 1;
                    } else {
                        break;
                    }
                } else {
                    s.push(chars[i]);
                    i += 1;
                }
            }
            Kind::String(s)
        } else if c.is_ascii_digit()
            || (c == '.' && chars.get(i + 1).is_some_and(char::is_ascii_digit))
            || c == '+'
            || c == '-'
        {
            if c == '+' || c == '-' {
                i += 1;
            }
            if chars
                .get(i..i + 3)
                .is_some_and(|s| s.iter().collect::<String>() == "Inf")
            {
                i += 3;
            } else {
                while i < chars.len() && chars[i].is_ascii_digit() {
                    i += 1;
                }
                if chars.get(i) == Some(&'.') {
                    i += 1;
                    while i < chars.len() && chars[i].is_ascii_digit() {
                        i += 1;
                    }
                }
                if chars.get(i).is_some_and(|c| matches!(c, 'e' | 'E')) {
                    i += 1;
                    if chars.get(i).is_some_and(|c| matches!(c, '+' | '-')) {
                        i += 1;
                    }
                    while i < chars.len() && chars[i].is_ascii_digit() {
                        i += 1;
                    }
                }
            }
            let s: String = chars[begin..i].iter().collect();
            Kind::Number(
                s.parse()
                    .map_err(|_| start.error("expected a numeric literal"))?,
            )
        } else if c.is_ascii_alphabetic() || c == '_' {
            i += 1;
            while i < chars.len() && (chars[i].is_ascii_alphanumeric() || chars[i] == '_') {
                i += 1;
            }
            let s: String = chars[begin..i].iter().collect();
            if s == "Inf" {
                Kind::Number(f64::INFINITY)
            } else {
                Kind::Word(s)
            }
        } else if "[]=;,.(){}".contains(c) {
            i += 1;
            Kind::Symbol(c)
        } else {
            return Err(start.error(format!("unsupported character {c:?}")));
        };
        col += i - begin;
        out.push(Token {
            kind,
            end_column: col,
            ..start
        });
    }
    out.push(Token {
        kind: Kind::Newline,
        line,
        column: col,
        end_column: col,
    });
    log::trace!("finished lexing MATPOWER input ({} tokens)", out.len());
    Ok(out)
}

struct Parser {
    tokens: Vec<Token>,
    pos: usize,
}
impl Parser {
    fn peek(&self) -> &Token {
        &self.tokens[self.pos.min(self.tokens.len() - 1)]
    }
    fn take(&mut self) -> Token {
        let t = self.peek().clone();
        self.pos += 1;
        t
    }
    fn expect(&mut self, kind: Kind) -> Result<(), Error> {
        let t = self.take();
        if t.kind == kind {
            Ok(())
        } else {
            Err(t.error(format!("expected {kind:?}, found {:?}", t.kind)))
        }
    }
    fn separators(&mut self) {
        while self.pos < self.tokens.len()
            && matches!(self.peek().kind, Kind::Newline | Kind::Symbol(';'))
        {
            self.pos += 1;
        }
    }
    fn matrix(&mut self) -> Result<Vec<Vec<f64>>, Error> {
        self.expect(Kind::Symbol('['))?;
        let mut rows = Vec::new();
        let mut row = Vec::new();
        loop {
            if self.pos >= self.tokens.len() {
                return Err(self.peek().error("unterminated matrix"));
            }
            let t = self.take();
            match t.kind {
                Kind::Number(n) => {
                    // Adjacent literals such as 1+2 are expressions, not columns.
                    if let Some(prev) = self.tokens.get(self.pos.saturating_sub(2))
                        && matches!(prev.kind, Kind::Number(_))
                        && prev.line == t.line
                        && prev.end_column == t.column
                    {
                        return Err(t.error("numeric columns need whitespace or commas; expressions are unsupported"));
                    }
                    row.push(n);
                }
                Kind::Symbol(',') => {}
                Kind::Newline | Kind::Symbol(';') | Kind::Symbol(']') => {
                    if !row.is_empty() {
                        rows.push(std::mem::take(&mut row));
                    }
                    if t.kind == Kind::Symbol(']') {
                        break;
                    }
                }
                _ => {
                    return Err(t.error(
                        "expected a numeric matrix; executable expressions are unsupported",
                    ));
                }
            }
        }
        if rows.windows(2).any(|w| w[0].len() != w[1].len()) {
            return Err(self.peek().error("matrix rows have different lengths"));
        }
        Ok(rows)
    }
}

/// Parse literal MATPOWER version-2 data, without executing MATLAB code.
///
/// Accepts numeric tables and supported string labels, preserving row order and
/// extra columns. Expressions and unknown fields return source-located
/// [`Error::Parse`] errors. This checks syntax and table shape; [`crate::solve`]
/// validates physical inputs and topology.
pub fn parse_matpower(input: &str) -> Result<Case, Error> {
    log::debug!("parsing MATPOWER case ({} bytes)", input.len());
    let mut p = Parser {
        tokens: lex(input)?,
        pos: 0,
    };
    let mut matrices = HashMap::new();
    let mut base = None;
    let mut version = false;
    let mut labels = std::collections::BTreeMap::new();
    p.separators();
    if p.peek().kind == Kind::Word("function".into()) {
        p.take();
        p.expect(Kind::Word("mpc".into()))?;
        p.expect(Kind::Symbol('='))?;
        let name = p.take();
        if !matches!(name.kind, Kind::Word(_)) {
            return Err(name.error("expected case function name"));
        }
        if p.peek().kind == Kind::Symbol('(') {
            p.take();
            p.expect(Kind::Symbol(')'))?;
        }
        if p.peek().kind == Kind::Symbol(';') {
            p.take();
        }
        p.expect(Kind::Newline)?;
    }
    loop {
        p.separators();
        if p.pos >= p.tokens.len() {
            break;
        }
        if p.peek().kind == Kind::Word("end".into()) {
            p.take();
            p.separators();
            if p.pos < p.tokens.len() {
                return Err(p.peek().error("unexpected content after end"));
            }
            break;
        }
        p.expect(Kind::Word("mpc".into()))?;
        p.expect(Kind::Symbol('.'))?;
        let field = p.take();
        let Kind::Word(ref name) = field.kind else {
            return Err(field.error("expected case field"));
        };
        p.expect(Kind::Symbol('='))?;
        match name.as_str() {
            "version" => {
                if version {
                    return Err(field.error("duplicate version"));
                }
                p.expect(Kind::String("2".into()))?;
                version = true;
            }
            "baseMVA" => {
                if base.is_some() {
                    return Err(field.error("duplicate baseMVA"));
                }
                let t = p.take();
                let Kind::Number(n) = t.kind else {
                    return Err(t.error("expected numeric baseMVA"));
                };
                base = Some(n);
            }
            "bus" | "gen" | "branch" | "gencost" | "dcline" => {
                if matrices.contains_key(name) {
                    return Err(field.error("duplicate matrix"));
                }
                matrices.insert(name.clone(), p.matrix()?);
            }
            "bus_name" | "gentype" | "genfuel" => {
                if labels.contains_key(name) {
                    return Err(field.error("duplicate label table"));
                }
                p.expect(Kind::Symbol('{'))?;
                let mut values = Vec::new();
                loop {
                    if p.pos >= p.tokens.len() {
                        return Err(p.peek().error("unterminated label table"));
                    }
                    let t = p.take();
                    match t.kind {
                        Kind::String(s) => values.push(s),
                        Kind::Newline | Kind::Symbol(';') => {}
                        Kind::Symbol('}') => break,
                        _ => return Err(t.error("expected literal string label table")),
                    }
                }
                labels.insert(name.clone(), values);
            }
            _ => return Err(field.error(format!("unsupported field {name}"))),
        }
        if !matches!(p.peek().kind, Kind::Newline | Kind::Symbol(';')) {
            return Err(p
                .peek()
                .error("expected assignment terminator; expressions are unsupported"));
        }
    }
    let missing = |s: &str| Error::InvalidCase(format!("missing {s}"));
    if !version {
        return Err(missing("version = '2'"));
    }
    let mut rows = |name: &str, min: usize| -> Result<Vec<Vec<f64>>, Error> {
        let data = matrices.remove(name).ok_or_else(|| missing(name))?;
        if data.iter().any(|r| r.len() < min) {
            return Err(Error::InvalidCase(format!(
                "{name} requires at least {min} columns"
            )));
        }
        Ok(data)
    };
    let bus = rows("bus", 13)?;
    let gen_rows = rows("gen", 10)?;
    let branch = rows("branch", 13)?;
    fn id(n: f64) -> Result<u32, Error> {
        if !n.is_finite() || n < 0.0 || n > u32::MAX as f64 || n.fract() != 0.0 {
            Err(Error::InvalidCase(format!(
                "invalid integer identifier {n}"
            )))
        } else {
            Ok(n as u32)
        }
    }
    let buses = bus
        .into_iter()
        .map(|r| {
            Ok(Bus {
                id: id(r[0])?,
                kind: match r[1] {
                    1.0 => BusType::Pq,
                    2.0 => BusType::Pv,
                    3.0 => BusType::Reference,
                    4.0 => BusType::Isolated,
                    _ => return Err(Error::InvalidCase("invalid bus type".into())),
                },
                pd: r[2],
                qd: r[3],
                gs: r[4],
                bs: r[5],
                area: id(r[6])?,
                vm: r[7],
                va: r[8],
                base_kv: r[9],
                zone: id(r[10])?,
                vmax: r[11],
                vmin: r[12],
                extra: r[13..].to_vec(),
            })
        })
        .collect::<Result<Vec<_>, Error>>()?;
    let generators = gen_rows
        .into_iter()
        .map(|r| {
            Ok(Generator {
                bus: id(r[0])?,
                pg: r[1],
                qg: r[2],
                qmax: r[3],
                qmin: r[4],
                vg: r[5],
                mbase: r[6],
                online: r[7] > 0.0,
                pmax: r[8],
                pmin: r[9],
                extra: r[10..].to_vec(),
            })
        })
        .collect::<Result<Vec<_>, Error>>()?;
    let branches = branch
        .into_iter()
        .map(|r| {
            if r[10] != 0.0 && r[10] != 1.0 {
                return Err(Error::InvalidCase("branch status must be 0 or 1".into()));
            }
            Ok(Branch {
                from: id(r[0])?,
                to: id(r[1])?,
                r: r[2],
                x: r[3],
                b: r[4],
                rate_a: r[5],
                rate_b: r[6],
                rate_c: r[7],
                tap: r[8],
                shift: r[9],
                online: r[10] > 0.0,
                angmin: r[11],
                angmax: r[12],
                extra: r[13..].to_vec(),
            })
        })
        .collect::<Result<Vec<_>, Error>>()?;
    let dcline = matrices.remove("dcline");
    if dcline
        .as_ref()
        .is_some_and(|rows| rows.iter().any(|r| r.len() < 17))
    {
        return Err(Error::InvalidCase(
            "dcline requires at least 17 columns".into(),
        ));
    }
    let case = Case {
        base_mva: base.ok_or_else(|| missing("baseMVA"))?,
        buses,
        generators,
        branches,
        dcline,
        gencost: matrices.remove("gencost"),
        labels,
    };
    log::info!(
        "parsed MATPOWER case: {} buses, {} generators, {} branches, base {} MVA",
        case.buses.len(),
        case.generators.len(),
        case.branches.len(),
        case.base_mva
    );
    Ok(case)
}

/// Read and parse a literal MATPOWER `.m` case.
///
/// Returns [`Error::Io`] on read failure (including invalid UTF-8), or errors
/// from [`parse_matpower`].
pub fn load_matpower(path: impl AsRef<Path>) -> Result<Case, Error> {
    let path = path.as_ref();
    log::info!("loading MATPOWER case from {}", path.display());
    let input = std::fs::read_to_string(path)?;
    log::debug!("read {} bytes from {}", input.len(), path.display());
    parse_matpower(&input)
}
