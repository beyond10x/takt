//! story:gate-tests-are-deterministic: a gate test passes or fails on the code alone. A program
//! another process writes into a directory the step guard reads, on the caller's `PATH` or in
//! its `~/.cargo/bin`, while a step runs, refuses no step of a test's run
//! (`support/own_home.rs`); a commit a test or the store makes starts no background
//! maintenance that writes `.git/info/refs` into a later step; and the store tests read aep's
//! protocol cache from a named directory, not from wherever the caller's `XDG_CACHE_HOME`
//! points (`support/own_state.rs`).
//!
//! Each guard case runs its run in a child of this test binary whose `PATH` holds a directory
//! of the case's own and whose `HOME` is a home of the case's own, both under the temporary
//! directory, and which inherits neither support module's mark, so its support modules find
//! them as they find the operator's. The harness's one step writes a fresh program into the
//! directory the case names, as another session's install would, and the run must still stop
//! at the merge gate.

#[path = "support/own_home.rs"]
mod own_home;

use std::fs;
use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};
use std::process::Command;

use takt::harness::{Harness, StepContext, StepReport};
use takt::validator::{TestsRun, Validator};
use takt::{Run, RunOptions, Stop};

/// The directory the child's harness writes a program into, set only in the child.
const PROBE: &str = "TAKT_TEST_PROBE_DIR";

/// The child's case, set only in the child.
const CASE: &str = "TAKT_TEST_PROBE_CASE";

/// What `own_home.rs` and `own_state.rs` set once they are in place; a child that inherits
/// them keeps the environment it is given, so a child that must start as the operator's
/// process does is given neither.
const OWN_HOME: &str = "TAKT_TEST_OWN_HOME";
const OWN_STATE: &str = "TAKT_TEST_OWN_STATE";

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
    String::from_utf8_lossy(&output.stdout).trim().to_owned()
}

/// A directory of the test's own, removed when dropped.
struct Scratch(PathBuf);

