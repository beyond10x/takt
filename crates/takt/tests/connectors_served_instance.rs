//! story:connectors-forge-at-a-served-instance, acceptance 1 and 3: `forge: connectors` with
//! `forge_instance: github` and `forge_endpoint` opens, reads and merges a pull request through a
//! real served Connectors instance, connectors 0.62.0's `connectors-catalog-provider --config`,
//! not a stand-in of its wire.
//!
//! What runs, all on 127.0.0.1:
//! - a fixture of GitHub's REST API over HTTPS, its certificate made here and given to the
//!   instance as its only root (`SSL_CERT_FILE`); it records every request it answers;
//! - the instance, from the release asset CI installs and checks by sha256, serving the `github`
//!   definition of connectors-definitions 0.6.0 under the profile `github.token` with its
//!   endpoint moved to the fixture, writes admitted under a standing grant file
//!   (`service.standing_grants`) that covers `owner/repo`. Its credential document is a mounted
//!   file it reads under the `mounted_secret` checks, so it runs in `bwrap`: its own user and
//!   mount namespaces, the credential directory bound read-only, the host's network;
//! - the loopback boundary a pod reaches the instance through: it adds the service bearer to
//!   every request, since takt sends none, and records each invocation and its answer. For
//!   acceptance 3 it drops the instance's first answer to a write, as a lost answer.
//!
//! The run's push goes through a double of the forge's runner, which moves the fixture's branch.
//!
//! Inputs: `TAKT_TEST_CONNECTORS_BIN` names the release's `connectors-catalog-provider`, and
//! `TAKT_TEST_CONNECTORS_DEFINITIONS` the root of connectors-definitions' release archive. Without
//! them, or without `bwrap`, a test prints why and passes when `CI` is not `true`, and fails when
//! it is, so CI never passes it unrun. Linux only: the custody binding the instance reads its
//! credential through exists on Linux alone.
#![cfg(target_os = "linux")]

#[path = "support/own_home.rs"]
mod own_home;

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::{BufRead, BufReader, Read, Write as _};
use std::net::{Shutdown, SocketAddr, TcpListener, TcpStream};
use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, mpsc};
use std::time::{Duration, Instant};

use anyhow::anyhow;
use serde_json::{Value, json};
use takt::case::Authority;
use takt::evidence::Record;
use takt::forge::connectors::ConnectorsForge;
use takt::forge::{Exit, Forge, Runner};
use takt::harness::{Harness, StepContext, StepReport};
use takt::run_config::RunConfig;
use takt::scan::Scan;
use takt::validator::{CiRead, PrMerged, PrOpen, Validator};
use takt::{Run, RunOptions, Stop};

/// The environment variable naming the served instance's binary.
const BIN_ENV: &str = "TAKT_TEST_CONNECTORS_BIN";

/// The environment variable naming the root of the definitions the instance lowers.
const DEFINITIONS_ENV: &str = "TAKT_TEST_CONNECTORS_DEFINITIONS";

/// The Connectors release these tests run.
const VERSION: &str = "0.62.0";

/// The instance's name; its id is `github.<name>`.
const NAME: &str = "fixture";
const INSTANCE_ID: &str = "github.fixture";

/// The definition's profile the instance runs under, a bearer token.
const PROFILE: &str = "github.token";

/// The token the fixture of GitHub accepts, delivered to the instance as its mounted credential.
/// Fictional, and only compared.
const GITHUB_TOKEN: &str = "fixture-github-token-0001";

/// The bearer the instance admits under `static-bearer`, which the boundary adds.
const SERVICE_TOKEN: &str = "fixture-service-bearer-0001";

/// The pull request number the fixture gives the one it opens.
const NUMBER: u64 = 7;

/// The paths of the served wire, the only ones takt may request.
const DESCRIBE: &str = "/v1/describe";
const INVOKE: &str = "/v1alpha2/invoke";

/// The writes of the github map.
const CREATE: &str = "pull_request.create";
const MERGE: &str = "pull_request.merge";

/// The fixture's routes of the two writes.
const PULLS: &str = "/repos/owner/repo/pulls";
const MERGE_ROUTE: &str = "/repos/owner/repo/pulls/7/merge";

/// The case every run here opens or resumes.
const CASE: &str = "served";

/// Paths inside the instance's sandbox.
const SRV: &str = "/srv";
const CREDENTIAL: &str = "/run/connectors/credential";
const BINARIES: &str = "/opt/connectors";
const DEFINITIONS: &str = "/opt/definitions";

/// What a test needs from outside the repository.
struct Inputs {
    binary: PathBuf,
    definitions: PathBuf,
    bwrap: PathBuf,
}

/// The inputs, or `None` with the reason printed when one is absent and `CI` is not `true`. In
/// CI an absent input fails the test.
fn inputs() -> Option<Inputs> {
    let absent = |reason: String| -> Option<Inputs> {
        if std::env::var("CI").is_ok_and(|value| value == "true") {
            panic!("{reason}; CI installs it and must run this test");
        }
        eprintln!("skipped, not passed: {reason}");
        None
    };
    let Some(binary) = std::env::var_os(BIN_ENV).map(PathBuf::from) else {
        return absent(format!(
            "{BIN_ENV} is unset; it names connectors {VERSION}'s connectors-catalog-provider"
        ));
    };
    let Some(definitions) = std::env::var_os(DEFINITIONS_ENV).map(PathBuf::from) else {
        return absent(format!(
            "{DEFINITIONS_ENV} is unset; it names the root of connectors-definitions 0.6.0"
        ));
    };
    let Some(bwrap) = program("bwrap") else {
        return absent("bwrap is not on PATH; the instance runs in it".to_owned());
    };
    let binary = fs::canonicalize(&binary)
        .unwrap_or_else(|error| panic!("{BIN_ENV}={}: {error}", binary.display()));
    let definitions = fs::canonicalize(&definitions)
        .unwrap_or_else(|error| panic!("{DEFINITIONS_ENV}={}: {error}", definitions.display()));
    let version = Command::new(&binary)
        .arg("--version")
        .output()
        .expect("the instance's binary runs");
    let version = String::from_utf8_lossy(&version.stdout).into_owned();
    assert!(
        version.split_whitespace().any(|word| word == VERSION),
        "{BIN_ENV} is not connectors {VERSION}: {version}"
    );
    assert!(
        definitions.join("connectors/github.yaml").is_file(),
        "{DEFINITIONS_ENV} holds no connectors/github.yaml"
    );
    Some(Inputs {
        binary,
        definitions,
        bwrap,
    })
}

