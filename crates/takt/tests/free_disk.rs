//! story:runs-check-free-disk: before every step a run offers or runs, a run that was given a
//! threshold ([`Run::stop_when_disk_below`]) reads the free space of the file system holding the
//! case's tree and of the one holding the build directory, each file system once, and stops
//! `DiskLow` naming the path, its free space and the threshold when one is under it: no harness
//! call, no step. At or above the threshold nothing changes; a threshold of 0 reads nothing; a
//! reader that fails stops nothing and is said once. The free-space reader is injected
//! ([`Run::read_free_space_with`]); no test fills a disk.

use std::cell::{Cell, RefCell};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::rc::Rc;

use takt::harness::{Harness, StepContext, StepReport};
use takt::run::{FreeSpace, free_space};
use takt::scorecard::{HeldOut, Row};
use takt::validator::{ChangeRead, TestsRun, Validator};
use takt::{Run, RunOptions, Stop};

const PROTOCOL: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../protocols/story-delivery/1.yaml"
);

const GIB: u64 = 1 << 30;

fn git(tree: &Path, args: &[&str]) {
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

/// A committed scratch repository with a manifest.
fn scratch(name: &str) -> Scratch {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("free-disk-{}-{name}", std::process::id()));
    if dir.exists() {
        fs::remove_dir_all(&dir).expect("a stale scratch tree is removed");
    }
    fs::create_dir_all(&dir).expect("scratch dir");
    fs::write(dir.join("Cargo.toml"), "[package]\nname = \"scratch\"\n").expect("a file");
    git(&dir, &["init", "-q", "-b", "main"]);
    git(&dir, &["add", "-A"]);
    git(&dir, &["commit", "-q", "-m", "start"]);
    Scratch(dir)
}

/// Counts its calls and reports an edit that changes nothing.
#[derive(Default)]
struct Counting {
    calls: Cell<usize>,
}

impl Harness for Counting {
    fn name(&self) -> &str {
        "counting"
    }

    fn step(&self, _context: &StepContext<'_>) -> anyhow::Result<StepReport> {
        self.calls.set(self.calls.get() + 1);
        Ok(StepReport {
            action: "implementation.edit".to_owned(),
            summary: "the double".to_owned(),
            tokens: Some(10),
        })
    }
}

fn options(name: &str) -> RunOptions {
    RunOptions {
        protocol: PROTOCOL.to_owned(),
        intent: "deliver the story".to_owned(),
        inputs: Vec::new(),
        iterations: Some(4),
        hours: None,
        tokens: None,
        case_id: Some(name.to_owned()),
        gate: Some("repository.merge".to_owned()),
    }
}

/// The tests (the command `true`) and the change read: the first step is a deterministic one.
fn validators() -> Vec<Box<dyn Validator>> {
    vec![
        Box::new(TestsRun::new("true").expect("a command")),
        Box::new(ChangeRead),
    ]
}

/// What a run printed and recorded, with its stop and the harness's calls.
struct Ran {
    stop: Stop,
    steps: u64,
    out: String,
    events: String,
    calls: usize,
}

