//! story:forge-none-stops-when-only-forge-actions-remain at the command line: a scenario run
//! under `--forge none` works up to `change.read` ahead, then stops `Unperformable` naming
//! `pr.opened` and the forge, exit 7, with no harness call after it; `takt run --case` on the
//! case takes none either. The harness is a double on the program path that counts its calls;
//! nothing reaches a network or a model.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

/// The fixture stores of the library crate, one directory each.
const FIXTURES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../takt/tests/fixtures");

/// A gates document whose one assertion is true in a repository with a manifest.
const GATES: &str = "format: engineering-gates/1\nlanguage: canon-expr/1\ngates:\n  - id: manifest\n    \
                     text: The repository has its manifest.\n    assertions:\n      \
                     - 'fs.file.exists(\"Cargo.toml\")'\n";

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

/// The scratch world: a repository with the fixture store, its one story `proposed`, a manifest,
/// a gates document and scenario 1 declared on it, no origin; a `claude` double in `bin` that
/// appends a line to `calls` on every call, then edits `work.txt` once and hands the change over
/// when it is offered.
struct World {
    _scratch: Scratch,
    repo: PathBuf,
    bin: PathBuf,
    calls: PathBuf,
    state: PathBuf,
}

fn world(name: &str) -> World {
    let outer = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("unperformable-cli-{}-{name}", std::process::id()));
    if outer.exists() {
        fs::remove_dir_all(&outer).expect("a stale scratch tree is removed");
    }
    let repo = outer.join("repo");
    copy_dir(&Path::new(FIXTURES).join("one-ready"), &repo);
    fs::write(repo.join("Cargo.toml"), "[package]\nname = \"scratch\"\n").expect("a file");
    fs::write(repo.join(".engineering/gates.yaml"), GATES).expect("a file");
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
if [ "$1" = "--version" ]; then echo double; exit 0; fi
echo call >> '{}'
case "$2" in
  *"- change.handover:"*)
    if [ -e work.txt ]; then action=change.handover;
    else echo "the work" >> work.txt; action=implementation.edit; fi ;;
  *) echo "the work" >> work.txt; action=implementation.edit ;;
esac
printf '{{"result":"{{\\"action\\": \\"%s\\", \\"summary\\": \\"the double\\"}}","usage":{{"input_tokens":1}}}}' "$action"
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
    /// in the scratch world. The exit code, stdout and stderr.
    fn takt(&self, args: &[&str]) -> (i32, String, String) {
        let output = Command::new(env!("CARGO_BIN_EXE_takt"))
            .args(args)
            .env_remove("B10X_GATES_POLICY")
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

/// The story's acceptance: the scenario run reaches "ahead", stops naming `pr.opened` and the
/// forge, exit 7, with no harness call after `change.read`; the resume calls the harness not at
/// all and stops the same way.
#[test]
fn a_forge_none_scenario_run_stops_at_ahead_and_its_resume_calls_no_harness() {
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
        "20",
    ]);
    assert!(
        out.contains("evidence: change_record ahead"),
        "stdout {out} stderr {err}"
    );
    let after = out
        .split("evidence: change_record ahead")
        .nth(1)
        .expect("a line after ahead");
    assert!(!after.contains("step "), "a step after ahead: {out}");
    assert!(
        out.contains("stopped: Unperformable (pr.opened; nothing performs it without the forge)"),
        "stdout {out} stderr {err}"
    );
    assert_eq!(code, 7, "stdout {out} stderr {err}");
    let calls = world.calls();
    assert!(calls >= 1, "the double handed the change over: {out}");
    // Without the bot route the trial number is the instant and a nonce, so the case id is read
    // from the run's `case <id> on` line rather than assumed.
    let case = out
        .lines()
        .find_map(|line| {
            line.strip_prefix("case ")?
                .split_once(" on ")
                .map(|(id, _)| id)
        })
        .expect("the run names its case")
        .to_owned();
    assert!(case.starts_with("scenario-1-trial-"), "{case}");

    let (code, out, err) = world.takt(&[
        "run",
        "--repo",
        repo,
        "--case",
        &case,
        "--test-cmd",
        "true",
        "--forge",
        "none",
        "--iterations",
        "20",
    ]);
    assert_eq!(world.calls(), calls, "the resume called the harness: {out}");
    assert!(
        out.contains("stopped: Unperformable (pr.opened; nothing performs it without the forge)"),
        "stdout {out} stderr {err}"
    );
    assert_eq!(code, 7, "stdout {out} stderr {err}");
}
