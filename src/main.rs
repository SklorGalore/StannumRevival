//! Interactive power-flow command line.
use rs_power::{
    Case, Initialization, InjectionVariable, LodfBridgeHandling, Model, PowerFlowOptions,
    PowerFlowResult, RawOptions, SolveStatus, StateVariable, calculate_sensitivities,
    load_matpower, load_raw_with_options, make_jacobian, make_lodf_with_bridge_handling, make_ptdf,
    make_ptdf_columns, save_matpower, solve,
};
use std::io::{self, BufRead, IsTerminal, Write};
use std::time::Instant;

const SPLASH: &str = r"
  ____  ____        ____   _____        _______ ____
 |  _ \ / ___|      |  _ \ / _ \ \      / / ____|  _ \
 | |_) |\___ \ _____| |_) | | | \ \ /\ / /|  _| | |_) |
 |  _ <  ___) |_____|  __/| |_| |\ V  V / | |___|  _ <
 |_| \_\|____/      |_|    \___/  \_/\_/  |_____|_| \_\
";

const HELP: &str = "Commands:
  load <path>                 Load MATPOWER (.m) or PSS/E (.raw)
  summary                     Show case size and latest solve status
  solve [ac|dc]               Solve the working case
  sensitivities [ac|dc]       Solve and show state and branch-flow sensitivities
  jacobian [reduced|full]     Show the AC power-flow Jacobian
  ptdf [bus-id ...]           Show all or selected DC PTDF columns
  lodf [mask|raw]             Show the DC LODF matrix (default mask)
  buses                       Show latest voltages (or input voltages)
  generators                  Show latest generator outputs
  flows                       Show latest branch flows
  options                     Show solver settings
  set residual pu|mva          Residual display units (default mva)
  set model ac|dc
  set initialization case|flat
  set tolerance <positive number>
  set max_iterations <integer>
  set enforce_q_limits on|off
  set update_on_solve true|false  Update working case after solving (default true)
  save <path.m>               Export latest result (or input); overwrites file
  help                        Show commands
  quit                        Exit (also exit or end-of-input)
Paths may contain spaces; optionally wrap the whole path in quotes.
Add `to <path>` to write a report; `.csv` selects CSV, otherwise UTF-8 text.
Reports: summary, sensitivities, jacobian, ptdf, lodf, buses, generators, flows, options.";

fn init_logging(interactive: bool) {
    let default_filter = if interactive { "rs_power=info" } else { "off" };
    let env = env_logger::Env::default().filter_or("RUST_LOG", default_filter);
    let _ = env_logger::Builder::from_env(env)
        .format_timestamp_millis()
        .try_init();
}

#[derive(Clone, Copy)]
enum ResidualUnits {
    Pu,
    Mva,
}

impl ResidualUnits {
    fn name(self) -> &'static str {
        match self {
            Self::Pu => "pu",
            Self::Mva => "mva",
        }
    }

    fn report(self, result: &PowerFlowResult, out: &mut impl Write) -> io::Result<()> {
        let (value, unit) = match self {
            Self::Pu => (result.residual, "p.u."),
            Self::Mva => (result.residual * result.case.base_mva, "MVA"),
        };
        writeln!(
            out,
            "{:?}: {} iterations, residual {:.3e} {}",
            result.status, result.iterations, value, unit
        )
    }
}

struct Session {
    input: Option<Case>,
    result: Option<PowerFlowResult>,
    options: PowerFlowOptions,
    update_on_solve: bool,
    residual_units: ResidualUnits,
}

impl Default for Session {
    fn default() -> Self {
        Self {
            input: None,
            result: None,
            options: PowerFlowOptions::default(),
            update_on_solve: true,
            residual_units: ResidualUnits::Mva,
        }
    }
}

impl Session {
    fn case(&self) -> Result<&Case, Box<dyn std::error::Error>> {
        self.result
            .as_ref()
            .map(|r| &r.case)
            .or(self.input.as_ref())
            .ok_or_else(|| "load a case first".into())
    }

