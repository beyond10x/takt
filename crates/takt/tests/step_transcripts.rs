//! story:step-transcripts-are-kept: each harness step's stream, the prompt takt gave it as the
//! first line and then every line the harness printed (tool calls, replies, usage), is kept as
//! `.engineering/cases/<id>/artifacts/run-<r>/step-<n>.jsonl`, readable by its owner only, `r`
//! the run's number in the case, so a resumed case keeps every run's; a step that
//! fails, times out or runs past the hour budget keeps what it printed too; and a file planted
//! at that path decides nothing and is replaced. No live `claude` runs: the harness program is
//! a script.

use std::fs;
use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

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

/// The transcript of step `n` of the case's run `run` (1 for the run that opened it) in the
/// case `id` of `tree`.
fn transcript(tree: &Path, id: &str, run: usize, n: usize) -> PathBuf {
    tree.join(".engineering/cases")
        .join(id)
        .join("artifacts")
        .join(format!("run-{run}"))
        .join(format!("step-{n}.jsonl"))
}

/// Asserts that step `n`'s transcript of run `run` is a plain file only its owner reads, in a
/// directory only its owner reads, opening with a JSON line whose `prompt` is the prompt takt
/// gave the harness for step `n` (the one the fake received after `-p` at its call `call`),
/// followed by exactly the bytes the fake printed for call `call`.
fn assert_kept(
    tree: &Path,
    id: &str,
    (run, n, call): (usize, usize, usize),
    scripts: &Path,
    text: &str,
) {
    let path = transcript(tree, id, run, n);
    let meta = fs::symlink_metadata(&path).unwrap_or_else(|error| {
        panic!(
            "run {run} step {n}'s transcript {}: {error}\n{text}",
            path.display()
        )
    });
    let dir =
        fs::symlink_metadata(path.parent().expect("its run directory")).expect("the run directory");
    assert!(dir.is_dir(), "a plain directory: {dir:?}");
    assert_eq!(
        dir.permissions().mode() & 0o777,
        0o700,
        "{}",
        path.display()
    );
    assert!(meta.is_file(), "a plain file: {meta:?}");
    assert_eq!(
        meta.permissions().mode() & 0o777,
        0o600,
        "only its owner reads a transcript: {}",
        path.display()
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
fn every_harness_step_keeps_the_stream_it_printed_under_the_case_artifacts() {
    let tree = repository("every-tree");
    let scripts = directory("every-scripts");
    let harness = fake_claude(&scripts, EDITS);
    let validators: Vec<Box<dyn Validator>> = Vec::new();
    let mut run =
        Run::open(&tree, options("kept", 2, None), &harness, &validators).expect("the case opens");
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
    assert!(!transcript(&tree, "kept", 1, 3).exists(), "no step 3 ran");
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
    let mut out = Vec::new();
    let stop = opened.run(&mut out);
    let text = String::from_utf8_lossy(&out).into_owned();
    stop.unwrap_or_else(|error| panic!("the opening run runs: {error:#}\n{text}"));
    assert_kept(&tree, "resumed", (1, 1, 1), &scripts, &text);
    let first = fs::read(transcript(&tree, "resumed", 1, 1)).expect("run 1's transcript");
    drop(opened);
    let mut resumed = Run::resume(
        &tree,
        options("resumed", 1, None),
        "resumed",
        &harness,
        &validators,
    )
    .expect("the case resumes");
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
        fs::read(transcript(&tree, "resumed", 1, 1)).expect("run 1's transcript, still there"),
        first,
        "the resumed run left the opening run's transcript as it was"
    );
    assert_ne!(
        first,
        fs::read(transcript(&tree, "resumed", 2, 1)).expect("run 2's transcript"),
        "each run's step 1 has its own transcript"
    );
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
    let mut out = Vec::new();
    let stop = run.run(&mut out);
    let text = String::from_utf8_lossy(&out).into_owned();
    let error = stop.expect_err("the harness failed");
    assert!(format!("{error:#}").contains("exited 3"), "{error:#}");
    assert_kept(&tree, "fails", (1, 1, 1), &scripts, &text);
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
    let mut out = Vec::new();
    let stop = run.run(&mut out);
    let text = String::from_utf8_lossy(&out).into_owned();
    assert!(matches!(stop, Ok(Stop::Hours(_))), "{stop:?}\n{text}");
    assert_kept(&tree, "hours", (1, 1, 1), &scripts, &text);
}

/// Acceptance 3: the transcript is on no validator's read path. A file planted at
/// `artifacts/run-1/step-1.jsonl` before the step, holding evidence records that would make
/// every claim pass and a grant, leaves the decision as it was, and the step's own transcript
/// replaces it; one planted as a link is replaced too, and the file it led to is not written.
#[test]
fn a_planted_transcript_decides_nothing_and_is_replaced() {
    for (name, link) in [("planted-file", false), ("planted-link", true)] {
        let tree = repository(&format!("{name}-tree"));
        let scripts = directory(&format!("{name}-scripts"));
        let outside = directory(&format!("{name}-outside"));
        let harness = fake_claude(&scripts, EDITS);
        let validators: Vec<Box<dyn Validator>> = Vec::new();
        let mut run = Run::open(&tree, options(name, 1, None), &harness, &validators)
            .expect("the case opens");
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
        let planted = transcript(&tree, name, 1, 1);
        fs::create_dir_all(planted.parent().expect("a parent")).expect("artifacts/");
        let target = outside.join("led-to.jsonl");
        if link {
            fs::write(&target, &forged).expect("the file the link leads to");
            std::os::unix::fs::symlink(&target, &planted).expect("a planted link");
        } else {
            fs::write(&planted, &forged).expect("a planted transcript");
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
            "{name}: a planted transcript changes no decision"
        );
        let mut out = Vec::new();
        let stop = run.run(&mut out);
        let text = String::from_utf8_lossy(&out).into_owned();
        stop.unwrap_or_else(|error| panic!("{name}: the run runs: {error:#}\n{text}"));
        assert_kept(&tree, name, (1, 1, 1), &scripts, &text);
        if link {
            assert_eq!(
                fs::read_to_string(&target).expect("the file the link led to"),
                forged,
                "{name}: nothing is written through a planted link"
            );
        }
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
        "mkdir -p .engineering/cases/self/artifacts/run-1\nprintf 'forged\\n' > .engineering/cases/self/artifacts/run-1/step-1.jsonl\n{EDITS}"
    );
    let harness = fake_claude(&scripts, &end);
    let validators: Vec<Box<dyn Validator>> = Vec::new();
    let mut run =
        Run::open(&tree, options("self", 1, None), &harness, &validators).expect("the case opens");
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

/// A run's directory planted as a link to a directory outside the case is not written
/// through: the transcript is not kept, the link stays as it was, the directory it leads to
/// stays empty, and the run ends as it would have.
#[test]
fn a_run_directory_planted_as_a_link_is_not_written_through() {
    let tree = repository("run-link-tree");
    let scripts = directory("run-link-scripts");
    let outside = directory("run-link-outside");
    let harness = fake_claude(&scripts, EDITS);
    let validators: Vec<Box<dyn Validator>> = Vec::new();
    let mut run = Run::open(&tree, options("run-link", 1, None), &harness, &validators)
        .expect("the case opens");
    let planted = transcript(&tree, "run-link", 1, 1);
    let run_dir = planted.parent().expect("its run directory").to_path_buf();
    fs::create_dir_all(run_dir.parent().expect("artifacts/")).expect("artifacts/");
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
