//! Field values and how they are written as text.

use std::cmp::Ordering;

/// A field's value.
///
/// Fields the schema knows are read as the type it gives them: `int`
/// as [`Value::Int`], `float` as [`Value::Float`], `bool` as [`Value::Bool`],
/// and `status`, `enum`, `string`, `ref` and `element` as [`Value::Str`].
/// Fields it doesn't know keep their text: a bare token as [`Value::Raw`]
/// (so a number is written back exactly as it was), a quoted one as
/// [`Value::Str`].
#[derive(Clone, Debug)]
pub enum Value {
    Int(i64),
    Float(f64),
    Bool(bool),
    Str(String),
    /// A bare token from a field the schema doesn't know.
    Raw(String),
    List(Vec<Value>),
}

impl PartialEq for Value {
    /// Floats are equal when their bits are, so `-0` differs from `0` and a
    /// value that equals its default is exactly the default.
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Value::Int(a), Value::Int(b)) => a == b,
            (Value::Float(a), Value::Float(b)) => a.to_bits() == b.to_bits(),
            (Value::Bool(a), Value::Bool(b)) => a == b,
            (Value::Str(a), Value::Str(b)) | (Value::Raw(a), Value::Raw(b)) => a == b,
            (Value::List(a), Value::List(b)) => a == b,
            _ => false,
        }
    }
}

impl Value {
    pub fn as_f64(&self) -> Option<f64> {
        match self {
            Value::Float(v) => Some(*v),
            Value::Int(v) => Some(*v as f64),
            Value::Raw(s) => parse_float(s),
            _ => None,
        }
    }

    pub fn as_i64(&self) -> Option<i64> {
        match self {
            Value::Int(v) => Some(*v),
            Value::Raw(s) => s.parse().ok(),
            _ => None,
        }
    }

    pub fn as_str(&self) -> Option<&str> {
        match self {
            Value::Str(s) | Value::Raw(s) => Some(s),
            _ => None,
        }
    }

    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Value::Bool(b) => Some(*b),
            _ => None,
        }
    }

    pub fn as_list(&self) -> Option<&[Value]> {
        match self {
            Value::List(l) => Some(l),
            _ => None,
        }
    }

    /// Text for a value, as it appears after `name=`.
    pub fn to_text(&self) -> String {
        let mut out = String::new();
        self.write(&mut out);
        out
    }

    pub(crate) fn write(&self, out: &mut String) {
        match self {
            Value::Int(v) => out.push_str(&v.to_string()),
            Value::Float(v) => out.push_str(&format_float(*v)),
            Value::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
            Value::Str(s) => write_str(s, out),
            Value::Raw(s) => out.push_str(s),
            Value::List(items) => {
                out.push('[');
                for (i, item) in items.iter().enumerate() {
                    if i > 0 {
                        out.push_str(", ");
                    }
                    item.write(out);
                }
                out.push(']');
            }
        }
    }

    /// Order for sorting records: numbers by value, text in natural order
    /// (digit runs compared as numbers, so `9` comes before `10`).
    pub(crate) fn sort_cmp(&self, other: &Value) -> Ordering {
        match (self, other) {
            (Value::Int(a), Value::Int(b)) => a.cmp(b),
            (Value::Float(a), Value::Float(b)) => a.total_cmp(b),
            (Value::Str(a) | Value::Raw(a), Value::Str(b) | Value::Raw(b)) => natural_cmp(a, b),
            (a, b) => a.to_text().cmp(&b.to_text()),
        }
    }
}

/// The shortest text that reads back as the same float: `100`, `0.1`,
/// `1.2345e-7`, `inf`. NaN has no text; the writer refuses it.
pub fn format_float(v: f64) -> String {
    if v.is_infinite() {
        return if v > 0.0 { "inf".into() } else { "-inf".into() };
    }
    // Debug is Rust's shortest round-trip form, with an exponent for very
    // large and very small values.
    let s = format!("{:?}", v + 0.0);
    match s.strip_suffix(".0") {
        Some(whole) => whole.to_string(),
        None => s,
    }
}

