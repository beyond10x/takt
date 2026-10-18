//! Adversary pass 1 on `pr.open`'s scan before the push: content the push publishes that the
//! scan never reads. Each case builds a tree whose push would publish a literal the stand-in
//! policy forbids, and asserts what the story promises: nothing is pushed.
//!
//! A recording runner stands in for `b10x-gates` and `gh`, as in `scan_before_push.rs`. Its
//! `scan-text` refuses a file holding [`FORBIDDEN`] on one line, or split across two lines the
//! way the real scanner's wrapped rule still matches (an optional `-`, blanks, a line break,
//! then blanks or `>|*#`), so a soft-wrapped paragraph is refused as `b10x-gates` refuses it.

use std::cell::RefCell;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::rc::Rc;

use anyhow::anyhow;
use takt::forge::{BotRoute, Exit, Forge, Runner};
use takt::harness::{Harness, StepContext, StepReport};
use takt::validator::{PrOpen, Validator};
use takt::{Run, RunOptions, Stop};

const PROTOCOL: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../protocols/story-delivery/1.yaml"
);

const POLICY: &str = "/policies/gates.json";

const FORBIDDEN: &str = "FORBIDDEN-7Q2X";

fn git_with(tree: &Path, identity: (&str, &str), args: &[&str]) -> String {
    let output = Command::new("git")
        .args([
            "-c",
            &format!("user.name={}", identity.0),
            "-c",
            &format!("user.email={}", identity.1),
        ])
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

fn git(tree: &Path, args: &[&str]) -> String {
    git_with(tree, ("t", "t@localhost"), args)
}

struct Scratch(PathBuf);

impl std::ops::Deref for Scratch {
    type Target = Path;

    fn deref(&self) -> &Path {
        &self.0
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        if let Err(error) = fs::remove_dir_all(&self.0) {
            eprintln!("remove the scratch tree {}: {error}", self.0.display());
        }
    }
}

fn scratch(name: &str) -> Scratch {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("scan-push-adv1-{}-{name}", std::process::id()));
    if dir.exists() {
        fs::remove_dir_all(&dir).expect("a stale scratch tree is removed");
    }
    fs::create_dir_all(&dir).expect("scratch dir");
    fs::write(dir.join("README.md"), "a story\n").expect("a file");
    git(&dir, &["init", "-q", "-b", "main"]);
    git(&dir, &["add", "-A"]);
    git(&dir, &["commit", "-q", "-m", "start"]);
    git(
        &dir,
        &[
            "remote",
            "add",
            "origin",
            "https://github.com/owner/repo.git",
        ],
    );
    Scratch(dir)
}

struct NoHarness;

impl Harness for NoHarness {
    fn name(&self) -> &str {
        "none"
    }

    fn step(&self, context: &StepContext<'_>) -> Result<StepReport, anyhow::Error> {
        Err(anyhow!(
            "no harness: a judgement step among {} is left",
            context.offered.join(", ")
        ))
    }
}

/// Whether the stand-in policy refuses `text`: [`FORBIDDEN`] on one line, or across a line
/// break the real scanner's wrapped rule spans (`-?[ \t]*\r?\n[ \t>|*#]*`).
fn breaks(text: &str) -> bool {
    if text.contains(FORBIDDEN) {
        return true;
    }
    let lines: Vec<&str> = text.lines().collect();
    lines.windows(2).any(|pair| {
        let left = pair[0].trim_end_matches([' ', '\t', '\r']);
        let right = pair[1].trim_start_matches([' ', '\t', '>', '|', '*', '#']);
        let kept = format!("{left}{right}");
        let dropped = format!("{}{right}", left.strip_suffix('-').unwrap_or(left));
        kept.contains(FORBIDDEN) || dropped.contains(FORBIDDEN)
    })
}

#[derive(Default)]
struct Gates {
    lines: RefCell<Vec<String>>,
    pushed: RefCell<Option<(String, String)>>,
}

impl Gates {
    fn argvs(&self) -> Vec<String> {
        self.lines.borrow().clone()
    }
}

fn exit(code: i32, stdout: &str, stderr: &str) -> Exit {
    Exit {
        code: Some(code),
        stdout: stdout.to_owned(),
        stderr: stderr.to_owned(),
    }
}

impl Runner for Gates {
    fn run(&self, tree: &Path, argv: &[String]) -> anyhow::Result<Exit> {
        self.run_env(tree, argv, &[])
    }

