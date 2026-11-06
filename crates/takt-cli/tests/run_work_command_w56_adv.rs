//! Wave 56 adversary: where the `command` work source's `select` runs.
//!
//! The `aep` source reads the planning store with the tree put on the base first
//! (`to_base`, `crates/takt/src/run.rs`): "so that the store is read as the base has it, never as
//! a case's branch, which carries a model's commits". The `command` source's `select` runs "in
//! the repository" (`work_commands`, website/docs/reference/configuration.md), and the run takes
//! the next ready item from it as the frontier takes the next story. After a case finishes, the
//! tree is left on that case's branch; these cases ask whether the next `select` then runs on
//! the model's commits.

#[path = "../../takt/tests/support/own_home.rs"]
mod own_home;

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

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
    String::from_utf8_lossy(&output.stdout).trim().to_owned()
}

struct Scratch(PathBuf);

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn item(id: &str, title: &str, revision: &str) -> String {
    serde_json::json!({
        "format": "takt.work-item/1",
        "key": format!("command:{id}"),
        "title": title,
        "body": "the body",
        "revision": revision,
    })
    .to_string()
}

fn run(repo: &Path, config: &Path) -> (i32, String, String) {
    let output = Command::new(env!("CARGO_BIN_EXE_takt"))
        .args([
            "run",
            "--repo",
            repo.to_str().expect("utf-8"),
            "--harness",
            "none",
            "--iterations",
            "1",
            "--test-cmd",
            "true",
            "--config",
            config.to_str().expect("utf-8"),
        ])
        .output()
        .expect("takt runs");
    (
        output.status.code().expect("an exit code"),
        String::from_utf8_lossy(&output.stdout).into_owned(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
    )
}

fn cases(repo: &Path) -> Vec<PathBuf> {
    let dir = repo.join(".takt/cases");
    if !dir.exists() {
        return Vec::new();
    }
    let mut cases: Vec<PathBuf> = fs::read_dir(&dir)
        .expect("a readable cases directory")
        .map(|e| e.expect("an entry").path())
        .collect();
    cases.sort();
    cases
}

/// Exit 9's contract (`crates/takt-cli/src/main.rs`, the exit code table): a case declined at
/// its current case revision "stays open, a run on it at that case revision stops Declined
/// again before any step, an argument-less run passes it over". The `aep` source passes it over
/// (`Run::next_to_resume` through `resumable_undeclined`). With `work = ["command"]` the
/// argument-less run finds its open case through `takt::case::resumable_item`, which reads no
/// decline.
#[test]
fn an_argument_less_run_passes_a_declined_command_case_over() {
    let root = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("w56-adv-declined-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    let scratch = Scratch(root.clone());
    let repo = scratch.0.join("tree");
    let items = scratch.0.join("items");
    fs::create_dir_all(&repo).expect("the tree");
    fs::create_dir_all(&items).expect("the items");
    fs::write(repo.join("check.txt"), "a file\n").expect("a file");
    git(&repo, &["init", "-q", "-b", "main"]);
    git(&repo, &["add", "-A"]);
    git(&repo, &["commit", "-q", "-m", "start"]);
    fs::write(items.join("next.json"), item("X-1", "the item", "r1")).expect("an item");
    let config = scratch.0.join("run.toml");
    fs::write(
        &config,
        format!(
            "work = [\"command\"]\nwork_commands = {{ select = [\"cat\", \"{}/next.json\"] }}\n",
            items.display()
        ),
    )
    .expect("the run file");

    let (_, out, err) = run(&repo, &config);
    let opened = cases(&repo);
    assert_eq!(opened.len(), 1, "stdout {out} stderr {err}");
    // The case's last run stopped Declined at the case revision `case.yaml` holds, as a
    // judgement step that names `decline` leaves it.
    let doc: serde_json::Value = serde_yaml_ng::from_str(
        &fs::read_to_string(opened[0].join("case.yaml")).expect("case.yaml"),
    )
    .expect("case.yaml reads");
    let revision = doc["revision"].as_str().unwrap_or("r1").to_owned();
    let mut events = fs::read_to_string(opened[0].join("events.jsonl")).unwrap_or_default();
    events.push_str(
        &serde_json::json!({"event": "decline", "reason": "not mine to do", "case_revision": revision})
            .to_string(),
    );
    events.push('\n');
    events.push_str(
        &serde_json::json!({"event": "stop", "stop": "Declined (not mine to do)"}).to_string(),
    );
    events.push('\n');
    fs::write(opened[0].join("events.jsonl"), events).expect("the record");
    // Nothing else is ready.
    fs::write(items.join("next.json"), "").expect("no item");

    let (code, out, err) = run(&repo, &config);
    assert!(
        code != 9 && !out.contains("frontier: resumed"),
        "the argument-less run resumed the declined case and stopped Declined again (exit {code}):\nstdout {out}\nstderr {err}"
    );
}

/// The team's `select` reads the tracker's answer through a file of the repository
/// (`cat tracker/next.json`, run in the tree). The first case finishes on its branch, where a
/// step committed a new `tracker/next.json` naming an item the tracker never offered. The next
/// run's `select` must read the base, as the `aep` source reads its store there: it may not open
/// a case on what only the model's commit names.
#[test]
fn select_after_a_finished_case_does_not_run_on_the_cases_branch() {
    let root = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("w56-adv-select-branch-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    let scratch = Scratch(root.clone());
    let repo = scratch.0.join("tree");
    fs::create_dir_all(repo.join("tracker")).expect("the tree");
    fs::write(
        repo.join("tracker/next.json"),
        item("X-1", "the tracker's item", "r1"),
    )
    .expect("the tracker's answer");
    git(&repo, &["init", "-q", "-b", "main"]);
    git(&repo, &["add", "-A"]);
    git(&repo, &["commit", "-q", "-m", "start"]);
    let config = scratch.0.join("run.toml");
    fs::write(
        &config,
        "work = [\"command\"]\nwork_commands = { select = [\"cat\", \"tracker/next.json\"] }\n",
    )
    .expect("the run file");

    let (_, out, err) = run(&repo, &config);
    let opened = cases(&repo);
    assert_eq!(opened.len(), 1, "stdout {out} stderr {err}");
    let branch = git(&repo, &["branch", "--show-current"]);
    assert!(
        branch.starts_with("takt/command-x-1-"),
        "the run leaves the tree on its case's branch: {branch}\nstdout {out} stderr {err}"
    );

    // What a step of the model committed on the case's branch.
    fs::write(
        repo.join("tracker/next.json"),
        item("MODEL-1", "chosen by the model's commit", "m1"),
    )
    .expect("the model's edit");
    git(&repo, &["commit", "-q", "-am", "feat: a step's commit"]);
    // The case finishes.
    let decisions = opened[0].join("decisions");
    fs::create_dir_all(&decisions).expect("the decisions directory");
    fs::write(
        decisions.join("9999.json"),
        r#"{"claims":{},"outcomes":{"completed":{"status":"legitimate"}},"explanation":{"computed_from":{"at":"2026-11-06T12:00:00Z"}}}"#,
    )
    .expect("a finished decision");

    let (code, out, err) = run(&repo, &config);
    assert!(
        !out.contains("command:MODEL-1") && cases(&repo).len() == 1,
        "select ran on the case's branch {branch} and took the item the model's commit names (exit {code}):\nstdout {out}\nstderr {err}\ncases {:?}",
        cases(&repo)
    );
}
