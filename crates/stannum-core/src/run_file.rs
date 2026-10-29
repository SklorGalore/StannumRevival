//! Run files: Stannum commands written one per line, run in order.
//!
//! A run file (`.stnrun`) is plain text. Each line is one command from
//! [`crate::commands`], then its arguments as `name=value`:
//!
//! ```text
//! # Solve the case with and without the new generator.
//! open_case path=cases/case30.m
//! solve_power_flow settings={"model": "ac", "start": "flat"}
//! set_study_edits edits=[{"kind": "generation", "element": "gen:22:1", "mw": 40}]
//! solve_power_flow
//! export_matpower path=out/case30-solved.m solved=true
//! ```
//!
//! - A value is JSON (`"text"`, `3.5`, `true`, `null`, `[...]`, `{...}`), or
//!   a bare word with no spaces, which reads as text. A `[...]` or `{...}`
//!   value may run over several lines.
//! - Argument names are the commands' own (`case_path` or `casePath`).
//! - `#` starts a comment, at the start of a line or after a space.
//! - A relative file path is read from the run file's folder.
//! - The `case` argument is left out: every line works on the case that is
//!   open at that point.
//!
//! - A first comment line `# stannum-run-file version 1` says which version
//!   of the format the file follows. A file without one is version 1; a
//!   newer version than [`VERSION`] is refused.
//!
//! Every line is checked before the first one runs, so a misspelled command
//! on line 40 stops the file before line 1 changes anything. The run stops
//! at the first command that fails unless `keep_going` is set.

use crate::commands::{COMMANDS, command_args};
use crate::error::{Error, ErrorKind};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::path::{Path, PathBuf};

/// File extension for run files, without the dot.
pub const EXTENSION: &str = "stnrun";

/// The newest run file version this Stannum reads, and the one it writes.
pub const VERSION: u32 = 1;

/// The tag of the comment line that declares a run file's version.
pub const VERSION_TAG: &str = "stannum-run-file";

/// Commands that change the case or write a file. Record writes these into
/// a run file and leaves out the rest, which only read.
const ACTIONS: &[&str] = &[
    "open_case",
    "close_case",
    "solve_power_flow",
    "load_dc_grids",
    "clear_dc_grids",
    "load_coordinates",
    "clear_coordinates",
    "set_study_edits",
    "calculate_sensitivities",
    "export_matpower",
    "export_raw",
    "apply_record_edits",
    "save_case_as",
    "topology_set_merge",
    "diff_compare",
    "diff_compare_working",
    "run_contingencies",
    "trip_and_compare",
];

/// Commands a run file can't hold: running a run file from inside one.
const NOT_IN_RUN_FILES: &[&str] = &["run_file", "cancel_run_file", "save_run_file"];

const fn same(a: &str, b: &str) -> bool {
    let (a, b) = (a.as_bytes(), b.as_bytes());
    if a.len() != b.len() {
        return false;
    }
    let mut i = 0;
    while i < a.len() {
        if a[i] != b[i] {
            return false;
        }
        i += 1;
    }
    true
}

/// Whether a command changes the case or writes a file.
pub const fn is_action(command: &str) -> bool {
    let mut i = 0;
    while i < ACTIONS.len() {
        if same(ACTIONS[i], command) {
            return true;
        }
        i += 1;
    }
    false
}

/// One command of a run file, with its arguments as JSON, named as the
/// command takes them (camelCase).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Step {
    /// The line it starts on, from 1.
    #[serde(default)]
    pub line: usize,
    pub command: String,
    #[serde(default)]
    pub args: Map<String, Value>,
}

/// How a step went.
#[derive(Clone, Debug, Serialize, JsonSchema)]
pub struct StepOutcome {
    pub line: usize,
    pub command: String,
    pub seconds: f64,
    /// Why it failed; absent when it worked.
    pub error: Option<Error>,
    /// What it answered, when results were asked for.
    pub result: Option<Value>,
}

/// What running a run file did.
#[derive(Clone, Debug, Serialize, JsonSchema)]
pub struct RunReport {
    pub path: String,
    /// Steps in the file.
    pub steps: usize,
    /// Steps that ran, in order (the ones after a failure or a cancel don't).
    pub outcomes: Vec<StepOutcome>,
    pub failed: usize,
    pub cancelled: bool,
    pub seconds: f64,
}

