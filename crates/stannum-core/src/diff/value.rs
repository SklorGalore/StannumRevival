//! Exact RAW field values.
//!
//! Numbers compare by exact decimal value with no tolerance: `1.0` equals
//! `1.000`, but any change in a significant digit is a difference. The display
//! form follows Python's `Decimal`/`int` string rules so results read the same
//! as the original case-diff tool.

use std::fmt;

/// An exact decimal: sign, significant digits and base-10 exponent.
#[derive(Clone, Debug)]
pub struct Number {
    negative: bool,
    /// Coefficient digits without leading zeros ("0" for zero).
    digits: String,
    exponent: i64,
    integer: bool,
}

impl Number {
    /// Parse a base-10 integer token (`[+-]?[0-9]+`).
    pub fn parse_integer(token: &str) -> Option<Self> {
        let (negative, body) = split_sign(token);
        if body.is_empty() || !body.bytes().all(|b| b.is_ascii_digit()) {
            return None;
        }
        Some(Self::new(negative, body, 0, true))
    }

    /// Parse a decimal or scientific token
    /// (`[+-]?(digits[.digits?]|.digits)([eE][+-]?digits)?`).
    pub fn parse_number(token: &str) -> Option<Self> {
        let (negative, body) = split_sign(token);
        let (mantissa, exponent) = match body.find(['e', 'E']) {
            Some(at) => {
                let (m, e) = body.split_at(at);
                let e = &e[1..];
                let (eneg, edigits) = split_sign(e);
                if edigits.is_empty() || !edigits.bytes().all(|b| b.is_ascii_digit()) {
                    return None;
                }
                // Clamp absurd exponents instead of overflowing; they still
                // compare exactly against anything a RAW file can hold.
                let value: i64 = edigits.parse().unwrap_or(i64::MAX / 4).min(i64::MAX / 4);
                (m, if eneg { -value } else { value })
            }
            None => (body, 0),
        };
        let (int_part, frac_part) = match mantissa.split_once('.') {
            Some((i, f)) => (i, f),
            None => (mantissa, ""),
        };
        if int_part.is_empty() && frac_part.is_empty() {
            return None;
        }
        if !int_part.bytes().all(|b| b.is_ascii_digit())
            || !frac_part.bytes().all(|b| b.is_ascii_digit())
        {
            return None;
        }
        let digits = format!("{int_part}{frac_part}");
        Some(Self::new(
            negative,
            &digits,
            exponent - frac_part.len() as i64,
            false,
        ))
    }

    fn new(negative: bool, digits: &str, exponent: i64, integer: bool) -> Self {
        let trimmed = digits.trim_start_matches('0');
        let digits = if trimmed.is_empty() { "0" } else { trimmed };
        Self {
            // Python ints have no negative zero; Decimal keeps the sign.
            negative: negative && !(integer && digits == "0"),
            digits: digits.to_string(),
            exponent,
            integer,
        }
    }

    /// Value-only identity: trailing zeros and sign of zero are ignored.
    fn canonical(&self) -> (bool, &str, i64) {
        if self.digits == "0" {
            return (false, "0", 0);
        }
        let stripped = self.digits.trim_end_matches('0');
        let shift = (self.digits.len() - stripped.len()) as i64;
        (self.negative, stripped, self.exponent + shift)
    }
}

fn split_sign(token: &str) -> (bool, &str) {
    match token.as_bytes().first() {
        Some(b'-') => (true, &token[1..]),
        Some(b'+') => (false, &token[1..]),
        _ => (false, token),
    }
}

impl PartialEq for Number {
    fn eq(&self, other: &Self) -> bool {
        self.canonical() == other.canonical()
    }
}
impl Eq for Number {}

impl std::hash::Hash for Number {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.canonical().hash(state);
    }
}

