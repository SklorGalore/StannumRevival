//! `stannum-cli mcp`: Stannum as a Model Context Protocol server.
//!
//! An AI client such as Claude starts `stannum-cli mcp` and talks JSON-RPC
//! to it over stdin and stdout, one message per line (the MCP stdio
//! transport). The server offers Stannum's work as tools: open a case,
//! solve it, read its results a page at a time, run contingencies and the
//! NERC planning events, look at topology, compare cases, export.
//!
//! On its own the server keeps a session of its own, with nothing on
//! screen. With `--attach` it drives the running desktop app instead, so
//! the person at the window sees every case it opens and every result, and
//! a few more tools point the window at things.
//!
//! Only protocol messages go to stdout. Problems go to stderr.

pub mod backend;
mod shape;
mod tools;

use backend::{Backend, Failure};
use serde_json::{Value, json};
use std::io::{BufRead, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};

/// Protocol versions this server speaks, newest first.
const PROTOCOLS: &[&str] = &["2025-11-25", "2025-06-18", "2025-03-26", "2024-11-05"];

const INSTRUCTIONS: &str = "\
Stannum is a power systems workbench: PSS/E RAW (rev 23-35), MATPOWER and \
Stannum (.stn/.stnb) cases, AC/DC power flow, N-1 contingencies and NERC \
TPL-001 planning events, topology, record-level case comparison.

Start with open_case (a file path, or a built-in sample from \
list_sample_cases), then solve_power_flow. Read results with \
find_violations and query_results rather than asking for whole tables: \
real cases have tens of thousands of buses, so every table tool pages, \
filters and sorts in Stannum and returns at most a few hundred rows.

Elements are named by shared ids: bus:5, line:4:5:1, transformer:2:4:1, \
transformer:2:4:6:1 (three-winding), generator:1:1, load:3:1, \
shunt:4:1. Results are named: \"latest\" (the last solve), \"base\", \
\"study\", \"contingency 3\", \"trip line:1:2:1\".

Study edits (set_study_edits) change what every later solve uses without \
touching the case file. Paths are on this computer; use absolute paths.";

/// Writes protocol messages, one per line, from any thread.
#[derive(Clone)]
struct Out(Arc<Mutex<std::io::Stdout>>);

impl Out {
    fn send(&self, message: &Value) {
        let mut out = self.0.lock().unwrap_or_else(|p| p.into_inner());
        let _ = writeln!(out, "{message}");
        let _ = out.flush();
    }

    fn reply(&self, id: &Value, result: Value) {
        self.send(&json!({ "jsonrpc": "2.0", "id": id, "result": result }));
    }

    fn error(&self, id: &Value, code: i64, message: &str) {
        self.send(&json!({
            "jsonrpc": "2.0",
            "id": id,
            "error": { "code": code, "message": message },
        }));
    }
}

/// What a tool needs while it runs.
pub struct Call<'a> {
    pub backend: &'a dyn Backend,
    /// Set when the client cancels the call.
    pub cancelled: &'a AtomicBool,
    /// Sends a progress notification, when the client asked for them.
    pub progress: &'a (dyn Fn(f64, Option<f64>, &str) + Sync),
}

/// The tool call running now, so a cancel can find it.
#[derive(Default)]
struct Running {
    id: Option<Value>,
    cancelled: Arc<AtomicBool>,
}

struct Job {
    id: Value,
    name: String,
    args: Value,
    progress_token: Option<Value>,
    cancelled: Arc<AtomicBool>,
}

