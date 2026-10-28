//! Attach: a script drives the running app.
//!
//! The desktop app listens on a local TCP port (127.0.0.1 only) and writes
//! a *ticket* naming the port and a random token into a folder only this
//! user can read (see [`ticket_dir`]). `stannum.attach()` in Python reads
//! the newest ticket, connects and says hello with the token. From then on
//! it sends the same registry commands the window sends, on the window's
//! own session, so a script and the person at the window share one case,
//! one study and one set of results.
//!
//! Commands named `ui.*` go to the window instead (show an element, switch
//! module, run a Ctrl+K command); see [`Window::ui`].
//!
//! The protocol is one JSON object per line, both ways:
//!
//! ```text
//! -> {"hello": {"token": "…", "client": "stannum 0.3.0", "api_version": 3}}
//! <- {"ok": {"app": "Stannum", "version": "0.3.0", "api_version": 3, "protocol": 1, "pid": 4242}}
//! -> {"id": 1, "command": "solve_power_flow", "args": {}}
//! <- {"id": 1, "ok": {…the command's answer…}}
//! -> {"id": 2, "command": "ui.show", "args": {"element": "bus:5"}}
//! <- {"id": 2, "error": {"kind": "not_found", "message": "…"}}
//! ```
//!
//! A connection answers its requests in order. A client stops long work by
//! sending `cancel_solve` (or another cancel command) on a second
//! connection.

use crate::commands::{API_VERSION, Session};
use crate::error::Error;
use crate::run_file;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::io::{self, BufRead, BufReader, Read, Write};
use std::net::{Ipv4Addr, Shutdown, SocketAddr, TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// Version of the attach protocol itself (hello, framing). The commands
/// carry their own [`API_VERSION`].
pub const PROTOCOL: u32 = 1;

/// Longest hello line accepted before the token is checked.
const HELLO_LIMIT: u64 = 4096;

/// How long a new connection has to say hello.
const HELLO_TIMEOUT: Duration = Duration::from_secs(5);

/// The window's side of attach: the app implements it.
pub trait Window: Send + Sync {
    /// Answer a `ui.*` command (the full name, such as `ui.show`).
    fn ui(&self, command: &str, args: Value) -> Result<Value, Error>;
    /// A script's command changed the case, the study or its results; the
    /// window catches up. `answer` is the command's answer as JSON text.
    fn changed(&self, command: &str, args: &Value, answer: &str);
    /// How many scripts are attached now.
    fn clients(&self, count: usize);
}

/// What a ticket file says: where the app listens and the token to show.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Ticket {
    pub app: String,
    pub version: String,
    pub api_version: u32,
    pub protocol: u32,
    pub pid: u32,
    pub port: u16,
    pub token: String,
    /// When the app started listening, in ms since the epoch. A client
    /// picks the newest app.
    pub started: u64,
}

/// The folder tickets go in. Python looks in the same place:
///
/// - `STANNUM_ATTACH_DIR` when set;
/// - Windows: `%LOCALAPPDATA%\Stannum\attach`;
/// - macOS: `~/Library/Application Support/Stannum/attach`;
/// - elsewhere: `$XDG_RUNTIME_DIR/stannum/attach`, else
///   `~/.local/state/stannum/attach`.
pub fn ticket_dir() -> Option<PathBuf> {
    let env = |k: &str| {
        std::env::var_os(k)
            .filter(|v| !v.is_empty())
            .map(PathBuf::from)
    };
    if let Some(dir) = env("STANNUM_ATTACH_DIR") {
        return Some(dir);
    }
    if cfg!(windows) {
        return env("LOCALAPPDATA").map(|d| d.join("Stannum").join("attach"));
    }
    let home = env("HOME");
    if cfg!(target_os = "macos") {
        return home.map(|h| h.join("Library/Application Support/Stannum/attach"));
    }
    if let Some(run) = env("XDG_RUNTIME_DIR") {
        return Some(run.join("stannum").join("attach"));
    }
    home.map(|h| h.join(".local/state/stannum/attach"))
}

/// A token nobody else can guess: 32 random bytes as hex.
fn new_token() -> io::Result<String> {
    let mut bytes = [0u8; 32];
    getrandom::fill(&mut bytes).map_err(|e| io::Error::other(e.to_string()))?;
    Ok(bytes.iter().map(|b| format!("{b:02x}")).collect())
}

/// Compare without stopping at the first difference.
fn same_token(a: &str, b: &str) -> bool {
    let (a, b) = (a.as_bytes(), b.as_bytes());
    a.len() == b.len() && a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

/// Make the folder, readable by this user only where the OS supports it.
fn make_private_dir(dir: &Path) -> io::Result<()> {
    std::fs::create_dir_all(dir)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o700))?;
    }
    Ok(())
}