    fn csv_report(
        &self,
        command: &str,
        args: &str,
        out: &mut impl Write,
    ) -> Result<(), Box<dyn std::error::Error>> {
        match (command, args) {
            ("summary", "") => {
                let case = self.case()?;
                csv_row(out, ["metric", "value", "unit"])?;
                csv_row(
                    out,
                    ["buses".into(), case.buses.len().to_string(), String::new()],
                )?;
                csv_row(
                    out,
                    [
                        "generators".into(),
                        case.generators.len().to_string(),
                        String::new(),
                    ],
                )?;
                csv_row(
                    out,
                    [
                        "branches".into(),
                        case.branches.len().to_string(),
                        String::new(),
                    ],
                )?;
                csv_row(
                    out,
                    ["base_mva".into(), case.base_mva.to_string(), "MVA".into()],
                )?;
                if let Some(result) = &self.result {
                    csv_row(
                        out,
                        [
                            "status".into(),
                            format!("{:?}", result.status),
                            String::new(),
                        ],
                    )?;
                    csv_row(
                        out,
                        [
                            "iterations".into(),
                            result.iterations.to_string(),
                            String::new(),
                        ],
                    )?;
                    let (residual, unit) = match self.residual_units {
                        ResidualUnits::Pu => (result.residual, "p.u."),
                        ResidualUnits::Mva => (result.residual * result.case.base_mva, "MVA"),
                    };
                    csv_row(out, ["residual".into(), residual.to_string(), unit.into()])?;
                }
            }
            ("buses", "") => {
                csv_row(out, ["bus_id", "vm_pu", "va_degrees"])?;
                for bus in &self.case()?.buses {
                    csv_row(
                        out,
                        [bus.id.to_string(), bus.vm.to_string(), bus.va.to_string()],
                    )?;
                }
            }
            ("generators", "") => {
                csv_row(out, ["generator", "bus_id", "pg_mw", "qg_mvar", "online"])?;
                for (index, generator) in self.case()?.generators.iter().enumerate() {
                    csv_row(
                        out,
                        [
                            (index + 1).to_string(),
                            generator.bus.to_string(),
                            generator.pg.to_string(),
                            generator.qg.to_string(),
                            generator.online.to_string(),
                        ],
                    )?;
                }
            }
            ("flows", "") => {
                let result = self.result.as_ref().ok_or("solve a case first")?;
                csv_row(
                    out,
                    [
                        "branch", "from_bus", "to_bus", "pf_mw", "qf_mvar", "pt_mw", "qt_mvar",
                    ],
                )?;
                for (index, (branch, flow)) in result
                    .case
                    .branches
                    .iter()
                    .zip(&result.branch_flows)
                    .enumerate()
                {
                    csv_row(
                        out,
                        [
                            (index + 1).to_string(),
                            branch.from.to_string(),
                            branch.to.to_string(),
                            flow[0].to_string(),
                            flow[1].to_string(),
                            flow[2].to_string(),
                            flow[3].to_string(),
                        ],
                    )?;
                }
            }
            ("jacobian", "" | "reduced" | "full") => {
                let jacobian = make_jacobian(self.case()?, args == "full")?;
                let mut header = vec!["injection".into()];
                header.extend(jacobian.column_variables.iter().copied().map(state_label));
                csv_row(out, header)?;
                for (injection, values) in jacobian.row_variables.iter().zip(&jacobian.values) {
                    let mut row = vec![injection_label(*injection)];
                    row.extend(values.iter().map(ToString::to_string));
                    csv_row(out, row)?;
                }
            }
            ("ptdf", _) => {
                let case = self.case()?;
                let bus_ids = parse_bus_ids(args)?;
                let (columns, values) = if bus_ids.is_empty() {
                    (
                        case.buses.iter().map(|bus| bus.id).collect(),
                        make_ptdf(case, None)?,
                    )
                } else {
                    let values = make_ptdf_columns(case, None, &bus_ids)?;
                    (bus_ids, values)
                };
                let mut header = vec!["branch".into(), "from_bus".into(), "to_bus".into()];
                header.extend(columns.iter().map(|bus| format!("P({bus})")));
                csv_row(out, header)?;
                for (index, (branch, values)) in case.branches.iter().zip(&values).enumerate() {
                    let mut row = vec![
                        (index + 1).to_string(),
                        branch.from.to_string(),
                        branch.to.to_string(),
                    ];
                    row.extend(values.iter().map(ToString::to_string));
                    csv_row(out, row)?;
                }
            }
            ("lodf", "" | "mask" | "raw") => {
                let case = self.case()?;
                let ptdf = make_ptdf(case, None)?;
                let handling = if args == "raw" {
                    LodfBridgeHandling::Raw
                } else {
                    LodfBridgeHandling::Mask
                };
                let values = make_lodf_with_bridge_handling(case, &ptdf, handling)?;
                let mut header = vec!["branch".into(), "from_bus".into(), "to_bus".into()];
                header.extend((1..=case.branches.len()).map(|branch| format!("L({branch})")));
                csv_row(out, header)?;
                for (index, (branch, values)) in case.branches.iter().zip(&values).enumerate() {
                    let mut row = vec![
                        (index + 1).to_string(),
                        branch.from.to_string(),
                        branch.to.to_string(),
                    ];
                    row.extend(values.iter().map(ToString::to_string));
                    csv_row(out, row)?;
                }
            }
            ("options", "") => {
                csv_row(out, ["setting", "value", "unit"])?;
                for row in [
                    [
                        "model".into(),
                        model_name(self.options.model).into(),
                        "".into(),
                    ],
                    [
                        "initialization".into(),
                        format!("{:?}", self.options.initialization),
                        "".into(),
                    ],
                    [
                        "tolerance".into(),
                        self.options.tolerance.to_string(),
                        "p.u.".into(),
                    ],
                    [
                        "max_iterations".into(),
                        self.options.max_iterations.to_string(),
                        "".into(),
                    ],
                    [
                        "enforce_q_limits".into(),
                        self.options.enforce_q_limits.to_string(),
                        "".into(),
                    ],
                    [
                        "q_limit_tolerance".into(),
                        self.options.q_limit_tolerance.to_string(),
                        "MVAr".into(),
                    ],
                    [
                        "update_on_solve".into(),
                        self.update_on_solve.to_string(),
                        "".into(),
                    ],
                    [
                        "residual_display".into(),
                        self.residual_units.name().into(),
                        "".into(),
                    ],
                ] {
                    csv_row(out, row)?;
                }
            }
            ("sensitivities" | "sensitivity", _) => {
                return Err("sensitivity output includes a 3D branch-flow tensor; use a text destination instead of CSV".into());
            }
            _ => return Err("unknown command or invalid arguments; type help".into()),
        }
        Ok(())
    }

