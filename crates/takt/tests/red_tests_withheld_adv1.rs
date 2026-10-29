//! Adversary, wave 43 U5 (story:red-tests-block-withholds-per-line): the red tests' block a
//! judgement step's prompt opens with, driven through a real run (`Run::open`, `TestsRun`, the
//! run's change scan), not through the private builder alone. The test command prints libtest's
//! report naming the tree by its absolute path; the scan is a script that refuses fixed strings,
//! as the gates policy's `personal-paths` rule refuses a home path. No live `claude` runs: the
//! harness program keeps each prompt it is given and acts on what the prompt offers.

#[path = "support/own_home.rs"]
mod own_home;

use std::fs;
use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::rc::Rc;

use takt::evidence::Record;
use takt::forge::{Checks, Forge, Memory};
use takt::harness::{ClaudeCode, RED_TESTS_ASK, RED_TESTS_LABEL, TESTS_PATH_LABEL};
use takt::run::ChangeScan;
use takt::validator::{ChangeRead, CiRead, Observation, PrOpen, TestsRun, Validator};
use takt::{Run, RunOptions};

/// The failing test whose assertion is in the tree, printed by its absolute path.
const KEPT_TEST: &str = "remove::a_todo_is_removed_by_its_number";

/// The failing test whose error line the scan refuses.
const HIDDEN_TEST: &str = "remove::hidden";

/// What the scan refuses besides the tree's path.
const REFUSED: &str = "REFUSED-TEXT";

/// `story.delivery/2`'s `change.handover` precondition at the locked tag, and the same guarded
/// on the tests (as `handover_after_red.rs` runs it), so a red revision is offered the edit.
const HANDOVER_TODAY: &str = "    precondition:
      claim: change.handed_over
      is: unknown
    may_produce:
      - evidence: handover
";
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
        .join(format!("red-withheld-adv1-{}-{name}", std::process::id()));
    if dir.exists() {
        fs::remove_dir_all(&dir).expect("a stale scratch directory is removed");
    }
    fs::create_dir_all(&dir).expect("a scratch directory");
    Scratch(dir.canonicalize().expect("canonical"))
}

/// The test command: prints `report` (with `@TREE@` replaced by the tree's absolute path) and
/// fails while the tree holds the story's tests and not its implementation; passes otherwise.
fn test_command(report: &str, tree: &Path) -> String {
    let report = report.replace("@TREE@", &tree.display().to_string());
    format!(
        "#!/bin/sh\nif [ -f tests/remove_test.txt ] && [ ! -f src/remove.txt ]; then\n\
         cat <<'OUT'\n{report}OUT\nexit 101\nfi\n\
         echo 'running 1 test'\n\
         echo 'test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out'\n"
    )
}

fn repository(name: &str, report: &str) -> Scratch {
    let tree = directory(name);
    fs::write(tree.join("README.md"), "a story\n").expect("a file");
    let script = tree.join("tests.sh");
    fs::write(&script, test_command(report, &tree)).expect("the test command");
    fs::set_permissions(&script, fs::Permissions::from_mode(0o755)).expect("exec bit");
    git(&tree, &["init", "-q", "-b", "main"]);
    git(&tree, &["add", "-A"]);
    git(&tree, &["commit", "-q", "-m", "start"]);
    tree
}

/// A fake `claude` in `dir`, as `handover_after_red.rs`'s: keeps each prompt as
/// `dir/prompt-<n>`; writes the tests, then the implementation, then requests the merge.
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
elif [ ! -f src/remove.txt ] && offers implementation.edit; then
  mkdir -p src && echo 'removed' > src/remove.txt
  reply implementation.edit 'implemented the removal'
elif offers change.handover; then
  reply change.handover 'the change is complete'
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

