//! story:a-killed-step-records-its-tokens: a harness step that ends in an error or is killed by
//! the hour budget records the tokens its session reported before it ended, in a step line and
//! in the stop line's total, never as 0; and one whose session reported none is recorded as
//! unknown, so a comparison over its case is refused naming the case and the step. The harness
//! is Claude Code through metaharness, its program a script printing Claude Code's stream; no
//! live `claude` runs and no credential reaches it.

#[path = "support/own_home.rs"]
mod own_home;

use std::fs;
use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};
use std::process::Command;

use takt::case::CASES_DIR;
use takt::events::Events;
use takt::harness::{Kind, SessionHarness};
use takt::scorecard::Scorecard;
use takt::validator::Validator;
use takt::{Run, RunOptions, Stop};

fn git(tree: &Path, args: &[&str]) {
    let status = Command::new("git")
        .args(["-c", "user.name=t", "-c", "user.email=t@localhost"])
        .args(args)
        .current_dir(tree)
        .status()
        .expect("git runs");
    assert!(status.success(), "git {args:?}");
}

/// A scratch directory under this crate's build directory, removed when dropped.
struct Scratch(PathBuf);

impl Drop for Scratch {
    fn drop(&mut self) {
        if let Err(error) = fs::remove_dir_all(&self.0) {
            let message = format!("remove the scratch tree {}: {error}", self.0.display());
            if std::thread::panicking() {
                eprintln!("{message}");
            } else {
                panic!("{message}");
            }
        }
    }
}

/// A scratch directory holding `repo/`, a committed repository, and `claude`, a script that
/// prints `stream` and then runs `then`.
fn fixture(name: &str, stream: &[&str], then: &str) -> (Scratch, PathBuf, PathBuf) {
    let outer = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("killed-step-tokens-{}-{name}", std::process::id()));
    if outer.exists() {
        fs::remove_dir_all(&outer).expect("a stale scratch tree is removed");
    }
    let repo = outer.join("repo");
    fs::create_dir_all(&repo).expect("a repository");
    fs::write(repo.join("README.md"), "a story\n").expect("a file");
    git(&repo, &["init", "-q", "-b", "main"]);
    git(&repo, &["add", "-A"]);
    git(&repo, &["commit", "-q", "-m", "start"]);
    let mut script =
        "#!/bin/sh\ncase $1 in --version) echo '2.1.309 (Claude Code)'; exit 0 ;; esac\n"
            .to_owned();
    for line in stream {
        script.push_str(&format!("printf '%s\\n' '{line}'\n"));
    }
    script.push_str(then);
    script.push('\n');
    let program = outer.join("claude");
    fs::write(&program, script).expect("a script");
    fs::set_permissions(&program, fs::Permissions::from_mode(0o755)).expect("exec bit");
    (Scratch(outer), repo, program)
}

/// Claude Code's stream up to a kill or a failure: its init event, then two assistant
/// messages of 200,000 tokens each, the first written over two records that repeat its usage,
/// as Claude Code writes a message of two content blocks. 400,000 tokens in all.
const SPENT: &[&str] = &[
    r#"{"type":"system","subtype":"init","session_id":"s-1","model":"m","tools":["Read"]}"#,
    r#"{"type":"assistant","session_id":"s-1","message":{"id":"msg-1","model":"m","content":[{"type":"thinking","thinking":"plan"}],"usage":{"input_tokens":120000,"output_tokens":30000,"cache_read_input_tokens":40000,"cache_creation_input_tokens":10000}}}"#,
    r#"{"type":"assistant","session_id":"s-1","message":{"id":"msg-1","model":"m","content":[{"type":"text","text":"working"}],"usage":{"input_tokens":120000,"output_tokens":30000,"cache_read_input_tokens":40000,"cache_creation_input_tokens":10000}}}"#,
    r#"{"type":"assistant","session_id":"s-1","message":{"id":"msg-2","model":"m","content":[{"type":"text","text":"still working"}],"usage":{"input_tokens":150000,"output_tokens":50000}}}"#,
];

/// The tokens [`SPENT`] reports.
const SPENT_TOKENS: u64 = 400_000;

/// Claude Code through metaharness with `program`, on no credential.
fn harness(program: &Path) -> SessionHarness {
    let mut session = SessionHarness::of(Kind::Claude);
    session.program = program.display().to_string();
    session.credentials = loom_sdk::session::CredentialSource::None;
    session
}

fn options(
    id: &str,
    iterations: Option<u64>,
    hours: Option<f64>,
    tokens: Option<u64>,
) -> RunOptions {
    RunOptions {
        protocol: "software.change/1".to_owned(),
        intent: "probe".to_owned(),
        inputs: vec![
            ("risk".to_owned(), "standard".to_owned()),
            ("change_kind".to_owned(), "bugfix".to_owned()),
            ("affects_security_boundary".to_owned(), "false".to_owned()),
        ],
        iterations,
        hours,
        tokens,
        case_id: Some(id.to_owned()),
        gate: Some("repository.merge".to_owned()),
    }
}

/// Runs `options` on `repo` with `harness` and no validator: the stop, what the run printed,
/// and the case's record.
fn run(
    repo: &Path,
    harness: &SessionHarness,
    options: RunOptions,
) -> (Result<Stop, String>, String, Vec<serde_json::Value>) {
    let validators: Vec<Box<dyn Validator>> = Vec::new();
    let mut run = Run::open(repo, options, harness, &validators).expect("the case opens");
    let mut out = Vec::new();
    let stop = run.run(&mut out).map_err(|error| format!("{error:#}"));
    let text = String::from_utf8_lossy(&out).into_owned();
    let lines = Events::new(&run.case.dir).read().expect("the record reads");
    (stop, text, lines)
}

