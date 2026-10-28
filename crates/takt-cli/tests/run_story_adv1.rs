//! Adversarial cases for `takt run --story <id>` (story:run-names-its-story), through the
//! binary over a copy of the `one-ready` fixture store, as `run_story_cli.rs` builds it.

#[path = "../../takt/tests/support/own_home.rs"]
mod own_home;

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

const FIXTURES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../takt/tests/fixtures");
const STORIES: &str = ".engineering/planning/story";

fn git(tree: &Path, args: &[&str]) {
    let status = Command::new("git")
        .args(["-c", "user.name=t", "-c", "user.email=t@localhost"])
        .args(args)
        .current_dir(tree)
        .status()
        .expect("git runs");
    assert!(status.success(), "git {args:?}");
}

fn git_out(tree: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .args(args)
        .current_dir(tree)
        .output()
        .expect("git runs");
    assert!(output.status.success(), "git {args:?}");
    String::from_utf8_lossy(&output.stdout).trim().to_owned()
}

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

/// The `one-ready` store with `story:second-ready`, ready after `story:ready-one` in id
/// order, committed on `main`.
fn repository(name: &str) -> Scratch {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("run-story-adv1-{}-{name}", std::process::id()));
    if dir.exists() {
        fs::remove_dir_all(&dir).expect("a stale scratch tree is removed");
    }
    copy_dir(&Path::new(FIXTURES).join("one-ready"), &dir);
    let ready = fs::read_to_string(dir.join(STORIES).join("ready-one.md")).expect("ready-one.md");
    let second = ready
        .replace("ready-one", "second-ready")
        .replace("Make the check pass", "Make the second check pass")
        .replace("The one ready story", "A second ready story");
    fs::write(dir.join(STORIES).join("second-ready.md"), second).expect("a story");
    fs::write(dir.join("check.txt"), "broken\n").expect("a file");
    git(&dir, &["init", "-q", "-b", "main"]);
    git(&dir, &["add", "-A"]);
    git(&dir, &["commit", "-q", "-m", "start"]);
    Scratch(dir)
}

fn run(repo: &Path, extra: &[&str]) -> (i32, String, String) {
    // These tests run the first delivery protocol; the second one declares inputs they do not set.
    let pinned: &[&str] = if extra.contains(&"--protocol") {
        &[]
    } else {
        &["--protocol", "story.delivery/1"]
    };
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
        ])
        .args(pinned)
        .args(extra)
        .output()
        .expect("takt runs");
    (
        output.status.code().expect("an exit code"),
        String::from_utf8_lossy(&output.stdout).into_owned(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
    )
}

fn status(repo: &Path, id: &str) -> String {
    takt::frontier::Aep::default()
        .list(repo)
        .expect("the store lists")
        .into_iter()
        .find(|a| a.id == id)
        .map(|a| a.status)
        .unwrap_or_else(|| panic!("{id} is listed"))
}

fn case_names(repo: &Path) -> Vec<String> {
    let dir = repo.join(".takt/cases");
    if !dir.exists() {
        return Vec::new();
    }
    let mut names: Vec<String> = fs::read_dir(&dir)
        .expect("a readable cases directory")
        .map(|e| {
            e.expect("an entry")
                .file_name()
                .to_string_lossy()
                .into_owned()
        })
        .collect();
    names.sort();
    names
}

fn branch(repo: &Path) -> String {
    git_out(repo, &["rev-parse", "--abbrev-ref", "HEAD"])
}

/// A refused `--story` says no case was opened and is exit 1, "refused before the run"; it must
/// leave the tree where it was. After any run the tree is on the case's branch, so an operator's
/// next `takt run --story <typo>` starts there: the refusal moves the tree to the case's base
/// (`to_base` inside `take_next_as`, before the narrowed frontier is read) and leaves it there.
#[test]
fn a_refused_story_leaves_the_tree_on_the_branch_it_was_on() {
    let repo = repository("refusal-moves-head");
    let (code, out, err) = run(&repo, &[]);
    assert_eq!(code, 7, "stdout {out} stderr {err}");
    assert!(out.starts_with("frontier: took story:ready-one"), "{out}");
    let before = branch(&repo);
    assert!(before.starts_with("takt/story-ready-one-"), "{before}");
    let head_before = git_out(&repo, &["rev-parse", "HEAD"]);

    let (code, out, err) = run(&repo, &["--story", "story:no-such"]);
    assert_eq!(code, 1, "stdout {out} stderr {err}");
    assert!(err.contains("no case was opened"), "{err}");
    assert_eq!(
        branch(&repo),
        before,
        "a refused --story moved the tree off the branch it was on: stderr {err}"
    );
    assert_eq!(git_out(&repo, &["rev-parse", "HEAD"]), head_before);
}