/// The scan a judgement step's prompt reads is the environment's (`ChangeScan::from_env`, in
/// `ClaudeCode`'s step), not the run's `scan_change_with`: a `b10x-gates` on `PATH` under the
/// policy `B10X_GATES_POLICY` names. This one refuses a file holding any line of
/// `<repo>.refused`, a fixed string each, where `<repo>` is its `--repo`; with no such file it
/// passes. Called once, by the one test of this binary, before any run starts a thread.
fn install_scan() -> Scratch {
    let dir = directory("bin");
    let program = dir.join("b10x-gates");
    fs::write(
        &program,
        "#!/bin/sh\nfor last; do :; done\n\
         [ \"$1\" = scan-text ] || exit 2\n\
         [ -s \"$3.refused\" ] || exit 0\n\
         if grep -qFf \"$3.refused\" \"$last\"; then exit 1; fi\nexit 0\n",
    )
    .expect("a fake scan");
    fs::set_permissions(&program, fs::Permissions::from_mode(0o755)).expect("exec bit");
    let policy = dir.join("policy.json");
    fs::write(&policy, "{}\n").expect("a policy");
    let path = std::env::var_os("PATH").unwrap_or_default();
    let mut dirs = vec![dir.to_path_buf()];
    dirs.extend(std::env::split_paths(&path));
    let joined = std::env::join_paths(dirs).expect("a PATH");
    // SAFETY: this binary holds one test; no other thread reads the environment while it is set.
    unsafe {
        std::env::set_var("PATH", joined);
        std::env::set_var(takt::scan::gates::POLICY_VAR, &policy);
    }
    assert_eq!(
        ChangeScan::from_env().map(|scan| scan.policy),
        Some(policy),
        "the environment names the fake scan's policy"
    );
    dir
}

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

/// The prompts a delivery run gave while `tests.pass` was false, the tree it ran in, and what
/// it printed. `refused(tree)` is what the scan refuses (none: the scan cannot run).
fn red_prompts(
    name: &str,
    report: &str,
    refused: impl Fn(&Path) -> Option<Vec<String>>,
) -> (Vec<String>, Scratch, String) {
    let tree = repository(name, report);
    let fake = directory(&format!("{name}-claude"));
    let harness = fake_claude(&fake);
    let file = fake.join("protocol.yaml");
    fs::write(&file, story_delivery()).expect("the protocol file");
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
        inputs: [("tests_first", "required"), ("gate_on", "after_handover")]
            .iter()
            .map(|(name, value)| ((*name).to_owned(), (*value).to_owned()))
            .collect(),
        iterations: Some(20),
        hours: None,
        tokens: None,
        case_id: Some(name.to_owned()),
        gate: Some("repository.merge".to_owned()),
    };
    if let Some(refused) = refused(&tree) {
        fs::write(
            format!("{}.refused", tree.display()),
            format!("{}\n", refused.join("\n")),
        )
        .expect("the refused strings");
    }
    let mut run = Run::open(&tree, options, &harness, &validators).expect("the case opens");
    let mut out = Vec::new();
    let stop = run.run(&mut out).map_err(|error| format!("{error:#}"));
    drop(run);
    let text = format!("{stop:?}\n{}", String::from_utf8_lossy(&out));
    let calls: usize = fs::read_to_string(fake.join("calls"))
        .map(|calls| calls.trim().parse().expect("a count"))
        .unwrap_or(0);
    let prompts = (1..=calls)
        .map(|n| fs::read_to_string(fake.join(format!("prompt-{n}"))).expect("a prompt"))
        .filter(|prompt| prompt.contains("- tests.pass: false\n"))
        .collect();
    (prompts, tree, text)
}

/// The block before `Intent:`: what the prompt opens with.
fn opening(prompt: &str) -> &str {
    &prompt[..prompt.find("Intent:").expect("the intent")]
}

/// libtest's report of two failing tests: [`KEPT_TEST`] panicking at the tree's absolute path,
/// [`HIDDEN_TEST`] whose message the scan refuses.
const TWO_FAILING: &str = "running 2 tests
test remove::a_todo_is_removed_by_its_number ... FAILED
test remove::hidden ... FAILED

failures:

---- remove::a_todo_is_removed_by_its_number stdout ----

thread 'remove::a_todo_is_removed_by_its_number' panicked at @TREE@/tests/remove.rs:7:5:
assertion failed: removed

---- remove::hidden stdout ----

thread 'remove::hidden' panicked at tests/remove.rs:9:5:
REFUSED-TEXT

failures:
    remove::a_todo_is_removed_by_its_number
    remove::hidden

test result: FAILED. 0 passed; 2 failed; 0 ignored; 0 measured; 0 filtered out
";

