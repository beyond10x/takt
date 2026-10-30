//! story:second-harness-adapter: one session executor, the harness its kind (Claude Code, Codex,
//! b10x-harness), each given the same brief over the same scratch copy and held to the same
//! proposal, so what a case decides depends on the evidence and never on which harness took the
//! step. Each session here is metaharness's scripted one, the vendor's own record shape per kind:
//! no vendor process, no credential, no network.

#[path = "support/own_home.rs"]
mod own_home;

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::fs;
use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::rc::Rc;

use loom_sdk::session::metaharness::{
    HarnessProcess, LaunchPlanView, ProcessRunner, ScriptedLog, ScriptedRunner,
};
use loom_sdk::session::{CredentialSource, Endpoint, Kind};
use takt::harness::{Harness, SessionHarness};
use takt::run::{CREDENTIAL_SHAPES, PROPOSAL_NOT_RECORDED, credential_shape};
use takt::scan::Scan;
use takt::validator::{TestsRun, Validator};
use takt::{Run, RunOptions, Stop};

const PROTOCOL: &str = "story.delivery/1";

const KINDS: [Kind; 3] = [Kind::Claude, Kind::Codex, Kind::B10x];

const BROKEN: &str = "pub fn add(a: i32, b: i32) -> i32 {\n    a - b\n}\n";
const FIXED: &str = "pub fn add(a: i32, b: i32) -> i32 {\n    a + b\n}\n";

/// The codex version metaharness 0.10.1's Codex adapter is pinned to.
const CODEX_PIN: &str = "0.153.4";

fn git(tree: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .args(["-c", "user.name=t", "-c", "user.email=t@localhost"])
        .args(args)
        .current_dir(tree)
        .output()
        .expect("git runs");
    assert!(output.status.success(), "git {args:?}");
    String::from_utf8(output.stdout)
        .expect("utf-8")
        .trim()
        .to_owned()
}

/// A scratch directory under this crate's build directory, `tree/` in it the repository;
/// removed when dropped.
struct Scratch(PathBuf);

impl Scratch {
    fn tree(&self) -> PathBuf {
        self.0.join("tree")
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn repository(name: &str) -> Scratch {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("parity-{}-{name}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    let scratch = Scratch(dir);
    let tree = scratch.tree();
    fs::create_dir_all(tree.join("src")).expect("the tree");
    fs::write(tree.join("README.md"), "a story\n").expect("a file");
    fs::write(tree.join("src/lib.rs"), BROKEN).expect("a file");
    git(&tree, &["init", "-q", "-b", "main"]);
    git(&tree, &["add", "-A"]);
    git(&tree, &["commit", "-q", "-m", "start"]);
    scratch
}

fn options(id: &str, iterations: u64) -> RunOptions {
    RunOptions {
        protocol: PROTOCOL.to_owned(),
        intent: "make add add".to_owned(),
        inputs: Vec::new(),
        iterations: Some(iterations),
        hours: None,
        tokens: None,
        case_id: Some(id.to_owned()),
        gate: None,
    }
}

/// The tests pass when a tracked file adds, `src/lib.rs` once fixed; the command names no file,
/// so the file is no judge surface.
fn validators() -> Vec<Box<dyn Validator>> {
    vec![Box::new(
        TestsRun::new("git grep -qE a.[+].b$").expect("a command"),
    )]
}

/// Every file under `dir` but `.git/` and `.engineering/`, by path relative to it.
fn work(dir: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
    fn walk(root: &Path, dir: &Path, into: &mut BTreeMap<PathBuf, Vec<u8>>) {
        for entry in fs::read_dir(dir).expect("the listing") {
            let path = entry.expect("an entry").path();
            let relative = path.strip_prefix(root).expect("under").to_path_buf();
            if relative == Path::new(".git")
                || relative.starts_with(".engineering")
                || relative.starts_with(".takt")
            {
                continue;
            }
            if path.is_dir() {
                walk(root, &path, into);
            } else {
                into.insert(relative, fs::read(&path).expect("a file"));
            }
        }
    }
    let mut into = BTreeMap::new();
    walk(dir, dir, &mut into);
    into
}

/// What the scripted session was handed, read after the run.
#[derive(Default)]
struct Seen {
    program: Vec<String>,
    cwd: Vec<PathBuf>,
    env: Vec<BTreeMap<String, String>>,
    args: Vec<Vec<String>>,
}

/// What a scripted session of each kind prints, in that kind's own record: its opening record
/// (Codex's naming `version`), a reply ending in the JSON line naming `action` and `summary`,
/// the tokens when `tokens` is set, and its end.
fn record(
    kind: Kind,
    cwd: &Path,
    action: &str,
    summary: &str,
    tokens: bool,
    version: &str,
) -> Vec<String> {
    let reply = format!(
        "Done.\n{}",
        serde_json::json!({"action": action, "summary": summary})
    );
    let cwd = cwd.display().to_string();
    let lines = match kind {
        Kind::Claude => {
            let mut lines = vec![serde_json::json!({
                "type": "system", "subtype": "init", "claude_code_version": "2.1.304",
                "cwd": cwd, "tools": ["Read", "Edit", "Write", "Glob", "Grep"],
                "mcp_servers": [], "plugins": [], "output_style": "default",
                "apiKeySource": "none", "model": "scripted", "permissionMode": "default",
                "session_id": "s-1",
            })];
            lines.push(serde_json::json!({
                "type": "assistant",
                "message": {"content": [{"type": "text", "text": reply}]},
            }));
            let mut result = serde_json::json!({
                "type": "result", "subtype": "success", "is_error": false, "num_turns": 1,
                "duration_ms": 5, "duration_api_ms": 4, "modelUsage": {"scripted": {}},
            });
            if tokens {
                result["usage"] = serde_json::json!({"input_tokens": 60, "output_tokens": 20,
                    "cache_read_input_tokens": 40, "cache_creation_input_tokens": 0});
            }
            lines.push(result);
            lines
        }
        Kind::Codex => {
            let at = "2026-10-20T18:00:00.000Z";
            let turn = "01a15a19-5963-7d42-b749-21695314f7a4";
            let mut lines = vec![
                serde_json::json!({"timestamp": at, "type": "session_meta", "payload": {
                    "session_id": "01a15a19-57e0-7d83-97b1-da333ebb91e5",
                    "id": "01a15a19-57e0-7d83-97b1-da333ebb91e5", "timestamp": at,
                    "cwd": cwd, "originator": "codex_exec", "cli_version": version,
                    "source": "exec", "model_provider": "openai"}}),
                serde_json::json!({"timestamp": at, "type": "event_msg", "payload": {
                    "type": "task_started", "turn_id": turn}}),
                serde_json::json!({"timestamp": at, "type": "event_msg", "payload": {
                    "type": "agent_message", "message": reply, "phase": "final_answer"}}),
                serde_json::json!({"timestamp": at, "type": "response_item", "payload": {
                    "type": "message", "role": "assistant", "phase": "final_answer",
                    "content": [{"type": "output_text", "text": reply}]}}),
            ];
            if tokens {
                lines.push(
                    serde_json::json!({"timestamp": at, "type": "event_msg", "payload": {
                    "type": "token_count", "info": {"total_token_usage": {"input_tokens": 100,
                    "cached_input_tokens": 40, "output_tokens": 20, "reasoning_output_tokens": 0,
                    "total_tokens": 120}}}}),
                );
            }
            lines.push(
                serde_json::json!({"timestamp": at, "type": "event_msg", "payload": {
                "type": "task_complete", "turn_id": turn, "last_agent_message": reply,
                "duration_ms": 5}}),
            );
            lines
        }
        Kind::B10x => {
            let mut lines = vec![
                serde_json::json!({"kind": "started", "model": "scripted",
                    "published_tools": ["file_read", "file_write"],
                    "operations": ["file.read", "file.write"], "withheld": [], "skills": [],
                    "agents": [], "profiles": [], "context": [], "toolchains": [],
                    "credential_source": "oauth-token:file"}),
                serde_json::json!({"kind": "turn-started", "turn": 1}),
                serde_json::json!({"kind": "text-delta", "text": reply}),
            ];
            if tokens {
                lines.push(serde_json::json!({"kind": "usage", "model": "scripted",
                    "input_tokens": 100, "output_tokens": 20, "cached_input_tokens": 40}));
            }
            lines.push(
                serde_json::json!({"kind": "finished", "stop": {"kind": "completed"},
                "turns": 1}),
            );
            lines
        }
    };
    lines.iter().map(ToString::to_string).collect()
}

/// A scripted session of `kind`: at its start it keeps what it was handed, writes `writes`
/// into its working directory as the model's tools would, and prints [`record`].
struct Session {
    kind: Kind,
    writes: Vec<(String, String)>,
    action: String,
    summary: String,
    tokens: bool,
    version: String,
    /// The `credential_source` its opening record names in place of [`record`]'s, if any.
    source: Option<&'static str>,
    seen: Rc<RefCell<Seen>>,
}

impl ProcessRunner for Session {
    fn requires_executable(&self) -> bool {
        false
    }

    fn start(&mut self, plan: &LaunchPlanView) -> std::io::Result<Box<dyn HarnessProcess>> {
        {
            let mut seen = self.seen.borrow_mut();
            seen.program.push(plan.program.to_owned());
            seen.cwd.push(plan.cwd.to_path_buf());
            seen.env.push(plan.env.clone());
            seen.args.push(plan.args.to_vec());
        }
        for (path, text) in &self.writes {
            let at = plan.cwd.join(path);
            fs::create_dir_all(at.parent().expect("a parent"))?;
            fs::write(at, text)?;
        }
        let mut lines = record(
            self.kind,
            plan.cwd,
            &self.action,
            &self.summary,
            self.tokens,
            &self.version,
        );
        if let Some(source) = self.source {
            for line in &mut lines {
                let mut value: serde_json::Value = serde_json::from_str(line)?;
                if value["kind"] == "started" {
                    value["credential_source"] = source.into();
                    *line = value.to_string();
                }
            }
        }
        ScriptedRunner::of_lines(lines, ScriptedLog::new()).start(plan)
    }
}

/// How a scripted session of one kind behaves.
struct Script<'a> {
    writes: &'a [(&'a str, &'a str)],
    action: &'a str,
    summary: &'a str,
    tokens: bool,
    version: &'a str,
}

impl Script<'_> {
    fn fixing() -> Script<'static> {
        Script {
            writes: &[("src/lib.rs", FIXED)],
            action: "implementation.edit",
            summary: "scripted",
            tokens: true,
            version: CODEX_PIN,
        }
    }
}

