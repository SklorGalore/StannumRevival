//! The tools the MCP server offers. Each wraps one or a few registry
//! commands and shapes the answer for a model: summaries instead of whole
//! reports, a page of rows instead of every row, floats rounded.
//!
//! `call_command` reaches any registry command directly, for what the
//! curated tools don't cover.

use super::backend::{Answer, Failure};
use super::shape::{Rows, round};
use super::{Call, Rpc, failure_result};
use serde_json::{Map, Value, json};
use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

/// Rows a table tool sends when the call doesn't say, and the most it sends.
const DEFAULT_ROWS: u64 = 50;
const MAX_ROWS: u64 = 500;
/// Longest answer `call_command` sends, in characters.
const MAX_RAW: usize = 60_000;
/// Decimals floats are rounded to.
const PLACES: i32 = 4;

struct Tool {
    name: &'static str,
    title: &'static str,
    description: &'static str,
    input: fn() -> Value,
    /// Changes nothing: no case, study or file.
    read_only: bool,
    /// Writes a file or replaces the case, so it can't simply be undone.
    destructive: bool,
    /// Only when attached to the app.
    app: bool,
    run: fn(&Call, &Value) -> Answer,
}

const TOOLS: &[Tool] = &[
    // Cases
    Tool {
        name: "list_sample_cases",
        title: "List sample cases",
        description: "The small cases built into Stannum (nine-bus, five-bus RAW, a 200-bus synthetic grid and others), to open with open_case's `sample`.",
        input: no_args,
        read_only: true,
        destructive: false,
        app: false,
        run: list_samples,
    },
    Tool {
        name: "open_case",
        title: "Open a case",
        description: "Open a case file (PSS/E RAW rev 23-35, MATPOWER .m, or Stannum .stn/.stnb), or a built-in sample by id. It replaces the open case. Answers the case's summary: element counts and any warnings from reading it.",
        input: open_case_input,
        read_only: false,
        destructive: false,
        app: false,
        run: open_case,
    },
    Tool {
        name: "case_summary",
        title: "Case summary",
        description: "The open case: name, path, format, element counts, read warnings, attached coordinate and DC grid files, and the study's edits.",
        input: no_args,
        read_only: true,
        destructive: false,
        app: false,
        run: case_summary,
    },
    // Power flow
    Tool {
        name: "solve_power_flow",
        title: "Solve power flow",
        description: "Solve the power flow of the open case with the study's edits. Pass any setting on its own (method, tolerance_mva, start, enforce_q_limits, switch_shunts, adjust_transformer_taps, area_interchange, ...) or all in `settings`. Settings left out take their defaults: AC, automatic method, from the last solution, 2 MVA tolerance, reactive limits held, every control on. Answers a summary: convergence, totals, how controls moved and counts of limit problems. Then use find_violations and query_results.",
        input: solve_input,
        read_only: false,
        destructive: false,
        app: false,
        run: solve,
    },
    Tool {
        name: "find_violations",
        title: "Find violations",
        description: "The limit problems in a result: buses outside their voltage limits (worst first), branches loaded above `limit_pct` of rate A, and generators outside their reactive limits.",
        input: violations_input,
        read_only: true,
        destructive: false,
        app: false,
        run: violations,
    },
    Tool {
        name: "query_results",
        title: "Query results",
        description: "A page of a result's buses, branches or generators, filtered and sorted in Stannum. Columns: buses id, name, kind, area, base_kv, vm, va, vmin, vmax, violation, pd, qd; branches element, from, to, circuit, is_transformer, online, pf, qf, pt, qt, loss_mw, loading_pct, rate_a, tap, shift_deg; generators element, bus, bus_name, machine_id, online, pg, qg, qmin, qmax, pmax, vg, q_violation. Example: the ten most loaded branches is table=branches, sort=loading_pct, descending=true, limit=10.",
        input: query_results_input,
        read_only: true,
        destructive: false,
        app: false,
        run: query_results,
    },
    Tool {
        name: "element_detail",
        title: "Element detail",
        description: "One element's data and limits from the case file, and what connects to it, by id such as bus:5, line:4:5:1, transformer:2:4:1 or generator:1:1.",
        input: element_input,
        read_only: true,
        destructive: false,
        app: false,
        run: element_detail,
    },
    // Study edits
    Tool {
        name: "get_study_edits",
        title: "Get study edits",
        description: "The study's edits: outages, generator dispatch and bus loads that every solve applies on top of the case.",
        input: no_args,
        read_only: true,
        destructive: false,
        app: false,
        run: get_edits,
    },
    Tool {
        name: "set_study_edits",
        title: "Set study edits",
        description: "Replace the study's edits; every later solve and contingency run uses them. The case file is not touched. An empty list clears them. Solve again to see their effect.",
        input: set_edits_input,
        read_only: false,
        destructive: false,
        app: false,
        run: set_edits,
    },
    // Contingencies
    Tool {
        name: "run_contingencies",
        title: "Run contingencies",
        description: "Run N-1 contingencies (every branch and generator, or the `elements` given; bus:N takes out a bus, ids joined by & go out together), bus outages with `buses`, N-2 with `pairs` (every pair of the chosen branches and generators, at most 250,000), the contingencies of a `.stnl` lists file with `lists` (path; skipped equipment comes back in `notes`), checking only what a `.stnl` file's monitor records name (branch flows and bus voltages at their own limits, and interfaces) with `monitors` (path), or with `planning` the NERC TPL-001 planning events (P0, P1.1-P1.5, P2.1, P7.2). By default each outage is screened linearly and the worst `ac_count` get an AC solve; method=ac solves every one. Answers the run's summary and worst outage; query_contingencies pages the ranked results. Long runs report progress and stop when the call is cancelled.",
        input: contingencies_input,
        read_only: false,
        destructive: false,
        app: false,
        run: run_contingencies,
    },
    Tool {
        name: "planning_event_counts",
        title: "Count planning events",
        description: "How many NERC TPL-001 planning events each category gives on the open case, before running them with run_contingencies.",
        input: planning_input,
        read_only: true,
        destructive: false,
        app: false,
        run: planning_counts,
    },
    Tool {
        name: "query_contingencies",
        title: "Query contingency results",
        description: "A page of the last contingency run's results, ranked worst first unless sorted. Columns include rank, element, kind, category, description, severity, overloads, voltage_violations, converged, lost_load_mw, cut_off_buses, worst.element, worst.pct, worst_voltage.bus, worst_voltage.vm.",
        input: query_contingencies_input,
        read_only: true,
        destructive: false,
        app: false,
        run: query_contingencies,
    },
    Tool {
        name: "contingency_detail",
        title: "Contingency detail",
        description: "One outage of the last run, by rank: its overloads and voltage problems, the buses it cut off, and the branches and buses that moved most.",
        input: detail_input,
        read_only: true,
        destructive: false,
        app: false,
        run: contingency_detail,
    },
    Tool {
        name: "trip_and_compare",
        title: "Trip and compare",
        description: "Take one branch or generator out of the last AC solve, solve again and compare: overloads, voltage problems, cut-off buses and the biggest changes. The study's edits are not changed.",
        input: trip_input,
        read_only: false,
        destructive: false,
        app: false,
        run: trip_and_compare,
    },
    // Transfer limits
    Tool {
        name: "transfer_limits",
        title: "Transfer limits (DFAX, FCITC)",
        description: "How many MW can move from a `source` subsystem to a `sink` (areas, zones, owners, buses, kV) before a monitored branch or interface reaches its rating, in the base case and after each contingency (`contingencies: {}` for every branch outage): the first change of total transfer capability (FCITC), from linear PTDFs and OTDFs on the AC solve. Solve with the AC model first. Answers the summary (FCITC, the limiting element and contingency, generation limit, notes) and the lowest limits, elements already past their limit, each contingency's worst limit and the largest PTDFs, `max_rows` each (default 20).",
        input: transfer_input,
        read_only: false,
        destructive: false,
        app: false,
        run: transfer_limits,
    },
    // Topology
    Tool {
        name: "topology_summary",
        title: "Topology summary",
        description: "The case's islands (with their sizes), bridges (branches whose loss splits an island) and articulation buses (buses whose loss splits one).",
        input: limit_input,
        read_only: true,
        destructive: false,
        app: false,
        run: topology_summary,
    },
    Tool {
        name: "shortest_path",
        title: "Shortest path",
        description: "The fewest-hops path between two buses, through in-service equipment: the buses and the equipment on the way.",
        input: path_input,
        read_only: true,
        destructive: false,
        app: false,
        run: shortest_path,
    },
    // Records
    Tool {
        name: "list_records",
        title: "List record types",
        description: "The case's record types (RAW sections such as BUS, LOAD, BRANCH, TRANSFORMER, or MATPOWER tables) and how many of each.",
        input: no_args,
        read_only: true,
        destructive: false,
        app: false,
        run: list_records,
    },
    Tool {
        name: "query_records",
        title: "Query records",
        description: "A page of one record type exactly as the file has it, every field by its RAW or MATPOWER name (I, NAME, BASKV, ... for BUS), filtered and sorted in Stannum. Each row has `row`, its position, which edit_records uses.",
        input: query_records_input,
        read_only: true,
        destructive: false,
        app: false,
        run: query_records,
    },
    Tool {
        name: "edit_records",
        title: "Edit records",
        description: "Change record fields in the open case (in memory; the file on disk is only written by export_case or saving in the app) and reopen it in place. Each edit names the section, the row (from query_records) and the field by name.",
        input: edit_records_input,
        read_only: false,
        destructive: false,
        app: false,
        run: edit_records,
    },
    // Comparing
    Tool {
        name: "compare_cases",
        title: "Compare cases",
        description: "Compare two PSS/E RAW files record by record, or with no files the open case's file with the case as edited. Answers how many records were added, removed and changed in each section; diff_records lists them.",
        input: compare_input,
        read_only: true,
        destructive: false,
        app: false,
        run: compare,
    },
    Tool {
        name: "diff_records",
        title: "Diff records",
        description: "The last comparison's records of one section that were added, removed or changed (unchanged ones with include_unchanged).",
        input: diff_records_input,
        read_only: true,
        destructive: false,
        app: false,
        run: diff_records,
    },
    // Files
    Tool {
        name: "export_case",
        title: "Export case",
        description: "Write the open case, with its record edits and study edits, to a file. The format follows the extension: .raw (PSS/E RAW, revision 35 or 33), .m (MATPOWER, optionally with the solved voltages), .stn or .stnb (Stannum text or binary, format preview). Overwrites the file.",
        input: export_input,
        read_only: false,
        destructive: true,
        app: false,
        run: export,
    },
    Tool {
        name: "run_run_file",
        title: "Run a run file",
        description: "Run a Stannum run file (.stnrun): registry commands one per line, such as open_case, solve_power_flow and export_raw. Answers each line's outcome.",
        input: run_file_input,
        read_only: false,
        destructive: true,
        app: false,
        run: run_run_file,
    },
    // Anything else
    Tool {
        name: "list_commands",
        title: "List registry commands",
        description: "Every command in Stannum's command registry, with what it does and whether it changes the case. call_command runs any of them.",
        input: no_args,
        read_only: true,
        destructive: false,
        app: false,
        run: list_commands,
    },
    Tool {
        name: "call_command",
        title: "Call a registry command",
        description: "Run any registry command by name with its arguments (camelCase, as list_commands and command_schemas describe them) and get its answer as JSON. For what the other tools don't cover, such as sensitivities, DC grid files or the diagram network. Long answers are cut short: prefer the paged commands.",
        input: call_command_input,
        read_only: false,
        destructive: true,
        app: false,
        run: call_command,
    },
    // The window, when attached
    Tool {
        name: "app_state",
        title: "App window state",
        description: "What the Stannum window shows: module, selection, open case, whether it is solved and which result is on screen.",
        input: no_args,
        read_only: true,
        destructive: false,
        app: true,
        run: app_state,
    },
    Tool {
        name: "app_show",
        title: "Show element in app",
        description: "Select an element in every module of the Stannum window, by id such as bus:5 or line:4:5:1, so the person sees it.",
        input: element_input,
        read_only: false,
        destructive: false,
        app: true,
        run: app_show,
    },
    Tool {
        name: "app_go",
        title: "Switch app module",
        description: "Switch the Stannum window to a module: diagram, power, records or diff.",
        input: module_input,
        read_only: false,
        destructive: false,
        app: true,
        run: app_go,
    },
    Tool {
        name: "app_draw_around",
        title: "Draw one-line around element",
        description: "Draw a one-line diagram a few hops around a bus, branch or generator in the Stannum window's Diagram.",
        input: element_input,
        read_only: false,
        destructive: false,
        app: true,
        run: app_draw_around,
    },
    Tool {
        name: "app_say",
        title: "Write to app log",
        description: "Write a line in the Stannum window's log, marked as from a script, so the person can follow along.",
        input: say_input,
        read_only: false,
        destructive: false,
        app: true,
        run: app_say,
    },
];

