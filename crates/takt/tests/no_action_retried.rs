//! story:a-harness-step-without-an-action-is-retried: a judgement step whose session ends without
//! the line naming its action is tried again within the run, 3 tries at most, each try recorded
//! with its tokens; the run ends on the error only after the third. A retry starts from the state
//! the failed try was discarded to. A refused step, and a harness that failed some other way (a
//! program that could not start, a JSON line naming no action), are not retried. Each session
//! here is metaharness's scripted b10x-harness one: no vendor process, no credential, no network.

use std::cell::RefCell;
use std::fs;
use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::rc::Rc;

use loom_sdk::session::metaharness::{
    HarnessProcess, LaunchPlanView, ProcessRunner, ScriptedLog, ScriptedRunner,
};
use loom_sdk::session::{CredentialSource, Endpoint, Kind};
use takt::harness::SessionHarness;
use takt::run::ChangeScan;
use takt::validator::{TestsRun, Validator};
use takt::{Run, RunOptions, Stop};

const BROKEN: &str = "pub fn add(a: i32, b: i32) -> i32 {\n    a - b\n}\n";
const FIXED: &str = "pub fn add(a: i32, b: i32) -> i32 {\n    a + b\n}\n";

/// The loop's text when a session's reply holds no JSON line naming an action.
const NO_ACTION_LINE: &str = "without the JSON line naming its action";

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
        .join(format!("no-action-{}-{name}", std::process::id()));
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

fn options(id: &str, iterations: u64, tokens: Option<u64>) -> RunOptions {
    RunOptions {
        protocol: "story.delivery/1".to_owned(),
        intent: "make add add".to_owned(),
        inputs: Vec::new(),
        iterations: Some(iterations),
        hours: None,
        tokens,
        case_id: Some(id.to_owned()),
        gate: None,
    }
}

/// How the scripted session of one call ends.
#[derive(Clone, Copy)]
enum Reply {
    /// Writes `stray.txt` and a broken `src/lib.rs` into its copy, then ends without the JSON
    /// line naming its action, as a session that stopped after compaction does.
    NoLine,
    /// Fixes `src/lib.rs` and names `implementation.edit`.
    Fixes,
    /// Names an action it was not offered.
    Unoffered,
    /// Ends on a JSON line whose `action` is no string.
    NoStringAction,
    /// Its program does not start.
    NotStarted,
}

/// What each session found in its copy when it started: `src/lib.rs` and whether `stray.txt`
/// was there.
#[derive(Default)]
struct Seen {
    found: Vec<(String, bool)>,
}

/// A scripted b10x-harness session: call `n` (from 1) ends as `replies[n - 1]`, the last reply
/// repeating.
struct Session {
    replies: Vec<Reply>,
    seen: Rc<RefCell<Seen>>,
}

impl ProcessRunner for Session {
    fn requires_executable(&self) -> bool {
        false
    }

