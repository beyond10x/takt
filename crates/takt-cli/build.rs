//! The revision the binary is built from, read with git at build time into `TAKT_REVISION`:
//! the commit's sha, `-dirty` after it while a tracked file differs from the commit or an
//! untracked file lies where the build can compile it in, or `unknown revision` when no git
//! checkout with a commit tracks this crate, or git fails; never a guess. Cargo builds it again
//! when HEAD, the ref it names, the index, a tracked file or the workspace's `crates/` changes,
//! so a commit, a checkout, an edit or a new source is never left out.

use std::path::{Path, PathBuf};
use std::process::Command;

/// What a build prints in place of a revision it could not read.
const UNKNOWN: &str = "unknown revision";

/// The directory, from the top of the checkout, whose untracked files a build can compile in
/// besides the crate's own: the workspace's crates.
const CRATES: &str = "crates";

/// The untracked file under [`compiled_in`] that does not make a revision dirty: a
/// `Cargo.lock`, which cargo writes itself on a build from the committed manifests and which
/// is not compiled in. A tracked one that differs is a change like any other.
const NOT_COMPILED_IN: &str = ":(exclude,glob)**/Cargo.lock";

/// What every git this script runs is told on its command line, over any configuration: run
/// no code from configuration (`core.fsmonitor`, which `status` runs, hooks, helpers, signing,
/// automatic maintenance). The list of `crates/takt/src/git.rs`, copied: a build script cannot
/// use the library its crate depends on.
const NO_CONFIGURED_CODE: [&str; 18] = [
    "-c",
    "core.fsmonitor=false",
    "-c",
    "core.hooksPath=/dev/null",
    "-c",
    "credential.helper=",
    "-c",
    "core.sshCommand=false",
    "-c",
    "commit.gpgSign=false",
    "-c",
    "tag.gpgSign=false",
    "-c",
    "gc.auto=0",
    "-c",
    "maintenance.auto=false",
    "-c",
    "log.showSignature=false",
];

/// The variables of the build's environment that would point git elsewhere than the crate's
/// checkout, or change how it reads a path or its configuration; none reaches a git this
/// script runs. Git exports `GIT_DIR` to every hook, `rebase -x` command and `!` alias, so a
/// build run from one would otherwise read another tree. The list of `crates/takt/src/git.rs`,
/// with the namespace and the discovery ceiling.
const REDIRECTING_ENV: [&str; 14] = [
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
];

fn main() {
    let dir = PathBuf::from(std::env::var_os("CARGO_MANIFEST_DIR").unwrap_or_default());
    println!("cargo:rustc-env=TAKT_REVISION={}", revision(&dir));
    for path in watched(&dir) {
        println!("cargo:rerun-if-changed={}", path.display());
    }
}

/// `git <args>` run in `dir`, with no code from configuration and none of the redirecting
/// environment: its standard output when it succeeds, else `None`.
fn git(dir: &Path, args: &[&str]) -> Option<String> {
    let mut command = Command::new("git");
    for name in REDIRECTING_ENV {
        command.env_remove(name);
    }
    let output = command
        .args(NO_CONFIGURED_CODE)
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

/// The top of the checkout `dir` lies in, and the pathspecs from there under which an
/// untracked file can be compiled in: the workspace's `crates/`, and the crate's own
/// directory when it lies elsewhere.
fn compiled_in(dir: &Path) -> Option<(PathBuf, Vec<String>)> {
    let root = PathBuf::from(git(dir, &["rev-parse", "--show-toplevel"])?.trim());
    let prefix = git(dir, &["rev-parse", "--show-prefix"])?;
    let own = prefix.trim().trim_end_matches('/');
    let mut specs = vec![format!(":(literal){CRATES}")];
    if own.is_empty() {
        specs.push(".".to_owned());
    } else if own != CRATES && !own.starts_with(&format!("{CRATES}/")) {
        specs.push(format!(":(literal){own}"));
    }
    Some((root, specs))
}

/// The revision of the crate at `dir`: the sha of the commit its checkout is at, `-dirty` after
/// it while `git status` names a tracked file that differs from that commit, staged or not, or
/// an untracked file no ignore rule covers lies under [`compiled_in`], a `Cargo.lock` aside
/// ([`NOT_COMPILED_IN`]); [`UNKNOWN`] when no checkout with a commit tracks the crate, or git
/// fails.
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
    let Some((root, specs)) = compiled_in(dir) else {
        return UNKNOWN.to_owned();
    };
    let mut args = vec!["ls-files", "--others", "--exclude-standard", "-z", "--"];
    args.extend(specs.iter().map(String::as_str));
    args.push(NOT_COMPILED_IN);
    let Some(untracked) = git(&root, &args) else {
        return UNKNOWN.to_owned();
    };
    if status.trim().is_empty() && untracked.is_empty() {
        sha.to_owned()
    } else {
        format!("{sha}-dirty")
    }
}

