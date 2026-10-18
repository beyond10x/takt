//! Adversary pass 2 on the efficiency unit (story:pr-opens-when-change-is-done,
//! story:no-model-step-for-approval, story:no-inspect-for-stateless-harness), after correction
//! 1: `change.read` and `change.made`, the choice rule against the evidence rule, and the
//! refusal of a choice kind read about no artifact. Each case is named for the defect it shows.
//! Fake harnesses and the in-memory forge only.

use std::cell::RefCell;
use std::collections::VecDeque;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::rc::Rc;

use takt::evidence::Record;
use takt::forge::{Checks, Forge, Memory};
use takt::harness::{Harness, StepContext, StepReport};
use takt::validator::{ChangeRead, CiRead, Observation, PrOpen, Validator};
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

const SEED: &str = "a story\n";

fn scratch(name: &str) -> Scratch {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("efficiency-adv2-{}-{name}", std::process::id()));
    if dir.exists() {
        fs::remove_dir_all(&dir).expect("a stale scratch tree is removed");
    }
    fs::create_dir_all(&dir).expect("scratch dir");
    fs::write(dir.join("README.md"), SEED).expect("a file");
    git(&dir, &["init", "-q", "-b", "main"]);
    git(&dir, &["add", "-A"]);
    git(&dir, &["commit", "-q", "-m", "start"]);
    Scratch(dir)
}

/// The records of `kind` under `case_dir`, each as its JSON.
fn records(case_dir: &Path, kind: &str) -> Vec<serde_json::Value> {
    let Ok(entries) = fs::read_dir(case_dir.join("evidence")) else {
        return Vec::new();
    };
    entries
        .map(|entry| {
            serde_json::from_str(
                &fs::read_to_string(entry.expect("an entry").path()).expect("a file"),
            )
            .expect("json")
        })
        .filter(|record: &serde_json::Value| record["kind"] == kind)
        .collect()
}

/// Always `pass` at every revision: tests and a gate that stay green.
struct Green {
    action: &'static str,
    kind: &'static str,
}

impl Validator for Green {
    fn name(&self) -> &str {
        self.action
    }

    fn action(&self) -> &str {
        self.action
    }

    fn kind(&self) -> &str {
        self.kind
    }

    fn subject(&self) -> &str {
        "implementation"
    }

    fn observe(&self, _: &Path, revision: &str, seq: usize) -> anyhow::Result<Observation> {
        Ok(Observation {
            summary: "pass".to_owned(),
            artifact: None,
            records: vec![Record::new(
                format!("{}-{seq}", self.kind),
                self.kind,
                Some("pass".to_owned()),
                self.subject(),
                revision,
                self.name(),
            )],
        })
    }
}

