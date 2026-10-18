//! The merge under authority through the loop: two runs on `protocols/story-delivery/1.yaml`
//! with the forge double of the forge story, its checks passing. The first run, whose case holds
//! no grant, stops at the gate with the pull request open and nothing merged; the operator's
//! grant is written into the case's `authority.yaml` and recorded on the story in the planning
//! store; the resumed run reads the grant into the evaluation, merges through the forge as a
//! deterministic step, writes one `merge_observation` record from what the forge reports, and
//! stops Completed (accepted). Every record names its validator; the model never produces
//! evidence; a connection to the forge is never authority.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::rc::Rc;

use anyhow::anyhow;
use takt::case::{AUTHORITY_FILE, Approval, Authority, Branch, CaseDir, Grant, grant};
use takt::evidence::Record;
use takt::forge::{
    BotRoute, Checks, Exit, Forge, Head, Memory, PullRequest, PullRequestState, Runner,
};
use takt::frontier::{Aep, Frontier, Story, Take, Unit, approve};
use takt::harness::{Harness, StepContext, StepReport};
use takt::validator::{CiRead, PrMerged, PrOpen, Validator};
use takt::{Decision, LoadedProtocol, Run, RunOptions, Stop};

/// The protocol under test, relative to this crate.
const PROTOCOL: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../protocols/story-delivery/1.yaml"
);

/// The fixture stores, one directory each, relative to this crate.
const FIXTURES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures");

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

/// A scratch git tree under this crate's build directory. Dropping it removes the tree; a
/// removal that fails fails the case, unless the case is already failing.
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

/// A scratch tree with one commit on `main`.
fn scratch(name: &str) -> Scratch {
    let dir =
        Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("merge-{}-{name}", std::process::id()));
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

/// A scratch repository holding a committed copy of the fixture store `fixture`.
fn repository(fixture: &str, name: &str) -> Scratch {
    let dir =
        Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("merge-{}-{name}", std::process::id()));
    if dir.exists() {
        fs::remove_dir_all(&dir).expect("a stale scratch tree is removed");
    }
    copy_dir(&Path::new(FIXTURES).join(fixture), &dir);
    fs::write(dir.join("check.txt"), "broken\n").expect("a file");
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

/// A harness that requests the merge gate, changing nothing, as soon as it is offered: what a
/// judgement actor does once the intent holds and nothing but the approval is left.
struct Requester;

impl Harness for Requester {
    fn name(&self) -> &str {
        "requester"
    }

