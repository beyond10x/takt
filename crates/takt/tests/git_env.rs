//! What reaches a git takt runs from the environment it inherited. One case in its own test
//! binary, because it sets the process's `PATH` and `GIT_TEST_COMMIT_GRAPH`: a stand-in `git`
//! first on the `PATH` writes the environment it was started with, then runs the real git.

use std::fs;
use std::path::Path;
use std::process::Command;

/// `GIT_TEST_COMMIT_GRAPH`, which makes git write a commit-graph after every commit, does not
/// reach a git takt runs, and neither does any variable that points git elsewhere.
#[test]
fn a_git_takt_runs_gets_no_commit_graph_test_variable() {
    let real = Command::new("sh")
        .args(["-c", "command -v git"])
        .output()
        .expect("sh runs");
    let real = String::from_utf8(real.stdout)
        .expect("utf-8")
        .trim()
        .to_owned();
    assert!(!real.is_empty(), "git is on the PATH");
    let dir =
        Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("git-env-{}", std::process::id()));
    if dir.exists() {
        fs::remove_dir_all(&dir).expect("a stale directory is removed");
    }
    let bin = dir.join("bin");
    let tree = dir.join("tree");
    fs::create_dir_all(&bin).expect("the bin directory");
    fs::create_dir_all(&tree).expect("the tree");
    let seen = dir.join("seen-env");
    let stand_in = bin.join("git");
    fs::write(
        &stand_in,
        format!(
            "#!/bin/sh\nenv > '{}'\nexec '{real}' \"$@\"\n",
            seen.display()
        ),
    )
    .expect("the stand-in");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        fs::set_permissions(&stand_in, fs::Permissions::from_mode(0o755)).expect("executable");
    }
    let status = Command::new(&real)
        .args(["init", "-q", "-b", "main"])
        .current_dir(&tree)
        .status()
        .expect("git runs");
    assert!(status.success(), "the tree is a repository");
    let path = format!(
        "{}:{}",
        bin.display(),
        std::env::var("PATH").unwrap_or_default()
    );
    // SAFETY: this binary holds this one case, so no other thread reads the environment.
    unsafe {
        std::env::set_var("PATH", path);
        std::env::set_var("GIT_TEST_COMMIT_GRAPH", "1");
    }
    takt::git::run(&tree, &["rev-parse", "--git-dir"]).expect("git runs");
    // Only git's own variables are kept: the environment may hold credentials.
    let env: String = fs::read_to_string(&seen)
        .expect("the stand-in ran")
        .lines()
        .filter(|line| line.starts_with("GIT_"))
        .map(|line| format!("{line}\n"))
        .collect();
    fs::remove_dir_all(&dir).expect("the directory is removed");
    assert!(
        !env.lines()
            .any(|line| line.starts_with("GIT_TEST_COMMIT_GRAPH=")),
        "{env}"
    );
    assert!(
        env.lines().any(|line| line == "GIT_NO_REPLACE_OBJECTS=1"),
        "{env}"
    );
}