/// `--harness <kind>` over a scripted session.
fn harness(kind: Kind, script: &Script<'_>) -> (SessionHarness, Rc<RefCell<Seen>>) {
    harness_saying(kind, script, None)
}

/// [`harness`], a b10x-harness session's opening record naming `source` as its
/// `credential_source` when given, as the loop names the road its key came by.
fn harness_saying(
    kind: Kind,
    script: &Script<'_>,
    source: Option<&'static str>,
) -> (SessionHarness, Rc<RefCell<Seen>>) {
    let seen = Rc::new(RefCell::new(Seen::default()));
    let shared = Rc::clone(&seen);
    let writes: Vec<(String, String)> = script
        .writes
        .iter()
        .map(|(path, text)| ((*path).to_owned(), (*text).to_owned()))
        .collect();
    let (action, summary, tokens, version) = (
        script.action.to_owned(),
        script.summary.to_owned(),
        script.tokens,
        script.version.to_owned(),
    );
    let mut harness = SessionHarness::of(kind);
    harness.credentials = CredentialSource::None;
    if kind == Kind::B10x {
        harness.endpoint = Some(Endpoint {
            base_url: "https://endpoint.invalid/v1".to_owned(),
            wire: "anthropic-messages".to_owned(),
            model: "scripted".to_owned(),
            token_file: PathBuf::from("/nonexistent/token.json"),
            token_pointer: Some("/token".to_owned()),
        });
    }
    let harness = harness.with_runner(move || {
        Box::new(Session {
            kind,
            writes: writes.clone(),
            action: action.clone(),
            summary: summary.clone(),
            tokens,
            version: version.clone(),
            source,
            seen: Rc::clone(&shared),
        })
    });
    (harness, seen)
}

/// The run of `harness` on `tree` for `iterations` steps under the scan adapter `none`, declared
/// in a file beside `tree`: its stop and what it printed.
fn run(
    tree: &Path,
    id: &str,
    iterations: u64,
    harness: &dyn Harness,
) -> (anyhow::Result<Stop>, String) {
    run_declaring(tree, id, iterations, harness, true)
}

