//! story:binary-embeds-revision, adversary pass 1: the revision a binary built with
//! `crates/takt-cli/build.rs` prints, read back from a real build of a probe crate that uses that
//! very build script, or from the script's `revision` directly. Each case names the defect it is
//! red on.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

#[allow(dead_code)]
#[path = "../build.rs"]
mod build;

/// The build script under attack, copied into each probe crate unchanged.
const BUILD_RS: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/build.rs");

/// `git <args>` in `dir` with no configured code and a fixed identity; its trimmed stdout.
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

fn scratch(name: &str) -> Scratch {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("revision-adv1-{}-{name}", std::process::id()));
    if dir.exists() {
        fs::remove_dir_all(&dir).expect("a stale scratch tree is removed");
    }
    fs::create_dir_all(&dir).expect("a directory");
    Scratch(dir)
}

/// Writes a probe crate at `dir`: the build script under attack, and a `main` that prints
/// `TAKT_REVISION`; `main_rs` replaces the default `main.rs` when given.
fn probe_crate(dir: &Path, main_rs: Option<&str>) {
    fs::create_dir_all(dir.join("src")).expect("a directory");
    fs::copy(BUILD_RS, dir.join("build.rs")).expect("the build script is copied");
    fs::write(
        dir.join("Cargo.toml"),
        "[package]\nname = \"probe\"\nversion = \"0.0.0\"\nedition = \"2024\"\n\n[workspace]\n",
    )
    .expect("a manifest");
    fs::write(
        dir.join("src/main.rs"),
        main_rs.unwrap_or("fn main() { println!(\"{}\", env!(\"TAKT_REVISION\")); }\n"),
    )
    .expect("a source");
    fs::write(dir.join(".gitignore"), "target\n").expect("an ignore file");
}

fn cargo() -> String {
    std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_owned())
}

/// Builds the probe crate at `dir` the plain way and runs it; what it printed, trimmed.
fn build_and_run(dir: &Path) -> String {
    let built = Command::new(cargo())
        .args(["build", "--offline", "-q"])
        .current_dir(dir)
        .env_remove("CARGO_TARGET_DIR")
        .output()
        .expect("cargo runs");
    assert!(
        built.status.success(),
        "{}",
        String::from_utf8_lossy(&built.stderr)
    );
    run_probe(dir)
}

fn run_probe(dir: &Path) -> String {
    let ran = Command::new(dir.join("target/debug/probe"))
        .output()
        .expect("the probe runs");
    String::from_utf8(ran.stdout)
        .expect("utf-8")
        .trim()
        .to_owned()
}

/// A clean linked worktree laid out as this repository is (a tracked root `Cargo.toml`, the
/// probe crate under `crates/`), whose shared config holds the alias `git probe`, which builds
/// the probe crate; the scratch root, the linked tree and its HEAD.
fn linked_worktree(name: &str) -> (Scratch, PathBuf, String) {
    let root = scratch(name);
    let main = root.join("main");
    probe_crate(&main.join("crates/probe"), None);
    fs::write(main.join("Cargo.toml"), "[workspace]\nmembers = []\n").expect("a root manifest");
    git(&main, &["init", "-q", "-b", "main"]);
    git(&main, &["add", "-A"]);
    git(&main, &["commit", "-q", "-m", "one"]);
    git(
        &main,
        &[
            "config",
            "alias.probe",
            "!cd crates/probe && \"$PROBE_CARGO\" build --offline -q",
        ],
    );
    git(&main, &["worktree", "add", "-q", "../linked", "-b", "side"]);
    let linked = root.join("linked");
    let head = git(&linked, &["rev-parse", "HEAD"]);
    assert_eq!(git(&linked, &["status", "--porcelain"]), "", "clean tree");
    (root, linked, head)
}

/// Builds the probe crate of `linked` through git (`git probe`), so git exports `GIT_DIR` to
/// the build as it does to every hook, `rebase -x` command and `!` alias in a linked worktree.
fn build_through_git(linked: &Path) {
    let built = Command::new("git")
        .arg("probe")
        .current_dir(linked)
        .env("PROBE_CARGO", cargo())
        .env_remove("CARGO_TARGET_DIR")
        .env_remove("GIT_DIR")
        .output()
        .expect("git runs the alias");
    assert!(
        built.status.success(),
        "{}",
        String::from_utf8_lossy(&built.stderr)
    );
}

