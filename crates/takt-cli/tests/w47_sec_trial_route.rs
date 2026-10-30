//! Wave 47 security review of `takt scenario run`'s own git (story:takt-carries-no-organisations-own-route,
//! U1) and its loopback rule (story:git-and-model-through-loopback-relays, U2): "every URL git
//! reaches `origin` at is read before the fetch, and one that is `http://` to another host is
//! refused, naming it". Each case gives `origin` a way git reaches it over plain `http://` to a
//! host that is not a loopback host, and asserts the run is refused before git sends anything
//! there. A proxy stand-in on 127.0.0.1 records any request git sends off the machine and answers
//! 502, so nothing leaves it.

#[path = "../../takt/tests/support/own_home.rs"]
mod own_home;

use std::fs;
use std::io::{BufRead as _, BufReader, Write as _};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Arc, Mutex};

/// The fixture stores of the library crate, one directory each.
const FIXTURES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../takt/tests/fixtures");

/// A gates document whose one assertion is true in a repository with a manifest.
const GATES: &str = "format: engineering-gates/1\nlanguage: canon-expr/1\ngates:\n  - id: manifest\n    \
                     text: The repository has its manifest.\n    assertions:\n      \
                     - 'fs.file.exists(\"Cargo.toml\")'\n";

/// The proxy variables a git child reads; every run here clears them and then sets its own.
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

/// The first line of one request read from `reader`: `<method> <target>`.
fn first_line(reader: &mut BufReader<TcpStream>) -> Option<String> {
    let mut line = String::new();
    if reader.read_line(&mut line).ok()? == 0 {
        return None;
    }
    let mut parts = line.split_whitespace();
    let method = parts.next()?;
    let target = parts.next()?;
    loop {
        let mut header = String::new();
        reader.read_line(&mut header).ok()?;
        if header.trim_end().is_empty() {
            break;
        }
    }
    Some(format!("{method} {target}"))
}

/// A proxy stand-in on 127.0.0.1: it records the first line of every request it is sent and
/// answers 502, so a git told to use it reaches nothing beyond this machine.
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
                if let Some(line) = first_line(&mut reader) {
                    log.lock().expect("the log").push(line);
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

/// One scratch repository whose seed commit holds the fixture store with `story:ready-one`
/// `proposed`, and `main` past it with scenario 1 declared and a commit identity configured.
struct World {
    _scratch: Scratch,
    repo: PathBuf,
    state: PathBuf,
}

fn world(name: &str) -> World {
    let outer = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("w47-sec-trial-route-{}-{name}", std::process::id()));
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
    let state = outer.join("state");
    fs::create_dir_all(&state).expect("a state directory");
    World {
        _scratch: Scratch(outer),
        repo,
        state,
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
}

/// git's `<transport>::<address>` form names the `http` remote helper explicitly:
/// `http::http://example.com/o/r.git` is fetched over plain `http://` from `example.com`, as
/// `http://example.com/o/r.git` is. The run must refuse it before the fetch, as it refuses the
/// bare `http://` URL; git must send nothing to the host.
#[test]
fn an_http_helper_origin_on_another_host_is_refused_before_the_fetch() {
    let world = world("helper-form");
    let proxy = ProxyRecorder::start();
    git(
        &world.repo,
        &[
            "remote",
            "add",
            "origin",
            "http::http://example.com/o/r.git",
        ],
    );
    let (code, out, err) =
        world.scenario_run(&[("http_proxy", &proxy.url()), ("https_proxy", &proxy.url())]);
    assert_eq!(
        proxy.seen(),
        Vec::<String>::new(),
        "git sent the fetch over plain http to a host that is no loopback host; exit {code}, stdout {out} stderr {err}"
    );
    assert!(
        err.contains("no loopback host"),
        "exit {code}, stderr {err}"
    );
}

/// The URL rule is read through takt's own git, which drops `GIT_CONFIG_COUNT` and
/// `GIT_CONFIG_PARAMETERS`, while the fetch runs through the operator's git, which keeps them. A
/// rewrite set in the environment (`url.http://example.com/.insteadOf`) then sends the fetch of
/// an `https://` origin over plain `http://` to another host. The run must refuse it before the
/// fetch; git must send nothing to the host.
#[test]
fn a_rewrite_in_the_environment_is_read_before_the_fetch() {
    let world = world("env-rewrite");
    let proxy = ProxyRecorder::start();
    git(
        &world.repo,
        &[
            "remote",
            "add",
            "origin",
            "https://forge.example.invalid/o/r.git",
        ],
    );
    let (code, out, err) = world.scenario_run(&[
        ("http_proxy", &proxy.url()),
        ("https_proxy", &proxy.url()),
        ("GIT_CONFIG_COUNT", "1"),
        ("GIT_CONFIG_KEY_0", "url.http://example.com/.insteadOf"),
        ("GIT_CONFIG_VALUE_0", "https://forge.example.invalid/"),
    ]);
    let plain: Vec<String> = proxy
        .seen()
        .into_iter()
        .filter(|line| line.contains("http://example.com/"))
        .collect();
    assert_eq!(
        plain,
        Vec::<String>::new(),
        "git sent the fetch over plain http to a host that is no loopback host; exit {code}, stdout {out} stderr {err}"
    );
}

/// A password in the userinfo of a loopback `origin` is not repeated in what the run prints
/// when the fetch fails (a control: what git prints of the URL is anonymised).
#[test]
fn a_password_in_a_loopback_origin_is_not_printed_when_the_fetch_fails() {
    let world = world("userinfo");
    let closed = TcpListener::bind("127.0.0.1:0").expect("a loopback port");
    let port = closed.local_addr().expect("an address").port();
    drop(closed);
    let password = format!("pw0rd{}", "W47secZq");
    git(
        &world.repo,
        &[
            "remote",
            "add",
            "origin",
            &format!("http://user:{password}@127.0.0.1:{port}/o/r.git"),
        ],
    );
    let (code, out, err) = world.scenario_run(&[]);
    assert_ne!(code, 0, "stdout {out} stderr {err}");
    assert!(
        !out.contains(&password) && !err.contains(&password),
        "stdout {out} stderr {err}"
    );
}