/// [`run`] with the scan adapter `none` declared when `declared`, else with no scan declared.
fn run_declaring(
    tree: &Path,
    id: &str,
    iterations: u64,
    harness: &dyn Harness,
    declared: bool,
) -> (anyhow::Result<Stop>, String) {
    let validators = validators();
    let mut run = Run::open(tree, options(id, iterations), harness, &validators).expect("opens");
    if declared {
        run.scan_with(Scan::None {
            declared_in: tree.with_file_name("run.toml"),
        });
    }
    // story:a-run-keeps-every-decision-only-on-a-trial: the parity compares every decision the
    // case holds, so the run keeps every one, as a `--records trial` run does.
    run.keep_records(takt::case::Records::Trial);
    let mut out = Vec::new();
    run.continue_past_red_base();
    let stop = run.run(&mut out);
    (stop, String::from_utf8_lossy(&out).into_owned())
}

/// The case directory of `id` in `tree`.
fn case_dir(tree: &Path, id: &str) -> PathBuf {
    tree.join(".takt/cases").join(id)
}

/// Every decision the case recorded, as its claim values and obligation, action and outcome
/// statuses: what "the same decisions" compares, and nothing that names an instant, a commit or
/// the case.
fn decisions(tree: &Path, id: &str) -> Vec<serde_json::Value> {
    let dir = case_dir(tree, id).join("decisions");
    let mut files: Vec<PathBuf> = fs::read_dir(&dir)
        .expect("the decisions")
        .map(|entry| entry.expect("an entry").path())
        .collect();
    files.sort();
    files
        .iter()
        .map(|file| {
            let doc: serde_json::Value =
                serde_json::from_str(&fs::read_to_string(file).expect("a decision")).expect("JSON");
            // A map by name (claims, actions, outcomes) or a list of items naming their `id`
            // (obligations); either is read as name to value, and must be there.
            let statuses = |key: &str, field: &str| -> serde_json::Value {
                let named: Vec<(String, serde_json::Value)> = match &doc[key] {
                    serde_json::Value::Object(items) => items
                        .iter()
                        .map(|(name, item)| (name.clone(), item[field].clone()))
                        .collect(),
                    serde_json::Value::Array(items) => items
                        .iter()
                        .map(|item| {
                            let id = item["id"].as_str().expect("an id").to_owned();
                            (id, item[field].clone())
                        })
                        .collect(),
                    other => panic!("{key} in {}: {other}", file.display()),
                };
                assert!(!named.is_empty(), "{key} in {}", file.display());
                serde_json::Value::Object(named.into_iter().collect())
            };
            serde_json::json!({
                "claims": statuses("claims", "value"),
                "obligations": statuses("obligations", "status"),
                "actions": statuses("actions", "status"),
                "outcomes": statuses("outcomes", "status"),
            })
        })
        .collect()
}

/// The case's run record, one JSON object per line.
fn events(tree: &Path, id: &str) -> Vec<serde_json::Value> {
    fs::read_to_string(case_dir(tree, id).join("events.jsonl"))
        .expect("events.jsonl")
        .lines()
        .map(|line| serde_json::from_str(line).expect("a JSON line"))
        .collect()
}

/// The harness step lines of the run record.
fn harness_steps(tree: &Path, id: &str) -> Vec<serde_json::Value> {
    events(tree, id)
        .into_iter()
        .filter(|line| line["event"] == "step" && line["by"] == "harness")
        .collect()
}

/// Acceptance 1, without a model: under each kind the same scripted session's proposal is
/// applied by the edit effect and committed, the tests pass after it, and every decision the
/// case records has the same claim values and obligation, action and outcome statuses.
#[test]
fn every_kind_reaches_the_same_decisions() {
    let mut seen_decisions = Vec::new();
    for kind in KINDS {
        let scratch = repository(&format!("same-{}", kind.as_str()));
        let tree = scratch.tree();
        let id = "parity-same";
        let (harness, seen) = harness(kind, &Script::fixing());
        let (stop, text) = run(&tree, id, 3, &harness);
        let kind = kind.as_str();
        assert!(
            matches!(
                stop,
                Ok(Stop::Iterations(_)) | Ok(Stop::ApprovalRequired(_))
            ),
            "{kind}: {stop:?}\n{text}"
        );
        assert_eq!(seen.borrow().cwd.len(), 1, "{kind}: one session\n{text}");
        assert_eq!(
            fs::read_to_string(tree.join("src/lib.rs")).expect("the file"),
            FIXED,
            "{kind}: the edit effect applied the proposal\n{text}"
        );
        assert_eq!(
            git(&tree, &["show", "HEAD:src/lib.rs"]),
            FIXED.trim_end(),
            "{kind}: and the step committed it\n{text}"
        );
        let decided = decisions(&tree, id);
        assert!(decided.len() >= 3, "{kind}: {decided:?}\n{text}");
        assert_eq!(
            decided.last().expect("a decision")["claims"]["tests.pass"],
            "true",
            "{kind}\n{text}"
        );
        seen_decisions.push((kind, decided));
    }
    let (first, expected) = &seen_decisions[0];
    for (kind, decided) in &seen_decisions[1..] {
        assert_eq!(decided, expected, "{kind} against {first}");
    }
}

/// Each kind's session is the harness program it names, started in a scratch copy outside the
/// case tree, and the step line names the harness that took it.
#[test]
fn each_kind_runs_its_own_program_in_a_copy() {
    for (kind, program, name) in [
        (Kind::Claude, "claude", "claude-code"),
        (Kind::Codex, "codex", "codex"),
        (Kind::B10x, "b10x-harness", "b10x-harness"),
    ] {
        let scratch = repository(&format!("program-{}", kind.as_str()));
        let tree = scratch.tree();
        let id = "parity-program";
        let (harness, seen) = harness(kind, &Script::fixing());
        assert_eq!(harness.name(), name);
        let (_, text) = run(&tree, id, 2, &harness);
        let seen = seen.borrow();
        assert_eq!(seen.program.len(), 1, "{name}\n{text}");
        assert!(
            seen.program[0] == program || seen.program[0].ends_with(&format!("/{program}")),
            "{name}: {:?}",
            seen.program
        );
        let resolved = fs::canonicalize(&tree).expect("the tree");
        assert!(
            !seen.cwd[0].starts_with(&resolved),
            "{name}: {:?}",
            seen.cwd
        );
        assert!(!seen.cwd[0].exists(), "{name}: the copy is removed\n{text}");
        let steps = harness_steps(&tree, id);
        assert_eq!(steps.len(), 1, "{name}\n{text}");
        assert_eq!(steps[0]["name"], name, "{name}\n{text}");
    }
}