/// tools/list: the tools, without the app_* ones unless attached.
pub fn list(attached: bool) -> Vec<Value> {
    TOOLS
        .iter()
        .filter(|t| attached || !t.app)
        .map(|t| {
            json!({
                "name": t.name,
                "title": t.title,
                "description": t.description,
                "inputSchema": (t.input)(),
                "annotations": {
                    "title": t.title,
                    "readOnlyHint": t.read_only,
                    "destructiveHint": t.destructive,
                    "idempotentHint": t.read_only,
                    "openWorldHint": false,
                },
            })
        })
        .collect()
}

/// tools/call. A failing command is a tool result the model reads
/// (`isError`); only an unknown tool or arguments that aren't an object are
/// protocol errors.
pub fn call(call: &Call, name: &str, args: &Value, attached: bool) -> Result<Value, Rpc> {
    let tool = TOOLS
        .iter()
        .find(|t| t.name == name && (attached || !t.app))
        .ok_or_else(|| Rpc(-32602, format!("Unknown tool: {name}")))?;
    if !args.is_object() {
        return Err(Rpc(-32602, "Tool arguments must be an object.".into()));
    }
    match (tool.run)(call, args) {
        Ok(mut answer) => {
            round(&mut answer, PLACES);
            let text = match &answer {
                Value::String(s) => s.clone(),
                other => other.to_string(),
            };
            Ok(json!({ "content": [{ "type": "text", "text": text }], "isError": false }))
        }
        Err(f) => Ok(failure_result(&f)),
    }
}

// ---------------------------------------------------------------- schemas

fn no_args() -> Value {
    json!({ "type": "object", "properties": {}, "additionalProperties": false })
}

fn object(properties: Value, required: &[&str]) -> Value {
    let mut schema =
        json!({ "type": "object", "properties": properties, "additionalProperties": false });
    if !required.is_empty() {
        schema["required"] = json!(required);
    }
    schema
}

