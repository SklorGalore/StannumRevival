//! `stannum-cli mcp` end to end: a client on stdin and stdout, on its own
//! and attached to an app.

use serde_json::{Value, json};
use stannum_core::attach::{Server, Window};
use stannum_core::commands::Session;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::{Arc, Mutex};

struct Client {
    child: Child,
    input: ChildStdin,
    output: BufReader<ChildStdout>,
    next: u64,
}

impl Client {
    fn start(args: &[&str], env: &[(&str, &Path)]) -> Client {
        let mut command = Command::new(env!("CARGO_BIN_EXE_stannum-cli"));
        command
            .arg("mcp")
            .args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit());
        for (k, v) in env {
            command.env(k, v);
        }
        let mut child = command.spawn().expect("starts");
        let input = child.stdin.take().unwrap();
        let output = BufReader::new(child.stdout.take().unwrap());
        let mut client = Client {
            child,
            input,
            output,
            next: 0,
        };
        let init = client.request(
            "initialize",
            json!({
                "protocolVersion": "2025-06-18",
                "capabilities": {},
                "clientInfo": { "name": "test", "version": "0" },
            }),
        );
        assert_eq!(init["result"]["protocolVersion"], "2025-06-18", "{init}");
        assert_eq!(init["result"]["serverInfo"]["name"], "stannum");
        client.send(&json!({ "jsonrpc": "2.0", "method": "notifications/initialized" }));
        client
    }

    fn send(&mut self, message: &Value) {
        writeln!(self.input, "{message}").unwrap();
        self.input.flush().unwrap();
    }

    fn request(&mut self, method: &str, params: Value) -> Value {
        self.next += 1;
        let id = self.next;
        self.send(&json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params }));
        loop {
            let mut line = String::new();
            assert!(
                self.output.read_line(&mut line).unwrap() > 0,
                "server closed"
            );
            let message: Value = serde_json::from_str(&line).unwrap();
            if message["id"] == id {
                return message;
            }
        }
    }

    /// A tool's answer: its JSON, or Err with the error text.
    fn tool(&mut self, name: &str, args: Value) -> Result<Value, String> {
        let answer = self.request("tools/call", json!({ "name": name, "arguments": args }));
        let result = &answer["result"];
        let text = result["content"][0]["text"]
            .as_str()
            .unwrap_or_else(|| panic!("{answer}"));
        if result["isError"] == true {
            Err(text.to_string())
        } else {
            Ok(serde_json::from_str(text).unwrap_or_else(|_| Value::String(text.into())))
        }
    }

    fn tool_names(&mut self) -> Vec<String> {
        let list = self.request("tools/list", json!({}));
        list["result"]["tools"]
            .as_array()
            .unwrap()
            .iter()
            .map(|t| t["name"].as_str().unwrap().to_string())
            .collect()
    }
}

impl Drop for Client {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("stannum-mcp-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn case9() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../rs-power/tests/fixtures/case9.m")
}

