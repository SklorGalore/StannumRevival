//! Development bridge: serves the Tauri commands over local HTTP so the UI
//! can run in a normal browser (for browser tests and quick UI work).
//!
//! cargo run -p stannum-core --example dev_bridge        # listens on 127.0.0.1:1421
//!
//! POST /invoke/<command> with the command's JSON arguments. The commands
//! are the core's shared list (`stannum_core::commands`), the same ones the
//! desktop app runs; it is not part of the desktop app.
use serde_json::{Value, json};
use stannum_core::Error;
use stannum_core::commands::Session;
use stannum_core::logbook;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::Arc;

fn text_arg(args: &Value, key: &str) -> Result<String, Error> {
    args.get(key)
        .and_then(Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| Error::invalid_arguments(format!("argument {key}: expected a string")))
}

/// Answer one call: the desktop app's own file commands here, and every
/// core command through the shared command list. `stannum_command` is how
/// the app sends core commands; `/invoke/<name>` also works directly.
fn invoke(session: &Session, command: &str, args: Value) -> Result<String, Error> {
    match command {
        "stannum_command" => {
            let name = text_arg(&args, "command")?;
            let inner = args.get("args").cloned().unwrap_or(Value::Null);
            session.invoke(&name, inner)
        }
        "load_diagram" => {
            let path = text_arg(&args, "path")?;
            let text = std::fs::read_to_string(&path).map_err(|e| Error::file(path.as_ref(), e))?;
            Ok(Value::String(text).to_string())
        }
        "save_diagram" => {
            let path = text_arg(&args, "path")?;
            std::fs::write(&path, text_arg(&args, "content")?)
                .map_err(|e| Error::file(path.as_ref(), e))?;
            Ok("null".into())
        }
        _ => session.invoke(command, args),
    }
}

fn serve(session: &Session, mut stream: TcpStream) -> std::io::Result<()> {
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
        match invoke(session, command, args) {
            Ok(v) => ("200 OK", v),
            Err(e) => (
                "500 Internal Server Error",
                serde_json::to_string(&e).unwrap(),
            ),
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
    // Each request runs on its own thread, so cancel and progress calls
    // arrive while a solve or contingency run is going.
    let session = Arc::new(Session::new());
    for stream in listener.incoming() {
        let stream = stream?;
        let session = session.clone();
        std::thread::spawn(move || {
            if let Err(e) = serve(&session, stream) {
                eprintln!("request failed: {e}");
            }
        });
    }
    Ok(())
}