/// Acceptance 2: a scripted session of any kind that proposes an action it was not offered is
/// refused as a Claude session is: `StepRefused` (exit 3) before the edit effect, the case tree
/// unchanged and `HEAD` where it was.
#[test]
fn an_unoffered_action_is_refused_under_every_kind() {
    for kind in KINDS {
        let scratch = repository(&format!("unoffered-{}", kind.as_str()));
        let tree = scratch.tree();
        let before = work(&tree);
        let head = git(&tree, &["rev-parse", "HEAD"]);
        let script = Script {
            action: "repository.merge.now",
            ..Script::fixing()
        };
        let (harness, _) = harness(kind, &script);
        let (stop, text) = run(&tree, "parity-unoffered", 2, &harness);
        let kind = kind.as_str();
        let Ok(Stop::StepRefused(why)) = &stop else {
            panic!("{kind}: refused, not {stop:?}\n{text}");
        };
        assert!(
            text.contains("before the edit effect"),
            "{kind}: {why}\n{text}"
        );
        assert_eq!(work(&tree), before, "{kind}: the tree is unchanged\n{text}");
        assert_eq!(git(&tree, &["rev-parse", "HEAD"]), head, "{kind}\n{text}");
    }
}

/// The variables the run withholds from a harness, Codex's and the b10x loop's among them.
const WITHHELD: [(&str, &str); 10] = [
    ("ANTHROPIC_API_KEY", "sk-withheld-anthropic"),
    ("OPENAI_API_KEY", "sk-withheld-openai"),
    ("CODEX_API_KEY", "sk-withheld-codex"),
    ("CODEX_HOME", "/withheld/codex-home"),
    ("OPENAI_BASE_URL", "https://withheld.invalid/v1"),
    ("CLAUDECODE", "withheld-nested"),
    ("CLAUDE_CODE_ENTRYPOINT", "withheld-entrypoint"),
    ("GITHUB_TOKEN", "ghp-withheld-token"),
    ("SSH_AUTH_SOCK", "/withheld/agent.sock"),
    ("DEPLOY_SECRET", "withheld-deploy-secret"),
];

const CHILD: &str = "TAKT_HARNESS_PARITY_CHILD";

/// Acceptance 3: run in a child of this test binary with the withheld variables set, the
/// environment and the arguments (the prompt among them) each kind's executor hands metaharness's
/// child carry none of them by value, and none by name but the `CODEX_HOME` metaharness itself
/// points at its scratch home.
#[test]
fn no_kind_is_handed_a_withheld_variable() {
    if std::env::var_os(CHILD).is_some() {
        for kind in KINDS {
            let scratch = repository(&format!("withheld-{}", kind.as_str()));
            let tree = scratch.tree();
            let (harness, seen) = harness(kind, &Script::fixing());
            let (_, text) = run(&tree, "parity-withheld", 2, &harness);
            let seen = seen.borrow();
            let kind = kind.as_str();
            assert_eq!(seen.env.len(), 1, "{kind}\n{text}");
            for (name, value) in WITHHELD {
                assert_eq!(std::env::var(name).as_deref(), Ok(value), "set for the run");
                if name != "CODEX_HOME" {
                    assert!(
                        !seen.env[0].contains_key(name),
                        "{kind}: {name} reached the session"
                    );
                }
                for (key, held) in &seen.env[0] {
                    assert!(!held.contains(value), "{kind}: {name}'s value is in {key}");
                    assert!(
                        !takt::forge::is_credential(key),
                        "{kind}: {key} is withheld from a harness and reached the session"
                    );
                }
                for arg in &seen.args[0] {
                    assert!(
                        !arg.contains(value),
                        "{kind}: {name}'s value is in the prompt or arguments"
                    );
                }
            }
        }
        return;
    }
    let mut child = Command::new(std::env::current_exe().expect("this test binary"));
    child
        .args([
            "--exact",
            "no_kind_is_handed_a_withheld_variable",
            "--nocapture",
        ])
        .env(CHILD, "1");
    for (name, value) in WITHHELD {
        child.env(name, value);
    }
    let output = child.output().expect("the child runs");
    let said = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.status.success(), "{said}");
    assert!(said.contains("1 passed"), "the child ran the case: {said}");
}

/// The secret the trial arm's endpoints name their key by, `<namespace>/<name>`.
const SECRET_NAME: &str = "llm/opencode-go-api-key";

const SECRET_CHILD: &str = "TAKT_HARNESS_PARITY_SECRET_CHILD";

/// Acceptance 4 and 5 for the secret kind, over a run: b10x-harness pointed at an endpoint whose
/// key is the secret `llm/opencode-go-api-key` is handed that name and nothing more. In a child
/// of this test binary with the withheld variables set and no session bus, so no secrets store
/// or keychain is reachable, the case opens and the step runs; the `opened` line records the
/// secret by its namespace and name; the arguments name it once, as `--api-key-secret`'s value,
/// beside no token file, pointer or key variable; and neither the environment nor the prompt
/// holds it, a withheld value, or a variable that carries a credential.
#[test]
fn the_secret_kind_hands_the_harness_its_name_alone() {
    if std::env::var_os(SECRET_CHILD).is_some() {
        use takt::harness::{EndpointConfig, EndpointFields};
        let scratch = repository("secret-name");
        let tree = scratch.tree();
        let id = "parity-secret";
        let flags = EndpointFields {
            base_url: Some("https://opencode.ai/inference/go/openai/v1".to_owned()),
            wire: Some("openai-responses".to_owned()),
            model: Some("gpt-6-luna".to_owned()),
            secret: Some(SECRET_NAME.to_owned()),
            ..EndpointFields::default()
        };
        let config = EndpointConfig::of(&flags, None, None).expect("no HOME needed");
        config
            .check_token()
            .expect("a secret's name is checked by no read");
        let (mut harness, seen) =
            harness_saying(Kind::B10x, &Script::fixing(), Some("api-key:secrets"));
        harness.point_at(&config);
        let (stop, text) = run(&tree, id, 2, &harness);
        assert!(
            matches!(
                stop,
                Ok(Stop::Iterations(_)) | Ok(Stop::ApprovalRequired(_))
            ),
            "{stop:?}\n{text}"
        );
        let opened: Vec<serde_json::Value> = events(&tree, id)
            .into_iter()
            .filter(|line| line["event"] == "opened")
            .collect();
        assert_eq!(opened.len(), 1, "{opened:?}");
        assert_eq!(
            opened[0]["endpoint"],
            serde_json::json!({
                "base_url": "https://opencode.ai/inference/go/openai/v1",
                "wire": "openai-responses",
                "model": "gpt-6-luna",
                "credential": {
                    "kind": "secret",
                    "namespace": "llm",
                    "name": "opencode-go-api-key",
                },
            })
        );
        let seen = seen.borrow();
        assert_eq!(seen.args.len(), 1, "one session\n{text}");
        let argv = &seen.args[0];
        let at = argv
            .iter()
            .position(|word| word == "--api-key-secret")
            .unwrap_or_else(|| panic!("no secret named: {argv:?}"));
        assert_eq!(argv[at + 1], SECRET_NAME, "{argv:?}");
        assert_eq!(
            argv.iter()
                .filter(|word| word.contains("opencode-go-api-key"))
                .count(),
            1,
            "the name once: {argv:?}"
        );
        for word in argv {
            for flag in ["--oauth-token", "--api-key-env", "--api-key-file"] {
                assert!(
                    !word.starts_with(flag),
                    "{word} beside the secret: {argv:?}"
                );
            }
        }
        let prompt =
            String::from_utf8(harness.last_transcript().expect("a transcript")).expect("utf-8");
        assert!(
            !prompt.contains("opencode-go-api-key"),
            "the prompt names it"
        );
        for (key, value) in &seen.env[0] {
            assert!(
                !value.contains("opencode-go-api-key"),
                "{key} holds the name"
            );
            assert!(
                !takt::forge::is_credential(key),
                "{key} carries a credential to the session"
            );
        }
        for (name, value) in WITHHELD {
            assert!(
                !seen.env[0].contains_key(name),
                "{name} reached the session"
            );
            for held in seen.env[0].values().chain(argv.iter()) {
                assert!(!held.contains(value), "{name}'s value reached the session");
            }
        }
        for bus in ["DBUS_SESSION_BUS_ADDRESS", "XDG_RUNTIME_DIR"] {
            assert!(!seen.env[0].contains_key(bus), "{bus}: no store reachable");
        }
        return;
    }
    let mut child = Command::new(std::env::current_exe().expect("this test binary"));
    child
        .args([
            "--exact",
            "the_secret_kind_hands_the_harness_its_name_alone",
            "--nocapture",
        ])
        .env(SECRET_CHILD, "1")
        .env_remove("DBUS_SESSION_BUS_ADDRESS")
        .env_remove("XDG_RUNTIME_DIR");
    for (name, value) in WITHHELD {
        child.env(name, value);
    }
    let output = child.output().expect("the child runs");
    let said = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.status.success(), "{said}");
    assert!(said.contains("1 passed"), "the child ran the case: {said}");
}

