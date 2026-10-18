//! The forge seam through the loop: a run on `protocols/story-delivery/1.yaml` with no harness,
//! the tests and the gate already green at HEAD, and an in-memory forge double. `pr.open` pushes
//! the case's branch, opens one pull request through the forge and writes one `pull_request`
//! record with result `open`; `ci.read` writes one `ci_result` record from what the forge
//! reports about the checks, and nothing while they are pending. Every record is about
//! `implementation` at the pushed revision and names its validator; the model never produces
//! evidence. The bot-route adapter is tested for the argument lines it would run only: a
//! recording runner stands in for `b10x-gates` and `gh`, and nothing leaves the machine.

use std::cell::RefCell;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::rc::Rc;

use anyhow::anyhow;
use takt::case::{Branch, current};
use takt::evidence::Record;
use takt::forge::{
    BotRoute, Checks, Exit, Forge, Memory, PullRequest, PullRequestState, Runner, repository_of,
};
use takt::harness::{Harness, StepContext, StepReport};
use takt::validator::{CiRead, PrOpen, Validator};
use takt::{Decision, Run, RunOptions, Stop};

/// The protocol under test, relative to this crate.
const PROTOCOL: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../protocols/story-delivery/1.yaml"
);

/// Runs `git <args>` in `tree`; its trimmed standard output.
fn git(tree: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .args(["-c", "user.name=t", "-c", "user.email=t@localhost"])
        .args(args)
        .current_dir(tree)
        .output()
        .expect("git runs");
    assert!(output.status.success(), "git {args:?}");
    String::from_utf8(output.stdout)
        .expect("utf-8")
        .trim()
        .to_owned()
}

/// A scratch git tree with one commit on `main`, under this crate's build directory. Dropping
/// it removes the tree; a removal that fails fails the case, unless the case is already failing.
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
            let message = format!("remove the scratch tree {}: {error}", self.0.display());
            if std::thread::panicking() {
                eprintln!("{message}");
            } else {
                panic!("{message}");
            }
        }
    }
}

fn scratch(name: &str) -> Scratch {
    let dir =
        Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("forge-{}-{name}", std::process::id()));
    if dir.exists() {
        fs::remove_dir_all(&dir).expect("a stale scratch tree is removed");
    }
    fs::create_dir_all(&dir).expect("scratch dir");
    fs::write(dir.join("README.md"), "a story\n").expect("a file");
    git(&dir, &["init", "-q", "-b", "main"]);
    git(&dir, &["add", "-A"]);
    git(&dir, &["commit", "-q", "-m", "start"]);
    Scratch(dir)
}

/// A harness the runs below never reach: every step they take is a validator's.
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

fn record(n: usize, kind: &str, result: &str, revision: &str) -> Record {
    Record::new(
        format!("e{n}"),
        kind,
        Some(result.to_owned()),
        "implementation",
        revision,
        "forge-test",
    )
}

/// What a run left: how it stopped, what it printed, the records it wrote in file order, the
/// decision after it, the case directory and the revision the case was opened at.
struct Outcome {
    stop: Stop,
    text: String,
    records: Vec<serde_json::Value>,
    decision: Decision,
    case_dir: PathBuf,
    revision: String,
}

/// A run on the delivery protocol over `tree` with `pr.open` and `ci.read` over `forge` as the
/// only validators, the tests and the gate already green at HEAD, two steps at most: the
/// budget stops the run before a judgement step would be offered to the harness.
fn delivered(tree: &Path, id: &str, forge: Rc<dyn Forge>) -> Outcome {
    let options = RunOptions {
        protocol: PROTOCOL.to_owned(),
        intent: "deliver the story\n\nThe story's body.".to_owned(),
        inputs: Vec::new(),
        iterations: Some(2),
        hours: None,
        tokens: None,
        case_id: Some(id.to_owned()),
        gate: Some("repository.merge".to_owned()),
    };
    let validators: Vec<Box<dyn Validator>> = vec![
        Box::new(PrOpen::new(forge.clone())),
        Box::new(CiRead::new(forge)),
    ];
    let mut run = Run::open(tree, options, &NoHarness, &validators).expect("the case opens");
    let revision = run
        .case
        .revision("implementation")
        .expect("implementation has a revision");
    run.case
        .add_evidence(&record(1, "test_result", "pass", &revision))
        .expect("the record is written");
    run.case
        .add_evidence(&record(2, "gate_result", "pass", &revision))
        .expect("the record is written");
    let mut out = Vec::new();
    let stop = run.run(&mut out).expect("the run stops without an error");
    let text = String::from_utf8(out).expect("utf-8");
    let mut paths: Vec<PathBuf> = fs::read_dir(run.case.dir.join("evidence"))
        .expect("an evidence directory")
        .map(|entry| entry.expect("an entry").path())
        .collect();
    paths.sort();
    let records = paths
        .iter()
        .map(|path| {
            serde_json::from_str(&fs::read_to_string(path).expect("a record")).expect("json")
        })
        .collect();
    let decision = run.case.evaluate(99).expect("evaluates");
    Outcome {
        stop,
        text,
        records,
        decision,
        case_dir: run.case.dir.clone(),
        revision,
    }
}