#[test]
fn a_client_opens_solves_and_reads_a_case() {
    let mut c = Client::start(&[], &[]);
    let names = c.tool_names();
    for expected in [
        "open_case",
        "solve_power_flow",
        "find_violations",
        "run_contingencies",
        "call_command",
    ] {
        assert!(names.iter().any(|n| n == expected), "{names:?}");
    }
    // The window's tools only come with --attach.
    assert!(!names.iter().any(|n| n.starts_with("app_")), "{names:?}");

    assert_eq!(c.request("ping", json!({}))["result"], json!({}));
    let none = c.tool("case_summary", json!({})).unwrap();
    assert_eq!(none["open"], false);

    let opened = c.tool("open_case", json!({ "path": case9() })).unwrap();
    assert_eq!(opened["counts"]["buses"], 9, "{opened}");
    let solved = c
        .tool(
            "solve_power_flow",
            json!({ "settings": { "start": "flat" } }),
        )
        .unwrap();
    assert_eq!(solved["converged"], true, "{solved}");
    assert!(
        solved.get("buses").is_some_and(|b| b == 9),
        "a count, not the table: {solved}"
    );
    // Settings one by one, as an assistant most often passes them.
    let newton = c
        .tool(
            "solve_power_flow",
            json!({ "method": "newton", "start": "flat", "tolerance_mva": 0.001 }),
        )
        .unwrap();
    assert_eq!(newton["method"], "newton", "{newton}");
    assert_eq!(newton["decoupled_iterations"], 0, "{newton}");

    let top = c
        .tool(
            "query_results",
            json!({ "table": "branches", "sort": "loading_pct", "descending": true, "limit": 2, "columns": ["element", "loading_pct"] }),
        )
        .unwrap();
    assert_eq!(top["columns"], json!(["element", "loading_pct"]));
    assert_eq!(top["rows"].as_array().unwrap().len(), 2);
    assert_eq!(top["rows"][0][0], "line:2:8:1", "{top}");
    assert_eq!(top["next_offset"], 2);
    let wrong = c
        .tool(
            "query_results",
            json!({ "table": "buses", "sort": "voltage" }),
        )
        .unwrap_err();
    assert!(
        wrong.contains("No column voltage") && wrong.contains("vm"),
        "{wrong}"
    );

    // An outage overloads nothing on case9 at 100%, but does at 60%.
    let v = c
        .tool("find_violations", json!({ "limit_pct": 60 }))
        .unwrap();
    assert_eq!(v["overloaded_branches"]["count"], 1, "{v}");

    let run = c
        .tool("run_contingencies", json!({ "method": "ac" }))
        .unwrap();
    assert_eq!(run["total"], 12, "{run}");
    let transfer = c
        .tool(
            "transfer_limits",
            json!({ "source": { "buses": [2] }, "sink": { "buses": [3] }, "contingencies": {}, "max_rows": 3 }),
        )
        .unwrap();
    assert!(transfer["summary"]["fcitc_mw"].is_number(), "{transfer}");
    assert!(transfer["limits"]["rows"].as_array().unwrap().len() <= 3);
    assert_eq!(transfer["limits"]["columns"][0], "monitored", "{transfer}");
    let pv = c
        .tool(
            "pv_curve",
            json!({ "sink": { "buses": [5] }, "max_curves": 2 }),
        )
        .unwrap();
    assert!(pv["summary"]["nose_mw"].as_f64().unwrap() > 0.0, "{pv}");
    assert_eq!(pv["curves"]["columns"][0], "bus", "{pv}");
    assert!(pv["curves"]["rows"].as_array().unwrap().len() <= 2, "{pv}");
    let qv = c
        .tool("qv_curve", json!({ "buses": [5], "step_pu": 0.05 }))
        .unwrap();
    assert!(qv["summary"]["weakest_margin_mvar"].is_number(), "{qv}");
    assert_eq!(qv["buses"]["columns"][0], "bus", "{qv}");
    assert!(!qv["points"]["rows"].as_array().unwrap().is_empty(), "{qv}");
    let secure = c
        .tool(
            "secure_transfer",
            json!({ "sink": { "buses": [5] }, "major_step_mw": 20.0, "minor_step_mw": 2.0, "respect_generation_limits": false, "max_rows": 3 }),
        )
        .unwrap();
    assert!(secure["summary"]["limit_mw"].is_number(), "{secure}");
    assert_eq!(secure["summary"]["stopped_by"], "violation", "{secure}");
    assert!(
        secure["violations"]["rows"].as_array().unwrap().len() <= 3,
        "{secure}"
    );
    assert_eq!(
        secure["limiting_contingencies"]["columns"][0], "name",
        "{secure}"
    );
    let _ = std::fs::remove_file(secure["summary"]["results_file"].as_str().unwrap());
    let worst = c
        .tool("contingency_detail", json!({ "rank": 1, "limit": 3 }))
        .unwrap();
    assert_eq!(worst["outage"]["rank"], 1);
    assert!(
        worst["buses_changed_most"]["rows"]
            .as_array()
            .unwrap()
            .len()
            <= 3
    );

    // Study edits change the next solve, not the file.
    c.tool(
        "set_study_edits",
        json!({ "edits": [{ "kind": "status", "element": "line:4:9:1", "online": false }] }),
    )
    .unwrap();
    let after = c.tool("solve_power_flow", json!({})).unwrap();
    assert_eq!(after["converged"], true);
    let low = c.tool("find_violations", json!({})).unwrap();
    assert!(
        low["buses_outside_voltage_limits"]["count"]
            .as_u64()
            .unwrap()
            >= 1,
        "{low}"
    );

    let dir = scratch("export");
    let out = dir.join("study.raw");
    let written = c.tool("export_case", json!({ "path": out })).unwrap();
    assert_eq!(written["revision"], 35, "{written}");
    assert!(out.exists());

    // Errors are results the model reads, not protocol errors.
    let e = c
        .tool("element_detail", json!({ "element": "bus:999" }))
        .unwrap_err();
    assert!(!e.is_empty());
    let raw = c
        .tool("call_command", json!({ "command": "diagnostics" }))
        .unwrap();
    assert!(raw["api_version"].is_u64(), "{raw}");
    let unknown = c.request(
        "tools/call",
        json!({ "name": "no_such_tool", "arguments": {} }),
    );
    assert_eq!(unknown["error"]["code"], -32602, "{unknown}");
    let method = c.request("no/such", json!({}));
    assert_eq!(method["error"]["code"], -32601);
}

