//! story:binary-embeds-revision, the build script: the revision it reads for a crate directory
//! (`<sha>`, `<sha>-dirty` while a tracked file differs from the commit or an untracked file
//! lies where the build can compile it in, `unknown revision` when no git checkout with a
//! commit tracks the crate, never a guess), and the files it asks cargo to watch so that a
//! commit, a checkout, an edit or a new source builds the revision again. The scratch
//! repositories are laid out as this one is: a root manifest, the crate under `crates/`.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

#[allow(dead_code)]
#[path = "../build.rs"]
mod build;

/// The build script under test, copied into the probe crate of each scratch repository.
const BUILD_RS: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/build.rs");

/// Runs `git <args>` in `tree`, the caller's redirecting git environment left out; its trimmed
/// standard output.
fn git(tree: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .args([
            "-c",
            "user.name=t",
            "-c",
            "user.email=t@localhost",
            "-c",
            "commit.gpgSign=false",
            "-c",
            "core.hooksPath=/dev/null",
        ])
        .args(args)
        .current_dir(tree)
        .env_remove("GIT_DIR")
        .env_remove("GIT_WORK_TREE")
        .env_remove("GIT_INDEX_FILE")
        .output()
        .expect("git runs");
    assert!(
        output.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout)
        .expect("utf-8")
        .trim()
        .to_owned()
}

/// A scratch directory under this crate's build directory, removed on drop; a removal that
/// fails fails the case, unless the case is already failing.
struct Scratch(PathBuf);

impl std::ops::Deref for Scratch {
    type Target = Path;

    fn deref(&self) -> &Path {
        &self.0
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        if let Err(error) = fs::remove_dir_all(&self.0) {
            let message = format!("remove the scratch tree {}: {error}", self.0.display());
            if std::thread::panicking() {
                eprintln!("{message}");
            } else {
                panic!("{message}");
            }
        }
    }
}

/// A scratch workspace, not under git of its own: a root manifest naming the one member
/// `crates/probe`, a `README.md`, a `.gitignore` of `/target` and `*.log`, and the probe crate,
/// whose build script is the one under test and whose `main` prints `TAKT_REVISION`.
fn workspace(name: &str) -> Scratch {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("revision-build-{}-{name}", std::process::id()));
    if dir.exists() {
        fs::remove_dir_all(&dir).expect("a stale scratch tree is removed");
    }
    let probe = dir.join("crates/probe");
    fs::create_dir_all(probe.join("src")).expect("a directory");
    fs::write(
        dir.join("Cargo.toml"),
        "[workspace]\nmembers = [\"crates/probe\"]\nresolver = \"3\"\n",
    )
    .expect("a root manifest");
    fs::write(dir.join("README.md"), "a probe\n").expect("a readme");
    fs::write(dir.join(".gitignore"), "/target\n*.log\n").expect("an ignore file");
    fs::write(
        probe.join("Cargo.toml"),
        "[package]\nname = \"probe\"\nversion = \"0.0.0\"\nedition = \"2024\"\n",
    )
    .expect("a manifest");
    fs::copy(BUILD_RS, probe.join("build.rs")).expect("the build script is copied");
    fs::write(
        probe.join("src/main.rs"),
        "fn main() { println!(\"{}\", env!(\"TAKT_REVISION\")); }\n",
    )
    .expect("a source");
    Scratch(dir)
}

/// The probe crate's directory in `root`.
fn probe(root: &Path) -> PathBuf {
    root.join("crates/probe")
}

/// A scratch workspace on `main` whose one commit holds every file.
fn repository(name: &str) -> Scratch {
    let dir = workspace(name);
    git(&dir, &["init", "-q", "-b", "main"]);
    git(&dir, &["add", "-A"]);
    git(&dir, &["commit", "-q", "-m", "start"]);
    dir
}