/// The core's own schema for one of its types, with every `$ref` inlined,
/// so a tool's settings stay in step with the core. `loose` drops
/// `required`, for settings whose missing fields take defaults.
fn core_schema(name: &str, loose: bool) -> Value {
    static SCHEMAS: OnceLock<Value> = OnceLock::new();
    let all = SCHEMAS.get_or_init(stannum_core::commands::schemas);
    let defs = &all["$defs"];
    fn inline(v: &Value, defs: &Value, loose: bool, depth: usize) -> Value {
        match v {
            Value::Object(map) => {
                if let Some(r) = map.get("$ref").and_then(Value::as_str)
                    && depth < 12
                {
                    let name = r.rsplit('/').next().unwrap_or("");
                    let mut target = inline(&defs[name], defs, loose, depth + 1);
                    // Keep what sits beside the $ref (description, default).
                    if let Value::Object(t) = &mut target {
                        for (k, v) in map {
                            if k != "$ref" {
                                t.insert(k.clone(), inline(v, defs, loose, depth + 1));
                            }
                        }
                    }
                    return target;
                }
                let mut out = Map::new();
                for (k, v) in map {
                    if loose && k == "required" {
                        continue;
                    }
                    // Not every client reads these JSON Schema keywords.
                    if k == "format" || k == "title" {
                        continue;
                    }
                    out.insert(k.clone(), inline(v, defs, loose, depth + 1));
                }
                Value::Object(out)
            }
            Value::Array(items) => Value::Array(
                items
                    .iter()
                    .map(|i| inline(i, defs, loose, depth + 1))
                    .collect(),
            ),
            other => other.clone(),
        }
    }
    let mut schema = inline(&defs[name], defs, loose, 0);
    if let Value::Object(map) = &mut schema {
        map.remove("default");
    }
    schema
}

fn page_properties(sortable: &str) -> Value {
    json!({
        "sort": { "type": "string", "description": format!("Column to sort by, such as {sortable}. Left out, rows keep the table's order.") },
        "descending": { "type": "boolean", "description": "Sort largest first." },
        "filters": {
            "type": "array",
            "description": "Keep rows passing every filter.",
            "items": {
                "type": "object",
                "properties": {
                    "field": { "type": "string", "description": "Column name." },
                    "op": { "type": "string", "enum": ["eq", "neq", "contains", "gt", "gte", "lt", "lte", "between", "missing", "present"] },
                    "value": { "type": ["string", "number", "boolean"], "description": "Value to compare with; the lower bound for between." },
                    "upper": { "type": ["string", "number"], "description": "Upper bound, for between." },
                },
                "required": ["field", "op"],
                "additionalProperties": false,
            },
        },
        "search": { "type": "string", "description": "Keep rows where any column's text contains this, ignoring case." },
        "columns": { "type": "array", "items": { "type": "string" }, "description": "Send only these columns, in this order." },
        "offset": { "type": "integer", "minimum": 0, "description": "Rows to skip, for the next page." },
        "limit": { "type": "integer", "minimum": 1, "maximum": MAX_ROWS, "description": format!("Rows to send (default {DEFAULT_ROWS}, at most {MAX_ROWS}).") },
    })
}

fn with(mut base: Value, extra: Value) -> Value {
    if let (Some(b), Value::Object(e)) = (base.as_object_mut(), extra) {
        b.extend(e);
    }
    base
}

fn result_property() -> Value {
    json!({ "type": "string", "description": "Result name: \"latest\" (default, the last solve), \"base\", \"study\", \"contingency 3\", \"trip line:1:2:1\"." })
}

fn open_case_input() -> Value {
    object(
        json!({
            "path": { "type": "string", "description": "Case file to open (absolute path)." },
            "sample": { "type": "string", "description": "A built-in sample's id from list_sample_cases, such as case9." },
        }),
        &[],
    )
}

fn solve_input() -> Value {
    // Each setting on its own (the easy way to ask for one), and all of
    // them in `settings`, as the core takes them.
    let settings = core_schema("SolveSettings", true);
    let mut properties = settings["properties"].clone();
    properties["settings"] = settings;
    object(properties, &[])
}

fn violations_input() -> Value {
    object(
        json!({
            "result": result_property(),
            "limit_pct": { "type": "number", "description": "Branch loading, percent of rate A, above which a branch counts (default 100)." },
            "limit": { "type": "integer", "minimum": 1, "maximum": MAX_ROWS, "description": "Most rows of each kind (default 20)." },
        }),
        &[],
    )
}

fn query_results_input() -> Value {
    let mut p = page_properties("vm, loading_pct or pg");
    p = with(
        p,
        json!({
            "table": { "type": "string", "enum": ["buses", "branches", "generators"] },
            "result": result_property(),
        }),
    );
    object(p, &["table"])
}

fn element_input() -> Value {
    object(
        json!({ "element": { "type": "string", "description": "Element id, such as bus:5, line:4:5:1 or generator:1:1." } }),
        &["element"],
    )
}

fn set_edits_input() -> Value {
    object(
        json!({ "edits": { "type": "array", "items": core_schema("Edit", false), "description": "The full list of edits, replacing any there were." } }),
        &["edits"],
    )
}

fn contingencies_input() -> Value {
    let mut schema = core_schema("ContingencySettings", true);
    // The same settings solve_power_flow lists in full.
    schema["properties"]["solve"] = json!({
        "type": "object",
        "description": "How each outage is solved: the same settings as solve_power_flow's `settings`, defaults for any left out.",
    });
    schema
}

fn transfer_input() -> Value {
    let mut schema = core_schema("TransferSettings", false);
    schema["properties"]["max_rows"]["description"] =
        json!("Most rows in each list (default 20 here).");
    schema
}

fn planning_input() -> Value {
    core_schema("PlanningSettings", true)
}

fn query_contingencies_input() -> Value {
    object(page_properties("severity or worst.pct"), &[])
}

fn detail_input() -> Value {
    object(
        json!({
            "rank": { "type": "integer", "minimum": 1, "description": "The outage's rank in the last run (1 is the worst)." },
            "limit": { "type": "integer", "minimum": 1, "maximum": MAX_ROWS, "description": "Most changed branches and buses to list (default 20)." },
        }),
        &["rank"],
    )
}

fn trip_input() -> Value {
    object(
        json!({
            "element": { "type": "string", "description": "Branch or generator id, such as line:4:5:1." },
            "limit_pct": { "type": "number", "description": "Loading, percent of the rating, above which a branch is overloaded (default 100)." },
            "rating": { "type": "string", "enum": ["a", "b", "c"], "description": "The rating loadings are measured against after the trip (default a: rate A)." },
            "base_rating": { "type": "string", "enum": ["a", "b", "c"], "description": "The rating for loadings before the trip (default a: rate A)." },
            "limit": { "type": "integer", "minimum": 1, "maximum": MAX_ROWS, "description": "Most changed branches and buses to list (default 20)." },
        }),
        &["element"],
    )
}

fn limit_input() -> Value {
    object(
        json!({ "limit": { "type": "integer", "minimum": 1, "maximum": MAX_ROWS, "description": "Most bridges and articulation buses to list (default 50)." } }),
        &[],
    )
}

fn path_input() -> Value {
    object(
        json!({
            "from": { "type": "integer", "description": "Bus number." },
            "to": { "type": "integer", "description": "Bus number." },
        }),
        &["from", "to"],
    )
}

fn query_records_input() -> Value {
    let p = with(
        page_properties("a field name such as BASKV"),
        json!({ "section": { "type": "string", "description": "Record type id from list_records, such as BUS or BRANCH (any case)." } }),
    );
    object(p, &["section"])
}

fn edit_records_input() -> Value {
    object(
        json!({
            "edits": {
                "type": "array",
                "items": {
                    "type": "object",
                    "properties": {
                        "section": { "type": "string" },
                        "row": { "type": "integer", "minimum": 0, "description": "The row's `row` from query_records." },
                        "field": { "type": "string", "description": "Field name, such as VM." },
                        "value": { "type": ["string", "number", "null"] },
                    },
                    "required": ["section", "row", "field", "value"],
                    "additionalProperties": false,
                },
            },
        }),
        &["edits"],
    )
}

fn compare_input() -> Value {
    object(
        json!({
            "baseline": { "type": "string", "description": "RAW file to compare from (absolute path)." },
            "comparison": { "type": "string", "description": "RAW file to compare with (absolute path)." },
        }),
        &[],
    )
}

