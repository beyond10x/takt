//! Integrity conformance cases for the connectors forge's keyed served writes
//! (story:connectors-forge-at-a-served-instance): the refusal record a case keeps
//! (`artifacts/pull-request.refusals.json`) and the fallback to unkeyed writes. The stand-in is
//! the loopback instance of w48_adv_b.rs, cut down to the operations these cases reach.
#![allow(dead_code)]

#[path = "support/own_home.rs"]
mod own_home;

use std::collections::{BTreeMap, VecDeque};
use std::fs;
use std::io::{BufRead as _, BufReader, Read as _, Write as _};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use anyhow::anyhow;
use serde_json::{Value, json};

use takt::forge::connectors::ConnectorsForge;
use takt::forge::{Exit, Forge, PullRequest, Runner};
use takt::run_config::RunConfig;
use takt::scan::Scan;

const INSTANCE: &str = "github";
const REVISION: &str = "descriptor-r1";
const OPERATIONS: [&str; 6] = [
    "branch.get",
    "pull_request.create",
    "pull_request.get",
    "pull_request.merge",
    "check_runs.list",
    "commit.status",
];
const WRITES: [&str; 2] = ["pull_request.create", "pull_request.merge"];
const NUMBER: u64 = 7;
const BRANCH: &str = "takt/case-1";
const DESCRIBE: &str = "/v1/describe";
const INVOKE: &str = "/v1alpha2/invoke";
const NO_ANSWER: u16 = 0;

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

struct Scratch(PathBuf);