/// Acceptance 4: a Codex step whose session reports no tokens charges none: its step line
/// carries no tokens and no usage, which the report prints as not reported, and the run's count
/// stays 0. One that reports them is charged its tokens once: Codex's input figure holds its
/// cached tokens, which are not counted twice.
#[test]
fn a_codex_step_without_a_token_report_charges_none() {
    for (tokens, charged) in [(false, None), (true, Some(120))] {
        let scratch = repository(&format!("tokens-{tokens}"));
        let tree = scratch.tree();
        let id = "parity-tokens";
        let script = Script {
            tokens,
            ..Script::fixing()
        };
        let (harness, _) = harness(Kind::Codex, &script);
        let (stop, text) = run(&tree, id, 2, &harness);
        assert!(stop.is_ok(), "{stop:?}\n{text}");
        let steps = harness_steps(&tree, id);
        assert_eq!(steps.len(), 1, "{text}");
        assert_eq!(
            steps[0]["tokens"].as_u64(),
            charged,
            "{:?}\n{text}",
            steps[0]
        );
        if charged.is_none() {
            assert!(steps[0].get("tokens").is_none(), "{:?}", steps[0]);
            assert!(steps[0].get("usage").is_none(), "{:?}", steps[0]);
        } else {
            assert_eq!(steps[0]["usage"]["cache_read"], 40, "{:?}", steps[0]);
            assert_eq!(steps[0]["usage"]["input"], 60, "{:?}", steps[0]);
        }
        let stopped = events(&tree, id)
            .into_iter()
            .find(|line| line["event"] == "stop")
            .expect("a stop line");
        assert_eq!(
            stopped["tokens"].as_u64(),
            Some(charged.unwrap_or(0)),
            "{text}"
        );
    }
}

/// H9 is advisory for Codex (story:codex-off-pin-proposes): a session whose codex is not the
/// version metaharness's adapter is pinned to proposes its change like a pinned one, and its
/// step's summary records H9 as a `gap`; on the pinned version the summary records the hermetic
/// verdict, every row with its state.
#[test]
fn a_codex_session_on_another_version_proposes_its_change() {
    let scratch = repository("version");
    let tree = scratch.tree();
    let script = Script {
        version: "0.143.0",
        ..Script::fixing()
    };
    let (other, _) = harness(Kind::Codex, &script);
    let (stop, text) = run(&tree, "parity-version", 2, &other);
    assert!(
        stop.is_ok(),
        "story:codex-off-pin-proposes: {stop:?}\n{text}"
    );
    let steps = harness_steps(&tree, "parity-version");
    let summary = steps[0]["summary"].as_str().expect("a summary");
    assert!(
        summary.contains("H9 gap"),
        "story:codex-off-pin-proposes: H9 gap in the summary: {summary}"
    );
    assert_eq!(steps[0]["version"], "0.143.0", "{:?}", steps[0]);

    let scratch = repository("verdict");
    let tree = scratch.tree();
    let (harness, _) = harness(Kind::Codex, &Script::fixing());
    let (stop, text) = run(&tree, "parity-verdict", 2, &harness);
    assert!(stop.is_ok(), "{stop:?}\n{text}");
    let steps = harness_steps(&tree, "parity-verdict");
    let summary = steps[0]["summary"].as_str().expect("a summary");
    for row in ["H1a", "H2", "H3", "H7", "H8", "H9 ok", "H11"] {
        assert!(summary.contains(row), "{row} in the summary: {summary}");
    }
}

/// `n` characters of a fixed pseudo-random sequence over `alphabet`: the body of a fake
/// credential, built at run time so that no credential-shaped literal stands in this file.
fn filler(seed: u64, n: usize, alphabet: &[u8]) -> String {
    let mut state = seed;
    (0..n)
        .map(|_| {
            state = state
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            let at = usize::try_from(state >> 33).expect("fits") % alphabet.len();
            char::from(alphabet[at])
        })
        .collect()
}

const ALNUM: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";
const URLSAFE: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";

