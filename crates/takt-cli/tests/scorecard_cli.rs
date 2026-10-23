//! story:scorecard-reader through the binary: `takt scorecard --repo <path>` prints a row per
//! case and per run read from the runs' events only; `--save <dir> --revision <sha>` writes
//! `scorecard-<sha>.json`; `--against <file>` marks a scenario worse than the saved one's and
//! exits 3. The case directories are written through `takt::events`, as the run loop writes them.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use takt::case::CASES_DIR;
use takt::events::{Event, Events, TokenUsage};

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

fn scratch(name: &str) -> Scratch {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("scorecard-cli-{}-{name}", std::process::id()));
    if dir.exists() {
        fs::remove_dir_all(&dir).expect("a stale scratch tree is removed");
    }
    fs::create_dir_all(&dir).expect("a scratch tree");
    Scratch(dir)
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

fn takt(args: &[&str]) -> (i32, String, String) {
    let output = Command::new(env!("CARGO_BIN_EXE_takt"))
        .args(args)
        .output()
        .expect("takt runs");
    (
        output.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&output.stdout).into_owned(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
    )
}

/// One run of `case` in `repo`: a judgement step that reported `tokens` (all of them input),
/// then `stop`; the held-out check beside the record passed. Its `opened` line names no binary.
fn run(repo: &Path, case: &str, tokens: u64, stop: &str) {
    run_made_with(repo, case, tokens, stop, None);
}

/// As [`run`], the `opened` line naming `binary`, as `takt --version` prints it (none: a binary
/// that recorded no revision).
fn run_made_with(repo: &Path, case: &str, tokens: u64, stop: &str, binary: Option<&str>) {
    let dir = repo.join(CASES_DIR).join(case);
    fs::create_dir_all(&dir).expect("a case directory");
    fs::write(dir.join(takt::scorecard::HELD_OUT_FILE), "PASS\n").expect("the held-out result");
    let events = Events::new(&dir);
    let usage = TokenUsage {
        input: tokens,
        output: 0,
        cache_read: 0,
        cache_creation: 0,
    };
    for (step, event) in [
        (
            0,
            Event::Opened {
                case: case.to_owned(),
                protocol: "story.delivery/1".to_owned(),
                story: None,
                inputs: Vec::new(),
                base: None,
                binary: binary.map(str::to_owned),
                layers: None,
                confinement: None,
                endpoint: None,
            },
        ),
        (
            1,
            Event::Step {
                by: "harness".to_owned(),
                name: "claude-code".to_owned(),
                action: "implementation.edit".to_owned(),
                summary: "edited".to_owned(),
                tokens: Some(usage.total()),
                usage: Some(usage),
                revision: None,
                model: None,
                version: None,
                cost_usd: None,
                duration_ms: None,
                api_ms: None,
                turns: None,
                started_at: None,
                transcript: None,
                transcript_refused: None,
            },
        ),
        (
            1,
            Event::Stop {
                stop: stop.to_owned(),
                steps: 1,
                tokens,
                completed: None,
            },
        ),
    ] {
        events.emit(step, &event).expect("the event is written");
    }
}

/// A committed repository holding two scenario 1 runs, and a case with no record.
fn repository(name: &str, runs: &[(&str, u64, &str)]) -> Scratch {
    let repo = scratch(name);
    fs::write(repo.join("README"), "scratch\n").expect("a file");
    git(&repo, &["init", "-q", "-b", "main"]);
    git(&repo, &["add", "-A"]);
    git(&repo, &["commit", "-q", "-m", "start"]);
    for (case, tokens, stop) in runs {
        run(&repo, case, *tokens, stop);
    }
    repo
}

fn row<'a>(text: &'a str, case: &str) -> Vec<&'a str> {
    text.lines()
        .find(|line| line.split_whitespace().next() == Some(case))
        .unwrap_or_else(|| panic!("{case} in {text}"))
        .split_whitespace()
        .collect()
}

