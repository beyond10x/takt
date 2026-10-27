//! story:takt-confines-on-macos, acceptance 2 and 4.
//!
//! - A process a child started in a session of its own, still carrying the mark
//!   (`TAKT_CHILD`), is killed by `forge::sweep_left` and by the end of `forge::run_child`; on
//!   Linux through takt's adoption of it as subreaper, on macOS by its mark. Run on both.
//! - On macOS, a confined test command writes inside its tree, and a write outside it is denied,
//!   the file not there afterwards. Since story:confinement-through-substrate-on-linux-and-macos
//!   the confinement is Substrate's `macos` adapter, not a Seatbelt profile of takt's.
//!
//! story:wave-35-review-fixes, acceptance 1 to 4: on macOS a confined build and a test command
//! start no launchd job, the test command connects to no launchd socket, neither writes the
//! user's temporary or cache directory, and the test command writes no hook nor configuration
//! of the common git directory; on every platform, a confined test command in a linked worktree
//! writes nothing of the common git directory, and the release publishes only after the macOS
//! tests of its commit.
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

/// `git` with `args` in `dir`, under an identity of its own and no user or system
/// configuration; its standard output, trimmed.
fn git(dir: &Path, args: &[&str]) -> String {
    let output = std::process::Command::new("git")
        .args([
            "-c",
            "user.name=takt",
            "-c",
            "user.email=takt@example.invalid",
        ])
        .args(args)
        .current_dir(dir)
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .output()
        .expect("git runs");
    assert!(
        output.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).trim().to_owned()
}

/// A repository at `dir/main` with one commit and a linked worktree of it at `dir/linked`.
fn linked_worktree(dir: &Path) -> (PathBuf, PathBuf) {
    let main = dir.join("main");
    fs::create_dir_all(&main).expect("the main worktree");
    git(&main, &["init", "-q", "-b", "main"]);
    fs::write(main.join("README.md"), "a story\n").expect("a file");
    git(&main, &["add", "-A"]);
    git(&main, &["commit", "-q", "-m", "start"]);
    let linked = dir.join("linked");
    git(
        &main,
        &[
            "worktree",
            "add",
            "-q",
            "-b",
            "linked",
            linked.to_str().expect("UTF-8"),
        ],
    );
    (main, linked)
}

/// `script` with `/bin/sh` in `tree`, through the Substrate adapter `--confinement substrate`
/// chooses on this host ([`takt::confinement::Confined`]), on `PATH` `/usr/bin:/bin` with a home
/// in the tree, and on macOS the system directories its programs run from; held to 30 seconds.
/// `None`, said on stderr, when no adapter of this host serves the run.
fn confined(tree: &Path, script: &str) -> Option<takt::confinement::Ran> {
    let confined =
        match takt::confinement::Confined::open(tree, None, &takt::confinement::Options::default())
        {
            Ok(confined) => confined,
            Err(error) => {
                eprintln!("skipped: missing prerequisite: a Substrate adapter ({error:#})");
                return None;
            }
        };
    let home = tree.join("home");
    fs::create_dir_all(&home).expect("a home");
    let mut env = std::collections::BTreeMap::from([
        ("PATH".to_owned(), "/usr/bin:/bin".to_owned()),
        (
            "HOME".to_owned(),
            confined.sees(&home, "/workspace/home").expect("UTF-8"),
        ),
    ]);
    if confined.serves_writable_roots()
        || !confined
            .record()
            .facts
            .iter()
            .any(|fact| fact == "private_tmpdir")
    {
        let tmp = home.join("tmp");
        fs::create_dir_all(&tmp).expect("a temporary directory");
        env.insert(
            "TMPDIR".to_owned(),
            confined.sees(&tmp, "/workspace/home/tmp").expect("UTF-8"),
        );
    }
    let spec = takt::confinement::RunSpec {
        argv: vec!["/bin/sh".to_owned(), "-c".to_owned(), script.to_owned()],
        env,
        exec: vec!["/bin".to_owned(), "/usr/bin".to_owned()],
        timeout: Duration::from_secs(30),
        ..takt::confinement::RunSpec::default()
    };
    Some(
        confined
            .run(tree, &spec)
            .expect("the confined run starts and ends"),
    )
}

