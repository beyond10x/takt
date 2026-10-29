//! story:gate-commands-find-programs-on-a-system-path: a gate command names its program without an
//! absolute path or a `PATH` of its own. A bare program name is found on the fixed system path
//! `/usr/local/bin:/usr/bin:/bin`, or on the `PATH` the binding's own `env` declares, never on the
//! user's `PATH`, and the command runs with the `PATH` the binding declares or the fixed one. Each
//! test observes `gate.run` over a scratch repository whose project file admits one command.

#[path = "support/own_home.rs"]
mod own_home;

use std::fs;
use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::anyhow;
use takt::harness::{Harness, StepContext, StepReport};
use takt::validator::{FailedCheck, GateRun, Observation, Validator};
use takt::{Run, RunOptions};

/// The protocol the case opens on; no step of it runs.
const PROTOCOL: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/fixtures/gate-only/1.yaml"
);

/// The fixed path a bare program name is found on when its binding declares no `PATH`.
const FIXED_PATH: &str = "/usr/local/bin:/usr/bin:/bin";

/// A scratch directory under this crate's build directory, removed when dropped.
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

/// A new, empty scratch directory named `name` for this process.
fn directory(name: &str) -> Scratch {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!(
        "gate-commands-find-programs-{}-{name}",
        std::process::id()
    ));
    if dir.exists() {
        fs::remove_dir_all(&dir).expect("a stale scratch tree is removed");
    }
    fs::create_dir_all(&dir).expect("a scratch directory");
    Scratch(dir)
}

fn git(tree: &Path, args: &[&str]) {
    let status = Command::new("git")
        .args(["-c", "user.name=t", "-c", "user.email=t@localhost"])
        .args(args)
        .current_dir(tree)
        .status()
        .expect("git runs");
    assert!(status.success(), "git {args:?}");
}

/// An executable file `name` in `dir`, a `sh` script of `body`.
fn program(dir: &Path, name: &str, body: &str) -> PathBuf {
    fs::create_dir_all(dir).expect("the program's directory");
    let path = dir.join(name);
    fs::write(&path, format!("#!/bin/sh\n{body}\n")).expect("a program");
    fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).expect("executable");
    path
}

/// A committed repository whose gates document holds one gate, `probe`, true when the command
/// `probe` exits 0, and whose project file binds `probe` to `binding`, the lines under
/// `commands: probe:` (indented four spaces).
fn repository(name: &str, binding: &str) -> Scratch {
    repository_with(name, binding, |_| {})
}

/// As [`repository`], with `extra` run on the tree before its first commit.
fn repository_with(name: &str, binding: &str, extra: impl FnOnce(&Path)) -> Scratch {
    let tree = directory(name);
    fs::create_dir_all(tree.join(".engineering")).expect("its directory");
    extra(&tree);
    fs::write(
        tree.join(".engineering/gates.yaml"),
        "format: engineering-gates/1\nlanguage: canon-expr/1\ngates:\n  \
         - id: probe\n    text: The probe command exits 0.\n    assertions:\n      \
         - 'process.succeeded(\"probe\")'\n",
    )
    .expect("a gates document");
    fs::write(
        tree.join(".engineering/assertions-project.yaml"),
        format!("format: engineering-assertions-project/1\ncommands:\n  probe:\n{binding}"),
    )
    .expect("a project file");
    git(&tree, &["init", "-q", "-b", "main"]);
    git(&tree, &["add", "-A"]);
    git(&tree, &["commit", "-q", "-m", "start"]);
    tree
}

/// A harness the observation below never reaches: no step of the case runs.
struct NoHarness;

impl Harness for NoHarness {
    fn name(&self) -> &str {
        "none"
    }

    fn step(&self, context: &StepContext<'_>) -> Result<StepReport, anyhow::Error> {
        Err(anyhow!(
            "no harness: a judgement step among {} is left",
            context.offered.join(", ")
        ))
    }
}

/// `gate.run` over the repository's `.engineering/gates.yaml`, observed at its base revision in
/// a case `id` opened there: the observation, and the failed checks it kept.
fn observe(tree: &Path, id: &str) -> anyhow::Result<(Observation, Vec<FailedCheck>)> {
    let options = RunOptions {
        protocol: PROTOCOL.to_owned(),
        intent: "run the gate".to_owned(),
        inputs: Vec::new(),
        iterations: Some(0),
        hours: None,
        tokens: None,
        case_id: Some(id.to_owned()),
        gate: None,
    };
    let none: Vec<Box<dyn Validator>> = Vec::new();
    let _opened = Run::open(tree, options, &NoHarness, &none).expect("the case opens");
    let gate = GateRun::new(".engineering/gates.yaml");
    let observation = gate.observe(tree, "HEAD", 1)?;
    Ok((observation, gate.take_failed()))
}

/// The result of the gate's record, `pass` or `fail`, when the observation wrote one.
fn result(observation: &Observation) -> Option<&str> {
    observation
        .records
        .first()
        .and_then(|record| record.result.as_deref())
}

