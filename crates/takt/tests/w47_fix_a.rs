//! Wave 47, the fixes of the connectors forge's review findings, at the library: the merge
//! target's tip is read from `origin` through git and compared with the served answer; every
//! line to a loopback relay goes through no proxy and is refused when git's configuration would
//! carry it off loopback; the check-run walk reads up to the count; a port past 65535 is no
//! port. The forge is reached on its CLI route through a double of `connectors`; a `git` line
//! is recorded and answered by the double, and nothing leaves 127.0.0.1.

#[path = "support/own_home.rs"]
mod own_home;

use std::cell::RefCell;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::rc::Rc;

use serde_json::{Value, json};
use takt::forge::connectors::{ConnectorsForge, check_endpoint};
use takt::forge::{Checks, Exit, Forge, Runner};
use takt::scan::Scan;

const BRANCH: &str = "takt/case-1";
const NUMBER: u64 = 7;
const REVISION: &str = "descriptor-r1";
const RELAY: &str = "http://127.0.0.1:9/owner/repo.git";

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
            .join(format!("w47-fix-a-{}-{name}", std::process::id()));
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

/// A tree with one commit on `main` and the case branch one above it, its `origin` `url`:
/// the tree, `main`'s commit and the branch's.
fn tree_with_origin(scratch: &Scratch, url: &str) -> (PathBuf, String, String) {
    let tree = scratch.0.join("tree");
    fs::create_dir_all(&tree).expect("a directory");
    fs::write(tree.join("README.md"), "a repository\n").expect("a file");
    git(&tree, &["init", "-q", "-b", "main"]);
    git(&tree, &["add", "-A"]);
    git(&tree, &["commit", "-q", "-m", "start"]);
    let main = git(&tree, &["rev-parse", "HEAD"]);
    git(&tree, &["checkout", "-q", "-b", BRANCH]);
    fs::write(tree.join("README.md"), "a repository, changed\n").expect("a file");
    git(&tree, &["commit", "-q", "-am", "change"]);
    let head = git(&tree, &["rev-parse", "HEAD"]);
    git(&tree, &["checkout", "-q", "main"]);
    git(&tree, &["remote", "add", "origin", url]);
    (tree, main, head)
}

/// One recorded `git` line and its environment.
type Line = (Vec<String>, Vec<(String, String)>);

/// A double of the forge's runner: `connectors` answers as `connectors --output json` would,
/// `branch.get` with `served_tip` and `check_runs.list` with `pages`, one entry per page, each
/// with `total`; `git ls-remote` lists `main` at `origin_tip`; every other `git` line succeeds.
#[derive(Clone, Default)]
struct Double {
    served_tip: String,
    origin_tip: String,
    head: String,
    pages: Vec<usize>,
    total: Option<usize>,
    lines: Rc<RefCell<Vec<Line>>>,
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
            "branch.get" => json!({"name": "main", "commit": {"sha": self.served_tip}}),
            "pull_request.get" => json!({
                "state": "open", "merged": false,
                "head": {"ref": BRANCH, "sha": self.head}, "base": {"ref": "main"},
            }),
            "check_runs.list" => {
                let page = input["page"].as_u64().unwrap_or(1) as usize;
                let size = self.pages.get(page - 1).copied().unwrap_or(0);
                let runs = vec![json!({"status": "completed", "conclusion": "success"}); size];
                let mut body = json!({"check_runs": runs});
                if let Some(total) = self.total {
                    body["total_count"] = json!(total);
                }
                body
            }
            "commit.status" => json!({"state": "pending", "total_count": 0, "statuses": []}),
            other => panic!("the double answers no {other}"),
        };
        json!({"result": {"status": 200, "provenance": {}, "body": body}})
    }

    fn lines(&self) -> Vec<Line> {
        self.lines.borrow().clone()
    }
}

impl Runner for Double {
    fn run(&self, tree: &Path, argv: &[String]) -> anyhow::Result<Exit> {
        self.run_env(tree, argv, &[])
    }