/// `name` on `PATH`, as an absolute path.
fn program(name: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path)
        .map(|dir| dir.join(name))
        .find(|candidate| candidate.is_file())
}

/// `git <args>` in `tree`, which never looks for a repository above the build's scratch
/// directory.
fn git(tree: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .args(["-c", "user.name=t", "-c", "user.email=t@localhost"])
        .env("GIT_CEILING_DIRECTORIES", env!("CARGO_TARGET_TMPDIR"))
        .args(args)
        .current_dir(tree)
        .output()
        .expect("git runs");
    assert!(
        output.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout)
        .expect("utf-8")
        .trim()
        .to_owned()
}

/// A scratch directory under this crate's build directory, removed when dropped.
struct Scratch(PathBuf);

impl Scratch {
    fn new(name: &str) -> Self {
        let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!(
            "connectors-served-instance-{}-{name}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("a scratch directory");
        Self(dir)
    }
}

impl std::ops::Deref for Scratch {
    type Target = Path;

    fn deref(&self) -> &Path {
        &self.0
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// One HTTP/1.1 request as read off a connection.
struct Request {
    method: String,
    target: String,
    /// Each header, its name lowercased.
    headers: Vec<(String, String)>,
    body: Vec<u8>,
}

impl Request {
    fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(held, _)| held == name)
            .map(|(_, value)| value.as_str())
    }

    fn path(&self) -> &str {
        self.target.split('?').next().unwrap_or_default()
    }
}

/// The head of a message up to its blank line: its first line and its headers.
fn read_head(reader: &mut impl BufRead) -> Option<(String, Vec<(String, String)>)> {
    let mut first = String::new();
    if reader.read_line(&mut first).ok()? == 0 {
        return None;
    }
    let mut headers = Vec::new();
    loop {
        let mut line = String::new();
        if reader.read_line(&mut line).ok()? == 0 {
            return None;
        }
        let line = line.trim_end();
        if line.is_empty() {
            break;
        }
        let (name, value) = line.split_once(':')?;
        headers.push((name.trim().to_ascii_lowercase(), value.trim().to_owned()));
    }
    Some((first.trim_end().to_owned(), headers))
}

/// A message's body: `content-length` bytes, the chunks of a chunked body, or, when `to_end`,
/// everything up to the end of the stream.
fn read_body(reader: &mut impl BufRead, headers: &[(String, String)], to_end: bool) -> Vec<u8> {
    let header = |name: &str| {
        headers
            .iter()
            .find(|(held, _)| held == name)
            .map(|(_, value)| value.to_ascii_lowercase())
    };
    if let Some(length) = header("content-length").and_then(|value| value.parse().ok()) {
        let mut body = vec![0; length];
        reader.read_exact(&mut body).expect("the body");
        return body;
    }
    if header("transfer-encoding").is_some_and(|value| value.contains("chunked")) {
        let mut body = Vec::new();
        loop {
            let mut size = String::new();
            reader.read_line(&mut size).expect("a chunk size");
            let size = usize::from_str_radix(size.trim().split(';').next().unwrap_or("0"), 16)
                .expect("a hex chunk size");
            let mut chunk = vec![0; size + 2];
            reader.read_exact(&mut chunk).expect("a chunk");
            if size == 0 {
                return body;
            }
            body.extend_from_slice(&chunk[..size]);
        }
    }
    let mut body = Vec::new();
    if to_end {
        let _ = reader.read_to_end(&mut body);
    }
    body
}

fn read_request(reader: &mut impl BufRead) -> Option<Request> {
    let (first, headers) = read_head(reader)?;
    let mut words = first.split_whitespace();
    let method = words.next()?.to_owned();
    let target = words.next()?.to_owned();
    let body = read_body(reader, &headers, false);
    Some(Request {
        method,
        target,
        headers,
        body,
    })
}

/// `text` with every `%XX` decoded.
fn percent_decoded(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut at = 0;
    while at < bytes.len() {
        if bytes[at] == b'%'
            && let Some(byte) = text
                .get(at + 1..at + 3)
                .and_then(|hex| u8::from_str_radix(hex, 16).ok())
        {
            out.push(byte);
            at += 3;
            continue;
        }
        out.push(bytes[at]);
        at += 1;
    }
    String::from_utf8(out).expect("utf-8")
}

/// The pull request the fixture holds once one is opened.
#[derive(Clone, Debug)]
struct Pull {
    head: String,
    base: String,
    title: String,
    state: &'static str,
    merged: bool,
}

/// What the fixture of GitHub holds: one repository `owner/repo`, its branches, the one pull
/// request opened in it, and every request answered, as its method and path.
#[derive(Debug, Default)]
struct Hub {
    branches: BTreeMap<String, String>,
    pull: Option<Pull>,
    seen: Vec<(String, String)>,
}

impl Hub {
    fn view(&self) -> Option<Value> {
        let pull = self.pull.as_ref()?;
        Some(json!({
            "number": NUMBER,
            "state": pull.state,
            "title": pull.title,
            "merged": pull.merged,
            "mergeable": pull.state == "open",
            "head": {"ref": pull.head, "sha": self.branches.get(&pull.head)},
            "base": {"ref": pull.base, "sha": self.branches.get(&pull.base)},
        }))
    }

