//! story:takt-confines-on-macos, acceptance 2 and 4.
//!
//! - A process a child started in a session of its own, still carrying the mark
//!   (`TAKT_CHILD`), is killed by `forge::sweep_left` and by the end of `forge::run_child`; on
//!   Linux through takt's adoption of it as subreaper, on macOS by its mark. Run on both.
//! - On macOS, a test command takt runs (`confinement::unconfined`) writes inside its tree, and
//!   a write outside the tree, the fresh home and the temporary directories is denied by the
//!   sandbox, the file not there afterwards.
//!
//! The leftover is this test binary started again: `stage` with [`STAGE`] `1` starts itself
//! with [`STAGE`] `2` in a session of its own and exits; stage 2 writes its pid to
//! [`PID_FILE`] and sleeps.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use takt::confinement::Command;
use takt::forge::Limits;

/// Which stage the binary started again runs.
const STAGE: &str = "TAKT_SWEEP_TEST_STAGE";

/// Where stage 2 writes its pid.
const PID_FILE: &str = "TAKT_SWEEP_TEST_PID_FILE";

/// The mark every child of `run_child` carries (`crates/takt/src/forge.rs`, `MARK`).
const MARK: &str = "TAKT_CHILD";

/// A directory of this test's own under the target directory, removed when dropped.
struct Scratch(PathBuf);

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn scratch(name: &str) -> Scratch {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("macos-confinement-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("a scratch directory");
    Scratch(dir)
}

/// This test binary, to run `stage` with [`STAGE`] set to `stage` and the pid written to
/// `pid_file`.
fn stage_command(stage: &str, pid_file: &Path) -> Command {
    let mut command = Command::new(std::env::current_exe().expect("the path of this test binary"));
    command
        .args(["--exact", "stage", "--nocapture", "--test-threads=1"])
        .env(STAGE, stage)
        .env(PID_FILE, pid_file);
    command
}

/// The stages of the leftover, run only in the binary started again; a test that passes
/// otherwise.
#[test]
fn stage() {
    let Ok(stage) = std::env::var(STAGE) else {
        return;
    };
    let pid_file = PathBuf::from(std::env::var_os(PID_FILE).expect("a pid file"));
    match stage.as_str() {
        "1" => {
            let mut command = stage_command("2", &pid_file);
            command
                .stdin(std::process::Stdio::null())
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null());
            // SAFETY: `setsid` is async-signal-safe and touches no memory of this process.
            unsafe {
                command.pre_exec(|| {
                    if libc::setsid() < 0 {
                        return Err(std::io::Error::last_os_error());
                    }
                    Ok(())
                });
            }
            #[expect(
                clippy::zombie_processes,
                reason = "stage 1 exits and leaves stage 2 running: the leftover the test kills"
            )]
            let _leftover = command.spawn().expect("stage 2 starts");
        }
        "2" => {
            let written = pid_file.with_extension("partial");
            fs::write(&written, std::process::id().to_string()).expect("the pid written");
            fs::rename(&written, &pid_file).expect("the pid in place");
            std::thread::sleep(Duration::from_secs(60));
        }
        other => panic!("no stage {other}"),
    }
}

/// The pid stage 2 wrote, within ten seconds.
fn leftover(pid_file: &Path) -> libc::pid_t {
    let until = Instant::now() + Duration::from_secs(10);
    loop {
        if let Ok(text) = fs::read_to_string(pid_file) {
            return text.trim().parse().expect("a pid");
        }
        assert!(Instant::now() < until, "stage 2 wrote no pid");
        std::thread::sleep(Duration::from_millis(10));
    }
}

/// Whether `pid` is still running: signalled with nothing, it answers; and it is not a zombie
/// (`ps` prints its state).
fn alive(pid: libc::pid_t) -> bool {
    // SAFETY: `kill` with signal 0 takes two integers and sends nothing.
    if unsafe { libc::kill(pid, 0) } != 0 {
        return false;
    }
    let output = std::process::Command::new("ps")
        .args(["-o", "stat=", "-p", &pid.to_string()])
        .output()
        .expect("ps runs");
    let state = String::from_utf8_lossy(&output.stdout);
    output.status.success() && !state.trim().is_empty() && !state.trim().starts_with('Z')
}