    fn command(
        &mut self,
        line: &str,
        out: &mut impl Write,
    ) -> Result<bool, Box<dyn std::error::Error>> {
        let line = line.trim();
        if line.is_empty() {
            return Ok(true);
        }
        let (command, args) = line.split_once(char::is_whitespace).unwrap_or((line, ""));
        let args = args.trim();
        log::info!("executing CLI command {command:?} with arguments {args:?}");
        if is_report(command)
            && let Some((report_args, destination)) = split_report_destination(args)?
        {
            let destination = path_argument(destination)?;
            let report_command = if report_args.is_empty() {
                command.to_owned()
            } else {
                format!("{command} {report_args}")
            };
            let mut report = Vec::new();
            let keep_running = if is_csv_path(destination) {
                self.csv_report(command, report_args, &mut report)?;
                true
            } else {
                self.command(&report_command, &mut report)?
            };
            std::fs::write(destination, &report)?;
            log::info!(
                "wrote {command} report to {destination} ({} bytes)",
                report.len()
            );
            writeln!(out, "Wrote {command} report to {destination}")?;
            return Ok(keep_running);
        }
        match (command, args) {
            ("quit" | "exit", "") => return Ok(false),
            ("help", "") => writeln!(out, "{HELP}")?,
            ("load", path) if !path.is_empty() => {
                let path = path_argument(path)?;
                let case = if std::path::Path::new(path)
                    .extension()
                    .is_some_and(|s| s.eq_ignore_ascii_case("raw"))
                {
                    let imported = load_raw_with_options(path, &RawOptions::default())?;
                    for warning in imported.warnings {
                        writeln!(out, "RAW line {}: {}", warning.line, warning.message)?;
                    }
                    imported.case
                } else {
                    load_matpower(path)?
                };
                self.input = Some(case);
                self.result = None;
                self.command("summary", out)?;
            }
            ("summary", "") => {
                let case = self.case()?;
                writeln!(
                    out,
                    "{} buses, {} generators, {} branches; base {} MVA",
                    case.buses.len(),
                    case.generators.len(),
                    case.branches.len(),
                    case.base_mva
                )?;
                if let Some(result) = &self.result {
                    self.residual_units.report(result, out)?;
                }
            }
            ("solve", "" | "ac" | "dc") => {
                let case = self.input.as_ref().ok_or("load a case first")?;
                let mut options = self.options.clone();
                if !args.is_empty() {
                    options.model = model(args)?;
                }
                self.result = None;
                let result = solve(case, &options)?;
                let converged = result.status == SolveStatus::Converged;
                self.residual_units.report(&result, out)?;
                if self.update_on_solve {
                    self.input = Some(result.case.clone());
                }
                self.options = options;
                self.result = Some(result);
                if !converged {
                    return Err(
                        "power flow did not converge; final iterate is available for inspection"
                            .into(),
                    );
                }
            }
            ("sensitivities" | "sensitivity", "" | "ac" | "dc") => {
                let case = self.input.as_ref().ok_or("load a case first")?;
                let mut options = self.options.clone();
                if !args.is_empty() {
                    options.model = model(args)?;
                }
                writeln!(out, "Sensitivity analysis")?;
                writeln!(out, "  model: {}", model_name(options.model))?;
                writeln!(
                    out,
                    "  case: {} buses, {} generators, {} branches",
                    case.buses.len(),
                    case.generators.len(),
                    case.branches.len()
                )?;
                writeln!(
                    out,
                    "  solver: {:?} initialization, {:.3e} p.u. tolerance, {} max iterations, Q limits {}",
                    options.initialization,
                    options.tolerance,
                    options.max_iterations,
                    if options.enforce_q_limits {
                        "on"
                    } else {
                        "off"
                    }
                )?;
                writeln!(out, "  active-power balance: reference bus in each island")?;
                if cfg!(debug_assertions) {
                    writeln!(
                        out,
                        "  build: debug (AC linear algebra is faster with cargo run --release)"
                    )?;
                }
                if options.model == Model::Ac {
                    writeln!(
                        out,
                        "  variables: non-reference voltage angles and PQ-bus magnitudes"
                    )?;
                } else {
                    writeln!(out, "  variables: non-reference voltage angles")?;
                }
                writeln!(out, "Computing operating point and linearization...")?;
                out.flush()?;
                let started = Instant::now();
                let sensitivity = match calculate_sensitivities(case, &options) {
                    Ok(sensitivity) => sensitivity,
                    Err(error) => {
                        writeln!(
                            out,
                            "Sensitivity analysis failed after {:.3} s.",
                            started.elapsed().as_secs_f64()
                        )?;
                        return Err(error.into());
                    }
                };
                writeln!(
                    out,
                    "Sensitivity analysis completed in {:.3} s.",
                    started.elapsed().as_secs_f64()
                )?;
                self.residual_units.report(&sensitivity.power_flow, out)?;
                writeln!(
                    out,
                    "  state matrix: {} states x {} injection columns",
                    sensitivity.state_variables.len(),
                    sensitivity.injection_variables.len()
                )?;
                writeln!(
                    out,
                    "  branch-flow tensor: {} branches x {} injections x 4 flow components",
                    sensitivity.branch_flow_sensitivities.len(),
                    sensitivity.injection_variables.len()
                )?;
                write_sensitivities(&sensitivity, out)?;
                if self.update_on_solve {
                    self.input = Some(sensitivity.power_flow.case.clone());
                }
                self.options = options;
                self.result = Some(sensitivity.power_flow);
            }
            ("jacobian", "" | "reduced" | "full") => {
                let jacobian = make_jacobian(self.case()?, args == "full")?;
                writeln!(
                    out,
                    "AC power-flow Jacobian (p.u.; voltage angles in radians)"
                )?;
                writeln!(out, "Injection    State              Value")?;
                for (row, injection) in jacobian.row_variables.iter().enumerate() {
                    for (column, state) in jacobian.column_variables.iter().enumerate() {
                        writeln!(
                            out,
                            "{:<12} {:<12} {:>16.9e}",
                            injection_label(*injection),
                            state_label(*state),
                            jacobian.values[row][column]
                        )?;
                    }
                }
            }
            ("ptdf", _) => {
                let case = self.case()?;
                let bus_ids = parse_bus_ids(args)?;
                let (columns, values) = if bus_ids.is_empty() {
                    (
                        case.buses.iter().map(|bus| bus.id).collect(),
                        make_ptdf(case, None)?,
                    )
                } else {
                    let values = make_ptdf_columns(case, None, &bus_ids)?;
                    (bus_ids, values)
                };
                writeln!(out, "DC PTDF (MW/MW; reference-bus slack)")?;
                write_matrix_header(out, "Branch From To", &columns, "P")?;
                for (index, (branch, row)) in case.branches.iter().zip(&values).enumerate() {
                    write!(out, "{:>6} {:>4} {:>4}", index + 1, branch.from, branch.to)?;
                    write_matrix_row(out, row)?;
                }
            }
            ("lodf", "" | "mask" | "raw") => {
                let case = self.case()?;
                let ptdf = make_ptdf(case, None)?;
                let handling = if args == "raw" {
                    LodfBridgeHandling::Raw
                } else {
                    LodfBridgeHandling::Mask
                };
                let lodf = make_lodf_with_bridge_handling(case, &ptdf, handling)?;
                writeln!(out, "DC LODF (MW/MW; bridge handling: {:?})", handling)?;
                let columns: Vec<_> = (1..=case.branches.len()).collect();
                write_matrix_header(out, "Branch From To", &columns, "L")?;
                for (index, (branch, row)) in case.branches.iter().zip(&lodf).enumerate() {
                    write!(out, "{:>6} {:>4} {:>4}", index + 1, branch.from, branch.to)?;
                    write_matrix_row(out, row)?;
                }
            }
            ("buses", "") => {
                writeln!(out, "Bus       Vm (p.u.)    Va (degrees)")?;
                for bus in &self.case()?.buses {
                    writeln!(out, "{:>6}    {:>9.6}    {:>12.6}", bus.id, bus.vm, bus.va)?;
                }
            }
            ("generators", "") => {
                writeln!(out, "Bus       Pg (MW)      Qg (MVAr)    Online")?;
                for generator in &self.case()?.generators {
                    writeln!(
                        out,
                        "{:>6}    {:>10.6}    {:>10.6}    {}",
                        generator.bus, generator.pg, generator.qg, generator.online
                    )?;
                }
            }
            ("flows", "") => {
                let result = self.result.as_ref().ok_or("solve a case first")?;
                writeln!(
                    out,
                    "From    To       PF (MW)    QF (MVAr)       PT (MW)    QT (MVAr)"
                )?;
                for (branch, flow) in result.case.branches.iter().zip(&result.branch_flows) {
                    writeln!(
                        out,
                        "{:>4} {:>5} {:>13.6} {:>12.6} {:>13.6} {:>12.6}",
                        branch.from, branch.to, flow[0], flow[1], flow[2], flow[3]
                    )?;
                }
            }
            ("options", "") => {
                writeln!(out, "{:#?}", self.options)?;
                writeln!(out, "update_on_solve: {}", self.update_on_solve)?;
                writeln!(out, "residual: {}", self.residual_units.name())?;
            }
            ("set", args) => {
                let words: Vec<_> = args.split_whitespace().collect();
                match words.as_slice() {
                    ["update_on_solve", "true" | "on"] => self.update_on_solve = true,
                    ["update_on_solve", "false" | "off"] => self.update_on_solve = false,
                    ["residual", "pu"] => self.residual_units = ResidualUnits::Pu,
                    ["residual", "mva"] => self.residual_units = ResidualUnits::Mva,
                    ["model", value] => self.options.model = model(value)?,
                    ["initialization", "case"] => {
                        self.options.initialization = Initialization::Case
                    }
                    ["initialization", "flat"] => {
                        self.options.initialization = Initialization::Flat
                    }
                    ["tolerance", value] => {
                        let value: f64 = value.parse()?;
                        if !value.is_finite() || value <= 0.0 {
                            return Err("tolerance must be finite and positive".into());
                        }
                        self.options.tolerance = value;
                    }
                    ["max_iterations", value] => self.options.max_iterations = value.parse()?,
                    ["enforce_q_limits", "on"] => self.options.enforce_q_limits = true,
                    ["enforce_q_limits", "off"] => self.options.enforce_q_limits = false,
                    _ => return Err("invalid setting; type help for supported settings".into()),
                }
                if matches!(words.first(), Some(&"residual")) {
                    writeln!(out, "Residual display units updated.")?;
                } else {
                    writeln!(out, "Setting updated; run solve to apply.")?;
                }
            }
            ("save", path) if !path.is_empty() => {
                let path = path_argument(path)?;
                save_matpower(path, self.case()?)?;
                writeln!(out, "Saved {path}")?;
            }
            _ => return Err("unknown command or invalid arguments; type help".into()),
        }
        Ok(true)
    }
}

