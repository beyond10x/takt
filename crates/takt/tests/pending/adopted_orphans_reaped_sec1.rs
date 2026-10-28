//! Not built: kept for story:adopted-orphan-group-leaders-are-reaped, whose implementation moves
//! this file to `crates/takt/tests/` and makes it pass. The security pass on
//! story:adopted-orphans-are-reaped-while-a-child-runs wrote it.
//!
//! Conformance checks of story:adopted-orphans-are-reaped-while-a-child-runs at its edges.
//!
//! `orphaned_group_leader_is_reaped_while_the_child_runs`: the story's outcome is that no zombie
//! of a test's killed process group outlives its test. A test that daemonizes a process
//! (`setsid`), loses its parent, kills the daemon's group and waits until the group is gone
//! leaves an orphan that leads its own group; the reaper skips every group leader, so under
//! takt the group answers signal 0 until the next sweep, and on a plain shell `init` reaps it.
//!
//! `a_child_started_into_another_childs_group_keeps_its_exit_status` (ignored): the reaper's
//! one safety fact is that no direct child of takt is in a foreign process group without
//! leading it. `takt::confinement::Command::process_group` takes any group; a direct child
//! started into a running child's group loses its exit status to the reaper. Nothing in takt
//! or what it links starts one (reviewed: every `process_group` call passes 0), and
//! `forge::hold_sweeps` states it as a precondition, so the case is kept ignored as the
//! boundary's reproduction: `cargo test --test adopted_orphans_reaped_sec1 -- --ignored`.
//!
//! On Linux only: elsewhere takt adopts nothing.
#![cfg(target_os = "linux")]

use std::path::PathBuf;
use std::process::Stdio;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use takt::confinement::Command;
use takt::forge::Limits;

const STORY: &str = "story:adopted-orphans-are-reaped-while-a-child-runs";

/// Run as `sh -c LEADER leader <pid file>`. The inner shell starts `setsid sleep 30` in the
/// background (not a group leader there, so `setsid` calls `setsid()` in place and the sleep
/// leads a session and a group of its own), names it and exits; the sleep is orphaned and
/// adopted by this process. The command prints the sleep's parent and group, kills the group
/// and exits 0 once signal 0 to it fails, 1 when it still answers about 5 s later, 2 when the
/// inner shell never named the sleep.
const LEADER: &str = r#"pidfile=$1
sh -c 'setsid sleep 30 & echo $! > "$1"' inner "$pidfile"
n=0
until [ -s "$pidfile" ]; do
  n=$((n + 1)); [ "$n" -gt 500 ] && exit 2
  sleep 0.01
done
child=$(cat "$pidfile")
n=0
until [ "$(cut -d' ' -f4 /proc/$child/stat)" != "$$" ] && [ "$(cut -d' ' -f5 /proc/$child/stat)" = "$child" ]; do
  n=$((n + 1)); [ "$n" -gt 500 ] && break
  sleep 0.01
done
echo "child $child parent $(cut -d' ' -f4 /proc/$child/stat) group $(cut -d' ' -f5 /proc/$child/stat)"
kill -KILL -"$child"
n=0
while kill -0 -"$child" 2>/dev/null; do
  n=$((n + 1)); [ "$n" -gt 250 ] && exit 1
  sleep 0.02
done
exit 0
"#;

static ONE_AT_A_TIME: Mutex<()> = Mutex::new(());

/// A scratch directory under this crate's build directory, removed when dropped.
struct Scratch(PathBuf);

impl Scratch {
    fn new(name: &str) -> Self {
        let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(format!(
            "adopted-orphans-sec1-{}-{name}",
            std::process::id()
        ));
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

/// Makes this process the subreaper of what it starts, as takt is, and asserts it.
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

/// The number after `key` in what the command printed.
fn field(stdout: &str, key: &str) -> Option<u32> {
    stdout
        .split_whitespace()
        .skip_while(|word| *word != key)
        .nth(1)
        .and_then(|value| value.parse().ok())
}

fn zombie(pid: u32) -> bool {
    std::fs::read_to_string(format!("/proc/{pid}/stat")).is_ok_and(|text| {
        text.rfind(')')
            .and_then(|at| text.get(at + 2..))
            .is_some_and(|rest| rest.starts_with('Z'))
    })
}

/// The orphan leads its own group and session; it is adopted by this process and killed. The
/// story's outcome asks that its group stop answering within the bound, as under `init`.
#[test]
fn orphaned_group_leader_is_reaped_while_the_child_runs() {
    let _one = ONE_AT_A_TIME
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    assert!(
        ["/usr/bin/setsid", "/bin/setsid"]
            .iter()
            .any(|path| std::path::Path::new(path).is_file()),
        "setsid(1) is installed"
    );
    adopt();
    let scratch = Scratch::new("leader");
    let mut command = Command::new("sh");
    command.args(["-c", LEADER, "leader", &scratch.pid_file()]);
    let started = Instant::now();
    let finished = takt::forge::run_child(
        &mut command,
        Limits {
            timeout: Duration::from_secs(30),
            output: 64 * 1024,
        },
    )
    .expect("the command runs to its end");
    let stdout = String::from_utf8_lossy(&finished.stdout).into_owned();
    let child = field(&stdout, "child");
    assert_eq!(
        field(&stdout, "parent"),
        Some(std::process::id()),
        "the orphan was adopted by this process: {stdout}"
    );
    assert_eq!(
        field(&stdout, "group"),
        child,
        "the orphan leads its own process group: {stdout}"
    );
    assert_eq!(
        finished.code,
        Some(0),
        "{STORY}: an adopted orphan that leads its own process group (a daemon started with \
         setsid) was killed, yet its group still answered signal 0 after {:?}: it stays this \
         process's zombie until the next sweep, where init would reap it at once; stdout \
         {stdout}, stderr {}",
        started.elapsed(),
        String::from_utf8_lossy(&finished.stderr)
    );
}

/// Outside the documented precondition of `forge::hold_sweeps`; see the module comment.
#[test]
// Unreached in takt: a direct child in a foreign group it does not lead.
fn a_child_started_into_another_childs_group_keeps_its_exit_status() {
    let _one = ONE_AT_A_TIME
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    adopt();
    let held = takt::forge::hold_sweeps();
    let mut leader = Command::new("sleep");
    leader.arg("30").stdin(Stdio::null()).process_group(0);
    let mut leader = leader.spawn().expect("sleep starts");
    let group = i32::try_from(leader.id()).expect("a pid fits i32");
    let mut member = Command::new("sh");
    member
        .args(["-c", "exit 5"])
        .stdin(Stdio::null())
        .process_group(group);
    let mut member = member.spawn().expect("sh starts");
    let until = Instant::now() + Duration::from_secs(5);
    while !zombie(member.id()) && Instant::now() < until {
        std::thread::sleep(Duration::from_millis(5));
    }
    std::thread::sleep(Duration::from_millis(300));
    let waited = member.wait().map(|status| status.code());
    let _ = leader.kill();
    let _ = leader.wait();
    drop(held);
    assert_eq!(
        waited.as_ref().ok().copied(),
        Some(Some(5)),
        "{STORY}: a direct child started into another child's process group lost its exit \
         status to the reaper: {waited:?}"
    );
}