/// Defect: the build script's git inherits `GIT_DIR`, which git itself exports to every hook,
/// `git rebase -x` command and `!` alias it runs in a linked worktree. Git then takes the
/// build script's directory (the crate) as the top of the work tree, finds every tracked file
/// "deleted", and the binary of a clean commit says `<sha>-dirty` (`unknown revision` when the
/// root holds no `Cargo.toml`). Here the build runs through a repository alias in a clean
/// linked worktree, as `git rebase -x 'cargo build'` or a pre-commit hook would.
#[test]
fn a_build_run_by_git_in_a_clean_linked_worktree_is_not_dirty() {
    let (_root, linked, head) = linked_worktree("git-dir");
    build_through_git(&linked);
    assert_eq!(
        run_probe(&linked.join("crates/probe")),
        head,
        "a clean commit built from a hook, `rebase -x` or alias reports the wrong revision"
    );
}

/// Defect: `git status --untracked-files=no` does not see a source file the build compiles but
/// no commit holds. The commit names `mod extra;`, `src/extra.rs` was never added: the
/// binary runs, prints the commit's sha as clean, and that commit does not even build.
#[test]
fn a_source_the_binary_compiles_that_no_commit_holds_marks_the_revision_dirty() {
    let dir = scratch("untracked-source");
    probe_crate(
        &dir,
        Some("mod extra;\nfn main() { println!(\"{}\", extra::revision()); }\n"),
    );
    git(&dir, &["init", "-q", "-b", "main"]);
    git(&dir, &["add", "-A"]);
    git(
        &dir,
        &["commit", "-q", "-m", "names mod extra, holds no extra.rs"],
    );
    let head = git(&dir, &["rev-parse", "HEAD"]);
    fs::write(
        dir.join("src/extra.rs"),
        "pub fn revision() -> &'static str { env!(\"TAKT_REVISION\") }\n",
    )
    .expect("an untracked source");

    let printed = build_and_run(&dir);
    assert_eq!(
        printed,
        format!("{head}-dirty"),
        "the binary holds code from no commit and claims {head} clean"
    );
}

/// Defect: when the script reads no revision it asks cargo to watch nothing, so a later commit
/// that does track the crate never runs it again: the binary keeps printing `unknown revision`
/// at a commit, until a file of the crate itself is touched.
#[test]
fn a_build_that_read_no_revision_is_built_again_once_a_commit_tracks_the_crate() {
    let dir = scratch("unknown-sticks");
    probe_crate(&dir, None);
    assert_eq!(build_and_run(&dir), "unknown revision");

    git(&dir, &["init", "-q", "-b", "main"]);
    git(&dir, &["add", "-A"]);
    git(&dir, &["commit", "-q", "-m", "one"]);
    let head = git(&dir, &["rev-parse", "HEAD"]);
    assert_eq!(build_and_run(&dir), head, "a stale `unknown revision`");
}

/// Defect: the script's git runs code from configuration. This repository's own rule
/// (`crates/takt/src/git.rs`, `NO_CONFIGURED_CODE`: no `core.fsmonitor`, git runs it during
/// `status`) is not applied by the build script, so a configured fsmonitor runs at build time.
#[cfg(unix)]
#[test]
fn reading_the_revision_runs_no_code_from_git_configuration() {
    use std::os::unix::fs::PermissionsExt as _;
    let dir = scratch("fsmonitor");
    probe_crate(&dir, None);
    git(&dir, &["init", "-q", "-b", "main"]);
    git(&dir, &["add", "-A"]);
    git(&dir, &["commit", "-q", "-m", "one"]);
    let marker = dir.join("ran");
    let hook = dir.join(".git/monitor.sh");
    fs::write(
        &hook,
        format!("#!/bin/sh\ntouch '{}'\nexit 1\n", marker.display()),
    )
    .expect("a hook");
    fs::set_permissions(&hook, fs::Permissions::from_mode(0o755)).expect("executable");
    git(
        &dir,
        &["config", "core.fsmonitor", hook.to_str().expect("utf-8")],
    );

    let _ = build::revision(&dir);
    assert!(
        !marker.exists(),
        "the build script ran the configured core.fsmonitor command"
    );
}
