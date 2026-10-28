//! story:edits-keep-ess-citations-resolved: a failed `gate.run` keeps, per failed gate, the
//! lines its command printed that name what failed, bounded in bytes with the count of lines
//! left out; the run record's evidence line of the gate's record keeps the same lines, and the
//! next judgement step's prompt shows them under the failed gate's id. A gate that passes adds
//! nothing. No live `claude` runs: the harness program is a script that keeps each prompt it is
//! given.

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
                "FAILED tests::removes - assertion `left == right` failed".to_owned(),
                "FAILED tests::lists - no todo file".to_owned(),
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
