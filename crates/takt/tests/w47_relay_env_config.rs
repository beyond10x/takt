//! Wave 47, the confirm pass of story:connectors-forge-speaks-github-at-an-endpoint: a
//! `pushInsteadOf` rewrite the environment hands git (`GIT_CONFIG_COUNT`, `GIT_CONFIG_KEY_<n>`,
//! `GIT_CONFIG_VALUE_<n>`) is read before the connectors forge's push to the loopback relay, as
//! the push itself reads it. One test in this binary: it sets the process environment, which no
//! other test may share. The helpers are the adversarial pass's (`w47_adv_connectors_relay.rs`).
#![allow(dead_code, unused_imports)]

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
fn a_push_rewrite_given_in_the_environment_does_not_carry_a_relayed_push_off_loopback() {
    let scratch = Scratch::new("env-push-instead-of");
    let relay = "http://127.0.0.1:9/owner/repo.git";
    let (tree, _) = tree_with_origin(&scratch, relay);
    // SAFETY: the only test of this binary; no other thread reads the environment meanwhile.
    unsafe {
        std::env::set_var("GIT_CONFIG_COUNT", "1");
        std::env::set_var(
            "GIT_CONFIG_KEY_0",
            "url.http://example.invalid/.pushInsteadOf",
        );
        std::env::set_var("GIT_CONFIG_VALUE_0", "http://127.0.0.1:9/");
    }
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
        said.contains("no loopback host") && said.contains("example.invalid"),
        "a rewrite in the environment was not read before the relayed push: {said}"
    );
}
