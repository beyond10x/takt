//! story:pr-opens-when-change-is-done: a story run opens its pull request once the change is
//! complete, so continuous integration runs once per accepted unit, and no `pr.open` is taken at
//! the seed, before any change exists. A harness that edits the implementation, then writes the
//! tests, is the shape of scenario 1: on the base the run pushed and read CI after each edit.

#[path = "support/own_home.rs"]
mod own_home;

use std::cell::{Cell, RefCell};
use std::collections::VecDeque;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::rc::Rc;

use takt::case::CASES_DIR;
use takt::evidence::Record;
use takt::forge::{Checks, Forge, Memory};
use takt::harness::{Harness, StepContext, StepReport};
use takt::validator::{ChangeRead, CiRead, Observation, PrOpen, TestsRun, Validator};
use takt::{Run, RunOptions, Stop};

const PROTOCOL: &str = "story.delivery/1";

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

/// Adds `text` to the index of `tree` at `path`, byte for byte, through git alone: git holds a
/// name that is not UTF-8, which a file system that keeps names in UTF-8 only (APFS) refuses.
#[cfg(unix)]
fn stage(tree: &Path, path: &[u8], text: &[u8]) {
    use std::io::Write as _;
    use std::os::unix::ffi::OsStrExt as _;
    use std::process::Stdio;
    let mut hash = Command::new("git")
        .args(["hash-object", "-w", "--stdin"])
        .current_dir(tree)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("git runs");
    hash.stdin
        .take()
        .expect("its input")
        .write_all(text)
        .expect("the blob");
    let hashed = hash.wait_with_output().expect("git ends");
    assert!(hashed.status.success(), "git hash-object");
    let blob = String::from_utf8(hashed.stdout).expect("utf-8");
    let added = Command::new("git")
        .args([
            "update-index",
            "--add",
            "--cacheinfo",
            "100644",
            blob.trim(),
        ])
        .arg(std::ffi::OsStr::from_bytes(path))
        .current_dir(tree)
        .status()
        .expect("git runs");
    assert!(added.success(), "git update-index");
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
        .join(format!("pr-opens-when-done-{}-{name}", std::process::id()));
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

/// A validator that observes `pass` for its action at every revision: the tests and the gate of
/// a repository whose work keeps them green.
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

/// Scenario 1's developer: edits the implementation, then writes the story's tests, one step
/// each; then requests the merge when it is offered, and otherwise hands the finished change
/// over by taking `change.handover`, changing nothing.
struct Developer {
    edits: RefCell<VecDeque<(&'static str, &'static str)>>,
    calls: Cell<usize>,
}

impl Developer {
    fn new() -> Self {
        Self {
            edits: RefCell::new(VecDeque::from([
                ("implementation.edit", "src/remove.txt"),
                ("tests.write", "tests/remove_test.txt"),
            ])),
            calls: Cell::new(0),
        }
    }
}

impl Harness for Developer {
    fn name(&self) -> &str {
        "developer"
    }

    fn step(&self, context: &StepContext<'_>) -> anyhow::Result<StepReport> {
        self.calls.set(self.calls.get() + 1);
        let next = self.edits.borrow_mut().pop_front();
        if let Some((action, path)) = next {
            let at = context.tree.join(path);
            fs::create_dir_all(at.parent().expect("a parent")).expect("a directory");
            fs::write(&at, format!("{action}\n")).expect("a file");
            return Ok(StepReport {
                action: action.to_owned(),
                summary: format!("wrote {path}"),
                tokens: Some(100),
            });
        }
        let action = if context.approvals.contains(&"repository.merge".to_owned()) {
            "repository.merge"
        } else {
            "change.handover"
        };
        Ok(StepReport {
            action: action.to_owned(),
            summary: "the change is complete".to_owned(),
            tokens: Some(100),
        })
    }
}

/// The records of `kind` under the case, each as its JSON.
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

/// The validator steps of `action` the run printed.
fn validator_steps(text: &str, action: &str) -> usize {
    text.lines()
        .filter(|line| line.starts_with("step ") && line.ends_with(&format!(": {action} {{}}")))
        .count()
}

/// Scenario 1 on the fake forge: the developer edits the implementation, then writes the tests.
/// The story's Expected before, on the base: `pr.open` three times (at the seed, after the
/// implementation, after the tests) and `ci_result pass` at two revisions before
/// `ApprovalRequired`. The acceptance: exactly one CI result, at the revision the grant is
/// given at, and no `pr.open` before the first implementation commit.
#[test]
fn the_pull_request_opens_once_the_change_is_complete_and_ci_runs_once() {
    let tree = scratch("scenario-1");
    let memory = Rc::new(Memory {
        checks: Checks::Pass,
        ..Memory::default()
    });
    let forge: Rc<dyn Forge> = memory.clone();
    let validators: Vec<Box<dyn Validator>> = vec![
        Box::new(Green {
            action: "tests.run",
            kind: "test_result",
        }),
        Box::new(Green {
            action: "gate.run",
            kind: "gate_result",
        }),
        Box::new(PrOpen::new(forge.clone())),
        Box::new(CiRead::new(forge)),
        Box::new(ChangeRead),
    ];
    let options = RunOptions {
        protocol: PROTOCOL.to_owned(),
        intent: "remove a todo by its number".to_owned(),
        inputs: Vec::new(),
        iterations: Some(40),
        hours: None,
        tokens: None,
        case_id: Some("scenario-1".to_owned()),
        gate: Some("repository.merge".to_owned()),
    };
    let harness = Developer::new();
    let mut run = Run::open(&tree, options, &harness, &validators).expect("the case opens");
    let mut out = Vec::new();
    let stop = run.run(&mut out).map_err(|error| format!("{error:#}"));
    let text = String::from_utf8(out).expect("utf-8");

    assert_eq!(
        stop,
        Ok(Stop::ApprovalRequired(vec!["repository.merge".to_owned()])),
        "{text}"
    );
    assert_eq!(
        validator_steps(&text, "pr.open"),
        1,
        "pr.open taken more than once: {text}"
    );
    let first_commit = text
        .find("effect: committed")
        .unwrap_or_else(|| panic!("no implementation commit: {text}"));
    let first_pr_open = text.find(": pr.open {}").unwrap_or(usize::MAX);
    assert!(
        first_pr_open > first_commit,
        "pr.open taken before the first implementation commit: {text}"
    );
    let case_dir = tree.join(CASES_DIR).join("scenario-1");
    let ci = records(&case_dir, "ci_result");
    let revision = run
        .case
        .revision("implementation")
        .expect("implementation has a revision");
    assert_eq!(ci.len(), 1, "CI results: {ci:?}\n{text}");
    assert_eq!(ci[0]["result"], "pass", "{text}");
    assert_eq!(
        ci[0]["subject_revision"],
        revision.as_str(),
        "the one CI result is not at the revision the grant is given at: {text}"
    );
    let calls = memory.calls.borrow().clone();
    assert_eq!(
        calls.iter().filter(|c| c.starts_with("open ")).count(),
        1,
        "{calls:?}"
    );
    assert_eq!(
        calls.iter().filter(|c| c.starts_with("push ")).count(),
        1,
        "one push for the one complete change: {calls:?}"
    );
    assert_eq!(
        calls.iter().filter(|c| c.starts_with("checks ")).count(),
        1,
        "{calls:?}"
    );
    eprintln!(
        "scenario 1 on the fake forge: steps {}, harness calls {}, pr.open {}, ci.read {}, CI results {}",
        run.steps,
        harness.calls.get(),
        validator_steps(&text, "pr.open"),
        validator_steps(&text, "ci.read"),
        ci.len()
    );
}

/// One step of a [`Scripted`] harness: an edit of a file, a hand-over (`change.handover`), or a
/// hand-over that also writes a file.
#[derive(Clone, Copy)]
enum Move {
    Edit(&'static str),
    HandOver,
    HandOverEditing(&'static str),
}

/// Plays its moves in order, keeping whether each step was offered `change.handover`; once
/// they are played it reports an edit that changes nothing.
struct Scripted {
    moves: RefCell<VecDeque<Move>>,
    handover_offered: RefCell<Vec<bool>>,
}

impl Scripted {
    fn new(moves: &[Move]) -> Self {
        Self {
            moves: RefCell::new(moves.iter().copied().collect()),
            handover_offered: RefCell::new(Vec::new()),
        }
    }
}

impl Harness for Scripted {
    fn name(&self) -> &str {
        "scripted"
    }

    fn step(&self, context: &StepContext<'_>) -> anyhow::Result<StepReport> {
        self.handover_offered
            .borrow_mut()
            .push(context.offered.contains(&"change.handover".to_owned()));
        let next = self.moves.borrow_mut().pop_front();
        let (action, path) = match next {
            Some(Move::Edit(path)) => ("implementation.edit", Some(path)),
            Some(Move::HandOver) => ("change.handover", None),
            Some(Move::HandOverEditing(path)) => ("change.handover", Some(path)),
            None => ("implementation.edit", None),
        };
        if let Some(path) = path {
            let at = context.tree.join(path);
            let before = fs::read_to_string(&at).unwrap_or_default();
            fs::write(
                &at,
                format!("{before}{}\n", self.handover_offered.borrow().len()),
            )?;
        }
        Ok(StepReport {
            action: action.to_owned(),
            summary: "scripted".to_owned(),
            tokens: None,
        })
    }
}

/// A delivery run with green tests and gate and `pr.open` and `ci.read` over `memory`: the
/// stop, what was printed, and the case directory.
fn delivered(name: &str, memory: &Rc<Memory>, harness: &dyn Harness) -> (Stop, String, PathBuf) {
    let tree = scratch(name);
    let forge: Rc<dyn Forge> = memory.clone();
    let validators: Vec<Box<dyn Validator>> = vec![
        Box::new(Green {
            action: "tests.run",
            kind: "test_result",
        }),
        Box::new(Green {
            action: "gate.run",
            kind: "gate_result",
        }),
        Box::new(PrOpen::new(forge.clone())),
        Box::new(CiRead::new(forge)),
        Box::new(ChangeRead),
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
    let stop = run.run(&mut out).expect("the run stops without an error");
    let dir = run.case.dir.clone();
    drop(run);
    // The scratch tree goes when this returns; what the cases read is copied out first.
    let kept = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!(
        "pr-opens-when-done-{}-{name}-case",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&kept);
    copy(&dir, &kept);
    (stop, String::from_utf8(out).expect("utf-8"), kept)
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

/// At the seed the harness that edits first takes no `pr.open`: the protocol blocks it until
/// the change is handed over, and nothing reaches the forge.
#[test]
fn at_the_seed_pr_open_waits_for_the_hand_over_and_nothing_reaches_the_forge() {
    let memory = Rc::new(Memory {
        checks: Checks::Pass,
        ..Memory::default()
    });
    let harness = Scripted::new(&[]);
    let (stop, text, _) = delivered("seed", &memory, &harness);
    assert_eq!(stop, Stop::NoProgress, "{text}");
    assert!(text.contains("pr.open (blocked)"), "{text}");
    assert_eq!(validator_steps(&text, "pr.open"), 0, "{text}");
    assert_eq!(validator_steps(&text, "ci.read"), 0, "{text}");
    assert!(
        memory.calls.borrow().is_empty(),
        "{:?}",
        memory.calls.borrow()
    );
}

/// The hand-over is a step that changes nothing: takt records it as one `handover` record with
/// no result, by its own producer, at the revision, and `change.handed_over` then holds.
#[test]
fn a_hand_over_is_recorded_with_no_result_at_the_revision() {
    let memory = Rc::new(Memory {
        checks: Checks::Pass,
        ..Memory::default()
    });
    let harness = Scripted::new(&[Move::Edit("work.txt"), Move::HandOver]);
    let (stop, text, case_dir) = delivered("recorded", &memory, &harness);
    assert_eq!(
        stop,
        Stop::ApprovalRequired(vec!["repository.merge".to_owned()]),
        "{text}"
    );
    let handovers = records(&case_dir, "handover");
    assert_eq!(handovers.len(), 1, "{handovers:?}\n{text}");
    assert_eq!(handovers[0]["result"], serde_json::Value::Null, "{text}");
    assert_eq!(
        handovers[0]["producer"],
        takt::run::CHOICE_PRODUCER,
        "{text}"
    );
    assert_eq!(handovers[0]["subject"], "implementation", "{text}");
    let ci = records(&case_dir, "ci_result");
    assert_eq!(
        handovers[0]["subject_revision"], ci[0]["subject_revision"],
        "{text}"
    );
}

/// A hand-over that also writes a file is refused, the file is discarded, nothing is recorded
/// and nothing is pushed.
#[test]
fn a_hand_over_with_changes_is_refused_and_nothing_is_pushed() {
    let memory = Rc::new(Memory {
        checks: Checks::Pass,
        ..Memory::default()
    });
    let harness = Scripted::new(&[Move::Edit("work.txt"), Move::HandOverEditing("more.txt")]);
    let (stop, text, case_dir) = delivered("hand-over-editing", &memory, &harness);
    assert!(
        matches!(&stop, Stop::StepRefused(why) if why == "change.handover records a choice and changed 1 path(s)"),
        "{stop:?}\n{text}"
    );
    assert!(records(&case_dir, "handover").is_empty(), "{text}");
    assert_eq!(validator_steps(&text, "pr.open"), 0, "{text}");
    assert!(
        memory.calls.borrow().is_empty(),
        "{:?}",
        memory.calls.borrow()
    );
}

/// Continuous integration fails; the harness edits again, which returns `change.handed_over`
/// to unknown, and the new revision is pushed only once the harness hands it over again: one
/// push per hand-over, one pull request. Where the hand-over is recorded it is not offered.
#[test]
fn after_a_failed_ci_the_next_change_is_pushed_once_it_is_handed_over_again() {
    let memory = Rc::new(Memory {
        checks: Checks::Fail,
        ..Memory::default()
    });
    let harness = Scripted::new(&[
        Move::Edit("work.txt"),
        Move::HandOver,
        Move::Edit("work.txt"),
        Move::Edit("work.txt"),
        Move::HandOver,
    ]);
    let (stop, text, case_dir) = delivered("ci-failed", &memory, &harness);
    assert_eq!(stop, Stop::NoProgress, "{text}");
    let calls = memory.calls.borrow().clone();
    assert_eq!(
        calls.iter().filter(|c| c.starts_with("push ")).count(),
        2,
        "{calls:?}\n{text}"
    );
    assert_eq!(
        calls.iter().filter(|c| c.starts_with("open ")).count(),
        1,
        "{calls:?}"
    );
    assert_eq!(validator_steps(&text, "pr.open"), 2, "{text}");
    assert_eq!(validator_steps(&text, "ci.read"), 2, "{text}");
    assert_eq!(records(&case_dir, "handover").len(), 2, "{text}");
    assert_eq!(
        *harness.handover_offered.borrow(),
        [true, true, false, true, true, false],
        "{text}"
    );
}

/// The list the choice rule and the offer read (`takt::validator::BUILT_IN`) is the validators'
/// own: every `impl Validator for` in the library's sources has an entry, and each entry's
/// action and kind are what an instance of that validator answers. A validator added without
/// its entry fails here, whatever its action is called.
#[test]
fn the_built_in_list_is_every_built_in_validator_with_its_own_action_and_kind() {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut implemented: Vec<String> = Vec::new();
    for entry in fs::read_dir(&src).expect("the sources") {
        let path = entry.expect("an entry").path();
        if path.extension().is_some_and(|ext| ext == "rs") {
            let text = fs::read_to_string(&path).expect("a source");
            for line in text.lines() {
                if let Some(rest) = line.trim().strip_prefix("impl Validator for ") {
                    implemented.push(rest.trim_end_matches(" {").to_owned());
                }
            }
        }
    }
    implemented.sort();
    // A type that serves more than one action has one entry per action, each checked below
    // against an instance made for that action.
    let mut listed: Vec<String> = takt::validator::BUILT_IN
        .iter()
        .map(|built_in| built_in.validator.to_owned())
        .collect();
    listed.sort();
    listed.dedup();
    assert_eq!(implemented, listed);

    let forge: Rc<dyn Forge> = Rc::new(Memory::default());
    let instances: Vec<(&str, Box<dyn Validator>)> = vec![
        (
            "TestsRun",
            Box::new(takt::validator::TestsRun::new("true").expect("a command")),
        ),
        (
            "GateRun",
            Box::new(takt::validator::GateRun::new(".engineering/gates.yaml")),
        ),
        ("PrOpen", Box::new(PrOpen::new(forge.clone()))),
        ("CiRead", Box::new(CiRead::new(forge.clone()))),
        ("PrMerged", Box::new(takt::validator::PrMerged::new(forge))),
        ("ChangeRead", Box::new(ChangeRead)),
        ("LoopMeasure", Box::new(takt::loop_scorecard::LoopMeasure)),
        (
            "LoopDefinitionCheck",
            Box::new(takt::loop_scorecard::LoopDefinitionCheck),
        ),
        ("LoopCompare", Box::new(takt::loop_scorecard::LoopCompare)),
        (
            "FindingFile",
            Box::new(takt::improvement::FindingFile(
                takt::improvement::Checking::new(PathBuf::from("/srv/cache")),
            )),
        ),
        (
            "ScenarioReproduce",
            Box::new(takt::improvement::ScenarioReproduce(
                takt::improvement::Checking::new(PathBuf::from("/srv/cache")),
            )),
        ),
        (
            "ScenarioVerify",
            Box::new(takt::improvement::ScenarioVerify(
                takt::improvement::Checking::new(PathBuf::from("/srv/cache")),
            )),
        ),
        (
            "ScorecardCompare",
            Box::new(takt::improvement::ScorecardCompare::in_state(Path::new(
                "/srv/state",
            ))),
        ),
        (
            "CandidateMerged",
            Box::new(takt::improvement::CandidateMerged::new(
                takt::improvement::Mainline::Local,
            )),
        ),
        (
            "CandidateMerged",
            Box::new(takt::improvement::CandidateMerged::judge_surface(
                takt::improvement::Mainline::Local,
            )),
        ),
        (
            "BinaryPromote",
            Box::new(takt::improvement::BinaryPromote::in_state(Path::new(
                "/srv/state",
            ))),
        ),
    ];
    let mut matched = std::collections::BTreeSet::new();
    for (name, instance) in &instances {
        matched.insert((*name, instance.action().to_owned()));
        let built_in = takt::validator::BUILT_IN
            .iter()
            .find(|built_in| built_in.validator == *name && built_in.action == instance.action())
            .unwrap_or_else(|| panic!("{name} is not listed for {}", instance.action()));
        assert_eq!(
            (built_in.action, built_in.kind),
            (instance.action(), instance.kind()),
            "{name}"
        );
    }
    assert_eq!(instances.len(), takt::validator::BUILT_IN.len());
    assert_eq!(
        matched.len(),
        takt::validator::BUILT_IN.len(),
        "{matched:?}"
    );
}

/// Keeps every offer it is given and answers with an edit that changes nothing.
struct Watching {
    keeps: bool,
    offers: RefCell<Vec<Vec<String>>>,
}

impl Harness for Watching {
    fn name(&self) -> &str {
        "watching"
    }

    fn keeps_state(&self) -> bool {
        self.keeps
    }

    fn step(&self, context: &StepContext<'_>) -> anyhow::Result<StepReport> {
        self.offers.borrow_mut().push(context.offered.to_vec());
        let action = if self.offers.borrow().len() == 1 {
            fs::write(context.tree.join("work.txt"), "the work\n")?;
            "implementation.edit"
        } else {
            "change.handover"
        };
        Ok(StepReport {
            action: action.to_owned(),
            summary: "watched".to_owned(),
            tokens: None,
        })
    }
}

/// `change.read` is the run loop's to take and is never offered to the harness: not to one that
/// keeps state, not with the validator left out of the run, and not with it registered, where
/// the loop takes it once the change is handed over. The same holds for every built-in action.
#[test]
fn change_read_is_taken_by_the_loop_and_never_offered_to_the_harness() {
    for (keeps, registered) in [(false, true), (true, true), (true, false), (false, false)] {
        let tree = scratch(&format!("change-read-offer-{keeps}-{registered}"));
        let mut validators: Vec<Box<dyn Validator>> = vec![
            Box::new(Green {
                action: "tests.run",
                kind: "test_result",
            }),
            Box::new(Green {
                action: "gate.run",
                kind: "gate_result",
            }),
        ];
        if registered {
            validators.push(Box::new(ChangeRead));
        }
        let options = RunOptions {
            protocol: PROTOCOL.to_owned(),
            intent: "remove a todo by its number".to_owned(),
            inputs: Vec::new(),
            iterations: Some(12),
            hours: None,
            tokens: None,
            case_id: Some("offer".to_owned()),
            gate: Some("repository.merge".to_owned()),
        };
        let harness = Watching {
            keeps,
            offers: RefCell::new(Vec::new()),
        };
        let mut run = Run::open(&tree, options, &harness, &validators).expect("the case opens");
        let mut out = Vec::new();
        let _ = run.run(&mut out);
        let text = String::from_utf8(out).expect("utf-8");
        let offers = harness.offers.borrow();
        assert!(!offers.is_empty(), "{text}");
        for offer in offers.iter() {
            for action in offer {
                assert!(
                    !takt::validator::is_built_in_action(action),
                    "{action} offered (keeps state: {keeps}, change.read registered: {registered}): {offer:?}\n{text}"
                );
            }
        }
        assert_eq!(
            text.contains(": change.read {}"),
            registered,
            "the loop takes change.read once handed over, when registered: {text}"
        );
    }
}

/// Opens a case on the protocol document `yaml` with no validator: the error `Run::open` answers,
/// if any, and whether a case directory was written.
fn open_on(name: &str, yaml: &str, inputs: &[(&str, &str)]) -> (Option<String>, bool) {
    let tree = scratch(name);
    let protocol = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!(
        "pr-opens-when-done-{}-{name}.yaml",
        std::process::id()
    ));
    fs::write(&protocol, yaml).expect("a protocol");
    let options = RunOptions {
        protocol: protocol.display().to_string(),
        intent: "deliver".to_owned(),
        inputs: inputs
            .iter()
            .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
            .collect(),
        iterations: Some(1),
        hours: None,
        tokens: None,
        case_id: Some(name.to_owned()),
        gate: None,
    };
    let validators: Vec<Box<dyn Validator>> = Vec::new();
    let harness = Scripted::new(&[]);
    let opened = Run::open(&tree, options, &harness, &validators).err();
    let _ = fs::remove_file(&protocol);
    let written = tree.join(CASES_DIR).join(name).exists();
    (opened.map(|error| format!("{error:#}")), written)
}

/// A protocol head, its one artifact, the `implementation.edit` action, then `rest`.
fn protocol_with(id: &str, rest: &str) -> String {
    format!(
        "format: protocol/1\nprotocol:\n  id: {id}\n  revision: 1\nartifacts:\n  implementation:\n    description: the code\n{rest}"
    )
}

/// F1: an outcome, or an obligation, that holds with every claim a choice record decides true and
/// every other claim unknown is refused when the case would open, naming it, with nothing
/// written. A choice record may gate an action (story delivery's `change.handover` gates
/// `pr.open`), never an outcome or an obligation alone.
#[test]
fn a_protocol_whose_outcome_or_obligation_holds_on_choice_records_alone_is_refused_at_open() {
    let kinds = "evidence_kinds:\n  tests_said:\n    description: the model said the tests pass\n\
        claims:\n  tests.pass:\n    description: the tests pass\n    true_when:\n      evidence:\n        kind: tests_said\n        subject: implementation\n";
    let actions = "actions:\n  implementation.edit:\n    description: change the code\n    effect: write\n\
        \x20 tests.say:\n    description: say the tests pass\n    effect: read\n    may_produce:\n      - evidence: tests_said\n";
    let outcome = protocol_with(
        "tests.said",
        &format!(
            "{kinds}{actions}outcomes:\n  accepted:\n    description: the tests pass\n    requires:\n      claim: tests.pass\n"
        ),
    );
    let (error, written) = open_on("outcome-on-choice", &outcome, &[]);
    let error = error.expect("the protocol is refused");
    assert!(error.contains("outcome accepted"), "{error}");
    assert!(!written, "a case was written for a refused protocol");

    let obligation = protocol_with(
        "tests.said.owed",
        &format!(
            "{kinds}obligations:\n  tests.required:\n    description: the tests pass\n    discharged_when:\n      claim: tests.pass\n\
             {actions}outcomes:\n  abandoned:\n    description: given up\n    requires:\n      decision: explicitly_abandoned\n"
        ),
    );
    let (error, _) = open_on("obligation-on-choice", &obligation, &[]);
    let error = error.expect("the protocol is refused");
    assert!(error.contains("obligation tests.required"), "{error}");

    // Gated on a choice and on evidence only a validator writes, the outcome cannot hold on the
    // choice alone, and the protocol opens.
    let gated = protocol_with(
        "tests.said.and.run",
        "evidence_kinds:\n  tests_said:\n    description: the model said so\n  test_result:\n    description: the tests\n\
             claims:\n  tests.pass:\n    description: the tests pass\n    true_when:\n      all:\n        - evidence:\n            kind: tests_said\n            subject: implementation\n        - evidence:\n            kind: test_result\n            result: pass\n            subject: implementation\n\
             actions:\n  implementation.edit:\n    description: change the code\n    effect: write\n\
             \x20 tests.say:\n    description: say the tests pass\n    effect: read\n    may_produce:\n      - evidence: tests_said\n\
             \x20 tests.run:\n    description: run the tests\n    effect: read\n    may_produce:\n      - evidence: test_result\n\
             outcomes:\n  accepted:\n    description: the tests pass\n    requires:\n      claim: tests.pass\n",
    );
    let (error, written) = open_on("outcome-on-choice-and-tests", &gated, &[]);
    assert_eq!(error, None);
    assert!(written);
}

/// F3: a kind a built-in validator writes that a read action no built-in validator performs may
/// produce is refused when the case would open, nothing written. An action outside the
/// built-in validators that writes one with `effect: write` and needs a grant (the merges of
/// this repository's improvement protocols) can never pass for a choice, and opens.
#[test]
fn a_built_in_kind_a_foreign_read_action_may_produce_is_refused_at_open() {
    let shadow = protocol_with(
        "shadow.made",
        "evidence_kinds:\n  change_record:\n    description: shadow\n\
         claims:\n  change.made:\n    description: made\n    true_when:\n      evidence:\n        kind: change_record\n        subject: implementation\n\
         actions:\n  implementation.edit:\n    description: change the code\n    effect: write\n\
         \x20 change.ack:\n    description: ack\n    effect: read\n    may_produce:\n      - evidence: change_record\n\
         outcomes:\n  abandoned:\n    description: given up\n    requires:\n      decision: explicitly_abandoned\n",
    );
    let (error, written) = open_on("shadow", &shadow, &[]);
    let error = error.expect("the protocol is refused");
    assert!(
        error.contains("change.ack") && error.contains("change_record"),
        "{error}"
    );
    assert!(!written);

    for (name, inputs) in [
        ("self-improvement", &[("surface", "judge")][..]),
        ("loop-improvement", &[][..]),
    ] {
        let yaml = canon_engineering::registry::get(name, 1)
            .expect("the protocol")
            .yaml;
        let (error, written) = open_on(name, yaml, inputs);
        assert_eq!(error, None, "{name}");
        assert!(written, "{name}");
    }
}

/// F2: the same content landed on the base branch by another commit after the case's edit.
/// `change.read` compares the revision's tree with the base pinned at open and with the base
/// branch's tip now, outside `.engineering/`: it reads `ahead` while the base lacks the edit and
/// `none` once its tip has the same content, so no pull request is opened for nothing.
#[test]
fn change_read_reads_none_once_the_base_tip_has_the_same_content() {
    let tree = scratch("base-tip");
    let memory = Rc::new(Memory::default());
    let forge: Rc<dyn Forge> = memory;
    let validators: Vec<Box<dyn Validator>> = vec![Box::new(PrOpen::new(forge))];
    let options = RunOptions {
        protocol: PROTOCOL.to_owned(),
        intent: "deliver".to_owned(),
        inputs: Vec::new(),
        iterations: Some(1),
        hours: None,
        tokens: None,
        case_id: Some("base-tip".to_owned()),
        gate: None,
    };
    let harness = Scripted::new(&[]);
    let mut run = Run::open(&tree, options, &harness, &validators).expect("the case opens");
    fs::write(tree.join("README.md"), "a story\nthe fix\n").expect("an edit");
    git(&tree, &["commit", "-q", "-am", "the fix"]);
    let revision = git(&tree, &["rev-parse", "HEAD"]);
    run.case
        .set_revision("implementation", &revision)
        .expect("the revision moves");
    let first = ChangeRead.observe(&tree, &revision, 1).expect("observes");
    assert_eq!(
        first.records[0].result.as_deref(),
        Some("ahead"),
        "{}",
        first.summary
    );
    let other = tree.with_extension("other");
    let _ = fs::remove_dir_all(&other);
    git(
        &tree,
        &[
            "worktree",
            "add",
            "-q",
            other.to_str().expect("utf-8"),
            "main",
        ],
    );
    fs::write(other.join("README.md"), "a story\nthe fix\n").expect("the same edit");
    git(
        &other,
        &["commit", "-q", "-am", "the same fix by another commit"],
    );
    git(
        &tree,
        &[
            "worktree",
            "remove",
            "--force",
            other.to_str().expect("utf-8"),
        ],
    );
    let second = ChangeRead.observe(&tree, &revision, 2).expect("observes");
    assert_eq!(
        second.records[0].result.as_deref(),
        Some("none"),
        "{}",
        second.summary
    );
    assert!(second.summary.contains("its tip now"), "{}", second.summary);
}

/// A case `name` opened on `main` with `pr.open` over `memory`, one edit of `README.md` to
/// `text` committed on its branch and made its implementation revision; the tree and the
/// revision.
fn edited(name: &str, memory: &Rc<Memory>, text: &str) -> (Scratch, String) {
    let tree = scratch(name);
    let forge: Rc<dyn Forge> = memory.clone();
    let validators: Vec<Box<dyn Validator>> = vec![Box::new(PrOpen::new(forge))];
    let options = RunOptions {
        protocol: PROTOCOL.to_owned(),
        intent: "deliver".to_owned(),
        inputs: Vec::new(),
        iterations: Some(1),
        hours: None,
        tokens: None,
        case_id: Some(name.to_owned()),
        gate: None,
    };
    let harness = Scripted::new(&[]);
    let mut run = Run::open(&tree, options, &harness, &validators).expect("the case opens");
    fs::write(tree.join("README.md"), text).expect("an edit");
    git(&tree, &["commit", "-q", "-am", "the fix"]);
    let revision = git(&tree, &["rev-parse", "HEAD"]);
    run.case
        .set_revision("implementation", &revision)
        .expect("the revision moves");
    drop(run);
    (tree, revision)
}

/// `pr.open` counts the commits beyond the base branch, `refs/heads/<base>`: a tag of the same
/// name, which git would resolve first for the bare name, is never taken for it. A tag `main`
/// at the revision does not make the case's branch look as if it had nothing beyond its base.
#[test]
fn pr_open_counts_commits_beyond_the_base_branch_not_a_tag_of_its_name() {
    let memory = Rc::new(Memory::default());
    let (tree, revision) = edited("tag-named-main", &memory, "a story\nthe fix\n");
    git(&tree, &["tag", "main", &revision]);
    let observed = PrOpen::new(memory.clone())
        .observe(&tree, &revision, 1)
        .expect("observes");
    assert_eq!(
        memory.opened.borrow().len(),
        1,
        "no pull request opened: {}",
        observed.summary
    );
    assert_eq!(observed.records.len(), 1, "{}", observed.summary);
}

/// With the base branch deleted after the case opened, `change.read` and `pr.open` observe
/// without an error, each as its other missing-base path does: `change.read` compares with the
/// base pinned at open, and `pr.open` pushes nothing, records nothing and says the base branch
/// is not there, the claim staying unknown.
#[test]
fn with_the_base_branch_gone_change_read_and_pr_open_observe_without_an_error() {
    let memory = Rc::new(Memory::default());
    let (tree, revision) = edited("base-gone", &memory, "a story\nthe fix\n");
    git(&tree, &["branch", "-q", "-D", "main"]);
    let read = ChangeRead
        .observe(&tree, &revision, 1)
        .map_err(|error| format!("{error:#}"))
        .expect("change.read observes");
    assert_eq!(
        read.records[0].result.as_deref(),
        Some("ahead"),
        "{}",
        read.summary
    );
    assert!(read.summary.contains("not there"), "{}", read.summary);
    let opened = PrOpen::new(memory.clone())
        .observe(&tree, &revision, 2)
        .map_err(|error| format!("{error:#}"))
        .expect("pr.open observes");
    assert!(opened.records.is_empty(), "{}", opened.summary);
    assert!(
        opened.summary.contains("main is not there"),
        "{}",
        opened.summary
    );
    assert!(memory.pushed.borrow().is_empty(), "{}", opened.summary);
    assert!(memory.opened.borrow().is_empty(), "{}", opened.summary);
}

/// A revision whose merge into the base branch's tip conflicts is `ahead`: there is a change to
/// publish. The summary says the merge conflicts.
#[test]
fn change_read_reads_ahead_and_says_so_when_the_merge_into_the_base_conflicts() {
    let memory = Rc::new(Memory::default());
    let (tree, revision) = edited("conflict", &memory, "a story\nmine\n");
    let other = tree.with_extension("other");
    let _ = fs::remove_dir_all(&other);
    git(
        &tree,
        &[
            "worktree",
            "add",
            "-q",
            other.to_str().expect("utf-8"),
            "main",
        ],
    );
    fs::write(other.join("README.md"), "a story\ntheirs\n").expect("a conflicting edit");
    git(&other, &["commit", "-q", "-am", "a conflicting change"]);
    git(
        &tree,
        &[
            "worktree",
            "remove",
            "--force",
            other.to_str().expect("utf-8"),
        ],
    );
    let read = ChangeRead.observe(&tree, &revision, 1).expect("observes");
    assert_eq!(
        read.records[0].result.as_deref(),
        Some("ahead"),
        "{}",
        read.summary
    );
    assert!(read.summary.contains("conflicts"), "{}", read.summary);
}

/// A merge that conflicts only inside `.engineering/`, here a planning note both sides change,
/// is no change to publish outside it: `none`, and the summary names the conflicted path.
#[test]
fn change_read_reads_none_and_names_a_conflict_only_inside_engineering() {
    let tree = scratch("conflict-inside");
    fs::create_dir_all(tree.join(".engineering")).expect("a directory");
    fs::write(tree.join(".engineering/notes.md"), "a note\n").expect("a note");
    git(&tree, &["add", "--", ".engineering/notes.md"]);
    git(&tree, &["commit", "-q", "-m", "a note"]);
    let memory = Rc::new(Memory::default());
    let forge: Rc<dyn Forge> = memory;
    let validators: Vec<Box<dyn Validator>> = vec![Box::new(PrOpen::new(forge))];
    let options = RunOptions {
        protocol: PROTOCOL.to_owned(),
        intent: "deliver".to_owned(),
        inputs: Vec::new(),
        iterations: Some(1),
        hours: None,
        tokens: None,
        case_id: Some("conflict-inside".to_owned()),
        gate: None,
    };
    let harness = Scripted::new(&[]);
    let mut run = Run::open(&tree, options, &harness, &validators).expect("the case opens");
    fs::write(tree.join(".engineering/notes.md"), "a note, mine\n").expect("an edit");
    git(
        &tree,
        &["commit", "-q", "-m", "mine", "--", ".engineering/notes.md"],
    );
    let revision = git(&tree, &["rev-parse", "HEAD"]);
    run.case
        .set_revision("implementation", &revision)
        .expect("the revision moves");
    drop(run);
    let other = tree.with_extension("other");
    let _ = fs::remove_dir_all(&other);
    git(
        &tree,
        &[
            "worktree",
            "add",
            "-q",
            other.to_str().expect("utf-8"),
            "main",
        ],
    );
    fs::write(other.join(".engineering/notes.md"), "a note, theirs\n").expect("an edit");
    git(&other, &["commit", "-q", "-am", "theirs"]);
    git(
        &tree,
        &[
            "worktree",
            "remove",
            "--force",
            other.to_str().expect("utf-8"),
        ],
    );
    let read = ChangeRead.observe(&tree, &revision, 1).expect("observes");
    assert_eq!(
        read.records[0].result.as_deref(),
        Some("none"),
        "{}",
        read.summary
    );
    assert!(
        read.summary.contains("conflicts") && read.summary.contains(".engineering/notes.md"),
        "{}",
        read.summary
    );
}

/// `tests.run` and `ci.read` compare what the case's branch changed with its base on the judge
/// surface, reading the paths git lists byte for byte. A path that is not UTF-8 is never
/// decoded to a stand-in, which could read as off the surface: the observation is an error
/// naming it, its bytes escaped, and the test command is not run.
#[cfg(unix)]
#[test]
fn tests_run_refuses_a_path_that_is_not_utf8_on_the_case_branch_naming_it() {
    let memory = Rc::new(Memory::default());
    let (tree, _) = edited("not-utf8", &memory, "a story\nthe fix\n");
    let ran = tree.join("ran.txt");
    stage(&tree, b"r\xff.txt", b"a file\n");
    git(&tree, &["commit", "-q", "-m", "a path that is not utf-8"]);
    let revision = git(&tree, &["rev-parse", "HEAD"]);
    let command = format!("touch {}", ran.display());
    let observed = TestsRun::new(&command)
        .expect("a command")
        .observe(&tree, &revision, 1);
    let error = match observed {
        Ok(observation) => panic!(
            "observed with the path decoded: {} ({:?})",
            observation.summary,
            observation
                .records
                .iter()
                .map(|record| record.result.clone())
                .collect::<Vec<_>>()
        ),
        Err(error) => format!("{error:#}"),
    };
    assert!(error.contains("r\\xff.txt"), "{error}");
    assert!(!ran.exists(), "the test command ran: {error}");
}
