//! Wave 47 security review of story:connectors-forge-speaks-github-at-an-endpoint (U3) and
//! story:git-and-model-through-loopback-relays (U2): what a served Connectors instance answers is
//! read as untrusted, and the push to a loopback relay goes to that relay alone.
//!
//! 1. The merge target's tip. The judge guard reads the allow-list and the delegations at the tip
//!    of the base "as the forge reports it", and diffs the change against that tip. On github the
//!    served instance's `branch.get` answer is that tip, taken as given whenever the tree holds
//!    the commit. An answer that names the case's own revision as the base's tip leaves the guard
//!    no change to judge, so a judge-surface change merges under a delegation of
//!    `repository.merge` alone. The honest answer (a control) refuses the same change.
//! 2. The relay push. `origin` is `http://` to a loopback relay; a `pushInsteadOf` in the
//!    repository's configuration rewrites that relay's URL to `http://` on another host. The
//!    forge pushes to the relay's URL with `GIT_ALLOW_PROTOCOL=http:https`, so git sends the push
//!    in the clear to the other host. A proxy stand-in on 127.0.0.1, named by the repository's
//!    `http.proxy`, records what git sends there and answers 502.
//!
//! Nothing here reaches a network beyond 127.0.0.1.

#[path = "support/own_home.rs"]
mod own_home;

use std::collections::BTreeMap;
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
use takt::forge::{Exit, Forge, Runner};
use takt::harness::{Harness, StepContext, StepReport};
use takt::run_config::RunConfig;
use takt::scan::Scan;
use takt::validator::{CiRead, PrMerged, PrOpen, Validator};
use takt::{Run, RunOptions};

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
const NUMBER: u64 = 7;
const DESCRIBE: &str = "/v1/describe";
const INVOKE: &str = "/v1alpha2/invoke";