/// The three logins a session could read in the user's home, each fake and shaped as the file
/// holds it: its name, the file's text, and the credential in it that must never be printed.
/// Codex's `auth.json` (its tokens, the access token a JWT), Claude Code's `.credentials.json`
/// (`claudeAiOauth` with its access and refresh tokens) and gh's `hosts.yml` (`oauth_token`).
fn planted() -> Vec<(&'static str, String, String)> {
    let jwt = |seed| {
        format!(
            "eyJ{}.eyJ{}.{}",
            filler(seed, 30, ALNUM),
            filler(seed + 1, 64, ALNUM),
            filler(seed + 2, 43, URLSAFE)
        )
    };
    let access = jwt(10);
    let codex = serde_json::json!({
        "OPENAI_API_KEY": null,
        "tokens": {
            "id_token": jwt(20),
            "access_token": access,
            "refresh_token": filler(30, 64, ALNUM),
            "account_id": "00000000-0000-0000-0000-000000000000",
        },
        "last_refresh": "2026-10-20T00:00:00Z",
    });
    let oauth = format!("sk-ant-oat01-{}AA", filler(40, 93, URLSAFE));
    let claude = serde_json::json!({
        "claudeAiOauth": {
            "accessToken": oauth,
            "refreshToken": format!("sk-ant-ort01-{}AA", filler(50, 93, URLSAFE)),
            "expiresAt": 1_790_000_000_000_u64,
            "scopes": ["user:inference", "user:profile"],
            "subscriptionType": "max",
        },
    });
    let gh = format!("gho_{}", filler(60, 36, ALNUM));
    let hosts = format!(
        "github.com:\n    users:\n        someone:\n            oauth_token: {gh}\n    git_protocol: https\n    oauth_token: {gh}\n    user: someone\n"
    );
    vec![
        ("codex-auth", format!("{codex:#}\n"), access),
        ("claude-credentials", format!("{claude:#}\n"), oauth),
        ("gh-hosts", hosts, gh),
    ]
}

/// Every shape takt checks a proposal for ([`CREDENTIAL_SHAPES`]) is matched by a fake of it,
/// each the shape it is named for, and ordinary code that names tokens matches none: a shape
/// added to the list without a sample here, or one that no longer matches its sample, fails.
#[test]
fn every_credential_shape_takt_lists_matches_its_fake() {
    let samples: Vec<(&str, String)> = vec![
        (
            "private-key",
            format!(
                "{dashes}BEGIN OPENSSH PRIVATE KEY{dashes}\n{}\n{dashes}END OPENSSH PRIVATE KEY{dashes}\n",
                filler(70, 64, ALNUM),
                dashes = "-".repeat(5)
            ),
        ),
        (
            "jwt",
            format!(
                "token = \"eyJ{}.eyJ{}.{}\"",
                filler(71, 20, ALNUM),
                filler(72, 30, ALNUM),
                filler(73, 30, URLSAFE)
            ),
        ),
        (
            "anthropic-token",
            format!("key: sk-ant-api03-{}", filler(74, 60, URLSAFE)),
        ),
        ("github-token", format!("ghp_{}", filler(75, 36, ALNUM))),
        (
            "github-token",
            format!("github_pat_{}", filler(76, 40, ALNUM)),
        ),
        ("gitlab-token", format!("glpat-{}", filler(79, 20, URLSAFE))),
        ("openai-key", format!("sk-proj-{}", filler(77, 40, ALNUM))),
        (
            "token-field",
            format!("\"refresh_token\": \"{}9\"", filler(78, 40, ALNUM)),
        ),
        (
            "aws-access-key-id",
            ["AWS_ACCESS_KEY_ID=", "AKIA", "Z7Q3EGXN2MBV4KPL"].concat(),
        ),
        (
            "aws-secret-access-key",
            format!("aws_secret_access_key = {}9", filler(81, 39, ALNUM)),
        ),
        (
            "authorization-header",
            format!("> Authorization: Bearer {}9", filler(82, 39, ALNUM)),
        ),
        (
            "url-password",
            format!(
                "https://user:{}9@example.com/o/r.git",
                filler(83, 15, ALNUM)
            ),
        ),
    ];
    for shape in CREDENTIAL_SHAPES {
        assert!(
            samples.iter().any(|(named, _)| *named == shape),
            "{shape} has no sample here"
        );
    }
    for (shape, text) in &samples {
        assert_eq!(
            credential_shape(text),
            Some(*shape),
            "the sample of {shape}"
        );
    }
    for clean in [
        FIXED,
        "let refresh_token = read_token_from_disk();\npub access_token: String,\n",
        "# The OAuth access_token and refresh_token are read from the login file.\n",
        "-----BEGIN PUBLIC KEY-----\nAAAA\n-----END PUBLIC KEY-----\n",
        "risk-assessment-of-the-whole-repository-in-bytes-per-file",
        "request.header(\"Authorization\", format!(\"Bearer {token}\"));\n",
        "curl -H \"Authorization: Bearer $TOKEN\" https://example.com/\n",
        "let url = format!(\"https://{user}:{password}@{host}/\");\n",
        "DATABASE_URL=postgres://postgres:postgres@localhost/app\n",
        "const REGION: &str = \"ASIA_PACIFIC\";\n",
    ] {
        assert_eq!(credential_shape(clean), None, "{clean}");
    }
}

/// F-A1, layer 1: a session of any kind that read a login of the user's (Codex's, Claude
/// Code's, gh's, each fake) and proposes it, in a file or in its summary, is refused before the
/// edit effect under a declared `scan: none` (takt's own list of credential shapes refuses
/// them): the tree and
/// `HEAD` as they were, the file named, the step's summary in the record the fixed line, and the
/// credential nowhere in what the run printed, its refusal or its record.
#[test]
fn a_planted_credential_is_refused_under_every_kind() {
    for kind in KINDS {
        for (login, text, secret) in planted() {
            for place in ["file", "summary"] {
                let name = format!("planted-{}-{login}-{place}", kind.as_str());
                let scratch = repository(&name);
                let tree = scratch.tree();
                let before = work(&tree);
                let head = git(&tree, &["rev-parse", "HEAD"]);
                let summary = format!("copied the login: {}", text.replace('\n', " "));
                let in_file = [("src/lib.rs", FIXED), ("config/login.txt", text.as_str())];
                let script = match place {
                    "file" => Script {
                        writes: &in_file,
                        ..Script::fixing()
                    },
                    _ => Script {
                        summary: &summary,
                        ..Script::fixing()
                    },
                };
                let (harness, _) = harness(kind, &script);
                let id = "parity-planted";
                let (stop, said) = run(&tree, id, 2, &harness);
                let Ok(Stop::StepRefused(why)) = &stop else {
                    panic!("{name}: refused, not {stop:?}");
                };
                assert!(said.contains("before the edit effect"), "{name}: {why}");
                let named = if place == "file" {
                    "config/login.txt"
                } else {
                    "summary"
                };
                assert!(
                    why.contains(named),
                    "{name}: the refusal names {named}: {why}"
                );
                assert_eq!(work(&tree), before, "{name}: the tree is unchanged");
                assert_eq!(git(&tree, &["rev-parse", "HEAD"]), head, "{name}: HEAD");
                let record = fs::read_to_string(case_dir(&tree, id).join("events.jsonl"))
                    .expect("events.jsonl");
                for (what, text) in [("output", &said), ("refusal", why), ("record", &record)] {
                    assert!(
                        !text.contains(&secret),
                        "{name}: the credential is in the run's {what}"
                    );
                }
                let steps = harness_steps(&tree, id);
                assert_eq!(steps.len(), 1, "{name}");
                assert_eq!(steps[0]["summary"], PROPOSAL_NOT_RECORDED, "{name}");
            }
        }
    }
}