/// story:wave-35-review-fixes, M5, on every platform, since
/// story:confinement-through-substrate-on-linux-and-macos through Substrate: a test command in a
/// linked worktree writes nothing of the common git directory, neither its hooks, its
/// configuration, nor the worktree's own `commondir` or `config.worktree`: the tree alone is the
/// adapter's workspace, and takt declares no git directory writable.
#[test]
fn the_git_dirs_a_test_command_writes_leave_out_hooks_and_configuration() {
    let dir = scratch("git-dirs");
    let (main, linked) = linked_worktree(&dir.0);
    let common = main.join(".git");
    let own = common.join("worktrees/linked");
    let kept = [
        common.join("config"),
        own.join("commondir"),
        own.join("gitdir"),
    ];
    let before: Vec<Option<Vec<u8>>> = kept.iter().map(|path| fs::read(path).ok()).collect();
    for (index, target) in [
        common.join("hooks/probe"),
        common.join("config"),
        common.join("takt-probe"),
        own.join("commondir"),
        own.join("config.worktree"),
    ]
    .iter()
    .enumerate()
    {
        let marker = format!("ran-{index}");
        let Some(ran) = confined(
            &linked,
            &format!(
                "echo ran > {marker} && echo planted >> '{}'",
                target.display()
            ),
        ) else {
            return;
        };
        assert!(
            linked.join(&marker).exists() && ran.code.is_some(),
            "story:confinement-through-substrate-on-linux-and-macos: the command ran in the tree and exited ({}): {}",
            ran.ending(),
            String::from_utf8_lossy(&ran.stderr)
        );
        assert!(
            !ran.success(),
            "story:confinement-through-substrate-on-linux-and-macos: {} is not written: {}",
            target.display(),
            String::from_utf8_lossy(&ran.stderr)
        );
    }
    assert!(
        !common.join("hooks/probe").exists()
            && !common.join("takt-probe").exists()
            && !own.join("config.worktree").exists(),
        "story:confinement-through-substrate-on-linux-and-macos: nothing planted in the common git directory"
    );
    let after: Vec<Option<Vec<u8>>> = kept.iter().map(|path| fs::read(path).ok()).collect();
    assert_eq!(
        after, before,
        "story:confinement-through-substrate-on-linux-and-macos: the configuration and the worktree's links are as they were"
    );
}

/// story:confinement-through-substrate-on-linux-and-macos, review finding 3, on every platform:
/// a process a confined run (a promotion's build, a test command) started in a session of its
/// own does not outlive the run. On Linux the `systemd` adapter kills the run's whole tree; on
/// macOS, whose adapter proves no `whole_tree_kill`, takt kills what carries the run's mark once
/// it ended, as `run_child` did for a promotion's build before. The leftover writes `beat` in
/// the tree every 200 ms for a minute at most; once the run returned, `beat` is removed and must
/// not come back.
#[test]
fn a_process_a_confined_run_started_in_a_session_of_its_own_does_not_outlive_it() {
    let dir = scratch("session-leftover");
    let tree = dir.0.join("tree");
    fs::create_dir_all(&tree).expect("a tree");
    let Some(ran) = confined(
        &tree,
        "perl -MPOSIX -e 'POSIX::setsid(); exec @ARGV' /bin/sh -c 'i=0; while [ $i -lt 300 ]; do date > beat; sleep 0.2; i=$((i+1)); done' </dev/null >/dev/null 2>&1 &\n\
         i=0; while [ ! -s beat ] && [ $i -lt 50 ]; do sleep 0.1; i=$((i+1)); done\n\
         test -s beat",
    ) else {
        return;
    };
    assert!(
        ran.success(),
        "story:confinement-through-substrate-on-linux-and-macos: the leftover started and wrote its beat; the run {}: {}",
        ran.ending(),
        String::from_utf8_lossy(&ran.stderr)
    );
    let beat = tree.join("beat");
    let _ = fs::remove_file(&beat);
    std::thread::sleep(Duration::from_secs(2));
    assert!(
        !beat.exists(),
        "story:confinement-through-substrate-on-linux-and-macos: what the run started in a session of its own still ran after the run returned"
    );
}

/// The workflow at `.github/workflows/<name>` of this repository, parsed.
fn workflow(name: &str) -> serde_yaml_ng::Value {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../.github/workflows")
        .join(name);
    let text = fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("{} reads: {error}", path.display()));
    serde_yaml_ng::from_str(&text)
        .unwrap_or_else(|error| panic!("{} parses: {error}", path.display()))
}

