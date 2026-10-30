//! story:git-and-model-through-loopback-relays, acceptance 1: a run whose `origin` is
//! `http://127.0.0.1:<p>/o/r.git` fetches and pushes through it, a smart-HTTP stand-in here
//! (`git http-backend` behind a small server on 127.0.0.1); a run whose `origin`, push URL or
//! rewritten URL is `http://` to any other host is refused before anything is fetched, naming the
//! URL; and a loopback origin that answers the push with a redirect to another host does not
//! carry the push there. The run is `takt scenario run`, whose fetch and push are takt's own.
//!
//! Nothing leaves the machine: a run that might reach `example.com` is given a proxy stand-in on
//! 127.0.0.1 that records every request it is sent and answers 502, so a request git makes off
//! the machine is seen here and goes no further.
//!
//! Acceptance 2 and 4: `takt run --harness harness --model-endpoint http://127.0.0.1:<p>/v1` with
//! no credential flag starts its session, the session executor handing b10x-harness no
//! credential flag, and no variable of the harness's environment holds a value the stand-ins were
//! given as a token, though takt's own environment holds each of them under a credential's name.
//! A `b10x-harness` double records what it was started with. That the real b10x-harness, started
//! so, sends the model stand-in no `Authorization` and no `x-api-key` header is the one case here
//! that needs the real program, and is ignored unless asked for (`--ignored`).

#[path = "../../takt/tests/support/own_home.rs"]
mod own_home;

use std::fs;
use std::io::{BufRead as _, BufReader, Read as _, Write as _};
use std::net::{TcpListener, TcpStream};
use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::sync::{Arc, Mutex};

/// The fixture stores of the library crate, one directory each.
const FIXTURES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../takt/tests/fixtures");

/// A gates document whose one assertion is true in a repository with a manifest.
const GATES: &str = "format: engineering-gates/1\nlanguage: canon-expr/1\ngates:\n  - id: manifest\n    \
                     text: The repository has its manifest.\n    assertions:\n      \
                     - 'fs.file.exists(\"Cargo.toml\")'\n";

/// The proxy variables a git child reads; every run here sets them or clears them.
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

fn git(tree: &Path, args: &[&str]) {
    let status = Command::new("git")
        .args(["-c", "user.name=t", "-c", "user.email=t@localhost"])
        .args(["-c", "commit.gpgsign=false"])
        .args(args)
        .current_dir(tree)
        .status()
        .expect("git runs");
    assert!(status.success(), "git {args:?}");
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

/// One request a stand-in was sent: its method, its target and its headers, each name in lower
/// case.
#[derive(Debug, Clone)]
struct Request {
    method: String,
    target: String,
    headers: Vec<(String, String)>,
}

impl Request {
    fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(key, _)| key == name)
            .map(|(_, value)| value.as_str())
    }
}