/// With no scan declared, a Codex or b10x-harness proposal is refused before the edit effect
/// (not scanned is no pass), the tree unchanged and the summary not recorded; a Claude Code one
/// is applied as before.
#[test]
fn with_no_scan_declared_a_proposal_is_refused_as_not_scanned() {
    for kind in KINDS {
        let name = format!("unset-{}", kind.as_str());
        let scratch = repository(&name);
        let tree = scratch.tree();
        let before = work(&tree);
        let (harness, _) = harness(kind, &Script::fixing());
        let id = "parity-unset";
        let (stop, said) = run_declaring(&tree, id, 2, &harness, false);
        if kind == Kind::Claude {
            assert!(stop.is_ok(), "{name}: {stop:?}\n{said}");
            assert_eq!(
                fs::read_to_string(tree.join("src/lib.rs")).expect("the file"),
                FIXED,
                "{name}: applied as before\n{said}"
            );
            continue;
        }
        let Ok(Stop::StepRefused(why)) = &stop else {
            panic!("{name}: refused, not {stop:?}\n{said}");
        };
        assert!(said.contains("before the edit effect"), "{name}: {why}");
        assert!(why.contains("not scanned"), "{name}: {why}");
        assert_eq!(work(&tree), before, "{name}: the tree is unchanged\n{said}");
        let steps = harness_steps(&tree, id);
        assert_eq!(steps.len(), 1, "{name}\n{said}");
        assert_eq!(steps[0]["summary"], PROPOSAL_NOT_RECORDED, "{name}");
    }
}

/// story:scan-and-forge-are-adapters, Acceptance 2: under a declared `scan: none`
/// ([`Scan::None`]) a Codex or b10x-harness proposal is not refused as unscanned: a clean one is
/// applied and the `opened` line records `"scan":"none"`. One holding the shape of a
/// credential, in a file or in the summary, is still refused before the edit effect, the tree
/// and `HEAD` as they were, the summary not recorded and the credential nowhere in what the run
/// printed, its refusal or its record.
#[test]
fn under_a_declared_scan_none_a_credential_is_still_refused() {
    for kind in [Kind::Codex, Kind::B10x] {
        let name = format!("none-clean-{}", kind.as_str());
        let scratch = repository(&name);
        let tree = scratch.tree();
        let script = Script {
            summary: "made add add",
            ..Script::fixing()
        };
        let (clean, _) = harness(kind, &script);
        let id = "parity-none-clean";
        let (stop, said) = run(&tree, id, 2, &clean);
        assert!(stop.is_ok(), "{name}: {stop:?}\n{said}");
        assert_eq!(
            fs::read_to_string(tree.join("src/lib.rs")).expect("the file"),
            FIXED,
            "{name}: applied unscanned\n{said}"
        );
        let opened: Vec<_> = events(&tree, id)
            .into_iter()
            .filter(|line| line["event"] == "opened")
            .collect();
        assert_eq!(opened[0]["scan"], "none", "{name}: {}", opened[0]);

        for (login, text, secret) in planted() {
            for place in ["file", "summary"] {
                let name = format!("none-planted-{}-{login}-{place}", kind.as_str());
                let scratch = repository(&name);
                let tree = scratch.tree();
                let before = work(&tree);
                let head = git(&tree, &["rev-parse", "HEAD"]);
                let summary = format!("copied the login: {}", text.replace('\n', " "));
                let in_file = [("src/lib.rs", FIXED), ("config/login.txt", text.as_str())];
                let script = match place {
                    "file" => Script {
                        writes: &in_file,
                        ..Script::fixing()
                    },
                    _ => Script {
                        summary: &summary,
                        ..Script::fixing()
                    },
                };
                let (harness, _) = harness(kind, &script);
                let id = "parity-none-planted";
                let (stop, said) = run(&tree, id, 2, &harness);
                let Ok(Stop::StepRefused(why)) = &stop else {
                    panic!("{name}: refused, not {stop:?}\n{said}");
                };
                assert!(said.contains("before the edit effect"), "{name}: {why}");
                assert!(why.contains("shape of a credential"), "{name}: {why}");
                assert_eq!(work(&tree), before, "{name}: the tree is unchanged");
                assert_eq!(git(&tree, &["rev-parse", "HEAD"]), head, "{name}: HEAD");
                let record = fs::read_to_string(case_dir(&tree, id).join("events.jsonl"))
                    .expect("events.jsonl");
                for (what, text) in [("output", &said), ("refusal", why), ("record", &record)] {
                    assert!(
                        !text.contains(&secret),
                        "{name}: the credential is in the run's {what}"
                    );
                }
                let steps = harness_steps(&tree, id);
                assert_eq!(steps.len(), 1, "{name}");
                assert_eq!(steps[0]["summary"], PROPOSAL_NOT_RECORDED, "{name}");
            }
        }
    }
}