    fn start(&mut self, plan: &LaunchPlanView) -> std::io::Result<Box<dyn HarnessProcess>> {
        let n = {
            let mut seen = self.seen.borrow_mut();
            let lib = fs::read_to_string(plan.cwd.join("src/lib.rs")).unwrap_or_default();
            seen.found.push((lib, plan.cwd.join("stray.txt").exists()));
            seen.found.len()
        };
        let reply = self.replies[(n - 1).min(self.replies.len() - 1)];
        let text = match reply {
            Reply::NotStarted => {
                return Err(std::io::Error::other("the program could not be started"));
            }
            Reply::NoLine => {
                fs::write(plan.cwd.join("stray.txt"), "left by a failed try\n")?;
                fs::write(plan.cwd.join("src/lib.rs"), "broken by a failed try\n")?;
                "I was summarizing the conversation and".to_owned()
            }
            Reply::Fixes => {
                fs::write(plan.cwd.join("src/lib.rs"), FIXED)?;
                format!(
                    "Done.\n{}",
                    serde_json::json!({"action": "implementation.edit", "summary": "fixed"})
                )
            }
            Reply::Unoffered => format!(
                "Done.\n{}",
                serde_json::json!({"action": "repository.merge.now", "summary": "merged"})
            ),
            Reply::NoStringAction => format!(
                "Done.\n{}",
                serde_json::json!({"action": 5, "summary": "a number"})
            ),
        };
        let lines = [
            serde_json::json!({"kind": "started", "model": "scripted",
                "published_tools": ["file_read", "file_write"],
                "operations": ["file.read", "file.write"], "withheld": [], "skills": [],
                "agents": [], "profiles": [], "context": [], "toolchains": [],
                "credential_source": "oauth-token:file"}),
            serde_json::json!({"kind": "turn-started", "turn": 1}),
            serde_json::json!({"kind": "text-delta", "text": text}),
            serde_json::json!({"kind": "usage", "model": "scripted",
                "input_tokens": 100, "output_tokens": 20, "cached_input_tokens": 40}),
            serde_json::json!({"kind": "finished", "stop": {"kind": "completed"}, "turns": 1}),
        ];
        ScriptedRunner::of_lines(lines.iter().map(ToString::to_string), ScriptedLog::new())
            .start(plan)
    }
}

/// `--harness harness` over scripted sessions ending as `replies`.
fn harness(replies: &[Reply]) -> (SessionHarness, Rc<RefCell<Seen>>) {
    let seen = Rc::new(RefCell::new(Seen::default()));
    let shared = Rc::clone(&seen);
    let replies = replies.to_vec();
    let mut harness = SessionHarness::of(Kind::B10x);
    harness.credentials = CredentialSource::None;
    harness.endpoint = Some(Endpoint {
        base_url: "https://endpoint.invalid/v1".to_owned(),
        wire: "anthropic-messages".to_owned(),
        model: "scripted".to_owned(),
        token_file: PathBuf::from("/nonexistent/token.json"),
        token_pointer: Some("/token".to_owned()),
    });
    let harness = harness.with_runner(move || {
        Box::new(Session {
            replies: replies.clone(),
            seen: Rc::clone(&shared),
        })
    });
    (harness, seen)
}

/// A stand-in for `b10x-gates` beside `tree` that passes what it is given.
fn scanner(tree: &Path) -> ChangeScan {
    let dir = tree.with_file_name("scanner-pass");
    fs::create_dir_all(&dir).expect("the scanner's directory");
    let policy = dir.join("policy.json");
    fs::write(&policy, "{}\n").expect("a policy");
    let program = dir.join("b10x-gates");
    fs::write(&program, "#!/bin/sh\nexit 0\n").expect("a scanner");
    fs::set_permissions(&program, fs::Permissions::from_mode(0o755)).expect("exec bit");
    ChangeScan { program, policy }
}

/// The run of sessions ending as `replies` on `tree`: its stop, what it printed, and what each
/// session found when it started.
fn run(
    tree: &Path,
    id: &str,
    replies: &[Reply],
    tokens: Option<u64>,
) -> (anyhow::Result<Stop>, String, Vec<(String, bool)>) {
    let (harness, seen) = harness(replies);
    let validators: Vec<Box<dyn Validator>> = vec![Box::new(
        TestsRun::new("git grep -qE a.[+].b$").expect("a command"),
    )];
    let mut run = Run::open(tree, options(id, 12, tokens), &harness, &validators).expect("opens");
    run.scan_change_with(Some(scanner(tree)));
    run.continue_past_red_base();
    let mut out = Vec::new();
    let stop = run.run(&mut out);
    let found = std::mem::take(&mut seen.borrow_mut().found);
    (stop, String::from_utf8_lossy(&out).into_owned(), found)
}

