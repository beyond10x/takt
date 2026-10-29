//! story:a-scenario-declares-an-expected-red-base, through the binary: a scenario whose section
//! of `.engineering/scenarios.conf` gives `red-base = expected` goes past a seed whose tests fail,
//! without `--continue-past-red-base`, and says which declaration it went on by; the same seed
//! without the key still stops `RedBase`, exit 3; any other value of the key is refused before
//! anything is cut. A harness double on the program path counts its calls; nothing reaches a
//! network or a model.

#[path = "../../takt/tests/support/own_home.rs"]
mod own_home;

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

/// A test command that fails the same two tests whatever the revision: a seed red on purpose.
const RED: &str = "#!/bin/sh\nprintf 'running 3 tests\\ntest a::one ... ok\\ntest a::two ... FAILED\\ntest a::three ... FAILED\\n\\ntest result: FAILED. 1 passed; 2 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s\\n'\nexit 101\n";

/// A test command that passes whatever the revision: a green seed.
const GREEN: &str = "#!/bin/sh\nprintf 'running 1 test\\ntest a::one ... ok\\n\\ntest result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s\\n'\nexit 0\n";

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
/// a gates document and the red test command at the seed, then scenario 1 declared on the next
/// commit with `story`, `seed` and the lines `extra`; no origin. A `claude` double in `bin`
/// appends a line to `calls` and one to `work.txt` on every call and proposes
/// `implementation.edit`.
struct World {
    _scratch: Scratch,
    repo: PathBuf,
    bin: PathBuf,
    calls: PathBuf,
    state: PathBuf,
}

