//! story:a-handover-after-red-is-not-the-next-step: after a `tests.run` that failed at the
//! current revision, the next judgement step's prompt opens with the failing tests and where each
//! test's assertion is, and a `change.handover` at that revision is not the next step: the step
//! is offered again with the writes the frontier admits. A handover at a revision whose tests pass
//! is recorded as before, and a protocol without a tests claim keeps the handover it had.
//!
//! The refusal is the protocol's (`AGENTS.md` § Rules: protocols are data): `story.delivery/2`'s
//! `change.handover` admitted only while `tests.pass` is not false. The built-in of
//! `b10x-canon-engineering` at the locked tag does not say so yet; the case runs on a copy of
//! it whose `change.handover` precondition does ([`story_delivery`]), and on the built-in as it
//! stands once it does. The prompt is takt's: the test command's failing lines are kept as a
//! gate's are, and lead the prompt. No live `claude` runs: the harness program is a script that
//! keeps each prompt it is given and acts on what the prompt offers.

#[path = "support/own_home.rs"]
mod own_home;

use std::fs;
use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::rc::Rc;

use takt::evidence::Record;
use takt::forge::{Checks, Forge, Memory};
use takt::harness::ClaudeCode;
use takt::validator::{ChangeRead, CiRead, Observation, PrOpen, TestsRun, Validator};
use takt::{Run, RunOptions, Stop};

/// The story's test file and its implementation file.
const TESTS: &str = "tests/remove_test.txt";
const IMPLEMENTATION: &str = "src/remove.txt";

/// The one test the story's tests add, as the test command names it when it fails.
const FAILING_TEST: &str = "remove::a_todo_is_removed_by_its_number";

/// Where the failing test's assertion is, as the test command prints it.
const ASSERTION_AT: &str = "tests/remove.rs:7:5";

/// The test command: fails, printing libtest's report of [`FAILING_TEST`], while the tree holds
/// the story's tests and not its implementation; passes otherwise.
const TEST_COMMAND: &str = r"#!/bin/sh
if [ -f tests/remove_test.txt ] && [ ! -f src/remove.txt ]; then
cat <<'OUT'
running 1 test
test remove::a_todo_is_removed_by_its_number ... FAILED

failures:

---- remove::a_todo_is_removed_by_its_number stdout ----

thread 'remove::a_todo_is_removed_by_its_number' panicked at tests/remove.rs:7:5:
assertion `left == right` failed
  left: 1
 right: 0
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace


failures:
    remove::a_todo_is_removed_by_its_number

test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out
OUT
exit 101
fi
echo 'running 1 test'
echo 'test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out'
";

/// `story.delivery/2`'s `change.handover` precondition at the locked tag.
const HANDOVER_TODAY: &str = "    precondition:
      claim: change.handed_over
      is: unknown
    may_produce:
      - evidence: handover
";

/// The same, admitted only while the tests do not fail at the current revision: the protocol
/// change this story asks of `story.delivery/2`.
const HANDOVER_GUARDED: &str = "    precondition:
      all:
        - claim: change.handed_over
          is: unknown
        - any:
            - claim: tests.pass
            - claim: tests.pass
              is: unknown
    may_produce:
      - evidence: handover
";

/// A protocol with no tests claim: the implementation is edited, the change handed over, and
/// the gate run on the handed-over revision.
const NO_TESTS_PROTOCOL: &str = "format: protocol/1

protocol:
  id: handover.only
  revision: 1
  description: >-
    Edit the implementation, hand the change over, and run the gate on the handed-over revision.

artifacts:
  implementation:
    description: a revision of the code

evidence_kinds:
  handover:
    description: >-
      the harness chose to hand the change over at this implementation revision, with no file
      changed; it has no result
  gate_result:
    description: the result of the repository's gate against an implementation revision

claims:
  change.handed_over:
    description: the harness handed the change over at the current implementation revision
    true_when:
      evidence:
        kind: handover
        subject: implementation
  gate.green:
    description: the repository's gate is green for the current implementation revision
    true_when:
      evidence:
        kind: gate_result
        result: pass
        subject: implementation

actions:
  implementation.edit:
    description: change the implementation in the repository's working revision
    effect: write
  change.handover:
    description: hand the change over, changing no file
    effect: read
    precondition:
      claim: change.handed_over
      is: unknown
    may_produce:
      - evidence: handover
  gate.run:
    description: run the repository's gate on the handed-over revision
    effect: read
    precondition:
      claim: change.handed_over
    may_produce:
      - evidence: gate_result