    /// GitHub's answer to `request`, as the REST API documents it for these routes.
    fn answer(&mut self, request: &Request) -> (u16, Value) {
        let path = request.path().to_owned();
        self.seen.push((request.method.clone(), path.clone()));
        let wanted = format!("Bearer {GITHUB_TOKEN}");
        if request.header("authorization") != Some(wanted.as_str()) {
            return (401, json!({"message": "Bad credentials"}));
        }
        let not_found = (404, json!({"message": "Not Found"}));
        let Some(rest) = path.strip_prefix("/repos/owner/repo") else {
            return not_found;
        };
        let rest = percent_decoded(rest);
        let sent: Value = serde_json::from_slice(&request.body).unwrap_or(Value::Null);
        match (request.method.as_str(), rest.as_str()) {
            ("GET", route) if route.starts_with("/branches/") => {
                let name = &route["/branches/".len()..];
                match self.branches.get(name) {
                    Some(sha) => (200, json!({"name": name, "commit": {"sha": sha}})),
                    None => (404, json!({"message": "Branch not found"})),
                }
            }
            ("POST", "/pulls") => {
                let text = |name: &str| sent[name].as_str().unwrap_or_default().to_owned();
                if self.pull.is_some() {
                    return (
                        422,
                        json!({"message": "Validation Failed",
                               "errors": [{"message": "A pull request already exists"}]}),
                    );
                }
                if !self.branches.contains_key(&text("head")) {
                    return (422, json!({"message": "Validation Failed"}));
                }
                self.pull = Some(Pull {
                    head: text("head"),
                    base: text("base"),
                    title: text("title"),
                    state: "open",
                    merged: false,
                });
                (201, self.view().expect("opened"))
            }
            ("GET", "/pulls/7") => self.view().map_or(not_found, |view| (200, view)),
            ("PUT", "/pulls/7/merge") => {
                let Some(pull) = self.pull.clone() else {
                    return not_found;
                };
                if pull.state != "open" {
                    return (405, json!({"message": "Pull Request is not mergeable"}));
                }
                if self.branches.get(&pull.head).map(String::as_str) != sent["sha"].as_str() {
                    return (
                        409,
                        json!({"message": "Head branch was modified. Review and try the merge again."}),
                    );
                }
                if let Some(held) = self.pull.as_mut() {
                    held.state = "closed";
                    held.merged = true;
                }
                (
                    200,
                    json!({"sha": "0123456789abcdef0123456789abcdef01234567", "merged": true,
                           "message": "Pull Request successfully merged"}),
                )
            }
            ("GET", route) if route.starts_with("/commits/") && route.ends_with("/check-runs") => (
                200,
                json!({"total_count": 1, "check_runs": [
                    {"id": 1, "name": "ci", "status": "completed", "conclusion": "success"}
                ]}),
            ),
            ("GET", route) if route.starts_with("/commits/") && route.ends_with("/status") => (
                200,
                json!({"state": "pending", "total_count": 0, "statuses": []}),
            ),
            _ => not_found,
        }
    }

    /// The writes the fixture answered, as method and path.
    fn writes(&self) -> Vec<(String, String)> {
        self.seen
            .iter()
            .filter(|(method, _)| method != "GET")
            .cloned()
            .collect()
    }
}

/// The fixture of GitHub's REST API: HTTPS on 127.0.0.1 under a certificate made for it.
struct GitHub {
    port: u16,
    pem: String,
    hub: Arc<Mutex<Hub>>,
    stop: Arc<AtomicBool>,
}

impl GitHub {
    fn start() -> Self {
        let made = rcgen::generate_simple_self_signed(vec![
            "127.0.0.1".to_owned(),
            "localhost".to_owned(),
        ])
        .expect("a certificate");
        let config = rustls::ServerConfig::builder_with_provider(Arc::new(
            rustls::crypto::ring::default_provider(),
        ))
        .with_safe_default_protocol_versions()
        .expect("TLS versions")
        .with_no_client_auth()
        .with_single_cert(
            vec![made.cert.der().clone()],
            rustls::pki_types::PrivatePkcs8KeyDer::from(made.signing_key.serialize_der()).into(),
        )
        .expect("the certificate");
        let config = Arc::new(config);
        let listener = TcpListener::bind("127.0.0.1:0").expect("a port");
        let port = listener.local_addr().expect("an address").port();
        let hub = Arc::new(Mutex::new(Hub::default()));
        let stop = Arc::new(AtomicBool::new(false));
        {
            let (hub, stop) = (hub.clone(), stop.clone());
            std::thread::spawn(move || {
                for stream in listener.incoming() {
                    if stop.load(Ordering::SeqCst) {
                        break;
                    }
                    let Ok(stream) = stream else { continue };
                    let (hub, config) = (hub.clone(), config.clone());
                    std::thread::spawn(move || serve_github(stream, config, &hub));
                }
            });
        }
        Self {
            port,
            pem: made.cert.pem(),
            hub,
            stop,
        }
    }

    fn endpoint(&self) -> String {
        format!("https://127.0.0.1:{}", self.port)
    }

    fn writes(&self) -> Vec<(String, String)> {
        self.hub.lock().expect("the hub").writes()
    }
}

impl Drop for GitHub {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        let _ = TcpStream::connect(("127.0.0.1", self.port));
    }
}

fn serve_github(stream: TcpStream, config: Arc<rustls::ServerConfig>, hub: &Mutex<Hub>) {
    let _ = stream.set_read_timeout(Some(Duration::from_secs(30)));
    let Ok(connection) = rustls::ServerConnection::new(config) else {
        return;
    };
    let mut tls = BufReader::new(rustls::StreamOwned::new(connection, stream));
    let Some(request) = read_request(&mut tls) else {
        return;
    };
    let (status, body) = hub.lock().expect("the hub").answer(&request);
    let body = serde_json::to_vec(&body).expect("json");
    let head = format!(
        "HTTP/1.1 {status} fixture\r\ncontent-type: application/json; charset=utf-8\r\ncontent-length: {}\r\nx-ratelimit-remaining: 4999\r\nx-ratelimit-reset: 4000000000\r\nconnection: close\r\n\r\n",
        body.len()
    );
    let tls = tls.get_mut();
    let _ = tls.write_all(head.as_bytes());
    let _ = tls.write_all(&body);
    tls.conn.send_close_notify();
    let _ = tls.flush();
}

