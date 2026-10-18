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
    BotRoute, Checks, Exit, Forge, Head, Memory, PullRequest, PullRequestState, PullRequestView,
    Runner, repository_of, without_userinfo,
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
        r#"{"mergedAt":null,"state":"OPEN","headRefName":"takt/c1","headRefOid":"abc123"}"#,
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
        PullRequestView {
            state: PullRequestState::Open,
            head: Head {
                branch: "takt/c1".to_owned(),
                sha: "abc123".to_owned(),
            },
        }
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
            "gh pr view 7 --json state,mergedAt,headRefName,headRefOid".to_owned(),
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
        r#"{"mergedAt":"2026-01-01T00:00:00Z","state":"MERGED","headRefName":"takt/c3","headRefOid":"def"}"#,
        r#"[{"bucket":"pass"},{"bucket":"pending"}]"#,
    );
    let view = state.expect("reads");
    assert_eq!(view.state, PullRequestState::Merged);
    assert_eq!(view.head.branch, "takt/c3");
    assert_eq!(view.head.sha, "def");
    assert_eq!(checks.expect("reads"), Checks::Pending);
    let (state, checks) = read(
        r#"{"mergedAt":null,"state":"CLOSED","headRefName":"takt/c3","headRefOid":"def"}"#,
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
        })
    }

    fn checks(&self, _: &Path, number: u64) -> anyhow::Result<Checks> {
        self.calls.borrow_mut().push(format!("checks #{number}"));
        Ok(Checks::Pass)
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
    let protocol =
        takt::LoadedProtocol::from_file(Path::new(PROTOCOL)).expect("the protocol loads");
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

/// A case opened while the tree is on another case's branch (the state every finished run
/// leaves the tree in) is discarded back to the branch it left, as `CaseDir::discard`
/// documents, not to the base it lent from the other case; the other case keeps its directory.
#[test]
fn adv_a_case_discarded_over_another_cases_branch_returns_the_tree_to_that_branch() {
    let tree = scratch("adv-over");
    let protocol =
        takt::LoadedProtocol::from_file(Path::new(PROTOCOL)).expect("the protocol loads");
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
    assert_eq!(outcome.stop, Stop::Iterations(2), "{}", outcome.text);
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
    let first = run
        .case
        .revision("implementation")
        .expect("implementation has a revision");
    run.case
        .add_evidence(&record(1, "test_result", "pass", &first))
        .expect("the record is written");
    run.case
        .add_evidence(&record(2, "gate_result", "pass", &first))
        .expect("the record is written");
    let mut out = Vec::new();
    let stop = run.run(&mut out).expect("the run stops without an error");
    let text = String::from_utf8(out).expect("utf-8");
    assert_eq!(stop, Stop::Iterations(2), "{text}");
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
    let protocol =
        takt::LoadedProtocol::from_file(Path::new(PROTOCOL)).expect("the protocol loads");
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
    assert_eq!(outcome.records.len(), 4, "{}", outcome.text);
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
/// because that field reaches `b10x-gates bot -- push origin <name>` as given: a refspec there
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
    let stop = run.run(&mut out).expect("the run stops without an error");
    let text = String::from_utf8(out).expect("utf-8");
    let calls = memory.calls.borrow().clone();
    let pushes: Vec<&String> = calls.iter().filter(|c| c.starts_with("push ")).collect();
    assert!(
        !pushes.is_empty(),
        "nothing was pushed; stopped {stop}\n{text}"
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
    let protocol =
        takt::LoadedProtocol::from_file(Path::new(PROTOCOL)).expect("the protocol loads");
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
    let protocol =
        takt::LoadedProtocol::from_file(Path::new(PROTOCOL)).expect("the protocol loads");
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
    let protocol =
        takt::LoadedProtocol::from_file(Path::new(PROTOCOL)).expect("the protocol loads");
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
    let protocol =
        takt::LoadedProtocol::from_file(Path::new(PROTOCOL)).expect("the protocol loads");
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
    let protocol =
        takt::LoadedProtocol::from_file(Path::new(PROTOCOL)).expect("the protocol loads");
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
    let protocol =
        takt::LoadedProtocol::from_file(Path::new(PROTOCOL)).expect("the protocol loads");
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
    assert_eq!(outcome.records.len(), 4, "{}", outcome.text);
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