/// story:wave-35-review-fixes, acceptance 4: the release publishes only after the macOS tests
/// passed on the commit it releases. `darwin.yml` is callable with the commit to test, and
/// the release's publish job needs a job that calls it with the tagged commit.
#[test]
fn the_release_publishes_only_after_the_macos_tests_of_the_tagged_commit() {
    let darwin = workflow("darwin.yml");
    let on = darwin
        .get("on")
        .or_else(|| darwin.get(serde_yaml_ng::Value::Bool(true)))
        .expect("darwin.yml has triggers");
    assert!(
        on.get("workflow_call")
            .and_then(|call| call.get("inputs"))
            .and_then(|inputs| inputs.get("ref"))
            .is_some(),
        "darwin.yml is callable with a ref: {on:?}"
    );
    let release = workflow("release.yml");
    let jobs = release.get("jobs").expect("release.yml has jobs");
    let needs: Vec<&str> = jobs
        .get("release")
        .and_then(|job| job.get("needs"))
        .and_then(serde_yaml_ng::Value::as_sequence)
        .expect("the publish job needs jobs")
        .iter()
        .filter_map(serde_yaml_ng::Value::as_str)
        .collect();
    let calls_darwin: Vec<&str> = needs
        .iter()
        .copied()
        .filter(|name| {
            let job = jobs.get(*name);
            job.and_then(|job| job.get("uses"))
                .and_then(serde_yaml_ng::Value::as_str)
                == Some("./.github/workflows/darwin.yml")
                && job
                    .and_then(|job| job.get("with"))
                    .and_then(|with| with.get("ref"))
                    .and_then(serde_yaml_ng::Value::as_str)
                    == Some("${{ needs.provenance.outputs.commit }}")
        })
        .collect();
    assert!(
        !calls_darwin.is_empty(),
        "the publish job needs no job testing the tagged commit on macOS: {needs:?}"
    );
}

/// On macOS, since story:confinement-through-substrate-on-linux-and-macos: takt has no Seatbelt
/// profile of its own; a test command under `--confinement substrate` and a promotion's build
/// both run through Substrate's `macos` adapter ([`confined`]), whose profile denies by
/// default, and these cases hold of it.
#[cfg(target_os = "macos")]
mod seatbelt {
    use super::*;

    /// `script` run as a confined test command or build is: through Substrate's `macos` adapter,
    /// which a macOS host must serve.
    fn test_command(tree: &Path, script: &str) -> takt::confinement::Ran {
        confined(tree, script).expect(
            "story:confinement-through-substrate-on-linux-and-macos: Substrate's macos adapter serves a run here",
        )
    }

    /// A promotion's build goes through the same adapter as a test command.
    fn confined_build(writable: &Path, script: &str) -> takt::confinement::Ran {
        test_command(writable, script)
    }

    #[test]
    fn a_test_command_writes_in_its_tree_and_is_denied_a_write_outside_it() {
        let dir = scratch("tree");
        let tree = dir.0.join("tree");
        fs::create_dir_all(&tree).expect("the tree");
        let outside = dir.0.join("outside.txt");

        let finished = test_command(&tree, "echo in > inside.txt");
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

        let finished = test_command(&tree, &format!("echo out > '{}'", outside.display()));
        let stderr = String::from_utf8_lossy(&finished.stderr);
        assert_ne!(finished.code, Some(0), "the write outside fails: {stderr}");
        assert!(
            stderr.contains("Operation not permitted"),
            "story:confinement-through-substrate-on-linux-and-macos: Substrate's profile denies it: {stderr}"
        );
        assert!(!outside.exists(), "no file outside the tree");
    }

    /// `command` as every program this module starts itself is run: through
    /// `forge::run_child`, standard input empty, killed with what it started once it has run
    /// for `seconds`, so no test here waits on a program without a bound. An error past the
    /// bound.
    fn bounded(command: &mut Command, seconds: u64) -> anyhow::Result<takt::forge::Finished> {
        takt::forge::run_child(
            command,
            Limits {
                timeout: Duration::from_secs(seconds),
                output: 4096,
            },
        )
    }

    /// `script` with `/bin/sh`, unconfined, held to [`bounded`]'s bound of `seconds`.
    fn shell(script: &str, seconds: u64) -> anyhow::Result<takt::forge::Finished> {
        bounded(Command::new("/bin/sh").args(["-c", script]), seconds)
    }

