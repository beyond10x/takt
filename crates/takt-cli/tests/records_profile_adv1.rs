//! Adversary pass 1 on story:a-run-keeps-every-decision-only-on-a-trial, the CLI side. A case is
//! resumed by a later run, and every run of it writes its own `opened` line: what the profile of
//! one run drops must not be decisions another run said it kept. A scenario case keeps every
//! decision whatever the flag says, also when a later run resumes it. The `claude` is a double;
//! nothing reaches a model.

use std::fs;
use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

/// The fixture stores of the library crate, one directory each.
const FIXTURES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../takt/tests/fixtures");

fn git(tree: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .args(["-c", "user.name=t", "-c", "user.email=t@localhost"])
        .args(["-c", "commit.gpgsign=false"])
        .args(args)
        .current_dir(tree)
        .output()
        .expect("git runs");
    assert!(output.status.success(), "git {args:?}");
    String::from_utf8_lossy(&output.stdout).trim().to_owned()
}

fn executable(path: &Path, text: &str) {
    fs::write(path, text).expect("a script");
    fs::set_permissions(path, fs::Permissions::from_mode(0o755)).expect("exec bit");
}

/// The `claude` double: it writes a file and answers `tests.write`, each call.
const CLAUDE: &str = "#!/bin/sh\nmkdir -p tests\necho work >> tests/work.txt\nprintf '%s\\n' '{\"type\":\"result\",\"subtype\":\"success\",\"result\":\"{\\\"action\\\": \\\"tests.write\\\", \\\"summary\\\": \\\"wrote a test\\\"}\",\"usage\":{\"input_tokens\":3,\"output_tokens\":4}}'\n";

/// A scratch world under this crate's build directory, removed on drop.
struct World(PathBuf);

impl Drop for World {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

impl World {
    fn new(name: &str) -> Self {
        let outer = Path::new(env!("CARGO_TARGET_TMPDIR"))
            .join(format!("records-adv1-{name}-{}", std::process::id()));
        if outer.exists() {
            fs::remove_dir_all(&outer).expect("a stale scratch tree is removed");
        }
        let world = Self(outer);
        for dir in [world.repo(), world.bin()] {
            fs::create_dir_all(dir).expect("a directory");
        }
        fs::write(world.repo().join("README.md"), "a story\n").expect("a file");
        git(&world.repo(), &["init", "-q", "-b", "main"]);
        git(&world.repo(), &["add", "-A"]);
        git(&world.repo(), &["commit", "-q", "-m", "start"]);
        executable(&world.bin().join("claude"), CLAUDE);
        world
    }

    fn repo(&self) -> PathBuf {
        self.0.join("repo")
    }

    fn bin(&self) -> PathBuf {
        self.0.join("bin")
    }

    fn path(&self) -> String {
        format!(
            "{}:{}",
            self.bin().display(),
            std::env::var("PATH").unwrap_or_default()
        )
    }

    /// `takt` with `args` on the repository, the `claude` double on its path, two iterations,
    /// no forge and a passing test command.
    fn takt(&self, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_takt"))
            .args(args)
            .args(["--repo", self.repo().to_str().expect("utf-8")])
            .args(["--harness", "claude", "--forge", "none"])
            .args(["--iterations", "2", "--test-cmd", "true"])
            .env("PATH", self.path())
            .env("XDG_STATE_HOME", self.0.join("xdg-state"))
            .env_remove("B10X_GATES_POLICY")
            .env_remove("DBUS_SESSION_BUS_ADDRESS")
            .output()
            .expect("takt runs")
    }

    /// The one case directory the repository holds.
    fn case(&self) -> PathBuf {
        only_case(&self.repo())
    }