/// Write the ticket so it appears whole, readable by this user only.
fn write_ticket(path: &Path, ticket: &Ticket) -> io::Result<()> {
    let text = serde_json::to_string_pretty(ticket).map_err(io::Error::other)?;
    let partial = path.with_extension("json.part");
    {
        let mut options = std::fs::OpenOptions::new();
        options.write(true).create(true).truncate(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut file = options.open(&partial)?;
        file.write_all(text.as_bytes())?;
    }
    std::fs::rename(&partial, path)
}

/// The app's attach listener. Dropping it stops listening and removes the
/// ticket.
pub struct Server {
    addr: SocketAddr,
    ticket: Ticket,
    ticket_path: PathBuf,
    stop: Arc<AtomicBool>,
}

/// What every connection shares.
struct Shared {
    session: Arc<Session>,
    window: Arc<dyn Window>,
    ticket: Ticket,
    clients: AtomicUsize,
    stop: Arc<AtomicBool>,
    /// Connections still open, so stopping can close them.
    open: Mutex<Vec<TcpStream>>,
}

impl Server {
    /// Listen on a free local port and write a ticket into `dir` (usually
    /// [`ticket_dir`]). `app` and `version` name the app in the hello.
    pub fn start(
        session: Arc<Session>,
        window: Arc<dyn Window>,
        dir: &Path,
        app: &str,
        version: &str,
    ) -> io::Result<Server> {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0))?;
        let addr = listener.local_addr()?;
        let pid = std::process::id();
        let ticket = Ticket {
            app: app.to_string(),
            version: version.to_string(),
            api_version: API_VERSION,
            protocol: PROTOCOL,
            pid,
            port: addr.port(),
            token: new_token()?,
            started: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_millis() as u64)
                .unwrap_or_default(),
        };
        make_private_dir(dir)?;
        // The port too, so two listeners in one process keep their own tickets.
        let ticket_path = dir.join(format!("{pid}-{}.json", addr.port()));
        write_ticket(&ticket_path, &ticket)?;
        let stop = Arc::new(AtomicBool::new(false));
        let shared = Arc::new(Shared {
            session,
            window,
            ticket: ticket.clone(),
            clients: AtomicUsize::new(0),
            stop: stop.clone(),
            open: Mutex::new(Vec::new()),
        });
        std::thread::Builder::new()
            .name("stannum-attach".into())
            .spawn(move || accept(listener, shared))?;
        log::info!(target: "attach", "Scripts can attach on port {}.", addr.port());
        Ok(Server {
            addr,
            ticket,
            ticket_path,
            stop,
        })
    }

    pub fn port(&self) -> u16 {
        self.addr.port()
    }

    pub fn ticket(&self) -> &Ticket {
        &self.ticket
    }

    pub fn ticket_path(&self) -> &Path {
        &self.ticket_path
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        // Wake the accept loop so it sees the flag.
        let _ = TcpStream::connect_timeout(&self.addr, Duration::from_millis(200));
        let _ = std::fs::remove_file(&self.ticket_path);
    }
}

fn accept(listener: TcpListener, shared: Arc<Shared>) {
    for stream in listener.incoming() {
        if shared.stop.load(Ordering::SeqCst) {
            break;
        }
        let Ok(stream) = stream else { continue };
        let shared = shared.clone();
        let _ = std::thread::Builder::new()
            .name("stannum-attach-client".into())
            .spawn(move || {
                if let Err(e) = serve(&shared, stream) {
                    log::debug!(target: "attach", "A script's connection ended: {e}");
                }
            });
    }
    // Close the connections still open.
    if let Ok(open) = shared.open.lock() {
        for s in open.iter() {
            let _ = s.shutdown(Shutdown::Both);
        }
    }
}

/// One line of a request after the hello.
#[derive(Deserialize)]
struct Request {
    id: Value,
    command: String,
    #[serde(default)]
    args: Value,
}

#[derive(Deserialize)]
struct Hello {
    hello: HelloBody,
}

#[derive(Deserialize)]
struct HelloBody {
    token: String,
    #[serde(default)]
    client: Option<String>,
}

fn send(stream: &mut TcpStream, line: &str) -> io::Result<()> {
    stream.write_all(line.as_bytes())?;
    stream.write_all(b"\n")?;
    stream.flush()
}

fn error_line(id: Option<&Value>, e: &Error) -> String {
    match id {
        Some(id) => json!({ "id": id, "error": e }).to_string(),
        None => json!({ "error": e }).to_string(),
    }
}