fn claim<'a>(decision: &'a Decision, id: &str) -> &'a str {
    decision
        .claims
        .get(id)
        .map(String::as_str)
        .unwrap_or("absent")
}

fn action<'a>(decision: &'a Decision, id: &str) -> &'a str {
    decision
        .actions
        .get(id)
        .map(String::as_str)
        .unwrap_or("absent")
}

/// The story's acceptance: with the double reporting the pull request open and its checks
/// passing, the run writes a `pull_request` record with result `open` and a `ci_result` record
/// with result `pass`, both about `implementation` at the pushed revision, and the decision
/// shows `pr.opened` and `ci.green` true and `repository.merge` approval-required.
#[test]
fn with_checks_passing_the_pull_request_and_the_ci_result_are_recorded_at_the_pushed_revision() {
    let tree = scratch("pass");
    let head = git(&tree, &["rev-parse", "HEAD"]);
    let memory = Rc::new(Memory {
        checks: Checks::Pass,
        ..Memory::default()
    });
    let outcome = delivered(&tree, "pass", memory.clone());
    assert_eq!(outcome.stop, Stop::Iterations(2), "{}", outcome.text);
    assert_eq!(outcome.revision, head, "{}", outcome.text);
    assert!(
        outcome.text.contains("step 1: pr.open {}"),
        "{}",
        outcome.text
    );
    assert!(
        outcome.text.contains("evidence: pull_request open"),
        "{}",
        outcome.text
    );
    assert!(
        outcome.text.contains("step 2: ci.read {}"),
        "{}",
        outcome.text
    );
    assert!(
        outcome.text.contains("evidence: ci_result pass"),
        "{}",
        outcome.text
    );
    assert_eq!(outcome.records.len(), 4, "{}", outcome.text);
    let pull_request = &outcome.records[2];
    assert_eq!(pull_request["format"], "canon-evidence/1", "{pull_request}");
    assert_eq!(pull_request["kind"], "pull_request", "{pull_request}");
    assert_eq!(pull_request["result"], "open", "{pull_request}");
    assert_eq!(pull_request["subject"], "implementation", "{pull_request}");
    assert_eq!(
        pull_request["subject_revision"],
        head.as_str(),
        "{pull_request}"
    );
    assert_eq!(pull_request["producer"], "takt.pr-open", "{pull_request}");
    let ci = &outcome.records[3];
    assert_eq!(ci["kind"], "ci_result", "{ci}");
    assert_eq!(ci["result"], "pass", "{ci}");
    assert_eq!(ci["subject"], "implementation", "{ci}");
    assert_eq!(ci["subject_revision"], head.as_str(), "{ci}");
    assert_eq!(ci["producer"], "takt.ci-read", "{ci}");
    assert_eq!(
        claim(&outcome.decision, "pr.opened"),
        "true",
        "{}",
        outcome.decision.json
    );
    assert_eq!(
        claim(&outcome.decision, "ci.green"),
        "true",
        "{}",
        outcome.decision.json
    );
    assert_eq!(
        action(&outcome.decision, "repository.merge"),
        "approval-required",
        "{}",
        outcome.decision.json
    );
    // The branch is the case's, created at open from HEAD, pushed before the pull request was
    // opened, and the pull request is the branch's against the branch the tree was on.
    assert_eq!(
        git(&tree, &["rev-parse", "--abbrev-ref", "HEAD"]),
        "takt/pass"
    );
    assert_eq!(
        memory.calls.borrow().as_slice(),
        ["push takt/pass", "open takt/pass #1", "checks #1"],
        "{}",
        outcome.text
    );
    let opened = memory.opened.borrow();
    assert_eq!(opened.len(), 1);
    assert_eq!(opened[0].branch, "takt/pass");
    assert_eq!(opened[0].base, "main");
    assert_eq!(opened[0].title, "deliver the story");
    assert!(
        opened[0].body.contains("The story's body."),
        "{}",
        opened[0].body
    );
    assert!(opened[0].body.contains("tests.pass"), "{}", opened[0].body);
    assert!(opened[0].body.contains("gate.green"), "{}", opened[0].body);
    // The number and the pushed revision are recorded in the case directory.
    let branch = Branch::read(&outcome.case_dir).expect("the branch file");
    assert_eq!(branch.name, "takt/pass");
    assert_eq!(branch.base.as_deref(), Some("main"));
    assert_eq!(branch.pull_request, Some(1));
    assert_eq!(branch.pushed.as_deref(), Some(head.as_str()));
    assert_eq!(
        current(&tree).expect("the case of the branch"),
        outcome.case_dir
    );
}