impl RunReport {
    pub fn ok(&self) -> bool {
        self.failed == 0 && !self.cancelled && self.outcomes.len() == self.steps
    }
}

/// `case_path` -> `casePath`; camelCase names stay as they are.
pub fn camel(name: &str) -> String {
    let mut out = String::new();
    let mut upper = false;
    for ch in name.chars() {
        if ch == '_' {
            upper = true;
        } else if upper {
            out.extend(ch.to_uppercase());
            upper = false;
        } else {
            out.push(ch);
        }
    }
    out
}

fn line_error(line: usize, message: impl std::fmt::Display) -> Error {
    Error::new(ErrorKind::Failed, format!("Line {line}: {message}"))
}

/// Reads a run file's text into steps, checking each command and argument
/// name. Relative paths are left as written (see [`resolve_paths`]).
pub fn parse(text: &str) -> Result<Vec<Step>, Error> {
    crate::file_version::check(text, VERSION_TAG, VERSION, "run file")
        .map_err(|e| Error::new(ErrorKind::Failed, e))?;
    let mut steps = Vec::new();
    let mut lexer = Lexer::new(text);
    while let Some((line, command)) = lexer.next_command()? {
        let Some(declared) = command_args(&command) else {
            return Err(line_error(
                line,
                format!("Stannum has no command called {command}."),
            ));
        };
        if NOT_IN_RUN_FILES.contains(&command.as_str()) {
            return Err(line_error(
                line,
                format!("{command} can't be used inside a run file."),
            ));
        }
        let mut args = Map::new();
        while let Some((name, value)) = lexer.next_arg(line)? {
            let key = camel(&name);
            if !declared.iter().any(|(field, _)| camel(field) == key) {
                let known: Vec<_> = declared
                    .iter()
                    .filter(|(f, _)| *f != "case")
                    .map(|(f, _)| *f)
                    .collect();
                return Err(line_error(
                    line,
                    if known.is_empty() {
                        format!("{command} takes no arguments, but {name} was given.")
                    } else {
                        format!(
                            "{command} has no argument {name}. It takes {}.",
                            known.join(", ")
                        )
                    },
                ));
            }
            if args.insert(key, value).is_some() {
                return Err(line_error(line, format!("{name} is given twice.")));
            }
        }
        steps.push(Step {
            line,
            command,
            args,
        });
    }
    Ok(steps)
}

/// Reads relative file paths in `steps` from `folder`.
pub fn resolve_paths(steps: &mut [Step], folder: &Path) {
    for step in steps {
        let Some(declared) = command_args(&step.command) else {
            continue;
        };
        for (field, is_path) in declared {
            if !is_path {
                continue;
            }
            if let Some(Value::String(p)) = step.args.get_mut(&camel(field)) {
                let path = Path::new(p.as_str());
                if path.is_relative() {
                    *p = folder.join(path).to_string_lossy().into_owned();
                }
            }
        }
    }
}

/// Reads a run file from disk, with its paths resolved.
pub fn read(path: &Path) -> Result<Vec<Step>, Error> {
    let text = std::fs::read_to_string(path).map_err(|e| Error::file(path, e))?;
    let mut steps = parse(&text)
        .map_err(|e| Error::new(e.kind, format!("{}: {}", path.display(), e.message)))?;
    resolve_paths(&mut steps, path.parent().unwrap_or(Path::new(".")));
    Ok(steps)
}

