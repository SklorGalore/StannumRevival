#![doc = include_str!("../README.md")]
mod case;
mod input;
mod network;
mod output;
mod solver;

pub use case::{Branch, Bus, BusType, Case, Generator};
pub use input::{load_matpower, parse_matpower};
pub use output::{format_matpower, save_matpower};
pub use solver::{Model, PowerFlowOptions, PowerFlowResult, SolveStatus, solve};

/// Input, topology, or numerical failure. Nonconvergence is a result status.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("{0}")]
    Io(#[from] std::io::Error),
    #[error("line {line}, column {column}: {message}")]
    Parse {
        line: usize,
        column: usize,
        message: String,
    },
    #[error("invalid case: {0}")]
    InvalidCase(String),
    #[error("unsupported configuration: {0}")]
    Unsupported(String),
    #[error("numerical failure: {0}")]
    Numerical(String),
}