/// Whether `pid` has ended within five seconds.
fn ends(pid: libc::pid_t) -> bool {
    let until = Instant::now() + Duration::from_secs(5);
    while Instant::now() < until {
        if !alive(pid) {
            return true;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    false
}

/// Kills `pid`, should a check have left it running.
fn kill(pid: libc::pid_t) {
    // SAFETY: `kill` takes two integers and touches no memory of this process.
    unsafe {
        libc::kill(pid, libc::SIGKILL);
    }
}

/// Both in one test, so that no other test of this binary sweeps the leftover first.
#[test]
fn a_marked_process_in_a_session_of_its_own_is_killed_by_the_sweep_and_by_the_end_of_run_child() {
    let dir = scratch("sweep");
    // On Linux the first sweep makes this process the subreaper of what it starts.
    takt::forge::sweep_left().expect("the first sweep ends");

    let pid_file = dir.0.join("swept");
    let mut command = stage_command("1", &pid_file);
    command.env(MARK, format!("{}-swept", std::process::id()));
    command.stdout(std::process::Stdio::null());
    let status = command.status().expect("stage 1 runs");
    assert!(status.success(), "stage 1 ends: {status}");
    let pid = leftover(&pid_file);
    assert!(alive(pid), "stage 2 runs before the sweep");
    takt::forge::sweep_left().expect("the sweep ends");
    let swept = ends(pid);
    kill(pid);
    assert!(
        swept,
        "sweep_left kills {pid}, in a session of its own and marked"
    );

    let pid_file = dir.0.join("ended");
    let finished = takt::forge::run_child(
        &mut stage_command("1", &pid_file),
        Limits {
            timeout: Duration::from_secs(30),
            output: 4096,
        },
    )
    .expect("stage 1 runs through run_child");
    assert_eq!(finished.code, Some(0), "stage 1 ends: {finished:?}");
    let pid = leftover(&pid_file);
    let ended = !alive(pid);
    kill(pid);
    assert!(
        ended,
        "the end of run_child kills {pid}, in a session of its own and carrying the call's mark"
    );
}

#[cfg(target_os = "macos")]
mod seatbelt {
    use super::*;
    use takt::confinement::{TestRun, unconfined};

    fn limits() -> Limits {
        Limits {
            timeout: Duration::from_secs(30),
            output: 4096,
        }
    }

    fn sh(script: &str) -> Vec<String> {
        vec!["/bin/sh".to_owned(), "-c".to_owned(), script.to_owned()]
    }

    #[test]
    fn a_test_command_writes_in_its_tree_and_is_denied_a_write_outside_it() {
        let dir = scratch("tree");
        let tree = dir.0.join("tree");
        fs::create_dir_all(&tree).expect("the tree");
        let outside = dir.0.join("outside.txt");

        let argv = sh("echo in > inside.txt");
        let finished = unconfined(&TestRun {
            argv: &argv,
            tree: &tree,
            limits: limits(),
        })
        .expect("the test command runs");
        assert_eq!(
            finished.code,
            Some(0),
            "{}",
            String::from_utf8_lossy(&finished.stderr)
        );
        assert_eq!(
            fs::read_to_string(tree.join("inside.txt")).expect("the file in the tree"),
            "in\n"
        );

        let argv = sh(&format!("echo out > '{}'", outside.display()));
        let finished = unconfined(&TestRun {
            argv: &argv,
            tree: &tree,
            limits: limits(),
        })
        .expect("the test command runs");
        let stderr = String::from_utf8_lossy(&finished.stderr);
        assert_ne!(finished.code, Some(0), "the write outside fails: {stderr}");
        assert!(
            stderr.contains("Operation not permitted"),
            "the sandbox denies it: {stderr}"
        );
        assert!(!outside.exists(), "no file outside the tree");
    }
}
