//! Adversary cases for the move of the test command's confinement to Substrate 0.14.0, whose
//! `linux` and `systemd` around it adapters now give a test command a scoped workspace: only
//! `target` is writable.
//!
//! takt documents three ways out for a test command that writes a `Cargo.lock` the commit lacks
//! (`run.rs`, `CARGO_LOCK_REMEDY`; `website/docs/getting-started.md`): commit it, ignore it in
//! `.gitignore`, or give `--test-cmd` a command that does not write it. The second leaves cargo
//! to write the lock file in the tree on every run, which a workspace read-only but its `target`
//! refuses: such a crate fails its test command there, and the failure must say why.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use takt::confinement::{self, Confinement as _, Substrate, TestRun};
use takt::validator::TestsRun;

/// A scratch directory under this crate's build directory, removed on drop.
struct Scratch(PathBuf);

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn scratch(name: &str) -> Scratch {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("adversary-hop-{}-{name}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("a scratch directory");
    Scratch(dir)
}

fn git(tree: &Path, args: &[&str]) {
    let output = Command::new("git")
        .args(["-c", "user.name=t", "-c", "user.email=t@localhost"])
        .args(args)
        .current_dir(tree)
        .output()
        .expect("git runs");
    assert!(output.status.success(), "git {args:?}");
}

/// A crate with one passing test whose `.gitignore` ignores `Cargo.lock`, committed without one:
/// the second way out takt's refusal names.
fn crate_ignoring_its_lock(outer: &Path) -> PathBuf {
    let repo = outer.join("repo");
    fs::create_dir_all(repo.join("src")).expect("a directory");
    fs::write(repo.join(".gitignore"), "target/\nCargo.lock\n").expect("a file");
    fs::write(
        repo.join("Cargo.toml"),
        "[package]\nname = \"probe\"\nversion = \"0.1.0\"\nedition = \"2021\"\n\n[workspace]\n",
    )
    .expect("a file");
    fs::write(
        repo.join("src/lib.rs"),
        "#[test]\nfn the_passing_test() {\n    assert_eq!(1 + 1, 2);\n}\n",
    )
    .expect("a file");
    git(&repo, &["init", "-q", "-b", "main"]);
    git(&repo, &["add", "-A"]);
    git(&repo, &["commit", "-q", "-m", "start"]);
    repo
}

/// Under `--confinement substrate`, a crate that ignores its `Cargo.lock` has its test command
/// fail under `linux` and `systemd` around it, and the failure names the lock file the run could
/// not write, so the step that reads it is told what to commit; under `macos`, whose tree stays
/// writable, the same crate passes.
#[test]
fn adversary_substrate_hop_a_crate_ignoring_its_lock_is_told_why_under_substrate() {
    let outer = scratch("ignored-lock");
    let repo = crate_ignoring_its_lock(&outer.0);
    let adapter = match Substrate::open(&repo, &confinement::Options::default()) {
        Ok(adapter) => adapter,
        Err(error) => {
            eprintln!("skipped: missing prerequisite: {error:#}");
            return;
        }
    };
    let argv: Vec<String> = ["cargo", "test", "--quiet"]
        .iter()
        .map(|arg| (*arg).to_owned())
        .collect();
    let finished = adapter
        .run_test(&TestRun {
            argv: &argv,
            tree: &repo,
            limits: TestsRun::LIMITS,
        })
        .expect("the confined test command runs");
    let record = adapter.record();
    let said = format!(
        "{}{}",
        String::from_utf8_lossy(&finished.stdout),
        String::from_utf8_lossy(&finished.stderr)
    );
    if record.adapter == "macos" {
        assert_eq!(
            finished.code,
            Some(0),
            "{}: the tree is writable, so cargo writes its lock: {said}",
            record.describe()
        );
    } else {
        assert!(
            finished.code != Some(0)
                && said.contains("Cargo.lock")
                && said.contains("Read-only file system"),
            "{}: only target is writable, so the run fails naming the lock it could not write (exit {:?}): {said}",
            record.describe(),
            finished.code
        );
        assert!(
            !repo.join("Cargo.lock").exists(),
            "{}: no lock file reached the tree",
            record.describe()
        );
    }
}
