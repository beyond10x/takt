//! Adversarial cases for story:a-scenario-declares-an-expected-red-base, through the binary:
//! where the `red-base` declaration is read from (only `--declared-on`, never the seed), which
//! section it belongs to, the spellings and values git and takt admit, a resume of a scenario
//! case that declared its red base expected, a green seed that declares one, and the help text.
//! A harness double on the program path counts its calls; nothing reaches a network or a model.

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

struct World {
    _scratch: Scratch,
    repo: PathBuf,
    bin: PathBuf,
    calls: PathBuf,
    state: PathBuf,
    seed: String,
}

/// The scratch world: a repository with the fixture store, its one story `proposed`, a manifest,
/// a gates document, `red.sh` and `green.sh` at the seed, and the seed's own
/// `.engineering/scenarios.conf` when `seed_conf` gives one; then `main`'s declaration, the
/// text `declare` with `{seed}` replaced by the seed's id, on the next commit. A `claude` double
/// in `bin` counts its calls and proposes `implementation.edit`.
fn world(name: &str, seed_conf: Option<&str>, declare: &str) -> World {
    let outer = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!(
        "scenario-red-base-adv1-{}-{name}",
        std::process::id()
    ));
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
    if let Some(conf) = seed_conf {
        fs::write(repo.join(".engineering/scenarios.conf"), conf).expect("a file");
    }
    git(&repo, &["init", "-q", "-b", "main"]);
    git(&repo, &["add", "-A"]);
    git(&repo, &["commit", "-q", "-m", "seed"]);
    let seed = git(&repo, &["rev-parse", "HEAD"]);
    fs::write(
        repo.join(".engineering/scenarios.conf"),
        declare.replace("{seed}", &seed),
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
        seed,
    }
}

/// Scenario 1's section, `story` and `seed` and the lines `extra`.
fn one(extra: &str) -> String {
    format!("[scenario \"1\"]\n\tstory = story:ready-one\n\tseed = {{seed}}\n{extra}")
}

