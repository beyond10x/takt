//! A run whose gate fails the same gates at every revision stops `RepeatedGateFailure` after
//! the third such `gate.run` result, each at a new implementation revision, naming the gates; no
//! judgement step follows it, and the case stays open.

use std::cell::Cell;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use takt::evidence::Record;
use takt::harness::{Harness, StepContext, StepReport};
use takt::run::REPEATED_GATE_FAILURES;
use takt::validator::{FailedCheck, GateRun, Observation, TestsRun, Validator};
use takt::{Run, RunOptions, Stop};

fn git(tree: &Path, args: &[&str]) {
    let status = Command::new("git")
        .args(["-c", "user.name=t", "-c", "user.email=t@localhost"])
        .args(args)
        .current_dir(tree)
        .status()
        .expect("git runs");
    assert!(status.success(), "git {args:?}");
}

/// A scratch tree under this crate's build directory, removed on drop.
struct Scratch(PathBuf);

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn scratch(name: &str) -> Scratch {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("gate-failures-{}-{name}", std::process::id()));
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

/// `tests.run` passing at every revision.
struct Green;

impl Validator for Green {
    fn name(&self) -> &str {
        "green-tests"
    }

    fn action(&self) -> &str {
        TestsRun::ACTION
    }

    fn kind(&self) -> &str {
        TestsRun::KIND
    }

    fn subject(&self) -> &str {
        "implementation"
    }

    fn observe(&self, _: &Path, revision: &str, seq: usize) -> anyhow::Result<Observation> {
        Ok(Observation {
            summary: "pass".to_owned(),
            artifact: None,
            records: vec![Record::new(
                format!("tests-{seq}"),
                TestsRun::KIND,
                Some("pass".to_owned()),
                "implementation",
                revision,
                self.name(),
            )],
        })
    }
}

/// `gate.run` failing the gates `citations` and `format` at every revision.
struct RedGate {
    failed: Cell<bool>,
}

impl Validator for RedGate {
    fn name(&self) -> &str {
        "red-gate"
    }

    fn action(&self) -> &str {
        GateRun::ACTION
    }

    fn kind(&self) -> &str {
        GateRun::KIND
    }

    fn subject(&self) -> &str {
        "implementation"
    }

    fn observe(&self, _: &Path, revision: &str, seq: usize) -> anyhow::Result<Observation> {
        self.failed.set(true);
        Ok(Observation {
            summary: "fail; gate citations false, gate format false".to_owned(),
            artifact: None,
            records: vec![Record::new(
                format!("gate-{seq}"),
                GateRun::KIND,
                Some("fail".to_owned()),
                "implementation",
                revision,
                self.name(),
            )],
        })
    }

    fn take_failed(&self) -> Vec<FailedCheck> {
        if !self.failed.replace(false) {
            return Vec::new();
        }
        ["citations", "format"]
            .into_iter()
            .map(|id| FailedCheck {
                id: id.to_owned(),
                lines: vec![format!("{id} failed")],
                omitted: 0,
            })
            .collect()
    }
}

/// Edits the implementation at every step, each a new revision, and counts its steps.
struct Editor {
    steps: Cell<u32>,
}

impl Harness for Editor {
    fn name(&self) -> &str {
        "editor"
    }

    fn step(&self, context: &StepContext<'_>) -> anyhow::Result<StepReport> {
        let n = self.steps.get() + 1;
        self.steps.set(n);
        let at = context.tree.join("src/edit.txt");
        fs::create_dir_all(at.parent().expect("a parent")).expect("a directory");
        fs::write(&at, format!("edit {n}\n")).expect("a file");
        Ok(StepReport {
            action: "implementation.edit".to_owned(),
            summary: format!("edit {n}"),
            tokens: Some(10),
        })
    }
}

#[test]
fn three_gate_failures_of_the_same_gates_at_new_revisions_stop_the_run() {
    let tree = scratch("same");
    let validators: Vec<Box<dyn Validator>> = vec![
        Box::new(Green),
        Box::new(RedGate {
            failed: Cell::new(false),
        }),
    ];
    let editor = Editor {
        steps: Cell::new(0),
    };
    let options = RunOptions {
        protocol: "story.delivery/2".to_owned(),
        intent: "a story".to_owned(),
        inputs: vec![
            ("tests_first".to_owned(), "optional".to_owned()),
            ("gate_on".to_owned(), "each_revision".to_owned()),
        ],
        iterations: Some(40),
        hours: None,
        tokens: None,
        case_id: Some("gates".to_owned()),
        gate: None,
    };
    let mut run = Run::open(&tree.0, options, &editor, &validators).expect("the case opens");
    let mut out = Vec::new();
    let stop = run.run(&mut out).map_err(|error| format!("{error:#}"));
    let text = String::from_utf8(out).expect("utf-8");
    assert_eq!(
        stop,
        Ok(Stop::RepeatedGateFailure {
            gates: vec!["citations".to_owned(), "format".to_owned()],
            count: REPEATED_GATE_FAILURES,
        }),
        "{text}"
    );
    assert_eq!(
        text.matches("evidence: gate_result fail").count(),
        3,
        "{text}"
    );
    assert_eq!(
        editor.steps.get(),
        2,
        "a judgement step after the third failure: {text}"
    );
    assert!(
        !takt::case::is_finished(&run.case.dir).expect("the case reads"),
        "the case stays open: {text}"
    );
}