fn diff_records_input() -> Value {
    object(
        json!({
            "section": { "type": "string", "description": "Section name from compare_cases, such as BUS." },
            "include_unchanged": { "type": "boolean" },
            "offset": { "type": "integer", "minimum": 0 },
            "limit": { "type": "integer", "minimum": 1, "maximum": MAX_ROWS, "description": format!("Default {DEFAULT_ROWS}.") },
        }),
        &["section"],
    )
}

fn export_input() -> Value {
    object(
        json!({
            "path": { "type": "string", "description": "File to write (absolute path); .raw, .m, .stn or .stnb." },
            "revision": { "type": "integer", "enum": [33, 35], "description": "RAW revision (default 35)." },
            "solved": { "type": "boolean", "description": "MATPOWER only: write the solved voltages and dispatch (default false)." },
        }),
        &["path"],
    )
}

fn run_file_input() -> Value {
    object(
        json!({
            "path": { "type": "string", "description": "Run file (absolute path)." },
            "keep_going": { "type": "boolean", "description": "Carry on after a line fails." },
        }),
        &["path"],
    )
}

fn call_command_input() -> Value {
    object(
        json!({
            "command": { "type": "string", "description": "Registry command name, from list_commands." },
            "args": { "type": "object", "description": "Its arguments (camelCase names)." },
        }),
        &["command"],
    )
}

fn module_input() -> Value {
    object(
        json!({ "module": { "type": "string", "enum": ["diagram", "power", "records", "diff"] } }),
        &["module"],
    )
}

fn say_input() -> Value {
    object(
        json!({
            "message": { "type": "string" },
            "level": { "type": "string", "enum": ["info", "warn", "error"] },
        }),
        &["message"],
    )
}

// ---------------------------------------------------------------- arguments

fn text<'a>(args: &'a Value, name: &str) -> Result<&'a str, Failure> {
    args[name]
        .as_str()
        .filter(|s| !s.trim().is_empty())
        .ok_or_else(|| Failure::invalid(format!("{name}: give it as text.")))
}

fn opt_text<'a>(args: &'a Value, name: &str) -> Option<&'a str> {
    args[name].as_str().filter(|s| !s.trim().is_empty())
}

fn count(args: &Value, name: &str, default: u64) -> u64 {
    args[name].as_u64().unwrap_or(default).clamp(1, MAX_ROWS)
}

fn result_name(args: &Value) -> &str {
    opt_text(args, "result").unwrap_or("latest")
}

/// A filter value as the core's rules take it: text.
fn rule_text(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Null => String::new(),
        other => other.to_string(),
    }
}

/// The core's PageQuery from a table tool's arguments. `field` maps a
/// column name to the core's (records use positions).
fn page_query(
    args: &Value,
    field: &dyn Fn(&str) -> Result<String, Failure>,
) -> Result<Value, Failure> {
    let mut query = json!({
        "offset": args["offset"].as_u64().unwrap_or(0),
        "limit": count(args, "limit", DEFAULT_ROWS),
    });
    if let Some(sort) = opt_text(args, "sort") {
        query["sort"] = json!({ "field": field(sort)?, "descending": args["descending"].as_bool().unwrap_or(false) });
    }
    if let Some(search) = opt_text(args, "search") {
        query["search"] = json!(search);
    }
    if let Some(filters) = args["filters"].as_array() {
        let mut rules = Vec::new();
        for f in filters {
            let name = f["field"]
                .as_str()
                .ok_or_else(|| Failure::invalid("filters: each needs a field."))?;
            let op = f["op"]
                .as_str()
                .ok_or_else(|| Failure::invalid("filters: each needs an op."))?;
            let mut rule =
                json!({ "field": field(name)?, "operator": op, "value": rule_text(&f["value"]) });
            if !f["upper"].is_null() {
                rule["upper"] = json!(rule_text(&f["upper"]));
            }
            rules.push(rule);
        }
        query["rules"] = json!(rules);
    }
    Ok(query)
}

/// Records as a table, these columns first.
fn led(records: &[Value], first: &[&str]) -> Value {
    let mut rows = Rows::from_records(records);
    rows.lead(first);
    rows.to_json()
}

fn same(name: &str) -> Result<String, Failure> {
    Ok(name.to_string())
}

