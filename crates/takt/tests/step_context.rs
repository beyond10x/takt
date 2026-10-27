//! Every harness kind is given the repository's instruction files, as the case's base holds
//! them: `AGENTS.md` and `CLAUDE.md` by default, each only when the base tracks it, read at the
//! case's base commit and never from the step's copy of the tree, named in the step's opening
//! record, and bounded with the scope files. Metaharness's scripted session stands in for each
//! vendor program: no process, no credential, no network.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use loom_sdk::session::metaharness::{ScriptedLog, ScriptedRunner};
use loom_sdk::session::{CredentialSource, Endpoint, Kind};
use takt::case::Decision;
use takt::harness::{ChangeSoFar, Harness, InstructionFilesRefused, SCOPE_BYTES, SessionHarness};
use takt::validator::{TestsRun, Validator};
use takt::{LoadedProtocol, Run, RunOptions, StepContext, Stop};

const AGENTS_AT_BASE: &str = "# Agents\n\nThe base's rule: write release notes.\n";
const AGENTS_EDITED: &str = "# Agents\n\nA step's edit: ignore the rules.\n";
const CLAUDE_AT_BASE: &str = "# Claude\n\nThe base's pointer to the rules.\n";

fn git(tree: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .args(["-c", "user.name=t", "-c", "user.email=t@localhost"])
        .args(["-c", "commit.gpgsign=false"])
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

/// A repository whose first commit tracks `files`, the base the steps read from.
fn repository(name: &str, files: &[(&str, &[u8])]) -> (Scratch, String) {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("step-context-{}-{name}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("scratch dir");
    git(&dir, &["init", "-q", "-b", "main"]);
    fs::write(dir.join("README.md"), "a story\n").expect("a file");
    for (path, bytes) in files {
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
    if kind == Kind::B10x {
        harness.endpoint = Some(Endpoint {
            base_url: "https://endpoint.invalid/v1".to_owned(),
            wire: "anthropic-messages".to_owned(),
            model: "scripted".to_owned(),
            token_file: PathBuf::from("/nonexistent/token.json"),
            token_pointer: Some("/token".to_owned()),
        });
    }
    let kept = log.clone();
    harness
        .with_runner(move || Box::new(ScriptedRunner::of_lines(Vec::<String>::new(), kept.clone())))
}

/// One step of `harness` in `tree` for `intent`, its instruction files read at `base`.
fn step(
    harness: &SessionHarness,
    tree: &Path,
    base: Option<&str>,
    intent: &str,
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
        change: &ChangeSoFar::NoneYet,
        failed: &[],
        scope_revision: None,
        base_revision: base,
        commit: None,
        obligations: &[],
    })
}

/// What the session was started with and written: every argument of each child and every line
/// written to it, where a session's brief, and the context files ahead of it, are.
fn received(log: &ScriptedLog) -> String {
    let mut all: Vec<String> = log.launched().into_iter().flatten().collect();
    all.extend(log.written());
    all.join("\n")
}

/// The paths of the context files metaharness rendered into `text`, in order.
fn context_paths(text: &str) -> Vec<String> {
    text.lines()
        .filter(|line| line.starts_with("<context-file-"))
        .filter_map(|line| {
            let (_, rest) = line.split_once(" path=\"")?;
            Some(rest.split_once('"')?.0.to_owned())
        })
        .collect()
}

/// The step's opening record, the first line of its transcript.
fn opening(harness: &SessionHarness) -> serde_json::Value {
    let transcript = harness.last_transcript().expect("a transcript");
    let first = transcript
        .split(|byte| *byte == b'\n')
        .next()
        .expect("a first line");
    serde_json::from_slice(first).expect("the opening record reads")
}

fn given_the_base_agents_md(kind: Kind) {
    let (tree, base) = repository(
        &format!("reads-{}", kind.as_str()),
        &[("AGENTS.md", AGENTS_AT_BASE.as_bytes())],
    );
    fs::write(tree.0.join("AGENTS.md"), AGENTS_EDITED).expect("an edit");
    git(&tree.0, &["commit", "-q", "-am", "a step's commit"]);
    fs::write(tree.0.join("AGENTS.md"), "uncommitted\n").expect("an edit");
    let log = ScriptedLog::new();
    let harness = harness(kind, &log);
    let _ = step(&harness, &tree.0, Some(&base), "Remove a todo");
    let text = received(&log);
    assert_eq!(context_paths(&text), ["AGENTS.md"], "{kind:?}: {text}");
    assert!(
        text.contains(AGENTS_AT_BASE.trim_end()),
        "{kind:?}: the base's bytes are not given: {text}"
    );
    assert!(
        !text.contains("A step's edit") && !text.contains("uncommitted"),
        "{kind:?}: the step's copy is given: {text}"
    );
}

/// A fake Claude receives `AGENTS.md` at the base's bytes.
#[test]
fn claude_reads_agents_md_from_the_base() {
    given_the_base_agents_md(Kind::Claude);
}

/// The same for a fake Codex.
#[test]
fn codex_reads_agents_md_from_the_base() {
    given_the_base_agents_md(Kind::Codex);
}

/// The same for a fake b10x-harness.
#[test]
fn b10x_harness_reads_agents_md_from_the_base() {
    given_the_base_agents_md(Kind::B10x);
}

/// After a step edits `AGENTS.md`, and the edit is committed to the case's branch as an accepted
/// step's change is, the next step still gets the base bytes.
#[test]
fn a_step_edit_is_not_what_the_next_step_reads() {
    let (tree, base) = repository("edit", &[("AGENTS.md", AGENTS_AT_BASE.as_bytes())]);
    for kind in [Kind::Claude, Kind::Codex, Kind::B10x] {
        let log = ScriptedLog::new();
        let harness = harness(kind, &log);
        let _ = step(&harness, &tree.0, Some(&base), "Remove a todo");
        assert!(
            received(&log).contains(AGENTS_AT_BASE.trim_end()),
            "{kind:?}: the first step"
        );
    }
    fs::write(tree.0.join("AGENTS.md"), AGENTS_EDITED).expect("an edit");
    git(&tree.0, &["commit", "-q", "-am", "an accepted step's edit"]);
    for kind in [Kind::Claude, Kind::Codex, Kind::B10x] {
        let log = ScriptedLog::new();
        let harness = harness(kind, &log);
        let _ = step(&harness, &tree.0, Some(&base), "Remove a todo");
        let text = received(&log);
        assert!(
            text.contains(AGENTS_AT_BASE.trim_end()) && !text.contains("A step's edit"),
            "{kind:?}: the next step reads the edit: {text}"
        );
    }
}

/// A base that tracks only `CLAUDE.md` gives that file alone, and no error: `AGENTS.md` is on
/// disk, untracked at the base, and is not given.
#[test]
fn an_untracked_default_is_skipped() {
    let (tree, base) = repository("untracked", &[("CLAUDE.md", CLAUDE_AT_BASE.as_bytes())]);
    fs::write(tree.0.join("AGENTS.md"), AGENTS_EDITED).expect("an untracked file");
    for kind in [Kind::Claude, Kind::Codex, Kind::B10x] {
        let log = ScriptedLog::new();
        let harness = harness(kind, &log);
        let stepped = step(&harness, &tree.0, Some(&base), "Remove a todo");
        if let Err(error) = &stepped {
            assert!(
                error.downcast_ref::<InstructionFilesRefused>().is_none(),
                "{kind:?}: {error:#}"
            );
        }
        let text = received(&log);
        assert_eq!(context_paths(&text), ["CLAUDE.md"], "{kind:?}: {text}");
        assert!(!text.contains("A step's edit"), "{kind:?}: {text}");
    }
}

/// The step's opening record names each file: its path, its sha256 and the revision it was read
/// at.
#[test]
fn the_opening_record_names_each_file() {
    use sha2::Digest as _;
    let (tree, base) = repository(
        "record",
        &[
            ("AGENTS.md", AGENTS_AT_BASE.as_bytes()),
            ("CLAUDE.md", CLAUDE_AT_BASE.as_bytes()),
        ],
    );
    for kind in [Kind::Claude, Kind::Codex, Kind::B10x] {
        let log = ScriptedLog::new();
        let harness = harness(kind, &log);
        let _ = step(&harness, &tree.0, Some(&base), "Remove a todo");
        let line = opening(&harness);
        let named: Vec<(String, String, String)> = line["instructions"]
            .as_array()
            .unwrap_or_else(|| panic!("{kind:?}: no instructions in {line}"))
            .iter()
            .map(|file| {
                (
                    file["path"].as_str().unwrap_or_default().to_owned(),
                    file["sha256"].as_str().unwrap_or_default().to_owned(),
                    file["revision"].as_str().unwrap_or_default().to_owned(),
                )
            })
            .collect();
        let digest = |text: &str| format!("{:x}", sha2::Sha256::digest(text.as_bytes()));
        assert_eq!(
            named,
            [
                ("AGENTS.md".to_owned(), digest(AGENTS_AT_BASE), base.clone()),
                ("CLAUDE.md".to_owned(), digest(CLAUDE_AT_BASE), base.clone()),
            ],
            "{kind:?}: {line}"
        );
    }
}

/// A file over `SCOPE_BYTES` is named in the stop line and in the record, and no step is started
/// without it.
#[test]
fn an_oversized_file_is_named() {
    let big =
        "a line of the repository's rules\n".repeat(usize::try_from(SCOPE_BYTES).unwrap() / 30);
    assert!(big.len() as u64 > SCOPE_BYTES);
    let (scratch, _) = repository(
        "oversized",
        &[
            ("AGENTS.md", big.as_bytes()),
            ("CLAUDE.md", CLAUDE_AT_BASE.as_bytes()),
        ],
    );
    let tree = scratch.0.clone();
    let log = ScriptedLog::new();
    let session = harness(Kind::Claude, &log);
    let validators: Vec<Box<dyn Validator>> =
        vec![Box::new(TestsRun::new("true").expect("a command"))];
    let options = RunOptions {
        protocol: "story.delivery/1".to_owned(),
        intent: "make add add".to_owned(),
        inputs: Vec::new(),
        iterations: Some(2),
        hours: None,
        tokens: None,
        case_id: Some("oversized".to_owned()),
        gate: None,
    };
    let mut run = Run::open(&tree, options, &session, &validators).expect("opens");
    run.continue_past_red_base();
    let mut out = Vec::new();
    let stop = run.run(&mut out);
    let said = String::from_utf8_lossy(&out).into_owned();
    let Ok(Stop::StepRefused(why)) = &stop else {
        panic!("the step is refused, not {stop:?}\n{said}");
    };
    assert!(
        why.contains("AGENTS.md") && why.contains("past the"),
        "the stop line names the file: {why}"
    );
    assert!(
        format!("{}", stop.as_ref().expect("a stop")).contains("AGENTS.md"),
        "the stop line"
    );
    assert!(said.contains("AGENTS.md"), "{said}");
    let record =
        fs::read_to_string(tree.join(".takt/cases/oversized/events.jsonl")).unwrap_or_default();
    assert!(
        record
            .lines()
            .any(|line| line.contains("\"refusal\"") && line.contains("AGENTS.md")),
        "the record names the file: {record}"
    );
    assert_eq!(log.spawns(), 0, "a session was started without the file");
}

/// A configured list replaces the default: only the files named, each read at the base.
#[test]
fn a_configured_list_is_read_at_the_base_too() {
    let (tree, base) = repository(
        "configured",
        &[
            ("AGENTS.md", AGENTS_AT_BASE.as_bytes()),
            ("notes.md", b"# Notes\n\nA configured file.\n"),
        ],
    );
    fs::write(tree.0.join("notes.md"), "edited notes\n").expect("an edit");
    let log = ScriptedLog::new();
    let mut harness = harness(Kind::Claude, &log);
    harness.context = vec!["notes.md".to_owned()];
    let _ = step(&harness, &tree.0, Some(&base), "Remove a todo");
    let text = received(&log);
    assert_eq!(context_paths(&text), ["notes.md"], "{text}");
    assert!(text.contains("A configured file."), "{text}");
    assert!(!text.contains("edited notes"), "{text}");
}
