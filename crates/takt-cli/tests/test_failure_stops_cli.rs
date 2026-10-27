//! Two stops on `tests.run` results, through the binary, with a harness double on the program
//! path that counts its calls and edits `work.txt` at every call; nothing reaches a network or a
//! model.
//!
//! story:a-red-base-stops-at-open: on scenario 1 with `done()` broken and committed at the seed,
//! the run stops `RedBase` before the first harness step, exit 3, naming
//! `test_done_marks_one_item`, with 0 tokens; on the unbroken seed the run goes on to the harness.
//!
//! story:repeated-identical-test-failures-stop-the-run: two consecutive `tests.run` results
//! that fail the same tests at two revisions stop the run `RepeatedTestFailure`, exit 3, naming
//! the tests and the count, with no harness call after the second; a failing set that changes, or
//! one the output does not name, does not stop it.

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

/// Scenario 1's code under test with `done()` broken: it marks nothing.
fn broken_todo() -> String {
    let todo = fs::read_to_string(Path::new(FIXTURES).join("scenario-1/base/todo.py"))
        .expect("scenario 1's todo.py");
    let marked = "items[number - 1][\"done\"] = True";
    assert!(todo.contains(marked), "scenario 1's done() changed");
    todo.replace(marked, "items[number - 1][\"done\"] = False")
}

/// A test script that passes until a harness step committed (the run pins `--commit-style action`, so
/// the subject of `HEAD` opens with
/// `implementation.edit`; the script names no file the step writes, which would put it on the
/// judge surface), then prints a cargo run whose failed tests are `failing` (none: a failure
/// that names no test) and exits 101; `$n` is the number of commits.
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

/// The same two tests fail at every revision.
const SAME_TWO: &str = r"printf 'running 3 tests\ntest a::one ... ok\ntest a::two ... FAILED\ntest a::three ... FAILED\n\ntest result: FAILED. 1 passed; 2 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s\n\nerror: test failed, to rerun pass `--lib`\n'";

/// A test named by the number of commits fails: another one at every revision.
const ANOTHER_EACH_TIME: &str = r"printf 'running 2 tests\ntest a::one ... ok\ntest a::case_%s ... FAILED\n\ntest result: FAILED. 1 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s\n' $n";

/// A failure whose output names no test.
const UNNAMED: &str = r"echo 'the runner crashed'";

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

/// A scratch directory under this crate's build directory, removed on drop.
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

/// The scratch world: a repository with the fixture store, its one story `proposed`, a manifest,
/// a gates document, `files` (path, text, executable) and scenario 1 declared on the commit that
/// holds them, no origin; a `claude` double in `bin` that appends a line to `calls` and one to
/// `work.txt` on every call and proposes `implementation.edit`.
struct World {
    _scratch: Scratch,
    repo: PathBuf,
    bin: PathBuf,
    calls: PathBuf,
    state: PathBuf,
}

fn world(name: &str, files: &[(&str, &str, bool)]) -> World {
    let outer = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("test-failure-stops-{}-{name}", std::process::id()));
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
case $1 in --version) echo '2.1.309 (Claude Code)'; exit 0 ;; esac
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
    /// Runs `takt scenario run` of scenario 1 with the test command `test_cmd`, no forge and
    /// `iterations` steps: the double first on the program path, the state home in the scratch
    /// world. The exit code, stdout and stderr.
    fn scenario(&self, test_cmd: &str, iterations: &str) -> (i32, String, String) {
        self.scenario_with(test_cmd, iterations, &[])
    }

    /// As [`World::scenario`], with `extra` arguments after the others.
    fn scenario_with(
        &self,
        test_cmd: &str,
        iterations: &str,
        extra: &[&str],
    ) -> (i32, String, String) {
        let repo = self.repo.to_str().expect("utf-8");
        let mut args = vec![
            "scenario",
            "run",
            "--repo",
            repo,
            "1",
            "--test-cmd",
            test_cmd,
            "--forge",
            "none",
            "--commit-style",
            "action",
            "--iterations",
            iterations,
        ];
        args.extend_from_slice(extra);
        self.takt(&args)
    }

    /// `takt run --case <case>`, a resume as a restarted pod makes it, with the test command
    /// `test_cmd`, no forge and `iterations` steps.
    fn resume(&self, case: &str, test_cmd: &str, iterations: &str) -> (i32, String, String) {
        self.takt(&[
            "run",
            "--repo",
            self.repo.to_str().expect("utf-8"),
            "--case",
            case,
            "--test-cmd",
            test_cmd,
            "--forge",
            "none",
            "--commit-style",
            "action",
            "--iterations",
            iterations,
        ])
    }

    /// Runs takt with `args`: the double first on the program path, the state home in the
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

    /// The harness double's calls so far.
    fn calls(&self) -> usize {
        fs::read_to_string(&self.calls)
            .unwrap_or_default()
            .lines()
            .count()
    }
}

