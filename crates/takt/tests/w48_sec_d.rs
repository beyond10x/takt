//! Conformance of the read-back after a merge answered `outcome_unknown`: a merge is sent again
//! only while the pull request reads open and unmerged at exactly the head the first merge named,
//! a merged read-back is the merge applied only when it is that merge, the fresh key is derived
//! deterministically and recorded before it is sent, and a record whose `unknown` member does not
//! conform is an error with nothing sent. A loopback stand-in of a served Connectors instance with
//! standing grants, for the github and the gitlab connector.
#![allow(dead_code)]

#[path = "support/own_home.rs"]
mod own_home;

use std::collections::BTreeMap;
use std::fs;
use std::io::{BufRead as _, BufReader, Read as _, Write as _};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Arc, Mutex};

use serde_json::{Value, json};

use takt::forge::connectors::{ConnectorsForge, idempotency_key};
use takt::forge::{Exit, Forge, Runner};
use takt::scan::Scan;

/// The descriptor revision the stand-in describes.
const REVISION: &str = "descriptor-r1";

/// The pull request every case merges.
const NUMBER: u64 = 7;

/// The pipeline of the merge request's head on gitlab.
const PIPELINE: u64 = 41;

/// A head the pull request is at when it is not at the case branch's commit.
const OTHER: &str = "fedcba9876543210fedcba9876543210fedcba98";

/// The case branch.
const BRANCH: &str = "takt/case-1";

const DESCRIBE: &str = "/v1/describe";
const INVOKE: &str = "/v1alpha2/invoke";

const GITHUB: [&str; 6] = [
    "branch.get",
    "pull_request.create",
    "pull_request.get",
    "pull_request.merge",
    "check_runs.list",
    "commit.status",
];

const GITLAB: [&str; 4] = [
    "merge_request.create",
    "merge_request.get",
    "merge_request.merge",
    "pipeline.get",
];

fn git(tree: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .args(["-c", "user.name=t", "-c", "user.email=t@localhost"])
        .env("GIT_CEILING_DIRECTORIES", env!("CARGO_TARGET_TMPDIR"))
        .args(args)
        .current_dir(tree)
        .output()
        .expect("git runs");
    assert!(
        output.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout)
        .expect("utf-8")
        .trim()
        .to_owned()
}

/// A scratch directory under this crate's build directory, removed when dropped.
struct Scratch(PathBuf);