/// Reads one request's line and headers from `reader`; none at the end of the stream.
fn read_head(reader: &mut BufReader<TcpStream>) -> Option<Request> {
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
fn read_body(reader: &mut BufReader<TcpStream>, request: &Request) -> std::io::Result<Vec<u8>> {
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

/// A smart-HTTP stand-in on 127.0.0.1: the bare repositories under its root, served by
/// `git http-backend`; with a redirect, every receive-pack request (the push) is answered with a
/// 302 to the same path under that base instead. It logs every request it is sent.
struct GitStandIn {
    port: u16,
    log: Arc<Mutex<Vec<Request>>>,
}

impl GitStandIn {
    fn serve(root: PathBuf, redirect: Option<String>) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("a loopback port");
        let port = listener.local_addr().expect("an address").port();
        let log = Arc::new(Mutex::new(Vec::new()));
        let seen = Arc::clone(&log);
        std::thread::spawn(move || {
            for stream in listener.incoming().flatten() {
                let _ = Self::answer(stream, &root, redirect.as_deref(), &seen);
            }
        });
        Self { port, log }
    }

    fn answer(
        stream: TcpStream,
        root: &Path,
        redirect: Option<&str>,
        log: &Mutex<Vec<Request>>,
    ) -> std::io::Result<()> {
        let mut reader = BufReader::new(stream.try_clone()?);
        let Some(request) = read_head(&mut reader) else {
            return Ok(());
        };
        let body = read_body(&mut reader, &request)?;
        log.lock().expect("the log").push(request.clone());
        let mut out = stream;
        if let Some(base) = redirect
            && request.target.contains("git-receive-pack")
        {
            return write!(
                out,
                "HTTP/1.1 302 Found\r\nLocation: {base}{}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
                request.target
            );
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

    fn requests(&self) -> Vec<Request> {
        self.log.lock().expect("the log").clone()
    }

    fn url(&self, path: &str) -> String {
        format!("http://127.0.0.1:{}/{path}", self.port)
    }
}

/// A proxy stand-in on 127.0.0.1: it records the first line of every request it is sent
/// (`GET http://example.com/… HTTP/1.1`, `CONNECT example.com:443 HTTP/1.1`) and answers 502, so
/// a git told to use it reaches nothing beyond this machine.
struct ProxyRecorder {
    port: u16,
    seen: Arc<Mutex<Vec<String>>>,
}

impl ProxyRecorder {
    fn start() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("a loopback port");
        let port = listener.local_addr().expect("an address").port();
        let seen = Arc::new(Mutex::new(Vec::new()));
        let log = Arc::clone(&seen);
        std::thread::spawn(move || {
            for stream in listener.incoming().flatten() {
                let Ok(clone) = stream.try_clone() else {
                    continue;
                };
                let mut reader = BufReader::new(clone);
                if let Some(request) = read_head(&mut reader) {
                    log.lock()
                        .expect("the log")
                        .push(format!("{} {}", request.method, request.target));
                }
                let mut out = stream;
                let _ = out.write_all(
                    b"HTTP/1.1 502 Bad Gateway\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
                );
            }
        });
        Self { port, seen }
    }

    fn url(&self) -> String {
        format!("http://127.0.0.1:{}", self.port)
    }

    fn seen(&self) -> Vec<String> {
        self.seen.lock().expect("the log").clone()
    }
}

/// A scratch directory under this crate's build directory, removed on drop.
struct Scratch(PathBuf);

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// One scratch world: the repository at `repo`, whose seed commit holds the fixture store with
/// `story:ready-one` `proposed`, and `main` past it with scenario 1 declared and a commit
/// identity configured; and the bare repository `served`, `o/r.git` under the stand-in's root,
/// which takes a push.
struct World {
    _scratch: Scratch,
    repo: PathBuf,
    root: PathBuf,
    served: PathBuf,
    state: PathBuf,
    seed: String,
}

fn world(name: &str) -> World {
    let outer = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("loopback-relays-{}-{name}", std::process::id()));
    if outer.exists() {
        fs::remove_dir_all(&outer).expect("a stale scratch tree is removed");
    }
    let repo = outer.join("repo");
    copy_dir(&Path::new(FIXTURES).join("one-ready"), &repo);
    fs::write(repo.join("Cargo.toml"), "[package]\nname = \"scratch\"\n").expect("a file");
    fs::write(repo.join(".engineering/gates.yaml"), GATES).expect("a file");
    git(&repo, &["init", "-q", "-b", "main"]);
    git(&repo, &["add", "-A"]);
    git(&repo, &["commit", "-q", "-m", "seed"]);
    let seed = git_out(&repo, &["rev-parse", "HEAD"]);
    fs::write(
        repo.join(".engineering/scenarios.conf"),
        format!("[scenario \"1\"]\n\tstory = story:ready-one\n\tseed = {seed}\n"),
    )
    .expect("the declaration is written");
    fs::write(
        repo.join(".engineering/commit-identity.conf"),
        "[identity]\n\tname = Test Bot\n\temail = bot@example.invalid\n",
    )
    .expect("the identity is written");
    git(&repo, &["add", "-A"]);
    git(&repo, &["commit", "-q", "-m", "declare the scenario"]);
    let root = outer.join("served");
    let served = root.join("o/r.git");
    fs::create_dir_all(&served).expect("a directory");
    git(&served, &["init", "-q", "--bare", "-b", "main"]);
    git(&served, &["config", "http.receivepack", "true"]);
    let state = outer.join("state");
    fs::create_dir_all(&state).expect("a state directory");
    World {
        _scratch: Scratch(outer),
        repo,
        root,
        served,
        state,
        seed,
    }
}

impl World {
    /// `takt scenario run --repo <repo> 1 --harness none --iterations 0 --test-cmd true`, with
    /// every proxy variable cleared and then `env` set. The exit code, stdout and stderr.
    fn scenario_run(&self, env: &[(&str, &str)]) -> (i32, String, String) {
        let mut command = Command::new(env!("CARGO_BIN_EXE_takt"));
        command
            .args([
                "scenario",
                "run",
                "--repo",
                self.repo.to_str().expect("utf-8"),
                "1",
                "--harness",
                "none",
                "--iterations",
                "0",
                "--test-cmd",
                "true",
            ])
            .env("XDG_STATE_HOME", self.state.join("xdg-state"));
        for variable in PROXY_VARIABLES {
            command.env_remove(variable);
        }
        let output = command
            .envs(env.iter().copied())
            .output()
            .expect("takt runs");
        (
            output.status.code().expect("an exit code"),
            String::from_utf8_lossy(&output.stdout).into_owned(),
            String::from_utf8_lossy(&output.stderr).into_owned(),
        )
    }

    /// The commit the served repository holds for `branch`; none when it holds no such branch.
    fn served_branch(&self, branch: &str) -> Option<String> {
        let output = Command::new("git")
            .args([
                "rev-parse",
                "--verify",
                "--quiet",
                &format!("refs/heads/{branch}"),
            ])
            .current_dir(&self.served)
            .output()
            .expect("git runs");
        output
            .status
            .success()
            .then(|| String::from_utf8_lossy(&output.stdout).trim().to_owned())
    }

    /// The local branches named `trial/…`.
    fn trial_branches(&self) -> String {
        git_out(
            &self.repo,
            &[
                "for-each-ref",
                "--format=%(refname:short)",
                "refs/heads/trial/",
            ],
        )
    }

    /// Asserts the run changed nothing: no trial branch, the tree on `main` at `head`, no case.
    fn unchanged(&self, head: &str) {
        assert_eq!(self.trial_branches(), "");
        assert_eq!(git_out(&self.repo, &["branch", "--show-current"]), "main");
        assert_eq!(git_out(&self.repo, &["rev-parse", "HEAD"]), head);
        assert!(!self.repo.join(".takt/cases").exists());
        assert_eq!(self.served_branch("trial/0"), None);
    }
}

/// Acceptance 1: with `origin` `http://127.0.0.1:<p>/o/r.git` served by the stand-in, the run
/// fetches `origin`'s trial branches and pushes `trial/0` through it, at the seed; the stand-in
/// is sent no `Authorization` header, since takt holds no credential for it.
#[test]
fn a_loopback_origin_is_fetched_from_and_pushed_to() {
    let world = world("fetch-and-push");
    let standin = GitStandIn::serve(world.root.clone(), None);
    git(
        &world.repo,
        &["remote", "add", "origin", &standin.url("o/r.git")],
    );
    let (code, out, err) = world.scenario_run(&[]);
    assert_eq!(code, 3, "stdout {out} stderr {err}");
    assert!(
        out.contains(&format!(
            "scenario 1: story:ready-one proposed on trial/0 at {}, pushed to origin, no forge",
            &world.seed[..8]
        )),
        "stdout {out} stderr {err}"
    );
    assert_eq!(
        world.served_branch("trial/0").as_deref(),
        Some(world.seed.as_str())
    );
    let requests = standin.requests();
    assert!(
        requests
            .iter()
            .any(|request| request.target.contains("service=git-upload-pack")),
        "the fetch reached the stand-in: {requests:?}"
    );
    assert!(
        requests.iter().any(
            |request| request.method == "POST" && request.target == "/o/r.git/git-receive-pack"
        ),
        "the push reached the stand-in: {requests:?}"
    );
    assert!(
        requests
            .iter()
            .all(|request| request.header("authorization").is_none()),
        "{requests:?}"
    );
}

/// Acceptance 1's refusal: with `origin` `http://example.com/o/r.git` the run refuses the fetch
/// before git runs, naming the URL and that its host is no loopback host; nothing reaches the
/// proxy, and nothing is changed.
#[test]
fn an_http_origin_on_another_host_is_refused_before_the_fetch() {
    let world = world("other-host");
    let proxy = ProxyRecorder::start();
    git(
        &world.repo,
        &["remote", "add", "origin", "http://example.com/o/r.git"],
    );
    let head = git_out(&world.repo, &["rev-parse", "HEAD"]);
    let (code, out, err) =
        world.scenario_run(&[("http_proxy", &proxy.url()), ("https_proxy", &proxy.url())]);
    assert_eq!(code, 1, "stdout {out} stderr {err}");
    assert!(
        err.contains("fetch the trial branches of origin; nothing was changed")
            && err.contains("http://example.com/o/r.git")
            && err.contains("no loopback host"),
        "stderr {err}"
    );
    assert_eq!(proxy.seen(), Vec::<String>::new());
    world.unchanged(&head);
}

/// The same rule for every URL git would reach `origin` at: a push URL that is `http://` to
/// another host is refused before the fetch from a loopback `url`, naming the push URL.
#[test]
fn an_http_push_url_on_another_host_is_refused_before_the_fetch() {
    let world = world("push-url");
    let standin = GitStandIn::serve(world.root.clone(), None);
    let proxy = ProxyRecorder::start();
    git(
        &world.repo,
        &["remote", "add", "origin", &standin.url("o/r.git")],
    );
    git(
        &world.repo,
        &[
            "config",
            "remote.origin.pushurl",
            "http://example.com/o/r.git",
        ],
    );
    let head = git_out(&world.repo, &["rev-parse", "HEAD"]);
    let (code, out, err) =
        world.scenario_run(&[("http_proxy", &proxy.url()), ("no_proxy", "127.0.0.1")]);
    assert_eq!(code, 1, "stdout {out} stderr {err}");
    assert!(
        err.contains("http://example.com/o/r.git") && err.contains("no loopback host"),
        "stderr {err}"
    );
    assert_eq!(proxy.seen(), Vec::<String>::new());
    assert!(standin.requests().is_empty(), "{:?}", standin.requests());
    world.unchanged(&head);
}

/// And for the URL as git rewrites it: a loopback `origin` that the repository's configuration
/// rewrites (`url.<base>.insteadOf`) to `http://` on another host is refused, naming the URL git
/// would reach.
#[test]
fn a_loopback_origin_rewritten_to_another_host_is_refused() {
    let world = world("rewritten");
    let standin = GitStandIn::serve(world.root.clone(), None);
    let proxy = ProxyRecorder::start();
    git(
        &world.repo,
        &["remote", "add", "origin", &standin.url("o/r.git")],
    );
    git(
        &world.repo,
        &[
            "config",
            "url.http://example.com/.insteadOf",
            &standin.url(""),
        ],
    );
    let head = git_out(&world.repo, &["rev-parse", "HEAD"]);
    let (code, out, err) =
        world.scenario_run(&[("http_proxy", &proxy.url()), ("no_proxy", "127.0.0.1")]);
    assert_eq!(code, 1, "stdout {out} stderr {err}");
    assert!(
        err.contains("http://example.com/o/r.git") && err.contains("no loopback host"),
        "stderr {err}"
    );
    assert_eq!(proxy.seen(), Vec::<String>::new());
    world.unchanged(&head);
}

/// A loopback origin that answers the push with a redirect to another host does not carry the
/// push there: git follows no redirect from a loopback relay (`http.followRedirects=false`), the
/// push fails, `trial/0` is created nowhere, and the proxy is sent nothing.
#[test]
fn a_redirect_from_a_loopback_origin_does_not_carry_the_push() {
    let world = world("redirect");
    let standin = GitStandIn::serve(world.root.clone(), Some("http://example.com".to_owned()));
    let proxy = ProxyRecorder::start();
    git(
        &world.repo,
        &["remote", "add", "origin", &standin.url("o/r.git")],
    );
    let head = git_out(&world.repo, &["rev-parse", "HEAD"]);
    let (code, out, err) =
        world.scenario_run(&[("http_proxy", &proxy.url()), ("no_proxy", "127.0.0.1")]);
    assert_eq!(code, 1, "stdout {out} stderr {err}");
    assert!(err.contains("trial/0 was not created"), "stderr {err}");
    assert_eq!(proxy.seen(), Vec::<String>::new());
    assert!(
        standin
            .requests()
            .iter()
            .any(|request| request.target.contains("service=git-receive-pack")),
        "the push was tried at the stand-in: {:?}",
        standin.requests()
    );
    world.unchanged(&head);
}

/// The token the model stand-in was given, as the relay beside a run holds the model's key.
const MODEL_RELAY_TOKEN: &str = concat!("model-relay-", "stand-in-", "value-7c41e2");
/// The token the git stand-in was given, as the git relay holds the forge's.
const GIT_RELAY_TOKEN: &str = concat!("git-relay-", "stand-in-", "value-90b3d8");

/// The variables `takt`'s own environment holds a stand-in's token under in these runs, each a
/// name a model client or git reads a credential from. None of the values may reach the harness.
const CREDENTIAL_VARIABLES: [(&str, &str); 7] = [
    ("ANTHROPIC_API_KEY", MODEL_RELAY_TOKEN),
    ("ANTHROPIC_AUTH_TOKEN", MODEL_RELAY_TOKEN),
    ("OPENAI_API_KEY", MODEL_RELAY_TOKEN),
    ("CLAUDE_CODE_OAUTH_TOKEN", MODEL_RELAY_TOKEN),
    ("GITHUB_TOKEN", GIT_RELAY_TOKEN),
    ("GH_TOKEN", GIT_RELAY_TOKEN),
    ("GIT_PASSWORD", GIT_RELAY_TOKEN),
];

/// Every flag by which the session executor can hand b10x-harness a credential.
const CREDENTIAL_FLAGS: [&str; 6] = [
    "--api-key-file",
    "--api-key-env",
    "--api-key-secret",
    "--oauth-token-file",
    "--oauth-token-env",
    "--oauth-token-pointer",
];

/// A model endpoint stand-in on 127.0.0.1, as the relay beside a run is: it logs every request it
/// is sent with its headers, and answers each with a 400 and an error body of the messages wire,
/// so a session that reaches it ends there.
struct ModelStandIn {
    port: u16,
    log: Arc<Mutex<Vec<Request>>>,
}

impl ModelStandIn {
    fn serve() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("a loopback port");
        let port = listener.local_addr().expect("an address").port();
        let log = Arc::new(Mutex::new(Vec::new()));
        let seen = Arc::clone(&log);
        std::thread::spawn(move || {
            for stream in listener.incoming().flatten() {
                let Ok(clone) = stream.try_clone() else {
                    continue;
                };
                let mut reader = BufReader::new(clone);
                let Some(request) = read_head(&mut reader) else {
                    continue;
                };
                let _ = read_body(&mut reader, &request);
                seen.lock().expect("the log").push(request);
                let body = r#"{"type":"error","error":{"type":"invalid_request_error","message":"a stand-in"}}"#;
                let mut out = stream;
                let _ = write!(
                    out,
                    "HTTP/1.1 400 Bad Request\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
            }
        });
        Self { port, log }
    }

    fn base_url(&self) -> String {
        format!("http://127.0.0.1:{}/v1", self.port)
    }

    fn requests(&self) -> Vec<Request> {
        self.log.lock().expect("the log").clone()
    }
}

/// The program the session executor starts as b10x-harness in a [`HarnessWorld`].
enum Harness {
    /// A double that answers `--version` and otherwise writes its environment and its arguments
    /// to `started` and exits 1, so `started` holds the session's call, not the version probe.
    Double,
    /// The real b10x-harness at this path, linked in.
    Real(PathBuf),
}

/// One scratch world for `takt run --harness harness`: a repository with one commit whose
/// `origin` is the git stand-in, a HOME whose `.local/bin` (where the session executor finds it)
/// holds b10x-harness, a runtime directory of mode 0700, and the model and git stand-ins.
struct HarnessWorld {
    scratch: Scratch,
    model: ModelStandIn,
    _git: GitStandIn,
}

impl HarnessWorld {
    fn new(name: &str, harness: &Harness) -> Self {
        let outer = Path::new(env!("CARGO_TARGET_TMPDIR"))
            .join(format!("loopback-model-{}-{name}", std::process::id()));
        if outer.exists() {
            fs::remove_dir_all(&outer).expect("a stale scratch tree is removed");
        }
        let repo = outer.join("repo");
        let program = outer.join("home/.local/bin/b10x-harness");
        let runtime = outer.join("runtime");
        let served = outer.join("served/o/r.git");
        for dir in [
            &repo,
            program.parent().expect("a parent"),
            &runtime,
            &served,
        ] {
            fs::create_dir_all(dir).expect("a directory");
        }
        fs::set_permissions(&runtime, fs::Permissions::from_mode(0o700))
            .expect("a private runtime directory");
        git(&served, &["init", "-q", "--bare", "-b", "main"]);
        let git_standin = GitStandIn::serve(outer.join("served"), None);
        fs::write(repo.join("README.md"), "a story\n").expect("a file");
        git(&repo, &["init", "-q", "-b", "main"]);
        git(&repo, &["add", "-A"]);
        git(&repo, &["commit", "-q", "-m", "start"]);
        git(
            &repo,
            &["remote", "add", "origin", &git_standin.url("o/r.git")],
        );
        match harness {
            Harness::Double => {
                fs::write(
                    &program,
                    format!(
                        "#!/bin/sh\nif [ \"$1\" = --version ]; then echo b10x-harness 0.22.0; exit 0; fi\nexec > '{}' 2>&1\nenv\necho '-- args'\nfor arg in \"$@\"; do echo \"$arg\"; done\necho '-- end'\nexit 1\n",
                        outer.join("started").display()
                    ),
                )
                .expect("a double");
                fs::set_permissions(&program, fs::Permissions::from_mode(0o755)).expect("exec bit");
            }
            Harness::Real(path) => {
                std::os::unix::fs::symlink(path, &program).expect("the real b10x-harness linked");
            }
        }
        Self {
            scratch: Scratch(outer),
            model: ModelStandIn::serve(),
            _git: git_standin,
        }
    }

    fn outer(&self) -> &Path {
        &self.scratch.0
    }

    /// `takt run --harness harness --model-endpoint <the model stand-in> --iterations 2` (step 1
    /// is `tests.run`, step 2 the judgement step whose session starts b10x-harness), with no
    /// credential flag, `--test-cmd true` (the scratch repository lies under this workspace's
    /// build directory, where the default `cargo test` would find this workspace), `--forge none`
    /// and a run file declaring `scan: none` (a harness run opens
    /// only under a declared scan adapter); HOME, the state and the runtime directory the scratch
    /// ones, no session bus, every proxy variable cleared, and each of [`CREDENTIAL_VARIABLES`]
    /// set to its stand-in's token.
    fn run(&self) -> Output {
        let scan = self.outer().join("scan-none.yaml");
        fs::write(&scan, "scan: none\n").expect("a run configuration file");
        let mut command = Command::new(env!("CARGO_BIN_EXE_takt"));
        command
            .args([
                "run",
                "--repo",
                self.outer().join("repo").to_str().expect("utf-8"),
                "--intent",
                "probe",
                "--forge",
                "none",
                "--harness",
                "harness",
                "--model-endpoint",
                &self.model.base_url(),
                "--iterations",
                "2",
                "--test-cmd",
                "true",
                "--config",
                scan.to_str().expect("utf-8"),
            ])
            .env("HOME", self.outer().join("home"))
            .env("XDG_STATE_HOME", self.outer().join("xdg-state"))
            .env("XDG_RUNTIME_DIR", self.outer().join("runtime"))
            .env_remove("DBUS_SESSION_BUS_ADDRESS");
        for variable in PROXY_VARIABLES {
            command.env_remove(variable);
        }
        command
            .envs(CREDENTIAL_VARIABLES)
            .output()
            .expect("takt runs")
    }

    /// What the double wrote: its environment, and its arguments one per line. Panics, naming
    /// the run's output, when the double was never started.
    fn started(&self, output: &Output) -> (String, Vec<String>) {
        let (stdout, stderr) = (
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr),
        );
        let text = fs::read_to_string(self.outer().join("started")).unwrap_or_else(|error| {
            panic!("b10x-harness was never started ({error}):\nstdout {stdout}\nstderr {stderr}")
        });
        let (environment, rest) = text
            .split_once("-- args\n")
            .expect("the double listed its arguments");
        let args = rest
            .split_once("-- end")
            .map_or(rest, |(args, _)| args)
            .lines()
            .map(str::to_owned)
            .collect();
        (environment.to_owned(), args)
    }

    /// The `opened` line of the run's one case.
    fn opened(&self) -> serde_json::Value {
        let cases = self.outer().join("repo/.takt/cases");
        let mut events = Vec::new();
        let mut dirs = vec![cases];
        while let Some(dir) = dirs.pop() {
            for entry in fs::read_dir(&dir).into_iter().flatten().flatten() {
                let path = entry.path();
                if path.is_dir() {
                    dirs.push(path);
                } else if path.file_name().is_some_and(|name| name == "events.jsonl") {
                    events.push(path);
                }
            }
        }
        assert_eq!(events.len(), 1, "one case record: {events:?}");
        fs::read_to_string(&events[0])
            .expect("the run record")
            .lines()
            .map(|line| serde_json::from_str::<serde_json::Value>(line).expect("a JSON line"))
            .find(|line| line["event"] == "opened")
            .expect("an opened line")
    }
}