/// The served instance in its sandbox, its stderr kept for the assertions' messages.
struct Instance {
    child: Option<Child>,
    address: SocketAddr,
    log: Arc<Mutex<String>>,
}

impl Instance {
    /// Lays out the sandbox under `root` and starts the instance in it: its configuration, its
    /// service bearer, the fixture's root, the grant file covering both writes on `owner/repo`,
    /// and the credential document as the flat mounted item `<instance id>/<profile>`.
    fn start(root: &Path, inputs: &Inputs, github: &GitHub) -> Self {
        let srv = root.join("srv");
        let secret = root.join("secret");
        for (dir, mode) in [
            (srv.clone(), 0o755),
            (srv.join("state"), 0o700),
            (srv.join("tmp"), 0o700),
            (secret.clone(), 0o755),
            (secret.join(INSTANCE_ID), 0o755),
        ] {
            fs::create_dir_all(&dir).expect("a directory");
            fs::set_permissions(&dir, fs::Permissions::from_mode(mode)).expect("its mode");
        }
        let write = |path: PathBuf, bytes: &[u8], mode: u32| {
            fs::write(&path, bytes).expect("a file");
            fs::set_permissions(&path, fs::Permissions::from_mode(mode)).expect("its mode");
        };
        let grant = |id: &str, operation: &str| {
            json!({
                "id": id, "instance": INSTANCE_ID, "operation": operation,
                "target": [{"pointer": "/owner", "value": "owner"}, {"pointer": "/repo", "value": "repo"}],
                "not_after_unix_ms": 4_000_000_000_000_i64, "max_per_hour": 20
            })
        };
        let grants = json!({
            "format": "connectors.standing-grants/1",
            "grants": [grant("open-in-owner-repo", CREATE), grant("merge-in-owner-repo", MERGE)],
            "revoked": []
        });
        let config = json!({
            "service": {
                "instance": INSTANCE_ID,
                "listen": "127.0.0.1:0",
                "service_credential": {"kind": "file", "path": format!("{SRV}/service-token")},
                "state": format!("{SRV}/state"),
                "standing_grants": format!("{SRV}/grants.json"),
            },
            "catalog": {
                "definition": format!("{DEFINITIONS}/connectors/github.yaml"),
                "definitions_root": DEFINITIONS,
                "instance": NAME,
                "endpoint": github.endpoint(),
                "profile": PROFILE,
            },
            "credential": {"binding": "mounted_secret", "directory": CREDENTIAL},
        });
        write(srv.join("service-token"), SERVICE_TOKEN.as_bytes(), 0o600);
        write(srv.join("ca.pem"), github.pem.as_bytes(), 0o644);
        write(
            srv.join("grants.json"),
            &serde_json::to_vec(&grants).expect("json"),
            0o600,
        );
        write(
            srv.join("served.json"),
            &serde_json::to_vec(&config).expect("json"),
            0o600,
        );
        write(
            secret.join(INSTANCE_ID).join(PROFILE),
            &serde_json::to_vec(&json!({"token": GITHUB_TOKEN})).expect("json"),
            0o400,
        );

        let binaries = inputs.binary.parent().expect("the binary's directory");
        let name = inputs.binary.file_name().expect("the binary's name");
        let mut command = Command::new(&inputs.bwrap);
        command.args(["--unshare-user", "--unshare-ipc", "--die-with-parent"]);
        for dir in ["usr", "etc"] {
            command.args(["--ro-bind", &format!("/{dir}"), &format!("/{dir}")]);
        }
        for name in ["bin", "sbin", "lib", "lib32", "lib64", "libx32"] {
            let host = Path::new("/").join(name);
            if let Ok(target) = fs::read_link(&host) {
                command.arg("--symlink").arg(target).arg(&host);
            } else if host.is_dir() {
                command.arg("--ro-bind").arg(&host).arg(&host);
            }
        }
        command
            .args(["--proc", "/proc", "--dev", "/dev"])
            .arg("--bind")
            .arg(&srv)
            .arg(SRV)
            .arg("--ro-bind")
            .arg(&secret)
            .arg(CREDENTIAL)
            .arg("--ro-bind")
            .arg(binaries)
            .arg(BINARIES)
            .arg("--ro-bind")
            .arg(&inputs.definitions)
            .arg(DEFINITIONS)
            .arg("--")
            .arg(Path::new(BINARIES).join(name))
            .args(["--config", &format!("{SRV}/served.json")])
            .env_clear()
            .env("SSL_CERT_FILE", format!("{SRV}/ca.pem"))
            .env("TMPDIR", format!("{SRV}/tmp"))
            .env("NO_COLOR", "1")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::piped());
        let mut child = command.spawn().expect("bwrap starts");
        let stderr = child.stderr.take().expect("its stderr");
        let log = Arc::new(Mutex::new(String::new()));
        let (lines, listening) = mpsc::channel();
        {
            let log = log.clone();
            std::thread::spawn(move || {
                for line in BufReader::new(stderr).lines() {
                    let Ok(line) = line else { break };
                    log.lock().expect("the log").push_str(&format!("{line}\n"));
                    let _ = lines.send(line);
                }
            });
        }
        let mut instance = Self {
            child: Some(child),
            address: SocketAddr::from(([127, 0, 0, 1], 0)),
            log,
        };
        let started = Instant::now();
        while started.elapsed() < Duration::from_secs(60) {
            match listening.recv_timeout(Duration::from_millis(200)) {
                Ok(line) if line.contains("adapter service listening") => {
                    let listen = line
                        .split_whitespace()
                        .find_map(|field| field.strip_prefix("listen="))
                        .expect("the listening line names its address");
                    instance.address = listen.parse().expect("a socket address");
                    return instance;
                }
                Ok(_) | Err(mpsc::RecvTimeoutError::Timeout) => {}
                Err(mpsc::RecvTimeoutError::Disconnected) => {
                    let status = instance.child.as_mut().map(Child::wait);
                    panic!("the instance exited {status:?}: {}", instance.log());
                }
            }
        }
        panic!(
            "the instance did not listen within 60 s: {}",
            instance.log()
        );
    }

    fn log(&self) -> String {
        self.log.lock().expect("the log").clone()
    }
}