/// Writes steps as a run file. File paths under `folder` are written
/// relative to it; `case` arguments are left out.
pub fn format(steps: &[Step], folder: Option<&Path>, heading: &str) -> Result<String, Error> {
    let mut out = crate::file_version::line(VERSION_TAG, VERSION);
    out.push('\n');
    for line in heading.lines() {
        out.push_str("# ");
        out.push_str(line);
        out.push('\n');
    }
    if !heading.is_empty() {
        out.push('\n');
    }
    for step in steps {
        let Some(declared) = command_args(&step.command) else {
            return Err(Error::invalid_arguments(format!(
                "Stannum has no command called {}.",
                step.command
            )));
        };
        let mut line = step.command.clone();
        for (key, _) in &step.args {
            if !declared.iter().any(|(f, _)| camel(f) == *key) {
                return Err(Error::invalid_arguments(format!(
                    "{} has no argument {key}.",
                    step.command
                )));
            }
        }
        // In the order the command declares them.
        for (field, is_path) in declared {
            if *field == "case" {
                continue;
            }
            let Some(value) = step.args.get(&camel(field)) else {
                continue;
            };
            if value.is_null() {
                continue;
            }
            let value = match (is_path, value, folder) {
                (true, Value::String(p), Some(folder)) => {
                    Value::String(relative_to(Path::new(p), folder))
                }
                _ => value.clone(),
            };
            line.push(' ');
            line.push_str(field);
            line.push('=');
            line.push_str(&write_value(&value));
        }
        out.push_str(&line);
        out.push('\n');
    }
    Ok(out)
}

/// Leaves out settings that match the defaults, so a recorded
/// `solve_power_flow` reads as just that unless something was changed.
pub fn without_defaults(steps: &mut [Step]) {
    let solve = serde_json::to_value(crate::power::SolveSettings::default()).unwrap_or_default();
    let contingency = serde_json::to_value(crate::contingency::ContingencySettings::default())
        .unwrap_or_default();
    for step in steps {
        let default = match step.command.as_str() {
            "solve_power_flow" | "calculate_sensitivities" => &solve,
            "run_contingencies" => &contingency,
            _ => continue,
        };
        if let Some(settings) = step.args.get_mut("settings") {
            strip(settings, default);
            if settings.as_object().is_some_and(Map::is_empty) {
                step.args.remove("settings");
            }
        }
    }
}

/// Removes the fields of `value` that equal `default`'s (numbers by value).
fn strip(value: &mut Value, default: &Value) {
    let (Value::Object(fields), Value::Object(defaults)) = (value, default) else {
        return;
    };
    fields.retain(|key, field| match defaults.get(key) {
        None => true,
        Some(d) => {
            strip(field, d);
            !(same_value(field, d) || field.as_object().is_some_and(Map::is_empty) && d.is_object())
        }
    });
}

fn same_value(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Number(x), Value::Number(y)) => x.as_f64() == y.as_f64(),
        _ => a == b,
    }
}

/// A path under `folder` as a relative path with forward slashes; any other
/// path as it is.
fn relative_to(path: &Path, folder: &Path) -> String {
    match path.strip_prefix(folder) {
        Ok(rest) if !rest.as_os_str().is_empty() => rest
            .components()
            .map(|c| c.as_os_str().to_string_lossy())
            .collect::<Vec<_>>()
            .join("/"),
        _ => path.to_string_lossy().into_owned(),
    }
}

/// Text that reads back as itself without quotes.
fn is_bare(text: &str) -> bool {
    !text.is_empty()
        && !text.starts_with(['"', '[', '{', '#'])
        && !text.chars().any(char::is_whitespace)
        && serde_json::from_str::<Value>(text).is_err()
}

fn write_value(value: &Value) -> String {
    match value {
        Value::String(s) if is_bare(s) => s.clone(),
        Value::Object(_) | Value::Array(_) => {
            // Spaced like a person would write it: `{"a": 1, "b": 2}`.
            let compact = value.to_string();
            spaced(&compact)
        }
        other => other.to_string(),
    }
}

/// Adds a space after each `,` and `:` outside strings in compact JSON.
fn spaced(json: &str) -> String {
    let mut out = String::with_capacity(json.len() + json.len() / 8);
    let mut in_string = false;
    let mut escaped = false;
    for ch in json.chars() {
        out.push(ch);
        if in_string {
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == '"' {
                in_string = false;
            }
        } else if ch == '"' {
            in_string = true;
        } else if ch == ',' || ch == ':' {
            out.push(' ');
        }
    }
    out
}

/// Splits run file text into commands and `name=value` arguments.
struct Lexer<'a> {
    text: &'a str,
    pos: usize,
    line: usize,
}