    fn run_env(&self, tree: &Path, argv: &[String], _env: &[(&str, &str)]) -> anyhow::Result<Exit> {
        let words: Vec<&str> = argv.iter().map(String::as_str).collect();
        let answer = match words.as_slice() {
            ["b10x-gates", "scan-text", rest @ ..] => {
                let mut refusal = None;
                let mut rest = rest.iter();
                while let Some(word) = rest.next() {
                    if word.starts_with("--") {
                        rest.next();
                        continue;
                    }
                    let text = fs::read_to_string(word)
                        .unwrap_or_else(|error| panic!("scan-text names {word}: {error}"));
                    if breaks(&text) {
                        refusal.get_or_insert(format!(
                            "{word} breaks 1 private rule(s); private details remain local"
                        ));
                    }
                }
                match refusal {
                    Some(said) => exit(1, "", &said),
                    None => exit(0, "1 file(s) carry no private rule", ""),
                }
            }
            ["b10x-gates", "bot", "--", "push", _, spec] => {
                let (revision, branch) = match spec.split_once(":refs/heads/") {
                    Some((revision, branch)) => (revision.to_owned(), branch.to_owned()),
                    None => (
                        git(tree, &["rev-parse", &format!("refs/heads/{spec}")]),
                        (*spec).to_owned(),
                    ),
                };
                *self.pushed.borrow_mut() = Some((branch, revision));
                exit(0, "", "")
            }
            ["b10x-gates", "api", ..] => {
                let output = argv
                    .iter()
                    .position(|a| a == "--output")
                    .map(|i| &argv[i + 1])
                    .expect("--output");
                fs::write(output, r#"{"number": 7}"#)?;
                exit(0, "", "")
            }
            ["gh", "pr", "view", ..] => {
                let (branch, sha) = self.pushed.borrow().clone().expect("a push before a view");
                let view = serde_json::json!({
                    "state": "OPEN",
                    "mergedAt": null,
                    "headRefName": branch,
                    "headRefOid": sha,
                });
                exit(0, &view.to_string(), "")
            }
            _ => exit(2, "", "the double does not know this line"),
        };
        self.lines.borrow_mut().push(argv.join(" "));
        Ok(answer)
    }
}

struct Shared(Rc<Gates>);

impl Runner for Shared {
    fn run(&self, tree: &Path, argv: &[String]) -> anyhow::Result<Exit> {
        self.0.run(tree, argv)
    }