/// The same run with the double reporting the checks pending writes no `ci_result` record, says
/// so, and leaves `ci.green` unknown; the pull request is still recorded open.
#[test]
fn with_checks_pending_no_ci_result_is_written_and_ci_green_stays_unknown() {
    let tree = scratch("pending");
    let memory = Rc::new(Memory {
        checks: Checks::Pending,
        ..Memory::default()
    });
    let outcome = delivered(&tree, "pending", memory.clone());
    assert_eq!(outcome.stop, Stop::Iterations(2), "{}", outcome.text);
    assert!(
        outcome.text.contains("step 2: ci.read {}"),
        "{}",
        outcome.text
    );
    assert!(
        !outcome.text.contains("evidence: ci_result"),
        "{}",
        outcome.text
    );
    assert!(outcome.text.contains("pending"), "{}", outcome.text);
    assert_eq!(outcome.records.len(), 3, "{}", outcome.text);
    assert_eq!(outcome.records[2]["kind"], "pull_request");
    assert_eq!(
        claim(&outcome.decision, "pr.opened"),
        "true",
        "{}",
        outcome.decision.json
    );
    assert_eq!(
        claim(&outcome.decision, "ci.green"),
        "unknown",
        "{}",
        outcome.decision.json
    );
    assert_eq!(
        action(&outcome.decision, "repository.merge"),
        "blocked",
        "{}",
        outcome.decision.json
    );
    assert_eq!(
        memory.calls.borrow().as_slice(),
        ["push takt/pending", "open takt/pending #1", "checks #1"]
    );
}

/// Failing checks write one `ci_result` record with result `fail`: `ci.green` is false, not
/// unknown, and the merge stays blocked.
#[test]
fn with_checks_failing_one_fail_record_is_written_and_ci_green_is_false() {
    let tree = scratch("fail");
    let memory = Rc::new(Memory {
        checks: Checks::Fail,
        ..Memory::default()
    });
    let outcome = delivered(&tree, "fail", memory);
    assert!(
        outcome.text.contains("evidence: ci_result fail"),
        "{}",
        outcome.text
    );
    assert_eq!(outcome.records.len(), 4, "{}", outcome.text);
    assert_eq!(outcome.records[3]["result"], "fail");
    assert_eq!(outcome.records[3]["producer"], "takt.ci-read");
    assert_eq!(
        claim(&outcome.decision, "ci.green"),
        "false",
        "{}",
        outcome.decision.json
    );
    assert_eq!(
        action(&outcome.decision, "repository.merge"),
        "blocked",
        "{}",
        outcome.decision.json
    );
}

/// `ci.read` before any pull request exists observes nothing and writes nothing: a claim
/// about continuous integration needs a pull request head to be about.
#[test]
fn ci_read_without_a_pull_request_writes_no_record() {
    let tree = scratch("no-pr");
    let memory = Rc::new(Memory {
        checks: Checks::Pass,
        ..Memory::default()
    });
    let forge: Rc<dyn Forge> = memory.clone();
    let options = RunOptions {
        protocol: PROTOCOL.to_owned(),
        intent: "deliver the story".to_owned(),
        inputs: Vec::new(),
        iterations: Some(1),
        hours: None,
        tokens: None,
        case_id: Some("no-pr".to_owned()),
        gate: None,
    };
    let validators: Vec<Box<dyn Validator>> = vec![Box::new(CiRead::new(forge))];
    let mut run = Run::open(&tree, options, &NoHarness, &validators).expect("the case opens");
    let mut out = Vec::new();
    let stop = run.run(&mut out).expect("the run stops without an error");
    let text = String::from_utf8(out).expect("utf-8");
    assert_eq!(stop, Stop::Iterations(1), "{text}");
    assert!(text.contains("step 1: ci.read {}"), "{text}");
    assert!(!text.contains("evidence:"), "{text}");
    assert!(
        memory.calls.borrow().is_empty(),
        "{:?}",
        memory.calls.borrow()
    );
    let decision = run.case.evaluate(99).expect("evaluates");
    assert_eq!(claim(&decision, "ci.green"), "unknown", "{}", decision.json);
}

