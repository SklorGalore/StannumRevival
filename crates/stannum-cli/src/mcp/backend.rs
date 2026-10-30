//! Where the MCP server's commands go: a session of its own, or the
//! running desktop app over attach.

use serde_json::{Value, json};
use stannum_core::attach::{self, Ticket};
use stannum_core::commands::{API_VERSION, Session};
use std::io::{BufRead, BufReader, Write};
use std::net::TcpStream;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::Duration;

/// A command's failure, as the core reports it.
#[derive(Clone, Debug, PartialEq)]
pub struct Failure {
    pub kind: String,
    pub message: String,
}

impl Failure {
    pub fn new(kind: &str, message: impl Into<String>) -> Self {
        Self {
            kind: kind.into(),
            message: message.into(),
        }
    }

    pub fn invalid(message: impl Into<String>) -> Self {
        Self::new("invalid_arguments", message)
    }
}

impl From<stannum_core::Error> for Failure {
    fn from(e: stannum_core::Error) -> Self {
        let kind = serde_json::to_value(e.kind)
            .ok()
            .and_then(|v| v.as_str().map(str::to_string))
            .unwrap_or_else(|| "internal".into());
        Self {
            kind,
            message: e.message,
        }
    }
}

pub type Answer = Result<Value, Failure>;

/// Which app the server is attached to.
#[derive(Clone, Debug)]
pub struct AppInfo {
    pub app: String,
    pub version: String,
    pub pid: u32,
}

/// Runs registry commands. `call` runs one at a time; `side` runs the
/// commands that stop or follow a long run while `call` waits on it.
pub trait Backend: Send + Sync {
    fn call(&self, command: &str, args: Value) -> Answer;
    fn side(&self, command: &str) -> Answer;
    /// The app, when attached; None for a session of the server's own.
    fn app(&self) -> Option<&AppInfo>;
}

/// A session of the server's own, with nothing on screen.
pub struct Local(Session);

impl Local {
    pub fn new() -> Self {
        Self(Session::new())
    }
}

impl Backend for Local {
    fn call(&self, command: &str, args: Value) -> Answer {
        let text = self.0.invoke(command, args)?;
        serde_json::from_str(&text).map_err(|e| Failure::new("internal", e.to_string()))
    }

    fn side(&self, command: &str) -> Answer {
        // The cancel and progress commands don't wait for the workspace.
        self.call(command, Value::Null)
    }

    fn app(&self) -> Option<&AppInfo> {
        None
    }
}

/// One attach connection: hello done, then a line out and a line back.
struct Line {
    writer: TcpStream,
    reader: BufReader<TcpStream>,
    next: u64,
}

impl Line {
    fn open(ticket: &Ticket) -> Result<Self, String> {
        let stream = TcpStream::connect(("127.0.0.1", ticket.port))
            .map_err(|e| format!("couldn't reach port {}: {e}", ticket.port))?;
        stream.set_nodelay(true).ok();
        let reader = BufReader::new(stream.try_clone().map_err(|e| e.to_string())?);
        let mut line = Line {
            writer: stream,
            reader,
            next: 1,
        };
        let hello = json!({ "hello": {
            "token": ticket.token,
            "client": format!("stannum-cli {} (MCP)", env!("CARGO_PKG_VERSION")),
            "api_version": API_VERSION,
        }});
        line.writer
            .set_read_timeout(Some(Duration::from_secs(5)))
            .ok();
        let answer = line.exchange(&hello).map_err(|f| f.message)?;
        if let Some(error) = answer.get("error") {
            return Err(error["message"].as_str().unwrap_or("refused").to_string());
        }
        line.writer.set_read_timeout(None).ok();
        Ok(line)
    }

    fn exchange(&mut self, message: &Value) -> Answer {
        let lost = |e: std::io::Error| Failure::new("internal", format!("Lost the app: {e}"));
        let mut text = message.to_string();
        text.push('\n');
        self.writer.write_all(text.as_bytes()).map_err(lost)?;
        let mut back = String::new();
        if self.reader.read_line(&mut back).map_err(lost)? == 0 {
            return Err(Failure::new("internal", "The app closed the connection."));
        }
        serde_json::from_str(&back).map_err(|e| Failure::new("internal", e.to_string()))
    }

    fn command(&mut self, command: &str, args: Value) -> Answer {
        let id = self.next;
        self.next += 1;
        let answer = self.exchange(&json!({ "id": id, "command": command, "args": args }))?;
        if let Some(error) = answer.get("error") {
            return Err(Failure::new(
                error["kind"].as_str().unwrap_or("internal"),
                error["message"]
                    .as_str()
                    .unwrap_or("The app refused the command."),
            ));
        }
        Ok(answer.get("ok").cloned().unwrap_or(Value::Null))
    }
}

/// The running desktop app: the person at the window sees what the
/// server's commands do.
pub struct Attached {
    ticket: Ticket,
    info: AppInfo,
    main: Mutex<Line>,
    /// A second connection for cancel and progress, opened when needed.
    side: Mutex<Option<Line>>,
}

impl Attached {
    /// Attach to the newest app (or the one with `pid`) that left a ticket
    /// in `dir` (the usual folder when None).
    pub fn connect(pid: Option<u32>, dir: Option<PathBuf>) -> Result<Self, String> {
        let dir = dir
            .or_else(attach::ticket_dir)
            .ok_or("There is no folder for attach tickets on this system.")?;
        let mut last = None;
        for (_, ticket) in attach::tickets(&dir) {
            if pid.is_some_and(|p| p != ticket.pid) {
                continue;
            }
            match Line::open(&ticket) {
                Ok(line) => {
                    return Ok(Self {
                        info: AppInfo {
                            app: ticket.app.clone(),
                            version: ticket.version.clone(),
                            pid: ticket.pid,
                        },
                        ticket,
                        main: Mutex::new(line),
                        side: Mutex::new(None),
                    });
                }
                Err(e) => last = Some(e),
            }
        }
        let which = pid.map_or("running".to_string(), |p| format!("with pid {p}"));
        Err(match last {
            Some(e) => format!("Found a ticket for a Stannum app {which} but {e}."),
            None => format!(
                "No Stannum app {which} to attach to (looked for tickets in {}). Start the app first.",
                dir.display()
            ),
        })
    }
}

impl Backend for Attached {
    fn call(&self, command: &str, args: Value) -> Answer {
        let mut line = self.main.lock().unwrap_or_else(|p| p.into_inner());
        line.command(command, args)
    }

    fn side(&self, command: &str) -> Answer {
        let mut side = self.side.lock().unwrap_or_else(|p| p.into_inner());
        if side.is_none() {
            *side = Some(Line::open(&self.ticket).map_err(|e| Failure::new("internal", e))?);
        }
        let answer = side.as_mut().map(|l| l.command(command, Value::Null));
        if answer.as_ref().is_some_and(|a| a.is_err()) {
            *side = None;
        }
        answer.unwrap_or(Ok(Value::Null))
    }

    fn app(&self) -> Option<&AppInfo> {
        Some(&self.info)
    }
}
