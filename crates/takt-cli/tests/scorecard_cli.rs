//! story:scorecard-reader through the binary: `takt scorecard --repo <path>` prints a row per
//! run read from the runs' events only; `--save <dir>` writes `scorecard-<revision>.json` and
//! says what named it; `--against <file>` marks a row worse than the saved scenario's and exits
//! 3. The case directories are written through `takt::events`, as the run loop writes them.

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
/// then `stop`.
fn run(repo: &Path, case: &str, tokens: u64, stop: &str) {
    let dir = repo.join(CASES_DIR).join(case);
    fs::create_dir_all(&dir).expect("a case directory");
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

/// `--save` writes the scorecard named by `HEAD` of the repository when the binary knows no
/// revision, and says so; `--against` it marks a `NoProgress` run worse and exits 3.
#[test]
fn save_then_against_marks_a_worse_row_and_exits_3() {
    let baseline = repository(
        "baseline",
        &[("scenario-1-trial-1", 500, "Completed (accepted)")],
    );
    let saves = scratch("saves");
    let (code, text, stderr) = takt(&[
        "scorecard",
        "--repo",
        &baseline.display().to_string(),
        "--save",
        &saves.display().to_string(),
    ]);
    assert_eq!(code, 0, "{text}{stderr}");
    let named = match option_env!("TAKT_REVISION") {
        Some(revision) => revision.to_owned(),
        None => git(&baseline, &["rev-parse", "HEAD"]),
    };
    let saved = saves.join(format!("scorecard-{named}.json"));
    assert!(saved.is_file(), "{text}");
    assert!(text.contains(&saved.display().to_string()), "{text}");
    if option_env!("TAKT_REVISION").is_none() {
        assert!(text.contains("named by HEAD of"), "{text}");
    }

    let candidate = repository("candidate", &[("scenario-1-trial-2", 650, "NoProgress")]);
    let (code, text, stderr) = takt(&[
        "scorecard",
        "--repo",
        &candidate.display().to_string(),
        "--against",
        &saved.display().to_string(),
    ]);
    assert_eq!(code, 3, "{text}{stderr}");
    let line = text
        .lines()
        .filter(|line| line.starts_with("scenario-1-trial-2"))
        .find(|line| line.contains("worse"))
        .unwrap_or_else(|| panic!("a compared row in {text}"));
    assert!(!line.contains("not worse"), "{line}");
    assert!(text.contains("scenario 1  median tokens +150"), "{text}");

    let better = repository(
        "better",
        &[("scenario-1-trial-3", 400, "Completed (accepted)")],
    );
    let (code, text, stderr) = takt(&[
        "scorecard",
        "--repo",
        &better.display().to_string(),
        "--against",
        &saved.display().to_string(),
    ]);
    assert_eq!(code, 0, "{text}{stderr}");
    assert!(text.contains("not worse"), "{text}");
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