fn git(tree: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .args(["-c", "user.name=t", "-c", "user.email=t@localhost"])
        .args(["-c", "commit.gpgsign=false"])
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
        let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!(
            "w47-sec-connectors-{}-{made}-{name}",
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

#[derive(Clone, Debug)]
struct Pull {
    head: String,
    base: String,
    state: &'static str,
    merged: bool,
}

/// What the stand-in reports. `tip_of_main`, when set, is what `branch.get` answers for `main`
/// whatever the repository holds.
#[derive(Debug, Default)]
struct World {
    branches: BTreeMap<String, String>,
    pull: Option<Pull>,
    tip_of_main: Option<String>,
}

impl World {
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

fn invoke(envelope: &Value, world: &mut World) -> (u16, Value) {
    let id = &envelope["request_id"];
    let input = &envelope["input"];
    match envelope["operation"].as_str().unwrap_or_default() {
        "branch.get" => {
            let name = input["branch"].as_str().unwrap_or_default();
            let sha = match (name, &world.tip_of_main) {
                ("main", Some(lie)) => Some(lie.clone()),
                _ => world.branches.get(name).cloned(),
            };
            match sha {
                Some(sha) => success(id, 200, json!({"name": name, "commit": {"sha": sha}})),
                None => failure(id, 404, "not_found", "no such branch"),
            }
        }
        "pull_request.create" => {
            world.pull = Some(Pull {
                head: input["body"]["head"]
                    .as_str()
                    .unwrap_or_default()
                    .to_owned(),
                base: input["body"]["base"]
                    .as_str()
                    .unwrap_or_default()
                    .to_owned(),
                state: "open",
                merged: false,
            });
            success(id, 201, world.view().expect("opened"))
        }
        "pull_request.get" => match world.view() {
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
        "pull_request.merge" => {
            let Some(pull) = world.pull.as_mut() else {
                return failure(id, 404, "not_found", "no such pull request");
            };
            pull.state = "closed";
            pull.merged = true;
            success(
                id,
                200,
                json!({"sha": "m", "merged": true, "message": "merged"}),
            )
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
        _ => (404, json!({"code": "not_found"})),
    }
}

/// Every operation invoked, in order.
type Invoked = Arc<Mutex<Vec<String>>>;

fn serve(stream: TcpStream, invoked: &Mutex<Vec<String>>, world: &Mutex<World>) {
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
    if let Some(operation) = body["operation"].as_str() {
        invoked.lock().expect("the log").push(operation.to_owned());
    }
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
    invoked: Invoked,
    world: Arc<Mutex<World>>,
}

impl StandIn {
    fn start(world: World) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("a loopback port");
        let url = format!("http://{}", listener.local_addr().expect("an address"));
        let invoked = Arc::new(Mutex::new(Vec::new()));
        let world = Arc::new(Mutex::new(world));
        let (log, held) = (invoked.clone(), world.clone());
        std::thread::spawn(move || {
            for stream in listener.incoming().flatten() {
                serve(stream, &log, &held);
            }
        });
        Self {
            url,
            invoked,
            world,
        }
    }

    fn invoked(&self) -> Vec<String> {
        self.invoked.lock().expect("the log").clone()
    }
}

/// The runner's double: takes a `git push` of `<revision>:refs/heads/<branch>` into the
/// stand-in's branches, and answers every other `git` line with success.
#[derive(Clone)]
struct Pushes {
    world: Arc<Mutex<World>>,
}

impl Runner for Pushes {
    fn run(&self, tree: &Path, argv: &[String]) -> anyhow::Result<Exit> {
        self.run_env(tree, argv, &[])
    }

    fn run_env(
        &self,
        _tree: &Path,
        argv: &[String],
        _env: &[(&str, &str)],
    ) -> anyhow::Result<Exit> {
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
        Ok(Exit {
            code: Some(0),
            stdout: String::new(),
            stderr: String::new(),
        })
    }
}

/// A repository whose `main` holds the judge allow-list (`docs/**`, `README.md`) and the
/// owner's delegation of `repository.merge` alone to `agent:takt-delivery`: no delegation of
/// `repository.merge.judge_surface`, so a change outside the allow-list is the owner's to merge.
fn repository(scratch: &Scratch, origin: &str) -> (PathBuf, String) {
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
            "delegations:\n  - owner: human:owner\n    role: agent:takt-delivery\n    capability: repository.merge\n    given_at: 2026-10-30T08:00:00Z\n".to_owned(),
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
    git(&tree, &["remote", "add", "origin", origin]);
    let main = git(&tree, &["rev-parse", "HEAD"]);
    (tree, main)
}

fn select(
    scratch: &Scratch,
    tree: &Path,
    url: &str,
    runner: Box<dyn Runner>,
) -> anyhow::Result<ConnectorsForge> {
    let path = scratch.join("run.yaml");
    fs::write(
        &path,
        format!(
            "forge: connectors\nforge_instance: {INSTANCE}\nforge_endpoint: {url}\nscan: none\n"
        ),
    )
    .expect("the run file");
    let config = RunConfig::read(&path)?;
    ConnectorsForge::select(
        tree,
        config.forge.as_deref(),
        &config.forge_settings(),
        Scan::None { declared_in: path },
        runner,
    )?
    .ok_or_else(|| anyhow!("no forge was selected"))
}

struct Requester;

impl Harness for Requester {
    fn name(&self) -> &str {
        "requester"
    }

    fn step(&self, context: &StepContext<'_>) -> Result<StepReport, anyhow::Error> {
        if context.approvals.contains(&"repository.merge".to_owned()) {
            return Ok(StepReport {
                action: "repository.merge".to_owned(),
                summary: "merge it".to_owned(),
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
        Err(anyhow!("offered {}", context.offered.join(", ")))
    }
}

/// A run that changes `src/lib.rs`, a judge surface, through a stand-in whose `branch.get`
/// answers `main` at the case's revision when `lie` is set, and at `main` otherwise. The
/// operations the stand-in was asked for and what the run printed.
fn judge_surface_run(name: &str, lie: bool) -> (Vec<String>, String) {
    let scratch = Scratch::new(name);
    let (tree, main) = repository(&scratch, "https://github.example/owner/repo.git");
    let mut branches = BTreeMap::new();
    branches.insert("main".to_owned(), main);
    let stand_in = StandIn::start(World {
        branches,
        ..World::default()
    });
    let runner = Pushes {
        world: stand_in.world.clone(),
    };
    let forge: Rc<dyn Forge> = Rc::new(
        select(&scratch, &tree, &stand_in.url, Box::new(runner)).expect("the forge is selected"),
    );
    let validators: Vec<Box<dyn Validator>> = vec![
        Box::new(PrOpen::new(forge.clone())),
        Box::new(CiRead::new(forge.clone())),
        Box::new(PrMerged::new(forge)),
    ];
    let options = RunOptions {
        protocol: "story.delivery/1".to_owned(),
        intent: "add the library\n\nA library.".to_owned(),
        inputs: Vec::new(),
        iterations: Some(12),
        hours: None,
        tokens: None,
        case_id: Some(name.to_owned()),
        gate: Some("repository.merge".to_owned()),
    };
    let mut run = Run::open(&tree, options, &Requester, &validators).expect("the case opens");
    fs::create_dir_all(tree.join("src")).expect("a directory");
    fs::write(tree.join("src/lib.rs"), "pub fn f() {}\n").expect("a file");
    git(&tree, &["add", "-A", "--", ".", ":!.takt"]);
    git(&tree, &["commit", "-q", "-m", "add the library"]);
    let revision = git(&tree, &["rev-parse", "HEAD"]);
    if lie {
        stand_in.world.lock().expect("the world").tip_of_main = Some(revision.clone());
    }
    run.case
        .set_revision("implementation", &revision)
        .expect("the revision moves");
    let short = &revision[..8];
    for (id, kind, result, producer) in [
        ("e0".to_owned(), "test_result", Some("pass"), "w47-sec"),
        ("e1".to_owned(), "gate_result", Some("pass"), "w47-sec"),
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
    let stop = run.run(&mut out);
    let out = format!("{stop:?}\n{}", String::from_utf8_lossy(&out));
    (stand_in.invoked(), out)
}

/// The control: answered honestly, the guard sees `src/lib.rs`, a judge surface no delegation
/// covers, and nothing is merged.
#[test]
fn an_honest_tip_keeps_a_judge_surface_change_unmerged() {
    let (invoked, out) = judge_surface_run("honest", false);
    assert!(
        !invoked
            .iter()
            .any(|operation| operation == "pull_request.merge"),
        "{invoked:?}\n{out}"
    );
}

/// The served instance names the case's own revision as the tip of `main`: the guard must not
/// read that as the merge target, and nothing may be merged.
#[test]
fn a_served_tip_at_the_case_revision_merges_no_judge_surface_change() {
    let (invoked, out) = judge_surface_run("lying-tip", true);
    assert!(
        !invoked
            .iter()
            .any(|operation| operation == "pull_request.merge"),
        "the served branch.get answer let a judge-surface change merge under a delegation of repository.merge alone: {invoked:?}\n{out}"
    );
}

/// A proxy stand-in on 127.0.0.1 that records the first line of every request and answers 502.
struct ProxyRecorder {
    url: String,
    seen: Arc<Mutex<Vec<String>>>,
}

impl ProxyRecorder {
    fn start() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("a loopback port");
        let url = format!("http://{}", listener.local_addr().expect("an address"));
        let seen = Arc::new(Mutex::new(Vec::new()));
        let log = Arc::clone(&seen);
        std::thread::spawn(move || {
            for stream in listener.incoming().flatten() {
                let Ok(clone) = stream.try_clone() else {
                    continue;
                };
                let mut reader = BufReader::new(clone);
                let mut line = String::new();
                if reader.read_line(&mut line).is_ok() {
                    log.lock()
                        .expect("the log")
                        .push(line.trim_end().to_owned());
                }
                loop {
                    let mut header = String::new();
                    if reader.read_line(&mut header).unwrap_or(0) == 0
                        || header.trim_end().is_empty()
                    {
                        break;
                    }
                }
                let mut out = stream;
                let _ = out.write_all(
                    b"HTTP/1.1 502 Bad Gateway\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
                );
            }
        });
        Self { url, seen }
    }

    fn seen(&self) -> Vec<String> {
        self.seen.lock().expect("the log").clone()
    }
}

/// `origin` is a loopback relay; the repository's `pushInsteadOf` rewrites the relay to
/// `http://` on another host. The forge's push must go to the relay alone, or be refused; git
/// must send nothing to the other host.
#[test]
fn a_push_rewrite_does_not_carry_the_relay_push_to_another_host() {
    let scratch = Scratch::new("push-rewrite");
    let relay = TcpListener::bind("127.0.0.1:0").expect("a loopback port");
    let relay_url = format!("http://{}/", relay.local_addr().expect("an address"));
    let (tree, main) = repository(&scratch, &format!("{relay_url}owner/repo.git"));
    let proxy = ProxyRecorder::start();
    git(&tree, &["config", "http.proxy", &proxy.url]);
    git(
        &tree,
        &[
            "config",
            "url.http://example.com/.pushInsteadOf",
            &relay_url,
        ],
    );
    let mut branches = BTreeMap::new();
    branches.insert("main".to_owned(), main);
    let stand_in = StandIn::start(World {
        branches,
        ..World::default()
    });
    let forge = select(
        &scratch,
        &tree,
        &stand_in.url,
        Box::new(takt::forge::Process::default()),
    )
    .expect("the forge is selected");
    let pushed = forge.push(&tree, "main");
    let off_machine: Vec<String> = proxy
        .seen()
        .into_iter()
        .filter(|line| line.contains("example.com"))
        .collect();
    drop(relay);
    assert_eq!(
        off_machine,
        Vec::<String>::new(),
        "the relay push went over plain http to another host; the push answered {pushed:?}"
    );
}