    fn step(&self, context: &StepContext<'_>) -> Result<StepReport, anyhow::Error> {
        if context.approvals.contains(&"repository.merge".to_owned()) {
            return Ok(StepReport {
                action: "repository.merge".to_owned(),
                summary: "the intent holds; merge it".to_owned(),
                tokens: None,
            });
        }
        Err(anyhow!(
            "the gate was not offered; a judgement step among {} is left",
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
        "merge-test",
    )
}

fn options(id: &str) -> RunOptions {
    RunOptions {
        protocol: PROTOCOL.to_owned(),
        intent: "deliver the story\n\nThe story's body.".to_owned(),
        inputs: Vec::new(),
        iterations: Some(8),
        hours: None,
        tokens: None,
        case_id: Some(id.to_owned()),
        gate: Some("repository.merge".to_owned()),
    }
}

/// The three forge validators over `forge`, in the order the binary registers them.
fn validators(forge: Rc<dyn Forge>) -> Vec<Box<dyn Validator>> {
    vec![
        Box::new(PrOpen::new(forge.clone())),
        Box::new(CiRead::new(forge.clone())),
        Box::new(PrMerged::new(forge)),
    ]
}

/// The records under the case's `evidence/`, in file order, each with its file name.
fn records(case_dir: &Path) -> Vec<(String, serde_json::Value)> {
    let mut paths: Vec<PathBuf> = fs::read_dir(case_dir.join("evidence"))
        .expect("an evidence directory")
        .map(|entry| entry.expect("an entry").path())
        .collect();
    paths.sort();
    paths
        .iter()
        .map(|path| {
            (
                path.file_name()
                    .expect("a name")
                    .to_string_lossy()
                    .into_owned(),
                serde_json::from_str(&fs::read_to_string(path).expect("a record")).expect("json"),
            )
        })
        .collect()
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

fn outcome<'a>(decision: &'a Decision, id: &str) -> &'a str {
    decision
        .outcomes
        .get(id)
        .map(String::as_str)
        .unwrap_or("absent")
}

/// The first run of a case on the delivery protocol over `tree`: the tests and the gate already
/// green at HEAD, the forge double's checks passing, the `Requester` harness. Stops at the gate.
fn first_run(tree: &Path, id: &str, memory: &Rc<Memory>) -> (String, PathBuf, String) {
    let forge: Rc<dyn Forge> = memory.clone();
    let validators = validators(forge);
    let mut run = Run::open(tree, options(id), &Requester, &validators).expect("the case opens");
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
    assert_eq!(
        stop,
        Stop::ApprovalRequired(vec!["repository.merge".to_owned()]),
        "{text}"
    );
    (revision, run.case.dir.clone(), text)
}

/// The story's acceptance. The first run stops at `ApprovalRequired (repository.merge)` with the
/// pull request open, the double recording no merge and no `merge_observation` written; the
/// grant is written as the case's `authority.yaml`; the resumed run merges through the double as
/// a deterministic step, writes exactly one `merge_observation` record with result `merged`
/// about `implementation` at the revision, named after its validator and numbered after the
/// records already there, and stops Completed (accepted). The decision document of the resumed
/// case shows the grant it was computed from.
#[test]
fn without_a_grant_the_run_stops_at_the_gate_and_the_resumed_run_merges_under_it() {
    let tree = scratch("grant");
    let head = git(&tree, &["rev-parse", "HEAD"]);
    let memory = Rc::new(Memory {
        checks: Checks::Pass,
        ..Memory::default()
    });
    let (revision, case_dir, text) = first_run(&tree, "grant", &memory);
    assert_eq!(revision, head);
    assert_eq!(
        memory.calls.borrow().as_slice(),
        [
            "push takt/grant",
            "open takt/grant #1",
            "view #1",
            "checks #1"
        ],
        "{text}"
    );
    assert_eq!(
        memory
            .pull_request(&tree, 1)
            .expect("the double reports")
            .state,
        PullRequestState::Open
    );
    // What the double has been asked so far, this probe included.
    let asked = memory.calls.borrow().len();
    let before = records(&case_dir);
    assert_eq!(before.len(), 4, "{text}");
    assert!(
        before
            .iter()
            .all(|(_, record)| record["kind"] != "merge_observation"),
        "{text}"
    );
    assert!(!case_dir.join(AUTHORITY_FILE).exists());
    {
        let case = CaseDir::resume(&tree, "grant").expect("the case reads back");
        let decision = case.evaluate(50).expect("evaluates");
        assert_eq!(
            action(&decision, "repository.merge"),
            "approval-required",
            "{}",
            decision.json
        );
        assert_eq!(
            claim(&decision, "pr.merged"),
            "unknown",
            "{}",
            decision.json
        );
        assert_eq!(
            outcome(&decision, "accepted"),
            "blocked",
            "{}",
            decision.json
        );
    }

    // The operator grants `repository.merge` to the case.
    let approval =
        grant(&tree, "grant", "repository.merge", "operator").expect("the grant is written");
    assert_eq!(
        approval,
        Approval {
            case: "grant".to_owned(),
            story: None,
            capability: "repository.merge".to_owned(),
            approver: "operator".to_owned(),
            granted_at: approval.granted_at.clone(),
        }
    );
    assert_eq!(approval.reference(), "grant/authority.yaml");
    let authority = Authority::read(&case_dir)
        .expect("reads")
        .expect("the file is there");
    assert_eq!(authority.decisions.len(), 1, "{authority:?}");
    let decision = &authority.decisions[0];
    assert_eq!(decision.capability, "repository.merge");
    assert_eq!(decision.decision, Grant::Granted);
    assert_eq!(decision.approver, "operator");
    assert_eq!(decision.granted_at, approval.granted_at);
    // What Canon is handed: a `canon-authority/1` list of capability and decision, nothing else.
    let canon: serde_yaml_ng::Value =
        serde_yaml_ng::from_str(&authority.canon().expect("renders")).expect("yaml");
    let entries = canon.as_sequence().expect("a list");
    assert_eq!(entries.len(), 1);
    let keys: Vec<&str> = entries[0]
        .as_mapping()
        .expect("a map")
        .keys()
        .map(|key| key.as_str().expect("a key"))
        .collect();
    assert_eq!(keys, ["capability", "decision"]);
    assert_eq!(entries[0]["decision"], "granted");

    // The resumed run.
    let forge: Rc<dyn Forge> = memory.clone();
    let validators = validators(forge);
    let mut run = Run::resume(&tree, options("grant"), "grant", &NoHarness, &validators)
        .expect("the case resumes");
    assert_eq!(run.case.id, "grant");
    assert_eq!(run.case.dir, case_dir);
    assert_eq!(
        run.unit,
        Unit::Intent("deliver the story\n\nThe story's body.".to_owned())
    );
    let mut out = Vec::new();
    let stop = run.run(&mut out).expect("the run stops without an error");
    let text = String::from_utf8(out).expect("utf-8");
    assert_eq!(stop, Stop::Completed("accepted".to_owned()), "{text}");
    assert_eq!(run.steps, 1, "{text}");
    assert!(text.contains("step 1: repository.merge {}"), "{text}");
    assert!(
        text.contains("evidence: merge_observation merged"),
        "{text}"
    );
    assert_eq!(
        &memory.calls.borrow()[asked..],
        ["view #1", "merge #1", "view #1"],
        "{text}"
    );
    assert_eq!(
        memory
            .pull_request(&tree, 1)
            .expect("the double reports")
            .state,
        PullRequestState::Merged
    );
    let after = records(&case_dir);
    assert_eq!(after.len(), 5, "{text}");
    let merges: Vec<&(String, serde_json::Value)> = after
        .iter()
        .filter(|(_, record)| record["kind"] == "merge_observation")
        .collect();
    assert_eq!(merges.len(), 1, "{text}");
    let (name, merge) = merges[0];
    assert!(
        name.starts_with("0005-"),
        "numbered after the four records already there: {name}"
    );
    assert_eq!(merge["format"], "canon-evidence/1", "{merge}");
    assert_eq!(merge["result"], "merged", "{merge}");
    assert_eq!(merge["subject"], "implementation", "{merge}");
    assert_eq!(merge["subject_revision"], head.as_str(), "{merge}");
    assert_eq!(merge["producer"], "takt.pr-merged", "{merge}");
    let decision = run.case.evaluate(51).expect("evaluates");
    assert_eq!(claim(&decision, "pr.merged"), "true", "{}", decision.json);
    assert_eq!(
        action(&decision, "repository.merge"),
        "admissible",
        "{}",
        decision.json
    );
    assert_eq!(
        outcome(&decision, "accepted"),
        "legitimate",
        "{}",
        decision.json
    );
    let document: serde_json::Value = serde_json::from_str(&decision.json).expect("json");
    assert_eq!(
        document["explanation"]["computed_from"]["authority"],
        serde_json::json!([{"capability": "repository.merge", "decision": "granted"}]),
        "{}",
        decision.json
    );
}

/// The grant is read into every evaluation of the case, not only a run's: a case whose four
/// records hold has `repository.merge` approval-required, and admissible once `authority.yaml`
/// grants it; the file is read at each evaluation, so a grant written between two evaluations
/// of one `CaseDir` is seen by the second.
#[test]
fn the_grant_is_read_into_every_evaluation_of_the_case() {
    let tree = scratch("every");
    let protocol = LoadedProtocol::from_file(Path::new(PROTOCOL)).expect("the protocol loads");
    let mut case =
        CaseDir::open(&tree, protocol, "every", "deliver the story", &[]).expect("the case opens");
    let revision = case
        .revision("implementation")
        .expect("implementation has a revision");
    for (n, (kind, result)) in [
        ("test_result", "pass"),
        ("gate_result", "pass"),
        ("pull_request", "open"),
        ("ci_result", "pass"),
    ]
    .iter()
    .enumerate()
    {
        case.add_evidence(&record(n + 1, kind, result, &revision))
            .expect("the record is written");
    }
    let decision = case.evaluate(1).expect("evaluates");
    assert_eq!(
        action(&decision, "repository.merge"),
        "approval-required",
        "{}",
        decision.json
    );
    grant(&tree, "every", "repository.merge", "operator").expect("the grant is written");
    let decision = case.evaluate(2).expect("evaluates");
    assert_eq!(
        action(&decision, "repository.merge"),
        "admissible",
        "{}",
        decision.json
    );
    assert_eq!(
        outcome(&decision, "accepted"),
        "blocked",
        "the grant admits the merge; it earns nothing: {}",
        decision.json
    );
}

/// A capability the case already decides is refused, and the file is left as it was: Canon
/// refuses a capability decided twice, so the second grant would break every later evaluation.
/// A case that does not exist is refused with nothing written.
#[test]
fn grant_refuses_a_capability_already_decided_and_a_case_that_does_not_exist() {
    let tree = scratch("twice");
    let protocol = LoadedProtocol::from_file(Path::new(PROTOCOL)).expect("the protocol loads");
    let case =
        CaseDir::open(&tree, protocol, "twice", "deliver the story", &[]).expect("the case opens");
    grant(&tree, "twice", "repository.merge", "operator").expect("the first grant is written");
    let error = format!(
        "{:#}",
        grant(&tree, "twice", "repository.merge", "someone else").expect_err("decided twice")
    );
    assert!(error.contains("repository.merge"), "{error}");
    assert!(error.contains("operator"), "{error}");
    let authority = Authority::read(&case.dir)
        .expect("reads")
        .expect("the file is there");
    assert_eq!(authority.decisions.len(), 1, "{authority:?}");
    assert_eq!(authority.decisions[0].approver, "operator");

    let error = format!(
        "{:#}",
        grant(&tree, "absent", "repository.merge", "operator").expect_err("no such case")
    );
    assert!(error.contains("absent"), "{error}");
    assert!(!tree.join(takt::case::CASES_DIR).join("absent").exists());
    for (capability, approver) in [
        ("", "operator"),
        ("repository merge", "operator"),
        ("x", ""),
    ] {
        assert!(
            grant(&tree, "twice", capability, approver).is_err(),
            "capability {capability:?} by {approver:?} is not a grant"
        );
    }
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

/// `repository.merge` merges nothing unless the forge reports the pull request's head as the
/// case's branch at the revision: a head on another commit or another branch earns no record
/// and no merge call, and a pushed revision other than the one evaluated asks the forge
/// nothing. A number the case directory carries is not trusted on its own.
#[test]
fn pr_merged_merges_nothing_when_the_head_is_not_the_case_branch_at_the_revision() {
    let tree = scratch("elsewhere");
    let memory = Rc::new(Memory {
        checks: Checks::Pass,
        ..Memory::default()
    });
    let (revision, case_dir, _) = first_run(&tree, "elsewhere", &memory);
    let wrong_sha = Rc::new(Memory {
        head: Some(Head {
            branch: "takt/elsewhere".to_owned(),
            sha: "f".repeat(40),
        }),
        opened: opened_for("takt/elsewhere"),
        ..Memory::default()
    });
    let wrong_branch = Rc::new(Memory {
        head: Some(Head {
            branch: "takt/other".to_owned(),
            sha: revision.clone(),
        }),
        opened: opened_for("takt/other"),
        ..Memory::default()
    });
    for (what, forge) in [("sha", wrong_sha), ("branch", wrong_branch)] {
        let observation = PrMerged::new(forge.clone())
            .observe(&tree, &revision, 7)
            .unwrap_or_else(|error| panic!("{what}: repository.merge observes: {error:#}"));
        assert!(
            observation.records.is_empty(),
            "{what}: {}",
            observation.summary
        );
        assert!(
            observation.summary.contains("nothing to merge"),
            "{what}: {}",
            observation.summary
        );
        assert_eq!(
            forge.calls.borrow().as_slice(),
            ["view #1"],
            "{what}: the merge was asked for"
        );
    }
    // A revision other than the pushed one: nothing is read, nothing merged.
    let other = "0".repeat(40);
    let observation = PrMerged::new(memory.clone())
        .observe(&tree, &other, 8)
        .expect("observes");
    assert!(observation.records.is_empty(), "{}", observation.summary);
    assert!(
        observation.summary.contains("nothing to merge"),
        "{}",
        observation.summary
    );
    assert_eq!(
        memory.calls.borrow().len(),
        4,
        "{:?}",
        memory.calls.borrow()
    );
    assert_eq!(
        Branch::read(&case_dir)
            .expect("the branch file")
            .pull_request,
        Some(1)
    );
}

/// A pull request the forge already reports merged is recorded merged without a second merge
/// call; one it reports closed is recorded closed and never merged.
#[test]
fn pr_merged_records_what_the_forge_reports_and_merges_only_an_open_pull_request() {
    let tree = scratch("already");
    let memory = Rc::new(Memory {
        checks: Checks::Pass,
        ..Memory::default()
    });
    let (revision, _, _) = first_run(&tree, "already", &memory);
    for (state, result) in [
        (PullRequestState::Merged, "merged"),
        (PullRequestState::Closed, "closed"),
    ] {
        let forge = Rc::new(Memory {
            state,
            opened: opened_for("takt/already"),
            pushed: RefCell::new(BTreeMap::from([(
                "takt/already".to_owned(),
                revision.clone(),
            )])),
            ..Memory::default()
        });
        let observation = PrMerged::new(forge.clone())
            .observe(&tree, &revision, 9)
            .expect("observes");
        assert_eq!(observation.records.len(), 1, "{}", observation.summary);
        assert_eq!(observation.records[0].result.as_deref(), Some(result));
        assert_eq!(observation.records[0].kind, "merge_observation");
        assert_eq!(observation.records[0].producer, "takt.pr-merged");
        assert_eq!(observation.records[0].subject_revision, revision);
        assert_eq!(
            forge.calls.borrow().as_slice(),
            ["view #1"],
            "{result}: the merge was asked for"
        );
    }
}

/// The double refuses a merge whose head is not the revision last pushed for the branch, as
/// the API does when given the head to match; and a pull request it never opened.
#[test]
fn the_double_merges_only_the_head_it_was_pushed() {
    let tree = scratch("double");
    let memory = Memory::default();
    memory.push(&tree, "takt/double").expect("pushes");
    let request = PullRequest {
        branch: "takt/double".to_owned(),
        base: "main".to_owned(),
        title: String::new(),
        body: String::new(),
    };
    let number = memory
        .open_pull_request(&tree, &tree.join("out"), &request)
        .expect("opens");
    let head = git(&tree, &["rev-parse", "HEAD"]);
    assert!(
        memory
            .merge(&tree, &tree.join("out"), number, &"f".repeat(40))
            .is_err()
    );
    assert!(memory.merge(&tree, &tree.join("out"), 2, &head).is_err());
    assert_eq!(
        memory.pull_request(&tree, number).expect("reports").state,
        PullRequestState::Open
    );
    memory
        .merge(&tree, &tree.join("out"), number, &head)
        .expect("merges");
    assert_eq!(
        memory.pull_request(&tree, number).expect("reports").state,
        PullRequestState::Merged
    );
}

/// A runner that records every line and answers the merge API call with the response given.
struct Recording {
    lines: RefCell<Vec<String>>,
    response: &'static str,
}

impl Runner for Recording {
    fn run(&self, _: &Path, argv: &[String]) -> anyhow::Result<Exit> {
        self.lines.borrow_mut().push(argv.join(" "));
        if argv.get(1).map(String::as_str) == Some("api") {
            let output = argv
                .iter()
                .position(|a| a == "--output")
                .map(|i| &argv[i + 1])
                .expect("--output");
            fs::write(output, self.response)?;
        }
        Ok(Exit {
            code: Some(0),
            stdout: String::new(),
            stderr: String::new(),
        })
    }
}

/// The bot route merges through `b10x-gates api`, a PUT on the pull request's merge resource
/// with the head to match in a JSON file and the response in another, and runs nothing else;
/// no token appears on the line. A response that does not say `merged` is an error.
#[test]
fn the_bot_route_adapter_merges_through_the_api_with_the_head_to_match() {
    let tree = scratch("argv");
    let dir = tree.join("out");
    let recording = Rc::new(Recording {
        lines: RefCell::new(Vec::new()),
        response: r#"{"sha":"abc123","merged":true,"message":"Pull Request successfully merged"}"#,
    });
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
    forge.merge(&tree, &dir, 7, "abc123").expect("merges");
    let input = dir.join(BotRoute::MERGE_REQUEST);
    let output = dir.join(BotRoute::MERGE_RESPONSE);
    assert_eq!(
        recording.lines.borrow().as_slice(),
        [format!(
            "b10x-gates api --method PUT --path /repos/owner/repo/pulls/7/merge --input {} --output {} --repository owner/repo --policy policy.yaml",
            input.display(),
            output.display()
        )]
    );
    let sent: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&input).expect("the request")).expect("json");
    assert_eq!(sent, serde_json::json!({"sha": "abc123"}), "{sent}");

