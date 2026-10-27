//! Development bridge: serves the Tauri commands over local HTTP so the UI
//! can run in a normal browser (for browser tests and quick UI work).
//!
//! cargo run -p stannum-core --example dev_bridge        # listens on 127.0.0.1:1421
//!
//! POST /invoke/<command> with the command's JSON arguments. Mirrors
//! src-tauri/src/analysis.rs; it is not part of the desktop app.
use serde_json::{Value, json};
use stannum_core::power::SolveSettings;
use stannum_core::{Workspace, logbook};
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

fn arg<T: serde::de::DeserializeOwned>(args: &Value, key: &str) -> Result<T, String> {
    serde_json::from_value(args.get(key).cloned().unwrap_or(Value::Null))
        .map_err(|e| format!("argument {key}: {e}"))
}

fn to_json<T: serde::Serialize>(v: T) -> Result<Value, String> {
    serde_json::to_value(v).map_err(|e| e.to_string())
}

fn dispatch(ws: &mut Workspace, command: &str, args: &Value) -> Result<Value, String> {
    match command {
        "open_case" => to_json(ws.open(&PathBuf::from(arg::<String>(args, "path")?))?),
        "close_case" => {
            ws.close();
            Ok(Value::Null)
        }
        "case_summary" => to_json(ws.case.as_ref().map(|c| c.summary())),
        "set_study_edits" => {
            let edits: Vec<stannum_core::study::Edit> = arg(args, "edits")?;
            ws.current_mut()?.set_edits(edits)?;
            Ok(Value::Null)
        }
        "load_dc_grids" => to_json(ws.current_mut()?.load_dc_grids(&PathBuf::from(arg::<
            String,
        >(
            args, "path",
        )?))?),
        "clear_dc_grids" => {
            ws.current_mut()?.clear_dc_grids();
            Ok(Value::Null)
        }
        "calculate_sensitivities" => {
            let s: SolveSettings = arg(args, "settings")?;
            to_json(ws.current_mut()?.sensitivities(s)?)
        }
        "sensitivity_responses" => to_json(ws.current()?.responses(
            arg(args, "column")?,
            &arg::<String>(args, "quantity")?,
            arg(args, "engineering")?,
        )?),
        "export_matpower" => to_json(ws.current()?.export_matpower(
            &PathBuf::from(arg::<String>(args, "path")?),
            arg(args, "solved")?,
        )?),
        "element_detail" => to_json(
            ws.current()?
                .element_detail(&arg::<String>(args, "element")?)?,
        ),
        "case_records" => to_json(ws.current_mut()?.record_book()?),
        "record_table" => to_json(
            ws.current_mut()?
                .record_table(&arg::<String>(args, "section")?)?,
        ),
        "apply_record_edits" => {
            let edits: Vec<stannum_core::record_edit::RecordEdit> = arg(args, "edits")?;
            to_json(ws.current_mut()?.apply_record_edits(&edits)?)
        }
        "save_case_as" => to_json(
            ws.current_mut()?
                .save_as(&PathBuf::from(arg::<String>(args, "path")?))?,
        ),
        "diagram_network" => to_json(ws.current()?.diagram_network()?),
        "topology_snapshot" => to_json(ws.current()?.topology_snapshot()?),
        "topology_set_merge" => to_json(
            ws.current_mut()?
                .set_merge_zero_impedance(arg(args, "enabled")?)?,
        ),
        "topology_shortest_path" => to_json(
            ws.current()?
                .shortest_path(arg(args, "from")?, arg(args, "to")?)?,
        ),
        "diff_compare" => to_json(ws.compare(
            &PathBuf::from(arg::<String>(args, "baseline")?),
            &PathBuf::from(arg::<String>(args, "comparison")?),
        )?),
        "diff_compare_working" => to_json(ws.compare_working()?),
        "diff_section_records" => to_json(
            ws.comparison()?
                .section_records(&arg::<String>(args, "section")?),
        ),
        "diff_record_detail" => to_json(
            ws.comparison()?
                .record_detail(&arg::<String>(args, "section")?, arg(args, "index")?),
        ),
        "load_diagram" => to_json(
            std::fs::read_to_string(arg::<String>(args, "path")?).map_err(|e| e.to_string())?,
        ),
        "file_fingerprint" => to_json(stannum_core::study_file::fingerprint(&PathBuf::from(
            arg::<String>(args, "path")?,
        ))?),
        "study_relative_path" => to_json(stannum_core::study_file::relative_path(
            &PathBuf::from(arg::<String>(args, "study")?),
            &PathBuf::from(arg::<String>(args, "case")?),
        )),
        "study_locate_case" => to_json(stannum_core::study_file::locate_case(
            &PathBuf::from(arg::<String>(args, "study")?),
            &arg::<String>(args, "absolute")?,
            args.get("relative").and_then(|v| v.as_str()),
        )),
        "save_diagram" => {
            std::fs::write(
                arg::<String>(args, "path")?,
                arg::<String>(args, "content")?,
            )
            .map_err(|e| e.to_string())?;
            Ok(Value::Null)
        }
        // Runs in the request: the bridge serves one request at a time, so
        // there is nothing to cancel or follow.
        "run_contingencies" => to_json(
            ws.current_mut()?
                .run_contingencies(arg(args, "settings")?)?,
        ),
        "cancel_contingencies" => Ok(Value::Null),
        // Commands run one at a time here, so there is never a solve to stop.
        "contingency_progress" => to_json(stannum_core::contingency::Progress::default()),
        "contingency_summary" => to_json(ws.current()?.contingency_summary()),
        "contingency_rows" => to_json(
            ws.current()?
                .contingency_rows(arg(args, "offset")?, arg(args, "limit")?)?,
        ),
        "contingency_detail" => to_json(ws.current_mut()?.contingency_detail(arg(args, "rank")?)?),
        "trip_and_compare" => to_json(
            ws.current_mut()?
                .trip_and_compare(&arg::<String>(args, "element")?, arg(args, "limitPct")?)?,
        ),
        "result_report" => to_json(ws.current_mut()?.result(&arg::<String>(args, "name")?)?),
        "log_entries" => to_json(logbook::since(arg(args, "since")?)),
        other => Err(format!("unknown command {other}")),
    }
}