outcomes:
  accepted:
    description: the gate is green on the handed-over revision
    requires:
      claim: gate.green
";

/// `story.delivery/2` as this story asks it to be: the built-in of the engineering protocols,
/// its `change.handover` guarded on the tests ([`HANDOVER_GUARDED`]) while the built-in still
/// holds the precondition it had ([`HANDOVER_TODAY`]), else the built-in as it stands.
fn story_delivery() -> String {
    let builtin = canon_engineering::registry::get("story-delivery", 2)
        .expect("the built-in story.delivery/2")
        .yaml;
    match builtin.matches(HANDOVER_TODAY).count() {
        0 => builtin.to_owned(),
        1 => builtin.replace(HANDOVER_TODAY, HANDOVER_GUARDED),
        n => panic!("story.delivery/2 holds the handover precondition {n} times"),
    }
}

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
    String::from_utf8(output.stdout)
        .expect("utf-8")
        .trim()
        .to_owned()
}

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
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn directory(name: &str) -> Scratch {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("handover-after-red-{}-{name}", std::process::id()));
    if dir.exists() {
        fs::remove_dir_all(&dir).expect("a stale scratch directory is removed");
    }
    fs::create_dir_all(&dir).expect("a scratch directory");
    Scratch(dir)
}

/// A committed repository holding the test command ([`TEST_COMMAND`]), and the story's tests
/// when `with_tests`.
fn repository(name: &str, with_tests: bool) -> Scratch {
    let tree = directory(name);
    fs::write(tree.join("README.md"), "a story\n").expect("a file");
    let script = tree.join("tests.sh");
    fs::write(&script, TEST_COMMAND).expect("the test command");
    fs::set_permissions(&script, fs::Permissions::from_mode(0o755)).expect("exec bit");
    if with_tests {
        fs::create_dir_all(tree.join("tests")).expect("tests/");
        fs::write(tree.join(TESTS), "assert removed\n").expect("the tests");
    }
    git(&tree, &["init", "-q", "-b", "main"]);
    git(&tree, &["add", "-A"]);
    git(&tree, &["commit", "-q", "-m", "start"]);
    tree
}

/// A fake `claude` in `dir`: each call keeps its prompt as `dir/prompt-<n>` and acts on what the
/// prompt offers, in this order: writes the story's tests when they are absent and `tests.write`
/// is offered; hands the change over whenever `change.handover` is offered, as the harness of
/// the finding did at a red revision; writes the implementation when it is absent and
/// `implementation.edit` is offered; requests the merge when it needs an approval; else
/// declines.
fn fake_claude(dir: &Path) -> ClaudeCode {
    let script = r#"#!/bin/sh
d='@DIR@'
n=$(cat "$d/calls" 2>/dev/null || echo 0)
n=$((n + 1))
echo "$n" > "$d/calls"
printf '%s' "$2" > "$d/prompt-$n"
reply() {
  printf '{"type":"result","subtype":"success","result":"{\\"action\\": \\"%s\\", \\"summary\\": \\"%s\\"}"}\n' "$1" "$2"
}
offers() {
  case "$prompt" in *"- $1:"*) return 0 ;; esac
  return 1
}
needs_approval() {
  case "$prompt" in *"These need an approval"*"- $1"*) return 0 ;; esac
  return 1
}
prompt=$2
if [ ! -f tests/remove_test.txt ] && offers tests.write; then
  mkdir -p tests && echo 'assert removed' > tests/remove_test.txt
  reply tests.write 'wrote the story tests'
elif offers change.handover; then
  reply change.handover 'the change is complete'
elif [ ! -f src/remove.txt ] && offers implementation.edit; then
  mkdir -p src && echo 'removed' > src/remove.txt
  reply implementation.edit 'implemented the removal'
elif needs_approval repository.merge; then
  reply repository.merge 'requesting the merge'
else
  reply decline 'nothing is left to do'
fi
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

/// A validator of `action` that observes `pass` at every revision.
struct Passing {
    action: &'static str,
    kind: &'static str,
}

impl Validator for Passing {
    fn name(&self) -> &str {
        self.action
    }

    fn action(&self) -> &str {
        self.action
    }

    fn kind(&self) -> &str {
        self.kind
    }

    fn subject(&self) -> &str {
        "implementation"
    }

    fn observe(&self, _tree: &Path, revision: &str, seq: usize) -> anyhow::Result<Observation> {
        Ok(Observation {
            summary: "pass".to_owned(),
            artifact: None,
            records: vec![Record::new(
                format!("{}-{seq}", self.kind),
                self.kind,
                Some("pass".to_owned()),
                self.subject(),
                revision,
                self.name(),
            )],
        })
    }
}