/// The files whose change may change [`revision`] for `dir`: HEAD; the ref HEAD names, or
/// its nearest existing directory while it is packed; `packed-refs` and `reftable` when they
/// exist; the index; every tracked file; and the directories of [`compiled_in`], where a new
/// source may appear ([`watch_tree`]). Paths into the git directory go through `git rev-parse
/// --git-path`, so a linked worktree's HEAD and index and the shared refs are the ones named. A
/// git file that does not exist is left out, since cargo runs a build script whose watched file
/// is missing on every build; a tracked file deleted from the tree stays in, so every build
/// reads the revision again until the file is back or its deletion committed. For a crate no
/// checkout tracks: its own directory, where a `git init` would make one, and the HEAD and
/// index of a repository it lies in, where an `add` and a commit would.
pub fn watched(dir: &Path) -> Vec<PathBuf> {
    let git_path = |name: &str| {
        git(
            dir,
            &["rev-parse", "--path-format=absolute", "--git-path", name],
        )
        .map(|path| dir.join(path.trim()))
        .filter(|path| path.exists())
    };
    let mut paths = Vec::new();
    if !tracked(dir) {
        paths.push(dir.to_path_buf());
        paths.extend(["HEAD", "index"].into_iter().filter_map(git_path));
        return paths;
    }
    for name in ["HEAD", "index", "packed-refs", "reftable"] {
        paths.extend(git_path(name));
    }
    if let Some(reference) = git(dir, &["symbolic-ref", "-q", "HEAD"])
        && let Some(loose) = git(
            dir,
            &[
                "rev-parse",
                "--path-format=absolute",
                "--git-path",
                reference.trim(),
            ],
        )
    {
        let loose = dir.join(loose.trim());
        paths.extend(
            loose
                .ancestors()
                .find(|path| path.exists())
                .map(Path::to_path_buf),
        );
    }
    let Some((root, specs)) = compiled_in(dir) else {
        return paths;
    };
    if let Some(files) = git(&root, &["ls-files", "-z"]) {
        paths.extend(
            files
                .split('\0')
                .filter(|file| !file.is_empty() && !file.contains('\n'))
                .map(|file| root.join(file)),
        );
    }
    let mut args = vec![
        "ls-files",
        "--others",
        "--ignored",
        "--exclude-standard",
        "--directory",
        "-z",
        "--",
    ];
    args.extend(specs.iter().map(String::as_str));
    let ignored: Vec<PathBuf> = git(&root, &args)
        .unwrap_or_default()
        .split('\0')
        .filter(|path| !path.is_empty())
        .map(|path| root.join(path.trim_end_matches('/')))
        .collect();
    for spec in &specs {
        let scope = match spec.strip_prefix(":(literal)") {
            Some(path) => root.join(path),
            None => root.clone(),
        };
        if scope.is_dir() {
            watch_tree(&scope, &ignored, &mut paths);
        }
    }
    paths
}

/// Watches `dir` whole when it holds no ignored path and no `.git`, else each of its entries
/// that is neither, by the same rule: cargo watches a directory with everything in it, and an
/// ignored one, a build directory above all, changes on every build, which would build the
/// revision again every time.
fn watch_tree(dir: &Path, ignored: &[PathBuf], paths: &mut Vec<PathBuf>) {
    if !dir.join(".git").exists() && !ignored.iter().any(|path| path.starts_with(dir)) {
        paths.push(dir.to_path_buf());
        return;
    }
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if entry.file_name() == ".git" || ignored.contains(&path) {
            continue;
        }
        if entry.file_type().is_ok_and(|kind| kind.is_dir()) {
            watch_tree(&path, ignored, paths);
        } else {
            paths.push(path);
        }
    }
}