fn serve(shared: &Shared, stream: TcpStream) -> io::Result<()> {
    let mut out = stream.try_clone()?;
    let me = stream.peer_addr().ok();
    stream.set_read_timeout(Some(HELLO_TIMEOUT))?;
    let mut reader = BufReader::new(stream.try_clone()?);

    // Hello first, read with a limit, before anything else is trusted.
    let mut line = String::new();
    (&mut reader).take(HELLO_LIMIT).read_line(&mut line)?;
    let hello: Option<Hello> = serde_json::from_str(line.trim()).ok();
    let Some(hello) = hello.filter(|h| same_token(&h.hello.token, &shared.ticket.token)) else {
        let e = Error::invalid_arguments("Say hello with the token from the app's ticket first.");
        let _ = send(&mut out, &error_line(None, &e));
        return Ok(());
    };
    stream.set_read_timeout(None)?;
    let t = &shared.ticket;
    let welcome = json!({ "ok": {
        "app": t.app, "version": t.version, "api_version": t.api_version,
        "protocol": t.protocol, "pid": t.pid,
    }});
    // Count the client before welcoming it, so that once the client hears
    // back, the window already shows it.
    if let Ok(mut open) = shared.open.lock() {
        open.push(stream.try_clone()?);
    }
    let count = shared.clients.fetch_add(1, Ordering::SeqCst) + 1;
    shared.window.clients(count);
    let client = hello.hello.client.unwrap_or_else(|| "a script".into());
    log::info!(target: "attach", "{client} attached.");
    if let Err(e) = send(&mut out, &welcome.to_string()) {
        let count = shared.clients.fetch_sub(1, Ordering::SeqCst) - 1;
        shared.window.clients(count);
        if let Ok(mut open) = shared.open.lock() {
            open.retain(|s| s.peer_addr().ok() != me);
        }
        return Err(e);
    }
    let result = requests(shared, &mut reader, &mut out);
    let count = shared.clients.fetch_sub(1, Ordering::SeqCst) - 1;
    shared.window.clients(count);
    if let Ok(mut open) = shared.open.lock() {
        open.retain(|s| s.peer_addr().ok() != me);
    }
    log::info!(target: "attach", "{client} detached.");
    result
}

fn requests(shared: &Shared, reader: &mut impl BufRead, out: &mut TcpStream) -> io::Result<()> {
    let mut line = String::new();
    loop {
        line.clear();
        if reader.read_line(&mut line)? == 0 || shared.stop.load(Ordering::SeqCst) {
            return Ok(());
        }
        if line.trim().is_empty() {
            continue;
        }
        let request: Request = match serde_json::from_str(line.trim()) {
            Ok(r) => r,
            Err(e) => {
                let e = Error::invalid_arguments(format!("Not a request: {e}"));
                send(out, &error_line(None, &e))?;
                continue;
            }
        };
        let reply = match answer(shared, &request.command, request.args) {
            // A command's answer is compact JSON text: no raw line breaks.
            Ok(text) => format!("{{\"id\":{},\"ok\":{text}}}", request.id),
            Err(e) => error_line(Some(&request.id), &e),
        };
        send(out, &reply)?;
    }
}

/// Run one command for a script: `ui.*` in the window, anything else in
/// the shared session, telling the window when it changed something.
fn answer(shared: &Shared, command: &str, args: Value) -> Result<String, Error> {
    if command.starts_with("ui.") {
        return shared.window.ui(command, args).map(|v| v.to_string());
    }
    let answer = shared.session.invoke(command, args.clone())?;
    if run_file::is_action(command) || command == "run_file" {
        shared.window.changed(command, &args, &answer);
    }
    Ok(answer)
}

