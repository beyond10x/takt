//! story:step-transcripts-are-kept: each harness step's stream, the prompt takt gave it as the
//! first line and then every line the harness printed (tool calls, replies, usage), is kept as
//! `<transcripts>/<id>/run-<r>/step-<n>.jsonl`, outside the tree and its cases directory,
//! readable by its owner only, `r` the run's number in the case, so a resumed case keeps every
//! run's; the step's line names it; a step that fails, times out or runs past the hour budget
//! keeps what it printed too; and a file planted at that path is replaced, never written
//! through. Each test gives its runs its own transcripts directory. No live `claude` runs: the
//! harness program is a script.

#[path = "support/own_home.rs"]
mod own_home;

use std::fs;
use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};
use std::time::Duration;
use takt::confinement::Command;

use takt::events::Events;
use takt::harness::ClaudeCode;
use takt::validator::Validator;
use takt::{Run, RunOptions, Stop};

/// A scratch directory under this crate's build directory, removed when dropped.
struct Scratch(PathBuf);

impl std::ops::Deref for Scratch {
    type Target = Path;

    fn deref(&self) -> &Path {
        &self.0
    }
}

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

/// An empty scratch directory named for this process and `name`.
fn directory(name: &str) -> Scratch {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("step-transcripts-{}-{name}", std::process::id()));
    if dir.exists() {
        fs::remove_dir_all(&dir).expect("a stale scratch tree is removed");
    }
    fs::create_dir_all(&dir).expect("a scratch directory");
    Scratch(dir)
}

fn git(tree: &Path, args: &[&str]) {
    let status = Command::new("git")
        .args(["-c", "user.name=t", "-c", "user.email=t@localhost"])
        .args(args)
        .current_dir(tree)
        .status()
        .expect("git runs");
    assert!(status.success(), "git {args:?}");
}

/// A committed repository on `main` with a README.
fn repository(name: &str) -> Scratch {
    let tree = directory(name);
    fs::write(tree.join("README.md"), "a story\n").expect("a file");
    git(&tree, &["init", "-q", "-b", "main"]);
    git(&tree, &["add", "-A"]);
    git(&tree, &["commit", "-q", "-m", "start"]);
    tree
}

fn options(id: &str, iterations: u64, hours: Option<f64>) -> RunOptions {
    RunOptions {
        protocol: "software.change/1".to_owned(),
        intent: "keep the transcript".to_owned(),
        inputs: vec![
            ("risk".to_owned(), "standard".to_owned()),
            ("change_kind".to_owned(), "bugfix".to_owned()),
            ("affects_security_boundary".to_owned(), "false".to_owned()),
        ],
        iterations: Some(iterations),
        hours,
        tokens: None,
        case_id: Some(id.to_owned()),
        gate: Some("repository.merge".to_owned()),
    }
}

/// A fake `claude` in `dir`. Each call counts itself in `dir/calls`, writes its arguments one
/// per line to `dir/args-<n>`, writes the stream it prints to `dir/printed-<n>` (an init line,
/// a tool call, a tool result, all naming the call), prints it, and then runs `end`, the shell
/// text that ends the call: `$printed` names the file, `$n` the call.
fn fake_claude(dir: &Path, end: &str) -> ClaudeCode {
    let script = r#"#!/bin/sh
d='@DIR@'
n=$(cat "$d/calls" 2>/dev/null || echo 0)
n=$((n + 1))
echo "$n" > "$d/calls"
printf '%s\n' "$@" > "$d/args-$n"
printed="$d/printed-$n"
printf '%s\n' "{\"type\":\"system\",\"subtype\":\"init\",\"call\":$n}" > "$printed"
printf '%s\n' "{\"type\":\"assistant\",\"message\":{\"content\":[{\"type\":\"tool_use\",\"name\":\"Read\",\"input\":{\"file_path\":\"README.md\"}}]},\"call\":$n}" >> "$printed"
printf '%s\n' "{\"type\":\"user\",\"message\":{\"content\":[{\"type\":\"tool_result\",\"content\":\"a story\"}]},\"call\":$n}" >> "$printed"
cat "$printed"
@END@
"#
    .replace("@DIR@", &dir.display().to_string())
    .replace("@END@", end);
    let program = dir.join("claude");
    fs::write(&program, script).expect("a script");
    fs::set_permissions(&program, fs::Permissions::from_mode(0o755)).expect("exec bit");
    ClaudeCode {
        program: program.display().to_string(),
        ..ClaudeCode::default()
    }
}

