//! The revision the binary is built from, read with git at build time into `TAKT_REVISION`:
//! the commit's sha, `-dirty` after it while a tracked file differs from the commit or an
//! untracked file lies where the build can compile it in, or `unknown revision` when no git
//! checkout with a commit tracks this crate, or git fails; never a guess. Cargo builds it again
//! when HEAD, the ref it names, the index, the exclude file, a tracked file or the workspace's
//! `crates/`, `protocols/` or root `.cargo/` changes, so a commit, a checkout, an edit or a new
//! source is never left out; and on every build while the revision is unknown.

use std::path::{Path, PathBuf};
use std::process::Command;

/// What a build prints in place of a revision it could not read.
const UNKNOWN: &str = "unknown revision";

/// The directories, from the top of the checkout, whose untracked files a build can compile in
/// besides the crate's own: the workspace's crates, the protocol documents the binary embeds
/// (`crates/takt/src/protocol.rs`, `include_str!`), and the root `.cargo/`, whose
/// configuration cargo reads on a build from the root (`rustflags`, `[patch]`, `[env]`). A
/// root `.cargo/` made after a build is seen by the next build that reads the revision for
/// another reason, and until then a binary compiled with it prints the clean revision, a
/// known limit: a directory that does not exist is not watched, as cargo would run the script,
/// and build the crate again, on every build.
const COMPILED_IN: [&str; 3] = ["crates", "protocols", ".cargo"];

/// The untracked file under [`compiled_in`] that does not make a revision dirty: a
/// `Cargo.lock`, which cargo writes itself on a build from the committed manifests and which
/// is not compiled in. A tracked one that differs is a change like any other.
const NOT_COMPILED_IN: &str = ":(exclude,glob)**/Cargo.lock";

/// The path in the crate's directory nothing writes, watched while the revision is unknown so
/// that cargo, which runs a build script again whenever a watched path is missing, reads it
/// again on every build: a `git init` and a first commit anywhere above the crate are seen by
/// the next build. A build at a commit does not watch it.
const NEVER_WRITTEN: &str = ".takt-unknown-revision";

/// What every git this script runs is told on its command line, over any configuration: run
/// no code from configuration (`core.fsmonitor`, which `status` runs, hooks, helpers, signing,
/// automatic maintenance). The list of `crates/takt/src/git.rs`, copied: a build script cannot
/// use the library its crate depends on. A filter, which `status` runs on a file whose stat
/// differs from the index, is turned off twice: no attributes are read where
/// [`NO_ATTRIBUTES`] reaches, and every filter driver the configuration names, which the
/// repository's own `info/attributes` can still name, has its commands set empty
/// ([`no_filter_drivers`]).
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

/// What `status` is told so that it reads no attributes, and so names no filter driver to run:
/// none from the work tree (`GIT_ATTR_SOURCE` at the empty tree), none from the user's
/// attributes file and none from the system's. A file an attribute would have filtered or
/// converted then reads as it is on disk: when its stat differs from the index and its bytes
/// from the commit's, the revision is `-dirty` rather than read through the filter.
const NO_ATTRIBUTES: [&str; 2] = ["-c", "core.attributesFile=/dev/null"];

/// The empty tree's id, by the length of the repository's object ids: what `GIT_ATTR_SOURCE`
/// points at.
const EMPTY_TREE_SHA1: &str = "4b825dc642cb6eb9a060e54bf8d69288fbee4904";
const EMPTY_TREE_SHA256: &str = "6ef19b41225c5369f1c104d45d8d85efa9b057b53b14b4b9b939dd74decc5321";

/// The variables of the build's environment that would point git elsewhere than the crate's
/// checkout, or change how it reads a path, its attributes or its configuration; none reaches a
/// git this script runs. Git exports `GIT_DIR` to every hook, `rebase -x` command and `!`
/// alias, so a build run from one would otherwise read another tree. The list of
/// `crates/takt/src/git.rs`, with the namespace, the discovery ceiling and the attribute
/// source.
const REDIRECTING_ENV: [&str; 15] = [
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
    "GIT_ATTR_SOURCE",
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
    git_with(dir, args, &[])
}

