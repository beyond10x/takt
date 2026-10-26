//! Adversary pass on story:a-red-base-stops-at-open and
//! story:repeated-identical-test-failures-stop-the-run, through the binary, with the harness
//! double of `test_failure_stops_cli.rs` (it counts its calls and edits `work.txt` on each).
//!
//! The trial both stories come from ran takt as a pod that restarted it after every exit 3
//! (story:fresh-home-first-test-run-is-not-refused: "The pod restarted takt four times with the
//! same result"; story:a-harness-step-without-an-action-is-retried: "Each restart by the pod
//! resumed the case"), its first five runs stopped at step 1 before any `tests.run` result, and
//! its failing tests had long names. Each case here builds one of those shapes.

use std::fs;
use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};
use std::process::Command;

/// The fixture stores of the library crate, one directory each.
const FIXTURES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../takt/tests/fixtures");

/// A gates document whose one assertion is true in a repository with a manifest.
const GATES: &str = "format: engineering-gates/1\nlanguage: canon-expr/1\ngates:\n  - id: manifest\n    \
                     text: The repository has its manifest.\n    assertions:\n      \
                     - 'fs.file.exists(\"Cargo.toml\")'\n";

/// The three failing tests story:staging-tests-pass-under-a-shared-temp-directory names, as
/// cargo names them.
const TRIAL: [&str; 3] = [
    "harness::endpoint_tests::staging_writes_a_protected_file_under_the_first_passing_candidate_and_drop_removes_it",
    "harness::endpoint_tests::staging_under_a_linked_ancestor_uses_the_resolved_path",
    "harness::endpoint_tests::a_staged_token_reaches_the_session_as_its_token_file_and_is_recorded_by_name",
];

/// Scenario 1's code under test with `done()` broken: it marks nothing.
fn broken_todo() -> String {
    let todo = fs::read_to_string(Path::new(FIXTURES).join("scenario-1/base/todo.py"))
        .expect("scenario 1's todo.py");
    let marked = "items[number - 1][\"done\"] = True";
    assert!(todo.contains(marked), "scenario 1's done() changed");
    todo.replace(marked, "items[number - 1][\"done\"] = False")
}

/// A test script that passes until a harness step committed, then runs `failing` and exits 101.
fn cargo_script(failing: &str) -> String {
    format!(
        r#"#!/bin/sh
if git log -1 --format=%s | grep -q "^implementation.edit"; then
  n=$(git rev-list --count HEAD)
  {failing}
  exit 101
fi
printf 'running 3 tests\ntest result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s\n'
"#
    )
}

/// The same two short-named tests fail at every revision.
const SAME_TWO: &str = r"printf 'running 3 tests\ntest a::one ... ok\ntest a::two ... FAILED\ntest a::three ... FAILED\n\ntest result: FAILED. 1 passed; 2 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s\n\nerror: test failed, to rerun pass `--lib`\n'";

/// What `cargo test --quiet --locked -p b10x-takt --lib` printed in the trial: no `... FAILED`
/// lines, the names under the last `failures:` heading, the rerun hint on standard error.
fn trial_failing() -> String {
    let mut snippet =
        String::from("printf '\\nrunning 130 tests\\n....F....F.....F\\n\\nfailures:\\n\\n");
    for name in TRIAL {
        snippet.push_str(&format!(
            "---- {name} stdout ----\\nthread panicked: refused\\n\\n"
        ));
    }
    snippet.push_str("\\nfailures:\\n");
    for name in TRIAL {
        snippet.push_str(&format!("    {name}\\n"));
    }
    snippet.push_str("\\ntest result: FAILED. 127 passed; 3 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.20s\\n\\n'\n  ");
    snippet.push_str("printf 'error: test failed, to rerun pass `-p b10x-takt --lib`\\n' >&2");
    snippet
}

fn git(tree: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .args(["-c", "user.name=t", "-c", "user.email=t@localhost"])
        .args(args)
        .current_dir(tree)
        .output()
        .expect("git runs");
    assert!(output.status.success(), "git {args:?}");
    String::from_utf8_lossy(&output.stdout).trim().to_owned()
}

