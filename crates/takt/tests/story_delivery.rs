//! The story-delivery protocol as data: `protocols/story-delivery/1.yaml` is loaded, validated
//! and compiled by Canon, a case is opened on it over a scratch git tree, and the six records
//! the delivery of a story produces are added one at a time, each evaluated: one more claim true
//! after each, `pr.open` admissible only once the change is handed over, `repository.merge` approval-required before the last, `accepted` legitimate only
//! after it. Every record is about `implementation` at its revision, as the kernel binds claims.

use std::fs;
use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};
use std::process::Command;

use takt::evidence::Record;
use takt::{CaseDir, Decision, LoadedProtocol};

/// The protocol under test, relative to this crate.
const PROTOCOL: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../protocols/story-delivery/1.yaml"
);

/// The records a delivery produces, in order, each with the claim it establishes. The merge is
/// its own kind: Canon leaves a match unknown when records of one kind disagree, so a `merged`
/// beside an `open` record of `pull_request` at one revision would decide neither claim.
/// The hand-over has no result: takt records the harness's choice, not an outcome.
const LADDER: [(&str, &str, &str); 6] = [
    ("test_result", "pass", "tests.pass"),
    ("gate_result", "pass", "gate.green"),
    ("handover", "", "change.handed_over"),
    ("pull_request", "open", "pr.opened"),
    ("ci_result", "pass", "ci.green"),
    ("merge_observation", "merged", "pr.merged"),
];

fn git(tree: &Path, args: &[&str]) {
    let status = Command::new("git")
        .args(["-c", "user.name=t", "-c", "user.email=t@localhost"])
        .args(args)
        .current_dir(tree)
        .status()
        .expect("git runs");
    assert!(status.success(), "git {args:?}");
}

/// A scratch git tree with one commit, under this crate's build directory. Dropping it removes
/// the tree; a removal that fails fails the case, unless the case is already failing.
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
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("story-delivery-{}-{name}", std::process::id()));
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

fn open(tree: &Path, id: &str) -> CaseDir {
    let protocol = LoadedProtocol::from_file(Path::new(PROTOCOL))
        .expect("the protocol loads, validates and compiles");
    CaseDir::open(tree, protocol, id, "deliver the story", &[]).expect("the case opens")
}

/// A record of `kind` about `implementation` at `revision`; an empty `result` writes none.
fn record(n: usize, kind: &str, result: &str, revision: &str) -> Record {
    Record::new(
        format!("e{n}"),
        kind,
        (!result.is_empty()).then(|| result.to_owned()),
        "implementation",
        revision,
        "story-delivery-test",
    )
}

/// The claims the decision holds, in id order.
fn held(decision: &Decision) -> Vec<&str> {
    decision
        .claims
        .iter()
        .filter(|(_, value)| value.as_str() == "true")
        .map(|(id, _)| id.as_str())
        .collect()
}

fn status<'a>(section: &'a std::collections::BTreeMap<String, String>, id: &str) -> &'a str {
    section.get(id).map(String::as_str).unwrap_or("absent")
}

#[test]
fn each_record_earns_one_claim_and_accepted_needs_the_merge() {
    let tree = scratch("ladder");
    let mut case = open(&tree, "ladder");
    assert_eq!(case.protocol.id, "story.delivery/1");
    let revision = case
        .revision("implementation")
        .expect("implementation has a revision");

    let decision = case.evaluate(0).expect("evaluates");
    assert!(held(&decision).is_empty(), "{}", decision.json);
    assert_eq!(status(&decision.actions, "pr.open"), "blocked");
    assert_eq!(status(&decision.actions, "repository.merge"), "blocked");
    assert_eq!(status(&decision.outcomes, "accepted"), "blocked");

    for (n, (kind, result, claim)) in LADDER.iter().enumerate() {
        let seq = n + 1;
        case.add_evidence(&record(seq, kind, result, &revision))
            .expect("the record is written");
        let decision = case.evaluate(seq).expect("evaluates");
        let mut expected: Vec<&str> = LADDER[..seq].iter().map(|(_, _, c)| *c).collect();
        expected.sort_unstable();
        assert_eq!(
            held(&decision),
            expected,
            "after {kind} {result}: {claim} should be one more claim true\n{}",
            decision.json
        );
        let pr_open = status(&decision.actions, "pr.open");
        let merge = status(&decision.actions, "repository.merge");
        let accepted = status(&decision.outcomes, "accepted");
        match seq {
            1 | 2 => {
                assert_eq!(pr_open, "blocked", "{}", decision.json);
                assert_eq!(merge, "blocked", "{}", decision.json);
                assert_eq!(accepted, "blocked", "{}", decision.json);
            }
            3 | 4 => {
                assert_eq!(pr_open, "admissible", "{}", decision.json);
                assert_eq!(merge, "blocked", "{}", decision.json);
                assert_eq!(accepted, "blocked", "{}", decision.json);
            }
            5 => {
                assert_eq!(merge, "approval-required", "{}", decision.json);
                assert_eq!(accepted, "blocked", "{}", decision.json);
            }
            _ => {
                assert_eq!(merge, "approval-required", "{}", decision.json);
                assert_eq!(accepted, "legitimate", "{}", decision.json);
            }
        }
    }
}