/// Opens the case `name` in `tree` and runs it; `prepare` configures the run before it runs.
fn run_with(tree: &Path, name: &str, prepare: impl FnOnce(&mut Run<'_>)) -> Ran {
    let harness = Counting::default();
    let validators = validators();
    let mut run = Run::open(tree, options(name), &harness, &validators).expect("the case opens");
    prepare(&mut run);
    let mut out = Vec::new();
    let stop = run.run(&mut out).expect("the run stops without an error");
    let events = fs::read_to_string(
        tree.join(".engineering/cases")
            .join(name)
            .join("events.jsonl"),
    )
    .expect("the record");
    Ran {
        stop,
        steps: run.steps,
        out: String::from_utf8(out).expect("utf-8"),
        events,
        calls: harness.calls.get(),
    }
}

/// The acceptance's first bullet: free space under the threshold stops the run before its first
/// step, naming the path checked, its free space and the threshold; no harness call, no step,
/// and the stop line records no completion.
#[test]
fn below_the_threshold_the_run_stops_before_any_step_naming_the_path_and_its_free_space() {
    let tree = scratch("below");
    let seen = Rc::new(RefCell::new(Vec::new()));
    let reads = seen.clone();
    let ran = run_with(&tree, "below", |run| {
        run.stop_when_disk_below(10 * GIB, None);
        run.read_free_space_with(move |path| {
            reads.borrow_mut().push(path.to_path_buf());
            Ok(FreeSpace {
                device: 1,
                bytes: 64 << 20,
            })
        });
    });
    assert_eq!(
        ran.stop,
        Stop::DiskLow {
            path: tree.to_path_buf(),
            free_bytes: 64 << 20,
            min_bytes: 10 * GIB,
        },
        "{}",
        ran.out
    );
    assert_eq!(
        ran.stop.to_string(),
        format!(
            "DiskLow ({}: 67108864 bytes (0.06 GiB) free, under 10737418240 bytes (10.00 GiB))",
            tree.display()
        )
    );
    assert_eq!(ran.calls, 0, "a harness call: {}", ran.out);
    assert_eq!(ran.steps, 0, "{}", ran.out);
    assert!(!ran.out.contains("step "), "{}", ran.out);
    assert!(!ran.events.contains("\"event\":\"step\""), "{}", ran.events);
    let stop = ran
        .events
        .lines()
        .rfind(|line| line.contains("\"event\":\"stop\""))
        .expect("a stop line");
    assert!(stop.contains("\"stop\":\"DiskLow ("), "{stop}");
    assert!(stop.contains("\"completed\":false"), "{stop}");
    assert_eq!(*seen.borrow(), vec![tree.to_path_buf()]);
}

/// The check runs before every step, not once: free space that falls under the threshold after
/// the first step stops the run before the second, the harness never called.
#[test]
fn free_space_that_falls_after_a_step_stops_the_run_before_the_next() {
    let tree = scratch("falls");
    let reads = Rc::new(Cell::new(0_u32));
    let counted = reads.clone();
    let ran = run_with(&tree, "falls", |run| {
        run.stop_when_disk_below(10 * GIB, None);
        run.read_free_space_with(move |_| {
            counted.set(counted.get() + 1);
            Ok(FreeSpace {
                device: 1,
                bytes: if counted.get() == 1 { 20 * GIB } else { GIB },
            })
        });
    });
    assert!(ran.out.contains("step 1: tests.run {}"), "{}", ran.out);
    assert!(!ran.out.contains("step 2:"), "{}", ran.out);
    assert_eq!(ran.steps, 1, "{}", ran.out);
    assert_eq!(ran.calls, 0, "{}", ran.out);
    assert!(
        matches!(ran.stop, Stop::DiskLow { free_bytes, .. } if free_bytes == GIB),
        "{}",
        ran.stop
    );
    assert_eq!(reads.get(), 2);
}

/// The acceptance's second bullet: at or above the threshold nothing changes; the run is the run
/// without the check, the same stop, steps and harness calls, though the reader was read.
#[test]
fn at_or_above_the_threshold_nothing_changes() {
    let unchecked_tree = scratch("unchecked");
    let unchecked = run_with(&unchecked_tree, "same", |_| {});
    let checked_tree = scratch("at");
    let reads = Rc::new(Cell::new(0_u32));
    let counted = reads.clone();
    let checked = run_with(&checked_tree, "same", |run| {
        run.stop_when_disk_below(10 * GIB, None);
        run.read_free_space_with(move |_| {
            counted.set(counted.get() + 1);
            Ok(FreeSpace {
                device: 1,
                bytes: 10 * GIB,
            })
        });
    });
    assert_eq!(checked.stop, unchecked.stop, "{}", checked.out);
    assert_eq!(checked.steps, unchecked.steps, "{}", checked.out);
    assert_eq!(checked.calls, unchecked.calls, "{}", checked.out);
    assert!(checked.calls >= 1, "{}", checked.out);
    assert_eq!(
        checked
            .out
            .replace(&*checked_tree.to_string_lossy(), "<tree>"),
        unchecked
            .out
            .replace(&*unchecked_tree.to_string_lossy(), "<tree>")
    );
    assert!(reads.get() >= 2, "read before each step: {}", reads.get());
}

/// A threshold of 0 turns the check off: nothing is read, nothing stops on it.
#[test]
fn a_threshold_of_zero_reads_nothing() {
    let tree = scratch("zero");
    let reads = Rc::new(Cell::new(0_u32));
    let counted = reads.clone();
    let ran = run_with(&tree, "zero", |run| {
        run.stop_when_disk_below(0, None);
        run.read_free_space_with(move |_| {
            counted.set(counted.get() + 1);
            Ok(FreeSpace {
                device: 1,
                bytes: 0,
            })
        });
    });
    assert_eq!(reads.get(), 0);
    assert!(ran.calls >= 1, "{}", ran.out);
    assert!(!ran.stop.to_string().starts_with("DiskLow"), "{}", ran.out);
}

/// Without a threshold, as a library caller that sets none, nothing is read either.
#[test]
fn without_a_threshold_nothing_is_read() {
    let tree = scratch("unset");
    let reads = Rc::new(Cell::new(0_u32));
    let counted = reads.clone();
    let ran = run_with(&tree, "unset", |run| {
        run.read_free_space_with(move |_| {
            counted.set(counted.get() + 1);
            Ok(FreeSpace {
                device: 1,
                bytes: 0,
            })
        });
    });
    assert_eq!(reads.get(), 0);
    assert!(ran.calls >= 1, "{}", ran.out);
}

/// Two file systems, the tree's with room and the build directory's under the threshold: the
/// run stops naming the build directory.
#[test]
fn two_file_systems_the_second_low_stops_naming_it() {
    let tree = scratch("two");
    let tree_path = tree.to_path_buf();
    let ran = run_with(&tree, "two", |run| {
        run.stop_when_disk_below(10 * GIB, Some(PathBuf::from("/srv/build")));
        run.read_free_space_with(move |path| {
            Ok(if path == tree_path {
                FreeSpace {
                    device: 1,
                    bytes: 500 * GIB,
                }
            } else {
                FreeSpace {
                    device: 2,
                    bytes: 3 * GIB,
                }
            })
        });
    });
    assert_eq!(
        ran.stop,
        Stop::DiskLow {
            path: PathBuf::from("/srv/build"),
            free_bytes: 3 * GIB,
            min_bytes: 10 * GIB,
        },
        "{}",
        ran.out
    );
    assert_eq!(ran.calls, 0, "{}", ran.out);
    assert_eq!(ran.steps, 0, "{}", ran.out);
}

/// Each file system once: a build directory on the tree's file system is not compared again, so
/// what its read says does not decide the stop; a relative one is the tree's.
#[test]
fn a_build_directory_on_the_trees_file_system_is_checked_once() {
    let tree = scratch("same-fs");
    let tree_path = tree.to_path_buf();
    let seen = Rc::new(RefCell::new(Vec::new()));
    let reads = seen.clone();
    let ran = run_with(&tree, "same-fs", |run| {
        run.stop_when_disk_below(10 * GIB, Some(PathBuf::from("target")));
        run.read_free_space_with(move |path| {
            reads.borrow_mut().push(path.to_path_buf());
            Ok(FreeSpace {
                device: 7,
                bytes: if path == tree_path { 500 * GIB } else { GIB },
            })
        });
    });
    assert!(ran.calls >= 1, "{}", ran.out);
    assert!(!ran.stop.to_string().starts_with("DiskLow"), "{}", ran.out);
    assert!(
        seen.borrow().contains(&tree.join("target")),
        "{:?}",
        seen.borrow()
    );
}

/// A reader that fails stops nothing: the run goes on as without the check, and the error is
/// printed once, however many steps read it.
#[test]
fn a_reader_that_fails_stops_nothing_and_is_said_once() {
    let tree = scratch("unread");
    let reads = Rc::new(Cell::new(0_u32));
    let counted = reads.clone();
    let ran = run_with(&tree, "unread", |run| {
        run.stop_when_disk_below(10 * GIB, None);
        run.read_free_space_with(move |_| {
            counted.set(counted.get() + 1);
            Err(std::io::Error::other("no statvfs here"))
        });
    });
    assert!(ran.calls >= 1, "{}", ran.out);
    assert!(reads.get() >= 2, "{}", reads.get());
    assert!(!ran.stop.to_string().starts_with("DiskLow"), "{}", ran.out);
    assert_eq!(ran.out.matches("no statvfs here").count(), 1, "{}", ran.out);
}

/// The reader a run uses unless given another: the free space and the device of the file
/// system holding a path, a path that does not exist yet read on its nearest existing ancestor.
#[test]
fn the_default_reader_reads_the_file_system_holding_the_path() {
    use std::os::unix::fs::MetadataExt as _;
    let tree = scratch("statvfs");
    let free = free_space(&tree).expect("statvfs reads");
    assert!(free.bytes > 0);
    assert_eq!(free.device, fs::metadata(&*tree).expect("metadata").dev());
    let later = free_space(&tree.join("target/debug")).expect("an ancestor reads");
    assert_eq!(later.device, free.device);
}

/// A gates document whose one assertion is true in a repository with a manifest.
const GATES: &str = "format: engineering-gates/1\nlanguage: canon-expr/1\ngates:\n  - id: manifest\n    \
                     text: The repository has its manifest.\n    assertions:\n      \
                     - 'fs.file.exists(\"Cargo.toml\")'\n";

/// A committed scratch repository with a manifest and a gates document.
fn gated(name: &str) -> Scratch {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("free-disk-{}-{name}", std::process::id()));
    if dir.exists() {
        fs::remove_dir_all(&dir).expect("a stale scratch tree is removed");
    }
    fs::create_dir_all(dir.join(".engineering")).expect("scratch dir");
    fs::write(dir.join("Cargo.toml"), "[package]\nname = \"scratch\"\n").expect("a file");
    fs::write(dir.join(".engineering/gates.yaml"), GATES).expect("a file");
    git(&dir, &["init", "-q", "-b", "main"]);
    git(&dir, &["add", "-A"]);
    git(&dir, &["commit", "-q", "-m", "start"]);
    Scratch(dir)
}