/// Builds the workspace at `root` the plain way and runs the probe; what it printed, trimmed.
fn build_and_run(root: &Path) -> String {
    let built = Command::new(std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_owned()))
        .args(["build", "--offline", "-q"])
        .current_dir(root)
        .env_remove("CARGO_TARGET_DIR")
        .output()
        .expect("cargo runs");
    assert!(
        built.status.success(),
        "{}",
        String::from_utf8_lossy(&built.stderr)
    );
    let ran = Command::new(root.join("target/debug/probe"))
        .output()
        .expect("the probe runs");
    String::from_utf8(ran.stdout)
        .expect("utf-8")
        .trim()
        .to_owned()
}

/// A directory no git repository of its own tracks prints `unknown revision`. The scratch
/// directory lies in the ignored build directory of this repository's checkout, so git finds
/// a HEAD above it: that commit does not hold the crate, and printing it would be a guess.
#[test]
fn a_crate_no_checkout_tracks_has_an_unknown_revision() {
    let dir = workspace("untracked");
    assert_eq!(build::revision(&probe(&dir)), "unknown revision");
}

/// A repository with no commit yet has no revision to print, though it tracks the crate.
#[test]
fn a_repository_without_a_commit_has_an_unknown_revision() {
    let dir = workspace("unborn");
    git(&dir, &["init", "-q", "-b", "main"]);
    git(&dir, &["add", "-A"]);
    assert_eq!(build::revision(&probe(&dir)), "unknown revision");
}

/// At a commit with no tracked file changed the revision is the commit's sha; a tracked file
/// edited anywhere, or a change staged, adds `-dirty`; the next commit is clean again.
#[test]
fn the_revision_is_the_commit_and_dirty_while_a_tracked_file_differs() {
    let dir = repository("commit");
    let crate_dir = probe(&dir);
    let head = git(&dir, &["rev-parse", "HEAD"]);
    assert_eq!(head.len(), 40, "{head}");
    assert_eq!(build::revision(&crate_dir), head);

    fs::write(dir.join("README.md"), "a probe, edited\n").expect("an edit");
    assert_eq!(
        build::revision(&crate_dir),
        format!("{head}-dirty"),
        "a tracked file outside crates/ is a change"
    );
    git(&dir, &["checkout", "--", "README.md"]);
    assert_eq!(build::revision(&crate_dir), head);

    fs::write(crate_dir.join("src/main.rs"), "fn main() { }\n").expect("an edit");
    assert_eq!(build::revision(&crate_dir), format!("{head}-dirty"));

    git(&dir, &["add", "crates/probe/src/main.rs"]);
    assert_eq!(
        build::revision(&crate_dir),
        format!("{head}-dirty"),
        "a staged change is uncommitted"
    );

    git(&dir, &["commit", "-q", "-m", "edit"]);
    let next = git(&dir, &["rev-parse", "HEAD"]);
    assert_ne!(next, head);
    assert_eq!(build::revision(&crate_dir), next);
}

/// An untracked file the build can compile in, one under the workspace's `crates/` (the crate's
/// own or another's), makes the revision `-dirty`; an untracked file elsewhere does not, and
/// neither does an ignored one under `crates/`, nor an untracked `Cargo.lock`, which cargo
/// writes itself on a build.
#[test]
fn an_untracked_file_is_dirty_under_crates_and_clean_elsewhere() {
    let dir = repository("untracked-files");
    let crate_dir = probe(&dir);
    let head = git(&dir, &["rev-parse", "HEAD"]);

    fs::write(dir.join("notes.txt"), "untracked\n").expect("a file");
    fs::create_dir_all(dir.join("docs")).expect("a directory");
    fs::write(dir.join("docs/draft.md"), "untracked\n").expect("a file");
    assert_eq!(
        build::revision(&crate_dir),
        head,
        "an untracked file outside crates/ is not compiled in"
    );

    fs::write(crate_dir.join("build.log"), "ignored\n").expect("a file");
    assert_eq!(
        build::revision(&crate_dir),
        head,
        "an ignored file is not a change"
    );

    fs::write(crate_dir.join("Cargo.lock"), "version = 4\n").expect("a lockfile");
    assert_eq!(
        build::revision(&crate_dir),
        head,
        "a lockfile cargo writes on the build is not compiled in"
    );

    fs::write(crate_dir.join("src/extra.rs"), "pub fn f() {}\n").expect("a source");
    assert_eq!(
        build::revision(&crate_dir),
        format!("{head}-dirty"),
        "an untracked source of the crate"
    );
    fs::remove_file(crate_dir.join("src/extra.rs")).expect("removed");
    assert_eq!(build::revision(&crate_dir), head);

    fs::create_dir_all(dir.join("crates/other/src")).expect("a directory");
    fs::write(dir.join("crates/other/src/lib.rs"), "pub fn g() {}\n").expect("a source");
    assert_eq!(
        build::revision(&crate_dir),
        format!("{head}-dirty"),
        "an untracked source of another crate of the workspace"
    );
}