/// F-A1, layer 2: the Codex session's home is not the user's. Its `HOME` and the four XDG base
/// directories lie in the session's scratch root beside its copy, so `~` reaches nothing of the
/// user's; `CODEX_HOME` is metaharness's, and `PATH` is metaharness's, built from the user's
/// home, so the codex installed there is still the one found.
#[test]
fn the_codex_session_has_a_home_of_its_own() {
    let scratch = repository("home");
    let tree = scratch.tree();
    let (harness, seen) = harness(Kind::Codex, &Script::fixing());
    let (stop, said) = run(&tree, "parity-home", 2, &harness);
    assert!(stop.is_ok(), "{stop:?}\n{said}");
    let seen = seen.borrow();
    let env = &seen.env[0];
    let copy = &seen.cwd[0];
    let root = copy.parent().expect("the scratch root");
    let home = PathBuf::from(env.get("HOME").expect("a HOME"));
    assert!(home.starts_with(root), "{home:?} in {root:?}");
    assert!(!home.starts_with(copy), "{home:?} is outside the copy");
    if let Some(user) = std::env::var_os("HOME") {
        assert_ne!(home, PathBuf::from(&user), "the user's own home");
        let bin = format!("{}/.local/bin:", user.to_string_lossy());
        assert!(env["PATH"].starts_with(&bin), "{}", env["PATH"]);
    }
    for xdg in [
        "XDG_CONFIG_HOME",
        "XDG_DATA_HOME",
        "XDG_STATE_HOME",
        "XDG_CACHE_HOME",
    ] {
        let at = PathBuf::from(env.get(xdg).unwrap_or_else(|| panic!("{xdg} is set")));
        assert!(at.starts_with(&home), "{xdg}: {at:?}");
    }
    assert!(env.contains_key("CODEX_HOME"), "{env:?}");
    assert!(!home.exists(), "removed with the scratch copy");
}

/// F-A2: a Codex session on a codex other than the pinned one is not held back by H9
/// (story:codex-off-pin-proposes), and the tokens it reported are charged: its step line
/// carries them, their components and the version that ran.
#[test]
fn a_codex_session_off_the_pin_charges_its_tokens() {
    let scratch = repository("held");
    let tree = scratch.tree();
    let script = Script {
        version: "0.143.0",
        ..Script::fixing()
    };
    let (harness, _) = harness(Kind::Codex, &script);
    let id = "parity-held";
    let (stop, said) = run(&tree, id, 2, &harness);
    assert!(
        stop.is_ok(),
        "story:codex-off-pin-proposes: {stop:?}\n{said}"
    );
    let steps = harness_steps(&tree, id);
    assert!(!steps.is_empty(), "the step is recorded\n{said}");
    assert_eq!(steps[0]["tokens"].as_u64(), Some(120), "{:?}", steps[0]);
    assert_eq!(steps[0]["usage"]["input"], 60, "{:?}", steps[0]);
    assert_eq!(steps[0]["usage"]["cache_read"], 40, "{:?}", steps[0]);
    assert_eq!(steps[0]["version"], "0.143.0", "{:?}", steps[0]);
}

const RESOLVE_CHILD: &str = "TAKT_HARNESS_PARITY_RESOLVE";

/// F-A2: `--harness codex` runs the codex on the `PATH` metaharness gives its child (the user's
/// `~/.local/bin` first), the file whose version H9 reads, never the first on takt's own `PATH`.
/// Run in a child of this test binary whose `HOME` holds one fake codex in `.local/bin` and
/// whose `PATH` puts another first: only the one in `HOME` runs.
#[test]
fn codex_is_the_one_on_the_childs_path() {
    if let Some(dir) = std::env::var_os(RESOLVE_CHILD) {
        let dir = PathBuf::from(dir);
        let scratch = repository("resolve-run");
        let tree = scratch.tree();
        let mut harness = SessionHarness::of(Kind::Codex);
        harness.credentials = CredentialSource::None;
        harness.timeout = std::time::Duration::from_secs(60);
        let (stop, said) = run(&tree, "parity-resolve", 2, &harness);
        // H9 no longer holds an unpinned codex back (story:codex-off-pin-proposes): the fake
        // names no action and the run stops at its iterations, or it fails.
        assert!(
            stop.is_err() || said.contains("the session named no action"),
            "story:codex-off-pin-proposes: the fake proposes nothing: {stop:?}\n{said}"
        );
        assert!(
            dir.join("ran-child-path").exists(),
            "the codex in HOME ran: {stop:?}\n{said}"
        );
        assert!(
            !dir.join("ran-takt-path").exists(),
            "the codex first on takt's PATH did not"
        );
        return;
    }
    let base = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("parity-{}-resolve", std::process::id()));
    let _ = fs::remove_dir_all(&base);
    let home = base.join("home");
    let wrong = base.join("wrong");
    for (dir, marker) in [
        (home.join(".local/bin"), "ran-child-path"),
        (wrong.clone(), "ran-takt-path"),
    ] {
        fs::create_dir_all(&dir).expect("a directory");
        let program = dir.join("codex");
        fs::write(
            &program,
            format!(
                "#!/bin/sh\necho \"$@\" > '{}'\nexit 1\n",
                base.join(marker).display()
            ),
        )
        .expect("a fake codex");
        fs::set_permissions(&program, fs::Permissions::from_mode(0o755)).expect("exec bit");
    }
    let path = format!(
        "{}:{}",
        wrong.display(),
        std::env::var("PATH").unwrap_or_default()
    );
    let output = Command::new(std::env::current_exe().expect("this test binary"))
        .args([
            "--exact",
            "codex_is_the_one_on_the_childs_path",
            "--nocapture",
        ])
        .env(RESOLVE_CHILD, &base)
        .env("HOME", &home)
        .env("PATH", path)
        .output()
        .expect("the child runs");
    let said = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&base);
    assert!(output.status.success(), "{said}");
    assert!(said.contains("1 passed"), "the child ran the case: {said}");
}

/// story:takt-uses-loom-session-executor, acceptance 1: takt keeps no session executor of its
/// own; the one its sessions run through is Loom's crate, at the tag takt pins loom-sdk at.
#[test]
fn the_session_executor_is_looms() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    assert!(
        !root.join("crates/session-executor").exists(),
        "takt keeps no executor of its own"
    );
    let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
    let output = Command::new(cargo)
        .args([
            "tree",
            "-p",
            "b10x-takt",
            "-i",
            "b10x-loom-session-executor",
            "--offline",
            "--locked",
            "--prefix",
            "none",
            "--format",
            "{p}",
        ])
        .current_dir(&root)
        .output()
        .expect("cargo tree runs");
    let listed = String::from_utf8_lossy(&output.stdout).into_owned();
    assert!(
        output.status.success(),
        "{listed}{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let manifest =
        fs::read_to_string(root.join("Cargo.toml")).expect("the workspace manifest reads");
    let tag = manifest
        .lines()
        .find(|line| line.starts_with("b10x-loom-sdk "))
        .and_then(|line| line.split("tag = \"").nth(1))
        .and_then(|rest| rest.split('"').next())
        .expect("loom-sdk is pinned by tag");
    let first = listed.lines().next().unwrap_or_default();
    assert!(
        first.starts_with("b10x-loom-session-executor ")
            && first.contains(&format!("github.com/beyond10x/loom?tag={tag}")),
        "{listed}"
    );
}
