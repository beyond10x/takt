//! story:a-connectors-forge-adapter: `forge: connectors` goes through `connectors operations
//! invoke` on the instance the run file's `forge_instance` names. Its preflight lists the
//! instance's operations before any case opens, and the merge carries the head the checks were
//! read on. A double of `connectors` answers from a scenario and records every argument list it
//! is run with; nothing here reaches an instance, a credential or a host.

use std::cell::RefCell;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::rc::Rc;

use anyhow::Result;
use serde_json::{Value, json};
use takt::forge::connectors::{ConnectorsForge, REQUIRED};
use takt::forge::{Checks, Exit, Forge, PullRequest, PullRequestState, Runner};
use takt::run_config::{ForgeSettings, RunConfig};
use takt::scan::Scan;

const INSTANCE: &str = "forge";
const HEAD: &str = "0123456789abcdef0123456789abcdef01234567";
const OTHER: &str = "fedcba9876543210fedcba9876543210fedcba98";
const BRANCH: &str = "takt/case-1";

/// A scratch directory under this crate's build directory, removed on drop.
struct Scratch(PathBuf);

impl Scratch {
    fn new(name: &str) -> Self {
        let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
            .join(format!("connectors-forge-{}-{name}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("a scratch directory");
        Self(dir)
    }

    fn path(&self) -> &Path {
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
    assert!(output.status.success(), "git {args:?}");
    String::from_utf8_lossy(&output.stdout).trim().to_owned()
}

/// A repository whose `origin` names a project on a host that does not exist, with one commit on
/// the case's branch.
fn repository(scratch: &Scratch) -> PathBuf {
    let repo = scratch.path().join("repo");
    fs::create_dir_all(&repo).expect("a directory");
    git(&repo, &["init", "-q", "-b", "main"]);
    fs::write(repo.join("README.md"), "a story\n").expect("a file");
    git(&repo, &["add", "-A"]);
    git(&repo, &["commit", "-q", "-m", "start"]);
    git(&repo, &["checkout", "-q", "-b", BRANCH]);
    git(
        &repo,
        &[
            "remote",
            "add",
            "origin",
            "https://forge.example/group/project.git",
        ],
    );
    repo
}

/// What the double of `connectors` reports.
struct World {
    operations: Vec<String>,
    merge_request_head: String,
    head_pipeline: Option<(u64, String)>,
    pipeline_head: String,
    pipeline_status: String,
}

impl Default for World {
    fn default() -> Self {
        Self {
            operations: REQUIRED.iter().map(|name| (*name).to_owned()).collect(),
            merge_request_head: HEAD.to_owned(),
            head_pipeline: Some((9, HEAD.to_owned())),
            pipeline_head: HEAD.to_owned(),
            pipeline_status: "success".to_owned(),
        }
    }
}

/// The double of `connectors`: records every argument list and answers as the CLI's JSON does.
#[derive(Clone, Default)]
struct Fake {
    calls: Rc<RefCell<Vec<Vec<String>>>>,
    world: Rc<RefCell<World>>,
}

fn exit(code: i32, stdout: &Value, stderr: &str) -> Exit {
    Exit {
        code: Some(code),
        stdout: stdout.to_string(),
        stderr: stderr.to_owned(),
    }
}

/// The value of `--name` in `argv`.
fn flag<'a>(argv: &'a [String], name: &str) -> Option<&'a str> {
    argv.iter()
        .position(|word| word == name)
        .and_then(|at| argv.get(at + 1))
        .map(String::as_str)
}

impl Fake {
    fn reading_only() -> Self {
        let fake = Self::default();
        fake.world
            .borrow_mut()
            .operations
            .retain(|name| !name.ends_with(".create") && !name.ends_with(".merge"));
        fake
    }

    fn calls(&self) -> Vec<Vec<String>> {
        self.calls.borrow().clone()
    }