/// Acceptance 2, as far as takt and the session executor go: `--harness harness --model-endpoint
/// http://127.0.0.1:<p>/v1` with no credential flag starts its session, b10x-harness pointed at
/// the stand-in and handed no credential flag; the case records the credential as `none`.
#[test]
fn a_keyless_loopback_endpoint_starts_its_session_with_no_credential() {
    let world = HarnessWorld::new("keyless-starts", &Harness::Double);
    let output = world.run();
    let (_, args) = world.started(&output);
    let base_url = world.model.base_url();
    assert!(
        args.windows(2)
            .any(|pair| pair[0] == "--base-url" && pair[1] == base_url),
        "b10x-harness is pointed at the stand-in: {args:?}"
    );
    for flag in CREDENTIAL_FLAGS {
        assert!(
            !args
                .iter()
                .any(|arg| arg == flag || arg.starts_with(&format!("{flag}="))),
            "b10x-harness was handed {flag}: {args:?}"
        );
    }
    assert_eq!(
        world.opened()["endpoint"]["credential"],
        serde_json::json!({"kind": "none"})
    );
}

/// Acceptance 4: no variable of the harness's environment holds a value the stand-ins were given
/// as a token, though takt's own environment holds each of them under a credential's name; and
/// no argument holds one either.
#[test]
fn no_variable_of_the_harness_environment_holds_a_stand_in_token() {
    let world = HarnessWorld::new("keyless-environment", &Harness::Double);
    let output = world.run();
    let (environment, args) = world.started(&output);
    assert!(
        environment.lines().any(|line| line.starts_with("PATH=")),
        "the double wrote its environment:\n{environment}"
    );
    for token in [MODEL_RELAY_TOKEN, GIT_RELAY_TOKEN] {
        let holding: Vec<&str> = environment
            .lines()
            .filter(|line| line.contains(token))
            .collect();
        assert!(
            holding.is_empty(),
            "the harness's environment holds a stand-in's token: {holding:?}"
        );
        assert!(
            !args.iter().any(|arg| arg.contains(token)),
            "an argument holds a stand-in's token: {args:?}"
        );
    }
}