/// The end of a call that writes a test file and reports `tests.write` in the result line,
/// written to the stream file as well.
const EDITS: &str = r#"mkdir -p tests
echo "$n" >> tests/work.txt
line='{"type":"result","subtype":"success","result":"{\"action\": \"tests.write\", \"summary\": \"wrote a test\"}","usage":{"input_tokens":3,"output_tokens":4},"num_turns":2}'
printf '%s\n' "$line" >> "$printed"
printf '%s\n' "$line""#;

/// The transcripts directory a test's runs are given: `transcripts` in its scripts directory,
/// resolved, outside the tree and its cases directory.
fn root(scripts: &Path) -> PathBuf {
    fs::canonicalize(scripts)
        .expect("the scripts directory resolves")
        .join("transcripts")
}

/// The repository's segment of the transcripts directory: the first 16 hex digits of the
/// sha256 over its canonical path.
fn repo_key(tree: &Path) -> String {
    use sha2::Digest as _;
    use std::os::unix::ffi::OsStrExt as _;
    let canonical = fs::canonicalize(tree).expect("the tree resolves");
    let digest = format!(
        "{:x}",
        sha2::Sha256::digest(canonical.as_os_str().as_bytes())
    );
    digest[..16].to_owned()
}

/// The directory of the case `id` of `tree` under the transcripts directory `root`.
fn case_dir_in(root: &Path, tree: &Path, id: &str) -> PathBuf {
    root.join(repo_key(tree)).join(id)
}

/// The transcript of step `n` of run `run` of the case `id` of `tree` as the run record names
/// it: relative to the transcripts directory, `<repo-key>/<id>/run-<run>/step-<n>.jsonl`.
fn relative(tree: &Path, id: &str, run: usize, n: usize) -> String {
    format!("{}/{id}/run-{run}/step-{n}.jsonl", repo_key(tree))
}

/// The one line of the case `id`'s run record whose `event` is `event`.
fn line_of(tree: &Path, id: &str, event: &str) -> serde_json::Value {
    let lines = Events::new(&tree.join(".takt/cases").join(id))
        .read()
        .expect("the run record");
    let found: Vec<&serde_json::Value> =
        lines.iter().filter(|line| line["event"] == event).collect();
    assert_eq!(found.len(), 1, "one {event} line in {lines:#?}");
    found[0].clone()
}

/// The transcript of step `n` of the case's run `run` (1 for the run that opened it) of the
/// case `id` of `tree`, under the transcripts directory of `scripts`.
fn transcript(tree: &Path, scripts: &Path, id: &str, run: usize, n: usize) -> PathBuf {
    case_dir_in(&root(scripts), tree, id)
        .join(format!("run-{run}"))
        .join(format!("step-{n}.jsonl"))
}

/// Asserts that step `n`'s transcript of run `run` is a plain file only its owner reads, its
/// run's and its case's directories only its owner reads, opening with a JSON line whose
/// `prompt` is the prompt takt gave the harness for step `n` (the one the fake received after
/// `-p` at its call `call`), followed by exactly the bytes the fake printed for call `call`;
/// and that the case's `artifacts/` in `tree` holds no transcript.
fn assert_kept(
    tree: &Path,
    id: &str,
    (run, n, call): (usize, usize, usize),
    scripts: &Path,
    text: &str,
) {
    assert_kept_in(&root(scripts), tree, id, (run, n, call), scripts, text);
}

