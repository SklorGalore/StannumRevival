//! Stannum's analysis core, independent of the desktop shell.
//!
//! - [`dc_grids`]: Stannum DC grid files (meshed VSC grids beside a case).
//! - [`contingency`]: N-1 contingency analysis (linear screen, then AC).
//! - [`power`]: AC/DC power flow and sensitivities via rs-power.
//! - [`workspace`]: the shared open case, with topology via nodal.
//! - [`derived_topology`]: topology for cases nodal can't read, from the power model.
//! - [`diagram`]: the case as a drawable network for one-line diagrams.
//! - [`diff`]: exact RAW case comparison (ported from case-diff).
//! - [`inspect`]: one element's data and limits, for the element inspector.
//! - [`elements`]: one id per bus, branch and generator, shared by all modules.
//! - [`record_edit`]: edits from the Records module, written into the case.
//! - [`records`]: the open case's records as tables, one per record type.
//! - [`logbook`]: the application log shown in the log panel.

pub mod contingency;
pub mod dc_grids;
pub mod derived_topology;
pub mod diagram;
pub mod diff;
pub mod elements;
pub mod inspect;
pub mod logbook;
pub mod power;
pub mod record_edit;
pub mod records;
pub mod study;
pub mod workspace;

pub use workspace::{CaseSummary, OpenCase, Workspace};
