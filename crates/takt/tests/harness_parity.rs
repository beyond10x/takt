//! story:second-harness-adapter: one session executor, the harness its kind (Claude Code, Codex,
//! b10x-harness), each given the same brief over the same scratch copy and held to the same
//! proposal, so what a case decides depends on the evidence and never on which harness took the
//! step. Each session here is metaharness's scripted one, the vendor's own record shape per kind:
//! no vendor process, no credential, no network.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::rc::Rc;

use session_executor::metaharness::{
    HarnessProcess, LaunchPlanView, ProcessRunner, ScriptedLog, ScriptedRunner,
};
use session_executor::{CredentialSource, Endpoint, Kind};
use takt::harness::{Harness, SessionHarness};
use takt::validator::{TestsRun, Validator};
use takt::{Run, RunOptions, Stop};

const PROTOCOL: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../protocols/story-delivery/1.yaml"
);

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
            if relative == Path::new(".git") || relative.starts_with(".engineering") {
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
/// (Codex's naming `version`), a reply ending in the JSON line naming `action`, the tokens when
/// `tokens` is set, and its end.
fn record(kind: Kind, cwd: &Path, action: &str, tokens: bool, version: &str) -> Vec<String> {
    let reply = format!("Done.\n{{\"action\": \"{action}\", \"summary\": \"scripted\"}}");
    let cwd = cwd.display().to_string();
    let lines = match kind {
        Kind::Claude => {
            let mut lines = vec![serde_json::json!({
                "type": "system", "subtype": "init", "claude_code_version": "2.1.259",
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
    tokens: bool,
    version: String,
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
        let lines = record(
            self.kind,
            plan.cwd,
            &self.action,
            self.tokens,
            &self.version,
        );
        ScriptedRunner::of_lines(lines, ScriptedLog::new()).start(plan)
    }
}

/// How a scripted session of one kind behaves.
struct Script<'a> {
    writes: &'a [(&'a str, &'a str)],
    action: &'a str,
    tokens: bool,
    version: &'a str,
}

impl Script<'_> {
    fn fixing() -> Script<'static> {
        Script {
            writes: &[("src/lib.rs", FIXED)],
            action: "implementation.edit",
            tokens: true,
            version: CODEX_PIN,
        }
    }
}

/// `--harness <kind>` over a scripted session.
fn harness(kind: Kind, script: &Script<'_>) -> (SessionHarness, Rc<RefCell<Seen>>) {
    let seen = Rc::new(RefCell::new(Seen::default()));
    let shared = Rc::clone(&seen);
    let writes: Vec<(String, String)> = script
        .writes
        .iter()
        .map(|(path, text)| ((*path).to_owned(), (*text).to_owned()))
        .collect();
    let (action, tokens, version) = (
        script.action.to_owned(),
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
            tokens,
            version: version.clone(),
            seen: Rc::clone(&shared),
        })
    });
    (harness, seen)
}

/// The run of `harness` on `tree` for `iterations` steps: its stop and what it printed.
fn run(
    tree: &Path,
    id: &str,
    iterations: u64,
    harness: &dyn Harness,
) -> (anyhow::Result<Stop>, String) {
    let validators = validators();
    let mut run = Run::open(tree, options(id, iterations), harness, &validators).expect("opens");
    let mut out = Vec::new();
    let stop = run.run(&mut out);
    (stop, String::from_utf8_lossy(&out).into_owned())
}

/// The case directory of `id` in `tree`.
fn case_dir(tree: &Path, id: &str) -> PathBuf {
    tree.join(".engineering/cases").join(id)
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

/// H9 gates for Codex: a session whose codex is not the version metaharness's adapter is pinned
/// to proposes nothing and says so, the tree as it was; on the pinned version the step's summary
/// records the hermetic verdict, every row with its state.
#[test]
fn a_codex_session_on_another_version_proposes_nothing() {
    let scratch = repository("version");
    let tree = scratch.tree();
    let before = work(&tree);
    let script = Script {
        version: "0.160.0",
        ..Script::fixing()
    };
    let (other, _) = harness(Kind::Codex, &script);
    let (stop, text) = run(&tree, "parity-version", 2, &other);
    let said = match &stop {
        Err(error) => format!("{error:#}"),
        Ok(stop) => format!("{stop:?}"),
    };
    assert!(said.contains("H9"), "{said}\n{text}");
    assert_eq!(work(&tree), before, "the tree is unchanged\n{text}");

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

/// Acceptance 5: the session executor's dependency tree holds no crate of this repository but
/// itself, so it can move into Loom unchanged.
#[test]
fn the_session_executor_depends_on_no_crate_of_this_repository() {
    let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let output = Command::new(cargo)
        .args([
            "tree",
            "-p",
            "b10x-session-executor",
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
    let crates = fs::canonicalize(root.join("crates")).expect("the crates directory");
    let ours: Vec<&str> = listed
        .lines()
        .filter(|line| line.contains(&format!("({}/", crates.display())))
        .collect();
    assert!(
        ours.iter()
            .all(|line| line.starts_with("b10x-session-executor ")),
        "{ours:?}"
    );
    assert!(!ours.is_empty(), "the executor itself is listed: {listed}");
}
