//! story:edits-keep-ess-citations-resolved: a failed `gate.run` keeps, per failed gate, the
//! lines its command printed that name what failed, bounded in bytes with the count of lines
//! left out; the run record's evidence line of the gate's record keeps the same lines, and the
//! next judgement step's prompt shows them under the failed gate's id. A gate that passes adds
//! nothing. No live `claude` runs: the harness program is a script that keeps each prompt it is
//! given.

#[path = "support/own_home.rs"]
mod own_home;

use std::fs;
use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};
use std::process::Command;

use takt::harness::ClaudeCode;
use takt::validator::{
    FAILED_LINES_PER_CHECK, FAILED_LINES_TOTAL, FailedCheck, GateRun, Validator, failing_lines,
};
use takt::{Run, RunOptions};

/// A protocol whose gate runs first, and whose one judgement action edits the implementation
/// until the gate is green.
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

fn directory(name: &str) -> Scratch {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("failed-gate-lines-{}-{name}", std::process::id()));
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

/// A committed repository whose gates document holds a `citations` gate on the command
/// `check.sh` (its body `check`) and a `manifest` gate that holds, with the project file that
/// admits the command.
fn repository(name: &str, check: &str) -> Scratch {
    let tree = directory(name);
    fs::create_dir_all(tree.join(".engineering")).expect("its directory");
    fs::write(tree.join("Cargo.toml"), "[package]\nname = \"scratch\"\n").expect("a manifest");
    fs::write(
        tree.join(".engineering/gates.yaml"),
        "format: engineering-gates/1\nlanguage: canon-expr/1\ngates:\n  \
         - id: manifest\n    text: The repository has its manifest.\n    assertions:\n      \
         - 'fs.file.exists(\"Cargo.toml\")'\n  \
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

/// A fake `claude` in `dir`: each call keeps its prompt as `dir/prompt-<n>`, appends a line to
/// `work.txt` in the tree and reports `implementation.edit`.
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

/// What a two-step run (the gate, then one judgement step) left: what it printed, the first
/// prompt and the run record's lines.
struct Ran {
    text: String,
    prompt: String,
    record: Vec<serde_json::Value>,
}

fn run(tree: &Path, id: &str) -> Ran {
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
    let record = fs::read_to_string(run.case.dir.join("events.jsonl"))
        .expect("a run record")
        .lines()
        .map(|line| serde_json::from_str(line).expect("json"))
        .collect();
    Ran {
        text,
        prompt,
        record,
    }
}

/// The run record's `evidence` lines.
fn evidence(record: &[serde_json::Value]) -> Vec<&serde_json::Value> {
    record
        .iter()
        .filter(|line| line["event"] == "evidence")
        .collect()
}

const UNRESOLVED: [&str; 3] = [
    "ess/domains/takt.yaml:12: unresolved: crates/takt/src/git.rs:30-34",
    "ess/domains/takt.yaml:40: unresolved: crates/takt/src/git.rs:88",
    "ess/domains/takt.yaml:97: unresolved: crates/takt/src/git.rs:120-131",
];

/// Acceptance 1 and 2: each `<document>:<line>: unresolved:` line the failing command printed
/// is in the next judgement step's prompt under the gate's id, and in the run record's evidence
/// line of the gate's record; the gate that held is named nowhere in the block.
#[test]
fn the_failed_gates_lines_reach_the_next_prompt_and_the_record() {
    let mut check = String::new();
    for line in UNRESOLVED {
        check.push_str(&format!(
            "echo '{line}'\necho '    cited lines no longer match their digest'\n"
        ));
    }
    check.push_str("echo '3 unresolved, 40 resolved'\nexit 1");
    let tree = repository("citations", &check);
    let ran = run(&tree, "citations");
    let evidence = evidence(&ran.record);
    assert_eq!(evidence.len(), 1, "{}", ran.text);
    assert_eq!(evidence[0]["result"], "fail", "{}", ran.text);
    let failed: Vec<FailedCheck> = serde_json::from_value(evidence[0]["failed"].clone())
        .unwrap_or_else(|error| {
            panic!(
                "the evidence line keeps the failed gate: {error}\n{}",
                evidence[0]
            )
        });
    assert_eq!(failed.len(), 1, "{failed:?}");
    assert_eq!(failed[0].id, "citations", "{failed:?}");
    assert_eq!(failed[0].omitted, 0, "{failed:?}");
    for line in UNRESOLVED {
        assert!(
            failed[0].lines.iter().any(|kept| kept == line),
            "{line} in {failed:?}"
        );
    }
    assert_eq!(
        failed[0].lines.last().map(String::as_str),
        Some("3 unresolved, 40 resolved")
    );
    let block = ran
        .prompt
        .split_once("gate citations")
        .unwrap_or_else(|| panic!("the prompt names the failed gate:\n{}", ran.prompt))
        .1;
    for line in UNRESOLVED {
        assert!(block.contains(line), "{line} in\n{}", ran.prompt);
    }
    assert!(
        !ran.prompt.contains("gate manifest"),
        "a gate that held adds nothing:\n{}",
        ran.prompt
    );
}

/// Acceptance 2: a gate that passes adds nothing to the record nor to the next prompt, which
/// carries no failed-gate block at all.
#[test]
fn a_gate_that_passes_adds_nothing() {
    let failing = repository(
        "failing-block",
        "echo 'a.yaml:1: unresolved: b.rs:2'\nexit 1",
    );
    let with = run(&failing, "failing-block");
    let passing = repository("passing", "echo 'all resolved'\nexit 0");
    let fake = directory("passing-claude");
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
        case_id: Some("passing".to_owned()),
        gate: None,
    };
    let validators: Vec<Box<dyn Validator>> =
        vec![Box::new(GateRun::new(".engineering/gates.yaml"))];
    let mut run = Run::open(&passing, options, &harness, &validators).expect("the case opens");
    let mut out = Vec::new();
    run.run(&mut out).expect("the run stops");
    let text = String::from_utf8(out).expect("utf-8");
    let record: Vec<serde_json::Value> = fs::read_to_string(run.case.dir.join("events.jsonl"))
        .expect("a run record")
        .lines()
        .map(|line| serde_json::from_str(line).expect("json"))
        .collect();
    let evidence = evidence(&record);
    assert_eq!(evidence.len(), 1, "{text}");
    assert_eq!(evidence[0]["result"], "pass", "{text}");
    assert!(
        evidence[0].get("failed").is_none(),
        "a passing gate keeps nothing: {}",
        evidence[0]
    );
    assert!(
        !fake.join("prompt-1").exists(),
        "a green gate accepts with no judgement step:\n{text}"
    );
    // The failing run's prompt is the passing one's but for the block: without it, the prompt
    // is byte for byte what it was before failed gates were shown.
    let (head, tail) = with
        .prompt
        .split_once(takt::harness::FAILED_CHECKS_LABEL)
        .unwrap_or_else(|| panic!("a failed gate's block:\n{}", with.prompt));
    assert!(
        head.ends_with("\n\n") || head.ends_with('\n'),
        "the block opens on a line of its own:\n{}",
        with.prompt
    );
    assert!(
        tail.contains("a.yaml:1: unresolved: b.rs:2"),
        "{}",
        with.prompt
    );
}

/// Acceptance 3: the kept lines are bounded in bytes, the last line (the summary) is always
/// kept, and the count of lines left out is named in the record and the prompt.
#[test]
fn output_past_the_bound_is_cut_with_the_count_left_out() {
    let check = "i=0\nwhile [ $i -lt 300 ]; do\n  \
                 echo \"ess/domains/takt.yaml:$i: unresolved: crates/takt/src/git.rs:$i-$((i + 1))\"\n  \
                 echo '    cited lines no longer match their digest'\n  i=$((i + 1))\ndone\n\
                 echo '300 unresolved'\nexit 1";
    let tree = repository("bounded", check);
    let ran = run(&tree, "bounded");
    let evidence = evidence(&ran.record);
    assert_eq!(evidence.len(), 1, "{}", ran.text);
    let failed: Vec<FailedCheck> =
        serde_json::from_value(evidence[0]["failed"].clone()).expect("the failed gate is kept");
    let kept = &failed[0];
    let bytes: usize = kept.lines.iter().map(|line| line.len() + 1).sum();
    assert!(bytes <= FAILED_LINES_PER_CHECK, "{bytes} bytes kept");
    assert_eq!(kept.lines.len() + kept.omitted as usize, 601, "{kept:?}");
    assert!(kept.omitted > 0, "{kept:?}");
    assert_eq!(
        kept.lines.last().map(String::as_str),
        Some("300 unresolved")
    );
    assert_eq!(
        kept.lines.first().map(String::as_str),
        Some("ess/domains/takt.yaml:0: unresolved: crates/takt/src/git.rs:0-1")
    );
    // Lines that name what failed are kept before indented detail.
    assert!(
        kept.lines
            .iter()
            .all(|line| !line.starts_with(char::is_whitespace)),
        "{kept:?}"
    );
    assert!(
        ran.prompt
            .contains(&format!("gate citations ({} lines left out)", kept.omitted)),
        "{}",
        ran.prompt
    );
}

/// The selection on its own: every non-blank line when they fit; past the bound, the lines
/// that are not indented detail first, in order, then detail as room is left, and the last
/// line always; for any command's output (a clippy-shaped one here).
#[test]
fn line_selection_is_generic_and_bounded() {
    let clippy = "    Checking takt v0.1.0\n\
                  error: unused variable: `x`\n  --> src/lib.rs:3:9\n   |\n3  |     let x = 1;\n\n\
                  error: could not compile `takt` (lib) due to 1 previous error\n";
    let (lines, omitted) = failing_lines(clippy, FAILED_LINES_PER_CHECK);
    assert_eq!(omitted, 0);
    assert_eq!(
        lines,
        [
            "    Checking takt v0.1.0",
            "error: unused variable: `x`",
            "  --> src/lib.rs:3:9",
            "   |",
            "3  |     let x = 1;",
            "error: could not compile `takt` (lib) due to 1 previous error",
        ]
    );
    // Tight: the summary, then the lines not indented from the start; no indented detail
    // while one of those is left out.
    let (lines, omitted) = failing_lines(clippy, 120);
    assert_eq!(
        lines,
        [
            "error: unused variable: `x`",
            "3  |     let x = 1;",
            "error: could not compile `takt` (lib) due to 1 previous error",
        ]
    );
    assert_eq!(omitted, 3);
    // Room for every line that is not indented: the earliest detail after them.
    let (lines, omitted) = failing_lines(clippy, 150);
    assert_eq!(
        lines,
        [
            "    Checking takt v0.1.0",
            "error: unused variable: `x`",
            "3  |     let x = 1;",
            "error: could not compile `takt` (lib) due to 1 previous error",
        ]
    );
    assert_eq!(omitted, 2);
    // No room at all: nothing kept, every line counted.
    assert_eq!(failing_lines(clippy, 0), (Vec::<String>::new(), 6));
    // A line longer than the bound is cut, and still counted as kept.
    let long = "x".repeat(10_000);
    let (lines, omitted) = failing_lines(&long, FAILED_LINES_PER_CHECK);
    assert_eq!(omitted, 0);
    assert!(
        lines[0].len() < FAILED_LINES_PER_CHECK,
        "{}",
        lines[0].len()
    );
    assert!(lines[0].ends_with('…'));
    const { assert!(FAILED_LINES_TOTAL >= FAILED_LINES_PER_CHECK) };
}

/// The next judgement step is shown the last gate run's failures only: a later gate run that
/// passed, at the same action, leaves nothing to show.
#[test]
fn only_the_last_gate_runs_failures_are_shown() {
    let failed =
        serde_json::json!([{"id": "citations", "lines": ["a:1: unresolved: b"], "omitted": 0}]);
    let step = |n: u64, action: &str| {
        let by = if action == "gate.run" {
            "validator"
        } else {
            "harness"
        };
        serde_json::json!({"step": n, "event": "step", "by": by, "name": "n", "action": action, "summary": "s"})
    };
    let mut record = vec![
        step(1, "gate.run"),
        serde_json::json!({"step": 1, "event": "evidence", "id": "gate-result-1", "kind": "gate_result", "result": "fail", "path": "p", "failed": failed}),
    ];
    let shown = takt::run::failed_checks(&record);
    assert_eq!(shown.len(), 1, "{shown:?}");
    assert_eq!(shown[0].lines, ["a:1: unresolved: b"]);
    record.push(step(2, "implementation.edit"));
    assert_eq!(takt::run::failed_checks(&record).len(), 1);
    record.push(step(3, "gate.run"));
    record.push(serde_json::json!({"step": 3, "event": "evidence", "id": "gate-result-3", "kind": "gate_result", "result": "pass", "path": "p"}));
    assert!(takt::run::failed_checks(&record).is_empty());
}

// story:failed-gate-output-floods-prompt: a test runner's output reaches the next step's prompt
// as each failed test's name and its last error line, bounded, never as the lines it printed.

/// What `python -m unittest -v` printed for two passing tests, a test whose call the seed's
/// argparse refuses (its usage text interleaved with the progress lines) and a failed
/// assertion.
const UNITTEST: &str = "test_add (tests.test_todo.TodoTest.test_add) ... ok
test_list (tests.test_todo.TodoTest.test_list) ... ok
test_remove (test_remove.RemoveTest.test_remove) ... usage: todo.py [-h] --file FILE {add,list,done} ...
todo.py: error: argument command: invalid choice: 'remove' (choose from 'add', 'list', 'done')
ERROR
test_done (tests.test_todo.TodoTest.test_done) ... FAIL

======================================================================
ERROR: test_remove (test_remove.RemoveTest.test_remove)
----------------------------------------------------------------------
Traceback (most recent call last):
  File \"/repo/tests/test_remove.py\", line 5, in test_remove
    self.assertEqual(todo.main([\"--file\", \"t.json\", \"remove\", \"1\"]), 0)
                     ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
  File \"/repo/todo.py\", line 30, in main
    args = parser.parse_args(argv)
  File \"/usr/lib/python3.12/argparse.py\", line 2643, in error
    self.exit(2, _('%(prog)s: error: %(message)s\\n') % args)
  File \"/usr/lib/python3.12/argparse.py\", line 2630, in exit
    _sys.exit(status)
SystemExit: 2

======================================================================
FAIL: test_done (tests.test_todo.TodoTest.test_done)
----------------------------------------------------------------------
Traceback (most recent call last):
  File \"/repo/tests/test_todo.py\", line 20, in test_done
    self.assertEqual(items, [])
AssertionError: Lists differ: ['a'] != []

First list contains 1 additional elements.
First extra element 0:
'a'

- ['a']
+ []

----------------------------------------------------------------------
Ran 4 tests in 0.003s

FAILED (failures=1, errors=1)
";

/// Every line of [`UNITTEST`] that must not reach a prompt: a passing test's, the usage text,
/// a traceback's frames and its header.
const NOT_SHOWN: [&str; 6] = [
    "... ok",
    "usage: todo.py",
    "invalid choice",
    "Traceback (most recent call last)",
    "File \"/usr/lib/python3.12/argparse.py\"",
    "_sys.exit(status)",
];

/// Acceptance 1: a unittest run's output is reduced to each failed test's name and its last
/// error line, in order, with nothing left out.
#[test]
fn a_unittest_run_is_its_failed_tests_and_their_last_error_lines() {
    let (lines, omitted) = failing_lines(UNITTEST, FAILED_LINES_PER_CHECK);
    assert_eq!(
        lines,
        [
            "ERROR test_remove (test_remove.RemoveTest.test_remove) - SystemExit: 2",
            "FAIL test_done (tests.test_todo.TodoTest.test_done) - AssertionError: Lists differ: ['a'] != []",
        ]
    );
    assert_eq!(omitted, 0);
}

/// Acceptance 1, for the two other runners named: pytest's short summary and libtest's
/// failures, each test with its error line.
#[test]
fn pytest_and_libtest_runs_are_their_failed_tests() {
    let pytest = "============================= test session starts ==============================
collected 3 items

tests/test_todo.py .F                                                     [ 66%]
tests/test_remove.py F                                                    [100%]

=================================== FAILURES ===================================
_________________________________ test_done ____________________________________

    def test_done():
>       assert done(1) == []
E       assert ['a'] == []

tests/test_todo.py:9: AssertionError
=========================== short test summary info ============================
FAILED tests/test_todo.py::test_done - assert ['a'] == []
FAILED tests/test_remove.py::test_remove - SystemExit: 2
========================= 2 failed, 1 passed in 0.02s ==========================
";
    assert_eq!(
        failing_lines(pytest, FAILED_LINES_PER_CHECK),
        (
            vec![
                "FAILED tests/test_todo.py::test_done - assert ['a'] == []".to_owned(),
                "FAILED tests/test_remove.py::test_remove - SystemExit: 2".to_owned(),
            ],
            0
        )
    );
    let libtest = "running 3 tests
test tests::adds ... ok
test tests::removes ... FAILED
test tests::lists ... FAILED

failures:

---- tests::removes stdout ----

thread 'tests::removes' panicked at src/lib.rs:12:9:
assertion `left == right` failed
  left: 1
 right: 0
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace

---- tests::lists stdout ----
thread 'tests::lists' panicked at src/lib.rs:20:5:
no todo file

failures:
    tests::lists
    tests::removes

test result: FAILED. 1 passed; 2 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

error: test failed, to rerun pass `--lib`
";
    assert_eq!(
        failing_lines(libtest, FAILED_LINES_PER_CHECK),
        (
            vec![
                "FAILED tests::removes - panicked at src/lib.rs:12:9: assertion `left == right` failed | left: 1 | right: 0"
                    .to_owned(),
                "FAILED tests::lists - panicked at src/lib.rs:20:5: no todo file".to_owned(),
            ],
            0
        )
    );
}

/// Acceptance 2: however many tests failed, the lines kept stay within the bound, every failed
/// test is either kept or counted as left out, and none of the runner's own lines comes back.
#[test]
fn failed_tests_past_the_bound_are_counted() {
    let mut printed = String::new();
    for n in 0..300 {
        printed.push_str(&format!(
            "test_{n} (tests.test_many.ManyTest.test_{n}) ... FAIL\n"
        ));
    }
    for n in 0..300 {
        printed.push_str(&format!(
            "======================================================================\n\
             FAIL: test_{n} (tests.test_many.ManyTest.test_{n})\n\
             ----------------------------------------------------------------------\n\
             Traceback (most recent call last):\n  \
             File \"/repo/tests/test_many.py\", line {n}, in test_{n}\n    \
             self.assertEqual({n}, -1)\n\
             AssertionError: {n} != -1\n\n"
        ));
    }
    printed.push_str("Ran 300 tests in 0.1s\n\nFAILED (failures=300)\n");
    let (lines, omitted) = failing_lines(&printed, FAILED_LINES_PER_CHECK);
    let bytes: usize = lines.iter().map(|line| line.len() + 1).sum();
    assert!(bytes <= FAILED_LINES_PER_CHECK, "{bytes} bytes kept");
    assert_eq!(lines.len() + omitted as usize, 300, "{lines:?}");
    assert!(omitted > 0);
    assert_eq!(
        lines[0],
        "FAIL test_0 (tests.test_many.ManyTest.test_0) - AssertionError: 0 != -1"
    );
    assert!(
        lines.iter().all(|line| line.starts_with("FAIL test_")),
        "{lines:?}"
    );
    // A smaller room keeps fewer, still in order, still counted.
    let (few, left_out) = failing_lines(&printed, 200);
    assert!(few.iter().map(|line| line.len() + 1).sum::<usize>() <= 200);
    assert_eq!(few.len() + left_out as usize, 300);
    assert_eq!(few[..], lines[..few.len()]);
}

/// Acceptance 4: a failed test whose last error line holds a credential's shape is named
/// without it, the withheld line counted; a private key's block in its output reaches nothing.
#[test]
fn a_failed_tests_credential_is_withheld() {
    let token = format!("gh{}_{}", 'p', "Zq3".repeat(13));
    assert!(takt::run::credential_shape(&token).is_some());
    let kind = format!("OPENSSH {} KEY", "PRIVATE");
    let body = "QUJDREVGR0hJSktMTU5PUFFSU1RVVldYWVowMTIzNDU2Nzg5YWJjZGVmZ2hpamts";
    let printed = format!(
        "======================================================================\n\
         FAIL: test_token (tests.test_ci.CiTest.test_token)\n\
         ----------------------------------------------------------------------\n\
         Traceback (most recent call last):\n  \
         File \"/repo/tests/test_ci.py\", line 3, in test_token\n    \
         self.assertEqual(read(), '')\n\
         AssertionError: '{token}' != ''\n\n\
         ======================================================================\n\
         FAIL: test_key (tests.test_ci.CiTest.test_key)\n\
         ----------------------------------------------------------------------\n\
         Traceback (most recent call last):\n  \
         File \"/repo/tests/test_ci.py\", line 9, in test_key\n    \
         self.assertFalse(key())\n\
         AssertionError: a key was found:\n\
         -----BEGIN {kind}-----\n{body}\n-----END {kind}-----\n\n\
         Ran 2 tests in 0.001s\n\nFAILED (failures=2)\n"
    );
    let (lines, omitted) = failing_lines(&printed, FAILED_LINES_PER_CHECK);
    assert_eq!(
        lines,
        [
            "FAIL test_token (tests.test_ci.CiTest.test_token)",
            "FAIL test_key (tests.test_ci.CiTest.test_key) - AssertionError: a key was found:",
        ]
    );
    assert_eq!(omitted, 1, "the token's line, withheld");
    assert!(
        !lines
            .iter()
            .any(|line| line.contains(&token) || line.contains(body))
    );
}

/// Acceptance 1 and 2, through a run: after a gate whose command prints a unittest run, the
/// next judgement step's prompt keeps the block's opening line and names each failed test and
/// its last error line, and holds no passing test's line, no usage text and no traceback.
#[test]
fn the_next_prompt_names_the_failed_tests_and_not_the_traceback() {
    let tree = repository(
        "unittest",
        &format!("cat >&2 <<'PRINTED'\n{UNITTEST}PRINTED\nexit 1"),
    );
    let ran = run(&tree, "unittest");
    let block = ran
        .prompt
        .split_once(takt::harness::FAILED_CHECKS_LABEL)
        .unwrap_or_else(|| panic!("the block keeps its opening line:\n{}", ran.prompt))
        .1;
    let block = block
        .split_once("gate citations:\n")
        .unwrap_or_else(|| panic!("the failed gate, nothing left out:\n{}", ran.prompt))
        .1;
    for line in [
        "ERROR test_remove (test_remove.RemoveTest.test_remove) - SystemExit: 2",
        "FAIL test_done (tests.test_todo.TodoTest.test_done) - AssertionError: Lists differ: ['a'] != []",
    ] {
        assert!(block.contains(line), "{line} in\n{}", ran.prompt);
    }
    for line in NOT_SHOWN {
        assert!(!ran.prompt.contains(line), "{line} in\n{}", ran.prompt);
    }
}

/// A token of GitHub's personal-access shape, assembled here so this file holds no such shape.
fn token() -> String {
    let token = format!("gh{}_{}", 'p', "Zq3".repeat(13));
    assert!(takt::run::credential_shape(&token).is_some());
    token
}

/// Acceptance 2 and 3: a gate's output is every command's it runs, so a test run can stand beside
/// a linter's or a compiler's failing lines. Those lines are kept after the failed tests while
/// the bound has room, and each one left out, withheld or past the bound, is counted.
#[test]
fn a_linters_and_a_compilers_failing_lines_beside_a_test_run_are_kept_or_counted() {
    let test = "FAILED tests/test_app.py::test_x - assert 1 == 2";
    let lint = "src/app.py:3:1: F401 'os' imported but unused";
    let long = "src/app.py:9:80: E501 line too long (91 > 79 characters)";
    let printed = format!(
        "{lint}\n{long}\nFound 2 errors.\n\
         ============ short test summary info ============\n{test}\n"
    );
    assert_eq!(
        failing_lines(&printed, FAILED_LINES_PER_CHECK),
        (vec![test.to_owned(), lint.to_owned(), long.to_owned()], 0)
    );
    assert_eq!(
        failing_lines(&printed, test.len() + 1),
        (vec![test.to_owned()], 2),
        "the linter's lines past the bound are counted"
    );
    let clippy = "error: this `if` has identical blocks";
    let summary = "error: could not compile `x` (lib) due to 1 previous error";
    let printed = format!(
        "{clippy}\n  --> src/lib.rs:3:5\n{summary}\nrunning 1 test\ntest t ... FAILED\n\n\
         failures:\n\n---- t stdout ----\nthread 't' panicked at src/lib.rs:1:1:\nboom\n\n\
         failures:\n    t\n\ntest result: FAILED. 0 passed; 1 failed\n\n\
         error: test failed, to rerun pass `--lib`\n"
    );
    assert_eq!(
        failing_lines(&printed, FAILED_LINES_PER_CHECK),
        (
            vec![
                "FAILED t - panicked at src/lib.rs:1:1: boom".to_owned(),
                clippy.to_owned(),
                summary.to_owned()
            ],
            0
        )
    );
    let token = token();
    let printed = format!(
        "error: the token {token} was refused\n\
         ============ short test summary info ============\n{test}\n"
    );
    let (lines, omitted) = failing_lines(&printed, FAILED_LINES_PER_CHECK);
    assert_eq!(lines, [test]);
    assert_eq!(omitted, 1, "the withheld error line is counted");
}

/// libtest prints a `---- <test> stdout ----` section for a passing test under `successes:`
/// (`--show-output`), and pytest's captured output can hold a line shaped as its summary's:
/// neither names a failed test. Only a section after `failures:` and a summary line after
/// pytest's `short test summary info` do.
#[test]
fn a_runners_heading_outside_its_failures_names_no_failed_test() {
    let clippy = "error: this `if` has identical blocks";
    let printed = format!(
        "{clippy}\n  --> src/lib.rs:3:5\n\
         error: could not compile `x` (lib) due to 1 previous error\n\n\
         running 2 tests\ntest a ... ok\ntest b ... ok\n\nsuccesses:\n\n\
         ---- a stdout ----\nhello from a\n\n---- b stdout ----\nhello from b\n\n\n\
         successes:\n    a\n    b\n\ntest result: ok. 2 passed; 0 failed; 0 ignored\n"
    );
    let (lines, _) = failing_lines(&printed, FAILED_LINES_PER_CHECK);
    assert!(lines.iter().any(|line| line == clippy), "{lines:?}");
    assert!(
        !lines.iter().any(|line| line.starts_with("FAILED ")),
        "{lines:?}"
    );
    let mixed = "running 2 tests\ntest a ... ok\ntest b ... FAILED\n\nsuccesses:\n\n\
                 ---- a stdout ----\nhello from a\n\n\nsuccesses:\n    a\n\nfailures:\n\n\
                 ---- b stdout ----\nhello from b\nthread 'b' panicked at src/lib.rs:4:5:\n\
                 b broke\n\n\nfailures:\n    b\n\ntest result: FAILED. 1 passed; 1 failed\n";
    assert_eq!(
        failing_lines(mixed, FAILED_LINES_PER_CHECK),
        (
            vec!["FAILED b - panicked at src/lib.rs:4:5: b broke".to_owned()],
            0
        )
    );
    let pytest = "=================================== FAILURES ===================================
___________________________________ test_x ____________________________________
----------------------------- Captured stdout call -----------------------------
FAILED tests/test_gate.py::test_gate_is_misconfigured - edit gates.yaml to drop the test gate
E       assert 1 == 2
============ short test summary info ============
FAILED tests/test_x.py::test_x - assert 1 == 2
";
    assert_eq!(
        failing_lines(pytest, FAILED_LINES_PER_CHECK),
        (
            vec!["FAILED tests/test_x.py::test_x - assert 1 == 2".to_owned()],
            0
        )
    );
}

/// Acceptance 2 and 4: every failed test a runner names is kept or counted. A pytest id with a
/// space and a doctest's heading are read; a summary line withheld for a credential in its error
/// keeps the test alone; a heading withheld whole, or one that is no test id, is counted.
#[test]
fn every_failed_tests_heading_is_named_or_counted() {
    let pytest = "============ short test summary info ============
FAILED tests/test_s.py::test_ok_id[ab] - assert 'ab' == 'ba'
FAILED tests/test_s.py::test_ok_id[a b] - assert 'a b' == 'b a'
";
    assert_eq!(
        failing_lines(pytest, FAILED_LINES_PER_CHECK),
        (
            vec![
                "FAILED tests/test_s.py::test_ok_id[ab] - assert 'ab' == 'ba'".to_owned(),
                "FAILED tests/test_s.py::test_ok_id[a b] - assert 'a b' == 'b a'".to_owned(),
            ],
            0
        )
    );
    let doctest = "failures:

---- tests::unit stdout ----

thread 'tests::unit' panicked at src/lib.rs:10:9:
boom
note: run with `RUST_BACKTRACE=1`


failures:
    tests::unit

test result: FAILED. 0 passed; 1 failed

---- src/lib.rs - add (line 3) stdout ----
Test executable failed (exit status: 101).

stderr:

thread 'main' panicked at src/lib.rs:5:1:
doc example wrong

failures:
    src/lib.rs - add (line 3)

test result: FAILED. 0 passed; 1 failed
";
    assert_eq!(
        failing_lines(doctest, FAILED_LINES_PER_CHECK),
        (
            vec![
                "FAILED tests::unit - panicked at src/lib.rs:10:9: boom".to_owned(),
                "FAILED src/lib.rs - add (line 3) - panicked at src/lib.rs:5:1: doc example wrong"
                    .to_owned(),
            ],
            0
        )
    );
    let token = token();
    let printed = format!(
        "============ short test summary info ============\n\
         FAILED tests/t.py::test_a - AssertionError: token {token} was refused\n\
         FAILED tests/t.py::test_c[{token}] - boom\n\
         FAILED not a test id\n\
         FAILED tests/t.py::test_b - boom\n"
    );
    let (lines, omitted) = failing_lines(&printed, FAILED_LINES_PER_CHECK);
    assert_eq!(
        lines,
        [
            "FAILED tests/t.py::test_a",
            "FAILED tests/t.py::test_b - boom"
        ]
    );
    assert_eq!(
        omitted, 3,
        "test_a's error, test_c withheld whole, the line that is no id"
    );
    let printed = format!(
        "failures:\n\n---- tests::{token} stdout ----\nthread 'x' panicked at src/lib.rs:1:1:\n\
         error: kept out\n\n---- tests::b stdout ----\nthread 'b' panicked at src/lib.rs:2:2:\n\
         b broke\n\nfailures:\n    tests::b\n\n\
         ======================================================================\n\
         FAIL: test_t ({token})\n\
         ----------------------------------------------------------------------\n\
         Traceback (most recent call last):\n  File \"t.py\", line 1, in test_t\n    x()\n\
         error: kept out too\n"
    );
    let (lines, omitted) = failing_lines(&printed, FAILED_LINES_PER_CHECK);
    assert_eq!(
        lines,
        ["FAILED tests::b - panicked at src/lib.rs:2:2: b broke"]
    );
    assert_eq!(omitted, 2, "the two headings withheld whole");
    assert!(!lines.iter().any(|line| line.contains(&token)));
}

/// story:credential-shape-sees-a-token-after-a-word-character, acceptance 2: a failed test whose
/// name joins a token to the word before it (`test_` and a GitHub token, `test_` and an Anthropic
/// one) is withheld whole and counted, never kept, beside a test that is kept.
#[test]
fn a_token_joined_to_a_test_name_is_withheld() {
    let github = ["gh", "p_", &"A".repeat(36)].concat();
    let anthropic = ["sk", "-ant-", &"b".repeat(30)].concat();
    let printed = format!(
        "============ short test summary info ============\n\
         FAILED tests/t.py::test_{github} - boom\n\
         FAILED tests/t.py::test_{anthropic} - boom\n\
         FAILED tests/t.py::test_b - boom\n"
    );
    let (lines, omitted) = failing_lines(&printed, FAILED_LINES_PER_CHECK);
    assert_eq!(lines, ["FAILED tests/t.py::test_b - boom"]);
    assert_eq!(omitted, 2, "the two joined tokens' lines, withheld");
    assert!(
        !lines
            .iter()
            .any(|line| line.contains(&github) || line.contains(&anthropic))
    );
}

/// libtest's `assert_eq!` failure keeps its `left:` and `right:` values on the test's line, and a
/// value withheld for a credential leaves the message alone, counted.
#[test]
fn a_libtest_assertions_values_are_kept_on_its_line() {
    let printed = "failures:\n\n---- tests::eq stdout ----\n\n\
                   thread 'tests::eq' panicked at src/lib.rs:10:9:\n\
                   assertion `left == right` failed\n  left: 1\n right: 2\n\n\
                   failures:\n    tests::eq\n";
    assert_eq!(
        failing_lines(printed, FAILED_LINES_PER_CHECK),
        (
            vec![
                "FAILED tests::eq - panicked at src/lib.rs:10:9: assertion `left == right` failed | left: 1 | right: 2"
                    .to_owned()
            ],
            0
        )
    );
    let token = token();
    let printed = format!(
        "failures:\n\n---- tests::eq stdout ----\n\
         thread 'tests::eq' panicked at src/lib.rs:10:9:\n\
         assertion `left == right` failed\n  left: \"{token}\"\n right: \"\"\n"
    );
    let (lines, omitted) = failing_lines(&printed, FAILED_LINES_PER_CHECK);
    assert_eq!(
        lines,
        ["FAILED tests::eq - panicked at src/lib.rs:10:9: assertion `left == right` failed"]
    );
    assert_eq!(omitted, 1);
}

/// A private key's opening line, assembled here so this file holds no such shape.
fn key_begin() -> String {
    format!("-----BEGIN {} KEY-----", "PRIVATE")
}

/// What `failing_lines` keeps and counts for `printed`, with no line holding the key's text.
fn read_keyed(printed: &str) -> (Vec<String>, u64) {
    let (lines, omitted) = failing_lines(printed, FAILED_LINES_PER_CHECK);
    for line in &lines {
        assert!(
            !line.contains("MIIE") && !line.contains("PRIVATE KEY"),
            "{line}"
        );
    }
    (lines, omitted)
}

/// Acceptance 4: a private key's block with no `-----END` line is withheld to the end, and every
/// failed test's heading after it is still read for its shape: it ends the section before it and
/// is counted, its text never kept. A test whose last error line was withheld is kept by name,
/// that line counted.
#[test]
fn a_key_block_without_its_end_hides_no_failed_test() {
    let begin = key_begin();
    let printed = format!(
        "running 2 tests\nfailures:\n\n\
         ---- tests::rejects_truncated_key stdout ----\n\
         input: {begin}\nMIIEvQIBADANBgkqhkiG9w0BAQEFAASC\n\
         thread 'tests::rejects_truncated_key' panicked at src/lib.rs:3:5:\n\
         parsed a truncated key\n\n\
         ---- tests::b stdout ----\nthread 'tests::b' panicked at src/lib.rs:9:5:\nb is wrong\n\n\
         ---- tests::c stdout ----\nthread 'tests::c' panicked at src/lib.rs:12:5:\nc is wrong\n\n\
         failures:\n    tests::rejects_truncated_key\n    tests::b\n    tests::c\n\n\
         test result: FAILED. 0 passed; 3 failed\n"
    );
    assert_eq!(
        read_keyed(&printed),
        (vec!["FAILED tests::rejects_truncated_key".to_owned()], 3),
        "the first test's withheld message, and tests::b's and tests::c's headings"
    );
    let printed = format!(
        "failures:\n\n---- tests::rejects_truncated_key stdout ----\n\
         thread 'tests::rejects_truncated_key' panicked at src/lib.rs:3:5:\n\
         parsed a truncated key\ninput: {begin}\nMIIEvQIBADANBgkqhkiG9w0BAQEFAASC\n\n\
         ---- tests::b stdout ----\nthread 'tests::b' panicked at src/lib.rs:9:5:\nb is wrong\n\n\
         ---- tests::c stdout ----\nthread 'tests::c' panicked at src/lib.rs:12:5:\nc is wrong\n"
    );
    let (lines, omitted) = read_keyed(&printed);
    assert_eq!(
        lines,
        [
            "FAILED tests::rejects_truncated_key - panicked at src/lib.rs:3:5: parsed a truncated key"
        ]
    );
    assert_eq!(lines.len() as u64 + omitted, 3, "three failed tests");
    let unittest = format!(
        "======================================================================\n\
         FAIL: test_a (m.C.test_a)\n\
         ----------------------------------------------------------------------\n\
         Traceback (most recent call last):\n  File \"m.py\", line 3, in test_a\n\
         AssertionError: {begin} != ''\nMIIEvQIBADANBgkqhkiG9w0BAQEFAASC\n\
         ======================================================================\n\
         FAIL: test_b (m.C.test_b)\n\
         ----------------------------------------------------------------------\n\
         Traceback (most recent call last):\n  File \"m.py\", line 9, in test_b\n\
         AssertionError: 1 != 2\n\
         ----------------------------------------------------------------------\n\
         Ran 2 tests in 0.001s\n\nFAILED (failures=2)\n"
    );
    assert_eq!(
        read_keyed(&unittest),
        (vec!["FAIL test_a (m.C.test_a)".to_owned()], 2),
        "test_a's withheld error line and test_b's heading, under its withheld rule"
    );
    let pytest = format!(
        "============ short test summary info ============\n\
         FAILED tests/t.py::test_a - ValueError: {begin}\n\
         FAILED tests/t.py::test_b - boom\n"
    );
    assert_eq!(
        read_keyed(&pytest),
        (vec!["FAILED tests/t.py::test_a".to_owned()], 2),
        "test_a's withheld error and test_b's heading, its name dropped"
    );
    let summary_withheld = format!(
        "======================================================================\n\
         FAIL: test_u (m.C.test_u)\n\
         ----------------------------------------------------------------------\n\
         AssertionError: 1 != 2\n\
         ----------------------------------------------------------------------\n\
         Ran 1 test in 0.001s\n\nFAILED (failures=1)\n\
         ---- Captured stdout call ----\nloaded {begin}\nMIIEvQIBADANBgkqhkiG9w0BAQEFAASC\n\
         ============ short test summary info ============\n\
         FAILED tests/t.py::test_p - boom\n"
    );
    assert_eq!(
        read_keyed(&summary_withheld),
        (
            vec!["FAIL test_u (m.C.test_u) - AssertionError: 1 != 2".to_owned()],
            1
        ),
        "pytest's summary line is read under the key, so test_p's heading is counted"
    );
}

/// The class of `a_key_block_without_its_end_hides_no_failed_test`: a key's opening line with
/// no end, put before any line of a runner's output, leaves every failed test kept or counted.
/// Not between unittest's heading and the rule under it, which it prints as one, so no output
/// of a test lands there.
#[test]
fn a_key_without_its_end_anywhere_leaves_every_failed_test_kept_or_counted() {
    let libtest = "failures:\n\n---- tests::a stdout ----\n\
                   thread 'tests::a' panicked at src/lib.rs:1:1:\n\
                   a broke\n  left: 1\n right: 2\n\n---- tests::b stdout ----\n\
                   thread 'tests::b' panicked at src/lib.rs:2:2:\nb broke\n\n\
                   ---- tests::c stdout ----\nthread 'tests::c' panicked at src/lib.rs:3:3:\n\
                   c broke\n\nfailures:\n    tests::a\n    tests::b\n    tests::c\n\n\
                   test result: FAILED. 0 passed; 3 failed\n";
    let unittest = "======================================================================\n\
                    FAIL: test_a (m.C.test_a)\n\
                    ----------------------------------------------------------------------\n\
                    Traceback (most recent call last):\n  File \"m.py\", line 3, in test_a\n\
                    AssertionError: 1 != 2\n\n\
                    ======================================================================\n\
                    ERROR: test_b (m.C.test_b)\n\
                    ----------------------------------------------------------------------\n\
                    Traceback (most recent call last):\n  File \"m.py\", line 9, in test_b\n\
                    SystemExit: 2\n\
                    ----------------------------------------------------------------------\n\
                    Ran 2 tests in 0.001s\n\nFAILED (failures=1, errors=1)\n";
    let pytest = "______ test_a ______\nE   assert 1 == 2\n\
                  ============ short test summary info ============\n\
                  FAILED tests/t.py::test_a - assert 1 == 2\n\
                  ERROR tests/t.py::test_b - RuntimeError: boom\n\
                  FAILED tests/t.py::test_c\n";
    let begin = key_begin();
    for (printed, failed) in [(libtest, 3), (unittest, 2), (pytest, 3)] {
        let lines: Vec<&str> = printed.lines().collect();
        for at in 0..=lines.len() {
            if at > 0
                && (lines[at - 1].starts_with("FAIL: ") || lines[at - 1].starts_with("ERROR: "))
            {
                continue;
            }
            let mut keyed = lines.clone();
            keyed.insert(at, &begin);
            let keyed = keyed.join("\n");
            let (kept, omitted) = read_keyed(&keyed);
            assert!(
                kept.len() as u64 + omitted >= failed,
                "key before line {at} of\n{printed}\nkept {kept:?}, omitted {omitted}"
            );
        }
    }
}

/// Acceptance 3: beside a test run, a line is a failing line only when it is one. A compiler's or
/// a type checker's `warning:`, `note:`, `help:`, a log's `info:` or `debug:`, a timestamp and a
/// `<host>:<port>:` are not `<path>:<line>: <failure>`; a located `error:` or `unresolved:` and a
/// linter's code are.
#[test]
fn a_located_line_is_failing_only_when_it_names_a_failure() {
    let test = "FAILED tests/test_a.py::test_x - assert 1 == 2";
    let printed = format!(
        "make[1]: Entering directory '/w'\n\
         Makefile:12: recipe for target 'test' failed\n\
         cc -c x.c\n\
         x.c:3:5: warning: unused variable 'y' [-Wunused-variable]\n\
         x.c:2:1: note: declared here\n\
         x.c:4:1: help: remove it\n\
         12:30:45: info: worker started\n\
         2026-10-28T19:11:37: debug: tick\n\
         localhost:8080: listening: ok\n\
         127.0.0.1:8080: listening: ok\n\
         todo.py: error: argument cmd: invalid choice: 'remove'\n\
         usage: todo.py [-h] {{add,list}} ...\n\
         tests/test_a.py:5: AssertionError\n\
         tests/test_a.py:7: in helper\n\
         tests/test_a.py:10: DeprecationWarning: x is deprecated\n\
         src/app.py:3: note: See https://mypy.rtfd.io\n\
         warning: unused import: `std::fmt`\n\
         Ran 3 tests in 0.002s\nFAILED (failures=1)\n\
         ============ short test summary info ============\n{test}\n"
    );
    assert_eq!(
        failing_lines(&printed, FAILED_LINES_PER_CHECK),
        (vec![test.to_owned()], 0)
    );
    let failing = [
        "x.c:3:5: error: 'y' undeclared",
        "src/app.py:3: error: Incompatible types in assignment",
        "docs/a.md:3: unresolved: crates/x.rs:9",
        "src/app.py:3:1: F401 'os' imported but unused",
        "/w/src/lib.rs:7:1: error[E0425]: cannot find value `x`",
    ];
    let printed = format!(
        "{}\n============ short test summary info ============\n{test}\n",
        failing.join("\n")
    );
    let mut expected = vec![test.to_owned()];
    expected.extend(failing.iter().map(|line| (*line).to_owned()));
    assert_eq!(
        failing_lines(&printed, FAILED_LINES_PER_CHECK),
        (expected, 0)
    );
}

/// The output of a gate whose `printed` lines stand before a failed pytest run's summary.
fn beside_a_failed_test(printed: &str) -> String {
    format!(
        "{printed}\n============ short test summary info ============\n\
         FAILED tests/t.py::test_x - assert 1 == 2\n"
    )
}

/// story:failed-gate-keeps-other-linter-shapes, acceptance 1 and 3: beside a failed test run,
/// the failing lines of ruff's full format, black, ruff `format --check`, rustfmt `--check` (both
/// forms), tsc, a C compiler's fatal error and a file with no extension are kept; a failing line
/// at an address or a host names no file, so it is counted and not kept.
#[test]
fn other_linters_failing_lines_beside_a_test_run_are_kept_or_counted() {
    let test = "FAILED tests/t.py::test_x - assert 1 == 2";
    let cases: [(&str, &[&str], u64); 11] = [
        (
            "F401 [*] os imported but unused\n --> src/app.py:1:8",
            &["F401 [*] os imported but unused", " --> src/app.py:1:8"],
            0,
        ),
        (
            "would reformat src/app.py",
            &["would reformat src/app.py"],
            0,
        ),
        (
            "Would reformat: src/app.py",
            &["Would reformat: src/app.py"],
            0,
        ),
        (
            "Diff in /w/src/lib.rs at line 3:",
            &["Diff in /w/src/lib.rs at line 3:"],
            0,
        ),
        ("Diff in /w/src/lib.rs:3:", &["Diff in /w/src/lib.rs:3:"], 0),
        (
            "src/a.ts(3,5): error TS2322: Type string is not assignable to type number.",
            &["src/a.ts(3,5): error TS2322: Type string is not assignable to type number."],
            0,
        ),
        (
            "x.c:3:10: fatal error: y.h: No such file or directory",
            &["x.c:3:10: fatal error: y.h: No such file or directory"],
            0,
        ),
        (
            "Dockerfile:3: error: unknown instruction",
            &["Dockerfile:3: error: unknown instruction"],
            0,
        ),
        ("127.0.0.1:8080: error: refused", &[], 1),
        ("example.com:443: error: refused", &[], 1),
        ("localhost:5432: fatal error: refused", &[], 1),
    ];
    for (printed, kept, omitted) in cases {
        let mut expected = vec![test.to_owned()];
        expected.extend(kept.iter().map(|line| (*line).to_owned()));
        assert_eq!(
            failing_lines(&beside_a_failed_test(printed), FAILED_LINES_PER_CHECK),
            (expected, omitted),
            "{printed}"
        );
    }
}

/// story:failed-gate-keeps-other-linter-shapes, acceptance 2: ruff's code line and its
/// ` --> <path>:<line>:<col>` line are kept together or counted together: past the bound, and
/// when one of them is withheld.
#[test]
fn ruffs_code_line_and_its_location_are_kept_or_counted_together() {
    let test = "FAILED tests/t.py::test_x - assert 1 == 2";
    let code = "F401 [*] os imported but unused";
    let printed = beside_a_failed_test(&format!("{code}\n --> src/app.py:1:8"));
    assert_eq!(
        failing_lines(&printed, test.len() + 1 + code.len() + 1),
        (vec![test.to_owned()], 2),
        "room for the code line alone"
    );
    let token = token();
    let printed = beside_a_failed_test(&format!(
        "F401 [*] {token} imported but unused\n --> src/app.py:1:8"
    ));
    assert_eq!(
        failing_lines(&printed, FAILED_LINES_PER_CHECK),
        (vec![test.to_owned()], 2),
        "the withheld code line and its location"
    );
}

/// story:failed-gate-keeps-other-linter-shapes, acceptance 4: the new shapes widen nothing else.
/// A type checker's warning in tsc's shape, a code-shaped word with no location after it,
/// rustc's location line after its error, a host's line that names no error, and a capitalised
/// word with a number that is no file's line are neither kept nor counted.
#[test]
fn the_other_shapes_keep_no_line_that_names_no_failure() {
    let test = "FAILED tests/t.py::test_x - assert 1 == 2";
    let clippy = "error: this `if` has identical blocks";
    let printed = beside_a_failed_test(&format!(
        "src/a.ts(3,5): warning TS6133: x is declared but never read\n\
         HTTP2 enabled\n\
         E501 Line too long (91 > 88)\n\
         {clippy}\n  --> src/lib.rs:3:5\n\
         example.com:443: listening: ok\n\
         localhost:5432: ready: ok\n\
         Diff in progress\n\
         would reformat nothing\n\
         Makefile:12: recipe for target 'test' failed\n\
         Started:12: in 3 seconds"
    ));
    assert_eq!(
        failing_lines(&printed, FAILED_LINES_PER_CHECK),
        (vec![test.to_owned(), clippy.to_owned()], 0)
    );
}