/// The case's run record, one JSON object per line.
fn events(tree: &Path, id: &str) -> Vec<serde_json::Value> {
    fs::read_to_string(tree.join(".takt/cases").join(id).join("events.jsonl"))
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

/// Asserts that `step` is the line of a try whose session named no action: no action, the
/// tokens its session reported, and a summary that says so.
fn assert_no_action_try(step: &serde_json::Value, text: &str) {
    assert_eq!(step["action"], "", "{step}\n{text}");
    assert!(
        step["tokens"].as_u64().is_some_and(|tokens| tokens > 0),
        "the try is charged its tokens: {step}\n{text}"
    );
    let summary = step["summary"].as_str().expect("a summary");
    assert!(
        summary.contains("named no action"),
        "the try's line says its session named no action: {summary}\n{text}"
    );
}

/// Acceptance 1 and 2: two sessions in a row end without the action line, the third names one;
/// the run goes on from it, each failed try is recorded with its tokens, and each retry started
/// from the tree the failed try was discarded to.
#[test]
fn a_session_without_an_action_line_is_tried_again() {
    let scratch = repository("again");
    let tree = scratch.tree();
    let id = "no-action-again";
    let (stop, text, found) = run(
        &tree,
        id,
        &[Reply::NoLine, Reply::NoLine, Reply::Fixes],
        None,
    );
    assert!(
        stop.is_ok(),
        "the run does not end on the error: {stop:?}\n{text}"
    );
    // The third try's session, and later steps' once the tests pass, are steps as any.
    assert!(found.len() >= 3, "three tries at least\n{text}");
    for (n, (lib, stray)) in found.iter().take(3).enumerate() {
        assert_eq!(
            lib,
            BROKEN,
            "session {}: the tree as discarded\n{text}",
            n + 1
        );
        assert!(!stray, "session {}: nothing of a failed try\n{text}", n + 1);
    }
    assert_eq!(
        git(&tree, &["show", "HEAD:src/lib.rs"]),
        FIXED.trim_end(),
        "the third try's change is kept\n{text}"
    );
    assert!(!tree.join("stray.txt").exists(), "{text}");
    let steps = harness_steps(&tree, id);
    assert_eq!(steps.len(), found.len(), "a line for each session\n{text}");
    assert_no_action_try(&steps[0], &text);
    assert_no_action_try(&steps[1], &text);
    assert!(
        steps[0]["summary"]
            .as_str()
            .is_some_and(|said| said.ends_with("(try 1 of 3)")),
        "{text}"
    );
    assert!(
        steps[1]["summary"]
            .as_str()
            .is_some_and(|said| said.ends_with("(try 2 of 3)")),
        "{text}"
    );
    assert_eq!(steps[2]["action"], "implementation.edit", "{text}");
    let record = events(&tree, id);
    assert!(
        !record.iter().any(|line| line["event"] == "error"),
        "no error line\n{text}"
    );
    let stopped = record
        .iter()
        .find(|line| line["event"] == "stop")
        .expect("a stop line");
    let charged: u64 = record
        .iter()
        .filter(|line| line["event"] == "step")
        .filter_map(|line| line["tokens"].as_u64())
        .sum();
    assert_eq!(
        stopped["tokens"], charged,
        "every try's tokens are the run's\n{text}"
    );
}

/// Acceptance 1: a session that never names its action is tried 3 times, each try recorded with
/// its tokens, and the run ends on the error after the third, the tree as it was.
#[test]
fn the_run_ends_on_the_error_after_the_third_try() {
    let scratch = repository("bound");
    let tree = scratch.tree();
    let id = "no-action-bound";
    let head = git(&tree, &["rev-parse", "HEAD"]);
    let (stop, text, found) = run(&tree, id, &[Reply::NoLine], None);
    let error = stop.expect_err("the run ends on the error");
    let said = format!("{error:#}");
    assert!(said.contains(NO_ACTION_LINE), "{said}\n{text}");
    assert!(
        said.contains("3 tries"),
        "the error names the bound: {said}\n{text}"
    );
    assert_eq!(found.len(), 3, "three sessions, no fourth\n{text}");
    for (n, (lib, stray)) in found.iter().enumerate() {
        assert_eq!(lib, BROKEN, "session {}\n{text}", n + 1);
        assert!(!stray, "session {}\n{text}", n + 1);
    }
    let steps = harness_steps(&tree, id);
    assert_eq!(steps.len(), 3, "{steps:#?}\n{text}");
    for step in &steps {
        assert_no_action_try(step, &text);
    }
    assert_eq!(git(&tree, &["rev-parse", "HEAD"]), head, "{text}");
    assert_eq!(
        fs::read_to_string(tree.join("src/lib.rs")).expect("the file"),
        BROKEN,
        "{text}"
    );
    assert!(!tree.join("stray.txt").exists(), "{text}");
    let record = events(&tree, id);
    let last = record.last().expect("a line");
    assert_eq!(last["event"], "error", "{last}\n{text}");
}

/// A try whose session named no action proposed nothing, and only a session that proposed is
/// observed into the case: of two such tries and a third that names its action, the case holds
/// that third session's `AgentSession` alone, and none naming no action.
#[test]
fn a_try_that_named_no_action_leaves_no_session_in_the_case() {
    let scratch = repository("observed");
    let tree = scratch.tree();
    let id = "no-action-observed";
    let (stop, text, _) = run(
        &tree,
        id,
        &[Reply::NoLine, Reply::NoLine, Reply::Fixes],
        None,
    );
    assert!(stop.is_ok(), "{stop:?}\n{text}");
    let path = tree
        .join(takt::case::CASES_DIR)
        .join(id)
        .join("observations.json");
    let held = fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("read {}: {error}\n{text}", path.display()));
    let observations: Vec<serde_json::Value> = serde_json::from_str(&held).expect("json");
    let sessions: Vec<&serde_json::Value> = observations
        .iter()
        .filter(|observation| observation["payload"]["record"] == "loom.session.AgentSession")
        .collect();
    assert!(!sessions.is_empty(), "the third try's session: {held}");
    assert_eq!(
        sessions[0]["payload"]["action"], "implementation.edit",
        "the first session observed is the third try's: {held}"
    );
    assert!(
        sessions
            .iter()
            .all(|session| session["payload"]["action"] != ""),
        "no session that named no action: {held}"
    );
}