#[test]
fn a_new_implementation_revision_returns_every_claim_to_unknown() {
    let tree = scratch("revision");
    let mut case = open(&tree, "revision");
    let revision = case
        .revision("implementation")
        .expect("implementation has a revision");
    for (n, (kind, result, _)) in LADDER.iter().enumerate() {
        case.add_evidence(&record(n + 1, kind, result, &revision))
            .expect("the record is written");
    }
    let decision = case.evaluate(1).expect("evaluates");
    assert_eq!(held(&decision).len(), LADDER.len(), "{}", decision.json);
    assert_eq!(status(&decision.outcomes, "accepted"), "legitimate");

    case.set_revision("implementation", "0000000000000000000000000000000000000000")
        .expect("the revision moves");
    let decision = case.evaluate(2).expect("evaluates");
    assert!(held(&decision).is_empty(), "{}", decision.json);
    assert!(
        decision.claims.values().all(|value| value == "unknown"),
        "{:?}",
        decision.claims
    );
    assert_eq!(status(&decision.actions, "repository.merge"), "blocked");
    assert_eq!(status(&decision.outcomes, "accepted"), "blocked");
}

/// Teardown is part of the case: a scratch tree that cannot be removed fails it, loudly, instead
/// of surviving under the build directory with the error discarded.
#[test]
fn a_scratch_tree_that_cannot_be_removed_fails_the_case_loudly() {
    let tree = scratch("unremovable");
    let path = tree.to_path_buf();
    let locked = path.join("locked");
    fs::create_dir(&locked).expect("a subdirectory");
    fs::write(locked.join("file"), "x\n").expect("a file in it");
    fs::set_permissions(&locked, fs::Permissions::from_mode(0o500)).expect("no write bit");
    if fs::write(locked.join("probe"), "").is_ok() {
        // A privileged user writes regardless of the bit; the case can show nothing here.
        fs::set_permissions(&locked, fs::Permissions::from_mode(0o700)).expect("write bit back");
        fs::remove_dir_all(&path).expect("the tree is removed");
        return;
    }
    let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || drop(tree)));
    fs::set_permissions(&locked, fs::Permissions::from_mode(0o700)).expect("write bit back");
    fs::remove_dir_all(&path).expect("the tree is removed once writable");
    assert!(
        outcome.is_err(),
        "the scratch tree could not be removed and the case did not fail"
    );
}

/// Adversary cases, added after the unit declared itself green; each is named `adv_*` so the
/// suite runs without them under `--skip adv_`. The protocol they load may be overridden with
/// `STORY_DELIVERY_PROTOCOL`, so a mutated copy of the document outside the tree shows which
/// mutation each case turns red on.
mod adv {
    use std::collections::BTreeMap;

    use takt::harness::{Harness, StepContext, StepReport};
    use takt::{Run, RunOptions, Validator};

    use super::*;