/// The acceptance through the binary: each run's tokens are the sum its events report, and a
/// planted `task_spend` file anywhere in the tree changes nothing printed.
#[test]
fn scorecard_prints_each_runs_tokens_and_ignores_a_planted_task_spend() {
    let repo = repository(
        "print",
        &[
            ("scenario-1-trial-1", 1200, "Completed (accepted)"),
            ("scenario-1-trial-2", 900, "NoProgress"),
        ],
    );
    fs::create_dir_all(repo.join(CASES_DIR).join("takt-20261018T100000Z")).expect("a case");
    let path = repo.display().to_string();
    let (code, before, stderr) = takt(&["scorecard", "--repo", &path]);
    assert_eq!(code, 0, "{before}{stderr}");
    let header: Vec<&str> = before
        .lines()
        .find(|line| line.starts_with("case "))
        .expect("a header")
        .split_whitespace()
        .collect();
    let tokens = header.iter().position(|c| *c == "tokens").expect("tokens");
    assert_eq!(row(&before, "scenario-1-trial-1")[tokens], "1200");
    assert_eq!(row(&before, "scenario-1-trial-2")[tokens], "900");
    assert_eq!(
        row(&before, "takt-20261018T100000Z"),
        ["takt-20261018T100000Z", "no", "record"]
    );

    let spend = r#"{"kind":"task_spend","tokens":1}"#;
    for dir in [
        repo.to_path_buf(),
        repo.join(CASES_DIR).join("scenario-1-trial-1"),
        repo.join(CASES_DIR)
            .join("scenario-1-trial-1")
            .join("evidence"),
    ] {
        fs::create_dir_all(&dir).expect("a directory");
        fs::write(dir.join("task_spend.json"), spend).expect("a planted file");
    }
    let (code, after, stderr) = takt(&["scorecard", "--repo", &path]);
    assert_eq!(code, 0, "{after}{stderr}");
    assert_eq!(before, after);
}

/// The revision of takt under test the saves below are named by.
const REVISION: &str = "0123456789abcdef0123456789abcdef01234567";