/// A finished run: its stop, what it printed, every prompt its harness was given in order, the
/// case's evidence records and its run record, and the tree it ran in, kept until dropped.
struct Delivered {
    stop: Result<Stop, String>,
    text: String,
    prompts: Vec<String>,
    records: Vec<serde_json::Value>,
    events: Vec<serde_json::Value>,
    tree: Scratch,
    _fake: Scratch,
}

impl Delivered {
    /// The case's evidence records of `kind`.
    fn of(&self, kind: &str) -> Vec<&serde_json::Value> {
        self.records
            .iter()
            .filter(|record| record["kind"] == kind)
            .collect()
    }

    /// The revisions at which a `test_result` record says `result`.
    fn tests_at(&self, result: &str) -> Vec<&str> {
        self.of("test_result")
            .into_iter()
            .filter(|record| record["result"] == result)
            .filter_map(|record| record["subject_revision"].as_str())
            .collect()
    }

    /// The prompts given while `tests.pass` was false.
    fn red_prompts(&self) -> Vec<&String> {
        self.prompts
            .iter()
            .filter(|prompt| prompt.contains("- tests.pass: false\n"))
            .collect()
    }
}

/// A delivery run of the story on the protocol file `protocol` (its `inputs` given), the tests
/// run by [`TestsRun`] on [`TEST_COMMAND`], the gate green, `pr.open` and `ci.read` on a fake
/// forge whose checks pass, and `change.read`; the harness is [`fake_claude`].
fn delivered(name: &str, protocol: &str, inputs: &[(&str, &str)], with_tests: bool) -> Delivered {
    let tree = repository(name, with_tests);
    let fake = directory(&format!("{name}-claude"));
    let harness = fake_claude(&fake);
    let file = fake.join("protocol.yaml");
    fs::write(&file, protocol).expect("the protocol file");
    let memory = Rc::new(Memory {
        checks: Checks::Pass,
        ..Memory::default()
    });
    let forge: Rc<dyn Forge> = memory.clone();
    let validators: Vec<Box<dyn Validator>> = vec![
        Box::new(TestsRun::new("./tests.sh").expect("a test command")),
        Box::new(Passing {
            action: "gate.run",
            kind: "gate_result",
        }),
        Box::new(PrOpen::new(forge.clone())),
        Box::new(CiRead::new(forge)),
        Box::new(ChangeRead),
    ];
    let options = RunOptions {
        protocol: file.display().to_string(),
        intent: "remove a todo by its number".to_owned(),
        inputs: inputs
            .iter()
            .map(|(name, value)| ((*name).to_owned(), (*value).to_owned()))
            .collect(),
        iterations: Some(40),
        hours: None,
        tokens: None,
        case_id: Some(name.to_owned()),
        gate: Some("repository.merge".to_owned()),
    };
    let mut run = Run::open(&tree, options, &harness, &validators).expect("the case opens");
    let mut out = Vec::new();
    let stop = run.run(&mut out).map_err(|error| format!("{error:#}"));
    let case_dir = run.case.dir.clone();
    drop(run);
    let calls: usize = fs::read_to_string(fake.join("calls"))
        .map(|calls| calls.trim().parse().expect("a count"))
        .unwrap_or(0);
    let prompts = (1..=calls)
        .map(|n| fs::read_to_string(fake.join(format!("prompt-{n}"))).expect("a prompt"))
        .collect();
    let records = fs::read_dir(case_dir.join("evidence"))
        .map(|entries| {
            entries
                .map(|entry| {
                    serde_json::from_str(
                        &fs::read_to_string(entry.expect("an entry").path()).expect("a file"),
                    )
                    .expect("json")
                })
                .collect()
        })
        .unwrap_or_default();
    let events = fs::read_to_string(case_dir.join("events.jsonl"))
        .expect("a run record")
        .lines()
        .map(|line| serde_json::from_str(line).expect("json"))
        .collect();
    Delivered {
        stop,
        text: String::from_utf8(out).expect("utf-8"),
        prompts,
        records,
        events,
        tree,
        _fake: fake,
    }
}

/// The inputs a story case opens with by default: tests first, the gate after the handover.
const INPUTS: [(&str, &str); 2] = [("tests_first", "required"), ("gate_on", "after_handover")];