    /// The invocations, in order, as the operation each names.
    fn invoked(&self) -> Vec<String> {
        self.calls()
            .iter()
            .filter(|argv| argv.get(2).map(String::as_str) == Some("invoke"))
            .filter_map(|argv| flag(argv, "--operation").map(str::to_owned))
            .collect()
    }

    /// The input of the one invocation of `operation`.
    fn input_of(&self, operation: &str) -> Value {
        let calls = self.calls();
        let call = calls
            .iter()
            .find(|argv| {
                argv.get(2).map(String::as_str) == Some("invoke")
                    && flag(argv, "--operation") == Some(operation)
            })
            .unwrap_or_else(|| panic!("{operation} was not invoked: {calls:?}"));
        serde_json::from_str(flag(call, "--input-json").expect("an input")).expect("JSON")
    }

    fn answer(&self, argv: &[String]) -> Exit {
        let world = self.world.borrow();
        let ok = |result: Value| exit(0, &json!({"ok": true, "result": result}), "");
        match argv.get(2).map(String::as_str) {
            Some("list") => ok(json!({
                "adapter": INSTANCE,
                "connector": "gitlab",
                "instance": flag(argv, "--instance"),
                "revision": "descriptor-r1",
                "operations": world.operations.iter().map(|id| json!({
                    "id": id, "description": "", "contract": "operations/v1alpha1", "profile": "token"
                })).collect::<Vec<_>>(),
                "source": "cached",
                "stale": false,
            })),
            Some("describe") => ok(json!({
                "connector": "gitlab",
                "instance": flag(argv, "--instance"),
                "revision": "descriptor-r1",
                "schema": format!("gitlab/{}/input/descriptor-r1", flag(argv, "--operation").unwrap_or("")),
                "operation": {"id": flag(argv, "--operation")},
                "source": "cached",
                "stale": false,
            })),
            Some("invoke") => {
                let body = match flag(argv, "--operation") {
                    Some("merge_request.create") => json!({"iid": 17, "state": "opened"}),
                    Some("merge_request.get") => json!({
                        "iid": 17,
                        "state": "opened",
                        "source_branch": BRANCH,
                        "target_branch": "main",
                        "sha": world.merge_request_head,
                        "head_pipeline": world.head_pipeline.as_ref().map(|(id, sha)| json!({
                            "id": id, "sha": sha, "status": "running"
                        })),
                    }),
                    Some("pipeline.get") => json!({
                        "id": 9,
                        "sha": world.pipeline_head,
                        "status": world.pipeline_status,
                    }),
                    Some("merge_request.merge") => json!({"iid": 17, "state": "merged"}),
                    other => panic!("an operation the story does not use: {other:?}"),
                };
                ok(json!({
                    "operation": flag(argv, "--operation"),
                    "result": {"status": 200, "body": body, "provenance": {}},
                    "revision": "descriptor-r1",
                }))
            }
            other => panic!("a subcommand the adapter has no use for: {other:?}"),
        }
    }
}

impl Runner for Fake {
    fn run(&self, _tree: &Path, argv: &[String]) -> Result<Exit> {
        self.calls.borrow_mut().push(argv.to_vec());
        assert_eq!(&argv[..2], ["connectors", "operations"], "{argv:?}");
        assert_eq!(flag(argv, "--instance"), Some(INSTANCE), "{argv:?}");
        assert_eq!(flag(argv, "--output"), Some("json"), "{argv:?}");
        Ok(self.answer(argv))
    }

    fn run_env(&self, tree: &Path, argv: &[String], _env: &[(&str, &str)]) -> Result<Exit> {
        self.run(tree, argv)
    }
}

/// A runner that no line may reach.
struct Never;

impl Runner for Never {
    fn run(&self, _tree: &Path, argv: &[String]) -> Result<Exit> {
        panic!("a line was run: {argv:?}");
    }

