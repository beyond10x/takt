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

/// Creates `branch` and checks it out, at `from` when given, else at `HEAD`; a branch that
/// exists is an error.
pub fn create_branch(tree: &Path, branch: &str, from: Option<&str>) -> Result<()> {
    let mut args = vec!["checkout", "-q", "-b", branch];
    args.extend(from);
    run(tree, &args)?;
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

/// Stages everything but what is under the cases directory (the directory itself, never a
/// path that merely starts with its name) and commits with `message`; answers the new `HEAD`.
pub fn commit_all(tree: &Path, message: &str) -> Result<String> {
    let exclude = format!(":(exclude){}/", crate::case::CASES_DIR);
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

/// Discards every uncommitted change, tracked and untracked, but what is under the cases
/// directory (the directory itself, never a path that merely starts with its name).
pub fn discard_all(tree: &Path) -> Result<()> {
    let keep = format!("{}/", crate::case::CASES_DIR);
    run(tree, &["checkout", "--", "."])?;
    run(tree, &["clean", "-fdq", "-e", &keep])?;
    Ok(())
}
