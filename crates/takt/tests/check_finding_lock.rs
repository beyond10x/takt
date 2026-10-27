//! story:check-finding-runs-one-at-a-time: `scripts/check-finding` builds and runs one check at a
//! time per cache directory, under an advisory `flock` on `takt/check-finding/lock` there, and
//! says so when it waits; it refuses before building when the file system of that directory has
//! less free space than `CHECK_FINDING_MIN_FREE_GIB` (20 GiB by default), naming both; and a run
//! killed while it holds the lock leaves none behind. Nothing is built: the confiner on `PATH`
//! (bubblewrap, on macOS Seatbelt's) is a stand-in that records the start and the end of the
//! build it was asked for and fails it, and rustup's home is a stand-in toolchain, so every check
//! ends `takt does not build`, exit 1.

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Output, Stdio};
use std::time::{Duration, Instant};

const CHECK_FINDING: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../scripts/check-finding");

/// The confiner `check-finding` builds under, as the script looks it up on `PATH`: bubblewrap,
/// and on macOS Seatbelt's. Read from the script: since
/// story:confinement-through-substrate-on-linux-and-macos no crate of takt names a confinement
/// program, and only the script under test runs one.
fn confiner() -> &'static str {
    static NAME: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    NAME.get_or_init(|| {
        let script = fs::read_to_string(CHECK_FINDING).expect("the script");
        let lookup = if cfg!(target_os = "macos") {
            "confiner=$(command -v "
        } else {
            "bwrap=$(command -v "
        };
        script
            .lines()
            .find_map(|line| {
                Some(
                    line.trim()
                        .strip_prefix(lookup)?
                        .split(')')
                        .next()?
                        .to_owned(),
                )
            })
            .expect("check-finding looks its confiner up on PATH")
    })
}

/// The stand-in for the confiner of the build named `name`: records `start <name> <pid>` in
/// `log`, sleeps `sleep` seconds, records `end <name>` and fails the build. What it records is
/// written into the script, not read from the environment: on macOS the check clears the
/// environment before it starts the confiner (`env -i`), as bubblewrap's `--clearenv` does only
/// after it started.
fn standin(name: &str, sleep: u32, log: &Path) -> String {
    let log = log.to_str().expect("UTF-8").replace('\'', "'\\''");
    format!(
        "#!/bin/sh\n\
         printf 'start %s %s\\n' '{name}' \"$$\" >> '{log}'\n\
         sleep {sleep}\n\
         printf 'end %s\\n' '{name}' >> '{log}'\n\
         exit 1\n"
    )
}

/// The finding the checks run: its predicate in the repository form, never reached.
const FINDING: &str = "# A finding\n\nText.\n\n```expected_before\n\
repository: https://example.org/scenarios\n\
seed: 0123456789abcdef0123456789abcdef01234567\n\
command: \"$TAKT\" --version\n\
exit: 3\n\
source: docs/runs/trial-1.md\n\
```\n";

struct Scratch(PathBuf);

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn write(path: &Path, text: &str) {
    fs::create_dir_all(path.parent().expect("a parent")).expect("a directory");
    fs::write(path, text).expect("a file");
}

fn executable(path: &Path, text: &str) {
    write(path, text);
    fs::set_permissions(path, fs::Permissions::from_mode(0o755)).expect("executable");
}

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
    String::from_utf8_lossy(&output.stdout).trim().to_owned()
}

fn shown(output: &Output) -> String {
    format!(
        "exit {:?}\nstdout:\n{}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
}

/// A repository holding the finding `story:lock-probe`, its revision, a stand-in rustup home and
/// the cache the checks share; each check puts its own stand-in confiner first on `PATH`.
struct World {
    dir: Scratch,
    repo: PathBuf,
    sha: String,
}

fn world(name: &str) -> World {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("check-finding-lock-{}-{name}", std::process::id()));
    if dir.exists() {
        fs::remove_dir_all(&dir).expect("a stale scratch tree is removed");
    }
    fs::create_dir_all(&dir).expect("a scratch directory");
    let dir = Scratch(dir);
    let repo = dir.0.join("repo");
    write(
        &repo.join(".engineering/planning/story/lock-probe.md"),
        FINDING,
    );
    git(&repo, &["init", "-q", "-b", "main"]);
    git(&repo, &["add", "-A"]);
    git(&repo, &["commit", "-q", "-m", "seed"]);
    let sha = git(&repo, &["rev-parse", "HEAD"]);
    let toolchain = dir
        .0
        .join("rustup/toolchains/stable-x86_64-unknown-linux-gnu");
    write(
        &dir.0.join("rustup/settings.toml"),
        "default_toolchain = \"stable-x86_64-unknown-linux-gnu\"\n",
    );
    executable(&toolchain.join("bin/cargo"), "#!/bin/sh\nexit 1\n");
    executable(
        &toolchain.join("bin/rustc"),
        "#!/bin/sh\necho 'host: x86_64-unknown-linux-gnu'\n",
    );
    fs::create_dir_all(toolchain.join("lib/rustlib")).expect("a directory");
    World { dir, repo, sha }
}

