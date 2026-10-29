//! story:console-shows-reads-and-diffs: after each harness step the console lists the step's
//! tool calls under its `said:` line and the diff of its commit before its `effect:` line, each
//! escaped and bounded, in colour when the console is ([`takt::console::Console::color`]); the
//! records a run writes carry no colour. A fake harness edits one file and reports a Codex or a
//! Claude transcript holding two shell commands.

#[path = "support/own_home.rs"]
mod own_home;

use std::cell::Cell;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use takt::console::Console;
use takt::harness::{Harness, StepContext, StepReport};
use takt::validator::{ChangeRead, TestsRun, Validator};
use takt::{Run, RunOptions};

const PROTOCOL: &str = "story.delivery/1";

fn git(tree: &Path, args: &[&str]) {
    let output = Command::new("git")
        .args(["-c", "user.name=t", "-c", "user.email=t@localhost"])
        .args(args)
        .current_dir(tree)
        .output()
        .expect("git runs");
    assert!(
        output.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

struct Scratch(PathBuf);

impl std::ops::Deref for Scratch {
    type Target = Path;

    fn deref(&self) -> &Path {
        &self.0
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// A committed scratch repository holding `src/lib.rs`.
fn scratch(name: &str) -> Scratch {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("console-reads-{}-{name}", std::process::id()));
    if dir.exists() {
        fs::remove_dir_all(&dir).expect("a stale scratch tree is removed");
    }
    fs::create_dir_all(dir.join("src")).expect("scratch dir");
    fs::write(dir.join("src/lib.rs"), "pub fn add() {}\n").expect("a file");
    git(&dir, &["init", "-q", "-b", "main"]);
    git(&dir, &["add", "-A"]);
    git(&dir, &["commit", "-q", "-m", "start"]);
    Scratch(dir)
}

/// A transcript as takt keeps it: its first line, then `lines`.
fn transcript(lines: &[serde_json::Value]) -> Vec<u8> {
    let mut bytes = b"{\"context\":[],\"prompt\":\"p\",\"step\":1,\"type\":\"takt\"}\n".to_vec();
    for line in lines {
        bytes.extend_from_slice(line.to_string().as_bytes());
        bytes.push(b'\n');
    }
    bytes
}

/// A Codex step that ran `cat src/lib.rs` and `git status --short`.
fn codex() -> Vec<u8> {
    let call = |command: &str| {
        serde_json::json!({"type": "response_item", "payload": {
            "type": "custom_tool_call",
            "name": "exec",
            "input": format!("text(await tools.exec_command({{cmd:\"{command}\"}}));\n"),
        }})
    };
    transcript(&[
        serde_json::json!({"type": "session_meta", "payload": {"cwd": "/w/copy"}}),
        call("cat src/lib.rs"),
        call("git status --short"),
    ])
}

/// A Claude step that ran `cat src/lib.rs` and `git status --short`.
fn claude() -> Vec<u8> {
    let call = |command: &str| {
        serde_json::json!({"type": "assistant", "message": {"content": [
            {"type": "tool_use", "id": "t", "name": "Bash", "input": {"command": command}},
        ]}})
    };
    transcript(&[
        serde_json::json!({"type": "system", "subtype": "init", "cwd": "/w/copy"}),
        call("cat src/lib.rs"),
        call("git status --short"),
    ])
}

/// On its first call adds `pub fn sub() {}` to `src/lib.rs` under the first of
/// `implementation.edit` and `tests.write` it is offered; on every later call names the same
/// action and changes nothing. Reports `transcript` as each step's.
struct Editing {
    transcript: Vec<u8>,
    calls: Cell<usize>,
}

impl Harness for Editing {
    fn name(&self) -> &str {
        "editing"
    }

    fn step(&self, context: &StepContext<'_>) -> anyhow::Result<StepReport> {
        self.calls.set(self.calls.get() + 1);
        let action = ["implementation.edit", "tests.write"]
            .into_iter()
            .find(|action| context.offered.iter().any(|offered| offered == action))
            .unwrap_or("implementation.edit");
        if self.calls.get() == 1 {
            fs::write(
                context.tree.join("src/lib.rs"),
                "pub fn add() {}\npub fn sub() {}\n",
            )?;
        }
        Ok(StepReport {
            action: action.to_owned(),
            summary: "added sub".to_owned(),
            tokens: Some(10),
        })
    }

    fn last_transcript(&self) -> Option<Vec<u8>> {
        Some(self.transcript.clone())
    }
}

/// What a run of `transcript`'s harness printed under `console`, and its `events.jsonl`.
fn ran(name: &str, transcript: Vec<u8>, console: Console) -> (String, String) {
    let tree = scratch(name);
    let harness = Editing {
        transcript,
        calls: Cell::new(0),
    };
    let validators: Vec<Box<dyn Validator>> = vec![
        Box::new(TestsRun::new("true").expect("a command")),
        Box::new(ChangeRead),
    ];
    let options = RunOptions {
        protocol: PROTOCOL.to_owned(),
        intent: "add sub".to_owned(),
        inputs: Vec::new(),
        iterations: Some(4),
        hours: None,
        tokens: None,
        case_id: Some(name.to_owned()),
        gate: Some("repository.merge".to_owned()),
    };
    let mut run = Run::open(&tree, options, &harness, &validators).expect("the case opens");
    run.console(console);
    let mut out = Vec::new();
    run.run(&mut out).expect("the run stops without an error");
    let events = fs::read_to_string(tree.join(".takt/cases").join(name).join("events.jsonl"))
        .expect("the record");
    (String::from_utf8(out).expect("utf-8"), events)
}

/// The lines of the first harness step that committed: from its `said:` line to its `effect:`
/// line, both included.
fn committed_step(out: &str) -> Vec<&str> {
    let lines: Vec<&str> = out.lines().collect();
    let effect = lines
        .iter()
        .position(|line| line.contains("effect:") && line.contains("committed 1 path(s)"))
        .unwrap_or_else(|| panic!("no step committed:\n{out}"));
    let said = lines[..effect]
        .iter()
        .rposition(|line| line.contains("said:"))
        .unwrap_or_else(|| panic!("no said: line before the effect:\n{out}"));
    lines[said..=effect].to_vec()
}

/// Acceptance 1, through a run: both commands and the file's diff stand between the step's
/// `said:` and `effect:` lines, the commands first; for a Codex and a Claude transcript.
#[test]
fn a_step_prints_its_two_commands_and_its_diff_between_said_and_effect() {
    for (name, bytes) in [("codex", codex()), ("claude", claude())] {
        let (out, _) = ran(name, bytes, Console::default());
        let step = committed_step(&out);
        assert!(
            step.len() > 3,
            "{name}: nothing between said: and effect:\n{out}"
        );
        assert_eq!(
            &step[1..3],
            &["  $ cat src/lib.rs", "  $ git status --short"],
            "{name}:\n{out}"
        );
        let diff = &step[3..step.len() - 1];
        assert!(
            diff.contains(&"    diff --git a/src/lib.rs b/src/lib.rs"),
            "{name}:\n{out}"
        );
        assert!(diff.contains(&"    +pub fn sub() {}"), "{name}:\n{out}");
        assert!(
            !diff.iter().any(|line| line.contains(".takt/")),
            "the case's own files are in the diff:\n{out}"
        );
    }
}

/// Acceptance 3 and the Outcome's last paragraph: in colour the said label is dim and the added
/// line green, and no record holds a sequence; with both listings off neither is printed.
#[test]
fn colour_reaches_the_console_only_and_quiet_steps_lists_nothing() {
    let (out, events) = ran(
        "colour",
        codex(),
        Console {
            color: true,
            ..Console::default()
        },
    );
    let step = committed_step(&out);
    assert!(
        step[0].starts_with("  \u{1b}[2msaid:\u{1b}[0m "),
        "{:?}",
        step[0]
    );
    assert!(
        step.contains(&"    \u{1b}[32m+pub fn sub() {}\u{1b}[0m"),
        "{step:?}"
    );
    assert!(!events.contains('\u{1b}'), "{events}");
    assert!(!events.contains("\\u001b"), "{events}");
    let (quiet, _) = ran(
        "quiet",
        codex(),
        Console {
            reads: false,
            diff: false,
            ..Console::default()
        },
    );
    let step = committed_step(&quiet);
    assert_eq!(step.len(), 2, "{quiet}");
    assert!(!quiet.contains('\u{1b}'), "{quiet}");
}
