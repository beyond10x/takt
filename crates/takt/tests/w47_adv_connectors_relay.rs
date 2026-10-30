//! Wave 47, adversarial pass on story:git-and-model-through-loopback-relays and
//! story:connectors-forge-speaks-github-at-an-endpoint, at the library: the connectors forge's
//! push to a loopback relay, its check-run walk and the `forge_endpoint` reader.
//!
//! The rule under attack (`crates/takt/src/git.rs`, `RemoteTransport::LoopbackHttp`): `http://`
//! goes to a loopback host alone, "so takt holds none and nothing crosses the network in the
//! clear"; the connectors forge's push and fetch go to the loopback relay `origin` names "and to
//! no other host" (`crates/takt/src/forge/connectors.rs`, `relay`).
//!
//! The forge is reached on its CLI route through a double of `connectors` that answers each
//! `--describe` and each invocation; a `git` line is recorded, or run for real where git's own
//! reading of its configuration is the point. Nothing here reaches beyond 127.0.0.1: a host that
//! would be reached is `example.invalid`, which no resolver answers, or a proxy stand-in on
//! 127.0.0.1 that records what it is sent.

#[path = "support/own_home.rs"]
mod own_home;

use std::cell::RefCell;
use std::fs;
use std::io::{BufRead as _, BufReader, Write as _};
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::rc::Rc;
use std::sync::{Arc, Mutex};

use serde_json::{Value, json};
use takt::forge::connectors::ConnectorsForge;
use takt::forge::{Checks, Exit, Forge, Process, Runner};
use takt::git::RemoteTransport;
use takt::run_config::RunConfig;
use takt::scan::Scan;

const BRANCH: &str = "takt/case-1";
const NUMBER: u64 = 7;
const REVISION: &str = "descriptor-r1";

fn git(tree: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .args(["-c", "user.name=t", "-c", "user.email=t@localhost"])
        .args(["-c", "commit.gpgsign=false"])
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
    String::from_utf8_lossy(&output.stdout).trim().to_owned()
}

struct Scratch(PathBuf);

