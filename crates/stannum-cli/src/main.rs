//! Stannum without the window: run a run file, check one, or list the
//! commands a run file can use.
//!
//! ```text
//! stannum-cli run study.stnrun [--keep-going] [--json] [--verbose]
//! stannum-cli check study.stnrun
//! stannum-cli commands
//! ```
//!
//! Exit codes: 0 when every line ran, 1 when a line failed or the run was
//! cancelled, 2 when the file or the command line is wrong.

use stannum_core::commands::{RunOptions, Session};
use stannum_core::{logbook, run_file};
use std::path::PathBuf;
use std::process::ExitCode;

const USAGE: &str = "\
Usage:
  stannum-cli run <file.stnrun> [--keep-going] [--json] [--verbose]
      Run every line in order. Stops at the first line that fails unless
      --keep-going. --json prints the full report, with each command's
      answer, on stdout. --verbose shows the core's log as it runs.
  stannum-cli check <file.stnrun>
      Read the file and list its steps without running them.
  stannum-cli commands
      List the commands a run file can use.

A run file holds one command per line, then its arguments as name=value:
  open_case path=case30.m
  solve_power_flow settings={\"start\": \"flat\"}
  export_matpower path=case30_solved.m solved=true
";

struct Args {
    command: String,
    file: Option<PathBuf>,
    keep_going: bool,
    json: bool,
    verbose: bool,
}

fn parse_args() -> Result<Args, String> {
    let mut args = Args {
        command: String::new(),
        file: None,
        keep_going: false,
        json: false,
        verbose: false,
    };
    for arg in std::env::args().skip(1) {
        match arg.as_str() {
            "--keep-going" | "-k" => args.keep_going = true,
            "--json" => args.json = true,
            "--verbose" | "-v" => args.verbose = true,
            "--help" | "-h" | "help" if args.command.is_empty() => args.command = "help".into(),
            "--version" | "-V" if args.command.is_empty() => args.command = "version".into(),
            flag if flag.starts_with('-') => return Err(format!("Unknown option {flag}.")),
            _ if args.command.is_empty() => args.command = arg,
            _ if args.file.is_none() => args.file = Some(arg.into()),
            extra => return Err(format!("Unexpected argument {extra}.")),
        }
    }
    Ok(args)
}

fn file(args: &Args) -> Result<PathBuf, String> {
    args.file
        .clone()
        .ok_or_else(|| format!("{} needs a run file.", args.command))
}

fn main() -> ExitCode {
    let args = match parse_args() {
        Ok(a) => a,
        Err(e) => {
            eprintln!("{e}\n\n{USAGE}");
            return ExitCode::from(2);
        }
    };
    let result = match args.command.as_str() {
        "run" => file(&args).and_then(|f| run(&f, &args)),
        "check" => file(&args).and_then(|f| check(&f)),
        "commands" => {
            commands();
            Ok(true)
        }
        "version" => {
            println!("stannum-cli {}", env!("CARGO_PKG_VERSION"));
            Ok(true)
        }
        "help" | "" => {
            print!("{USAGE}");
            Ok(true)
        }
        other => Err(format!("No command called {other}.\n\n{USAGE}")),
    };
    match result {
        Ok(true) => ExitCode::SUCCESS,
        Ok(false) => ExitCode::from(1),
        Err(e) => {
            eprintln!("{e}");
            ExitCode::from(2)
        }
    }
}

fn read(path: &std::path::Path) -> Result<Vec<run_file::Step>, String> {
    run_file::read(path).map_err(|e| e.message)
}

fn check(path: &std::path::Path) -> Result<bool, String> {
    let steps = read(path)?;
    for step in &steps {
        println!("{:>4}  {}", step.line, summary(step));
    }
    println!("{} steps, ready to run.", steps.len());
    Ok(true)
}

/// A step on one line, long values cut short.
fn summary(step: &run_file::Step) -> String {
    let mut out = step.command.clone();
    for (k, v) in &step.args {
        let v = match v {
            serde_json::Value::String(s) => s.clone(),
            other => other.to_string(),
        };
        let v = if v.chars().count() > 48 {
            format!("{}…", v.chars().take(47).collect::<String>())
        } else {
            v
        };
        out.push_str(&format!(" {k}={v}"));
    }
    out
}

fn run(path: &std::path::Path, args: &Args) -> Result<bool, String> {
    let steps = read(path)?;
    if args.verbose {
        logbook::install();
    }
    let session = Session::new();
    let mut seen = 0;
    let show_log = |seen: &mut u64| {
        if !args.verbose {
            return;
        }
        for entry in logbook::since(*seen) {
            eprintln!(
                "      {:<5} {}: {}",
                entry.level, entry.source, entry.message
            );
            *seen = entry.seq;
        }
    };
    let options = RunOptions {
        keep_going: args.keep_going,
        results: args.json,
    };
    let report = session
        .run_steps(path, &steps, options, |o| {
            show_log(&mut seen);
            match &o.error {
                None => eprintln!("{:>4}  ok    {:<24} {:.2} s", o.line, o.command, o.seconds),
                Some(e) => eprintln!(
                    "{:>4}  {:<5} {:<24} {}",
                    o.line,
                    if e.kind == stannum_core::ErrorKind::Cancelled {
                        "stop"
                    } else {
                        "FAIL"
                    },
                    o.command,
                    e.message
                ),
            }
        })
        .map_err(|e| e.message)?;
    show_log(&mut seen);
    let ran = report.outcomes.len();
    if report.ok() {
        eprintln!("Ran {ran} steps in {:.1} s.", report.seconds);
    } else if report.cancelled {
        eprintln!("Cancelled after {ran} of {} steps.", report.steps);
    } else {
        eprintln!(
            "{} of {ran} steps failed; {} not run.",
            report.failed,
            report.steps - ran
        );
    }
    if args.json {
        println!(
            "{}",
            serde_json::to_string_pretty(&report).map_err(|e| e.to_string())?
        );
    }
    Ok(report.ok())
}

fn commands() {
    use std::io::Write;
    // Stop quietly when the reader goes away (`stannum-cli commands | head`).
    let mut out = std::io::stdout().lock();
    for c in run_file::commands() {
        let doc = stannum_core::commands::clean_doc(c.doc);
        let args = stannum_core::commands::command_args(c.name)
            .unwrap_or_default()
            .iter()
            .filter(|(f, _)| *f != "case")
            .map(|(f, _)| format!(" {f}="))
            .collect::<String>();
        if writeln!(out, "{}{args}", c.name).is_err() {
            return;
        }
        for line in doc.lines() {
            if writeln!(out, "    {line}").is_err() {
                return;
            }
        }
    }
}
