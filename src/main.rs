//! Interactive power-flow command line.
use rs_power::{
    Case, Initialization, Model, PowerFlowOptions, PowerFlowResult, RawOptions, SolveStatus,
    load_matpower, load_raw_with_options, save_matpower, solve,
};
use std::io::{self, BufRead, IsTerminal, Write};

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
Paths may contain spaces; optionally wrap the whole path in quotes.";

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

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args == ["--help"] || args == ["-h"] {
        println!("Usage: rs-power [case.m|case.raw]\n\n{HELP}");
        return Ok(());
    }
    if args.len() > 1 {
        return Err("usage: rs-power [case.m|case.raw]".into());
    }
    let stdin = io::stdin();
    let interactive = stdin.is_terminal() && io::stdout().is_terminal();
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
                writeln!(out, "Error: {error}")?;
                failed = true;
            }
        }
    }
    if failed && !interactive {
        return Err("one or more commands failed".into());
    }
    Ok(())
}
