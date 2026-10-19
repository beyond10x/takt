//! Adversary pass 1 on story:scorecard-reader, through the binary. The case directories are
//! written through `takt::events`, as the run loop writes them.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use takt::case::CASES_DIR;
use takt::events::{Event, Events};

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

fn scratch(name: &str) -> Scratch {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("scorecard-cli-adv1-{}-{name}", std::process::id()));
    if dir.exists() {
        fs::remove_dir_all(&dir).expect("a stale scratch tree is removed");
    }
    fs::create_dir_all(&dir).expect("a scratch tree");
    Scratch(dir)
}

fn git(tree: &Path, args: &[&str]) {
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

/// Scenario 1 as documented: a first run that stops `ApprovalRequired (repository.merge)`, a
/// grant, and a resumed run that ends `accepted`; both in the one case's record, the held-out
/// check passed beside it.
fn scenario_one_with_its_grant(repo: &Path, case: &str) {
    let dir = repo.join(CASES_DIR).join(case);
    fs::create_dir_all(&dir).expect("a case directory");
    fs::write(dir.join(takt::scorecard::HELD_OUT_FILE), "PASS\n").expect("the held-out result");
    let events = Events::new(&dir);
    let opened = || Event::Opened {
        case: case.to_owned(),
        protocol: "story.delivery/1".to_owned(),
        story: Some("story:remove-a-todo".to_owned()),
        inputs: Vec::new(),
        base: None,
        binary: None,
    };
    let step = |tokens: u64| Event::Step {
        by: "harness".to_owned(),
        name: "claude-code".to_owned(),
        action: "implementation.edit".to_owned(),
        summary: "edited".to_owned(),
        tokens: Some(tokens),
        usage: None,
        revision: None,
        model: None,
        cost_usd: None,
        duration_ms: None,
        api_ms: None,
        turns: None,
        started_at: None,
        transcript: None,
        transcript_refused: None,
    };
    for (n, event) in [
        (0, opened()),
        (1, step(900)),
        (
            1,
            Event::Stop {
                stop: "ApprovalRequired (repository.merge)".to_owned(),
                steps: 1,
                tokens: 900,
                completed: None,
            },
        ),
        (0, opened()),
        (
            0,
            Event::Stop {
                stop: "Completed (accepted)".to_owned(),
                steps: 0,
                tokens: 0,
                completed: None,
            },
        ),
    ] {
        events.emit(n, &event).expect("the event is written");
    }
}

/// `--save`, then `--against` the file just saved, on the same unchanged repository of five
/// cases, the rule's trials: nothing can be worse than itself, so the exit is 0, not 3.
#[test]
fn a_saved_scorecard_against_itself_exits_0() {
    let repo = scratch("self");
    fs::write(repo.join("README"), "scratch\n").expect("a file");
    git(&repo, &["init", "-q", "-b", "main"]);
    git(&repo, &["add", "-A"]);
    git(&repo, &["commit", "-q", "-m", "start"]);
    for n in 1..=5 {
        scenario_one_with_its_grant(&repo, &format!("scenario-1-trial-{n}"));
    }
    let saves = scratch("self-saves");
    let repo_arg = repo.display().to_string();
    let saves_arg = saves.display().to_string();
    let (code, stdout, stderr) = takt(&[
        "scorecard",
        "--repo",
        &repo_arg,
        "--save",
        &saves_arg,
        "--revision",
        "5555555555555555555555555555555555555555",
    ]);
    assert_eq!(code, 0, "{stdout}\n{stderr}");
    let saved = fs::read_dir(&*saves)
        .expect("the save directory")
        .next()
        .expect("a saved file")
        .expect("an entry")
        .path();
    let saved_arg = saved.display().to_string();
    let (code, stdout, stderr) = takt(&["scorecard", "--repo", &repo_arg, "--against", &saved_arg]);
    assert_eq!(
        code, 0,
        "the same record against itself exits {code}:\n{stdout}\n{stderr}"
    );
}

/// `--save` on a repository with no commit yet: refused with exit 1 and the reason, and no
/// file is written.
#[test]
fn save_on_an_unborn_head_is_refused_and_writes_nothing() {
    let repo = scratch("unborn");
    git(&repo, &["init", "-q", "-b", "main"]);
    scenario_one_with_its_grant(&repo, "scenario-1-trial-1");
    let saves = scratch("unborn-saves");
    let (code, stdout, stderr) = takt(&[
        "scorecard",
        "--repo",
        &repo.display().to_string(),
        "--save",
        &saves.display().to_string(),
    ]);
    assert_eq!(code, 1, "{stdout}\n{stderr}");
    assert!(stderr.contains("HEAD"), "{stderr}");
    assert_eq!(
        fs::read_dir(&*saves).expect("the save directory").count(),
        0
    );
}