impl Scratch {
    fn new(base: &Path, name: &str) -> Self {
        let dir = base.join(format!("own-state-{}-{name}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("a scratch directory");
        Self(dir)
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn program(path: &Path) {
    fs::write(path, "#!/bin/sh\nexit 0\n").expect("a program");
    fs::set_permissions(path, fs::Permissions::from_mode(0o755)).expect("its mode");
}

/// A harness that fixes the check on its first step, writing a fresh program into
/// [`PROBE`]'s directory as it does, and requests the gate on its next.
struct WritesOutside;

impl Harness for WritesOutside {
    fn name(&self) -> &str {
        "writes-outside"
    }

    fn step(&self, context: &StepContext<'_>) -> anyhow::Result<StepReport> {
        if context.decision.holds("tests.pass") {
            return Ok(StepReport {
                action: "repository.merge".to_owned(),
                summary: "done".to_owned(),
                tokens: Some(1),
            });
        }
        let probe = PathBuf::from(std::env::var_os(PROBE).expect("the probe directory"));
        program(&probe.join(format!("takt-probe-{}", std::process::id())));
        fs::write(context.tree.join("check.txt"), "fixed\n").expect("the fix");
        Ok(StepReport {
            action: "implementation.edit".to_owned(),
            summary: "fixed the check".to_owned(),
            tokens: Some(1),
        })
    }
}

/// The run in the child: a check that fails until the harness's one edit, then the gate.
#[test]
fn probe_inner() {
    let Some(case) = std::env::var_os(CASE) else {
        return;
    };
    let base = PathBuf::from(env!("CARGO_TARGET_TMPDIR"));
    let scratch = Scratch::new(&base, &format!("tree-{}", case.to_string_lossy()));
    let tree = &scratch.0;
    fs::write(tree.join("check.txt"), "broken\n").expect("a file");
    fs::write(
        tree.join("check.sh"),
        "#!/bin/sh\ngrep -q fixed check.txt\n",
    )
    .expect("a file");
    fs::set_permissions(tree.join("check.sh"), fs::Permissions::from_mode(0o755)).expect("mode");
    git(tree, &["init", "-q", "-b", "main"]);
    git(tree, &["add", "-A"]);
    git(tree, &["commit", "-q", "-m", "broken"]);
    let options = RunOptions {
        protocol: "software.change/1".to_owned(),
        intent: "make the check pass".to_owned(),
        inputs: vec![
            ("risk".to_owned(), "trivial".to_owned()),
            ("change_kind".to_owned(), "feature".to_owned()),
            ("affects_security_boundary".to_owned(), "false".to_owned()),
        ],
        iterations: Some(8),
        hours: None,
        tokens: None,
        case_id: Some("own-state".to_owned()),
        gate: Some("repository.merge".to_owned()),
    };
    let validators: Vec<Box<dyn Validator>> =
        vec![Box::new(TestsRun::new("./check.sh").expect("a command"))];
    let mut run = Run::open(tree, options, &WritesOutside, &validators).expect("the case opens");
    run.continue_past_red_base();
    let mut out = Vec::new();
    let stop = run.run(&mut out).map_err(|error| format!("{error:#}"));
    let text = String::from_utf8_lossy(&out);
    assert_eq!(
        stop,
        Ok(Stop::ApprovalRequired(vec!["repository.merge".to_owned()])),
        "PROBE a program written into {} during the step reached the run\n{text}",
        std::env::var(PROBE).unwrap_or_default()
    );
}

/// Runs [`probe_inner`] in a child whose `PATH` starts with a directory of its own and whose
/// `HOME` holds `~/.cargo/bin`, both holding a program; the harness writes into `probe`, one
/// of them, given relative to the case's root.
fn probe(case: &str, probe: &str) {
    let scratch = Scratch::new(&std::env::temp_dir(), case);
    let root = &scratch.0;
    let home = root.join("home");
    for dir in [root.join("bin"), home.join(".cargo/bin")] {
        fs::create_dir_all(&dir).expect("a directory");
    }
    program(&root.join("bin/tool"));
    program(&home.join(".cargo/bin/cargo"));
    let output = Command::new(std::env::current_exe().expect("this test binary"))
        .args(["--exact", "probe_inner", "--nocapture", "--test-threads=1"])
        .env(
            "PATH",
            format!(
                "{}:/usr/local/bin:/usr/bin:/bin",
                root.join("bin").display()
            ),
        )
        .env("HOME", &home)
        .env_remove("CARGO_HOME")
        .env_remove("RUSTUP_HOME")
        .env_remove("RUSTUP_TOOLCHAIN")
        .env_remove(OWN_HOME)
        .env_remove(OWN_STATE)
        .env(CASE, case)
        .env(PROBE, root.join(probe))
        .output()
        .expect("the child runs");
    let text = String::from_utf8_lossy(&output.stdout) + String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success() && text.contains("1 passed"),
        "{case}: {}",
        text.lines()
            .filter(|line| {
                ["PROBE", "left:", "right:", "panicked"]
                    .iter()
                    .any(|needle| line.contains(needle))
            })
            .collect::<Vec<_>>()
            .join("\n")
    );
    assert!(
        fs::read_dir(root.join(probe))
            .expect("the probe directory")
            .flatten()
            .any(|entry| entry
                .file_name()
                .to_string_lossy()
                .starts_with("takt-probe-")),
        "{case}: the harness wrote no program into {probe}"
    );
}

#[test]
fn a_program_written_on_the_path_during_a_step_refuses_no_step() {
    probe("path", "bin");
}

#[test]
fn a_program_written_in_the_cargo_bin_during_a_step_refuses_no_step() {
    probe("cargo-bin", "home/.cargo/bin");
}

/// The git blob id of `bytes`, as `git hash-object` computes it.
fn blob_id(bytes: &[u8]) -> String {
    use sha2::Digest as _;
    let mut hasher = sha1_checked::Sha1::new();
    hasher.update(format!("blob {}\0", bytes.len()).as_bytes());
    hasher.update(bytes);
    hasher
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

/// A repository of three loose objects, two of them in `objects/17`, the directory git samples
/// to estimate its loose objects: before this unit, a commit there started
/// `git maintenance run --auto --detach`, which repacked it in the background and wrote
/// `.git/info/refs`, the file a step guard refused in `records_reach_base_adv1`.
#[test]
fn a_commit_starts_no_background_maintenance() {
    let scratch = Scratch::new(Path::new(env!("CARGO_TARGET_TMPDIR")), "maintenance");
    let repo = &scratch.0;
    git(repo, &["init", "-q", "-b", "main"]);
    let mut written = 0;
    for n in 0.. {
        let bytes = format!("probe {n}\n");
        if blob_id(bytes.as_bytes()).starts_with("17") {
            let blob = repo.join("blob");
            fs::write(&blob, &bytes).expect("a blob");
            git(repo, &["hash-object", "-w", "blob"]);
            fs::remove_file(&blob).expect("the blob file removed");
            written += 1;
            if written == 2 {
                break;
            }
        }
    }
    assert_eq!(
        fs::read_dir(repo.join(".git/objects/17"))
            .expect("objects/17")
            .count(),
        2
    );
    fs::write(repo.join("f"), "x\n").expect("a file");
    git(repo, &["add", "f"]);
    let trace = repo.with_extension("trace2");
    let output = Command::new("git")
        .args(["-c", "user.name=t", "-c", "user.email=t@localhost"])
        .args(["commit", "-q", "-m", "one"])
        .current_dir(repo)
        .env("GIT_TRACE2", &trace)
        .output()
        .expect("git runs");
    assert!(output.status.success(), "the commit");
    let traced = fs::read_to_string(&trace).unwrap_or_default();
    let _ = fs::remove_file(&trace);
    let started: Vec<&str> = traced
        .lines()
        .filter(|line| line.contains("child_start") && line.contains("maintenance"))
        .collect();
    // What the background maintenance would write, had it started.
    for _ in 0..20 {
        if repo.join(".git/info/refs").exists() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    assert!(
        started.is_empty() && !repo.join(".git/info/refs").exists(),
        "the commit started maintenance {started:?}; .git/info/refs: {}",
        repo.join(".git/info/refs").exists()
    );
}

/// aep's protocol cache is named, so the store tests read the cache the repository's own `aep`
/// fills, whatever `XDG_CACHE_HOME` the caller has: the caller's `AEP_CACHE_DIR`, else
/// `~/.cache/aep`.
#[test]
fn the_aep_cache_is_named_whatever_the_xdg_cache() {
    let output = Command::new(std::env::current_exe().expect("this test binary"))
        .args([
            "--exact",
            "aep_cache_inner",
            "--nocapture",
            "--test-threads=1",
        ])
        .env("XDG_CACHE_HOME", "/nonexistent/xdg-cache")
        .env_remove("AEP_CACHE_DIR")
        .env_remove(OWN_HOME)
        .env_remove(OWN_STATE)
        .env(CASE, "aep-cache")
        .output()
        .expect("the child runs");
    let text = String::from_utf8_lossy(&output.stdout) + String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success() && text.contains("1 passed"),
        "{text}"
    );
}

/// The child of [`the_aep_cache_is_named_whatever_the_xdg_cache`].
#[test]
fn aep_cache_inner() {
    if std::env::var_os(CASE).is_none_or(|case| case != "aep-cache") {
        return;
    }
    let home = PathBuf::from(std::env::var_os("HOME").expect("a HOME"));
    assert_eq!(
        std::env::var_os("AEP_CACHE_DIR").map(PathBuf::from),
        Some(home.join(".cache/aep")),
        "PROBE aep's cache follows XDG_CACHE_HOME"
    );
}