    fn run_env(&self, _tree: &Path, argv: &[String], _env: &[(&str, &str)]) -> Result<Exit> {
        panic!("a line was run: {argv:?}");
    }
}

fn forge_over(scratch: &Scratch, fake: &Fake) -> Result<ConnectorsForge> {
    ConnectorsForge::for_tree(
        &repository(scratch),
        INSTANCE,
        Scan::None {
            declared_in: scratch.path().join("run.yaml"),
        },
        Box::new(fake.clone()),
    )
}

fn request() -> PullRequest {
    PullRequest {
        branch: BRANCH.to_owned(),
        base: "main".to_owned(),
        title: "A title".to_owned(),
        body: "the body".to_owned(),
    }
}

/// A fake listing read operations only gives an error before the case opens, naming the create
/// operation, and nothing is invoked.
#[test]
fn preflight_names_the_missing_write() {
    let scratch = Scratch::new("preflight");
    let fake = Fake::reading_only();
    let error = forge_over(&scratch, &fake).expect_err("refused at open");
    let said = format!("{error:#}");
    assert!(said.contains("merge_request.create"), "{said}");
    assert!(said.contains("merge_request.merge"), "{said}");
    assert!(!said.contains("merge_request.get"), "{said}");
    assert!(!said.contains("pipeline.get"), "{said}");
    assert!(said.contains(INSTANCE), "{said}");
    assert_eq!(fake.invoked(), Vec::<String>::new());
    let calls = fake.calls();
    assert_eq!(calls.len(), 1, "{calls:?}");
    assert_eq!(
        calls[0],
        [
            "connectors",
            "operations",
            "list",
            "--instance",
            INSTANCE,
            "--output",
            "json"
        ]
    );

    let scratch = Scratch::new("preflight-merge");
    let fake = Fake::default();
    fake.world
        .borrow_mut()
        .operations
        .retain(|name| name != "pipeline.get");
    let said = format!("{:#}", forge_over(&scratch, &fake).expect_err("refused"));
    assert!(said.contains("pipeline.get"), "{said}");
    assert!(!said.contains("merge_request.create"), "{said}");
}

/// The preflight passes when every operation is listed, and says which instance it found.
#[test]
fn preflight_prints_the_identity_of_the_instance() {
    let scratch = Scratch::new("identity");
    let forge = forge_over(&scratch, &Fake::default()).expect("a forge");
    let identity = forge.identity();
    assert!(identity.contains("gitlab"), "{identity}");
    assert!(identity.contains(INSTANCE), "{identity}");
    assert!(identity.contains("descriptor-r1"), "{identity}");
    assert_eq!(forge.name(), "connectors");
}

/// The fake records create, then read, with the named instance.
#[test]
fn a_run_opens_and_reads_its_change_request() {
    let scratch = Scratch::new("opens");
    let fake = Fake::default();
    let forge = forge_over(&scratch, &fake).expect("a forge");
    let tree = scratch.path().join("repo");
    let artifacts = scratch.path().join("artifacts");
    let number = forge
        .open_pull_request(&tree, &artifacts, &request())
        .expect("opens");
    assert_eq!(number, 17);
    let view = forge.pull_request(&tree, number).expect("reads");
    assert_eq!(
        fake.invoked(),
        ["merge_request.create", "merge_request.get"]
    );
    for call in fake.calls() {
        assert_eq!(flag(&call, "--instance"), Some(INSTANCE), "{call:?}");
    }
    let branch_head = git(&tree, &["rev-parse", &format!("refs/heads/{BRANCH}")]);
    assert_eq!(
        fake.input_of("merge_request.create"),
        json!({
            "id": "group/project",
            "sha": branch_head,
            "body": {
                "source_branch": BRANCH,
                "target_branch": "main",
                "title": "A title",
                "description": "the body",
            },
        })
    );
    assert_eq!(
        fake.input_of("merge_request.get"),
        json!({"id": "group/project", "merge_request_iid": 17})
    );
    let schema = fake
        .calls()
        .iter()
        .find(|call| {
            flag(call, "--operation") == Some("merge_request.create") && call[2] == "invoke"
        })
        .and_then(|call| flag(call, "--schema").map(str::to_owned))
        .expect("a schema");
    assert_eq!(schema, "gitlab/merge_request.create/input/descriptor-r1");
    assert_eq!(view.state, PullRequestState::Open);
    assert_eq!(view.head.sha, HEAD);
    assert_eq!(view.head.branch, BRANCH);
    assert_eq!(view.base, "main");
}

/// A run file without `forge: connectors` never invokes `connectors`: no other forge, and no
/// forge at all, builds the adapter or runs a line; a `forge_instance` under another forge is
/// refused, and `forge: connectors` needs it.
#[test]
fn it_is_never_the_default() {
    let scratch = Scratch::new("never");
    let tree = repository(&scratch);
    let path = scratch.path().join("run.yaml");
    let select = |config: &RunConfig| {
        ConnectorsForge::select(
            &tree,
            config.forge.as_deref(),
            &config.forge_settings(),
            Scan::None {
                declared_in: path.clone(),
            },
            Box::new(Never),
        )
    };

    fs::write(&path, "iterations: 1\n").expect("a file");
    let config = RunConfig::read(&path).expect("reads");
    assert_eq!(config.forge, None);
    assert!(select(&config).expect("no forge named").is_none());

    for forge in ["bot", "gitlab", "none"] {
        fs::write(&path, format!("forge: {forge}\n")).expect("a file");
        let config = RunConfig::read(&path).expect("reads");
        assert!(select(&config).expect("another forge").is_none(), "{forge}");
    }

    fs::write(&path, "forge_instance: forge\n").expect("a file");
    let config = RunConfig::read(&path).expect("reads");
    let error = select(&config).expect_err("an instance with no forge: connectors");
    assert!(format!("{error:#}").contains("forge_instance"), "{error:#}");

    fs::write(&path, "forge: bot\nforge_instance: forge\n").expect("a file");
    let config = RunConfig::read(&path).expect("reads");
    assert!(select(&config).is_err());

    fs::write(&path, "forge: connectors\n").expect("a file");
    let config = RunConfig::read(&path).expect("reads");
    let error = select(&config).expect_err("an adapter with no instance");
    assert!(format!("{error:#}").contains("forge_instance"), "{error:#}");

    assert!(
        ForgeSettings::default().check_for(None).is_ok(),
        "no key, no forge"
    );

    for word in ["", "-x", "a b", "a/b", "$(id)"] {
        fs::write(
            &path,
            format!("forge: connectors\nforge_instance: '{word}'\n"),
        )
        .expect("a file");
        assert!(RunConfig::read(&path).is_err(), "{word:?}");
    }
}

/// With the key, the forge is the adapter, over the instance the file names.
#[test]
fn the_run_file_names_the_instance() {
    let scratch = Scratch::new("named");
    let tree = repository(&scratch);
    let path = scratch.path().join("run.yaml");
    fs::write(&path, "forge: connectors\nforge_instance: forge\n").expect("a file");
    let config = RunConfig::read(&path).expect("reads");
    let fake = Fake::default();
    let forge = ConnectorsForge::select(
        &tree,
        config.forge.as_deref(),
        &config.forge_settings(),
        Scan::None { declared_in: path },
        Box::new(fake.clone()),
    )
    .expect("selected")
    .expect("the adapter");
    assert_eq!(forge.name(), "connectors");
    assert_eq!(fake.calls().len(), 1);
}

/// The fake reports the pipeline of the head, and checks read it; the merge invocation then
/// carries that head as `body.sha`, and the pipeline the checks were read on.
#[test]
fn merge_carries_the_head() {
    let scratch = Scratch::new("merge");
    let fake = Fake::default();
    let forge = forge_over(&scratch, &fake).expect("a forge");
    let tree = scratch.path().join("repo");
    let view = forge.pull_request(&tree, 17).expect("reads");
    assert_eq!(forge.checks(&tree, 17).expect("checks"), Checks::Pass);
    forge
        .merge(&tree, &scratch.path().join("artifacts"), 17, &view.head.sha)
        .expect("merges");
    assert_eq!(
        fake.invoked(),
        [
            "merge_request.get",
            "merge_request.get",
            "pipeline.get",
            "merge_request.merge"
        ]
    );
    assert_eq!(
        fake.input_of("merge_request.merge"),
        json!({
            "id": "group/project",
            "merge_request_iid": 17,
            "pipeline_id": 9,
            "body": {"sha": HEAD},
        })
    );
}

/// A pipeline the fake reports for another head leaves the checks pending, and no merge is
/// invoked; the pipeline of the merge request's own head is the one read.
#[test]
fn checks_for_another_head_are_not_read() {
    let scratch = Scratch::new("another");
    let fake = Fake::default();
    fake.world.borrow_mut().pipeline_head = OTHER.to_owned();
    let forge = forge_over(&scratch, &fake).expect("a forge");
    let tree = scratch.path().join("repo");
    assert_eq!(forge.checks(&tree, 17).expect("checks"), Checks::Pending);
    let error = forge
        .merge(&tree, &scratch.path().join("artifacts"), 17, HEAD)
        .expect_err("no checks on the head were read");
    assert!(format!("{error:#}").contains(HEAD), "{error:#}");
    assert!(
        !fake.invoked().contains(&"merge_request.merge".to_owned()),
        "{:?}",
        fake.invoked()
    );

    let scratch = Scratch::new("another-listed");
    let fake = Fake::default();
    fake.world.borrow_mut().head_pipeline = Some((9, OTHER.to_owned()));
    let forge = forge_over(&scratch, &fake).expect("a forge");
    let tree = scratch.path().join("repo");
    assert_eq!(forge.checks(&tree, 17).expect("checks"), Checks::Pending);
    assert_eq!(
        fake.invoked(),
        ["merge_request.get"],
        "a pipeline of another head is not read"
    );
    assert!(
        forge
            .merge(&tree, &scratch.path().join("artifacts"), 17, HEAD)
            .is_err()
    );
    assert!(!fake.invoked().contains(&"merge_request.merge".to_owned()));

    let scratch = Scratch::new("none-yet");
    let fake = Fake::default();
    fake.world.borrow_mut().head_pipeline = None;
    let forge = forge_over(&scratch, &fake).expect("a forge");
    assert_eq!(
        forge
            .checks(&scratch.path().join("repo"), 17)
            .expect("checks"),
        Checks::Pending
    );
}

/// A head that moved since the checks were read is refused, not merged.
#[test]
fn a_head_that_moved_is_not_merged() {
    let scratch = Scratch::new("moved");
    let fake = Fake::default();
    let forge = forge_over(&scratch, &fake).expect("a forge");
    let tree = scratch.path().join("repo");
    assert_eq!(forge.checks(&tree, 17).expect("checks"), Checks::Pass);
    fake.world.borrow_mut().merge_request_head = OTHER.to_owned();
    assert!(
        forge
            .merge(&tree, &scratch.path().join("artifacts"), 17, OTHER)
            .is_err(),
        "the checks were read on another head"
    );
    assert!(!fake.invoked().contains(&"merge_request.merge".to_owned()));
}

/// The pipeline's status is the checks': a failed one fails, a running one is pending.
#[test]
fn the_pipelines_status_is_the_checks() {
    for (status, expected) in [
        ("success", Checks::Pass),
        ("failed", Checks::Fail),
        ("canceled", Checks::Fail),
        ("running", Checks::Pending),
        ("pending", Checks::Pending),
    ] {
        let scratch = Scratch::new(&format!("status-{status}"));
        let fake = Fake::default();
        fake.world.borrow_mut().pipeline_status = status.to_owned();
        let forge = forge_over(&scratch, &fake).expect("a forge");
        assert_eq!(
            forge
                .checks(&scratch.path().join("repo"), 17)
                .expect("checks"),
            expected,
            "{status}"
        );
    }
}
