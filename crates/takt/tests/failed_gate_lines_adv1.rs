//! story:edits-keep-ess-citations-resolved, attacked: what a failed gate printed reaches the next
//! judgement step's prompt and the run record's evidence line. A credential's shape must reach
//! neither (`AGENTS.md` § Rules: credentials never enter a prompt or a case record), and what is
//! shown as what failed must be what failed, not the output of a run of the command that passed.
//! The credential-shaped text is assembled at run time, so this file holds no such shape itself.

use std::fs;
use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};
use std::process::Command;

use takt::harness::ClaudeCode;
use takt::validator::{FAILED_LINES_PER_CHECK, FailedCheck, GateRun, Validator, failing_lines};
use takt::{Run, RunOptions};

const EDIT_PROTOCOL: &str = "format: protocol/1

protocol:
  id: gate.edit
  revision: 1
  description: >-
    Edit the implementation until the repository's gate is green at its current revision.

artifacts:
  implementation:
    description: a revision of the code

evidence_kinds:
  gate_result:
    description: the result of the repository's gate against an implementation revision

claims:
  gate.green:
    description: the repository's gate is green for the current implementation revision
    true_when:
      evidence:
        kind: gate_result
        result: pass
        subject: implementation

obligations:
  gate.required:
    description: the repository's gate is green for the current implementation revision
    discharged_when:
      claim: gate.green

actions:
  implementation.edit:
    description: change the implementation in the repository's working revision
    effect: write
  gate.run:
    description: run the repository's gate against an implementation revision
    effect: read
    may_produce:
      - evidence: gate_result

outcomes:
  accepted:
    description: the repository's gate is green for the current implementation revision
    requires:
      claim: gate.green
";

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

fn directory(name: &str) -> Scratch {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!(
        "failed-gate-lines-adv1-{}-{name}",
        std::process::id()
    ));
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

/// A committed repository whose one gate, `citations`, holds when `check.sh` (its body `check`)
/// exits 0.
fn repository(name: &str, check: &str) -> Scratch {
    let tree = directory(name);
    fs::create_dir_all(tree.join(".engineering")).expect("its directory");
    fs::write(
        tree.join(".engineering/gates.yaml"),
        "format: engineering-gates/1\nlanguage: canon-expr/1\ngates:\n  \
         - id: citations\n    text: Every citation resolves.\n    assertions:\n      \
         - 'process.succeeded(\"citations\")'\n",
    )
    .expect("a gates document");
    fs::write(
        tree.join(".engineering/assertions-project.yaml"),
        "format: engineering-assertions-project/1\ncommands:\n  citations:\n    \
         executable: check.sh\n",
    )
    .expect("a project file");
    let script = tree.join("check.sh");
    fs::write(&script, format!("#!/bin/sh\n{check}\n")).expect("a script");
    fs::set_permissions(&script, fs::Permissions::from_mode(0o755)).expect("exec bit");
    git(&tree, &["init", "-q", "-b", "main"]);
    git(&tree, &["add", "-A"]);
    git(&tree, &["commit", "-q", "-m", "start"]);
    tree
}

/// A fake `claude` keeping each prompt as `dir/prompt-<n>` and reporting an edit.
fn fake_claude(dir: &Path) -> ClaudeCode {
    let script = r#"#!/bin/sh
d='@DIR@'
n=$(cat "$d/calls" 2>/dev/null || echo 0)
n=$((n + 1))
echo "$n" > "$d/calls"
printf '%s' "$2" > "$d/prompt-$n"
echo "call-$n" >> work.txt
printf '%s\n' '{"type":"result","subtype":"success","result":"{\"action\": \"implementation.edit\", \"summary\": \"made a change\"}"}'
"#
    .replace("@DIR@", &dir.display().to_string());
    let program = dir.join("claude");
    fs::write(&program, script).expect("a script");
    fs::set_permissions(&program, fs::Permissions::from_mode(0o755)).expect("exec bit");
    ClaudeCode {
        program: program.display().to_string(),
        ..ClaudeCode::default()
    }
}