impl fmt::Display for Number {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let sign = if self.negative { "-" } else { "" };
        if self.integer {
            return write!(f, "{sign}{}", self.digits);
        }
        // Python's Decimal.__str__ (scientific, not engineering, notation).
        let len = self.digits.len() as i64;
        let left = self.exponent + len;
        let dot = if self.exponent <= 0 && left > -6 {
            left
        } else {
            1
        };
        let (int_part, frac_part) = if dot <= 0 {
            (
                "0".to_string(),
                format!(".{}{}", "0".repeat((-dot) as usize), self.digits),
            )
        } else if dot >= len {
            (
                format!("{}{}", self.digits, "0".repeat((dot - len) as usize)),
                String::new(),
            )
        } else {
            (
                self.digits[..dot as usize].to_string(),
                format!(".{}", &self.digits[dot as usize..]),
            )
        };
        let exp = if left == dot {
            String::new()
        } else {
            format!("E{:+}", left - dot)
        };
        write!(f, "{sign}{int_part}{frac_part}{exp}")
    }
}

/// One decoded field.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Value {
    Number(Number),
    Text(String),
}

impl Value {
    /// Text form used in tables: strings are quoted so padding is visible.
    pub fn display(&self) -> String {
        match self {
            Self::Number(n) => n.to_string(),
            Self::Text(s) => python_repr(s),
        }
    }

    pub fn as_text(&self) -> Option<&str> {
        match self {
            Self::Text(s) => Some(s),
            Self::Number(_) => None,
        }
    }

    /// Key form: identifier padding is not significant for matching.
    pub fn normalized(&self) -> Self {
        match self {
            Self::Text(s) => Self::Text(s.trim().to_string()),
            other => other.clone(),
        }
    }
}

/// Python `repr(str)`: single quotes unless the text has only single quotes.
pub fn python_repr(s: &str) -> String {
    let quote = if s.contains('\'') && !s.contains('"') {
        '"'
    } else {
        '\''
    };
    let mut out = String::with_capacity(s.len() + 2);
    out.push(quote);
    for ch in s.chars() {
        match ch {
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if c == quote => {
                out.push('\\');
                out.push(c);
            }
            c if (c as u32) < 0x20 || (0x7f..0xa0).contains(&(c as u32)) => {
                out.push_str(&format!("\\x{:02x}", c as u32));
            }
            c => out.push(c),
        }
    }
    out.push(quote);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn num(s: &str) -> Number {
        Number::parse_number(s).unwrap()
    }

    #[test]
    fn exact_numeric_equality() {
        assert_eq!(num("1.0"), num("1.000"));
        assert_eq!(num("1"), Number::parse_integer("1").unwrap());
        assert_eq!(num("-0.0"), num("0"));
        assert_eq!(num("1e2"), num("100"));
        assert_ne!(num("1.0000000001"), num("1"));
    }

    #[test]
    fn rejects_malformed_numbers() {
        for bad in ["", ".", "1e", "e5", "1.2.3", "--1", "1x", "nan", "inf"] {
            assert!(Number::parse_number(bad).is_none(), "{bad}");
        }
        assert!(Number::parse_integer("1.0").is_none());
    }

    #[test]
    fn displays_like_python_decimal() {
        for (input, expected) in [
            ("1.000", "1.000"),
            (".5", "0.5"),
            ("+1.5", "1.5"),
            ("-0.0", "-0.0"),
            ("1.5e-3", "0.0015"),
            ("1e5", "1E+5"),
            ("00012.5", "12.5"),
            ("1.23e-8", "1.23E-8"),
            ("0.000001", "0.000001"),
            ("0.0000001", "1E-7"),
            ("100", "100"),
        ] {
            assert_eq!(num(input).to_string(), expected, "{input}");
        }
        assert_eq!(Number::parse_integer("+007").unwrap().to_string(), "7");
        assert_eq!(Number::parse_integer("-0").unwrap().to_string(), "0");
    }

    #[test]
    fn repr_quotes_like_python() {
        assert_eq!(python_repr(" ID "), "' ID '");
        assert_eq!(python_repr("O'Brien"), "\"O'Brien\"");
        assert_eq!(python_repr("a'b\"c"), "'a\\'b\"c'");
    }
}
