//! story:binary-embeds-revision, the build script: the revision it reads for a crate directory
//! (`<sha>`, `<sha>-dirty` while a tracked file differs from the commit, `unknown revision`
//! when no git checkout with a commit tracks the crate, never a guess), and the files it asks
//! cargo to watch so that a commit, a checkout or an edit builds the revision again.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

#[allow(dead_code)]
#[path = "../build.rs"]
mod build;

/// Runs `git <args>` in `tree`; its trimmed standard output.
fn git(tree: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .args(["-c", "user.name=t", "-c", "user.email=t@localhost"])
        .args(args)
        .current_dir(tree)
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

/// An empty scratch directory holding a `Cargo.toml`, not under git of its own.
fn scratch(name: &str) -> Scratch {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("revision-build-{}-{name}", std::process::id()));
    if dir.exists() {
        fs::remove_dir_all(&dir).expect("a stale scratch tree is removed");
    }
    fs::create_dir_all(dir.join("src")).expect("a directory");
    fs::write(dir.join("Cargo.toml"), "[package]\nname = \"x\"\n").expect("a manifest");
    fs::write(dir.join("src/main.rs"), "fn main() {}\n").expect("a source");
    Scratch(dir)
}

/// A scratch repository on `main` whose one commit holds `Cargo.toml` and `src/main.rs`.
fn repository(name: &str) -> Scratch {
    let dir = scratch(name);
    git(&dir, &["init", "-q", "-b", "main"]);
    git(&dir, &["add", "-A"]);
    git(&dir, &["commit", "-q", "-m", "start"]);
    dir
}

/// A directory no git repository of its own tracks prints `unknown revision`. The scratch
/// directory lies in the ignored build directory of this repository's checkout, so git finds
/// a HEAD above it: that commit does not hold the crate, and printing it would be a guess.
#[test]
fn a_crate_no_checkout_tracks_has_an_unknown_revision() {
    let dir = scratch("untracked");
    assert_eq!(build::revision(&dir), "unknown revision");
}

/// A repository with no commit yet has no revision to print, though it tracks the crate.
#[test]
fn a_repository_without_a_commit_has_an_unknown_revision() {
    let dir = scratch("unborn");
    git(&dir, &["init", "-q", "-b", "main"]);
    git(&dir, &["add", "-A"]);
    assert_eq!(build::revision(&dir), "unknown revision");
}

/// At a commit with no tracked file changed the revision is the commit's sha; an untracked
/// file does not change it; a tracked file edited, or a change staged, adds `-dirty`.
#[test]
fn the_revision_is_the_commit_and_dirty_while_a_tracked_file_differs() {
    let dir = repository("commit");
    let head = git(&dir, &["rev-parse", "HEAD"]);
    assert_eq!(head.len(), 40, "{head}");
    assert_eq!(build::revision(&dir), head);

    fs::write(dir.join("notes.txt"), "untracked\n").expect("a file");
    assert_eq!(
        build::revision(&dir),
        head,
        "an untracked file is not a change"
    );

    fs::write(dir.join("src/main.rs"), "fn main() { }\n").expect("an edit");
    assert_eq!(build::revision(&dir), format!("{head}-dirty"));

    git(&dir, &["add", "src/main.rs"]);
    assert_eq!(
        build::revision(&dir),
        format!("{head}-dirty"),
        "a staged change is uncommitted"
    );

    git(&dir, &["commit", "-q", "-m", "edit"]);
    let next = git(&dir, &["rev-parse", "HEAD"]);
    assert_ne!(next, head);
    assert_eq!(build::revision(&dir), next);
}

/// The files watched in a repository: its HEAD, the branch ref HEAD names, its index and every
/// tracked file; every one exists, since cargo runs a build script whose watched file is
/// missing on every build; nothing untracked.
#[test]
fn the_watched_files_are_head_its_ref_the_index_and_the_tracked_tree() {
    let dir = repository("watched");
    fs::write(dir.join("notes.txt"), "untracked\n").expect("a file");
    let root = fs::canonicalize(&*dir).expect("a path");
    let watched: Vec<PathBuf> = build::watched(&dir)
        .iter()
        .map(|path| fs::canonicalize(path).expect("every watched path exists"))
        .collect();
    for expected in [
        ".git/HEAD",
        ".git/refs/heads/main",
        ".git/index",
        "Cargo.toml",
        "src/main.rs",
    ] {
        assert!(
            watched.contains(&root.join(expected)),
            "{expected} in {watched:?}"
        );
    }
    assert!(!watched.contains(&root.join("notes.txt")), "{watched:?}");
}

/// In this repository's checkout, a linked worktree or not, the crate's manifest is watched,
/// and a watched file holds the commit HEAD is at: HEAD itself when detached, else the
/// branch's loose ref or `packed-refs`.
#[test]
fn in_this_checkout_a_watched_file_holds_the_commit() {
    let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    if build::revision(crate_dir) == "unknown revision" {
        assert!(build::watched(crate_dir).is_empty());
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