/// Scenario 1 as its seed holds it, with `todo.py` given.
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

/// story:a-red-base-stops-at-open's acceptance: scenario 1 with `done()` broken and committed at
/// the seed stops before the first harness step, exit 3, naming `test_done_marks_one_item`, with
/// 0 tokens; the failure is recorded as the case's test result.
#[test]
fn a_red_base_stops_before_the_first_harness_step() {
    let files = scenario_1(&broken_todo());
    let world = world("red-base", &borrowed(&files));
    let (code, out, err) = world.scenario("./check.sh", "20");
    let stop = out
        .lines()
        .find(|line| line.starts_with("stopped: "))
        .unwrap_or_else(|| panic!("no stop line: stdout {out} stderr {err}"));
    assert!(
        stop.starts_with("stopped: RedBase ("),
        "{stop}\n{out}\n{err}"
    );
    assert!(stop.contains("test_done_marks_one_item"), "{stop}");
    assert!(stop.contains("tokens 0,"), "{stop}");
    assert!(
        out.contains("evidence: test_result fail"),
        "the failure is recorded: {out}"
    );
    assert_eq!(world.calls(), 0, "a model call at a red base: {out}");
    assert_eq!(code, 3, "stdout {out} stderr {err}");
}

/// The opt-out: `takt scenario run` by its flag, `takt run` by its configuration file's
/// `continue_past_red_base` (TOML or a `loom.agent/1` document): the red base is recorded, and
/// the run goes on to the harness.
#[test]
fn a_run_goes_on_past_a_red_base_by_flag_or_configuration() {
    let files = scenario_1(&broken_todo());
    for by in ["flag", "toml", "yaml"] {
        let world = world(&format!("red-base-{by}"), &borrowed(&files));
        let config = world.state.join(format!("run.{by}"));
        let repo = world.repo.display().to_string();
        let config_run = |text: &str| {
            fs::write(&config, text).expect("a file");
            world.takt(&[
                "run",
                "--repo",
                &repo,
                "--story",
                "story:ready-one",
                "--config",
                config.to_str().expect("utf-8"),
                "--test-cmd",
                "./check.sh",
                "--forge",
                "none",
                "--iterations",
                "3",
            ])
        };
        let (_, out, err) = match by {
            "flag" => world.scenario_with("./check.sh", "3", &["--continue-past-red-base"]),
            "toml" => config_run("continue_past_red_base = true\n"),
            _ => config_run("format: loom.agent/1\ntakt:\n  continue_past_red_base: true\n"),
        };
        assert!(
            out.contains("evidence: test_result fail"),
            "{by}: stdout {out} stderr {err}"
        );
        assert!(!out.contains("RedBase"), "{by}: {out}");
        assert!(world.calls() >= 1, "{by}: no harness call: {out} {err}");
    }
}

