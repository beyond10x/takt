//! story:test-command-reaches-no-forge-login, the refusal: the test command starts in a user and
//! a mount namespace of its own, and on a host that refuses it one the error names that cause,
//! where the error was a bare `Operation not permitted`. Any other failure to start keeps its
//! text.
//!
//! The refusal is provoked without root: the case runs itself again as a child test process
//! started in a user namespace of its own with no user mapped into it, and the kernel refuses a
//! user namespace to a process whose user is not mapped in its parent's (`EPERM`). On a host that
//! refuses user namespaces outright, that first step fails, the child starts as it is, and the
//! test command's own namespace is refused all the same.
#![cfg(target_os = "linux")]

use std::fs;
use std::os::unix::fs::PermissionsExt as _;
use std::os::unix::process::CommandExt as _;
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

/// Where the host refuses the test command its user namespace, the error names the namespaces
/// and the two settings that refuse them, beside the command.
#[test]
fn a_refused_namespace_is_named_with_the_settings_that_refuse_it() {
    if let Some(tree) = std::env::var_os("TAKT_NAMESPACE_CHILD") {
        let text = refusal(Path::new(&tree), "true");
        for needle in [
            "run true",
            "user and a mount namespace of its own",
            "refuses unprivileged user namespaces",
            "kernel.apparmor_restrict_unprivileged_userns",
            "kernel.unprivileged_userns_clone",
        ] {
            assert!(text.contains(needle), "{needle:?} in: {text}");
        }
        return;
    }
    let scratch = repository("refused");
    let mut command = Command::new(std::env::current_exe().expect("the test binary"));
    command
        .args([
            "a_refused_namespace_is_named_with_the_settings_that_refuse_it",
            "--exact",
            "--test-threads",
            "1",
        ])
        .env("TAKT_NAMESPACE_CHILD", scratch.join("repo"))
        .env("TMPDIR", scratch.join("tmp"));
    // SAFETY: `unshare` takes an integer; its result is ignored, see the module's comment.
    unsafe {
        command.pre_exec(|| {
            libc::unshare(libc::CLONE_NEWUSER);
            Ok(())
        });
    }
    let output = command.output().expect("the child test runs");
    assert!(
        output.status.success(),
        "the child test failed:\n{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        String::from_utf8_lossy(&output.stdout).contains("1 passed"),
        "the child test ran: {}",
        String::from_utf8_lossy(&output.stdout)
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