impl Scratch {
    fn new(name: &str) -> Self {
        static MADE: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let made = MADE.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
            .join(format!("w48-sec-d-{}-{made}-{name}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("a scratch directory");
        Self(dir)
    }
}

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

/// A change made to the world at a given read of the pull request.
type Change = fn(&mut World);

/// What the stand-in holds and reports.
struct World {
    /// `github` or `gitlab`.
    connector: &'static str,
    /// The pull request's state as the provider spells it (`open`/`closed` on github,
    /// `opened`/`locked`/`merged`/`closed` on gitlab).
    state: String,
    /// github's `merged`.
    merged: Value,
    /// The pull request's head.
    head: String,
    /// Whether a read of the pull request answers a provider error.
    read_fails: bool,
    /// How many of the next keyed merges are answered `outcome_unknown` and stored under their
    /// key without reaching the provider.
    lost: usize,
    /// Each key held, with the request it was used for and the stored answer.
    keys: BTreeMap<String, (Value, (u16, Value))>,
    /// Each merge invocation received, in order: its key, and what the case's refusal record
    /// held on disk at that moment.
    merges: Vec<(Option<String>, Option<String>)>,
    /// How many merges reached the provider.
    applied: usize,
    /// The case's refusal record, read at every merge invocation.
    record: Option<PathBuf>,
    /// How many reads of the pull request answer before `change` is made to the world, and the
    /// change: a world that changes between the read before the merge and the read-back.
    at_read: Option<(usize, Change)>,
    /// How many reads of the pull request were answered.
    reads: usize,
}

impl World {
    fn new(connector: &'static str, head: &str) -> Self {
        let state = if connector == "github" {
            "open"
        } else {
            "opened"
        };
        Self {
            connector,
            state: state.to_owned(),
            merged: json!(false),
            head: head.to_owned(),
            read_fails: false,
            lost: 0,
            keys: BTreeMap::new(),
            merges: Vec::new(),
            applied: 0,
            record: None,
            at_read: None,
            reads: 0,
        }
    }

    fn operations(&self) -> &'static [&'static str] {
        if self.connector == "github" {
            &GITHUB
        } else {
            &GITLAB
        }
    }
}

fn success(request_id: &Value, status: u16, body: Value) -> (u16, Value) {
    (
        200,
        json!({
            "version": "v1alpha2",
            "request_id": request_id,
            "status": "success",
            "result": {"status": status, "body": body, "provenance": {}},
            "audit_ref": "audit-1",
            "audit_status": "complete",
        }),
    )
}

fn failure(request_id: &Value, status: u16, code: &str, message: &str) -> (u16, Value) {
    (
        status,
        json!({
            "version": "v1alpha2",
            "request_id": request_id,
            "status": "error",
            "error": {"code": code, "message": message},
            "audit_ref": "audit-2",
            "audit_status": "complete",
        }),
    )
}

fn is_merge(operation: &str) -> bool {
    matches!(operation, "pull_request.merge" | "merge_request.merge")
}

fn invoke(envelope: &Value, world: &mut World) -> (u16, Value) {
    let id = envelope["request_id"].clone();
    let operation = envelope["operation"]
        .as_str()
        .unwrap_or_default()
        .to_owned();
    let key = envelope["idempotency_key"].as_str().map(str::to_owned);
    if is_merge(&operation) {
        let held = world
            .record
            .as_ref()
            .and_then(|path| fs::read_to_string(path).ok());
        world.merges.push((key.clone(), held));
    }
    if !is_merge(&operation) && !operation.ends_with(".create") {
        if key.is_some() {
            return failure(&id, 400, "unsupported", "idempotency_key on a read");
        }
        return dispatch(&operation, &id, &envelope["input"], world);
    }
    let Some(key) = key else {
        return failure(&id, 400, "invalid_input", "a write needs idempotency_key");
    };
    let request = json!([operation, envelope["revision"], envelope["input"]]);
    if let Some((held, (status, answer))) = world.keys.get(&key) {
        if *held != request {
            return failure(
                &id,
                409,
                "idempotency_conflict",
                "the key was used for another request",
            );
        }
        let mut answer = answer.clone();
        answer["request_id"] = id;
        answer["mutation"]["replayed"] = json!(true);
        return (*status, answer);
    }
    if world.lost > 0 {
        world.lost -= 1;
        let (status, mut answer) = failure(&id, 502, "outcome_unknown", "the answer was lost");
        answer["mutation"] = json!({
            "classification": "unknown",
            "attempt": {"instance": world.connector, "id": "lost"},
            "original_request_id": id,
            "replayed": false,
        });
        world.keys.insert(key, (request, (status, answer.clone())));
        return (status, answer);
    }
    let (status, mut answer) = dispatch(&operation, &id, &envelope["input"], world);
    let classification = if answer["status"] == "success" {
        "applied"
    } else {
        "refused"
    };
    answer["mutation"] = json!({
        "classification": classification,
        "attempt": {"instance": world.connector, "id": format!("attempt-{}", world.keys.len())},
        "original_request_id": id,
        "replayed": false,
    });
    world.keys.insert(key, (request, (status, answer.clone())));
    (status, answer)
}

fn dispatch(operation: &str, id: &Value, input: &Value, world: &mut World) -> (u16, Value) {
    if matches!(operation, "pull_request.get" | "merge_request.get") {
        if let Some((at, change)) = world.at_read
            && world.reads == at
        {
            world.at_read = None;
            change(world);
        }
        world.reads += 1;
    }
    match operation {
        "pull_request.get" | "merge_request.get" if world.read_fails => failure(
            id,
            503,
            "provider_unavailable",
            "the provider did not answer the read",
        ),
        "pull_request.get" => success(
            id,
            200,
            json!({
                "number": NUMBER,
                "state": world.state,
                "merged": world.merged,
                "head": {"ref": BRANCH, "sha": world.head},
                "base": {"ref": "main"},
            }),
        ),
        "merge_request.get" => success(
            id,
            200,
            json!({
                "iid": NUMBER,
                "state": world.state,
                "sha": world.head,
                "source_branch": BRANCH,
                "target_branch": "main",
                "head_pipeline": {"id": PIPELINE, "sha": world.head},
            }),
        ),
        "pipeline.get" => success(
            id,
            200,
            json!({"id": PIPELINE, "sha": world.head, "status": "success"}),
        ),
        "pull_request.merge" => {
            if world.state != "open" || input["body"]["sha"] != world.head.as_str() {
                return failure(id, 405, "conflict", "the pull request is not mergeable");
            }
            world.applied += 1;
            world.state = "closed".to_owned();
            world.merged = json!(true);
            success(
                id,
                200,
                json!({"sha": "a-merge-commit", "merged": true, "message": "merged"}),
            )
        }
        "merge_request.merge" => {
            if world.state != "opened" || input["body"]["sha"] != world.head.as_str() {
                return failure(id, 405, "conflict", "the merge request is not mergeable");
            }
            world.applied += 1;
            world.state = "merged".to_owned();
            success(id, 200, json!({"iid": NUMBER, "state": "merged"}))
        }
        _ => failure(id, 404, "not_found", "no such operation"),
    }
}

fn answer(method: &str, path: &str, body: &Value, world: &mut World) -> (u16, Value) {
    match (method, path) {
        ("GET", DESCRIBE) => (
            200,
            json!({
                "version": "v1alpha1",
                "instance": world.connector,
                "adapter": "catalog",
                "revision": REVISION,
                "operations": world.operations().iter().map(|id| json!({
                    "id": id,
                    "description": "",
                    "contract": "operations/v1alpha1",
                    "profile": "app",
                    "input_schema": {},
                    "output_schema": {},
                })).collect::<Vec<_>>(),
                "configuration_schema": {"type": "object"},
            }),
        ),
        ("POST", INVOKE) => invoke(body, world),
        _ => (
            404,
            json!({"code": "not_found", "message": "no such route"}),
        ),
    }
}

fn serve(stream: TcpStream, world: &Mutex<World>) {
    let mut reader = BufReader::new(stream.try_clone().expect("the stream"));
    let mut line = String::new();
    if reader.read_line(&mut line).is_err() {
        return;
    }
    let mut words = line.split_whitespace();
    let method = words.next().unwrap_or_default().to_owned();
    let path = words.next().unwrap_or_default().to_owned();
    let mut length = 0;
    loop {
        let mut header = String::new();
        if reader.read_line(&mut header).is_err() {
            return;
        }
        let header = header.trim_end();
        if header.is_empty() {
            break;
        }
        if let Some((name, value)) = header.split_once(':')
            && name.trim().eq_ignore_ascii_case("content-length")
        {
            length = value.trim().parse().unwrap_or(0);
        }
    }
    let mut bytes = vec![0; length];
    if reader.read_exact(&mut bytes).is_err() {
        return;
    }
    let body: Value = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
    let (status, answer) = answer(&method, &path, &body, &mut world.lock().expect("the world"));
    let text = answer.to_string();
    let mut stream = stream;
    let _ = write!(
        stream,
        "HTTP/1.1 {status} Answer\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{text}",
        text.len()
    );
}

struct StandIn {
    url: String,
    world: Arc<Mutex<World>>,
}

impl StandIn {
    fn start(world: World) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("a loopback port");
        let url = format!("http://{}", listener.local_addr().expect("an address"));
        let world = Arc::new(Mutex::new(world));
        let held = world.clone();
        std::thread::spawn(move || {
            for stream in listener.incoming().flatten() {
                serve(stream, &held);
            }
        });
        Self { url, world }
    }