/// A float as the format writes it: decimal or exponent form, or `inf` and
/// `-inf`. No NaN, no hex, no `infinity`; `-0` reads as `0`.
pub fn parse_float(s: &str) -> Option<f64> {
    match s {
        "inf" => return Some(f64::INFINITY),
        "-inf" => return Some(f64::NEG_INFINITY),
        _ => {}
    }
    let body = s.strip_prefix(['-', '+']).unwrap_or(s);
    if !body.starts_with(|c: char| c.is_ascii_digit() || c == '.') {
        return None;
    }
    if !body
        .chars()
        .all(|c| c.is_ascii_digit() || matches!(c, '.' | 'e' | 'E' | '-' | '+'))
    {
        return None;
    }
    // Negative zero reads as zero: the format has one zero.
    s.parse::<f64>()
        .ok()
        .filter(|v| v.is_finite())
        .map(|v| v + 0.0)
}

/// Characters a string may hold and still be written without quotes.
pub(crate) fn bare_char(c: char) -> bool {
    c.is_ascii_alphanumeric()
        || matches!(
            c,
            '_' | '.'
                | ':'
                | '+'
                | '-'
                | '/'
                | '&'
                | '*'
                | '@'
                | '!'
                | '?'
                | '\''
                | '('
                | ')'
                | '<'
                | '>'
                | '$'
                | '%'
                | '^'
                | '~'
                | '|'
        )
}

/// Whether a string can be written bare: not empty, only bare characters.
pub(crate) fn is_bare(s: &str) -> bool {
    !s.is_empty() && s.chars().all(bare_char)
}

pub(crate) fn write_str(s: &str, out: &mut String) {
    if is_bare(s) {
        out.push_str(s);
        return;
    }
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c => out.push(c),
        }
    }
    out.push('"');
}

fn natural_cmp(a: &str, b: &str) -> Ordering {
    let (mut a, mut b) = (a, b);
    loop {
        match (a.chars().next(), b.chars().next()) {
            (None, None) => return Ordering::Equal,
            (None, Some(_)) => return Ordering::Less,
            (Some(_), None) => return Ordering::Greater,
            (Some(x), Some(y)) if x.is_ascii_digit() && y.is_ascii_digit() => {
                let da = a.find(|c: char| !c.is_ascii_digit()).unwrap_or(a.len());
                let db = b.find(|c: char| !c.is_ascii_digit()).unwrap_or(b.len());
                let (na, nb) = (
                    a[..da].trim_start_matches('0'),
                    b[..db].trim_start_matches('0'),
                );
                let ord = na.len().cmp(&nb.len()).then_with(|| na.cmp(nb));
                if ord != Ordering::Equal {
                    return ord;
                }
                a = &a[da..];
                b = &b[db..];
            }
            (Some(x), Some(y)) => {
                if x != y {
                    return x.cmp(&y);
                }
                a = &a[x.len_utf8()..];
                b = &b[y.len_utf8()..];
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn floats_are_shortest_and_read_back() {
        for v in [
            0.0,
            1.0,
            100.0,
            0.1,
            1.2345e-7,
            6.02e23,
            -3.5,
            1e300,
            f64::MIN_POSITIVE,
        ] {
            let s = format_float(v);
            assert_eq!(parse_float(&s).map(f64::to_bits), Some(v.to_bits()), "{s}");
        }
        assert_eq!(format_float(100.0), "100");
        // One zero: -0 writes and reads as 0.
        assert_eq!(format_float(-0.0), "0");
        assert_eq!(parse_float("-0").map(f64::to_bits), Some(0.0f64.to_bits()));
        assert_eq!(format_float(0.0125), "0.0125");
        assert_eq!(format_float(f64::NEG_INFINITY), "-inf");
        assert_eq!(parse_float("inf"), Some(f64::INFINITY));
        assert_eq!(parse_float("nan"), None);
        assert_eq!(parse_float("infinity"), None);
        assert_eq!(parse_float("0x10"), None);
    }

    #[test]
    fn strings_are_quoted_only_when_needed() {
        assert_eq!(Value::Str("GEN-A".into()).to_text(), "GEN-A");
        assert_eq!(Value::Str("HILL 230".into()).to_text(), "\"HILL 230\"");
        assert_eq!(Value::Str(String::new()).to_text(), "\"\"");
        assert_eq!(Value::Str("a\"b\\c".into()).to_text(), "\"a\\\"b\\\\c\"");
        assert_eq!(Value::Str("x=1".into()).to_text(), "\"x=1\"");
    }

    #[test]
    fn natural_order_compares_digit_runs_as_numbers() {
        assert_eq!(natural_cmp("9", "10"), Ordering::Less);
        assert_eq!(natural_cmp("205:310:1", "205:1000:1"), Ordering::Less);
        assert_eq!(natural_cmp("a2", "a10"), Ordering::Less);
        assert_eq!(natural_cmp("B", "A"), Ordering::Greater);
    }
}
