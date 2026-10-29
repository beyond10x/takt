//! Adversarial cases on story:gate-commands-find-programs-on-a-system-path. The specification
//! (`ess/domains/takt.yaml`): "A bare program name a gate's project file admits (no `/`, and not
//! a file of the tree) is written absolute, found on the `PATH` its binding's `env` declares or
//! else on `/usr/local/bin:/usr/bin:/bin` [...]; a name found nowhere, or only inside the tree,
//! refuses the gate". Each case observes `gate.run` over a scratch repository.

#[path = "support/own_home.rs"]
mod own_home;

use std::fs;
use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::anyhow;
use takt::harness::{Harness, StepContext, StepReport};
use takt::validator::{GateRun, Observation, Validator};
use takt::{Run, RunOptions};

const PROTOCOL: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/fixtures/gate-only/1.yaml"
);

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

fn directory(name: &str) -> Scratch {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("gate-path-adv-frd-{}-{name}", std::process::id()));
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

fn program(dir: &Path, name: &str, body: &str, mode: u32) -> PathBuf {
    fs::create_dir_all(dir).expect("the program's directory");
    let path = dir.join(name);
    fs::write(&path, format!("#!/bin/sh\n{body}\n")).expect("a program");
    fs::set_permissions(&path, fs::Permissions::from_mode(mode)).expect("a mode");
    path
}

/// A committed repository whose one gate is true when the command `probe` exits 0, `probe` bound
/// to `binding`; `extra` runs on the tree before its first commit.
fn repository(name: &str, binding: &str, extra: impl FnOnce(&Path)) -> Scratch {
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

struct NoHarness;

impl Harness for NoHarness {
    fn name(&self) -> &str {
        "none"
    }

    fn step(&self, _context: &StepContext<'_>) -> Result<StepReport, anyhow::Error> {
        Err(anyhow!("no harness"))
    }
}

fn observe(tree: &Path, id: &str) -> anyhow::Result<Observation> {
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
    gate.observe(tree, "HEAD", 1)
}

fn result(observation: &Observation) -> Option<&str> {
    observation
        .records
        .first()
        .and_then(|record| record.result.as_deref())
}

/// The guide's gate names `sh` bare; the system `sh` runs `exit 1` and the gate fails. A file
/// `sh` in the tree that git ignores (a test run's output may stay; a committed `.gitignore` is
/// not on the judge surface) is "a file of the tree", so the name is left bare, the assertions
/// library joins it to the tree's root, and the planted program runs. It is in no commit, so the
/// base comparison sees nothing: the gate passes on a program that is not the system `sh`.
#[test]
fn an_ignored_tree_file_does_not_shadow_a_bare_program_name() {
    let tree = repository(
        "ignored-shadow",
        "    executable: sh\n    args: ['-c', 'exit 1']\n",
        |tree| fs::write(tree.join(".gitignore"), "/sh\n").expect("an ignore file"),
    );
    program(&tree, "sh", "exit 0", 0o755);
    let observed = observe(&tree, "ignored-shadow");
    if let Ok(observation) = &observed {
        assert_ne!(
            result(observation),
            Some("pass"),
            "story:gate-commands-find-programs-on-a-system-path: an ignored ./sh shadows the system sh and the gate passes: {}",
            observation.summary
        );
    }
}

/// A shell finds a bare name on the first directory of `PATH` holding an executable file of that
/// name. The resolution takes the first file of that name, executable or not, so a declared
/// `PATH` whose first directory holds a non-executable `probe` runs that file, not the program
/// the command's own `PATH` lookup finds.
#[test]
fn a_bare_name_is_found_as_a_path_lookup_finds_it_skipping_a_non_executable_file() {
    let dir = directory("non-exec-programs");
    let first = dir.join("first");
    let second = dir.join("second");
    program(&first, "probe-tool", "exit 1", 0o644);
    program(&second, "probe-tool", "exit 0", 0o755);
    let tree = repository(
        "non-exec",
        &format!(
            "    executable: probe-tool\n    env:\n      PATH: '{}:{}'\n",
            first.display(),
            second.display()
        ),
        |_| {},
    );
    let observed = observe(&tree, "non-exec");
    let summary = match &observed {
        Ok(observation) => format!("{:?}: {}", result(observation), observation.summary),
        Err(error) => format!("Err({error:#})"),
    };
    assert!(
        observed
            .as_ref()
            .is_ok_and(|observation| result(observation) == Some("pass")),
        "story:gate-commands-find-programs-on-a-system-path: the bare name resolved to a non-executable file: {summary}"
    );
}