    fn with(&self, change: impl FnOnce(&mut World)) {
        change(&mut self.world.lock().expect("the world"));
    }

    fn merges(&self) -> Vec<(Option<String>, Option<String>)> {
        self.world.lock().expect("the world").merges.clone()
    }

    fn applied(&self) -> usize {
        self.world.lock().expect("the world").applied
    }
}

/// A runner every line succeeds on, with nothing printed; no merge reaches it.
struct Quiet;

impl Runner for Quiet {
    fn run(&self, tree: &Path, argv: &[String]) -> anyhow::Result<Exit> {
        self.run_env(tree, argv, &[])
    }

    fn run_env(
        &self,
        _tree: &Path,
        _argv: &[String],
        _env: &[(&str, &str)],
    ) -> anyhow::Result<Exit> {
        Ok(Exit {
            code: Some(0),
            stdout: String::new(),
            stderr: String::new(),
        })
    }
}

/// A repository whose `origin` names `owner/repo`, with the case branch one commit above
/// `main`: the tree and the case branch's commit.
fn repository(scratch: &Scratch) -> (PathBuf, String) {
    let tree = scratch.join("tree");
    fs::create_dir_all(&tree).expect("a tree");
    fs::write(tree.join("README.md"), "a repository\n").expect("a file");
    git(&tree, &["init", "-q", "-b", "main"]);
    git(&tree, &["add", "-A"]);
    git(&tree, &["commit", "-q", "-m", "start"]);
    git(
        &tree,
        &[
            "remote",
            "add",
            "origin",
            "https://forge.example/owner/repo.git",
        ],
    );
    git(&tree, &["checkout", "-q", "-b", BRANCH]);
    fs::write(tree.join("README.md"), "a repository, changed\n").expect("a file");
    git(&tree, &["commit", "-q", "-am", "change"]);
    let head = git(&tree, &["rev-parse", "HEAD"]);
    git(&tree, &["checkout", "-q", "main"]);
    (tree, head)
}

