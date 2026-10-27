//! story:test-command-reaches-no-forge-login, the refusal: the test command once started in a
//! user and a mount namespace of its own, and on a host that refused it one the error named
//! that cause. Since story:confinement-through-substrate-on-linux-and-macos takt makes no
//! namespace: a `none` run's test command starts through Substrate's `local` adapter in takt's
//! own namespaces, so there is no such refusal to name, and a failure to start keeps its text
//! with no namespace hint.
#![cfg(target_os = "linux")]

use std::fs;
use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};
use std::process::Command;

use takt::validator::{TestsRun, Validator};

/// A scratch directory under this crate's build directory, removed when dropped.
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

/// A committed repository named for this process and `name`, with a temporary directory
/// beside it.
fn repository(name: &str) -> Scratch {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("namespace-{}-{name}", std::process::id()));
    if dir.exists() {
        fs::remove_dir_all(&dir).expect("a stale scratch tree is removed");
    }
    fs::create_dir_all(dir.join("repo")).expect("scratch dir");
    fs::create_dir_all(dir.join("tmp")).expect("a temporary directory");
    fs::write(dir.join("repo/README.md"), "a story\n").expect("a file");
    fs::write(dir.join("repo/plain.sh"), "#!/bin/sh\nexit 0\n").expect("a script");
    fs::set_permissions(dir.join("repo/plain.sh"), fs::Permissions::from_mode(0o644))
        .expect("not executable");
    for args in [
        &["init", "-q", "-b", "main"][..],
        &["add", "-A"],
        &["commit", "-q", "-m", "start"],
    ] {
        let status = Command::new("git")
            .args(["-c", "user.name=t", "-c", "user.email=t@localhost"])
            .args(args)
            .current_dir(dir.join("repo"))
            .status()
            .expect("git runs");
        assert!(status.success(), "git {args:?}");
    }
    Scratch(dir)
}

/// What observing `command` on `tree` failed with, in full.
fn refusal(tree: &Path, command: &str) -> String {
    let error = TestsRun::new(command)
        .expect("a command")
        .observe(tree, "r0", 1)
        .expect_err("the test command cannot start");
    format!("{error:#}")
}

/// Since story:confinement-through-substrate-on-linux-and-macos a `none` run's test command is
/// started by Substrate's `local` adapter, in no namespace takt makes: it runs in takt's own
/// user namespace, so no host setting that refuses unprivileged user namespaces can refuse it,
/// and there is no namespace refusal left to name.
#[test]
fn a_none_test_command_starts_in_no_namespace_of_takt_s_own() {
    let scratch = repository("own-namespace");
    let tree = scratch.join("repo");
    fs::write(
        tree.join("ns.sh"),
        "#!/bin/sh\nreadlink /proc/self/ns/user > ns.txt\nreadlink /proc/self/ns/mnt >> ns.txt\n",
    )
    .expect("a script");
    fs::set_permissions(tree.join("ns.sh"), fs::Permissions::from_mode(0o755)).expect("executable");
    let observed = TestsRun::new("./ns.sh")
        .expect("a command")
        .observe(&tree, "r0", 1)
        .expect("the test command starts and is observed");
    let seen = fs::read_to_string(tree.join("ns.txt")).unwrap_or_else(|error| {
        panic!(
            "the test command wrote its namespaces ({error}): {}",
            observed.summary
        )
    });
    let own = format!(
        "{}\n{}\n",
        fs::read_link("/proc/self/ns/user")
            .expect("this process's user namespace")
            .display(),
        fs::read_link("/proc/self/ns/mnt")
            .expect("this process's mount namespace")
            .display()
    );
    assert_eq!(
        seen, own,
        "story:confinement-through-substrate-on-linux-and-macos: a none test command shares takt's user and mount namespaces"
    );
}

/// A test command that cannot start for another reason, a program that is not there or a
/// script that is not executable, keeps its own error and gets no namespace hint.
#[test]
fn another_failure_to_start_keeps_its_text() {
    let scratch = repository("other");
    let tree = scratch.join("repo");
    for (command, cause) in [
        ("no-such-program-takt", "No such file or directory"),
        ("./plain.sh", "Permission denied"),
    ] {
        let text = refusal(&tree, command);
        assert!(text.contains(&format!("run {command}")), "{text}");
        assert!(text.contains(cause), "{cause:?} in: {text}");
        assert!(!text.contains("namespace"), "no namespace hint: {text}");
    }
}