/// [`assert_kept`] under the transcripts directory `root`; the repository's directory there
/// is only its owner's too.
fn assert_kept_in(
    root: &Path,
    tree: &Path,
    id: &str,
    (run, n, call): (usize, usize, usize),
    scripts: &Path,
    text: &str,
) {
    let case_dir = case_dir_in(root, tree, id);
    let path = case_dir
        .join(format!("run-{run}"))
        .join(format!("step-{n}.jsonl"));
    let meta = fs::symlink_metadata(&path).unwrap_or_else(|error| {
        panic!(
            "run {run} step {n}'s transcript {}: {error}\n{text}",
            path.display()
        )
    });
    for dir in [
        path.parent().expect("its run directory"),
        &case_dir,
        case_dir.parent().expect("its repository's directory"),
    ] {
        let meta = fs::symlink_metadata(dir).expect("the directory");
        assert!(meta.is_dir(), "a plain directory: {}", dir.display());
        assert_eq!(
            meta.permissions().mode() & 0o777,
            0o700,
            "{}",
            dir.display()
        );
    }
    assert!(meta.is_file(), "a plain file: {meta:?}");
    assert_eq!(
        meta.permissions().mode() & 0o777,
        0o600,
        "only its owner reads a transcript: {}",
        path.display()
    );
    let in_case = tree
        .join(".takt/cases")
        .join(id)
        .join("artifacts")
        .join(format!("run-{run}"));
    let left: Vec<PathBuf> = fs::read_dir(&in_case)
        .map(|entries| {
            entries
                .map(|entry| entry.expect("an entry").path())
                .collect()
        })
        .unwrap_or_default();
    assert!(
        left.is_empty(),
        "no transcript under the case's artifacts: {left:?}"
    );
    let kept = fs::read(&path).expect("the transcript reads");
    let (first, rest) = kept.split_at(
        kept.iter()
            .position(|byte| *byte == b'\n')
            .expect("a first line")
            + 1,
    );
    let first: serde_json::Value = serde_json::from_slice(first).expect("the first line is JSON");
    let args = fs::read_to_string(scripts.join(format!("args-{call}"))).expect("the arguments");
    let given = args
        .split_once("-p\n")
        .map(|(_, after)| after)
        .expect("a prompt after -p");
    let prompt = first["prompt"]
        .as_str()
        .expect("the first line carries the prompt");
    assert!(
        prompt.contains(&format!("Step {n}.")) && given.starts_with(prompt),
        "the prompt given at step {n}: {first}"
    );
    let printed = fs::read(scripts.join(format!("printed-{call}"))).expect("what the fake printed");
    assert_eq!(
        String::from_utf8_lossy(rest),
        String::from_utf8_lossy(&printed),
        "step {n}'s transcript holds the stream the harness printed"
    );
}

/// Acceptance 1: after a run with the claude harness, every harness step has its transcript,
/// holding the stream the harness printed; the harness is asked for the stream, not the one
/// result document.
#[test]
fn every_harness_step_keeps_the_stream_it_printed() {
    let tree = repository("every-tree");
    let scripts = directory("every-scripts");
    let harness = fake_claude(&scripts, EDITS);
    let validators: Vec<Box<dyn Validator>> = Vec::new();
    let mut run =
        Run::open(&tree, options("kept", 2, None), &harness, &validators).expect("the case opens");
    run.transcripts_in(root(&scripts))
        .expect("a transcripts directory");
    let mut out = Vec::new();
    let stop = run.run(&mut out);
    let text = String::from_utf8_lossy(&out).into_owned();
    stop.unwrap_or_else(|error| panic!("the run runs: {error:#}\n{text}"));
    assert_eq!(run.steps, 2, "two harness steps\n{text}");
    for n in 1..=2 {
        assert_kept(&tree, "kept", (1, n, n), &scripts, &text);
        let args = fs::read_to_string(scripts.join(format!("args-{n}"))).expect("the arguments");
        let args: Vec<&str> = args.lines().collect();
        assert!(
            args.windows(2)
                .any(|pair| pair == ["--output-format", "stream-json"])
                && args.contains(&"--verbose"),
            "the harness is asked for its stream: {args:?}"
        );
    }
    assert!(
        !transcript(&tree, &scripts, "kept", 1, 3).exists(),
        "no step 3 ran"
    );
    let lines = Events::new(&run.case.dir).read().expect("the run record");
    let named: Vec<&str> = lines
        .iter()
        .filter(|line| line["event"] == "step" && line["by"] == "harness")
        .map(|line| line["transcript"].as_str().unwrap_or("none"))
        .collect();
    // Relative to the transcripts directory: no home directory in a run record.
    let paths: Vec<String> = (1..=2).map(|n| relative(&tree, "kept", 1, n)).collect();
    assert_eq!(named, paths, "each step's line names its transcript");
}

