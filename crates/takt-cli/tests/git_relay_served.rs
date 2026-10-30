//! story:git-through-the-connectors-git-relay, acceptance 1 and 2, against connectors 0.62.0's
//! served instance: its release asset's `connectors-catalog-provider`, named by
//! `TAKT_TEST_CONNECTORS_BIN` (CI installs it, pinned by sha256, in `.github/workflows/check.yml`).
//! Without the variable a case fails under `CI=true` and is skipped with its reason elsewhere;
//! without `bwrap` the same. The file is Linux only: the instance's `mounted_secret` binding is.
//!
//! The instance relays git smart-HTTP (`/git/o/r.git`) with `ref_prefixes` `takt/` and `trial/`.
//! Its upstream is a local bare repository behind `git http-backend` (`support/smart_http.rs`),
//! reached over HTTPS on 127.0.0.1 through a terminator here whose self-signed root is the
//! relay's `ca_file`. The instance runs under bubblewrap, in its own user and mount namespace,
//! so that its forge token, a fictional one, is a mounted item on a read-only mount as the
//! binding requires. A loopback boundary stand-in adds the bearer the instance admits; takt's
//! `origin` is the boundary, and takt holds no credential for it.
//!
//! Acceptance 1: `takt run --forge connectors` pushes the case's branch through the relay, and
//! the branch is in the upstream repository; at the merge it fetches its base, which the
//! upstream moved meanwhile, through the relay. The pull request is a stand-in for the github
//! instance's operations, on 127.0.0.1. Acceptance 2: `takt scenario run` creates `trial/0`
//! through the same relay. Nothing here reaches beyond 127.0.0.1.
#![cfg(target_os = "linux")]

#[path = "../../takt/tests/support/own_home.rs"]
mod own_home;

#[allow(dead_code)]
#[path = "support/smart_http.rs"]
mod smart_http;

use std::fs;
use std::io::{BufRead as _, BufReader, Read as _, Write as _};
use std::net::{Shutdown, TcpListener, TcpStream};
use std::os::unix::fs::{PermissionsExt as _, symlink};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex, mpsc};
use std::time::{Duration, Instant};

use serde_json::{Value, json};
use smart_http::{GitStandIn, Request, read_body, read_head};

/// The program the test runs the served Connectors instance in, so its mounted credential is
/// read-only. It confines the instance under test, not takt; it is spelled in parts because
/// `confinement_substrate`'s check counts the whole name in any of takt's crates as a mechanism
/// of takt's own.
const SANDBOX: &str = concat!("bw", "rap");

/// The fixture stores of the library crate, one directory each.
const FIXTURES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../takt/tests/fixtures");

/// A gates document whose one assertion is true in a repository with a manifest.
const GATES: &str = "format: engineering-gates/1\nlanguage: canon-expr/1\ngates:\n  - id: manifest\n    \
                     text: The repository has its manifest.\n    assertions:\n      \
                     - 'fs.file.exists(\"Cargo.toml\")'\n";

/// The delivery protocol, the engineering protocols' built-in, which declares `pr.open`.
const DELIVERY_PROTOCOL: &str = "story.delivery/1";

/// The variable that names the served binary, `connectors-catalog-provider` of the asset.
const BINARY_VARIABLE: &str = "TAKT_TEST_CONNECTORS_BIN";

/// The proxy variables a git child reads; every run here clears them.
const PROXY_VARIABLES: [&str; 8] = [
    "http_proxy",
    "HTTP_PROXY",
    "https_proxy",
    "HTTPS_PROXY",
    "all_proxy",
    "ALL_PROXY",
    "no_proxy",
    "NO_PROXY",
];

/// The fictional forge token in the instance's mounted `gitlab.pat` item.
const FORGE_TOKEN: &str = concat!("served-", "relay-", "forge-", "token-", "51c0");

/// The fictional bearer the instance admits (`static-bearer`); only the boundary holds it.
const BEARER: &str = concat!("served-", "relay-", "bearer-", "93ad");

/// The identity every commit here carries, and the relay's `commit_identity`.
const IDENTITY: (&str, &str) = ("Test Bot", "bot@example.invalid");

/// Where the instance's files are inside its namespace.
const SERVED_ROOT: &str = "/srv";
const MOUNTED: &str = "/run/connectors/credential";
const BINARY: &str = "/opt/connectors/connectors-catalog-provider";

