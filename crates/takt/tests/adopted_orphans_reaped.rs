//! An orphan takt adopted is reaped once it has ended, while the command that orphaned it still
//! runs. takt is the subreaper of what it starts (`PR_SET_CHILD_SUBREAPER`), so a process a test
//! command started and orphaned becomes takt's child; once it has ended it stays takt's zombie
//! until takt reaps it, and while it does its process group still answers signal 0. A test that
//! kills a process group and waits until the group is gone, as `b10x-tool-process`'s tests do,
//! then waits out its bound under takt and not on a plain shell, where `init` reaps the orphan
//! at once.
//!
//! The command here is that test in a shell: it starts a shell in a process group of its own
//! (`set -m`) whose child stays in that group, kills the shell so the child is orphaned and
//! adopted by this process, kills the child, and waits until signal 0 to the group fails. It
//! exits 0 within its bound (about 5 s) and 1 past it. It runs as takt runs a test command:
//! through `forge::run_child`, and as a process group leader of its own while
//! `forge::hold_sweeps` is held, as the confined path and the gate's commands are run. A child
//! another thread waits on is not taken meanwhile: it keeps its exit status.
//!
//! On Linux only: elsewhere takt adopts nothing.
#![cfg(target_os = "linux")]

use std::path::PathBuf;
use std::process::{Child, Stdio};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use takt::confinement::Command;
use takt::forge::Limits;

/// The command, run as `sh -c ORPHANS orphans <pid file>`: it prints the orphan's parent once
/// the orphan's shell is gone, then exits 0 once the orphan's group is empty, 1 when the group
/// still answers about 5 s after its last member was killed, 2 when the inner shell never named
/// its child.
const ORPHANS: &str = r#"pidfile=$1
set -m
sh -c 'sleep 30 & echo $! > "$1"; wait' inner "$pidfile" &
inner=$!
n=0
until [ -s "$pidfile" ]; do
  n=$((n + 1)); [ "$n" -gt 500 ] && exit 2
  sleep 0.01
done
child=$(cat "$pidfile")
kill -KILL "$inner"
wait "$inner"
echo "group $inner child $child parent $(cut -d' ' -f4 /proc/$child/stat)"
kill -KILL "$child"
n=0
while kill -0 -"$inner" 2>/dev/null; do
  n=$((n + 1)); [ "$n" -gt 250 ] && exit 1
  sleep 0.02
done
exit 0
"#;

/// The cases one at a time: a sweep at the end of one case's `run_child` reaps what this
/// process adopted, and would make the other case's orphan go for a reason other than the one
/// under test.
static ONE_AT_A_TIME: Mutex<()> = Mutex::new(());

/// A scratch directory under this crate's build directory, removed when dropped.
struct Scratch(PathBuf);

impl Scratch {
    fn new(name: &str) -> Self {
        let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
            .join(format!("adopted-orphans-{}-{name}", std::process::id()));
        if dir.exists() {
            std::fs::remove_dir_all(&dir).expect("a stale scratch directory is removed");
        }
        std::fs::create_dir_all(&dir).expect("the scratch directory is made");
        Self(dir)
    }