impl Drop for Instance {
    fn drop(&mut self) {
        if let Some(mut child) = self.child.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

/// One invocation the boundary passed on: the operation, its key, the instance's answer and
/// whether the boundary dropped that answer.
#[derive(Clone, Debug)]
struct Exchange {
    operation: String,
    key: Option<String>,
    status: u16,
    answer: Value,
    dropped: bool,
}

/// What the boundary saw of takt's requests: each method and path, and whether takt sent an
/// `Authorization` header of its own.
#[derive(Debug, Default)]
struct Passed {
    requests: Vec<(String, String, bool)>,
    exchanges: Vec<Exchange>,
    /// The operations whose next answer is dropped.
    drop: BTreeSet<String>,
}

/// The loopback boundary between takt and the instance.
struct Boundary {
    port: u16,
    passed: Arc<Mutex<Passed>>,
    stop: Arc<AtomicBool>,
}

impl Boundary {
    fn start(upstream: SocketAddr, drop: &[&str]) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("a port");
        let port = listener.local_addr().expect("an address").port();
        let passed = Arc::new(Mutex::new(Passed {
            drop: drop.iter().map(|name| (*name).to_owned()).collect(),
            ..Passed::default()
        }));
        let stop = Arc::new(AtomicBool::new(false));
        {
            let (passed, stop) = (passed.clone(), stop.clone());
            std::thread::spawn(move || {
                for stream in listener.incoming() {
                    if stop.load(Ordering::SeqCst) {
                        break;
                    }
                    let Ok(stream) = stream else { continue };
                    let passed = passed.clone();
                    std::thread::spawn(move || relay(stream, upstream, &passed));
                }
            });
        }
        Self { port, passed, stop }
    }

    fn url(&self) -> String {
        format!("http://127.0.0.1:{}", self.port)
    }

    fn exchanges_of(&self, operation: &str) -> Vec<Exchange> {
        self.passed
            .lock()
            .expect("the boundary")
            .exchanges
            .iter()
            .filter(|exchange| exchange.operation == operation)
            .cloned()
            .collect()
    }

    /// That takt requested only the served wire's two paths and never sent a credential.
    fn assert_served_wire_only(&self) {
        let passed = self.passed.lock().expect("the boundary");
        assert!(!passed.requests.is_empty(), "takt reached the boundary");
        for (method, path, authorization) in &passed.requests {
            assert!(
                (method == "GET" && path == DESCRIBE) || (method == "POST" && path == INVOKE),
                "a request off the served wire: {method} {path}"
            );
            assert!(!authorization, "takt sent a credential: {method} {path}");
        }
    }
}

impl Drop for Boundary {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        let _ = TcpStream::connect(("127.0.0.1", self.port));
    }
}

/// Passes one request from takt to the instance with the service bearer added, and the
/// instance's answer back, unless it is the answer to a write whose answer is to be dropped:
/// then the connection is closed with nothing sent.
fn relay(client: TcpStream, upstream: SocketAddr, passed: &Mutex<Passed>) {
    let _ = client.set_read_timeout(Some(Duration::from_secs(30)));
    let mut reader = BufReader::new(client.try_clone().expect("the client"));
    let Some(request) = read_request(&mut reader) else {
        return;
    };
    let path = request.path().to_owned();
    passed.lock().expect("the boundary").requests.push((
        request.method.clone(),
        path.clone(),
        request.header("authorization").is_some(),
    ));
    let mut forward = format!("{} {} HTTP/1.1\r\n", request.method, request.target).into_bytes();
    for (name, value) in &request.headers {
        if !matches!(
            name.as_str(),
            "authorization" | "connection" | "content-length" | "transfer-encoding" | "keep-alive"
        ) {
            forward.extend_from_slice(format!("{name}: {value}\r\n").as_bytes());
        }
    }
    forward.extend_from_slice(
        format!(
            "authorization: Bearer {SERVICE_TOKEN}\r\ncontent-length: {}\r\nconnection: close\r\n\r\n",
            request.body.len()
        )
        .as_bytes(),
    );
    forward.extend_from_slice(&request.body);
    let mut instance = TcpStream::connect(upstream).expect("the instance answers");
    let _ = instance.set_read_timeout(Some(Duration::from_secs(60)));
    instance
        .write_all(&forward)
        .expect("the request is passed on");
    let mut raw = Vec::new();
    let _ = instance.read_to_end(&mut raw);
    let mut answer = BufReader::new(raw.as_slice());
    let (first, headers) = read_head(&mut answer).unwrap_or_default();
    let status = first
        .split_whitespace()
        .nth(1)
        .and_then(|code| code.parse().ok())
        .unwrap_or(0);
    let body = read_body(&mut answer, &headers, true);
    let mut dropped = false;
    if path == INVOKE {
        let envelope: Value = serde_json::from_slice(&request.body).unwrap_or(Value::Null);
        let operation = envelope["operation"]
            .as_str()
            .unwrap_or_default()
            .to_owned();
        let mut passed = passed.lock().expect("the boundary");
        dropped = passed.drop.remove(&operation);
        passed.exchanges.push(Exchange {
            key: envelope["idempotency_key"].as_str().map(str::to_owned),
            operation,
            status,
            answer: serde_json::from_slice(&body).unwrap_or(Value::Null),
            dropped,
        });
    }
    let mut client = client;
    if !dropped {
        let _ = client.write_all(&raw);
    }
    let _ = client.shutdown(Shutdown::Both);
}

/// The fixture, the instance in front of it and the boundary in front of that.
struct Served {
    github: GitHub,
    instance: Instance,
    boundary: Boundary,
}