/// A run given no transcripts directory keeps no transcript, and its step's line names none:
/// a library caller opts in ([`Run::transcripts_in`]), so nothing is written outside what it
/// gave.
#[test]
fn a_run_given_no_transcripts_directory_keeps_none() {
    let tree = repository("none-tree");
    let scripts = directory("none-scripts");
    let harness = fake_claude(&scripts, EDITS);
    let validators: Vec<Box<dyn Validator>> = Vec::new();
    let mut run =
        Run::open(&tree, options("none", 1, None), &harness, &validators).expect("the case opens");
    assert!(run.transcripts().is_none());
    let mut out = Vec::new();
    let stop = run.run(&mut out);
    let text = String::from_utf8_lossy(&out).into_owned();
    stop.unwrap_or_else(|error| panic!("the run runs: {error:#}\n{text}"));
    let lines = Events::new(&run.case.dir).read().expect("the run record");
    let step = lines
        .iter()
        .find(|line| line["event"] == "step" && line["by"] == "harness")
        .unwrap_or_else(|| panic!("a harness step in {lines:#?}"));
    assert!(step.get("transcript").is_none(), "{step}");
    assert!(!root(&scripts).exists());
    assert!(!run.case.dir.join("artifacts").join("run-1").exists());
}

/// Acceptance 1 across runs: a resumed case keeps every run's transcripts. The run that opens
/// the case takes a step, the run that resumes it takes its own step 1; each run's transcript
/// is under its own number, and the second does not replace the first.
#[test]
fn a_resumed_case_keeps_the_transcripts_of_every_run() {
    let tree = repository("resumed-tree");
    let scripts = directory("resumed-scripts");
    let harness = fake_claude(&scripts, EDITS);
    let validators: Vec<Box<dyn Validator>> = Vec::new();
    let mut opened = Run::open(&tree, options("resumed", 1, None), &harness, &validators)
        .expect("the case opens");
    opened
        .transcripts_in(root(&scripts))
        .expect("a transcripts directory");
    let mut out = Vec::new();
    let stop = opened.run(&mut out);
    let text = String::from_utf8_lossy(&out).into_owned();
    stop.unwrap_or_else(|error| panic!("the opening run runs: {error:#}\n{text}"));
    assert_kept(&tree, "resumed", (1, 1, 1), &scripts, &text);
    let first = fs::read(transcript(&tree, &scripts, "resumed", 1, 1)).expect("run 1's transcript");
    drop(opened);
    let mut resumed = Run::resume(
        &tree,
        options("resumed", 1, None),
        "resumed",
        &harness,
        &validators,
    )
    .expect("the case resumes");
    resumed
        .transcripts_in(root(&scripts))
        .expect("a transcripts directory");
    let mut out = Vec::new();
    let stop = resumed.run(&mut out);
    let text = String::from_utf8_lossy(&out).into_owned();
    stop.unwrap_or_else(|error| panic!("the resumed run runs: {error:#}\n{text}"));
    assert_eq!(
        resumed.steps, 1,
        "the resumed run took its own step 1\n{text}"
    );
    assert_kept(&tree, "resumed", (2, 1, 2), &scripts, &text);
    assert_eq!(
        fs::read(transcript(&tree, &scripts, "resumed", 1, 1))
            .expect("run 1's transcript, still there"),
        first,
        "the resumed run left the opening run's transcript as it was"
    );
    assert_ne!(
        first,
        fs::read(transcript(&tree, &scripts, "resumed", 2, 1)).expect("run 2's transcript"),
        "each run's step 1 has its own transcript"
    );
}

/// The reply is read from the result event wherever it stands, not from the last line: a
/// stream with a line after its result event (a late system event) is read from the result
/// event, the step is taken, and the transcript keeps every line, the late one included.
#[test]
fn a_line_after_the_result_event_does_not_hide_the_reply() {
    let tree = repository("late-tree");
    let scripts = directory("late-scripts");
    let end = format!(
        "{EDITS}\nlate='{{\"type\":\"system\",\"subtype\":\"late\",\"note\":\"after the result\"}}'\nprintf '%s\\n' \"$late\" >> \"$printed\"\nprintf '%s\\n' \"$late\""
    );
    let harness = fake_claude(&scripts, &end);
    let validators: Vec<Box<dyn Validator>> = Vec::new();
    let mut run =
        Run::open(&tree, options("late", 1, None), &harness, &validators).expect("the case opens");
    run.transcripts_in(root(&scripts))
        .expect("a transcripts directory");
    let mut out = Vec::new();
    let stop = run.run(&mut out);
    let text = String::from_utf8_lossy(&out).into_owned();
    stop.unwrap_or_else(|error| {
        panic!("the step is read from its result event: {error:#}\n{text}")
    });
    assert!(text.contains("step 1: tests.write"), "{text}");
    assert_kept(&tree, "late", (1, 1, 1), &scripts, &text);
}