/// Read every ticket in `dir`, newest first.
pub fn tickets(dir: &Path) -> Vec<(PathBuf, Ticket)> {
    let mut found: Vec<(PathBuf, Ticket)> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "json"))
        .filter_map(|p| {
            let text = std::fs::read_to_string(&p).ok()?;
            Some((p, serde_json::from_str(&text).ok()?))
        })
        .collect();
    found.sort_by_key(|t| std::cmp::Reverse(t.1.started));
    found
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::BufReader;

    /// A window that answers `ui.echo` and keeps what changed.
    #[derive(Default)]
    struct FakeWindow {
        changed: Mutex<Vec<String>>,
        clients: AtomicUsize,
    }

    impl Window for FakeWindow {
        fn ui(&self, command: &str, args: Value) -> Result<Value, Error> {
            match command {
                "ui.echo" => Ok(args),
                _ => Err(Error::new(
                    crate::ErrorKind::UnknownCommand,
                    format!("No window command {command}."),
                )),
            }
        }
        fn changed(&self, command: &str, _args: &Value, _answer: &str) {
            self.changed.lock().unwrap().push(command.to_string());
        }
        fn clients(&self, count: usize) {
            self.clients.store(count, Ordering::SeqCst);
        }
    }

    fn temp_dir(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("stannum-attach-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    struct Client {
        reader: BufReader<TcpStream>,
        out: TcpStream,
    }

    impl Client {
        fn connect(ticket: &Ticket, token: &str) -> (Client, Value) {
            let out = TcpStream::connect(("127.0.0.1", ticket.port)).unwrap();
            let reader = BufReader::new(out.try_clone().unwrap());
            let mut c = Client { reader, out };
            let hello = c.round(&json!({ "hello": { "token": token, "client": "test" } }));
            (c, hello)
        }
        fn round(&mut self, msg: &Value) -> Value {
            send(&mut self.out, &msg.to_string()).unwrap();
            let mut line = String::new();
            self.reader.read_line(&mut line).unwrap();
            serde_json::from_str(&line).unwrap()
        }
        fn call(&mut self, id: u64, command: &str, args: Value) -> Value {
            self.round(&json!({ "id": id, "command": command, "args": args }))
        }
    }

    fn case9() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/case9.m")
    }

    #[test]
    fn a_script_shares_the_window_session() {
        let dir = temp_dir("share");
        let session = Arc::new(Session::new());
        let window = Arc::new(FakeWindow::default());
        let server =
            Server::start(session.clone(), window.clone(), &dir, "Stannum", "test").unwrap();

        // The ticket is where a client looks, and names the server.
        let found = tickets(&dir);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].1, *server.ticket());
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(server.ticket_path())
                .unwrap()
                .permissions()
                .mode();
            assert_eq!(mode & 0o077, 0, "ticket readable by others");
        }

        let (mut c, hello) = Client::connect(server.ticket(), &server.ticket().token);
        assert_eq!(hello["ok"]["api_version"], API_VERSION);
        // The server counts the client just after sending the welcome, so
        // give its thread a moment.
        for _ in 0..50 {
            if window.clients.load(Ordering::SeqCst) == 1 {
                break;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        assert_eq!(window.clients.load(Ordering::SeqCst), 1);

        let opened = c.call(1, "open_case", json!({ "path": case9() }));
        assert_eq!(opened["id"], 1);
        assert!(opened["ok"]["id"].is_number(), "{opened}");
        // The window's own session sees the case the script opened.
        let summary: Value =
            serde_json::from_str(&session.invoke("case_summary", Value::Null).unwrap()).unwrap();
        assert_eq!(summary["id"], opened["ok"]["id"]);

        let solved = c.call(2, "solve_power_flow", json!({}));
        assert_eq!(solved["ok"]["converged"], true, "{solved}");
        // Reads don't tell the window; actions do.
        c.call(3, "case_summary", json!({}));
        assert_eq!(
            *window.changed.lock().unwrap(),
            ["open_case", "solve_power_flow"]
        );

        // Errors keep their kind; ui.* reaches the window.
        let bad = c.call(4, "element_detail", json!({ "element": "bus:999" }));
        assert_eq!(bad["id"], 4);
        assert!(bad["error"]["kind"].is_string(), "{bad}");
        let echo = c.call(5, "ui.echo", json!({ "x": 1 }));
        assert_eq!(echo["ok"], json!({ "x": 1 }));
        let missing = c.call(6, "ui.nope", json!({}));
        assert_eq!(missing["error"]["kind"], "unknown_command");

        drop(c);
        // The count drops once the connection's thread sees it close.
        for _ in 0..50 {
            if window.clients.load(Ordering::SeqCst) == 0 {
                break;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        assert_eq!(window.clients.load(Ordering::SeqCst), 0);

        let path = server.ticket_path().to_path_buf();
        drop(server);
        assert!(!path.exists(), "ticket left behind");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_wrong_token_is_turned_away() {
        let dir = temp_dir("token");
        let window = Arc::new(FakeWindow::default());
        let server = Server::start(
            Arc::new(Session::new()),
            window.clone(),
            &dir,
            "Stannum",
            "test",
        )
        .unwrap();
        let (mut c, hello) = Client::connect(server.ticket(), "not-the-token");
        assert_eq!(hello["error"]["kind"], "invalid_arguments");
        // The server hung up: nothing more is answered.
        send(
            &mut c.out,
            &json!({ "id": 1, "command": "case_summary" }).to_string(),
        )
        .ok();
        let mut line = String::new();
        assert_eq!(c.reader.read_line(&mut line).unwrap_or(0), 0);
        assert_eq!(window.clients.load(Ordering::SeqCst), 0);
        drop(server);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn tokens_are_long_and_differ() {
        let a = new_token().unwrap();
        let b = new_token().unwrap();
        assert_eq!(a.len(), 64);
        assert_ne!(a, b);
        assert!(same_token(&a, &a.clone()));
        assert!(!same_token(&a, &b));
    }
}