impl Served {
    /// Everything started under `scratch`, the boundary dropping the first answer to each
    /// write in `drop`.
    fn start(scratch: &Scratch, inputs: &Inputs, drop: &[&str]) -> Self {
        let github = GitHub::start();
        let instance = Instance::start(&scratch.join("instance"), inputs, &github);
        let boundary = Boundary::start(instance.address, drop);
        Self {
            github,
            instance,
            boundary,
        }
    }

    /// What the instance logged and what the fixture saw, for an assertion's message.
    fn said(&self) -> String {
        format!(
            "fixture saw {:?}\ninstance log:\n{}",
            self.github.hub.lock().expect("the hub").seen,
            self.instance.log()
        )
    }
}

/// The forge's runner double: a push moves the fixture's branch, `ls-remote` reads it.
#[derive(Clone)]
struct Pushes {
    hub: Arc<Mutex<Hub>>,
}

impl Runner for Pushes {
    fn run(&self, tree: &Path, argv: &[String]) -> anyhow::Result<Exit> {
        self.run_env(tree, argv, &[])
    }

    fn run_env(
        &self,
        _tree: &Path,
        argv: &[String],
        _env: &[(&str, &str)],
    ) -> anyhow::Result<Exit> {
        assert_eq!(
            argv.first().map(String::as_str),
            Some("git"),
            "a line that is not git: {argv:?}"
        );
        if argv.iter().any(|word| word == "push") {
            let refspec = argv
                .iter()
                .find(|word| word.contains(":refs/heads/"))
                .ok_or_else(|| anyhow!("a push with no refspec: {argv:?}"))?;
            let (revision, branch) = refspec.split_once(":refs/heads/").expect("a refspec");
            self.hub
                .lock()
                .expect("the hub")
                .branches
                .insert(branch.to_owned(), revision.to_owned());
        }
        let mut stdout = String::new();
        if argv.iter().any(|word| word == "ls-remote") {
            let reference = argv.last().expect("a reference");
            let branch = reference.strip_prefix("refs/heads/").expect("a branch");
            if let Some(sha) = self.hub.lock().expect("the hub").branches.get(branch) {
                stdout = format!("{sha}\t{reference}\n");
            }
        }
        Ok(Exit {
            code: Some(0),
            stdout,
            stderr: String::new(),
        })
    }
}

/// A repository at `<scratch>/tree` whose `main` holds a README, the judge allow-list and the
/// owner's delegation of the merge to `agent:takt-delivery`, `origin` naming `owner/repo` on a
/// host that does not exist. Returns the tree and `main`'s commit.
fn repository(scratch: &Scratch) -> (PathBuf, String) {
    let tree = scratch.join("tree");
    let files = [
        ("README.md", "a repository\n".to_owned()),
        ("docs/guide.md", "a guide\n".to_owned()),
        (
            ".engineering/judge-allow.yaml",
            "paths:\n  - docs/**\n  - README.md\n  - CHANGELOG.md\n".to_owned(),
        ),
        (
            ".engineering/delegations.yaml",
            ["repository.merge", "repository.merge.judge_surface"]
                .iter()
                .fold("delegations:\n".to_owned(), |text, capability| {
                    text + &format!(
                        "  - owner: human:owner\n    role: agent:takt-delivery\n    capability: {capability}\n    given_at: 2026-10-30T08:00:00Z\n"
                    )
                }),
        ),
    ];
    for (path, text) in files {
        let path = tree.join(path);
        fs::create_dir_all(path.parent().expect("a parent")).expect("a directory");
        fs::write(path, text).expect("a file");
    }
    git(&tree, &["init", "-q", "-b", "main"]);
    git(&tree, &["add", "-A"]);
    git(&tree, &["commit", "-q", "-m", "start"]);
    git(
        &tree,
        &[
            "remote",
            "add",
            "origin",
            "https://github.example/owner/repo.git",
        ],
    );
    let main = git(&tree, &["rev-parse", "HEAD"]);
    (tree, main)
}

/// A fresh forge over the boundary, as a new takt process selects one from its run file:
/// `forge: connectors`, `forge_instance: github`, `forge_endpoint` the boundary.
fn process(scratch: &Scratch, tree: &Path, served: &Served) -> Rc<dyn Forge> {
    let path = scratch.join("run.yaml");
    fs::write(
        &path,
        format!(
            "forge: connectors\nforge_instance: github\nforge_endpoint: {}\nscan: none\n",
            served.boundary.url()
        ),
    )
    .expect("the run file");
    let config = RunConfig::read(&path).expect("the run file reads");
    let forge = ConnectorsForge::select(
        tree,
        config.forge.as_deref(),
        &config.forge_settings(),
        Scan::None { declared_in: path },
        Box::new(Pushes {
            hub: served.github.hub.clone(),
        }),
    )
    .unwrap_or_else(|error| panic!("the forge is selected: {error:#}\n{}", served.said()))
    .expect("a forge");
    Rc::new(forge)
}

/// A harness that asks for the merge gate whenever it is offered and hands the change over when
/// that is offered: the run's grant is never its choice.
struct Requester;

impl Harness for Requester {
    fn name(&self) -> &str {
        "requester"
    }

    fn step(&self, context: &StepContext<'_>) -> Result<StepReport, anyhow::Error> {
        if context.approvals.contains(&"repository.merge".to_owned()) {
            return Ok(StepReport {
                action: "repository.merge".to_owned(),
                summary: "the intent holds; merge it".to_owned(),
                tokens: None,
            });
        }
        if context.offered.contains(&"change.handover".to_owned()) {
            return Ok(StepReport {
                action: "change.handover".to_owned(),
                summary: "hand it over".to_owned(),
                tokens: None,
            });
        }
        Err(anyhow!(
            "neither the gate nor the handover was offered; among {}",
            context.offered.join(", ")
        ))
    }
}

fn run_options() -> RunOptions {
    RunOptions {
        protocol: "story.delivery/1".to_owned(),
        intent: "change the guide\n\nA guide that says more.".to_owned(),
        inputs: Vec::new(),
        iterations: Some(12),
        hours: None,
        tokens: None,
        case_id: Some(CASE.to_owned()),
        gate: Some("repository.merge".to_owned()),
    }
}

