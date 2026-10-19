//! story:runs-check-free-disk at the command line: `--min-free-gib` on `takt run` and `takt
//! scenario run` sets the free space a step needs on the file system of the tree and of the build
//! directory; under it the run stops `DiskLow` before any step, exit 8, naming the path checked.
//! No reader is injected here: a threshold far above any disk stops every run. `0` turns the
//! check off. The harness is a double on the program path that counts its calls; nothing reaches
//! a network or a model, and no test fills a disk.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

/// The fixture stores of the library crate, one directory each.
const FIXTURES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../takt/tests/fixtures");

/// More GiB than any disk this runs on has free.
const BEYOND_ANY_DISK: &str = "1000000";

fn git(tree: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .args(["-c", "user.name=t", "-c", "user.email=t@localhost"])
        .args(args)
        .current_dir(tree)
        .output()
        .expect("git runs");
    assert!(output.status.success(), "git {args:?}");
    String::from_utf8_lossy(&output.stdout).trim().to_owned()
}

/// A scratch directory under this crate's build directory, removed on drop.
struct Scratch(PathBuf);

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
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

/// The scratch world: a repository with the fixture store, its one story `proposed`, a manifest
/// and scenario 1 declared on it, no origin; a `claude` double in `bin` that appends a line to
/// `calls` on every call and edits `work.txt`.
struct World {
    _scratch: Scratch,
    repo: PathBuf,
    bin: PathBuf,
    calls: PathBuf,
    state: PathBuf,
}

fn world(name: &str) -> World {
    let outer = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("free-disk-cli-{}-{name}", std::process::id()));
    if outer.exists() {
        fs::remove_dir_all(&outer).expect("a stale scratch tree is removed");
    }
    let repo = outer.join("repo");
    copy_dir(&Path::new(FIXTURES).join("one-ready"), &repo);
    fs::write(repo.join("Cargo.toml"), "[package]\nname = \"scratch\"\n").expect("a file");
    git(&repo, &["init", "-q", "-b", "main"]);
    git(&repo, &["add", "-A"]);
    git(&repo, &["commit", "-q", "-m", "seed"]);
    let seed = git(&repo, &["rev-parse", "HEAD"]);
    fs::write(
        repo.join(".engineering/scenarios.conf"),
        format!("[scenario \"1\"]\n\tstory = story:ready-one\n\tseed = {seed}\n"),
    )
    .expect("the declaration is written");
    git(&repo, &["add", ".engineering/scenarios.conf"]);
    git(&repo, &["commit", "-q", "-m", "declare the scenarios"]);
    let bin = outer.join("stub-bin");
    fs::create_dir_all(&bin).expect("a stub directory");
    let calls = outer.join("calls");
    let state = outer.join("state");
    fs::create_dir_all(&state).expect("a state directory");
    let path = bin.join("claude");
    fs::write(
        &path,
        format!(
            r#"#!/bin/sh
echo call >> '{}'
echo "the work" >> work.txt
printf '{{"result":"{{\\"action\\": \\"implementation.edit\\", \\"summary\\": \\"the double\\"}}","usage":{{"input_tokens":1}}}}'
"#,
            calls.display()
        ),
    )
    .expect("the double is written");
    {
        use std::os::unix::fs::PermissionsExt as _;
        fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).expect("executable");
    }
    World {
        _scratch: Scratch(outer),
        repo,
        bin,
        calls,
        state,
    }
}

impl World {
    /// Runs the built `takt` with `args`: the double first on the program path, the state home
    /// in the scratch world, no build directory named. The exit code, stdout and stderr.
    fn takt(&self, args: &[&str]) -> (i32, String, String) {
        let output = Command::new(env!("CARGO_BIN_EXE_takt"))
            .args(args)
            .env_remove("B10X_GATES_POLICY")
            .env_remove("CARGO_TARGET_DIR")
            .env("XDG_STATE_HOME", self.state.join("xdg-state"))
            .env(
                "PATH",
                format!(
                    "{}:{}",
                    self.bin.display(),
                    std::env::var("PATH").unwrap_or_default()
                ),
            )
            .output()
            .expect("takt runs");
        (
            output.status.code().expect("an exit code"),
            String::from_utf8_lossy(&output.stdout).into_owned(),
            String::from_utf8_lossy(&output.stderr).into_owned(),
        )
    }

