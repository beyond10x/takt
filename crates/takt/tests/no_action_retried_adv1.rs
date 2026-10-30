//! Adversary pass on story:a-harness-step-without-an-action-is-retried.
//!
//! 1. A try whose session reported no tokens is still written as a harness step line, with no
//!    count; the scorecard reads such a line as unknown run tokens (story
//!    run-tokens-unknown-is-not-zero: "a harness step without a count ... which takt never
//!    writes"), so a run that recovered on its second try has unknown tokens and every
//!    comparison with its case is refused. Before the retry, a failed session that reported no
//!    tokens wrote no line ([`takt::run`]'s `charge_unproposed`).
//! 2. A try after which the run's own stop comes first (a budget) is told on the run's output as
//!    "the step is tried again", and the run then stops without trying it again.
//!
//! Each session here is metaharness's scripted b10x-harness one: no vendor process, no
//! credential, no network.

#[path = "support/own_home.rs"]
mod own_home;

use std::cell::RefCell;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::rc::Rc;

use loom_sdk::session::metaharness::{
    HarnessProcess, LaunchPlanView, ProcessRunner, ScriptedLog, ScriptedRunner,
};
use loom_sdk::session::{CredentialSource, Endpoint, Kind};
use takt::harness::SessionHarness;
use takt::scorecard::Scorecard;
use takt::validator::{TestsRun, Validator};
use takt::{Run, RunOptions, Stop};

const BROKEN: &str = "pub fn add(a: i32, b: i32) -> i32 {\n    a - b\n}\n";
const FIXED: &str = "pub fn add(a: i32, b: i32) -> i32 {\n    a + b\n}\n";

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
        .join(format!("no-action-adv1-{}-{name}", std::process::id()));
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
        protocol: "story.delivery/1".to_owned(),
        intent: "make add add".to_owned(),
        inputs: Vec::new(),
        iterations: Some(iterations),
        hours: None,
        tokens: None,
        case_id: Some(id.to_owned()),
        gate: None,
    }
}

#[derive(Clone, Copy)]
enum Reply {
    /// Ends without the JSON line naming its action, its stream reporting tokens.
    NoLine,
    /// Ends without the JSON line naming its action, its stream reporting no usage.
    NoLineNoUsage,
    /// Fixes `src/lib.rs` and names `implementation.edit`.
    Fixes,
}

struct Session {
    replies: Vec<Reply>,
    calls: Rc<RefCell<usize>>,
}

impl ProcessRunner for Session {
    fn requires_executable(&self) -> bool {
        false
    }

    fn start(&mut self, plan: &LaunchPlanView) -> std::io::Result<Box<dyn HarnessProcess>> {
        let n = {
            let mut calls = self.calls.borrow_mut();
            *calls += 1;
            *calls
        };
        let reply = self.replies[(n - 1).min(self.replies.len() - 1)];
        let (text, usage) = match reply {
            Reply::NoLine => ("I was summarizing the conversation and".to_owned(), true),
            Reply::NoLineNoUsage => ("I was summarizing the conversation and".to_owned(), false),
            Reply::Fixes => {
                fs::write(plan.cwd.join("src/lib.rs"), FIXED)?;
                (
                    format!(
                        "Done.\n{}",
                        serde_json::json!({"action": "implementation.edit", "summary": "fixed"})
                    ),
                    true,
                )
            }
        };
        let mut lines = vec![
            serde_json::json!({"kind": "started", "model": "scripted",
                "published_tools": ["file_read", "file_write"],
                "operations": ["file.read", "file.write"], "withheld": [], "skills": [],
                "agents": [], "profiles": [], "context": [], "toolchains": [],
                "credential_source": "oauth-token:file"}),
            serde_json::json!({"kind": "turn-started", "turn": 1}),
            serde_json::json!({"kind": "text-delta", "text": text}),
        ];
        if usage {
            lines.push(serde_json::json!({"kind": "usage", "model": "scripted",
                "input_tokens": 100, "output_tokens": 20, "cached_input_tokens": 40}));
        }
        lines.push(
            serde_json::json!({"kind": "finished", "stop": {"kind": "completed"}, "turns": 1}),
        );
        ScriptedRunner::of_lines(lines.iter().map(ToString::to_string), ScriptedLog::new())
            .start(plan)
    }
}

