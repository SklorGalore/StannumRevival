pub mod model;
pub mod parser;
pub mod topology;
pub use parser::parse_raw;
pub use topology::{analyze, shortest_path};

#[cfg(target_arch = "wasm32")]
mod wasm;