impl Scratch {
    fn new(name: &str) -> Self {
        let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
            .join(format!("w47-adv-relay-{}-{name}", std::process::id()));
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

/// A tree with one commit on `main` and the case branch one above it, its `origin` `url`.
fn tree_with_origin(scratch: &Scratch, url: &str) -> (PathBuf, String) {
    let tree = scratch.0.join("tree");
    fs::create_dir_all(&tree).expect("a directory");
    fs::write(tree.join("README.md"), "a repository\n").expect("a file");
    git(&tree, &["init", "-q", "-b", "main"]);
    git(&tree, &["add", "-A"]);
    git(&tree, &["commit", "-q", "-m", "start"]);
    git(&tree, &["checkout", "-q", "-b", BRANCH]);
    fs::write(tree.join("README.md"), "a repository, changed\n").expect("a file");
    git(&tree, &["commit", "-q", "-am", "change"]);
    let head = git(&tree, &["rev-parse", "HEAD"]);
    git(&tree, &["checkout", "-q", "main"]);
    git(&tree, &["remote", "add", "origin", url]);
    (tree, head)
}

/// One page of check runs as the double answers `check_runs.list`: it serves `page_size` per
/// page whatever `per_page` asks, and counts all of them in `total_count`.
#[derive(Clone, Default)]
struct Served {
    head: String,
    runs: Vec<(&'static str, Option<&'static str>)>,
    page_size: usize,
}

/// One recorded `git` line: its arguments and the environment it was given.
type GitLine = (Vec<String>, Vec<(String, String)>);

/// A double of the forge's runner: `connectors` lines are answered as `connectors --output
/// json` would (every github operation described; `pull_request.get`, `check_runs.list` and
/// `commit.status` answered from `served`); `git` lines are recorded with their environment and,
/// when `real` is set, run for real through takt's own [`Process`].
#[derive(Clone, Default)]
struct Double {
    real: bool,
    served: Served,
    git_lines: Rc<RefCell<Vec<GitLine>>>,
}

impl Double {
    fn answer(&self, argv: &[String]) -> Value {
        let operation = argv[5].as_str();
        if argv.iter().any(|word| word == "--describe") {
            return json!({
                "connector": "github", "instance": "only", "operation": operation,
                "effect": "read", "schema": format!("s-{operation}"), "revision": REVISION,
                "input_schema": {},
            });
        }
        let input: Value = argv
            .last()
            .and_then(|text| serde_json::from_str(text).ok())
            .unwrap_or(Value::Null);
        let body = match operation {
            "pull_request.get" => json!({
                "state": "open", "merged": false,
                "head": {"ref": BRANCH, "sha": self.served.head}, "base": {"ref": "main"},
            }),
            "check_runs.list" => {
                let page = input["page"].as_u64().unwrap_or(1) as usize;
                let size = self.served.page_size;
                let runs: Vec<Value> = self
                    .served
                    .runs
                    .iter()
                    .skip((page - 1) * size)
                    .take(size)
                    .map(|(status, conclusion)| json!({"status": status, "conclusion": conclusion}))
                    .collect();
                json!({"total_count": self.served.runs.len(), "check_runs": runs})
            }
            "commit.status" => json!({"state": "pending", "total_count": 0, "statuses": []}),
            other => panic!("the double answers no {other}"),
        };
        json!({"result": {"status": 200, "provenance": {}, "body": body}})
    }
}

impl Runner for Double {
    fn run(&self, tree: &Path, argv: &[String]) -> anyhow::Result<Exit> {
        self.run_env(tree, argv, &[])
    }

    fn run_env(&self, tree: &Path, argv: &[String], env: &[(&str, &str)]) -> anyhow::Result<Exit> {
        match argv.first().map(String::as_str) {
            Some("connectors") => Ok(Exit {
                code: Some(0),
                stdout: json!({"ok": true, "result": self.answer(argv)}).to_string(),
                stderr: String::new(),
            }),
            Some("git") => {
                self.git_lines.borrow_mut().push((
                    argv.to_vec(),
                    env.iter()
                        .map(|(name, value)| ((*name).to_owned(), (*value).to_owned()))
                        .collect(),
                ));
                if self.real {
                    let mut process = Process::default();
                    process.limits.timeout = std::time::Duration::from_secs(20);
                    process.run_env(tree, argv, env)
                } else {
                    Ok(Exit {
                        code: Some(0),
                        stdout: String::new(),
                        stderr: String::new(),
                    })
                }
            }
            other => panic!("a line the forge should not run: {other:?} {argv:?}"),
        }
    }
}

fn forge(scratch: &Scratch, tree: &Path, double: &Double) -> anyhow::Result<ConnectorsForge> {
    ConnectorsForge::for_tree(
        tree,
        "github",
        None,
        Scan::None {
            declared_in: scratch.0.join("run.yaml"),
        },
        Box::new(double.clone()),
    )
}

/// The URL a recorded `git push` line pushes to: the word after `push`.
fn pushed_to(line: &[String]) -> String {
    let at = line
        .iter()
        .position(|word| word == "push")
        .expect("a push line");
    line[at + 1].clone()
}

/// The URL is admitted as a loopback relay by `remote_transport`: what git and curl read as
/// its host is `127.0.0.1`, since a URL's authority ends at the first `/`, `?` or `#`. The
/// forge then derives the URL it pushes to with `without_userinfo`, which ends the authority at
/// the first `/` alone, and keeps whatever follows the last `@` before it: `example.com`. The
/// push goes there over `http`, with `GIT_ALLOW_PROTOCOL=http:https`.
#[test]
fn an_origin_admitted_as_a_loopback_relay_is_pushed_to_that_relay_and_no_other_host() {
    for (name, origin) in [
        (
            "fragment",
            "http://127.0.0.1:8403#@example.com/owner/repo.git",
        ),
        ("query", "http://127.0.0.1:8403?@example.com/owner/repo.git"),
    ] {
        assert_eq!(
            takt::git::remote_transport(origin).expect("admitted"),
            RemoteTransport::LoopbackHttp,
            "{origin} is admitted as a loopback relay"
        );
        let scratch = Scratch::new(&format!("hidden-at-{name}"));
        let (tree, _) = tree_with_origin(&scratch, origin);
        let double = Double::default();
        let Ok(forge) = forge(&scratch, &tree, &double) else {
            continue;
        };
        let pushed = forge.push(&tree, BRANCH);
        let lines = double.git_lines.borrow().clone();
        let Some((line, env)) = lines.first() else {
            assert!(pushed.is_err(), "no push line ran and none was refused");
            continue;
        };
        let url = pushed_to(line);
        let allowed = env
            .iter()
            .find(|(name, _)| name == "GIT_ALLOW_PROTOCOL")
            .map(|(_, value)| value.as_str());
        assert!(
            url.starts_with("http://127.0.0.1:8403"),
            "origin {origin}, admitted as the relay 127.0.0.1:8403, is pushed to {url} with GIT_ALLOW_PROTOCOL={allowed:?}"
        );
    }
}

/// A relayed push names the relay's URL on git's command line, and git applies the
/// configuration's `url.<base>.pushInsteadOf` to a URL given there too: a rewrite to `http://`
/// on another host carries the push off the machine in the clear, with `http` allowed. The trial
/// route reads every URL as git rewrites it (`remote_urls`) and refuses such a rewrite; the
/// connectors forge reads none.
#[test]
fn a_push_rewrite_does_not_carry_a_relayed_push_off_loopback() {
    let scratch = Scratch::new("push-instead-of");
    let relay = "http://127.0.0.1:9/owner/repo.git";
    let (tree, _) = tree_with_origin(&scratch, relay);
    git(
        &tree,
        &[
            "config",
            "url.http://example.invalid/.pushInsteadOf",
            "http://127.0.0.1:9/",
        ],
    );
    let double = Double {
        real: true,
        ..Double::default()
    };
    let forge = forge(&scratch, &tree, &double).expect("the forge");
    let said = match forge.push(&tree, BRANCH) {
        Ok(()) => "pushed".to_owned(),
        Err(error) => format!("{error:#}"),
    };
    assert!(
        said.contains("no loopback host")
            || !said.contains("Could not resolve host: example.invalid"),
        "the relayed push went to http://example.invalid: {said}"
    );
}

/// A proxy stand-in on 127.0.0.1 that records each request line it is sent and answers 502.
fn proxy_recorder() -> (String, Arc<Mutex<Vec<String>>>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("a loopback port");
    let url = format!("http://{}", listener.local_addr().expect("an address"));
    let seen = Arc::new(Mutex::new(Vec::new()));
    let log = Arc::clone(&seen);
    std::thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            let Ok(clone) = stream.try_clone() else {
                continue;
            };
            let mut line = String::new();
            let _ = BufReader::new(clone).read_line(&mut line);
            log.lock().expect("the log").push(line.trim().to_owned());
            let mut out = stream;
            let _ = out.write_all(
                b"HTTP/1.1 502 Bad Gateway\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
            );
        }
    });
    (url, seen)
}