/// [`git`] with `env` set on top.
fn git_with(dir: &Path, args: &[&str], env: &[(&str, &str)]) -> Option<String> {
    let output = git_command(dir, args, env).output().ok()?;
    output
        .status
        .success()
        .then(|| String::from_utf8_lossy(&output.stdout).into_owned())
}

/// The command `git <args>` in `dir`, with no code from configuration, none of the redirecting
/// environment, and `env` set on top.
fn git_command(dir: &Path, args: &[&str], env: &[(&str, &str)]) -> Command {
    let mut command = Command::new("git");
    for name in REDIRECTING_ENV {
        command.env_remove(name);
    }
    command
        .envs(env.iter().copied())
        .args(NO_CONFIGURED_CODE)
        .args(args)
        .current_dir(dir);
    command
}

/// What turns off every filter driver git's configuration for `dir` names
/// (`git config --name-only --get-regexp '^filter\.'`): for each, `-c filter.<driver>.clean=`,
/// `smudge=` and `process=`, which git reads as no command, and `required=false`, so that a
/// file of a required driver is read as it is rather than failing `status`. An attribute that
/// still names the driver, from the repository's own `info/attributes`, then runs nothing.
/// `None`, read as an unknown revision, when the listing fails or names a driver `-c` cannot
/// set (a name that is not UTF-8 or holds `=`).
fn no_filter_drivers(dir: &Path) -> Option<Vec<String>> {
    let output = git_command(
        dir,
        &["config", "-z", "--name-only", "--get-regexp", r"^filter\."],
        &[],
    )
    .output()
    .ok()?;
    let listed = match output.status.code() {
        Some(0) => output.stdout,
        Some(1) => Vec::new(),
        _ => return None,
    };
    let mut drivers: Vec<String> = Vec::new();
    for key in listed
        .split(|byte| *byte == 0)
        .filter(|key| !key.is_empty())
    {
        let key = std::str::from_utf8(key).ok()?;
        let (driver, _) = key.strip_prefix("filter.")?.rsplit_once('.')?;
        if driver.contains('=') {
            return None;
        }
        if !drivers.iter().any(|known| known == driver) {
            drivers.push(driver.to_owned());
        }
    }
    let mut args = Vec::new();
    for driver in drivers {
        for setting in ["clean=", "smudge=", "process=", "required=false"] {
            args.push("-c".to_owned());
            args.push(format!("filter.{driver}.{setting}"));
        }
    }
    Some(args)
}

/// Whether a git checkout tracks the crate whose directory is `dir`: one that does not, a
/// crate unpacked inside an unrelated repository, has no revision of its own.
fn tracked(dir: &Path) -> bool {
    git(dir, &["ls-files", "--error-unmatch", "Cargo.toml"]).is_some()
}

/// The top of the checkout `dir` lies in, and the pathspecs from there under which an
/// untracked file can be compiled in: the directories of [`COMPILED_IN`], and the crate's own
/// directory when it lies elsewhere.
fn compiled_in(dir: &Path) -> Option<(PathBuf, Vec<String>)> {
    let root = PathBuf::from(git(dir, &["rev-parse", "--show-toplevel"])?.trim());
    let prefix = git(dir, &["rev-parse", "--show-prefix"])?;
    let own = prefix.trim().trim_end_matches('/');
    let mut specs: Vec<String> = COMPILED_IN
        .iter()
        .map(|path| format!(":(literal){path}"))
        .collect();
    let covered = COMPILED_IN
        .iter()
        .any(|path| own == *path || own.starts_with(&format!("{path}/")));
    if own.is_empty() {
        specs.push(".".to_owned());
    } else if !covered {
        specs.push(format!(":(literal){own}"));
    }
    Some((root, specs))
}