/// story:scenario-run-takes-config: `takt scenario run` takes the opt-out as `takt run` does, by
/// its flag or by its `--config` file's `continue_past_red_base`: the red base is recorded, and the
/// run goes on to the harness.
#[test]
fn a_scenario_run_goes_on_past_a_red_base_by_flag_or_configuration() {
    let files = scenario_1(&broken_todo());
    for by in ["flag", "toml", "yaml"] {
        let world = world(&format!("scenario-red-base-{by}"), &borrowed(&files));
        let config = world.state.join(format!("run.{by}"));
        let extra: Vec<String> = match by {
            "flag" => vec!["--continue-past-red-base".to_owned()],
            "toml" => {
                fs::write(&config, "continue_past_red_base = true\n").expect("a file");
                vec!["--config".to_owned(), config.display().to_string()]
            }
            _ => {
                fs::write(
                    &config,
                    "format: loom.agent/1\ntakt:\n  continue_past_red_base: true\n",
                )
                .expect("a file");
                vec!["--config".to_owned(), config.display().to_string()]
            }
        };
        let extra: Vec<&str> = extra.iter().map(String::as_str).collect();
        let (_, out, err) = world.scenario_with("./check.sh", "3", &extra);
        assert!(
            out.contains("evidence: test_result fail"),
            "{by}: stdout {out} stderr {err}"
        );
        assert!(!out.contains("RedBase"), "{by}: {out}");
        assert!(world.calls() >= 1, "{by}: no harness call: {out} {err}");
    }
}

/// On the unbroken seed the first `tests.run` passes and the run goes on to the harness.
#[test]
fn the_unbroken_seed_goes_on_to_the_harness() {
    let todo = fs::read_to_string(Path::new(FIXTURES).join("scenario-1/base/todo.py"))
        .expect("scenario 1's todo.py");
    let files = scenario_1(&todo);
    let world = world("green-base", &borrowed(&files));
    let (_, out, err) = world.scenario("./check.sh", "4");
    assert!(
        out.contains("evidence: test_result pass"),
        "stdout {out} stderr {err}"
    );
    assert!(!out.contains("RedBase"), "{out}");
    assert!(world.calls() >= 1, "no harness call: {out}");
}

/// story:repeated-identical-test-failures-stop-the-run's first acceptance: after two
/// consecutive results that fail the same two tests at two revisions, the run stops exit 3,
/// naming the tests and 2, with no harness call after the second.
#[test]
fn two_identical_failures_at_new_revisions_stop_the_run() {
    let script = cargo_script(SAME_TWO);
    let world = world("same", &[("t.sh", &script, true)]);
    let (code, out, err) = world.scenario("./t.sh", "40");
    let stop = out
        .lines()
        .find(|line| line.starts_with("stopped: "))
        .unwrap_or_else(|| panic!("no stop line: stdout {out} stderr {err}"));
    assert!(
        stop.starts_with("stopped: RepeatedTestFailure (2 "),
        "{stop}\n{out}\n{err}"
    );
    assert!(stop.contains("a::two, a::three"), "{stop}");
    assert_eq!(
        out.matches("evidence: test_result fail").count(),
        2,
        "{out}"
    );
    assert_eq!(world.calls(), 2, "a harness call after the second: {out}");
    assert_eq!(code, 3, "stdout {out} stderr {err}");
}

/// The second acceptance: a run whose failing set changes at every result does not stop for
/// this; it runs on to its step budget.
#[test]
fn a_failing_set_that_changes_does_not_stop_the_run() {
    let script = cargo_script(ANOTHER_EACH_TIME);
    let world = world("changing", &[("t.sh", &script, true)]);
    let (code, out, err) = world.scenario("./t.sh", "16");
    assert!(
        out.matches("evidence: test_result fail").count() > 3,
        "stdout {out} stderr {err}"
    );
    assert!(!out.contains("RepeatedTestFailure"), "{out}");
    assert!(out.contains("stopped: Iterations (16)"), "{out}");
    assert_eq!(code, 3, "stdout {out} stderr {err}");
}