fn model(value: &str) -> Result<Model, Box<dyn std::error::Error>> {
    match value {
        "ac" => Ok(Model::Ac),
        "dc" => Ok(Model::Dc),
        _ => Err("model must be ac or dc".into()),
    }
}

fn model_name(value: Model) -> &'static str {
    match value {
        Model::Ac => "AC",
        Model::Dc => "DC",
    }
}

fn state_label(variable: StateVariable) -> String {
    match variable {
        StateVariable::VoltageAngle(bus) => format!("Va({bus})"),
        StateVariable::VoltageMagnitude(bus) => format!("Vm({bus})"),
    }
}

fn injection_label(variable: InjectionVariable) -> String {
    match variable {
        InjectionVariable::ActivePower(bus) => format!("P({bus})"),
        InjectionVariable::ReactivePower(bus) => format!("Q({bus})"),
    }
}

fn write_sensitivities(
    sensitivity: &rs_power::PowerFlowSensitivity,
    out: &mut impl Write,
) -> io::Result<()> {
    writeln!(
        out,
        "State sensitivities (state/p.u. injection; voltage angles in radians)"
    )?;
    writeln!(out, "State        Injection           Value")?;
    for (row, state) in sensitivity.state_variables.iter().enumerate() {
        for (column, injection) in sensitivity.injection_variables.iter().enumerate() {
            writeln!(
                out,
                "{:<12} {:<12} {:>16.9e}",
                state_label(*state),
                injection_label(*injection),
                sensitivity.state_sensitivities[row][column]
            )?;
        }
    }
    writeln!(out, "Branch-flow sensitivities (p.u./p.u.)")?;
    writeln!(
        out,
        "Branch From To Injection             dPF             dQF             dPT             dQT"
    )?;
    for (index, (branch, row)) in sensitivity
        .power_flow
        .case
        .branches
        .iter()
        .zip(&sensitivity.branch_flow_sensitivities)
        .enumerate()
    {
        for (injection, values) in sensitivity.injection_variables.iter().zip(row) {
            writeln!(
                out,
                "{:>6} {:>4} {:>4} {:<12} {:>15.8e} {:>15.8e} {:>15.8e} {:>15.8e}",
                index + 1,
                branch.from,
                branch.to,
                injection_label(*injection),
                values[0],
                values[1],
                values[2],
                values[3]
            )?;
        }
    }
    Ok(())
}