/// Serve MCP on stdin and stdout until stdin closes.
pub fn serve(backend: Arc<dyn Backend>) -> Result<(), String> {
    let out = Out(Arc::new(Mutex::new(std::io::stdout())));
    let attached = backend.app().is_some();
    let running = Arc::new(Mutex::new(Running::default()));

    // Tool calls run one at a time on a worker, so the reader stays free to
    // answer pings and cancels while a long contingency run goes on.
    let (jobs, queue) = mpsc::channel::<Job>();
    let worker = {
        let backend = backend.clone();
        let out = out.clone();
        let running = running.clone();
        std::thread::Builder::new()
            .name("mcp-tools".into())
            .spawn(move || {
                for job in queue {
                    {
                        let mut r = running.lock().unwrap_or_else(|p| p.into_inner());
                        r.id = Some(job.id.clone());
                        r.cancelled = job.cancelled.clone();
                    }
                    let progress = |done: f64, total: Option<f64>, message: &str| {
                        if let Some(token) = &job.progress_token {
                            let mut params = json!({
                                "progressToken": token,
                                "progress": done,
                                "message": message,
                            });
                            if let Some(t) = total {
                                params["total"] = json!(t);
                            }
                            out.send(&json!({
                                "jsonrpc": "2.0",
                                "method": "notifications/progress",
                                "params": params,
                            }));
                        }
                    };
                    let call = Call {
                        backend: &*backend,
                        cancelled: &job.cancelled,
                        progress: &progress,
                    };
                    let result = tools::call(&call, &job.name, &job.args, attached);
                    running.lock().unwrap_or_else(|p| p.into_inner()).id = None;
                    match result {
                        Ok(result) => out.reply(&job.id, result),
                        Err(Rpc(code, message)) => out.error(&job.id, code, &message),
                    }
                }
            })
            .map_err(|e| e.to_string())?
    };

    let stdin = std::io::stdin();
    for line in stdin.lock().lines() {
        let line = line.map_err(|e| e.to_string())?;
        if line.trim().is_empty() {
            continue;
        }
        let message: Value = match serde_json::from_str(&line) {
            Ok(m) => m,
            Err(e) => {
                out.error(&Value::Null, -32700, &format!("Parse error: {e}"));
                continue;
            }
        };
        // A batch (2025-03-26 only) is answered message by message.
        let messages = match message {
            Value::Array(items) => items,
            one => vec![one],
        };
        for message in messages {
            handle(&message, &out, &jobs, &running, &*backend, attached);
        }
    }
    drop(jobs);
    let _ = worker.join();
    Ok(())
}

/// A JSON-RPC error: code and message.
pub struct Rpc(pub i64, pub String);

fn handle(
    message: &Value,
    out: &Out,
    jobs: &mpsc::Sender<Job>,
    running: &Mutex<Running>,
    backend: &dyn Backend,
    attached: bool,
) {
    let method = message["method"].as_str().unwrap_or("");
    let params = &message["params"];
    let Some(id) = message.get("id").filter(|id| !id.is_null()) else {
        // A notification, or an answer to a request we never send.
        if method == "notifications/cancelled" {
            let r = running.lock().unwrap_or_else(|p| p.into_inner());
            if r.id.as_ref() == Some(&params["requestId"]) {
                r.cancelled.store(true, Ordering::SeqCst);
                for command in [
                    "cancel_solve",
                    "cancel_contingencies",
                    "cancel_transfer_limits",
                    "cancel_pv_curve",
                    "cancel_run_file",
                ] {
                    let _ = backend.side(command);
                }
            }
        }
        return;
    };
    match method {
        "initialize" => {
            let asked = params["protocolVersion"].as_str().unwrap_or("");
            let version = PROTOCOLS
                .iter()
                .find(|v| **v == asked)
                .unwrap_or(&PROTOCOLS[0]);
            let mut instructions = INSTRUCTIONS.to_string();
            if let Some(app) = backend.app() {
                instructions.push_str(&format!(
                    "\n\nAttached to the running {} {} app (pid {}): the person sees \
                     every case, edit and result. app_* tools point the window at things.",
                    app.app, app.version, app.pid
                ));
            }
            out.reply(
                id,
                json!({
                    "protocolVersion": version,
                    "capabilities": { "tools": { "listChanged": false } },
                    "serverInfo": {
                        "name": "stannum",
                        "title": "Stannum",
                        "version": env!("CARGO_PKG_VERSION"),
                    },
                    "instructions": instructions,
                }),
            );
        }
        "ping" => out.reply(id, json!({})),
        "tools/list" => out.reply(id, json!({ "tools": tools::list(attached) })),
        "tools/call" => {
            let Some(name) = params["name"].as_str() else {
                out.error(id, -32602, "tools/call needs a tool name.");
                return;
            };
            let job = Job {
                id: id.clone(),
                name: name.to_string(),
                args: params
                    .get("arguments")
                    .cloned()
                    .filter(|a| !a.is_null())
                    .unwrap_or_else(|| json!({})),
                progress_token: params["_meta"].get("progressToken").cloned(),
                cancelled: Arc::new(AtomicBool::new(false)),
            };
            if jobs.send(job).is_err() {
                out.error(id, -32603, "The tool runner stopped.");
            }
        }
        // Not offered, but some clients ask anyway.
        "resources/list" => out.reply(id, json!({ "resources": [] })),
        "resources/templates/list" => out.reply(id, json!({ "resourceTemplates": [] })),
        "prompts/list" => out.reply(id, json!({ "prompts": [] })),
        "logging/setLevel" => out.reply(id, json!({})),
        other => out.error(id, -32601, &format!("Method not found: {other}")),
    }
}

/// A failure as the tool result the model reads.
pub fn failure_result(f: &Failure) -> Value {
    json!({
        "content": [{ "type": "text", "text": format!("{} ({})", f.message, f.kind) }],
        "isError": true,
    })
}