    fn run_env(&self, _tree: &Path, argv: &[String], env: &[(&str, &str)]) -> anyhow::Result<Exit> {
        let stdout = match argv.first().map(String::as_str) {
            Some("connectors") => json!({"ok": true, "result": self.answer(argv)}).to_string(),
            Some("git") => {
                self.lines.borrow_mut().push((
                    argv.to_vec(),
                    env.iter()
                        .map(|(name, value)| ((*name).to_owned(), (*value).to_owned()))
                        .collect(),
                ));
                if argv.iter().any(|word| word == "ls-remote") {
                    format!("{}\trefs/heads/main\n", self.origin_tip)
                } else {
                    String::new()
                }
            }
            other => panic!("a line the forge should not run: {other:?} {argv:?}"),
        };
        Ok(Exit {
            code: Some(0),
            stdout,
            stderr: String::new(),
        })
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

fn verb(line: &Line) -> String {
    line.0
        .iter()
        .find(|word| matches!(word.as_str(), "push" | "fetch" | "ls-remote"))
        .cloned()
        .unwrap_or_default()
}

/// Finding 1: the served `branch.get` answer is the tip only when `git ls-remote` of origin
/// lists the base at the same commit; a served answer that differs is refused naming both, and
/// nothing is fetched.
#[test]
fn a_served_tip_that_origin_does_not_hold_is_refused() {
    let scratch = Scratch::new("tip");
    let (tree, main, head) = tree_with_origin(&scratch, "https://github.example/owner/repo.git");
    let lying = Double {
        served_tip: head.clone(),
        origin_tip: main.clone(),
        ..Double::default()
    };
    let said = format!(
        "{:#}",
        forge(&scratch, &tree, &lying)
            .expect("the forge")
            .target_tip(&tree, "main")
            .expect_err("a served tip origin does not hold")
    );
    assert!(said.contains(&head) && said.contains(&main), "{said}");
    assert_eq!(
        lying.lines().iter().map(verb).collect::<Vec<_>>(),
        ["ls-remote"],
        "nothing is fetched on a refused tip"
    );
    assert_eq!(
        lying.lines()[0].0,
        [
            "git",
            "-c",
            "core.hooksPath=/dev/null",
            "-c",
            "http.followRedirects=false",
            "ls-remote",
            "--refs",
            "https://github.example/owner/repo.git",
            "refs/heads/main",
        ]
    );

    let honest = Double {
        served_tip: main.clone(),
        origin_tip: main.clone(),
        ..Double::default()
    };
    let tip = forge(&scratch, &tree, &honest)
        .expect("the forge")
        .target_tip(&tree, "main")
        .expect("the tip");
    assert_eq!(tip, main);

    let unlisted = Double {
        served_tip: main.clone(),
        origin_tip: "not-an-object".to_owned(),
        ..Double::default()
    };
    assert!(
        forge(&scratch, &tree, &unlisted)
            .expect("the forge")
            .target_tip(&tree, "main")
            .is_err(),
        "a branch origin does not list is no tip"
    );
}

/// Finding 2c: every line to the loopback relay, the push, the read of the tip and the fetch,
/// sets `http.proxy` empty and `no_proxy` for every host; a line over HTTPS sets neither.
#[test]
fn every_relayed_line_goes_through_no_proxy() {
    let scratch = Scratch::new("no-proxy");
    let (tree, main, _) = tree_with_origin(&scratch, RELAY);
    let double = Double {
        served_tip: "0123456789abcdef0123456789abcdef01234567".to_owned(),
        origin_tip: "0123456789abcdef0123456789abcdef01234567".to_owned(),
        ..Double::default()
    };
    let forge = forge(&scratch, &tree, &double).expect("the forge");
    forge.push(&tree, BRANCH).expect("the push");
    forge.target_tip(&tree, "main").expect("the tip");
    let lines = double.lines();
    assert_eq!(
        lines.iter().map(verb).collect::<Vec<_>>(),
        ["push", "ls-remote", "fetch"]
    );
    for (argv, env) in &lines {
        let proxy = argv
            .windows(2)
            .any(|pair| pair[0] == "-c" && pair[1] == "http.proxy=");
        assert!(proxy, "{argv:?}");
        for name in ["no_proxy", "NO_PROXY"] {
            assert!(
                env.iter().any(|(held, value)| held == name && value == "*"),
                "{name} in {env:?} of {argv:?}"
            );
        }
        assert!(argv.iter().any(|word| word == RELAY), "{argv:?}");
    }
    assert_ne!(main, double.served_tip);

    let scratch = Scratch::new("over-https");
    let (tree, ..) = tree_with_origin(&scratch, "https://github.example/owner/repo.git");
    let double = Double::default();
    forge_over_https(&scratch, &tree, &double);
    for (argv, env) in double.lines() {
        assert!(!argv.iter().any(|word| word == "http.proxy="), "{argv:?}");
        assert!(
            !env.iter()
                .any(|(name, _)| name.eq_ignore_ascii_case("no_proxy")),
            "{env:?}"
        );
    }
}

fn forge_over_https(scratch: &Scratch, tree: &Path, double: &Double) {
    forge(scratch, tree, double)
        .expect("the forge")
        .push(tree, BRANCH)
        .expect("the push");
    assert_eq!(double.lines().len(), 1);
}

/// Finding 2b, the fetch side of the class: an `insteadOf` rewrite of the relay to another host
/// refuses the read of the tip before any line runs, as a `pushInsteadOf` or a `pushurl` of a
/// remote named by the relay's URL refuses the push. `origin` carries a userinfo, so the rewrite
/// matches the relay's URL, which has none, and not `origin`'s.
#[test]
fn a_rewrite_refuses_every_relayed_line_before_it_runs() {
    for (key, call) in [
        ("url.http://example.invalid/.insteadOf", "tip"),
        ("url.http://example.invalid/.pushInsteadOf", "push"),
        ("url.https://example.invalid/.pushInsteadOf", "push"),
        ("remote.http://127.0.0.1:9/owner/repo.git.pushurl", "push"),
    ] {
        let scratch = Scratch::new("rewrite");
        let (tree, main, _) = tree_with_origin(&scratch, "http://agent@127.0.0.1:9/owner/repo.git");
        let value = if key.starts_with("remote.") {
            "http://example.invalid/owner/repo.git"
        } else {
            "http://127.0.0.1:9/"
        };
        git(&tree, &["config", key, value]);
        let double = Double {
            served_tip: main.clone(),
            origin_tip: main,
            ..Double::default()
        };
        let forge = forge(&scratch, &tree, &double).expect("the forge");
        let refused = match call {
            "tip" => forge.target_tip(&tree, "main").map(drop),
            _ => forge.push(&tree, BRANCH),
        };
        let said = format!("{:#}", refused.expect_err("refused"));
        assert!(said.contains("no loopback host"), "{key}: {said}");
        assert!(double.lines().is_empty(), "{key}: {:?}", double.lines());
    }
}

/// What a relayed line could be sent to is read off `git config --list -z`: every rewrite
/// whose value begins the URL, of the URL and of a remote named by it, and no other.
#[test]
fn the_urls_a_relayed_line_reaches_are_every_matching_rewrite() {
    let config = [
        "url.http://example.invalid/.insteadof\nhttp://127.0.0.1:9/",
        "url.https://other.example/.pushinsteadof\nhttp://127.0.0.1:9/owner/",
        "url.https://unrelated.example/.insteadof\nhttp://127.0.0.1:10/",
        "remote.http://127.0.0.1:9/owner/repo.git.pushurl\nhttp://127.0.0.1:9/x.git",
        "http.proxy\nhttp://127.0.0.1:3128",
    ]
    .join("\0")
        + "\0";
    assert_eq!(
        takt::forge::connectors::reached_by(RELAY, config.as_bytes()),
        [
            RELAY,
            "http://127.0.0.1:9/x.git",
            "http://example.invalid/owner/repo.git",
            "https://other.example/repo.git",
            "http://example.invalid/x.git",
        ]
    );
}

/// Finding 2a: a relay origin holding a `?` or a `#` anywhere is refused before anything is
/// opened, so no reader of the URL finds another host in it.
#[test]
fn a_relay_origin_with_a_query_or_a_fragment_is_refused() {
    for origin in [
        "http://127.0.0.1:8403/owner/repo.git#@example.invalid/x",
        "http://127.0.0.1:8403/owner/repo.git?a=b",
        "http://u@127.0.0.1:8403#@example.invalid/owner/repo.git",
    ] {
        let scratch = Scratch::new("query");
        let (tree, ..) = tree_with_origin(&scratch, origin);
        let double = Double::default();
        let said = format!(
            "{:#}",
            forge(&scratch, &tree, &double).expect_err("refused")
        );
        assert!(said.contains("http://127.0.0.1:8403"), "{said}");
        assert!(!said.contains("u@"), "{said}");
        assert!(double.lines().is_empty());
    }
}

/// Finding 3: the walk reads up to `total_count`; an empty page before it is an error, never a
/// pass on part of the runs; with no count, a short page ends it.
#[test]
fn the_check_run_walk_reads_up_to_the_count() {
    let scratch = Scratch::new("walk");
    let (tree, _, head) = tree_with_origin(&scratch, "https://github.example/owner/repo.git");
    let checks = |pages: Vec<usize>, total: Option<usize>| {
        let double = Double {
            head: head.clone(),
            pages,
            total,
            ..Double::default()
        };
        forge(&scratch, &tree, &double)
            .expect("the forge")
            .checks(&tree, NUMBER)
    };
    assert_eq!(checks(vec![30, 30, 5], Some(65)).ok(), Some(Checks::Pass));
    let said = format!(
        "{:#}",
        checks(vec![30, 0], Some(31)).expect_err("an empty page short of the count")
    );
    assert!(said.contains("31") && said.contains("30"), "{said}");
    assert_eq!(checks(vec![30], None).ok(), Some(Checks::Pass));
}

/// Finding 4: a port is at most 65535, wherever a URL's host is read.
#[test]
fn a_port_is_at_most_65535() {
    assert_eq!(check_endpoint("http://127.0.0.1:65535"), Ok(true));
    assert_eq!(check_endpoint("https://[::1]:443/x"), Ok(true));
    for bad in [
        "http://127.0.0.1:65536",
        "http://127.0.0.1:99999",
        "https://connectors.example:4294967297",
        "http://[::1]:70000",
    ] {
        assert!(check_endpoint(bad).is_err(), "{bad}");
    }
    assert!(takt::git::remote_transport("http://127.0.0.1:65536/o/r.git").is_err());
}