/// A line to a loopback relay must not leave the machine: the served wire turns proxies off for
/// a loopback endpoint (`exchange`, `client.no_proxy()`), but a relayed git line keeps the
/// proxy git is configured with (`http.proxy`, as `http_proxy` in the environment does), and
/// sends the push for `http://127.0.0.1:<p>/…` to that proxy, in the clear.
#[test]
fn a_relayed_push_does_not_go_through_a_proxy() {
    let scratch = Scratch::new("proxy");
    let (proxy, seen) = proxy_recorder();
    let (tree, _) = tree_with_origin(&scratch, "http://127.0.0.1:9/owner/repo.git");
    git(&tree, &["config", "http.proxy", &proxy]);
    let double = Double {
        real: true,
        ..Double::default()
    };
    let forge = forge(&scratch, &tree, &double).expect("the forge");
    let _ = forge.push(&tree, BRANCH);
    let seen = seen.lock().expect("the log").clone();
    assert!(
        seen.is_empty(),
        "the relayed push went through the proxy: {seen:?}"
    );
}

/// The check-run walk stops at a page shorter than the 100 it asks for, even when the page's
/// `total_count` says more runs are counted than were read: an instance that serves fewer per
/// page (30, GitHub's default) leaves every later run unread, and a failed run among them is
/// read as a pass.
#[test]
fn a_short_page_with_more_runs_counted_is_not_the_last_page() {
    let scratch = Scratch::new("short-page");
    let (tree, head) = tree_with_origin(&scratch, "https://github.example/owner/repo.git");
    let mut runs = vec![("completed", Some("success")); 30];
    runs.push(("completed", Some("failure")));
    let double = Double {
        served: Served {
            head,
            runs,
            page_size: 30,
        },
        ..Double::default()
    };
    let forge = forge(&scratch, &tree, &double).expect("the forge");
    let checks = forge.checks(&tree, NUMBER).expect("the checks");
    assert_eq!(
        checks,
        Checks::Fail,
        "31 runs counted, 30 read, the 31st failed"
    );
}