/// From a tree on `main`, a refused `--story` writes nothing: no case directory, no branch, no
/// commit, no change in the tree, the store as it was.
#[test]
fn a_refused_story_from_the_base_writes_nothing() {
    for story in [
        "story:no-such",
        "second-ready",
        "",
        "story:sécond-ready",
        "story:blocked-one",
    ] {
        let repo = repository(&format!("refusal-writes-nothing-{}", story.len()));
        let head = git_out(&repo, &["rev-parse", "HEAD"]);
        let (code, out, err) = run(&repo, &["--story", story]);
        assert_eq!(code, 1, "{story:?}: stdout {out} stderr {err}");
        assert!(err.contains("no case was opened"), "{story:?}: {err}");
        assert!(case_names(&repo).is_empty(), "{story:?}");
        assert_eq!(branch(&repo), "main", "{story:?}");
        assert_eq!(git_out(&repo, &["rev-parse", "HEAD"]), head, "{story:?}");
        assert_eq!(
            git_out(&repo, &["branch", "--list", "takt/*"]),
            "",
            "{story:?}"
        );
        assert_eq!(git_out(&repo, &["status", "--porcelain"]), "", "{story:?}");
        assert_eq!(status(&repo, "story:second-ready"), "proposed", "{story:?}");
    }
}

/// Two story cases open in one tree, the second taken by `--story` while the first was open:
/// the named one forks from the first's base, not its branch; a plain run then resumes the
/// first, and `--story` the second, so neither is lost and no third case opens.
#[test]
fn two_open_cases_both_stay_resumable() {
    let repo = repository("two-open");
    let (code, out, err) = run(&repo, &[]);
    assert_eq!(code, 7, "stdout {out} stderr {err}");
    let (code, out, err) = run(&repo, &["--story", "story:second-ready"]);
    assert_eq!(code, 7, "stdout {out} stderr {err}");
    let names = case_names(&repo);
    assert_eq!(names.len(), 2, "{names:?}");
    let second_branch = format!("takt/{}", names[1]);
    let subjects = git_out(&repo, &["log", "--format=%s", &second_branch]);
    assert!(
        !subjects.contains("takt: take story:ready-one"),
        "the named story's case forks from the first case's branch: {subjects}"
    );

    let (code, out, err) = run(&repo, &[]);
    assert_eq!(code, 7, "stdout {out} stderr {err}");
    assert!(
        out.starts_with(&format!(
            "frontier: resumed story:ready-one: Make the check pass (case {})\n",
            names[0]
        )),
        "{out}"
    );
    let (code, out, err) = run(&repo, &["--story", "story:second-ready"]);
    assert_eq!(code, 7, "stdout {out} stderr {err}");
    assert!(
        out.starts_with(&format!(
            "frontier: resumed story:second-ready: Make the second check pass (case {})\n",
            names[1]
        )),
        "{out}"
    );
    assert_eq!(case_names(&repo), names, "no third case");
}

/// `--config` carries no `story`: a file naming one is refused before any case opens, naming
/// the key, so the story never comes from a file nobody passed on the command line.
#[test]
fn a_config_file_does_not_name_the_story() {
    let repo = repository("config-story");
    let config = repo.join("run.toml");
    fs::write(&config, "story = \"story:second-ready\"\n").expect("a config");
    let (code, out, err) = run(&repo, &["--config", config.to_str().expect("utf-8")]);
    assert_eq!(code, 1, "stdout {out} stderr {err}");
    assert!(err.contains("story"), "{err}");
    assert!(case_names(&repo).is_empty());
    assert_eq!(status(&repo, "story:ready-one"), "proposed");
    assert_eq!(status(&repo, "story:second-ready"), "proposed");
}