fn parse_bus_ids(args: &str) -> Result<Vec<u32>, Box<dyn std::error::Error>> {
    args.split_whitespace()
        .map(|value| {
            value
                .parse()
                .map_err(|_| format!("invalid bus ID {value}").into())
        })
        .collect()
}

fn write_matrix_header<T: std::fmt::Display>(
    out: &mut impl Write,
    leading: &str,
    columns: &[T],
    prefix: &str,
) -> io::Result<()> {
    write!(out, "{leading}")?;
    for column in columns {
        write!(out, " {prefix}({column})")?;
    }
    writeln!(out)
}

fn write_matrix_row(out: &mut impl Write, row: &[f64]) -> io::Result<()> {
    for value in row {
        write!(out, " {:>15.8e}", value)?;
    }
    writeln!(out)
}

fn path_argument(value: &str) -> Result<&str, Box<dyn std::error::Error>> {
    if value.starts_with(['\'', '"']) {
        let quote = value.chars().next().unwrap();
        let inner = value
            .strip_prefix(quote)
            .and_then(|s| s.strip_suffix(quote));
        inner
            .filter(|s| !s.is_empty())
            .ok_or_else(|| "expected a nonempty path with matching quotes".into())
    } else {
        Ok(value)
    }
}

fn is_report(command: &str) -> bool {
    matches!(
        command,
        "summary"
            | "sensitivities"
            | "sensitivity"
            | "jacobian"
            | "ptdf"
            | "lodf"
            | "buses"
            | "generators"
            | "flows"
            | "options"
    )
}