    let refusing = BotRoute::new(
        "owner/repo",
        None,
        Box::new(Recording {
            lines: RefCell::new(Vec::new()),
            response: r#"{"message":"Head branch was modified. Review and try the merge again.","merged":false}"#,
        }),
    );
    let error = format!(
        "{:#}",
        refusing
            .merge(&tree, &tree.join("out2"), 7, "abc123")
            .expect_err("not merged")
    );
    assert!(error.contains("Head branch was modified"), "{error}");
}

/// The status the store lists `id` at.
fn status(repo: &Path, id: &str) -> String {
    Aep::default()
        .list(repo)
        .expect("the store lists")
        .into_iter()
        .find(|a| a.id == id)
        .map(|a| a.status)
        .unwrap_or_else(|| panic!("{id} is listed"))
}

/// The names under `<repo>/.engineering/cases`, sorted.
fn case_names(repo: &Path) -> Vec<String> {
    let mut names: Vec<String> = fs::read_dir(repo.join(takt::case::CASES_DIR))
        .expect("a readable cases directory")
        .map(|e| {
            e.expect("an entry")
                .file_name()
                .to_string_lossy()
                .into_owned()
        })
        .collect();
    names.sort();
    names
}

fn store_options() -> RunOptions {
    RunOptions {
        protocol: "software.change/1".to_owned(),
        intent: String::new(),
        inputs: vec![
            ("risk".to_owned(), "trivial".to_owned()),
            ("change_kind".to_owned(), "feature".to_owned()),
            ("affects_security_boundary".to_owned(), "false".to_owned()),
        ],
        iterations: Some(1),
        hours: None,
        tokens: None,
        case_id: None,
        gate: None,
    }
}