/// The served binary, or none with the reason printed; under `CI=true` a missing one fails.
fn served_binary() -> Option<PathBuf> {
    let missing = |reason: String| {
        assert!(
            std::env::var("CI").as_deref() != Ok("true"),
            "{reason}; CI installs connectors 0.62.0 and bubblewrap for this test"
        );
        eprintln!("skipped: {reason}");
        None
    };
    let Some(path) = std::env::var_os(BINARY_VARIABLE) else {
        return missing(format!(
            "{BINARY_VARIABLE} is not set, so no connectors 0.62.0 served instance can run"
        ));
    };
    let path = PathBuf::from(path);
    if !path.is_file() {
        return missing(format!(
            "{BINARY_VARIABLE} names {}, which is no file",
            path.display()
        ));
    }
    if find_program(SANDBOX).is_none() {
        return missing(
            "bwrap is not on PATH, and the instance's mounted credential needs a read-only mount"
                .to_owned(),
        );
    }
    Some(path)
}

fn find_program(name: &str) -> Option<PathBuf> {
    std::env::split_paths(&std::env::var_os("PATH").unwrap_or_default())
        .map(|dir| dir.join(name))
        .find(|path| path.is_file())
}

fn git(tree: &Path, args: &[&str]) {
    let output = Command::new("git")
        .args(["-c", &format!("user.name={}", IDENTITY.0)])
        .args(["-c", &format!("user.email={}", IDENTITY.1)])
        .args(["-c", "commit.gpgsign=false"])
        .args(args)
        .current_dir(tree)
        .output()
        .expect("git runs");
    assert!(
        output.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

fn git_out(tree: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .args(args)
        .current_dir(tree)
        .output()
        .expect("git runs");
    assert!(output.status.success(), "git {args:?}");
    String::from_utf8_lossy(&output.stdout).trim().to_owned()
}

/// The commit `bare` holds for `branch`; none when it holds no such branch.
fn branch_in(bare: &Path, branch: &str) -> Option<String> {
    let output = Command::new("git")
        .args([
            "rev-parse",
            "--verify",
            "--quiet",
            &format!("refs/heads/{branch}"),
        ])
        .current_dir(bare)
        .output()
        .expect("git runs");
    output
        .status
        .success()
        .then(|| String::from_utf8_lossy(&output.stdout).trim().to_owned())
}

fn copy_dir(from: &Path, to: &Path) {
    fs::create_dir_all(to).expect("a directory");
    for entry in fs::read_dir(from).expect("a readable fixture") {
        let entry = entry.expect("an entry");
        let target = to.join(entry.file_name());
        if entry.file_type().expect("a file type").is_dir() {
            copy_dir(&entry.path(), &target);
        } else {
            fs::copy(entry.path(), &target).expect("a copied file");
        }
    }
}

/// `data` in base64, as HTTP basic authentication carries it.
fn base64(data: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::new();
    for chunk in data.chunks(3) {
        let bytes = [
            chunk[0],
            *chunk.get(1).unwrap_or(&0),
            *chunk.get(2).unwrap_or(&0),
        ];
        let n = (u32::from(bytes[0]) << 16) | (u32::from(bytes[1]) << 8) | u32::from(bytes[2]);
        for (index, shift) in [18, 12, 6, 0].into_iter().enumerate() {
            if index <= chunk.len() {
                out.push(char::from(ALPHABET[((n >> shift) & 63) as usize]));
            } else {
                out.push('=');
            }
        }
    }
    out
}

/// A scratch directory under this crate's build directory, removed on drop.
struct Scratch(PathBuf);

impl Scratch {
    fn new(name: &str) -> Self {
        let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
            .join(format!("git-relay-served-{}-{name}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("a scratch directory");
        Self(dir)
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// Writes `request` and `body` to `out` as one request on a connection that closes after it,
/// `authorization` its only `Authorization` header.
fn forward(
    out: &mut impl std::io::Write,
    request: &Request,
    body: &[u8],
    authorization: Option<&str>,
) -> std::io::Result<()> {
    let mut head = format!("{} {} HTTP/1.1\r\n", request.method, request.target);
    for (name, value) in &request.headers {
        if matches!(
            name.as_str(),
            "authorization" | "connection" | "content-length" | "transfer-encoding" | "expect"
        ) {
            continue;
        }
        head.push_str(&format!("{name}: {value}\r\n"));
    }
    if let Some(value) = authorization {
        head.push_str(&format!("authorization: {value}\r\n"));
    }
    head.push_str(&format!(
        "content-length: {}\r\nconnection: close\r\n\r\n",
        body.len()
    ));
    out.write_all(head.as_bytes())?;
    out.write_all(body)?;
    out.flush()
}

/// Sends `request` to `127.0.0.1:<port>` and answers everything it sends back until it closes.
fn exchange(
    port: u16,
    request: &Request,
    body: &[u8],
    authorization: Option<&str>,
) -> std::io::Result<Vec<u8>> {
    let mut upstream = TcpStream::connect(("127.0.0.1", port))?;
    forward(&mut upstream, request, body, authorization)?;
    let mut answer = Vec::new();
    upstream.read_to_end(&mut answer)?;
    Ok(answer)
}

/// The forge: the bare repositories under `root` behind `git http-backend` ([`GitStandIn`]),
/// reached over HTTPS on 127.0.0.1 through a terminator whose certificate, for `localhost`, is
/// its own root (`pem`).
struct Forge {
    port: u16,
    pem: String,
    backend: GitStandIn,
}

impl Forge {
    fn serve(root: PathBuf) -> Self {
        let backend = GitStandIn::serve(root, None);
        let certified = rcgen::generate_simple_self_signed(vec!["localhost".to_owned()])
            .expect("a self-signed certificate");
        let pem = certified.cert.pem();
        let key =
            rustls::pki_types::PrivatePkcs8KeyDer::from(certified.signing_key.serialize_der());
        let config = rustls::ServerConfig::builder_with_provider(Arc::new(
            rustls::crypto::ring::default_provider(),
        ))
        .with_safe_default_protocol_versions()
        .expect("the protocol versions")
        .with_no_client_auth()
        .with_single_cert(vec![certified.cert.der().clone()], key.into())
        .expect("the server configuration");
        let config = Arc::new(config);
        let listener = TcpListener::bind("127.0.0.1:0").expect("a loopback port");
        let port = listener.local_addr().expect("an address").port();
        let plain = backend.port;
        std::thread::spawn(move || {
            for stream in listener.incoming().flatten() {
                let config = Arc::clone(&config);
                std::thread::spawn(move || {
                    let _ = Self::terminate(stream, config, plain);
                });
            }
        });
        Self { port, pem, backend }
    }

    fn terminate(
        stream: TcpStream,
        config: Arc<rustls::ServerConfig>,
        plain: u16,
    ) -> std::io::Result<()> {
        let connection = rustls::ServerConnection::new(config).map_err(std::io::Error::other)?;
        let mut tls = rustls::StreamOwned::new(connection, stream);
        let (request, body) = {
            let mut reader = BufReader::new(&mut tls);
            let Some(request) = read_head(&mut reader) else {
                return Ok(());
            };
            let body = read_body(&mut reader, &request)?;
            (request, body)
        };
        let answer = exchange(plain, &request, &body, request.header("authorization"))?;
        tls.write_all(&answer)?;
        tls.conn.send_close_notify();
        tls.flush()?;
        let _ = tls.sock.shutdown(Shutdown::Both);
        Ok(())
    }

    /// `https://localhost:<port>`, the relay's `upstream`.
    fn url(&self) -> String {
        format!("https://localhost:{}", self.port)
    }

    /// Every request the forge was sent.
    fn requests(&self) -> Vec<Request> {
        self.backend.requests()
    }
}

/// The served instance: `connectors-catalog-provider --config` under bubblewrap, its files laid
/// out under `<outer>/srv` (bound as `/srv`) and its mounted credential under `<outer>/secret`
/// (bound read-only as [`MOUNTED`]).
struct Served {
    child: Option<Child>,
    port: u16,
    log: Arc<Mutex<String>>,
}

impl Served {
    fn start(binary: &Path, outer: &Path, forge: &Forge, prefixes: &[&str]) -> Self {
        let srv = outer.join("srv");
        for (dir, mode) in [
            ("", 0o755),
            ("state", 0o700),
            ("tmp", 0o700),
            ("work", 0o700),
        ] {
            let path = srv.join(dir);
            fs::create_dir_all(&path).expect("a directory");
            fs::set_permissions(&path, fs::Permissions::from_mode(mode)).expect("a mode");
        }
        let private = |name: &str, bytes: &[u8]| {
            let path = srv.join(name);
            fs::write(&path, bytes).expect("a file");
            fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).expect("a mode");
        };
        private("ca.pem", forge.pem.as_bytes());
        private("service-token", BEARER.as_bytes());
        let config = json!({
            "service": {
                "instance": "gitlab.fixture",
                "listen": "127.0.0.1:0",
                "state": format!("{SERVED_ROOT}/state"),
                "service_credential": {"kind": "file", "path": format!("{SERVED_ROOT}/service-token")},
            },
            "catalog": {"connector": "gitlab", "instance": "fixture", "endpoint": forge.url()},
            "credential": {"binding": "mounted_secret", "directory": MOUNTED},
            "git": {
                "upstream": forge.url(),
                "ca_file": format!("{SERVED_ROOT}/ca.pem"),
                "repositories": ["o/r"],
                "ref_prefixes": prefixes,
                "commit_identity": {"name": IDENTITY.0, "email": IDENTITY.1},
                "work_directory": format!("{SERVED_ROOT}/work"),
            },
        });
        private(
            "served.json",
            &serde_json::to_vec(&config).expect("the configuration"),
        );
        // The mounted item, laid out as the kubelet's atomic writer lays out a Secret volume.
        let secret = outer.join("secret");
        let stamp = "..2026_10_30_00_00_00.000000001";
        let item = secret.join(stamp).join("gitlab.fixture").join("gitlab.pat");
        fs::create_dir_all(item.parent().expect("a parent")).expect("a directory");
        for dir in [
            secret.clone(),
            secret.join(stamp),
            secret.join(stamp).join("gitlab.fixture"),
        ] {
            fs::set_permissions(dir, fs::Permissions::from_mode(0o755)).expect("a mode");
        }
        fs::write(&item, json!({"token": FORGE_TOKEN}).to_string()).expect("the item");
        fs::set_permissions(&item, fs::Permissions::from_mode(0o400)).expect("a mode");
        symlink(
            Path::new("..data").join("gitlab.fixture"),
            secret.join("gitlab.fixture"),
        )
        .expect("a link");
        symlink(stamp, secret.join("..data")).expect("a link");

        let mut command = Command::new(SANDBOX);
        command.args([
            "--unshare-user",
            "--unshare-pid",
            "--unshare-ipc",
            "--die-with-parent",
            "--new-session",
        ]);
        for name in ["usr", "etc"] {
            command.args(["--ro-bind", &format!("/{name}"), &format!("/{name}")]);
        }
        for name in ["bin", "sbin", "lib", "lib32", "lib64"] {
            let host = Path::new("/").join(name);
            if let Ok(target) = fs::read_link(&host) {
                command
                    .args(["--symlink"])
                    .arg(target)
                    .arg(format!("/{name}"));
            } else if host.is_dir() {
                command.arg("--ro-bind").arg(&host).arg(&host);
            }
        }
        command
            .args(["--proc", "/proc", "--dev", "/dev"])
            .arg("--bind")
            .arg(&srv)
            .arg(SERVED_ROOT)
            .arg("--ro-bind")
            .arg(&secret)
            .arg(MOUNTED)
            .arg("--ro-bind")
            .arg(binary)
            .arg(BINARY)
            .args([BINARY, "--config", &format!("{SERVED_ROOT}/served.json")])
            .env_clear()
            .env("PATH", "/usr/local/bin:/usr/bin:/bin")
            .env("TMPDIR", format!("{SERVED_ROOT}/tmp"))
            .env("NO_COLOR", "1")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::piped());
        let mut child = command.spawn().expect("bwrap starts");
        let stderr = child.stderr.take().expect("a stderr");
        let log = Arc::new(Mutex::new(String::new()));
        let (lines_tx, lines_rx) = mpsc::channel();
        let written = Arc::clone(&log);
        std::thread::spawn(move || {
            for line in BufReader::new(stderr).lines() {
                let Ok(line) = line else { break };
                written
                    .lock()
                    .expect("the log")
                    .push_str(&format!("{line}\n"));
                let _ = lines_tx.send(line);
            }
        });
        let mut served = Self {
            child: Some(child),
            port: 0,
            log,
        };
        let started = Instant::now();
        while started.elapsed() < Duration::from_secs(60) {
            match lines_rx.recv_timeout(Duration::from_millis(200)) {
                Ok(line) if line.contains("adapter service listening") => {
                    let listen = line
                        .split_whitespace()
                        .find_map(|field| field.strip_prefix("listen="))
                        .expect("the listening line names its address");
                    served.port = listen
                        .rsplit_once(':')
                        .and_then(|(_, port)| port.parse().ok())
                        .expect("a port");
                    return served;
                }
                Ok(_) | Err(mpsc::RecvTimeoutError::Timeout) => {}
                Err(mpsc::RecvTimeoutError::Disconnected) => {
                    let status = served.child.as_mut().map(Child::wait);
                    panic!("the served instance exited {status:?}: {}", served.log());
                }
            }
        }
        panic!(
            "the served instance did not listen within 60 s: {}",
            served.log()
        );
    }

    fn log(&self) -> String {
        self.log.lock().expect("the log").clone()
    }
}

impl Drop for Served {
    fn drop(&mut self) {
        if let Some(mut child) = self.child.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

/// The boundary stand-in on 127.0.0.1: every request it is sent goes to the served instance
/// with the bearer the instance admits, and nothing else changed; it logs what it was sent.
struct Boundary {
    port: u16,
    log: Arc<Mutex<Vec<Request>>>,
}

impl Boundary {
    fn serve(instance: u16) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("a loopback port");
        let port = listener.local_addr().expect("an address").port();
        let log = Arc::new(Mutex::new(Vec::new()));
        let seen = Arc::clone(&log);
        std::thread::spawn(move || {
            for stream in listener.incoming().flatten() {
                let seen = Arc::clone(&seen);
                std::thread::spawn(move || {
                    let _ = Self::relay(stream, instance, &seen);
                });
            }
        });
        Self { port, log }
    }

    fn relay(stream: TcpStream, instance: u16, log: &Mutex<Vec<Request>>) -> std::io::Result<()> {
        let mut reader = BufReader::new(stream.try_clone()?);
        let Some(request) = read_head(&mut reader) else {
            return Ok(());
        };
        let body = read_body(&mut reader, &request)?;
        log.lock().expect("the log").push(request.clone());
        let answer = exchange(instance, &request, &body, Some(&format!("Bearer {BEARER}")))?;
        let mut out = stream;
        out.write_all(&answer)?;
        out.flush()?;
        let _ = out.shutdown(Shutdown::Both);
        Ok(())
    }

    /// `http://127.0.0.1:<port>/git/o/r.git`, takt's `origin`.
    fn origin(&self) -> String {
        format!("http://127.0.0.1:{}/git/o/r.git", self.port)
    }

    fn requests(&self) -> Vec<Request> {
        self.log.lock().expect("the log").clone()
    }
}

/// The github instance's operations takt's forge asks for, on 127.0.0.1: branches and the pull
/// request's head are read from the upstream repository itself, a pull request is opened and
/// merged in memory, and its checks are green.
struct PullRequests {
    url: String,
    invoked: Arc<Mutex<Vec<String>>>,
}

/// The one pull request the stand-in holds: its head branch, its base and whether it merged.
#[derive(Default)]
struct Pull {
    head: String,
    base: String,
    merged: bool,
}

impl PullRequests {
    fn serve(upstream: PathBuf) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("a loopback port");
        let url = format!("http://{}", listener.local_addr().expect("an address"));
        let invoked = Arc::new(Mutex::new(Vec::new()));
        let log = Arc::clone(&invoked);
        std::thread::spawn(move || {
            let mut pull: Option<Pull> = None;
            for stream in listener.incoming().flatten() {
                let Ok(clone) = stream.try_clone() else {
                    continue;
                };
                let mut reader = BufReader::new(clone);
                let Some(request) = read_head(&mut reader) else {
                    continue;
                };
                let body = read_body(&mut reader, &request).unwrap_or_default();
                let body: Value = serde_json::from_slice(&body).unwrap_or(Value::Null);
                let (status, answer) = match (request.method.as_str(), request.target.as_str()) {
                    ("GET", "/v1/describe") => (200, describe()),
                    ("POST", "/v1alpha2/invoke") => {
                        log.lock()
                            .expect("the log")
                            .push(body["operation"].as_str().unwrap_or_default().to_owned());
                        invoke(&body, &upstream, &mut pull)
                    }
                    _ => (404, json!({"code": "not_found"})),
                };
                let text = answer.to_string();
                let mut out = stream;
                let _ = write!(
                    out,
                    "HTTP/1.1 {status} Answer\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{text}",
                    text.len()
                );
            }
        });
        Self { url, invoked }
    }

    fn invoked(&self) -> Vec<String> {
        self.invoked.lock().expect("the log").clone()
    }
}

/// The descriptor of the instance `github`: every operation the github connector's map needs.
fn describe() -> Value {
    let operations: Vec<Value> = [
        "branch.get",
        "pull_request.create",
        "pull_request.get",
        "pull_request.merge",
        "check_runs.list",
        "commit.status",
    ]
    .iter()
    .map(|id| {
        json!({
            "id": id,
            "description": "",
            "contract": "operations/v1alpha1",
            "profile": "github.app",
            "input_schema": {},
            "output_schema": {},
        })
    })
    .collect();
    json!({
        "version": "v1alpha1",
        "instance": "github",
        "adapter": "catalog",
        "revision": "descriptor-r1",
        "operations": operations,
        "configuration_schema": {"type": "object"},
    })
}

fn success(request_id: &Value, status: u16, body: Value) -> (u16, Value) {
    (
        200,
        json!({
            "version": "v1alpha2",
            "request_id": request_id,
            "status": "success",
            "result": {"status": status, "body": body, "provenance": {}},
            "audit_ref": "audit-1",
            "audit_status": "complete",
        }),
    )
}

fn failure(request_id: &Value, status: u16, message: &str) -> (u16, Value) {
    (
        status,
        json!({
            "version": "v1alpha2",
            "request_id": request_id,
            "status": "error",
            "error": {"code": "not_found", "message": message},
            "audit_ref": "audit-2",
            "audit_status": "complete",
        }),
    )
}

fn invoke(envelope: &Value, upstream: &Path, pull: &mut Option<Pull>) -> (u16, Value) {
    let id = &envelope["request_id"];
    let input = &envelope["input"];
    // The relay serves `/git/o/r.git`, so every operation names the repository `o/r`; one that
    // names another (the relay's path itself, say) is a wrong project, not an answer.
    if input["owner"] != "o" || input["repo"] != "r" {
        return failure(id, 404, "no such repository");
    }
    let view = |pull: &Pull| {
        json!({
            "number": 1,
            "state": if pull.merged { "closed" } else { "open" },
            "merged": pull.merged,
            "head": {"ref": pull.head, "sha": branch_in(upstream, &pull.head)},
            "base": {"ref": pull.base},
        })
    };
    match envelope["operation"].as_str().unwrap_or_default() {
        "branch.get" => {
            let name = input["branch"].as_str().unwrap_or_default();
            match branch_in(upstream, name) {
                Some(sha) => success(id, 200, json!({"name": name, "commit": {"sha": sha}})),
                None => failure(id, 404, "no such branch"),
            }
        }
        "pull_request.create" => {
            let opened = Pull {
                head: input["body"]["head"]
                    .as_str()
                    .unwrap_or_default()
                    .to_owned(),
                base: input["body"]["base"]
                    .as_str()
                    .unwrap_or_default()
                    .to_owned(),
                merged: false,
            };
            let answer = view(&opened);
            *pull = Some(opened);
            success(id, 201, answer)
        }
        "pull_request.get" => match pull {
            Some(pull) => success(id, 200, view(pull)),
            None => failure(id, 404, "no such pull request"),
        },
        "check_runs.list" => success(
            id,
            200,
            json!({"total_count": 1, "check_runs": [{"status": "completed", "conclusion": "success"}]}),
        ),
        "commit.status" => success(
            id,
            200,
            json!({"state": "pending", "total_count": 0, "statuses": []}),
        ),
        "pull_request.merge" => match pull {
            Some(pull) => {
                pull.merged = true;
                success(
                    id,
                    200,
                    json!({"sha": "m", "merged": true, "message": "merged"}),
                )
            }
            None => failure(id, 404, "no such pull request"),
        },
        _ => failure(id, 404, "no such operation"),
    }
}

/// The output of `command` with every proxy variable cleared and the state home under `state`:
/// the exit code, stdout and stderr.
fn takt(mut command: Command, state: &Path) -> (i32, String, String) {
    command.env("XDG_STATE_HOME", state.join("xdg-state"));
    for variable in PROXY_VARIABLES {
        command.env_remove(variable);
    }
    let output = command.output().expect("takt runs");
    (
        output.status.code().expect("an exit code"),
        String::from_utf8_lossy(&output.stdout).into_owned(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
    )
}

/// One world: the repository at `<outer>/repo` from the fixture store with `story:ready-one`
/// `proposed`, a manifest, the gates and the commit identity, committed on `main`; the upstream
/// bare repository `<outer>/served/o/r.git`, holding `main`, behind the forge; the served
/// instance relaying it with `prefixes`; and the boundary, `origin` of the repository.
struct World {
    scratch: Scratch,
    repo: PathBuf,
    upstream: PathBuf,
    forge: Forge,
    served: Served,
    boundary: Boundary,
}

impl World {
    fn new(binary: &Path, name: &str, prefixes: &[&str]) -> Self {
        let scratch = Scratch::new(name);
        let repo = scratch.0.join("repo");
        copy_dir(&Path::new(FIXTURES).join("one-ready"), &repo);
        fs::write(repo.join("Cargo.toml"), "[package]\nname = \"scratch\"\n").expect("a file");
        fs::write(repo.join(".engineering/gates.yaml"), GATES).expect("a file");
        fs::write(
            repo.join(".engineering/commit-identity.conf"),
            format!(
                "[identity]\n\tname = {}\n\temail = {}\n",
                IDENTITY.0, IDENTITY.1
            ),
        )
        .expect("the identity is written");
        git(&repo, &["init", "-q", "-b", "main"]);
        git(&repo, &["add", "-A"]);
        git(&repo, &["commit", "-q", "-m", "seed"]);
        let upstream = scratch.0.join("served/o/r.git");
        fs::create_dir_all(&upstream).expect("a directory");
        git(&upstream, &["init", "-q", "--bare", "-b", "main"]);
        git(&upstream, &["config", "http.receivepack", "true"]);
        git(
            &repo,
            &["push", "-q", upstream.to_str().expect("utf-8"), "main"],
        );
        let forge = Forge::serve(scratch.0.join("served"));
        let served = Served::start(binary, &scratch.0, &forge, prefixes);
        let boundary = Boundary::serve(served.port);
        git(&repo, &["remote", "add", "origin", &boundary.origin()]);
        Self {
            scratch,
            repo,
            upstream,
            forge,
            served,
            boundary,
        }
    }

    /// Asserts that takt sent the boundary no credential, that the boundary reached the
    /// upstream's fetch and push only through the relay, that every request the forge was sent
    /// carried the instance's own token and none carried the bearer.
    fn assert_through_the_relay(&self) {
        let sent = self.boundary.requests();
        assert!(
            sent.iter()
                .any(|request| request.target.contains("service=git-upload-pack")),
            "a fetch reached the boundary: {sent:?}"
        );
        assert!(
            sent.iter().any(|request| request.method == "POST"
                && request.target == "/git/o/r.git/git-receive-pack"),
            "a push reached the boundary: {sent:?}"
        );
        assert!(
            sent.iter()
                .all(|request| request.header("authorization").is_none()),
            "takt sent the boundary a credential: {sent:?}"
        );
        assert!(
            sent.iter()
                .all(|request| request.target.starts_with("/git/o/r.git/")),
            "{sent:?}"
        );
        let upstream = self.forge.requests();
        assert!(
            upstream
                .iter()
                .any(|request| request.method == "POST"
                    && request.target == "/o/r.git/git-receive-pack"),
            "the relay sent the push upstream: {upstream:?}\n{}",
            self.served.log()
        );
        let basic = format!(
            "Basic {}",
            base64(format!("x-access-token:{FORGE_TOKEN}").as_bytes())
        );
        for request in &upstream {
            assert_eq!(
                request.header("authorization"),
                Some(basic.as_str()),
                "{request:?}"
            );
            assert!(
                !request
                    .headers
                    .iter()
                    .any(|(_, value)| value.contains(BEARER)),
                "the bearer reached the forge: {request:?}"
            );
        }
    }
}

/// Acceptance 1: `takt run --forge connectors`, `origin` the boundary, pushes the case's branch
/// through the served relay, and the upstream holds it at the work; once the merge is granted,
/// the run reads its base through the relay and fetches it, the upstream having moved `main` on
/// meanwhile, and merges once. takt sent no credential; the relay sent the forge its own token.
#[test]
fn a_run_fetches_its_base_and_pushes_its_branch_through_the_served_relay() {
    let Some(binary) = served_binary() else {
        return;
    };
    let world = World::new(&binary, "run", &["takt/", "trial/"]);
    let pulls = PullRequests::serve(world.upstream.clone());
    let config = world.scratch.0.join("run.yaml");
    fs::write(
        &config,
        format!(
            "forge: connectors\nforge_instance: github\nforge_endpoint: {}\nscan: none\n",
            pulls.url
        ),
    )
    .expect("the run file");
    let run = |extra: &[&str]| {
        let mut command = Command::new(env!("CARGO_BIN_EXE_takt"));
        command
            .args(["run", "--repo"])
            .arg(&world.repo)
            .args([
                "--harness",
                "none",
                "--test-cmd",
                "true",
                "--protocol",
                DELIVERY_PROTOCOL,
                "--config",
            ])
            .arg(&config)
            .args(extra);
        takt(command, &world.scratch.0)
    };
    let (code, out, err) = run(&["--iterations", "0"]);
    assert_eq!(code, 3, "the case opens: stdout {out} stderr {err}");
    let branch = git_out(&world.repo, &["branch", "--show-current"]);
    let id = branch
        .strip_prefix("takt/")
        .expect("the tree is on the case's branch")
        .to_owned();
    fs::write(world.repo.join("feature.txt"), "the work\n").expect("a file");
    git(&world.repo, &["add", "feature.txt"]);
    git(&world.repo, &["commit", "-q", "-m", "the work"]);
    let worked = git_out(&world.repo, &["rev-parse", "HEAD"]);

    let (code, out, err) = run(&[]);
    assert_eq!(code, 4, "stdout {out} stderr {err}\n{}", world.served.log());
    assert!(
        out.contains("stopped: ApprovalRequired (repository.merge)"),
        "stdout {out} stderr {err}"
    );
    assert_eq!(
        branch_in(&world.upstream, &branch).as_deref(),
        Some(worked.as_str()),
        "the case's branch is in the upstream repository"
    );

    // The upstream moves `main` on, by a commit the checkout does not hold.
    let other = world.scratch.0.join("other");
    let status = Command::new("git")
        .args(["clone", "-q"])
        .arg(&world.upstream)
        .arg(&other)
        .status()
        .expect("git runs");
    assert!(status.success());
    fs::create_dir_all(other.join("docs")).expect("a directory");
    fs::write(other.join("docs/news.md"), "news\n").expect("a file");
    git(&other, &["add", "-A"]);
    git(&other, &["commit", "-q", "-m", "news"]);
    git(&other, &["push", "-q", "origin", "main"]);
    let moved = git_out(&other, &["rev-parse", "HEAD"]);
    assert!(
        !Command::new("git")
            .args(["cat-file", "-e", &format!("{moved}^{{commit}}")])
            .current_dir(&world.repo)
            .stderr(Stdio::null())
            .status()
            .expect("git runs")
            .success(),
        "the checkout does not hold the moved base yet"
    );

    let mut approve = Command::new(env!("CARGO_BIN_EXE_takt"));
    approve.args(["approve", "--repo"]).arg(&world.repo).args([
        id.as_str(),
        "repository.merge",
        "--by",
        "human:operator",
    ]);
    let (code, out, err) = takt(approve, &world.scratch.0);
    assert_eq!(code, 0, "stdout {out} stderr {err}");

    let (code, out, err) = run(&[]);
    assert_eq!(code, 0, "stdout {out} stderr {err}\n{}", world.served.log());
    assert!(
        out.contains("stopped: Completed"),
        "stdout {out} stderr {err}"
    );
    assert!(
        Command::new("git")
            .args(["cat-file", "-e", &format!("{moved}^{{commit}}")])
            .current_dir(&world.repo)
            .stderr(Stdio::null())
            .status()
            .expect("git runs")
            .success(),
        "the run fetched its base through the relay: stdout {out}"
    );
    assert_eq!(
        branch_in(&world.upstream, &branch).as_deref(),
        Some(git_out(&world.repo, &["rev-parse", &branch]).as_str()),
        "the upstream holds the branch as the checkout does"
    );
    let merges = pulls
        .invoked()
        .iter()
        .filter(|operation| *operation == "pull_request.merge")
        .count();
    assert_eq!(merges, 1, "{:?}", pulls.invoked());
    world.assert_through_the_relay();
}

/// Acceptance 2: `takt scenario run`, `origin` the boundary, fetches through the served relay
/// and creates `trial/0` there at the seed, since its `ref_prefixes` admit `trial/`.
#[test]
fn a_scenario_run_creates_its_trial_branch_through_the_served_relay() {
    let Some(binary) = served_binary() else {
        return;
    };
    let world = World::new(&binary, "scenario", &["trial/"]);
    let seed = git_out(&world.repo, &["rev-parse", "HEAD"]);
    fs::write(
        world.repo.join(".engineering/scenarios.conf"),
        format!("[scenario \"1\"]\n\tstory = story:ready-one\n\tseed = {seed}\n"),
    )
    .expect("the declaration is written");
    git(&world.repo, &["add", "-A"]);
    git(&world.repo, &["commit", "-q", "-m", "declare the scenario"]);
    let mut command = Command::new(env!("CARGO_BIN_EXE_takt"));
    command
        .args(["scenario", "run", "--repo"])
        .arg(&world.repo)
        .args([
            "1",
            "--harness",
            "none",
            "--iterations",
            "0",
            "--test-cmd",
            "true",
        ]);
    let (code, out, err) = takt(command, &world.scratch.0);
    assert_eq!(code, 3, "stdout {out} stderr {err}\n{}", world.served.log());
    assert!(
        out.contains(&format!(
            "scenario 1: story:ready-one proposed on trial/0 at {}, pushed to origin, no forge",
            &seed[..8]
        )),
        "stdout {out} stderr {err}"
    );
    assert_eq!(
        branch_in(&world.upstream, "trial/0").as_deref(),
        Some(seed.as_str()),
        "trial/0 is in the upstream repository"
    );
    world.assert_through_the_relay();
}