impl Scratch {
    fn new(name: &str) -> Self {
        static MADE: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let made = MADE.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
            .join(format!("w48-sec-b-{}-{made}-{name}", std::process::id()));
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

#[derive(Clone, Debug)]
struct Seen {
    path: String,
    body: Value,
}

#[derive(Debug, Default)]
struct Standing {
    keys: BTreeMap<String, (Value, (u16, Value))>,
    applied: Vec<String>,
}

#[derive(Clone, Debug)]
struct Pull {
    head: String,
    base: String,
    state: &'static str,
    merged: bool,
}

#[derive(Debug)]
struct World {
    branches: BTreeMap<String, String>,
    pull: Option<Pull>,
    conflicts: usize,
    standing: Option<Standing>,
    /// Raw answers (an HTTP status and a body) the next keyed invocations get before the host
    /// reads them, one each, in order: what something in front of the instance answered.
    front: VecDeque<(u16, Value)>,
}

impl World {
    fn keyed(main: &str, head: &str) -> Self {
        Self {
            branches: [("main", main), (BRANCH, head)]
                .iter()
                .map(|(name, sha)| ((*name).to_owned(), (*sha).to_owned()))
                .collect(),
            pull: None,
            conflicts: 0,
            standing: Some(Standing::default()),
            front: VecDeque::new(),
        }
    }

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

/// A host with standing grants, as compatibility §2.1 describes it: a write needs a key, a key
/// on a read is `unsupported` with the request ID echoed, the same key and request replays.
fn invoke(envelope: &Value, world: &mut World) -> (u16, Value) {
    if envelope.get("idempotency_key").is_some()
        && let Some(front) = world.front.pop_front()
    {
        return front;
    }
    let id = &envelope["request_id"];
    let operation = envelope["operation"]
        .as_str()
        .unwrap_or_default()
        .to_owned();
    let key = match (
        WRITES.contains(&operation.as_str()),
        envelope.get("idempotency_key"),
    ) {
        (false, None) => return dispatch(envelope, world),
        (false, Some(_)) => return failure(id, 400, "unsupported", "idempotency_key on a read"),
        (true, None) => return failure(id, 400, "invalid_input", "a write needs idempotency_key"),
        (true, Some(key)) => match key.as_str().filter(|key| (1..=256).contains(&key.len())) {
            Some(key) => key.to_owned(),
            None => return failure(id, 400, "invalid_input", "idempotency_key is malformed"),
        },
    };
    let request = json!([operation, envelope["revision"], envelope["input"]]);
    let standing = world.standing.as_mut().expect("standing grants");
    if let Some((held, (status, answer))) = standing.keys.get(&key) {
        if *held != request {
            return failure(id, 409, "idempotency_conflict", "the key was used for another request");
        }
        let mut answer = answer.clone();
        answer["request_id"] = id.clone();
        answer["mutation"]["replayed"] = json!(true);
        return (*status, answer);
    }
    let (status, mut answer) = dispatch(envelope, world);
    let standing = world.standing.as_mut().expect("standing grants");
    standing.applied.push(operation);
    let classification = if answer["status"] == "success" {
        "applied"
    } else {
        "refused"
    };
    answer["mutation"] = json!({
        "classification": classification,
        "attempt": {"instance": INSTANCE, "id": format!("attempt-{}", standing.applied.len())},
        "original_request_id": id,
        "replayed": false,
        "cause": null,
    });
    standing.keys.insert(key, (request, (status, answer.clone())));
    (status, answer)
}

fn dispatch(envelope: &Value, world: &mut World) -> (u16, Value) {
    let id = &envelope["request_id"];
    if envelope["version"] != "v1alpha2" || !id.is_string() {
        return failure(&Value::Null, 400, "invalid_input", "invalid invocation envelope");
    }
    if envelope["revision"] != REVISION {
        return failure(id, 409, "stale_description", "the descriptor revision changed");
    }
    let input = &envelope["input"];
    match envelope["operation"].as_str().unwrap_or_default() {
        "branch.get" => {
            let name = input["branch"].as_str().unwrap_or_default();
            match world.branches.get(name) {
                Some(sha) => success(id, 200, json!({"name": name, "commit": {"sha": sha}})),
                None => failure(id, 404, "not_found", "no such branch"),
            }
        }
        "pull_request.create" => {
            world.pull = Some(Pull {
                head: input["body"]["head"].as_str().unwrap_or_default().to_owned(),
                base: input["body"]["base"].as_str().unwrap_or_default().to_owned(),
                state: "open",
                merged: false,
            });
            success(id, 201, world.view().expect("opened"))
        }
        "pull_request.get" if input["pull_number"] == NUMBER => match world.view() {
            Some(view) => success(id, 200, view),
            None => failure(id, 404, "not_found", "no such pull request"),
        },
        "check_runs.list" => success(
            id,
            200,
            json!({"total_count": 1, "check_runs": [{"status": "completed", "conclusion": "success"}]}),
        ),
        "commit.status" => success(
            id,
            200,
            json!({"state": "pending", "total_count": 0, "statuses": []}),
        ),
        "pull_request.merge" if input["pull_number"] == NUMBER => {
            if world.conflicts > 0 {
                world.conflicts -= 1;
                return failure(id, 409, "conflict", "the target changed since it was read");
            }
            let Some(pull) = world.pull.as_mut() else {
                return failure(id, 404, "not_found", "no such pull request");
            };
            pull.state = "closed";
            pull.merged = true;
            success(id, 200, json!({"sha": "a-merge-commit", "merged": true, "message": "merged"}))
        }
        _ => failure(id, 404, "not_found", "no such pull request"),
    }
}

fn answer(method: &str, path: &str, body: &Value, world: &mut World) -> (u16, Value) {
    match (method, path) {
        ("GET", DESCRIBE) => (
            200,
            json!({
                "version": "v1alpha1",
                "instance": INSTANCE,
                "adapter": "catalog",
                "revision": REVISION,
                "operations": OPERATIONS.iter().map(|id| json!({
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
        _ => (404, json!({"code": "not_found", "message": "no such route"})),
    }
}

fn serve(stream: TcpStream, seen: &Mutex<Vec<Seen>>, world: &Mutex<World>) {
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
    seen.lock().expect("the log").push(Seen { path, body });
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

    /// The `idempotency_key` of every invocation of `operation`, `None` for one sent with none.
    fn keys_of(&self, operation: &str) -> Vec<Option<String>> {
        self.seen
            .lock()
            .expect("the log")
            .iter()
            .filter(|seen| seen.path == INVOKE && seen.body["operation"] == operation)
            .map(|seen| seen.body["idempotency_key"].as_str().map(str::to_owned))
            .collect()
    }
}

#[derive(Clone)]
struct Pushes {
    world: Arc<Mutex<World>>,
}

impl Runner for Pushes {
    fn run(&self, tree: &Path, argv: &[String]) -> anyhow::Result<Exit> {
        self.run_env(tree, argv, &[])
    }

    fn run_env(&self, _tree: &Path, argv: &[String], _env: &[(&str, &str)]) -> anyhow::Result<Exit> {
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

/// A repository at `<scratch>/tree` with `main` and the case branch one commit above it.
fn repository(scratch: &Path) -> (PathBuf, String, String) {
    let tree = scratch.join("tree");
    fs::create_dir_all(tree.join("docs")).expect("a directory");
    fs::write(tree.join("README.md"), "a repository\n").expect("a file");
    fs::write(tree.join("docs/guide.md"), "a guide\n").expect("a file");
    git(&tree, &["init", "-q", "-b", "main"]);
    git(&tree, &["add", "-A"]);
    git(&tree, &["commit", "-q", "-m", "start"]);
    git(
        &tree,
        &["remote", "add", "origin", "https://github.example/owner/repo.git"],
    );
    let main = git(&tree, &["rev-parse", "HEAD"]);
    git(&tree, &["checkout", "-q", "-b", BRANCH]);
    fs::write(tree.join("docs/guide.md"), "a guide, changed\n").expect("a file");
    git(&tree, &["commit", "-q", "-am", "change the guide"]);
    let head = git(&tree, &["rev-parse", "HEAD"]);
    git(&tree, &["checkout", "-q", "main"]);
    (tree, main, head)
}

fn select(scratch: &Path, tree: &Path, stand_in: &StandIn) -> ConnectorsForge {
    let path = scratch.join("run.yaml");
    fs::write(
        &path,
        format!(
            "forge: connectors\nforge_instance: {INSTANCE}\nforge_endpoint: {}\nscan: none\n",
            stand_in.url
        ),
    )
    .expect("the run file");
    let config = RunConfig::read(&path).expect("the run file reads");
    ConnectorsForge::select(
        tree,
        config.forge.as_deref(),
        &config.forge_settings(),
        Scan::None { declared_in: path },
        Box::new(Pushes {
            world: stand_in.world.clone(),
        }),
    )
    .expect("the forge is selected")
    .ok_or_else(|| anyhow!("no forge was selected"))
    .expect("a forge")
}

fn the_request() -> PullRequest {
    PullRequest {
        branch: BRANCH.to_owned(),
        base: "main".to_owned(),
        title: "A title".to_owned(),
        body: "the body".to_owned(),
    }
}

/// `<scratch>/cases/case/artifacts`, made.
fn artifacts(scratch: &Path) -> PathBuf {
    let dir = scratch.join("cases").join("case").join("artifacts");
    fs::create_dir_all(&dir).expect("the artifacts directory");
    dir
}

// I3: the refusal record is a trust boundary.

/// The refusal record is written to a file beside it and renamed over it. A link planted at
/// that file's name (`.pull-request.refusals.json.<pid>.partial`, the pid of the process) is
/// followed: the record's bytes land in the file the link names, outside the case.
#[test]
fn the_refusal_record_does_not_write_through_a_link_at_its_partial_name() {
    let scratch = Scratch::new("partial-link");
    let (tree, main, head) = repository(&scratch);
    let stand_in = StandIn::start({
        let mut world = World::keyed(&main, &head).with_open_pull();
        world.conflicts = 1;
        world
    });
    let case = artifacts(&scratch);
    let outside = scratch.join("outside");
    fs::create_dir_all(&outside).expect("a directory outside the case");
    let victim = outside.join("kept.txt");
    fs::write(&victim, "kept\n").expect("a file outside the case");
    let partial = case.join(format!(
        ".{}.{}.partial",
        ConnectorsForge::REFUSALS,
        std::process::id()
    ));
    std::os::unix::fs::symlink(&victim, &partial).expect("a link at the partial name");

    let forge = select(&scratch, &tree, &stand_in);
    let outcome = forge.merge(&tree, &case, NUMBER, &head);
    assert!(outcome.is_err(), "the provider refused the merge");
    assert_eq!(
        fs::read_to_string(&victim).expect("the outside file"),
        "kept\n",
        "recording the refusal wrote through the link into {}; the merge said {:#}",
        victim.display(),
        outcome.err().expect("an error")
    );
}

/// A refusal record that is a link to a FIFO outside the case: the forge opens and reads it
/// through the link, with no bound on its size, and the write waits on it for as long as nothing
/// writes the FIFO. It must refuse such a record (or not read it) and return.
#[test]
fn a_refusal_record_that_links_out_of_the_case_is_not_read_through() {
    let scratch = Scratch::new("record-link");
    let (tree, main, head) = repository(&scratch);
    let stand_in = StandIn::start(World::keyed(&main, &head));
    let case = artifacts(&scratch);
    let outside = scratch.join("outside");
    fs::create_dir_all(&outside).expect("a directory outside the case");
    let fifo = outside.join("fifo");
    let made = Command::new("mkfifo").arg(&fifo).status().expect("mkfifo runs");
    assert!(made.success(), "a FIFO outside the case");
    std::os::unix::fs::symlink(&fifo, case.join(ConnectorsForge::REFUSALS))
        .expect("a link at the record's name");

    let (sent, done) = std::sync::mpsc::channel();
    let (worker_tree, worker_case, worker_scratch) =
        (tree.clone(), case.clone(), scratch.0.clone());
    let url = stand_in.url.clone();
    let world = stand_in.world.clone();
    let seen = stand_in.seen.clone();
    let worker = std::thread::spawn(move || {
        let stand_in = StandIn { url, seen, world };
        let forge = select(&worker_scratch, &worker_tree, &stand_in);
        let outcome = forge
            .open_pull_request(&worker_tree, &worker_case, &the_request())
            .map_err(|error| format!("{error:#}"));
        let _ = sent.send(outcome);
    });
    let outcome = done.recv_timeout(Duration::from_secs(20));
    if outcome.is_err() {
        // Let the blocked reader go: open the FIFO for writing and close it.
        drop(fs::OpenOptions::new().write(true).open(&fifo));
        let _ = worker.join();
        panic!(
            "the open waited 20 s reading the refusal record through a link to {} outside the case",
            fifo.display()
        );
    }
    let _ = worker.join();
}

/// The record's own reader promises that a file that is no record is an error, never a count of
/// zero. A record whose `refused` is no object reads as no refusals at all, and the write goes.
#[test]
fn a_refusal_record_whose_counts_are_no_object_is_an_error() {
    let scratch = Scratch::new("record-shape");
    let (tree, main, head) = repository(&scratch);
    let stand_in = StandIn::start(World::keyed(&main, &head));
    let case = artifacts(&scratch);
    fs::write(
        case.join(ConnectorsForge::REFUSALS),
        r#"{"format":"takt.connectors.refusals/1","refused":["not","counts"]}"#,
    )
    .expect("a record of another shape");

    let forge = select(&scratch, &tree, &stand_in);
    let outcome = forge.open_pull_request(&tree, &case, &the_request());
    assert!(
        outcome.is_err() && stand_in.keys_of("pull_request.create").is_empty(),
        "a record whose counts are no object was read as zero refusals: the open sent {:?} and answered {:?}",
        stand_in.keys_of("pull_request.create"),
        outcome.map_err(|error| format!("{error:#}"))
    );
}

// I2: no unkeyed write on a host with standing grants.

/// A 400 whose body is no v1alpha2 envelope (no `version`, no `status`, no `request_id`), only a
/// top-level `code: unsupported`, as something in front of the instance may answer: the
/// selected client validates the closed envelope (compatibility §5), so this is no refusal of
/// `idempotency_key` by the host. The forge reads the top-level `code` as the envelope's error
/// and sends the write again with no key.
#[test]
fn an_answer_that_is_no_envelope_does_not_drop_the_key() {
    let scratch = Scratch::new("no-envelope");
    let (tree, main, head) = repository(&scratch);
    let stand_in = StandIn::start(World::keyed(&main, &head));
    let case = artifacts(&scratch);
    let forge = select(&scratch, &tree, &stand_in);
    stand_in
        .world
        .lock()
        .expect("the world")
        .front
        .push_back((400, json!({"code": "unsupported", "message": "unsupported"})));

    let outcome = forge.open_pull_request(&tree, &case, &the_request());
    let keys = stand_in.keys_of("pull_request.create");
    assert!(
        keys.iter().all(Option::is_some),
        "after an answer that is no envelope the create went with no key: keys {keys:?}; the open answered {:?}",
        outcome.map_err(|error| format!("{error:#}"))
    );
}
