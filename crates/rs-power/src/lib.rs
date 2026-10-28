#![doc = include_str!("../README.md")]
mod case;
mod controls;
mod dc;
mod facts;
mod flow_controls;
mod input;
mod interchange;
mod mtdc;
mod network;
mod outage;
mod output;
mod raw;
mod sensitivity;
mod solver;
mod vsc;
mod vsc_grid;

pub use case::{Branch, Bus, BusType, Case, Generator};
pub use controls::{ShuntMode, SwitchedShunt, TapControl, VoltageControls};
pub use dc::{LccControl, LccConverter, LccEnd, LccLine, LccMode, LccSolution};
pub use facts::{FactsDevice, FactsSolution};
pub use flow_controls::{Asymmetric, DcLineTap, FlowControls, PhaseShifter, ReactiveTap};
pub use input::{load_matpower, parse_matpower};
pub use interchange::{Area, AreaInterchange, AreaLoad, AreaSolution, InterchangeMode};
pub use mtdc::{
    MtdcBus, MtdcBusSolution, MtdcConverter, MtdcConverterSolution, MtdcLink, MtdcLinkSolution,
    MtdcMode, MtdcSolution, MtdcSystem,
};
pub use outage::{DcFactors, OutageFlows};
pub use output::{format_matpower, save_matpower};
pub use raw::{
    BranchKind, BranchSource, DcLineKind, GeneratorSource, RawImport, RawOptions, RawSubstation,
    RawWarning, SwitchType, SwitchingDevice, load_raw, load_raw_with_options, parse_raw,
    parse_raw_with_options,
};
pub use sensitivity::{
    BranchFlowDerivatives, BusPowerDerivatives, InjectionVariable, LodfBridgeHandling,
    PowerFlowJacobian, PowerFlowSensitivity, SensitivityColumn, SensitivityModel,
    SquaredMagnitudeDerivatives, StateVariable, VoltageCoordinates, branch_flow_derivatives,
    bus_power_derivatives, calculate_sensitivities, make_jacobian, make_lodf,
    make_lodf_with_bridge_handling, make_ptdf, make_ptdf_columns, make_ptdf_transfers,
    make_ptdf_with_slack_matrix,
};
pub use solver::{
    AcMethod, DcModel, Initialization, Model, PowerFlowOptions, PowerFlowResult, SolveStatus, solve,
};
pub use vsc::{VscAcControl, VscConverter, VscDcControl, VscLine, VscSolution};
pub use vsc_grid::{
    AcControl, AcDcResult, AcState, ConverterLimits, ConverterLosses, DcControl, DcGrid, DcGridBus,
    DcGridBusSolution, DcGridLine, DcGridLineSolution, DcGridSettings, DcGridSolution,
    GridConverter, GridConverterSolution, check_dc_grids, solve_dc_grids,
};

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
    /// The solve was stopped through [`PowerFlowOptions::cancel`].
    #[error("cancelled")]
    Cancelled,
}
