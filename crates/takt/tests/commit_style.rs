//! story:step-commits-follow-a-configurable-style: a step commit's message follows a configured
//! style, `semantic` by default, with the story in a trailer takt writes itself. The repository's
//! own instructions, read at the case's base, win over a configured style. Each session here is
//! metaharness's scripted Claude Code: no vendor process, no credential, no network.

#[path = "support/own_home.rs"]
mod own_home;

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
use takt::commit_style::{Settings, Style};
use takt::harness::ClaudeSession;
use takt::validator::{TestsRun, Validator};
use takt::{Run, RunOptions, Story, Unit};

const PROTOCOL: &str = "story.delivery/1";
const BROKEN: &str = "pub fn add(a: i32, b: i32) -> i32 {\n    a - b\n}\n";
const FIXED: &str = "pub fn add(a: i32, b: i32) -> i32 {\n    a + b\n}\n";
const TESTS: &str = "#[test]\nfn adds() {\n    assert_eq!(1 + 1, 2);\n}\n";
const TITLE: &str = "Make add add";

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

/// A repository whose base commit holds the story `story:add`, tagged `tags`, and an `AGENTS.md`
/// with `agents` when given.
fn repository(name: &str, tags: &[&str], agents: Option<&str>) -> (Scratch, PathBuf) {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("commit-style-{}-{name}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    let tree = dir.join("tree");
    fs::create_dir_all(tree.join("src")).expect("the tree");
    fs::create_dir_all(tree.join(".engineering/planning/story")).expect("the store");
    fs::write(tree.join("README.md"), "a story\n").expect("a file");
    fs::write(tree.join("src/lib.rs"), BROKEN).expect("a file");
    fs::write(
        tree.join(".engineering/planning/story/add.md"),
        format!(
            "---\nformat: aep.planning-md/3\nid: story:add\nkind: story\nstatus: active\ntitle: {TITLE}\ntags: [{}]\nrevision: 1\n---\n# Add\n",
            tags.join(", ")
        ),
    )
    .expect("the story");
    if let Some(agents) = agents {
        fs::write(tree.join("AGENTS.md"), agents).expect("a file");
    }
    git(&tree, &["init", "-q", "-b", "main"]);
    git(&tree, &["add", "-A"]);
    git(&tree, &["commit", "-q", "-m", "start"]);
    (Scratch(dir), tree)
}

/// What one scripted session writes and replies.
#[derive(Clone)]
struct Turn {
    action: &'static str,
    writes: Vec<(&'static str, &'static str)>,
    commit: Option<serde_json::Value>,
}

impl Turn {
    fn tests(commit: Option<serde_json::Value>) -> Self {
        Self {
            action: "tests.write",
            writes: vec![("tests/add.rs", TESTS)],
            commit,
        }
    }

    fn edit(commit: Option<serde_json::Value>) -> Self {
        Self {
            action: "implementation.edit",
            writes: vec![("src/lib.rs", FIXED)],
            commit,
        }
    }
}

/// A well-formed `semantic` reply.
fn semantic(subject: &str) -> Option<serde_json::Value> {
    Some(serde_json::json!({
        "subject": subject,
        "body": "Cover the sum of two numbers.\nThe base subtracts, so the test fails.\nThe next step fixes it."
    }))
}

/// A scripted Claude Code session that takes the next [`Turn`] of `turns`: it writes the turn's
/// files into its working directory as the model's file tools would, and replies with the JSON
/// line naming the action, its summary and the commit message the turn gives.
struct Session {
    turns: Rc<RefCell<VecDeque<Turn>>>,
    prompts: Rc<RefCell<Vec<String>>>,
}

impl ProcessRunner for Session {
    fn requires_executable(&self) -> bool {
        false
    }

    fn start(&mut self, plan: &LaunchPlanView) -> std::io::Result<Box<dyn HarnessProcess>> {
        let turn = self
            .turns
            .borrow_mut()
            .pop_front()
            .expect("a scripted turn for the step");
        for (path, text) in &turn.writes {
            let at = plan.cwd.join(path);
            fs::create_dir_all(at.parent().expect("a parent"))?;
            fs::write(at, text)?;
        }
        self.prompts.borrow_mut().extend(plan.args.iter().cloned());
        let mut line = serde_json::json!({"action": turn.action, "summary": "scripted"});
        if let Some(commit) = &turn.commit {
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

/// What a run left: the repository, the case's id, and the messages of the commits its steps
/// made, oldest first.
struct Left {
    _scratch: Scratch,
    tree: PathBuf,
    case: String,
    messages: Vec<String>,
    events: Vec<serde_json::Value>,
    output: String,
}

impl Left {
    /// The lines of the run record that are `kind` events.
    fn events_of(&self, kind: &str) -> Vec<&serde_json::Value> {
        self.events
            .iter()
            .filter(|line| line["event"] == kind)
            .collect()
    }

    /// The `commit` member of the run's `opened` line.
    fn opened_commit(&self) -> &serde_json::Value {
        &self.events_of("opened")[0]["commit"]
    }
}

/// `git interpret-trailers --parse` over `message`: the trailer lines git reads from it.
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

/// The message of a commit split into its subject, its body's lines and what follows the last
/// blank line: `(subject, body, trailer paragraph)`.
fn parts(message: &str) -> (String, Vec<String>, Vec<String>) {
    let paragraphs: Vec<&str> = message.trim_end().split("\n\n").collect();
    let subject = paragraphs[0].to_owned();
    let (body, trailer) = match paragraphs.len() {
        1 => (Vec::new(), Vec::new()),
        2 => (
            paragraphs[1].lines().map(str::to_owned).collect(),
            Vec::new(),
        ),
        _ => (
            paragraphs[1..paragraphs.len() - 1]
                .iter()
                .flat_map(|paragraph| paragraph.lines().map(str::to_owned))
                .collect(),
            paragraphs[paragraphs.len() - 1]
                .lines()
                .map(str::to_owned)
                .collect(),
        ),
    };
    (subject, body, trailer)
}

/// What one case needs.
struct Case<'a> {
    name: &'a str,
    tags: &'a [&'a str],
    agents: Option<&'a str>,
    story: bool,
    settings: Option<Settings>,
    turns: Vec<Turn>,
}

impl<'a> Case<'a> {
    fn of(name: &'a str, turns: Vec<Turn>) -> Self {
        Self {
            name,
            tags: &[],
            agents: None,
            story: true,
            settings: None,
            turns,
        }
    }

    fn run(self) -> Left {
        let (scratch, tree) = repository(self.name, self.tags, self.agents);
        let base = git(&tree, &["rev-parse", "HEAD"]).trim().to_owned();
        let turns = Rc::new(RefCell::new(VecDeque::from(self.turns.clone())));
        let prompts = Rc::new(RefCell::new(Vec::new()));
        let (shared, seen) = (Rc::clone(&turns), Rc::clone(&prompts));
        let harness = ClaudeSession {
            credentials: CredentialSource::None,
            ..ClaudeSession::default()
        }
        .with_runner(move || {
            Box::new(Session {
                turns: Rc::clone(&shared),
                prompts: Rc::clone(&seen),
            })
        });
        let validators: Vec<Box<dyn Validator>> =
            vec![Box::new(TestsRun::new("true").expect("a command"))];
        let id = format!("commit-style-{}", self.name);
        let options = RunOptions {
            protocol: PROTOCOL.to_owned(),
            intent: TITLE.to_owned(),
            inputs: Vec::new(),
            iterations: Some(self.turns.len() as u64 * 2),
            hours: None,
            tokens: None,
            case_id: Some(id.clone()),
            gate: None,
        };
        let mut run = if self.story {
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
        if let Some(settings) = self.settings {
            run.commit_settings(settings);
        }
        let mut out = Vec::new();
        let stop = run.run(&mut out);
        let output = String::from_utf8_lossy(&out).into_owned();
        assert!(stop.is_ok(), "the run ends: {stop:?}\n{output}");
        let listed = git(&tree, &["rev-list", "--reverse", &format!("{base}..HEAD")]);
        let messages = listed
            .lines()
            .map(|id| git(&tree, &["show", "-s", "--format=%B", id]))
            .collect();
        let record = tree
            .join(takt::case::CASES_DIR)
            .join(&id)
            .join("events.jsonl");
        let events = fs::read_to_string(&record)
            .unwrap_or_else(|error| panic!("read {}: {error}\n{output}", record.display()))
            .lines()
            .map(|line| serde_json::from_str(line).expect("a line"))
            .collect();
        drop(run);
        Left {
            _scratch: scratch,
            tree,
            case: id,
            messages,
            events,
            output,
        }
    }
}

const REPOSITORY_RULE: &str = "# Agents\n\nBe brief.\nWrite every commit message as a short imperative sentence, as the commit message convention of this repository says.\n";

/// Acceptance 1: by default the subjects are `test: ...` and `feat: ...`, the bodies 3 or 4
/// lines, and git reads exactly `Refs: story:<id>` as the trailer of every step commit.
#[test]
fn semantic_by_default() {
    let left = Case::of(
        "semantic",
        vec![
            Turn::tests(semantic("test: cover the sum of two numbers")),
            Turn::edit(semantic("feat: add the numbers instead of subtracting")),
        ],
    )
    .run();
    assert_eq!(left.messages.len(), 2, "two step commits\n{}", left.output);
    let subjects: Vec<String> = left
        .messages
        .iter()
        .map(|message| parts(message).0)
        .collect();
    assert_eq!(
        subjects,
        [
            "test: cover the sum of two numbers",
            "feat: add the numbers instead of subtracting"
        ],
        "{}",
        left.output
    );
    for message in &left.messages {
        let (_, body, _) = parts(message);
        assert!(
            (3..=4).contains(&body.len()),
            "a body of 3 or 4 lines: {message}"
        );
        assert_eq!(
            trailers(&left.tree, message),
            ["Refs: story:add"],
            "exactly takt's trailer: {message}"
        );
    }
    assert!(
        left.events_of("commit.fallback").is_empty(),
        "no fallback\n{}",
        left.output
    );
    assert_eq!(left.opened_commit()["style"], "semantic", "{}", left.output);
    assert_eq!(left.opened_commit()["trailer"], "Refs", "{}", left.output);
}

/// Acceptance 2: a base `AGENTS.md` with a commit rule makes the effective style `repository`,
/// and `opened` records the file and the line that matched; the harness's own message is kept.
#[test]
fn the_repository_instructions_win() {
    let mut case = Case::of(
        "repository",
        vec![Turn::edit(Some(serde_json::json!({
            "subject": "Add the numbers instead of subtracting them",
            "body": "The convention here is a sentence."
        })))],
    );
    case.agents = Some(REPOSITORY_RULE);
    let left = case.run();
    let commit = left.opened_commit();
    assert_eq!(commit["style"], "repository", "{commit}\n{}", left.output);
    assert_eq!(commit["rule"]["file"], "AGENTS.md", "{commit}");
    assert_eq!(commit["rule"]["line"], 4, "{commit}");
    let (subject, _, _) = parts(&left.messages[0]);
    assert_eq!(subject, "Add the numbers instead of subtracting them");
    assert_eq!(trailers(&left.tree, &left.messages[0]), ["Refs: story:add"]);
}

/// Acceptance 3: a reply whose body holds `Refs: story:other` or a line starting `takt:` is not
/// used; the message is the fallback and the commit has only takt's trailer.
#[test]
fn a_reply_cannot_forge_a_trailer_or_a_takt_line() {
    for (name, forged) in [
        ("trailer", "Refs: story:other"),
        ("takt-line", "takt: take story:other"),
    ] {
        let body = format!("Fix the sum.\n{forged}\nThird line.");
        let left = Case::of(
            &format!("forge-{name}"),
            vec![Turn::edit(Some(serde_json::json!({
                "subject": "feat: add the numbers",
                "body": body
            })))],
        )
        .run();
        let message = &left.messages[0];
        assert_eq!(
            trailers(&left.tree, message),
            ["Refs: story:add"],
            "{name}: only takt's trailer: {message}"
        );
        assert!(
            !message.contains("story:other"),
            "{name}: nothing of the reply is kept: {message}"
        );
        assert!(
            message.lines().all(|line| !line.starts_with("takt:")),
            "{name}: no line of takt's: {message}"
        );
        assert_eq!(parts(message).0, format!("feat: {TITLE}"), "{name}");
        assert_eq!(
            left.events_of("commit.fallback").len(),
            1,
            "{name}\n{}",
            left.output
        );
    }
}

/// Acceptance 4: a subject over the bound gives the fallback message, `<type>: <story title>`
/// with the summary as the body, and one `commit.fallback` event.
#[test]
fn a_bad_reply_falls_back_and_says_so() {
    let long = format!("test: {}", "x".repeat(80));
    let left = Case::of(
        "fallback",
        vec![Turn::tests(Some(serde_json::json!({
            "subject": long,
            "body": "One.\nTwo.\nThree."
        })))],
    )
    .run();
    let message = &left.messages[0];
    let (subject, body, trailer) = parts(message);
    assert_eq!(subject, format!("test: {TITLE}"), "{message}");
    assert_eq!(body, ["scripted"], "{message}");
    assert_eq!(trailer, ["Refs: story:add"], "{message}");
    let fallbacks = left.events_of("commit.fallback");
    assert_eq!(fallbacks.len(), 1, "{}", left.output);
    assert!(
        !fallbacks[0]["reason"]
            .as_str()
            .unwrap_or_default()
            .is_empty(),
        "the reason is said: {}",
        fallbacks[0]
    );
    let lines: Vec<&str> = left
        .events
        .iter()
        .map(|line| line["event"].as_str().unwrap_or_default())
        .collect();
    let at = lines
        .iter()
        .position(|kind| *kind == "commit.fallback")
        .expect("the event");
    assert_eq!(
        lines[at + 1],
        "effect",
        "written before the effect: {lines:?}"
    );
    let mut missing = Case::of("no-commit", vec![Turn::tests(None)]).run();
    assert_eq!(
        parts(&missing.messages.remove(0)).0,
        format!("test: {TITLE}")
    );
    assert_eq!(missing.events_of("commit.fallback").len(), 1);
}

/// Acceptance 5: a case opened on an intent has no trailer.
#[test]
fn an_intent_case_has_no_trailer() {
    let mut case = Case::of(
        "intent",
        vec![Turn::edit(semantic(
            "feat: add the numbers instead of subtracting",
        ))],
    );
    case.story = false;
    let left = case.run();
    let message = &left.messages[0];
    assert!(
        trailers(&left.tree, message).is_empty(),
        "no trailer: {message}"
    );
    assert!(!message.contains("Refs"), "{message}");
    assert_eq!(
        parts(message).0,
        "feat: add the numbers instead of subtracting"
    );
    assert!(left.case.starts_with("commit-style"));
}

/// Acceptance 6: `style: action` gives `<action>: <summary>` plus the trailer.
#[test]
fn action_keeps_todays_subject() {
    let mut case = Case::of(
        "action",
        vec![Turn::tests(semantic("test: ignored")), Turn::edit(None)],
    );
    case.settings = Some(Settings {
        style: Style::Action,
        ..Settings::default()
    });
    let left = case.run();
    let subjects: Vec<String> = left
        .messages
        .iter()
        .map(|message| parts(message).0)
        .collect();
    assert_eq!(
        subjects,
        ["tests.write: scripted", "implementation.edit: scripted"]
    );
    for message in &left.messages {
        assert_eq!(
            trailers(&left.tree, message),
            ["Refs: story:add"],
            "{message}"
        );
    }
    assert!(
        left.events_of("commit.fallback").is_empty(),
        "{}",
        left.output
    );
    assert_eq!(left.opened_commit()["style"], "action");
}

/// Acceptance 7: `style: semantic` with a base `AGENTS.md` commit rule is effective style
/// `repository` on `opened`, the configured one kept beside it.
#[test]
fn the_repository_wins_over_an_explicit_style() {
    let mut case = Case::of(
        "explicit",
        vec![Turn::edit(Some(serde_json::json!({
            "subject": "Add the numbers instead of subtracting them",
            "body": "A sentence."
        })))],
    );
    case.agents = Some(REPOSITORY_RULE);
    case.settings = Some(Settings {
        style: Style::Semantic,
        ..Settings::default()
    });
    let left = case.run();
    let commit = left.opened_commit();
    assert_eq!(commit["style"], "repository", "{commit}");
    assert_eq!(commit["configured"], "semantic", "{commit}");
    assert_eq!(commit["rule"]["line"], 4, "{commit}");
}

/// Acceptance 8: `trailer: Story` with `types: {implementation.edit: build}` gives `Story:
/// story:<id>` and a fallback subject starting `build: `; an empty trailer gives none.
#[test]
fn a_configured_trailer_and_types_are_used() {
    let mut case = Case::of("trailer", vec![Turn::edit(None)]);
    case.settings = Some(Settings {
        trailer: "Story".to_owned(),
        types: [("implementation.edit".to_owned(), "build".to_owned())].into(),
        ..Settings::default()
    });
    let left = case.run();
    let message = &left.messages[0];
    assert!(parts(message).0.starts_with("build: "), "{message}");
    assert_eq!(
        trailers(&left.tree, message),
        ["Story: story:add"],
        "{message}"
    );
    assert_eq!(left.opened_commit()["trailer"], "Story");

    let mut case = Case::of("no-trailer", vec![Turn::edit(None)]);
    case.settings = Some(Settings {
        trailer: String::new(),
        ..Settings::default()
    });
    let left = case.run();
    let message = &left.messages[0];
    assert!(trailers(&left.tree, message).is_empty(), "{message}");
    assert!(!message.contains("story:add"), "{message}");
}

/// Acceptance 9: a story tagged `bug` gives the fallback subject `fix: <story title>` for its
/// implementation commit.
#[test]
fn a_bug_story_is_a_fix() {
    let mut case = Case::of("bug", vec![Turn::edit(None)]);
    case.tags = &["bug"];
    let left = case.run();
    assert_eq!(parts(&left.messages[0]).0, format!("fix: {TITLE}"));
    assert_eq!(left.opened_commit()["types"]["implementation.edit"], "fix");
}
