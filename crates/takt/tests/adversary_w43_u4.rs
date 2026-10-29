//! Adversarial cases for story:codex-step-gets-each-file-once (wave 43, unit U4): what a step
//! is no longer given once the change so far shows a file "whole", and what a Codex step is no
//! longer given beside an `AGENTS.md` Codex loads itself. Metaharness's scripted session stands
//! in for each vendor program, as in `step_context.rs`.

#[path = "support/own_home.rs"]
mod own_home;

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use loom_sdk::session::metaharness::{ScriptedLog, ScriptedRunner};
use loom_sdk::session::{CredentialSource, Kind};
use takt::case::Decision;
use takt::harness::{ChangeSoFar, Harness, SessionHarness};
use takt::validator::{TestsRun, Validator};
use takt::{LoadedProtocol, Run, RunOptions, StepContext, Stop};

const AGENTS_AT_BASE: &str = "# Agents\n\nThe base's rule: write release notes.\n";
const CLAUDE_OWN_RULE: &str = "Never edit the vendored directory.";

fn git(tree: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .args(["-c", "user.name=t", "-c", "user.email=t@localhost"])
        .args(["-c", "commit.gpgsign=false", "-c", "core.autocrlf=false"])
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

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn repository(name: &str, files: &[(&str, &[u8])]) -> (Scratch, String) {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("adv-w43-u4-{}-{name}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("scratch dir");
    git(&dir, &["init", "-q", "-b", "main"]);
    fs::write(dir.join("README.md"), "a story\n").expect("a file");
    for (path, bytes) in files {
        if let Some(parent) = dir.join(path).parent() {
            fs::create_dir_all(parent).expect("a directory");
        }
        fs::write(dir.join(path), bytes).expect("a file");
    }
    git(&dir, &["add", "-A"]);
    git(&dir, &["commit", "-q", "-m", "base"]);
    let base = git(&dir, &["rev-parse", "HEAD"]);
    (Scratch(dir), base)
}

fn harness(kind: Kind, log: &ScriptedLog) -> SessionHarness {
    let mut harness = SessionHarness::of(kind);
    harness.credentials = CredentialSource::None;
    let kept = log.clone();
    harness
        .with_runner(move || Box::new(ScriptedRunner::of_lines(Vec::<String>::new(), kept.clone())))
}

fn step_after(
    harness: &SessionHarness,
    tree: &Path,
    base: Option<&str>,
    intent: &str,
    change: &ChangeSoFar,
) -> anyhow::Result<takt::StepReport> {
    let protocol = LoadedProtocol::builtin("story.delivery/1").expect("a built-in");
    let decision = Decision {
        claims: std::collections::BTreeMap::new(),
        obligations: std::collections::BTreeMap::new(),
        actions: std::collections::BTreeMap::new(),
        outcomes: std::collections::BTreeMap::new(),
        json: String::new(),
    };
    let offered = ["tests.write".to_owned()];
    harness.step(&StepContext {
        tree,
        intent,
        offered: &offered,
        approvals: &[],
        choices: &[],
        granted: &[],
        decision: &decision,
        protocol: &protocol,
        step: 1,
        time_left: None,
        change,
        failed: &[],
        last_gate: None,
        scope_revision: None,
        base_revision: base,
        commit: None,
        obligations: &[],
    })
}

fn received(log: &ScriptedLog) -> String {
    let mut all: Vec<String> = log.launched().into_iter().flatten().collect();
    all.extend(log.written());
    all.join("\n")
}

/// The change so far of a case at `base` in `tree` whose accepted steps committed `paths`, read
/// as a run reads it (`step_context.rs`'s `change_of`).
fn change_of(tree: &Path, base: &str, paths: &[&str]) -> ChangeSoFar {
    let case = tree.join(".git/takt-case");
    fs::create_dir_all(&case).expect("a case dir");
    fs::write(
        case.join("branch.yaml"),
        format!("name: takt/case\nbase_commit: {base}\n"),
    )
    .expect("a branch record");
    let effect = serde_json::json!({"event": "effect", "revision": "r", "paths": paths});
    let mut out = Vec::new();
    takt::run::change_so_far(tree, &case, || Ok(vec![effect]), None, None, &mut out)
        .expect("a change so far")
}

/// The diff a change so far shows, with its variant's name.
fn shown_diff(change: &ChangeSoFar) -> String {
    match change {
        ChangeSoFar::ShownWhole { diff } => format!("ShownWhole\n{diff}"),
        ChangeSoFar::Shown { diff, whole } => format!("Shown whole={whole}\n{diff}"),
        ChangeSoFar::ShownWholeBut { diff, hunks } => format!("ShownWholeBut {hunks:?}\n{diff}"),
        other => format!("{other:?}"),
    }
}

/// A repository whose `CLAUDE.md` holds a rule its `AGENTS.md` does not: a Codex step whose
/// copy holds `AGENTS.md` at the base's bytes (so Codex loads that file itself) is still given
/// the rule, since Codex does not read `CLAUDE.md` and nothing else carries it. Today
/// `codex_instruction_files` drops every `CLAUDE.md` beside an `AGENTS.md`, whatever it holds.
#[test]
fn a_codex_step_gets_a_claude_md_rule_agents_md_does_not_hold() {
    let claude = format!("# Claude\n\n{CLAUDE_OWN_RULE}\n");
    let (tree, base) = repository(
        "claude-own-rule",
        &[
            ("AGENTS.md", AGENTS_AT_BASE.as_bytes()),
            ("CLAUDE.md", claude.as_bytes()),
        ],
    );
    let log = ScriptedLog::new();
    let harness = harness(Kind::Codex, &log);
    let _ = step_after(
        &harness,
        &tree.0,
        Some(&base),
        "Remove a todo",
        &ChangeSoFar::NoneYet,
    );
    let text = received(&log);
    assert!(
        text.contains(CLAUDE_OWN_RULE),
        "the Codex step was never given CLAUDE.md's own rule: {text}"
    );
}

/// The same with the instruction files configured by name: an operator who lists `CLAUDE.md`
/// as an instruction file still has a Codex step given it.
#[test]
fn a_configured_claude_md_reaches_a_codex_step() {
    let claude = format!("# Claude\n\n{CLAUDE_OWN_RULE}\n");
    let (tree, base) = repository(
        "claude-configured",
        &[
            ("AGENTS.md", AGENTS_AT_BASE.as_bytes()),
            ("CLAUDE.md", claude.as_bytes()),
        ],
    );
    let log = ScriptedLog::new();
    let mut harness = harness(Kind::Codex, &log);
    harness.context = vec!["AGENTS.md".to_owned(), "CLAUDE.md".to_owned()];
    let _ = step_after(
        &harness,
        &tree.0,
        Some(&base),
        "Remove a todo",
        &ChangeSoFar::NoneYet,
    );
    let text = received(&log);
    assert!(
        text.contains(CLAUDE_OWN_RULE),
        "the configured CLAUDE.md was never given to the Codex step: {text}"
    );
}

/// A binary test file in scope that an accepted step changed: the change so far's diff names it
/// (`diff --git a/<p> b/<p>`) but shows only "Binary files ... differ", so it is not shown
/// whole, and the Claude step is still given it as a mention, as it was before the unit.
#[test]
fn a_binary_test_file_shown_only_as_differing_is_still_mentioned() {
    let (tree, base) = repository(
        "binary-test",
        &[
            ("app.py", b"def add():\n    pass\n"),
            ("tests/golden.bin", b"\x00\x01golden\x00\x02\n"),
        ],
    );
    fs::write(
        tree.0.join("tests/golden.bin"),
        b"\x00\x01golden two\x00\x03\n",
    )
    .expect("a step's edit");
    git(&tree.0, &["commit", "-q", "-am", "an accepted step's edit"]);
    let change = change_of(&tree.0, &base, &["tests/golden.bin"]);
    let shown = shown_diff(&change);
    assert!(
        shown.contains("diff --git a/tests/golden.bin b/tests/golden.bin")
            && shown.contains("Binary files"),
        "the precondition: the change so far shows the file only as differing: {shown}"
    );
    let intent = "Update the golden file\n\n## Scope\n\n- `tests/golden.bin`\n";
    let mentioned = takt::harness::scope_files(&tree.0, None, intent, &change, None);
    assert_eq!(
        mentioned,
        ["tests/golden.bin"],
        "a file whose bytes the prompt never shows is not mentioned either: {shown}"
    );
}

/// A test script an accepted step only made executable: the diff holds its header and its
/// mode lines and none of its text, and a Codex step is still given the file as a context file.
#[test]
fn a_test_file_changed_only_in_mode_is_still_given_to_codex() {
    let script = "#!/bin/sh\n# The suite's driver: run every case.\nexec python3 -m unittest\n";
    let (tree, base) = repository(
        "mode-only",
        &[
            ("app.py", b"def add():\n    pass\n"),
            ("tests/run.sh", script.as_bytes()),
        ],
    );
    git(&tree.0, &["update-index", "--chmod=+x", "tests/run.sh"]);
    git(
        &tree.0,
        &["commit", "-q", "-m", "an accepted step's mode change"],
    );
    let change = change_of(&tree.0, &base, &["tests/run.sh"]);
    let shown = shown_diff(&change);
    assert!(
        shown.contains("diff --git a/tests/run.sh b/tests/run.sh")
            && !shown.contains("The suite's driver"),
        "the precondition: the change so far shows none of the file's text: {shown}"
    );
    let intent = "Run the suite\n\n## Scope\n\n- `tests/run.sh`\n";
    let given = takt::harness::context_files(&tree.0, intent, &change, None, &|_| String::new());
    let paths: Vec<&str> = given.iter().map(|file| file.path()).collect();
    assert_eq!(
        paths,
        ["tests/run.sh"],
        "a file whose text the prompt never shows is not given either: {shown}"
    );
}

/// A test file with CRLF line ends that an accepted step changed, in scope: the change so far
/// "shows it whole, as it is now", but with every carriage return replaced by U+FFFD, and the
/// Codex step is no longer given its bytes as a context file, so nothing it is given holds the
/// file as it is.
#[test]
fn a_crlf_test_file_shown_whole_reaches_codex_as_it_is() {
    let before = "import unittest\r\n\r\n\r\nclass Todo(unittest.TestCase):\r\n    def test_add(self):\r\n        pass\r\n";
    let after = format!("{before}\r\n    def test_remove(self):\r\n        pass\r\n");
    let (tree, base) = repository(
        "crlf",
        &[
            ("todo.py", b"def add():\n    pass\n"),
            ("tests/test_todo.py", before.as_bytes()),
        ],
    );
    fs::write(tree.0.join("tests/test_todo.py"), &after).expect("a step's edit");
    git(&tree.0, &["commit", "-q", "-am", "an accepted step's edit"]);
    let change = change_of(&tree.0, &base, &["tests/test_todo.py"]);
    assert!(
        matches!(change, ChangeSoFar::ShownWhole { .. }),
        "the precondition: the test file is shown whole: {change:?}"
    );
    let intent = "Remove a todo\n\n## Scope\n\n- `todo.py`\n- `tests/test_todo.py`\n";
    // Read where the prompt is built, not through the scripted log, whose lines lose their `\r`.
    let shown = shown_diff(&change);
    let in_diff = shown.contains("+    def test_remove(self):\r\n+        pass\r\n");
    let given = takt::harness::context_files(&tree.0, intent, &change, None, &|_| String::new());
    let as_context = given
        .iter()
        .any(|file| file.path() == "tests/test_todo.py" && file.text() == after);
    assert!(
        in_diff || as_context,
        "neither the change so far nor a context file holds the file as it is \
         (shown {:?}; given {:?})",
        shown,
        given.iter().map(|file| file.path()).collect::<Vec<_>>()
    );
}

/// A Codex program that writes `stray.txt` into its working directory, the session's copy, and
/// into `tree` itself, as a session reaching past its copy would, so that only the step's
/// discard takes it out of the tree; and a rollout under `CODEX_HOME` whose user input holds no
/// `# AGENTS.md instructions` block.
fn codex_not_loading(dir: &Path, tree: &Path) -> PathBuf {
    use std::os::unix::fs::PermissionsExt as _;
    let at = "2026-10-20T18:00:00.000Z";
    let id = "01a15a19-57e0-7d83-97b1-da333ebb91e5";
    let turn = "01a15a19-5963-7d42-b749-21695314f7a4";
    let reply = "Done.\n{\"action\": \"tests.write\", \"summary\": \"wrote a test\"}";
    let lines = [
        serde_json::json!({"timestamp": at, "type": "session_meta", "payload": {
            "session_id": id, "id": id, "timestamp": at, "cwd": "@CWD@",
            "originator": "codex_exec", "cli_version": "0.160.0", "source": "exec",
            "model_provider": "openai"}}),
        serde_json::json!({"timestamp": at, "type": "event_msg", "payload": {
            "type": "task_started", "turn_id": turn}}),
        serde_json::json!({"timestamp": at, "type": "response_item", "payload": {
            "type": "message", "role": "user", "content": [{"type": "input_text",
            "text": "<environment_context>\n</environment_context>"}]}}),
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
    ];
    fs::create_dir_all(dir).expect("the program's directory");
    let template = dir.join("rollout.jsonl");
    let text: String = lines.iter().map(|line| format!("{line}\n")).collect();
    fs::write(&template, text).expect("the rollout's template");
    let program = dir.join("codex");
    fs::write(
        &program,
        format!(
            "#!/bin/sh\n\
             [ \"$1\" = \"--version\" ] && {{ echo \"codex-cli 0.160.0\"; exit 0; }}\n\
             echo stray > stray.txt\n\
             echo stray > '{}/stray.txt'\n\
             d=\"$CODEX_HOME/sessions/2026/10/20\"\n\
             mkdir -p \"$d\"\n\
             sed \"s|@CWD@|$PWD|\" '{}' > \"$d/rollout-2026-10-20T18-00-00-{id}.jsonl\"\n",
            tree.display(),
            template.display()
        ),
    )
    .expect("a fake codex");
    fs::set_permissions(&program, fs::Permissions::from_mode(0o755)).expect("exec bit");
    program
}

/// Acceptance 3 and the unit's own claim in `run.rs`: a Codex step refused because its session
/// did not load `AGENTS.md` has its change discarded and its tokens charged. The story's test
/// asserts only the stop's name; this asserts the other two halves.
#[test]
fn a_refused_unloaded_step_is_charged_and_its_change_discarded() {
    let programs = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("adv-w43-u4-{}-programs", std::process::id()));
    let _programs = Scratch(programs.clone());
    let (tree, _) = repository("refused", &[("AGENTS.md", AGENTS_AT_BASE.as_bytes())]);
    let program = codex_not_loading(&programs.join("not-loading"), &tree.0);
    let mut session = SessionHarness::of(Kind::Codex);
    session.credentials = CredentialSource::None;
    session.program = program.display().to_string();
    session.timeout = std::time::Duration::from_secs(60);
    let validators: Vec<Box<dyn Validator>> =
        vec![Box::new(TestsRun::new("true").expect("a command"))];
    let options = RunOptions {
        protocol: "story.delivery/1".to_owned(),
        intent: "make add add".to_owned(),
        inputs: Vec::new(),
        iterations: Some(2),
        hours: None,
        tokens: None,
        case_id: Some("refused".to_owned()),
        gate: None,
    };
    let mut run = Run::open(&tree.0, options, &session, &validators).expect("opens");
    run.scan_with(takt::scan::Scan::None {
        declared_in: tree.0.with_file_name("run.toml"),
    });
    run.continue_past_red_base();
    let mut out = Vec::new();
    let stop = run.run(&mut out);
    let said = String::from_utf8_lossy(&out).into_owned();
    assert!(
        matches!(&stop, Ok(Stop::StepRefused(why)) if why.contains("did not load AGENTS.md")),
        "{stop:?}\n{said}"
    );
    assert!(
        !tree.0.join("stray.txt").exists(),
        "the refused session's change was kept\n{said}"
    );
    let record = fs::read_to_string(tree.0.join(".takt/cases/refused/events.jsonl"))
        .expect("the run record");
    assert!(
        record
            .lines()
            .any(|line| line.contains("\"by\":\"harness\"") && line.contains("\"tokens\":120")),
        "the refused session's 120 tokens are not charged: {record}"
    );
}

/// Round 2: a one-line `CLAUDE.md` that names `AGENTS.md` and adds a rule of its own is not a
/// pointer. `claude_md_adds_nothing` treats every line under 200 bytes that names `AGENTS.md`
/// as one, so the Codex step loses the rule.
#[test]
fn a_one_line_claude_md_naming_agents_md_with_a_rule_of_its_own_reaches_codex() {
    let claude = format!("Follow AGENTS.md. {CLAUDE_OWN_RULE}\n");
    assert!(claude.len() < 200);
    let (tree, base) = repository(
        "claude-pointer-plus-rule",
        &[
            ("AGENTS.md", AGENTS_AT_BASE.as_bytes()),
            ("CLAUDE.md", claude.as_bytes()),
        ],
    );
    let log = ScriptedLog::new();
    let harness = harness(Kind::Codex, &log);
    let _ = step_after(
        &harness,
        &tree.0,
        Some(&base),
        "Remove a todo",
        &ChangeSoFar::NoneYet,
    );
    let text = received(&log);
    assert!(
        text.contains(CLAUDE_OWN_RULE),
        "the Codex step was never given the rule beside the pointer: {text}"
    );
}

/// Round 2: `shown_whole` opens a section only on a line starting `diff --git a/`. A path git
/// quotes (`core.quotePath`, on in the isolated diff: any byte past ASCII, a quote, a control)
/// opens its section with `diff --git "a/...`, so its `@@` hunk is read as the previous
/// section's. A binary test file in scope followed by such a file is then counted as shown
/// whole, and the Claude step is not given it, though the prompt shows none of its bytes.
#[test]
fn a_quoted_path_after_a_binary_file_does_not_make_the_binary_shown_whole() {
    let (tree, base) = repository(
        "quoted-after-binary",
        &[
            ("app.py", b"def add():\n    pass\n"),
            ("tests/golden.bin", b"\x00\x01golden\x00\x02\n"),
            ("tests/\u{fc}ber.txt", b"one\n"),
        ],
    );
    fs::write(
        tree.0.join("tests/golden.bin"),
        b"\x00\x01golden two\x00\x03\n",
    )
    .expect("a step's edit");
    fs::write(tree.0.join("tests/\u{fc}ber.txt"), "one\ntwo\n").expect("a step's edit");
    git(&tree.0, &["commit", "-q", "-am", "an accepted step's edit"]);
    let intent = "Update the golden file\n\n## Scope\n\n- `tests/golden.bin`\n";
    let case = tree.0.join(".git/takt-case");
    fs::create_dir_all(&case).expect("a case dir");
    fs::write(
        case.join("branch.yaml"),
        format!("name: takt/case\nbase_commit: {base}\n"),
    )
    .expect("a branch record");
    let effect = serde_json::json!({"event": "effect", "revision": "r",
        "paths": ["tests/golden.bin", "tests/\u{fc}ber.txt"]});
    let mut out = Vec::new();
    let change = takt::run::change_so_far_for(
        &tree.0,
        &case,
        || Ok(vec![effect]),
        (None, Some(intent)),
        None,
        &mut out,
    )
    .expect("a change so far");
    let shown = shown_diff(&change);
    assert!(
        shown.contains("Binary files a/tests/golden.bin and b/tests/golden.bin differ")
            && shown.contains("diff --git \"a/tests/"),
        "the precondition: the binary shows only as differing and a quoted path follows: {shown}"
    );
    let mentioned = takt::harness::scope_files(&tree.0, None, intent, &change, None);
    assert_eq!(
        mentioned,
        ["tests/golden.bin"],
        "the binary is counted as shown whole on the next file's hunk: {shown}"
    );
}