fn columns(args: &Value) -> Vec<String> {
    args["columns"]
        .as_array()
        .map(|c| {
            c.iter()
                .filter_map(|v| v.as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default()
}

/// A page answer as `{total, matched, offset, columns, rows, next_offset}`.
fn page_answer(page: &Value, mut rows: Rows, args: &Value, extra: Value) -> Answer {
    let wanted = columns(args);
    if !wanted.is_empty() {
        rows.pick(&wanted).map_err(Failure::invalid)?;
    }
    let offset = page["offset"].as_u64().unwrap_or(0);
    let matched = page["matched"].as_u64().unwrap_or(0);
    let sent = rows.rows.len() as u64;
    let mut out = json!({
        "total": page["total"],
        "matched": matched,
        "offset": offset,
    });
    if offset + sent < matched {
        out["next_offset"] = json!(offset + sent);
    }
    let table = rows.to_json();
    out["columns"] = table["columns"].clone();
    out["rows"] = table["rows"].clone();
    if let (Some(o), Value::Object(e)) = (out.as_object_mut(), extra) {
        o.extend(e);
    }
    Ok(out)
}

// ---------------------------------------------------------------- cases

fn list_samples(c: &Call, _: &Value) -> Answer {
    let samples = c.backend.call("sample_cases", json!({}))?;
    let mut rows = Rows::from_records(samples.as_array().map(Vec::as_slice).unwrap_or(&[]));
    rows.lead(&["id", "title", "buses", "format"]);
    rows.drop(&["file_name"]);
    Ok(rows.to_json())
}

/// A case summary, smaller: the first warnings only.
fn summary(s: &Value) -> Value {
    if s.is_null() {
        return json!({ "open": false, "note": "No case is open. Use open_case." });
    }
    let power = &s["power"];
    let warnings = power["info"]["warnings"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    let mut out = json!({
        "id": s["id"],
        "name": s["name"],
        "path": s["path"],
        "format": s["format"],
        "revision": power["info"]["revision"],
        "counts": power["info"]["counts"],
        "warning_count": warnings.len(),
        "warnings": warnings.iter().take(10).map(|w| {
            match w["element"].as_str() {
                Some(e) => format!("line {}: {} ({e})", w["line"], w["message"].as_str().unwrap_or("")),
                None => format!("line {}: {}", w["line"], w["message"].as_str().unwrap_or("")),
            }
        }).collect::<Vec<_>>(),
        "record_edits": s["record_edits"],
        "diffable": s["diffable"],
    });
    if let Some(e) = power["error"].as_str() {
        out["power_flow_unavailable"] = json!(e);
    }
    if let Some(e) = s["topology"]["error"].as_str() {
        out["topology_unavailable"] = json!(e);
    }
    let coords = &s["coordinates"];
    if !coords.is_null() {
        out["coordinates"] = json!({
            "file": coords["name"],
            "from_file": coords["from_file"],
            "estimated": coords["estimated"],
            "missing": coords["missing"],
        });
    }
    if !s["dc_grids"].is_null() {
        out["dc_grids"] = s["dc_grids"].clone();
    }
    out
}

fn open_case(c: &Call, args: &Value) -> Answer {
    let opened = match (opt_text(args, "path"), opt_text(args, "sample")) {
        (Some(path), None) => c.backend.call("open_case", json!({ "path": path }))?,
        (None, Some(id)) => c.backend.call("open_sample", json!({ "id": id }))?,
        _ => return Err(Failure::invalid("Give a path or a sample, not both.")),
    };
    Ok(summary(&opened))
}

fn case_summary(c: &Call, _: &Value) -> Answer {
    let s = c.backend.call("case_summary", json!({}))?;
    let mut out = summary(&s);
    if !s.is_null() {
        out["study_edits"] = c.backend.call("study_edits", json!({}))?;
    }
    Ok(out)
}

// ---------------------------------------------------------------- power flow

/// A solve report without its per-element tables.
fn solve_summary(r: &Value) -> Value {
    let mut out = json!({
        "status": r["status"],
        "converged": r["converged"],
        "model": r["model"],
        "method": r["method"],
        "iterations": r["iterations"],
        "decoupled_iterations": r["decoupled_iterations"],
        "newton_fallbacks": r["newton_fallbacks"],
        "residual_mva": r["residual_mva"],
        "total_generation_mw": r["total_generation_mw"],
        "total_load_mw": r["total_load_mw"],
        "total_losses_mw": r["total_losses_mw"],
        "elapsed_ms": r["elapsed_ms"],
    });
    let limits = &r["limits"];
    out["limits"] = json!({
        "voltage_violations": limits["voltage_violations"],
        "branch_overloads": limits["branch_overloads"],
        "reactive_violations": limits["reactive_violations"],
    });
    if let Some([from, to, pct]) = limits["worst_branch"].as_array().map(Vec::as_slice) {
        out["limits"]["most_loaded_branch"] = json!({ "from": from, "to": to, "loading_pct": pct });
    }
    if let Some([bus, outside]) = limits["worst_voltage"].as_array().map(Vec::as_slice) {
        out["limits"]["worst_voltage"] = json!({ "bus": bus, "outside_pu": outside });
    }
    if let Some([bus, over]) = limits["worst_reactive"].as_array().map(Vec::as_slice) {
        out["limits"]["worst_reactive"] = json!({ "bus": bus, "outside_mvar": over });
    }
    let mut controls = Map::new();
    for k in [
        "q_limit_passes",
        "taps_moved",
        "shunts_switched",
        "phase_shifters_moved",
        "voltage_control_passes",
        "flow_control_passes",
        "interchange_passes",
        "dc_tap_passes",
        "vsc_limit_passes",
    ] {
        if r[k].as_u64().is_some_and(|n| n > 0) {
            controls.insert(k.into(), r[k].clone());
        }
    }
    for k in ["taps_locked", "shunts_locked", "phase_shifters_locked"] {
        if r[k].as_bool() == Some(true) {
            controls.insert(k.into(), json!(true));
        }
    }
    out["controls"] = Value::Object(controls);
    if r["area_interchange"].as_str().is_some_and(|m| m != "off") {
        out["area_interchange"] = r["area_interchange"].clone();
        out["areas"] = r["areas"].clone();
    }
    for k in ["dc_lines", "facts"] {
        if let Some(n) = r[k].as_array().map(Vec::len).filter(|n| *n > 0) {
            out[format!("{k}_count")] = json!(n);
        }
    }
    let counts = |k: &str| r[k]["count"].clone();
    out["buses"] = counts("buses");
    out["branches"] = counts("branches");
    out["generators"] = counts("generators");
    out
}

fn solve(c: &Call, args: &Value) -> Answer {
    let mut solve_args = json!({});
    if args["settings"].is_object() {
        solve_args["settings"] = args["settings"].clone();
    }
    for name in stannum_core::commands::flat_settings("solve_power_flow") {
        if let Some(value) = args.get(name.as_str()).filter(|v| !v.is_null()) {
            solve_args[name.as_str()] = value.clone();
        }
    }
    let report = c.backend.call("solve_power_flow", solve_args)?;
    if c.cancelled.load(Ordering::SeqCst) {
        return Err(Failure::new("cancelled", "The solve was cancelled."));
    }
    Ok(solve_summary(&report))
}

fn result_page(c: &Call, table: &str, name: &str, query: Value) -> Answer {
    let command = match table {
        "buses" => "result_buses",
        "branches" => "result_branches",
        "generators" => "result_generators",
        other => {
            return Err(Failure::invalid(format!(
                "table: buses, branches or generators, not {other}."
            )));
        }
    };
    c.backend
        .call(command, json!({ "name": name, "query": query }))
}

fn lead_for(table: &str) -> &'static [&'static str] {
    match table {
        "buses" => &["id", "name", "kind", "area", "base_kv", "vm", "va"],
        "branches" => &[
            "element",
            "from",
            "to",
            "circuit",
            "online",
            "pf",
            "qf",
            "loading_pct",
            "rate_a",
        ],
        _ => &["element", "bus", "bus_name", "online", "pg", "qg"],
    }
}

/// Columns that are internal or rarely useful to a reader.
const NOISE: &[&str] = &["index", "winding", "load_parts"];

/// The columns a sort or filter names must be in the table: the core
/// would quietly leave an unknown one out.
fn check_fields(args: &Value, table: &Value) -> Result<(), Failure> {
    let Some(columns) = table.as_object() else {
        return Ok(());
    };
    let known = |f: &str| columns.contains_key(f.split('.').next().unwrap_or(f));
    let mut named: Vec<&str> = opt_text(args, "sort").into_iter().collect();
    named.extend(
        args["filters"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|f| f["field"].as_str()),
    );
    match named.into_iter().find(|f| !known(f)) {
        Some(f) => {
            let names: Vec<&str> = columns
                .keys()
                .map(String::as_str)
                .filter(|k| *k != "count")
                .collect();
            Err(Failure::invalid(format!(
                "No column {f}. Columns: {}.",
                names.join(", ")
            )))
        }
        None => Ok(()),
    }
}

fn query_results(c: &Call, args: &Value) -> Answer {
    let table = text(args, "table")?;
    let name = result_name(args);
    let page = result_page(c, table, name, page_query(args, &same)?)?;
    check_fields(args, &page["rows"])?;
    let mut rows = Rows::from_columns(&page["rows"]);
    rows.drop(NOISE);
    rows.lead(lead_for(table));
    page_answer(&page, rows, args, json!({ "table": table, "result": name }))
}

fn violations(c: &Call, args: &Value) -> Answer {
    let name = result_name(args);
    let limit = count(args, "limit", 20);
    let pct = args["limit_pct"].as_f64().unwrap_or(100.0);
    let mut out = json!({ "result": name });
    let ask = |table: &str, field: &str, op: &str, value: String, descending: bool| -> Answer {
        let query = json!({
            "limit": limit,
            "sort": { "field": field, "descending": descending },
            "rules": [{ "field": field, "operator": op, "value": value }],
        });
        let page = result_page(c, table, name, query)?;
        let mut rows = Rows::from_columns(&page["rows"]);
        rows.drop(NOISE);
        rows.lead(lead_for(table));
        let mut t = rows.to_json();
        t["count"] = page["matched"].clone();
        Ok(t)
    };
    // `violation` is how far outside its limits a bus is, in p.u.
    out["buses_outside_voltage_limits"] = ask("buses", "violation", "gt", "0".into(), true)?;
    out["overloaded_branches"] = ask("branches", "loading_pct", "gt", pct.to_string(), true)?;
    out["overloaded_branches"]["limit_pct"] = json!(pct);
    out["generators_outside_q_limits"] = ask("generators", "q_violation", "gt", "0".into(), true)?;
    Ok(out)
}

fn element_detail(c: &Call, args: &Value) -> Answer {
    let d = c.backend.call(
        "element_detail",
        json!({ "element": text(args, "element")? }),
    )?;
    // Fields as "label: value unit", by group.
    let line = |f: &Value| {
        let unit = f["unit"].as_str().unwrap_or("");
        let value = match &f["value"] {
            Value::String(s) => s.clone(),
            other => {
                let mut v = other.clone();
                round(&mut v, PLACES);
                v.to_string()
            }
        };
        let label = f["label"].as_str().unwrap_or("");
        if unit.is_empty() {
            format!("{label}: {value}")
        } else {
            format!("{label}: {value} {unit}")
        }
    };
    let groups: Vec<Value> = d["data"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|g| {
            json!({
                "group": g["title"],
                "fields": g["fields"].as_array().into_iter().flatten().map(line).collect::<Vec<_>>(),
            })
        })
        .collect();
    let mut out = json!({
        "element": d["element"],
        "kind": d["kind"],
        "online": d["online"],
        "buses": d["buses"],
        "data": groups,
        "limits": d["limits"].as_array().into_iter().flatten().map(line).collect::<Vec<_>>(),
        "connected": d["connected"],
    });
    if !d["note"].is_null() {
        out["note"] = d["note"].clone();
    }
    Ok(out)
}

// ---------------------------------------------------------------- study

fn get_edits(c: &Call, _: &Value) -> Answer {
    let edits = c.backend.call("study_edits", json!({}))?;
    Ok(json!({ "edits": edits }))
}

fn set_edits(c: &Call, args: &Value) -> Answer {
    let edits = args["edits"].clone();
    if !edits.is_array() {
        return Err(Failure::invalid("edits: give a list (empty to clear)."));
    }
    c.backend
        .call("set_study_edits", json!({ "edits": edits }))?;
    let now = c.backend.call("study_edits", json!({}))?;
    Ok(json!({ "edits": now, "note": "Solve again to see their effect." }))
}

// ---------------------------------------------------------------- contingencies

/// Follows a long run's progress on the side and reports it, until dropped.
struct Follow<'a> {
    stop: &'a AtomicBool,
}

impl Drop for Follow<'_> {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
    }
}