/// The gate, then one judgement step: the first prompt, the whole run record as text, and the
/// failed checks its evidence line keeps.
fn run(tree: &Path, id: &str) -> (String, String, Vec<FailedCheck>) {
    let fake = directory(&format!("{id}-claude"));
    let harness = fake_claude(&fake);
    let protocol = fake.join("protocol.yaml");
    fs::write(&protocol, EDIT_PROTOCOL).expect("a protocol");
    let options = RunOptions {
        protocol: protocol.display().to_string(),
        intent: "keep the citations resolved".to_owned(),
        inputs: Vec::new(),
        iterations: Some(2),
        hours: None,
        tokens: None,
        case_id: Some(id.to_owned()),
        gate: None,
    };
    let validators: Vec<Box<dyn Validator>> =
        vec![Box::new(GateRun::new(".engineering/gates.yaml"))];
    let mut run = Run::open(tree, options, &harness, &validators).expect("the case opens");
    let mut out = Vec::new();
    let stop = run.run(&mut out);
    let text = String::from_utf8(out).expect("utf-8");
    stop.unwrap_or_else(|error| panic!("the run stops without an error: {error:#}\n{text}"));
    let prompt = fs::read_to_string(fake.join("prompt-1"))
        .unwrap_or_else(|error| panic!("the judgement step ran: {error}\n{text}"));
    let record = fs::read_to_string(run.case.dir.join("events.jsonl")).expect("a run record");
    let failed = record
        .lines()
        .map(|line| serde_json::from_str::<serde_json::Value>(line).expect("json"))
        .filter(|line| line["event"] == "evidence")
        .filter_map(|line| serde_json::from_value::<Vec<FailedCheck>>(line["failed"].clone()).ok())
        .flatten()
        .collect();
    (prompt, record, failed)
}

/// A token of GitHub's personal-access shape, assembled here: `gh` `p_` and 39 alphanumerics.
fn token() -> String {
    let token = format!("gh{}_{}", 'p', "Zq3".repeat(13));
    assert!(
        takt::run::credential_shape(&token).is_some(),
        "the probe is a credential's shape"
    );
    token
}

/// A secret scanner (or any command run with colour forced) prints the match in colour: the SGR
/// sequence `ESC [ 31 m` ends in the word character `m` right before the token, and
/// `credential_shape` only matches a token that no word character precedes. The escape
/// sequence is taken out before the check, so the line is withheld.
#[test]
fn a_token_printed_in_colour_is_withheld() {
    let token = token();
    let printed = format!(
        "error: a secret in config/ci.env:3\n  found: \u{1b}[31m{token}\u{1b}[0m\n1 secret found\n"
    );
    let (lines, _) = failing_lines(&printed, FAILED_LINES_PER_CHECK);
    assert!(
        !lines.iter().any(|line| line.contains(&token)),
        "a credential's shape is kept: {lines:?}"
    );
}

/// A private key is a block of lines: only its `-----BEGIN … PRIVATE KEY-----` line has the
/// shape `credential_shape` knows, so the selection, which asks line by line, keeps every line of
/// the key's body.
#[test]
fn a_private_keys_body_is_withheld_with_its_first_line() {
    let body = [
        "QUJDREVGR0hJSktMTU5PUFFSU1RVVldYWVowMTIzNDU2Nzg5YWJjZGVmZ2hpamts",
        "bW5vcHFyc3R1dnd4eXpBQkNERUZHSElKS0xNTk9QUVJTVFVWV1hZWjAxMjM0NTY3",
    ];
    let printed = format!(
        "error: a key in fixtures/deploy_key\n-----BEGIN {kind}-----\n{}\n{}\n-----END {kind}-----\n1 secret found\n",
        body[0],
        body[1],
        kind = format!("OPENSSH {} KEY", "PRIVATE"),
    );
    assert!(takt::run::credential_shape(&printed).is_some());
    let (lines, _) = failing_lines(&printed, FAILED_LINES_PER_CHECK);
    for line in body {
        assert!(
            !lines.iter().any(|kept| kept.contains(line)),
            "a line of the private key's body is kept: {lines:?}"
        );
    }
}

/// The same coloured token, printed by a failing gate command, reaches neither the next
/// judgement step's prompt nor the run record.
#[test]
fn a_coloured_token_reaches_neither_the_prompt_nor_the_record() {
    let token = token();
    let tree = repository(
        "coloured-token",
        &format!(
            "echo 'error: a secret in config/ci.env:3'\nprintf '  found: \\033[31m%s\\033[0m\\n' '{token}'\necho '1 secret found'\nexit 1"
        ),
    );
    let (prompt, record, failed) = run(&tree, "coloured-token");
    assert_eq!(
        failed.len(),
        1,
        "the gate failed and kept its lines: {record}"
    );
    assert!(
        !prompt.contains(&token),
        "the prompt holds a credential's shape:\n{prompt}"
    );
    assert!(
        !record.contains(&token),
        "the run record holds a credential's shape:\n{record}"
    );
}