fn harness(replies: &[Reply]) -> (SessionHarness, Rc<RefCell<usize>>) {
    let calls = Rc::new(RefCell::new(0));
    let shared = Rc::clone(&calls);
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
            calls: Rc::clone(&shared),
        })
    });
    (harness, calls)
}

// story:takt-carries-no-organisations-own-route: the stand-in scanner is gone; these runs declare scan: none.

fn run(
    tree: &Path,
    id: &str,
    replies: &[Reply],
    iterations: u64,
) -> (anyhow::Result<Stop>, String, usize) {
    let (harness, calls) = harness(replies);
    let validators: Vec<Box<dyn Validator>> = vec![Box::new(
        TestsRun::new("git grep -qE a.[+].b$").expect("a command"),
    )];
    let mut run = Run::open(tree, options(id, iterations), &harness, &validators).expect("opens");
    run.scan_with(takt::scan::Scan::None {
        declared_in: tree.with_file_name("run.yaml"),
    });
    run.continue_past_red_base();
    let mut out = Vec::new();
    let stop = run.run(&mut out);
    let calls = *calls.borrow();
    (stop, String::from_utf8_lossy(&out).into_owned(), calls)
}

fn events(tree: &Path, id: &str) -> String {
    fs::read_to_string(tree.join(".takt/cases").join(id).join("events.jsonl"))
        .expect("events.jsonl")
}

/// A try whose session reported no tokens, then a try that names its action: the run recovers,
/// and its tokens are unknown, never a sum that counts the try 0: the try's line carries no
/// count (story:a-killed-step-records-its-tokens, which replaces the earlier reading that such a
/// try leaves the run's tokens known).
#[test]
fn a_try_that_reported_no_tokens_leaves_the_run_tokens_unknown() {
    let scratch = repository("no-usage");
    let tree = scratch.tree();
    let id = "no-action-adv1-no-usage";
    let (stop, text, calls) = run(&tree, id, &[Reply::NoLineNoUsage, Reply::Fixes], 12);
    assert!(stop.is_ok(), "the run recovers: {stop:?}\n{text}");
    assert!(calls >= 2, "a second try was made\n{text}");
    let scorecard = Scorecard::read(&tree).expect("the scorecard reads");
    let row = scorecard
        .rows
        .iter()
        .find(|row| row.case == id)
        .expect("the case's row");
    assert!(
        row.tokens.is_none() && !row.unknown_steps().is_empty(),
        "a try that reported no tokens leaves the run's tokens unknown, so `takt scorecard \
         --against` and `takt promote` refuse the case (tokens {:?}, unknown steps {:?})\n{}",
        row.tokens,
        row.unknown_steps(),
        events(&tree, id)
    );
}

/// A try after which the run's step budget is spent (step 1 the tests, step 2 the try): the run
/// stops on the budget, makes no second try, and its output does not say the step is tried again.
#[test]
fn a_try_the_budget_ends_is_not_told_as_tried_again() {
    let scratch = repository("told");
    let tree = scratch.tree();
    let id = "no-action-adv1-told";
    let (stop, text, calls) = run(&tree, id, &[Reply::NoLine], 2);
    assert!(matches!(stop, Ok(Stop::Iterations(2))), "{stop:?}\n{text}");
    assert_eq!(calls, 1, "one session\n{text}");
    assert!(
        !text.contains("the step is tried again"),
        "the output says the step is tried again, and the run stopped without a second try:\n{text}"
    );
}

/// The bound is per step, not per run: two judgement steps, each recovering on its third try,
/// take six sessions and the run does not end on the error. Green on the tree attacked; it is the
/// case that fails a counter kept across steps, which no case of `no_action_retried.rs` does.
#[test]
fn the_bound_on_tries_is_per_step() {
    let scratch = repository("per-step");
    let tree = scratch.tree();
    let id = "no-action-adv1-per-step";
    let replies = [
        Reply::NoLine,
        Reply::NoLine,
        Reply::Fixes,
        Reply::NoLine,
        Reply::NoLine,
        Reply::Fixes,
    ];
    let (stop, text, calls) = run(&tree, id, &replies, 20);
    assert!(
        stop.is_ok(),
        "the run does not end on the error: {stop:?}\n{text}"
    );
    assert!(calls >= 6, "two steps of three tries each: {calls}\n{text}");
}
