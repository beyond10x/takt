//! A smart-HTTP git stand-in on 127.0.0.1, shared by the test files that reach a loopback git
//! relay: the bare repositories under its root, served by `git http-backend`. It logs every
//! request it is sent. With a redirect, every receive-pack request (the push) is answered with a
//! 302 to the same path under that base. With a refusal, a push the refusal names a reason for is
//! answered as a Connectors git relay answers one: a report-status that refuses every command
//! with that reason (`ng <ref> <reason>`), which git shows as `! [remote rejected] <ref>
//! (<reason>)`, and nothing is applied.

use std::io::{BufRead, BufReader, Write as _};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};

/// One request a stand-in was sent: its method, its target and its headers, each name in lower
/// case.
#[derive(Debug, Clone)]
pub struct Request {
    pub method: String,
    pub target: String,
    pub headers: Vec<(String, String)>,
}

impl Request {
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(key, _)| key == name)
            .map(|(_, value)| value.as_str())
    }
}

/// Reads one request's line and headers from `reader`; none at the end of the stream.
pub fn read_head(reader: &mut impl BufRead) -> Option<Request> {
    let mut line = String::new();
    if reader.read_line(&mut line).ok()? == 0 {
        return None;
    }
    let mut parts = line.split_whitespace();
    let method = parts.next()?.to_owned();
    let target = parts.next()?.to_owned();
    let mut headers = Vec::new();
    loop {
        let mut header = String::new();
        reader.read_line(&mut header).ok()?;
        let header = header.trim_end();
        if header.is_empty() {
            break;
        }
        if let Some((name, value)) = header.split_once(':') {
            headers.push((name.trim().to_ascii_lowercase(), value.trim().to_owned()));
        }
    }
    Some(Request {
        method,
        target,
        headers,
    })
}

/// Reads the body `request` announces, by `Content-Length` or in chunks.
pub fn read_body(reader: &mut impl BufRead, request: &Request) -> std::io::Result<Vec<u8>> {
    let mut body = Vec::new();
    if let Some(length) = request.header("content-length") {
        body.resize(length.parse().unwrap_or(0), 0);
        reader.read_exact(&mut body)?;
    } else if request
        .header("transfer-encoding")
        .is_some_and(|value| value.eq_ignore_ascii_case("chunked"))
    {
        loop {
            let mut size = String::new();
            reader.read_line(&mut size)?;
            let size =
                usize::from_str_radix(size.trim().split(';').next().unwrap_or(""), 16).unwrap_or(0);
            if size == 0 {
                let mut trailer = String::new();
                while reader.read_line(&mut trailer)? > 0 && !trailer.trim().is_empty() {
                    trailer.clear();
                }
                break;
            }
            let start = body.len();
            body.resize(start + size, 0);
            reader.read_exact(&mut body[start..])?;
            let mut end = [0; 2];
            reader.read_exact(&mut end)?;
        }
    }
    Ok(body)
}

/// What a refusing stand-in asks of each command of a push: given its ref and the commit it
/// would set the ref to, the reason it is refused, or none.
pub type Refusal = Arc<dyn Fn(&str, &str) -> Option<String> + Send + Sync>;

/// How the stand-in answers a push.
#[derive(Clone, Default)]
pub enum Push {
    /// Through `git http-backend`.
    #[default]
    Served,
    /// With a 302 to the same path under this base.
    Redirected(String),
    /// Refused, every command with the first reason this names for any command of the push, as a
    /// Connectors git relay refuses one; served when it names none.
    Refused(Refusal),
    /// Answered with this HTTP status and an empty body, nothing applied.
    Status(u16),
}

/// What the stand-in asks of every request before it answers it as [`Push`] says.
#[derive(Clone, Copy, Default)]
pub enum Before {
    /// Nothing.
    #[default]
    Nothing,
    /// A request with no `Authorization` header is answered with a 401 and a Basic challenge.
    Challenge,
    /// A fetch (any request to `git-upload-pack`) is answered with this HTTP status.
    RefuseFetch(u16),
}

/// One command of a push: the commit it sets its ref to, and the ref.
struct Update {
    new: String,
    name: String,
}

