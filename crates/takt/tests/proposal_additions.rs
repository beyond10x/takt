//! What a session proposes is checked for what it adds. A file the case's tree already holds may
//! hold text of the shape of a credential (a design that quotes a private key's first line); an
//! edit elsewhere in that file adds none, and is admitted. What it adds is read with the lines
//! around it, so a shape assembled from a line the file held and lines the edit adds is still
//! refused; a new file, or one whose old content is no text, is read whole. The run's scan reads
//! the same text.
//!
//! The helpers are copies of `proposal_path_sec1.rs`'s: metaharness's scripted Codex session, no
//! vendor process, no credential, no network. Every key and token here is fake, and built at run
//! time so no file of this repository holds one.

#[path = "support/own_home.rs"]
mod own_home;

use std::fs;
use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};
use std::process::Command;

use loom_sdk::session::metaharness::{
    HarnessProcess, LaunchPlanView, ProcessRunner, ScriptedLog, ScriptedRunner,
};
use loom_sdk::session::{CredentialSource, Kind};
use takt::harness::SessionHarness;
use takt::run::credential_shape;
use takt::scan::Scan;
use takt::scan::gates::ChangeScan;
use takt::validator::{TestsRun, Validator};
use takt::{Run, RunOptions, Stop};

const BROKEN: &str = "pub fn add(a: i32, b: i32) -> i32 {\n    a - b\n}\n";
const FIXED: &str = "pub fn add(a: i32, b: i32) -> i32 {\n    a + b\n}\n";
const CODEX_PIN: &str = "0.153.4";
const ALNUM: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";
const NOTES: &str = "notes/keys.md";
/// What the stand-in scanner of [`marker_scan`] refuses.
const MARKER: &str = "SCANNER-REFUSES-THIS";