impl World {
    /// `takt scenario run` of scenario 1 with `test_cmd`, no forge and 3 steps, no
    /// `--continue-past-red-base`, and the arguments `more`.
    fn scenario(&self, test_cmd: &str, more: &[&str]) -> (i32, String, String) {
        let mut args = vec![
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
        ];
        args.extend_from_slice(more);
        self.takt(&args)
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

    fn takt(&self, args: &[&str]) -> (i32, String, String) {
        let output = Command::new(env!("CARGO_BIN_EXE_takt"))
            .args(args)
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

    fn calls(&self) -> usize {
        fs::read_to_string(&self.calls)
            .unwrap_or_default()
            .lines()
            .count()
    }

    fn trial_branches(&self) -> String {
        git(&self.repo, &["branch", "--list", "trial/*"])
    }
}

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

/// Asserts the run stopped `RedBase`, exit 3, with no model call and no declaration line.
fn stopped_red(world: &World, (code, out, err): &(i32, String, String)) {
    let stop = stop_line(out, err);
    assert!(
        stop.starts_with("stopped: RedBase ("),
        "{stop}\n{out}\n{err}"
    );
    assert!(!out.contains("red base: expected"), "{out}");
    assert_eq!(world.calls(), 0, "a model call at a red base: {out}");
    assert_eq!(*code, 3, "stdout {out} stderr {err}");
}

/// Asserts the run went on past the red base by the declaration read at `at`.
fn went_on(world: &World, at: &str, (_, out, err): &(i32, String, String)) {
    assert!(
        out.contains(&format!(
            "red base: expected by scenario 1 in .engineering/scenarios.conf at {at}"
        )),
        "the declaration is named: stdout {out} stderr {err}"
    );
    assert!(!out.contains("RedBase"), "stdout {out} stderr {err}");
    assert!(
        world.calls() >= 1,
        "no judgement step: stdout {out} stderr {err}"
    );
}

/// Asserts the run was refused before anything was cut, exit 1, stderr holding each of `named`.
fn refused(world: &World, named: &[&str], (code, out, err): &(i32, String, String)) {
    assert_eq!(*code, 1, "stdout {out} stderr {err}");
    for name in named {
        assert!(err.contains(name), "{name} is named: {err}");
    }
    assert!(world.trial_branches().is_empty(), "a trial branch was cut");
    assert!(
        !world.repo.join(".takt/cases").exists(),
        "a case was opened: {out}"
    );
    assert_eq!(world.calls(), 0, "{out}");
}

/// The seed's own `scenarios.conf` declares the red base expected; `main`, the declaring
/// revision, does not. The seed is no source of the declaration: the run stops `RedBase`.
#[test]
fn the_seeds_own_declaration_is_not_read() {
    let world = world(
        "seed-declares",
        Some("[scenario \"1\"]\n\tstory = story:ready-one\n\tseed = HEAD\n\tred-base = expected\n"),
        &one(""),
    );
    let run = world.scenario("./red.sh", &[]);
    stopped_red(&world, &run);
}

/// `--declared-on` names the revision: a branch `declared` gives the key, `main` does not. The
/// run goes on by the branch's declaration and names that revision; `main` alone stops.
#[test]
fn the_declaration_comes_from_declared_on() {
    let world = world("declared-on", None, &one(""));
    git(&world.repo, &["checkout", "-q", "-b", "declared"]);
    fs::write(
        world.repo.join(".engineering/scenarios.conf"),
        one("\tred-base = expected\n").replace("{seed}", &world.seed),
    )
    .expect("a file");
    git(&world.repo, &["commit", "-q", "-am", "expect the red base"]);
    git(&world.repo, &["checkout", "-q", "main"]);
    let run = world.scenario("./red.sh", &["--declared-on", "declared"]);
    went_on(&world, "declared", &run);
}

/// Scenario `10` and scenario `1-x` declare their red base expected; scenario 1 does not. Their
/// keys are not scenario 1's: it stops `RedBase`.
#[test]
fn another_scenarios_declaration_does_not_leak() {
    let declare = format!(
        "{}[scenario \"10\"]\n\tstory = story:ready-one\n\tseed = {{seed}}\n\tred-base = expected\n\
         [scenario \"1-x\"]\n\tstory = story:ready-one\n\tseed = {{seed}}\n\tred-base = expected\n",
        one("")
    );
    let world = world("leak", None, &declare);
    let run = world.scenario("./red.sh", &[]);
    stopped_red(&world, &run);
}

/// `red-base` twice in the section is refused before anything is cut, naming the key.
#[test]
fn a_duplicate_red_base_is_refused() {
    let world = world(
        "twice",
        None,
        &one("\tred-base = expected\n\tred-base = expected\n"),
    );
    let run = world.scenario("./red.sh", &[]);
    refused(&world, &["`red-base`", "twice"], &run);
}

/// The same key in a second `[scenario "1"]` section: git reads one section, so twice too.
#[test]
fn a_red_base_in_a_repeated_section_is_refused() {
    let declare = format!(
        "{}[scenario \"1\"]\n\tred-base = expected\n",
        one("\tred-base = expected\n")
    );
    let world = world("twice-section", None, &declare);
    let run = world.scenario("./red.sh", &[]);
    refused(&world, &["`red-base`", "twice"], &run);
}

/// `red-base` with no value (git's bare-key form) is refused before anything is cut.
#[test]
fn a_bare_red_base_is_refused() {
    let world = world("bare", None, &one("\tred-base\n"));
    let run = world.scenario("./red.sh", &[]);
    refused(&world, &["`red-base`", "no value"], &run);
}

/// `red-base =` (empty) is refused before anything is cut.
#[test]
fn an_empty_red_base_is_refused() {
    let world = world("empty", None, &one("\tred-base =\n"));
    let run = world.scenario("./red.sh", &[]);
    refused(&world, &["`red-base`"], &run);
}

/// `Expected` is not the one value: refused, naming the value that reads.
#[test]
fn a_capitalised_value_is_refused() {
    let world = world("capital", None, &one("\tred-base = Expected\n"));
    let run = world.scenario("./red.sh", &[]);
    refused(&world, &["`red-base", "Expected", "`expected`"], &run);
}

/// `redBase` is no spelling git turns into `red-base` (it reads `redbase`): refused, naming the
/// key that reads.
#[test]
fn a_camel_case_key_is_refused_naming_red_base() {
    let world = world("camel", None, &one("\tredBase = expected\n"));
    let run = world.scenario("./red.sh", &[]);
    refused(&world, &["redbase", "`red-base`"], &run);
}

/// git's key names are case-insensitive: `RED-BASE` is the key `red-base` as git reads it, and
/// the run goes on by it.
#[test]
fn an_upper_case_key_reads_as_git_reads_it() {
    let world = world("upper", None, &one("\tRED-BASE = expected\n"));
    let run = world.scenario("./red.sh", &[]);
    went_on(&world, "main", &run);
}

/// Surrounding whitespace is no part of the value: ` expected ` reads as `expected`.
#[test]
fn whitespace_around_the_value_reads() {
    let world = world("spaces", None, &one("\tred-base =   expected   \n"));
    let run = world.scenario("./red.sh", &[]);
    went_on(&world, "main", &run);
}

/// The guide says the line comes before the case opens: it precedes the `case … on` line.
#[test]
fn the_declaration_line_comes_before_the_case_opens() {
    let world = world("order", None, &one("\tred-base = expected\n"));
    let (code, out, err) = world.scenario("./red.sh", &[]);
    let declared = out
        .find("red base: expected by scenario 1")
        .unwrap_or_else(|| panic!("no declaration line: {code} {out} {err}"));
    let case = out
        .lines()
        .position(|line| line.starts_with("case ") && line.contains(" on "))
        .and_then(|n| out.lines().nth(n))
        .and_then(|line| out.find(line))
        .unwrap_or_else(|| panic!("no case line: {out}"));
    assert!(declared < case, "{out}");
}

/// A scenario run that declared its red base expected and stopped before its first `tests.run`
/// (a `setup` command failed, a stop the case stays open and is resumed from) resumes with
/// `takt run --case` and no flag, as both guides say: "A resume of the case with `takt run
/// --case` needs no flag either." The resume's first `tests.run` is the case's first, at the
/// unchanged base, and the declaration is not known to the resume.
#[test]
fn a_resume_after_a_stop_before_the_first_tests_goes_on_as_the_guide_says() {
    let world = world("resume-setup", None, &one("\tred-base = expected\n"));
    let config = world.state.join("run.yaml");
    fs::write(
        &config,
        "setup:\n  - name: deps\n    argv: [sh, -c, exit 3]\n",
    )
    .expect("a file");
    let (code, out, err) =
        world.scenario("./red.sh", &["--config", config.to_str().expect("utf-8")]);
    assert!(
        out.contains("red base: expected by scenario 1"),
        "stdout {out} stderr {err}"
    );
    assert!(
        stop_line(&out, &err).starts_with("stopped: SetupFailed (deps exited 3)"),
        "stdout {out} stderr {err}"
    );
    assert_eq!(code, 1, "stdout {out} stderr {err}");
    assert!(!out.contains("evidence: test_result"), "{out}");
    let case = case_of(&out);
    let (code, out, err) = world.resume(&case);
    assert!(
        out.contains("evidence: test_result fail"),
        "the resume ran the tests: stdout {out} stderr {err}"
    );
    assert!(
        !out.contains("RedBase"),
        "the resume of a scenario case whose red base is declared expected stopped RedBase, exit {code}; the guides say it needs no flag: stdout {out} stderr {err}"
    );
    assert!(world.calls() >= 1, "stdout {out} stderr {err}");
}

/// A green seed that declares its red base expected runs as any green seed: it goes on, no
/// `RedBase`, the tests passing at the base.
#[test]
fn an_expected_red_base_on_a_green_seed_runs_as_green() {
    let world = world("green", None, &one("\tred-base = expected\n"));
    let (_, out, err) = world.scenario("./green.sh", &[]);
    assert!(
        out.contains("evidence: test_result pass"),
        "stdout {out} stderr {err}"
    );
    assert!(!out.contains("RedBase"), "stdout {out} stderr {err}");
    assert!(world.calls() >= 1, "stdout {out} stderr {err}");
}

/// The flag and the declaration together go on, and the declaration is still named.
#[test]
fn the_flag_and_the_declaration_together_go_on() {
    let world = world("both", None, &one("\tred-base = expected\n"));
    let run = world.scenario("./red.sh", &["--continue-past-red-base"]);
    went_on(&world, "main", &run);
}

/// `takt scenario run --help` states the key, its line and that `--config`'s false does not
/// turn it off, as the guide does.
#[test]
fn the_help_states_the_declaration() {
    let output = Command::new(env!("CARGO_BIN_EXE_takt"))
        .args(["scenario", "run", "--help"])
        .output()
        .expect("takt runs");
    let help = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success(), "{help}");
    let flat = help.split_whitespace().collect::<Vec<_>>().join(" ");
    for said in [
        "`red-base = expected`",
        "red base: expected by scenario <id> in .engineering/scenarios.conf at <revision>",
        "`continue_past_red_base = false` in `--config` does not turn it off",
        "Any other value of `red-base` is refused before anything is cut",
    ] {
        assert!(flat.contains(said), "{said}: {help}");
    }
}