/// The commands of a receive-pack request `body` and the capabilities its first one names.
fn commands(body: &[u8]) -> (Vec<Update>, Vec<String>) {
    let mut updates = Vec::new();
    let mut capabilities = Vec::new();
    let mut at = 0;
    while at + 4 <= body.len() {
        let Some(length) = std::str::from_utf8(&body[at..at + 4])
            .ok()
            .and_then(|hex| usize::from_str_radix(hex, 16).ok())
        else {
            break;
        };
        if length < 4 || at + length > body.len() {
            break;
        }
        let line = &body[at + 4..at + length];
        at += length;
        let (command, caps) = match line.iter().position(|byte| *byte == 0) {
            Some(nul) => (&line[..nul], Some(&line[nul + 1..])),
            None => (line, None),
        };
        if let Some(caps) = caps {
            capabilities = String::from_utf8_lossy(caps)
                .split_whitespace()
                .map(str::to_owned)
                .collect();
        }
        let command = String::from_utf8_lossy(command);
        let mut words = command.split_whitespace();
        if let (Some(_old), Some(new), Some(name)) = (words.next(), words.next(), words.next()) {
            updates.push(Update {
                new: new.to_owned(),
                name: name.to_owned(),
            });
        }
    }
    (updates, capabilities)
}

/// One pkt-line holding `data`.
fn pkt(data: &[u8]) -> Vec<u8> {
    let mut line = format!("{:04x}", data.len() + 4).into_bytes();
    line.extend_from_slice(data);
    line
}

/// The report-status refusing every command of `updates` with `reason`, in side-band 1 when the
/// client asked for a side band.
fn refusal_report(updates: &[Update], capabilities: &[String], reason: &str) -> Vec<u8> {
    let mut inner = pkt(b"unpack ok\n");
    for update in updates {
        inner.extend(pkt(format!("ng {} {reason}\n", update.name).as_bytes()));
    }
    inner.extend_from_slice(b"0000");
    let banded = |size: usize| {
        let mut out = Vec::new();
        for chunk in inner.chunks(size) {
            let mut data = vec![1_u8];
            data.extend_from_slice(chunk);
            out.extend(pkt(&data));
        }
        out.extend_from_slice(b"0000");
        out
    };
    if capabilities.iter().any(|cap| cap == "side-band-64k") {
        banded(65515)
    } else if capabilities.iter().any(|cap| cap == "side-band") {
        banded(995)
    } else {
        inner
    }
}

/// A smart-HTTP stand-in on 127.0.0.1 (the module's header says how it answers).
pub struct GitStandIn {
    pub port: u16,
    log: Arc<Mutex<Vec<Request>>>,
}

impl GitStandIn {
    /// Serves the bare repositories under `root`; with a redirect, a push is answered with a
    /// 302 to the same path under that base.
    pub fn serve(root: PathBuf, redirect: Option<String>) -> Self {
        Self::serve_with(root, redirect.map_or(Push::Served, Push::Redirected))
    }

    /// Serves the bare repositories under `root`, a push answered as `push` says.
    pub fn serve_with(root: PathBuf, push: Push) -> Self {
        Self::serve_before(root, push, Before::Nothing)
    }