fn world(name: &str, extra: &str) -> World {
    let outer = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("scenario-red-base-{}-{name}", std::process::id()));
    if outer.exists() {
        fs::remove_dir_all(&outer).expect("a stale scratch tree is removed");
    }
    let repo = outer.join("repo");
    copy_dir(&Path::new(FIXTURES).join("one-ready"), &repo);
    fs::write(repo.join("Cargo.toml"), "[package]\nname = \"scratch\"\n").expect("a file");
    fs::write(repo.join(".engineering/gates.yaml"), GATES).expect("a file");
    for (file, text) in [("red.sh", RED), ("green.sh", GREEN)] {
        let path = repo.join(file);
        fs::write(&path, text).expect("a file");
        fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).expect("executable");
    }
    git(&repo, &["init", "-q", "-b", "main"]);
    git(&repo, &["add", "-A"]);
    git(&repo, &["commit", "-q", "-m", "seed"]);
    let seed = git(&repo, &["rev-parse", "HEAD"]);
    fs::write(
        repo.join(".engineering/scenarios.conf"),
        format!("[scenario \"1\"]\n\tstory = story:ready-one\n\tseed = {seed}\n{extra}"),
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
    /// `takt scenario run` of scenario 1 with the red test command, no forge and 3 steps, no
    /// `--continue-past-red-base`: the double first on the program path, the state home in the
    /// scratch world. The exit code, stdout and stderr.
    fn scenario(&self) -> (i32, String, String) {
        self.scenario_with("./red.sh")
    }

    /// [`World::scenario`] with the test command `test_cmd`.
    fn scenario_with(&self, test_cmd: &str) -> (i32, String, String) {
        self.takt(&[
            "scenario",
            "run",
            "--repo",
            self.repo.to_str().expect("utf-8"),
            "1",
            "--test-cmd",
            test_cmd,
            "--forge",
            "none",
            "--commit-style",
            "action",
            "--iterations",
            "3",
        ])
    }

    /// `takt run --case <case>` with the red test command, no forge and 3 steps, no
    /// `--continue-past-red-base`: a resume as the trial guide gives it.
    fn resume(&self, case: &str) -> (i32, String, String) {
        self.takt(&[
            "run",
            "--repo",
            self.repo.to_str().expect("utf-8"),
            "--case",
            case,
            "--test-cmd",
            "./red.sh",
            "--forge",
            "none",
            "--commit-style",
            "action",
            "--iterations",
            "3",
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

    /// The local `trial/*` branches.
    fn trial_branches(&self) -> String {
        git(&self.repo, &["branch", "--list", "trial/*"])
    }
}

/// Acceptance 1: a scenario declaring `red-base = expected` whose seed's tests fail runs past
/// its first `tests.run` to a judgement step, with no `RedBase` stop, and prints the declaration
/// it went on by. A resume with `takt run --case`, without the flag, does not stop `RedBase`
/// either.
#[test]
fn an_expected_red_base_goes_on_without_the_flag() {
    let world = world("expected", "\tred-base = expected\n");
    let (_, out, err) = world.scenario();
    assert!(
        out.contains("evidence: test_result fail"),
        "the red base is recorded: stdout {out} stderr {err}"
    );
    assert!(!out.contains("RedBase"), "stdout {out} stderr {err}");
    assert!(
        out.contains("red base: expected by scenario 1 in .engineering/scenarios.conf at main"),
        "the declaration is named: stdout {out} stderr {err}"
    );
    assert!(
        world.calls() >= 1,
        "no judgement step was offered: stdout {out} stderr {err}"
    );
    let case = out
        .lines()
        .find_map(|line| {
            line.strip_prefix("case ")?
                .split_once(" on ")
                .map(|(id, _)| id)
        })
        .unwrap_or_else(|| panic!("the run names no case: {out}"))
        .to_owned();
    let (_, out, err) = world.resume(&case);
    assert!(
        out.contains("evidence: test_result fail"),
        "the resume ran the tests: stdout {out} stderr {err}"
    );
    assert!(
        !out.contains("RedBase"),
        "the resume: stdout {out} stderr {err}"
    );
}

/// Acceptance 2: the same seed without the key stops `RedBase`, exit 3, before any model call.
#[test]
fn an_undeclared_red_base_still_stops() {
    let world = world("undeclared", "");
    let (code, out, err) = world.scenario();
    let stop = out
        .lines()
        .find(|line| line.starts_with("stopped: "))
        .unwrap_or_else(|| panic!("no stop line: stdout {out} stderr {err}"));
    assert!(
        stop.starts_with("stopped: RedBase ("),
        "{stop}\n{out}\n{err}"
    );
    assert!(!out.contains("red base: expected"), "{out}");
    assert_eq!(world.calls(), 0, "a model call at a red base: {out}");
    assert_eq!(code, 3, "stdout {out} stderr {err}");
}

/// Acceptance 3: `red-base = maybe` is refused before anything is cut, exit 1, naming the key
/// and its one value.
#[test]
fn another_red_base_value_is_refused() {
    let world = world("maybe", "\tred-base = maybe\n");
    let (code, out, err) = world.scenario();
    assert_eq!(code, 1, "stdout {out} stderr {err}");
    assert!(err.contains("`red-base`"), "the key is named: {err}");
    assert!(err.contains("maybe"), "the value given is named: {err}");
    assert!(err.contains("`expected`"), "the one value is named: {err}");
    assert!(world.trial_branches().is_empty(), "a trial branch was cut");
    assert!(
        !world.repo.join(".takt/cases").exists(),
        "a case was opened: {out}"
    );
    assert_eq!(world.calls(), 0, "{out}");
}

/// The declaration only turns going on past a red base on: a `--config` file's
/// `continue_past_red_base = false` does not turn it back off.
#[test]
fn a_configured_false_does_not_turn_the_declaration_off() {
    let world = world("config-false", "\tred-base = expected\n");
    let config = world.state.join("run.toml");
    fs::write(&config, "continue_past_red_base = false\n").expect("a file");
    let (_, out, err) = world.takt(&[
        "scenario",
        "run",
        "--repo",
        world.repo.to_str().expect("utf-8"),
        "1",
        "--config",
        config.to_str().expect("utf-8"),
        "--test-cmd",
        "./red.sh",
        "--forge",
        "none",
        "--commit-style",
        "action",
        "--iterations",
        "3",
    ]);
    assert!(
        out.contains("red base: expected by scenario 1"),
        "stdout {out} stderr {err}"
    );
    assert!(!out.contains("RedBase"), "stdout {out} stderr {err}");
    assert!(world.calls() >= 1, "stdout {out} stderr {err}");
}

/// The key is `red-base`: git's configuration syntax admits no `_` in a key, so `red_base` is no
/// line git reads. It is refused before anything is cut, exit 1, naming the spelling that reads.
#[test]
fn the_underscored_spelling_is_refused_naming_the_one_that_reads() {
    let world = world("underscore", "\tred_base = expected\n");
    let (code, out, err) = world.scenario();
    assert_eq!(code, 1, "stdout {out} stderr {err}");
    assert!(
        err.contains("`red-base`"),
        "the spelling that reads is named: {err}"
    );
    assert!(world.trial_branches().is_empty(), "a trial branch was cut");
    assert_eq!(world.calls(), 0, "{out}");
}

/// The case id a run's `case <id> on …` line names.
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

/// The case keeps its declared red base in its own record, written at open: `red_base:
/// expected` in its `case.yaml`; a case whose scenario declares none records none.
#[test]
fn the_case_records_its_declared_red_base_at_open() {
    let declared = world("recorded", "\tred-base = expected\n");
    let (_, out, err) = declared.scenario();
    let case = case_of(&out);
    let snapshot = fs::read_to_string(
        declared
            .repo
            .join(".takt/cases")
            .join(&case)
            .join("case.yaml"),
    )
    .unwrap_or_else(|error| panic!("{error}: stdout {out} stderr {err}"));
    assert!(
        snapshot.lines().any(|line| line == "red_base: expected"),
        "{snapshot}"
    );
    let other = world("not-recorded", "");
    let (_, out, _) = other.scenario();
    let case = case_of(&out);
    let snapshot = fs::read_to_string(other.repo.join(".takt/cases").join(&case).join("case.yaml"))
        .expect("the case's snapshot");
    assert!(!snapshot.contains("red_base"), "{snapshot}");
}

/// A case whose scenario declares no red base stops `RedBase` on its resume too, without the
/// flag: only the case's record turns the stop off, never the resume itself.
#[test]
fn an_undeclared_case_stops_red_base_on_its_resume() {
    let world = world("undeclared-resume", "");
    let (code, out, err) = world.scenario();
    assert_eq!(code, 3, "stdout {out} stderr {err}");
    let (code, out, err) = world.resume(&case_of(&out));
    assert!(
        out.lines()
            .any(|line| line.starts_with("stopped: RedBase (")),
        "stdout {out} stderr {err}"
    );
    assert_eq!(code, 3, "stdout {out} stderr {err}");
    assert_eq!(world.calls(), 0, "a model call at a red base: {out}");
}

/// A `scenarios.conf` git refuses for a reason other than `red_base` is refused without the
/// `red-base` spelling hint, which is given only when the file's text holds `red_base`.
#[test]
fn a_refusal_without_red_base_gives_no_spelling_hint() {
    let world = world("other-refusal", "\tlabel_x = y\n");
    let (code, out, err) = world.scenario();
    assert_eq!(code, 1, "stdout {out} stderr {err}");
    assert!(
        err.contains("does not read as git's configuration syntax"),
        "{err}"
    );
    // The scratch path itself holds `red-base`; the hint is named in backticks.
    assert!(!err.contains("`red-base`"), "{err}");
    assert!(!err.contains("`red_base`"), "{err}");
    assert!(world.trial_branches().is_empty(), "a trial branch was cut");
    assert_eq!(world.calls(), 0, "{out}");
}

/// On a green seed the declaration's line still says what the run was told, and claims no red
/// base: it says only that tests failing at the base do not stop the run.
#[test]
fn a_green_seed_with_the_declaration_claims_no_red_base() {
    let world = world("green", "\tred-base = expected\n");
    let (_, out, err) = world.scenario_with("./green.sh");
    assert!(
        out.lines().any(|line| line
            == "red base: expected by scenario 1 in .engineering/scenarios.conf at main: tests failing at the base do not stop the run"),
        "stdout {out} stderr {err}"
    );
    assert!(
        out.contains("evidence: test_result pass"),
        "stdout {out} stderr {err}"
    );
    assert!(!out.contains("RedBase"), "stdout {out} stderr {err}");
}
