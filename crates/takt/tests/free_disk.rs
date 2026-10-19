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
    };
    assert!(!row.disagrees());
}
