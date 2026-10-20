//! story:second-harness-adapter: one session executor, the harness its kind (Claude Code, Codex,
//! b10x-harness), each given the same brief over the same scratch copy and held to the same
//! proposal, so what a case decides depends on the evidence and never on which harness took the
//! step. Each session here is metaharness's scripted one, the vendor's own record shape per kind:
//! no vendor process, no credential, no network.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::fs;
use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::rc::Rc;

use session_executor::metaharness::{
    HarnessProcess, LaunchPlanView, ProcessRunner, ScriptedLog, ScriptedRunner,
};
use session_executor::{CredentialSource, Endpoint, Kind};
use takt::harness::{Harness, SessionHarness};
use takt::run::{CREDENTIAL_SHAPES, ChangeScan, PROPOSAL_NOT_RECORDED, credential_shape};
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
    summary: String,
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
            &self.summary,
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
            seen: Rc::clone(&shared),
        })
    });
    (harness, seen)
}

/// The run of `harness` on `tree` for `iterations` steps under a gates scan that passes what it
/// is given ([`scanner`] `pass`): its stop and what it printed.
fn run(
    tree: &Path,
    id: &str,
    iterations: u64,
    harness: &dyn Harness,
) -> (anyhow::Result<Stop>, String) {
    let scan = scanner(tree, "pass");
    run_scanned(tree, id, iterations, harness, Some(scan))
}

/// [`run`] under the gates scan `scan`, or none.
fn run_scanned(
    tree: &Path,
    id: &str,
    iterations: u64,
    harness: &dyn Harness,
    scan: Option<ChangeScan>,
) -> (anyhow::Result<Stop>, String) {
    let validators = validators();
    let mut run = Run::open(tree, options(id, iterations), harness, &validators).expect("opens");
    run.scan_change_with(scan);
    let mut out = Vec::new();
    let stop = run.run(&mut out);
    (stop, String::from_utf8_lossy(&out).into_owned())
}

/// The text a fake gates policy refuses: [`scanner`] `policy` exits 1 on a file holding it.
const POLICY_MARK: &str = "POLICY-RULE-MATCHED";

/// A stand-in for `b10x-gates` beside `tree` (outside it), run as `scan-text --repo <tree>
/// --policy <policy> <file>...`: `pass` exits 0; `policy` appends each scanned file to `log`
/// and exits 1 when one holds [`POLICY_MARK`], as a policy rule's match does; `missing` names a
/// program that is not there, a scan that cannot run.
fn scanner(tree: &Path, kind: &str) -> ChangeScan {
    let dir = tree.with_file_name(format!("scanner-{kind}"));
    fs::create_dir_all(&dir).expect("the scanner's directory");
    let policy = dir.join("policy.json");
    fs::write(&policy, "{}\n").expect("a policy");
    let body = match kind {
        "pass" => "exit 0".to_owned(),
        "policy" => format!(
            "for f; do case \"$f\" in /*) if [ -f \"$f\" ]; then cat \"$f\" >> '{log}'; if grep -q {POLICY_MARK} \"$f\"; then exit 1; fi; fi;; esac; done\nexit 0",
            log = dir.join("log").display()
        ),
        "missing" => {
            return ChangeScan {
                program: dir.join("not-there"),
                policy,
            };
        }
        other => panic!("no scanner {other}"),
    };
    let program = dir.join("b10x-gates");
    fs::write(&program, format!("#!/bin/sh\n{body}\n")).expect("a scanner");
    fs::set_permissions(&program, fs::Permissions::from_mode(0o755)).expect("exec bit");
    ChangeScan { program, policy }
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
        ("openai-key", format!("sk-proj-{}", filler(77, 40, ALNUM))),
        (
            "token-field",
            format!("\"refresh_token\": \"{}9\"", filler(78, 40, ALNUM)),
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
    ] {
        assert_eq!(credential_shape(clean), None, "{clean}");
    }
}

/// F-A1, layer 1: a session of any kind that read a login of the user's (Codex's, Claude
/// Code's, gh's, each fake) and proposes it, in a file or in its summary, is refused before the
/// edit effect under a gates policy that passes it, as the repository's own policy does (it holds
/// no rule for these shapes; takt's own list of credential shapes refuses them): the tree and
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

