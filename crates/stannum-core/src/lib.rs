//! Stannum's analysis core, independent of the desktop shell.
//!
//! - [`power`]: AC/DC power flow and sensitivities via rs-power.
//! - [`workspace`]: the shared open case, with topology via nodal.
//! - [`diagram`]: the case as a drawable network for one-line diagrams.
//! - [`diff`]: exact RAW case comparison (ported from case-diff).
//! - [`elements`]: one id per bus, branch and generator, shared by all modules.

pub mod diagram;
pub mod diff;
pub mod elements;
pub mod power;
pub mod workspace;

pub use workspace::{CaseSummary, OpenCase, Workspace};