    fn protocol_path() -> PathBuf {
        std::env::var_os("STORY_DELIVERY_PROTOCOL")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(PROTOCOL))
    }

    fn adv_open(tree: &Path, id: &str) -> CaseDir {
        let protocol = LoadedProtocol::from_file(&protocol_path())
            .expect("the protocol loads, validates and compiles");
        CaseDir::open(tree, protocol, id, "deliver the story", &[]).expect("the case opens")
    }

    /// A harness that is never stepped: `Run::open` only opens the case.
    struct Idle;

    impl Harness for Idle {
        fn name(&self) -> &str {
            "idle"
        }

        fn step(&self, _: &StepContext<'_>) -> anyhow::Result<StepReport> {
            anyhow::bail!("never stepped")
        }
    }

    /// The decided delivery sequence, as the store has it and as story:merge-under-authority's
    /// `pr.merged` validator writes it: the fifth record is `merge_observation` merged, beside
    /// the `pull_request` open record at the same revision, and both claims hold.
    #[test]
    fn adv_the_decided_sequence_earns_pr_opened_and_pr_merged() {
        let tree = scratch("adv-pr-merged");
        let mut case = adv_open(&tree, "adv-pr-merged");
        let revision = case
            .revision("implementation")
            .expect("implementation has a revision");
        let sequence = [
            ("test_result", "pass"),
            ("gate_result", "pass"),
            ("pull_request", "open"),
            ("ci_result", "pass"),
            ("merge_observation", "merged"),
        ];
        for (n, (kind, result)) in sequence.iter().enumerate() {
            case.add_evidence(&record(n + 1, kind, result, &revision))
                .expect("the record is written");
        }
        let decision = case.evaluate(1).expect("evaluates");
        let got = [
            ("claim pr.opened", status(&decision.claims, "pr.opened")),
            ("claim pr.merged", status(&decision.claims, "pr.merged")),
            ("outcome accepted", status(&decision.outcomes, "accepted")),
        ];
        assert_eq!(
            got,
            [
                ("claim pr.opened", "true"),
                ("claim pr.merged", "true"),
                ("outcome accepted", "legitimate"),
            ],
            "{}",
            decision.json
        );
    }

    /// README.md names `--protocol protocols/story-delivery/1.yaml` as the way to run on this
    /// protocol; `takt run` adds no case input for a protocol file, so the case opens with the
    /// inputs that command line gives: none.
    #[test]
    fn adv_the_readme_invocation_opens_a_case_without_inputs() {
        let tree = scratch("adv-readme");
        let options = RunOptions {
            protocol: protocol_path().to_string_lossy().into_owned(),
            intent: "deliver the story".to_owned(),
            inputs: Vec::new(),
            iterations: Some(1),
            hours: None,
            tokens: None,
            case_id: Some("adv-readme".to_owned()),
            gate: Some("repository.merge".to_owned()),
        };
        let validators: Vec<Box<dyn Validator>> = Vec::new();
        match Run::open(&tree, options, &Idle, &validators) {
            Ok(_) => {}
            Err(error) => panic!("the README's invocation does not open a case: {error:#}"),
        }
    }

    /// The ladder always has `tests.pass` before `gate.green`, so it cannot tell a `pr.open`
    /// precondition of both from one of `gate.green` alone.
    #[test]
    fn adv_pr_open_needs_tests_pass_not_only_gate_green() {
        let tree = scratch("adv-pr-open");
        let mut case = adv_open(&tree, "adv-pr-open");
        let revision = case
            .revision("implementation")
            .expect("implementation has a revision");
        case.add_evidence(&record(1, "gate_result", "pass", &revision))
            .expect("the record is written");
        let decision = case.evaluate(1).expect("evaluates");
        assert_eq!(
            status(&decision.claims, "gate.green"),
            "true",
            "{}",
            decision.json
        );
        assert_eq!(
            status(&decision.claims, "tests.pass"),
            "unknown",
            "{}",
            decision.json
        );
        assert_eq!(
            status(&decision.actions, "pr.open"),
            "blocked",
            "{}",
            decision.json
        );
    }

    /// The ladder always has `pr.opened` before `ci.green`, so it cannot tell a
    /// `repository.merge` precondition of both from one of `ci.green` alone.
    #[test]
    fn adv_repository_merge_needs_pr_opened_not_only_ci_green() {
        let tree = scratch("adv-merge");
        let mut case = adv_open(&tree, "adv-merge");
        let revision = case
            .revision("implementation")
            .expect("implementation has a revision");
        for (n, (kind, result)) in [
            ("test_result", "pass"),
            ("gate_result", "pass"),
            ("handover", ""),
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
            status(&decision.claims, "ci.green"),
            "true",
            "{}",
            decision.json
        );
        assert_eq!(
            status(&decision.claims, "pr.opened"),
            "unknown",
            "{}",
            decision.json
        );
        assert_eq!(
            status(&decision.actions, "pr.open"),
            "admissible",
            "{}",
            decision.json
        );
        assert_eq!(
            status(&decision.actions, "repository.merge"),
            "blocked",
            "{}",
            decision.json
        );
    }

    /// The ladder never adds a record with the other result, so it cannot tell a claim that
    /// matches `result: pass` from one that matches any record of the kind. Canon decides a
    /// match `false` when records of the kind exist and none carries the result.
    #[test]
    fn adv_a_failing_result_decides_each_claim_false() {
        let tree = scratch("adv-fail");
        let mut case = adv_open(&tree, "adv-fail");
        let revision = case
            .revision("implementation")
            .expect("implementation has a revision");
        for (n, (kind, result)) in [
            ("test_result", "fail"),
            ("gate_result", "fail"),
            ("pull_request", "closed"),
            ("ci_result", "fail"),
            ("merge_observation", "closed"),
        ]
        .iter()
        .enumerate()
        {
            case.add_evidence(&record(n + 1, kind, result, &revision))
                .expect("the record is written");
        }
        let decision = case.evaluate(1).expect("evaluates");
        let mut expected: BTreeMap<String, String> = [
            "ci.green",
            "gate.green",
            "pr.merged",
            "pr.opened",
            "tests.pass",
        ]
        .into_iter()
        .map(|claim| (claim.to_owned(), "false".to_owned()))
        .collect();
        // A hand-over has no result to fail with: with no `handover` record the claim is unknown.
        expected.insert("change.handed_over".to_owned(), "unknown".to_owned());
        assert_eq!(decision.claims, expected, "{}", decision.json);
        assert_eq!(
            status(&decision.actions, "pr.open"),
            "blocked",
            "{}",
            decision.json
        );
        assert_eq!(
            status(&decision.actions, "repository.merge"),
            "blocked",
            "{}",
            decision.json
        );
        assert_eq!(
            status(&decision.outcomes, "accepted"),
            "blocked",
            "{}",
            decision.json
        );
    }

    /// Every ladder record is about `implementation`, so the suite cannot tell a claim bound to
    /// `subject: implementation` from one reading records about any artifact.
    #[test]
    fn adv_a_record_about_another_artifact_does_not_count() {
        let tree = scratch("adv-subject");
        let mut case = adv_open(&tree, "adv-subject");
        let plan = case.revision("plan").expect("plan has a revision");
        case.add_evidence(&Record::new(
            "e1",
            "test_result",
            Some("pass".to_owned()),
            "plan",
            plan,
            "story-delivery-test",
        ))
        .expect("the record is written");
        let decision = case.evaluate(1).expect("evaluates");
        assert_eq!(
            status(&decision.claims, "tests.pass"),
            "unknown",
            "{}",
            decision.json
        );
        assert_eq!(
            status(&decision.actions, "pr.open"),
            "blocked",
            "{}",
            decision.json
        );
    }
}

