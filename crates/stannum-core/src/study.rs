//! A study: edits to the open case (outages, generator output, bus load)
//! that every solve uses, without touching the file.

use crate::power::PowerCase;
use serde::{Deserialize, Serialize};

/// One change to the case, by shared element id or bus number.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Edit {
    /// Take a branch or generator out of service, or put it back. Every leg of
    /// a three-winding transformer goes together.
    Status { element: String, online: bool },
    /// A generator's active power output, in MW.
    Generation { element: String, mw: f64 },
    /// A bus's total load, in MW and Mvar.
    Load { bus: u32, mw: f64, mvar: f64 },
}

/// The case with every edit applied in order. Fails on an edit that names
/// nothing in the case, so a typo never passes silently.
pub fn apply(base: &PowerCase, edits: &[Edit]) -> Result<PowerCase, String> {
    let mut power = base.clone();
    for edit in edits {
        match edit {
            Edit::Status { element, online } => {
                let mut found = false;
                for (row, e) in power.elements.branches.iter().enumerate() {
                    if &e.element == element {
                        power.case.branches[row].online = *online;
                        found = true;
                    }
                }
                for (row, e) in power.elements.generators.iter().enumerate() {
                    if &e.element == element {
                        power.case.generators[row].online = *online;
                        found = true;
                    }
                }
                if !found {
                    return Err(format!(
                        "{element} isn't a branch or generator in this case."
                    ));
                }
            }
            Edit::Generation { element, mw } => {
                let row = power
                    .elements
                    .generators
                    .iter()
                    .position(|e| &e.element == element)
                    .ok_or_else(|| format!("{element} isn't a generator in this case."))?;
                power.case.generators[row].pg = *mw;
            }
            Edit::Load { bus, mw, mvar } => {
                let b = power
                    .case
                    .buses
                    .iter_mut()
                    .find(|b| b.id == *bus)
                    .ok_or_else(|| format!("Bus {bus} isn't in this case."))?;
                b.pd = *mw;
                b.qd = *mvar;
            }
        }
    }
    Ok(power)
}

/// A solve error in study terms: an outage that splits off part of the
/// network is the usual cause when edits are present.
pub fn explain(error: String, edits: &[Edit]) -> String {
    if edits.is_empty() {
        return error;
    }
    if error.contains("island") {
        return format!(
            "The study's outages split the network, and a part without a reference bus can't be solved. Put a branch back in service. ({error})"
        );
    }
    format!("{error} (with the study's edits applied)")
}
