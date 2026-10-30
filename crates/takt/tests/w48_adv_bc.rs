//! Wave 48, adversarial pass on story:git-through-the-connectors-git-relay (the loopback relay's
//! path `git/<owner>/<repo>` names `<owner>/<repo>`) at the library, with no served instance.
//!
//! The only test of that naming is `crates/takt-cli/tests/git_relay_served.rs`, which runs only
//! where `TAKT_TEST_CONNECTORS_BIN` names connectors 0.62.0, and whose stand-in for the github
//! operations never reads `owner` or `repo`: it would pass with any two-segment name. These cases
//! read what the forge sends as `owner` and `repo` (github) or `id` (gitlab) through a double of
//! `connectors` on the CLI route, for the relay's path and for the paths around it. Nothing here
//! reaches a network: every `origin` is either a loopback URL nothing listens on or a host under
//! `example.invalid`, and no git line runs.

use std::cell::RefCell;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::rc::Rc;

use serde_json::{Value, json};
use takt::forge::connectors::ConnectorsForge;
use takt::forge::{Exit, Forge, Runner};
use takt::scan::Scan;

fn git(tree: &Path, args: &[&str]) {
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
}

struct Scratch(PathBuf);

impl Scratch {
    fn new(name: &str) -> Self {
        let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
            .join(format!("w48-adv-bc-{}-{name}", std::process::id()));
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

/// A tree with one commit on `main`, its `origin` `url`.
fn tree_with_origin(scratch: &Scratch, url: &str) -> PathBuf {
    let tree = scratch.0.join("tree");
    fs::create_dir_all(&tree).expect("a directory");
    fs::write(tree.join("README.md"), "a repository\n").expect("a file");
    git(&tree, &["init", "-q", "-b", "main"]);
    git(&tree, &["add", "-A"]);
    git(&tree, &["commit", "-q", "-m", "start"]);
    git(&tree, &["remote", "add", "origin", url]);
    tree
}

/// A double of `connectors --output json invoke <target> <operation> [...] <input>`: every
/// operation is described, and every invocation's input is recorded by operation. The read of
/// a pull or merge request is answered as an open one.
#[derive(Clone, Default)]
struct Double {
    inputs: Rc<RefCell<Vec<(String, Value)>>>,
}

impl Double {
    fn answer(&self, argv: &[String]) -> Value {
        let target = argv[4].as_str();
        let operation = argv[5].as_str();
        if argv.iter().any(|word| word == "--describe") {
            return json!({
                "connector": target, "instance": "only", "operation": operation,
                "effect": "read", "schema": format!("s-{operation}"), "revision": "r1",
                "input_schema": {},
            });
        }
        let input: Value = argv
            .last()
            .and_then(|text| serde_json::from_str(text).ok())
            .unwrap_or(Value::Null);
        self.inputs.borrow_mut().push((operation.to_owned(), input));
        let body = json!({
            "state": "open", "merged": false, "iid": 7,
            "head": {"ref": "takt/case", "sha": "a".repeat(40)}, "base": {"ref": "main"},
            "source_branch": "takt/case", "target_branch": "main", "sha": "a".repeat(40),
        });
        json!({"result": {"status": 200, "provenance": {}, "body": body}})
    }

    /// The input of the first invocation of `operation`.
    fn input_of(&self, operation: &str) -> Value {
        self.inputs
            .borrow()
            .iter()
            .find(|(name, _)| name == operation)
            .map(|(_, input)| input.clone())
            .unwrap_or_else(|| panic!("no {operation} was invoked: {:?}", self.inputs.borrow()))
    }
}

impl Runner for Double {
    fn run(&self, tree: &Path, argv: &[String]) -> anyhow::Result<Exit> {
        self.run_env(tree, argv, &[])
    }

    fn run_env(&self, _: &Path, argv: &[String], _: &[(&str, &str)]) -> anyhow::Result<Exit> {
        match argv.first().map(String::as_str) {
            Some("connectors") => Ok(Exit {
                code: Some(0),
                stdout: json!({"ok": true, "result": self.answer(argv)}).to_string(),
                stderr: String::new(),
            }),
            other => panic!("a line this case should not run: {other:?} {argv:?}"),
        }
    }
}

fn forge(
    scratch: &Scratch,
    tree: &Path,
    target: &str,
    double: &Double,
) -> anyhow::Result<ConnectorsForge> {
    ConnectorsForge::for_tree(
        tree,
        target,
        None,
        Scan::None {
            declared_in: scratch.0.join("run.yaml"),
        },
        Box::new(double.clone()),
    )
}

/// The `owner` and `repo` the github connector sends for `origin`, read from `pull_request.get`.
fn github_names(name: &str, origin: &str) -> (String, String) {
    let scratch = Scratch::new(name);
    let tree = tree_with_origin(&scratch, origin);
    let double = Double::default();
    let forge = forge(&scratch, &tree, "github", &double)
        .unwrap_or_else(|error| panic!("{origin}: the forge opens: {error:#}"));
    let _ = forge.pull_request(&tree, 7);
    let input = double.input_of("pull_request.get");
    (
        input["owner"].as_str().unwrap_or_default().to_owned(),
        input["repo"].as_str().unwrap_or_default().to_owned(),
    )
}

/// The refusal the github connector's forge gives for `origin`, which names no
/// `<owner>/<repo>`.
fn github_refusal(name: &str, origin: &str) -> String {
    let scratch = Scratch::new(name);
    let tree = tree_with_origin(&scratch, origin);
    let double = Double::default();
    match forge(&scratch, &tree, "github", &double) {
        Ok(forge) => panic!("{origin} is refused, and the forge opened: {forge:?}"),
        Err(error) => format!("{error:#}"),
    }
}

/// The acceptance the served test does not observe: the relay's path `git/o/r` names owner `o`
/// and repo `r`, literally, with `.git`, without it, behind a userinfo and on `localhost`.
/// `git_relay_served.rs`'s stand-in answers `pull_request.*` whatever `owner` and `repo` say, so
/// there a mutant of `relay_project` that names another two-segment project stays green; and
/// where `TAKT_TEST_CONNECTORS_BIN` is unset nothing reads the mapping at all.
#[test]
fn a_loopback_relay_path_names_its_owner_and_repo_literally() {
    for (name, origin) in [
        ("dot-git", "http://127.0.0.1:8403/git/o/r.git"),
        ("bare", "http://127.0.0.1:8403/git/o/r"),
        ("slash", "http://127.0.0.1:8403/git/o/r.git/"),
        ("userinfo", "http://u@127.0.0.1:8403/git/o/r.git"),
        ("localhost", "http://localhost:8403/git/o/r.git"),
        ("owner-git", "http://127.0.0.1:8403/git/git/r.git"),
    ] {
        let expected_owner = if name == "owner-git" { "git" } else { "o" };
        assert_eq!(
            github_names(name, origin),
            (expected_owner.to_owned(), "r".to_owned()),
            "{origin}"
        );
    }
}

/// The mapping is the loopback relay's alone: an `https://` origin whose path is `git/o/r` names
/// the three-segment project `git/o/r`, which the github connector refuses, naming it. A mutant
/// that maps whatever the transport stays green everywhere else in the suite.
#[test]
fn an_https_origin_whose_path_begins_with_git_is_not_renamed() {
    for (name, origin) in [
        ("https", "https://github.example.invalid/git/o/r.git"),
        (
            "https-port",
            "https://github.example.invalid:8443/git/o/r.git",
        ),
        ("https-loopback", "https://127.0.0.1:8443/git/o/r.git"),
        ("ssh", "git@github.example.invalid:git/o/r.git"),
    ] {
        let refusal = github_refusal(name, origin);
        assert!(
            refusal.contains("the origin remote names git/o/r;"),
            "{origin}: {refusal}"
        );
    }
}

/// The relay serves `/git/<owner>/<repo>` alone; any other count of segments after `git/` is
/// not the relay's path and names itself, so the github connector refuses it naming the whole
/// path. A mutant that maps `git/<a>/<b>/<c>` to `<a>/<b>/<c>` (`>= 2`) is caught by the name.
#[test]
fn a_loopback_path_with_other_than_two_segments_after_git_names_itself() {
    let refusal = github_refusal("three", "http://127.0.0.1:8403/git/a/b/c.git");
    assert!(
        refusal.contains("the origin remote names git/a/b/c;"),
        "{refusal}"
    );
    assert_eq!(
        github_names("one", "http://127.0.0.1:8403/git/a.git"),
        ("git".to_owned(), "a".to_owned()),
        "`git/a` is owner git, repo a"
    );
}

/// Traversal and empty segments are refused before any mapping, as on every origin: the relay's
/// prefix does not turn `git/../x/y` or `git//b/c` into a name.
#[test]
fn a_loopback_relay_path_with_a_dot_segment_or_an_empty_one_is_refused() {
    for (name, origin) in [
        ("dotdot", "http://127.0.0.1:8403/git/../x.git"),
        ("dotdot-owner", "http://127.0.0.1:8403/git/../x/y.git"),
        ("dot", "http://127.0.0.1:8403/git/./r.git"),
        ("empty", "http://127.0.0.1:8403/git//r.git"),
    ] {
        let refusal = github_refusal(name, origin);
        assert!(
            refusal.contains("names the project by the origin remote, and it names none"),
            "{origin}: {refusal}"
        );
    }
}

/// The gitlab connector through a loopback relay takes no mapping: `git` can be a top-level
/// group there, so `git/group/repo` is the project `git/group/repo`, sent as `id`.
#[test]
fn the_gitlab_connector_keeps_the_relay_path_as_its_project() {
    let scratch = Scratch::new("gitlab");
    let tree = tree_with_origin(&scratch, "http://127.0.0.1:8403/git/group/repo.git");
    let double = Double::default();
    let forge = forge(&scratch, &tree, "gitlab", &double).expect("the forge opens");
    let _ = forge.pull_request(&tree, 7);
    assert_eq!(double.input_of("merge_request.get")["id"], "git/group/repo");
}

/// story:connectors-forge-at-a-served-instance, acceptance 2, against what the served tests do
/// not observe: they assert a key begins with `takt-` and is the same on a replay, so a key
/// that ignored the input, the revision or the refusal count would leave them green. A merge of
/// the same case at a moved head is another write and must go under another key, else a served
/// instance with standing grants answers `idempotency_conflict` and the case never merges.
#[test]
fn the_key_moves_with_the_input_the_revision_and_the_refusals() {
    use takt::forge::connectors::idempotency_key;
    let merge = |sha: &str| json!({"owner": "o", "repo": "r", "pull_number": 7, "sha": sha});
    let base = idempotency_key(
        "case-1",
        "pull_request.merge",
        "r1",
        &merge(&"a".repeat(40)),
        0,
    );
    assert_eq!(base.len(), 69, "{base}");
    assert!(base.starts_with("takt-"), "{base}");
    assert_eq!(
        base,
        idempotency_key(
            "case-1",
            "pull_request.merge",
            "r1",
            &merge(&"a".repeat(40)),
            0
        ),
        "the same write, the same key"
    );
    for (what, other) in [
        (
            "a moved head",
            idempotency_key(
                "case-1",
                "pull_request.merge",
                "r1",
                &merge(&"b".repeat(40)),
                0,
            ),
        ),
        (
            "another revision",
            idempotency_key(
                "case-1",
                "pull_request.merge",
                "r2",
                &merge(&"a".repeat(40)),
                0,
            ),
        ),
        (
            "a refusal more",
            idempotency_key(
                "case-1",
                "pull_request.merge",
                "r1",
                &merge(&"a".repeat(40)),
                1,
            ),
        ),
        (
            "another case",
            idempotency_key(
                "case-2",
                "pull_request.merge",
                "r1",
                &merge(&"a".repeat(40)),
                0,
            ),
        ),
        (
            "another write",
            idempotency_key(
                "case-1",
                "pull_request.create",
                "r1",
                &merge(&"a".repeat(40)),
                0,
            ),
        ),
    ] {
        assert_ne!(base, other, "{what}, another key");
    }
}