/// The gate's command fails the run that decides the gate and passes when takt runs it again to
/// read what it printed (a flaky gate, or one whose input changed between the two runs). What the
/// step is shown under `gate citations:` as "what the command of each failed gate printed that
/// names what failed" is then the passing run's output: nothing that failed, and nothing saying
/// the second run passed.
#[test]
fn a_run_again_that_passes_is_not_shown_as_what_failed() {
    let marks = directory("flaky-marks");
    let mark = marks.join("ran-once");
    let tree = repository(
        "flaky",
        &format!(
            "if [ -e '{}' ]; then echo 'all 43 citations resolved'; exit 0; fi\necho x > '{}'\necho 'ess/a.yaml:1: unresolved: src/b.rs:2'\necho '1 unresolved, 42 resolved'\nexit 1",
            mark.display(),
            mark.display()
        ),
    );
    let (prompt, record, failed) = run(&tree, "flaky");
    assert!(mark.exists(), "the gate ran: {record}");
    assert!(
        record.contains("\"result\":\"fail\""),
        "the gate failed: {record}"
    );
    let shown: Vec<&String> = failed.iter().flat_map(|check| &check.lines).collect();
    assert!(
        !shown
            .iter()
            .any(|line| line.contains("all 43 citations resolved")),
        "the passing run's output is kept as what failed: {shown:?}\n{prompt}"
    );
}

/// Bounds that hold (kept green): exactly at the budget every line is kept; one byte under it,
/// the summary is kept first and the rest counted; a line of two-byte characters is cut on a
/// character boundary within 512 bytes.
#[test]
fn bounds_hold_at_the_edge() {
    let printed = "a:1: unresolved: b\nsummary\n";
    let exact = "a:1: unresolved: b\n".len() + "summary\n".len();
    assert_eq!(
        failing_lines(printed, exact),
        (
            vec!["a:1: unresolved: b".to_owned(), "summary".to_owned()],
            0
        )
    );
    assert_eq!(
        failing_lines(printed, exact - 1),
        (vec!["summary".to_owned()], 1)
    );
    let wide = "é".repeat(600);
    let (lines, omitted) = failing_lines(&wide, FAILED_LINES_PER_CHECK);
    assert_eq!(omitted, 0);
    assert!(lines[0].len() <= 512, "{}", lines[0].len());
    assert!(lines[0].ends_with('…'));
    assert!(lines[0].trim_end_matches('…').chars().all(|c| c == 'é'));
}

/// Found on the way, not this story's: a gate script holding the shell's `:` builtin (a no-op,
/// common as `: > file`) makes the gate's reader of what Python loads ask git for the pathspec
/// `:/__main__.py`, which git refuses, and `gate.run` ends the run with an error instead of
/// failing the gate. This holds today's behaviour; story:a-gate-script-with-a-colon-is-run
/// changes it.
#[test]
fn today_a_gate_script_with_the_colon_builtin_ends_the_run_on_an_error() {
    let tree = repository("colon", ":\necho 'a:1: unresolved: b'\nexit 1");
    let fake = directory("colon-claude");
    let harness = fake_claude(&fake);
    let protocol = fake.join("protocol.yaml");
    fs::write(&protocol, EDIT_PROTOCOL).expect("a protocol");
    let options = RunOptions {
        protocol: protocol.display().to_string(),
        intent: "keep the citations resolved".to_owned(),
        inputs: Vec::new(),
        iterations: Some(2),
        hours: None,
        tokens: None,
        case_id: Some("colon".to_owned()),
        gate: None,
    };
    let validators: Vec<Box<dyn Validator>> =
        vec![Box::new(GateRun::new(".engineering/gates.yaml"))];
    let mut run = Run::open(&tree, options, &harness, &validators).expect("the case opens");
    let mut out = Vec::new();
    let stop = run.run(&mut out);
    let text = String::from_utf8(out).expect("utf-8");
    let error = format!(
        "{:#}",
        stop.expect_err(
            "story:a-gate-script-with-a-colon-is-run landed: rewrite this test to assert the \
             run stops without an error and the gate's evidence line keeps one failed check \
             holding `a:1: unresolved: b`"
        )
    );
    assert!(
        error.contains(":/__main__.py") && error.contains("pathspec magic"),
        "today the gate's reader of what Python loads asks git for the pathspec \
         `:/__main__.py`, which git refuses (story:a-gate-script-with-a-colon-is-run): \
         {error}\n{text}"
    );
}