impl World {
    fn log(&self) -> PathBuf {
        self.dir.0.join("builds.log")
    }

    fn cache(&self) -> PathBuf {
        self.dir.0.join("cache")
    }

    /// The lines the stand-in build wrote, its pids dropped.
    fn builds(&self) -> Vec<String> {
        fs::read_to_string(self.log())
            .unwrap_or_default()
            .lines()
            .map(|line| line.split(' ').take(2).collect::<Vec<_>>().join(" "))
            .collect()
    }

    /// `check-finding` on the finding at the revision, its build the stand-in named `name`
    /// sleeping `sleep` seconds, `env` added.
    fn check(&self, name: &str, sleep: u32, env: &[(&str, &str)]) -> Command {
        let bin = self.dir.0.join(format!("bin-{name}"));
        executable(&bin.join(confiner()), &standin(name, sleep, &self.log()));
        let path = format!(
            "{}:{}",
            bin.display(),
            std::env::var("PATH").expect("a PATH")
        );
        let mut command = Command::new(CHECK_FINDING);
        command
            .args(["--repo", self.repo.to_str().expect("UTF-8")])
            .args(["story:lock-probe", &self.sha])
            .current_dir(&self.repo)
            .env("PATH", path)
            .env("XDG_CACHE_HOME", self.cache())
            .env("RUSTUP_HOME", self.dir.0.join("rustup"))
            .env("CHECK_FINDING_MIN_FREE_GIB", "0")
            .envs(env.iter().copied())
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        command
    }

    /// Waits until the stand-in build `name` has started, at most 60 s.
    fn started(&self, name: &str) {
        let deadline = Instant::now() + Duration::from_secs(60);
        let line = format!("start {name}");
        while !self.builds().contains(&line) {
            assert!(
                Instant::now() < deadline,
                "the build {name} did not start within 60 s: {:?}",
                self.builds()
            );
            std::thread::sleep(Duration::from_millis(50));
        }
    }

    /// The pid the stand-in build `name` recorded.
    fn pid(&self, name: &str) -> String {
        let prefix = format!("start {name} ");
        fs::read_to_string(self.log())
            .unwrap_or_default()
            .lines()
            .find_map(|line| line.strip_prefix(&prefix).map(str::to_owned))
            .unwrap_or_else(|| panic!("no start of {name} recorded"))
    }
}

