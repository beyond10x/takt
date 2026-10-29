//! Security review of story:scan-and-forge-are-adapters, invariant 2: under every scan adapter
//! the in-process check for the shape of a credential reads what a Codex or b10x-harness session
//! proposes before the edit effect, so a credential such a session read outside its copy never
//! reaches the tree, the case record or what the forge publishes. Under a declared `scan: none`
//! that check is the only one. A proposal carries its files' paths as well as their content;
//! these cases propose a file whose path holds a GitHub token's shape and check that it is
//! refused before the edit effect, as the same text in the file's content or the summary is
//! (`under_a_declared_scan_none_a_credential_is_still_refused` in `harness_parity.rs`).
//!
//! The helpers are copies of `harness_parity.rs`'s: metaharness's scripted session per kind, no
//! vendor process, no credential, no network. The token is fake.

#[path = "support/own_home.rs"]
mod own_home;

use std::fs;
use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};
use std::process::Command;

use loom_sdk::session::metaharness::{
    HarnessProcess, LaunchPlanView, ProcessRunner, ScriptedLog, ScriptedRunner,
};
use loom_sdk::session::{CredentialSource, Endpoint, Kind};
use takt::harness::SessionHarness;
use takt::run::credential_shape;
use takt::scan::Scan;
use takt::scan::gates::ChangeScan;
use takt::validator::{TestsRun, Validator};
use takt::{Run, RunOptions, Stop};

const BROKEN: &str = "pub fn add(a: i32, b: i32) -> i32 {\n    a - b\n}\n";
const FIXED: &str = "pub fn add(a: i32, b: i32) -> i32 {\n    a + b\n}\n";
const CODEX_PIN: &str = "0.153.4";
const ALNUM: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";

