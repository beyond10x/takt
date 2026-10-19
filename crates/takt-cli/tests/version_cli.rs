//! story:binary-embeds-revision through the binary: `takt --version` prints the git revision
//! the binary was built from, `takt <version> (<sha>)`, with `-dirty` after the sha when the
//! tree held uncommitted changes to tracked files or an untracked file under `crates/`,
//! `protocols/` or a root `.cargo/`, and `(unknown revision)` when the crate was not built from
//! a git checkout that tracks it. What `-dirty` means is decided by the build script's own
//! cases (`revision_build.rs`); this one checks what the binary prints.

use std::path::Path;
use std::process::Command;

/// The revision the build script read when this package was built: the one it embeds in the
/// binary, and in this test, as `TAKT_REVISION`. Never `git rev-parse HEAD` read now: the
/// checkout may move, or a file change, between the build and the run, and the binary still
/// prints the revision it was built from.
const BUILT_FROM: &str = env!("TAKT_REVISION");

/// Whether `sha` names a commit of the repository this crate lies in, asked of git with no code
/// from configuration and none of the caller's environment that would point git elsewhere.
fn is_a_commit(sha: &str) -> bool {
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
        "GIT_NAMESPACE",
        "GIT_CEILING_DIRECTORIES",
    ] {
        command.env_remove(name);
    }
    command
        .args([
            "-c",
            "core.fsmonitor=false",
            "-c",
            "core.hooksPath=/dev/null",
            "cat-file",
            "-e",
            &format!("{sha}^{{commit}}"),
        ])
        .current_dir(Path::new(env!("CARGO_MANIFEST_DIR")))
        .output()
        .is_ok_and(|output| output.status.success())
}

/// The acceptance: the binary prints its version and, in parentheses, the revision it was
/// built from: a commit's sha, `-dirty` after it or not, or `unknown revision`; the sha names a
/// commit of this repository, whatever `HEAD` is now.
#[test]
fn version_prints_the_revision_the_binary_was_built_from() {
    let output = Command::new(env!("CARGO_BIN_EXE_takt"))
        .arg("--version")
        .output()
        .expect("takt runs");
    assert!(output.status.success(), "{output:?}");
    let stdout = String::from_utf8(output.stdout).expect("utf-8");
    assert_eq!(
        stdout,
        format!("takt {} ({BUILT_FROM})\n", env!("CARGO_PKG_VERSION"))
    );
    if BUILT_FROM != "unknown revision" {
        let sha = BUILT_FROM.strip_suffix("-dirty").unwrap_or(BUILT_FROM);
        assert!(matches!(sha.len(), 40 | 64), "{stdout}");
        assert!(
            sha.bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)),
            "{stdout}"
        );
        assert!(is_a_commit(sha), "{sha} is no commit here: {stdout}");
    }
}