fn forge(scratch: &Scratch, tree: &Path, connector: &str, stand_in: &StandIn) -> ConnectorsForge {
    ConnectorsForge::for_tree(
        tree,
        connector,
        Some(&stand_in.url),
        Scan::None {
            declared_in: scratch.join("run.yaml"),
        },
        Box::new(Quiet),
    )
    .expect("the forge is selected")
}

/// The case `case`'s artifacts directory under `scratch`.
fn artifacts_of(scratch: &Scratch) -> PathBuf {
    scratch.join("cases").join("case").join("artifacts")
}

/// A case whose first merge of pull request [`NUMBER`] at its head was answered
/// `outcome_unknown` and never reached the provider, in a process that has ended: the scratch,
/// the stand-in, the tree, the head and the case's artifacts directory.
fn after_a_lost_merge(
    name: &str,
    connector: &'static str,
) -> (Scratch, StandIn, PathBuf, String, PathBuf) {
    let scratch = Scratch::new(name);
    let (tree, head) = repository(&scratch);
    let stand_in = StandIn::start(World::new(connector, &head));
    let case = artifacts_of(&scratch);
    stand_in.with(|world| {
        world.lost = 1;
        world.record = Some(case.join(ConnectorsForge::REFUSALS));
    });
    let first = forge(&scratch, &tree, connector, &stand_in);
    let error = first
        .merge(&tree, &case, NUMBER, &head)
        .expect_err("outcome_unknown is no merge");
    assert!(
        format!("{error:#}").contains("outcome_unknown"),
        "{error:#}"
    );
    drop(first);
    assert_eq!(
        stand_in.merges().len(),
        1,
        "one merge sent before the resume"
    );
    (scratch, stand_in, tree, head, case)
}

/// Makes `change` to the world at the resumed merge's read-back of the pull request, after the
/// reads the merge makes before its write (on gitlab, the read for the pipeline).
fn at_read_back(stand_in: &StandIn, connector: &str, change: fn(&mut World)) {
    let before = usize::from(connector == "gitlab");
    stand_in.with(|world| world.at_read = Some((world.reads + before, change)));
}

/// A merge request on gitlab that reads `locked` after its merge was answered `outcome_unknown`
/// is a merge GitLab is still making, not an open, unmerged one: the resumed merge sends nothing.
#[test]
fn a_gitlab_merge_request_read_locked_after_outcome_unknown_sends_no_merge() {
    let (scratch, stand_in, tree, head, case) = after_a_lost_merge("gitlab-locked", "gitlab");
    stand_in.with(|world| world.state = "locked".to_owned());
    let resumed = forge(&scratch, &tree, "gitlab", &stand_in);
    let outcome = resumed.merge(&tree, &case, NUMBER, &head);
    let merges = stand_in.merges();
    assert_eq!(
        merges.len(),
        1,
        "a merge was sent while the merge request reads locked, a merge in progress: {} merges sent, outcome {:?}",
        merges.len(),
        outcome.map_err(|error| format!("{error:#}"))
    );
}

/// A github read-back of `state: open` with `merged: true` is no open, unmerged pull request: a
/// malformed answer, and the resumed merge sends nothing.
#[test]
fn a_github_pull_read_open_and_merged_after_outcome_unknown_sends_no_merge() {
    let (scratch, stand_in, tree, head, case) = after_a_lost_merge("github-open-merged", "github");
    stand_in.with(|world| world.merged = json!(true));
    let resumed = forge(&scratch, &tree, "github", &stand_in);
    let outcome = resumed.merge(&tree, &case, NUMBER, &head);
    let merges = stand_in.merges();
    assert_eq!(
        merges.len(),
        1,
        "a merge was sent on a read-back saying open and merged at once: {} merges sent, outcome {:?}",
        merges.len(),
        outcome.map_err(|error| format!("{error:#}"))
    );
}