/// Edits `work.txt` once, then hands the change over when offered, and requests the gate when
/// one is pending; counts its calls.
#[derive(Default)]
struct Editing {
    calls: Cell<usize>,
}

impl Harness for Editing {
    fn name(&self) -> &str {
        "editing"
    }

    fn step(&self, context: &StepContext<'_>) -> anyhow::Result<StepReport> {
        self.calls.set(self.calls.get() + 1);
        if let Some(gate) = context.approvals.first() {
            return Ok(StepReport {
                action: gate.clone(),
                summary: "approval".to_owned(),
                tokens: Some(1),
            });
        }
        let work = context.tree.join("work.txt");
        if work.exists() && context.offered.iter().any(|a| a == "change.handover") {
            return Ok(StepReport {
                action: "change.handover".to_owned(),
                summary: "handover".to_owned(),
                tokens: Some(1),
            });
        }
        fs::write(&work, format!("edit {}\n", self.calls.get())).expect("an edit");
        Ok(StepReport {
            action: "implementation.edit".to_owned(),
            summary: "edit".to_owned(),
            tokens: Some(1),
        })
    }
}

/// The delivery's validators with its gates document, and the forge's three when given one.
fn delivery_validators(forge: Option<Rc<dyn takt::Forge>>) -> Vec<Box<dyn Validator>> {
    let mut validators: Vec<Box<dyn Validator>> = vec![
        Box::new(TestsRun::new("true").expect("a command")),
        Box::new(ChangeRead),
        Box::new(takt::validator::GateRun::new(".engineering/gates.yaml")),
    ];
    if let Some(forge) = forge {
        validators.push(Box::new(takt::validator::PrOpen::new(forge.clone())));
        validators.push(Box::new(takt::validator::CiRead::new(forge.clone())));
        validators.push(Box::new(takt::validator::PrMerged::new(forge)));
    }
    validators
}