/// The open workspace, and the flag that stops the solve in progress. Each
/// request runs on its own thread so "cancel_solve" can arrive mid-solve.
#[derive(Default)]
struct Bridge {
    ws: Mutex<Workspace>,
    solving: Mutex<Option<Arc<AtomicBool>>>,
}

impl Bridge {
    fn invoke(&self, command: &str, args: &Value) -> Result<Value, String> {
        match command {
            "cancel_solve" => {
                if let Some(flag) = self.solving.lock().unwrap().as_ref() {
                    flag.store(true, Ordering::Relaxed);
                }
                Ok(Value::Null)
            }
            "solve_power_flow" => {
                let settings: SolveSettings = arg(args, "settings")?;
                let flag = Arc::new(AtomicBool::new(false));
                *self.solving.lock().unwrap() = Some(flag.clone());
                let mut ws = self.ws.lock().unwrap_or_else(|e| e.into_inner());
                to_json(ws.current_mut()?.solve_cancellable(settings, Some(flag))?)
            }
            _ => dispatch(
                &mut self.ws.lock().unwrap_or_else(|e| e.into_inner()),
                command,
                args,
            ),
        }
    }
}

fn serve(bridge: &Bridge, mut stream: TcpStream) -> std::io::Result<()> {
    let mut reader = BufReader::new(stream.try_clone()?);
    let mut request_line = String::new();
    reader.read_line(&mut request_line)?;
    let mut length = 0usize;
    loop {
        let mut line = String::new();
        if reader.read_line(&mut line)? == 0 || line == "\r\n" {
            break;
        }
        if let Some((k, v)) = line.split_once(':')
            && k.eq_ignore_ascii_case("content-length")
        {
            length = v.trim().parse().unwrap_or(0);
        }
    }
    let mut body = vec![0; length];
    reader.read_exact(&mut body)?;
    let mut parts = request_line.split_whitespace();
    let method = parts.next().unwrap_or_default();
    let path = parts.next().unwrap_or_default();
    let (status, payload) = if method == "OPTIONS" {
        ("204 No Content", String::new())
    } else if let Some(command) = path.strip_prefix("/invoke/") {
        let args: Value = serde_json::from_slice(&body).unwrap_or(json!({}));
        match bridge.invoke(command, &args) {
            Ok(v) => ("200 OK", v.to_string()),
            Err(e) => ("500 Internal Server Error", Value::String(e).to_string()),
        }
    } else {
        ("404 Not Found", String::new())
    };
    let response = format!(
        "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nAccess-Control-Allow-Origin: *\r\nAccess-Control-Allow-Headers: content-type\r\nAccess-Control-Allow-Methods: POST, OPTIONS\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{payload}",
        payload.len()
    );
    stream.write_all(response.as_bytes())
}

fn main() -> std::io::Result<()> {
    let addr = std::env::var("STANNUM_BRIDGE").unwrap_or_else(|_| "127.0.0.1:1421".into());
    logbook::install();
    let listener = TcpListener::bind(&addr)?;
    eprintln!("stannum dev bridge on http://{addr}");
    let bridge = Arc::new(Bridge::default());
    for stream in listener.incoming() {
        let stream = stream?;
        let bridge = bridge.clone();
        std::thread::spawn(move || {
            if let Err(e) = serve(&bridge, stream) {
                eprintln!("request failed: {e}");
            }
        });
    }
    Ok(())
}