/// A pull request read merged at another head than the one the merge named was not merged by
/// that merge (its sha pins the head): the resumed merge is not reported applied.
fn merged_at_another_head_is_not_the_merge_applied(
    connector: &'static str,
    change: fn(&mut World),
) {
    let (scratch, stand_in, tree, head, case) =
        after_a_lost_merge(&format!("{connector}-merged-elsewhere"), connector);
    at_read_back(&stand_in, connector, change);
    let resumed = forge(&scratch, &tree, connector, &stand_in);
    let outcome = resumed.merge(&tree, &case, NUMBER, &head);
    assert_eq!(
        stand_in.merges().len(),
        1,
        "{connector}: no merge sent again"
    );
    assert!(
        outcome.is_err(),
        "{connector}: the merge at {head} is reported applied though the pull request reads merged at {OTHER}"
    );
}

#[test]
fn a_github_pull_read_merged_at_another_head_is_not_the_merge_applied() {
    merged_at_another_head_is_not_the_merge_applied("github", |world| {
        world.head = OTHER.to_owned();
        world.state = "closed".to_owned();
        world.merged = json!(true);
    });
}

#[test]
fn a_gitlab_merge_request_read_merged_at_another_head_is_not_the_merge_applied() {
    merged_at_another_head_is_not_the_merge_applied("gitlab", |world| {
        world.head = OTHER.to_owned();
        world.state = "merged".to_owned();
    });
}

/// A record whose `unknown` names something no write can have taken as its key (a key is
/// `takt-` and 64 lowercase hexadecimal digits) is no record of a write: an error, with nothing
/// sent. Another case's well-formed key cannot be told from this case's and changes nothing
/// sent, since the lookup goes by this case's own key; only the form is checked.
#[test]
fn an_unknown_member_naming_no_key_is_refused() {
    let scratch = Scratch::new("foreign-key");
    let (tree, head) = repository(&scratch);
    let case = artifacts_of(&scratch);
    let key = idempotency_key(
        "another-case",
        "pull_request.merge",
        REVISION,
        &json!({"owner": "owner", "repo": "repo", "pull_number": NUMBER, "body": {"sha": head}}),
        0,
    );
    for unknown in [
        json!({"not-a-key": null}),
        json!({key.to_uppercase(): null}),
        json!({key.clone(): "takt-short"}),
    ] {
        let stand_in = StandIn::start(World::new("github", &head));
        fs::create_dir_all(&case).expect("the artifacts");
        fs::write(
            case.join(ConnectorsForge::REFUSALS),
            json!({
                "format": "takt.connectors.refusals/1",
                "refused": {},
                "unknown": unknown,
            })
            .to_string(),
        )
        .expect("the record");
        let forge = forge(&scratch, &tree, "github", &stand_in);
        let outcome = forge.merge(&tree, &case, NUMBER, &head);
        assert!(
            outcome.is_err() && stand_in.merges().is_empty(),
            "a record naming no key at `unknown` ({unknown}) was admitted: {} merges sent, outcome {:?}",
            stand_in.merges().len(),
            outcome.map_err(|error| format!("{error:#}"))
        );
    }
}

/// The read-back fails: nothing is sent, and the error says so.
#[test]
fn a_read_back_that_fails_sends_no_merge() {
    for connector in ["github", "gitlab"] {
        let (scratch, stand_in, tree, head, case) =
            after_a_lost_merge(&format!("{connector}-read-fails"), connector);
        let resumed = forge(&scratch, &tree, connector, &stand_in);
        at_read_back(&stand_in, connector, |world| world.read_fails = true);
        let error = resumed
            .merge(&tree, &case, NUMBER, &head)
            .expect_err("a failed read-back is no merge");
        assert_eq!(
            stand_in.merges().len(),
            1,
            "{connector}: no merge sent: {error:#}"
        );
        assert_eq!(stand_in.applied(), 0);
    }
}