    fn case_id(&self) -> String {
        self.case()
            .file_name()
            .expect("a name")
            .to_string_lossy()
            .into_owned()
    }
}

/// The one case directory under `repo`.
fn only_case(repo: &Path) -> PathBuf {
    let cases: Vec<PathBuf> = fs::read_dir(repo.join(".engineering/cases"))
        .expect("the cases directory")
        .map(|entry| entry.expect("an entry").path())
        .collect();
    assert_eq!(cases.len(), 1, "{cases:?}");
    cases[0].clone()
}

/// The names under `case`'s `decisions/`, sorted.
fn decisions_of(case: &Path) -> Vec<String> {
    let mut names: Vec<String> = fs::read_dir(case.join("decisions"))
        .expect("the decisions directory")
        .map(|entry| {
            entry
                .expect("an entry")
                .file_name()
                .to_string_lossy()
                .into_owned()
        })
        .collect();
    names.sort();
    names
}

/// The run record of `case`, one JSON object a line.
fn events_of(case: &Path) -> Vec<serde_json::Value> {
    fs::read_to_string(case.join("events.jsonl"))
        .expect("the run record")
        .lines()
        .map(|line| serde_json::from_str(line).expect("a JSON line"))
        .collect()
}

/// Each run of the record, in order: the profile its `opened` line names (none on a line
/// written before the field) and the decision files its decision lines name.
fn runs_of(record: &[serde_json::Value]) -> Vec<(Option<String>, Vec<String>)> {
    let mut runs: Vec<(Option<String>, Vec<String>)> = Vec::new();
    for line in record {
        if line["event"] == "opened" {
            runs.push((line["records"].as_str().map(str::to_owned), Vec::new()));
        } else if line["event"] == "decision"
            && let (Some(run), Some(file)) = (runs.last_mut(), line["file"].as_str())
        {
            run.1.push(file.to_owned());
        }
    }
    runs
}

/// The decision files a run that says it keeps every decision (`trial`, or no field) names and
/// the case no longer holds.
fn dropped_from_kept_runs(case: &Path) -> Vec<String> {
    let mut dropped = Vec::new();
    for (records, files) in runs_of(&events_of(case)) {
        if records.as_deref() == Some("default") {
            continue;
        }
        for file in files {
            if !case.join(&file).is_file() {
                dropped.push(file);
            }
        }
    }
    dropped
}

fn text(output: &Output) -> String {
    format!(
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
}

/// A case opened with `--records trial` and resumed with `takt run --case <id>` and no
/// `--records` keeps the decisions its trial run kept: the resume is the same case, the profile
/// the case was opened under is the one the operator asked for, and `takt run --case` is how a
/// stopped trial goes on after `takt approve`.
#[test]
fn a_resume_without_the_flag_keeps_the_decisions_a_trial_run_kept() {
    let world = World::new("resume-no-flag");
    let opened = world.takt(&["run", "--intent", "probe", "--records", "trial"]);
    let said = text(&opened);
    assert_eq!(
        decisions_of(&world.case()),
        ["0001.json", "0002.json", "0003.json"],
        "{said}"
    );
    let resumed = world.takt(&["run", "--case", &world.case_id()]);
    let said = text(&resumed);
    let kept = decisions_of(&world.case());
    for earlier in ["0001.json", "0002.json", "0003.json"] {
        assert!(
            kept.contains(&earlier.to_owned()),
            "the trial run's {earlier} was dropped by the resume: {kept:?}\n{said}"
        );
    }
}

/// Whatever profile a resume runs under, a run whose `opened` line says `trial` (or names no
/// profile, as a line written before the field, which reads as every decision kept) still has
/// every decision its record names: the `opened` line tells the truth about its run.
#[test]
fn a_resume_under_default_leaves_every_trial_runs_decisions_on_disk() {
    let world = World::new("resume-default");
    world.takt(&["run", "--intent", "probe", "--records", "trial"]);
    let resumed = world.takt(&["run", "--case", &world.case_id(), "--records", "default"]);
    let said = text(&resumed);
    let runs = runs_of(&events_of(&world.case()));
    assert_eq!(runs.len(), 2, "two runs of the case\n{said}");
    assert_eq!(runs[0].0.as_deref(), Some("trial"), "{runs:?}");
    let dropped = dropped_from_kept_runs(&world.case());
    assert!(
        dropped.is_empty(),
        "decisions of a run whose opened line says trial are gone: {dropped:?}; on disk {:?}\n{said}",
        decisions_of(&world.case())
    );
}

/// The same holds for a case whose `opened` line was written before the field (an old case,
/// every decision kept): a resume by this binary, which runs under `default` unless told,
/// does not drop the old run's decisions.
#[test]
fn a_resume_of_an_old_case_keeps_its_earlier_decisions() {
    let world = World::new("resume-old");
    world.takt(&["run", "--intent", "probe", "--records", "trial"]);
    // The case as a binary before the profile wrote it: the same record, no `records` member.
    let events = world.case().join("events.jsonl");
    let old: String = fs::read_to_string(&events)
        .expect("the run record")
        .lines()
        .map(|line| {
            let mut value: serde_json::Value = serde_json::from_str(line).expect("a JSON line");
            if value["event"] == "opened" {
                value.as_object_mut().expect("an object").remove("records");
            }
            format!("{value}\n")
        })
        .collect();
    fs::write(&events, old).expect("the old record");
    let resumed = world.takt(&["run", "--case", &world.case_id()]);
    let said = text(&resumed);
    let dropped = dropped_from_kept_runs(&world.case());
    assert!(
        dropped.is_empty(),
        "an old case lost the decisions it kept: {dropped:?}; on disk {:?}\n{said}",
        decisions_of(&world.case())
    );
}

/// A replay of a step of a `trial` run after a later `default` resume: the resume removed only
/// its own decisions, so the trial run's decision is still there and the step replays from it,
/// with no "is not in the case" (story:a-run-keeps-every-decision-only-on-a-trial).
#[test]
fn replay_of_a_trial_runs_step_dropped_by_a_later_default_run_names_the_profile() {
    let world = World::new("replay-across-runs");
    world.takt(&["run", "--intent", "probe", "--records", "trial"]);
    world.takt(&["run", "--case", &world.case_id(), "--records", "default"]);
    let gone = !world.case().join("decisions/0002.json").exists();
    let replay = Command::new(env!("CARGO_BIN_EXE_takt"))
        .args([
            "replay",
            "--case",
            world.case().to_str().expect("utf-8"),
            "--step",
            "2",
            "--run",
            "1",
        ])
        .env("PATH", world.path())
        .env("XDG_STATE_HOME", world.0.join("xdg-state"))
        .env_remove("B10X_GATES_POLICY")
        .output()
        .expect("takt replay runs");
    let said = text(&replay);
    // story:a-run-keeps-every-decision-only-on-a-trial: a default run removes only the decisions
    // it wrote itself, so the trial run's decision stays and its step replays from it.
    assert!(
        !gone,
        "story:a-run-keeps-every-decision-only-on-a-trial: a later default run removed the trial run's decision 0002\n{said}"
    );
    assert!(
        !said.contains("is not in the case"),
        "story:a-run-keeps-every-decision-only-on-a-trial: the trial run's step replays from its kept decision\n{said}"
    );
}

/// A scenario run's case keeps every decision whatever `--records` says (acceptance 4), and
/// still does when `takt run --case` resumes it: the case is a scenario's, its scorecard reads
/// its record, and the resume's profile is the CLI's `default`.
#[test]
fn a_resumed_scenario_case_keeps_every_decision() {
    let outer = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("records-adv1-scenario-{}", std::process::id()));
    if outer.exists() {
        fs::remove_dir_all(&outer).expect("a stale scratch tree is removed");
    }
    let repo = outer.join("repo");
    copy_dir(&Path::new(FIXTURES).join("one-ready"), &repo);
    git(&repo, &["init", "-q", "-b", "main"]);
    git(&repo, &["add", "-A"]);
    git(&repo, &["commit", "-q", "-m", "seed"]);
    let seed = git(&repo, &["rev-parse", "HEAD"]);
    fs::write(
        repo.join(".engineering/scenarios.conf"),
        format!("[scenario \"1\"]\n\tstory = story:ready-one\n\tseed = {seed}\n"),
    )
    .expect("the declaration");
    git(&repo, &["add", "-A"]);
    git(&repo, &["commit", "-q", "-m", "declare the scenarios"]);
    let bin = outer.join("bin");
    fs::create_dir_all(&bin).expect("a directory");
    executable(&bin.join("claude"), CLAUDE);
    let path = format!(
        "{}:{}",
        bin.display(),
        std::env::var("PATH").unwrap_or_default()
    );
    let takt = |args: &[&str]| {
        Command::new(env!("CARGO_BIN_EXE_takt"))
            .args(args)
            .args(["--repo", repo.to_str().expect("utf-8")])
            .args(["--harness", "claude", "--forge", "none"])
            .args(["--iterations", "2", "--test-cmd", "true"])
            .env("PATH", &path)
            .env("XDG_STATE_HOME", outer.join("xdg-state"))
            .env_remove("B10X_GATES_POLICY")
            .output()
            .expect("takt runs")
    };
    let first = takt(&["scenario", "run", "1"]);
    let said = text(&first);
    let scenario = only_case(&repo);
    let before = decisions_of(&scenario);
    assert!(before.len() >= 2, "{before:?}\n{said}");
    let id = scenario
        .file_name()
        .expect("a name")
        .to_string_lossy()
        .into_owned();
    assert!(id.starts_with("scenario-1-trial-"), "{id}");
    let resumed = takt(&["run", "--case", &id]);
    let said = text(&resumed);
    let record = events_of(&scenario);
    let runs = runs_of(&record);
    let evaluated: usize = runs.iter().map(|(_, files)| files.len()).sum();
    assert!(
        evaluated > before.len(),
        "the resume evaluated the case again\n{said}"
    );
    assert_eq!(
        decisions_of(&scenario).len(),
        evaluated,
        "a scenario case dropped decisions on resume; runs {runs:?}\n{said}"
    );
    let _ = fs::remove_dir_all(&outer);
}

/// Copies the directory `from` to `to`, every file under it.
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

/// A removal cut short (the new decision written, an earlier one left) leaves two files, never
/// none. The next evaluation of a `default` run, a later run, removes only the decisions it
/// wrote itself, so the leftover of an earlier run stays and the resume's own latest is the
/// only one of its decisions the case keeps, its number one past the highest
/// (story:a-run-keeps-every-decision-only-on-a-trial: an earlier run's decisions are never
/// removed by a later one).
#[test]
fn a_decision_left_by_a_cut_removal_goes_at_the_next_evaluation() {
    let world = World::new("cut-removal");
    world.takt(&["run", "--intent", "probe"]);
    let case = world.case();
    assert_eq!(decisions_of(&case), ["0003.json"]);
    fs::copy(
        case.join("decisions/0003.json"),
        case.join("decisions/0002.json"),
    )
    .expect("an earlier decision left behind");
    let resumed = world.takt(&["run", "--case", &world.case_id()]);
    let said = text(&resumed);
    let kept = decisions_of(&case);
    assert_eq!(
        kept.len(),
        3,
        "story:a-run-keeps-every-decision-only-on-a-trial: the earlier run's two files stay and the resume keeps its latest: {kept:?}\n{said}"
    );
    assert_eq!(
        kept[..2],
        ["0002.json", "0003.json"],
        "story:a-run-keeps-every-decision-only-on-a-trial: {kept:?}\n{said}"
    );
    assert!(kept[2].as_str() > "0003.json", "{kept:?}\n{said}");
}
