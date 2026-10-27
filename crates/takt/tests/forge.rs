//! The forge seam through the loop: a run on the built-in `story-delivery@1` with no harness,
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
use takt::forge::gitlab::{Glab, encoded, project_of};
use takt::forge::{
    BotRoute, Checks, Exit, Forge, Head, LOGIN_LOCATIONS, Memory, Outgoing, PullRequest,
    PullRequestState, PullRequestView, Runner, repository_of, without_userinfo,
};
use takt::harness::{Harness, StepContext, StepReport};
use takt::run::{
    GITLAB_CI, GITLAB_DIR, GITLAB_TOKEN_PREFIXES, JUDGE_SURFACE, JudgeSurface, credential_shape,
};
use takt::scan::Scan;
use takt::validator::{CiRead, PrOpen, Validator};
use takt::{Decision, Run, RunOptions, Stop};

/// The protocol under test, the engineering protocols' built-in.
const PROTOCOL: &str = "story.delivery/1";

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

/// One commit on the case's branch beyond its base, as an implementation step leaves it, and the
/// case's `implementation` moved to it; answers the new revision.
fn implemented(tree: &Path, case: &mut takt::CaseDir) -> String {
    implemented_at(tree, case, "implemented.txt")
}

/// [`implemented`], the commit writing `path`.
fn implemented_at(tree: &Path, case: &mut takt::CaseDir, path: &str) -> String {
    let at = tree.join(path);
    fs::create_dir_all(at.parent().expect("a parent")).expect("a directory");
    fs::write(&at, "done\n").expect("a file");
    git(tree, &["add", path]);
    git(tree, &["commit", "-q", "-m", "implemented by the test"]);
    let revision = git(tree, &["rev-parse", "HEAD"]);
    case.set_revision("implementation", &revision)
        .expect("the revision moves");
    revision
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
/// only validators, the tests and the gate already green, the change handed over and a commit
/// beyond the base recorded at HEAD (four records), two steps at most: the budget stops the run
/// before a judgement step would be offered to the harness.
fn delivered(tree: &Path, id: &str, forge: Rc<dyn Forge>) -> Outcome {
    delivered_at(tree, id, forge, "implemented.txt")
}

/// [`delivered`], the implementation commit writing `path`.
fn delivered_at(tree: &Path, id: &str, forge: Rc<dyn Forge>, path: &str) -> Outcome {
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
    let revision = implemented_at(tree, &mut run.case, path);
    run.case
        .add_evidence(&record(1, "test_result", "pass", &revision))
        .expect("the record is written");
    run.case
        .add_evidence(&record(2, "gate_result", "pass", &revision))
        .expect("the record is written");
    run.case
        .add_evidence(&takt::evidence::Record::new(
            format!("handover-{}", &revision[..8]),
            "handover",
            None,
            "implementation",
            &revision,
            takt::run::CHOICE_PRODUCER,
        ))
        .expect("the record is written");
    // A change to publish at the revision, as `change.read` records it.
    run.case
        .add_evidence(&takt::evidence::Record::new(
            format!("change-record-{}", &revision[..8]),
            "change_record",
            Some("ahead".to_owned()),
            "implementation",
            &revision,
            "takt.change-read",
        ))
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

/// The stop of a delivery that reached the merge with its checks passing: only the merge's
/// grant moves the case, and the run stops there ahead of its step budget
/// (approval-record:loop-pre-checks-ahead-of-loom).
fn merge_waits() -> Stop {
    Stop::ApprovalRequired(vec!["repository.merge".to_owned()])
}

/// The story's acceptance: with the double reporting the pull request open and its checks
/// passing, the run writes a `pull_request` record with result `open` and a `ci_result` record
/// with result `pass`, both about `implementation` at the pushed revision, and the decision
/// shows `pr.opened` and `ci.green` true and `repository.merge` approval-required.
#[test]
fn with_checks_passing_the_pull_request_and_the_ci_result_are_recorded_at_the_pushed_revision() {
    let tree = scratch("pass");
    let memory = Rc::new(Memory {
        checks: Checks::Pass,
        ..Memory::default()
    });
    let outcome = delivered(&tree, "pass", memory.clone());
    let head = git(&tree, &["rev-parse", "HEAD"]);
    assert_eq!(outcome.stop, merge_waits(), "{}", outcome.text);
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
    assert_eq!(outcome.records.len(), 6, "{}", outcome.text);
    let pull_request = &outcome.records[4];
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
    let ci = &outcome.records[5];
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
        [
            "push takt/pass",
            "open takt/pass #1",
            "view #1",
            "checks #1"
        ],
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
/// so, and leaves `ci.green` unknown; the pull request is still recorded open. The run's step
/// budget ends while it waits on the checks: it stops `Waiting` on `ci.read`
/// (story:ci-pending-waits), not on the bare budget.
#[test]
fn with_checks_pending_no_ci_result_is_written_and_ci_green_stays_unknown() {
    let tree = scratch("pending");
    let memory = Rc::new(Memory {
        checks: Checks::Pending,
        ..Memory::default()
    });
    let outcome = delivered(&tree, "pending", memory.clone());
    assert_eq!(
        outcome.stop,
        Stop::Waiting {
            on: vec!["ci.read".to_owned()],
            budget: Some(Box::new(Stop::Iterations(2))),
            local: Vec::new(),
        },
        "{}",
        outcome.text
    );
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
    assert_eq!(outcome.records.len(), 5, "{}", outcome.text);
    assert_eq!(outcome.records[4]["kind"], "pull_request");
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
        [
            "push takt/pending",
            "open takt/pending #1",
            "view #1",
            "checks #1"
        ]
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
    assert_eq!(outcome.records.len(), 6, "{}", outcome.text);
    assert_eq!(outcome.records[5]["result"], "fail");
    assert_eq!(outcome.records[5]["producer"], "takt.ci-read");
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
/// about continuous integration needs a pull request head to be about. Since
/// story:pr-opens-when-change-is-done the protocol does not admit `ci.read` before `pr.opened`
/// holds, so the loop never takes it there: the validator is observed directly, and the run
/// shows it blocked and takes no step for it.
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
    let validators: Vec<Box<dyn Validator>> = vec![Box::new(CiRead::new(forge.clone()))];
    let mut run = Run::open(&tree, options, &NoHarness, &validators).expect("the case opens");
    let revision = run
        .case
        .revision("implementation")
        .expect("implementation has a revision");
    let observation = CiRead::new(forge)
        .observe(&tree, &revision, 1)
        .expect("observes");
    assert!(observation.records.is_empty(), "{}", observation.summary);
    assert!(
        observation.summary.contains("no pull request is open"),
        "{}",
        observation.summary
    );
    let mut out = Vec::new();
    let stop = run.run(&mut out);
    let text = String::from_utf8(out).expect("utf-8");
    assert!(
        stop.is_err(),
        "the harness is left a judgement step: {text}"
    );
    assert!(text.contains("ci.read (blocked)"), "{text}");
    assert!(!text.contains("ci.read {}"), "{text}");
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
    let protocol = takt::LoadedProtocol::builtin(PROTOCOL).expect("the protocol loads");
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
    assert!(!tree.join(".takt/cases").exists());
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
    /// Runs the line as [`Runner::run`] does; the double starts no process, so `env` is
    /// dropped.
    fn run_env(
        &self,
        tree: &std::path::Path,
        argv: &[String],
        _: &[(&str, &str)],
    ) -> anyhow::Result<takt::forge::Exit> {
        self.run(tree, argv)
    }

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
    git(&tree, &["branch", "takt/c1"]);
    let c1 = git(&tree, &["rev-parse", "takt/c1"]);
    let recording = Rc::new(Recording::new(
        r#"{"mergedAt":null,"state":"OPEN","headRefName":"takt/c1","headRefOid":"abc123","baseRefName":"trunk"}"#,
        r#"[{"bucket":"pass"},{"bucket":"skipping"}]"#,
    ));
    struct Shared(Rc<Recording>);
    impl Runner for Shared {
        /// Runs the line as [`Runner::run`] does; the double starts no process, so `env` is
        /// dropped.
        fn run_env(
            &self,
            tree: &std::path::Path,
            argv: &[String],
            _: &[(&str, &str)],
        ) -> anyhow::Result<takt::forge::Exit> {
            self.run(tree, argv)
        }

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
        PullRequestView {
            state: PullRequestState::Open,
            head: Head {
                branch: "takt/c1".to_owned(),
                sha: "abc123".to_owned(),
            },
            base: "trunk".to_owned(),
        }
    );
    assert_eq!(forge.checks(&tree, number).expect("reads"), Checks::Pass);
    let input = dir.join("pull-request.request.json");
    let output = dir.join("pull-request.response.json");
    assert_eq!(
        recording.lines.borrow().as_slice(),
        [
            format!(
                "b10x-gates bot -- push https://github.com/owner/repo.git {c1}:refs/heads/takt/c1 --no-follow-tags --recurse-submodules=no"
            ),
            format!(
                "b10x-gates scan-text --repository owner/repo --policy policy.yaml {} {}",
                input.display(),
                dir.join("pull-request.published.txt").display()
            ),
            format!(
                "b10x-gates api --method POST --path /repos/owner/repo/pulls --input {} --output {} --repository owner/repo --policy policy.yaml",
                input.display(),
                output.display()
            ),
            "gh pr view 7 --json state,mergedAt,headRefName,headRefOid,baseRefName --repo owner/repo"
                .to_owned(),
            "gh pr checks 7 --json bucket --repo owner/repo".to_owned(),
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
        r#"{"mergedAt":"2026-01-01T00:00:00Z","state":"MERGED","headRefName":"takt/c3","headRefOid":"def","baseRefName":"main"}"#,
        r#"[{"bucket":"pass"},{"bucket":"pending"}]"#,
    );
    let view = state.expect("reads");
    assert_eq!(view.state, PullRequestState::Merged);
    assert_eq!(view.head.branch, "takt/c3");
    assert_eq!(view.head.sha, "def");
    assert_eq!(checks.expect("reads"), Checks::Pending);
    let (state, checks) = read(
        r#"{"mergedAt":null,"state":"CLOSED","headRefName":"takt/c3","headRefOid":"def","baseRefName":"main"}"#,
        r#"[{"bucket":"pass"},{"bucket":"fail"},{"bucket":"pending"}]"#,
    );
    assert_eq!(state.expect("reads").state, PullRequestState::Closed);
    assert_eq!(checks.expect("reads"), Checks::Fail);
    let (_, checks) = read(r#"{"state":"OPEN"}"#, "[]");
    assert_eq!(checks.expect("reads"), Checks::Pending);
    // A reply without the head is no reading: the validators check the head before they trust
    // a number.
    let (state, _) = read(r#"{"mergedAt":null,"state":"OPEN"}"#, "[]");
    let error = format!("{:#}", state.expect_err("no head"));
    assert!(error.contains("headRefName"), "{error}");
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
        let origin = repository_of(url).unwrap_or_else(|| panic!("{url} names a repository"));
        assert_eq!(origin.repository, "owner/repo", "{url}");
        assert_eq!(origin.host, "github.com", "{url}");
    }
    let origin = repository_of("ssh://git@github.com:22/owner/repo.git").expect("a repository");
    assert_eq!(
        (origin.host.as_str(), origin.repository.as_str()),
        ("github.com", "owner/repo")
    );
    // Another host is named as such: the bot route is not its forge.
    for url in [
        "git@gitlab.example.com:owner/repo.git",
        "https://gitlab.example.com/owner/repo.git",
    ] {
        let origin = repository_of(url).unwrap_or_else(|| panic!("{url} names a repository"));
        assert_eq!(origin.host, "gitlab.example.com", "{url}");
        assert_eq!(origin.repository, "owner/repo", "{url}");
    }
    for url in [
        "/var/lib/git/repo",
        "../repo",
        "repo",
        "",
        "file:///srv/git/owner/repo.git",
    ] {
        assert_eq!(repository_of(url), None, "{url}");
    }
}

// Adversary pass 1 over story:forge-pull-request-evidence. Every helper below is this pass's
// own; nothing above is changed.

/// A forge double that answers for any pull request number, open with passing checks, and logs
/// every call: what a number planted in `branch.yaml` reaches when nothing checks that the
/// number names a pull request the kernel opened for the case's branch.
struct AnyNumber {
    calls: RefCell<Vec<String>>,
}

impl Forge for AnyNumber {
    /// Scans nothing: the double publishes nothing off the machine.
    fn scan_outgoing(
        &self,
        _: &std::path::Path,
        _: &std::path::Path,
        _: &takt::forge::Outgoing<'_>,
    ) -> anyhow::Result<()> {
        Ok(())
    }

    fn name(&self) -> &str {
        "any"
    }

    fn push(&self, _: &Path, branch: &str) -> anyhow::Result<()> {
        self.calls.borrow_mut().push(format!("push {branch}"));
        Ok(())
    }

    fn open_pull_request(&self, _: &Path, _: &Path, request: &PullRequest) -> anyhow::Result<u64> {
        self.calls
            .borrow_mut()
            .push(format!("open {} #1", request.branch));
        Ok(1)
    }

    fn pull_request(&self, _: &Path, number: u64) -> anyhow::Result<PullRequestView> {
        self.calls.borrow_mut().push(format!("view #{number}"));
        Ok(PullRequestView {
            state: PullRequestState::Open,
            head: Head {
                branch: "someone/else".to_owned(),
                sha: "0".repeat(40),
            },
            base: "main".to_owned(),
        })
    }

    fn checks(&self, _: &Path, number: u64) -> anyhow::Result<Checks> {
        self.calls.borrow_mut().push(format!("checks #{number}"));
        Ok(Checks::Pass)
    }

    fn merge(&self, _: &Path, _: &Path, number: u64, _: &str) -> anyhow::Result<()> {
        self.calls.borrow_mut().push(format!("merge #{number}"));
        Ok(())
    }
}

/// `pr.open` on a case opened from a detached `HEAD` has no base to open the pull request
/// against and refuses; nothing may leave the machine before the refusal. A detached tree is
/// what a CI checkout or `git checkout <sha>` leaves, and the bot route is the default forge
/// whenever `origin` exists.
#[test]
fn adv_pr_open_on_a_detached_head_pushes_nothing_before_refusing() {
    let tree = scratch("adv-detached");
    git(&tree, &["checkout", "-q", "--detach"]);
    let protocol = takt::LoadedProtocol::builtin(PROTOCOL).expect("the protocol loads");
    let case = takt::CaseDir::open(&tree, protocol, "adv-detached", "deliver the story", &[])
        .expect("the case opens");
    let revision = case
        .revision("implementation")
        .expect("implementation has a revision");
    assert_eq!(Branch::read(&case.dir).expect("the branch file").base, None);
    let memory = Rc::new(Memory::default());
    let error = format!(
        "{:#}",
        PrOpen::new(memory.clone())
            .observe(&tree, &revision, 1)
            .expect_err("no base branch: refused")
    );
    assert!(error.contains("base"), "{error}");
    assert!(
        memory.calls.borrow().is_empty(),
        "the branch was pushed before the refusal: {:?}",
        memory.calls.borrow()
    );
}

/// `pr.open` on a case branch with no commit beyond its base pushes nothing and writes no
/// record: the forge has nothing to compare there and refuses the pull request.
#[test]
fn pr_open_at_the_untouched_base_pushes_nothing_and_writes_no_record() {
    let tree = scratch("untouched");
    let protocol = takt::LoadedProtocol::builtin(PROTOCOL).expect("the protocol loads");
    let case = takt::CaseDir::open(&tree, protocol, "untouched", "deliver the story", &[])
        .expect("the case opens");
    let revision = case
        .revision("implementation")
        .expect("implementation has a revision");
    let memory = Rc::new(Memory::default());
    let observation = PrOpen::new(memory.clone())
        .observe(&tree, &revision, 1)
        .expect("observes");
    assert!(observation.records.is_empty(), "{}", observation.summary);
    assert!(
        observation.summary.contains("no commit beyond main"),
        "{}",
        observation.summary
    );
    assert!(
        memory.calls.borrow().is_empty(),
        "{:?}",
        memory.calls.borrow()
    );
    assert_eq!(
        Branch::read(&case.dir).expect("the branch file").pushed,
        None
    );
}

/// Through the loop, with the tests and the gate green at the untouched base, the harness is
/// offered a judgement step before `pr.open` reaches the forge.
#[test]
fn at_the_untouched_base_the_harness_is_offered_a_step_before_anything_is_pushed() {
    let tree = scratch("untouched-loop");
    let memory = Rc::new(Memory::default());
    let forge: Rc<dyn Forge> = memory.clone();
    let options = RunOptions {
        protocol: PROTOCOL.to_owned(),
        intent: "deliver the story".to_owned(),
        inputs: Vec::new(),
        iterations: Some(3),
        hours: None,
        tokens: None,
        case_id: Some("untouched-loop".to_owned()),
        gate: Some("repository.merge".to_owned()),
    };
    let validators: Vec<Box<dyn Validator>> = vec![
        Box::new(PrOpen::new(forge.clone())),
        Box::new(CiRead::new(forge)),
    ];
    let mut run = Run::open(&tree, options, &NoHarness, &validators).expect("the case opens");
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
    run.case
        .add_evidence(&takt::evidence::Record::new(
            format!("handover-{}", &revision[..8]),
            "handover",
            None,
            "implementation",
            &revision,
            takt::run::CHOICE_PRODUCER,
        ))
        .expect("the record is written");
    // A change to publish at the revision, as `change.read` records it.
    run.case
        .add_evidence(&takt::evidence::Record::new(
            format!("change-record-{}", &revision[..8]),
            "change_record",
            Some("ahead".to_owned()),
            "implementation",
            &revision,
            "takt.change-read",
        ))
        .expect("the record is written");
    let mut out = Vec::new();
    let error = format!(
        "{:#}",
        run.run(&mut out)
            .expect_err("NoHarness refuses the judgement step")
    );
    let text = String::from_utf8(out).expect("utf-8");
    assert!(error.contains("implementation.edit"), "{error}\n{text}");
    assert!(
        memory.calls.borrow().is_empty(),
        "{:?}\n{text}",
        memory.calls.borrow()
    );
}

/// A case opened while the tree is on another case's branch (the state every finished run
/// leaves the tree in) is discarded back to the branch it left, as `CaseDir::discard`
/// documents, not to the base it lent from the other case; the other case keeps its directory.
#[test]
fn adv_a_case_discarded_over_another_cases_branch_returns_the_tree_to_that_branch() {
    let tree = scratch("adv-over");
    let protocol = takt::LoadedProtocol::builtin(PROTOCOL).expect("the protocol loads");
    let first = takt::CaseDir::open(
        &tree,
        protocol.clone(),
        "adv-over-a",
        "deliver the story",
        &[],
    )
    .expect("the first case opens");
    assert_eq!(
        git(&tree, &["rev-parse", "--abbrev-ref", "HEAD"]),
        "takt/adv-over-a"
    );
    let second = takt::CaseDir::open(&tree, protocol, "adv-over-b", "deliver another", &[])
        .expect("the second case opens over the first's branch");
    assert_eq!(
        git(&tree, &["rev-parse", "--abbrev-ref", "HEAD"]),
        "takt/adv-over-b"
    );
    assert_eq!(
        Branch::read(&second.dir)
            .expect("the branch file")
            .base
            .as_deref(),
        Some("main")
    );
    second.discard().expect("the second case is discarded");
    assert!(
        first.dir.join("case.yaml").is_file(),
        "the first case lost its directory"
    );
    assert_eq!(git(&tree, &["branch", "--list", "takt/adv-over-b"]), "");
    assert_eq!(
        git(&tree, &["rev-parse", "--abbrev-ref", "HEAD"]),
        "takt/adv-over-a",
        "the tree is not back on the branch the case left"
    );
}

/// A harness that, in the one judgement step that makes the tests pass, also plants a pull
/// request number in the case's `branch.yaml`: the case directory is outside the kernel's
/// change detection, its commit and its discard, so the kernel sees one changed path,
/// `ok.txt`. Once `ci.green` holds it asks for the merge.
struct Planter;

impl Harness for Planter {
    fn name(&self) -> &str {
        "planter"
    }

    fn step(&self, context: &StepContext<'_>) -> Result<StepReport, anyhow::Error> {
        if context.decision.holds("ci.green") {
            return Ok(StepReport {
                action: "repository.merge".to_owned(),
                summary: "merge it".to_owned(),
                tokens: None,
            });
        }
        assert!(
            context.offered.contains(&"implementation.edit".to_owned()),
            "{:?}",
            context.offered
        );
        fs::write(context.tree.join("ok.txt"), "ok\n")?;
        let dir = current(context.tree)?;
        let mut branch = Branch::read(&dir)?;
        branch.pull_request = Some(7);
        branch.write(&dir)?;
        Ok(StepReport {
            action: "implementation.edit".to_owned(),
            summary: "made the tests pass".to_owned(),
            tokens: None,
        })
    }
}

/// The story's outcome: `pr.opened` and `ci.green` become true only from what the forge
/// reports about the revision, never from what the harness says. A number the harness wrote
/// into `branch.yaml` is what `pr.open` reads the state of and `ci.read` reads the checks of:
/// no pull request is ever opened for the case's branch, and the run still asks for the merge.
#[test]
fn adv_a_pull_request_number_planted_by_the_harness_does_not_earn_pr_opened_or_ci_green() {
    let tree = scratch("adv-planted");
    fs::create_dir_all(tree.join(".engineering")).expect("a directory");
    fs::write(
        tree.join(".engineering/gates.yaml"),
        "format: engineering-gates/1\nlanguage: canon-expr/1\ngates:\n  - id: readme\n    \
         text: The repository has its readme.\n    assertions:\n      \
         - 'fs.file.exists(\"README.md\")'\n",
    )
    .expect("a gates document");
    git(&tree, &["add", "-A"]);
    git(&tree, &["commit", "-q", "-m", "gates"]);
    let any = Rc::new(AnyNumber {
        calls: RefCell::new(Vec::new()),
    });
    let forge: Rc<dyn Forge> = any.clone();
    let options = RunOptions {
        protocol: PROTOCOL.to_owned(),
        intent: "deliver the story".to_owned(),
        inputs: Vec::new(),
        iterations: Some(12),
        hours: None,
        tokens: None,
        case_id: Some("adv-planted".to_owned()),
        gate: Some("repository.merge".to_owned()),
    };
    let validators: Vec<Box<dyn Validator>> = vec![
        Box::new(takt::validator::TestsRun::new("cat ok.txt").expect("a command")),
        Box::new(takt::validator::GateRun::new(".engineering/gates.yaml")),
        Box::new(PrOpen::new(forge.clone())),
        Box::new(CiRead::new(forge)),
    ];
    let mut run = Run::open(&tree, options, &Planter, &validators).expect("the case opens");
    let mut out = Vec::new();
    let stop = run.run(&mut out).expect("the run stops without an error");
    let text = String::from_utf8(out).expect("utf-8");
    let decision = run.case.evaluate(99).expect("evaluates");
    let calls = any.calls.borrow().clone();
    assert!(
        !calls.iter().any(|call| call.starts_with("open ")),
        "a pull request was opened for the branch: {calls:?}\n{text}"
    );
    assert_ne!(
        claim(&decision, "pr.opened"),
        "true",
        "pr.opened holds with no pull request opened for the case's branch; the forge was asked {calls:?}; stopped {stop}\n{text}"
    );
    assert_ne!(
        claim(&decision, "ci.green"),
        "true",
        "ci.green holds from the checks of a pull request the harness named; the forge was asked {calls:?}; stopped {stop}\n{text}"
    );
}

/// At a second implementation revision the case's branch is pushed again, the one pull
/// request's state is read, and no second pull request is opened; `ci.read` then reads the
/// checks at the new revision. One pull request per case.
#[test]
fn adv_a_second_revision_pushes_again_and_opens_no_second_pull_request() {
    let tree = scratch("adv-second");
    let memory = Rc::new(Memory {
        checks: Checks::Pass,
        ..Memory::default()
    });
    let outcome = delivered(&tree, "adv-second", memory.clone());
    assert_eq!(outcome.stop, merge_waits(), "{}", outcome.text);
    fs::write(tree.join("README.md"), "second\n").expect("a change");
    git(&tree, &["commit", "-q", "-am", "second"]);
    let second = git(&tree, &["rev-parse", "HEAD"]);
    assert_ne!(second, outcome.revision);
    let observation = PrOpen::new(memory.clone())
        .observe(&tree, &second, 5)
        .expect("pushes again");
    assert_eq!(observation.records.len(), 1, "{}", observation.summary);
    assert_eq!(observation.records[0].result.as_deref(), Some("open"));
    assert_eq!(observation.records[0].subject_revision, second);
    let observation = CiRead::new(memory.clone())
        .observe(&tree, &second, 6)
        .expect("reads the checks");
    assert_eq!(observation.records.len(), 1, "{}", observation.summary);
    assert_eq!(observation.records[0].subject_revision, second);
    assert_eq!(
        memory.calls.borrow().as_slice(),
        [
            "push takt/adv-second",
            "open takt/adv-second #1",
            "view #1",
            "checks #1",
            "push takt/adv-second",
            "view #1",
            "view #1",
            "checks #1",
        ]
    );
    assert_eq!(memory.opened.borrow().len(), 1);
    let branch = Branch::read(&outcome.case_dir).expect("the branch file");
    assert_eq!(branch.pushed.as_deref(), Some(second.as_str()));
    assert_eq!(branch.pull_request, Some(1));
}

/// At a second revision the `pull_request` record carries the state the forge reports, not
/// `open` by construction: a pull request the forge reports closed is recorded `closed`, and
/// `pr.opened` does not hold at that revision.
#[test]
fn adv_a_closed_pull_request_at_a_second_revision_is_recorded_closed() {
    let tree = scratch("adv-closed");
    let memory = Rc::new(Memory {
        checks: Checks::Pass,
        state: PullRequestState::Closed,
        ..Memory::default()
    });
    let forge: Rc<dyn Forge> = memory.clone();
    let options = RunOptions {
        protocol: PROTOCOL.to_owned(),
        intent: "deliver the story".to_owned(),
        inputs: Vec::new(),
        iterations: Some(2),
        hours: None,
        tokens: None,
        case_id: Some("adv-closed".to_owned()),
        gate: Some("repository.merge".to_owned()),
    };
    let validators: Vec<Box<dyn Validator>> = vec![
        Box::new(PrOpen::new(forge.clone())),
        Box::new(CiRead::new(forge)),
    ];
    let mut run = Run::open(&tree, options, &NoHarness, &validators).expect("the case opens");
    let first = implemented(&tree, &mut run.case);
    run.case
        .add_evidence(&record(1, "test_result", "pass", &first))
        .expect("the record is written");
    run.case
        .add_evidence(&record(2, "gate_result", "pass", &first))
        .expect("the record is written");
    run.case
        .add_evidence(&takt::evidence::Record::new(
            format!("handover-{}", &first[..8]),
            "handover",
            None,
            "implementation",
            &first,
            takt::run::CHOICE_PRODUCER,
        ))
        .expect("the record is written");
    // A change to publish at the revision, as `change.read` records it.
    run.case
        .add_evidence(&takt::evidence::Record::new(
            format!("change-record-{}", &first[..8]),
            "change_record",
            Some("ahead".to_owned()),
            "implementation",
            &first,
            "takt.change-read",
        ))
        .expect("the record is written");
    let mut out = Vec::new();
    let stop = run.run(&mut out).expect("the run stops without an error");
    let text = String::from_utf8(out).expect("utf-8");
    assert_eq!(stop, merge_waits(), "{text}");
    assert!(text.contains("evidence: pull_request open"), "{text}");
    fs::write(tree.join("README.md"), "second\n").expect("a change");
    git(&tree, &["commit", "-q", "-am", "second"]);
    let second = git(&tree, &["rev-parse", "HEAD"]);
    run.case
        .set_revision("implementation", &second)
        .expect("the revision moves");
    let observation = PrOpen::new(memory.clone())
        .observe(&tree, &second, 5)
        .expect("pushes again");
    assert_eq!(
        observation.records[0].result.as_deref(),
        Some("closed"),
        "{}",
        observation.summary
    );
    assert!(
        observation.summary.contains("closed"),
        "{}",
        observation.summary
    );
    for record in &observation.records {
        run.case
            .add_evidence(record)
            .expect("the record is written");
    }
    let decision = run.case.evaluate(98).expect("evaluates");
    assert_ne!(claim(&decision, "pr.opened"), "true", "{}", decision.json);
    assert_eq!(
        memory.calls.borrow().as_slice(),
        [
            "push takt/adv-closed",
            "open takt/adv-closed #1",
            "view #1",
            "checks #1",
            "push takt/adv-closed",
            "view #1",
        ]
    );
}

/// A runner whose `scan-text` refuses the request and records every line it is given.
struct Refusing(RefCell<Vec<String>>);

impl Runner for Refusing {
    /// Runs the line as [`Runner::run`] does; the double starts no process, so `env` is
    /// dropped.
    fn run_env(
        &self,
        tree: &std::path::Path,
        argv: &[String],
        _: &[(&str, &str)],
    ) -> anyhow::Result<takt::forge::Exit> {
        self.run(tree, argv)
    }

    fn run(&self, _: &Path, argv: &[String]) -> anyhow::Result<Exit> {
        self.0.borrow_mut().push(argv.join(" "));
        let refused = argv.get(1).map(String::as_str) == Some("scan-text");
        Ok(Exit {
            code: Some(if refused { 1 } else { 0 }),
            stdout: String::new(),
            stderr: if refused {
                "a private rule".to_owned()
            } else {
                String::new()
            },
        })
    }
}

/// A request `scan-text` refuses never reaches the API: the open is an error naming the scan,
/// and the API line is not run.
#[test]
fn adv_a_request_scan_text_refuses_never_reaches_the_api() {
    let tree = scratch("adv-scan");
    let refusing = Rc::new(Refusing(RefCell::new(Vec::new())));
    struct Shared(Rc<Refusing>);
    impl Runner for Shared {
        /// Runs the line as [`Runner::run`] does; the double starts no process, so `env` is
        /// dropped.
        fn run_env(
            &self,
            tree: &std::path::Path,
            argv: &[String],
            _: &[(&str, &str)],
        ) -> anyhow::Result<takt::forge::Exit> {
            self.run(tree, argv)
        }

        fn run(&self, tree: &Path, argv: &[String]) -> anyhow::Result<Exit> {
            self.0.run(tree, argv)
        }
    }
    let forge = BotRoute::new("owner/repo", None, Box::new(Shared(refusing.clone())));
    let request = PullRequest {
        branch: "takt/c1".to_owned(),
        base: "main".to_owned(),
        title: "deliver the story".to_owned(),
        body: "the body".to_owned(),
    };
    let error = format!(
        "{:#}",
        forge
            .open_pull_request(&tree, &tree.join("out"), &request)
            .expect_err("the scan refused")
    );
    assert!(error.contains("scan-text"), "{error}");
    assert!(error.contains("a private rule"), "{error}");
    let lines = refusing.0.borrow();
    assert_eq!(lines.len(), 1, "{lines:?}");
    assert!(lines[0].starts_with("b10x-gates scan-text "), "{lines:?}");
}

/// An API reply without a `number` is an error, never a pull request numbered by default.
#[test]
fn adv_an_api_reply_without_a_number_is_an_error_not_a_pull_request() {
    struct NoNumber;
    impl Runner for NoNumber {
        /// Runs the line as [`Runner::run`] does; the double starts no process, so `env` is
        /// dropped.
        fn run_env(
            &self,
            tree: &std::path::Path,
            argv: &[String],
            _: &[(&str, &str)],
        ) -> anyhow::Result<takt::forge::Exit> {
            self.run(tree, argv)
        }

        fn run(&self, _: &Path, argv: &[String]) -> anyhow::Result<Exit> {
            if argv.get(1).map(String::as_str) == Some("api") {
                let output = argv
                    .iter()
                    .position(|a| a == "--output")
                    .map(|i| &argv[i + 1])
                    .expect("--output");
                fs::write(output, r#"{"message":"Validation Failed","errors":[]}"#)?;
            }
            Ok(Exit {
                code: Some(0),
                stdout: String::new(),
                stderr: String::new(),
            })
        }
    }
    let tree = scratch("adv-no-number");
    let forge = BotRoute::new("owner/repo", None, Box::new(NoNumber));
    let request = PullRequest {
        branch: "takt/c1".to_owned(),
        base: "main".to_owned(),
        title: "deliver the story".to_owned(),
        body: "the body".to_owned(),
    };
    let error = format!(
        "{:#}",
        forge
            .open_pull_request(&tree, &tree.join("out"), &request)
            .expect_err("no number")
    );
    assert!(error.contains("number"), "{error}");
}

/// The two `gh` buckets the unit's cases do not reach: `cancel` is a failure, and a bucket the
/// adapter does not know is an error, never a reading.
#[test]
fn adv_a_cancelled_check_fails_and_an_unknown_bucket_is_an_error() {
    let tree = scratch("adv-buckets");
    let read = |checks: &'static str| {
        BotRoute::new("owner/repo", None, Box::new(Recording::new("", checks))).checks(&tree, 3)
    };
    assert_eq!(
        read(r#"[{"bucket":"pass"},{"bucket":"cancel"}]"#).expect("reads"),
        Checks::Fail
    );
    let error = format!(
        "{:#}",
        read(r#"[{"bucket":"pass"},{"bucket":"neutral"}]"#).expect_err("unknown bucket")
    );
    assert!(error.contains("neutral"), "{error}");
}

// Correction round 1 over story:forge-pull-request-evidence: the branch forks from its base,
// and the forge's report of a pull request's head decides whether a number is trusted.

/// A case opened while the tree is on another case's branch forks from that case's base, at
/// the base's tip: its implementation revision is the base's tip, its first commit's parent is
/// the base's tip, and the pull request is against the base. `left` records the branch the
/// tree was on, for `discard`.
#[test]
fn a_case_opened_over_another_cases_branch_forks_from_the_base_not_from_that_branch() {
    let tree = scratch("fork");
    let main_tip = git(&tree, &["rev-parse", "HEAD"]);
    let protocol = takt::LoadedProtocol::builtin(PROTOCOL).expect("the protocol loads");
    let first = takt::CaseDir::open(&tree, protocol.clone(), "fork-a", "deliver the story", &[])
        .expect("the first case opens");
    fs::write(tree.join("README.md"), "a's work\n").expect("a change");
    git(&tree, &["commit", "-q", "-am", "a's work"]);
    let a_tip = git(&tree, &["rev-parse", "HEAD"]);
    assert_ne!(a_tip, main_tip);
    let second = takt::CaseDir::open(&tree, protocol, "fork-b", "deliver another", &[])
        .expect("the second case opens over the first's branch");
    assert_eq!(
        git(&tree, &["rev-parse", "--abbrev-ref", "HEAD"]),
        "takt/fork-b"
    );
    assert_eq!(
        git(&tree, &["rev-parse", "HEAD"]),
        main_tip,
        "takt/fork-b forks from main's tip, not from takt/fork-a's"
    );
    assert_eq!(
        second.revision("implementation").as_deref(),
        Some(main_tip.as_str())
    );
    let branch = Branch::read(&second.dir).expect("the branch file");
    assert_eq!(branch.base.as_deref(), Some("main"));
    assert_eq!(branch.left.as_deref(), Some("takt/fork-a"));
    fs::write(tree.join("README.md"), "b's work\n").expect("a change");
    git(&tree, &["commit", "-q", "-am", "b's work"]);
    assert_eq!(
        git(&tree, &["rev-parse", "HEAD~1"]),
        main_tip,
        "b's first commit's parent is the base's tip"
    );
    assert!(first.dir.join("case.yaml").is_file());
    assert_eq!(
        git(&tree, &["rev-parse", "takt/fork-a"]),
        a_tip,
        "the first case's branch is untouched"
    );
}

/// A pull request the double records as opened, for the head it is told to report.
fn opened_for(branch: &str) -> RefCell<Vec<PullRequest>> {
    RefCell::new(vec![PullRequest {
        branch: branch.to_owned(),
        base: "main".to_owned(),
        title: String::new(),
        body: String::new(),
    }])
}

/// The forge's report of the pull request's head decides: a head on the case's branch at
/// another commit, or at the revision on another branch, earns no `pull_request` and no
/// `ci_result` record, and neither validator reads the checks.
#[test]
fn a_pull_request_head_the_forge_reports_elsewhere_earns_no_record() {
    let tree = scratch("elsewhere");
    let memory = Rc::new(Memory {
        checks: Checks::Pass,
        ..Memory::default()
    });
    let outcome = delivered(&tree, "elsewhere", memory);
    assert_eq!(outcome.records.len(), 6, "{}", outcome.text);
    fs::write(tree.join("README.md"), "second\n").expect("a change");
    git(&tree, &["commit", "-q", "-am", "second"]);
    let second = git(&tree, &["rev-parse", "HEAD"]);
    let wrong_sha = Rc::new(Memory {
        checks: Checks::Pass,
        head: Some(Head {
            branch: "takt/elsewhere".to_owned(),
            sha: "f".repeat(40),
        }),
        opened: opened_for("takt/elsewhere"),
        ..Memory::default()
    });
    let wrong_branch = Rc::new(Memory {
        checks: Checks::Pass,
        head: Some(Head {
            branch: "takt/other".to_owned(),
            sha: second.clone(),
        }),
        opened: opened_for("takt/other"),
        ..Memory::default()
    });
    for (what, forge) in [("sha", wrong_sha), ("branch", wrong_branch)] {
        let observation = PrOpen::new(forge.clone())
            .observe(&tree, &second, 5)
            .unwrap_or_else(|error| panic!("{what}: pr.open observes: {error:#}"));
        assert!(
            observation.records.is_empty(),
            "{what}: {}",
            observation.summary
        );
        assert!(
            observation
                .summary
                .contains("not the case's branch at the revision"),
            "{what}: {}",
            observation.summary
        );
        let observation = CiRead::new(forge.clone())
            .observe(&tree, &second, 6)
            .unwrap_or_else(|error| panic!("{what}: ci.read observes: {error:#}"));
        assert!(
            observation.records.is_empty(),
            "{what}: {}",
            observation.summary
        );
        assert!(
            observation.summary.contains("nothing to read"),
            "{what}: {}",
            observation.summary
        );
        assert_eq!(
            forge.calls.borrow().as_slice(),
            ["push takt/elsewhere", "view #1", "view #1"],
            "{what}: the checks are never read"
        );
    }
}

// Adversary pass 2 over story:forge-pull-request-evidence (the unit and its correction 1): the
// fork from the base, the forge's report of the pull request's head, and the GitHub-only
// default. Every helper below is this pass's own; nothing above is changed.

/// A harness that, in the one judgement step that makes the tests pass, also rewrites the
/// case's `branch.yaml`: the `name` field becomes a refspec of its choosing. The case directory
/// is outside the kernel's change detection, its commit and its discard, so the kernel sees one
/// changed path, `ok.txt`. Once `ci.green` holds it asks for the merge.
struct Repointer;

impl Harness for Repointer {
    fn name(&self) -> &str {
        "repointer"
    }

    fn step(&self, context: &StepContext<'_>) -> Result<StepReport, anyhow::Error> {
        if context.decision.holds("ci.green") {
            return Ok(StepReport {
                action: "repository.merge".to_owned(),
                summary: "merge it".to_owned(),
                tokens: None,
            });
        }
        fs::write(context.tree.join("ok.txt"), "ok\n")?;
        let dir = current(context.tree)?;
        let mut branch = Branch::read(&dir)?;
        branch.name = "HEAD:main".to_owned();
        branch.write(&dir)?;
        Ok(StepReport {
            action: "implementation.edit".to_owned(),
            summary: "made the tests pass".to_owned(),
            tokens: None,
        })
    }
}

/// A committed `.engineering/gates.yaml` whose one gate holds on the scratch tree.
fn commit_gates(tree: &Path) {
    fs::create_dir_all(tree.join(".engineering")).expect("a directory");
    fs::write(
        tree.join(".engineering/gates.yaml"),
        "format: engineering-gates/1\nlanguage: canon-expr/1\ngates:\n  - id: readme\n    \
         text: The repository has its readme.\n    assertions:\n      \
         - 'fs.file.exists(\"README.md\")'\n",
    )
    .expect("a gates document");
    git(tree, &["add", "-A"]);
    git(tree, &["commit", "-q", "-m", "gates"]);
}

/// `pr.open` pushes the case's branch, `takt/<id>`: the branch the tree is on, the one `current`
/// resolved the case from. It does not push the `name` field of a file the harness can write,
/// because that field reaches `b10x-gates bot -- push <url> <name>` as given: a refspec there
/// (`HEAD:main`, `:main`, `--all`) is what the bot credential then pushes.
#[test]
fn adv2_pr_open_pushes_the_case_branch_not_the_name_the_harness_wrote_into_branch_yaml() {
    let tree = scratch("adv2-repoint");
    commit_gates(&tree);
    let memory = Rc::new(Memory {
        checks: Checks::Pass,
        ..Memory::default()
    });
    let forge: Rc<dyn Forge> = memory.clone();
    let options = RunOptions {
        protocol: PROTOCOL.to_owned(),
        intent: "deliver the story".to_owned(),
        inputs: Vec::new(),
        iterations: Some(12),
        hours: None,
        tokens: None,
        case_id: Some("adv2-repoint".to_owned()),
        gate: Some("repository.merge".to_owned()),
    };
    let validators: Vec<Box<dyn Validator>> = vec![
        Box::new(takt::validator::TestsRun::new("cat ok.txt").expect("a command")),
        Box::new(takt::validator::GateRun::new(".engineering/gates.yaml")),
        Box::new(PrOpen::new(forge.clone())),
        Box::new(CiRead::new(forge)),
    ];
    let mut run = Run::open(&tree, options, &Repointer, &validators).expect("the case opens");
    let mut out = Vec::new();
    run.continue_past_red_base();
    let stop = run.run(&mut out).expect("the run stops without an error");
    let text = String::from_utf8(out).expect("utf-8");
    let calls = memory.calls.borrow().clone();
    let pushes: Vec<&String> = calls.iter().filter(|c| c.starts_with("push ")).collect();
    // Since story:merge-under-authority (correction 1) a judgement step may not touch the case
    // directory at all: the rewrite is refused naming the file, the file is put back, and
    // nothing is pushed. A `branch.yaml` rewritten outside a judgement step is still set right
    // by `pr.open` (`pr_open_sets_branch_yaml_right_when_its_name_was_rewritten`).
    assert!(
        matches!(&stop, Stop::StepRefused(why) if why.contains("branch.yaml")),
        "the step that rewrote branch.yaml was not refused; stopped {stop}\n{text}"
    );
    assert!(
        pushes.is_empty(),
        "the forge was asked to push after the harness wrote into branch.yaml: {calls:?}; stopped {stop}\n{text}"
    );
    assert_eq!(
        Branch::read(&run.case.dir).expect("the branch file").name,
        "takt/adv2-repoint",
        "branch.yaml was not put back"
    );
    assert!(
        pushes.iter().all(|c| *c == "push takt/adv2-repoint"),
        "the forge was asked to push what the harness wrote into branch.yaml: {calls:?}; stopped {stop}\n{text}"
    );
    assert!(
        memory
            .opened
            .borrow()
            .iter()
            .all(|pr| pr.branch == "takt/adv2-repoint"),
        "a pull request was opened for a head the harness named: {:?}",
        memory.opened.borrow()
    );
}

/// Opens case `first` under `tree` (from a detached `HEAD` when `detached`), commits on its
/// branch, and removes the cases directory when `gone` (as `git clean -fdx` does); the tip of
/// its branch.
fn first_case_on_its_branch(tree: &Path, first: &str, detached: bool, gone: bool) -> String {
    if detached {
        git(tree, &["checkout", "-q", "--detach"]);
    }
    let protocol = takt::LoadedProtocol::builtin(PROTOCOL).expect("the protocol loads");
    let case = takt::CaseDir::open(tree, protocol, first, "deliver the story", &[])
        .expect("the first case opens");
    fs::write(tree.join("README.md"), "a's work\n").expect("a change");
    git(tree, &["commit", "-q", "-am", "a's work"]);
    if gone {
        let cases = case.dir.parent().expect("the cases directory");
        fs::remove_dir_all(cases).expect("the cases directory goes, as git clean -fdx leaves it");
    }
    git(tree, &["rev-parse", "HEAD"])
}

/// Opens `second` over the tree as it is; a refusal leaves the tree on `left`, an opened case
/// has its `HEAD` elsewhere than `tip`, the other case's commits.
fn second_case_does_not_fork_from(tree: &Path, second: &str, left: &str, tip: &str, what: &str) {
    let protocol = takt::LoadedProtocol::builtin(PROTOCOL).expect("the protocol loads");
    match takt::CaseDir::open(tree, protocol, second, "deliver another", &[]) {
        Err(_) => {
            assert_eq!(
                git(tree, &["rev-parse", "--abbrev-ref", "HEAD"]),
                left,
                "{what}: a refused case leaves the tree where it was"
            );
            assert!(!tree.join(takt::case::CASES_DIR).join(second).exists());
        }
        Ok(case) => {
            let head = git(tree, &["rev-parse", "HEAD"]);
            assert_ne!(
                head,
                tip,
                "{what}: takt/{second} forks from {left}'s tip, the other case's commits; branch.yaml: {:?}",
                Branch::read(&case.dir)
            );
        }
    }
}

/// A case opened while the tree is on a case branch that records no base (that case was opened
/// from a detached `HEAD`, as a CI checkout leaves it) has no base to fork from. `CaseDir::open`
/// documents that a case never forks from another case's commits: it refuses, or forks
/// elsewhere; it does not start the new case on the other branch's tip.
#[test]
fn adv2_a_case_opened_over_a_case_branch_without_a_base_does_not_fork_from_its_commits() {
    let tree = scratch("adv2-nobase");
    let tip = first_case_on_its_branch(&tree, "adv2-nobase-a", true, false);
    assert_eq!(
        Branch::read(&tree.join(takt::case::CASES_DIR).join("adv2-nobase-a"))
            .expect("the branch file")
            .base,
        None
    );
    second_case_does_not_fork_from(
        &tree,
        "adv2-nobase-b",
        "takt/adv2-nobase-a",
        &tip,
        "no base recorded",
    );
}

/// The same when the case branch the tree is on names no case any more: its directory is gone,
/// as `git clean -fdx` leaves a tree after a run (the directory is untracked). Nothing records
/// a base; the new case must not fork from the leftover branch's commits.
#[test]
fn adv2_a_case_opened_over_a_leftover_case_branch_does_not_fork_from_its_commits() {
    let tree = scratch("adv2-leftover");
    let tip = first_case_on_its_branch(&tree, "adv2-leftover-a", false, true);
    assert!(!tree.join(takt::case::CASES_DIR).exists());
    second_case_does_not_fork_from(
        &tree,
        "adv2-leftover-b",
        "takt/adv2-leftover-a",
        &tip,
        "case directory gone",
    );
}

/// `BotRoute::for_tree` names the origin URL in its refusals. A checkout a CI runner leaves
/// carries a credential in that URL; the credential must not appear in what the refusal says,
/// which the binary prints.
#[test]
fn adv2_a_credential_in_the_origin_url_is_not_printed_by_the_bot_routes_refusal() {
    for (name, url) in [
        (
            "elsewhere",
            "https://ci-user:t0k3n-not-real@gitlab.example.com/owner/repo.git",
        ),
        ("no-owner", "https://ci-user:t0k3n-not-real@github.com/repo"),
    ] {
        let tree = scratch(&format!("adv2-cred-{name}"));
        git(&tree, &["remote", "add", "origin", url]);
        let error = format!(
            "{:#}",
            BotRoute::for_tree(&tree).expect_err("the bot route refuses this origin")
        );
        assert!(
            !error.contains("t0k3n-not-real"),
            "{name}: the refusal prints the credential: {error}"
        );
    }
}

/// A host name is case-insensitive: an origin written as `GitHub.com` is on github.com, and
/// the bot route serves it.
#[test]
fn adv2_an_origin_host_written_in_upper_case_is_github() {
    let tree = scratch("adv2-upper");
    git(
        &tree,
        &[
            "remote",
            "add",
            "origin",
            "https://GitHub.com/owner/repo.git",
        ],
    );
    let forge = BotRoute::for_tree(&tree).unwrap_or_else(|error| {
        panic!("GitHub.com is github.com, a host name being case-insensitive: {error:#}")
    });
    assert_eq!(forge.repository, "owner/repo");
}

/// A chain of cases, each opened over the previous one's branch: every case forks from the
/// shared base at its tip, records that base and the branch it left, and a discarded case
/// returns the tree to the branch it left, the other cases untouched. Only a chain of three
/// tells `base` from `left`.
#[test]
fn adv2_a_chain_of_cases_each_forks_from_the_shared_base_and_discards_to_the_branch_it_left() {
    let tree = scratch("adv2-chain");
    let main_tip = git(&tree, &["rev-parse", "HEAD"]);
    let protocol = takt::LoadedProtocol::builtin(PROTOCOL).expect("the protocol loads");
    let first = takt::CaseDir::open(
        &tree,
        protocol.clone(),
        "adv2-chain-a",
        "deliver the story",
        &[],
    )
    .expect("the first case opens");
    fs::write(tree.join("README.md"), "a's work\n").expect("a change");
    git(&tree, &["commit", "-q", "-am", "a's work"]);
    let a_tip = git(&tree, &["rev-parse", "HEAD"]);
    let second = takt::CaseDir::open(
        &tree,
        protocol.clone(),
        "adv2-chain-b",
        "deliver another",
        &[],
    )
    .expect("the second case opens");
    assert_eq!(git(&tree, &["rev-parse", "HEAD"]), main_tip);
    fs::write(tree.join("README.md"), "b's work\n").expect("a change");
    git(&tree, &["commit", "-q", "-am", "b's work"]);
    let b_tip = git(&tree, &["rev-parse", "HEAD"]);
    let third = takt::CaseDir::open(&tree, protocol, "adv2-chain-c", "deliver a third", &[])
        .expect("the third case opens");
    let branch = Branch::read(&third.dir).expect("the branch file");
    assert_eq!(branch.base.as_deref(), Some("main"), "{branch:?}");
    assert_eq!(
        branch.left.as_deref(),
        Some("takt/adv2-chain-b"),
        "{branch:?}"
    );
    assert_eq!(git(&tree, &["rev-parse", "HEAD"]), main_tip);
    assert_eq!(
        third.revision("implementation").as_deref(),
        Some(main_tip.as_str())
    );
    third.discard().expect("the third case is discarded");
    assert_eq!(
        git(&tree, &["rev-parse", "--abbrev-ref", "HEAD"]),
        "takt/adv2-chain-b"
    );
    assert_eq!(git(&tree, &["rev-parse", "HEAD"]), b_tip);
    assert_eq!(git(&tree, &["rev-parse", "takt/adv2-chain-a"]), a_tip);
    assert!(first.dir.join("case.yaml").is_file());
    assert!(second.dir.join("case.yaml").is_file());
    assert_eq!(git(&tree, &["branch", "--list", "takt/adv2-chain-c"]), "");
}

/// The base moved on (a merge elsewhere) while the tree stayed on the first case's branch: the
/// second case forks from the base's tip as it is now, its implementation revision is that
/// tip, and its tree is the base's, not the first case's.
#[test]
fn adv2_a_case_opened_over_another_cases_branch_forks_from_the_bases_tip_as_it_is_now() {
    let tree = scratch("adv2-moved");
    let old_tip = git(&tree, &["rev-parse", "HEAD"]);
    let protocol = takt::LoadedProtocol::builtin(PROTOCOL).expect("the protocol loads");
    let first = takt::CaseDir::open(
        &tree,
        protocol.clone(),
        "adv2-moved-a",
        "deliver the story",
        &[],
    )
    .expect("the first case opens");
    fs::write(tree.join("README.md"), "a's work\n").expect("a change");
    git(&tree, &["commit", "-q", "-am", "a's work"]);
    let a_tip = git(&tree, &["rev-parse", "HEAD"]);
    git(&tree, &["checkout", "-q", "main"]);
    fs::write(tree.join("NEWS.md"), "merged elsewhere\n").expect("a change");
    git(&tree, &["add", "NEWS.md"]);
    git(&tree, &["commit", "-q", "-m", "merged elsewhere"]);
    let new_tip = git(&tree, &["rev-parse", "HEAD"]);
    assert_ne!(new_tip, old_tip);
    git(&tree, &["checkout", "-q", "takt/adv2-moved-a"]);
    let second = takt::CaseDir::open(&tree, protocol, "adv2-moved-b", "deliver another", &[])
        .expect("the second case opens");
    assert_eq!(git(&tree, &["rev-parse", "HEAD"]), new_tip);
    assert_eq!(
        second.revision("implementation").as_deref(),
        Some(new_tip.as_str())
    );
    let branch = Branch::read(&second.dir).expect("the branch file");
    assert_eq!(branch.base.as_deref(), Some("main"));
    assert_eq!(branch.left.as_deref(), Some("takt/adv2-moved-a"));
    assert!(tree.join("NEWS.md").is_file());
    assert_eq!(
        fs::read_to_string(tree.join("README.md")).expect("the readme"),
        "a story\n"
    );
    assert_eq!(git(&tree, &["rev-parse", "takt/adv2-moved-a"]), a_tip);
    assert!(first.dir.join("case.yaml").is_file());
}

/// A case opened from a detached `HEAD` records no branch to return to; discarding it leaves
/// the tree detached at the commit it was on, the branch and the directory gone.
#[test]
fn adv2_a_case_opened_from_a_detached_head_discards_back_to_that_detached_head() {
    let tree = scratch("adv2-detached-discard");
    git(&tree, &["checkout", "-q", "--detach"]);
    let tip = git(&tree, &["rev-parse", "HEAD"]);
    let protocol = takt::LoadedProtocol::builtin(PROTOCOL).expect("the protocol loads");
    let case = takt::CaseDir::open(&tree, protocol, "adv2-dd", "deliver the story", &[])
        .expect("the case opens");
    let branch = Branch::read(&case.dir).expect("the branch file");
    assert_eq!(branch.left, None, "{branch:?}");
    case.discard().expect("the case is discarded");
    assert_eq!(git(&tree, &["rev-parse", "--abbrev-ref", "HEAD"]), "HEAD");
    assert_eq!(git(&tree, &["rev-parse", "HEAD"]), tip);
    assert_eq!(git(&tree, &["branch", "--list", "takt/adv2-dd"]), "");
    assert!(!tree.join(takt::case::CASES_DIR).exists());
}

/// A dirty tree at open over another case's branch, the dirt in a file that differs between
/// that branch and the base: git refuses the checkout, and the case is not opened: no
/// directory, no branch, the tree still on the other case's branch with its dirt.
#[test]
fn adv2_a_dirty_tree_over_another_cases_branch_is_refused_before_the_case_is_written() {
    let tree = scratch("adv2-dirty");
    let protocol = takt::LoadedProtocol::builtin(PROTOCOL).expect("the protocol loads");
    let first = takt::CaseDir::open(
        &tree,
        protocol.clone(),
        "adv2-dirty-a",
        "deliver the story",
        &[],
    )
    .expect("the first case opens");
    fs::write(tree.join("README.md"), "a's work\n").expect("a change");
    git(&tree, &["commit", "-q", "-am", "a's work"]);
    fs::write(tree.join("README.md"), "uncommitted\n").expect("dirt");
    let error = format!(
        "{:#}",
        takt::CaseDir::open(&tree, protocol, "adv2-dirty-b", "deliver another", &[])
            .expect_err("a dirty tree that the checkout would overwrite is refused")
    );
    assert!(error.contains("README.md"), "{error}");
    assert!(
        !tree
            .join(takt::case::CASES_DIR)
            .join("adv2-dirty-b")
            .exists()
    );
    assert_eq!(git(&tree, &["branch", "--list", "takt/adv2-dirty-b"]), "");
    assert_eq!(
        git(&tree, &["rev-parse", "--abbrev-ref", "HEAD"]),
        "takt/adv2-dirty-a"
    );
    assert_eq!(
        fs::read_to_string(tree.join("README.md")).expect("the readme"),
        "uncommitted\n"
    );
    assert!(first.dir.join("case.yaml").is_file());
}

/// A runner standing in for `gh pr checks` on a pull request whose head has no check run yet:
/// exit 1, nothing on stdout, the message on stderr.
struct NoChecksYet;

impl Runner for NoChecksYet {
    /// Runs the line as [`Runner::run`] does; the double starts no process, so `env` is
    /// dropped.
    fn run_env(
        &self,
        tree: &std::path::Path,
        argv: &[String],
        _: &[(&str, &str)],
    ) -> anyhow::Result<takt::forge::Exit> {
        self.run(tree, argv)
    }

    fn run(&self, _: &Path, argv: &[String]) -> anyhow::Result<Exit> {
        assert_eq!(argv.get(2).map(String::as_str), Some("checks"), "{argv:?}");
        Ok(Exit {
            code: Some(1),
            stdout: String::new(),
            stderr: "no checks reported on the 'takt/c1' branch".to_owned(),
        })
    }
}

/// `gh pr checks` on a head no check has been reported for exits 1 with a message and no
/// JSON; that is the state of every pull request in the seconds after its push, which is when
/// `ci.read` first runs. The adapter reads it as pending, not as an error that ends the run.
#[test]
fn adv2_no_checks_reported_yet_is_pending_not_an_error() {
    let tree = scratch("adv2-no-checks");
    let forge = BotRoute::new("owner/repo", None, Box::new(NoChecksYet));
    let checks = forge.checks(&tree, 7).unwrap_or_else(|error| {
        panic!("no checks reported yet is pending, not an error: {error:#}")
    });
    assert_eq!(checks, Checks::Pending);
}

// Correction round 2 over story:forge-pull-request-evidence: the refusal at open with a forge
// and no base, what takt prints of a remote URL, and the branch name a file cannot change.

/// With a validator for `pr.open` registered, a case opened from a detached `HEAD` has no base
/// to open its pull request against, and that is known at open: the run is refused before its
/// first step, in one line, and nothing is left behind: no case directory, no branch, the tree
/// detached where it was, the forge never asked. Without a forge the same case opens.
#[test]
fn a_run_with_a_forge_on_a_detached_head_is_refused_at_open_and_leaves_nothing() {
    let tree = scratch("detached-run");
    git(&tree, &["checkout", "-q", "--detach"]);
    let tip = git(&tree, &["rev-parse", "HEAD"]);
    let memory = Rc::new(Memory::default());
    let forge: Rc<dyn Forge> = memory.clone();
    let options = || RunOptions {
        protocol: PROTOCOL.to_owned(),
        intent: "deliver the story".to_owned(),
        inputs: Vec::new(),
        iterations: Some(2),
        hours: None,
        tokens: None,
        case_id: Some("detached-run".to_owned()),
        gate: None,
    };
    let validators: Vec<Box<dyn Validator>> = vec![Box::new(PrOpen::new(forge))];
    let error = format!(
        "{:#}",
        Run::open(&tree, options(), &NoHarness, &validators)
            .err()
            .expect("refused at open")
    );
    assert!(error.contains("no base branch"), "{error}");
    assert!(error.contains("detached-run"), "{error}");
    assert!(!tree.join(takt::case::CASES_DIR).exists());
    assert_eq!(git(&tree, &["branch", "--list", "takt/detached-run"]), "");
    assert_eq!(git(&tree, &["rev-parse", "--abbrev-ref", "HEAD"]), "HEAD");
    assert_eq!(git(&tree, &["rev-parse", "HEAD"]), tip);
    assert!(
        memory.calls.borrow().is_empty(),
        "{:?}",
        memory.calls.borrow()
    );
    let none: Vec<Box<dyn Validator>> = Vec::new();
    let run = Run::open(&tree, options(), &NoHarness, &none).expect("opens without a forge");
    assert_eq!(run.case.branch, "takt/detached-run");
}

/// What takt prints of a remote URL carries no userinfo: the credential a CI checkout leaves
/// in its origin never reaches an error or a line of output.
#[test]
fn a_remote_url_is_shown_without_its_userinfo() {
    for (url, shown) in [
        (
            "https://ci-user:t0k3n-not-real@github.com/owner/repo.git",
            "https://github.com/owner/repo.git",
        ),
        (
            "ssh://git@github.com:22/owner/repo.git",
            "ssh://github.com:22/owner/repo.git",
        ),
        ("git@github.com:owner/repo.git", "github.com:owner/repo.git"),
        (
            "https://github.com/owner/repo",
            "https://github.com/owner/repo",
        ),
        ("/var/lib/git/repo", "/var/lib/git/repo"),
        (
            "file:///srv/git/owner/repo.git",
            "file:///srv/git/owner/repo.git",
        ),
    ] {
        assert_eq!(without_userinfo(url), shown, "{url}");
    }
}

/// `pr.open` pushes and opens for the case's branch from the case id; a `name` a harness wrote
/// into `branch.yaml` never reaches the forge, the file is set right, and the summary says so.
#[test]
fn pr_open_sets_branch_yaml_right_when_its_name_was_rewritten() {
    let tree = scratch("renamed");
    let memory = Rc::new(Memory {
        checks: Checks::Pass,
        ..Memory::default()
    });
    let outcome = delivered(&tree, "renamed", memory.clone());
    assert_eq!(outcome.records.len(), 6, "{}", outcome.text);
    let mut branch = Branch::read(&outcome.case_dir).expect("the branch file");
    branch.name = "HEAD:main".to_owned();
    branch
        .write(&outcome.case_dir)
        .expect("the harness's write");
    fs::write(tree.join("README.md"), "second\n").expect("a change");
    git(&tree, &["commit", "-q", "-am", "second"]);
    let second = git(&tree, &["rev-parse", "HEAD"]);
    let observation = PrOpen::new(memory.clone())
        .observe(&tree, &second, 5)
        .expect("observes");
    assert_eq!(observation.records.len(), 1, "{}", observation.summary);
    assert!(
        observation
            .summary
            .contains("branch.yaml named HEAD:main, not the case's branch; set right"),
        "{}",
        observation.summary
    );
    let branch = Branch::read(&outcome.case_dir).expect("the branch file");
    assert_eq!(branch.name, "takt/renamed");
    assert_eq!(branch.pushed.as_deref(), Some(second.as_str()));
    let calls = memory.calls.borrow();
    assert!(
        calls
            .iter()
            .filter(|call| call.starts_with("push "))
            .all(|call| call == "push takt/renamed"),
        "{calls:?}"
    );
    assert_eq!(calls.last().map(String::as_str), Some("view #1"));
}

/// A runner that answers the dry-run fetch with the porcelain line for `main` at `tip`.
struct Fetching {
    lines: RefCell<Vec<String>>,
    tip: String,
}

impl Runner for Fetching {
    fn run_env(&self, tree: &Path, argv: &[String], _: &[(&str, &str)]) -> anyhow::Result<Exit> {
        self.run(tree, argv)
    }

    fn run(&self, _: &Path, argv: &[String]) -> anyhow::Result<Exit> {
        self.lines.borrow_mut().push(argv.join(" "));
        Ok(Exit {
            code: Some(0),
            stdout: format!(
                "  {} {} refs/remotes/origin/main\n",
                "0".repeat(40),
                self.tip
            ),
            stderr: String::new(),
        })
    }
}

/// D7: the bot route reports the target's tip through its dry-run fetch of the base branch,
/// then fetches that commit into no ref, so the tree's repository holds it.
#[test]
fn the_bot_route_reports_the_target_tip_through_a_dry_run_fetch() {
    let tree = scratch("bot-tip");
    let tip = "a".repeat(40);
    let runner = Rc::new(Fetching {
        lines: RefCell::new(Vec::new()),
        tip: tip.clone(),
    });
    struct Shared(Rc<Fetching>);
    impl Runner for Shared {
        fn run_env(
            &self,
            tree: &Path,
            argv: &[String],
            env: &[(&str, &str)],
        ) -> anyhow::Result<Exit> {
            self.0.run_env(tree, argv, env)
        }
        fn run(&self, tree: &Path, argv: &[String]) -> anyhow::Result<Exit> {
            self.0.run(tree, argv)
        }
    }
    let forge = BotRoute::new("owner/repo", None, Box::new(Shared(runner.clone())));
    let answered = forge.target_tip(&tree, "main").expect("a tip");
    assert_eq!(answered, tip);
    let lines = runner.lines.borrow();
    assert_eq!(lines.len(), 2, "{lines:?}");
    assert!(lines[0].contains("--dry-run"), "{lines:?}");
    assert!(
        lines[0].contains("refs/heads/main:refs/remotes/origin/main"),
        "{lines:?}"
    );
    assert_eq!(
        lines[1],
        format!(
            "b10x-gates bot -- fetch https://github.com/owner/repo.git {tip} --no-write-fetch-head --no-tags --recurse-submodules=no"
        ),
        "{lines:?}"
    );
}

/// A runner that runs the bot route's lines as plain git against the bare repository `remote`
/// in place of the repository's URL: the dry-run fetch answered from `git ls-remote`, which
/// stores no object, as a dry run is documented to change nothing; every other line run as
/// git. The environment is dropped: it allows HTTPS alone, and `remote` is a path.
struct LocalRemote {
    remote: PathBuf,
    url: String,
    lines: RefCell<Vec<String>>,
}

impl Runner for LocalRemote {
    fn run_env(&self, tree: &Path, argv: &[String], _: &[(&str, &str)]) -> anyhow::Result<Exit> {
        self.run(tree, argv)
    }

    fn run(&self, tree: &Path, argv: &[String]) -> anyhow::Result<Exit> {
        self.lines.borrow_mut().push(argv.join(" "));
        let [bot, sub, dashes, rest @ ..] = argv else {
            return Err(anyhow!("not a bot line: {argv:?}"));
        };
        assert_eq!([bot, sub, dashes], ["b10x-gates", "bot", "--"], "{argv:?}");
        let args: Vec<String> = rest
            .iter()
            .map(|arg| {
                if *arg == self.url {
                    self.remote.display().to_string()
                } else {
                    arg.clone()
                }
            })
            .collect();
        if args.iter().any(|arg| arg == "--dry-run") {
            let (source, target) = args[2].split_once(':').expect("a refspec");
            let listed = git(&self.remote, &["ls-remote", ".", source]);
            let new = listed.split_whitespace().next().expect("the branch");
            return Ok(Exit {
                code: Some(0),
                stdout: format!("* {} {new} {target}\n", "0".repeat(40)),
                stderr: String::new(),
            });
        }
        let output = Command::new("git").args(&args).current_dir(tree).output()?;
        Ok(Exit {
            code: output.status.code(),
            stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        })
    }
}

/// The merge target's tip newer than anything the tree's repository holds: the bot route
/// reports it, and the tree then holds its commit, so the allow-list and the delegation record
/// at it read; no ref of the tree has moved and no `FETCH_HEAD` is written.
#[test]
fn the_bot_route_brings_a_target_tip_newer_than_the_tree_into_no_ref() {
    let tree = scratch("bot-newer");
    let remote = scratch("bot-newer-remote");
    fs::remove_dir_all(&*remote).expect("emptied");
    git(
        &tree,
        &[
            "clone",
            "-q",
            "--bare",
            ".",
            remote.to_str().expect("utf-8"),
        ],
    );
    let newer = git(
        &remote,
        &["commit-tree", "HEAD^{tree}", "-p", "HEAD", "-m", "newer"],
    );
    git(&remote, &["update-ref", "refs/heads/main", &newer]);
    let held = |at: &str| {
        Command::new("git")
            .args(["cat-file", "-e", &format!("{at}^{{commit}}")])
            .current_dir(&*tree)
            .status()
            .expect("git runs")
            .success()
    };
    assert!(!held(&newer), "the tree holds nothing of the newer tip yet");
    let refs = git(&tree, &["for-each-ref"]);
    let runner = Rc::new(LocalRemote {
        remote: remote.to_path_buf(),
        url: "https://github.com/owner/repo.git".to_owned(),
        lines: RefCell::new(Vec::new()),
    });
    struct Shared(Rc<LocalRemote>);
    impl Runner for Shared {
        fn run_env(
            &self,
            tree: &Path,
            argv: &[String],
            env: &[(&str, &str)],
        ) -> anyhow::Result<Exit> {
            self.0.run_env(tree, argv, env)
        }
        fn run(&self, tree: &Path, argv: &[String]) -> anyhow::Result<Exit> {
            self.0.run(tree, argv)
        }
    }
    let forge = BotRoute::new("owner/repo", None, Box::new(Shared(runner.clone())));
    let answered = forge.target_tip(&tree, "main").expect("a tip");
    assert_eq!(answered, newer);
    assert!(
        held(&newer),
        "the tree holds the tip it was told: {:?}",
        runner.lines.borrow()
    );
    assert_eq!(git(&tree, &["for-each-ref"]), refs, "no ref moved");
    assert!(!tree.join(".git/FETCH_HEAD").exists(), "no FETCH_HEAD");
}

/// A runner that answers the dry-run fetch with `main` at `tip`, and the object fetch with
/// `fetch_code`.
struct Scripted {
    tip: String,
    fetch_code: i32,
    lines: RefCell<Vec<String>>,
}

impl Runner for Scripted {
    fn run_env(&self, tree: &Path, argv: &[String], _: &[(&str, &str)]) -> anyhow::Result<Exit> {
        self.run(tree, argv)
    }

    fn run(&self, _: &Path, argv: &[String]) -> anyhow::Result<Exit> {
        self.lines.borrow_mut().push(argv.join(" "));
        if argv.iter().any(|arg| arg == "--dry-run") {
            return Ok(Exit {
                code: Some(0),
                stdout: format!(
                    "  {} {} refs/remotes/origin/main\n",
                    "0".repeat(40),
                    self.tip
                ),
                stderr: String::new(),
            });
        }
        Ok(Exit {
            code: Some(self.fetch_code),
            stdout: String::new(),
            stderr: "fatal: remote error: upload-pack: not our ref".to_owned(),
        })
    }
}

/// A bot route over `runner`, shared so the test reads its lines.
fn scripted(runner: &Rc<Scripted>) -> BotRoute {
    struct Shared(Rc<Scripted>);
    impl Runner for Shared {
        fn run_env(
            &self,
            tree: &Path,
            argv: &[String],
            env: &[(&str, &str)],
        ) -> anyhow::Result<Exit> {
            self.0.run_env(tree, argv, env)
        }
        fn run(&self, tree: &Path, argv: &[String]) -> anyhow::Result<Exit> {
            self.0.run(tree, argv)
        }
    }
    BotRoute::new("owner/repo", None, Box::new(Shared(runner.clone())))
}

/// The bot route's answer for the target's tip is a full lowercase object id, never a name
/// or an expression git would resolve; anything else is refused before any object is fetched.
#[test]
fn the_bot_route_refuses_a_target_tip_that_is_no_object_id() {
    let tree = scratch("bot-no-id");
    for tip in [
        "abc1234".to_owned(),
        "main".to_owned(),
        "refs/heads/main".to_owned(),
        "HEAD".to_owned(),
        "g".repeat(40),
        "a".repeat(39),
        "a".repeat(41),
        "a".repeat(63),
        "A".repeat(40),
        format!("{}^{{tree}}", "a".repeat(40)),
        format!("-{}", "a".repeat(39)),
        format!("{}~1", "a".repeat(38)),
    ] {
        let runner = Rc::new(Scripted {
            tip: tip.clone(),
            fetch_code: 0,
            lines: RefCell::new(Vec::new()),
        });
        let error = scripted(&runner)
            .target_tip(&tree, "main")
            .expect_err(&format!("{tip} is refused"));
        assert!(
            format!("{error:#}").contains("which is no object id"),
            "{tip}: {error:#}"
        );
        assert_eq!(runner.lines.borrow().len(), 1, "{tip}: nothing fetched");
    }
}

/// An object fetch that fails is the error: the target's tip is not held, so nothing is read
/// at it and nothing is merged.
#[test]
fn the_bot_route_reports_no_target_tip_when_the_object_fetch_fails() {
    let tree = scratch("bot-fetch-fails");
    let runner = Rc::new(Scripted {
        tip: "a".repeat(40),
        fetch_code: 128,
        lines: RefCell::new(Vec::new()),
    });
    let error = scripted(&runner)
        .target_tip(&tree, "main")
        .expect_err("a failed fetch is the error");
    let shown = format!("{error:#}");
    assert!(shown.contains("--no-write-fetch-head"), "{shown}");
    assert!(shown.contains("not our ref"), "{shown}");
    assert_eq!(runner.lines.borrow().len(), 2);
}

// story:gitlab-forge: the adapter over `glab` for a repository on a GitLab host. A recording
// runner stands in for `glab`, the HTTPS push and fetch and `b10x-gates scan-text`; nothing
// leaves the machine.

/// What a program the double stands in for answers: `code`, `stdout`, `stderr`.
fn exit(code: i32, stdout: &str, stderr: &str) -> Exit {
    Exit {
        code: Some(code),
        stdout: stdout.to_owned(),
        stderr: stderr.to_owned(),
    }
}

/// A line the double ran and the environment it was given.
type Ran = (String, Vec<(String, String)>);

/// How the double answers a line.
type Answer = Box<dyn Fn(&str) -> anyhow::Result<Exit>>;

/// Records each line with the environment it was given, and answers it with `answer`.
struct GlabLines {
    lines: RefCell<Vec<Ran>>,
    answer: Answer,
}

impl Runner for GlabLines {
    fn run(&self, tree: &Path, argv: &[String]) -> anyhow::Result<Exit> {
        self.run_env(tree, argv, &[])
    }

    fn run_env(&self, _: &Path, argv: &[String], env: &[(&str, &str)]) -> anyhow::Result<Exit> {
        let line = argv.join(" ");
        self.lines.borrow_mut().push((
            line.clone(),
            env.iter()
                .map(|(name, value)| ((*name).to_owned(), (*value).to_owned()))
                .collect(),
        ));
        (self.answer)(&line)
    }
}

/// The adapter over `group/sub/repo` on `gitlab.example` with the policy `policy.yaml`, every
/// line answered by `answer`; the lines it ran are read from the second.
fn glab_over(answer: impl Fn(&str) -> anyhow::Result<Exit> + 'static) -> (Glab, Rc<GlabLines>) {
    glab_under(
        Scan::Gates {
            policy: Some(PathBuf::from("policy.yaml")),
        },
        answer,
    )
}

/// [`glab_over`] under the scan adapter `scan`.
fn glab_under(
    scan: Scan,
    answer: impl Fn(&str) -> anyhow::Result<Exit> + 'static,
) -> (Glab, Rc<GlabLines>) {
    struct Shared(Rc<GlabLines>);
    impl Runner for Shared {
        fn run(&self, tree: &Path, argv: &[String]) -> anyhow::Result<Exit> {
            self.0.run(tree, argv)
        }
        fn run_env(
            &self,
            tree: &Path,
            argv: &[String],
            env: &[(&str, &str)],
        ) -> anyhow::Result<Exit> {
            self.0.run_env(tree, argv, env)
        }
    }
    let lines = Rc::new(GlabLines {
        lines: RefCell::new(Vec::new()),
        answer: Box::new(answer),
    });
    let glab = Glab::new(
        "gitlab.example",
        "group/sub/repo",
        scan,
        Box::new(Shared(lines.clone())),
    );
    (glab, lines)
}

/// The adapter runs exactly these argument lines, in this order: the push over HTTPS under
/// `glab`'s credential helper alone, the base read through `glab api` for the scan, the scan of
/// the push and of the merge request through `b10x-gates scan-text` under the project, the merge
/// request opened, read twice (state, then head pipeline) and merged through `glab api` on the
/// percent-encoded project, and the target's tip read and fetched into no ref. The push and the
/// fetch run over HTTPS alone; every `glab` line names the host; no token is in any of them.
#[test]
fn the_gitlab_adapter_runs_these_argument_lines() {
    let tree = scratch("glab-argv");
    let dir = tree.join("out");
    let main = git(&tree, &["rev-parse", "main"]);
    git(&tree, &["update-ref", "refs/remotes/origin/main", &main]);
    git(&tree, &["checkout", "-q", "-b", "takt/c1"]);
    fs::write(tree.join("work.txt"), "the work\n").expect("a file");
    git(&tree, &["add", "work.txt"]);
    git(&tree, &["commit", "-q", "-m", "the work"]);
    let c1 = git(&tree, &["rev-parse", "HEAD"]);
    let merge_request = format!(
        r#"{{"iid":7,"state":"opened","source_branch":"takt/c1","target_branch":"trunk","sha":"{c1}","head_pipeline":{{"sha":"{c1}","status":"success"}}}}"#
    );
    let branch = format!(r#"{{"name":"main","commit":{{"id":"{main}"}}}}"#);
    let (forge, lines) = glab_over(move |line| {
        Ok(if line.contains("--method POST") {
            exit(0, r#"{"iid": 7, "id": 1234}"#, "")
        } else if line.contains("--method PUT") {
            exit(0, r#"{"iid": 7, "state": "merged"}"#, "")
        } else if line.ends_with("/repository/branches/main") {
            exit(0, &branch, "")
        } else if line.ends_with("/merge_requests/7") {
            exit(0, &merge_request, "")
        } else {
            exit(0, "", "")
        })
    });
    assert_eq!(forge.name(), "gitlab");
    forge.push(&tree, "takt/c1").expect("pushes");
    let request = PullRequest {
        branch: "takt/c1".to_owned(),
        base: "main".to_owned(),
        title: "deliver the story".to_owned(),
        body: "the body".to_owned(),
    };
    forge
        .scan_outgoing(
            &tree,
            &dir,
            &Outgoing {
                branch: "takt/c1",
                base: "main",
                head: &c1,
                request: Some(&request),
            },
        )
        .expect("scans");
    let number = forge
        .open_pull_request(&tree, &dir, &request)
        .expect("opens");
    assert_eq!(number, 7);
    assert_eq!(
        forge.pull_request(&tree, number).expect("reads"),
        PullRequestView {
            state: PullRequestState::Open,
            head: Head {
                branch: "takt/c1".to_owned(),
                sha: c1.clone(),
            },
            base: "trunk".to_owned(),
        }
    );
    assert_eq!(forge.checks(&tree, number).expect("reads"), Checks::Pass);
    forge.merge(&tree, &dir, number, &c1).expect("merges");
    assert_eq!(forge.target_tip(&tree, "main").expect("a tip"), main);

    let request_file = dir.join(Glab::REQUEST);
    let published = dir.join(Glab::PUBLISHED);
    let https = vec![("GIT_ALLOW_PROTOCOL".to_owned(), "https".to_owned())];
    let api = "glab api --hostname gitlab.example";
    let project = "projects/group%2Fsub%2Frepo";
    let json = "--header Content-Type: application/json";
    let expected: Vec<(String, Vec<(String, String)>)> = vec![
        (
            format!(
                "git -c core.hooksPath=/dev/null -c http.followRedirects=false -c credential.helper= -c credential.helper=!glab auth git-credential push https://gitlab.example/group/sub/repo.git {c1}:refs/heads/takt/c1 --no-verify --no-follow-tags --recurse-submodules=no"
            ),
            https.clone(),
        ),
        (
            format!("{api} {project}/repository/branches/main"),
            Vec::new(),
        ),
        (
            format!(
                "b10x-gates scan-text --repository group/sub/repo --policy policy.yaml {} {} {} {}",
                request_file.display(),
                published.display(),
                dir.join("push.messages.txt").display(),
                dir.join("push.files.txt").display()
            ),
            Vec::new(),
        ),
        (
            format!(
                "b10x-gates scan-text --repository group/sub/repo --policy policy.yaml {} {}",
                request_file.display(),
                published.display()
            ),
            Vec::new(),
        ),
        (
            format!(
                "{api} --method POST {json} {project}/merge_requests --input {}",
                request_file.display()
            ),
            Vec::new(),
        ),
        (format!("{api} {project}/merge_requests/7"), Vec::new()),
        (format!("{api} {project}/merge_requests/7"), Vec::new()),
        (
            format!(
                "{api} --method PUT {json} {project}/merge_requests/7/merge --input {}",
                dir.join(Glab::MERGE_BODY).display()
            ),
            Vec::new(),
        ),
        (
            format!("{api} {project}/repository/branches/main"),
            Vec::new(),
        ),
        (
            format!(
                "git -c core.hooksPath=/dev/null -c http.followRedirects=false -c credential.helper= -c credential.helper=!glab auth git-credential fetch https://gitlab.example/group/sub/repo.git {main} --no-write-fetch-head --no-tags --recurse-submodules=no"
            ),
            https,
        ),
    ];
    assert_eq!(lines.lines.borrow().as_slice(), expected.as_slice());

    let sent: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&request_file).expect("the request"))
            .expect("json");
    assert_eq!(
        sent,
        serde_json::json!({
            "source_branch": "takt/c1",
            "target_branch": "main",
            "title": "deliver the story",
            "description": "the body",
        })
    );
    assert_eq!(
        fs::read_to_string(&published).expect("the published text"),
        "deliver the story\n\nthe body"
    );
    assert!(
        fs::read_to_string(dir.join(Glab::RESPONSE))
            .expect("the response is kept")
            .contains("\"iid\": 7")
    );
    let merge: serde_json::Value = serde_json::from_str(
        &fs::read_to_string(dir.join(Glab::MERGE_BODY)).expect("the merge request"),
    )
    .expect("json");
    assert_eq!(merge, serde_json::json!({ "sha": c1 }));
    assert!(
        fs::read_to_string(dir.join(Glab::MERGE_RESPONSE))
            .expect("the merge response is kept")
            .contains("merged")
    );
    // The project holds `main` as the tracking ref says: only the case's commit is scanned.
    let messages = fs::read_to_string(dir.join("push.messages.txt")).expect("the messages");
    assert!(messages.contains(&format!("commit {c1}")), "{messages}");
    assert!(!messages.contains(&format!("commit {main}")), "{messages}");
}

/// story:scan-and-forge-are-adapters, Acceptance 5: the gitlab adapter scans what it publishes
/// through the run's scan adapter. Under `gates` the push and the merge request pass the same
/// two `b10x-gates scan-text` lines under the project and the policy as before, and the commit
/// identity a third; under a declared `none` no `b10x-gates` line runs and no text is written
/// for a scan, the push, the merge request and its merge go through `glab` as under `gates`,
/// and the commit identity is not scanned, naming the declaration.
#[test]
fn the_gitlab_adapter_scans_through_the_runs_scan_adapter() {
    let declared = PathBuf::from("/config/agent.yaml");
    for scan in [
        Scan::Gates {
            policy: Some(PathBuf::from("policy.yaml")),
        },
        Scan::None {
            declared_in: declared.clone(),
        },
    ] {
        let name = scan.name();
        let tree = scratch(&format!("glab-scan-{name}"));
        let dir = tree.join("out");
        let main = git(&tree, &["rev-parse", "main"]);
        git(&tree, &["update-ref", "refs/remotes/origin/main", &main]);
        git(&tree, &["checkout", "-q", "-b", "takt/c1"]);
        fs::write(tree.join("work.txt"), "the work\n").expect("a file");
        git(&tree, &["add", "work.txt"]);
        git(&tree, &["commit", "-q", "-m", "the work"]);
        let c1 = git(&tree, &["rev-parse", "HEAD"]);
        let merge_request = format!(
            r#"{{"iid":7,"state":"opened","source_branch":"takt/c1","target_branch":"main","sha":"{c1}","head_pipeline":{{"sha":"{c1}","status":"success"}}}}"#
        );
        let branch = format!(r#"{{"name":"main","commit":{{"id":"{main}"}}}}"#);
        let (forge, lines) = glab_under(scan.clone(), move |line| {
            Ok(if line.contains("--method POST") {
                exit(0, r#"{"iid": 7, "id": 1234}"#, "")
            } else if line.contains("--method PUT") {
                exit(0, r#"{"iid": 7, "state": "merged"}"#, "")
            } else if line.ends_with("/repository/branches/main") {
                exit(0, &branch, "")
            } else if line.ends_with("/merge_requests/7") {
                exit(0, &merge_request, "")
            } else {
                exit(0, "", "")
            })
        });
        let request = PullRequest {
            branch: "takt/c1".to_owned(),
            base: "main".to_owned(),
            title: "deliver the story".to_owned(),
            body: "the body".to_owned(),
        };
        forge
            .scan_outgoing(
                &tree,
                &dir,
                &Outgoing {
                    branch: "takt/c1",
                    base: "main",
                    head: &c1,
                    request: Some(&request),
                },
            )
            .expect("scans");
        forge.push(&tree, "takt/c1").expect("pushes");
        let number = forge
            .open_pull_request(&tree, &dir, &request)
            .expect("opens");
        assert_eq!(number, 7, "{name}");
        forge.merge(&tree, &dir, number, &c1).expect("merges");
        let identity = takt::git::CommitIdentity::takt();
        let scanned = forge
            .scan_identity(&tree, &dir.join("identity"), &identity)
            .expect("an answer");
        let ran: Vec<String> = lines
            .lines
            .borrow()
            .iter()
            .map(|(line, _)| line.clone())
            .collect();
        let gates: Vec<&String> = ran
            .iter()
            .filter(|line| line.starts_with("b10x-gates"))
            .collect();
        let glab = |what: &str| ran.iter().filter(|line| line.contains(what)).count();
        assert_eq!(glab(" push https://gitlab.example/"), 1, "{name}: {ran:?}");
        assert_eq!(glab("--method POST"), 1, "{name}: {ran:?}");
        assert_eq!(glab("--method PUT"), 1, "{name}: {ran:?}");
        match &scan {
            Scan::Gates { .. } => {
                let scope = "b10x-gates scan-text --repository group/sub/repo --policy policy.yaml";
                assert_eq!(gates.len(), 3, "{name}: {ran:?}");
                assert!(
                    gates.iter().all(|line| line.starts_with(scope)),
                    "{name}: {gates:?}"
                );
                assert!(
                    gates[0].ends_with("push.files.txt") && gates[1].ends_with(Glab::PUBLISHED),
                    "{name}: {gates:?}"
                );
                assert_eq!(scanned, takt::forge::IdentityScan::Passed, "{name}");
                assert!(dir.join("push.files.txt").exists(), "{name}");
            }
            Scan::None { .. } => {
                assert!(gates.is_empty(), "{name}: no gates line: {gates:?}");
                assert!(
                    !ran.iter().any(|line| line.contains("scan-text")),
                    "{name}: {ran:?}"
                );
                assert!(!dir.join("push.files.txt").exists(), "{name}");
                assert!(!dir.join("push.messages.txt").exists(), "{name}");
                let takt::forge::IdentityScan::NotScanned(why) = &scanned else {
                    panic!("{name}: not scanned, not {scanned:?}");
                };
                assert!(
                    why.contains("scan: none") && why.contains(&declared.display().to_string()),
                    "{name}: {why}"
                );
            }
        }
    }
}

/// A merge request with `state` and `head_pipeline` as given, its head at `sha`.
fn merge_request(state: &str, pipeline: &str) -> String {
    format!(
        r#"{{"state":"{state}","source_branch":"takt/c3","target_branch":"main","sha":"{}","head_pipeline":{pipeline}}}"#,
        "c".repeat(40)
    )
}

/// The states `glab api` reports map onto the three (`locked`, a merge in progress, is open),
/// and the head pipeline onto the three check results: none yet, one of an older push, or one
/// that names no commit, is pending. An unknown state or status, or a reply without a field the view needs, is an error
/// naming it, never a record.
#[test]
fn the_gitlab_adapter_maps_states_and_head_pipelines() {
    let tree = scratch("glab-map");
    let read = |document: String| {
        let (forge, _) = glab_over(move |_| Ok(exit(0, &document, "")));
        (forge.pull_request(&tree, 3), forge.checks(&tree, 3))
    };
    let current = |status: &str| format!(r#"{{"sha":"{}","status":"{status}"}}"#, "c".repeat(40));
    for (state, expected) in [
        ("opened", PullRequestState::Open),
        ("locked", PullRequestState::Open),
        ("merged", PullRequestState::Merged),
        ("closed", PullRequestState::Closed),
    ] {
        let (view, _) = read(merge_request(state, "null"));
        let view = view.expect(state);
        assert_eq!(view.state, expected, "{state}");
        assert_eq!(view.head.branch, "takt/c3");
        assert_eq!(view.head.sha, "c".repeat(40));
        assert_eq!(view.base, "main");
    }
    let (view, _) = read(merge_request("draft", "null"));
    let error = format!("{:#}", view.expect_err("an unknown state"));
    assert!(error.contains("\"draft\""), "{error}");
    for (status, expected) in [
        ("success", Checks::Pass),
        ("skipped", Checks::Pass),
        ("failed", Checks::Fail),
        ("canceled", Checks::Fail),
        ("created", Checks::Pending),
        ("waiting_for_resource", Checks::Pending),
        ("preparing", Checks::Pending),
        ("pending", Checks::Pending),
        ("running", Checks::Pending),
        ("manual", Checks::Pending),
        ("scheduled", Checks::Pending),
    ] {
        let (_, checks) = read(merge_request("opened", &current(status)));
        assert_eq!(checks.expect(status), expected, "{status}");
    }
    let (_, checks) = read(merge_request("opened", "null"));
    assert_eq!(checks.expect("no pipeline yet"), Checks::Pending);
    // A pipeline of an older push says nothing about the head.
    let stale = format!(r#"{{"sha":"{}","status":"success"}}"#, "d".repeat(40));
    let (_, checks) = read(merge_request("opened", &stale));
    assert_eq!(checks.expect("a stale pipeline"), Checks::Pending);
    // Nor does one that names no commit, whatever its status.
    for pipeline in [
        r#"{"status":"success"}"#,
        r#"{"sha":null,"status":"success"}"#,
        r#"{"sha":7,"status":"failed"}"#,
    ] {
        let (_, checks) = read(merge_request("opened", pipeline));
        assert_eq!(checks.expect(pipeline), Checks::Pending, "{pipeline}");
    }
    let (_, checks) = read(merge_request("opened", &current("exploded")));
    let error = format!("{:#}", checks.expect_err("an unknown status"));
    assert!(error.contains("\"exploded\""), "{error}");
    let (view, _) = read(r#"{"state":"opened","sha":"abc","target_branch":"main"}"#.to_owned());
    let error = format!("{:#}", view.expect_err("no source branch"));
    assert!(error.contains("source_branch"), "{error}");
    let (view, checks) = read("not json".to_owned());
    assert!(view.is_err());
    assert!(checks.is_err());
}

/// An API reply without an `iid` opens nothing a validator could record; a merge the reply does
/// not say is `merged` is an error with the reply's message; a push a URL rewrite would send
/// over SSH is refused naming the transport.
#[test]
fn the_gitlab_adapter_refuses_what_it_cannot_read() {
    let tree = scratch("glab-refuse");
    let dir = tree.join("out");
    let (forge, _) = glab_over(|line| {
        Ok(if line.contains("--method POST") {
            exit(0, r#"{"id": 1234}"#, "")
        } else if line.contains("--method PUT") {
            exit(0, r#"{"state":"opened","message":"the head moved"}"#, "")
        } else if line.starts_with("git ") {
            exit(128, "", "fatal: transport 'ssh' not allowed\n")
        } else {
            exit(0, "", "")
        })
    });
    let request = PullRequest {
        branch: "takt/c1".to_owned(),
        base: "main".to_owned(),
        title: "t".to_owned(),
        body: "b".to_owned(),
    };
    let error = format!(
        "{:#}",
        forge
            .open_pull_request(&tree, &dir, &request)
            .expect_err("no iid")
    );
    assert!(error.contains("no iid"), "{error}");
    let error = format!(
        "{:#}",
        forge
            .merge(&tree, &dir, 7, &"c".repeat(40))
            .expect_err("not merged")
    );
    assert!(error.contains("the head moved"), "{error}");
    let head = git(&tree, &["rev-parse", "HEAD"]);
    let error = format!(
        "{:#}",
        forge
            .push_revision(&tree, "takt/c1", &head)
            .expect_err("over ssh")
    );
    assert!(
        error.contains("would go over ssh") && error.contains("the forge gitlab"),
        "{error}"
    );
    // A project with no such branch: no tip, and nothing fetched.
    let (forge, lines) = glab_over(|_| Ok(exit(1, "", "glab: 404 Branch Not Found (HTTP 404)")));
    let error = format!(
        "{:#}",
        forge.target_tip(&tree, "main").expect_err("no branch")
    );
    assert!(
        error.contains("group/sub/repo") && error.contains("no branch main"),
        "{error}"
    );
    assert_eq!(lines.lines.borrow().len(), 1);
}

/// The host and the full project path of an `origin` URL in the forms git writes them, a nested
/// group and a port included; a URL with one segment, an empty, `.` or `..` segment, or no host
/// names none. Userinfo never reaches the host or what the adapter pushes to.
#[test]
fn the_project_is_read_from_the_origin_url() {
    for url in [
        "git@gitlab.example:group/sub/repo.git",
        "git@gitlab.example:group/sub/repo",
        "ssh://git@gitlab.example/group/sub/repo.git",
        "ssh://git@gitlab.example:2222/group/sub/repo.git",
        "https://gitlab.example/group/sub/repo.git",
        "https://gitlab.example/group/sub/repo",
        "https://gitlab.example/group/sub/repo/",
        "https://ci:t0k3n-not-real@gitlab.example/group/sub/repo.git",
    ] {
        assert_eq!(
            project_of(url),
            Some(("gitlab.example".to_owned(), "group/sub/repo".to_owned())),
            "{url}"
        );
    }
    assert_eq!(
        project_of("git@gitlab.example:owner/repo.git"),
        Some(("gitlab.example".to_owned(), "owner/repo".to_owned()))
    );
    for url in [
        "https://gitlab.example/repo.git",
        "https://gitlab.example/group//repo.git",
        "https://gitlab.example/group/../repo.git",
        "/var/lib/git/repo",
        "../repo",
        "repo",
        "",
        "file:///srv/git/group/repo.git",
    ] {
        assert_eq!(project_of(url), None, "{url}");
    }
    let (host, project) =
        project_of("https://ci:t0k3n-not-real@gitlab.example/group/sub/repo.git").expect("named");
    let glab = Glab::new(
        host,
        project,
        Scan::Gates { policy: None },
        Box::new(GlabLines {
            lines: RefCell::new(Vec::new()),
            answer: Box::new(|_| Ok(exit(0, "", ""))),
        }),
    );
    assert_eq!(glab.push_url(), "https://gitlab.example/group/sub/repo.git");
    assert_eq!(encoded("group/sub/repo"), "group%2Fsub%2Frepo");
    assert_eq!(encoded("takt/case-1"), "takt%2Fcase-1");
}

/// An HTTPS origin on a port of its own keeps the port: the push goes to it and every `glab`
/// line names the host with it, as `glab auth login --hostname` stored the login. An SSH port
/// is the SSH server's and is dropped.
#[test]
fn an_https_origin_keeps_its_port() {
    let at_port = Some(("gitlab.example:8443".to_owned(), "group/repo".to_owned()));
    for url in [
        "https://gitlab.example:8443/group/repo.git",
        "HTTPS://gitlab.example:8443/group/repo",
        "https://ci:t0k3n-not-real@gitlab.example:8443/group/repo.git",
    ] {
        assert_eq!(project_of(url), at_port, "{url}");
    }
    for url in [
        "ssh://git@gitlab.example:2222/group/repo.git",
        "https://gitlab.example:/group/repo.git",
    ] {
        assert_eq!(
            project_of(url),
            Some(("gitlab.example".to_owned(), "group/repo".to_owned())),
            "{url}"
        );
    }
    let tree = scratch("glab-port");
    git(
        &tree,
        &[
            "remote",
            "add",
            "origin",
            "https://gitlab.example:8443/group/repo.git",
        ],
    );
    let lines = Rc::new(GlabLines {
        lines: RefCell::new(Vec::new()),
        answer: Box::new(|_| Ok(exit(0, "", ""))),
    });
    struct Shared(Rc<GlabLines>);
    impl Runner for Shared {
        fn run(&self, tree: &Path, argv: &[String]) -> anyhow::Result<Exit> {
            self.0.run(tree, argv)
        }
        fn run_env(
            &self,
            tree: &Path,
            argv: &[String],
            env: &[(&str, &str)],
        ) -> anyhow::Result<Exit> {
            self.0.run_env(tree, argv, env)
        }
    }
    let glab = Glab::for_tree_through(&tree, &Scan::from_env(), Box::new(Shared(lines.clone())))
        .expect("logged in");
    assert_eq!(glab.host, "gitlab.example:8443");
    assert_eq!(
        glab.push_url(),
        "https://gitlab.example:8443/group/repo.git"
    );
    assert_eq!(
        lines.lines.borrow()[0].0,
        "glab auth status --hostname gitlab.example:8443"
    );
    let head = git(&tree, &["rev-parse", "HEAD"]);
    glab.push_revision(&tree, "takt/c1", &head).expect("pushes");
    let ran = lines.lines.borrow();
    let pushed: Vec<&str> = ran
        .iter()
        .map(|(line, _)| line.as_str())
        .filter(|line| line.contains(" push "))
        .collect();
    assert_eq!(
        pushed,
        [format!(
            "git -c core.hooksPath=/dev/null -c http.followRedirects=false -c credential.helper= -c credential.helper=!glab auth git-credential push https://gitlab.example:8443/group/repo.git {head}:refs/heads/takt/c1 --no-verify --no-follow-tags --recurse-submodules=no"
        )],
        "{ran:?}"
    );
    drop(ran);
    // github.com on its HTTPS port is still the bot route's host.
    git(
        &tree,
        &[
            "remote",
            "set-url",
            "origin",
            "https://github.com:443/owner/repo.git",
        ],
    );
    let asked = lines.lines.borrow().len();
    let error = format!(
        "{:#}",
        Glab::for_tree_through(&tree, &Scan::from_env(), Box::new(Shared(lines.clone())))
            .expect_err("github.com is the bot route's")
    );
    assert!(error.contains("--forge bot"), "{error}");
    assert_eq!(lines.lines.borrow().len(), asked, "glab was not asked");
}

/// `glab auth status --hostname <host>` decides whether the adapter is made: one that exits 1,
/// or a `glab` that cannot be run, is an error naming `glab`, the host and what it said. An
/// origin on github.com is refused before `glab` runs, naming the bot route, and a credential in
/// the origin URL is not printed.
#[test]
fn the_gitlab_adapter_needs_glab_logged_in_to_the_host() {
    let tree = scratch("glab-login");
    let (forge, lines) = glab_over(|_| {
        Ok(exit(
            1,
            "",
            "gitlab.example\n  x gitlab.example has not been authenticated with glab.\n",
        ))
    });
    let error = format!("{:#}", forge.logged_in(&tree).expect_err("not logged in"));
    for named in [
        "glab",
        "gitlab.example",
        "has not been authenticated with glab",
    ] {
        assert!(error.contains(named), "{named}: {error}");
    }
    assert!(!error.contains('\n'), "one line: {error}");
    assert_eq!(
        lines
            .lines
            .borrow()
            .iter()
            .map(|(line, _)| line.as_str())
            .collect::<Vec<_>>(),
        ["glab auth status --hostname gitlab.example"]
    );
    let (forge, _) = glab_over(|_| {
        Err(
            anyhow::Error::new(std::io::Error::from(std::io::ErrorKind::NotFound))
                .context("run glab"),
        )
    });
    let error = format!("{:#}", forge.logged_in(&tree).expect_err("no glab"));
    assert!(
        error.contains("glab could not be run") && error.contains("gitlab.example"),
        "{error}"
    );
    let (forge, _) = glab_over(|_| Ok(exit(0, "", "")));
    forge.logged_in(&tree).expect("logged in");

    git(
        &tree,
        &[
            "remote",
            "add",
            "origin",
            "https://ci:t0k3n-not-real@github.com/owner/repo.git",
        ],
    );
    let error = format!(
        "{:#}",
        Glab::for_tree(&tree, &Scan::from_env()).expect_err("github.com is the bot route's")
    );
    assert!(
        error.contains("bot route") && error.contains("--forge bot"),
        "{error}"
    );
    assert!(!error.contains("t0k3n"), "{error}");
}

/// `.gitlab-ci.yml` at the tree's root decides what a GitLab pipeline runs: it is on the judge
/// surface as `.github/**` is, a link to it is read through, and a case whose branch changed it
/// leaves `ci.read` nothing to read.
#[test]
fn the_gitlab_ci_definition_is_on_the_judge_surface() {
    let tree = scratch("gitlab-ci");
    let surface = JudgeSurface::new(&[], &tree);
    assert!(surface.contains(GITLAB_CI));
    assert!(surface.contains(".GitLab-CI.yml"));
    assert!(!surface.contains("sub/.gitlab-ci.yml"));
    assert!(surface.without_store().reads_through(GITLAB_CI));
    assert!(JUDGE_SURFACE.contains("`.gitlab-ci.yml` at the root"));

    let memory = Rc::new(Memory {
        checks: Checks::Pass,
        ..Memory::default()
    });
    let outcome = delivered_at(&tree, "gitlab-ci", memory, GITLAB_CI);
    assert!(
        outcome.text.contains("evidence: pull_request open"),
        "{}",
        outcome.text
    );
    assert!(
        !outcome
            .records
            .iter()
            .any(|record| record["kind"] == "ci_result"),
        "{}",
        outcome.text
    );
    assert!(
        outcome.text.contains("changed the judge surface") && outcome.text.contains(GITLAB_CI),
        "{}",
        outcome.text
    );
}

/// What a GitLab pipeline runs is decided by `.gitlab-ci.yml` and by what it includes, kept
/// under `.gitlab/` at the root (`include: local: .gitlab/ci/build.yml`), with the review rules
/// (`.gitlab/CODEOWNERS`): every path under `.gitlab/` is on the judge surface, so a case whose
/// branch changed one leaves `ci.read` nothing to read. A `.gitlab/` deeper down is not, nor is
/// a file an `include: local:` names outside both.
#[test]
fn the_gitlab_directory_is_on_the_judge_surface() {
    let tree = scratch("gitlab-dir");
    let surface = JudgeSurface::new(&[], &tree);
    for path in [
        GITLAB_DIR,
        ".gitlab/",
        ".gitlab/ci/build.yml",
        ".GitLab/CODEOWNERS",
    ] {
        assert!(surface.contains(path), "{path}");
    }
    for path in [
        "sub/.gitlab/ci/build.yml",
        ".gitlab-ci",
        ".gitlabx/build.yml",
        "ci/build.yml",
    ] {
        assert!(!surface.contains(path), "{path}");
    }
    assert!(JUDGE_SURFACE.contains("`.gitlab/**`"));

    let memory = Rc::new(Memory {
        checks: Checks::Pass,
        ..Memory::default()
    });
    let outcome = delivered_at(&tree, "gitlab-dir", memory, ".gitlab/ci/build.yml");
    assert!(
        outcome.text.contains("evidence: pull_request open"),
        "{}",
        outcome.text
    );
    assert!(
        !outcome
            .records
            .iter()
            .any(|record| record["kind"] == "ci_result"),
        "{}",
        outcome.text
    );
    assert!(
        outcome.text.contains("changed the judge surface")
            && outcome.text.contains(".gitlab/ci/build.yml"),
        "{}",
        outcome.text
    );
}

/// A fresh run on the delivery protocol over `tree`, no validator and no harness, two steps at
/// most: how it stopped and what it printed. Its first step is a judgement step, so the step
/// guard reads the surface before it; nothing is set aside but under the scratch directory.
fn first_step(tree: &Path, id: &str) -> (anyhow::Result<Stop>, String) {
    let options = RunOptions {
        protocol: PROTOCOL.to_owned(),
        intent: "deliver the story\n\nThe story's body.".to_owned(),
        inputs: Vec::new(),
        iterations: Some(2),
        hours: None,
        tokens: None,
        case_id: Some(id.to_owned()),
        gate: None,
    };
    let validators: Vec<Box<dyn Validator>> = Vec::new();
    let mut run = Run::open(tree, options, &NoHarness, &validators).expect("the case opens");
    run.set_aside_in(
        Path::new(env!("CARGO_TARGET_TMPDIR"))
            .join(format!("forge-{}-{id}-aside", std::process::id())),
    )
    .expect("a set-aside root");
    let mut out = Vec::new();
    let stop = run.run(&mut out);
    (stop, String::from_utf8_lossy(&out).into_owned())
}

/// The refusal a run stopped on before the harness; fails the case on any other stop.
fn refused_before_the_harness(stop: anyhow::Result<Stop>, text: &str) -> String {
    match stop {
        Ok(Stop::StepRefused(why)) if text.contains("refused before the harness") => why,
        other => panic!("refused before the harness, not {other:?}\n{text}"),
    }
}

/// A link under `.gitlab/` that git tracks is read through (`include: local:` would read what
/// it points at): no judgement step is taken while it is there, and the refusal names it.
#[test]
fn a_tracked_link_under_the_gitlab_directory_is_refused() {
    let tree = scratch("gitlab-link");
    fs::create_dir_all(tree.join("ci-real")).expect("a directory");
    fs::write(
        tree.join("ci-real/build.yml"),
        "build:\n  script: [\"true\"]\n",
    )
    .expect("a file");
    fs::create_dir_all(tree.join(GITLAB_DIR)).expect("a directory");
    std::os::unix::fs::symlink("../ci-real", tree.join(".gitlab/ci")).expect("a link");
    git(&tree, &["add", "-A"]);
    git(
        &tree,
        &["commit", "-q", "-m", "the CI includes through a link"],
    );
    let surface = JudgeSurface::new(&[], &tree).without_store();
    for link in [GITLAB_DIR, ".gitlab/ci", ".GitLab/ci/build.yml"] {
        assert!(surface.reads_through(link), "{link}");
    }
    assert!(!surface.reads_through("sub/.gitlab/ci"));
    let (stop, text) = first_step(&tree, "gitlab-link");
    let why = refused_before_the_harness(stop, &text);
    assert!(
        why.contains("reached through a link") && why.contains(".gitlab/ci -> ../ci-real"),
        "{why}"
    );
}

/// `.gitlab` itself a link git ignores and does not track: only the read of the tree's root
/// finds it, and it is refused as a tracked one is.
#[test]
fn an_ignored_link_at_the_gitlab_directory_is_refused() {
    let tree = scratch("gitlab-ignored-link");
    fs::write(tree.join(".gitignore"), "/.gitlab\n").expect("a file");
    git(&tree, &["add", ".gitignore"]);
    git(&tree, &["commit", "-q", "-m", "ignore .gitlab"]);
    std::os::unix::fs::symlink("../gitlab-elsewhere", tree.join(GITLAB_DIR)).expect("a link");
    assert_eq!(git(&tree, &["status", "--porcelain"]), "");
    assert_eq!(git(&tree, &["ls-files", "--", GITLAB_DIR]), "");
    let (stop, text) = first_step(&tree, "gitlab-ignored-link");
    let why = refused_before_the_harness(stop, &text);
    assert!(
        why.contains("reached through a link") && why.contains(".gitlab -> ../gitlab-elsewhere"),
        "{why}"
    );
}

/// A `glab api` that exits non-zero is an error naming the line, how it exited, the message
/// of the JSON body glab printed and the HTTP status: never a pending pipeline, an open merge
/// request or no branch. Only a 404 on the branch read means the project has no such branch.
#[test]
fn a_failed_glab_api_is_an_error_naming_the_line_and_the_status() {
    let tree = scratch("glab-failed");
    let dir = tree.join("out");
    let api = "glab api --hostname gitlab.example projects/group%2Fsub%2Frepo";
    let (forge, _) = glab_over(|line| {
        Ok(if line.starts_with("b10x-gates ") {
            exit(0, "", "")
        } else {
            exit(
                1,
                r#"{"message":"401 Unauthorized"}"#,
                "glab: 401 Unauthorized (HTTP 401)\n",
            )
        })
    });
    let checks = forge.checks(&tree, 7).map(|_| ());
    let view = forge.pull_request(&tree, 7).map(|_| ());
    for (what, result) in [("checks", checks), ("the merge request", view)] {
        let error = format!("{:#}", result.expect_err(what));
        assert!(
            error.contains(&format!("`{api}/merge_requests/7` failed: exit 1, 401 Unauthorized (glab: 401 Unauthorized (HTTP 401))")),
            "{what}: {error}"
        );
    }
    let error = format!(
        "{:#}",
        forge
            .target_tip(&tree, "main")
            .expect_err("a 401 is no tip")
    );
    assert!(
        error.contains(&format!("`{api}/repository/branches/main` failed"))
            && error.contains("HTTP 401")
            && !error.contains("no branch"),
        "{error}"
    );
    let request = PullRequest {
        branch: "takt/c1".to_owned(),
        base: "main".to_owned(),
        title: "t".to_owned(),
        body: "b".to_owned(),
    };
    let (forge, _) = glab_over(|line| {
        Ok(if line.starts_with("b10x-gates ") {
            exit(0, "", "")
        } else if line.contains("--method POST") {
            exit(
                1,
                r#"{"message":["Another open merge request already exists for this source branch: !6"]}"#,
                "glab: 409 Conflict (HTTP 409)\n",
            )
        } else {
            exit(
                1,
                r#"{"message":"405 Method Not Allowed"}"#,
                "glab: 405 Method Not Allowed (HTTP 405)\n",
            )
        })
    });
    let error = format!(
        "{:#}",
        forge
            .open_pull_request(&tree, &dir, &request)
            .expect_err("a conflict opens nothing")
    );
    assert!(
        error.contains("--method POST")
            && error.contains("Another open merge request already exists")
            && error.contains("HTTP 409"),
        "{error}"
    );
    let error = format!(
        "{:#}",
        forge
            .merge(&tree, &dir, 7, &"c".repeat(40))
            .expect_err("a refused merge")
    );
    assert!(
        error.contains("--method PUT")
            && error.contains("405 Method Not Allowed")
            && error.contains("HTTP 405"),
        "{error}"
    );
    // A 404 on the branch read, the body glab prints with it, is no such branch.
    let (forge, lines) = glab_over(|_| {
        Ok(exit(
            1,
            r#"{"message":"404 Branch Not Found"}"#,
            "glab: 404 Branch Not Found (HTTP 404)\n",
        ))
    });
    let error = format!(
        "{:#}",
        forge.target_tip(&tree, "main").expect_err("no branch")
    );
    assert!(
        error.contains("has no branch main to merge into"),
        "{error}"
    );
    assert_eq!(lines.lines.borrow().len(), 1, "nothing fetched");
}

/// glab reads its login from `GLAB_CONFIG_DIR` when it is set, as gh does from
/// `GH_CONFIG_DIR`: a test command takt starts does not inherit it. Since
/// story:confinement-through-substrate-on-linux-and-macos takt has no command wrapper of its
/// own: the variable is kept out of the environment a `none` run hands Substrate's `local`
/// adapter ([`takt::confinement::inheritable`]).
#[test]
fn glab_s_configuration_directory_is_no_login_a_child_inherits() {
    assert!(LOGIN_LOCATIONS.contains(&"GLAB_CONFIG_DIR"));
    assert!(
        !takt::confinement::inheritable("GLAB_CONFIG_DIR", "/elsewhere/glab-cli"),
        "story:confinement-through-substrate-on-linux-and-macos: a login location is not passed to a test command"
    );
    assert!(
        takt::confinement::inheritable("LANG", "C.UTF-8"),
        "story:confinement-through-substrate-on-linux-and-macos: the locale is"
    );
}

/// A token GitLab issues, by each prefix GitLab's documentation lists, and the token glab keeps
/// under `token:` in its `config.yml`, are credentials a proposal may not carry; the prefixes
/// in prose, a short value and a line of code that names a token are not.
#[test]
fn gitlab_tokens_and_glab_s_token_key_are_credential_shapes() {
    // Built here, never written whole in the source: no scanner reads a token in this file.
    let body: String = "Zq8xR2vL0pW4nB6tY1cK".to_owned();
    for prefix in GITLAB_TOKEN_PREFIXES {
        for text in [
            format!("{prefix}{body}"),
            format!("export GITLAB_TOKEN={prefix}{body}\n"),
            format!("\"{prefix}{body}.01.0w1h2k3n4\""),
        ] {
            assert_eq!(credential_shape(&text), Some("gitlab-token"), "{text}");
        }
    }
    let hex = "0123456789abcdef".repeat(4);
    for text in [
        format!(
            "hosts:\n    gitlab.example:\n        token: {hex}\n        api_host: gitlab.example\n"
        ),
        format!("token: \"{hex}\"\n"),
        format!("  token: '{hex}'"),
    ] {
        assert_eq!(credential_shape(&text), Some("token-field"), "{text}");
    }
    for clean in [
        "The glpat- prefix names a personal access token; gldt- a deploy token.\n",
        "glrt-runner and glagent-config are names, not tokens\n",
        "token: read_token_from_file(path),\n",
        "let token = glab_token();\ntoken: Some(token),\n",
        "token: !!null\n",
        "token: short-1\n",
        "tokens: 12345678901234567890abc\n",
    ] {
        assert_eq!(credential_shape(clean), None, "{clean}");
    }
    // A prefix inside a longer word starts no token.
    let inside = format!("x{}{body}", GITLAB_TOKEN_PREFIXES[0]);
    assert_eq!(credential_shape(&inside), None, "{inside}");
}