#[test]
fn records_topology_and_comparison_read_a_raw_case() {
    let mut c = Client::start(&[], &[]);
    c.tool("open_case", json!({ "sample": "five_bus_devices" }))
        .unwrap();
    let records = c
        .tool(
            "query_records",
            json!({ "section": "bus", "sort": "BASKV", "limit": 2 }),
        )
        .unwrap();
    assert_eq!(records["section"], "BUS");
    assert_eq!(records["columns"][0], "row");
    assert_eq!(records["rows"][0][4], 13.8, "{records}");
    c.tool(
        "edit_records",
        json!({ "edits": [{ "section": "BUS", "row": 0, "field": "NAME", "value": "Renamed" }] }),
    )
    .unwrap();
    let diff = c.tool("compare_cases", json!({})).unwrap();
    assert_eq!(diff["differing_records"], 1, "{diff}");
    let changed = c.tool("diff_records", json!({ "section": "BUS" })).unwrap();
    assert_eq!(changed["matched"], 1, "{changed}");

    let topo = c.tool("topology_summary", json!({})).unwrap();
    assert_eq!(topo["island_count"], 1, "{topo}");
    assert!(
        topo["bridges"]
            .as_array()
            .unwrap()
            .iter()
            .all(|b| b.is_string()),
        "{topo}"
    );
    let path = c
        .tool("shortest_path", json!({ "from": 1, "to": 5 }))
        .unwrap();
    assert_eq!(path["buses"], json!([1, 2, 4, 5]), "{path}");

    let counts = c
        .tool("planning_event_counts", json!({ "categories": ["P1"] }))
        .unwrap();
    assert_eq!(counts["columns"][0], "category");
}

/// A window that answers ui.state and keeps what it was asked to show.
#[derive(Default)]
struct FakeWindow {
    shown: Mutex<Vec<String>>,
}

impl Window for FakeWindow {
    fn ui(&self, command: &str, args: Value) -> Result<Value, stannum_core::Error> {
        match command {
            "ui.state" => {
                Ok(json!({ "module": "power", "selection": self.shown.lock().unwrap().last() }))
            }
            "ui.show" => {
                let element = args["element"].as_str().unwrap_or("").to_string();
                self.shown.lock().unwrap().push(element);
                Ok(json!({ "module": "power" }))
            }
            other => Err(stannum_core::Error::new(
                stannum_core::ErrorKind::UnknownCommand,
                format!("no {other}"),
            )),
        }
    }
    fn changed(&self, _: &str, _: &Value, _: &str) {}
    fn clients(&self, _: usize) {}
}

#[test]
fn attached_it_drives_the_apps_session_and_window() {
    let dir = scratch("attach");
    let session = Arc::new(Session::new());
    let window = Arc::new(FakeWindow::default());
    let _server = Server::start(session.clone(), window.clone(), &dir, "Stannum", "test").unwrap();

    let mut c = Client::start(&["--attach"], &[("STANNUM_ATTACH_DIR", &dir)]);
    let names = c.tool_names();
    assert!(names.iter().any(|n| n == "app_show"), "{names:?}");

    // A case opened over MCP is the app's case.
    c.tool("open_case", json!({ "path": case9() })).unwrap();
    let summary: Value =
        serde_json::from_str(&session.invoke("case_summary", Value::Null).unwrap()).unwrap();
    assert_eq!(summary["name"], "case9.m", "{summary}");
    c.tool("solve_power_flow", json!({})).unwrap();
    // A long run's progress comes over a second connection.
    let run = c.tool("run_contingencies", json!({})).unwrap();
    assert_eq!(run["total"], 12, "{run}");

    c.tool("app_show", json!({ "element": "bus:5" })).unwrap();
    assert_eq!(*window.shown.lock().unwrap(), ["bus:5"]);
    let state = c.tool("app_state", json!({})).unwrap();
    assert_eq!(state["selection"], "bus:5");
    let refused = c
        .tool("app_go", json!({ "module": "diagram" }))
        .unwrap_err();
    assert!(refused.contains("no ui.go"), "{refused}");
}

#[test]
fn attach_without_an_app_says_so() {
    let dir = scratch("no-app");
    let out = Command::new(env!("CARGO_BIN_EXE_stannum-cli"))
        .args(["mcp", "--attach"])
        .env("STANNUM_ATTACH_DIR", &dir)
        .stdin(Stdio::null())
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(2));
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(err.contains("Start the app first"), "{err}");
}