/// `pr.open` refuses to open a pull request whose head would not be the revision the claim is
/// about: the tree's HEAD has moved past the case's implementation revision.
#[test]
fn pr_open_refuses_a_head_that_is_not_the_implementation_revision() {
    let tree = scratch("moved");
    let memory = Rc::new(Memory::default());
    let forge: Rc<dyn Forge> = memory.clone();
    let options = RunOptions {
        protocol: PROTOCOL.to_owned(),
        intent: "deliver the story".to_owned(),
        inputs: Vec::new(),
        iterations: Some(1),
        hours: None,
        tokens: None,
        case_id: Some("moved".to_owned()),
        gate: None,
    };
    let validators: Vec<Box<dyn Validator>> = vec![Box::new(PrOpen::new(forge))];
    let run = Run::open(&tree, options, &NoHarness, &validators).expect("the case opens");
    let revision = run
        .case
        .revision("implementation")
        .expect("implementation has a revision");
    fs::write(tree.join("README.md"), "moved\n").expect("a change");
    git(&tree, &["commit", "-q", "-am", "moved"]);
    let error = format!(
        "{:#}",
        PrOpen::new(memory.clone())
            .observe(&tree, &revision, 1)
            .expect_err("a moved head is refused")
    );
    assert!(error.contains("HEAD"), "{error}");
    assert!(error.contains(&revision[..8]), "{error}");
    assert!(
        memory.calls.borrow().is_empty(),
        "{:?}",
        memory.calls.borrow()
    );
}

/// A case the store would not take is discarded whole: its directory, the cases directory
/// when it was the last, and its branch, the tree back on the branch the case left.
#[test]
fn a_discarded_case_leaves_no_directory_and_no_branch() {
    let tree = scratch("discard");
    let protocol =
        takt::LoadedProtocol::from_file(Path::new(PROTOCOL)).expect("the protocol loads");
    let case = takt::CaseDir::open(&tree, protocol, "discard", "deliver the story", &[])
        .expect("the case opens");
    assert_eq!(case.branch, "takt/discard");
    assert_eq!(
        git(&tree, &["rev-parse", "--abbrev-ref", "HEAD"]),
        "takt/discard"
    );
    case.discard().expect("the case is discarded");
    assert_eq!(git(&tree, &["rev-parse", "--abbrev-ref", "HEAD"]), "main");
    assert_eq!(git(&tree, &["branch", "--list", "takt/discard"]), "");
    assert!(!tree.join(".engineering/cases").exists());
}

/// A runner that records every argument line it is given instead of running it, and answers
/// what the real programs would: a response file with the pull request's number for the API
/// call, the pull request's state for `gh pr view`, and the checks for `gh pr checks`.
struct Recording {
    lines: RefCell<Vec<String>>,
    view: &'static str,
    checks: &'static str,
}

impl Recording {
    fn new(view: &'static str, checks: &'static str) -> Self {
        Self {
            lines: RefCell::new(Vec::new()),
            view,
            checks,
        }
    }
}

