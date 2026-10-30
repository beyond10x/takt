//! story:connectors-forge-speaks-github-at-an-endpoint: `forge: connectors` with
//! `forge_instance: github` and `forge_endpoint` drives a GitHub pull request through a served
//! Connectors instance: `GET /v1/describe` once, then one `POST /v1alpha2/invoke` per operation,
//! with no credential of any kind; the `connectors` program is never run. The stand-in of the
//! served wire is a small HTTP server on 127.0.0.1 inside this test that records each request's
//! method, path and headers and answers as an instance of the github connector would; the push
//! goes through a double of the forge's runner. Nothing here reaches a network beyond 127.0.0.1.
//!
//! story:connectors-forge-at-a-served-instance: the stand-in is a host without standing grants
//! by default, which refuses `idempotency_key` as an unknown member (`unsupported`, HTTP 400,
//! `request_id: null`), and one with them when its world says so ([`Standing`]), which takes
//! keyed writes, replays a key and refuses as connectors 0.62.0 documents.

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
use takt::case::Authority;
use takt::evidence::Record;
use takt::forge::connectors::ConnectorsForge;
use takt::forge::{Checks, Exit, Forge, PullRequest, PullRequestState, Runner};
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
            "connectors-forge-github-{}-{made}-{name}",
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
    if envelope["revision"] != REVISION {
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
                "revision": REVISION,
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

/// Acceptance 1: a run on `forge: connectors`, `forge_instance: github` and `forge_endpoint` at
/// the stand-in opens the pull request, reads its checks and merges it under the owner's
/// delegation; the stand-in saw only `GET /v1/describe` and `POST /v1alpha2/invoke`, none with
/// an `Authorization` header, and the runner ran no `connectors` line, only the push.
#[test]
fn a_run_opens_reads_and_merges_a_pull_request_through_the_served_wire() {
    let scratch = Scratch::new("run");
    let (stand_in, pushes, out) = delivered(&scratch, |main| World::new(&[("main", main)]));
    served_wire_of_a_delivered_run(&stand_in, &pushes, &out);
}

/// A run of `story.delivery/1` on `forge: connectors` at a stand-in of `world` (given `main`'s
/// commit), the change a docs edit with its tests, gate, handover and change records in place:
/// it completes accepted, merged once, the pull request opened and the merge granted under the
/// owner's delegation. The stand-in, the runner's double and what the run wrote.
fn delivered(scratch: &Scratch, world: impl FnOnce(&str) -> World) -> (StandIn, Pushes, String) {
    let (tree, main, _) = repository(scratch);
    let stand_in = StandIn::start(world(&main));
    let pushes = Pushes::new(stand_in.world.clone());
    let forge: Rc<dyn Forge> =
        Rc::new(select(scratch, &tree, &stand_in, &pushes).expect("the forge is selected"));
    assert_eq!(forge.name(), "connectors");
    let validators: Vec<Box<dyn Validator>> = vec![
        Box::new(PrOpen::new(forge.clone())),
        Box::new(CiRead::new(forge.clone())),
        Box::new(PrMerged::new(forge)),
    ];
    let options = RunOptions {
        protocol: "story.delivery/1".to_owned(),
        intent: "change the guide\n\nA guide that says more.".to_owned(),
        inputs: Vec::new(),
        iterations: Some(12),
        hours: None,
        tokens: None,
        case_id: Some("github".to_owned()),
        gate: Some("repository.merge".to_owned()),
    };
    let mut run = Run::open(&tree, options, &Requester, &validators).expect("the case opens");
    fs::write(tree.join("docs/guide.md"), "a guide, changed\n").expect("a file");
    git(&tree, &["add", "-A", "--", ".", ":!.takt"]);
    git(&tree, &["commit", "-q", "-m", "change the guide"]);
    let revision = git(&tree, &["rev-parse", "HEAD"]);
    run.case
        .set_revision("implementation", &revision)
        .expect("the revision moves");
    let short = &revision[..8];
    for (id, kind, result, producer) in [
        (
            "e0".to_owned(),
            "test_result",
            Some("pass"),
            "github-forge-test",
        ),
        (
            "e1".to_owned(),
            "gate_result",
            Some("pass"),
            "github-forge-test",
        ),
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
    let mut out = Vec::new();
    let stop = run.run(&mut out).expect("the run stops without an error");
    let out = String::from_utf8_lossy(&out).into_owned();
    let dir = run.case.dir.clone();

    assert_eq!(stop, Stop::Completed("accepted".to_owned()), "{out}");
    let merged = records_of(&dir, "merge_observation");
    assert_eq!(merged.len(), 1, "{out}");
    assert_eq!(merged[0]["result"], "merged", "{out}");
    let opened = records_of(&dir, "pull_request");
    assert!(
        opened.iter().any(|record| record["result"] == "open"),
        "the pull request was opened: {opened:?}\n{out}"
    );
    let authority = Authority::read(&dir)
        .expect("authority.yaml reads")
        .expect("a grant");
    assert_eq!(
        authority
            .decision("repository.merge")
            .map(|grant| grant.approver.as_str()),
        Some("agent:takt-delivery"),
        "{out}"
    );
    (stand_in, pushes, out)
}

/// What the stand-in of a [`delivered`] run saw: only `GET /v1/describe` and
/// `POST /v1alpha2/invoke`, no credential, the instance described once, every operation of the
/// map invoked at the described revision, the merge at the pushed head, and the runner running
/// only the push and the read of the target's tip.
fn served_wire_of_a_delivered_run(stand_in: &StandIn, pushes: &Pushes, out: &str) {
    stand_in.assert_served_wire_only();
    let seen = stand_in.seen();
    assert_eq!(
        seen.iter().filter(|seen| seen.path == DESCRIBE).count(),
        1,
        "the instance is described once"
    );
    let invoked = stand_in.invoked();
    for operation in OPERATIONS {
        assert!(
            invoked.iter().any(|name| name == operation),
            "{operation} was not invoked: {invoked:?}\n{out}"
        );
    }
    for envelope in seen.iter().filter(|seen| seen.path == INVOKE) {
        assert_eq!(envelope.body["version"], "v1alpha2", "{envelope:?}");
        assert_eq!(envelope.body["revision"], REVISION, "{envelope:?}");
    }
    let pushed = stand_in
        .world
        .lock()
        .expect("the world")
        .pull
        .clone()
        .expect("a pull request");
    let head = stand_in.world.lock().expect("the world").branches[&pushed.head].clone();
    assert_eq!(
        stand_in.inputs_of("pull_request.merge"),
        [json!({"owner": "owner", "repo": "repo", "pull_number": NUMBER, "body": {"sha": head}})],
        "the merge sends the head sha"
    );
    let create = &stand_in.inputs_of("pull_request.create")[0];
    assert_eq!(create["body"]["head"], pushed.head.as_str(), "{create}");
    assert_eq!(create["body"]["base"], "main", "{create}");
    assert_eq!(
        stand_in.inputs_of("branch.get")[0],
        json!({"owner": "owner", "repo": "repo", "branch": "main"})
    );
    let lines = pushes.lines();
    assert!(
        lines.iter().all(|argv| argv[0] == "git"
            && argv
                .iter()
                .any(|word| word == "push" || word == "ls-remote")),
        "the runner ran only the push and the read of the target's tip: {lines:?}"
    );
    assert!(!lines.is_empty(), "the branch was pushed");
}

/// The forge-level read the run makes: create, then read, with the GitHub fields mapped.
#[test]
fn the_pull_request_is_read_from_its_github_fields() {
    let scratch = Scratch::new("read");
    let (forge, stand_in, _, tree, head) = forge_over(&scratch, branches);
    let number = forge
        .open_pull_request(
            &tree,
            &scratch.join("artifacts"),
            &PullRequest {
                branch: BRANCH.to_owned(),
                base: "main".to_owned(),
                title: "A title".to_owned(),
                body: "the body".to_owned(),
            },
        )
        .expect("opens");
    assert_eq!(number, NUMBER);
    assert_eq!(
        stand_in.inputs_of("pull_request.create"),
        [json!({
            "owner": "owner",
            "repo": "repo",
            "sha": head,
            "body": {"head": BRANCH, "base": "main", "title": "A title", "body": "the body"},
        })]
    );
    let view = forge.pull_request(&tree, number).expect("reads");
    assert_eq!(view.state, PullRequestState::Open);
    assert_eq!(view.head.branch, BRANCH);
    assert_eq!(view.head.sha, head);
    assert_eq!(view.base, "main");
    assert_eq!(
        stand_in.inputs_of("pull_request.get"),
        [json!({"owner": "owner", "repo": "repo", "pull_number": NUMBER})]
    );
    for (state, merged, expected) in [
        ("closed", true, PullRequestState::Merged),
        ("closed", false, PullRequestState::Closed),
    ] {
        if let Some(pull) = stand_in.world.lock().expect("the world").pull.as_mut() {
            pull.state = state;
            pull.merged = merged;
        }
        assert_eq!(
            forge.pull_request(&tree, number).expect("reads").state,
            expected,
            "{state} merged {merged}"
        );
    }
    stand_in.assert_served_wire_only();
}

/// The checks of a world whose head carries `runs` and the combined status `combined`, and the
/// inputs the two reads were given.
fn checks_of(
    runs: &[(&'static str, Option<&'static str>)],
    combined: (&'static str, usize),
) -> (Checks, Vec<Value>, Vec<Value>, String) {
    let scratch = Scratch::new(&format!("checks-{}-{}", runs.len(), combined.0));
    let (forge, stand_in, _, tree, head) = forge_over(&scratch, |main, head| {
        let mut world = branches(main, head).with_open_pull();
        world.check_runs = runs.to_vec();
        world.combined = combined;
        world
    });
    let checks = forge.checks(&tree, NUMBER).expect("checks");
    stand_in.assert_served_wire_only();
    (
        checks,
        stand_in.inputs_of("check_runs.list"),
        stand_in.inputs_of("commit.status"),
        head,
    )
}

/// Acceptance 2: check runs `[completed/success, completed/skipped]` with the combined status
/// `pending` and no statuses pass; one `completed/timed_out` fails; one `in_progress` is
/// pending. Both reads are for the head sha the pull request reports.
#[test]
fn the_checks_rule_for_github() {
    let (checks, runs, status, head) = checks_of(
        &[
            ("completed", Some("success")),
            ("completed", Some("skipped")),
        ],
        ("pending", 0),
    );
    assert_eq!(checks, Checks::Pass);
    assert_eq!(runs.len(), 1, "{runs:?}");
    assert_eq!(runs[0]["ref"], head.as_str(), "{runs:?}");
    assert_eq!(
        status,
        [json!({"owner": "owner", "repo": "repo", "ref": head})]
    );

    let (checks, ..) = checks_of(&[("completed", Some("timed_out"))], ("pending", 0));
    assert_eq!(checks, Checks::Fail);

    let (checks, ..) = checks_of(&[("in_progress", None)], ("pending", 0));
    assert_eq!(checks, Checks::Pending);
}

/// The rest of the rule: `neutral` passes as `success` and `skipped` do; `failure` and
/// `cancelled` fail as `timed_out` does, whatever else is pending; the combined status must be
/// `success`, or `pending` with no statuses: one pending status keeps the checks pending, a
/// `failure` fails them. A head on which nothing at all has reported, no check run and no
/// status, is pending: no check is never a pass.
#[test]
fn the_checks_rule_for_github_beyond_the_acceptance() {
    for (runs, combined, expected) in [
        (
            vec![("completed", Some("neutral"))],
            ("success", 2),
            Checks::Pass,
        ),
        (
            vec![("completed", Some("failure")), ("in_progress", None)],
            ("pending", 0),
            Checks::Fail,
        ),
        (
            vec![("completed", Some("cancelled"))],
            ("success", 1),
            Checks::Fail,
        ),
        (
            vec![("completed", Some("success"))],
            ("pending", 1),
            Checks::Pending,
        ),
        (
            vec![("completed", Some("success"))],
            ("failure", 1),
            Checks::Fail,
        ),
        (
            vec![("completed", Some("action_required"))],
            ("pending", 0),
            Checks::Pending,
        ),
        (vec![], ("success", 1), Checks::Pass),
        (vec![], ("pending", 0), Checks::Pending),
    ] {
        let (checks, ..) = checks_of(&runs, combined);
        assert_eq!(checks, expected, "{runs:?} {combined:?}");
    }
}

/// Check runs past one page are read page by page, and every one counts.
#[test]
fn every_page_of_check_runs_is_read() {
    let mut runs = vec![("completed", Some("success")); 150];
    runs.push(("completed", Some("timed_out")));
    let (checks, inputs, ..) = checks_of(&runs, ("pending", 0));
    assert_eq!(checks, Checks::Fail);
    assert_eq!(
        inputs
            .iter()
            .map(|input| input["page"].as_u64())
            .collect::<Vec<_>>(),
        [Some(1), Some(2)],
        "{inputs:?}"
    );
}

/// Acceptance 3: a merge the stand-in answers 409 is no merge; the forge reads the pull request
/// again before any further write, and a head that moved meanwhile is not merged at all.
#[test]
fn a_merge_answered_409_is_no_merge_and_the_pull_request_is_read_again() {
    let scratch = Scratch::new("conflict-moved");
    let (forge, stand_in, _, tree, head) = forge_over(&scratch, |main, head| {
        let mut world = branches(main, head).with_open_pull();
        world.conflicts = 1;
        world.moved_to = Some(OTHER.to_owned());
        world
    });
    let artifacts = scratch.join("artifacts");
    let error = forge
        .merge(&tree, &artifacts, NUMBER, &head)
        .expect_err("a 409 is no merge");
    let said = format!("{error:#}");
    assert!(said.contains("409"), "{said}");
    assert!(said.contains("nothing was merged"), "{said}");
    assert_eq!(stand_in.invoked(), ["pull_request.merge"]);

    let error = forge
        .merge(&tree, &artifacts, NUMBER, &head)
        .expect_err("the head moved");
    let said = format!("{error:#}");
    assert!(said.contains(OTHER), "{said}");
    assert_eq!(
        stand_in.invoked(),
        ["pull_request.merge", "pull_request.get"],
        "read again before the next write, and no merge of a head that moved"
    );
    let view = forge.pull_request(&tree, NUMBER).expect("reads");
    assert_eq!(view.state, PullRequestState::Open);

    let scratch = Scratch::new("conflict-open");
    let (forge, stand_in, _, tree, _) = forge_over(&scratch, |main, head| {
        let mut world = branches(main, head).with_open_pull();
        world.conflicts = 1;
        world
    });
    let request = PullRequest {
        branch: BRANCH.to_owned(),
        base: "main".to_owned(),
        title: "A title".to_owned(),
        body: "the body".to_owned(),
    };
    forge
        .merge(&tree, &scratch.join("artifacts"), NUMBER, OTHER)
        .expect_err("a 409 is no merge");
    forge
        .open_pull_request(&tree, &scratch.join("artifacts"), &request)
        .expect("opens");
    assert_eq!(
        stand_in.invoked(),
        [
            "pull_request.merge",
            "pull_request.get",
            "pull_request.create"
        ],
        "the pull request is read again before the next write of any kind"
    );
    stand_in.assert_served_wire_only();
}

/// After a 409 on a head that did not move, the merge is taken again once the pull request is
/// read; a read the caller makes counts, and no second read is added.
#[test]
fn after_a_409_a_read_comes_before_the_merge_is_taken_again() {
    let scratch = Scratch::new("conflict-again");
    let (forge, stand_in, _, tree, head) = forge_over(&scratch, |main, head| {
        let mut world = branches(main, head).with_open_pull();
        world.conflicts = 1;
        world
    });
    let artifacts = scratch.join("artifacts");
    forge
        .merge(&tree, &artifacts, NUMBER, &head)
        .expect_err("a 409 is no merge");
    forge
        .merge(&tree, &artifacts, NUMBER, &head)
        .expect("merged once read again");
    assert_eq!(
        stand_in.invoked(),
        [
            "pull_request.merge",
            "pull_request.get",
            "pull_request.merge"
        ]
    );
    let merged = forge.pull_request(&tree, NUMBER).expect("reads");
    assert_eq!(merged.state, PullRequestState::Merged);

    let scratch = Scratch::new("conflict-caller-reads");
    let (forge, stand_in, _, tree, head) = forge_over(&scratch, |main, head| {
        let mut world = branches(main, head).with_open_pull();
        world.conflicts = 1;
        world
    });
    let artifacts = scratch.join("artifacts");
    forge
        .merge(&tree, &artifacts, NUMBER, &head)
        .expect_err("a 409 is no merge");
    forge.pull_request(&tree, NUMBER).expect("reads");
    forge
        .merge(&tree, &artifacts, NUMBER, &head)
        .expect("merged");
    assert_eq!(
        stand_in.invoked(),
        [
            "pull_request.merge",
            "pull_request.get",
            "pull_request.merge"
        ]
    );
}

/// The merge target's tip is the base branch's `commit.sha` through `branch.get` when
/// `git ls-remote` of the project `origin` names lists the branch at the same commit, a commit
/// the tree holds; one it does not hold is fetched through the runner from that project before
/// it is answered.
#[test]
fn the_target_tip_is_the_base_branchs_commit() {
    let scratch = Scratch::new("tip");
    let (forge, stand_in, pushes, tree, _) = forge_over(&scratch, branches);
    let main = git(&tree, &["rev-parse", "main"]);
    assert_eq!(forge.target_tip(&tree, "main").expect("a tip"), main);
    assert_eq!(
        stand_in.inputs_of("branch.get"),
        [json!({"owner": "owner", "repo": "repo", "branch": "main"})]
    );
    assert_eq!(
        pushes.lines(),
        [[
            "git",
            "-c",
            "core.hooksPath=/dev/null",
            "-c",
            "http.followRedirects=false",
            "ls-remote",
            "--refs",
            "https://github.example/owner/repo.git",
            "refs/heads/main",
        ]],
        "a held tip is read from origin and not fetched"
    );
    assert!(forge.target_tip(&tree, "absent").is_err());
}

/// `GIT_ALLOW_PROTOCOL` of each line the runner was given, in order.
fn allowed_protocols(pushes: &Pushes) -> Vec<String> {
    pushes
        .envs()
        .iter()
        .map(|env| {
            env.iter()
                .find(|(name, _)| name == "GIT_ALLOW_PROTOCOL")
                .map(|(_, value)| value.clone())
                .unwrap_or_default()
        })
        .collect()
}

/// An `origin` that is `http://` to a loopback relay with a port is where the push, the read
/// and the fetch of the merge target's tip go: that URL without its userinfo, the port kept, no
/// redirect followed, no proxy, `http` allowed beside `https` and nothing else. An `https://` origin is
/// pushed to over HTTPS alone, and an `http://` origin to another host is refused before
/// anything is opened, naming it without its userinfo.
#[test]
fn a_loopback_relay_origin_keeps_its_port() {
    let relay = "http://127.0.0.1:8403/owner/repo.git";
    let scratch = Scratch::new("relay");
    let (tree, main, head) = repository(&scratch);
    git(
        &tree,
        &[
            "remote",
            "set-url",
            "origin",
            "http://agent:secret@127.0.0.1:8403/owner/repo.git",
        ],
    );
    let stand_in = StandIn::start(World::new(&[("main", OTHER), (BRANCH, &head)]));
    let pushes = Pushes::new(stand_in.world.clone());
    let forge = select(&scratch, &tree, &stand_in, &pushes).expect("the forge is selected");
    forge.push(&tree, BRANCH).expect("the push");
    assert_eq!(forge.target_tip(&tree, "main").expect("a tip"), OTHER);
    let refspec = format!("{head}:refs/heads/{BRANCH}");
    assert_eq!(
        pushes.lines(),
        [
            vec![
                "git",
                "-c",
                "http.followRedirects=false",
                "-c",
                "http.proxy=",
                "push",
                relay,
                &refspec,
                "--no-follow-tags",
                "--recurse-submodules=no",
            ],
            vec![
                "git",
                "-c",
                "core.hooksPath=/dev/null",
                "-c",
                "http.followRedirects=false",
                "-c",
                "http.proxy=",
                "ls-remote",
                "--refs",
                relay,
                "refs/heads/main",
            ],
            vec![
                "git",
                "-c",
                "core.hooksPath=/dev/null",
                "-c",
                "http.followRedirects=false",
                "-c",
                "http.proxy=",
                "fetch",
                relay,
                OTHER,
                "--no-write-fetch-head",
                "--no-tags",
                "--recurse-submodules=no",
            ],
        ]
    );
    assert_eq!(
        allowed_protocols(&pushes),
        ["http:https", "http:https", "http:https"]
    );
    assert_ne!(main, OTHER);

    let scratch = Scratch::new("over-https");
    let (forge, _stand_in, pushes, tree, head) = forge_over(&scratch, branches);
    forge.push(&tree, BRANCH).expect("the push");
    assert_eq!(
        pushes.lines(),
        [vec![
            "git".to_owned(),
            "push".to_owned(),
            "https://github.example/owner/repo.git".to_owned(),
            format!("{head}:refs/heads/{BRANCH}"),
            "--no-follow-tags".to_owned(),
            "--recurse-submodules=no".to_owned(),
        ]]
    );
    assert_eq!(allowed_protocols(&pushes), ["https"]);

    let scratch = Scratch::new("plain-http");
    let (tree, main, head) = repository(&scratch);
    git(
        &tree,
        &[
            "remote",
            "set-url",
            "origin",
            "http://agent:secret@github.example:8403/owner/repo.git",
        ],
    );
    let stand_in = StandIn::start(branches(&main, &head));
    let pushes = Pushes::new(stand_in.world.clone());
    let refused = select(&scratch, &tree, &stand_in, &pushes)
        .expect_err("an http origin to another host is refused")
        .to_string();
    assert!(refused.contains("no loopback host"), "{refused}");
    assert!(!refused.contains("secret"), "{refused}");
    assert!(pushes.lines().is_empty());
    assert!(stand_in.seen().is_empty(), "nothing was called");
}

/// The preflight describes the instance once before anything is opened: an instance named
/// other than the one `forge_instance` names after its `/`, or one that does not describe an
/// operation the forge needs, is refused naming it, and nothing is invoked.
#[test]
fn the_preflight_describes_the_instance() {
    let scratch = Scratch::new("preflight");
    let (tree, main, head) = repository(&scratch);
    let mut world = branches(&main, &head);
    world.operations.retain(|name| name != "pull_request.merge");
    let stand_in = StandIn::start(world);
    let pushes = Pushes::new(stand_in.world.clone());
    let said = format!(
        "{:#}",
        select(&scratch, &tree, &stand_in, &pushes).expect_err("refused")
    );
    assert!(said.contains("pull_request.merge"), "{said}");
    assert!(!said.contains("pull_request.get"), "{said}");
    assert!(stand_in.invoked().is_empty());
    assert_eq!(stand_in.seen().len(), 1);
    stand_in.assert_served_wire_only();

    let scratch = Scratch::new("preflight-instance");
    let (tree, main, head) = repository(&scratch);
    let mut world = branches(&main, &head);
    world.instance = "another".to_owned();
    let stand_in = StandIn::start(world);
    let pushes = Pushes::new(stand_in.world.clone());
    let said = format!(
        "{:#}",
        select_named(&scratch, &tree, &stand_in, &pushes, "github/work").expect_err("refused")
    );
    assert!(said.contains("another") && said.contains("work"), "{said}");
    assert!(stand_in.invoked().is_empty());
    select_named(&scratch, &tree, &stand_in, &pushes, "github/another")
        .expect("the instance named is the one served");
    select(&scratch, &tree, &stand_in, &pushes)
        .expect("a target with no instance takes the one the endpoint serves");
    let said = format!(
        "{:#}",
        select_named(&scratch, &tree, &stand_in, &pushes, "gitlab").expect_err("refused")
    );
    assert!(
        said.contains("merge_request.create"),
        "the gitlab map: {said}"
    );

    let scratch = Scratch::new("preflight-identity");
    let (forge, ..) = forge_over(&scratch, branches);
    let identity = forge.identity();
    assert!(identity.contains("github"), "{identity}");
    assert!(identity.contains(REVISION), "{identity}");
}

/// Acceptance 4, in the reader: `forge_endpoint` takes `https://`, or `http://` to a loopback
/// host; `http://example.com` is refused naming the key and its line, and any other shape is
/// refused too, never repeating the value. The key belongs to `forge: connectors`.
#[test]
fn forge_endpoint_is_https_or_http_to_a_loopback_host() {
    let scratch = Scratch::new("endpoint");
    for good in [
        "http://127.0.0.1:8402",
        "http://localhost:8402/",
        "http://[::1]:8402",
        "http://127.0.0.2:1/connectors",
        "https://connectors.example",
        "https://connectors.example:8443/prefix/",
    ] {
        let path = scratch.join("good.yaml");
        fs::write(
            &path,
            format!("forge: connectors\nforge_instance: github\nforge_endpoint: {good}\n"),
        )
        .expect("a file");
        let config = RunConfig::read(&path).unwrap_or_else(|error| panic!("{good}: {error:#}"));
        assert_eq!(config.forge_endpoint.as_deref(), Some(good));
        assert_eq!(config.forge_settings().endpoint.as_deref(), Some(good));
    }
    for (name, text, line) in [
        (
            "plain.yaml",
            "forge: connectors\nforge_instance: github\nforge_endpoint: http://example.com\n",
            3,
        ),
        (
            "plain.toml",
            "forge = \"connectors\"\nforge_endpoint = \"http://example.com\"\nforge_instance = \"github\"\n",
            2,
        ),
    ] {
        let path = scratch.join(name);
        fs::write(&path, text).expect("a file");
        let error = RunConfig::read(&path).expect_err("http to another host");
        let said = format!("{error:#}");
        assert!(said.contains("`forge_endpoint`"), "{said}");
        assert!(said.contains(&format!("at line {line}")), "{said}");
        assert!(!said.contains("example.com"), "{said}");
        assert!(
            error
                .downcast_ref::<takt::run_config::TokensRefused>()
                .is_some(),
            "the refusal is the one takt run exits 2 on: {said}"
        );
    }
    for bad in [
        "http://example.com:8402",
        "http://127.0.0.1.example.com",
        "http://localhost.example",
        "ftp://127.0.0.1",
        "127.0.0.1:8402",
        "https://user:secret@connectors.example",
        "https://connectors.example/?token=x",
        "https://connectors.example/#x",
        "https://:8402",
        "https://connectors example",
        "",
    ] {
        let path = scratch.join("bad.yaml");
        fs::write(&path, format!("forge_endpoint: '{bad}'\n")).expect("a file");
        let error = RunConfig::read(&path).expect_err(bad);
        let said = format!("{error:#}");
        assert!(said.contains("`forge_endpoint`"), "{bad}: {said}");
        assert!(said.contains("at line 1"), "{bad}: {said}");
        assert!(
            bad.is_empty() || !said.contains(bad),
            "the refusal repeats the value: {said}"
        );
        assert!(!said.contains("secret"), "{said}");
    }
    let path = scratch.join("other.yaml");
    fs::write(
        &path,
        "forge: none\nforge_endpoint: https://connectors.example\n",
    )
    .expect("a file");
    let config = RunConfig::read(&path).expect("reads");
    let said = format!(
        "{:#}",
        config
            .forge_settings()
            .check_for(config.forge.as_deref())
            .expect_err("forge_endpoint belongs to forge: connectors")
    );
    assert!(said.contains("forge_endpoint"), "{said}");
}

/// A double of `connectors` on the CLI route: records every argument list and answers each
/// `--describe` and a `pull_request.get` as `connectors --output json` does.
#[derive(Clone, Default)]
struct Program {
    lines: Rc<std::cell::RefCell<Vec<Vec<String>>>>,
}

impl Runner for Program {
    fn run(&self, _tree: &Path, argv: &[String]) -> anyhow::Result<Exit> {
        self.lines.borrow_mut().push(argv.to_vec());
        assert_eq!(
            argv[..5],
            ["connectors", "--output", "json", "invoke", INSTANCE],
            "{argv:?}"
        );
        let operation = argv[5].as_str();
        let result = if argv.iter().any(|word| word == "--describe") {
            json!({
                "connector": "github", "instance": "only", "operation": operation,
                "effect": "read", "schema": format!("s-{operation}"), "revision": REVISION,
                "input_schema": {},
            })
        } else {
            assert_eq!(operation, "pull_request.get", "{argv:?}");
            json!({"result": {"status": 200, "provenance": {}, "body": {
                "state": "open", "merged": false,
                "head": {"ref": BRANCH, "sha": OTHER}, "base": {"ref": "main"},
            }}})
        };
        Ok(Exit {
            code: Some(0),
            stdout: json!({"ok": true, "result": result}).to_string(),
            stderr: String::new(),
        })
    }

    fn run_env(&self, tree: &Path, argv: &[String], _env: &[(&str, &str)]) -> anyhow::Result<Exit> {
        self.run(tree, argv)
    }
}

/// Without `forge_endpoint` the github map goes through `connectors invoke github <operation>`,
/// as the gitlab map does, and nothing is served.
#[test]
fn without_an_endpoint_the_github_map_runs_connectors_invoke() {
    let scratch = Scratch::new("cli");
    let (tree, ..) = repository(&scratch);
    let path = scratch.join("run.yaml");
    fs::write(
        &path,
        format!("forge: connectors\nforge_instance: {INSTANCE}\nscan: none\n"),
    )
    .expect("the run file");
    let config = RunConfig::read(&path).expect("reads");
    let program = Program::default();
    let forge = ConnectorsForge::select(
        &tree,
        config.forge.as_deref(),
        &config.forge_settings(),
        Scan::None { declared_in: path },
        Box::new(program.clone()),
    )
    .expect("selected")
    .expect("the adapter");
    assert_eq!(forge.route(), format!("connectors invoke {INSTANCE}"));
    let view = forge.pull_request(&tree, NUMBER).expect("reads");
    assert_eq!(view.head.sha, OTHER);
    let lines = program.lines.borrow().clone();
    assert_eq!(
        lines
            .iter()
            .take(OPERATIONS.len())
            .map(|argv| (argv[5].clone(), argv[6].clone()))
            .collect::<Vec<_>>(),
        OPERATIONS
            .iter()
            .map(|operation| ((*operation).to_owned(), "--describe".to_owned()))
            .collect::<Vec<_>>()
    );
    let read = lines.last().expect("the read");
    assert_eq!(
        read[6..read.len() - 1],
        [
            "--schema",
            "s-pull_request.get",
            "--revision",
            REVISION,
            "--input-json"
        ]
    );
    assert_eq!(
        serde_json::from_str::<Value>(read.last().expect("an input")).expect("JSON"),
        json!({"owner": "owner", "repo": "repo", "pull_number": NUMBER})
    );
}

// story:connectors-forge-at-a-served-instance: keyed writes under a standing grant.

/// The request for the case branch into `main` the forge-level cases open.
fn the_request() -> PullRequest {
    PullRequest {
        branch: BRANCH.to_owned(),
        base: "main".to_owned(),
        title: "A title".to_owned(),
        body: "the body".to_owned(),
    }
}

/// The artifacts directory of the case `id` under `scratch`, as the validators hand it to the
/// forge: `<case directory>/artifacts`.
fn artifacts_of(scratch: &Scratch, id: &str) -> PathBuf {
    scratch.join("cases").join(id).join("artifacts")
}

/// A world served by a host with standing grants.
fn keyed(main: &str, head: &str) -> World {
    branches(main, head).with_standing_grants()
}

/// Whether `key` has the form the forge sends: `takt-` and 64 lowercase hexadecimal digits,
/// within the 1 to 256 bytes connectors admits.
fn is_takt_key(key: &str) -> bool {
    key.len() <= 256
        && key.strip_prefix("takt-").is_some_and(|digest| {
            digest.len() == 64
                && digest
                    .bytes()
                    .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        })
}

/// Acceptance 2: on a host with standing grants the create and the merge each carry an
/// `idempotency_key`, the reads none; the two keys differ. The same case and write give the same
/// key from another forge, as another process would make it, and another case another key.
#[test]
fn each_write_on_a_host_with_standing_grants_carries_a_key_of_its_case_and_write() {
    let scratch = Scratch::new("keyed");
    let (tree, main, head) = repository(&scratch);
    let over = |world: World| {
        let stand_in = StandIn::start(world);
        let pushes = Pushes::new(stand_in.world.clone());
        let forge = select(&scratch, &tree, &stand_in, &pushes).expect("the forge is selected");
        (forge, stand_in)
    };
    let case = artifacts_of(&scratch, "case-a");
    let (forge, stand_in) = over(keyed(&main, &head));
    let number = forge
        .open_pull_request(&tree, &case, &the_request())
        .expect("opens");
    forge.pull_request(&tree, number).expect("reads");
    forge.merge(&tree, &case, number, &head).expect("merges");

    let created = stand_in.keys_of("pull_request.create");
    let merged = stand_in.keys_of("pull_request.merge");
    assert_eq!(
        (created.len(), merged.len()),
        (1, 1),
        "{created:?} {merged:?}"
    );
    let create = created[0].clone().expect("the create carries a key");
    let merge = merged[0].clone().expect("the merge carries a key");
    assert!(is_takt_key(&create), "{create}");
    assert!(is_takt_key(&merge), "{merge}");
    assert_ne!(create, merge, "another write, another key");
    assert_eq!(
        stand_in.keys_of("pull_request.get"),
        [None],
        "a read carries no key"
    );
    assert_eq!(stand_in.applied(), WRITES);
    assert!(stand_in.refused_members().is_empty());
    stand_in.assert_served_wire_only();

    let (forge, again) = over(keyed(&main, &head));
    forge
        .open_pull_request(&tree, &case, &the_request())
        .expect("opens");
    assert_eq!(
        again.keys_of("pull_request.create"),
        [Some(create.clone())],
        "the same case and write, the same key"
    );

    let (forge, other) = over(keyed(&main, &head));
    forge
        .open_pull_request(&tree, &artifacts_of(&scratch, "case-b"), &the_request())
        .expect("opens");
    let other = other.keys_of("pull_request.create")[0]
        .clone()
        .expect("the create carries a key");
    assert!(is_takt_key(&other), "{other}");
    assert_ne!(other, create, "another case, another key");
}

/// Acceptance 5, at the forge: a host without standing grants refuses the key as a member it
/// does not take, before anything runs; the create is sent again with no key and the merge is
/// sent with none, as before keyed writes.
#[test]
fn a_host_without_standing_grants_opens_and_merges_unkeyed() {
    let scratch = Scratch::new("unkeyed");
    let (forge, stand_in, _, tree, head) = forge_over(&scratch, branches);
    let case = artifacts_of(&scratch, "case");
    let number = forge
        .open_pull_request(&tree, &case, &the_request())
        .expect("opens");
    forge.merge(&tree, &case, number, &head).expect("merges");
    assert_eq!(
        stand_in.refused_members(),
        [("pull_request.create".to_owned(), true)],
        "the key is offered once and refused before anything ran"
    );
    assert_eq!(stand_in.keys_of("pull_request.create"), [None]);
    assert_eq!(stand_in.keys_of("pull_request.merge"), [None]);
    let pull = stand_in.world.lock().expect("the world").pull.clone();
    assert!(pull.is_some_and(|pull| pull.merged), "merged");
}

/// Acceptance 5, through a run: on a host without standing grants the run opens and merges with
/// unkeyed writes, the key refused once before anything ran.
#[test]
fn a_run_on_a_host_without_standing_grants_writes_unkeyed() {
    let scratch = Scratch::new("run-unkeyed");
    let (stand_in, _, out) = delivered(&scratch, |main| World::new(&[("main", main)]));
    for write in WRITES {
        let keys = stand_in.keys_of(write);
        assert!(
            !keys.is_empty() && keys.iter().all(Option::is_none),
            "{write}: {keys:?}\n{out}"
        );
    }
    assert_eq!(
        stand_in.refused_members(),
        [("pull_request.create".to_owned(), true)],
        "{out}"
    );
}

/// Acceptances 1 and 2 against the stand-in: a run on a host with standing grants opens, reads
/// and merges with keyed writes; one create and one merge reach the provider.
#[test]
fn a_run_on_a_host_with_standing_grants_opens_and_merges_with_keyed_writes() {
    let scratch = Scratch::new("run-keyed");
    let (stand_in, pushes, out) = delivered(&scratch, |main| {
        World::new(&[("main", main)]).with_standing_grants()
    });
    served_wire_of_a_delivered_run(&stand_in, &pushes, &out);
    assert_eq!(stand_in.applied(), WRITES, "{out}");
    assert!(stand_in.refused_members().is_empty(), "{out}");
    for write in WRITES {
        let keys = stand_in.keys_of(write);
        assert!(
            keys.iter().flatten().all(|key| is_takt_key(key)) && keys.iter().all(Option::is_some),
            "{write}: {keys:?}\n{out}"
        );
    }
}

/// Acceptance 4: `approval_required` and `idempotency_conflict` end the create and the merge
/// naming the instance's code; nothing reaches the provider, and the merge is not called a
/// conflict of what was read.
#[test]
fn a_refused_keyed_write_ends_naming_the_instances_code() {
    for (status, code) in [(403, "approval_required"), (409, "idempotency_conflict")] {
        let scratch = Scratch::new(code);
        let (forge, stand_in, _, tree, head) = forge_over(&scratch, keyed);
        let case = artifacts_of(&scratch, "case");
        stand_in.standing(|standing| standing.refuse.push_back((status, code)));
        let error = forge
            .open_pull_request(&tree, &case, &the_request())
            .expect_err("the create is refused");
        let said = format!("{error:#}");
        assert!(said.contains(code), "{said}");
        assert!(stand_in.applied().is_empty(), "{code}");

        let number = forge
            .open_pull_request(&tree, &case, &the_request())
            .expect("opens once nothing refuses");
        stand_in.standing(|standing| standing.refuse.push_back((status, code)));
        let error = forge
            .merge(&tree, &case, number, &head)
            .expect_err("the merge is refused");
        let said = format!("{error:#}");
        assert!(said.contains(code), "{said}");
        assert!(said.contains("nothing was merged"), "{said}");
        assert!(!said.contains("what was read no longer holds"), "{said}");
        assert_eq!(stand_in.applied(), ["pull_request.create"], "{code}");
    }
}

/// Acceptance 4: a merge answered `outcome_unknown` ends naming the code and never says nothing
/// was merged; the pull request is read before any further write, and a merge it already holds
/// is not sent again.
#[test]
fn after_outcome_unknown_the_pull_request_is_read_before_any_further_write() {
    let scratch = Scratch::new("unknown");
    let (forge, stand_in, _, tree, head) =
        forge_over(&scratch, |main, head| keyed(main, head).with_open_pull());
    let case = artifacts_of(&scratch, "case");
    stand_in.standing(|standing| standing.unknown = 1);
    let error = forge
        .merge(&tree, &case, NUMBER, &head)
        .expect_err("the outcome is unknown");
    let said = format!("{error:#}");
    assert!(said.contains("outcome_unknown"), "{said}");
    assert!(!said.contains("nothing was merged"), "{said}");
    assert!(
        said.contains("read again before any further write"),
        "{said}"
    );

    let error = forge
        .merge(&tree, &case, NUMBER, &head)
        .expect_err("it is merged already");
    let said = format!("{error:#}");
    assert!(said.contains("Merged"), "{said}");
    assert_eq!(
        stand_in.invoked(),
        ["pull_request.merge", "pull_request.get"],
        "read before the next write, and the merge not sent again"
    );
    assert_eq!(stand_in.applied(), ["pull_request.merge"]);

    let scratch = Scratch::new("unknown-open");
    let (forge, stand_in, _, tree, head) =
        forge_over(&scratch, |main, head| keyed(main, head).with_open_pull());
    stand_in.standing(|standing| standing.unknown = 1);
    let case = artifacts_of(&scratch, "case");
    forge
        .merge(&tree, &case, NUMBER, &head)
        .expect_err("the outcome is unknown");
    let _ = forge.open_pull_request(&tree, &case, &the_request());
    assert_eq!(
        stand_in.invoked()[..2],
        ["pull_request.merge", "pull_request.get"],
        "read before a write of any kind"
    );
}

/// A write whose answer was lost is sent again under the same key, and the instance's replay is
/// taken as the original outcome: one create reached the provider, and the pull request's
/// number is the one it opened. A create answered `outcome_unknown` and sent again replays that
/// answer and is not applied again.
#[test]
fn a_write_sent_again_under_its_key_replays_its_first_outcome() {
    let scratch = Scratch::new("replay");
    let (forge, stand_in, _, tree, _) = forge_over(&scratch, keyed);
    let case = artifacts_of(&scratch, "case");
    stand_in.standing(|standing| standing.dropped = 1);
    forge
        .open_pull_request(&tree, &case, &the_request())
        .expect_err("the answer is lost");
    let number = forge
        .open_pull_request(&tree, &case, &the_request())
        .expect("the replay is the first outcome");
    assert_eq!(number, NUMBER);
    let keys = stand_in.keys_of("pull_request.create");
    assert_eq!(keys.len(), 2, "{keys:?}");
    assert!(keys[0].is_some() && keys[0] == keys[1], "{keys:?}");
    assert_eq!(stand_in.applied(), ["pull_request.create"]);

    let scratch = Scratch::new("replay-unknown");
    let (forge, stand_in, _, tree, _) = forge_over(&scratch, keyed);
    let case = artifacts_of(&scratch, "case");
    stand_in.standing(|standing| standing.unknown = 1);
    for _ in 0..2 {
        let error = forge
            .open_pull_request(&tree, &case, &the_request())
            .expect_err("the outcome is unknown");
        let said = format!("{error:#}");
        assert!(said.contains("outcome_unknown"), "{said}");
    }
    let keys = stand_in.keys_of("pull_request.create");
    assert!(keys[0].is_some() && keys[0] == keys[1], "{keys:?}");
    assert_eq!(stand_in.applied(), ["pull_request.create"]);
}

/// A write the provider refused, a merge answered 409, holds its key's answer: taken again after
/// the read it goes under a new key and merges, so a refusal is not replayed forever.
#[test]
fn a_write_the_provider_refused_is_taken_again_under_a_new_key() {
    let scratch = Scratch::new("refused-again");
    let (forge, stand_in, _, tree, head) = forge_over(&scratch, |main, head| {
        let mut world = keyed(main, head).with_open_pull();
        world.conflicts = 1;
        world
    });
    let case = artifacts_of(&scratch, "case");
    let error = forge
        .merge(&tree, &case, NUMBER, &head)
        .expect_err("a 409 is no merge");
    assert!(format!("{error:#}").contains("nothing was merged"));
    forge
        .merge(&tree, &case, NUMBER, &head)
        .expect("merged once read again");
    assert_eq!(
        stand_in.invoked(),
        [
            "pull_request.merge",
            "pull_request.get",
            "pull_request.merge"
        ]
    );
    let keys = stand_in.keys_of("pull_request.merge");
    assert!(
        keys.iter().all(Option::is_some) && keys[0] != keys[1],
        "{keys:?}"
    );
    assert_eq!(
        stand_in.applied(),
        ["pull_request.merge", "pull_request.merge"]
    );
}