struct Scratch(PathBuf);

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn copy_dir(from: &Path, to: &Path) {
    fs::create_dir_all(to).expect("a directory");
    for entry in fs::read_dir(from).expect("a readable fixture") {
        let entry = entry.expect("an entry");
        let target = to.join(entry.file_name());
        if entry.file_type().expect("a file type").is_dir() {
            copy_dir(&entry.path(), &target);
        } else {
            fs::copy(entry.path(), &target).expect("a copied file");
        }
    }
}

struct World {
    _scratch: Scratch,
    repo: PathBuf,
    bin: PathBuf,
    calls: PathBuf,
    state: PathBuf,
}

/// The world of `test_failure_stops_cli.rs`: the fixture store, its one story `proposed`, a
/// manifest, a gates document, `files`, scenario 1 declared on the commit that holds them, and a
/// `claude` double that counts its calls and edits `work.txt`.
fn world(name: &str, files: &[(&str, &str, bool)]) -> World {
    let outer = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!(
        "test-failure-stops-adv1-{}-{name}",
        std::process::id()
    ));
    if outer.exists() {
        fs::remove_dir_all(&outer).expect("a stale scratch tree is removed");
    }
    let repo = outer.join("repo");
    copy_dir(&Path::new(FIXTURES).join("one-ready"), &repo);
    fs::write(repo.join("Cargo.toml"), "[package]\nname = \"scratch\"\n").expect("a file");
    fs::write(repo.join(".engineering/gates.yaml"), GATES).expect("a file");
    fs::write(repo.join(".gitignore"), "__pycache__/\n").expect("a file");
    for (path, text, executable) in files {
        let at = repo.join(path);
        fs::create_dir_all(at.parent().expect("a parent")).expect("a directory");
        fs::write(&at, text).expect("a file");
        if *executable {
            fs::set_permissions(&at, fs::Permissions::from_mode(0o755)).expect("executable");
        }
    }
    git(&repo, &["init", "-q", "-b", "main"]);
    git(&repo, &["add", "-A"]);
    git(&repo, &["commit", "-q", "-m", "seed"]);
    let seed = git(&repo, &["rev-parse", "HEAD"]);
    fs::write(
        repo.join(".engineering/scenarios.conf"),
        format!("[scenario \"1\"]\n\tstory = story:ready-one\n\tseed = {seed}\n"),
    )
    .expect("the declaration is written");
    git(&repo, &["add", ".engineering/scenarios.conf"]);
    git(&repo, &["commit", "-q", "-m", "declare the scenarios"]);
    let bin = outer.join("stub-bin");
    fs::create_dir_all(&bin).expect("a stub directory");
    let calls = outer.join("calls");
    let state = outer.join("state");
    fs::create_dir_all(&state).expect("a state directory");
    let path = bin.join("claude");
    fs::write(
        &path,
        format!(
            r#"#!/bin/sh
echo call >> '{}'
echo "the work" >> work.txt
printf '{{"result":"{{\\"action\\": \\"implementation.edit\\", \\"summary\\": \\"the double\\"}}","usage":{{"input_tokens":1}}}}'
"#,
            calls.display()
        ),
    )
    .expect("the double is written");
    fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).expect("executable");
    World {
        _scratch: Scratch(outer),
        repo,
        bin,
        calls,
        state,
    }
}

impl World {
    /// Runs `takt <args>` with the double first on the program path and the state home in the
    /// scratch world. The exit code, stdout and stderr.
    fn takt(&self, args: &[&str]) -> (i32, String, String) {
        let output = Command::new(env!("CARGO_BIN_EXE_takt"))
            .args(args)
            .env_remove("B10X_GATES_POLICY")
            .env("XDG_STATE_HOME", self.state.join("xdg-state"))
            .env(
                "PATH",
                format!(
                    "{}:{}",
                    self.bin.display(),
                    std::env::var("PATH").unwrap_or_default()
                ),
            )
            .output()
            .expect("takt runs");
        (
            output.status.code().expect("an exit code"),
            String::from_utf8_lossy(&output.stdout).into_owned(),
            String::from_utf8_lossy(&output.stderr).into_owned(),
        )
    }

