//! story:binary-embeds-revision through the binary: `takt --version` prints the git revision
//! the binary was built from, `takt <version> (<sha>)`, with `-dirty` after the sha when the
//! tree held uncommitted changes to tracked files or an untracked file under `crates/`, and
//! `(unknown revision)` when the crate was not built from a git checkout that tracks it.

use std::path::Path;
use std::process::Command;

/// `git <args>` in `dir`, with no code from configuration and none of the caller's
/// environment that would point git at another repository, index or work tree (a build run by
/// a git hook or alias inherits `GIT_DIR`); its trimmed standard output when it succeeds, else
/// `None`.
fn git(dir: &Path, args: &[&str]) -> Option<String> {
    let mut command = Command::new("git");
    for name in [
        "GIT_DIR",
        "GIT_WORK_TREE",
        "GIT_COMMON_DIR",
        "GIT_INDEX_FILE",
        "GIT_OBJECT_DIRECTORY",
        "GIT_ALTERNATE_OBJECT_DIRECTORIES",
        "GIT_CONFIG_PARAMETERS",
        "GIT_CONFIG_COUNT",
        "GIT_GLOB_PATHSPECS",
        "GIT_NOGLOB_PATHSPECS",
        "GIT_ICASE_PATHSPECS",
        "GIT_LITERAL_PATHSPECS",
        "GIT_NAMESPACE",
        "GIT_CEILING_DIRECTORIES",
    ] {
        command.env_remove(name);
    }
    let output = command
        .args([
            "-c",
            "core.fsmonitor=false",
            "-c",
            "core.hooksPath=/dev/null",
        ])
        .args(args)
        .current_dir(dir)
        .output()
        .ok()?;
    output
        .status
        .success()
        .then(|| String::from_utf8_lossy(&output.stdout).trim().to_owned())
}

/// The revision this crate's tree is at, read here with git and not by the code under test:
/// the commit's sha, `-dirty` after it while a tracked file differs from it or an untracked,
/// unignored file other than a `Cargo.lock` lies under the workspace's `crates/`, or `unknown
/// revision` when the crate is
/// not tracked by a git checkout with a commit.
fn expected_revision() -> String {
    let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    if git(crate_dir, &["ls-files", "--error-unmatch", "Cargo.toml"]).is_none() {
        return "unknown revision".to_owned();
    }
    let Some(sha) = git(crate_dir, &["rev-parse", "HEAD"]) else {
        return "unknown revision".to_owned();
    };
    let status = git(
        crate_dir,
        &[
            "--no-optional-locks",
            "status",
            "--porcelain",
            "--untracked-files=no",
        ],
    )
    .expect("git status reads the tree");
    let untracked = git(
        crate_dir,
        &[
            "ls-files",
            "--others",
            "--exclude-standard",
            "--",
            ":(top)crates",
            ".",
            ":(exclude,glob)**/Cargo.lock",
        ],
    )
    .expect("git lists the untracked files");
    if status.is_empty() && untracked.is_empty() {
        sha
    } else {
        format!("{sha}-dirty")
    }
}

/// The acceptance: the binary prints its version and, in parentheses, the revision it was
/// built from: the sha of the commit this crate's tree is at, `-dirty` when a tracked file
/// differs from it or an untracked file lies under `crates/`.
#[test]
fn version_prints_the_revision_the_binary_was_built_from() {
    let output = Command::new(env!("CARGO_BIN_EXE_takt"))
        .arg("--version")
        .output()
        .expect("takt runs");
    assert!(output.status.success(), "{output:?}");
    let stdout = String::from_utf8(output.stdout).expect("utf-8");
    let expected = expected_revision();
    assert_eq!(
        stdout,
        format!("takt {} ({expected})\n", env!("CARGO_PKG_VERSION"))
    );
    if expected != "unknown revision" {
        let sha = expected.strip_suffix("-dirty").unwrap_or(&expected);
        assert_eq!(sha.len(), 40, "{stdout}");
        assert!(
            sha.bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)),
            "{stdout}"
        );
    }
}
