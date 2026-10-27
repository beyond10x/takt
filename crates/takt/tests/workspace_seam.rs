//! story:the-workspace-seam-enters-and-leaves-managed-worktrees: the `Workspace` seam creates a
//! managed worktree when a case is dispatched (`worktree create --purpose takt:<case>`), takes a
//! lease on it, leaves it archived and released when the child stops without a terminal outcome,
//! and finishes it (`worktree finish --discard-cache --archive`) when the child completes. The
//! child's records are written to the root's cases directory, never into its tree. Every
//! scenario runs against the real `worktree` binary on a fixture repository, with the
//! registry, the policy and the trees under the scratch directory.

use std::cell::RefCell;
use std::collections::VecDeque;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::rc::Rc;

use anyhow::anyhow;
use serde_json::Value;
use takt::case::grant;
use takt::evidence::Record;
use takt::forge::{Checks, Forge, Memory};
use takt::harness::{Harness, StepContext, StepReport};
use takt::run_config::SetupCommand;
use takt::validator::{ChangeRead, CiRead, Observation, PrMerged, PrOpen, TestsRun, Validator};
use takt::workspace::Workspace;
use takt::{Run, RunOptions, Stop};

/// The case id the dispatches below open, and the root case that dispatches it.
const CASE: &str = "child-1";
const PARENT: &str = "root-1";

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

/// A fixture repository `ws/repo` with two commits, the `worktree` registry, configuration and
/// trees under `state/` and `config/`, and the policy that governs `ws/`. Dropping it removes it.
struct World {
    dir: PathBuf,
    repo: PathBuf,
    session: String,
}

impl Drop for World {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.dir);
    }
}