fn run_contingencies(c: &Call, args: &Value) -> Answer {
    let settings = args.clone();
    let stop = AtomicBool::new(false);
    let summary = std::thread::scope(|scope| {
        scope.spawn(|| {
            let mut last = (usize::MAX, String::new());
            while !stop.load(Ordering::SeqCst) {
                std::thread::sleep(Duration::from_millis(400));
                if stop.load(Ordering::SeqCst) {
                    break;
                }
                let Ok(p) = c.backend.side("contingency_progress") else {
                    continue;
                };
                let phase = p["phase"].as_str().unwrap_or("idle").to_string();
                let done = p["done"].as_u64().unwrap_or(0) as usize;
                if phase == "idle" || (done, phase.clone()) == last {
                    continue;
                }
                let total = p["total"].as_u64().unwrap_or(0);
                let what = if phase == "ac" {
                    "AC solves"
                } else {
                    "screening"
                };
                (c.progress)(
                    done as f64,
                    Some(total as f64),
                    &format!("{what}: {done} of {total}"),
                );
                last = (done, phase);
            }
        });
        let _follow = Follow { stop: &stop };
        c.backend
            .call("run_contingencies", json!({ "settings": settings }))
    })?;
    let mut out = summary;
    if let Some(o) = out.as_object_mut() {
        for k in ["screen_ms", "ac_ms"] {
            o.remove(k);
        }
    }
    if c.cancelled.load(Ordering::SeqCst) || out["cancelled"].as_bool() == Some(true) {
        out["note"] = json!("Cancelled: these are the outages done before it stopped.");
    } else {
        out["note"] =
            json!("query_contingencies pages the ranked results; contingency_detail shows one.");
    }
    Ok(out)
}

fn transfer_limits(c: &Call, args: &Value) -> Answer {
    let mut settings = args.clone();
    if settings.get("max_rows").is_none() {
        settings["max_rows"] = json!(20);
    }
    let stop = AtomicBool::new(false);
    let r = std::thread::scope(|scope| {
        scope.spawn(|| {
            let mut last = usize::MAX;
            while !stop.load(Ordering::SeqCst) {
                std::thread::sleep(Duration::from_millis(400));
                if stop.load(Ordering::SeqCst) {
                    break;
                }
                let Ok(p) = c.backend.side("transfer_progress") else {
                    continue;
                };
                let done = p["done"].as_u64().unwrap_or(0) as usize;
                if p["phase"] == "idle" || done == last {
                    continue;
                }
                let total = p["total"].as_u64().unwrap_or(0);
                (c.progress)(
                    done as f64,
                    Some(total as f64),
                    &format!("cases studied: {done} of {total}"),
                );
                last = done;
            }
        });
        let _follow = Follow { stop: &stop };
        c.backend
            .call("run_transfer_limits", json!({ "settings": settings }))
    })?;
    let rows = |key: &str, lead: &[&str]| {
        let mut rows = Rows::from_records(r[key].as_array().map(Vec::as_slice).unwrap_or(&[]));
        rows.lead(lead);
        rows.to_json()
    };
    let limit_cols = [
        "monitored",
        "contingency",
        "fcitc_mw",
        "flow_mw",
        "limit_mw",
        "factor",
    ];
    let mut summary = r["summary"].clone();
    if let Some(o) = summary.as_object_mut() {
        o.remove("elapsed_ms");
    }
    if c.cancelled.load(Ordering::SeqCst) || summary["cancelled"].as_bool() == Some(true) {
        summary["note"] = json!("Cancelled: these are the cases studied before it stopped.");
    }
    Ok(json!({
        "summary": summary,
        "limits": rows("limits", &limit_cols),
        "past_limits": rows("past_limits", &limit_cols),
        "contingencies": rows("contingencies", &["contingency", "fcitc_mw", "monitored", "factor"]),
        "factors": rows("factors", &["monitored", "ptdf", "flow_mw", "limit_mw"]),
    }))
}

fn planning_counts(c: &Call, args: &Value) -> Answer {
    let counts = c
        .backend
        .call("planning_event_counts", json!({ "settings": args }))?;
    Ok(Rows::from_records(counts.as_array().map(Vec::as_slice).unwrap_or(&[])).to_json())
}

fn query_contingencies(c: &Call, args: &Value) -> Answer {
    let page = c.backend.call(
        "contingency_page",
        json!({ "query": page_query(args, &same)? }),
    )?;
    check_fields(args, &page["rows"])?;
    let mut rows = Rows::from_columns(&page["rows"]);
    rows.lead(&[
        "rank",
        "element",
        "kind",
        "category",
        "description",
        "severity",
        "overloads",
        "voltage_violations",
        "converged",
    ]);
    page_answer(&page, rows, args, json!({}))
}

/// A contingency detail with only the biggest branch and bus changes.
fn detail_answer(d: &Value, limit: usize) -> Value {
    let mut branches: Vec<Value> = d["branches"].as_array().cloned().unwrap_or_default();
    let change =
        |b: &Value| (b["mw"].as_f64().unwrap_or(0.0) - b["base_mw"].as_f64().unwrap_or(0.0)).abs();
    branches.sort_by(|a, b| change(b).total_cmp(&change(a)));
    let mut buses: Vec<Value> = d["buses"].as_array().cloned().unwrap_or_default();
    let shift =
        |b: &Value| (b["vm"].as_f64().unwrap_or(0.0) - b["base_vm"].as_f64().unwrap_or(0.0)).abs();
    buses.sort_by(|a, b| shift(b).total_cmp(&shift(a)));
    let rows = |items: &[Value]| {
        let mut r = Rows::from_records(&items[..items.len().min(limit)]);
        r.lead(&[
            "element", "bus", "from", "to", "tripped", "base_mw", "mw", "base_pct", "pct",
            "base_vm", "vm",
        ]);
        let mut t = r.to_json();
        t["count"] = json!(items.len());
        t
    };
    json!({
        "outage": d["row"],
        "overloads": d["overloads"],
        "voltage_problems": d["voltage_problems"],
        "cut_off_buses": d["cut_off"],
        "branches_changed_most": rows(&branches),
        "buses_changed_most": rows(&buses),
    })
}

fn contingency_detail(c: &Call, args: &Value) -> Answer {
    let rank = args["rank"]
        .as_u64()
        .ok_or_else(|| Failure::invalid("rank: give a number from 1."))?;
    let d = c
        .backend
        .call("contingency_detail", json!({ "rank": rank }))?;
    Ok(detail_answer(&d, count(args, "limit", 20) as usize))
}