/// A retry is a step like any: the run's token budget spent by a failed try stops the run on
/// the budget, with no further try.
#[test]
fn a_spent_budget_takes_no_further_try() {
    let scratch = repository("budget");
    let tree = scratch.tree();
    let id = "no-action-budget";
    let (stop, text, found) = run(&tree, id, &[Reply::NoLine], Some(1));
    assert!(matches!(stop, Ok(Stop::Tokens(1))), "{stop:?}\n{text}");
    assert_eq!(found.len(), 1, "one session\n{text}");
    let steps = harness_steps(&tree, id);
    assert_eq!(steps.len(), 1, "{steps:#?}\n{text}");
    assert_no_action_try(&steps[0], &text);
}

/// Acceptance 2: a step refused for doing what it may not is not retried.
#[test]
fn a_refused_step_is_not_retried() {
    let scratch = repository("refused");
    let tree = scratch.tree();
    let (stop, text, found) = run(&tree, "no-action-refused", &[Reply::Unoffered], None);
    assert!(matches!(stop, Ok(Stop::StepRefused(_))), "{stop:?}\n{text}");
    assert_eq!(found.len(), 1, "one session\n{text}");
}

/// Only a session that ends without the action line is retried: a harness whose program does
/// not start, or whose JSON line names no action, ends the run at its first try.
#[test]
fn another_harness_failure_is_not_retried() {
    for (name, reply) in [
        ("not-started", Reply::NotStarted),
        ("no-string", Reply::NoStringAction),
    ] {
        let scratch = repository(name);
        let tree = scratch.tree();
        let (stop, text, found) = run(&tree, &format!("no-action-{name}"), &[reply], None);
        let error = stop.expect_err("the run ends on the error");
        assert!(
            !format!("{error:#}").contains(NO_ACTION_LINE),
            "{name}: {error:#}"
        );
        assert_eq!(found.len(), 1, "{name}: one session\n{text}");
    }
}