/// A failure whose output names no test is no set to compare: it does not count.
#[test]
fn failures_that_name_no_test_do_not_stop_the_run() {
    let script = cargo_script(UNNAMED);
    let world = world("unnamed", &[("t.sh", &script, true)]);
    let (_, out, err) = world.scenario("./t.sh", "16");
    assert!(
        out.matches("evidence: test_result fail").count() > 3,
        "stdout {out} stderr {err}"
    );
    assert!(!out.contains("RepeatedTestFailure"), "{out}");
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

/// The stop line of a run's output.
fn stop_of(out: &str, err: &str) -> String {
    out.lines()
        .find(|line| line.starts_with("stopped: "))
        .unwrap_or_else(|| panic!("no stop line: stdout {out} stderr {err}"))
        .to_owned()
}

/// A test command that fails the same two tests whatever the revision.
const RED: &str = "#!/bin/sh\nprintf 'running 3 tests\\ntest a::one ... ok\\ntest a::two ... FAILED\\ntest a::three ... FAILED\\n\\ntest result: FAILED. 1 passed; 2 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s\\n'\nexit 101\n";

/// A test command that passes.
const GREEN: &str = "#!/bin/sh\nprintf 'running 3 tests\\ntest result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s\\n'\n";

/// A restart after `RedBase` runs the tests again before any model call: the same failure
/// stops it `RedBase` again, no harness call; once the base passes (the environment fixed, here
/// the test command), it goes on to the harness.
#[test]
fn a_restart_after_red_base_checks_again_and_goes_on_once_green() {
    let world = world(
        "red-restart",
        &[("red.sh", RED, true), ("green.sh", GREEN, true)],
    );
    let (code, out, err) = world.scenario("./red.sh", "20");
    assert!(
        stop_of(&out, &err).starts_with("stopped: RedBase ("),
        "{out}"
    );
    assert_eq!(code, 3, "stdout {out} stderr {err}");
    let case = case_of(&out);
    let (code, out, err) = world.resume(&case, "./red.sh", "20");
    let stop = stop_of(&out, &err);
    assert!(
        stop.starts_with("stopped: RedBase ("),
        "{stop}\n{out}\n{err}"
    );
    assert!(stop.contains("a::two, a::three"), "{stop}");
    assert!(
        out.contains("evidence: test_result fail"),
        "the tests ran again: {out}"
    );
    assert_eq!(world.calls(), 0, "{out}");
    assert_eq!(code, 3, "stdout {out} stderr {err}");
    let (_, out, err) = world.resume(&case, "./green.sh", "3");
    assert!(
        out.contains("evidence: test_result pass"),
        "stdout {out} stderr {err}"
    );
    assert!(!out.contains("RedBase"), "{out}");
    assert!(world.calls() >= 1, "stdout {out} stderr {err}");
}

/// A restart after `RepeatedTestFailure` runs the tests again before any model call, at the
/// revision the second failure was at: the same set stops it the same way, the count read from
/// the case's record, with no further harness call.
#[test]
fn a_restart_after_repeated_failures_stops_again_without_a_model_call() {
    let script = cargo_script(SAME_TWO);
    let world = world("repeat-restart", &[("t.sh", &script, true)]);
    let (code, out, err) = world.scenario("./t.sh", "40");
    assert!(
        stop_of(&out, &err).starts_with("stopped: RepeatedTestFailure (2 "),
        "{out}"
    );
    assert_eq!(code, 3, "stdout {out} stderr {err}");
    assert_eq!(world.calls(), 2, "{out}");
    let case = case_of(&out);
    let (code, out, err) = world.resume(&case, "./t.sh", "40");
    let stop = stop_of(&out, &err);
    assert!(
        stop.starts_with("stopped: RepeatedTestFailure (2 "),
        "{stop}\n{out}\n{err}"
    );
    assert_eq!(world.calls(), 2, "{out}");
    assert_eq!(code, 3, "stdout {out} stderr {err}");
}
