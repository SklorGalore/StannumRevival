#![doc = include_str!("../README.md")]
mod case;
mod input;
mod network;
mod output;
mod raw;
mod solver;

pub use case::{Branch, Bus, BusType, Case, Generator};
pub use input::{load_matpower, parse_matpower};
pub use output::{format_matpower, save_matpower};
pub use raw::{
    RawImport, RawOptions, RawWarning, load_raw, load_raw_with_options, parse_raw,
    parse_raw_with_options,
};
pub use solver::{Initialization, Model, PowerFlowOptions, PowerFlowResult, SolveStatus, solve};

/// Input, topology, or numerical failure. Nonconvergence is a result status.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// File read or write failure.
    #[error("{0}")]
    Io(#[from] std::io::Error),
    /// Malformed source data with a one-based location.
    #[error("line {line}, column {column}: {message}")]
    Parse {
        /// One-based source line.
        line: usize,
        /// One-based source column.
        column: usize,
        /// Description of the invalid input.
        message: String,
    },
    /// Invalid case data, serialization input, or solver options.
    #[error("invalid case: {0}")]
    InvalidCase(String),
    /// Unsupported topology, options, revision, or strict RAW conversion.
    #[error("unsupported configuration: {0}")]
    Unsupported(String),
    /// Singular system, failed factorization, or non-finite numerical result.
    #[error("numerical failure: {0}")]
    Numerical(String),
}