/// Waits for `child` at most `limit`; kills it and fails naming `what` past that.
fn finished(mut child: Child, limit: Duration, what: &str) -> Output {
    let deadline = Instant::now() + limit;
    loop {
        if child.try_wait().expect("the check is waited on").is_some() {
            return child.wait_with_output().expect("the check's output");
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let output = child.wait_with_output().expect("the check's output");
            panic!("{what}: still running after {limit:?}\n{}", shown(&output));
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

fn refused_build(output: &Output) {
    assert_eq!(output.status.code(), Some(1), "{}", shown(output));
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("takt does not build"),
        "the stand-in build failed the check: {}",
        shown(output)
    );
}

/// Acceptance 1: a second check started while the first builds waits on the lock under the
/// cache directory, says it is waiting, and builds only once the first has ended.
#[test]
fn two_checks_at_once_build_one_after_the_other_and_the_second_says_it_waits() {
    let world = world("two");
    let first = world
        .check("A", 3, &[])
        .spawn()
        .expect("the first check starts");
    world.started("A");
    let second = world
        .check("B", 0, &[])
        .spawn()
        .expect("the second check starts");
    let first = finished(first, Duration::from_secs(120), "the first check");
    let second = finished(second, Duration::from_secs(120), "the second check");
    refused_build(&first);
    refused_build(&second);
    assert_eq!(
        world.builds(),
        ["start A", "end A", "start B", "end B"],
        "the builds overlap\nfirst: {}\nsecond: {}",
        shown(&first),
        shown(&second)
    );
    let lock = world.cache().join("takt/check-finding/lock");
    let waited = String::from_utf8_lossy(&second.stderr);
    assert!(
        waited.contains("waiting") && waited.contains(lock.to_str().expect("UTF-8")),
        "the second check says it waits on {}: {}",
        lock.display(),
        shown(&second)
    );
    assert!(
        !String::from_utf8_lossy(&first.stderr).contains("waiting"),
        "the first check waited on nothing: {}",
        shown(&first)
    );
}

/// Acceptance 2: under the threshold the check refuses before building, exit 1, naming the free
/// space and the threshold; nothing is built.
#[test]
fn a_check_on_a_file_system_under_the_threshold_refuses_before_building() {
    let world = world("low");
    let output = world
        .check("A", 0, &[("CHECK_FINDING_MIN_FREE_GIB", "999999")])
        .output()
        .expect("the check runs");
    assert_eq!(output.status.code(), Some(1), "{}", shown(&output));
    assert!(output.stdout.is_empty(), "no result: {}", shown(&output));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("GiB free") && stderr.contains("threshold of 999999 GiB"),
        "the refusal names the free space and the threshold: {}",
        shown(&output)
    );
    assert!(
        stderr.contains(
            world
                .cache()
                .join("takt/check-finding")
                .to_str()
                .expect("UTF-8")
        ),
        "the refusal names the cache directory: {}",
        shown(&output)
    );
    assert!(world.builds().is_empty(), "built: {:?}", world.builds());
    let builds = world.cache().join("takt/check-finding/builds");
    let left: Vec<_> = fs::read_dir(&builds)
        .map(|entries| {
            entries
                .map(|entry| entry.expect("an entry").file_name())
                .collect()
        })
        .unwrap_or_default();
    assert!(left.is_empty(), "the build directory holds {left:?}");
}

/// The threshold is a whole number of GiB; anything else is a usage error, exit 64, and nothing
/// is built.
#[test]
fn a_threshold_that_is_no_whole_number_of_gib_is_a_usage_error() {
    let world = world("malformed");
    for threshold in ["twenty", "-1", "1.5", ""] {
        let output = world
            .check("A", 0, &[("CHECK_FINDING_MIN_FREE_GIB", threshold)])
            .output()
            .expect("the check runs");
        assert_eq!(
            output.status.code(),
            Some(64),
            "{threshold:?}: {}",
            shown(&output)
        );
        assert!(
            String::from_utf8_lossy(&output.stderr).contains("CHECK_FINDING_MIN_FREE_GIB"),
            "{threshold:?}: {}",
            shown(&output)
        );
    }
    assert!(world.builds().is_empty(), "built: {:?}", world.builds());
}

/// Acceptance 3: a check killed while it holds the lock, its build still running, leaves no lock
/// that blocks the next: the next builds at once, without waiting.
#[test]
fn a_check_killed_while_it_holds_the_lock_leaves_none_that_blocks_the_next() {
    let world = world("killed");
    let mut first = world
        .check("A", 30, &[])
        .spawn()
        .expect("the first check starts");
    world.started("A");
    assert!(
        world.cache().join("takt/check-finding/lock").is_file(),
        "the check holds its lock on a file under the cache directory"
    );
    first.kill().expect("the first check is killed");
    let _ = first.wait();
    let second = world
        .check("B", 0, &[])
        .spawn()
        .expect("the second check starts");
    let second = finished(
        second,
        Duration::from_secs(20),
        "the check after a killed one",
    );
    // The killed check's build outlives it; it is a stand-in and is ended here.
    let orphan = world.pid("A");
    let _ = Command::new("kill").arg(&orphan).status();
    refused_build(&second);
    assert_eq!(
        world.builds(),
        ["start A", "start B", "end B"],
        "the next check built while the killed one's build still ran: {}",
        shown(&second)
    );
    assert!(
        !String::from_utf8_lossy(&second.stderr).contains("waiting"),
        "the next check waited on a lock nobody holds: {}",
        shown(&second)
    );
}
