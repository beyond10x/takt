//! Adversary cases against keyed served writes (story:connectors-forge-at-a-served-instance):
//! the loopback stand-in of connectors_forge_github.rs, its descriptor revision movable, driven
//! across a second forge on the same case as a resumed process makes one.
#![allow(dead_code)]

#[path = "support/own_home.rs"]
mod own_home;

use std::collections::{BTreeMap, VecDeque};
use std::fs;
use std::io::{BufRead as _, BufReader, Read as _, Write as _};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::rc::Rc;
use std::sync::{Arc, Mutex};

use anyhow::anyhow;
use serde_json::{Value, json};

use takt::evidence::Record;
use takt::forge::connectors::ConnectorsForge;
use takt::forge::{Exit, Forge, PullRequest, Runner};
use takt::harness::{Harness, StepContext, StepReport};
use takt::run_config::RunConfig;
use takt::scan::Scan;
use takt::validator::{CiRead, PrMerged, PrOpen, Validator};
use takt::{Run, RunOptions, Stop};

/// The connector the run file's `forge_instance` names, with no instance: the endpoint names
/// one. Also the name the stand-in's instance describes itself by.
const INSTANCE: &str = "github";

/// The descriptor revision the stand-in describes, which every invocation must carry.
const REVISION: &str = "descriptor-r1";

/// The github connector's operations the stand-in answers.
const OPERATIONS: [&str; 6] = [
    "branch.get",
    "pull_request.create",
    "pull_request.get",
    "pull_request.merge",
    "check_runs.list",
    "commit.status",
];

/// The pull request the stand-in numbers every opened one.
const NUMBER: u64 = 7;

/// A head the pull request moves to while a merge is answered 409.
const OTHER: &str = "fedcba9876543210fedcba9876543210fedcba98";

/// The case branch of the forge-level cases.
const BRANCH: &str = "takt/case-1";

/// The paths of the served wire, the only ones takt may request.
const DESCRIBE: &str = "/v1/describe";
const INVOKE: &str = "/v1alpha2/invoke";

/// `git <args>` in `tree`, which never looks for a repository above the build's scratch
/// directory: a tree that is gone is an error, never the checkout the tests are built in.
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
    /// A directory of its own for every call, so tests running at once never share one.
    fn new(name: &str) -> Self {
        static MADE: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let made = MADE.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!(
            "w48-adv-b-{}-{made}-{name}",
            std::process::id()
        ));
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

/// One request the stand-in received: its method, its path, its headers (names lowercased) and
/// its body as JSON (`null` for none).
#[derive(Clone, Debug)]
struct Seen {
    method: String,
    path: String,
    headers: Vec<(String, String)>,
    body: Value,
    /// Whether the stand-in refused the request for a member it does not take, before reading
    /// anything else of it: no operation was run.
    refused_member: bool,
}

/// The writes of the github map, the operations a host with standing grants keys.
const WRITES: [&str; 2] = ["pull_request.create", "pull_request.merge"];

/// The stand-in as a host whose service configuration sets `standing_grants`, with a grant
/// covering every write unless [`Standing::refuse`] says otherwise.
#[derive(Debug, Default)]
struct Standing {
    /// Each key used and what it was used for (the operation, the revision and the input), with
    /// the answer stored for its replay.
    keys: BTreeMap<String, (Value, (u16, Value))>,
    /// The answers, an HTTP status and a code, the next writes with a new key meet before
    /// anything is recorded, one each, in order.
    refuse: VecDeque<(u16, &'static str)>,
    /// How many of the next writes recorded are answered `outcome_unknown`, applied all the same.
    unknown: usize,
    /// How many of the next writes recorded get no answer at all: applied, recorded, and the
    /// connection closed.
    dropped: usize,
    /// Each write that reached the provider, in order: what the fixture of GitHub saw.
    applied: Vec<String>,
}

/// The HTTP status [`invoke`] answers to close the connection with no answer.
const NO_ANSWER: u16 = 0;

/// The pull request the stand-in holds once one is opened.
#[derive(Clone, Debug)]
struct Pull {
    head: String,
    base: String,
    state: &'static str,
    merged: bool,
}

/// What the stand-in reports: one repository `owner/repo` on GitHub, seen through an instance of
/// the github connector.
#[derive(Debug)]
struct World {
    instance: String,
    operations: Vec<String>,
    /// Each branch and the commit it is at: what `branch.get` reports, and a pull request's head.
    branches: BTreeMap<String, String>,
    pull: Option<Pull>,
    /// Each check run on the head, as `(status, conclusion)`.
    check_runs: Vec<(&'static str, Option<&'static str>)>,
    /// The combined status of the head: its state and how many statuses it has.
    combined: (&'static str, usize),
    /// How many merges are still answered 409; each one answered moves the head to
    /// `moved_to`, when it names one.
    conflicts: usize,
    moved_to: Option<String>,
    /// `None` for a host without standing grants.
    standing: Option<Standing>,
    /// The descriptor revision the instance describes, which every invocation must carry.
    revision: String,
    /// How many of the next creates the provider applies and answers 502 all the same.
    gateway_errors: usize,
}

impl World {
    fn new(branches: &[(&str, &str)]) -> Self {
        Self {
            instance: INSTANCE.to_owned(),
            operations: OPERATIONS.iter().map(|name| (*name).to_owned()).collect(),
            branches: branches
                .iter()
                .map(|(name, sha)| ((*name).to_owned(), (*sha).to_owned()))
                .collect(),
            pull: None,
            check_runs: vec![("completed", Some("success"))],
            combined: ("pending", 0),
            conflicts: 0,
            moved_to: None,
            standing: None,
            revision: REVISION.to_owned(),
            gateway_errors: 0,
        }
    }