    /// The harness double's calls so far.
    fn calls(&self) -> usize {
        fs::read_to_string(&self.calls)
            .unwrap_or_default()
            .lines()
            .count()
    }
}

/// The stop line a `DiskLow` stop prints for the tree `repo`, up to its free space.
fn disk_low(repo: &Path) -> String {
    format!("stopped: DiskLow ({}: ", repo.display())
}

/// `takt run` with a threshold above any disk stops before its first step, exit 8, naming the
/// tree and the threshold; the harness is not called.
#[test]
fn takt_run_under_the_threshold_stops_disk_low_at_exit_8() {
    let world = world("run");
    let repo = world.repo.to_str().expect("utf-8");
    let (code, out, err) = world.takt(&[
        "run",
        "--repo",
        repo,
        "--intent",
        "the work",
        "--test-cmd",
        "true",
        "--forge",
        "none",
        "--iterations",
        "5",
        "--min-free-gib",
        BEYOND_ANY_DISK,
    ]);
    assert_eq!(code, 8, "stdout {out} stderr {err}");
    assert!(
        out.contains(&disk_low(&world.repo)),
        "stdout {out} stderr {err}"
    );
    assert!(
        out.contains("under 1073741824000000 bytes (1000000.00 GiB)); steps 0, tokens 0"),
        "stdout {out} stderr {err}"
    );
    assert!(!out.contains("step 1:"), "{out}");
    assert_eq!(world.calls(), 0, "{out}");
}

/// `takt scenario run` likewise.
#[test]
fn takt_scenario_run_under_the_threshold_stops_disk_low_at_exit_8() {
    let world = world("scenario");
    let repo = world.repo.to_str().expect("utf-8");
    let (code, out, err) = world.takt(&[
        "scenario",
        "run",
        "--repo",
        repo,
        "1",
        "--test-cmd",
        "true",
        "--forge",
        "none",
        "--iterations",
        "5",
        "--min-free-gib",
        BEYOND_ANY_DISK,
    ]);
    assert_eq!(code, 8, "stdout {out} stderr {err}");
    assert!(
        out.contains(&disk_low(&world.repo)),
        "stdout {out} stderr {err}"
    );
    assert!(
        out.contains("; steps 0, tokens 0"),
        "stdout {out} stderr {err}"
    );
    assert_eq!(world.calls(), 0, "{out}");
}

/// `--min-free-gib 0` turns the check off: the same run takes its steps.
#[test]
fn a_threshold_of_zero_turns_the_check_off() {
    let world = world("zero");
    let repo = world.repo.to_str().expect("utf-8");
    let (_, out, err) = world.takt(&[
        "run",
        "--repo",
        repo,
        "--intent",
        "the work",
        "--test-cmd",
        "true",
        "--forge",
        "none",
        "--iterations",
        "1",
        "--min-free-gib",
        "0",
    ]);
    assert!(out.contains("step 1:"), "stdout {out} stderr {err}");
    assert!(!out.contains("DiskLow"), "{out}");
}

/// Both verbs document the flag, its default and the stop under exit 8, its own code, which the
/// exit-3 line no longer lists; the flag's help names exit 8 too.
#[test]
fn the_help_documents_the_flag_and_the_stop() {
    let world = world("help");
    for verb in [&["run", "--help"][..], &["scenario", "run", "--help"][..]] {
        let (code, help, _) = world.takt(verb);
        assert_eq!(code, 0, "{help}");
        assert!(help.contains("--min-free-gib <GIB>"), "{help}");
        assert!(help.contains("[default: 10]"), "{help}");
        let three = help
            .lines()
            .find(|line| line.trim_start().starts_with("3  stopped:"))
            .expect("an exit-3 line");
        assert!(!three.contains("DiskLow"), "{three}");
        let eight = help
            .lines()
            .find(|line| line.trim_start().starts_with("8  stopped:"))
            .expect("an exit-8 line");
        assert!(eight.contains("DiskLow"), "{eight}");
        let flag = help
            .split("--min-free-gib <GIB>")
            .nth(1)
            .and_then(|rest| rest.split("[default: 10]").next())
            .expect("the flag's help");
        assert!(flag.contains("exit 8"), "{flag}");
        assert!(!flag.contains("exit 3"), "{flag}");
    }
}