/// Opens the case `name` with no approval gate given and runs it to its first stop, as the CLI
/// does (`stop_when_unperformable`), the disk not checked.
fn first_run(tree: &Path, name: &str, validators: &[Box<dyn Validator>], harness: &dyn Harness) {
    let mut options = options(name);
    options.iterations = Some(12);
    options.gate = None;
    let mut run = Run::open(tree, options, harness, validators).expect("the case opens");
    run.stop_when_unperformable();
    run.run(&mut Vec::new())
        .expect("the run stops without an error");
}

/// Resumes the case `name` as the CLI does, with the disk under the threshold when `low`: its
/// stop as printed and its output.
fn resumed(
    tree: &Path,
    name: &str,
    validators: &[Box<dyn Validator>],
    harness: &dyn Harness,
    low: bool,
) -> (String, String) {
    let mut options = options(name);
    options.iterations = Some(12);
    options.gate = None;
    let mut run = Run::resume(tree, options, name, harness, validators).expect("the case resumes");
    run.stop_when_unperformable();
    if low {
        run.stop_when_disk_below(10 * GIB, None);
        run.read_free_space_with(|_| {
            Ok(FreeSpace {
                device: 1,
                bytes: GIB,
            })
        });
    }
    let mut out = Vec::new();
    let stop = run.run(&mut out).expect("the run stops without an error");
    (stop.to_string(), String::from_utf8(out).expect("utf-8"))
}