/// The files watched in a repository: its HEAD, the branch ref HEAD names, its index, every
/// tracked file and the `crates/` directory, where a new source may appear; every one exists,
/// since cargo runs a build script whose watched file is missing on every build; nothing
/// untracked outside `crates/`.
#[test]
fn the_watched_files_are_head_its_ref_the_index_the_tracked_tree_and_crates() {
    let dir = repository("watched");
    fs::write(dir.join("notes.txt"), "untracked\n").expect("a file");
    let root = fs::canonicalize(&*dir).expect("a path");
    let watched: Vec<PathBuf> = build::watched(&probe(&dir))
        .iter()
        .map(|path| fs::canonicalize(path).expect("every watched path exists"))
        .collect();
    for expected in [
        ".git/HEAD",
        ".git/refs/heads/main",
        ".git/index",
        "Cargo.toml",
        "README.md",
        "crates",
    ] {
        assert!(
            watched.contains(&root.join(expected)),
            "{expected} in {watched:?}"
        );
    }
    assert!(!watched.contains(&root.join("notes.txt")), "{watched:?}");
}

/// Through cargo: a source added under `crates/` after a clean build builds the revision again
/// as `-dirty`, and again clean once it is removed; a file added outside `crates/` leaves it
/// clean.
#[test]
fn a_source_added_under_crates_after_a_build_builds_the_revision_again() {
    let dir = repository("rebuild");
    let head = git(&dir, &["rev-parse", "HEAD"]);
    assert_eq!(build_and_run(&dir), head);

    let extra = probe(&dir).join("src/extra.rs");
    fs::write(&extra, "pub fn f() {}\n").expect("a source");
    assert_eq!(build_and_run(&dir), format!("{head}-dirty"));

    fs::remove_file(&extra).expect("removed");
    assert_eq!(build_and_run(&dir), head);

    fs::write(dir.join("notes.txt"), "untracked\n").expect("a file");
    assert_eq!(build_and_run(&dir), head);
}

/// A crate no checkout tracks has its own directory watched, so a `git init` there, or a file
/// added to it, builds the revision again.
#[test]
fn a_crate_no_checkout_tracks_watches_its_own_directory() {
    let dir = workspace("unknown-watched");
    let crate_dir = fs::canonicalize(probe(&dir)).expect("a path");
    let watched: Vec<PathBuf> = build::watched(&crate_dir)
        .iter()
        .map(|path| fs::canonicalize(path).expect("every watched path exists"))
        .collect();
    assert!(watched.contains(&crate_dir), "{watched:?}");
}

/// In this repository's checkout, a linked worktree or not, the crate's manifest is watched,
/// and a watched file holds the commit HEAD is at: HEAD itself when detached, else the
/// branch's loose ref or `packed-refs`.
#[test]
fn in_this_checkout_a_watched_file_holds_the_commit() {
    let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    if build::revision(crate_dir) == "unknown revision" {
        return;
    }
    let head = git(crate_dir, &["rev-parse", "HEAD"]);
    let watched = build::watched(crate_dir);
    let manifest = fs::canonicalize(crate_dir.join("Cargo.toml")).expect("a path");
    assert!(
        watched
            .iter()
            .any(|path| fs::canonicalize(path).is_ok_and(|path| path == manifest)),
        "{watched:?}"
    );
    assert!(
        watched.iter().any(|path| path.is_file()
            && fs::read_to_string(path).is_ok_and(|text| text.contains(&head))),
        "no watched file holds {head}: {watched:?}"
    );
}
