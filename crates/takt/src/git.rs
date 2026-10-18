//! The few git reads and writes the kernel makes in a worktree, without a shell. A push goes
//! through the forge ([`crate::forge`]), under the bot route's credential, never through here.

use std::path::Path;
use std::process::Command;

use anyhow::{Context as _, Result, bail};

/// Runs `git <args>` in `tree` and answers its trimmed standard output.
pub fn run(tree: &Path, args: &[&str]) -> Result<String> {
    let output = Command::new("git")
        .args(args)
        .current_dir(tree)
        .env_remove("GIT_DIR")
        .env_remove("GIT_WORK_TREE")
        .output()
        .with_context(|| format!("run git {} in {}", args.join(" "), tree.display()))?;
    if !output.status.success() {
        bail!(
            "git {} in {} exited {}: {}",
            args.join(" "),
            tree.display(),
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_owned())
}

/// The commit `HEAD` names.
pub fn head(tree: &Path) -> Result<String> {
    run(tree, &["rev-parse", "HEAD"])
}

/// The branch `HEAD` is on; `None` when it is detached.
pub fn current_branch(tree: &Path) -> Result<Option<String>> {
    let name = run(tree, &["rev-parse", "--abbrev-ref", "HEAD"])?;
    Ok((name != "HEAD").then_some(name))
}

/// Creates `branch` at `HEAD` and checks it out. A branch that exists is an error, unless the
/// tree is already on it: a case's branch left behind when the store would not take the case
/// is reset to `HEAD` and used again.
pub fn create_branch(tree: &Path, branch: &str) -> Result<()> {
    let on_it = current_branch(tree)?.as_deref() == Some(branch);
    let flag = if on_it { "-B" } else { "-b" };
    run(tree, &["checkout", "-q", flag, branch])?;
    Ok(())
}

/// Checks out `target`, a branch, and deletes `branch`; `None` detaches `HEAD` instead.
pub fn leave_branch(tree: &Path, branch: &str, target: Option<&str>) -> Result<()> {
    match target {
        Some(target) => run(tree, &["checkout", "-q", target])?,
        None => run(tree, &["checkout", "-q", "--detach"])?,
    };
    run(tree, &["branch", "-q", "-D", branch])?;
    Ok(())
}

/// The URL of `remote`; an error when the tree has no such remote.
pub fn remote_url(tree: &Path, remote: &str) -> Result<String> {
    run(tree, &["remote", "get-url", remote])
}

/// The paths with uncommitted changes (staged, unstaged or untracked), relative to the tree.
pub fn changed_paths(tree: &Path) -> Result<Vec<String>> {
    let status = run(tree, &["status", "--porcelain", "--untracked-files=all"])?;
    Ok(status
        .lines()
        .filter(|line| line.len() > 3)
        .map(|line| line[3..].trim().to_owned())
        .collect())
}

/// Stages everything but the case directory and commits with `message`; answers the new `HEAD`.
pub fn commit_all(tree: &Path, message: &str) -> Result<String> {
    let exclude = format!(":(exclude){}", crate::case::CASES_DIR);
    run(tree, &["add", "-A", "--", ".", &exclude])?;
    run(
        tree,
        &[
            "-c",
            "user.name=takt",
            "-c",
            "user.email=takt@localhost",
            "commit",
            "-q",
            "-m",
            message,
        ],
    )?;
    head(tree)
}

/// Discards every uncommitted change, tracked and untracked, but the case directory.
pub fn discard_all(tree: &Path) -> Result<()> {
    run(tree, &["checkout", "--", "."])?;
    run(tree, &["clean", "-fdq", "-e", crate::case::CASES_DIR])?;
    Ok(())
}
