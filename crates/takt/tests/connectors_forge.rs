//! story:a-connectors-forge-adapter: `forge: connectors` goes through `connectors invoke
//! <connector>[/<instance>] <operation>` on the target the run file's `forge_instance` names. Its
//! preflight describes each operation it needs before any case opens, and the merge carries the
//! head the checks were read on. A double of `connectors` answers from a scenario and records
//! every argument list it is run with; nothing here reaches an instance, a credential or a host.

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
const TARGET: &str = "gitlab/forge";
const HEAD: &str = "0123456789abcdef0123456789abcdef01234567";
const OTHER: &str = "fedcba9876543210fedcba9876543210fedcba98";
const BRANCH: &str = "takt/case-1";

/// The words every line starts with.
const LEAD: [&str; 4] = ["connectors", "--output", "json", "invoke"];

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
    target: String,
    operations: Vec<String>,
    merge_request_head: String,
    head_pipeline: Option<(u64, String)>,
    pipeline_head: String,
    pipeline_status: String,
    /// Whether a write is refused `approval_required`, as `connectors invoke` refuses one
    /// without `--approval-file`.
    approval_required: bool,
    /// Whether the merge is refused `conflict`.
    conflict: bool,
}

impl Default for World {
    fn default() -> Self {
        Self {
            target: TARGET.to_owned(),
            operations: REQUIRED.iter().map(|name| (*name).to_owned()).collect(),
            merge_request_head: HEAD.to_owned(),
            head_pipeline: Some((9, HEAD.to_owned())),
            pipeline_head: HEAD.to_owned(),
            pipeline_status: "success".to_owned(),
            approval_required: false,
            conflict: false,
        }
    }
}

/// The double of `connectors`: records every argument list and answers as the CLI's JSON does.
#[derive(Clone, Default)]
struct Fake {
    calls: Rc<RefCell<Vec<Vec<String>>>>,
    world: Rc<RefCell<World>>,
}

fn exit(code: i32, stdout: &str, stderr: &str) -> Exit {
    Exit {
        code: Some(code),
        stdout: stdout.to_owned(),
        stderr: stderr.to_owned(),
    }
}

/// `connectors`' failure under `--output json`: nothing on standard output, the envelope on
/// standard error.
fn refusal(code: &str) -> Exit {
    exit(
        1,
        "",
        &json!({"ok": false, "error": {"code": code, "data": {}}}).to_string(),
    )
}

/// The value of `--name` in `argv`.
fn flag<'a>(argv: &'a [String], name: &str) -> Option<&'a str> {
    argv.iter()
        .position(|word| word == name)
        .and_then(|at| argv.get(at + 1))
        .map(String::as_str)
}

/// Whether `argv` asks for a description.
fn describes(argv: &[String]) -> bool {
    argv.iter().any(|word| word == "--describe")
}

