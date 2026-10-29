/// This crate's version, for problem reports.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

pub mod model;
pub mod parser;
pub mod topology;
pub use parser::parse_raw;
pub use topology::{analyze, shortest_path};