    fn repo(&self) -> &str {
        self.repo.to_str().expect("utf-8")
    }

    /// `takt scenario run` of scenario 1, no forge, with `extra` flags.
    fn scenario(&self, test_cmd: &str, extra: &[&str]) -> (i32, String, String) {
        let mut args = vec![
            "scenario",
            "run",
            "--repo",
            self.repo(),
            "1",
            "--test-cmd",
            test_cmd,
            "--forge",
            "none",
        ];
        args.extend_from_slice(extra);
        self.takt(&args)
    }

    /// `takt run --case <case>`: a resume, as a restarted pod makes it, with `extra` flags.
    fn resume(&self, case: &str, test_cmd: &str, extra: &[&str]) -> (i32, String, String) {
        let mut args = vec![
            "run",
            "--repo",
            self.repo(),
            "--case",
            case,
            "--test-cmd",
            test_cmd,
            "--forge",
            "none",
            "--min-free-gib",
            "0",
        ];
        args.extend_from_slice(extra);
        self.takt(&args)
    }

    fn calls(&self) -> usize {
        fs::read_to_string(&self.calls)
            .unwrap_or_default()
            .lines()
            .count()
    }
}

/// The case a run names on its `case <id> on` line.
fn case_of(out: &str) -> String {
    out.lines()
        .find_map(|line| {
            line.strip_prefix("case ")?
                .split_once(" on ")
                .map(|(id, _)| id)
        })
        .unwrap_or_else(|| panic!("the run names no case: {out}"))
        .to_owned()
}

fn stop_line(out: &str, err: &str) -> String {
    out.lines()
        .find(|line| line.starts_with("stopped: "))
        .unwrap_or_else(|| panic!("no stop line: stdout {out} stderr {err}"))
        .to_owned()
}

fn scenario_1(todo: &str) -> Vec<(&'static str, String, bool)> {
    let base = Path::new(FIXTURES).join("scenario-1/base");
    vec![
        (
            "check.sh",
            fs::read_to_string(base.join("check.sh")).expect("check.sh"),
            true,
        ),
        ("todo.py", todo.to_owned(), true),
        (
            "tests/test_todo.py",
            fs::read_to_string(base.join("tests/test_todo.py")).expect("the tests"),
            false,
        ),
    ]
}