    /// [`Self::serve_with`], every request first asked as `before` says.
    pub fn serve_before(root: PathBuf, push: Push, before: Before) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("a loopback port");
        let port = listener.local_addr().expect("an address").port();
        let log = Arc::new(Mutex::new(Vec::new()));
        let seen = Arc::clone(&log);
        std::thread::spawn(move || {
            for stream in listener.incoming().flatten() {
                let _ = Self::answer(stream, &root, &push, before, &seen);
            }
        });
        Self { port, log }
    }

    fn answer(
        stream: TcpStream,
        root: &Path,
        push: &Push,
        before: Before,
        log: &Mutex<Vec<Request>>,
    ) -> std::io::Result<()> {
        let mut reader = BufReader::new(stream.try_clone()?);
        let Some(request) = read_head(&mut reader) else {
            return Ok(());
        };
        let body = read_body(&mut reader, &request)?;
        log.lock().expect("the log").push(request.clone());
        let mut out = stream;
        let receiving = request.target.contains("git-receive-pack");
        match before {
            Before::Challenge if request.header("authorization").is_none() => {
                return write!(
                    out,
                    "HTTP/1.1 401 Unauthorized\r\nWWW-Authenticate: Basic realm=\"relay\"\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
                );
            }
            Before::RefuseFetch(status) if request.target.contains("git-upload-pack") => {
                return write!(
                    out,
                    "HTTP/1.1 {status} Refused\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
                );
            }
            _ => {}
        }
        match push {
            Push::Status(status) if receiving && request.method == "POST" => {
                return write!(
                    out,
                    "HTTP/1.1 {status} Refused\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
                );
            }
            Push::Redirected(base) if receiving => {
                return write!(
                    out,
                    "HTTP/1.1 302 Found\r\nLocation: {base}{}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
                    request.target
                );
            }
            Push::Refused(refusal) if receiving && request.method == "POST" => {
                let (updates, capabilities) = commands(&body);
                if let Some(reason) = updates
                    .iter()
                    .find_map(|update| refusal(&update.name, &update.new))
                {
                    let report = refusal_report(&updates, &capabilities, &reason);
                    write!(
                        out,
                        "HTTP/1.1 200 OK\r\nContent-Type: application/x-git-receive-pack-result\r\nCache-Control: no-cache\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                        report.len()
                    )?;
                    out.write_all(&report)?;
                    return out.flush();
                }
            }
            _ => {}
        }
        let (path, query) = request
            .target
            .split_once('?')
            .unwrap_or((request.target.as_str(), ""));
        let mut command = Command::new("git");
        command
            .arg("http-backend")
            .env_clear()
            .env("PATH", std::env::var_os("PATH").unwrap_or_default())
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .env("GIT_PROJECT_ROOT", root)
            .env("GIT_HTTP_EXPORT_ALL", "1")
            .env("REQUEST_METHOD", &request.method)
            .env("PATH_INFO", path)
            .env("QUERY_STRING", query)
            .env("CONTENT_LENGTH", body.len().to_string())
            .env("REMOTE_ADDR", "127.0.0.1")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());
        for (header, variable) in [
            ("content-type", "CONTENT_TYPE"),
            ("content-encoding", "HTTP_CONTENT_ENCODING"),
            ("git-protocol", "HTTP_GIT_PROTOCOL"),
        ] {
            if let Some(value) = request.header(header) {
                command.env(variable, value);
            }
        }
        let mut child = command.spawn()?;
        let mut stdin = child.stdin.take().expect("a stdin");
        let writer = std::thread::spawn(move || {
            let _ = stdin.write_all(&body);
        });
        let output = child.wait_with_output()?;
        let _ = writer.join();
        let cgi = output.stdout;
        let (head, payload) = match cgi.windows(4).position(|window| window == b"\r\n\r\n") {
            Some(at) => (&cgi[..at], &cgi[at + 4..]),
            None => match cgi.windows(2).position(|window| window == b"\n\n") {
                Some(at) => (&cgi[..at], &cgi[at + 2..]),
                None => (&cgi[..0], &cgi[..]),
            },
        };
        let head = String::from_utf8_lossy(head);
        let mut status = "200 OK".to_owned();
        let mut headers = String::new();
        for line in head.lines() {
            match line.split_once(':') {
                Some((name, value)) if name.eq_ignore_ascii_case("status") => {
                    status = value.trim().to_owned();
                }
                Some(_) => {
                    headers.push_str(line.trim_end());
                    headers.push_str("\r\n");
                }
                None => {}
            }
        }
        write!(
            out,
            "HTTP/1.1 {status}\r\n{headers}Content-Length: {}\r\nConnection: close\r\n\r\n",
            payload.len()
        )?;
        out.write_all(payload)?;
        out.flush()
    }

    /// Every request it was sent, in order.
    pub fn requests(&self) -> Vec<Request> {
        self.log.lock().expect("the log").clone()
    }

    /// How many pushes it was sent: `POST`s to a `git-receive-pack`.
    pub fn pushes(&self) -> usize {
        self.requests()
            .iter()
            .filter(|request| {
                request.method == "POST" && request.target.ends_with("/git-receive-pack")
            })
            .count()
    }

    /// `http://127.0.0.1:<port>/<path>`.
    pub fn url(&self, path: &str) -> String {
        format!("http://127.0.0.1:{}/{path}", self.port)
    }
}