fn stop_line(lines: &[serde_json::Value]) -> &serde_json::Value {
    lines
        .iter()
        .rfind(|line| line["event"] == "stop")
        .unwrap_or_else(|| panic!("a stop line: {lines:?}"))
}

fn harness_steps(lines: &[serde_json::Value]) -> Vec<&serde_json::Value> {
    lines
        .iter()
        .filter(|line| line["event"] == "step" && line["by"] == "harness")
        .collect()
}

/// Acceptance 1: a session that reported 400,000 tokens and then hangs, killed by the hour
/// budget: the run stops on the budget, and its stop line and the step's line count the
/// 400,000, not 0.
#[test]
fn a_step_killed_by_the_hour_budget_counts_the_tokens_its_session_reported() {
    let (_scratch, repo, program) = fixture("hours", SPENT, "sleep 30 &\nwait");
    let harness = harness(&program);
    let (stop, text, lines) = run(
        &repo,
        &harness,
        options("killed-step-hours", Some(3), Some(0.001), None),
    );
    assert!(matches!(stop, Ok(Stop::Hours(_))), "{stop:?}\n{text}");
    assert_eq!(
        stop_line(&lines)["tokens"],
        SPENT_TOKENS,
        "the stop line counts what the killed session reported\n{text}\n{lines:?}"
    );
    let steps = harness_steps(&lines);
    assert_eq!(steps.len(), 1, "{lines:?}");
    assert_eq!(steps[0]["tokens"], SPENT_TOKENS, "{lines:?}");
}

/// Acceptance 2: a session that reported 400,000 tokens and then exits non-zero: each failed
/// step's tokens are in the stop line's total, and under a budget of 1,000,000 the third such
/// step stops the run on the budget instead of running a fourth.
#[test]
fn a_failed_step_counts_its_tokens_toward_the_token_budget() {
    let (_scratch, repo, program) = fixture("failed", SPENT, "exit 1");
    let harness = harness(&program);
    let (stop, text, lines) = run(
        &repo,
        &harness,
        options("killed-step-failed", None, None, Some(1_000_000)),
    );
    assert!(
        matches!(stop, Ok(Stop::Tokens(1_000_000))),
        "the third failed step spends the budget: {stop:?}\n{text}"
    );
    let steps = harness_steps(&lines);
    assert_eq!(steps.len(), 3, "three steps, not a fourth: {lines:?}");
    for step in &steps {
        assert_eq!(step["tokens"], SPENT_TOKENS, "{lines:?}");
    }
    assert_eq!(stop_line(&lines)["tokens"], 3 * SPENT_TOKENS, "{lines:?}");
}

/// The fixture of a trial cycle's scenario cases, as `run_tokens_unknown.rs` reads it.
const FIXTURE: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/fixtures/judge-steps-0.3.0-cand-1"
);

/// Acceptance 3: a session that ends with no usage line: the step is recorded with its tokens
/// unknown, not 0, and a comparison over its case is refused, naming the case and the step.
#[test]
fn a_failed_step_whose_session_reported_no_tokens_is_recorded_unknown() {
    let case = "scenario-1-trial-97";
    let (_scratch, repo, program) = fixture("unreported", &[SPENT[0]], "exit 1");
    let harness = harness(&program);
    let (stop, text, lines) = run(&repo, &harness, options(case, Some(1), None, None));
    assert!(matches!(stop, Ok(Stop::Iterations(1))), "{stop:?}\n{text}");
    let steps = harness_steps(&lines);
    assert_eq!(steps.len(), 1, "the failed step has its line: {lines:?}");
    assert!(
        steps[0]
            .get("tokens")
            .is_none_or(serde_json::Value::is_null),
        "its tokens are unknown, never 0: {lines:?}"
    );

    // The case among the fixture's other candidate cases, its held-out check passed.
    let cases = repo.join(CASES_DIR);
    for entry in fs::read_dir(Path::new(FIXTURE).join("candidate")).expect("the fixture") {
        let from = entry.expect("a case").path();
        let name = from.file_name().expect("a name").to_owned();
        if name == case {
            continue;
        }
        let to = cases.join(&name);
        fs::create_dir_all(&to).expect("the case directory");
        for file in ["events.jsonl", "held-out.txt"] {
            fs::copy(from.join(file), to.join(file)).expect("a fixture file");
        }
    }
    fs::write(cases.join(case).join("held-out.txt"), "PASS\n").expect("the held-out result");
    let card = Scorecard::read(&repo).expect("the runs read");
    let row = card
        .rows
        .iter()
        .find(|row| row.case == case)
        .unwrap_or_else(|| panic!("{case}: {card:?}"));
    assert_eq!(row.tokens, None, "{row:?}");

    let baseline = Path::new(FIXTURE).join("baseline");
    let saved_repo = repo.parent().expect("the scratch").join("saved");
    for entry in fs::read_dir(&baseline).expect("the fixture") {
        let from = entry.expect("a case").path();
        let to = saved_repo
            .join(CASES_DIR)
            .join(from.file_name().expect("a name"));
        fs::create_dir_all(&to).expect("the case directory");
        for file in ["events.jsonl", "held-out.txt"] {
            fs::copy(from.join(file), to.join(file)).expect("a fixture file");
        }
    }
    let saved = Scorecard::read(&saved_repo).expect("the saved runs read");
    let refused = card
        .against(&saved, &mut Vec::new())
        .expect_err("refused")
        .to_string();
    assert!(
        refused.contains(case) && refused.contains("step"),
        "the refusal names the case and the step: {refused}"
    );
}