fn trip_and_compare(c: &Call, args: &Value) -> Answer {
    let d = c.backend.call(
        "trip_and_compare",
        json!({
            "element": text(args, "element")?,
            "limitPct": args["limit_pct"].as_f64().unwrap_or(100.0),
            "rating": args["rating"].as_str().unwrap_or("a"),
            "baseRating": args["base_rating"].as_str().unwrap_or("a"),
        }),
    )?;
    Ok(detail_answer(&d, count(args, "limit", 20) as usize))
}

// ---------------------------------------------------------------- topology

fn topology_summary(c: &Call, args: &Value) -> Answer {
    let t = c.backend.call("topology_snapshot", json!({}))?;
    let limit = count(args, "limit", 50) as usize;
    let a = &t["analysis"];
    // Island sizes from the degree table.
    let mut sizes: std::collections::BTreeMap<u64, usize> = Default::default();
    for island in a["degrees"]["island"].as_array().into_iter().flatten() {
        *sizes.entry(island.as_u64().unwrap_or(0)).or_default() += 1;
    }
    let mut islands: Vec<Value> = sizes
        .iter()
        .map(|(id, n)| json!({ "island": id, "buses": n }))
        .collect();
    islands.sort_by_key(|i| std::cmp::Reverse(i["buses"].as_u64().unwrap_or(0)));
    // Bridges are edge positions; edges name equipment by position.
    let equipment_of_edge = t["graph"]["edges"]["equipment"].as_array();
    let ids = t["case"]["equipment"]["id"].as_array();
    let bridges: Vec<Value> = a["bridges"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|e| {
            let eq = equipment_of_edge?.get(e.as_u64()? as usize)?.as_u64()?;
            ids?.get(eq as usize).cloned()
        })
        .collect();
    let articulation = a["articulation_buses"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    Ok(json!({
        "active_buses": a["active_buses"],
        "active_equipment": a["active_equipment"],
        "excluded_buses": a["excluded_buses"],
        "excluded_equipment": a["excluded_equipment"],
        "island_count": islands.len(),
        "islands": islands.into_iter().take(limit).collect::<Vec<_>>(),
        "bridge_count": bridges.len(),
        "bridges": bridges.into_iter().take(limit).collect::<Vec<_>>(),
        "articulation_bus_count": articulation.len(),
        "articulation_buses": articulation.into_iter().take(limit).collect::<Vec<_>>(),
    }))
}

fn shortest_path(c: &Call, args: &Value) -> Answer {
    let (Some(from), Some(to)) = (args["from"].as_i64(), args["to"].as_i64()) else {
        return Err(Failure::invalid("from and to: give bus numbers."));
    };
    let p = c
        .backend
        .call("topology_shortest_path", json!({ "from": from, "to": to }))?;
    if p.is_null() {
        return Ok(
            json!({ "path": null, "note": format!("No path from bus {from} to bus {to}: they are in different islands, or a bus isn't in the case.") }),
        );
    }
    Ok(json!({ "hops": p["hops"], "buses": p["buses"], "equipment": p["equipment_ids"] }))
}

// ---------------------------------------------------------------- records

fn list_records(c: &Call, _: &Value) -> Answer {
    let book = c.backend.call("case_records", json!({}))?;
    let sections: Vec<Value> = book["sections"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|s| s["count"].as_u64().unwrap_or(0) > 0)
        .cloned()
        .collect();
    let mut out = json!({
        "editable": book["editable"],
        "revision": book["revision"],
        "sections": led(&sections, &["id", "label", "count"]),
    });
    if !book["note"].is_null() {
        out["note"] = book["note"].clone();
    }
    Ok(out)
}

/// The section id as the case spells it, matched ignoring case.
fn section_id(c: &Call, asked: &str) -> Result<String, Failure> {
    let book = c.backend.call("case_records", json!({}))?;
    let sections = book["sections"].as_array().cloned().unwrap_or_default();
    sections
        .iter()
        .find_map(|s| {
            let id = s["id"].as_str()?;
            let label = s["label"].as_str().unwrap_or("");
            (id.eq_ignore_ascii_case(asked) || label.eq_ignore_ascii_case(asked))
                .then(|| id.to_string())
        })
        .ok_or_else(|| {
            let ids: Vec<&str> = sections.iter().filter_map(|s| s["id"].as_str()).collect();
            Failure::new(
                "not_found",
                format!("No record type {asked}. The case has: {}.", ids.join(", ")),
            )
        })
}

/// A record type's field names, by position.
fn record_fields(c: &Call, section: &str) -> Result<Vec<String>, Failure> {
    let page = c.backend.call(
        "record_page",
        json!({ "section": section, "query": { "limit": 1 } }),
    )?;
    Ok(page["columns"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|c| c["name"].as_str().map(str::to_string))
        .collect())
}

fn field_position(fields: &[String], name: &str) -> Result<String, Failure> {
    if name == "line" || name == "element" {
        return Ok(name.to_string());
    }
    fields
        .iter()
        .position(|f| f.eq_ignore_ascii_case(name))
        .map(|i| i.to_string())
        .ok_or_else(|| Failure::invalid(format!("No field {name}. Fields: {}.", fields.join(", "))))
}

fn query_records(c: &Call, args: &Value) -> Answer {
    let section = section_id(c, text(args, "section")?)?;
    let fields = record_fields(c, &section)?;
    let query = page_query(args, &|name| field_position(&fields, name))?;
    let page = c
        .backend
        .call("record_page", json!({ "section": section, "query": query }))?;
    let p = &page["page"];
    let cells = p["rows"]["cells"].as_array().cloned().unwrap_or_default();
    let positions = p["positions"].as_array().cloned().unwrap_or_default();
    let elements = p["rows"]["element"].as_array().cloned().unwrap_or_default();
    let mut rows = Rows {
        columns: ["row", "element"]
            .iter()
            .map(|s| s.to_string())
            .chain(fields.iter().cloned())
            .collect(),
        rows: Vec::new(),
    };
    for (i, row) in cells.iter().enumerate() {
        let mut r = vec![
            positions.get(i).cloned().unwrap_or(Value::Null),
            elements.get(i).cloned().unwrap_or(Value::Null),
        ];
        r.extend(row.as_array().cloned().unwrap_or_default());
        rows.rows.push(r);
    }
    let mut extra = json!({ "section": section, "label": page["label"] });
    if !page["note"].is_null() {
        extra["note"] = page["note"].clone();
    }
    page_answer(p, rows, args, extra)
}

fn edit_records(c: &Call, args: &Value) -> Answer {
    let edits = args["edits"]
        .as_array()
        .ok_or_else(|| Failure::invalid("edits: give a list."))?;
    let mut out = Vec::new();
    let mut fields_of: std::collections::HashMap<String, Vec<String>> = Default::default();
    for e in edits {
        let section = section_id(c, text(e, "section")?)?;
        if !fields_of.contains_key(&section) {
            fields_of.insert(section.clone(), record_fields(c, &section)?);
        }
        let column: usize = field_position(&fields_of[&section], text(e, "field")?)?
            .parse()
            .map_err(|_| Failure::invalid("field: give a field name, not line or element."))?;
        let row = e["row"]
            .as_u64()
            .ok_or_else(|| Failure::invalid("row: give the row's position from query_records."))?;
        out.push(json!({ "section": section, "row": row, "column": column, "value": rule_text(&e["value"]) }));
    }
    let summary_after = c
        .backend
        .call("apply_record_edits", json!({ "edits": out }))?;
    Ok(summary(&summary_after))
}

// ---------------------------------------------------------------- comparing

fn compare(c: &Call, args: &Value) -> Answer {
    let d = match (opt_text(args, "baseline"), opt_text(args, "comparison")) {
        (Some(b), Some(cmp)) => c
            .backend
            .call("diff_compare", json!({ "baseline": b, "comparison": cmp }))?,
        (None, None) => c.backend.call("diff_compare_working", json!({}))?,
        _ => {
            return Err(Failure::invalid(
                "Give both baseline and comparison, or neither to compare the open case with its file.",
            ));
        }
    };
    let sections: Vec<Value> = d["sections"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|s| {
            let n = &s["counts"];
            json!({ "section": s["name"], "added": n["added"], "removed": n["removed"], "changed": n["changed"], "unchanged": n["unchanged"] })
        })
        .collect();
    let notes: Vec<String> = d["diagnostics"]
        .as_array()
        .into_iter()
        .flatten()
        .take(10)
        .map(|n| {
            format!(
                "{} {} line {}: {}",
                n["side"].as_str().unwrap_or(""),
                n["section"].as_str().unwrap_or(""),
                n["line"],
                n["message"].as_str().unwrap_or("")
            )
        })
        .collect();
    Ok(json!({
        "baseline": d["baseline"],
        "comparison": d["comparison"],
        "differing_records": d["differing"],
        "complete": d["complete"],
        "sections": led(&sections, &["section", "added", "removed", "changed"]),
        "notes": notes,
    }))
}

fn diff_records(c: &Call, args: &Value) -> Answer {
    let section = text(args, "section")?;
    let all = c
        .backend
        .call("diff_section_records", json!({ "section": section }))?;
    let keep_unchanged = args["include_unchanged"].as_bool().unwrap_or(false);
    let records: Vec<Value> = all
        .as_array()
        .into_iter()
        .flatten()
        .filter(|r| keep_unchanged || r["status"].as_str() != Some("unchanged"))
        .cloned()
        .collect();
    let offset = args["offset"].as_u64().unwrap_or(0) as usize;
    let limit = count(args, "limit", DEFAULT_ROWS) as usize;
    let page: Vec<Value> = records.iter().skip(offset).take(limit).cloned().collect();
    let mut rows = Rows::from_records(&page);
    rows.lead(&["status", "element", "key", "name"]);
    let mut out = rows.to_json();
    out["matched"] = json!(records.len());
    out["offset"] = json!(offset);
    if offset + page.len() < records.len() {
        out["next_offset"] = json!(offset + page.len());
    }
    Ok(out)
}

// ---------------------------------------------------------------- files

fn export(c: &Call, args: &Value) -> Answer {
    let path = text(args, "path")?;
    let ext = std::path::Path::new(path)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    let answer = match ext.as_str() {
        "raw" => {
            let revision = args["revision"].as_u64().unwrap_or(35);
            c.backend
                .call("export_raw", json!({ "path": path, "revision": revision }))?
        }
        "m" => c.backend.call(
            "export_matpower",
            json!({ "path": path, "solved": args["solved"].as_bool().unwrap_or(false) }),
        )?,
        "stn" | "stnb" => c.backend.call("export_stn", json!({ "path": path }))?,
        _ => return Err(Failure::invalid("path: end it in .raw, .m, .stn or .stnb.")),
    };
    let mut out = json!({ "written": path });
    if let Some(o) = answer.as_object() {
        for (k, v) in o {
            out[k] = v.clone();
        }
    }
    Ok(out)
}

fn run_run_file(c: &Call, args: &Value) -> Answer {
    let report = c.backend.call(
        "run_file",
        json!({ "path": text(args, "path")?, "keepGoing": args["keep_going"].as_bool().unwrap_or(false) }),
    )?;
    let outcomes: Vec<Value> = report["outcomes"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|o| json!({ "line": o["line"], "command": o["command"], "seconds": o["seconds"], "error": o["error"]["message"] }))
        .collect();
    let mut out = report.clone();
    if let Some(o) = out.as_object_mut() {
        o.insert("outcomes".into(), Rows::from_records(&outcomes).to_json());
    }
    Ok(out)
}

// ---------------------------------------------------------------- registry

fn list_commands(c: &Call, _: &Value) -> Answer {
    let list = c.backend.call("list_commands", json!({}))?;
    let commands: Vec<Value> = list["commands"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|c| json!({ "name": c["name"], "changes_case": c["action"], "doc": c["doc"] }))
        .collect();
    Ok(json!({
        "api_version": list["api_version"],
        "commands": led(&commands, &["name", "changes_case"]),
        "note": "Arguments are camelCase; call_command with command_schemas gives each command's JSON Schema (large).",
    }))
}

fn call_command(c: &Call, args: &Value) -> Answer {
    let command = text(args, "command")?;
    if command.starts_with("ui.") && c.backend.app().is_none() {
        return Err(Failure::new(
            "unsupported",
            "ui.* commands need the server attached to the app (stannum-cli mcp --attach).",
        ));
    }
    let answer = c
        .backend
        .call(command, args.get("args").cloned().unwrap_or(json!({})))?;
    let text = answer.to_string();
    if text.chars().count() > MAX_RAW {
        let cut: String = text.chars().take(MAX_RAW).collect();
        return Ok(Value::String(format!(
            "{cut}\n…cut short at {MAX_RAW} of {} characters. Use a paged command (result_buses, record_page, contingency_page) or a curated tool instead.",
            text.chars().count()
        )));
    }
    Ok(answer)
}

// ---------------------------------------------------------------- the window

fn app_state(c: &Call, _: &Value) -> Answer {
    c.backend.call("ui.state", json!({}))
}

fn app_show(c: &Call, args: &Value) -> Answer {
    c.backend
        .call("ui.show", json!({ "element": text(args, "element")? }))
}

fn app_go(c: &Call, args: &Value) -> Answer {
    c.backend
        .call("ui.go", json!({ "module": text(args, "module")? }))
}

fn app_draw_around(c: &Call, args: &Value) -> Answer {
    c.backend.call(
        "ui.draw_around",
        json!({ "element": text(args, "element")? }),
    )
}

fn app_say(c: &Call, args: &Value) -> Answer {
    c.backend.call(
        "ui.say",
        json!({ "message": text(args, "message")?, "level": opt_text(args, "level").unwrap_or("info") }),
    )?;
    Ok(json!({ "said": true }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_are_unique_and_schemas_are_objects() {
        let mut names: Vec<_> = TOOLS.iter().map(|t| t.name).collect();
        names.sort();
        names.dedup();
        assert_eq!(names.len(), TOOLS.len());
        for t in TOOLS {
            let s = (t.input)();
            assert_eq!(s["type"], "object", "{}", t.name);
            assert!(
                !s.to_string().contains("$ref"),
                "{} has a $ref left",
                t.name
            );
            assert!(
                t.description.len() < 1024,
                "{} description too long",
                t.name
            );
        }
    }

    #[test]
    fn app_tools_are_listed_only_when_attached() {
        let alone = list(false);
        let attached = list(true);
        assert!(
            alone
                .iter()
                .all(|t| !t["name"].as_str().unwrap().starts_with("app_"))
        );
        assert!(attached.len() > alone.len());
    }

    #[test]
    fn solve_settings_schema_comes_from_the_core() {
        let s = solve_input();
        let settings = &s["properties"]["settings"];
        assert!(
            settings["properties"]["enforce_q_limits"].is_object(),
            "{settings}"
        );
        assert!(settings.get("required").is_none());
        let methods = settings["properties"]["method"].to_string();
        assert!(methods.contains("fast_decoupled_xb"), "{methods}");
    }

    #[test]
    fn solve_settings_can_be_given_one_by_one() {
        let s = solve_input();
        let methods = s["properties"]["method"].to_string();
        assert!(methods.contains("fast_decoupled_xb"), "{methods}");
        assert!(s["properties"]["tolerance_mva"].is_object(), "{s}");
    }
}
