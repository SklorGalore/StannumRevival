//! Stannum's analysis core, independent of the desktop shell.
//!
//! - [`power`]: AC/DC power flow and sensitivities via rs-power.
//! - [`workspace`]: the shared open case, with topology via nodal.
//! - [`diff`]: exact RAW case comparison (ported from case-diff).

pub mod diff;
pub mod power;
pub mod workspace;

pub use workspace::{CaseSummary, OpenCase, Workspace};
