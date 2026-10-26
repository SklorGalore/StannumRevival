//! Stannum's analysis core, independent of the desktop shell.
//!
//! - [`dc_grids`]: Stannum DC grid files (meshed VSC grids beside a case).
//! - [`power`]: AC/DC power flow and sensitivities via rs-power.
//! - [`workspace`]: the shared open case, with topology via nodal.
//! - [`derived_topology`]: topology for cases nodal can't read, from the power model.
//! - [`diagram`]: the case as a drawable network for one-line diagrams.
//! - [`diff`]: exact RAW case comparison (ported from case-diff).
//! - [`elements`]: one id per bus, branch and generator, shared by all modules.
//! - [`logbook`]: the application log shown in the log panel.

pub mod dc_grids;
pub mod derived_topology;
pub mod diagram;
pub mod diff;
pub mod elements;
pub mod logbook;
pub mod power;
pub mod study;
pub mod workspace;

pub use workspace::{CaseSummary, OpenCase, Workspace};