/// The `connectors invoke … --describe` line of `operation` on `target`.
fn describe_line(target: &str, operation: &str) -> Vec<String> {
    LEAD.iter()
        .copied()
        .chain([target, operation, "--describe"])
        .map(str::to_owned)
        .collect()
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
            .filter(|argv| !describes(argv))
            .map(|argv| argv[5].clone())
            .collect()
    }

    /// The input of the one invocation of `operation`.
    fn input_of(&self, operation: &str) -> Value {
        let calls = self.calls();
        let call = calls
            .iter()
            .find(|argv| !describes(argv) && argv[5] == operation)
            .unwrap_or_else(|| panic!("{operation} was not invoked: {calls:?}"));
        serde_json::from_str(flag(call, "--input-json").expect("an input")).expect("JSON")
    }

    fn answer(&self, argv: &[String]) -> Exit {
        let world = self.world.borrow();
        let ok = |result: Value| exit(0, &json!({"ok": true, "result": result}).to_string(), "");
        let operation = argv[5].as_str();
        let (connector, instance) = world
            .target
            .split_once('/')
            .unwrap_or((&world.target, INSTANCE));
        if describes(argv) {
            if !world.operations.iter().any(|listed| listed == operation) {
                return refusal("not_found");
            }
            return ok(json!({
                "connector": connector,
                "instance": instance,
                "operation": operation,
                "description": "",
                "effect": if operation.ends_with(".get") { "read" } else { "write" },
                "revision": "descriptor-r1",
                "schema": format!("gitlab/{operation}/input/descriptor-r1"),
                "input_schema": {},
            }));
        }
        assert_eq!(flag(argv, "--revision"), Some("descriptor-r1"), "{argv:?}");
        if world.approval_required && !operation.ends_with(".get") {
            return refusal("approval_required");
        }
        let body = match operation {
            "merge_request.create" => json!({"iid": 17, "state": "opened"}),
            "merge_request.get" => json!({
                "iid": 17,
                "state": "opened",
                "source_branch": BRANCH,
                "target_branch": "main",
                "sha": world.merge_request_head,
                "head_pipeline": world.head_pipeline.as_ref().map(|(id, sha)| json!({
                    "id": id, "sha": sha, "status": "running"
                })),
            }),
            "pipeline.get" => json!({
                "id": 9,
                "sha": world.pipeline_head,
                "status": world.pipeline_status,
            }),
            "merge_request.merge" if world.conflict => return refusal("conflict"),
            "merge_request.merge" => json!({"iid": 17, "state": "merged"}),
            other => panic!("an operation the story does not use: {other:?}"),
        };
        ok(json!({
            "connector": connector,
            "instance": instance,
            "operation": operation,
            "result": {"status": 200, "body": body, "provenance": {}},
            "revision": "descriptor-r1",
        }))
    }
}

