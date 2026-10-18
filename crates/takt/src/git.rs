//! The few git reads and writes the kernel makes in a worktree, without a shell.

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