/// Acceptance 1: a harness that hands over whenever it may, at a revision whose tests fail, is
/// not offered the handover there: the step after the red `tests.run` is offered the writes the
/// frontier admits, its prompt opens with the failing test and where its assertion is, and no
/// handover is recorded at a red revision.
#[test]
fn a_handover_at_a_red_revision_is_not_the_next_step() {
    let run = delivered("red", &story_delivery(), &INPUTS, false);
    let text = &run.text;
    let red = run.tests_at("fail");
    assert!(!red.is_empty(), "the story's tests never ran red:\n{text}");
    for handover in run.of("handover") {
        let at = handover["subject_revision"].as_str().expect("a revision");
        assert!(
            !red.contains(&at),
            "a handover was recorded at the red revision {at}:\n{text}"
        );
    }
    let prompts = run.red_prompts();
    assert!(
        !prompts.is_empty(),
        "no step was given the red revision:\n{text}"
    );
    for prompt in prompts {
        assert!(
            !prompt.contains("- change.handover:"),
            "the handover is offered at a red revision:\n{prompt}"
        );
        assert!(
            prompt.contains("- implementation.edit:"),
            "the step at a red revision is not offered the edit:\n{prompt}"
        );
        let intent = prompt.find("Intent:").expect("the intent");
        let named = prompt
            .find(FAILING_TEST)
            .unwrap_or_else(|| panic!("the prompt does not name the failing test:\n{prompt}"));
        let located = prompt
            .find(&format!("- {FAILING_TEST} at {ASSERTION_AT}"))
            .unwrap_or_else(|| panic!("the prompt does not say where the assertion is:\n{prompt}"));
        assert!(
            named < intent && located < intent,
            "the prompt does not open with the failing test:\n{prompt}"
        );
        assert_eq!(
            prompt.matches(takt::harness::TESTS_PATH_ASK).count(),
            1,
            "the assertions' block is given once:\n{prompt}"
        );
    }
    let evidence: Vec<&serde_json::Value> = run
        .events
        .iter()
        .filter(|line| line["event"] == "evidence" && line["kind"] == "test_result")
        .filter(|line| line["result"] == "fail")
        .collect();
    assert!(
        evidence.iter().all(|line| line["failed"]
            .as_array()
            .is_some_and(|failed| !failed.is_empty())),
        "a failing test run keeps no failing lines on its evidence line: {evidence:?}"
    );
}

/// Acceptance 2: a handover at a revision whose tests pass is recorded as before, and the run
/// goes on from it to `change.read`; the prompt it was chosen in carries no failing tests.
#[test]
fn a_handover_at_a_passing_revision_is_recorded() {
    let run = delivered("green", &story_delivery(), &INPUTS, false);
    let text = &run.text;
    let handovers = run.of("handover");
    assert_eq!(handovers.len(), 1, "{handovers:?}\n{text}");
    let at = handovers[0]["subject_revision"]
        .as_str()
        .expect("a revision");
    assert!(
        run.tests_at("pass").contains(&at),
        "the handover is not at a revision whose tests pass:\n{text}"
    );
    git(
        &run.tree,
        &["cat-file", "-e", &format!("{at}:{IMPLEMENTATION}")],
    );
    let reads = run.of("change_record");
    assert_eq!(reads.len(), 1, "{reads:?}\n{text}");
    assert_eq!(reads[0]["subject_revision"], at, "{text}");
    let chosen = run
        .prompts
        .iter()
        .find(|prompt| prompt.contains("- change.handover:"))
        .expect("the step that handed over");
    assert!(
        !chosen.contains(FAILING_TEST),
        "a passing revision's prompt names a failing test:\n{chosen}"
    );
    assert_eq!(
        run.stop,
        Ok(Stop::ApprovalRequired(vec!["repository.merge".to_owned()])),
        "{text}"
    );
}

/// Acceptance 3: a protocol without a tests claim keeps the handover it had: offered at once,
/// recorded, and the run accepted on the gate after it, whatever the test command would say.
#[test]
fn a_protocol_without_a_tests_claim_keeps_the_handover() {
    let run = delivered("no-tests", NO_TESTS_PROTOCOL, &[], true);
    let text = &run.text;
    assert_eq!(run.of("handover").len(), 1, "{text}");
    assert!(run.of("test_result").is_empty(), "{text}");
    assert!(
        run.prompts
            .iter()
            .all(|prompt| !prompt.contains(FAILING_TEST)),
        "{:?}",
        run.prompts
    );
    assert!(
        matches!(run.stop, Ok(Stop::Completed(_))),
        "{:?}\n{text}",
        run.stop
    );
}
