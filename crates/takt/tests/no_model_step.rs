//! story:no-model-step-for-approval: when the only thing between a case and its outcome is an
//! action that requires a capability not granted, the run takes no judgement step: it stops
//! `ApprovalRequired` naming the capability's action, with no harness call and no token spent.
//! A case with a judgement action that could still change a claim still offers it.

use std::cell::{Cell, RefCell};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::rc::Rc;

use takt::evidence::Record;
use takt::forge::{Checks, Forge, Memory};
use takt::harness::{Harness, StepContext, StepReport};
use takt::validator::{CiRead, PrMerged, PrOpen, Validator};
use takt::{Run, RunOptions, Stop};

const PROTOCOL: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../protocols/story-delivery/1.yaml"
);

fn git(tree: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .args(["-c", "user.name=t", "-c", "user.email=t@localhost"])
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

struct Scratch(PathBuf);

impl std::ops::Deref for Scratch {
    type Target = Path;

    fn deref(&self) -> &Path {
        &self.0
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn scratch(name: &str) -> Scratch {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("no-model-step-{}-{name}", std::process::id()));
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

/// Records every call and what it was offered; requests the merge when it is offered, and
/// otherwise reports an edit that changes nothing. Each call reports tokens, as a model does.
#[derive(Default)]
struct Recording {
    calls: Cell<usize>,
    offered: RefCell<Vec<Vec<String>>>,
}

impl Harness for Recording {
    fn name(&self) -> &str {
        "recording"
    }

    fn step(&self, context: &StepContext<'_>) -> anyhow::Result<StepReport> {
        self.calls.set(self.calls.get() + 1);
        self.offered.borrow_mut().push(context.offered.to_vec());
        let action = if context.approvals.contains(&"repository.merge".to_owned()) {
            "repository.merge"
        } else {
            "implementation.edit"
        };
        Ok(StepReport {
            action: action.to_owned(),
            summary: "asked".to_owned(),
            tokens: Some(29_908),
        })
    }
}

fn record(n: usize, kind: &str, revision: &str) -> Record {
    Record::new(
        format!("e{n}"),
        kind,
        Some("pass".to_owned()),
        "implementation",
        revision,
        "no-model-step-test",
    )
}

/// A case whose work is committed, green and handed over at it, run with `pr.open`,
/// `ci.read` and the merge validator over a forge double whose checks answer `checks`: the
/// stop, what was printed, and the run's tokens.
fn worked_run(name: &str, checks: Checks, harness: &dyn Harness) -> (Stop, String, u64) {
    let tree = scratch(name);
    let memory = Rc::new(Memory {
        checks,
        ..Memory::default()
    });
    let forge: Rc<dyn Forge> = memory;
    let validators: Vec<Box<dyn Validator>> = vec![
        Box::new(PrOpen::new(forge.clone())),
        Box::new(CiRead::new(forge.clone())),
        Box::new(PrMerged::new(forge)),
    ];
    let options = RunOptions {
        protocol: PROTOCOL.to_owned(),
        intent: "deliver the story".to_owned(),
        inputs: Vec::new(),
        iterations: Some(10),
        hours: None,
        tokens: None,
        case_id: Some(name.to_owned()),
        gate: Some("repository.merge".to_owned()),
    };
    let mut run = Run::open(&tree, options, harness, &validators).expect("the case opens");
    fs::write(tree.join("work.txt"), "done\n").expect("a file");
    git(&tree, &["add", "work.txt"]);
    git(&tree, &["commit", "-q", "-m", "the work"]);
    let revision = git(&tree, &["rev-parse", "HEAD"]);
    run.case
        .set_revision("implementation", &revision)
        .expect("the revision moves");
    run.case
        .add_evidence(&record(1, "test_result", &revision))
        .expect("written");
    run.case
        .add_evidence(&record(2, "gate_result", &revision))
        .expect("written");
    // The change handed over at the revision, as takt records a `change.handover` step.
    run.case
        .add_evidence(&Record::new(
            "e3",
            "handover",
            None,
            "implementation",
            &revision,
            takt::run::CHOICE_PRODUCER,
        ))
        .expect("written");
    let mut out = Vec::new();
    let stop = run.run(&mut out).expect("the run stops without an error");
    let text = String::from_utf8(out).expect("utf-8");
    (stop, text, run.tokens)
}

/// The story's Expected before: every claim but `pr.merged` held, the merge needing a grant not
/// given. On the base the harness was called once, to name the approval, and its tokens were
/// spent; now the run stops at once, naming `repository.merge`, with every step a validator's.
#[test]
fn only_an_ungranted_capability_left_stops_with_no_harness_call() {
    let harness = Recording::default();
    let (stop, text, tokens) = worked_run("gate-only", Checks::Pass, &harness);
    assert_eq!(
        stop,
        Stop::ApprovalRequired(vec!["repository.merge".to_owned()]),
        "{text}"
    );
    assert_eq!(
        harness.calls.get(),
        0,
        "the harness was called to ask for the approval: {text}"
    );
    assert_eq!(tokens, 0, "{text}");
    assert!(!text.contains("(recording)"), "{text}");
}

/// The other half of the acceptance: continuous integration failed, so `ci.green` is false and
/// an edit could still change it; the harness is offered its judgement step.
#[test]
fn a_claim_an_edit_could_still_change_keeps_the_judgement_step() {
    let harness = Recording::default();
    let (stop, text, _) = worked_run("ci-failed", Checks::Fail, &harness);
    assert_eq!(harness.calls.get(), 1, "stopped {stop}: {text}");
    let offered = harness.offered.borrow();
    assert!(
        offered[0].contains(&"implementation.edit".to_owned()),
        "{offered:?}\n{text}"
    );
    assert_eq!(stop, Stop::NoProgress, "{text}");
}

/// The rule reads the protocol, not the story protocol's names: a claim decided only by
/// evidence an action needing a capability produces (`release.ship`, requiring `release`),
/// with no gate named for the run, stops the run at its first decision naming that action,
/// with no harness call; a claim another action could still decide keeps the step.
#[test]
fn any_protocol_whose_open_claims_wait_only_for_a_grant_stops_before_the_harness() {
    let protocol = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("no-model-step-{}-ship.yaml", std::process::id()));
    let document = |extra: &str| {
        format!(
            "format: protocol/1\n\
             protocol:\n  id: ship.only\n  revision: 1\n\
             artifacts:\n  implementation:\n    description: the code\n\
             evidence_kinds:\n  ship_record:\n    description: the release shipped\n\
             claims:\n  shipped:\n    description: shipped\n    true_when:\n      evidence:\n        kind: ship_record\n        result: shipped\n        subject: implementation\n\
             actions:\n  implementation.edit:\n    description: change the code\n    effect: write\n\
             \x20 release.ship:\n    description: ship it\n    effect: write\n    requires:\n      - capability: release\n    may_produce:\n      - evidence: ship_record\n\
             {extra}\
             outcomes:\n  accepted:\n    description: shipped\n    requires:\n      claim: shipped\n"
        )
    };
    let run_on = |yaml: &str, id: &str| {
        let tree = scratch(id);
        fs::write(&protocol, yaml).expect("a protocol");
        let harness = Recording::default();
        let validators: Vec<Box<dyn Validator>> = Vec::new();
        let options = RunOptions {
            protocol: protocol.display().to_string(),
            intent: "ship".to_owned(),
            inputs: Vec::new(),
            iterations: Some(3),
            hours: None,
            tokens: None,
            case_id: Some(id.to_owned()),
            gate: None,
        };
        let mut run = Run::open(&tree, options, &harness, &validators).expect("the case opens");
        let mut out = Vec::new();
        let stop = run.run(&mut out).map_err(|error| format!("{error:#}"));
        let text = String::from_utf8(out).expect("utf-8");
        (stop, text, harness.calls.get())
    };

    let (stop, text, calls) = run_on(&document(""), "ship-gated");
    assert_eq!(
        stop,
        Ok(Stop::ApprovalRequired(vec!["release.ship".to_owned()])),
        "{text}"
    );
    assert_eq!(calls, 0, "{text}");

    // A second producer of `ship_record` that needs no grant: an edit and a validator could
    // still decide the claim, so the harness is asked.
    let (stop, text, calls) = run_on(
        &document(
            "\x20 release.observe:\n    description: observe the release\n    effect: read\n    may_produce:\n      - evidence: ship_record\n",
        ),
        "ship-observable",
    );
    assert_eq!(calls, 1, "stopped {stop:?}: {text}");
    let _ = fs::remove_file(&protocol);
}