/// story:gate-commands-find-programs-on-a-system-path, scenario `gate/bare-program-name-runs`: a
/// gates document whose command is `sh -c 'exit 0'` with no `env` makes `gate.run` pass; the
/// same with `exit 1` fails it, on the command's exit.
#[test]
fn gate_bare_program_name_runs() {
    let passing = repository(
        "bare-pass",
        "    executable: sh\n    args: ['-c', 'exit 0']\n",
    );
    let (observation, _) = observe(&passing, "bare-pass").expect("the gate is observed");
    assert_eq!(
        result(&observation),
        Some("pass"),
        "story:gate-commands-find-programs-on-a-system-path: {}",
        observation.summary
    );

    let failing = repository(
        "bare-fail",
        "    executable: sh\n    args: ['-c', 'exit 1']\n",
    );
    let (observation, _) = observe(&failing, "bare-fail").expect("the gate is observed");
    assert_eq!(
        result(&observation),
        Some("fail"),
        "story:gate-commands-find-programs-on-a-system-path: {}",
        observation.summary
    );
}

/// story:gate-commands-find-programs-on-a-system-path, scenario `gate/user-path-not-inherited`: a
/// program found only on the parent's `PATH`, not on the fixed path, is not found by a gate
/// command, and the refusal names the program and the path searched.
#[test]
fn gate_user_path_not_inherited() {
    let dir = directory("user-path-programs");
    let bin = dir.join("bin");
    let name = format!("takt-probe-{}", std::process::id());
    program(&bin, &name, "exit 0");
    let user = std::env::var("PATH").unwrap_or_default();
    // SAFETY: the other tests of this binary run `git` and takt, which find programs by name
    // through `PATH`; a prefix changes only which copy of a program they could find, and the
    // program this test adds is one no other test looks for.
    unsafe {
        std::env::set_var("PATH", format!("{}:{user}", bin.display()));
    }
    let tree = repository("user-path", &format!("    executable: {name}\n"));
    let refused = observe(&tree, "user-path");
    // SAFETY: as above; the `PATH` this test set is put back before its assertions run.
    unsafe {
        std::env::set_var("PATH", &user);
    }
    let error = refused.expect_err("a program found only on the user's PATH is refused");
    let message = format!("story:gate-commands-find-programs-on-a-system-path: {error:#}");
    assert!(message.contains(&name), "{message}");
    assert!(message.contains(FIXED_PATH), "{message}");
}

/// story:gate-commands-find-programs-on-a-system-path, scenario `gate/declared-path-wins`: a
/// command whose `env` sets `PATH` runs with that `PATH`, as today, and its program is found on
/// that `PATH`, not on the fixed one; a bare name the declared `PATH` lacks is refused, the
/// declared `PATH` named as the path searched.
#[test]
fn gate_declared_path_wins() {
    let dir = directory("declared-path-programs");
    let bin = dir.join("bin");
    let name = "probe-tool";
    program(
        &bin,
        name,
        &format!("[ \"$PATH\" = '{}' ] || exit 1\nexit 0", bin.display()),
    );
    let tree = repository(
        "declared-path",
        &format!(
            "    executable: {name}\n    env:\n      PATH: '{}'\n",
            bin.display()
        ),
    );
    let (observation, _) = observe(&tree, "declared-path").expect("the gate is observed");
    assert_eq!(
        result(&observation),
        Some("pass"),
        "story:gate-commands-find-programs-on-a-system-path: {}",
        observation.summary
    );

    let empty = dir.join("empty");
    fs::create_dir_all(&empty).expect("an empty directory");
    let tree = repository(
        "declared-path-refused",
        &format!(
            "    executable: sh\n    args: ['-c', 'exit 0']\n    env:\n      PATH: '{}'\n",
            empty.display()
        ),
    );
    let error = observe(&tree, "declared-path-refused")
        .expect_err("a bare name the declared PATH lacks is refused");
    let message = format!("{error:#}");
    assert!(message.contains("sh"), "{message}");
    assert!(message.contains(&empty.display().to_string()), "{message}");
}

/// What a failed gate's command printed again, on the run it failed, is what it prints on its
/// first run: the same `PATH`, the fixed one the binding does not declare.
#[test]
fn gate_failed_command_runs_again_on_the_same_path() {
    let tree = repository(
        "rerun-path",
        "    executable: sh\n    args: ['-c', 'echo path=$PATH; exit 1']\n",
    );
    let (observation, failed) = observe(&tree, "rerun-path").expect("the gate is observed");
    assert_eq!(
        result(&observation),
        Some("fail"),
        "{}",
        observation.summary
    );
    let printed = failed
        .iter()
        .flat_map(|check| check.lines.iter())
        .any(|line| line.contains(&format!("path={FIXED_PATH}")));
    assert!(printed, "the failed check names the fixed PATH: {failed:?}");
}

/// A program a declared `PATH` finds inside the tree is refused: the gate's comparison covers the
/// files a project file names, not the directories its `PATH` lists, so a program there is one a
/// step could change between the base and the observation.
#[test]
fn gate_declared_path_inside_the_tree_is_refused() {
    let name = "probe-tool";
    let at = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!(
        "gate-commands-find-programs-{}-declared-path-in-tree",
        std::process::id()
    ));
    let bin = at.join("bin");
    let tree = repository_with(
        "declared-path-in-tree",
        &format!(
            "    executable: {name}\n    env:\n      PATH: '{}'\n",
            bin.display()
        ),
        |tree| {
            program(&tree.join("bin"), name, "exit 0");
        },
    );
    let error = observe(&tree, "declared-path-in-tree")
        .expect_err("a program the declared PATH finds inside the tree is refused");
    let message = format!("{error:#}");
    assert!(message.contains("inside the tree"), "{message}");
    assert!(message.contains(name), "{message}");
}