/// Acceptance 2, through a run: the red step's prompt opens with the failing test at a path
/// relative to the tree and its assertion's location, the refused line withheld and counted,
/// and neither the tree's path nor the refused text anywhere in the prompt.
fn a_run_opens_the_red_step_with_relative_paths_and_one_line_withheld() {
    let (prompts, tree, text) = red_prompts("accept", TWO_FAILING, |tree| {
        Some(vec![tree.display().to_string(), REFUSED.to_owned()])
    });
    assert!(!prompts.is_empty(), "no red prompt:\n{text}");
    let root = tree.display().to_string();
    for prompt in &prompts {
        let open = opening(prompt);
        assert!(open.starts_with("You are the judgement step"), "{prompt}");
        assert!(open.contains(&format!("{RED_TESTS_LABEL}: ")), "{prompt}");
        assert!(
            open.contains("tests.run (1 line withheld: it did not pass the secret scan):"),
            "{prompt}"
        );
        assert!(
            open.contains(&format!("- {KEPT_TEST} at tests/remove.rs:7:5")),
            "{prompt}"
        );
        assert!(open.contains(TESTS_PATH_LABEL), "{prompt}");
        assert!(open.contains(RED_TESTS_ASK), "{prompt}");
        assert!(!prompt.contains(HIDDEN_TEST), "{prompt}");
        assert!(!prompt.contains(REFUSED), "{prompt}");
        assert!(!prompt.contains(&root), "{prompt}");
    }
}

/// libtest's report of one failing test, in a directory beside the tree whose name starts
/// with the tree's: `<tree>2` is not under `<tree>`.
const SIBLING: &str = "running 2 tests
test remove::a_todo_is_removed_by_its_number ... FAILED
test remove::beside ... FAILED

failures:

---- remove::a_todo_is_removed_by_its_number stdout ----

thread 'remove::a_todo_is_removed_by_its_number' panicked at @TREE@/tests/remove.rs:7:5:
assertion failed: removed

---- remove::beside stdout ----

thread 'remove::beside' panicked at @TREE@2/src/lib.rs:3:1:
assertion failed: beside

failures:
    remove::a_todo_is_removed_by_its_number
    remove::beside

test result: FAILED. 0 passed; 2 failed; 0 ignored; 0 measured; 0 filtered out
";

/// A path in a directory whose name extends the tree's is not under the tree: it is not written
/// as `2/src/lib.rs`, and, since the scan refuses it, it is withheld while the other is shown.
fn a_sibling_directory_named_after_the_tree_is_not_made_relative() {
    let (prompts, tree, text) = red_prompts("sibling", SIBLING, |tree| {
        Some(vec![tree.display().to_string()])
    });
    assert!(!prompts.is_empty(), "no red prompt:\n{text}");
    let root = tree.display().to_string();
    for prompt in &prompts {
        let open = opening(prompt);
        assert!(
            open.contains("tests.run (1 line withheld: it did not pass the secret scan):"),
            "{prompt}"
        );
        assert!(
            open.contains(&format!("- {KEPT_TEST} at tests/remove.rs:7:5")),
            "{prompt}"
        );
        assert!(!prompt.contains("2/src/lib.rs"), "{prompt}");
        assert!(!prompt.contains("remove::beside"), "{prompt}");
        assert!(!prompt.contains(&root), "{prompt}");
    }
}

/// Every line refused: the block is the one line saying the test run failed and that it did
/// not pass the scan, then the ask; no heading of a check with nothing in it.
fn every_line_refused_leaves_the_one_line() {
    let (prompts, _tree, text) =
        red_prompts("all", TWO_FAILING, |_| Some(vec!["remove::".to_owned()]));
    assert!(!prompts.is_empty(), "no red prompt:\n{text}");
    for prompt in &prompts {
        let open = opening(prompt);
        assert!(
            open.contains(&format!(
                "{RED_TESTS_LABEL}; what the test command printed is not shown: it did not pass \
                 the secret scan. {RED_TESTS_ASK}\n"
            )),
            "{prompt}"
        );
        assert!(!open.contains("tests.run ("), "{prompt}");
        assert!(!open.contains(TESTS_PATH_LABEL), "{prompt}");
    }
}

/// The one test of this binary: the environment's scan is installed once, then each scenario
/// runs in turn; each failing scenario is named, and its panic printed above.
#[test]
fn the_red_tests_block_through_a_run() {
    let _bin = install_scan();
    let scenarios: [(&str, fn()); 3] = [
        (
            "a_run_opens_the_red_step_with_relative_paths_and_one_line_withheld",
            a_run_opens_the_red_step_with_relative_paths_and_one_line_withheld,
        ),
        (
            "a_sibling_directory_named_after_the_tree_is_not_made_relative",
            a_sibling_directory_named_after_the_tree_is_not_made_relative,
        ),
        (
            "every_line_refused_leaves_the_one_line",
            every_line_refused_leaves_the_one_line,
        ),
    ];
    let failed: Vec<&str> = scenarios
        .iter()
        .filter(|(_, scenario)| std::panic::catch_unwind(*scenario).is_err())
        .map(|(name, _)| *name)
        .collect();
    assert!(failed.is_empty(), "scenarios failed: {failed:?}");
}