/// `aep plan artifact history <id> --format json` in `repo`: the evidence entries.
fn evidence_history(repo: &Path, id: &str) -> Vec<serde_json::Value> {
    let output = Command::new("aep")
        .args(["plan", "artifact", "history", id, "--format", "json"])
        .current_dir(repo)
        .output()
        .expect("aep runs");
    assert!(
        output.status.success(),
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let entries: Vec<serde_json::Value> =
        serde_json::from_slice(&output.stdout).expect("the history is json");
    entries
        .into_iter()
        .filter(|entry| entry["change"]["change"] == "evidence")
        .collect()
}

/// A run with no intent on a store whose story is `active` with an open case resumes that case
/// and opens no second one: the same case directory, the unit the story, the store not moved;
/// `takt run --case <id>` resumes the same. The grant given through the store adapter is
/// written into the case and recorded on the story as one `approval` evidence record whose
/// source names takt and the approver and whose reference is the case's `authority.yaml`.
#[test]
fn the_open_case_of_an_active_story_is_resumed_and_its_approval_recorded_on_the_story() {
    let repo = repository("one-ready", "store");
    let frontier = Aep::default();
    let validators: Vec<Box<dyn Validator>> = vec![];
    let mut out = Vec::new();
    let first = Run::open_next(
        &repo,
        store_options(),
        &frontier,
        &NoHarness,
        &validators,
        &mut out,
    )
    .expect("the frontier answers")
    .expect("story:ready-one is taken");
    let case_id = first.case.id.clone();
    let Unit::Story(story) = first.unit.clone() else {
        panic!("the unit is a story, not {:?}", first.unit);
    };
    assert_eq!(status(&repo, "story:ready-one"), "active");
    assert_eq!(
        first.case.story().expect("reads"),
        Some("story:ready-one".to_owned()),
        "the case directory records the story it was opened for"
    );
    drop(first);
    assert_eq!(
        String::from_utf8_lossy(&out),
        "frontier: took story:ready-one: Make the check pass\n"
    );

    // The next run on the repository resumes the case, never opens a second.
    let mut out = Vec::new();
    let second = Run::open_next(
        &repo,
        store_options(),
        &frontier,
        &NoHarness,
        &validators,
        &mut out,
    )
    .expect("the frontier answers")
    .expect("the open case is resumed");
    assert_eq!(second.case.id, case_id);
    assert_eq!(second.unit, Unit::Story(story.clone()));
    assert_eq!(
        String::from_utf8_lossy(&out),
        format!("frontier: resumed story:ready-one: Make the check pass (case {case_id})\n")
    );
    assert_eq!(case_names(&repo), std::slice::from_ref(&case_id));
    assert_eq!(status(&repo, "story:ready-one"), "active");
    drop(second);

    // `takt run --case <id>` resumes it too, with the story as its unit.
    let third = Run::resume(&repo, store_options(), &case_id, &NoHarness, &validators)
        .expect("the case resumes");
    assert_eq!(third.case.id, case_id);
    assert_eq!(third.unit, Unit::Story(story));
    assert_eq!(case_names(&repo), std::slice::from_ref(&case_id));
    drop(third);
    let error = format!(
        "{:#}",
        Run::resume(&repo, store_options(), "absent", &NoHarness, &validators)
            .err()
            .expect("no such case")
    );
    assert!(error.contains("absent"), "{error}");

    // The approval: the grant in the case, the record on the story.
    assert!(evidence_history(&repo, "story:ready-one").is_empty());
    let approval = approve(&repo, &frontier, &case_id, "repository.merge", "operator")
        .expect("the approval is given");
    assert_eq!(approval.story.as_deref(), Some("story:ready-one"));
    assert_eq!(approval.case, case_id);
    let authority = Authority::read(&repo.join(takt::case::CASES_DIR).join(&case_id))
        .expect("reads")
        .expect("the file is there");
    assert_eq!(authority.decisions[0].capability, "repository.merge");
    assert_eq!(authority.decisions[0].approver, "operator");
    let recorded = evidence_history(&repo, "story:ready-one");
    assert_eq!(recorded.len(), 1, "{recorded:?}");
    let change = &recorded[0]["change"];
    assert_eq!(change["kind"], "approval", "{change}");
    let source = change["source"].as_str().expect("a source");
    assert!(source.contains("takt"), "{change}");
    assert!(source.contains("operator"), "{change}");
    assert!(source.contains("repository.merge"), "{change}");
    assert_eq!(
        change["reference"],
        format!("{case_id}/{AUTHORITY_FILE}").as_str(),
        "{change}"
    );
    // The resumed case reads the grant: `software.change/1` requires it for its merge.
    let case = CaseDir::resume(&repo, &case_id).expect("the case reads back");
    let decision = case.evaluate(60).expect("evaluates");
    let document: serde_json::Value = serde_json::from_str(&decision.json).expect("json");
    assert_eq!(
        document["explanation"]["computed_from"]["authority"],
        serde_json::json!([{"capability": "repository.merge", "decision": "granted"}]),
        "{}",
        decision.json
    );
}

/// A frontier that is never asked to record: a case opened on an intent has no story to record
/// an approval on, and `takt approve` writes `authority.yaml` only.
struct Unasked;

impl Frontier for Unasked {
    fn name(&self) -> &str {
        "unasked"
    }

    fn candidates(&self, _: &Path) -> anyhow::Result<Vec<String>> {
        Ok(Vec::new())
    }

    fn active(&self, _: &Path) -> anyhow::Result<Vec<String>> {
        Ok(Vec::new())
    }

    fn show(&self, _: &Path, id: &str) -> anyhow::Result<Story> {
        panic!("{id} was shown")
    }

    fn take(&self, _: &Path, id: &str) -> anyhow::Result<Take> {
        panic!("{id} was taken")
    }

    fn record_approval(&self, _: &Path, story: &str, _: &Approval) -> anyhow::Result<()> {
        panic!("an approval was recorded on {story}")
    }
}

#[test]
fn an_approval_on_a_case_opened_on_an_intent_writes_the_grant_only() {
    let tree = scratch("intent");
    let protocol = LoadedProtocol::from_file(Path::new(PROTOCOL)).expect("the protocol loads");
    let case =
        CaseDir::open(&tree, protocol, "intent", "deliver the story", &[]).expect("the case opens");
    assert_eq!(case.story().expect("reads"), None);
    let approval = approve(&tree, &Unasked, "intent", "repository.merge", "operator")
        .expect("the grant is written");
    assert_eq!(approval.story, None);
    assert!(case.dir.join(AUTHORITY_FILE).is_file());
}

/// A store that lists nothing active offers nothing to resume: the ready story is taken as
/// before, and a case directory left by a case opened on an intent is not resumed for it.
#[test]
fn a_case_opened_on_an_intent_is_not_resumed_by_the_frontier() {
    let repo = repository("one-ready", "intent-case");
    let validators: Vec<Box<dyn Validator>> = vec![];
    let mut options = store_options();
    options.case_id = Some("on-intent".to_owned());
    let run = Run::open_unit(
        &repo,
        options,
        Unit::Intent("make the check pass".to_owned()),
        &NoHarness,
        &validators,
    )
    .expect("the case opens");
    drop(run);
    let mut out = Vec::new();
    let run = Run::open_next(
        &repo,
        store_options(),
        &Aep::default(),
        &NoHarness,
        &validators,
        &mut out,
    )
    .expect("the frontier answers")
    .expect("story:ready-one is taken");
    assert_ne!(run.case.id, "on-intent");
    assert!(
        String::from_utf8_lossy(&out).starts_with("frontier: took story:ready-one"),
        "{}",
        String::from_utf8_lossy(&out)
    );
    assert_eq!(case_names(&repo).len(), 2);
}