/// A case only a grant can move stops for the grant on a low disk too: no step would follow,
/// so free space is not what stops it (exit 4, not 8), and the harness is not called.
#[test]
fn a_case_only_a_grant_moves_stops_approval_required_on_a_low_disk() {
    let tree = gated("grant");
    let forge: Rc<dyn takt::Forge> = Rc::new(takt::forge::Memory {
        checks: takt::forge::Checks::Pass,
        ..takt::forge::Memory::default()
    });
    let validators = delivery_validators(Some(forge));
    let harness = Editing::default();
    first_run(&tree, "grant", &validators, &harness);
    let (fine, text) = resumed(&tree, "grant", &validators, &harness, false);
    assert!(
        fine.starts_with("ApprovalRequired ("),
        "the premise: {fine}\n{text}"
    );
    let calls = harness.calls.get();
    let (low, text) = resumed(&tree, "grant", &validators, &harness, true);
    assert_eq!(low, fine, "{text}");
    assert_eq!(harness.calls.get(), calls, "{text}");
}

/// A `--forge none` case at "ahead" stops `Unperformable` on a low disk too: no step would
/// follow (exit 7, not 8).
#[test]
fn a_forge_none_case_at_ahead_stops_unperformable_on_a_low_disk() {
    let tree = gated("ahead");
    let validators = delivery_validators(None);
    let harness = Editing::default();
    first_run(&tree, "ahead", &validators, &harness);
    let (fine, text) = resumed(&tree, "ahead", &validators, &harness, false);
    assert_eq!(
        fine, "Unperformable (pr.opened; nothing performs it without the forge)",
        "the premise: {text}"
    );
    let (low, text) = resumed(&tree, "ahead", &validators, &harness, true);
    assert_eq!(low, fine, "{text}");
}

/// Writes the scenario trial `case` under `repo`: one run per stop line, each with whether it
/// is a completion, and the held-out check's first line.
fn trial(repo: &Path, case: &str, stops: &[(&str, bool)], held: &str) {
    let dir = repo.join(".engineering/cases").join(case);
    fs::create_dir_all(&dir).expect("a case directory");
    let mut text = String::new();
    for (n, (stop, completed)) in stops.iter().enumerate() {
        let at = format!("2026-10-19T10:0{n}:00.000000Z");
        text.push_str(&format!(
            "{{\"at\":\"{at}\",\"step\":0,\"event\":\"opened\",\"case\":\"{case}\",\"protocol\":\"story.delivery/1\",\"inputs\":[]}}\n"
        ));
        text.push_str(&format!(
            "{{\"at\":\"{at}\",\"step\":1,\"event\":\"stop\",\"stop\":\"{stop}\",\"steps\":1,\"tokens\":10,\"completed\":{completed}}}\n"
        ));
    }
    fs::write(dir.join("events.jsonl"), text).expect("the record");
    fs::write(dir.join("held-out.txt"), format!("{held}\n")).expect("the held-out result");
}

/// A stop line a `DiskLow` run writes.
const DISK_LOW: &str =
    "DiskLow (/srv/tree: 1073741824 bytes (1.00 GiB) free, under 10737418240 bytes (10.00 GiB))";

/// Scenario `s` with two trials: the first accepted, the second accepted unless `low`, which
/// stopped `DiskLow` with its held-out check failed.
fn card(name: &str, low: bool) -> (Scratch, takt::scorecard::Scorecard) {
    let repo = scratch(name);
    trial(
        &repo,
        "scenario-s-trial-0",
        &[("Completed (accepted)", true)],
        "PASS",
    );
    if low {
        trial(
            &repo,
            "scenario-s-trial-1",
            &[(DISK_LOW, false)],
            "FAIL: no pull request",
        );
    } else {
        trial(
            &repo,
            "scenario-s-trial-1",
            &[("Completed (accepted)", true)],
            "PASS",
        );
    }
    let mut card = takt::scorecard::Scorecard::read(&repo).expect("the scorecard reads");
    card.rule = takt::scorecard::Rule {
        trials: 2,
        margin: 0,
    };
    (repo, card)
}