/// One move of [`Script`].
#[derive(Clone, Copy)]
enum Move {
    /// `implementation.edit`: the file's content set to the text, reported with the summary.
    Write(&'static str, &'static str, &'static str),
    /// `change.handover`, changing nothing.
    HandOver,
    /// Any named action, changing nothing.
    Name(&'static str),
}

/// Plays its moves in order; once they are played it names the merge when it is offered as an
/// approval, else an edit that changes nothing.
struct Script {
    moves: RefCell<VecDeque<Move>>,
}

impl Script {
    fn new(moves: &[Move]) -> Self {
        Self {
            moves: RefCell::new(moves.iter().copied().collect()),
        }
    }
}

impl Harness for Script {
    fn name(&self) -> &str {
        "script"
    }

    fn step(&self, context: &StepContext<'_>) -> anyhow::Result<StepReport> {
        let next = self.moves.borrow_mut().pop_front();
        let (action, summary) = match next {
            Some(Move::Write(path, text, summary)) => {
                fs::write(context.tree.join(path), text)?;
                ("implementation.edit", summary)
            }
            Some(Move::HandOver) => ("change.handover", "handed over"),
            Some(Move::Name(action)) => (action, "named"),
            None if context.approvals.contains(&"repository.merge".to_owned()) => {
                ("repository.merge", "merge")
            }
            None => ("implementation.edit", "nothing"),
        };
        Ok(StepReport {
            action: action.to_owned(),
            summary: summary.to_owned(),
            tokens: Some(100),
        })
    }
}

fn copy(from: &Path, to: &Path) {
    fs::create_dir_all(to).expect("a directory");
    for entry in fs::read_dir(from).expect("a directory") {
        let entry = entry.expect("an entry");
        let target = to.join(entry.file_name());
        if entry.file_type().expect("a type").is_dir() {
            copy(&entry.path(), &target);
        } else {
            fs::copy(entry.path(), &target).expect("a copy");
        }
    }
}

/// What a delivery run left: the stop (or the error), what was printed, a copy of the case
/// directory, and whether the last implementation revision's tree is the base's.
struct Delivered {
    stop: Result<Stop, String>,
    text: String,
    case_dir: PathBuf,
    tree_is_the_base: bool,
}

/// A delivery run with green tests and gate, `change.read`, and `pr.open` and `ci.read` over
/// `memory`, as the command line registers them.
fn delivered(name: &str, memory: &Rc<Memory>, harness: &dyn Harness) -> Delivered {
    let tree = scratch(name);
    let base = git(&tree, &["rev-parse", "HEAD^{tree}"]);
    let forge: Rc<dyn Forge> = memory.clone();
    let validators: Vec<Box<dyn Validator>> = vec![
        Box::new(Green {
            action: "tests.run",
            kind: "test_result",
        }),
        Box::new(ChangeRead),
        Box::new(Green {
            action: "gate.run",
            kind: "gate_result",
        }),
        Box::new(PrOpen::new(forge.clone())),
        Box::new(CiRead::new(forge)),
    ];
    let options = RunOptions {
        protocol: PROTOCOL.to_owned(),
        intent: "remove a todo by its number".to_owned(),
        inputs: Vec::new(),
        iterations: Some(40),
        hours: None,
        tokens: None,
        case_id: Some(name.to_owned()),
        gate: Some("repository.merge".to_owned()),
    };
    let mut run = Run::open(&tree, options, harness, &validators).expect("the case opens");
    let mut out = Vec::new();
    let stop = run.run(&mut out).map_err(|error| format!("{error:#}"));
    let dir = run.case.dir.clone();
    let revision = run
        .case
        .revision("implementation")
        .expect("an implementation revision");
    drop(run);
    let tree_is_the_base = git(&tree, &["rev-parse", &format!("{revision}^{{tree}}")]) == base;
    let kept = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!(
        "efficiency-adv2-{}-{name}-case",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&kept);
    copy(&dir, &kept);
    Delivered {
        stop,
        text: String::from_utf8(out).expect("utf-8"),
        case_dir: kept,
        tree_is_the_base,
    }
}

/// `change.made` says "there is a change to publish", and `change.read` decides it by counting
/// commits beyond the base, not by comparing content. A harness that edits a file and then
/// writes it back as it was, then hands over, has a revision whose tree is the base's: two
/// commits ahead, nothing changed. `change.read` writes `ahead`, `pr.open` pushes and opens a
/// pull request with an empty diff, and continuous integration is read for it; the merge gate is
/// then all that stands between the case and `accepted` with nothing delivered.
#[test]
fn adv2_change_read_reads_ahead_for_a_revision_whose_tree_is_the_base() {
    let memory = Rc::new(Memory {
        checks: Checks::Pass,
        ..Memory::default()
    });
    let harness = Script::new(&[
        Move::Write("README.md", "a story\nmore\n", "edited"),
        Move::Write("README.md", SEED, "put back"),
        Move::HandOver,
    ]);
    let run = delivered("tree-is-base", &memory, &harness);
    assert!(
        run.tree_is_the_base,
        "the scenario needs the handed-over revision's tree to be the base's: {}",
        run.text
    );
    let change = records(&run.case_dir, "change_record");
    let opened = memory.opened.borrow().len();
    assert!(
        opened == 0 && change.iter().all(|record| record["result"] != "ahead"),
        "a revision whose tree is the base's was read as a change ({:?}) and {opened} pull request(s) opened for it (stopped {:?}):\n{}",
        change
            .iter()
            .map(|record| record["result"].clone())
            .collect::<Vec<_>>(),
        run.stop,
        run.text
    );
}

/// `change.read` once left out takt's take with `--invert-grep --grep=^takt: take [^ ]+$`, which
/// git matches against every line of a commit message, and the commit of a judgement step is
/// `<action>: <summary>`, the summary the harness reported (a model's JSON string, which may
/// hold a newline). An edit whose summary had a second line `takt: take <anything>` was not
/// counted: the change it commits was read `none`, `change.made` false, and the hand-over
/// opened no pull request for a revision that does change the code. `change.read` now compares
/// trees, and a judgement step's commit message is one line; the case stays as the check.
#[test]
fn adv2_an_edit_whose_summary_carries_a_take_line_is_read_as_no_change() {
    let memory = Rc::new(Memory {
        checks: Checks::Pass,
        ..Memory::default()
    });
    let harness = Script::new(&[
        Move::Write(
            "README.md",
            "a story\nthe fix\n",
            "fixed the story\ntakt: take story:anything",
        ),
        Move::HandOver,
    ]);
    let run = delivered("take-line", &memory, &harness);
    assert!(
        !run.tree_is_the_base,
        "the scenario needs a revision that changes the code: {}",
        run.text
    );
    let change = records(&run.case_dir, "change_record");
    let opened = memory.opened.borrow().len();
    assert!(
        opened == 1 && change.iter().any(|record| record["result"] == "ahead"),
        "a revision with a real edit was read as no change ({:?}) and {opened} pull request(s) opened (stopped {:?}):\n{}",
        change
            .iter()
            .map(|record| record["result"].clone())
            .collect::<Vec<_>>(),
        run.stop,
        run.text
    );
}

/// AGENTS.md: a choice record "never makes a claim about the work true (`tests.pass`, ...),
/// and a protocol may gate an action on it but never an outcome on it alone". The choice rule
/// keys on action names (`BUILT_IN_ACTIONS`), not on the kinds the built-in validators write.
/// A protocol whose `tests.pass` reads `test_result` with no result, produced by an action named
/// `tests.check` (effect read): the harness names it, takt writes a `test_result` by
/// `takt.choice`, `tests.pass` holds and the case is accepted on that record alone.
#[test]
fn adv2_a_choice_record_makes_tests_pass_true_when_its_action_is_not_named_tests_run() {
    let yaml = "format: protocol/1\n\
        protocol:\n  id: tests.claimed\n  revision: 1\n\
        artifacts:\n  implementation:\n    description: the code\n\
        evidence_kinds:\n  test_result:\n    description: the result of the tests on a revision\n\
        claims:\n  tests.pass:\n    description: the tests pass\n    true_when:\n      evidence:\n        kind: test_result\n        subject: implementation\n\
        actions:\n  implementation.edit:\n    description: change the code\n    effect: write\n\
        \x20 tests.check:\n    description: check the tests\n    effect: read\n    may_produce:\n      - evidence: test_result\n\
        outcomes:\n  accepted:\n    description: the tests pass\n    requires:\n      claim: tests.pass\n";
    let harness = Script::new(&[Move::Name("tests.check")]);
    // Since correction 3 such a protocol is refused when the case would open, before anything
    // is written: a read action outside the built-in validators may not produce `test_result`,
    // so takt never writes one from the harness's choice.
    let tree = scratch("tests-claimed-open");
    let protocol = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!(
        "efficiency-adv2-{}-tests-claimed-open.yaml",
        std::process::id()
    ));
    fs::write(&protocol, yaml).expect("a protocol");
    let options = RunOptions {
        protocol: protocol.display().to_string(),
        intent: "deliver".to_owned(),
        inputs: Vec::new(),
        iterations: Some(5),
        hours: None,
        tokens: None,
        case_id: Some("tests-claimed-open".to_owned()),
        gate: None,
    };
    let validators: Vec<Box<dyn Validator>> = Vec::new();
    let opened = Run::open(&tree, options, &harness, &validators).err();
    let _ = fs::remove_file(&protocol);
    let error = format!("{:#}", opened.expect("the protocol is refused at open"));
    assert!(
        error.contains("tests.check") && error.contains("test_result"),
        "{error}"
    );
    assert!(
        !tree
            .join(takt::case::CASES_DIR)
            .join("tests-claimed-open")
            .exists(),
        "a case was written for a refused protocol"
    );
}

/// Correction 1 refuses a choice kind read about no artifact in `Shape::of`, which runs at the
/// start of `Run::run`: after `Run::open` has created the case directory, created the case's
/// branch and checked it out. The protocol is refused, and the case it can never run is left
/// open on its own branch; a story's run has taken the story by then.
#[test]
fn adv2_a_subject_less_choice_kind_is_refused_only_after_the_case_is_opened() {
    let yaml = "format: protocol/1\n\
        protocol:\n  id: ack.only\n  revision: 1\n\
        artifacts:\n  implementation:\n    description: the code\n\
        evidence_kinds:\n  ack:\n    description: the harness acknowledged\n\
        claims:\n  acked:\n    description: acknowledged\n    true_when:\n      evidence:\n        kind: ack\n\
        actions:\n  implementation.edit:\n    description: change the code\n    effect: write\n\
        \x20 ack.give:\n    description: acknowledge\n    effect: read\n    may_produce:\n      - evidence: ack\n\
        outcomes:\n  accepted:\n    description: acknowledged\n    requires:\n      claim: acked\n";
    let name = "ack-refused";
    let tree = scratch(name);
    let protocol = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!(
        "efficiency-adv2-{}-{name}.yaml",
        std::process::id()
    ));
    fs::write(&protocol, yaml).expect("a protocol");
    let harness = Script::new(&[]);
    let validators: Vec<Box<dyn Validator>> = Vec::new();
    let options = RunOptions {
        protocol: protocol.display().to_string(),
        intent: "deliver".to_owned(),
        inputs: Vec::new(),
        iterations: Some(5),
        hours: None,
        tokens: None,
        case_id: Some(name.to_owned()),
        gate: None,
    };
    let opened = Run::open(&tree, options, &harness, &validators);
    let _ = fs::remove_file(&protocol);
    let Ok(mut run) = opened else {
        return;
    };
    let mut out = Vec::new();
    let refused = run.run(&mut out).map_err(|error| format!("{error:#}"));
    let dir = run.case.dir.clone();
    drop(run);
    let branch = git(&tree, &["branch", "--show-current"]);
    assert!(
        !dir.join("case.yaml").exists() && branch == "main",
        "the case was opened ({} exists, the tree is on {branch}) and then refused: {refused:?}",
        dir.display()
    );
}