    /// The world served by a host with standing grants.
    fn with_standing_grants(mut self) -> Self {
        self.standing = Some(Standing::default());
        self
    }

    /// The world with pull request [`NUMBER`] open from `BRANCH` into `main`.
    fn with_open_pull(mut self) -> Self {
        self.pull = Some(Pull {
            head: BRANCH.to_owned(),
            base: "main".to_owned(),
            state: "open",
            merged: false,
        });
        self
    }

    fn view(&self) -> Option<Value> {
        let pull = self.pull.as_ref()?;
        Some(json!({
            "number": NUMBER,
            "state": pull.state,
            "merged": pull.merged,
            "head": {"ref": pull.head, "sha": self.branches.get(&pull.head)},
            "base": {"ref": pull.base},
        }))
    }
}

/// A success of the served wire for `request_id`, the operation answering `status` and `body`.
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

/// An error of the served wire for `request_id`, answered with the HTTP status `status`.
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

/// The stand-in's answer to one invocation, `envelope` the request body: on a host without
/// standing grants `idempotency_key` is refused as an unknown member and every write runs
/// unkeyed; on a host with them a write needs a key of 1 to 256 bytes, a key on a read is
/// `unsupported`, a key held for another request is `idempotency_conflict`, the same key and
/// request again replays the stored answer with `mutation.replayed: true`, and a new key meets
/// [`Standing::refuse`] before it is recorded.
fn invoke(envelope: &Value, world: &mut World) -> (u16, Value) {
    let id = &envelope["request_id"];
    let key = envelope.get("idempotency_key");
    if world.standing.is_none() {
        if key.is_some() {
            return failure(
                &Value::Null,
                400,
                "unsupported",
                "idempotency_key is not supported",
            );
        }
        return dispatch(envelope, world);
    }
    let operation = envelope["operation"]
        .as_str()
        .unwrap_or_default()
        .to_owned();
    let key = match (WRITES.contains(&operation.as_str()), key) {
        (false, None) => return dispatch(envelope, world),
        (false, Some(_)) => {
            return failure(id, 400, "unsupported", "idempotency_key on a read");
        }
        (true, None) => return failure(id, 400, "invalid_input", "a write needs idempotency_key"),
        (true, Some(key)) => match key.as_str().filter(|key| (1..=256).contains(&key.len())) {
            Some(key) => key.to_owned(),
            None => return failure(id, 400, "invalid_input", "idempotency_key is malformed"),
        },
    };
    let request = json!([operation, envelope["revision"], envelope["input"]]);
    let standing = world
        .standing
        .as_mut()
        .expect("a host with standing grants");
    if let Some((held, (status, answer))) = standing.keys.get(&key) {
        if *held != request {
            return failure(
                id,
                409,
                "idempotency_conflict",
                "the key was used for another request",
            );
        }
        let mut answer = answer.clone();
        answer["request_id"] = id.clone();
        answer["mutation"]["replayed"] = json!(true);
        return (*status, answer);
    }
    if let Some((status, code)) = standing.refuse.pop_front() {
        return failure(id, status, code, "refused before the attempt is recorded");
    }
    let (status, answer) = dispatch(envelope, world);
    let standing = world
        .standing
        .as_mut()
        .expect("a host with standing grants");
    standing.applied.push(operation);
    let (status, mut answer, classification) = if standing.unknown > 0 {
        standing.unknown -= 1;
        let (status, answer) =
            failure(id, 502, "outcome_unknown", "the provider's answer was lost");
        (status, answer, "unknown")
    } else if answer["status"] == "success" {
        (status, answer, "applied")
    } else {
        (status, answer, "refused")
    };
    answer["mutation"] = json!({
        "classification": classification,
        "attempt": {"instance": INSTANCE, "id": format!("attempt-{}", standing.applied.len())},
        "original_request_id": id,
        "replayed": false,
    });
    standing
        .keys
        .insert(key, (request, (status, answer.clone())));
    if standing.dropped > 0 {
        standing.dropped -= 1;
        return (NO_ANSWER, Value::Null);
    }
    (status, answer)
}

/// The operation `envelope` names run on `world`, as the provider answers it.
fn dispatch(envelope: &Value, world: &mut World) -> (u16, Value) {
    let id = &envelope["request_id"];
    if envelope["version"] != "v1alpha2" || !id.is_string() {
        return failure(
            &Value::Null,
            400,
            "invalid_input",
            "invalid invocation envelope",
        );
    }
    if envelope["revision"] != world.revision.as_str() {
        return failure(
            id,
            409,
            "stale_description",
            "the descriptor revision changed",
        );
    }
    let input = &envelope["input"];
    if input["owner"] != "owner" || input["repo"] != "repo" {
        return failure(
            id,
            400,
            "invalid_input",
            "owner and repo name no repository",
        );
    }
    let operation = envelope["operation"].as_str().unwrap_or_default();
    if !world.operations.iter().any(|listed| listed == operation) {
        return failure(id, 404, "not_found", "operation is not provided");
    }
    match operation {
        "branch.get" => {
            let name = input["branch"].as_str().unwrap_or_default();
            match world.branches.get(name) {
                Some(sha) => success(id, 200, json!({"name": name, "commit": {"sha": sha}})),
                None => failure(id, 404, "not_found", "no such branch"),
            }
        }
        "pull_request.create" => {
            let head = input["body"]["head"]
                .as_str()
                .unwrap_or_default()
                .to_owned();
            if world.branches.get(&head).map(String::as_str) != input["sha"].as_str() {
                return failure(
                    id,
                    403,
                    "forbidden",
                    "the head branch is not at the pinned sha",
                );
            }
            world.pull = Some(Pull {
                head,
                base: input["body"]["base"]
                    .as_str()
                    .unwrap_or_default()
                    .to_owned(),
                state: "open",
                merged: false,
            });
            if world.gateway_errors > 0 {
                world.gateway_errors -= 1;
                return success(id, 502, json!({"message": "Server Error"}));
            }
            success(id, 201, world.view().expect("opened"))
        }
        "pull_request.get" if input["pull_number"] == NUMBER => match world.view() {
            Some(view) => success(id, 200, view),
            None => failure(id, 404, "not_found", "no such pull request"),
        },
        "check_runs.list" => {
            let per_page = input["per_page"].as_u64().unwrap_or(30) as usize;
            let page = input["page"].as_u64().unwrap_or(1) as usize;
            let runs: Vec<Value> = world
                .check_runs
                .iter()
                .skip((page - 1) * per_page)
                .take(per_page)
                .map(|(status, conclusion)| json!({"status": status, "conclusion": conclusion}))
                .collect();
            success(
                id,
                200,
                json!({"total_count": world.check_runs.len(), "check_runs": runs}),
            )
        }
        "commit.status" => {
            let (state, count) = world.combined;
            let statuses: Vec<Value> = (0..count)
                .map(|n| json!({"state": state, "context": format!("status-{n}")}))
                .collect();
            success(
                id,
                200,
                json!({"state": state, "total_count": count, "statuses": statuses}),
            )
        }
        "pull_request.merge" if input["pull_number"] == NUMBER => {
            if world.conflicts > 0 {
                world.conflicts -= 1;
                if let (Some(moved), Some(pull)) = (world.moved_to.clone(), &world.pull) {
                    world.branches.insert(pull.head.clone(), moved);
                }
                return failure(
                    id,
                    409,
                    "conflict",
                    "provider answered that the target changed since it was read",
                );
            }
            let Some(pull) = world.pull.as_mut() else {
                return failure(id, 404, "not_found", "no such pull request");
            };
            if world.branches.get(&pull.head).map(String::as_str) != input["body"]["sha"].as_str() {
                return failure(id, 409, "conflict", "head branch was modified");
            }
            pull.state = "closed";
            pull.merged = true;
            success(
                id,
                200,
                json!({"sha": "a-merge-commit", "merged": true, "message": "merged"}),
            )
        }
        _ => failure(id, 404, "not_found", "no such pull request"),
    }
}

/// The stand-in's answer to one request.
fn answer(method: &str, path: &str, body: &Value, world: &mut World) -> (u16, Value) {
    match (method, path) {
        ("GET", DESCRIBE) => (
            200,
            json!({
                "version": "v1alpha1",
                "instance": world.instance,
                "adapter": "catalog",
                "revision": world.revision,
                "operations": world.operations.iter().map(|id| json!({
                    "id": id,
                    "description": "",
                    "contract": "operations/v1alpha1",
                    "profile": "github.app",
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

/// Reads one request from `stream`, records it in `seen` and answers it from `world`.
fn serve(stream: TcpStream, seen: &Mutex<Vec<Seen>>, world: &Mutex<World>) {
    let mut reader = BufReader::new(stream.try_clone().expect("the stream"));
    let mut line = String::new();
    if reader.read_line(&mut line).is_err() {
        return;
    }
    let mut words = line.split_whitespace();
    let method = words.next().unwrap_or_default().to_owned();
    let path = words.next().unwrap_or_default().to_owned();
    let mut headers = Vec::new();
    loop {
        let mut header = String::new();
        if reader.read_line(&mut header).is_err() {
            return;
        }
        let header = header.trim_end();
        if header.is_empty() {
            break;
        }
        if let Some((name, value)) = header.split_once(':') {
            headers.push((name.trim().to_ascii_lowercase(), value.trim().to_owned()));
        }
    }
    let length = headers
        .iter()
        .find(|(name, _)| name == "content-length")
        .and_then(|(_, value)| value.parse::<usize>().ok())
        .unwrap_or(0);
    let mut bytes = vec![0; length];
    if reader.read_exact(&mut bytes).is_err() {
        return;
    }
    let body: Value = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
    let (status, answer) = answer(&method, &path, &body, &mut world.lock().expect("the world"));
    seen.lock().expect("the log").push(Seen {
        method,
        path,
        headers,
        body,
        refused_member: status == 400
            && answer["request_id"].is_null()
            && answer["error"]["code"] == "unsupported",
    });
    if status == NO_ANSWER {
        return;
    }
    let text = answer.to_string();
    let mut stream = stream;
    let _ = write!(
        stream,
        "HTTP/1.1 {status} Answer\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{text}",
        text.len()
    );
}

/// The stand-in of a served Connectors instance, listening on 127.0.0.1 until the process ends.
struct StandIn {
    url: String,
    seen: Arc<Mutex<Vec<Seen>>>,
    world: Arc<Mutex<World>>,
}

impl StandIn {
    fn start(world: World) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("a loopback port");
        let url = format!("http://{}", listener.local_addr().expect("an address"));
        let seen = Arc::new(Mutex::new(Vec::new()));
        let world = Arc::new(Mutex::new(world));
        let (log, held) = (seen.clone(), world.clone());
        std::thread::spawn(move || {
            for stream in listener.incoming().flatten() {
                serve(stream, &log, &held);
            }
        });
        Self { url, seen, world }
    }

    fn seen(&self) -> Vec<Seen> {
        self.seen.lock().expect("the log").clone()
    }

    /// The invocations the stand-in read past their members, in order: every one but those it
    /// refused for a member it does not take ([`Seen::refused_member`]).
    fn read_invocations(&self) -> Vec<Seen> {
        self.seen()
            .into_iter()
            .filter(|seen| seen.path == INVOKE && !seen.refused_member)
            .collect()
    }

    /// The operations invoked, in order, those refused for a member left out.
    fn invoked(&self) -> Vec<String> {
        self.read_invocations()
            .iter()
            .filter_map(|seen| seen.body["operation"].as_str().map(str::to_owned))
            .collect()
    }

    /// The input of every invocation of `operation`, in order, those refused for a member left
    /// out.
    fn inputs_of(&self, operation: &str) -> Vec<Value> {
        self.read_invocations()
            .iter()
            .filter(|seen| seen.body["operation"] == operation)
            .map(|seen| seen.body["input"].clone())
            .collect()
    }

    /// The `idempotency_key` of every invocation of `operation` the stand-in read, `None` for
    /// one sent with none.
    fn keys_of(&self, operation: &str) -> Vec<Option<String>> {
        self.read_invocations()
            .iter()
            .filter(|seen| seen.body["operation"] == operation)
            .map(|seen| seen.body["idempotency_key"].as_str().map(str::to_owned))
            .collect()
    }

    /// The operation of every invocation refused for a member the stand-in does not take, and
    /// whether it carried `idempotency_key`.
    fn refused_members(&self) -> Vec<(String, bool)> {
        self.seen()
            .iter()
            .filter(|seen| seen.path == INVOKE && seen.refused_member)
            .map(|seen| {
                (
                    seen.body["operation"]
                        .as_str()
                        .unwrap_or_default()
                        .to_owned(),
                    seen.body.get("idempotency_key").is_some(),
                )
            })
            .collect()
    }

    /// Each write a host with standing grants let reach the provider, in order.
    fn applied(&self) -> Vec<String> {
        self.world
            .lock()
            .expect("the world")
            .standing
            .as_ref()
            .map(|standing| standing.applied.clone())
            .unwrap_or_default()
    }

    /// Changes the host's standing-grant state with `change`.
    fn standing(&self, change: impl FnOnce(&mut Standing)) {
        change(
            self.world
                .lock()
                .expect("the world")
                .standing
                .as_mut()
                .expect("a host with standing grants"),
        );
    }

    /// Every request is `GET /v1/describe` or `POST /v1alpha2/invoke`, and none carries a
    /// credential of any kind.
    fn assert_served_wire_only(&self) {
        let seen = self.seen();
        assert!(!seen.is_empty(), "the stand-in saw no request");
        for request in &seen {
            assert!(
                (request.method == "GET" && request.path == DESCRIBE)
                    || (request.method == "POST" && request.path == INVOKE),
                "a request outside the served wire: {} {}",
                request.method,
                request.path
            );
            for (name, _) in &request.headers {
                assert!(
                    !["authorization", "proxy-authorization", "cookie"].contains(&name.as_str()),
                    "{} {} carries {name}",
                    request.method,
                    request.path
                );
            }
        }
    }
}

/// The double of the forge's runner: records every argument list and the environment it was
/// given, takes a `git push` of `<revision>:refs/heads/<branch>` into the stand-in's branches,
/// as GitHub would, lists a branch the stand-in holds to a `git ls-remote` of it, as the
/// project's repository would, and answers every other `git` line with success. A
/// `connectors` line is never run.
#[derive(Clone)]
struct Pushes {
    lines: Rc<std::cell::RefCell<Vec<Vec<String>>>>,
    envs: Rc<std::cell::RefCell<Vec<Env>>>,
    world: Arc<Mutex<World>>,
}

/// The environment one line was given, each variable's name and value.
type Env = Vec<(String, String)>;

impl Pushes {
    fn new(world: Arc<Mutex<World>>) -> Self {
        Self {
            lines: Rc::default(),
            envs: Rc::default(),
            world,
        }
    }

    fn lines(&self) -> Vec<Vec<String>> {
        self.lines.borrow().clone()
    }

    fn envs(&self) -> Vec<Env> {
        self.envs.borrow().clone()
    }
}

impl Runner for Pushes {
    fn run(&self, tree: &Path, argv: &[String]) -> anyhow::Result<Exit> {
        self.run_env(tree, argv, &[])
    }

    fn run_env(&self, _tree: &Path, argv: &[String], env: &[(&str, &str)]) -> anyhow::Result<Exit> {
        self.lines.borrow_mut().push(argv.to_vec());
        self.envs.borrow_mut().push(
            env.iter()
                .map(|(name, value)| ((*name).to_owned(), (*value).to_owned()))
                .collect(),
        );
        assert_eq!(
            argv.first().map(String::as_str),
            Some("git"),
            "a line that is not git: {argv:?}"
        );
        if argv.iter().any(|word| word == "push") {
            let refspec = argv
                .iter()
                .find(|word| word.contains(":refs/heads/"))
                .ok_or_else(|| anyhow!("a push with no refspec: {argv:?}"))?;
            let (revision, branch) = refspec.split_once(":refs/heads/").expect("a refspec");
            self.world
                .lock()
                .expect("the world")
                .branches
                .insert(branch.to_owned(), revision.to_owned());
        }
        let mut stdout = String::new();
        if argv.iter().any(|word| word == "ls-remote") {
            let reference = argv.last().expect("a reference");
            let branch = reference.strip_prefix("refs/heads/").expect("a branch");
            if let Some(sha) = self.world.lock().expect("the world").branches.get(branch) {
                stdout = format!("{sha}\t{reference}\n");
            }
        }
        Ok(Exit {
            code: Some(0),
            stdout,
            stderr: String::new(),
        })
    }
}

/// A repository at `<scratch>/tree` whose `main` holds a README, the judge allow-list and the
/// owner's delegation of the merge to `agent:takt-delivery`, with `origin` naming
/// `owner/repo` on a host that does not exist, and the case branch [`BRANCH`] one commit above
/// `main`. Returns the tree, `main`'s commit and the branch's.
fn repository(scratch: &Scratch) -> (PathBuf, String, String) {
    let tree = scratch.join("tree");
    let files = [
        ("README.md", "a repository\n".to_owned()),
        ("docs/guide.md", "a guide\n".to_owned()),
        (
            ".engineering/judge-allow.yaml",
            "paths:\n  - docs/**\n  - README.md\n  - CHANGELOG.md\n".to_owned(),
        ),
        (
            ".engineering/delegations.yaml",
            ["repository.merge", "repository.merge.judge_surface"]
                .iter()
                .fold("delegations:\n".to_owned(), |text, capability| {
                    text + &format!(
                        "  - owner: human:owner\n    role: agent:takt-delivery\n    capability: {capability}\n    given_at: 2026-10-30T08:00:00Z\n"
                    )
                }),
        ),
    ];
    for (path, text) in files {
        let path = tree.join(path);
        fs::create_dir_all(path.parent().expect("a parent")).expect("a directory");
        fs::write(path, text).expect("a file");
    }
    git(&tree, &["init", "-q", "-b", "main"]);
    git(&tree, &["add", "-A"]);
    git(&tree, &["commit", "-q", "-m", "start"]);
    git(
        &tree,
        &[
            "remote",
            "add",
            "origin",
            "https://github.example/owner/repo.git",
        ],
    );
    let main = git(&tree, &["rev-parse", "HEAD"]);
    git(&tree, &["checkout", "-q", "-b", BRANCH]);
    fs::write(tree.join("docs/guide.md"), "a guide, changed\n").expect("a file");
    git(&tree, &["commit", "-q", "-am", "change the guide"]);
    let head = git(&tree, &["rev-parse", "HEAD"]);
    git(&tree, &["checkout", "-q", "main"]);
    (tree, main, head)
}

/// The run file naming `target` and the stand-in, written into `scratch`.
fn run_file(scratch: &Scratch, target: &str, url: &str) -> PathBuf {
    let path = scratch.join("run.yaml");
    fs::write(
        &path,
        format!("forge: connectors\nforge_instance: {target}\nforge_endpoint: {url}\nscan: none\n"),
    )
    .expect("the run file");
    path
}

/// The forge the run file over `stand_in` selects for `tree`, its runner `pushes`.
fn select(
    scratch: &Scratch,
    tree: &Path,
    stand_in: &StandIn,
    pushes: &Pushes,
) -> anyhow::Result<ConnectorsForge> {
    select_named(scratch, tree, stand_in, pushes, INSTANCE)
}

/// [`select`] with the run file's `forge_instance` `target`.
fn select_named(
    scratch: &Scratch,
    tree: &Path,
    stand_in: &StandIn,
    pushes: &Pushes,
    target: &str,
) -> anyhow::Result<ConnectorsForge> {
    let path = run_file(scratch, target, &stand_in.url);
    let config = RunConfig::read(&path)?;
    ConnectorsForge::select(
        tree,
        config.forge.as_deref(),
        &config.forge_settings(),
        Scan::None { declared_in: path },
        Box::new(pushes.clone()),
    )?
    .ok_or_else(|| anyhow!("no forge was selected"))
}

/// A forge over a stand-in of `world`, for a repository made in `scratch`: the forge, the
/// stand-in, the runner's double, the tree and the case branch's commit.
fn forge_over(
    scratch: &Scratch,
    world: impl FnOnce(&str, &str) -> World,
) -> (ConnectorsForge, StandIn, Pushes, PathBuf, String) {
    let (tree, main, head) = repository(scratch);
    let stand_in = StandIn::start(world(&main, &head));
    let pushes = Pushes::new(stand_in.world.clone());
    let forge = select(scratch, &tree, &stand_in, &pushes).expect("the forge is selected");
    (forge, stand_in, pushes, tree, head)
}

/// A world where `main` and the case branch are at the given commits.
fn branches(main: &str, head: &str) -> World {
    World::new(&[("main", main), (BRANCH, head)])
}

/// A harness that asks for the merge gate whenever it is offered and hands the change over when
/// that is offered: the run's grant is never its choice.
struct Requester;

impl Harness for Requester {
    fn name(&self) -> &str {
        "requester"
    }

    fn step(&self, context: &StepContext<'_>) -> Result<StepReport, anyhow::Error> {
        if context.approvals.contains(&"repository.merge".to_owned()) {
            return Ok(StepReport {
                action: "repository.merge".to_owned(),
                summary: "the intent holds; merge it".to_owned(),
                tokens: None,
            });
        }
        if context.offered.contains(&"change.handover".to_owned()) {
            return Ok(StepReport {
                action: "change.handover".to_owned(),
                summary: "hand it over".to_owned(),
                tokens: None,
            });
        }
        Err(anyhow!(
            "neither the gate nor the handover was offered; among {}",
            context.offered.join(", ")
        ))
    }
}

/// The records of `kind` under the case at `dir`, each as its JSON.
fn records_of(dir: &Path, kind: &str) -> Vec<Value> {
    let Ok(entries) = fs::read_dir(dir.join("evidence")) else {
        return Vec::new();
    };
    entries
        .map(|entry| {
            let path = entry.expect("an entry").path();
            serde_json::from_str::<Value>(&fs::read_to_string(&path).expect("a record"))
                .expect("json")
        })
        .filter(|record| record["kind"] == kind)
        .collect()
}

// Adversary cases.

/// The request for the case branch into `main` the forge-level cases open.
fn the_request() -> PullRequest {
    PullRequest {
        branch: BRANCH.to_owned(),
        base: "main".to_owned(),
        title: "A title".to_owned(),
        body: "the body".to_owned(),
    }
}

/// The artifacts directory of the case `id` under `scratch`: `<case directory>/artifacts`.
fn artifacts_of(scratch: &Scratch, id: &str) -> PathBuf {
    scratch.join("cases").join(id).join("artifacts")
}

/// A world served by a host with standing grants.
fn keyed(main: &str, head: &str) -> World {
    branches(main, head).with_standing_grants()
}

/// Every key that drew a provider refusal for a merge, as the stand-in stored it.
fn refused_keys(stand_in: &StandIn) -> Vec<String> {
    let world = stand_in.world.lock().expect("the world");
    world
        .standing
        .as_ref()
        .map(|standing| {
            standing
                .keys
                .iter()
                .filter(|(_, (_, (_, answer)))| answer["mutation"]["classification"] == "refused")
                .map(|(key, _)| key.clone())
                .collect()
        })
        .unwrap_or_default()
}

/// A resumed process (a second forge on the same case and stand-in) takes a merge the provider
/// refused in the process before again: its first merge, sent once the provider takes it,
/// reaches the provider. The refusal count lives in the process, so the second forge starts at
/// the key the first one's refusal is stored under, and its merge is that refusal replayed:
/// a run whose budget allows as many attempts as the run before it never merges.
#[test]
fn a_resumed_process_does_not_replay_a_refusal_the_process_before_it_drew() {
    let scratch = Scratch::new("restart-refusal");
    let (tree, main, head) = repository(&scratch);
    let stand_in = StandIn::start({
        let mut world = keyed(&main, &head).with_open_pull();
        world.conflicts = 2;
        world
    });
    let pushes = Pushes::new(stand_in.world.clone());
    let case = artifacts_of(&scratch, "case");
    let first = select(&scratch, &tree, &stand_in, &pushes).expect("the forge is selected");
    for _ in 0..2 {
        let error = first
            .merge(&tree, &case, NUMBER, &head)
            .expect_err("a 409 is no merge");
        assert!(format!("{error:#}").contains("nothing was merged"), "{error:#}");
    }
    drop(first);
    let held = refused_keys(&stand_in);
    assert_eq!(held.len(), 2, "two refusals stored under two keys: {held:?}");

    let resumed = select(&scratch, &tree, &stand_in, &pushes).expect("the forge is selected");
    let outcome = resumed.merge(&tree, &case, NUMBER, &head);
    let keys = stand_in.keys_of("pull_request.merge");
    let sent = keys.last().cloned().flatten().expect("a keyed merge");
    assert!(
        outcome.is_ok() && !held.contains(&sent),
        "the resumed process sent the merge under {sent}, a key whose refusal the host holds, and got {:?}; keys sent: {keys:?}",
        outcome.err().map(|error| format!("{error:#}"))
    );
}

/// A merge the provider refused, its refusal stored under the key, and the instance's
/// descriptor revision moved before the next process: that process sends the merge under the
/// same key at the new revision, the host answers `idempotency_conflict`, which takes no new
/// key, so every later attempt meets the same conflict and the pull request never merges.
#[test]
fn a_refused_merge_still_merges_after_the_descriptor_revision_moves() {
    let scratch = Scratch::new("revision-moved");
    let (tree, main, head) = repository(&scratch);
    let stand_in = StandIn::start({
        let mut world = keyed(&main, &head).with_open_pull();
        world.conflicts = 1;
        world
    });
    let pushes = Pushes::new(stand_in.world.clone());
    let case = artifacts_of(&scratch, "case");
    let first = select(&scratch, &tree, &stand_in, &pushes).expect("the forge is selected");
    first
        .merge(&tree, &case, NUMBER, &head)
        .expect_err("a 409 is no merge");
    drop(first);
    stand_in.world.lock().expect("the world").revision = "descriptor-r2".to_owned();

    let resumed = select(&scratch, &tree, &stand_in, &pushes).expect("the forge is selected");
    let mut said = Vec::new();
    let mut merged = false;
    for _ in 0..4 {
        match resumed.merge(&tree, &case, NUMBER, &head) {
            Ok(()) => {
                merged = true;
                break;
            }
            Err(error) => said.push(format!("{error:#}")),
        }
    }
    assert!(
        merged,
        "four attempts after the revision moved, none merged; keys {:?}; errors: {said:#?}",
        stand_in.keys_of("pull_request.merge")
    );
}

/// The provider applied the create and answered 502 (a gateway error after the write went
/// through), and the instance said so (`status: success`, the result's status 502). That is no
/// refusal: the create is sent again under the same key and replays, and one create reached
/// the provider. The forge reads any provider status outside 2xx as a refusal and takes a new
/// key, so the create is applied again.
#[test]
fn a_create_the_provider_answered_502_is_sent_again_under_its_key() {
    let scratch = Scratch::new("gateway");
    let (forge, stand_in, _, tree, _) = forge_over(&scratch, |main, head| {
        let mut world = keyed(main, head);
        world.gateway_errors = 1;
        world
    });
    let case = artifacts_of(&scratch, "case");
    let error = forge
        .open_pull_request(&tree, &case, &the_request())
        .expect_err("a 502 is no number");
    let _ = forge.open_pull_request(&tree, &case, &the_request());
    let keys = stand_in.keys_of("pull_request.create");
    assert_eq!(
        stand_in.applied(),
        ["pull_request.create"],
        "one create reaches the provider; keys {keys:?}; first error: {error:#}"
    );
}


/// The options of every run on the case `github`.
fn run_options() -> RunOptions {
    RunOptions {
        protocol: "story.delivery/1".to_owned(),
        intent: "change the guide\n\nA guide that says more.".to_owned(),
        inputs: Vec::new(),
        iterations: Some(12),
        hours: None,
        tokens: None,
        case_id: Some("github".to_owned()),
        gate: Some("repository.merge".to_owned()),
    }
}

/// The validators of a run over `forge`.
fn validators_of(forge: Rc<dyn Forge>) -> Vec<Box<dyn Validator>> {
    vec![
        Box::new(PrOpen::new(forge.clone())),
        Box::new(CiRead::new(forge.clone())),
        Box::new(PrMerged::new(forge)),
    ]
}

/// A fresh forge over `stand_in`, as a new takt process selects one.
fn process(scratch: &Scratch, tree: &Path, stand_in: &StandIn, pushes: &Pushes) -> Rc<dyn Forge> {
    Rc::new(select(scratch, tree, stand_in, pushes).expect("the forge is selected"))
}

/// The first run of `story.delivery/1` on the case `github` in `tree`, its change a docs edit
/// with its tests, gate, handover and change records in place, as `delivered` stages it in
/// connectors_forge_github.rs: how it stopped.
fn first_run(
    scratch: &Scratch,
    tree: &Path,
    stand_in: &StandIn,
    pushes: &Pushes,
    out: &mut Vec<u8>,
) -> anyhow::Result<Stop> {
    let validators = validators_of(process(scratch, tree, stand_in, pushes));
    let mut run = Run::open(tree, run_options(), &Requester, &validators).expect("the case opens");
    fs::write(tree.join("docs/guide.md"), "a guide, changed\n").expect("a file");
    git(tree, &["add", "-A", "--", ".", ":!.takt"]);
    git(tree, &["commit", "-q", "-m", "change the guide"]);
    let revision = git(tree, &["rev-parse", "HEAD"]);
    run.case
        .set_revision("implementation", &revision)
        .expect("the revision moves");
    let short = &revision[..8];
    for (id, kind, result, producer) in [
        ("e0".to_owned(), "test_result", Some("pass"), "adv-test"),
        ("e1".to_owned(), "gate_result", Some("pass"), "adv-test"),
        (
            format!("handover-{short}"),
            "handover",
            None,
            takt::run::CHOICE_PRODUCER,
        ),
        (
            format!("change-record-{short}"),
            "change_record",
            Some("ahead"),
            "takt.change-read",
        ),
    ] {
        run.case
            .add_evidence(&Record::new(
                id,
                kind,
                result.map(str::to_owned),
                "implementation",
                &revision,
                producer,
            ))
            .expect("the record is written");
    }
    run.run(out)
}

/// A run resumed on the case `github` by a new process: how it stopped.
fn resumed_run(
    scratch: &Scratch,
    tree: &Path,
    stand_in: &StandIn,
    pushes: &Pushes,
    out: &mut Vec<u8>,
) -> anyhow::Result<Stop> {
    let validators = validators_of(process(scratch, tree, stand_in, pushes));
    let mut run = Run::resume(tree, run_options(), "github", &Requester, &validators)
        .expect("the case resumes");
    run.run(out)
}

/// The same write in the same case from a resumed run: a run whose create lost its answer ends,
/// and the run resumed on that case opens under the same key, so the instance replays the first
/// create and the provider sees one.
#[test]
fn a_resumed_run_after_a_lost_create_replays_it_under_the_same_key() {
    let scratch = Scratch::new("resume-lost-create");
    let (tree, main, _) = repository(&scratch);
    let stand_in = StandIn::start(World::new(&[("main", &main)]).with_standing_grants());
    stand_in.standing(|standing| standing.dropped = 1);
    let pushes = Pushes::new(stand_in.world.clone());
    let mut out = Vec::new();
    let first = first_run(&scratch, &tree, &stand_in, &pushes, &mut out);
    assert!(
        first.is_err(),
        "the run whose create lost its answer ends: {first:?}\n{}",
        String::from_utf8_lossy(&out)
    );
    let second = resumed_run(&scratch, &tree, &stand_in, &pushes, &mut out);
    let out = String::from_utf8_lossy(&out).into_owned();
    let keys = stand_in.keys_of("pull_request.create");
    let creates = stand_in
        .applied()
        .iter()
        .filter(|write| *write == "pull_request.create")
        .count();
    assert!(
        creates == 1 && keys.len() >= 2 && keys[0] == keys[1],
        "creates that reached the provider: {creates}; create keys {keys:?}; resumed run: {second:?}\n{out}"
    );
}

/// A merge the provider refused once ends the run; the case is resumed, as an operator resumes
/// it, by a new process, and the provider now takes the merge. Each resumed process starts its
/// refusal count at zero, so its one merge goes under the key the first refusal is stored under,
/// the host replays that refusal, and the run ends again: no number of resumes merges.
#[test]
fn a_case_whose_merge_was_refused_once_merges_on_a_resumed_run() {
    let scratch = Scratch::new("resume-refused-merge");
    let (tree, main, _) = repository(&scratch);
    let stand_in = StandIn::start({
        let mut world = World::new(&[("main", &main)]).with_standing_grants();
        world.conflicts = 1;
        world
    });
    let pushes = Pushes::new(stand_in.world.clone());
    let mut out = Vec::new();
    let first = first_run(&scratch, &tree, &stand_in, &pushes, &mut out);
    assert!(
        first.is_err(),
        "the run whose merge was refused ends: {first:?}\n{}",
        String::from_utf8_lossy(&out)
    );
    let mut stops = Vec::new();
    for _ in 0..3 {
        let stop = resumed_run(&scratch, &tree, &stand_in, &pushes, &mut out);
        let done = matches!(stop, Ok(Stop::Completed(_)));
        stops.push(stop.map_err(|error| format!("{error:#}")));
        if done {
            break;
        }
    }
    let merged = stand_in
        .world
        .lock()
        .expect("the world")
        .pull
        .as_ref()
        .is_some_and(|pull| pull.merged);
    assert!(
        merged,
        "three resumed runs after one refused merge, none merged; merge keys {:?}; merges applied {:?}; stops {stops:#?}",
        stand_in.keys_of("pull_request.merge"),
        stand_in.applied()
    );
}