    fn pid_file(&self) -> String {
        self.0.join("child.pid").to_string_lossy().into_owned()
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// Makes this process the subreaper of what it starts, as takt is: `forge::sweep_left` does it
/// first. Asserted, as the case shows nothing where an orphan goes to `init`.
fn adopt() {
    takt::forge::sweep_left().expect("nothing is left to sweep");
    let mut set: libc::c_int = 0;
    // SAFETY: `PR_GET_CHILD_SUBREAPER` writes one integer into the location it is given.
    let read = unsafe { libc::prctl(libc::PR_GET_CHILD_SUBREAPER, &raw mut set, 0, 0, 0) };
    assert_eq!(read, 0, "the subreaper flag reads");
    assert_eq!(
        set, 1,
        "takt made this process the subreaper of what it starts"
    );
}

/// The command for the case whose scratch directory is `scratch`.
fn orphans(scratch: &Scratch) -> Command {
    let mut command = Command::new("sh");
    command.args(["-c", ORPHANS, "orphans", &scratch.pid_file()]);
    command
}

/// Asserts that the command's orphan was adopted by this process, from the line it printed.
fn adopted_here(stdout: &str) {
    let parent = stdout
        .split_whitespace()
        .skip_while(|word| *word != "parent")
        .nth(1)
        .and_then(|parent| parent.parse::<u32>().ok());
    assert_eq!(
        parent,
        Some(std::process::id()),
        "the orphan's parent is this process once its shell is gone: {stdout}"
    );
}

/// Whether process `pid` has ended and is not reaped yet.
fn zombie(pid: u32) -> bool {
    std::fs::read_to_string(format!("/proc/{pid}/stat")).is_ok_and(|text| {
        text.rfind(')')
            .and_then(|at| text.get(at + 2..))
            .is_some_and(|rest| rest.starts_with('Z'))
    })
}

/// Starts `sh -c 'exit <code>'`, in a process group of its own when `leader`, and waits until
/// it has ended and is not reaped yet: a child its caller has not waited on.
fn ended_unwaited(code: i32, leader: bool) -> Child {
    let mut command = Command::new("sh");
    command
        .args(["-c", &format!("exit {code}")])
        .stdin(Stdio::null());
    if leader {
        command.process_group(0);
    }
    let child = command.spawn().expect("sh starts");
    let until = Instant::now() + Duration::from_secs(5);
    while !zombie(child.id()) {
        assert!(Instant::now() < until, "sh -c 'exit {code}' ended");
        std::thread::sleep(Duration::from_millis(5));
    }
    child
}

/// Expected before: the orphan stays this process's zombie while `run_child`'s child runs, its
/// group answers past the bound and the command exits 1.
#[test]
fn an_orphan_of_a_child_of_run_child_is_reaped_while_the_child_runs() {
    let _one = ONE_AT_A_TIME
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    adopt();
    let scratch = Scratch::new("run-child");
    let started = Instant::now();
    let finished = takt::forge::run_child(
        &mut orphans(&scratch),
        Limits {
            timeout: Duration::from_secs(30),
            output: 64 * 1024,
        },
    )
    .expect("the command runs to its end");
    let stdout = String::from_utf8_lossy(&finished.stdout);
    assert_eq!(
        finished.code,
        Some(0),
        "the orphan's group is empty within the bound under takt as on a plain shell, after \
         {:?}: stdout {stdout}, stderr {}",
        started.elapsed(),
        String::from_utf8_lossy(&finished.stderr)
    );
    adopted_here(&stdout);
}

/// Expected before: while a hold lives no sweep runs, so the orphan stays this process's
/// zombie for as long as the command runs, its group answers past the bound and the command
/// exits 1. The children this thread started and has not waited on, a process group leader and
/// one in this process's own group, keep their exit status for the thread that waits on them.
#[test]
fn an_orphan_of_a_command_run_under_a_hold_is_reaped_while_it_runs() {
    let _one = ONE_AT_A_TIME
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    adopt();
    let scratch = Scratch::new("held");
    let held = takt::forge::hold_sweeps();
    let leader = ended_unwaited(7, true);
    let member = ended_unwaited(9, false);
    let started = Instant::now();
    let mut command = orphans(&scratch);
    command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .process_group(0);
    let output = command
        .spawn()
        .expect("sh starts")
        .wait_with_output()
        .expect("the caller waits on its own child");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let codes = std::thread::spawn(move || {
        [leader, member].map(|mut child| child.wait().map(|status| status.code()).ok())
    })
    .join()
    .expect("the waiter ends");
    drop(held);
    assert_eq!(
        output.status.code(),
        Some(0),
        "the orphan's group is empty within the bound while sweeps are held, after {:?}: \
         stdout {stdout}, stderr {}",
        started.elapsed(),
        String::from_utf8_lossy(&output.stderr)
    );
    adopted_here(&stdout);
    assert_eq!(
        codes,
        [Some(Some(7)), Some(Some(9))],
        "a child another thread waits on is not reaped from under it"
    );
}