/// F-A1, layer 1: under every kind a proposal the gates policy refuses (a file, or the summary,
/// holding what a rule matches) and one whose scan cannot run are refused before the edit
/// effect, naming the file and never what matched, the tree unchanged and the summary not
/// recorded. With no policy set, a Codex or b10x-harness proposal is refused (not scanned is no
/// pass) and a Claude Code one is applied as before.
#[test]
fn a_policy_refusal_or_a_scan_that_cannot_run_refuses_the_proposal() {
    let marked = format!("{FIXED}// {POLICY_MARK}\n");
    let marked_summary = format!("made add add {POLICY_MARK}");
    for kind in KINDS {
        let unset = (kind != Kind::Claude).then_some("not scanned");
        for (case, scan, file, summary, refused) in [
            (
                "file",
                Some("policy"),
                marked.as_str(),
                "scripted",
                Some("src/lib.rs"),
            ),
            (
                "summary",
                Some("policy"),
                FIXED,
                marked_summary.as_str(),
                Some("summary"),
            ),
            (
                "missing",
                Some("missing"),
                FIXED,
                "scripted",
                Some("could not run"),
            ),
            ("unset", None, FIXED, "scripted", unset),
        ] {
            let name = format!("policy-{}-{case}", kind.as_str());
            let scratch = repository(&name);
            let tree = scratch.tree();
            let before = work(&tree);
            let writes = [("src/lib.rs", file)];
            let script = Script {
                writes: &writes,
                summary,
                ..Script::fixing()
            };
            let (harness, _) = harness(kind, &script);
            let id = "parity-policy";
            let scan = scan.map(|scan| scanner(&tree, scan));
            let (stop, said) = run_scanned(&tree, id, 2, &harness, scan);
            let Some(named) = refused else {
                assert!(stop.is_ok(), "{name}: {stop:?}\n{said}");
                assert_eq!(
                    fs::read_to_string(tree.join("src/lib.rs")).expect("the file"),
                    FIXED,
                    "{name}: applied as before\n{said}"
                );
                continue;
            };
            let Ok(Stop::StepRefused(why)) = &stop else {
                panic!("{name}: refused, not {stop:?}\n{said}");
            };
            assert!(said.contains("before the edit effect"), "{name}: {why}");
            assert!(
                why.contains(named),
                "{name}: the refusal names {named}: {why}"
            );
            let record =
                fs::read_to_string(case_dir(&tree, id).join("events.jsonl")).expect("events");
            for (what, text) in [("output", &said), ("refusal", why), ("record", &record)] {
                assert!(
                    !text.contains(POLICY_MARK),
                    "{name}: what the rule matched is in the run's {what}"
                );
            }
            assert_eq!(work(&tree), before, "{name}: the tree is unchanged\n{said}");
            let steps = harness_steps(&tree, id);
            assert_eq!(steps.len(), 1, "{name}\n{said}");
            assert_eq!(steps[0]["summary"], PROPOSAL_NOT_RECORDED, "{name}");
        }
    }
}

/// F-A1, layer 1: a clean proposal passes the gates scan under every kind and is applied; the
/// scan was given its file's content and the session's summary.
#[test]
fn a_clean_proposal_passes_the_scan_under_every_kind() {
    for kind in KINDS {
        let name = format!("clean-{}", kind.as_str());
        let scratch = repository(&name);
        let tree = scratch.tree();
        let scan = scanner(&tree, "policy");
        let log = scan.program.with_file_name("log");
        let script = Script {
            summary: "made add add",
            ..Script::fixing()
        };
        let (harness, _) = harness(kind, &script);
        let id = "parity-clean";
        let (stop, said) = run_scanned(&tree, id, 2, &harness, Some(scan));
        assert!(stop.is_ok(), "{name}: {stop:?}\n{said}");
        assert_eq!(
            fs::read_to_string(tree.join("src/lib.rs")).expect("the file"),
            FIXED,
            "{name}\n{said}"
        );
        let scanned = fs::read_to_string(&log).expect("the scan ran");
        assert!(scanned.contains(FIXED), "{name}: the file was scanned");
        assert!(
            scanned.contains("made add add"),
            "{name}: the summary was scanned"
        );
        let steps = harness_steps(&tree, id);
        let recorded = steps[0]["summary"].as_str().expect("a summary");
        assert!(recorded.starts_with("made add add"), "{name}: {recorded}");
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

/// F-A2: a Codex session that H9 holds back (a codex other than the pinned one) proposes
/// nothing, and the tokens it reported are still charged: its step line carries them, their
/// components and the version that ran; the tree is as it was.
#[test]
fn a_codex_session_held_back_by_h9_charges_its_tokens() {
    let scratch = repository("held");
    let tree = scratch.tree();
    let before = work(&tree);
    let script = Script {
        version: "0.160.0",
        ..Script::fixing()
    };
    let (harness, _) = harness(Kind::Codex, &script);
    let id = "parity-held";
    let (stop, said) = run(&tree, id, 2, &harness);
    let ended = match &stop {
        Err(error) => format!("{error:#}"),
        Ok(stop) => format!("{stop:?}"),
    };
    assert!(ended.contains("H9"), "{ended}\n{said}");
    assert_eq!(work(&tree), before, "the tree is unchanged\n{said}");
    let steps = harness_steps(&tree, id);
    assert_eq!(steps.len(), 1, "the held step is recorded\n{said}");
    assert_eq!(steps[0]["tokens"].as_u64(), Some(120), "{:?}", steps[0]);
    assert_eq!(steps[0]["usage"]["input"], 60, "{:?}", steps[0]);
    assert_eq!(steps[0]["usage"]["cache_read"], 40, "{:?}", steps[0]);
    assert_eq!(steps[0]["version"], "0.160.0", "{:?}", steps[0]);
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
        assert!(stop.is_err(), "the fake proposes nothing: {stop:?}\n{said}");
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
