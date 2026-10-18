//! What reaches a git takt runs from the environment it inherited. One case in its own test
//! binary, because it sets the process's `PATH` and git's variables: a stand-in `git`
//! first on the `PATH` writes the environment it was started with, then runs the real git.

use std::fs;
use std::path::Path;
use std::process::Command;

/// `GIT_TEST_COMMIT_GRAPH` (a commit-graph written after every commit), `GIT_GRAFT_FILE` (parents
/// a file names) and `GIT_SHALLOW_FILE` (history cut where a file says) do not reach a git takt
/// runs: each would change what an ancestry or a history read answers.
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
        std::env::set_var("GIT_GRAFT_FILE", "/nonexistent/grafts");
        std::env::set_var("GIT_SHALLOW_FILE", "/nonexistent/shallow");
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
    for variable in [
        "GIT_TEST_COMMIT_GRAPH",
        "GIT_GRAFT_FILE",
        "GIT_SHALLOW_FILE",
    ] {
        assert!(
            !env.lines()
                .any(|line| line.starts_with(&format!("{variable}="))),
            "{variable} reached git: {env}"
        );
    }
    assert!(
        env.lines().any(|line| line == "GIT_NO_REPLACE_OBJECTS=1"),
        "{env}"
    );
}