/// The read-back shows the pull request closed and not merged: nothing is sent.
#[test]
fn a_pull_read_closed_and_unmerged_sends_no_merge() {
    for connector in ["github", "gitlab"] {
        let (scratch, stand_in, tree, head, case) =
            after_a_lost_merge(&format!("{connector}-closed"), connector);
        at_read_back(&stand_in, connector, |world| {
            world.state = "closed".to_owned()
        });
        let resumed = forge(&scratch, &tree, connector, &stand_in);
        let error = resumed
            .merge(&tree, &case, NUMBER, &head)
            .expect_err("a closed pull request is no merge");
        assert_eq!(
            stand_in.merges().len(),
            1,
            "{connector}: no merge sent: {error:#}"
        );
    }
}

/// The fresh key is the key of the same merge after one refusal, and the record on disk names it
/// when the stand-in receives the merge it goes with.
#[test]
fn the_fresh_key_is_derived_and_recorded_before_it_is_sent() {
    let (scratch, stand_in, tree, head, case) = after_a_lost_merge("fresh-recorded", "github");
    let resumed = forge(&scratch, &tree, "github", &stand_in);
    resumed
        .merge(&tree, &case, NUMBER, &head)
        .expect("the resumed merge is applied");
    let input =
        json!({"owner": "owner", "repo": "repo", "pull_number": NUMBER, "body": {"sha": head}});
    let first = idempotency_key("case", "pull_request.merge", REVISION, &input, 0);
    let fresh = idempotency_key("case", "pull_request.merge", REVISION, &input, 1);
    let merges = stand_in.merges();
    assert_eq!(merges.len(), 2, "{merges:?}");
    assert_eq!(merges[0].0.as_deref(), Some(first.as_str()));
    assert_eq!(merges[1].0.as_deref(), Some(fresh.as_str()));
    let held: Value = serde_json::from_str(merges[1].1.as_deref().expect("a record on disk"))
        .expect("the record is JSON");
    assert_eq!(held["unknown"][&first], json!(fresh), "{held}");
    assert_eq!(held["refused"][&first], json!(1), "{held}");
    assert_eq!(stand_in.applied(), 1);
}

/// The fresh key cannot be recorded: nothing is sent.
#[test]
fn a_fresh_key_that_cannot_be_recorded_is_not_sent() {
    use std::os::unix::fs::PermissionsExt as _;
    let (scratch, stand_in, tree, head, case) = after_a_lost_merge("unrecorded", "github");
    fs::set_permissions(&case, fs::Permissions::from_mode(0o555)).expect("read-only artifacts");
    let resumed = forge(&scratch, &tree, "github", &stand_in);
    let outcome = resumed.merge(&tree, &case, NUMBER, &head);
    fs::set_permissions(&case, fs::Permissions::from_mode(0o755)).expect("writable again");
    let error = outcome.expect_err("an unrecorded fresh key is not sent");
    assert_eq!(stand_in.merges().len(), 1, "no merge sent: {error:#}");
}

/// An `unknown` member of the wrong shape, or a record past 64 KiB of keys, is an error with
/// nothing sent.
#[test]
fn an_unknown_member_of_the_wrong_shape_is_an_error_and_nothing_is_sent() {
    let oversized: serde_json::Map<String, Value> = (0..1200)
        .map(|n| (format!("takt-{n:064x}"), Value::Null))
        .collect();
    let shapes = [
        json!([]),
        json!("takt-key"),
        json!({"takt-key": 1}),
        json!({"takt-key": {"fresh": "takt-other"}}),
        json!({"takt-key": ["takt-other"]}),
        Value::Object(oversized),
    ];
    for (n, unknown) in shapes.into_iter().enumerate() {
        let scratch = Scratch::new(&format!("shape-{n}"));
        let (tree, head) = repository(&scratch);
        let stand_in = StandIn::start(World::new("github", &head));
        let case = artifacts_of(&scratch);
        fs::create_dir_all(&case).expect("the artifacts");
        fs::write(
            case.join(ConnectorsForge::REFUSALS),
            json!({"format": "takt.connectors.refusals/1", "refused": {}, "unknown": unknown})
                .to_string(),
        )
        .expect("the record");
        let forge = forge(&scratch, &tree, "github", &stand_in);
        let error = forge
            .merge(&tree, &case, NUMBER, &head)
            .expect_err("a record of the wrong shape is no merge");
        assert!(
            format!("{error:#}").contains("nothing was sent"),
            "shape {n}: {error:#}"
        );
        assert!(stand_in.merges().is_empty(), "shape {n}: no merge sent");
    }
}