fn validators_of(forge: Rc<dyn Forge>) -> Vec<Box<dyn Validator>> {
    vec![
        Box::new(PrOpen::new(forge.clone())),
        Box::new(CiRead::new(forge.clone())),
        Box::new(PrMerged::new(forge)),
    ]
}

/// The first run of `story.delivery/1` on the case [`CASE`], its change a docs edit with its
/// tests, gate, handover and change records in place: how it stopped, and the case directory.
fn first_run(
    scratch: &Scratch,
    tree: &Path,
    served: &Served,
    out: &mut Vec<u8>,
) -> (anyhow::Result<Stop>, PathBuf) {
    let validators = validators_of(process(scratch, tree, served));
    let mut run = Run::open(tree, run_options(), &Requester, &validators).expect("the case opens");
    fs::write(tree.join("docs/guide.md"), "a guide, changed\n").expect("a file");
    git(tree, &["add", "-A", "--", ".", ":!.takt"]);
    git(tree, &["commit", "-q", "-m", "change the guide"]);
    let revision = git(tree, &["rev-parse", "HEAD"]);
    run.case
        .set_revision("implementation", &revision)
        .expect("the revision moves");
    let short = &revision[..8];
    for (id, kind, result, producer) in [
        ("e0".to_owned(), "test_result", Some("pass"), "served-test"),
        ("e1".to_owned(), "gate_result", Some("pass"), "served-test"),
        (
            format!("handover-{short}"),
            "handover",
            None,
            takt::run::CHOICE_PRODUCER,
        ),
        (
            format!("change-record-{short}"),
            "change_record",
            Some("ahead"),
            "takt.change-read",
        ),
    ] {
        run.case
            .add_evidence(&Record::new(
                id,
                kind,
                result.map(str::to_owned),
                "implementation",
                &revision,
                producer,
            ))
            .expect("the record is written");
    }
    let stop = run.run(out);
    (stop, run.case.dir.clone())
}

/// The run resumed on the case [`CASE`] by a new process: how it stopped.
fn resumed_run(
    scratch: &Scratch,
    tree: &Path,
    served: &Served,
    out: &mut Vec<u8>,
) -> anyhow::Result<Stop> {
    let validators = validators_of(process(scratch, tree, served));
    let mut run =
        Run::resume(tree, run_options(), CASE, &Requester, &validators).expect("the case resumes");
    run.run(out)
}

/// The records of `kind` under the case at `dir`, each as its JSON.
fn records_of(dir: &Path, kind: &str) -> Vec<Value> {
    let Ok(entries) = fs::read_dir(dir.join("evidence")) else {
        return Vec::new();
    };
    entries
        .map(|entry| {
            let path = entry.expect("an entry").path();
            serde_json::from_str::<Value>(&fs::read_to_string(&path).expect("a record"))
                .expect("json")
        })
        .filter(|record| record["kind"] == kind)
        .collect()
}

/// That the case at `dir` is merged once, its pull request opened and the merge granted under
/// the owner's delegation.
fn assert_delivered(dir: &Path, out: &str) {
    let merged = records_of(dir, "merge_observation");
    assert_eq!(merged.len(), 1, "{out}");
    assert_eq!(merged[0]["result"], "merged", "{out}");
    let opened = records_of(dir, "pull_request");
    assert!(
        opened.iter().any(|record| record["result"] == "open"),
        "the pull request was opened: {opened:?}\n{out}"
    );
    let authority = Authority::read(dir)
        .expect("authority.yaml reads")
        .expect("a grant");
    assert_eq!(
        authority
            .decision("repository.merge")
            .map(|grant| grant.approver.as_str()),
        Some("agent:takt-delivery"),
        "{out}"
    );
}

/// The two writes the fixture of GitHub must have seen, once each.
fn one_create_and_one_merge() -> Vec<(String, String)> {
    vec![
        ("POST".to_owned(), PULLS.to_owned()),
        ("PUT".to_owned(), MERGE_ROUTE.to_owned()),
    ]
}

/// Acceptance 1: a run on `forge: connectors`, `forge_instance: github` and `forge_endpoint` at
/// the instance (through the boundary) opens the pull request, reads it and its checks, and
/// merges it under the owner's delegation. The fixture of GitHub saw exactly one create and one
/// merge, both under the mounted credential; each write went keyed, the grant covered it, and
/// the instance applied it; takt sent no credential and requested only the served wire.
#[test]
fn a_run_opens_reads_and_merges_a_pull_request_through_a_served_instance() {
    let Some(inputs) = inputs() else { return };
    let scratch = Scratch::new("run");
    let served = Served::start(&scratch, &inputs, &[]);
    let (tree, main) = repository(&scratch);
    served
        .github
        .hub
        .lock()
        .expect("the hub")
        .branches
        .insert("main".to_owned(), main);
    let mut out = Vec::new();
    let (stop, dir) = first_run(&scratch, &tree, &served, &mut out);
    let out = String::from_utf8_lossy(&out).into_owned();
    let said = served.said();
    let stop =
        stop.unwrap_or_else(|error| panic!("the run ends in error: {error:#}\n{out}\n{said}"));
    assert_eq!(
        stop,
        Stop::Completed("accepted".to_owned()),
        "{out}\n{said}"
    );
    assert_delivered(&dir, &out);

    assert_eq!(served.github.writes(), one_create_and_one_merge(), "{said}");
    {
        let hub = served.github.hub.lock().expect("the hub");
        assert!(hub.pull.as_ref().is_some_and(|pull| pull.merged), "{said}");
        for route in [
            "/repos/owner/repo/pulls/7",
            "/repos/owner/repo/branches/main",
        ] {
            assert!(
                hub.seen
                    .iter()
                    .any(|(method, path)| method == "GET" && path == route),
                "GET {route} was not read: {said}"
            );
        }
    }
    served.boundary.assert_served_wire_only();
    for operation in [CREATE, MERGE] {
        let exchanges = served.boundary.exchanges_of(operation);
        assert_eq!(exchanges.len(), 1, "{operation}: {exchanges:?}");
        let exchange = &exchanges[0];
        assert!(
            exchange
                .key
                .as_deref()
                .is_some_and(|key| key.starts_with("takt-")),
            "{exchange:?}"
        );
        assert_eq!(exchange.status, 200, "{exchange:?}");
        assert_eq!(
            exchange.answer["mutation"]["classification"], "applied",
            "{exchange:?}"
        );
        assert_eq!(
            exchange.answer["mutation"]["replayed"], false,
            "{exchange:?}"
        );
    }
}

