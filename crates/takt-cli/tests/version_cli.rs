//! story:binary-embeds-revision through the binary: `takt --version` prints the git revision
//! the binary was built from, `takt <version> (<sha>)`, with `-dirty` after the sha when the
//! tree held uncommitted changes to tracked files, and `(unknown revision)` when the crate was
//! not built from a git checkout that tracks it.

use std::path::Path;
use std::process::Command;

/// `git <args>` in `dir`: its trimmed standard output when it succeeds, else `None`.
fn git(dir: &Path, args: &[&str]) -> Option<String> {
    let output = Command::new("git")
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
/// the commit's sha, `-dirty` after it while a tracked file differs from it, or `unknown
/// revision` when the crate is not tracked by a git checkout with a commit.
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
    if status.is_empty() {
        sha
    } else {
        format!("{sha}-dirty")
    }
}

/// The acceptance: the binary prints its version and, in parentheses, the revision it was
/// built from: the sha of the commit this crate's tree is at, `-dirty` when a tracked file
/// differs from it.
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