/// `candidate` against `saved`: `worse <n>`, or `no verdict: <the error>`, and the printed text.
fn verdict(
    candidate: &takt::scorecard::Scorecard,
    saved: &takt::scorecard::Scorecard,
) -> (String, String) {
    let mut out = Vec::new();
    let verdict = match candidate.against(saved, &mut out) {
        Ok(worse) => format!("worse {worse}"),
        Err(error) => format!("no verdict: {error:#}"),
    };
    (verdict, String::from_utf8(out).expect("utf-8"))
}

/// The planner's ruling: a `DiskLow` trial on the candidate's side gives its scenario no verdict,
/// whatever the count, naming the trial; it is no worse rank and no case dropped either.
#[test]
fn a_disk_low_trial_on_the_candidate_side_gives_no_verdict_naming_it() {
    let (_saved_repo, saved) = card("saved-fine", false);
    let (_repo, candidate) = card("candidate-low", true);
    let low = candidate
        .cases()
        .into_iter()
        .find(|case| case.case == "scenario-s-trial-1")
        .expect("the trial is listed");
    assert_eq!(low.scenario.as_deref(), Some("s"), "{low:?}");
    let (verdict, text) = verdict(&candidate, &saved);
    assert!(
        text.contains(
            "scenario s  no verdict: DiskLow trials (the candidate's scenario-s-trial-1)"
        ),
        "{text}"
    );
    assert!(verdict.starts_with("no verdict: "), "{verdict}");
    assert!(
        verdict.contains("scenario s (the candidate's scenario-s-trial-1)")
            && verdict.contains("not topped up"),
        "{verdict}"
    );
}

/// The same on the saved side: a baseline with a `DiskLow` trial judges nothing in that
/// scenario.
#[test]
fn a_disk_low_trial_on_the_saved_side_gives_no_verdict_naming_it() {
    let (_saved_repo, saved) = card("saved-low", true);
    let (_repo, candidate) = card("candidate-fine", false);
    let (verdict, text) = verdict(&candidate, &saved);
    assert!(
        text.contains(
            "scenario s  no verdict: DiskLow trials (the saved side's scenario-s-trial-1)"
        ),
        "{text}"
    );
    assert!(verdict.starts_with("no verdict: "), "{verdict}");
}

/// A run of scenario `s`'s trial `n` that stopped `stop`, its held-out check passed or not.
fn trial_row(n: usize, stop: &str, passed: bool) -> Row {
    Row {
        case: format!("scenario-s-trial-{n}"),
        scenario: Some("s".to_owned()),
        outcome: if passed {
            stop.to_owned()
        } else {
            takt::scorecard::FAILED.to_owned()
        },
        stop: stop.to_owned(),
        held_out: Some(HeldOut {
            passed,
            result: if passed { "PASS" } else { "FAIL: x" }.to_owned(),
        }),
        steps: 1,
        tokens: 100,
        refusals: 0,
        no_progress: 0,
        errors: 0,
        wall_ms: 0,
        saturated: false,
        binary: None,
        opened_binary: None,
        completed: Some(stop == "accepted"),
        step_tokens: Default::default(),
    }
}

/// A scorecard of `rows` under a rule of three trials and a margin of 0.
fn rows_card(rows: Vec<Row>) -> takt::scorecard::Scorecard {
    takt::scorecard::Scorecard {
        format: takt::scorecard::FORMAT.to_owned(),
        revision: None,
        named_by: None,
        metrics: takt::scorecard::METRICS
            .iter()
            .map(|(metric, version)| ((*metric).to_owned(), *version))
            .collect(),
        rule: takt::scorecard::Rule {
            trials: 3,
            margin: 0,
        },
        rows,
        no_record: Vec::new(),
        unstated: std::collections::BTreeMap::new(),
    }
}

/// Five accepted trials, the baseline every case below is judged against.
fn five_accepted() -> takt::scorecard::Scorecard {
    rows_card((0..5).map(|n| trial_row(n, "accepted", true)).collect())
}

/// A case any run of which stopped `DiskLow` is a `DiskLow` trial, though a later run completed
/// it: the case's runs, not its last, say whether the machine ended one of them.
#[test]
fn a_trial_with_any_disk_low_run_is_a_disk_low_trial_though_completed_later() {
    let saved = five_accepted();
    let mut rows: Vec<Row> = (0..3).map(|n| trial_row(n, "accepted", true)).collect();
    rows.push(trial_row(3, "DiskLow", false));
    rows.push(trial_row(3, "accepted", true));
    let (verdict, text) = verdict(&rows_card(rows), &saved);
    assert!(verdict.starts_with("no verdict: "), "{verdict}\n{text}");
    assert!(
        verdict.contains("the candidate's scenario-s-trial-3"),
        "{verdict}"
    );
}