    fn run_env(&self, tree: &Path, argv: &[String], env: &[(&str, &str)]) -> anyhow::Result<Exit> {
        self.0.run_env(tree, argv, env)
    }
}

fn bot(gates: &Rc<Gates>) -> Rc<dyn Forge> {
    Rc::new(BotRoute::new(
        "owner/repo",
        Some(PathBuf::from(POLICY)),
        Box::new(Shared(gates.clone())),
    ))
}

fn commit(tree: &Path, path: &str, text: &str, message: &str) {
    fs::write(tree.join(path), text).expect("a file");
    git(tree, &["add", "-A", "--", path]);
    git(tree, &["commit", "-q", "-m", message]);
}

struct Ran {
    stop: Result<Stop, String>,
    text: String,
    case_dir: PathBuf,
}

fn pr_open(tree: &Path, id: &str, forge: Rc<dyn Forge>, work: impl Fn(&Path)) -> Ran {
    let options = RunOptions {
        protocol: PROTOCOL.to_owned(),
        intent: "deliver the story\n\nThe story's body.".to_owned(),
        inputs: Vec::new(),
        iterations: Some(1),
        hours: None,
        tokens: None,
        case_id: Some(id.to_owned()),
        gate: Some("repository.merge".to_owned()),
    };
    let validators: Vec<Box<dyn Validator>> = vec![Box::new(PrOpen::new(forge))];
    let mut run = Run::open(tree, options, &NoHarness, &validators).expect("the case opens");
    work(tree);
    let revision = git(tree, &["rev-parse", "HEAD"]);
    run.case
        .set_revision("implementation", &revision)
        .expect("the revision moves");
    for (n, kind) in [(1, "test_result"), (2, "gate_result")] {
        run.case
            .add_evidence(&takt::evidence::Record::new(
                format!("e{n}"),
                kind,
                Some("pass".to_owned()),
                "implementation",
                &revision,
                "scan-test",
            ))
            .expect("the record is written");
    }
    let mut out = Vec::new();
    let stop = run.run(&mut out).map_err(|error| format!("{error:#}"));
    Ran {
        stop,
        text: String::from_utf8(out).expect("utf-8"),
        case_dir: run.case.dir.clone(),
    }
}

fn kinds(case_dir: &Path) -> Vec<String> {
    let Ok(entries) = fs::read_dir(case_dir.join("evidence")) else {
        return Vec::new();
    };
    entries
        .map(|entry| {
            let text = fs::read_to_string(entry.expect("an entry").path()).expect("a record");
            let record: serde_json::Value = serde_json::from_str(&text).expect("json");
            record["kind"].as_str().unwrap_or_default().to_owned()
        })
        .collect()
}

/// What the story promises when the push would publish a forbidden literal: the scan refuses,
/// no push line runs, no pull request is opened and no record is written.
fn nothing_left(ran: &Ran, gates: &Gates) {
    let argvs = gates.argvs();
    let pushes: Vec<&String> = argvs
        .iter()
        .filter(|argv| argv.starts_with("b10x-gates bot -- push "))
        .collect();
    assert!(
        pushes.is_empty(),
        "the push ran and published the forbidden literal unscanned: {pushes:#?}\nstop: {:?}\n{}",
        ran.stop,
        ran.text
    );
    assert!(ran.stop.is_err(), "{:?}\n{}", ran.stop, ran.text);
    assert!(
        !kinds(&ran.case_dir).contains(&"pull_request".to_owned()),
        "{:?}",
        kinds(&ran.case_dir)
    );
}

/// A step commits a `.gitattributes` marking a text file `-diff`, and the file holds the
/// literal. `git log --patch --no-textconv` prints "Binary files ... differ" for it, so the
/// scanned diff never holds the literal, while the push publishes the file whole.
#[test]
fn a_gitattributes_minus_diff_hides_a_forbidden_text_file_from_the_scan() {
    let tree = scratch("attributes");
    let gates = Rc::new(Gates::default());
    let ran = pr_open(&tree, "attributes", bot(&gates), |tree| {
        fs::write(tree.join(".gitattributes"), "notes.txt -diff\n").expect("attributes");
        git(tree, &["add", "--", ".gitattributes"]);
        commit(
            tree,
            "notes.txt",
            &format!("token {FORBIDDEN}\n"),
            "implementation.edit: notes",
        );
    });
    nothing_left(&ran, &gates);
}

/// The case opens from a local `main` that carries a commit the remote does not have yet. The
/// push of the case's branch publishes that commit too (it is an ancestor the remote lacks),
/// but the scan reads only `base..head`, which excludes it.
#[test]
fn a_commit_on_the_local_base_the_remote_lacks_is_pushed_unscanned() {
    let tree = scratch("unpushed-base");
    let start = git(&tree, &["rev-parse", "HEAD"]);
    git(&tree, &["update-ref", "refs/remotes/origin/main", &start]);
    commit(
        &tree,
        "local.txt",
        &format!("token {FORBIDDEN}\n"),
        "local work not yet pushed",
    );
    let unpushed = git(&tree, &["rev-parse", "HEAD"]);
    let gates = Rc::new(Gates::default());
    let ran = pr_open(&tree, "unpushed-base", bot(&gates), |tree| {
        commit(tree, "work.txt", "clean\n", "implementation.edit: clean");
        let published = git(tree, &["rev-list", "HEAD", "--not", "--remotes=origin"]);
        assert!(
            published.lines().any(|commit| commit == unpushed),
            "precondition: the push of HEAD publishes {unpushed}: {published}"
        );
    });
    nothing_left(&ran, &gates);
}

/// A commit's author and committer travel with the push and are shown on the forge for every
/// commit. An identity holding the literal is not in `git log --format=%B` nor in the diff's
/// `commit %H` header, so it is never scanned.
#[test]
fn an_author_identity_the_policy_forbids_is_pushed_unscanned() {
    let tree = scratch("identity");
    let gates = Rc::new(Gates::default());
    let ran = pr_open(&tree, "identity", bot(&gates), |tree| {
        fs::write(tree.join("work.txt"), "clean\n").expect("a file");
        git(tree, &["add", "-A", "--", "work.txt"]);
        git_with(
            tree,
            ("operator", &format!("operator@{FORBIDDEN}.example")),
            &["commit", "-q", "-m", "implementation.edit: clean"],
        );
    });
    nothing_left(&ran, &gates);
}

/// A soft-wrapped paragraph splits the literal across two lines. The scanner's wrapped rule
/// refuses the file as published, but in the diff each line starts with `+`, which the wrapped
/// rule does not span, so the scanned diff passes and the file is pushed.
#[test]
fn a_literal_soft_wrapped_across_two_added_lines_passes_the_diff_scan() {
    let (head, tail) = FORBIDDEN.split_at(6);
    let content = format!("A paragraph about {head}\n{tail} wrapped across a line.\n");
    assert!(
        breaks(&content),
        "precondition: the policy refuses the file as published"
    );
    let tree = scratch("wrapped");
    let gates = Rc::new(Gates::default());
    let ran = pr_open(&tree, "wrapped", bot(&gates), |tree| {
        commit(tree, "notes.md", &content, "implementation.edit: notes");
    });
    nothing_left(&ran, &gates);
}
