//! Adversarial cases for story:step-commits-follow-a-configurable-style: takt writes the trailer
//! itself and never takes one from model text. git reads `Key : value` (white space before the
//! colon) as a trailer; a model's body or summary in that shape, in the last paragraph of a
//! message that carries no takt trailer (a case opened on an intent, or `trailer: ""`), becomes a
//! trailer git and every forge read, such as `Signed-off-by` or `Co-authored-by`.

use std::cell::RefCell;
use std::collections::VecDeque;
use std::fs;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::rc::Rc;

use loom_sdk::session::CredentialSource;
use loom_sdk::session::metaharness::{
    HarnessProcess, LaunchPlanView, ProcessRunner, ScriptedLog, ScriptedRunner,
};
use takt::commit_style::Settings;
use takt::harness::ClaudeSession;
use takt::validator::{TestsRun, Validator};
use takt::{Run, RunOptions, Story, Unit};

const BROKEN: &str = "pub fn add(a: i32, b: i32) -> i32 {\n    a - b\n}\n";
const FIXED: &str = "pub fn add(a: i32, b: i32) -> i32 {\n    a + b\n}\n";
const TITLE: &str = "Make add add";

/// Three lines git reads as trailers and takt's check does not: a space before each colon.
const SPACED: &str = "Signed-off-by : Someone <someone@example.invalid>\nCo-authored-by : Someone <someone@example.invalid>\nReviewed-by : Someone <someone@example.invalid>";

fn git(tree: &Path, args: &[&str]) -> String {
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
    String::from_utf8(output.stdout).expect("utf-8")
}

