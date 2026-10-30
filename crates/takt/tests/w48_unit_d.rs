//! A merge the served instance answered `outcome_unknown`: the pull request is read again before
//! the next merge, and a pull request still open at the head the merge named takes one fresh key
//! per process; one merged since completes with no merge sent, and one at another head is not
//! merged. The loopback stand-in of w48_adv_b.rs, which can also answer `outcome_unknown` for a
//! merge it never applied.
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

use takt::forge::connectors::ConnectorsForge;
use takt::forge::{Exit, Forge, Runner};
use takt::run_config::RunConfig;
use takt::scan::Scan;

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
        let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
            .join(format!("w48-unit-d-{}-{made}-{name}", std::process::id()));
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
    /// How many of the next writes are answered `outcome_unknown` and stored under their key
    /// without reaching the provider: the write was never made.
    lost: usize,
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
    if standing.lost > 0 {
        standing.lost -= 1;
        let (status, mut answer) =
            failure(id, 502, "outcome_unknown", "the provider's answer was lost");
        answer["mutation"] = json!({
            "classification": "unknown",
            "attempt": {"instance": INSTANCE, "id": format!("lost-{}", standing.keys.len())},
            "original_request_id": id,
            "replayed": false,
        });
        standing
            .keys
            .insert(key, (request, (status, answer.clone())));
        return (status, answer);
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

/// The artifacts directory of the case `id` under `scratch`: `<case directory>/artifacts`.
fn artifacts_of(scratch: &Scratch, id: &str) -> PathBuf {
    scratch.join("cases").join(id).join("artifacts")
}

/// A world served by a host with standing grants.
fn keyed(main: &str, head: &str) -> World {
    branches(main, head).with_standing_grants()
}

/// How many merges reached the provider.
fn merges_applied(stand_in: &StandIn) -> usize {
    stand_in
        .applied()
        .iter()
        .filter(|operation| *operation == "pull_request.merge")
        .count()
}

/// Every merge's `idempotency_key`, in order; a merge sent with none fails the case.
fn merge_keys(stand_in: &StandIn) -> Vec<String> {
    stand_in
        .keys_of("pull_request.merge")
        .into_iter()
        .map(|key| key.expect("every merge is keyed"))
        .collect()
}

/// The case's refusal record as text, empty when there is none.
fn record_of(case: &Path) -> String {
    fs::read_to_string(case.join(ConnectorsForge::REFUSALS)).unwrap_or_default()
}

/// The first merge is answered `outcome_unknown` and never reached the provider; the pull
/// request reads open and unmerged at the head the merge named. A resumed process reads it,
/// sends the merge under a fresh key, and the provider applies it: two keys, one merge, both
/// keys in the case's record.
#[test]
fn a_lost_merge_read_open_at_its_head_merges_on_resume_under_a_fresh_key() {
    let scratch = Scratch::new("lost-resume");
    let (tree, main, head) = repository(&scratch);
    let stand_in = StandIn::start(keyed(&main, &head).with_open_pull());
    stand_in.standing(|standing| standing.lost = 1);
    let pushes = Pushes::new(stand_in.world.clone());
    let case = artifacts_of(&scratch, "case");
    let first = select(&scratch, &tree, &stand_in, &pushes).expect("the forge is selected");
    let error = first
        .merge(&tree, &case, NUMBER, &head)
        .expect_err("outcome_unknown is no merge");
    assert!(
        format!("{error:#}").contains("outcome_unknown"),
        "{error:#}"
    );
    drop(first);

    let resumed = select(&scratch, &tree, &stand_in, &pushes).expect("the forge is selected");
    let outcome = resumed.merge(&tree, &case, NUMBER, &head);
    let keys = merge_keys(&stand_in);
    assert!(
        outcome.is_ok(),
        "the resumed merge failed: {:?}; keys sent: {keys:?}",
        outcome.err().map(|error| format!("{error:#}"))
    );
    assert_eq!(keys.len(), 2, "two merges sent: {keys:?}");
    assert_ne!(keys[0], keys[1], "the resumed merge took a fresh key");
    assert_eq!(
        merges_applied(&stand_in),
        1,
        "one merge reached the provider"
    );
    let invoked = stand_in.invoked();
    assert_eq!(
        invoked[invoked.len() - 2],
        "pull_request.get",
        "the pull request is read before the fresh-key merge: {invoked:?}"
    );
    let record = record_of(&case);
    for key in &keys {
        assert!(
            record.contains(key.as_str()),
            "{key} is not in the case's record: {record}"
        );
    }
    assert!(
        stand_in
            .world
            .lock()
            .expect("the world")
            .pull
            .as_ref()
            .is_some_and(|pull| pull.merged),
        "the pull request is merged"
    );
}

/// The first merge is answered `outcome_unknown` though the provider applied it: the read
/// shows it merged, and the resumed merge completes with no second merge sent.
#[test]
fn an_unknown_merge_read_merged_completes_with_no_second_merge() {
    let scratch = Scratch::new("unknown-merged");
    let (tree, main, head) = repository(&scratch);
    let stand_in = StandIn::start(keyed(&main, &head).with_open_pull());
    stand_in.standing(|standing| standing.unknown = 1);
    let pushes = Pushes::new(stand_in.world.clone());
    let case = artifacts_of(&scratch, "case");
    let first = select(&scratch, &tree, &stand_in, &pushes).expect("the forge is selected");
    first
        .merge(&tree, &case, NUMBER, &head)
        .expect_err("outcome_unknown is no merge");
    drop(first);

    let resumed = select(&scratch, &tree, &stand_in, &pushes).expect("the forge is selected");
    let outcome = resumed.merge(&tree, &case, NUMBER, &head);
    assert!(
        outcome.is_ok(),
        "a merge read merged did not complete: {:?}",
        outcome.err().map(|error| format!("{error:#}"))
    );
    assert_eq!(merge_keys(&stand_in).len(), 1, "no second merge was sent");
    assert_eq!(merges_applied(&stand_in), 1);
}

/// The first merge is answered `outcome_unknown` and the head moves before the next process:
/// no merge is sent, and the error names the head the read showed.
#[test]
fn an_unknown_merge_read_at_another_head_is_not_sent_again() {
    let scratch = Scratch::new("unknown-moved");
    let (tree, main, head) = repository(&scratch);
    let stand_in = StandIn::start(keyed(&main, &head).with_open_pull());
    stand_in.standing(|standing| standing.lost = 1);
    let pushes = Pushes::new(stand_in.world.clone());
    let case = artifacts_of(&scratch, "case");
    let first = select(&scratch, &tree, &stand_in, &pushes).expect("the forge is selected");
    first
        .merge(&tree, &case, NUMBER, &head)
        .expect_err("outcome_unknown is no merge");
    drop(first);
    stand_in
        .world
        .lock()
        .expect("the world")
        .branches
        .insert(BRANCH.to_owned(), OTHER.to_owned());

    let resumed = select(&scratch, &tree, &stand_in, &pushes).expect("the forge is selected");
    let error = resumed
        .merge(&tree, &case, NUMBER, &head)
        .expect_err("a pull request at another head is not merged");
    assert!(format!("{error:#}").contains(OTHER), "{error:#}");
    assert_eq!(merge_keys(&stand_in).len(), 1, "no merge was sent again");
    assert_eq!(merges_applied(&stand_in), 0);
}

/// Within one process the instance answers `outcome_unknown` twice while the pull request stays
/// open at its head: one fresh-key merge, then the merge ends naming `outcome_unknown` and is
/// not sent again.
#[test]
fn one_process_takes_at_most_one_fresh_key_after_outcome_unknown() {
    let scratch = Scratch::new("unknown-twice");
    let (tree, main, head) = repository(&scratch);
    let stand_in = StandIn::start(keyed(&main, &head).with_open_pull());
    stand_in.standing(|standing| standing.lost = 2);
    let pushes = Pushes::new(stand_in.world.clone());
    let case = artifacts_of(&scratch, "case");
    let forge = select(&scratch, &tree, &stand_in, &pushes).expect("the forge is selected");
    for attempt in 0..3 {
        let error = forge
            .merge(&tree, &case, NUMBER, &head)
            .expect_err("outcome_unknown is no merge");
        assert!(
            format!("{error:#}").contains("outcome_unknown"),
            "attempt {attempt}: {error:#}"
        );
    }
    let keys = merge_keys(&stand_in);
    assert_eq!(keys.len(), 2, "one merge and one fresh-key merge: {keys:?}");
    assert_ne!(keys[0], keys[1], "the second merge took a fresh key");
    assert_eq!(merges_applied(&stand_in), 0);
}
