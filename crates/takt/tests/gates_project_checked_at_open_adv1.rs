//! Adversary cases for story:gates-project-checked-at-open. The check at open runs for every
//! validator registered, whatever the case's protocol does with it, and prepares the gate's
//! inputs under the case directory as if it were inside the tree.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::anyhow;
use takt::harness::{Harness, StepContext, StepReport};
use takt::validator::{GateRun, Validator};
use takt::workspace::Workspace;
use takt::{Run, RunOptions};

/// The protocol of the unit's own tests: `gate.run` and nothing a harness decides.
const GATE_ONLY: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/fixtures/gate-only/1.yaml"
);

/// The project file, relative to the tree.
const PROJECT: &str = ".engineering/assertions-project.yaml";

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
        let _ = fs::remove_dir_all(&self.0);
    }
}

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

/// Writes the gates document (one gate, `probe`) and a project file binding `probe` with
/// `timeout_seconds` into `tree`, and commits them as the repository's first commit.
fn gated_repository(tree: &Path, timeout_seconds: u64) {
    fs::create_dir_all(tree.join(".engineering")).expect("its directory");
    fs::write(
        tree.join(".engineering/gates.yaml"),
        "format: engineering-gates/1\nlanguage: canon-expr/1\ngates:\n  \
         - id: probe\n    text: The probe command exits 0.\n    assertions:\n      \
         - 'process.succeeded(\"probe\")'\n",
    )
    .expect("a gates document");
    fs::write(
        tree.join(PROJECT),
        format!(
            "format: engineering-assertions-project/1\ntimeout_seconds: {timeout_seconds}\n\
             commands:\n  probe:\n    executable: sh\n    args: ['-c', 'exit 0']\n"
        ),
    )
    .expect("a project file");
    fs::write(tree.join("README.md"), "a repository\n").expect("a file");
    git(tree, &["init", "-q", "-b", "main"]);
    git(tree, &["add", "-A"]);
    git(tree, &["commit", "-q", "-m", "start"]);
}

fn scratch(name: &str) -> Scratch {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("gates-project-adv1-{}-{name}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("a scratch directory");
    Scratch(fs::canonicalize(&dir).expect("a canonical scratch directory"))
}

/// A harness that takes no step.
struct NoStep;

impl Harness for NoStep {
    fn name(&self) -> &str {
        "no-step"
    }

    fn step(&self, context: &StepContext<'_>) -> Result<StepReport, anyhow::Error> {
        Err(anyhow!("no step among {}", context.offered.join(", ")))
    }
}

/// `software.change/1` is the protocol `takt run --intent` opens on by default, and it has no
/// `gate.run` action: its cases never run the gate. The CLI registers the gate validator
/// whenever `.engineering/gates.yaml` is in the repository (`validators_of`), so a project file
/// the gate would refuse now refuses a case that would never have read it. Before the unit it
/// opened and ran.
#[test]
fn a_protocol_without_a_gate_step_is_not_refused_over_the_gate_s_project_file() {
    let tree = scratch("no-gate-step");
    gated_repository(&tree, 1800);
    let protocol = takt::protocol::LoadedProtocol::builtin("software.change/1")
        .expect("the built-in protocol");
    assert!(
        !protocol.actions.iter().any(|action| action == "gate.run"),
        "software.change/1 has a gate.run action after all"
    );
    let validators: Vec<Box<dyn Validator>> =
        vec![Box::new(GateRun::new(".engineering/gates.yaml"))];
    let options = RunOptions {
        protocol: "software.change/1".to_owned(),
        intent: "fix the check".to_owned(),
        inputs: vec![
            ("risk".to_owned(), "trivial".to_owned()),
            ("change_kind".to_owned(), "feature".to_owned()),
            ("affects_security_boundary".to_owned(), "false".to_owned()),
        ],
        iterations: Some(1),
        hours: None,
        tokens: None,
        case_id: Some("no-gate-step".to_owned()),
        gate: None,
    };
    if let Err(error) = Run::open(&tree, options, &NoStep, &validators) {
        panic!(
            "story:gates-project-checked-at-open: a software.change/1 case, whose protocol never runs gate.run, was refused at open over the gate's project file: {error:#}"
        );
    }
}

/// A fixture repository `ws/repo` holding a valid gates document and project file
/// (`timeout_seconds: 60`), with the `worktree` registry, configuration and trees under the
/// scratch directory, as `workspace_seam` builds its world.
struct World {
    dir: Scratch,
    repo: PathBuf,
    session: String,
}

impl World {
    fn new(name: &str) -> Self {
        let dir = scratch(&format!("seam-{name}"));
        fs::create_dir_all(dir.join("ws/repo")).expect("the repository directory");
        fs::create_dir_all(dir.join("state")).expect("the state directory");
        fs::create_dir_all(dir.join("config")).expect("the configuration directory");
        let repo = dir.join("ws/repo");
        gated_repository(&repo, 60);
        fs::write(
            dir.join("profile.toml"),
            "version = 1\nname = \"default\"\nexpire_after_seconds = 604800\nprotect_workspace_root = false\n",
        )
        .expect("a profile");
        let world = World {
            session: format!("takt-adv1-{}-{name}", std::process::id()),
            dir,
            repo,
        };
        let profile = world.dir.join("profile.toml");
        let workspace = world.dir.join("ws");
        world.worktree(&[
            "activate",
            "--profile",
            profile.to_str().expect("utf-8"),
            "--workspace",
            workspace.to_str().expect("utf-8"),
        ]);
        world
    }

    fn worktree(&self, args: &[&str]) {
        let output = Command::new("worktree")
            .args(args)
            .arg("--json")
            .env("XDG_STATE_HOME", self.dir.join("state"))
            .env("XDG_CONFIG_HOME", self.dir.join("config"))
            .output()
            .expect("worktree runs");
        assert!(
            output.status.success(),
            "worktree {args:?}: {}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }

    fn workspace(&self) -> Workspace {
        Workspace::new(&self.session)
            .env("XDG_STATE_HOME", self.dir.join("state"))
            .env("XDG_CONFIG_HOME", self.dir.join("config"))
    }
}

/// A dispatched case keeps its records under the dispatching root, not in its managed tree
/// (`CaseDir::open_in`); the check at open copies the gate's inputs under the case directory
/// and takes them relative to the tree, so a valid project file is refused at open.
#[test]
fn a_dispatched_case_over_a_valid_project_file_opens() {
    let world = World::new("dispatched");
    let validators: Vec<Box<dyn Validator>> =
        vec![Box::new(GateRun::new(".engineering/gates.yaml"))];
    let options = RunOptions {
        protocol: GATE_ONLY.to_owned(),
        intent: "run the gate".to_owned(),
        inputs: Vec::new(),
        iterations: Some(1),
        hours: None,
        tokens: None,
        case_id: Some("child-1".to_owned()),
        gate: None,
    };
    if let Err(error) = Run::dispatch(
        &world.repo,
        &world.workspace(),
        "root-1",
        options,
        &NoStep,
        &validators,
    ) {
        panic!(
            "story:gates-project-checked-at-open: a dispatched case over a valid project file ({PROJECT}, timeout_seconds 60) was refused at open: {error:#}"
        );
    }
}
