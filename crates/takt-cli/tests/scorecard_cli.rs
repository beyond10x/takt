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
/// then `stop`; the held-out check beside the record passed.
fn run(repo: &Path, case: &str, tokens: u64, stop: &str) {
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
                binary: None,
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
                cost_usd: None,
                duration_ms: None,
                api_ms: None,
                turns: None,
                started_at: None,
            },
        ),
        (
            1,
            Event::Stop {
                stop: stop.to_owned(),
                steps: 1,
                tokens,
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