fn world(name: &str) -> World {
    let dir =
        Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("seam-{}-{name}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(dir.join("ws/repo")).expect("the repository directory");
    fs::create_dir_all(dir.join("state")).expect("the state directory");
    fs::create_dir_all(dir.join("config")).expect("the configuration directory");
    let dir = fs::canonicalize(&dir).expect("a canonical scratch directory");
    let repo = dir.join("ws/repo");
    fs::write(repo.join("README.md"), "a story\n").expect("a file");
    git(&repo, &["init", "-q", "-b", "main"]);
    git(&repo, &["add", "-A"]);
    git(&repo, &["commit", "-q", "-m", "start"]);
    fs::write(repo.join("NOTES.md"), "second\n").expect("a file");
    git(&repo, &["add", "-A"]);
    git(&repo, &["commit", "-q", "-m", "second"]);
    fs::write(
        dir.join("profile.toml"),
        "version = 1\nname = \"default\"\nexpire_after_seconds = 604800\nprotect_workspace_root = false\n",
    )
    .expect("a profile");
    let world = World {
        session: format!("takt-run-{}-{name}", std::process::id()),
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

impl World {
    /// Runs the real `worktree <args> --json` with this world's registry and configuration; its
    /// answer.
    fn worktree(&self, args: &[&str]) -> Value {
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
        serde_json::from_slice(&output.stdout).expect("worktree answers in JSON")
    }

    /// The seam over this world's `worktree`: the same registry and configuration, and a lease
    /// session of its own.
    fn workspace(&self) -> Workspace {
        Workspace::new(&self.session)
            .env("XDG_STATE_HOME", self.dir.join("state"))
            .env("XDG_CONFIG_HOME", self.dir.join("config"))
    }

    /// Every tree the registry holds for the repository, as `worktree inspect` reports it: the
    /// `record` and the `live_leases` of each.
    fn trees(&self) -> Vec<Value> {
        let repo = self.repo.to_str().expect("utf-8");
        self.worktree(&["inspect", "--repo", repo])["inspections"]
            .as_array()
            .expect("a list of inspections")
            .clone()
    }

    /// The trees whose purpose is `purpose`.
    fn trees_for(&self, purpose: &str) -> Vec<Value> {
        self.trees()
            .into_iter()
            .filter(|tree| tree["record"]["purpose"] == purpose)
            .collect()
    }

    /// The one tree whose purpose is `takt:<case>`.
    fn tree_of(&self, case: &str) -> Value {
        let mut trees = self.trees_for(&format!("takt:{case}"));
        assert_eq!(trees.len(), 1, "{trees:#?}");
        trees.remove(0)
    }

    /// The tree the case's `case.yaml` names as its `worktree`.
    fn own_tree(&self, case: &str) -> Value {
        let named = self.case_yaml(case)["worktree"]
            .as_str()
            .map(PathBuf::from)
            .expect("the case names its tree");
        let mut trees: Vec<Value> = self
            .trees()
            .into_iter()
            .filter(|tree| path_of(tree) == named)
            .collect();
        assert_eq!(trees.len(), 1, "{trees:#?}");
        trees.remove(0)
    }

    /// The case's directory under the root: where its records are.
    fn case_dir(&self, case: &str) -> PathBuf {
        self.repo.join(".takt/cases").join(case)
    }

    fn case_yaml(&self, case: &str) -> serde_yaml_ng::Value {
        let path = self.case_dir(case).join("case.yaml");
        serde_yaml_ng::from_str(&fs::read_to_string(&path).expect("the case is at the root"))
            .expect("case.yaml reads")
    }

    /// Where `worktree archive` keeps the archive of the tree `id`.
    fn archive(&self, id: &str) -> PathBuf {
        self.dir
            .join("state/worktree/archives/repo")
            .join(id)
            .join("manifest.json")
    }

    fn events(&self, case: &str) -> Vec<Value> {
        fs::read_to_string(self.case_dir(case).join("events.jsonl"))
            .unwrap_or_default()
            .lines()
            .map(|line| serde_json::from_str(line).expect("a line"))
            .collect()
    }
}

fn path_of(tree: &Value) -> PathBuf {
    PathBuf::from(tree["record"]["path"].as_str().expect("a path"))
}

fn id_of(tree: &Value) -> String {
    tree["record"]["id"].as_str().expect("an id").to_owned()
}

/// A harness that changes nothing and asks for the merge gate when it is offered, as a judgement
/// actor does once the intent holds; any other step it is offered is a test failure.
struct Gate;

impl Harness for Gate {
    fn name(&self) -> &str {
        "gate"
    }

    fn step(&self, context: &StepContext<'_>) -> Result<StepReport, anyhow::Error> {
        if context.approvals.contains(&"repository.merge".to_owned())
            || context.offered.contains(&"repository.merge".to_owned())
        {
            return Ok(StepReport {
                action: "repository.merge".to_owned(),
                summary: "the intent holds; merge it".to_owned(),
                tokens: None,
            });
        }
        Err(anyhow!(
            "no judgement step is wanted among {}",
            context.offered.join(", ")
        ))
    }
}

/// A delivery whose iterations are one: the setup, then the first check, then the stop.
fn options(case: &str) -> RunOptions {
    RunOptions {
        protocol: "story.delivery/1".to_owned(),
        intent: "Make a change".to_owned(),
        inputs: Vec::new(),
        iterations: Some(1),
        hours: None,
        tokens: None,
        case_id: Some(case.to_owned()),
        gate: None,
    }
}

/// A delivery that stops at the merge gate once its change is handed over, and is accepted once
/// the merge is granted.
fn delivery(case: &str) -> RunOptions {
    RunOptions {
        iterations: Some(40),
        gate: Some("repository.merge".to_owned()),
        ..options(case)
    }
}

/// A validator that observes `pass` for its action at every revision, under its own name.
struct Green {
    name: &'static str,
    action: &'static str,
    kind: &'static str,
}

impl Validator for Green {
    fn name(&self) -> &str {
        self.name
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

/// Edits the implementation, then writes the tests, one step each; then requests the merge when
/// it is granted, and otherwise hands the change over.
struct Developer {
    edits: RefCell<VecDeque<(&'static str, &'static str)>>,
}

impl Developer {
    fn new() -> Self {
        Self {
            edits: RefCell::new(VecDeque::from([
                ("implementation.edit", "src/change.txt"),
                ("tests.write", "tests/change_test.txt"),
            ])),
        }
    }
}

impl Harness for Developer {
    fn name(&self) -> &str {
        "developer"
    }

    fn step(&self, context: &StepContext<'_>) -> Result<StepReport, anyhow::Error> {
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

/// The validators of a delivery over a forge double that passes its checks.
fn delivering() -> Vec<Box<dyn Validator>> {
    let forge: Rc<dyn Forge> = Rc::new(Memory {
        checks: Checks::Pass,
        ..Memory::default()
    });
    vec![
        Box::new(Green {
            name: "green.tests-run",
            action: "tests.run",
            kind: "test_result",
        }),
        Box::new(Green {
            name: "green.gate-run",
            action: "gate.run",
            kind: "gate_result",
        }),
        Box::new(PrOpen::new(forge.clone())),
        Box::new(CiRead::new(forge.clone())),
        Box::new(ChangeRead),
        Box::new(PrMerged::new(forge)),
    ]
}

fn command(name: &str, argv: &[&str]) -> SetupCommand {
    SetupCommand {
        name: name.to_owned(),
        argv: argv.iter().map(|word| (*word).to_owned()).collect(),
    }
}

fn green() -> Vec<Box<dyn Validator>> {
    vec![Box::new(TestsRun::new("true").expect("a command"))]
}

/// Dispatches `case` of `options` from the root `world.repo` under the root case [`PARENT`].
fn dispatch<'a>(
    world: &World,
    options: RunOptions,
    harness: &'a dyn Harness,
    validators: &'a [Box<dyn Validator>],
) -> Run<'a> {
    Run::dispatch(
        &world.repo,
        &world.workspace(),
        PARENT,
        options,
        harness,
        validators,
    )
    .expect("the case is dispatched")
}

/// Runs `run` to its stop with `setup` declared; the stop and its output.
fn go(mut run: Run<'_>, world: &World, setup: Vec<SetupCommand>) -> (Stop, String) {
    run.transcripts_in(world.dir.join("state/transcripts"))
        .expect("a state directory");
    run.declare_setup(setup);
    let mut out = Vec::new();
    let stop = run.run(&mut out).expect("the run ends");
    (stop, String::from_utf8_lossy(&out).into_owned())
}

/// A change dispatched, run to the merge gate, granted and accepted: the whole life of a child
/// that ends in a terminal outcome. Answers the id of its tree.
fn accepted(world: &World, case: &str) -> String {
    let validators = delivering();
    let developer = Developer::new();
    let (stop, text) = go(
        dispatch(world, delivery(case), &developer, &validators),
        world,
        Vec::new(),
    );
    assert_eq!(
        stop,
        Stop::ApprovalRequired(vec!["repository.merge".to_owned()]),
        "{text}"
    );
    let id = id_of(&world.own_tree(case));
    grant(&world.repo, case, "repository.merge", "human:operator").expect("the grant is written");
    let (stop, text) = go(
        dispatch(world, delivery(case), &developer, &validators),
        world,
        Vec::new(),
    );
    assert_eq!(stop, Stop::Completed("accepted".to_owned()), "{text}");
    id
}

/// After dispatch, the registry holds one tree with purpose `takt:<case>` and a lease held by
/// the run. The child's `case.yaml` under the root names `parent` and that tree's path as
/// `worktree`.
#[test]
fn a_dispatched_child_gets_a_leased_tree() {
    let world = world("leased");
    let validators = green();
    let run = dispatch(&world, options(CASE), &Gate, &validators);
    let trees = world.trees();
    assert_eq!(trees.len(), 1, "{trees:#?}");
    let tree = world.tree_of(CASE);
    assert_eq!(tree["record"]["lifecycle"], "active", "{tree:#}");
    assert_eq!(tree["live_leases"], 1, "{tree:#}");
    assert_eq!(path_of(&tree), run.tree);
    let recorded = world.case_yaml(CASE);
    assert_eq!(recorded["parent"], PARENT);
    assert_eq!(
        recorded["worktree"].as_str().map(PathBuf::from),
        Some(path_of(&tree))
    );
    assert!(
        world.case_dir(CASE).join("case.yaml").is_file(),
        "the case is at the root"
    );
}

/// A child stopped by a failing setup command has an archive and no lease of the run's, and the
/// next dispatch of the case resumes it in the same tree, archiving and releasing again when it
/// stops again.
#[test]
fn a_stopped_child_is_archived_and_released() {
    let world = world("stopped");
    let validators = green();
    let (stop, text) = go(
        dispatch(&world, options(CASE), &Gate, &validators),
        &world,
        vec![command("deps", &["sh", "-c", "exit 3"])],
    );
    assert_eq!(
        stop,
        Stop::SetupFailed {
            name: "deps".to_owned(),
            exit: Some(3)
        },
        "{text}"
    );
    let tree = world.tree_of(CASE);
    let id = id_of(&tree);
    assert_eq!(tree["live_leases"], 0, "{tree:#}");
    assert_eq!(tree["record"]["lifecycle"], "active", "{tree:#}");
    assert!(world.archive(&id).is_file(), "no archive of {id}");
    assert!(path_of(&tree).is_dir());

    let resumed = dispatch(&world, options(CASE), &Gate, &validators);
    assert_eq!(resumed.tree, path_of(&tree), "another tree");
    assert_eq!(world.trees().len(), 1);
    let (stop, text) = go(resumed, &world, vec![command("deps", &["true"])]);
    assert_eq!(stop, Stop::Iterations(1), "{text}");
    let again = world.tree_of(CASE);
    assert_eq!(id_of(&again), id);
    assert_eq!(again["live_leases"], 0, "{again:#}");
    assert_eq!(again["record"]["lifecycle"], "active", "{again:#}");
    assert!(world.archive(&id).is_file());
}

/// After acceptance the tree is finished and holds no lease. The root's `.takt/cases/<id>/` holds
/// every record of the case, and the child's tree held none.
#[test]
fn a_terminal_child_is_finished_and_its_records_stay_at_the_root() {
    let world = world("terminal");
    let id = accepted(&world, CASE);
    let tree = world.tree_of(CASE);
    assert_eq!(id_of(&tree), id);
    assert_eq!(tree["record"]["lifecycle"], "finished", "{tree:#}");
    assert_eq!(tree["live_leases"], 0, "{tree:#}");
    let case_dir = world.case_dir(CASE);
    for record in [
        "case.yaml",
        "events.jsonl",
        "branch.yaml",
        "intent.md",
        "protocol.yaml",
        "authority.yaml",
    ] {
        assert!(
            case_dir.join(record).is_file(),
            "{record} is not at the root"
        );
    }
    for directory in ["evidence", "decisions"] {
        let held = fs::read_dir(case_dir.join(directory))
            .expect("a directory of records at the root")
            .count();
        assert!(held > 0, "{directory} holds no record at the root");
    }
    let last = world.events(CASE).pop().expect("a last event");
    assert_eq!(last["event"], "stop");
    let child = path_of(&tree);
    assert!(child.is_dir(), "finish removes nothing");
    for held in [".takt", ".engineering/cases"] {
        assert!(
            fs::symlink_metadata(child.join(held)).is_err(),
            "the child's tree holds {held}"
        );
    }
}

/// A setup command that writes its working directory records the child's tree, not the root.
#[test]
fn setup_runs_in_the_childs_tree() {
    let world = world("setup");
    let validators = green();
    let run = dispatch(&world, options(CASE), &Gate, &validators);
    let tree = run.tree.clone();
    let (stop, text) = go(run, &world, vec![command("where", &["pwd"])]);
    assert_eq!(stop, Stop::Iterations(1), "{text}");
    let kept = world
        .events(CASE)
        .into_iter()
        .find(|line| line["event"] == "setup")
        .and_then(|line| line["output"].as_str().map(str::to_owned))
        .expect("an output file named on the setup line");
    let written = fs::read_to_string(world.dir.join("state/transcripts").join(kept))
        .expect("the output file under the state directory");
    let recorded = fs::canonicalize(written.trim()).expect("a path that exists");
    assert_eq!(recorded, fs::canonicalize(&tree).expect("the tree"));
    assert_ne!(recorded, world.repo);
    assert_eq!(path_of(&world.tree_of(CASE)), tree);
}

/// A second dispatch of the same case reuses its tree: one tree in the registry, one lease, the
/// same path.
#[test]
fn enter_is_idempotent() {
    let world = world("idempotent");
    let validators = green();
    let first = dispatch(&world, options(CASE), &Gate, &validators);
    let path = first.tree.clone();
    let id = id_of(&world.tree_of(CASE));
    drop(first);
    let second = dispatch(&world, options(CASE), &Gate, &validators);
    assert_eq!(second.tree, path);
    assert_eq!(world.trees().len(), 1, "{:#?}", world.trees());
    let tree = world.tree_of(CASE);
    assert_eq!(id_of(&tree), id);
    assert_eq!(tree["live_leases"], 1, "{tree:#}");
    assert_eq!(
        world.case_yaml(CASE)["worktree"]
            .as_str()
            .map(PathBuf::from),
        Some(path)
    );
}

/// A tree made outside the seam, whose purpose even names the case, is unchanged by a full run:
/// the seam makes a tree of its own, leases, archives and finishes that one only.
#[test]
fn a_foreign_tree_is_never_touched() {
    let world = world("foreign");
    let made = world.worktree(&[
        "create",
        "--repo",
        world.repo.to_str().expect("utf-8"),
        "--purpose",
        &format!("takt:{CASE}"),
    ]);
    let foreign = PathBuf::from(made["evidence"]["path"].as_str().expect("a path"));
    fs::write(foreign.join("mine.txt"), "not takt's\n").expect("a file");
    let listing = |tree: &Path| {
        let mut names: Vec<String> = fs::read_dir(tree)
            .expect("the tree")
            .map(|entry| {
                entry
                    .expect("an entry")
                    .file_name()
                    .to_string_lossy()
                    .into_owned()
            })
            .collect();
        names.sort();
        names
    };
    let before = (
        git(&foreign, &["status", "--porcelain"]),
        git(&foreign, &["rev-parse", "HEAD"]),
        listing(&foreign),
    );
    let foreign_id = made["evidence"]["id"].as_str().expect("an id").to_owned();
    let record = |world: &World| {
        let mut found = world
            .trees()
            .into_iter()
            .filter(|tree| id_of(tree) == foreign_id)
            .collect::<Vec<_>>();
        assert_eq!(found.len(), 1);
        found.remove(0)
    };
    let registered = record(&world);

    let own = accepted(&world, CASE);
    assert_ne!(own, foreign_id);

    let after = (
        git(&foreign, &["status", "--porcelain"]),
        git(&foreign, &["rev-parse", "HEAD"]),
        listing(&foreign),
    );
    assert_eq!(before, after);
    assert_eq!(
        fs::read_to_string(foreign.join("mine.txt")).expect("still there"),
        "not takt's\n"
    );
    let now = record(&world);
    assert_eq!(
        now["record"], registered["record"],
        "the registry's record of the foreign tree moved"
    );
    assert_eq!(now["live_leases"], 0);
    assert_eq!(now["record"]["lifecycle"], "active");
    assert!(
        !world.archive(&foreign_id).exists(),
        "the foreign tree was archived"
    );
    let recorded = world.case_yaml(CASE);
    assert_ne!(
        recorded["worktree"].as_str().map(PathBuf::from),
        Some(foreign),
        "the case works in the foreign tree"
    );
    assert_eq!(world.trees_for(&format!("takt:{CASE}")).len(), 2);
}