impl<'a> Lexer<'a> {
    fn new(text: &'a str) -> Self {
        Self {
            text: text.strip_prefix('\u{feff}').unwrap_or(text),
            pos: 0,
            line: 1,
        }
    }

    fn peek(&self) -> Option<char> {
        self.text[self.pos..].chars().next()
    }

    fn bump(&mut self) -> Option<char> {
        let ch = self.peek()?;
        self.pos += ch.len_utf8();
        if ch == '\n' {
            self.line += 1;
        }
        Some(ch)
    }

    fn skip_comment(&mut self) {
        while let Some(ch) = self.peek() {
            if ch == '\n' {
                break;
            }
            self.bump();
        }
    }

    /// Skips spaces on this line (and a comment). True at the end of the
    /// line or file.
    fn skip_spaces(&mut self) -> bool {
        loop {
            match self.peek() {
                Some('\n') | None => return true,
                Some('#') => self.skip_comment(),
                Some(c) if c.is_whitespace() => {
                    self.bump();
                }
                Some(_) => return false,
            }
        }
    }

    fn word(&mut self) -> &'a str {
        let start = self.pos;
        while let Some(ch) = self.peek() {
            if ch.is_whitespace() || ch == '=' {
                break;
            }
            self.bump();
        }
        &self.text[start..self.pos]
    }

    /// The next command's line and name, skipping blank and comment lines.
    fn next_command(&mut self) -> Result<Option<(usize, String)>, Error> {
        loop {
            if self.skip_spaces() {
                if self.bump().is_none() {
                    return Ok(None);
                }
                continue;
            }
            let line = self.line;
            let name = self.word();
            if self.peek() == Some('=') {
                return Err(line_error(
                    line,
                    format!("a line starts with a command name, not {name}=."),
                ));
            }
            return Ok(Some((line, name.to_string())));
        }
    }

    /// The next argument on the command's line, or None at its end.
    fn next_arg(&mut self, line: usize) -> Result<Option<(String, Value)>, Error> {
        if self.skip_spaces() {
            return Ok(None);
        }
        let name = self.word().to_string();
        if self.peek() != Some('=') {
            return Err(line_error(
                line,
                format!("write arguments as name=value; {name} has no value."),
            ));
        }
        self.bump();
        let value = self.value(line, &name)?;
        Ok(Some((name, value)))
    }

    fn value(&mut self, line: usize, name: &str) -> Result<Value, Error> {
        let start = self.pos;
        match self.peek() {
            None | Some('\n') => Err(line_error(line, format!("{name}= has no value."))),
            Some(c) if c.is_whitespace() => Err(line_error(line, format!("{name}= has no value."))),
            Some('"' | '[' | '{') => {
                self.json_span(line, name)?;
                let text = &self.text[start..self.pos];
                serde_json::from_str(text).map_err(|e| line_error(line, format!("{name}: {e}")))
            }
            Some(_) => {
                while let Some(ch) = self.peek() {
                    if ch.is_whitespace() {
                        break;
                    }
                    self.bump();
                }
                let text = &self.text[start..self.pos];
                Ok(serde_json::from_str(text).unwrap_or_else(|_| Value::String(text.into())))
            }
        }
    }

    /// Moves past one JSON string, array or object, which may span lines.
    fn json_span(&mut self, line: usize, name: &str) -> Result<(), Error> {
        let mut depth = 0usize;
        let mut in_string = false;
        let mut escaped = false;
        while let Some(ch) = self.bump() {
            if in_string {
                if escaped {
                    escaped = false;
                } else if ch == '\\' {
                    escaped = true;
                } else if ch == '"' {
                    in_string = false;
                    if depth == 0 {
                        return Ok(());
                    }
                } else if ch == '\n' {
                    return Err(line_error(
                        line,
                        format!("{name}: text in quotes isn't closed."),
                    ));
                }
                continue;
            }
            match ch {
                '"' => in_string = true,
                '[' | '{' => depth += 1,
                ']' | '}' => {
                    depth = depth.saturating_sub(1);
                    if depth == 0 {
                        return Ok(());
                    }
                }
                _ => {}
            }
        }
        Err(line_error(
            line,
            format!("{name}: the value isn't closed before the end of the file."),
        ))
    }
}