fn git(tree: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .args(["-c", "user.name=t", "-c", "user.email=t@localhost"])
        .args(["-c", "commit.gpgsign=false"])
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

/// A repository whose `HEAD` holds the broken `add` and `notes` at [`NOTES`].
fn repository(name: &str, notes: &[u8]) -> Scratch {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("additions-{}-{name}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    let scratch = Scratch(dir);
    let tree = scratch.tree();
    fs::create_dir_all(tree.join("src")).expect("the tree");
    fs::create_dir_all(tree.join("notes")).expect("the tree");
    fs::write(tree.join("README.md"), "a story\n").expect("a file");
    fs::write(tree.join("src/lib.rs"), BROKEN).expect("a file");
    fs::write(tree.join(NOTES), notes).expect("a file");
    git(&tree, &["init", "-q", "-b", "main"]);
    git(&tree, &["add", "-A"]);
    git(&tree, &["commit", "-q", "-m", "start"]);
    scratch
}

fn options(id: &str) -> RunOptions {
    RunOptions {
        protocol: "story.delivery/1".to_owned(),
        intent: "make add add".to_owned(),
        inputs: Vec::new(),
        iterations: Some(2),
        hours: None,
        tokens: None,
        case_id: Some(id.to_owned()),
        gate: None,
    }
}

fn validators() -> Vec<Box<dyn Validator>> {
    vec![Box::new(
        TestsRun::new("git grep -qE a.[+].b$").expect("a command"),
    )]
}

fn filler(seed: u64, n: usize) -> String {
    let mut state = seed;
    (0..n)
        .map(|_| {
            state = state
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            let at = usize::try_from(state >> 33).expect("fits") % ALNUM.len();
            char::from(ALNUM[at])
        })
        .collect()
}

fn dashes() -> String {
    "-".repeat(5)
}

/// A private key's first line.
fn begin() -> String {
    format!("{d}BEGIN OPENSSH {} KEY{d}\n", "PRIVATE", d = dashes())
}

/// What follows a private key's first line: its body and its last line.
fn body_and_end() -> String {
    format!(
        "{}\n{}\n{d}END OPENSSH {} KEY{d}\n",
        filler(81, 64),
        filler(82, 64),
        "PRIVATE",
        d = dashes()
    )
}

/// A fake GitHub token, as gh's `hosts.yml` holds one.
fn token() -> String {
    format!("ghp_{}", filler(83, 36))
}

/// A line of prose that quotes a private key's first line, as a design describing the shape does.
fn quoting() -> String {
    format!(
        "# a private key's block, from a `{d}BEGIN ... {} KEY{d}` line through its END line\n",
        "PRIVATE",
        d = dashes()
    )
}

/// Twenty lines no edit here touches: more than a shape's reach apart from what is edited.
fn far() -> String {
    (1..=20)
        .map(|n| format!("line {n} of the notes, which no edit here reaches\n"))
        .collect()
}

/// The notes as `HEAD` holds them: the quoting line, then [`far`], then the status.
fn notes() -> String {
    format!("# Keys\n{}{}the status: draft\n", quoting(), far())
}

/// The scripted Codex session's record, its reply naming `implementation.edit`.
fn record(cwd: &Path) -> Vec<String> {
    let reply = format!(
        "Done.\n{}",
        serde_json::json!({"action": "implementation.edit", "summary": "made add add"})
    );
    let cwd = cwd.display().to_string();
    let at = "2026-10-29T18:00:00.000Z";
    let turn = "01a15a19-5963-7d42-b749-21695314f7a4";
    [
        serde_json::json!({"timestamp": at, "type": "session_meta", "payload": {
            "session_id": "01a15a19-57e0-7d83-97b1-da333ebb91e5",
            "id": "01a15a19-57e0-7d83-97b1-da333ebb91e5", "timestamp": at,
            "cwd": cwd, "originator": "codex_exec", "cli_version": CODEX_PIN,
            "source": "exec", "model_provider": "openai"}}),
        serde_json::json!({"timestamp": at, "type": "event_msg", "payload": {
            "type": "task_started", "turn_id": turn}}),
        serde_json::json!({"timestamp": at, "type": "event_msg", "payload": {
            "type": "agent_message", "message": reply, "phase": "final_answer"}}),
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
    ]
    .iter()
    .map(ToString::to_string)
    .collect()
}

/// A scripted session: writes `writes` into its working directory as the model's tools would,
/// then prints [`record`].
struct Session {
    writes: Vec<(String, String)>,
}

impl ProcessRunner for Session {
    fn requires_executable(&self) -> bool {
        false
    }

    fn start(&mut self, plan: &LaunchPlanView) -> std::io::Result<Box<dyn HarnessProcess>> {
        for (path, text) in &self.writes {
            let at = plan.cwd.join(path);
            fs::create_dir_all(at.parent().expect("a parent"))?;
            fs::write(at, text)?;
        }
        ScriptedRunner::of_lines(record(plan.cwd), ScriptedLog::new()).start(plan)
    }
}

fn harness(writes: Vec<(String, String)>) -> SessionHarness {
    let mut harness = SessionHarness::of(Kind::Codex);
    harness.credentials = CredentialSource::None;
    harness.with_runner(move || {
        Box::new(Session {
            writes: writes.clone(),
        })
    })
}

/// A stand-in for `b10x-gates` beside `tree` that refuses a text holding [`MARKER`] and passes
/// any other.
fn marker_scan(tree: &Path) -> ChangeScan {
    let dir = tree.with_file_name("scanner-marker");
    fs::create_dir_all(&dir).expect("the scanner's directory");
    let policy = dir.join("policy.json");
    fs::write(&policy, "{}\n").expect("a policy");
    let program = dir.join("b10x-gates");
    fs::write(
        &program,
        format!("#!/bin/sh\nfor file; do :; done\n! grep -q {MARKER} \"$file\"\n"),
    )
    .expect("a scanner");
    fs::set_permissions(&program, fs::Permissions::from_mode(0o755)).expect("exec bit");
    ChangeScan { program, policy }
}

/// How the run of a session proposing the fix and `writes` ended on a tree whose [`NOTES`] are
/// `notes`, under a declared `scan: none`, or under `gates` with [`marker_scan`] when `scanned`;
/// with the notes as the tree holds them after.
fn proposing(
    name: &str,
    notes: &[u8],
    writes: Vec<(String, String)>,
    scanned: bool,
) -> (std::io::Result<Stop>, String, Vec<u8>) {
    let scratch = repository(name, notes);
    let tree = scratch.tree();
    let mut writes = writes;
    writes.push(("src/lib.rs".to_owned(), FIXED.to_owned()));
    let session = harness(writes);
    let validators = validators();
    let mut run = Run::open(&tree, options(name), &session, &validators).expect("opens");
    if scanned {
        run.scan_change_with(Some(marker_scan(&tree)));
    } else {
        run.scan_with(Scan::None {
            declared_in: tree.with_file_name("run.toml"),
        });
    }
    run.continue_past_red_base();
    let mut out = Vec::new();
    let stop = run.run(&mut out);
    let said = String::from_utf8_lossy(&out).into_owned();
    let after = fs::read(tree.join(NOTES)).unwrap_or_default();
    (
        stop.map_err(|error| std::io::Error::other(format!("{error:#}"))),
        said,
        after,
    )
}

/// The refusal `stop` holds, or a failure naming what the run said.
fn refusal(stop: &std::io::Result<Stop>, said: &str) -> String {
    let Ok(Stop::StepRefused(why)) = stop else {
        panic!("refused before the edit effect, not {stop:?}\n{said}");
    };
    why.clone()
}

/// The fixtures hold the shapes they are written for, and the notes as `HEAD` has them hold a
/// private key's: what makes the cases below about what an edit adds.
#[test]
fn the_fixtures_hold_the_shapes_they_stand_for() {
    assert_eq!(credential_shape(&notes()), Some("private-key"));
    assert_eq!(credential_shape(&begin()), Some("private-key"));
    assert_eq!(credential_shape(&body_and_end()), None);
    assert_eq!(credential_shape(&far()), None);
    assert_eq!(credential_shape(&token()), Some("github-token"));
}

/// An edit of one line of a file that quotes a private key's first line twenty lines away adds
/// nothing of a credential's shape, and is admitted: run 8's refusal of an edit to a design
/// that quotes the shape it describes.
#[test]
fn an_edit_far_from_a_shape_the_file_held_is_admitted() {
    let edited = notes().replace("the status: draft", "the status: final");
    let (stop, said, after) = proposing(
        "far",
        notes().as_bytes(),
        vec![(NOTES.to_owned(), edited.clone())],
        false,
    );
    assert!(
        !matches!(stop, Ok(Stop::StepRefused(_))),
        "an edit that adds no shape was refused: {stop:?}\n{said}"
    );
    assert_eq!(String::from_utf8_lossy(&after), edited, "{said}");
}

/// A private key's block added to such a file is refused, the file named.
#[test]
fn a_private_key_added_to_a_file_that_quotes_one_is_refused() {
    let edited = format!("{}{}{}", notes(), begin(), body_and_end());
    let (stop, said, after) = proposing(
        "added-key",
        notes().as_bytes(),
        vec![(NOTES.to_owned(), edited)],
        false,
    );
    let why = refusal(&stop, &said);
    assert!(
        why.contains(&format!(
            "proposes {NOTES}, which holds text of the shape of a credential (private-key)"
        )),
        "{why}"
    );
    assert_eq!(String::from_utf8_lossy(&after), notes());
}

/// A key's body and last line added right after a first line the file held: none of the added
/// lines holds the shape, and with the line before them it does, so it is refused.
#[test]
fn a_key_s_body_added_after_a_first_line_the_file_held_is_refused() {
    let held = format!("# Keys\n{}{}the status: draft\n", begin(), far());
    let edited = format!(
        "# Keys\n{}{}{}the status: draft\n",
        begin(),
        body_and_end(),
        far()
    );
    let (stop, said, after) = proposing(
        "body",
        held.as_bytes(),
        vec![(NOTES.to_owned(), edited)],
        false,
    );
    let why = refusal(&stop, &said);
    assert!(why.contains("(private-key)"), "{why}");
    assert_eq!(String::from_utf8_lossy(&after), held);
}

/// A private key's first line may cross line ends: its label is read up to 40 bytes past
/// `-----BEGIN `, whatever they hold. One whose `-----BEGIN ` is a line the file held, an empty
/// line after it, and the rest of the label a line the edit adds, is refused: what is read
/// around an added line reaches as far back as such a match can start, not one line only.
#[test]
fn a_first_line_assembled_across_an_unchanged_empty_line_is_refused() {
    let held = format!("# Keys\n{}BEGIN \n\n{}the status: draft\n", dashes(), far());
    let edited = format!(
        "# Keys\n{d}BEGIN \n\nOPENSSH {} KEY{d}\n{}the status: draft\n",
        "PRIVATE",
        far(),
        d = dashes()
    );
    assert_eq!(credential_shape(&held), None);
    assert_eq!(credential_shape(&edited), Some("private-key"));
    let (stop, said, after) = proposing(
        "assembled",
        held.as_bytes(),
        vec![(NOTES.to_owned(), edited)],
        false,
    );
    let why = refusal(&stop, &said);
    assert!(why.contains("(private-key)"), "{why}");
    assert_eq!(String::from_utf8_lossy(&after), held);

    // The same reach after an added line: its `-----BEGIN ` added, the rest of the label a line
    // the file held past an empty one.
    let held = format!(
        "# Keys\n\nOPENSSH {} KEY{d}\n{}the status: draft\n",
        "PRIVATE",
        far(),
        d = dashes()
    );
    let edited = format!(
        "# Keys\n{d}BEGIN \n\nOPENSSH {} KEY{d}\n{}the status: draft\n",
        "PRIVATE",
        far(),
        d = dashes()
    );
    assert_eq!(credential_shape(&held), None);
    let (stop, said, after) = proposing(
        "assembled-after",
        held.as_bytes(),
        vec![(NOTES.to_owned(), edited)],
        false,
    );
    let why = refusal(&stop, &said);
    assert!(why.contains("(private-key)"), "{why}");
    assert_eq!(String::from_utf8_lossy(&after), held);
}

/// A token added to a file that quotes a private key's first line far from it is refused for
/// the token's shape: what is read is what the edit adds, not the line the file held.
#[test]
fn a_token_added_far_from_a_shape_the_file_held_is_refused_for_its_own() {
    let edited = format!("{}the token: {}\n", notes(), token());
    let (stop, said, after) = proposing(
        "token",
        notes().as_bytes(),
        vec![(NOTES.to_owned(), edited)],
        false,
    );
    let why = refusal(&stop, &said);
    assert!(
        why.contains(&format!(
            "proposes {NOTES}, which holds text of the shape of a credential (github-token)"
        )),
        "{why}"
    );
    assert!(!why.contains(&token()), "{why}");
    assert_eq!(String::from_utf8_lossy(&after), notes());
}

/// A file `HEAD` does not hold is read whole.
#[test]
fn a_new_file_is_read_whole() {
    let new = format!("# A key\n{}{}{}", far(), begin(), body_and_end());
    let (stop, said, _) = proposing(
        "new",
        notes().as_bytes(),
        vec![("notes/new.md".to_owned(), new)],
        false,
    );
    let why = refusal(&stop, &said);
    assert!(
        why.contains(
            "proposes notes/new.md, which holds text of the shape of a credential (private-key)"
        ),
        "{why}"
    );
}

/// A file whose content at `HEAD` is no UTF-8 text cannot be compared line by line, and is read
/// whole: the quoting line it held refuses an edit twenty lines from it.
#[test]
fn a_file_whose_old_content_is_no_text_is_read_whole() {
    let mut held = b"\xff\xfe\n".to_vec();
    held.extend_from_slice(notes().as_bytes());
    let edited = format!(
        "binary no more\n{}",
        notes().replace("the status: draft", "the status: final")
    );
    let (stop, said, after) = proposing("binary", &held, vec![(NOTES.to_owned(), edited)], false);
    let why = refusal(&stop, &said);
    assert!(why.contains("(private-key)"), "{why}");
    assert_eq!(after, held);
}

/// The run's scan reads the same text: a file holding what the scan refuses, edited far from
/// it, passes; the same text added is refused by the scan.
#[test]
fn the_scan_reads_what_the_edit_adds() {
    let held = format!(
        "# Notes\nthe marker: {MARKER}\n{}the status: draft\n",
        far()
    );
    let edited = held.replace("the status: draft", "the status: final");
    let (stop, said, after) = proposing(
        "scan-far",
        held.as_bytes(),
        vec![(NOTES.to_owned(), edited.clone())],
        true,
    );
    assert!(
        !matches!(stop, Ok(Stop::StepRefused(_))),
        "an edit that adds nothing the scan refuses was refused: {stop:?}\n{said}"
    );
    assert_eq!(String::from_utf8_lossy(&after), edited, "{said}");

    let added = format!("{held}the marker again: {MARKER}\n");
    let (stop, said, after) = proposing(
        "scan-added",
        held.as_bytes(),
        vec![(NOTES.to_owned(), added)],
        true,
    );
    let why = refusal(&stop, &said);
    assert!(
        why.contains(&format!(
            "proposes {NOTES}, which did not pass the secret scan"
        )),
        "{why}"
    );
    assert_eq!(String::from_utf8_lossy(&after), held);
}