/// `forge_endpoint` is refused at the run file's line when it names a port no URL can hold;
/// accepted there, it is refused later, at the preflight, as a call that did not answer.
#[test]
fn a_forge_endpoint_with_a_port_past_65535_is_refused_at_its_line() {
    let scratch = Scratch::new("port");
    for endpoint in ["http://127.0.0.1:65536", "http://127.0.0.1:99999"] {
        let path = scratch.0.join("run.yaml");
        fs::write(
            &path,
            format!("forge: connectors\nforge_instance: github\nforge_endpoint: {endpoint}\n"),
        )
        .expect("the run file");
        let read = RunConfig::read(&path);
        assert!(
            read.is_err(),
            "forge_endpoint {endpoint} is accepted by the reader"
        );
    }
}

/// Holds: hosts that only look like a loopback host, or that a resolver would turn into one,
/// are not loopback hosts to the rule, and `http://` to them is refused naming the URL; an
/// uppercase scheme is no way around it.
#[test]
fn hosts_that_look_like_loopback_are_refused() {
    for url in [
        "http://127.0.0.1.example.com/o/r.git",
        "http://localhost.example/o/r.git",
        "http://localhost./o/r.git",
        "http://[::ffff:127.0.0.1]/o/r.git",
        "http://0x7f.1/o/r.git",
        "http://2130706433/o/r.git",
        "http://0177.0.0.1/o/r.git",
        "http://127.1/o/r.git",
        "http://127.0.0.1@example.com/o/r.git",
        "http://127.0.0.1:8403\\@example.com/o/r.git",
        "HTTP://example.com/o/r.git",
        "hTtP://example.com/o/r.git",
        "http://127.0.0.1%2eexample.com/o/r.git",
    ] {
        assert!(
            takt::git::remote_transport(url).is_err(),
            "{url} is admitted"
        );
    }
    assert_eq!(
        takt::git::remote_transport("HTTP://LOCALHOST:8403/o/r.git").expect("admitted"),
        RemoteTransport::LoopbackHttp
    );
    assert_eq!(
        takt::git::RemoteTransport::Https.allowed_protocols(),
        "https"
    );
    assert_eq!(
        takt::git::RemoteTransport::Other.allowed_protocols(),
        "https"
    );
}