fn borrowed<'f>(files: &'f [(&'static str, String, bool)]) -> Vec<(&'static str, &'f str, bool)> {
    files
        .iter()
        .map(|(path, text, executable)| (*path, text.as_str(), *executable))
        .collect()
}

/// The trial's own failing set, at every revision after the first edit: two results in a row
/// fail the same three tests, so the run stops `RepeatedTestFailure` after the second harness
/// call, naming them, instead of editing on until its step budget.
#[test]
fn the_trials_failing_set_stops_the_run() {
    let script = cargo_script(&trial_failing());
    let world = world("trial-set", &[("t.sh", &script, true)]);
    let (code, out, err) = world.scenario("./t.sh", &["--iterations", "24", "--min-free-gib", "0"]);
    let stop = stop_line(&out, &err);
    assert!(
        stop.starts_with("stopped: RepeatedTestFailure (2 "),
        "{stop}; harness calls {} (story:repeated-identical-test-failures-stop-the-run: 2 in a row stop)",
        world.calls()
    );
    assert!(stop.contains(TRIAL[0]), "{stop}");
    assert_eq!(
        world.calls(),
        2,
        "story:repeated-identical-test-failures-stop-the-run: 2 in a row stop: {out}"
    );
    assert_eq!(code, 3, "stdout {out} stderr {err}");
}

/// The case's first `tests.run`, at its unchanged base, in a run after one that stopped before
/// any `tests.run` result (the trial's runs 1 to 5 stopped at step 1): it is still the case's
/// first test at a revision no step changed, so the run stops `RedBase` with no harness call.
#[test]
fn a_red_base_first_tested_in_a_later_run_stops_before_the_harness() {
    let files = scenario_1(&broken_todo());
    let world = world("red-later", &borrowed(&files));
    let (code, out, err) = world.scenario(
        "./check.sh",
        &["--iterations", "20", "--min-free-gib", "1000000000"],
    );
    assert!(
        out.contains("stopped: DiskLow"),
        "the first run stops before its first step: stdout {out} stderr {err}"
    );
    assert!(!out.contains("evidence: test_result"), "{out}");
    assert_eq!(code, 8, "stdout {out} stderr {err}");
    let case = case_of(&out);
    let (code, out, err) = world.resume(&case, "./check.sh", &["--iterations", "20"]);
    assert!(
        out.contains("evidence: test_result fail"),
        "the resume ran the tests: stdout {out} stderr {err}"
    );
    let stop = stop_line(&out, &err);
    assert_eq!(
        world.calls(),
        0,
        "a model call at a red base no step had changed; the run stopped {stop}"
    );
    assert!(stop.starts_with("stopped: RedBase ("), "{stop}");
    assert_eq!(code, 3, "stdout {out} stderr {err}");
}

/// A pod restarts takt after `RedBase` (exit 3) and the restart resumes the case, nothing
/// changed and nobody having answered: the base is as red as it was, and no step has changed
/// it, so no model call is spent; only `--continue-past-red-base` goes on.
#[test]
fn a_restart_after_red_base_spends_no_model_call() {
    let files = scenario_1(&broken_todo());
    let world = world("red-restart", &borrowed(&files));
    let (code, out, err) =
        world.scenario("./check.sh", &["--iterations", "20", "--min-free-gib", "0"]);
    assert!(
        stop_line(&out, &err).starts_with("stopped: RedBase ("),
        "{out}"
    );
    assert_eq!(code, 3, "stdout {out} stderr {err}");
    assert_eq!(world.calls(), 0, "{out}");
    let case = case_of(&out);
    let (_, out, err) = world.resume(&case, "./check.sh", &["--iterations", "20"]);
    let stop = stop_line(&out, &err);
    assert_eq!(
        world.calls(),
        0,
        "the restart called the harness at the unchanged red base; it stopped {stop}"
    );
}

/// The opt-out reaches a resumed case: after `RedBase`, `--continue-past-red-base` goes on to
/// the harness.
#[test]
fn a_resume_with_the_flag_goes_on_past_red_base() {
    let files = scenario_1(&broken_todo());
    let world = world("red-flag", &borrowed(&files));
    let (code, out, err) =
        world.scenario("./check.sh", &["--iterations", "20", "--min-free-gib", "0"]);
    assert!(
        stop_line(&out, &err).starts_with("stopped: RedBase ("),
        "{out}"
    );
    assert_eq!(code, 3, "stdout {out} stderr {err}");
    let case = case_of(&out);
    let (_, out, err) = world.resume(
        &case,
        "./check.sh",
        &["--iterations", "3", "--continue-past-red-base"],
    );
    assert!(world.calls() >= 1, "stdout {out} stderr {err}");
}

/// A pod restarts takt after `RepeatedTestFailure` (exit 3) and the restart resumes the case:
/// the case's last two results failed the same tests at two revisions and nothing has
/// changed since, so the restart spends no further harness call before stopping again.
#[test]
fn a_restart_after_repeated_failures_spends_no_further_model_call() {
    let script = cargo_script(SAME_TWO);
    let world = world("repeat-restart", &[("t.sh", &script, true)]);
    let (code, out, err) = world.scenario("./t.sh", &["--iterations", "40", "--min-free-gib", "0"]);
    assert!(
        stop_line(&out, &err).starts_with("stopped: RepeatedTestFailure (2 "),
        "story:repeated-identical-test-failures-stop-the-run: 2 in a row stop: {out}"
    );
    assert_eq!(code, 3, "stdout {out} stderr {err}");
    let before = world.calls();
    assert_eq!(
        before, 2,
        "story:repeated-identical-test-failures-stop-the-run: 2 in a row stop: {out}"
    );
    let case = case_of(&out);
    let (code, out, err) = world.resume(&case, "./t.sh", &["--iterations", "40"]);
    let stop = stop_line(&out, &err);
    assert_eq!(
        world.calls() - before,
        0,
        "harness calls in the restarted run before it stopped {stop} (exit {code}); stdout {out} stderr {err}"
    );
}