/// Where a run file is read from: its own folder for relative paths.
pub fn folder_of(path: &Path) -> PathBuf {
    path.parent()
        .filter(|p| !p.as_os_str().is_empty())
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."))
}

/// Every command a run file can hold, for `check` and help text.
pub fn commands() -> impl Iterator<Item = &'static crate::commands::CommandInfo> {
    COMMANDS
        .iter()
        .filter(|c| !NOT_IN_RUN_FILES.contains(&c.name))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn lines_read_as_commands_and_arguments() {
        let steps = parse(
            "# a comment\n\
             \n\
             open_case path=cases/case9.m   # trailing comment\n\
             solve_power_flow settings={\"model\": \"ac\",\n  \"start\": \"flat\"}\n\
             export_matpower path=\"out dir/x.m\" solved=true\n\
             study_relative_path study=a.stannum case_path=b.raw\n\
             list_commands\n",
        )
        .unwrap();
        assert_eq!(steps.len(), 5);
        assert_eq!(steps[0].line, 3);
        assert_eq!(steps[0].command, "open_case");
        assert_eq!(steps[0].args["path"], json!("cases/case9.m"));
        assert_eq!(steps[1].line, 4);
        assert_eq!(
            steps[1].args["settings"],
            json!({"model": "ac", "start": "flat"})
        );
        assert_eq!(steps[2].line, 6);
        assert_eq!(steps[2].args["path"], json!("out dir/x.m"));
        assert_eq!(steps[2].args["solved"], json!(true));
        assert_eq!(steps[3].args["casePath"], json!("b.raw"));
        assert!(steps[4].args.is_empty());
    }

    #[test]
    fn bare_words_read_as_json_when_they_are() {
        let steps =
            parse("contingency_rows offset=0 limit=25\nelement_detail element=bus:12\n").unwrap();
        assert_eq!(steps[0].args["offset"], json!(0));
        assert_eq!(steps[1].args["element"], json!("bus:12"));
    }

    #[test]
    fn mistakes_name_their_line() {
        for (text, says) in [
            (
                "open_case path=a.raw\nsolve_powerflow\n",
                "Line 2: Stannum has no command called solve_powerflow.",
            ),
            (
                "open_case pth=a.raw\n",
                "Line 1: open_case has no argument pth. It takes path.",
            ),
            (
                "list_commands x=1\n",
                "Line 1: list_commands takes no arguments, but x was given.",
            ),
            (
                "open_case path\n",
                "Line 1: write arguments as name=value; path has no value.",
            ),
            ("open_case path=\n", "Line 1: path= has no value."),
            ("open_case path=a path=b\n", "Line 1: path is given twice."),
            (
                "set_study_edits edits=[{\"kind\": 1}\n",
                "Line 1: edits: the value isn't closed before the end of the file.",
            ),
            (
                "run_file path=x.stnrun\n",
                "Line 1: run_file can't be used inside a run file.",
            ),
            (
                "path=x\n",
                "Line 1: a line starts with a command name, not path=.",
            ),
        ] {
            let e = parse(text).unwrap_err();
            assert_eq!(e.message, says, "{text:?}");
        }
    }

    #[test]
    fn relative_paths_are_read_from_the_file_folder() {
        let absolute = std::env::temp_dir().join("out.m");
        let mut steps = parse(&format!(
            "open_case path=cases/a.raw\nexport_matpower path={} solved=false\nelement_detail element=bus:1\n",
            write_value(&json!(absolute.to_string_lossy()))
        ))
        .unwrap();
        resolve_paths(&mut steps, Path::new("/work/runs"));
        assert_eq!(
            Path::new(steps[0].args["path"].as_str().unwrap()),
            Path::new("/work/runs/cases/a.raw")
        );
        assert_eq!(steps[1].args["path"], json!(absolute.to_string_lossy()));
        assert_eq!(steps[2].args["element"], json!("bus:1"));
    }

    #[test]
    fn written_steps_read_back_the_same() {
        let steps = vec![
            Step {
                line: 0,
                command: "open_case".into(),
                args: json!({ "path": "/work/runs/cases/a b.raw" })
                    .as_object()
                    .unwrap()
                    .clone(),
            },
            Step {
                line: 0,
                command: "solve_power_flow".into(),
                args: json!({ "case": 3, "settings": { "model": "ac", "tolerance_mva": 2.0 } })
                    .as_object()
                    .unwrap()
                    .clone(),
            },
            Step {
                line: 0,
                command: "trip_and_compare".into(),
                args: json!({ "element": "line:1-2-1", "limitPct": 100.0 })
                    .as_object()
                    .unwrap()
                    .clone(),
            },
            Step {
                line: 0,
                command: "export_matpower".into(),
                args: json!({ "path": "/elsewhere/x.m", "solved": true })
                    .as_object()
                    .unwrap()
                    .clone(),
            },
        ];
        let text = format(&steps, Some(Path::new("/work/runs")), "Recorded by Stannum").unwrap();
        assert_eq!(
            text,
            "# stannum-run-file version 1\n\
             # Recorded by Stannum\n\n\
             open_case path=\"cases/a b.raw\"\n\
             solve_power_flow settings={\"model\": \"ac\", \"tolerance_mva\": 2.0}\n\
             trip_and_compare element=line:1-2-1 limit_pct=100.0\n\
             export_matpower path=/elsewhere/x.m solved=true\n"
        );
        let mut back = parse(&text).unwrap();
        resolve_paths(&mut back, Path::new("/work/runs"));
        assert_eq!(
            Path::new(back[0].args["path"].as_str().unwrap()),
            Path::new("/work/runs/cases/a b.raw")
        );
        assert_eq!(back[1].args["settings"], steps[1].args["settings"]);
        assert!(!back[1].args.contains_key("case"));
        assert_eq!(back[2].args, steps[2].args);
        assert_eq!(back[3].args, steps[3].args);
    }

    #[test]
    fn a_newer_version_is_refused_before_anything_runs() {
        assert_eq!(
            parse("# stannum-run-file version 1\nlist_commands\n")
                .unwrap()
                .len(),
            1
        );
        let e = parse("# Made by Stannum 2.0\n# stannum-run-file version 2\nlist_commands\n")
            .unwrap_err();
        assert!(
            e.message.starts_with("Line 2: this run file is version 2"),
            "{}",
            e.message
        );
    }

    #[test]
    fn default_settings_are_left_out() {
        let mut steps = parse(
            "solve_power_flow settings={\"model\": \"ac\", \"start\": \"last\", \"tolerance_mva\": 2, \"max_iterations\": 10}\n\
             solve_power_flow settings={\"model\": \"dc\", \"tolerance_mva\": 2}\n\
             run_contingencies settings={\"method\": \"ac\", \"limit_pct\": 100, \"solve\": {\"start\": \"last\", \"enforce_q_limits\": false}}\n",
        )
        .unwrap();
        without_defaults(&mut steps);
        assert!(steps[0].args.is_empty());
        assert_eq!(steps[1].args["settings"], json!({ "model": "dc" }));
        let kept = &steps[2].args["settings"];
        assert_eq!(kept["solve"], json!({ "enforce_q_limits": false }));
        assert!(kept.get("limit_pct").is_none());
    }

    #[test]
    fn text_that_looks_like_json_is_quoted() {
        assert_eq!(write_value(&json!("true")), "\"true\"");
        assert_eq!(write_value(&json!("12")), "\"12\"");
        assert_eq!(write_value(&json!("#x")), "\"#x\"");
        assert_eq!(write_value(&json!("")), "\"\"");
        assert_eq!(write_value(&json!("bus:1")), "bus:1");
        assert_eq!(write_value(&json!({"a": "x,y: z"})), "{\"a\": \"x,y: z\"}");
    }

    #[test]
    fn actions_are_commands() {
        for name in ACTIONS {
            assert!(COMMANDS.iter().any(|c| c.name == *name), "{name}");
        }
        for name in NOT_IN_RUN_FILES {
            assert!(COMMANDS.iter().any(|c| c.name == *name), "{name}");
        }
        assert!(is_action("solve_power_flow"));
        assert!(!is_action("diagram_network"));
    }
}