/// Second adversary pass, after correction 1: the guard's two exit paths, and two mutants the
/// nine cases above miss (`tests.run` renamed; `abandoned` earned by a claim). Same
/// `STORY_DELIVERY_PROTOCOL` override as `adv`.
mod adv2 {
    use std::sync::Mutex;

    use takt::harness::{Harness, StepContext, StepReport};
    use takt::validator::{Observation, Validator};
    use takt::{Run, RunOptions, Stop};

    use super::*;

    fn protocol_path() -> PathBuf {
        std::env::var_os("STORY_DELIVERY_PROTOCOL")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(PROTOCOL))
    }

    fn adv2_open(tree: &Path, id: &str) -> CaseDir {
        let protocol = LoadedProtocol::from_file(&protocol_path())
            .expect("the protocol loads, validates and compiles");
        CaseDir::open(tree, protocol, id, "deliver the story", &[]).expect("the case opens")
    }

    /// A harness that is never stepped; reaching it is the failure.
    struct Idle;

    impl Harness for Idle {
        fn name(&self) -> &str {
            "idle"
        }

        fn step(&self, context: &StepContext<'_>) -> anyhow::Result<StepReport> {
            anyhow::bail!(
                "a judgement step was offered {:?} before any validator ran",
                context.offered
            )
        }
    }

    /// `tests.run` as the CLI wires it, without a test command: one `test_result` pass.
    struct TestsPass;

    impl Validator for TestsPass {
        fn name(&self) -> &str {
            "adv2.tests-pass"
        }

        fn action(&self) -> &str {
            "tests.run"
        }

        fn kind(&self) -> &str {
            "test_result"
        }

        fn subject(&self) -> &str {
            "implementation"
        }

        fn observe(&self, _: &Path, revision: &str, seq: usize) -> anyhow::Result<Observation> {
            Ok(Observation {
                summary: "pass".to_owned(),
                artifact: None,
                records: vec![Record::new(
                    format!("test-result-{seq}"),
                    self.kind(),
                    Some("pass".to_owned()),
                    self.subject(),
                    revision,
                    self.name(),
                )],
            })
        }
    }

    /// The guard on the success path: a case that opened a case in its tree and returned
    /// leaves nothing under the build directory.
    #[test]
    fn adv2_a_passing_case_leaves_no_scratch_tree() {
        let tree = scratch("adv2-pass");
        let path = tree.to_path_buf();
        let case = adv2_open(&tree, "adv2-pass");
        assert!(case.dir.starts_with(&path), "{}", case.dir.display());
        drop(case);
        drop(tree);
        assert!(
            !path.exists(),
            "the scratch tree survived a passing case: {}",
            path.display()
        );
    }

    /// The guard on the panic path: a case that fails after opening a case in its tree leaves
    /// nothing under the build directory either.
    #[test]
    fn adv2_a_failing_case_leaves_no_scratch_tree() {
        let seen: Mutex<Option<PathBuf>> = Mutex::new(None);
        let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let tree = scratch("adv2-fail");
            *seen.lock().expect("the cell") = Some(tree.to_path_buf());
            let _case = adv2_open(&tree, "adv2-fail");
            panic!("the case fails on purpose");
        }));
        assert!(outcome.is_err(), "the case did not fail");
        let path = seen
            .into_inner()
            .expect("the cell")
            .expect("the tree was created");
        assert!(
            !path.exists(),
            "the scratch tree survived a failing case: {}",
            path.display()
        );
    }

    /// The README's invocation with `--harness none --iterations 1`, through the loop: the
    /// first step is the `tests.run` validator, its record earns `tests.pass`, and the step
    /// budget stops the run. A protocol that renamed `tests.run` would offer the harness a
    /// judgement step instead, and none of the nine cases above would notice.
    #[test]
    fn adv2_the_loop_runs_the_tests_validator_first_on_this_protocol() {
        let tree = scratch("adv2-loop");
        let options = RunOptions {
            protocol: protocol_path().to_string_lossy().into_owned(),
            intent: "deliver the story".to_owned(),
            inputs: Vec::new(),
            iterations: Some(1),
            hours: None,
            tokens: None,
            case_id: Some("adv2-loop".to_owned()),
            gate: Some("repository.merge".to_owned()),
        };
        let validators: Vec<Box<dyn Validator>> = vec![Box::new(TestsPass)];
        let mut run = Run::open(&tree, options, &Idle, &validators).expect("the case opens");
        let mut out = Vec::new();
        let stop = run.run(&mut out).map_err(|error| format!("{error:#}"));
        let text = String::from_utf8(out).expect("utf-8");
        assert_eq!(stop, Ok(Stop::Iterations(1)), "{text}");
        assert_eq!(run.steps, 1, "{text}");
        assert!(text.contains("step 1: tests.run {}"), "{text}");
        assert!(text.contains("evidence: test_result pass"), "{text}");
        let decision = run.case.evaluate(99).expect("evaluates");
        assert_eq!(
            status(&decision.claims, "tests.pass"),
            "true",
            "{}",
            decision.json
        );
        assert_eq!(
            decision.legitimate(),
            Vec::<String>::new(),
            "{}",
            decision.json
        );
    }

    /// `abandoned` is earned by an explicit decision only: declared, blocked before any record,
    /// still blocked after all five, and `accepted` is then the only legitimate outcome. The
    /// loop completes through the first legitimate outcome, whichever one it is.
    #[test]
    fn adv2_abandoned_needs_an_explicit_decision_not_a_claim() {
        let tree = scratch("adv2-abandoned");
        let mut case = adv2_open(&tree, "adv2-abandoned");
        let revision = case
            .revision("implementation")
            .expect("implementation has a revision");
        let decision = case.evaluate(0).expect("evaluates");
        assert_eq!(
            status(&decision.outcomes, "abandoned"),
            "blocked",
            "{}",
            decision.json
        );
        for (n, (kind, result, _)) in LADDER.iter().enumerate() {
            case.add_evidence(&record(n + 1, kind, result, &revision))
                .expect("the record is written");
            let decision = case.evaluate(n + 1).expect("evaluates");
            assert_eq!(
                status(&decision.outcomes, "abandoned"),
                "blocked",
                "after {kind} {result}\n{}",
                decision.json
            );
        }
        let decision = case.evaluate(9).expect("evaluates");
        assert_eq!(
            decision.legitimate(),
            vec!["accepted".to_owned()],
            "{}",
            decision.json
        );
    }
}