fn split_report_destination(
    args: &str,
) -> Result<Option<(&str, &str)>, Box<dyn std::error::Error>> {
    let mut quote = None;
    for (index, character) in args.char_indices() {
        if matches!(character, '\'' | '"') {
            if quote == Some(character) {
                quote = None;
            } else if quote.is_none() {
                quote = Some(character);
            }
            continue;
        }
        if quote.is_some() || !args[index..].starts_with("to") {
            continue;
        }
        let before_is_boundary = index == 0
            || args[..index]
                .chars()
                .next_back()
                .is_some_and(char::is_whitespace);
        let after = &args[index + 2..];
        let after_is_boundary = after.chars().next().is_some_and(char::is_whitespace);
        if before_is_boundary && after_is_boundary {
            let destination = after.trim();
            if destination.is_empty() {
                return Err("report destination must be a nonempty path".into());
            }
            return Ok(Some((args[..index].trim(), destination)));
        }
    }
    Ok(None)
}

fn is_csv_path(path: &str) -> bool {
    std::path::Path::new(path)
        .extension()
        .is_some_and(|extension| extension.eq_ignore_ascii_case("csv"))
}

fn csv_row<I, S>(out: &mut impl Write, fields: I) -> io::Result<()>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    for (index, field) in fields.into_iter().enumerate() {
        if index > 0 {
            write!(out, ",")?;
        }
        let field = field.as_ref();
        if field.contains([',', '"', '\r', '\n']) {
            write!(out, "\"")?;
            for character in field.chars() {
                if character == '"' {
                    write!(out, "\"\"")?;
                } else {
                    write!(out, "{character}")?;
                }
            }
            write!(out, "\"")?;
        } else {
            write!(out, "{field}")?;
        }
    }
    writeln!(out)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let stdin = io::stdin();
    let interactive = stdin.is_terminal() && io::stdout().is_terminal();
    init_logging(interactive);
    let args: Vec<_> = std::env::args().skip(1).collect();
    log::debug!("starting rs-power CLI with arguments {args:?}");
    if args == ["--help"] || args == ["-h"] {
        println!("Usage: rs-power [case.m|case.raw]\n\n{HELP}");
        return Ok(());
    }
    if args.len() > 1 {
        return Err("usage: rs-power [case.m|case.raw]".into());
    }
    let mut out = io::stdout().lock();
    let mut session = Session::default();
    if interactive {
        write!(out, "\x1b[2J\x1b[H\x1b[1;36m{SPLASH}\x1b[0m")?;
        writeln!(
            out,
            "  AC / DC power flow  |  v{}",
            env!("CARGO_PKG_VERSION")
        )?;
        writeln!(out, "  Energize your next case.\n")?;
        writeln!(out, "  Type help for commands, quit to exit.\n")?;
        out.flush()?;
    }
    if let Some(path) = args.first() {
        session.command(&format!("load {path}"), &mut out)?;
    }
    let mut input = stdin.lock();
    let mut line = String::new();
    let mut failed = false;
    loop {
        if interactive {
            write!(out, "rs-power> ")?;
            out.flush()?;
        }
        line.clear();
        if input.read_line(&mut line)? == 0 {
            break;
        }
        match session.command(&line, &mut out) {
            Ok(false) => break,
            Ok(true) => {}
            Err(error) => {
                log::error!("CLI command {:?} failed: {error}", line.trim());
                writeln!(out, "Error: {error}")?;
                failed = true;
            }
        }
    }
    if failed && !interactive {
        log::error!("batch session completed with one or more command failures");
        return Err("one or more commands failed".into());
    }
    log::debug!("rs-power CLI stopped");
    Ok(())
}