impl Runner for Recording {
    fn run(&self, _: &Path, argv: &[String]) -> anyhow::Result<Exit> {
        self.lines.borrow_mut().push(argv.join(" "));
        let stdout = match argv.get(1).map(String::as_str) {
            Some("api") => {
                let output = argv
                    .iter()
                    .position(|a| a == "--output")
                    .map(|i| &argv[i + 1])
                    .expect("--output");
                fs::write(output, r#"{"number": 7, "state": "open"}"#)?;
                String::new()
            }
            Some("pr") if argv.get(2).map(String::as_str) == Some("view") => self.view.to_owned(),
            Some("pr") if argv.get(2).map(String::as_str) == Some("checks") => {
                self.checks.to_owned()
            }
            _ => String::new(),
        };
        Ok(Exit {
            code: Some(0),
            stdout,
            stderr: String::new(),
        })
    }
}

/// The bot-route adapter runs exactly these argument lines, in this order, and nothing else:
/// the push and the API write through `b10x-gates`, the pull-request body through `scan-text`
/// before it leaves the machine, and the read-only reads through `gh`. No token appears in
/// any of them.
#[test]
fn the_bot_route_adapter_runs_these_argument_lines() {
    let tree = scratch("argv");
    let dir = tree.join("out");
    let recording = Rc::new(Recording::new(
        r#"{"mergedAt":null,"state":"OPEN"}"#,
        r#"[{"bucket":"pass"},{"bucket":"skipping"}]"#,
    ));
    struct Shared(Rc<Recording>);
    impl Runner for Shared {
        fn run(&self, tree: &Path, argv: &[String]) -> anyhow::Result<Exit> {
            self.0.run(tree, argv)
        }
    }
    let forge = BotRoute::new(
        "owner/repo",
        Some(PathBuf::from("policy.yaml")),
        Box::new(Shared(recording.clone())),
    );
    forge.push(&tree, "takt/c1").expect("pushes");
    let request = PullRequest {
        branch: "takt/c1".to_owned(),
        base: "main".to_owned(),
        title: "deliver the story".to_owned(),
        body: "the body".to_owned(),
    };
    let number = forge
        .open_pull_request(&tree, &dir, &request)
        .expect("opens");
    assert_eq!(number, 7);
    assert_eq!(
        forge.pull_request(&tree, number).expect("reads"),
        PullRequestState::Open
    );
    assert_eq!(forge.checks(&tree, number).expect("reads"), Checks::Pass);
    let input = dir.join("pull-request.request.json");
    let output = dir.join("pull-request.response.json");
    assert_eq!(
        recording.lines.borrow().as_slice(),
        [
            "b10x-gates bot -- push origin takt/c1".to_owned(),
            format!("b10x-gates scan-text {}", input.display()),
            format!(
                "b10x-gates api --method POST --path /repos/owner/repo/pulls --input {} --output {} --repository owner/repo --policy policy.yaml",
                input.display(),
                output.display()
            ),
            "gh pr view 7 --json state,mergedAt".to_owned(),
            "gh pr checks 7 --json bucket".to_owned(),
        ]
    );
    let sent: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&input).expect("the request")).expect("json");
    assert_eq!(sent["head"], "takt/c1", "{sent}");
    assert_eq!(sent["base"], "main", "{sent}");
    assert_eq!(sent["title"], "deliver the story", "{sent}");
    assert_eq!(sent["body"], "the body", "{sent}");
}

/// What `gh` reports maps onto the three states and the three check results; a reply the
/// adapter cannot read is an error, never a record.
#[test]
fn the_bot_route_adapter_reads_states_and_checks_from_gh() {
    let tree = scratch("gh");
    let read = |view: &'static str, checks: &'static str| {
        let forge = BotRoute::new("owner/repo", None, Box::new(Recording::new(view, checks)));
        (forge.pull_request(&tree, 3), forge.checks(&tree, 3))
    };
    let (state, checks) = read(
        r#"{"mergedAt":"2026-01-01T00:00:00Z","state":"MERGED"}"#,
        r#"[{"bucket":"pass"},{"bucket":"pending"}]"#,
    );
    assert_eq!(state.expect("reads"), PullRequestState::Merged);
    assert_eq!(checks.expect("reads"), Checks::Pending);
    let (state, checks) = read(
        r#"{"mergedAt":null,"state":"CLOSED"}"#,
        r#"[{"bucket":"pass"},{"bucket":"fail"},{"bucket":"pending"}]"#,
    );
    assert_eq!(state.expect("reads"), PullRequestState::Closed);
    assert_eq!(checks.expect("reads"), Checks::Fail);
    let (_, checks) = read(r#"{"state":"OPEN"}"#, "[]");
    assert_eq!(checks.expect("reads"), Checks::Pending);
    let (state, checks) = read("not json", "not json");
    assert!(state.is_err());
    assert!(checks.is_err());
}

/// Without a policy file the adapter omits `--policy`; `b10x-gates` reads its own environment.
#[test]
fn the_bot_route_adapter_omits_the_policy_when_none_is_given() {
    let forge = BotRoute::new("owner/repo", None, Box::new(Recording::new("", "")));
    let line = forge
        .open_argv(Path::new("in.json"), Path::new("out.json"))
        .join(" ");
    assert_eq!(
        line,
        "b10x-gates api --method POST --path /repos/owner/repo/pulls --input in.json --output out.json --repository owner/repo"
    );
}

/// The `<owner>/<repo>` of an `origin` URL in the forms git writes them; a local path is none.
#[test]
fn the_repository_is_read_from_the_origin_url() {
    for url in [
        "git@github.com:owner/repo.git",
        "git@github.com:owner/repo",
        "ssh://git@github.com/owner/repo.git",
        "https://github.com/owner/repo.git",
        "https://github.com/owner/repo",
        "https://github.com/owner/repo/",
    ] {
        assert_eq!(repository_of(url).as_deref(), Some("owner/repo"), "{url}");
    }
    for url in ["/var/lib/git/repo", "../repo", "repo", ""] {
        assert_eq!(repository_of(url), None, "{url}");
    }
}
