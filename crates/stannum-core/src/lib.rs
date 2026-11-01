//! Stannum's analysis core, independent of the desktop shell.
//!
//! - [`dc_grids`]: Stannum DC grid files (meshed VSC grids beside a case).
//! - [`attach`]: lets a script drive the running app over a local socket.
//! - [`commands`]: every command the clients use, listed once.
//! - [`contingency`]: N-1 contingency analysis (linear screen, then AC).
//! - [`planning`]: NERC TPL-001 planning events generated from the case.
//! - [`power`]: AC/DC power flow and sensitivities via rs-power.
//! - [`workspace`]: the shared open case, with topology via nodal.
//! - [`derived_topology`]: topology for cases nodal can't read, from the power model.
//! - [`diagram`]: the case as a drawable network for one-line diagrams.
//! - [`diff`]: exact RAW case comparison (ported from case-diff).
//! - [`geo`]: bus coordinates for the map, from a CSV side file.
//! - [`inspect`]: one element's data and limits, for the element inspector.
//! - [`error`]: the typed errors every command returns.
//! - [`elements`]: one id per bus, branch and generator, shared by all modules.
//! - [`record_edit`]: edits from the Records module, written into the case.
//! - [`records`]: the open case's records as tables, one per record type.
//! - [`raw_export`]: the open case written as a complete RAW 33 or 35 file.
//! - [`schema`]: checks values against the command schemas and writes them as TypeScript.
//! - [`run_file`]: run files, commands one per line, run in order.
//! - [`samples`]: small cases built in, to open on first run.
//! - [`results`]: names for every solution the open case can show.
//! - [`topology_wire`]: the topology snapshot as compact columns for the app.
//! - [`logbook`]: the application log shown in the log panel.
//! - [`crash`]: internal errors (panics) logged and answered as errors.
//! - [`paging`]: tables read a page at a time, sorted and filtered in the core.
//! - [`stn`]: Stannum's open case format (`.stn`), to and from RAW.

pub mod attach;
pub mod commands;
pub mod contingency;
pub mod crash;
pub mod dc_grids;
pub mod derived_topology;
pub mod diagram;
pub mod diff;
pub mod elements;
pub mod error;
pub mod file_version;
pub mod file_voltages;
pub mod geo;
pub mod inspect;
pub mod lists;
pub mod logbook;
pub mod paging;
pub mod planning;
pub mod power;
pub mod pv;
pub mod qv;
pub mod raw_export;
pub mod record_edit;
pub mod records;
pub mod results;
pub mod results_file;
pub mod run_file;
pub mod samples;
pub mod scaling;
pub mod schema;
pub mod secure;
pub mod stn;
pub mod study;
pub mod study_file;
pub mod table;
pub mod topology_wire;
pub mod transfer;
pub mod working_case;
pub mod workspace;

pub use error::{Error, ErrorKind};
pub use workspace::{CaseId, CaseSummary, OpenCase, Workspace};