impl Runner for Fake {
    fn run(&self, _tree: &Path, argv: &[String]) -> Result<Exit> {
        self.calls.borrow_mut().push(argv.to_vec());
        assert_eq!(&argv[..4], LEAD, "{argv:?}");
        assert_eq!(argv[4], self.world.borrow().target, "{argv:?}");
        assert!(
            !argv
                .iter()
                .any(|word| word == "--instance" || word == "operations"),
            "{argv:?}"
        );
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
    let target = fake.world.borrow().target.clone();
    ConnectorsForge::for_tree(
        &repository(scratch),
        &target,
        None,
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

/// A fake describing read operations only gives an error before the case opens, naming the
/// create and merge operations, and nothing is invoked: the preflight is one
/// `connectors invoke <target> <operation> --describe` per operation.
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
    assert!(said.contains(TARGET), "{said}");
    assert_eq!(fake.invoked(), Vec::<String>::new());
    assert_eq!(
        fake.calls(),
        REQUIRED
            .iter()
            .map(|operation| describe_line(TARGET, operation))
            .collect::<Vec<_>>()
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

    let scratch = Scratch::new("preflight-none");
    let fake = Fake::default();
    fake.world.borrow_mut().operations.clear();
    let said = format!("{:#}", forge_over(&scratch, &fake).expect_err("refused"));
    assert!(said.contains("describes none of"), "{said}");
}

/// The preflight passes when every operation is described, and says which instance it found.
#[test]
fn preflight_prints_the_identity_of_the_instance() {
    let scratch = Scratch::new("identity");
    let forge = forge_over(&scratch, &Fake::default()).expect("a forge");
    let identity = forge.identity();
    assert!(identity.contains("gitlab"), "{identity}");
    assert!(identity.contains(INSTANCE), "{identity}");
    assert!(identity.contains("descriptor-r1"), "{identity}");
    assert_eq!(forge.name(), "connectors");
    assert_eq!(forge.route(), format!("connectors invoke {TARGET}"));
}

/// The fake records create, then read, on the named target, each with the schema and revision
/// the preflight described.
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
    let calls = fake.calls();
    let create = calls
        .iter()
        .find(|call| !describes(call) && call[5] == "merge_request.create")
        .expect("the create");
    assert_eq!(
        create[..8],
        [
            "connectors",
            "--output",
            "json",
            "invoke",
            TARGET,
            "merge_request.create",
            "--schema",
            "gitlab/merge_request.create/input/descriptor-r1",
        ]
    );
    assert_eq!(view.state, PullRequestState::Open);
    assert_eq!(view.head.sha, HEAD);
    assert_eq!(view.head.branch, BRANCH);
    assert_eq!(view.base, "main");
}

/// A target that names the connector alone leaves the instance to `connectors`, which takes the
/// connector's only one.
#[test]
fn the_instance_may_be_left_out() {
    let scratch = Scratch::new("connector-alone");
    let fake = Fake::default();
    fake.world.borrow_mut().target = "gitlab".to_owned();
    let forge = forge_over(&scratch, &fake).expect("a forge");
    forge
        .pull_request(&scratch.path().join("repo"), 17)
        .expect("reads");
    assert!(fake.calls().iter().all(|call| call[4] == "gitlab"));
}

/// A write `connectors` holds for an approval fails with its refusal, which names
/// `approval_required`, and nothing is opened.
#[test]
fn a_write_held_for_an_approval_fails_with_its_refusal() {
    let scratch = Scratch::new("approval");
    let fake = Fake::default();
    fake.world.borrow_mut().approval_required = true;
    let forge = forge_over(&scratch, &fake).expect("a forge");
    let said = format!(
        "{:#}",
        forge
            .open_pull_request(
                &scratch.path().join("repo"),
                &scratch.path().join("artifacts"),
                &request()
            )
            .expect_err("held for an approval")
    );
    assert!(said.contains("approval_required"), "{said}");
    assert!(said.contains("--approval-file"), "{said}");
    assert!(
        !said.contains("\"sha\""),
        "the input is never repeated: {said}"
    );
}

/// A merge refused `conflict` is no merge; the change request is read again before the next
/// write.
#[test]
fn a_merge_refused_as_a_conflict_is_read_again() {
    let scratch = Scratch::new("conflict");
    let fake = Fake::default();
    fake.world.borrow_mut().conflict = true;
    let forge = forge_over(&scratch, &fake).expect("a forge");
    let tree = scratch.path().join("repo");
    let artifacts = scratch.path().join("artifacts");
    assert_eq!(forge.checks(&tree, 17).expect("checks"), Checks::Pass);
    let said = format!(
        "{:#}",
        forge
            .merge(&tree, &artifacts, 17, HEAD)
            .expect_err("a conflict")
    );
    assert!(
        said.contains("409") && said.contains("nothing was merged"),
        "{said}"
    );
    fake.world.borrow_mut().conflict = false;
    forge.merge(&tree, &artifacts, 17, HEAD).expect("merges");
    assert_eq!(
        fake.invoked(),
        [
            "merge_request.get",
            "pipeline.get",
            "merge_request.merge",
            "merge_request.get",
            "merge_request.merge"
        ]
    );
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

    for forge in ["gitlab", "none"] {
        fs::write(&path, format!("forge: {forge}\n")).expect("a file");
        let config = RunConfig::read(&path).expect("reads");
        assert!(select(&config).expect("another forge").is_none(), "{forge}");
    }

    fs::write(&path, format!("forge_instance: {TARGET}\n")).expect("a file");
    let config = RunConfig::read(&path).expect("reads");
    let error = select(&config).expect_err("an instance with no forge: connectors");
    assert!(format!("{error:#}").contains("forge_instance"), "{error:#}");

    fs::write(&path, format!("forge: gitlab\nforge_instance: {TARGET}\n")).expect("a file");
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

    for word in [
        "",
        "-x",
        "a b",
        "a/b",
        "$(id)",
        "forge",
        "gitea/x",
        "gitlab/",
        "gitlab/-x",
        "github/a/b",
    ] {
        fs::write(
            &path,
            format!("forge: connectors\nforge_instance: '{word}'\n"),
        )
        .expect("a file");
        let said = format!("{:#}", RunConfig::read(&path).expect_err(word));
        assert!(
            said.contains("`forge_instance`") && said.contains("line 2"),
            "{word:?}: {said}"
        );
    }
}

/// With the key, the forge is the adapter, over the target the file names.
#[test]
fn the_run_file_names_the_instance() {
    let scratch = Scratch::new("named");
    let tree = repository(&scratch);
    let path = scratch.path().join("run.yaml");
    fs::write(
        &path,
        format!("forge: connectors\nforge_instance: {TARGET}\n"),
    )
    .expect("a file");
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
    assert_eq!(fake.calls().len(), REQUIRED.len());
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