/// The real b10x-harness on the program path (or at `TAKT_TEST_B10X_HARNESS`); none is a failure,
/// not a pass.
fn real_harness() -> PathBuf {
    if let Some(path) = std::env::var_os("TAKT_TEST_B10X_HARNESS") {
        return PathBuf::from(path);
    }
    std::env::split_paths(&std::env::var_os("PATH").unwrap_or_default())
        .map(|dir| dir.join("b10x-harness"))
        .find(|path| path.is_file())
        .expect("a real b10x-harness on the program path or at TAKT_TEST_B10X_HARNESS")
}

/// Acceptance 2 on the wire: the real b10x-harness, started by the session as above, reaches the
/// model stand-in, and no request it sends carries an `Authorization` or an `x-api-key` header, or
/// any header holding a stand-in's token. It needs the real program, which CI does not install.
#[test]
#[ignore = "needs a real b10x-harness on the program path or at TAKT_TEST_B10X_HARNESS"]
fn the_real_harness_sends_the_stand_in_no_credential_header() {
    let world = HarnessWorld::new("keyless-wire", &Harness::Real(real_harness()));
    let output = world.run();
    let requests = world.model.requests();
    assert!(
        !requests.is_empty(),
        "the session reached the stand-in:\nstdout {}\nstderr {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    for request in &requests {
        assert_eq!(request.header("authorization"), None, "{request:?}");
        assert_eq!(request.header("x-api-key"), None, "{request:?}");
        for token in [MODEL_RELAY_TOKEN, GIT_RELAY_TOKEN] {
            assert!(
                !request
                    .headers
                    .iter()
                    .any(|(_, value)| value.contains(token)),
                "{request:?}"
            );
        }
    }
}