/// The revision of the crate at `dir`: the sha of the commit its checkout is at, `-dirty` after
/// it while `git status`, every configured filter driver turned off ([`no_filter_drivers`]) and
/// no submodule looked into (`--ignore-submodules=all`: no submodule's git runs its own
/// filters, and a submodule at another commit is not seen), names a tracked file that differs from that commit, staged or not, or
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
    let empty_tree = if sha.len() == 64 {
        EMPTY_TREE_SHA256
    } else {
        EMPTY_TREE_SHA1
    };
    let Some(no_filters) = no_filter_drivers(dir) else {
        return UNKNOWN.to_owned();
    };
    let mut args = NO_ATTRIBUTES.to_vec();
    args.extend(no_filters.iter().map(String::as_str));
    args.extend([
        "--no-optional-locks",
        "status",
        "--porcelain",
        "--untracked-files=no",
        "--ignore-submodules=all",
    ]);
    let Some(status) = git_with(
        dir,
        &args,
        &[("GIT_ATTR_SOURCE", empty_tree), ("GIT_ATTR_NOSYSTEM", "1")],
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
/// exist; the index; `info/exclude` when it exists; every tracked file; and the directories of
/// [`compiled_in`], where a new source may appear ([`watch_tree`]). Paths into the git
/// directory go through `git rev-parse --git-path`, so a linked worktree's HEAD and index and
/// the shared refs are the ones named. A git file that does not exist is left out, since cargo
/// runs a build script whose watched file is missing on every build; a tracked file deleted
/// from the tree stays in, so every build reads the revision again until the file is back or
/// its deletion committed. The user's `core.excludesFile` is not watched. While the revision
/// is unknown: only [`NEVER_WRITTEN`], so every build reads it again.
pub fn watched(dir: &Path) -> Vec<PathBuf> {
    if revision(dir) == UNKNOWN {
        return vec![dir.join(NEVER_WRITTEN)];
    }
    let git_path = |name: &str| {
        git(
            dir,
            &["rev-parse", "--path-format=absolute", "--git-path", name],
        )
        .map(|path| dir.join(path.trim()))
        .filter(|path| path.exists())
    };
    let mut paths = Vec::new();
    for name in ["HEAD", "index", "packed-refs", "reftable", "info/exclude"] {
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
    let files: Vec<PathBuf> = git(&root, &["ls-files", "-z"])
        .unwrap_or_default()
        .split('\0')
        .filter(|file| !file.is_empty() && !file.contains('\n'))
        .map(|file| root.join(file))
        .collect();
    let out_dir = std::env::var_os("OUT_DIR").map(PathBuf::from);
    for spec in &specs {
        let scope = match spec.strip_prefix(":(literal)") {
            Some(path) => root.join(path),
            None => root.clone(),
        };
        if scope.is_dir() {
            watch_tree(&scope, out_dir.as_deref(), &files, &mut paths);
        }
    }
    paths.extend(files);
    paths
}

/// Watches `dir` whole, ignored files and all, so that a file added anywhere in it runs the
/// script again; unless it holds a `.git` or the build's own output directory `out_dir`, which
/// change on every build and would run the script every time: then each of its entries by the
/// same rule, but `.git` and the build directory (an entry that holds `out_dir` and no tracked
/// file of `tracked`). A file added directly into such a directory, the top of a crate that is
/// its own repository, is not seen.
fn watch_tree(dir: &Path, out_dir: Option<&Path>, tracked: &[PathBuf], paths: &mut Vec<PathBuf>) {
    let holds_output = |path: &Path| out_dir.is_some_and(|out| out.starts_with(path));
    if !dir.join(".git").exists() && !holds_output(dir) {
        paths.push(dir.to_path_buf());
        return;
    }
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if entry.file_name() == ".git" {
            continue;
        }
        if entry.file_type().is_ok_and(|kind| kind.is_dir()) {
            let build_directory =
                holds_output(&path) && !tracked.iter().any(|file| file.starts_with(&path));
            if !build_directory {
                watch_tree(&path, out_dir, tracked, paths);
            }
        } else {
            paths.push(path);
        }
    }
}