/// `--save` with `--revision` writes the scorecard named by that revision, and says so;
/// `--against` it, five cases a side as the rule needs, a scenario one of whose cases stopped
/// `NoProgress` and whose median of tokens is over the saved maximum is worse, exit 3; one whose
/// cases were accepted under it is not, exit 0.
#[test]
fn save_then_against_marks_a_worse_scenario_and_exits_3() {
    /// Five scenario 1 cases from trial `from`, each at `tokens`, the last stopped `last` and
    /// the others accepted.
    fn five(from: usize, tokens: u64, last: &'static str) -> Vec<(String, u64, &'static str)> {
        (from..from + 5)
            .map(|n| {
                let stop = if n == from + 4 { last } else { ACCEPTED };
                (format!("scenario-1-trial-{n}"), tokens, stop)
            })
            .collect()
    }
    fn borrowed<'a>(runs: &'a [(String, u64, &'static str)]) -> Vec<(&'a str, u64, &'static str)> {
        runs.iter()
            .map(|(case, tokens, stop)| (case.as_str(), *tokens, *stop))
            .collect()
    }
    const ACCEPTED: &str = "Completed (accepted)";
    let baseline = repository("baseline", &borrowed(&five(1, 500, ACCEPTED)));
    let saves = scratch("saves");
    let (code, text, stderr) = takt(&[
        "scorecard",
        "--repo",
        &baseline.display().to_string(),
        "--save",
        &saves.display().to_string(),
        "--revision",
        REVISION,
    ]);
    assert_eq!(code, 0, "{text}{stderr}");
    let saved = saves.join(format!("scorecard-{REVISION}.json"));
    assert!(saved.is_file(), "{text}");
    assert!(
        text.contains(&format!(
            "saved: {}, named by the revision of takt under test",
            saved.display()
        )),
        "{text}"
    );
    assert_ne!(REVISION, git(&baseline, &["rev-parse", "HEAD"]));

    let candidate = repository("candidate", &borrowed(&five(6, 650, "NoProgress")));
    let (code, text, stderr) = takt(&[
        "scorecard",
        "--repo",
        &candidate.display().to_string(),
        "--against",
        &saved.display().to_string(),
    ]);
    assert_eq!(code, 3, "{text}{stderr}");
    // One case lowered is within the rule's margin of one; the median of tokens over the saved
    // maximum is what marks the scenario worse.
    for line in [
        "rule  trials 5 a scenario  margin 1 case  tokens worse when the median exceeds the saved maximum",
        "scenario 1  median tokens 650 exceeds the saved maximum 500",
        "scenario 1  outcomes by rank held  tokens over  worse",
    ] {
        assert!(text.contains(line), "{line} in {text}");
    }
    assert!(text.contains("scenario 1  median tokens +150"), "{text}");

    let better = repository("better", &borrowed(&five(11, 400, ACCEPTED)));
    let (code, text, stderr) = takt(&[
        "scorecard",
        "--repo",
        &better.display().to_string(),
        "--against",
        &saved.display().to_string(),
    ]);
    assert_eq!(code, 0, "{text}{stderr}");
    assert!(
        text.contains("scenario 1  outcomes by rank held  tokens held  not worse"),
        "{text}"
    );
}

/// `--save` without `--revision` is refused, exit 1, and writes nothing: a scorecard is never
/// named by the scenario repository's `HEAD`. `--revision` without `--save` does not parse.
#[test]
fn save_without_a_revision_is_refused() {
    let repo = repository("no-revision", &[("scenario-1-trial-1", 5, "NoProgress")]);
    let saves = scratch("no-revision-saves");
    let (code, text, stderr) = takt(&[
        "scorecard",
        "--repo",
        &repo.display().to_string(),
        "--save",
        &saves.display().to_string(),
    ]);
    assert_eq!(code, 1, "{text}{stderr}");
    assert!(stderr.contains("--revision"), "{stderr}");
    assert_eq!(fs::read_dir(&*saves).expect("the directory").count(), 0);
    let (code, _, stderr) = takt(&[
        "scorecard",
        "--repo",
        &repo.display().to_string(),
        "--revision",
        REVISION,
    ]);
    assert_eq!(code, 2, "{stderr}");
}

/// A saved scorecard is not overwritten: a second save under the same revision is refused,
/// exit 1, and the first file stays as it was.
#[test]
fn a_second_save_under_the_same_revision_is_refused() {
    let repo = repository("twice", &[("scenario-1-trial-1", 5, "NoProgress")]);
    let saves = scratch("twice-saves");
    let save = || {
        takt(&[
            "scorecard",
            "--repo",
            &repo.display().to_string(),
            "--save",
            &saves.display().to_string(),
            "--revision",
            REVISION,
        ])
    };
    let (code, text, stderr) = save();
    assert_eq!(code, 0, "{text}{stderr}");
    let path = saves.join(format!("scorecard-{REVISION}.json"));
    let first = fs::read_to_string(&path).expect("the saved file");
    run(&repo, "scenario-1-trial-2", 7, "Completed (accepted)");
    let (code, text, stderr) = save();
    assert_eq!(code, 1, "{text}{stderr}");
    assert!(stderr.contains("exists"), "{stderr}");
    assert_eq!(fs::read_to_string(&path).expect("the saved file"), first);
}

/// The revision of a takt other than the one under test, as its `--version` prints it.
const OTHER_BUILD: &str = "8378cb29aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

/// The revision a save below names, a full commit id: the finding's `0000000` as forty zeros.
const ZEROS: &str = "0000000000000000000000000000000000000000";

/// `takt scorecard --repo <repo> --save <saves> --revision <revision>`.
fn save_under(repo: &Path, saves: &Path, revision: &str) -> (i32, String, String) {
    takt(&[
        "scorecard",
        "--repo",
        &repo.display().to_string(),
        "--save",
        &saves.display().to_string(),
        "--revision",
        revision,
    ])
}

/// story:scorecard-save-checks-binary: a save under a revision another than the one a run's
/// `opened` line names (another sha, `<sha>-dirty`) is refused, exit 1, naming that run and
/// both revisions, and writes nothing.
#[test]
fn save_under_a_revision_other_than_a_runs_binary_is_refused_naming_the_run() {
    let dirty = format!("{ZEROS}-dirty");
    for (label, binary) in [("other", OTHER_BUILD), ("dirty", dirty.as_str())] {
        let repo = repository(&format!("made-{label}"), &[]);
        run_made_with(&repo, "scenario-1-trial-1", 5, "NoProgress", Some(ZEROS));
        run_made_with(&repo, "scenario-1-trial-2", 5, "NoProgress", Some(binary));
        let saves = scratch(&format!("made-{label}-saves"));
        let (code, text, stderr) = save_under(&repo, &saves, ZEROS);
        assert_eq!(code, 1, "{label}: {text}{stderr}");
        assert!(
            stderr.contains("scenario-1-trial-2#1")
                && stderr.contains(binary)
                && stderr.contains(ZEROS),
            "{label}: the run and both revisions are named: {stderr}"
        );
        assert!(!text.contains("saved:"), "{label}: {text}");
        assert_eq!(
            fs::read_dir(&*saves).expect("the directory").count(),
            0,
            "{label}: nothing is written"
        );
    }

    let repo = repository("made-finding", &[]);
    run_made_with(
        &repo,
        "takt-20261019T120000Z",
        5,
        "NoProgress",
        Some(OTHER_BUILD),
    );
    let saves = scratch("made-finding-saves");
    let (code, text, stderr) = save_under(&repo, &saves, ZEROS);
    assert_eq!(code, 1, "{text}{stderr}");
    assert!(stderr.contains("takt-20261019T120000Z#1"), "{stderr}");
    assert!(
        !text.contains("named by the revision of takt under test"),
        "{text}"
    );
}

/// story:scorecard-save-checks-binary: a save under the revision every run's `opened` line
/// names succeeds, and says it is named by the revision of takt under test.
#[test]
fn save_under_the_revision_every_run_was_made_with_succeeds() {
    let repo = repository("made-own", &[]);
    run_made_with(&repo, "scenario-1-trial-1", 5, "NoProgress", Some(ZEROS));
    run_made_with(
        &repo,
        "scenario-1-trial-2",
        7,
        "Completed (accepted)",
        Some(ZEROS),
    );
    let saves = scratch("made-own-saves");
    let (code, text, stderr) = save_under(&repo, &saves, ZEROS);
    assert_eq!(code, 0, "{text}{stderr}");
    let saved = saves.join(format!("scorecard-{ZEROS}.json"));
    assert!(saved.is_file(), "{text}");
    assert!(
        text.contains(&format!(
            "saved: {}, named by the revision of takt under test",
            saved.display()
        )),
        "{text}"
    );
}

/// story:scorecard-save-checks-binary, decision D3: `--revision` for a save is the full commit
/// id, 40 lower-case hex digits; an abbreviated one, or a longer one, is refused, exit 1, before
/// any run is read against it, whatever the runs name, and nothing is written.
#[test]
fn save_under_anything_but_a_full_commit_id_is_refused() {
    let sha256 = "0".repeat(64);
    for (label, binary) in [("none", None), ("own", Some(ZEROS))] {
        let repo = repository(&format!("short-{label}"), &[]);
        run_made_with(&repo, "scenario-1-trial-1", 5, "NoProgress", binary);
        for revision in ["abcdef0", "0000000", &ZEROS[..39], sha256.as_str()] {
            let saves = scratch(&format!("short-{label}-saves"));
            let (code, text, stderr) = save_under(&repo, &saves, revision);
            assert_eq!(code, 1, "{label} {revision}: {text}{stderr}");
            assert!(
                stderr.contains("40 lower-case hex digits"),
                "{label} {revision}: {stderr}"
            );
            assert!(!text.contains("saved:"), "{label} {revision}: {text}");
            assert_eq!(
                fs::read_dir(&*saves).expect("the directory").count(),
                0,
                "{label} {revision}: nothing is written"
            );
        }
    }
}

/// story:scorecard-save-checks-binary, decision D2 and the planner's ruling on it: a run whose
/// `opened` line names no revision (no `binary`, as a binary built before `--version` named its
/// revision left it, or `unknown revision`, as one built outside a checkout prints) is saved
/// only when no run names one; a mix is refused, exit 1, naming the run that names none and what
/// its line says, and writes nothing.
#[test]
fn a_mix_of_runs_naming_a_revision_and_naming_none_is_refused() {
    for (label, binary) in [("none", None), ("unknown", Some("unknown revision"))] {
        let repo = repository(&format!("made-mix-{label}"), &[]);
        run_made_with(&repo, "scenario-1-trial-1", 5, "NoProgress", Some(ZEROS));
        run_made_with(&repo, "scenario-1-trial-2", 5, "NoProgress", binary);
        let saves = scratch(&format!("made-mix-{label}-saves"));
        let (code, text, stderr) = save_under(&repo, &saves, ZEROS);
        assert_eq!(code, 1, "{label}: {text}{stderr}");
        assert!(
            stderr.contains("scenario-1-trial-2#1")
                && stderr.contains("no revision")
                && binary.is_none_or(|binary| stderr.contains(binary)),
            "{label}: {stderr}"
        );
        assert!(!text.contains("saved:"), "{label}: {text}");
        assert_eq!(
            fs::read_dir(&*saves).expect("the directory").count(),
            0,
            "{label}: nothing is written"
        );
    }

    let unknown = Some("unknown revision");
    for (label, binaries) in [
        ("none", [None, None]),
        ("unknown", [unknown, unknown]),
        ("unknown-and-none", [unknown, None]),
    ] {
        let repo = repository(&format!("made-{label}"), &[]);
        for (n, binary) in binaries.into_iter().enumerate() {
            let case = format!("scenario-1-trial-{}", n + 1);
            run_made_with(&repo, &case, 5, "NoProgress", binary);
        }
        let saves = scratch(&format!("made-{label}-saves"));
        let (code, text, stderr) = save_under(&repo, &saves, ZEROS);
        assert_eq!(code, 0, "{label}: {text}{stderr}");
        assert!(
            saves.join(format!("scorecard-{ZEROS}.json")).is_file(),
            "{label}: {text}"
        );
    }
}

/// A saved scorecard that does not read is refused, exit 1, the reason on stderr.
#[test]
fn against_a_file_that_does_not_read_is_refused() {
    let repo = repository("refused", &[("scenario-1-trial-1", 5, "NoProgress")]);
    let bad = repo.join("not-a-scorecard.json");
    fs::write(&bad, "{").expect("a file");
    let (code, text, stderr) = takt(&[
        "scorecard",
        "--repo",
        &repo.display().to_string(),
        "--against",
        &bad.display().to_string(),
    ]);
    assert_eq!(code, 1, "{text}{stderr}");
    assert!(stderr.contains("not-a-scorecard.json"), "{stderr}");
}

/// The help documents the exit codes.
#[test]
fn help_lists_the_exit_codes() {
    let (code, text, stderr) = takt(&["scorecard", "--help"]);
    assert_eq!(code, 0, "{stderr}");
    for line in ["Exit codes:", "  0  ", "  1  ", "  2  ", "  3  "] {
        assert!(text.contains(line), "{line} in {text}");
    }
}

/// story:scorecard-help-names-the-held-out-check: the help names the held-out check beside the
/// record as where a scenario run's outcome is read from, and no longer says `events.jsonl` only.
#[test]
fn help_names_the_held_out_check_as_a_scenario_runs_outcome() {
    let (code, text, stderr) = takt(&["scorecard", "--help"]);
    assert_eq!(code, 0, "{stderr}");
    let help = text.split_whitespace().collect::<Vec<_>>().join(" ");
    assert!(!help.contains("events.jsonl` only"), "{text}");
    assert!(
        help.contains("read from the case's `events.jsonl` and, for a scenario run's outcome, from the held-out check beside it"),
        "{text}"
    );
}

/// story:scorecard-against-needs-the-trials: against a saved 5/5 baseline under the default
/// rule, a candidate with one case (outcome error) gets no verdict: the scenario is printed as
/// `too few cases (1 of 5)` and the command exits non-zero, as promotion refuses; with five
/// cases the verdict is given as before.
#[test]
fn against_gives_no_verdict_on_fewer_cases_than_the_trials() {
    let names: Vec<String> = (1..=10).map(|n| format!("scenario-1-trial-{n}")).collect();
    fn accepted(cases: &[String]) -> Vec<(&str, u64, &str)> {
        cases
            .iter()
            .map(|case| (case.as_str(), 500, "Completed (accepted)"))
            .collect()
    }
    let baseline = repository("trials-baseline", &accepted(&names[..5]));
    let saves = scratch("trials-saves");
    let (code, text, stderr) = takt(&[
        "scorecard",
        "--repo",
        &baseline.display().to_string(),
        "--save",
        &saves.display().to_string(),
        "--revision",
        REVISION,
    ]);
    assert_eq!(code, 0, "{text}{stderr}");
    let saved = saves.join(format!("scorecard-{REVISION}.json"));

    let one = repository("trials-one", &[("scenario-1-trial-6", 500, "error")]);
    let (code, text, stderr) = takt(&[
        "scorecard",
        "--repo",
        &one.display().to_string(),
        "--against",
        &saved.display().to_string(),
    ]);
    assert_ne!(code, 0, "{text}{stderr}");
    assert!(
        text.contains("scenario 1  too few cases (1 of 5)"),
        "{text}{stderr}"
    );
    assert!(
        !text.contains("scenario 1  outcomes by rank"),
        "no verdict: {text}"
    );
    assert!(stderr.contains("too few cases"), "{stderr}");

    let five = repository("trials-five", &accepted(&names[5..]));
    let (code, text, stderr) = takt(&[
        "scorecard",
        "--repo",
        &five.display().to_string(),
        "--against",
        &saved.display().to_string(),
    ]);
    assert_eq!(code, 0, "{text}{stderr}");
    assert!(
        text.contains("scenario 1  outcomes by rank held  tokens held  not worse"),
        "{text}"
    );
    assert!(!text.contains("too few"), "{text}");
}

/// story:scorecard-against-needs-the-trials, review N7: `--against` applies the trials as a
/// promotion does: a saved side with fewer cases than the rule's trials, and a scenario only the
/// candidate has with fewer, get no verdict, and the command exits non-zero naming them.
#[test]
fn against_applies_the_trials_to_the_saved_side_and_to_new_scenarios() {
    let names: Vec<String> = (1..=13).map(|n| format!("scenario-1-trial-{n}")).collect();
    fn accepted(cases: &[String]) -> Vec<(&str, u64, &str)> {
        cases
            .iter()
            .map(|case| (case.as_str(), 500, "Completed (accepted)"))
            .collect()
    }
    let thin = repository("trials-thin", &accepted(&names[..3]));
    let saves = scratch("trials-thin-saves");
    let (code, text, stderr) = takt(&[
        "scorecard",
        "--repo",
        &thin.display().to_string(),
        "--save",
        &saves.display().to_string(),
        "--revision",
        REVISION,
    ]);
    assert_eq!(code, 0, "{text}{stderr}");
    let saved = saves.join(format!("scorecard-{REVISION}.json"));
    let five = repository("trials-five-cases", &accepted(&names[3..8]));
    let (code, text, stderr) = takt(&[
        "scorecard",
        "--repo",
        &five.display().to_string(),
        "--against",
        &saved.display().to_string(),
    ]);
    assert_ne!(code, 0, "{text}{stderr}");
    assert!(
        text.contains("scenario 1  too few cases on the saved side (3 of 5)"),
        "{text}{stderr}"
    );
    assert!(
        !stderr.contains("on the candidate's side") && stderr.contains("on each side"),
        "the saved side is not the candidate's: {stderr}"
    );
    let (code, help, _) = takt(&["scorecard", "--help"]);
    assert_eq!(code, 0);
    assert!(
        !help.contains("it has no run"),
        "no exit 3 for a missing run: {help}"
    );
    assert!(
        !text.contains("scenario 1  outcomes by rank"),
        "no verdict: {text}"
    );

    let full = repository("trials-full", &accepted(&names[8..13]));
    let saves = scratch("trials-full-saves");
    let (code, text, stderr) = takt(&[
        "scorecard",
        "--repo",
        &full.display().to_string(),
        "--save",
        &saves.display().to_string(),
        "--revision",
        REVISION,
    ]);
    assert_eq!(code, 0, "{text}{stderr}");
    let saved = saves.join(format!("scorecard-{REVISION}.json"));
    let mut runs = accepted(&names[3..8]);
    runs.push(("scenario-2-trial-1", 500, "Completed (accepted)"));
    let new = repository("trials-new-scenario", &runs);
    let (code, text, stderr) = takt(&[
        "scorecard",
        "--repo",
        &new.display().to_string(),
        "--against",
        &saved.display().to_string(),
    ]);
    assert_ne!(code, 0, "{text}{stderr}");
    assert!(
        text.contains("scenario 2  too few cases (1 of 5)") && stderr.contains("scenario 2"),
        "{text}{stderr}"
    );
}

/// `--against <saved>` with this repository's runs; the exit code, stdout and stderr.
fn against(repo: &Path, saved: &Path) -> (i32, String, String) {
    takt(&[
        "scorecard",
        "--repo",
        &repo.display().to_string(),
        "--against",
        &saved.display().to_string(),
    ])
}

/// `takt scorecard --save` of `repo` under [`REVISION`] in a directory of its own; the file.
fn saved(repo: &Path, saves: &Path) -> PathBuf {
    let (code, text, stderr) = takt(&[
        "scorecard",
        "--repo",
        &repo.display().to_string(),
        "--save",
        &saves.display().to_string(),
        "--revision",
        REVISION,
    ]);
    assert_eq!(code, 0, "{text}{stderr}");
    saves.join(format!("scorecard-{REVISION}.json"))
}

/// Five accepted cases of `scenario`, trials `from` to `from + 4`, at 500 tokens each.
fn five_accepted(scenario: u32, from: usize) -> Vec<(String, u64, &'static str)> {
    (from..from + 5)
        .map(|n| {
            (
                format!("scenario-{scenario}-trial-{n}"),
                500,
                "Completed (accepted)",
            )
        })
        .collect()
}

/// A committed repository holding `runs`, owned.
fn repository_of(name: &str, runs: &[(String, u64, &'static str)]) -> Scratch {
    let runs: Vec<(&str, u64, &str)> = runs
        .iter()
        .map(|(case, tokens, stop)| (case.as_str(), *tokens, *stop))
        .collect();
    repository(name, &runs)
}

/// One repository of scenario 1 only and one of scenarios 1 and 2, five accepted cases each.
fn one_and_both(name: &str) -> (Scratch, Scratch) {
    let one = repository_of(&format!("{name}-one"), &five_accepted(1, 1));
    let mut both = five_accepted(1, 6);
    both.extend(five_accepted(2, 6));
    (one, repository_of(&format!("{name}-both"), &both))
}

/// story:the-judge-requires-the-whole-corpus: `--against` with no scenario case on either side
/// is no corpus: exit 1, saying there is no scenario case, never `not worse`.
#[test]
fn against_refuses_empty_against_empty() {
    let empty = repository("corpus-empty", &[]);
    let saves = scratch("corpus-empty-saves");
    let nothing = saved(&empty, &saves);
    let (code, text, stderr) = against(&empty, &nothing);
    assert_eq!(code, 1, "empty against empty: {text}{stderr}");
    assert!(stderr.contains("no scenario case"), "{stderr}");
}

/// story:the-judge-requires-the-whole-corpus: a scenario only this side has, with the rule's five
/// cases, is refused with exit 1 naming it, printed `not in the saved scorecard`; never `no
/// baseline`, never a verdict.
#[test]
fn against_refuses_a_scenario_only_this_side_has() {
    let (one, both) = one_and_both("corpus-this-side");
    let saves = scratch("corpus-this-side-saves");
    let only_one = saved(&one, &saves);
    let (code, text, stderr) = against(&both, &only_one);
    assert_eq!(code, 1, "a scenario only this side has: {text}{stderr}");
    assert!(
        text.contains("scenario 2  not in the saved scorecard"),
        "{text}"
    );
    assert!(
        stderr.contains("scenario 2") && stderr.contains("not in the saved scorecard"),
        "{stderr}"
    );
    assert!(!text.contains("no baseline"), "{text}");
    assert!(
        !text.contains("scenario 2  outcomes by rank"),
        "no verdict: {text}"
    );
}

/// story:the-judge-requires-the-whole-corpus: a scenario only the saved side has, with the rule's
/// five cases there, is refused with exit 1 naming it.
#[test]
fn against_refuses_a_scenario_only_the_saved_side_has() {
    let (one, both) = one_and_both("corpus-saved-side");
    let saves = scratch("corpus-saved-side-saves");
    let with_two = saved(&both, &saves);
    let (code, text, stderr) = against(&one, &with_two);
    assert_eq!(
        code, 1,
        "a scenario only the saved side has: {text}{stderr}"
    );
    assert!(
        text.contains("scenario 2  too few cases (0 of 5)") && stderr.contains("scenario 2"),
        "{text}{stderr}"
    );
}

/// story:the-judge-requires-the-whole-corpus: `takt scorecard --help` names both new refusals.
#[test]
fn help_names_the_corpus_refusals() {
    let (code, help, stderr) = takt(&["scorecard", "--help"]);
    assert_eq!(code, 0, "{stderr}");
    for refusal in ["no scenario case", "not in the saved scorecard"] {
        assert!(help.contains(refusal), "{refusal} in {help}");
    }
}

/// story:outcome-metric-takes-version-3, acceptance 1: `Unperformable` ranking with the budget
/// stops and a `DiskLow` trial taking no rank changed what the outcome ranks mean, so the outcome
/// metric is at definition version 3: `takt scorecard` prints `outcome v3` and a saved scorecard
/// records it.
#[test]
fn the_outcome_metric_is_printed_and_saved_at_definition_version_3() {
    let repo = repository(
        "outcome-v3",
        &[("scenario-1-trial-1", 1200, "Completed (accepted)")],
    );
    let (code, text, stderr) = takt(&["scorecard", "--repo", &repo.display().to_string()]);
    assert_eq!(code, 0, "{text}{stderr}");
    let definitions = text
        .lines()
        .find_map(|line| line.split_once("definitions: ").map(|(_, rest)| rest))
        .unwrap_or_else(|| panic!("the definitions in {text}"));
    assert!(
        definitions
            .split(", ")
            .any(|definition| definition == "outcome v3"),
        "{definitions}"
    );
    let saves = scratch("outcome-v3-saves");
    let card = fs::read_to_string(saved(&repo, &saves)).expect("the saved scorecard");
    assert_eq!(card.matches("\"outcome\": 3,").count(), 1, "{card}");
}

/// As [`run_made_with`], the `opened` line carrying `layers` and the step reporting `model`.
fn run_with_layers(repo: &Path, case: &str, layers: &[(&str, &str)], model: Option<&str>) {
    let dir = repo.join(CASES_DIR).join(case);
    fs::create_dir_all(&dir).expect("a case directory");
    fs::write(dir.join(takt::scorecard::HELD_OUT_FILE), "PASS\n").expect("the held-out result");
    let events = Events::new(&dir);
    let usage = TokenUsage {
        input: 100,
        output: 0,
        cache_read: 0,
        cache_creation: 0,
    };
    let layers: std::collections::BTreeMap<String, String> = layers
        .iter()
        .map(|(name, revision)| ((*name).to_owned(), (*revision).to_owned()))
        .collect();
    for (step, event) in [
        (
            0,
            Event::Opened {
                case: case.to_owned(),
                protocol: "story.delivery/1".to_owned(),
                story: None,
                inputs: Vec::new(),
                base: None,
                binary: layers.get("takt").cloned(),
                layers: Some(layers),
                confinement: None,
                endpoint: None,
            },
        ),
        (
            1,
            Event::Step {
                by: "harness".to_owned(),
                name: "claude-code".to_owned(),
                action: "implementation.edit".to_owned(),
                summary: "edited".to_owned(),
                tokens: Some(usage.total()),
                usage: Some(usage),
                revision: None,
                model: model.map(str::to_owned),
                version: None,
                cost_usd: None,
                duration_ms: None,
                api_ms: None,
                turns: None,
                started_at: None,
                transcript: None,
                transcript_refused: None,
            },
        ),
        (
            1,
            Event::Stop {
                stop: "Completed (accepted)".to_owned(),
                steps: 1,
                tokens: usage.total(),
                completed: Some(true),
            },
        ),
    ] {
        events.emit(step, &event).expect("the event is written");
    }
}

/// story:a-trial-records-the-revision-of-each-layer (ADR 0101) through the binary: the
/// scorecard prints the layers of the runs it scores; runs whose layers differ are refused,
/// exit 1, naming the layer and the runs, unless `--across-layers` asks to compare across them.
#[test]
fn runs_whose_layers_differ_are_refused_unless_across_layers() {
    let repo = repository("layers", &[]);
    let takt_a = "0123456789abcdef0123456789abcdef01234567";
    run_with_layers(
        &repo,
        "scenario-1-trial-1",
        &[("takt", takt_a), ("harness", "2.1.304 (Claude Code)")],
        Some("claude-opus-5-5"),
    );
    run_with_layers(
        &repo,
        "scenario-1-trial-2",
        &[("takt", takt_a), ("harness", "2.1.305 (Claude Code)")],
        Some("claude-opus-5-5"),
    );
    let path = repo.display().to_string();
    let (code, stdout, stderr) = takt(&["scorecard", "--repo", &path]);
    assert_eq!(code, 1, "{stdout}{stderr}");
    assert!(stderr.contains("layer `harness`"), "{stderr}");
    assert!(stderr.contains("--across-layers"), "{stderr}");
    assert!(
        !stdout.contains("scenario-1-trial-1#1"),
        "nothing scored: {stdout}"
    );

    let (code, stdout, stderr) = takt(&["scorecard", "--repo", &path, "--across-layers"]);
    assert_eq!(code, 0, "{stdout}{stderr}");
    assert!(stdout.contains("2.1.304 (Claude Code)"), "{stdout}");
    assert!(stdout.contains("2.1.305 (Claude Code)"), "{stdout}");
    assert!(stdout.contains("model claude-opus-5-5"), "{stdout}");

    let (code, text, stderr) = takt(&["scorecard", "--help"]);
    assert_eq!(code, 0, "{stderr}");
    assert!(text.contains("--across-layers"), "{text}");
    assert!(text.contains("layers differ"), "{text}");
}

/// story:the-scorecard-names-no-metric-it-did-not-compute, acceptance 1: against a saved
/// scorecard whose `step_tokens` definition differs from this takt's, absent as in a file saved
/// before the metric (`v-`) or at another version, `takt scorecard --against` prints no `median
/// step_tokens` line: the metric is a map by action with no per-case figure, and no line shows
/// another metric's figures (wall time) under its name. The verdict is the one it was.
#[test]
fn against_a_changed_step_tokens_definition_prints_no_median_of_it() {
    let baseline = repository_of("step-tokens-baseline", &five_accepted(1, 1));
    let candidate = repository_of("step-tokens-candidate", &five_accepted(1, 6));
    for (n, other) in [None, Some(1)].into_iter().enumerate() {
        let saves = scratch(&format!("step-tokens-saves-{n}"));
        let path = saved(&baseline, &saves);
        let mut file: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&path).expect("the saved scorecard"))
                .expect("json");
        let metrics = file["metrics"].as_object_mut().expect("the definitions");
        let current = metrics
            .remove("step_tokens")
            .and_then(|version| version.as_u64())
            .expect("a step_tokens definition");
        if let Some(other) = other {
            assert_ne!(other, current, "another version");
            metrics.insert("step_tokens".to_owned(), other.into());
        }
        fs::write(&path, serde_json::to_string(&file).expect("json")).expect("the file");

        let (code, text, stderr) = against(&candidate, &path);
        assert_eq!(code, 0, "{text}{stderr}");
        assert!(!text.contains("median step_tokens"), "{text}");
        assert!(
            text.contains("scenario 1  outcomes by rank held  tokens held  not worse"),
            "{text}"
        );
    }
}