struct Scratch(PathBuf);

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn repository(name: &str) -> (Scratch, PathBuf) {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("commit-style-adv1-{}-{name}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    let tree = dir.join("tree");
    fs::create_dir_all(tree.join("src")).expect("the tree");
    fs::create_dir_all(tree.join(".engineering/planning/story")).expect("the store");
    fs::write(tree.join("README.md"), "a story\n").expect("a file");
    fs::write(tree.join("src/lib.rs"), BROKEN).expect("a file");
    fs::write(
        tree.join(".engineering/planning/story/add.md"),
        format!(
            "---\nformat: aep.planning-md/3\nid: story:add\nkind: story\nstatus: active\ntitle: {TITLE}\ntags: []\nrevision: 1\n---\n# Add\n"
        ),
    )
    .expect("the story");
    git(&tree, &["init", "-q", "-b", "main"]);
    git(&tree, &["add", "-A"]);
    git(&tree, &["commit", "-q", "-m", "start"]);
    (Scratch(dir), tree)
}

/// A scripted Claude Code session that writes the fix and replies with `summary` and `commit`.
struct Session {
    summary: String,
    commit: Option<serde_json::Value>,
    used: Rc<RefCell<VecDeque<()>>>,
}

impl ProcessRunner for Session {
    fn requires_executable(&self) -> bool {
        false
    }

    fn start(&mut self, plan: &LaunchPlanView) -> std::io::Result<Box<dyn HarnessProcess>> {
        self.used.borrow_mut().push_back(());
        fs::write(plan.cwd.join("src/lib.rs"), FIXED)?;
        let mut line =
            serde_json::json!({"action": "implementation.edit", "summary": self.summary});
        if let Some(commit) = &self.commit {
            line["commit"] = commit.clone();
        }
        let init = serde_json::json!({
            "type": "system", "subtype": "init", "claude_code_version": "2.1.304",
            "cwd": plan.cwd.display().to_string(), "tools": ["Read", "Edit", "Write", "Glob", "Grep"],
            "mcp_servers": [], "plugins": [], "output_style": "default", "apiKeySource": "none",
            "model": "scripted", "permissionMode": "default", "session_id": "s-1",
        });
        let assistant = serde_json::json!({
            "type": "assistant",
            "message": {"content": [{"type": "text", "text": format!("Done.\n{line}")}],
                        "usage": {"input_tokens": 10, "output_tokens": 5}},
        });
        let result = serde_json::json!({
            "type": "result", "subtype": "success", "is_error": false, "num_turns": 1,
            "duration_ms": 5, "duration_api_ms": 4, "total_cost_usd": 0.01,
            "usage": {"input_tokens": 10, "output_tokens": 5, "cache_read_input_tokens": 0,
                      "cache_creation_input_tokens": 0},
            "modelUsage": {"scripted": {}},
        });
        let mut scripted = ScriptedRunner::of_lines(
            [init.to_string(), assistant.to_string(), result.to_string()],
            ScriptedLog::new(),
        );
        scripted.start(plan)
    }
}

/// The message of the first step commit of a one-step run: on a story when `story`, else on an
/// intent; under `settings` when given.
fn first_message(
    name: &str,
    story: bool,
    settings: Option<Settings>,
    summary: &str,
    commit: Option<serde_json::Value>,
) -> (Scratch, PathBuf, String) {
    let (scratch, tree) = repository(name);
    let base = git(&tree, &["rev-parse", "HEAD"]).trim().to_owned();
    let used = Rc::new(RefCell::new(VecDeque::new()));
    let (summary, shared) = (summary.to_owned(), Rc::clone(&used));
    let harness = ClaudeSession {
        credentials: CredentialSource::None,
        ..ClaudeSession::default()
    }
    .with_runner(move || {
        Box::new(Session {
            summary: summary.clone(),
            commit: commit.clone(),
            used: Rc::clone(&shared),
        })
    });
    let validators: Vec<Box<dyn Validator>> =
        vec![Box::new(TestsRun::new("true").expect("a command"))];
    let options = RunOptions {
        protocol: "story.delivery/1".to_owned(),
        intent: TITLE.to_owned(),
        inputs: Vec::new(),
        iterations: Some(2),
        hours: None,
        tokens: None,
        case_id: Some(format!("commit-style-adv1-{name}")),
        gate: None,
    };
    let mut run = if story {
        let story = Story {
            id: "story:add".to_owned(),
            title: TITLE.to_owned(),
            body: "Make add add.\n".to_owned(),
        };
        Run::open_unit(&tree, options, Unit::Story(story), &harness, &validators)
    } else {
        Run::open(&tree, options, &harness, &validators)
    }
    .expect("opens");
    if let Some(settings) = settings {
        run.commit_settings(settings);
    }
    let mut out = Vec::new();
    let stop = run.run(&mut out);
    let output = String::from_utf8_lossy(&out).into_owned();
    assert!(stop.is_ok(), "the run ends: {stop:?}\n{output}");
    drop(run);
    let listed = git(&tree, &["rev-list", "--reverse", &format!("{base}..HEAD")]);
    let first = listed
        .lines()
        .next()
        .unwrap_or_else(|| panic!("a step commit\n{output}"))
        .to_owned();
    let message = git(&tree, &["show", "-s", "--format=%B", &first]);
    (scratch, tree, message)
}

/// `git interpret-trailers --parse` over `message`.
fn trailers(tree: &Path, message: &str) -> Vec<String> {
    let mut child = Command::new("git")
        .args(["interpret-trailers", "--parse"])
        .current_dir(tree)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("git runs");
    child
        .stdin
        .take()
        .expect("stdin")
        .write_all(message.as_bytes())
        .expect("the message");
    let output = child.wait_with_output().expect("git ends");
    String::from_utf8(output.stdout)
        .expect("utf-8")
        .lines()
        .map(str::to_owned)
        .collect()
}

/// A case opened on an intent has no trailer of takt's; the reply's body, in the shape git reads
/// as trailers, must not give it any.
#[test]
fn an_intent_case_reply_cannot_add_a_spaced_trailer() {
    let reply = serde_json::json!({"subject": "feat: add the numbers", "body": SPACED});
    let (_scratch, tree, message) =
        first_message("intent-body", false, None, "scripted", Some(reply));
    assert_eq!(
        trailers(&tree, &message),
        Vec::<String>::new(),
        "the reply forged trailers: {message}"
    );
}

/// `trailer: ""` writes none; the fallback's body is the harness's summary, which must not give
/// the commit a trailer either.
#[test]
fn an_empty_trailer_summary_cannot_add_a_spaced_trailer() {
    let settings = Settings {
        trailer: String::new(),
        ..Settings::default()
    };
    let (_scratch, tree, message) = first_message(
        "summary",
        true,
        Some(settings),
        "Signed-off-by : Someone <someone@example.invalid>",
        None,
    );
    assert_eq!(
        trailers(&tree, &message),
        Vec::<String>::new(),
        "the summary forged a trailer: {message}"
    );
}