/// Acceptance 2: a step whose harness exits non-zero after printing keeps its transcript; the
/// run ends on the harness's failure.
#[test]
fn a_step_whose_harness_fails_keeps_its_transcript() {
    let tree = repository("fails-tree");
    let scripts = directory("fails-scripts");
    let harness = fake_claude(&scripts, "exit 3");
    let validators: Vec<Box<dyn Validator>> = Vec::new();
    let mut run =
        Run::open(&tree, options("fails", 1, None), &harness, &validators).expect("the case opens");
    run.transcripts_in(root(&scripts))
        .expect("a transcripts directory");
    let mut out = Vec::new();
    let stop = run.run(&mut out);
    let text = String::from_utf8_lossy(&out).into_owned();
    let error = stop.expect_err("the harness failed");
    assert!(format!("{error:#}").contains("exited 3"), "{error:#}");
    assert_kept(&tree, "fails", (1, 1, 1), &scripts, &text);
    // No step line is written for it: the run's `error` line names the transcript, relative to
    // the transcripts directory, and the run's output names it whole.
    let named = relative(&tree, "fails", 1, 1);
    let line = line_of(&tree, "fails", "error");
    let said = line["error"].as_str().expect("the error");
    assert!(
        said.contains(&format!("its transcript: {named}"))
            && !said.contains(&root(&scripts).display().to_string()),
        "the error line names the transcript {named}: {said}"
    );
    let whole = transcript(&tree, &scripts, "fails", 1, 1);
    assert!(
        text.contains(&format!("transcript: {}", whole.display())),
        "the run's output names the transcript {}:\n{text}",
        whole.display()
    );
}

/// Acceptance 2: a step whose harness ends without the line naming its action keeps its
/// transcript.
#[test]
fn a_step_whose_reply_names_no_action_keeps_its_transcript() {
    let tree = repository("silent-tree");
    let scripts = directory("silent-scripts");
    let end = r#"line='{"type":"result","subtype":"error_max_turns","usage":{"input_tokens":3}}'
printf '%s\n' "$line" >> "$printed"
printf '%s\n' "$line""#;
    let harness = fake_claude(&scripts, end);
    let validators: Vec<Box<dyn Validator>> = Vec::new();
    let mut run = Run::open(&tree, options("silent", 1, None), &harness, &validators)
        .expect("the case opens");
    run.transcripts_in(root(&scripts))
        .expect("a transcripts directory");
    let mut out = Vec::new();
    let stop = run.run(&mut out);
    let text = String::from_utf8_lossy(&out).into_owned();
    let error = stop.expect_err("the reply names no action");
    assert!(
        format!("{error:#}").contains("without the JSON line naming its action"),
        "{error:#}"
    );
    assert_kept(&tree, "silent", (1, 1, 1), &scripts, &text);
}

/// Acceptance 2: a step whose harness outlives its own timeout is killed and keeps what it
/// printed before.
#[test]
fn a_step_whose_harness_times_out_keeps_what_it_printed() {
    let tree = repository("timeout-tree");
    let scripts = directory("timeout-scripts");
    let mut harness = fake_claude(&scripts, "exec sleep 30");
    harness.limits.timeout = Duration::from_secs(2);
    let validators: Vec<Box<dyn Validator>> = Vec::new();
    let mut run = Run::open(&tree, options("timeout", 1, None), &harness, &validators)
        .expect("the case opens");
    run.transcripts_in(root(&scripts))
        .expect("a transcripts directory");
    let mut out = Vec::new();
    let stop = run.run(&mut out);
    let text = String::from_utf8_lossy(&out).into_owned();
    let error = stop.expect_err("the harness timed out");
    assert!(format!("{error:#}").contains("timed out"), "{error:#}");
    assert_kept(&tree, "timeout", (1, 1, 1), &scripts, &text);
}

