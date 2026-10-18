//! The revision the binary is built from, read with git at build time into `TAKT_REVISION`:
//! the commit's sha, `-dirty` after it while a tracked file differs from the commit, or
//! `unknown revision` when no git checkout with a commit tracks this crate, or git fails;
//! never a guess. Cargo builds it again when HEAD, the ref it names, the index or a tracked
//! file changes, so a commit, a checkout or an edit is never left out.

use std::path::{Path, PathBuf};
use std::process::Command;

/// What a build prints in place of a revision it could not read.
const UNKNOWN: &str = "unknown revision";

fn main() {
    let dir = PathBuf::from(std::env::var_os("CARGO_MANIFEST_DIR").unwrap_or_default());
    println!("cargo:rustc-env=TAKT_REVISION={}", revision(&dir));
    for path in watched(&dir) {
        println!("cargo:rerun-if-changed={}", path.display());
    }
}

/// `git <args>` run in `dir`: its standard output when it succeeds, else `None`.
fn git(dir: &Path, args: &[&str]) -> Option<String> {
    let output = Command::new("git")
        .args(args)
        .current_dir(dir)
        .output()
        .ok()?;
    output
        .status
        .success()
        .then(|| String::from_utf8_lossy(&output.stdout).into_owned())
}

/// Whether a git checkout tracks the crate whose directory is `dir`: one that does not, a
/// crate unpacked inside an unrelated repository, has no revision of its own.
fn tracked(dir: &Path) -> bool {
    git(dir, &["ls-files", "--error-unmatch", "Cargo.toml"]).is_some()
}

/// The revision of the crate at `dir`: the sha of the commit its checkout is at, `-dirty` after
/// it while `git status` names a tracked file that differs from that commit, staged or not;
/// [`UNKNOWN`] when no checkout with a commit tracks the crate, or git fails.
pub fn revision(dir: &Path) -> String {
    if !tracked(dir) {
        return UNKNOWN.to_owned();
    }
    let Some(sha) = git(dir, &["rev-parse", "--verify", "-q", "HEAD"]) else {
        return UNKNOWN.to_owned();
    };
    let sha = sha.trim();
    if !matches!(sha.len(), 40 | 64) || !sha.bytes().all(|b| b.is_ascii_hexdigit()) {
        return UNKNOWN.to_owned();
    }
    let Some(status) = git(
        dir,
        &[
            "--no-optional-locks",
            "status",
            "--porcelain",
            "--untracked-files=no",
        ],
    ) else {
        return UNKNOWN.to_owned();
    };
    if status.trim().is_empty() {
        sha.to_owned()
    } else {
        format!("{sha}-dirty")
    }
}

/// The files whose change may change [`revision`] for `dir`: HEAD; the ref HEAD names, or
/// its nearest existing directory while it is packed; `packed-refs` and `reftable` when they
/// exist; the index; and every tracked file. Paths into the git directory go through `git
/// rev-parse --git-path`, so a linked worktree's HEAD and index and the shared refs are the
/// ones named. None when no checkout tracks the crate. A git file that does not exist is left
/// out, since cargo runs a build script whose watched file is missing on every build; a tracked
/// file deleted from the tree stays in, so every build reads the revision again until the
/// file is back or its deletion committed.
pub fn watched(dir: &Path) -> Vec<PathBuf> {
    if !tracked(dir) {
        return Vec::new();
    }
    let git_path = |name: &str| {
        git(
            dir,
            &["rev-parse", "--path-format=absolute", "--git-path", name],
        )
        .map(|path| dir.join(path.trim()))
    };
    let mut paths = Vec::new();
    for name in ["HEAD", "index", "packed-refs", "reftable"] {
        paths.extend(git_path(name).filter(|path| path.exists()));
    }
    if let Some(reference) = git(dir, &["symbolic-ref", "-q", "HEAD"])
        && let Some(loose) = git_path(reference.trim())
    {
        paths.extend(
            loose
                .ancestors()
                .find(|path| path.exists())
                .map(Path::to_path_buf),
        );
    }
    if let Some(root) = git(dir, &["rev-parse", "--show-toplevel"]) {
        let root = PathBuf::from(root.trim());
        if let Some(files) = git(&root, &["ls-files", "-z"]) {
            paths.extend(
                files
                    .split('\0')
                    .filter(|file| !file.is_empty() && !file.contains('\n'))
                    .map(|file| root.join(file)),
            );
        }
    }
    paths
}
