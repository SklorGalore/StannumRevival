//! Named results: every solution the open case can show, by one name that
//! the frontend, the log and (later) Python share.
//!
//! - `base`: the latest solve of the case as filed.
//! - `study`: the latest solve, with the study's edits applied.
//! - `contingency <rank>`: one outage of the last contingency run, solved in AC.
//! - `trip <element>`: the last "Trip and compare".

use std::fmt;
use std::str::FromStr;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ResultName {
    Base,
    Study,
    Contingency(usize),
    Trip(String),
}

impl fmt::Display for ResultName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ResultName::Base => f.write_str("base"),
            ResultName::Study => f.write_str("study"),
            ResultName::Contingency(rank) => write!(f, "contingency {rank}"),
            ResultName::Trip(element) => write!(f, "trip {element}"),
        }
    }
}

impl FromStr for ResultName {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, String> {
        let s = s.trim();
        let bad = || {
            format!(
                "No result named \"{s}\". Results are base, study, contingency <rank> and trip <element>."
            )
        };
        match s.split_once(' ') {
            None if s == "base" => Ok(ResultName::Base),
            None if s == "study" => Ok(ResultName::Study),
            Some(("contingency", rank)) => rank
                .trim()
                .parse()
                .ok()
                .filter(|&r| r > 0)
                .map(ResultName::Contingency)
                .ok_or_else(bad),
            Some(("trip", element)) if !element.trim().is_empty() => {
                Ok(ResultName::Trip(element.trim().to_string()))
            }
            _ => Err(bad()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_round_trip() {
        for name in [
            ResultName::Base,
            ResultName::Study,
            ResultName::Contingency(3),
            ResultName::Trip("line:1-2-1".into()),
        ] {
            assert_eq!(name.to_string().parse::<ResultName>(), Ok(name));
        }
    }

    #[test]
    fn bad_names_are_refused() {
        for bad in [
            "",
            "basecase",
            "contingency",
            "contingency 0",
            "contingency x",
            "trip ",
            "solve 2",
        ] {
            assert!(bad.parse::<ResultName>().is_err(), "{bad:?}");
        }
    }
}
