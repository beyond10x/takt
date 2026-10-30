//! Wave 47, adversarial pass on story:takt-carries-no-organisations-own-route and
//! story:git-and-model-through-loopback-relays, at the binary: `takt scenario run`'s trial
//! route to an `origin` that is a loopback relay, and the removed bot value of `--forge`.
//!
//! The rule under attack (`crates/takt-cli/src/main.rs`, `TrialRoute`): "Plain `http://` goes
//! to a loopback host alone, a relay that holds the forge's credential: every URL git reaches
//! `origin` at is read before the fetch, and one that is `http://` to another host is refused".
//!
//! Nothing leaves the machine: the origin is a smart-HTTP stand-in on 127.0.0.1 (`git
//! http-backend`), and a request git would make elsewhere goes to a proxy stand-in on 127.0.0.1
//! that records its request line and answers 502.

#[path = "../../takt/tests/support/own_home.rs"]
mod own_home;

use std::fs;
use std::io::{BufRead as _, BufReader, Read as _, Write as _};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};

const FIXTURES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../takt/tests/fixtures");

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

/// The variables that hand git configuration through the environment; every run here clears
/// them before it sets its own.
const CONFIG_VARIABLES: [&str; 2] = ["GIT_CONFIG_PARAMETERS", "GIT_CONFIG_COUNT"];

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

/// A smart-HTTP stand-in on 127.0.0.1 serving the bare repositories under its root.
struct GitStandIn {
    port: u16,
    log: Arc<Mutex<Vec<Request>>>,
}

impl GitStandIn {
    fn serve(root: PathBuf) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("a loopback port");
        let port = listener.local_addr().expect("an address").port();
        let log = Arc::new(Mutex::new(Vec::new()));
        let seen = Arc::clone(&log);
        std::thread::spawn(move || {
            for stream in listener.incoming().flatten() {
                let _ = Self::answer(stream, &root, &seen);
            }
        });
        Self { port, log }
    }

    fn answer(stream: TcpStream, root: &Path, log: &Mutex<Vec<Request>>) -> std::io::Result<()> {
        let mut reader = BufReader::new(stream.try_clone()?);
        let Some(request) = read_head(&mut reader) else {
            return Ok(());
        };
        let body = read_body(&mut reader, &request)?;
        log.lock().expect("the log").push(request.clone());
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
        let mut out = stream;
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

struct Scratch(PathBuf);

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// The repository at `repo` with scenario 1 declared on `main` over a seed holding the fixture
/// store with `story:ready-one` `proposed`, and the bare `o/r.git` under `root` that the
/// stand-in serves.
struct World {
    _scratch: Scratch,
    repo: PathBuf,
    root: PathBuf,
    served: PathBuf,
    state: PathBuf,
}

fn world(name: &str) -> World {
    let outer = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("w47-adv-trial-{}-{name}", std::process::id()));
    if outer.exists() {
        fs::remove_dir_all(&outer).expect("a stale scratch tree is removed");
    }
    let repo = outer.join("repo");
    copy_dir(&Path::new(FIXTURES).join("one-ready"), &repo);
    fs::write(repo.join("Cargo.toml"), "[package]\nname = \"scratch\"\n").expect("a file");
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
    }
}

impl World {
    /// `takt scenario run --repo <repo> 1 --harness none --iterations 0 --test-cmd true`, every
    /// proxy and configuration variable cleared and then `env` set.
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
        for variable in PROXY_VARIABLES.iter().chain(CONFIG_VARIABLES.iter()) {
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
}

/// An operator whose environment names a proxy (`http_proxy`, as a pod or a corporate host
/// often does) and no `no_proxy` for 127.0.0.1: the trial fetch to the loopback relay goes to
/// that proxy, in the clear, and never reaches the relay. The connectors forge's served calls
/// turn proxies off for a loopback endpoint; the trial route's git lines do not.
#[test]
fn a_proxy_in_the_environment_does_not_carry_a_relayed_trial_fetch() {
    let world = world("proxy");
    let standin = GitStandIn::serve(world.root.clone());
    let proxy = ProxyRecorder::start();
    git(
        &world.repo,
        &["remote", "add", "origin", &standin.url("o/r.git")],
    );
    let (code, out, err) = world.scenario_run(&[("http_proxy", &proxy.url())]);
    assert_eq!(
        proxy.seen(),
        Vec::<String>::new(),
        "the relayed trial line went through the proxy; exit {code}, stdout {out} stderr {err}"
    );
    assert!(
        !standin.requests().is_empty(),
        "the relay was reached; exit {code}, stderr {err}"
    );
}

/// The check reads `origin`'s URLs through takt's own git, which drops `GIT_CONFIG_COUNT` and
/// `GIT_CONFIG_PARAMETERS` from its environment; the fetch and the push run as the operator's
/// git, which keeps them. A rewrite handed through the environment (as `git -c` hands one to a
/// program a git alias or hook starts) is applied by the fetch and never seen by the check:
/// the fetch goes to `http://` on another host.
#[test]
fn a_rewrite_given_in_the_environment_is_read_by_the_check_as_by_the_fetch() {
    let world = world("env-rewrite");
    let standin = GitStandIn::serve(world.root.clone());
    let proxy = ProxyRecorder::start();
    git(
        &world.repo,
        &["remote", "add", "origin", &standin.url("o/r.git")],
    );
    let base = standin.url("");
    let (code, out, err) = world.scenario_run(&[
        ("http_proxy", &proxy.url()),
        ("no_proxy", "127.0.0.1"),
        ("GIT_CONFIG_COUNT", "1"),
        ("GIT_CONFIG_KEY_0", "url.http://example.com/.insteadOf"),
        ("GIT_CONFIG_VALUE_0", &base),
    ]);
    assert_eq!(
        proxy.seen(),
        Vec::<String>::new(),
        "the trial fetch went to http://example.com; exit {code}, stdout {out} stderr {err}"
    );
    assert_eq!(world.served_branch("trial/0"), None, "stderr {err}");
}

/// Holds (story:takt-carries-no-organisations-own-route, acceptance 1 and the predicate's second
/// half): the bot value of `--forge` is a clap refusal, exit 2, on `takt run` and on
/// `takt scenario run`, and neither command's help names `bot` as a word.
#[test]
fn no_command_offers_or_takes_a_bot_forge() {
    for args in [vec!["run", "--help"], vec!["scenario", "run", "--help"]] {
        let output = Command::new(env!("CARGO_BIN_EXE_takt"))
            .args(&args)
            .output()
            .expect("takt runs");
        assert!(output.status.success(), "{args:?}");
        let help = String::from_utf8_lossy(&output.stdout);
        let words: Vec<&str> = help
            .split(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
            .filter(|word| word.eq_ignore_ascii_case("bot"))
            .collect();
        assert!(words.is_empty(), "{args:?} names bot");
    }
    let scratch = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("w47-adv-trial-{}-forge-bot", std::process::id()));
    fs::create_dir_all(&scratch).expect("a directory");
    for args in [
        vec!["run", "--repo", ".", "--forge", "bot", "--intent", "x"],
        vec!["scenario", "run", "--repo", ".", "1", "--forge", "bot"],
    ] {
        let output = Command::new(env!("CARGO_BIN_EXE_takt"))
            .args(&args)
            .current_dir(&scratch)
            .output()
            .expect("takt runs");
        let err = String::from_utf8_lossy(&output.stderr);
        assert_eq!(output.status.code(), Some(2), "{args:?}: {err}");
        assert!(err.contains("invalid value 'bot'"), "{args:?}: {err}");
    }
    let _ = fs::remove_dir_all(&scratch);
}