/// Surplus trials: five, two of which failed. Honestly stopped they are worse; stopped
/// `DiskLow`, the scenario gets no verdict naming both, never the verdict of the three good ones.
#[test]
fn surplus_trials_stopping_disk_low_give_no_verdict_rather_than_drop_failures() {
    let saved = five_accepted();
    let honest = rows_card(
        (0..5)
            .map(|n| match n {
                0..3 => trial_row(n, "accepted", true),
                _ => trial_row(n, "NoProgress", false),
            })
            .collect(),
    );
    assert_eq!(verdict(&honest, &saved).0, "worse 1", "the premise");
    let dropped = rows_card(
        (0..5)
            .map(|n| match n {
                0..3 => trial_row(n, "accepted", true),
                _ => trial_row(n, "DiskLow", false),
            })
            .collect(),
    );
    let (verdict, text) = verdict(&dropped, &saved);
    assert!(verdict.starts_with("no verdict: "), "{verdict}\n{text}");
    assert!(
        verdict.contains("the candidate's scenario-s-trial-3, scenario-s-trial-4"),
        "{verdict}"
    );
}

/// Exactly the rule's trials, one stopped `DiskLow`: no verdict; topping the set up with more
/// good trials does not clear it.
#[test]
fn a_disk_low_trial_is_not_cleared_by_topping_up() {
    let saved = five_accepted();
    let mut rows: Vec<Row> = (0..3)
        .map(|n| match n {
            0..2 => trial_row(n, "accepted", true),
            _ => trial_row(n, "DiskLow", false),
        })
        .collect();
    let (exact, text) = verdict(&rows_card(rows.clone()), &saved);
    assert!(exact.starts_with("no verdict: "), "{exact}\n{text}");
    assert!(!exact.contains("too few cases"), "{exact}");
    rows.extend((3..5).map(|n| trial_row(n, "accepted", true)));
    let (topped, text) = verdict(&rows_card(rows), &saved);
    assert!(topped.starts_with("no verdict: "), "{topped}\n{text}");
    assert!(
        topped.contains("the candidate's scenario-s-trial-2"),
        "{topped}"
    );
}

/// A failed trial counted, then resumed into a `DiskLow` stop: the case stays in its scenario
/// and the scenario gets no verdict; the resume does not launder the failure.
#[test]
fn a_resume_that_stops_disk_low_does_not_drop_a_counted_failure() {
    let saved = five_accepted();
    let mut rows: Vec<Row> = (0..3).map(|n| trial_row(n, "accepted", true)).collect();
    rows.push(trial_row(3, "NoProgress", false));
    rows.push(trial_row(3, "DiskLow", false));
    rows.push(trial_row(4, "accepted", true));
    let candidate = rows_card(rows);
    let resumed = candidate
        .cases()
        .into_iter()
        .find(|case| case.case == "scenario-s-trial-3")
        .expect("the trial is listed");
    assert_eq!(resumed.scenario.as_deref(), Some("s"), "{resumed:?}");
    let (verdict, text) = verdict(&candidate, &saved);
    assert!(verdict.starts_with("no verdict: "), "{verdict}\n{text}");
    assert!(
        verdict.contains("the candidate's scenario-s-trial-3"),
        "{verdict}"
    );
}

/// The scorecard counts a `DiskLow` run as no completion.
#[test]
fn a_disk_low_stop_is_no_completion() {
    let row = Row {
        case: "scenario-s-trial-0".to_owned(),
        scenario: Some("s".to_owned()),
        outcome: "failed".to_owned(),
        stop: "DiskLow".to_owned(),
        held_out: Some(HeldOut {
            passed: false,
            result: "fail".to_owned(),
        }),
        steps: 0,
        tokens: 0,
        refusals: 0,
        no_progress: 0,
        errors: 0,
        wall_ms: 0,
        saturated: false,
        binary: None,
        opened_binary: None,
        completed: None,
        step_tokens: Default::default(),
    };
    assert!(!row.disagrees());
}