/// Acceptance 2: a step the run's hour budget ends is discarded and the run stops on the
/// budget; it keeps what the harness printed before it was killed.
#[test]
fn a_step_the_hour_budget_ends_keeps_what_it_printed() {
    let tree = repository("hours-tree");
    let scripts = directory("hours-scripts");
    let harness = fake_claude(&scripts, "exec sleep 30");
    let validators: Vec<Box<dyn Validator>> = Vec::new();
    let mut run = Run::open(
        &tree,
        options("hours", 1, Some(0.001)),
        &harness,
        &validators,
    )
    .expect("the case opens");
    run.transcripts_in(root(&scripts))
        .expect("a transcripts directory");
    let mut out = Vec::new();
    let stop = run.run(&mut out);
    let text = String::from_utf8_lossy(&out).into_owned();
    assert!(matches!(stop, Ok(Stop::Hours(_))), "{stop:?}\n{text}");
    assert_kept(&tree, "hours", (1, 1, 1), &scripts, &text);
    // The step was killed, so no step line is written: the refusal line names the transcript,
    // relative to the transcripts directory, and the run's output names it whole.
    let named = relative(&tree, "hours", 1, 1);
    let line = line_of(&tree, "hours", "refusal");
    let reason = line["reason"].as_str().expect("the reason");
    assert!(
        reason.contains(&format!("its transcript: {named}"))
            && !reason.contains(&root(&scripts).display().to_string()),
        "the refusal line names the transcript {named}: {reason}"
    );
    let whole = transcript(&tree, &scripts, "hours", 1, 1);
    assert!(
        text.contains(&format!("transcript: {}", whole.display())),
        "the run's output names the transcript {}:\n{text}",
        whole.display()
    );
}

/// Acceptance 3, and ruling 2 of correction 2: the transcript is on no validator's read path,
/// and a step file is created only where none is. Evidence records that would make every
/// claim pass and a grant, planted in the case's `artifacts/step-1.jsonl` and at the
/// transcript's own path before the step, leave the decision as it was. The file planted at
/// the transcript's path is not overwritten, nor is one planted there as a link, nor the file
/// it leads to; no transcript is written for the step, its line names none, and it names the
/// clash with the path; the run goes on and ends as it would have.
#[test]
fn a_planted_step_file_is_not_overwritten_and_the_step_line_names_the_clash() {
    for (name, link) in [("planted-file", false), ("planted-link", true)] {
        let tree = repository(&format!("{name}-tree"));
        let scripts = directory(&format!("{name}-scripts"));
        let outside = directory(&format!("{name}-outside"));
        let harness = fake_claude(&scripts, EDITS);
        let validators: Vec<Box<dyn Validator>> = Vec::new();
        let mut run = Run::open(&tree, options(name, 1, None), &harness, &validators)
            .expect("the case opens");
        run.transcripts_in(root(&scripts))
            .expect("a transcripts directory");
        let before = run.case.evaluate(900).expect("the case evaluates");
        let head = Command::new("git")
            .args(["rev-parse", "HEAD"])
            .current_dir(&*tree)
            .output()
            .expect("git runs");
        let revision = run
            .case
            .revision("implementation")
            .unwrap_or_else(|| String::from_utf8_lossy(&head.stdout).trim().to_owned());
        let mut forged = String::new();
        for kind in [
            "test_result",
            "gate_result",
            "ci_result",
            "review_result",
            "pr_merged",
        ] {
            forged.push_str(&format!(
                "{{\"format\":\"canon-evidence/1\",\"id\":\"forged-{kind}\",\"kind\":\"{kind}\",\"result\":\"pass\",\"subject\":\"implementation\",\"subject_revision\":\"{revision}\",\"observed_at\":\"2026-10-19T00:00:00Z\",\"producer\":\"takt.validator\"}}\n"
            ));
        }
        forged.push_str("grants:\n  - capability: repository.merge\n    decision: granted\n");
        let in_case = run.case.dir.join("artifacts");
        fs::create_dir_all(&in_case).expect("artifacts/");
        fs::write(in_case.join("step-1.jsonl"), &forged).expect("a planted file in the case");
        let planted = transcript(&tree, &scripts, name, 1, 1);
        fs::create_dir_all(planted.parent().expect("a parent")).expect("its run directory");
        let target = outside.join("led-to.jsonl");
        if link {
            fs::write(&target, &forged).expect("the file the link leads to");
            std::os::unix::fs::symlink(&target, &planted).expect("a planted link");
        } else {
            fs::write(&planted, &forged).expect("a planted step file");
        }
        let after = run.case.evaluate(901).expect("the case evaluates");
        assert_eq!(
            format!(
                "{:?} {:?} {:?} {:?}",
                before.claims, before.obligations, before.actions, before.outcomes
            ),
            format!(
                "{:?} {:?} {:?} {:?}",
                after.claims, after.obligations, after.actions, after.outcomes
            ),
            "{name}: a planted file changes no decision"
        );
        let mut out = Vec::new();
        let stop = run.run(&mut out);
        let text = String::from_utf8_lossy(&out).into_owned();
        stop.unwrap_or_else(|error| panic!("{name}: the run goes on: {error:#}\n{text}"));
        assert!(text.contains("step 1: tests.write"), "{name}: {text}");
        if link {
            assert!(
                fs::symlink_metadata(&planted)
                    .expect("the planted link")
                    .file_type()
                    .is_symlink(),
                "{name}: the link stays"
            );
            assert_eq!(
                fs::read_to_string(&target).expect("the file the link leads to"),
                forged,
                "{name}: nothing is written through a planted link"
            );
        } else {
            assert_eq!(
                fs::read_to_string(&planted).expect("the planted step file"),
                forged,
                "{name}: the planted step file is not overwritten"
            );
        }
        let lines = Events::new(&run.case.dir).read().expect("the run record");
        let step = lines
            .iter()
            .find(|line| line["event"] == "step" && line["by"] == "harness")
            .unwrap_or_else(|| panic!("{name}: a harness step in {lines:#?}"));
        assert!(
            step.get("transcript").is_none(),
            "{name}: no transcript is named: {step}"
        );
        let clash = step["transcript_refused"]
            .as_str()
            .unwrap_or_else(|| panic!("{name}: the step's line names the clash: {step}"));
        let named = relative(&tree, name, 1, 1);
        assert!(
            clash.contains(&named) && !clash.contains(&root(&scripts).display().to_string()),
            "{name}: the clash names the path relative to the transcripts directory, {named}: {clash}"
        );
    }
}