    /// Whether `path` exists within `seconds`.
    fn appears(path: &Path, seconds: u64) -> bool {
        let until = Instant::now() + Duration::from_secs(seconds);
        while Instant::now() < until {
            if path.exists() {
                return true;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        path.exists()
    }

    /// Removes the launchd job `label`, should one have been submitted.
    fn remove_job(label: &str) {
        let _ = bounded(Command::new("launchctl").args(["remove", label]), 10);
    }

    /// The `launchctl submit` of a job `label` that writes `target`, then a pause for launchd
    /// to start it.
    fn submit(label: &str, target: &Path) -> String {
        format!(
            "launchctl submit -l '{label}' -- /usr/bin/touch '{}'; sleep 2",
            target.display()
        )
    }

    /// story:wave-35-review-fixes, acceptance 1 (H1): a job submitted to launchd runs outside
    /// every sandbox, so a confined build and a test command submit none: the file the job
    /// would write is not there. The same submission unconfined writes it, so the job is one
    /// launchd runs here.
    #[test]
    fn a_confined_step_starts_no_launchd_job() {
        let dir = scratch("launchd");
        let tree = dir.0.join("tree");
        fs::create_dir_all(&tree).expect("the tree");

        let label = format!("takt.test.unconfined.{}", std::process::id());
        let control = dir.0.join("unconfined");
        let output = shell(&submit(&label, &control), 30).expect("sh runs and ends");
        let ran = appears(&control, 5);
        remove_job(&label);
        assert!(
            ran,
            "launchd runs a job submitted unconfined: {}",
            String::from_utf8_lossy(&output.stderr)
        );

        let label = format!("takt.test.build.{}", std::process::id());
        let escaped = dir.0.join("escaped-build");
        let output = confined_build(&tree, &submit(&label, &escaped));
        let escaped_build = appears(&escaped, 3);
        remove_job(&label);
        assert!(
            !escaped_build,
            "story:confinement-through-substrate-on-linux-and-macos: a confined build submits no job: {}",
            String::from_utf8_lossy(&output.stderr)
        );

        let label = format!("takt.test.command.{}", std::process::id());
        let escaped = dir.0.join("escaped-test");
        let finished = test_command(&tree, &submit(&label, &escaped));
        let escaped_test = appears(&escaped, 3);
        remove_job(&label);
        assert!(
            !escaped_test,
            "story:confinement-through-substrate-on-linux-and-macos: a test command submits no job: {}",
            String::from_utf8_lossy(&finished.stderr)
        );
    }

    /// story:wave-35-review-fixes, acceptance 2 (M2): a test command connects to no socket
    /// under `/private/tmp/com.apple.launchd.*`, where launchd puts the ssh agent's; the same
    /// connection unconfined is taken. The listener never answers nor closes, and macOS's `nc`
    /// may stay connected after its standard input ends, so each `nc` gives up after a second
    /// idle (`-w 1`) and each run is bounded besides: unconfined by [`bounded`], whose bound
    /// reached is no failure, as the connection is what is asked; confined by its timeout.
    #[test]
    fn a_test_command_connects_to_no_launchd_socket() {
        use std::os::unix::net::UnixListener;
        let dir = scratch("launchd-socket");
        let tree = dir.0.join("tree");
        fs::create_dir_all(&tree).expect("the tree");
        let sockets = Scratch(PathBuf::from(format!(
            "/private/tmp/com.apple.launchd.takt-test-{}",
            std::process::id()
        )));
        let _ = fs::remove_dir_all(&sockets.0);
        fs::create_dir_all(&sockets.0).expect("a launchd-like directory");
        let socket = sockets.0.join("Listeners");
        let listener = UnixListener::bind(&socket).expect("the socket");
        listener.set_nonblocking(true).expect("non-blocking");
        let connect = format!("/usr/bin/nc -U -w 1 '{}' < /dev/null", socket.display());

        let unconfined = shell(&connect, 10);
        assert!(
            listener.accept().is_ok(),
            "nc connects unconfined: {unconfined:?}"
        );

        let finished = test_command(&tree, &connect);
        assert_ne!(
            finished.code,
            Some(0),
            "the connection fails: {}",
            String::from_utf8_lossy(&finished.stderr)
        );
        assert!(
            listener.accept().is_err(),
            "story:confinement-through-substrate-on-linux-and-macos: the socket took no connection from the test command"
        );
    }

    /// story:wave-35-review-fixes, acceptance 3 (M4): neither a confined build nor a test
    /// command writes the user's temporary or cache directory, which the system's tools read
    /// for every program of the user; the test command's `TMPDIR` is a directory of its own,
    /// which it writes: Substrate's private one where the adapter proves `private_tmpdir`,
    /// removed once the run ended.
    #[test]
    fn a_confined_step_writes_no_user_directory_of_the_system() {
        let dir = scratch("user-dirs");
        let tree = dir.0.join("tree");
        fs::create_dir_all(&tree).expect("the tree");
        let getconf = |name: &str| -> PathBuf {
            let output =
                bounded(Command::new("getconf").arg(name), 10).expect("getconf runs and ends");
            PathBuf::from(String::from_utf8_lossy(&output.stdout).trim())
        };
        for name in ["DARWIN_USER_CACHE_DIR", "DARWIN_USER_TEMP_DIR"] {
            let planted = getconf(name).join(format!("takt-planted-{}", std::process::id()));
            let script = format!("echo planted > '{}'", planted.display());

            let output = confined_build(&tree, &script);
            let after_build = planted.exists();
            let _ = fs::remove_file(&planted);
            assert!(
                output.code != Some(0) && !after_build,
                "story:confinement-through-substrate-on-linux-and-macos: a confined build does not write {name}: {}",
                String::from_utf8_lossy(&output.stderr)
            );

            let finished = test_command(&tree, &script);
            let after_test = planted.exists();
            let _ = fs::remove_file(&planted);
            assert!(
                finished.code != Some(0) && !after_test,
                "story:confinement-through-substrate-on-linux-and-macos: a test command does not write {name}: {}",
                String::from_utf8_lossy(&finished.stderr)
            );
        }

        let finished = test_command(
            &tree,
            "echo tmp > \"$TMPDIR/own\" && cat \"$TMPDIR/own\" && printf '%s\\n' \"$TMPDIR\"",
        );
        let stdout = String::from_utf8_lossy(&finished.stdout).into_owned();
        assert_eq!(
            finished.code,
            Some(0),
            "{stdout}{}",
            String::from_utf8_lossy(&finished.stderr)
        );
        let lines: Vec<&str> = stdout.lines().collect();
        assert_eq!(lines.first(), Some(&"tmp"), "{stdout}");
        let tmp = Path::new(lines[1]);
        let user_tmp = getconf("DARWIN_USER_TEMP_DIR");
        assert!(
            tmp != user_tmp && (tmp.starts_with(&tree) || !tmp.exists()),
            "story:confinement-through-substrate-on-linux-and-macos: TMPDIR {tmp:?} is the test command's own: in its tree, or Substrate's private one, gone once the run ended"
        );
    }

    /// story:wave-35-review-fixes, acceptance 3 (M5): a test command in a linked worktree
    /// writes neither the hooks nor the configuration of the common git directory. Since
    /// story:confinement-through-substrate-on-linux-and-macos it writes nothing of that
    /// directory: only its tree is the adapter's workspace.
    #[test]
    fn a_test_command_writes_no_hook_nor_configuration_of_the_common_git_dir() {
        let dir = scratch("git-hooks");
        let (main, linked) = linked_worktree(&dir.0);
        let common = main.join(".git");

        let finished = test_command(
            &linked,
            &format!("echo ok > '{}'", common.join("takt-probe").display()),
        );
        assert_ne!(
            finished.code,
            Some(0),
            "story:confinement-through-substrate-on-linux-and-macos: the common git directory is not written: {}",
            String::from_utf8_lossy(&finished.stderr)
        );

        let config = fs::read(common.join("config")).expect("the configuration");
        for (what, script) in [
            (
                "hooks/probe",
                format!(
                    "echo '#!/bin/sh' > '{}'",
                    common.join("hooks/probe").display()
                ),
            ),
            (
                "config",
                format!(
                    "printf '[core]\\n\\tfsmonitor = /bin/true\\n' >> '{}'",
                    common.join("config").display()
                ),
            ),
            (
                "hooks, renamed",
                format!(
                    "mv '{}' '{}'",
                    common.join("hooks").display(),
                    common.join("hooks-moved").display()
                ),
            ),
        ] {
            let finished = test_command(&linked, &script);
            assert_ne!(
                finished.code,
                Some(0),
                "{what} is not written: {}",
                String::from_utf8_lossy(&finished.stderr)
            );
        }
        assert!(!common.join("hooks/probe").exists(), "no hook planted");
        assert!(
            common.join("hooks").is_dir(),
            "the hooks stay where they are"
        );
        assert_eq!(
            fs::read(common.join("config")).expect("the configuration"),
            config,
            "the configuration is as it was"
        );
    }
}
