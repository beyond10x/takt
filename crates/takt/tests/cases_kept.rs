//! story:a-cases-root-survives-worktree-cleanup. When takt creates its state root `.takt/` it
//! writes `.takt/.worktreekeep` holding `cases/` beside `.takt/.gitignore`, and leaves one that
//! exists alone, so a managed worktree's cleanup keeps every case record and its archive images
//! them. takt edits no root file of the repository. Scenario 1 drives the real `worktree`
//! binary found on `PATH`, release 0.19.0 or newer; without one the test fails and says so.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use takt::harness::{Harness, StepContext, StepReport};
use takt::validator::{TestsRun, Validator};
use takt::{Run, RunOptions};

/// The oldest `worktree` release that keeps what a nested `.worktreekeep` names.
const WORKTREE_AT_LEAST: (u32, u32, u32) = (0, 19, 0);

/// Runs `git <args>` in `tree`; its trimmed standard output.
fn git(tree: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .args(["-c", "user.name=t", "-c", "user.email=t@localhost"])
        .args(["-c", "commit.gpgsign=false", "-c", "maintenance.auto=false"])
        .args(args)
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_NOSYSTEM", "1")
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

/// A scratch directory under this crate's build directory; removed when dropped.
struct Scratch(PathBuf);

impl Scratch {
    fn new(name: &str) -> Self {
        let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
            .join(format!("cases-kept-{}-{name}", std::process::id()));
        if dir.exists() {
            fs::remove_dir_all(&dir).expect("a stale scratch directory is removed");
        }
        fs::create_dir_all(&dir).expect("the scratch directory");
        Self(dir)
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// A committed repository on `main` at `dir/repo` holding a README and `files`.
fn repository(dir: &Path, files: &[(&str, &str)]) -> PathBuf {
    let repo = dir.join("repo");
    fs::create_dir_all(&repo).expect("the repository");
    fs::write(repo.join("README.md"), "a story\n").expect("a file");
    for (path, text) in files {
        let path = repo.join(path);
        fs::create_dir_all(path.parent().expect("a parent")).expect("a parent directory");
        fs::write(&path, text).expect("a file");
    }
    git(&repo, &["init", "-q", "-b", "main"]);
    git(&repo, &["add", "-A"]);
    git(&repo, &["commit", "-q", "-m", "start"]);
    repo
}

fn options(id: &str) -> RunOptions {
    RunOptions {
        protocol: "story.delivery/1".to_owned(),
        intent: "deliver the story".to_owned(),
        inputs: Vec::new(),
        iterations: Some(1),
        hours: None,
        tokens: None,
        case_id: Some(id.to_owned()),
        gate: None,
    }
}

/// A harness that is never asked for a step.
struct Idle;

impl Harness for Idle {
    fn name(&self) -> &str {
        "idle"
    }

    fn step(&self, _context: &StepContext<'_>) -> anyhow::Result<StepReport> {
        Err(anyhow::anyhow!("no step"))
    }
}

/// Opens the case `id` in `tree` and drops the run, leaving the case on disk.
fn open_case(tree: &Path, id: &str) {
    let validators: Vec<Box<dyn Validator>> =
        vec![Box::new(TestsRun::new("true").expect("a command"))];
    let run = Run::open(tree, options(id), &Idle, &validators).expect("the case opens");
    drop(run);
}

/// Every file under `dir`, as its path relative to `base`.
fn files_under(base: &Path, dir: &Path) -> Vec<PathBuf> {
    let mut found = Vec::new();
    let mut pending = vec![dir.to_path_buf()];
    while let Some(next) = pending.pop() {
        for entry in fs::read_dir(&next).expect("a directory") {
            let path = entry.expect("an entry").path();
            if path.is_dir() {
                pending.push(path);
            } else {
                found.push(path.strip_prefix(base).expect("under base").to_path_buf());
            }
        }
    }
    found.sort();
    found
}

/// The `worktree` binary on `PATH`, checked to be 0.19.0 or newer. A missing or older one fails
/// the test with a message naming what to install; it never passes silently.
fn worktree() -> PathBuf {
    let path = std::env::var_os("PATH").unwrap_or_default();
    let found = std::env::split_paths(&path)
        .map(|dir| dir.join("worktree"))
        .find(|candidate| candidate.is_file())
        .unwrap_or_else(|| {
            panic!(
                "this test needs the `worktree` binary, release {}.{}.{} or newer, on PATH; none is there",
                WORKTREE_AT_LEAST.0, WORKTREE_AT_LEAST.1, WORKTREE_AT_LEAST.2
            )
        });
    let output = Command::new(&found)
        .arg("--version")
        .output()
        .expect("worktree runs");
    let text = String::from_utf8_lossy(&output.stdout).into_owned();
    let version: Vec<u32> = text
        .split_whitespace()
        .last()
        .unwrap_or("")
        .split('.')
        .map_while(|part| part.parse().ok())
        .collect();
    let seen = (
        version.first().copied().unwrap_or(0),
        version.get(1).copied().unwrap_or(0),
        version.get(2).copied().unwrap_or(0),
    );
    assert!(
        seen >= WORKTREE_AT_LEAST,
        "this test needs `worktree` {}.{}.{} or newer, which keeps what a nested .worktreekeep names; {} reports `{}`",
        WORKTREE_AT_LEAST.0,
        WORKTREE_AT_LEAST.1,
        WORKTREE_AT_LEAST.2,
        found.display(),
        text.trim()
    );
    found
}

/// A `worktree` run with its configuration and state under `home`, JSON on.
fn run_worktree(home: &Path, args: &[&str]) -> Output {
    Command::new(worktree())
        .arg("--json")
        .args(args)
        .env("XDG_CONFIG_HOME", home.join("config"))
        .env("XDG_STATE_HOME", home.join("state"))
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .output()
        .expect("worktree runs")
}

fn worktree_ok(home: &Path, args: &[&str]) -> serde_json::Value {
    let output = run_worktree(home, args);
    assert!(
        output.status.success(),
        "worktree {args:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).expect("worktree prints JSON")
}

/// Acceptance 1. In a managed worktree a case is opened; `worktree finish --discard-cache
/// --archive` keeps every file under `.takt/cases/<id>/`, and the archive's patch applied to a
/// fresh checkout of the tree's revision replays each of them to the same bytes.
#[test]
fn a_finished_tree_keeps_its_case() {
    let scratch = Scratch::new("finished");
    let workspace = scratch.0.join("workspace");
    let repo = repository(&workspace, &[]);
    let profile = scratch.0.join("profile.toml");
    fs::write(
        &profile,
        "version = 1\nname = 'test'\nexpire_after_seconds = 604800\nprotect_workspace_root = false\n",
    )
    .expect("a profile");
    let home = scratch.0.join("home");
    worktree_ok(
        &home,
        &[
            "activate",
            "--profile",
            profile.to_str().expect("utf-8"),
            "--workspace",
            workspace.to_str().expect("utf-8"),
        ],
    );
    let created = worktree_ok(
        &home,
        &[
            "create",
            "--repo",
            repo.to_str().expect("utf-8"),
            "--id",
            "kept",
            "--purpose",
            "a case under cleanup",
        ],
    );
    let tree = PathBuf::from(created["evidence"]["path"].as_str().expect("a path"));

    open_case(&tree, "kept");
    let case = tree.join(".takt/cases/kept");
    let kept = files_under(&tree, &case);
    assert!(
        kept.contains(&PathBuf::from(".takt/cases/kept/case.yaml")),
        "the case holds its record: {kept:?}"
    );
    let before: Vec<Vec<u8>> = kept
        .iter()
        .map(|path| fs::read(tree.join(path)).expect("a case file"))
        .collect();
    let head = git(&tree, &["rev-parse", "HEAD"]);

    let finished = worktree_ok(
        &home,
        &[
            "finish",
            "--discard-cache",
            "--archive",
            tree.to_str().expect("utf-8"),
        ],
    );
    let discarded: Vec<&str> = finished["cache"]["discarded"]
        .as_array()
        .expect("a list")
        .iter()
        .filter_map(|entry| entry["path"].as_str())
        .collect();
    assert!(
        discarded.iter().all(|path| !path.starts_with(".takt")),
        "worktree discarded part of .takt: {discarded:?}\n{finished}"
    );
    assert_eq!(
        files_under(&tree, &case),
        kept,
        "the case's files are where they were after the cleanup"
    );

    let archive = PathBuf::from(finished["archive"]["path"].as_str().expect("an archive"));
    let patch = archive.join("dirty.patch");
    let text = fs::read_to_string(&patch).expect("the archive holds a patch");
    for path in &kept {
        assert!(
            text.contains(&format!("diff --git a/{0} b/{0}\n", path.display())),
            "the archive's patch holds {}\n{text}",
            path.display()
        );
    }

    let replay = scratch.0.join("replay");
    git(
        &scratch.0,
        &[
            "clone",
            "-q",
            repo.to_str().expect("utf-8"),
            replay.to_str().expect("utf-8"),
        ],
    );
    git(&replay, &["checkout", "-q", "--detach", &head]);
    git(&replay, &["apply", patch.to_str().expect("utf-8")]);
    for (path, bytes) in kept.iter().zip(&before) {
        assert_eq!(
            &fs::read(replay.join(path)).unwrap_or_default(),
            bytes,
            "{} replays to the same bytes",
            path.display()
        );
    }
}

/// Acceptance 2. A `.takt/.worktreekeep` of other content, present when the case opens, is not
/// rewritten, byte for byte.
#[test]
fn an_existing_keep_file_is_left_alone() {
    let scratch = Scratch::new("existing");
    let other: &[u8] = b"!cases/\nbuild/\n\n# mine\n";
    let repo = repository(
        &scratch.0,
        &[(".takt/.worktreekeep", "!cases/\nbuild/\n\n# mine\n")],
    );

    open_case(&repo, "left-alone");

    assert_eq!(
        fs::read(repo.join(".takt/.worktreekeep")).expect("the file is still there"),
        other,
        "the existing .takt/.worktreekeep is left byte for byte"
    );
    assert!(
        repo.join(".takt/cases/left-alone/case.yaml").is_file(),
        "the case opened"
    );
}

/// Acceptance 1's rule where no keep file exists: the case's open writes `.takt/.worktreekeep`
/// holding `cases/` beside `.takt/.gitignore`.
#[test]
fn a_new_state_root_gets_its_keep_file_beside_its_gitignore() {
    let scratch = Scratch::new("written");
    let repo = repository(&scratch.0, &[]);

    open_case(&repo, "written");

    assert_eq!(
        fs::read_to_string(repo.join(".takt/.worktreekeep")).expect("takt writes the keep file"),
        "cases/\n"
    );
    assert_eq!(
        fs::read_to_string(repo.join(".takt/.gitignore")).expect("takt writes the gitignore"),
        "*\n"
    );
}

/// Acceptance 3. The repository's root `.gitignore` and `.worktreekeep` are byte-identical after
/// a case opens, and a repository with neither gets neither.
#[test]
fn no_root_file_is_touched() {
    let scratch = Scratch::new("root");
    let repo = repository(
        &scratch.0,
        &[(".gitignore", "target/\n"), (".worktreekeep", "target/\n")],
    );
    open_case(&repo, "root-untouched");
    assert_eq!(
        fs::read(repo.join(".gitignore")).expect("the file"),
        b"target/\n"
    );
    assert_eq!(
        fs::read(repo.join(".worktreekeep")).expect("the file"),
        b"target/\n"
    );
    assert_eq!(git(&repo, &["status", "--porcelain"]), "");

    let bare = repository(&scratch.0.join("bare-dir-parent"), &[]);
    open_case(&bare, "root-absent");
    assert!(!bare.join(".gitignore").exists(), "no root .gitignore");
    assert!(
        !bare.join(".worktreekeep").exists(),
        "no root .worktreekeep"
    );
}

/// A harness whose first judgement step runs `script` with `sh` in the tree and reports an edit.
struct Script(&'static str);

impl Harness for Script {
    fn name(&self) -> &str {
        "script"
    }

    fn step(&self, context: &StepContext<'_>) -> anyhow::Result<StepReport> {
        let status = Command::new("sh")
            .args(["-c", self.0])
            .current_dir(context.tree)
            .status()
            .expect("sh runs");
        assert!(status.success(), "{}", self.0);
        Ok(StepReport {
            action: "implementation.edit".to_owned(),
            summary: "edited".to_owned(),
            tokens: None,
        })
    }
}

/// A judgement step that empties `.takt/.worktreekeep`, which would let a cleanup discard the
/// cases, is refused and the file put back as takt wrote it, byte for byte.
#[test]
fn a_step_emptying_the_keep_file_is_refused_and_put_back() {
    let scratch = Scratch::new("emptied");
    let repo = repository(&scratch.0, &[]);
    let validators: Vec<Box<dyn Validator>> =
        vec![Box::new(TestsRun::new("true").expect("a command"))];
    let harness = Script(": > .takt/.worktreekeep");
    let mut chosen = options("emptied");
    chosen.iterations = Some(4);
    let mut run = Run::open(&repo, chosen, &harness, &validators).expect("opens");
    let mut out = Vec::new();
    let stop = run.run(&mut out);
    drop(run);
    let text = String::from_utf8_lossy(&out).into_owned();
    assert!(
        matches!(stop, Ok(takt::Stop::StepRefused(_))),
        "the step is refused: {stop:?}\n{text}"
    );
    assert_eq!(
        fs::read_to_string(repo.join(".takt/.worktreekeep")).unwrap_or_default(),
        "cases/\n",
        "the keep file is put back\n{text}"
    );
}