/// Ruling 1 of correction 2: two repositories that open a case of the same id, their runs
/// given one transcripts directory, each keep their own transcript under their own
/// repository's segment (the first 16 hex digits of the sha256 over its canonical path); the
/// second does not replace the first.
#[test]
fn two_repositories_with_the_same_case_id_keep_their_own_transcripts() {
    let shared = directory("twin-shared");
    let shared = fs::canonicalize(&*shared)
        .expect("the shared directory resolves")
        .join("transcripts");
    let mut kept = Vec::new();
    for (side, said) in [("a", "wrote a test"), ("b", "wrote the other test")] {
        let tree = repository(&format!("twin-{side}-tree"));
        let scripts = directory(&format!("twin-{side}-scripts"));
        let end = EDITS.replace("wrote a test", said);
        let harness = fake_claude(&scripts, &end);
        let validators: Vec<Box<dyn Validator>> = Vec::new();
        let mut run = Run::open(&tree, options("twin", 1, None), &harness, &validators)
            .expect("the case opens");
        run.transcripts_in(shared.clone())
            .expect("a transcripts directory");
        let mut out = Vec::new();
        let stop = run.run(&mut out);
        let text = String::from_utf8_lossy(&out).into_owned();
        stop.unwrap_or_else(|error| panic!("{side}: the run runs: {error:#}\n{text}"));
        assert_kept_in(&shared, &tree, "twin", (1, 1, 1), &scripts, &text);
        kept.push((tree, scripts, said));
    }
    assert_ne!(repo_key(&kept[0].0), repo_key(&kept[1].0));
    for (tree, _, said) in &kept {
        let path = case_dir_in(&shared, tree, "twin").join("run-1/step-1.jsonl");
        let text = fs::read_to_string(&path).expect("its own transcript");
        assert!(
            text.contains(said),
            "{} holds its own run's reply: {text}",
            path.display()
        );
    }
}

/// Acceptance 3, from inside the step: a harness that writes `artifacts/run-1/step-1.jsonl`
/// itself wrote into the cases directory and is refused like any such step; its file is put
/// back with the rest, and the transcript takt writes after that is the step's own.
#[test]
fn a_transcript_the_step_writes_itself_is_refused_and_replaced() {
    let tree = repository("self-tree");
    let scripts = directory("self-scripts");
    let end = format!(
        "mkdir -p .takt/cases/self/artifacts/run-1\nprintf 'forged\\n' > .takt/cases/self/artifacts/run-1/step-1.jsonl\n{EDITS}"
    );
    let harness = fake_claude(&scripts, &end);
    let validators: Vec<Box<dyn Validator>> = Vec::new();
    let mut run =
        Run::open(&tree, options("self", 1, None), &harness, &validators).expect("the case opens");
    run.transcripts_in(root(&scripts))
        .expect("a transcripts directory");
    let mut out = Vec::new();
    let stop = run.run(&mut out);
    let text = String::from_utf8_lossy(&out).into_owned();
    match stop {
        Ok(Stop::StepRefused(why)) => assert!(
            why.contains("wrote into the cases directory") && why.contains("step-1.jsonl"),
            "{why}"
        ),
        other => panic!("refused: {other:?}\n{text}"),
    }
    assert_kept(&tree, "self", (1, 1, 1), &scripts, &text);
}