/// A case whose first run met a lost answer to each write in `drop`: everything started, the
/// first run's stop and the case directory.
struct Lost {
    scratch: Scratch,
    served: Served,
    tree: PathBuf,
    dir: PathBuf,
    stops: Vec<Result<Stop, String>>,
    out: Vec<u8>,
}

impl Lost {
    fn new(inputs: &Inputs, name: &str, drop: &[&str]) -> Self {
        let scratch = Scratch::new(name);
        let served = Served::start(&scratch, inputs, drop);
        let (tree, main) = repository(&scratch);
        served
            .github
            .hub
            .lock()
            .expect("the hub")
            .branches
            .insert("main".to_owned(), main);
        let mut out = Vec::new();
        let (first, dir) = first_run(&scratch, &tree, &served, &mut out);
        let stops = vec![first.map_err(|error| format!("{error:#}"))];
        Self {
            scratch,
            served,
            tree,
            dir,
            stops,
            out,
        }
    }

    /// Resumes the case by a new process until it completes, at most four times.
    fn resume_until_completed(&mut self) {
        for _ in 0..4 {
            if matches!(self.stops.last(), Some(Ok(Stop::Completed(_)))) {
                break;
            }
            let stop = resumed_run(&self.scratch, &self.tree, &self.served, &mut self.out);
            self.stops.push(stop.map_err(|error| format!("{error:#}")));
        }
    }

    /// That the case completed accepted and merged, and the fixture of GitHub saw one create and
    /// one merge.
    fn assert_delivered_once(&self) {
        let out = String::from_utf8_lossy(&self.out).into_owned();
        let said = self.served.said();
        let stops = &self.stops;
        assert!(
            matches!(stops.last(), Some(Ok(Stop::Completed(outcome))) if outcome == "accepted"),
            "stops {stops:#?}\n{out}\n{said}"
        );
        assert_delivered(&self.dir, &out);
        assert_eq!(
            self.served.github.writes(),
            one_create_and_one_merge(),
            "stops {stops:#?}\n{said}"
        );
    }

    /// That `operation` was applied once, its answer dropped, then sent again under the same key
    /// and answered with the instance's replay of that first attempt.
    fn assert_replayed(&self, operation: &str) {
        let exchanges = self.served.boundary.exchanges_of(operation);
        assert!(
            exchanges.len() >= 2,
            "{operation} was not sent again: {exchanges:#?}\nstops {:#?}",
            self.stops
        );
        assert!(exchanges[0].dropped, "{exchanges:#?}");
        assert_eq!(
            exchanges[0].answer["mutation"]["classification"], "applied",
            "the instance applied the first {operation}: {exchanges:#?}"
        );
        let key = exchanges[0].key.clone();
        assert!(
            key.as_deref().is_some_and(|key| key.starts_with("takt-")),
            "{exchanges:#?}"
        );
        for again in &exchanges[1..] {
            assert!(!again.dropped, "{again:?}");
            assert_eq!(again.key, key, "the same key: {exchanges:#?}");
            assert_eq!(again.status, 200, "{again:?}");
            assert_eq!(again.answer["mutation"]["replayed"], true, "{again:?}");
            assert_eq!(
                again.answer["mutation"]["attempt"], exchanges[0].answer["mutation"]["attempt"],
                "the replay is the first attempt: {exchanges:#?}"
            );
        }
    }
}

/// Acceptance 3, the open: the instance applies the create and its answer is lost between it and
/// takt. The run that lost it ends; the case is resumed by a new process, whose forge sends the
/// create again under the same key, and the instance replays the first outcome: the fixture of
/// GitHub saw no second create, and the case merges.
#[test]
fn a_create_whose_answer_was_lost_is_replayed_and_applied_once() {
    let Some(inputs) = inputs() else { return };
    let mut lost = Lost::new(&inputs, "lost-create", &[CREATE]);
    assert!(
        lost.stops[0].is_err(),
        "the run whose create lost its answer ends: {:?}",
        lost.stops
    );
    lost.resume_until_completed();
    lost.assert_delivered_once();
    lost.assert_replayed(CREATE);
}

/// Acceptance 3, the merge: the instance applies the merge and its answer is lost between it and
/// takt, so the run that sent it ends. A new process's forge sends the merge again for the same
/// case and head, under the same key, and the instance replays the first outcome, a merge; the
/// case resumed then completes merged. The fixture of GitHub saw no second merge.
#[test]
fn a_merge_whose_answer_was_lost_is_replayed_and_applied_once() {
    let Some(inputs) = inputs() else { return };
    let mut lost = Lost::new(&inputs, "lost-merge", &[MERGE]);
    assert!(
        !matches!(lost.stops[0], Ok(Stop::Completed(_))),
        "the run whose merge lost its answer does not complete: {:?}",
        lost.stops
    );
    let head = {
        let hub = lost.served.github.hub.lock().expect("the hub");
        let pull = hub.pull.clone().expect("a pull request");
        assert!(pull.merged, "the instance applied the merge");
        hub.branches[&pull.head].clone()
    };
    process(&lost.scratch, &lost.tree, &lost.served)
        .merge(&lost.tree, &lost.dir.join("artifacts"), NUMBER, &head)
        .unwrap_or_else(|error| {
            panic!(
                "the merge sent again replays: {error:#}\n{}",
                lost.served.said()
            )
        });
    lost.assert_replayed(MERGE);
    lost.resume_until_completed();
    lost.assert_delivered_once();
}