fn git(tree: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .args(["-c", "user.name=t", "-c", "user.email=t@localhost"])
        .args(["-c", "commit.gpgsign=false"])
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
        .join(format!("sec1-path-{}-{name}", std::process::id()));
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

fn options(id: &str) -> RunOptions {
    RunOptions {
        protocol: "story.delivery/1".to_owned(),
        intent: "make add add".to_owned(),
        inputs: Vec::new(),
        iterations: Some(2),
        hours: None,
        tokens: None,
        case_id: Some(id.to_owned()),
        gate: None,
    }
}

fn validators() -> Vec<Box<dyn Validator>> {
    vec![Box::new(
        TestsRun::new("git grep -qE a.[+].b$").expect("a command"),
    )]
}

fn filler(seed: u64, n: usize) -> String {
    let mut state = seed;
    (0..n)
        .map(|_| {
            state = state
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            let at = usize::try_from(state >> 33).expect("fits") % ALNUM.len();
            char::from(ALNUM[at])
        })
        .collect()
}

/// A fake GitHub token, as gh's `hosts.yml` holds one: `ghp_` and 36 letters and digits.
fn token() -> String {
    format!("ghp_{}", filler(77, 36))
}

/// The scripted session's record for `kind`, its reply naming `action` and `summary`.
fn record(kind: Kind, cwd: &Path, action: &str, summary: &str) -> Vec<String> {
    let reply = format!(
        "Done.\n{}",
        serde_json::json!({"action": action, "summary": summary})
    );
    let cwd = cwd.display().to_string();
    let lines = match kind {
        Kind::Codex => {
            let at = "2026-10-20T18:00:00.000Z";
            let turn = "01a15a19-5963-7d42-b749-21695314f7a4";
            vec![
                serde_json::json!({"timestamp": at, "type": "session_meta", "payload": {
                    "session_id": "01a15a19-57e0-7d83-97b1-da333ebb91e5",
                    "id": "01a15a19-57e0-7d83-97b1-da333ebb91e5", "timestamp": at,
                    "cwd": cwd, "originator": "codex_exec", "cli_version": CODEX_PIN,
                    "source": "exec", "model_provider": "openai"}}),
                serde_json::json!({"timestamp": at, "type": "event_msg", "payload": {
                    "type": "task_started", "turn_id": turn}}),
                serde_json::json!({"timestamp": at, "type": "event_msg", "payload": {
                    "type": "agent_message", "message": reply, "phase": "final_answer"}}),
                serde_json::json!({"timestamp": at, "type": "response_item", "payload": {
                    "type": "message", "role": "assistant", "phase": "final_answer",
                    "content": [{"type": "output_text", "text": reply}]}}),
                serde_json::json!({"timestamp": at, "type": "event_msg", "payload": {
                    "type": "token_count", "info": {"total_token_usage": {"input_tokens": 100,
                    "cached_input_tokens": 40, "output_tokens": 20, "reasoning_output_tokens": 0,
                    "total_tokens": 120}}}}),
                serde_json::json!({"timestamp": at, "type": "event_msg", "payload": {
                    "type": "task_complete", "turn_id": turn, "last_agent_message": reply,
                    "duration_ms": 5}}),
            ]
        }
        Kind::B10x => vec![
            serde_json::json!({"kind": "started", "model": "scripted",
                "published_tools": ["file_read", "file_write"],
                "operations": ["file.read", "file.write"], "withheld": [], "skills": [],
                "agents": [], "profiles": [], "context": [], "toolchains": [],
                "credential_source": "oauth-token:file"}),
            serde_json::json!({"kind": "turn-started", "turn": 1}),
            serde_json::json!({"kind": "text-delta", "text": reply}),
            serde_json::json!({"kind": "usage", "model": "scripted",
                "input_tokens": 100, "output_tokens": 20, "cached_input_tokens": 40}),
            serde_json::json!({"kind": "finished", "stop": {"kind": "completed"},
                "turns": 1}),
        ],
        other => panic!("no scripted record for {other:?} here"),
    };
    lines.iter().map(ToString::to_string).collect()
}

/// A scripted session: writes `writes` into its working directory as the model's tools would,
/// then prints [`record`].
struct Session {
    kind: Kind,
    writes: Vec<(String, String)>,
}

impl ProcessRunner for Session {
    fn requires_executable(&self) -> bool {
        false
    }

    fn start(&mut self, plan: &LaunchPlanView) -> std::io::Result<Box<dyn HarnessProcess>> {
        for (path, text) in &self.writes {
            let at = plan.cwd.join(path);
            fs::create_dir_all(at.parent().expect("a parent"))?;
            fs::write(at, text)?;
        }
        let lines = record(self.kind, plan.cwd, "implementation.edit", "made add add");
        ScriptedRunner::of_lines(lines, ScriptedLog::new()).start(plan)
    }
}

fn harness(kind: Kind, writes: Vec<(String, String)>) -> SessionHarness {
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
    harness.with_runner(move || {
        Box::new(Session {
            kind,
            writes: writes.clone(),
        })
    })
}

/// A stand-in for `b10x-gates` beside `tree` that passes whatever it is given.
fn passing_scan(tree: &Path) -> ChangeScan {
    let dir = tree.with_file_name("scanner-pass");
    fs::create_dir_all(&dir).expect("the scanner's directory");
    let policy = dir.join("policy.json");
    fs::write(&policy, "{}\n").expect("a policy");
    let program = dir.join("b10x-gates");
    fs::write(&program, "#!/bin/sh\nexit 0\n").expect("a scanner");
    fs::set_permissions(&program, fs::Permissions::from_mode(0o755)).expect("exec bit");
    ChangeScan { program, policy }
}

/// Every file of `HEAD`'s tree and the working tree whose path holds `needle`.
fn paths_holding(tree: &Path, needle: &str) -> Vec<String> {
    let mut found: Vec<String> = git(tree, &["ls-tree", "-r", "--name-only", "HEAD"])
        .lines()
        .filter(|path| path.contains(needle))
        .map(|path| format!("HEAD:{path}"))
        .collect();
    let notes = tree.join("notes");
    if let Ok(entries) = fs::read_dir(&notes) {
        for entry in entries.flatten() {
            if entry.file_name().to_string_lossy().contains(needle) {
                found.push(format!(
                    "tree:notes/{}",
                    entry.file_name().to_string_lossy()
                ));
            }
        }
    }
    found
}

/// The run of a `kind` session proposing the fix and `notes/<token>.md` under `configure`'s scan,
/// checked as the invariant states it: refused before the edit effect for the shape of a
/// credential, nothing of it in the tree, `HEAD` or the case record.
fn a_credential_in_a_proposed_path_is_refused(label: &str, configure: fn(&mut Run<'_>, &Path)) {
    let token = token();
    assert_eq!(
        credential_shape(&token),
        Some("github-token"),
        "the fixture holds the shape the in-process check knows"
    );
    for kind in [Kind::Codex, Kind::B10x] {
        let name = format!("{label}-{}", kind.as_str());
        let scratch = repository(&name);
        let tree = scratch.tree();
        let head = git(&tree, &["rev-parse", "HEAD"]);
        let writes = vec![
            ("src/lib.rs".to_owned(), FIXED.to_owned()),
            (format!("notes/{token}.md"), "a note\n".to_owned()),
        ];
        let session = harness(kind, writes);
        let validators = validators();
        let id = "sec1-path";
        let mut run = Run::open(&tree, options(id), &session, &validators).expect("opens");
        configure(&mut run, &tree);
        run.continue_past_red_base();
        let mut out = Vec::new();
        let stop = run.run(&mut out);
        let said = String::from_utf8_lossy(&out).into_owned();
        let record = fs::read_to_string(tree.join(".takt/cases").join(id).join("events.jsonl"))
            .unwrap_or_default();
        let in_record: Vec<String> = record
            .lines()
            .filter(|line| line.contains(&token))
            .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
            .map(|line| line["event"].as_str().unwrap_or("?").to_owned())
            .collect();
        let in_tree = paths_holding(&tree, &token);
        assert!(
            in_record.is_empty() && in_tree.is_empty() && !said.contains(&token),
            "{name}: a proposed path of the shape of a credential (github-token) passed the \
             in-process check and the edit effect: the case record's {in_record:?} lines, \
             {in_tree:?} and the run's output {} hold it; HEAD moved from {head}: {}; stop {stop:?}",
            if said.contains(&token) { "" } else { "do not" },
            git(&tree, &["rev-parse", "HEAD"])
        );
        let Ok(Stop::StepRefused(why)) = &stop else {
            panic!("{name}: refused before the edit effect, not {stop:?}\n{said}");
        };
        assert!(why.contains("shape of a credential"), "{name}: {why}");
        assert_eq!(git(&tree, &["rev-parse", "HEAD"]), head, "{name}: HEAD");
    }
}

/// Under a declared `scan: none`, the run's only check of what a Codex or b10x-harness session
/// proposes is the in-process one; a path is part of what it proposes.
#[test]
fn under_a_declared_scan_none_a_credential_in_a_proposed_path_is_refused() {
    a_credential_in_a_proposed_path_is_refused("none", |run, tree| {
        run.scan_with(Scan::None {
            declared_in: tree.with_file_name("run.toml"),
        });
    });
}

/// The same proposal under `gates` with a scan that passes what it is given, as a gates policy
/// with no rule for a GitHub token's shape does (`CREDENTIAL_SHAPES`: "the gates policy's scan,
/// which holds no rule for them"): the in-process check is again what must refuse it.
#[test]
fn under_gates_with_a_passing_scan_a_credential_in_a_proposed_path_is_refused() {
    a_credential_in_a_proposed_path_is_refused("gates", |run, tree| {
        run.scan_change_with(Some(passing_scan(tree)));
    });
}
