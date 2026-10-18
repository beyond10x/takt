//! story:binary-embeds-revision, adversary pass 2: the revision a binary built with
//! `crates/takt-cli/build.rs` prints after correction 1, read back from a real build of a probe
//! crate that uses that very build script, or from the script's `revision` directly. The scratch
//! workspaces are laid out as this repository is: a root manifest, the crate under `crates/`.
//! Each case names the defect it is red on.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, SystemTime};

#[allow(dead_code)]
#[path = "../build.rs"]
mod build;

/// The build script under attack, copied into each probe crate unchanged.
const BUILD_RS: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/build.rs");

/// The probe's default `main`: prints `TAKT_REVISION`.
const MAIN_RS: &str = "fn main() { println!(\"{}\", env!(\"TAKT_REVISION\")); }\n";

/// `git <args>` in `dir` with no configured code, a fixed identity and none of the caller's
/// redirecting environment; its trimmed stdout.
fn git(dir: &Path, args: &[&str]) -> String {
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
        .current_dir(dir)
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

/// A scratch directory under this crate's build directory, removed on drop.
struct Scratch(PathBuf);

impl std::ops::Deref for Scratch {
    type Target = Path;

    fn deref(&self) -> &Path {
        &self.0
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// A scratch workspace, not under git of its own: a root manifest naming the one member
/// `crates/probe`, a `README.md`, a `.gitignore` of `/target` and `*.log`, and the probe crate
/// whose build script is the one under attack and whose `main` is `main_rs`.
fn workspace(name: &str, main_rs: &str) -> Scratch {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("revision-adv2-{}-{name}", std::process::id()));
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
    fs::write(probe.join("src/main.rs"), main_rs).expect("a source");
    Scratch(dir)
}

/// The probe crate's directory in `root`.
fn probe(root: &Path) -> PathBuf {
    root.join("crates/probe")
}

/// Commits every file of `dir` not ignored; the new HEAD.
fn commit_all(dir: &Path) -> String {
    git(dir, &["add", "-A"]);
    git(dir, &["commit", "-q", "-m", "start"]);
    git(dir, &["rev-parse", "HEAD"])
}

/// A scratch workspace on `main` whose one commit holds every file; the workspace and HEAD.
fn repository(name: &str) -> (Scratch, String) {
    let dir = workspace(name, MAIN_RS);
    git(&dir, &["init", "-q", "-b", "main"]);
    let head = commit_all(&dir);
    (dir, head)
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

/// Defect: an untracked file the binary compiles in from outside `crates/` leaves the revision
/// clean. This repository compiles files from its root `protocols/` into the binary
/// (`crates/takt/src/protocol.rs`, `EMBEDDED`: `include_str!("../../../protocols/...")`), so a
/// commit that adds a protocol to that list without `git add` of its document builds here,
/// prints the commit's sha as clean, and that commit does not build anywhere else.
#[test]
fn an_untracked_file_compiled_in_from_outside_crates_marks_the_revision_dirty() {
    let dir = workspace(
        "outside-crates",
        "const DOC: &str = include_str!(\"../../../protocols/p.yaml\");\n\
         fn main() { assert!(!DOC.is_empty()); println!(\"{}\", env!(\"TAKT_REVISION\")); }\n",
    );
    git(&dir, &["init", "-q", "-b", "main"]);
    let head = commit_all(&dir);
    fs::create_dir_all(dir.join("protocols")).expect("a directory");
    fs::write(dir.join("protocols/p.yaml"), "protocol: 1\n").expect("a document");
    assert_eq!(
        build_and_run(&dir),
        format!("{head}-dirty"),
        "protocols/p.yaml is compiled in and no commit holds it"
    );
}

/// Defect: a directory under `crates/` that holds an ignored path is not watched itself, only
/// its entries one by one (`watch_tree`), so a source added beside an ignored file after a
/// clean build does not run the build script again: the binary keeps the clean sha although
/// the script, run again, says `-dirty`. Any ignored file under `crates/` (a log, an editor's
/// or agent's local file ignored globally) opens it; for a crate at the top of its repository
/// the ignored `target/` always does.
#[test]
fn a_source_added_beside_an_ignored_file_builds_the_revision_again() {
    let (dir, head) = repository("beside-ignored");
    fs::write(probe(&dir).join("src/scratch.log"), "ignored\n").expect("an ignored file");
    assert_eq!(build_and_run(&dir), head);

    fs::write(probe(&dir).join("src/extra.rs"), "pub fn f() {}\n").expect("a source");
    assert_eq!(
        build::revision(&probe(&dir)),
        format!("{head}-dirty"),
        "the script's own rule"
    );
    assert_eq!(
        build_and_run(&dir),
        format!("{head}-dirty"),
        "the build after the new source"
    );
}

/// Defect: `.git/info/exclude` decides which untracked files under `crates/` count, and it is
/// not watched: a file it stops ignoring after a clean build leaves the binary clean, though
/// the script, run again, says `-dirty`.
#[test]
fn a_file_info_exclude_stops_ignoring_builds_the_revision_again() {
    let (dir, head) = repository("info-exclude");
    let exclude = dir.join(".git/info/exclude");
    fs::create_dir_all(exclude.parent().expect("a parent")).expect("a directory");
    fs::write(&exclude, "draft.rs\n").expect("an exclude file");
    fs::write(probe(&dir).join("src/draft.rs"), "pub fn f() {}\n").expect("a source");
    assert_eq!(build_and_run(&dir), head);

    fs::write(&exclude, "").expect("the exclude emptied");
    assert_eq!(
        build::revision(&probe(&dir)),
        format!("{head}-dirty"),
        "the script's own rule"
    );
    assert_eq!(
        build_and_run(&dir),
        format!("{head}-dirty"),
        "the build after the exclude changed"
    );
}

/// Defect: a build before the repository exists watches the crate's directory and the HEAD
/// and index of an enclosing repository, but a `git init` and first commit at the top of the
/// workspace, as this repository is laid out, touch neither: the binary keeps printing
/// `unknown revision` at a commit until a file of the crate itself changes.
#[test]
fn a_first_commit_at_the_workspace_root_after_a_build_builds_the_revision_again() {
    let dir = workspace("first-commit-at-root", MAIN_RS);
    assert_eq!(build_and_run(&dir), "unknown revision");

    git(&dir, &["init", "-q", "-b", "main"]);
    let head = commit_all(&dir);
    assert_eq!(build::revision(&probe(&dir)), head, "the script's own rule");
    assert_eq!(
        build_and_run(&dir),
        head,
        "the build after the first commit"
    );
}

/// Defect: the script's own contract (`NO_CONFIGURED_CODE`: "run no code from configuration")
/// does not hold for a configured clean filter: `git status` runs `filter.<driver>.clean` on
/// every tracked file whose stat differs from the index (git-lfs installs one globally), so a
/// command from configuration runs during the build.
#[test]
fn reading_the_revision_runs_no_configured_clean_filter() {
    let (dir, _) = repository("clean-filter");
    let marker = dir.join("filter-ran");
    fs::write(dir.join(".gitattributes"), "*.md filter=probe\n").expect("attributes");
    git(
        &dir,
        &[
            "config",
            "filter.probe.clean",
            &format!("touch '{}'; cat", marker.display()),
        ],
    );
    commit_all(&dir);
    let _ = fs::remove_file(&marker);
    fs::File::options()
        .write(true)
        .open(dir.join("README.md"))
        .expect("a tracked file")
        .set_modified(SystemTime::UNIX_EPOCH + Duration::from_secs(1_000_000_000))
        .expect("its mtime is set");

    let revision = build::revision(&probe(&dir));
    assert!(
        !marker.exists(),
        "the configured clean filter ran while the revision was read ({revision})"
    );
}