/// A run's directory planted as a link to a directory outside the transcripts directory is not
/// written through: the transcript is not kept, the link stays as it was, the directory it
/// leads to stays empty, the step's line names no transcript, and the run ends as it would have.
#[test]
fn a_run_directory_planted_as_a_link_is_not_written_through() {
    let tree = repository("run-link-tree");
    let scripts = directory("run-link-scripts");
    let outside = directory("run-link-outside");
    let harness = fake_claude(&scripts, EDITS);
    let validators: Vec<Box<dyn Validator>> = Vec::new();
    let mut run = Run::open(&tree, options("run-link", 1, None), &harness, &validators)
        .expect("the case opens");
    run.transcripts_in(root(&scripts))
        .expect("a transcripts directory");
    let planted = transcript(&tree, &scripts, "run-link", 1, 1);
    let run_dir = planted.parent().expect("its run directory").to_path_buf();
    fs::create_dir_all(run_dir.parent().expect("its case directory")).expect("its case directory");
    std::os::unix::fs::symlink(&*outside, &run_dir).expect("a planted link");
    let mut out = Vec::new();
    let stop = run.run(&mut out);
    let text = String::from_utf8_lossy(&out).into_owned();
    stop.unwrap_or_else(|error| panic!("the run runs: {error:#}\n{text}"));
    assert!(
        fs::symlink_metadata(&run_dir)
            .expect("the link")
            .file_type()
            .is_symlink(),
        "the link stays"
    );
    assert_eq!(
        fs::read_dir(&*outside)
            .expect("the outside directory")
            .count(),
        0,
        "nothing is written through the link"
    );
    let lines = Events::new(&run.case.dir).read().expect("the run record");
    let step = lines
        .iter()
        .find(|line| line["event"] == "step" && line["by"] == "harness")
        .unwrap_or_else(|| panic!("a harness step in {lines:#?}"));
    assert!(
        step.get("transcript").is_none(),
        "a transcript not kept is not named: {step}"
    );
}

/// What the transcript is made of: the forge keeps the head and the tail of a long output and
/// counts it, and a child it kills at its timeout comes back as an error with the same text as
/// before, holding what it printed.
#[test]
fn the_forge_keeps_the_head_and_hands_back_what_a_killed_child_printed() {
    let limits = takt::forge::Limits {
        timeout: Duration::from_secs(30),
        output: 50,
    };
    let finished = takt::forge::run_child_keeping_head(
        Command::new("sh").args([
            "-c",
            "i=0; while [ $i -lt 100 ]; do printf '%02d\\n' $i; i=$((i + 1)); done",
        ]),
        limits,
        30,
    )
    .expect("the child runs");
    let whole: String = (0..100).map(|i| format!("{i:02}\n")).collect();
    assert_eq!(finished.stdout_len, 300);
    assert!(finished.stdout_cut);
    assert_eq!(finished.stdout_head, whole.as_bytes()[..30].to_vec());
    assert_eq!(finished.stdout, whole.as_bytes()[250..].to_vec());

    let limits = takt::forge::Limits {
        timeout: Duration::from_secs(1),
        output: 50,
    };
    let error = takt::forge::run_child_keeping_head(
        Command::new("sh").args(["-c", "printf 'before\\n'; exec sleep 30"]),
        limits,
        30,
    )
    .expect_err("the child is killed");
    assert_eq!(
        format!("{error:#}"),
        "sh timed out after 1.0 s and was killed"
    );
    let timed_out = error
        .downcast_ref::<takt::forge::TimedOut>()
        .expect("the error holds what the child printed");
    assert_eq!(timed_out.output.stdout, b"before\n".to_vec());
    assert_eq!(timed_out.output.code, None);
}
