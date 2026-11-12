//! story:a-run-on-a-schedule through the binary: `takt run --every <duration>` stays resident and
//! starts one tick per slot, each a child `takt run --once --outcome-file <tick>/outcome.json`;
//! it stops on an `--until` condition, a refused tick or a signal, and writes every tick to files
//! under `<state dir>/schedule/<id>/`. `--outcome-file` writes `takt.run-outcome/1` for any run.
//!
//! The work is the `command` source's `select`, given programs every Unix has: `cat` of an empty
//! file (nothing ready at once) or `sleep 2` (nothing ready, two seconds later). Every schedule
//! here is `--every 1s` and ends within a few seconds.

#[path = "../../takt/tests/support/own_home.rs"]
mod own_home;

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

/// How long one schedule may take before the test fails.
const LIMIT: Duration = Duration::from_secs(30);

/// A run file whose `select` prints nothing: nothing ready, at once.
const NOTHING: &str =
    "work = [\"command\"]\nwork_commands = { select = [\"cat\", \"{items}/none.json\"] }\n";

/// A run file whose `select` sleeps two seconds and prints nothing: a tick longer than `1s`.
const SLOW: &str = "work = [\"command\"]\nwork_commands = { select = [\"sleep\", \"2\"] }\n";

fn git(tree: &Path, args: &[&str]) {
    let status = Command::new("git")
        .args(["-c", "user.name=t", "-c", "user.email=t@localhost"])
        .args(args)
        .current_dir(tree)
        .status()
        .expect("git runs");
    assert!(status.success(), "git {args:?}");
}

/// A scratch directory under this crate's build directory, removed on drop: `tree` the
/// repository, `items` the files the commands read, `state` the state directory.
struct Scratch(PathBuf);

impl Scratch {
    fn tree(&self) -> PathBuf {
        self.0.join("tree")
    }

    fn state(&self) -> PathBuf {
        self.0.join("state")
    }

    /// A run file `name` holding `text`, `{items}` replaced by the items directory.
    fn config_named(&self, name: &str, text: &str) -> PathBuf {
        let path = self.0.join(name);
        let items = self.0.join("items").display().to_string();
        fs::write(&path, text.replace("{items}", &items)).expect("a run configuration file");
        path
    }

    fn config(&self, text: &str) -> PathBuf {
        self.config_named("run.toml", text)
    }

    /// The one schedule's directory under the state directory `state`.
    fn schedule_dir_in(state: &Path) -> PathBuf {
        let dirs: Vec<PathBuf> = fs::read_dir(state.join("schedule"))
            .expect("a schedule directory")
            .map(|entry| entry.expect("an entry").path())
            .collect();
        assert_eq!(dirs.len(), 1, "one schedule: {dirs:?}");
        dirs[0].clone()
    }

    fn schedule_dir(&self) -> PathBuf {
        Self::schedule_dir_in(&self.state())
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// A committed repository with no planning store, an empty item file and a state directory.
fn scratch(name: &str) -> Scratch {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("run-schedule-{}-{name}", std::process::id()));
    if dir.exists() {
        fs::remove_dir_all(&dir).expect("a stale scratch tree is removed");
    }
    let scratch = Scratch(dir);
    let tree = scratch.tree();
    fs::create_dir_all(&tree).expect("the tree");
    fs::create_dir_all(scratch.0.join("items")).expect("the items");
    fs::create_dir_all(scratch.state()).expect("the state directory");
    fs::write(scratch.0.join("items/none.json"), "").expect("an empty item file");
    fs::write(tree.join("check.txt"), "broken\n").expect("a file");
    git(&tree, &["init", "-q", "-b", "main"]);
    git(&tree, &["add", "-A"]);
    git(&tree, &["commit", "-q", "-m", "start"]);
    scratch
}

/// The built `takt run` on `repo` with no harness, one iteration, a passing test command and
/// the state directory `state`, plus `extra`, started with its output to files beside `state`.
fn start(repo: &Path, state: &Path, extra: &[&str]) -> Child {
    Command::new(env!("CARGO_BIN_EXE_takt"))
        .args(["run", "--repo"])
        .arg(repo)
        .args([
            "--harness",
            "none",
            "--iterations",
            "1",
            "--test-cmd",
            "true",
        ])
        .arg("--state-dir")
        .arg(state)
        .args(extra)
        .stdin(Stdio::null())
        .stdout(fs::File::create(state.with_extension("stdout")).expect("a stdout file"))
        .stderr(fs::File::create(state.with_extension("stderr")).expect("a stderr file"))
        .spawn()
        .expect("takt runs")
}

/// Waits for `child` up to [`LIMIT`]: its exit code, stdout and stderr.
fn finish(mut child: Child, state: &Path) -> (i32, String, String) {
    let started = Instant::now();
    let status = loop {
        if let Some(status) = child.try_wait().expect("the run is waited on") {
            break status;
        }
        if started.elapsed() > LIMIT {
            let _ = child.kill();
            let _ = child.wait();
            panic!(
                "the schedule did not end within {LIMIT:?}: {}",
                fs::read_to_string(state.with_extension("stderr")).unwrap_or_default()
            );
        }
        std::thread::sleep(Duration::from_millis(20));
    };
    (
        status.code().unwrap_or(-1),
        fs::read_to_string(state.with_extension("stdout")).unwrap_or_default(),
        fs::read_to_string(state.with_extension("stderr")).unwrap_or_default(),
    )
}

fn run(repo: &Path, state: &Path, extra: &[&str]) -> (i32, String, String) {
    finish(start(repo, state, extra), state)
}

fn json(path: &Path) -> serde_json::Value {
    let text = fs::read_to_string(path).unwrap_or_else(|_| panic!("{} reads", path.display()));
    serde_json::from_str(&text).unwrap_or_else(|_| panic!("{} is JSON: {text}", path.display()))
}

fn ticks(dir: &Path) -> Vec<serde_json::Value> {
    fs::read_to_string(dir.join("ticks.jsonl"))
        .unwrap_or_default()
        .lines()
        .map(|line| serde_json::from_str(line).expect("a tick line is JSON"))
        .collect()
}

/// Waits until `ready` holds, up to [`LIMIT`].
fn wait_for(what: &str, ready: impl Fn() -> bool) {
    let started = Instant::now();
    while !ready() {
        assert!(started.elapsed() < LIMIT, "waited too long for {what}");
        std::thread::sleep(Duration::from_millis(20));
    }
}

/// Whether the schedule under `state` has started a tick: a tick directory exists.
fn a_tick_started(state: &Path) -> bool {
    fs::read_dir(state.join("schedule"))
        .into_iter()
        .flatten()
        .flatten()
        .any(|dir| {
            fs::read_dir(dir.path().join("ticks")).is_ok_and(|mut ticks| ticks.next().is_some())
        })
}

fn terminate(child: &Child) {
    let pid = libc::pid_t::try_from(child.id()).expect("a pid");
    // SAFETY: a signal to the process this test started.
    assert_eq!(unsafe { libc::kill(pid, libc::SIGTERM) }, 0);
}

/// `seconds` after the Unix epoch as an RFC 3339 instant in UTC.
fn rfc3339(seconds: u64) -> String {
    let days = i64::try_from(seconds / 86_400).expect("days");
    let rest = seconds % 86_400;
    // Howard Hinnant's civil_from_days.
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}Z",
        rest / 3600,
        rest % 3600 / 60,
        rest % 60
    )
}

/// The instant `seconds` from now, RFC 3339.
fn in_seconds(seconds: u64) -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("after the epoch")
        .as_secs();
    rfc3339(now + seconds)
}

#[test]
fn schedule_until_nothing_ready() {
    let scratch = scratch("nothing-ready");
    let config = scratch.config(NOTHING);
    let (code, out, err) = run(
        &scratch.tree(),
        &scratch.state(),
        &[
            "--config",
            config.to_str().expect("utf-8"),
            "--every",
            "1s",
            "--until",
            "nothing-ready",
        ],
    );
    assert_eq!(code, 0, "stdout {out} stderr {err}");
    assert!(out.contains("schedule: done (nothing-ready)"), "{out}");
    let dir = scratch.schedule_dir();
    let lines = ticks(&dir);
    assert_eq!(lines.len(), 1, "{lines:?}");
    assert_eq!(lines[0]["format"], "takt.schedule-tick/1");
    assert_eq!(lines[0]["exit"], 0);
    let outcome = json(Path::new(
        lines[0]["outcome"].as_str().expect("an outcome path"),
    ));
    assert_eq!(outcome["format"], "takt.run-outcome/1");
    assert_eq!(outcome["frontier"], "nothing_ready");
    assert!(outcome["case"].is_null());
    let tick = Path::new(lines[0]["outcome"].as_str().expect("a path"))
        .parent()
        .expect("the tick's directory")
        .to_path_buf();
    assert_eq!(
        tick.parent()
            .and_then(Path::file_name)
            .and_then(|n| n.to_str()),
        Some("ticks")
    );
    assert_eq!(
        fs::read_to_string(tick.join("stdout")).expect("the tick's stdout"),
        "frontier: nothing ready\n"
    );
    assert!(tick.join("stderr").is_file());
    let end = json(&dir.join("end.json"));
    assert_eq!(end["reason"], "nothing-ready");
    assert_eq!(end["exit"], 0);
    let schedule = json(&dir.join("schedule.json"));
    assert_eq!(schedule["resolved"]["every"], "1s");
    assert!(schedule["pid"].as_u64().is_some());
    let args: Vec<&str> = schedule["args"]
        .as_array()
        .expect("the ticks' arguments")
        .iter()
        .map(|arg| arg.as_str().expect("a string"))
        .collect();
    assert!(
        !args.contains(&"--every") && !args.contains(&"--until"),
        "{args:?}"
    );
}

#[test]
fn schedule_until_exit() {
    let scratch = scratch("exit");
    let config = scratch.config(NOTHING);
    let (code, out, err) = run(
        &scratch.tree(),
        &scratch.state(),
        &[
            "--config",
            config.to_str().expect("utf-8"),
            "--every",
            "1s",
            "--until",
            "exit:0",
        ],
    );
    assert_eq!(code, 0, "stdout {out} stderr {err}");
    let dir = scratch.schedule_dir();
    assert_eq!(ticks(&dir).len(), 1);
    let end = json(&dir.join("end.json"));
    assert_eq!(end["reason"], "exit", "{end}");
    assert_eq!(end["until"], "exit:0", "{end}");
    assert_eq!(end["exit"], 0);
}

#[test]
fn schedule_until_deadline() {
    let scratch = scratch("deadline");
    let config = scratch.config(NOTHING);
    let deadline = format!("deadline:{}", in_seconds(3));
    let (code, out, err) = run(
        &scratch.tree(),
        &scratch.state(),
        &[
            "--config",
            config.to_str().expect("utf-8"),
            "--every",
            "1s",
            "--until",
            &deadline,
        ],
    );
    assert_eq!(code, 0, "stdout {out} stderr {err}");
    assert!(out.contains("schedule: done (deadline)"), "{out}");
    let dir = scratch.schedule_dir();
    let lines = ticks(&dir);
    assert!(
        (2..=4).contains(&lines.len()),
        "a tick a second until the deadline: {lines:?}"
    );
    let end = json(&dir.join("end.json"));
    assert_eq!(end["reason"], "deadline", "{end}");
    assert_eq!(end["exit"], 0);
}

/// A protocol whose one outcome, `released`, is legitimate once the test command passed.
const RELEASED: &str = "format: protocol/1

protocol:
  id: schedule.probe
  revision: 1
  description: reaches released once the test command passed

artifacts:
  implementation:
    description: a revision of the repository

evidence_kinds:
  test_result:
    description: the result of the test command; pass or fail

claims:
  tests.pass:
    description: the test command passes
    true_when:
      evidence:
        kind: test_result
        result: pass
        subject: implementation

obligations:
  tests.required:
    description: the test command passes
    discharged_when:
      claim: tests.pass

actions:
  tests.run:
    description: run the test command
    effect: read
    may_produce:
      - evidence: test_result

outcomes:
  released:
    description: the test command passed
    requires:
      claim: tests.pass
";

#[test]
fn schedule_until_outcome() {
    let scratch = scratch("outcome");
    let protocol = scratch.config_named("released.yaml", RELEASED);
    let (code, out, err) = run(
        &scratch.tree(),
        &scratch.state(),
        &[
            "--intent",
            "release it",
            "--protocol",
            protocol.to_str().expect("utf-8"),
            "--every",
            "1s",
            "--until",
            "outcome:released",
        ],
    );
    assert_eq!(code, 0, "stdout {out} stderr {err}");
    let dir = scratch.schedule_dir();
    let lines = ticks(&dir);
    assert_eq!(lines.len(), 1, "{lines:?}");
    let outcome = json(Path::new(lines[0]["outcome"].as_str().expect("a path")));
    assert_eq!(outcome["outcome"], "released", "{outcome}");
    assert_eq!(outcome["stop"], "Completed", "{outcome}");
    let end = json(&dir.join("end.json"));
    assert_eq!(end["reason"], "outcome", "{end}");
    assert_eq!(end["until"], "outcome:released", "{end}");
    assert_eq!(end["exit"], 0);
}

#[test]
fn schedule_refused_tick_ends_it() {
    let scratch = scratch("refused");
    let config = scratch.config(NOTHING);
    let state = scratch.state();
    let child = start(
        &scratch.tree(),
        &state,
        &["--config", config.to_str().expect("utf-8"), "--every", "1s"],
    );
    // The first tick finds nothing ready and exits 0; the run file then holds a `tokens` no
    // tick reads, so the next one exits 2, as `--tokens 0` would.
    wait_for("the first tick", || {
        fs::read_dir(state.join("schedule"))
            .into_iter()
            .flatten()
            .flatten()
            .any(|dir| !ticks(&dir.path()).is_empty())
    });
    scratch.config(&format!("tokens = 0\n{NOTHING}"));
    let (code, out, err) = finish(child, &state);
    assert_eq!(code, 2, "stdout {out} stderr {err}");
    let dir = scratch.schedule_dir();
    let lines = ticks(&dir);
    assert_eq!(lines.last().expect("a tick")["exit"], 2, "{lines:?}");
    let end = json(&dir.join("end.json"));
    assert_eq!(end["reason"], "refused", "{end}");
    assert_eq!(end["exit"], 2);
}

#[test]
fn schedule_busy_tick_skips_a_slot() {
    let scratch = scratch("busy");
    let config = scratch.config(SLOW);
    let deadline = format!("deadline:{}", in_seconds(6));
    let (code, out, err) = run(
        &scratch.tree(),
        &scratch.state(),
        &[
            "--config",
            config.to_str().expect("utf-8"),
            "--every",
            "1s",
            "--until",
            &deadline,
        ],
    );
    assert_eq!(code, 0, "stdout {out} stderr {err}");
    let lines = ticks(&scratch.schedule_dir());
    assert!(lines.len() >= 2, "{lines:?}");
    assert!(
        lines[1]["skipped"].as_u64().expect("skipped") >= 1,
        "{lines:?}"
    );
    for pair in lines.windows(2) {
        let ended: &str = pair[0]["ended"].as_str().expect("ended");
        let started: &str = pair[1]["started"].as_str().expect("started");
        // Both are RFC 3339 in UTC, so they order as text when they have the same length.
        assert!(
            instant_nanos(ended) <= instant_nanos(started),
            "the ticks overlap: {pair:?}"
        );
    }
}

/// An RFC 3339 instant in UTC as nanoseconds since its day's start plus the date as a number,
/// enough to order two instants of a test that runs for seconds.
fn instant_nanos(text: &str) -> (String, u128) {
    let (date, time) = text.split_once('T').expect("an instant");
    let time = time.trim_end_matches('Z');
    let (whole, fraction) = time.split_once('.').unwrap_or((time, "0"));
    let parts: Vec<u128> = whole
        .split(':')
        .map(|part| part.parse().expect("a number"))
        .collect();
    let fraction: u128 = format!("{fraction:0<9}")[..9].parse().expect("nanoseconds");
    (
        date.to_owned(),
        ((parts[0] * 60 + parts[1]) * 60 + parts[2]) * 1_000_000_000 + fraction,
    )
}

#[test]
fn schedule_sigterm_ends_after_the_tick() {
    let scratch = scratch("sigterm");
    let config = scratch.config(SLOW);
    let state = scratch.state();
    let child = start(
        &scratch.tree(),
        &state,
        &["--config", config.to_str().expect("utf-8"), "--every", "1s"],
    );
    wait_for("a tick", || a_tick_started(&state));
    std::thread::sleep(Duration::from_millis(300));
    terminate(&child);
    let (code, out, err) = finish(child, &state);
    assert_eq!(code, 3, "stdout {out} stderr {err}");
    let dir = scratch.schedule_dir();
    let lines = ticks(&dir);
    assert_eq!(lines.len(), 1, "no further tick starts: {lines:?}");
    assert_eq!(lines[0]["exit"], 0, "the tick ended on its own: {lines:?}");
    let end = json(&dir.join("end.json"));
    assert_eq!(end["reason"], "signal", "{end}");
    assert_eq!(end["exit"], 3);
}

#[test]
fn schedule_second_signal_forwarded() {
    let scratch = scratch("second-signal");
    let config = scratch.config(SLOW);
    let state = scratch.state();
    let child = start(
        &scratch.tree(),
        &state,
        &["--config", config.to_str().expect("utf-8"), "--every", "1s"],
    );
    wait_for("a tick", || a_tick_started(&state));
    std::thread::sleep(Duration::from_millis(300));
    terminate(&child);
    std::thread::sleep(Duration::from_millis(200));
    terminate(&child);
    let (code, out, err) = finish(child, &state);
    assert_eq!(code, 3, "stdout {out} stderr {err}");
    let dir = scratch.schedule_dir();
    let lines = ticks(&dir);
    assert_eq!(lines.len(), 1, "{lines:?}");
    let exit = lines[0]["exit"].as_i64().expect("an exit");
    assert_ne!(exit, 0, "the second signal reached the tick: {lines:?}");
    let end = json(&dir.join("end.json"));
    assert_eq!(end["reason"], "signal", "{end}");
    assert_eq!(end["exit"], 3);
}

#[test]
fn schedule_second_start_refused() {
    let scratch = scratch("second-start");
    let config = scratch.config(SLOW);
    let state = scratch.state();
    let args = [
        "--config",
        config.to_str().expect("utf-8"),
        "--every",
        "1s",
        "--until",
        "exit:0",
    ];
    let first = start(&scratch.tree(), &state, &args);
    wait_for("a tick", || a_tick_started(&state));
    // The same schedule: the same state directory, the same arguments; its own output files.
    let second = Command::new(env!("CARGO_BIN_EXE_takt"))
        .args(["run", "--repo"])
        .arg(scratch.tree())
        .args([
            "--harness",
            "none",
            "--iterations",
            "1",
            "--test-cmd",
            "true",
        ])
        .arg("--state-dir")
        .arg(&state)
        .args(args)
        .stdin(Stdio::null())
        .output()
        .expect("takt runs");
    let err = String::from_utf8_lossy(&second.stderr);
    assert_eq!(second.status.code(), Some(1), "stderr {err}");
    assert!(
        err.contains(&format!("pid {}", first.id())),
        "the refusal names the first's pid {}: {err}",
        first.id()
    );
    let (code, out, err) = finish(first, &state);
    assert_eq!(code, 0, "stdout {out} stderr {err}");
    assert_eq!(ticks(&scratch.schedule_dir()).len(), 1);
}

#[test]
fn schedule_run_file_key() {
    let scratch = scratch("run-file-key");
    let filed = scratch.config_named(
        "filed.toml",
        &format!("{NOTHING}[schedule]\nevery = \"1s\"\nuntil = [\"nothing-ready\"]\n"),
    );
    let plain = scratch.config(NOTHING);
    let by_file = scratch.0.join("by-file");
    let by_flags = scratch.0.join("by-flags");
    let (code, out, err) = run(
        &scratch.tree(),
        &by_file,
        &["--config", filed.to_str().expect("utf-8")],
    );
    assert_eq!(code, 0, "stdout {out} stderr {err}");
    let (code, out, err) = run(
        &scratch.tree(),
        &by_flags,
        &[
            "--config",
            plain.to_str().expect("utf-8"),
            "--every",
            "1s",
            "--until",
            "nothing-ready",
        ],
    );
    assert_eq!(code, 0, "stdout {out} stderr {err}");
    let file_schedule = json(&Scratch::schedule_dir_in(&by_file).join("schedule.json"));
    let flag_schedule = json(&Scratch::schedule_dir_in(&by_flags).join("schedule.json"));
    assert_eq!(file_schedule["resolved"], flag_schedule["resolved"]);
    assert_eq!(ticks(&Scratch::schedule_dir_in(&by_file)).len(), 1);
}

#[test]
fn schedule_flag_wins() {
    let scratch = scratch("flag-wins");
    let config = scratch.config(&format!(
        "{NOTHING}[schedule]\nevery = \"1s\"\nuntil = [\"nothing-ready\"]\n"
    ));
    let (code, out, err) = run(
        &scratch.tree(),
        &scratch.state(),
        &["--config", config.to_str().expect("utf-8"), "--every", "2s"],
    );
    assert_eq!(code, 0, "stdout {out} stderr {err}");
    let schedule = json(&scratch.schedule_dir().join("schedule.json"));
    assert_eq!(schedule["resolved"]["every"], "2s", "{schedule}");
    assert_eq!(
        schedule["resolved"]["until"][0], "nothing-ready",
        "{schedule}"
    );
}

#[test]
fn schedule_run_file_refusals() {
    let scratch = scratch("refusals");
    let cases = [
        (
            "[schedule]\nevery = \"1s\"\nbogus = \"x\"\n",
            3,
            "`schedule`",
        ),
        (
            "[schedule]\nevery = \"1s\"\ntime_zone = \"Mars/Olympus\"\n",
            3,
            "`schedule.time_zone`",
        ),
        ("[schedule]\nevery = \"0s\"\n", 2, "`schedule.every`"),
        ("[schedule]\nevery = \"500ms\"\n", 2, "`schedule.every`"),
        ("[schedule]\nevery = \"7d1s\"\n", 2, "`schedule.every`"),
        (
            "schedule:\n  every: 18446744073709551615s\n",
            2,
            "`schedule.every`",
        ),
        (
            "[schedule]\nevery = \"1s\"\nwindows = [\"Monday 7\"]\n",
            3,
            "`schedule.windows`",
        ),
        (
            "schedule:\n  every: 1s\n  windows:\n    - Mon 25:00\n",
            3,
            "`schedule.windows`",
        ),
    ];
    for (index, (text, line, member)) in cases.into_iter().enumerate() {
        let name = if text.starts_with("schedule:") {
            format!("refused-{index}.yaml")
        } else {
            format!("refused-{index}.toml")
        };
        let config = scratch.config_named(&name, text);
        let state = scratch.0.join(format!("state-{index}"));
        let (code, out, err) = run(
            &scratch.tree(),
            &state,
            &["--config", config.to_str().expect("utf-8")],
        );
        assert_eq!(code, 2, "{text}: stdout {out} stderr {err}");
        assert!(
            err.contains(&format!("is refused at line {line}: {member}")),
            "{text}: {err}"
        );
        assert!(!state.join("schedule").exists(), "{text}: nothing started");
    }
}

#[test]
fn outcome_file_without_schedule() {
    let scratch = scratch("outcome-file");
    let state = scratch.state();
    let opened = scratch.0.join("opened.json");
    let (code, out, err) = run(
        &scratch.tree(),
        &state,
        &[
            "--intent",
            "fix it",
            "--outcome-file",
            opened.to_str().expect("utf-8"),
        ],
    );
    let outcome = json(&opened);
    assert_eq!(outcome["format"], "takt.run-outcome/1");
    assert_eq!(outcome["exit"], code, "stdout {out} stderr {err}");
    let case = outcome["case"].as_str().expect("a case id");
    assert!(
        scratch.tree().join(".takt/cases").join(case).is_dir(),
        "{outcome}: stdout {out}"
    );
    assert!(outcome["frontier"].is_null(), "{outcome}");
    assert!(outcome["stop"].as_str().is_some(), "{outcome}");
    assert!(outcome["started"].as_str().is_some() && outcome["ended"].as_str().is_some());

    let config = scratch.config(NOTHING);
    let nothing = scratch.0.join("nothing.json");
    let (code, out, err) = run(
        &scratch.tree(),
        &state,
        &[
            "--config",
            config.to_str().expect("utf-8"),
            "--outcome-file",
            nothing.to_str().expect("utf-8"),
        ],
    );
    assert_eq!(code, 0, "stdout {out} stderr {err}");
    assert_eq!(out, "frontier: nothing ready\n", "stdout stays as it is");
    let outcome = json(&nothing);
    assert_eq!(outcome["exit"], 0);
    assert!(outcome["case"].is_null(), "{outcome}");
    assert!(outcome["stop"].is_null(), "{outcome}");
    assert_eq!(outcome["frontier"], "nothing_ready", "{outcome}");
}

/// `--every` above seven days is refused by the argument parser, exit 2, and the outcome file
/// says so; `--help` exits 0 and writes none.
#[test]
fn schedule_every_above_a_week_refused_and_help_writes_no_outcome() {
    let scratch = scratch("every-cap");
    let state = scratch.state();
    let refused = scratch.0.join("refused.json");
    for every in ["8d", "7d1s", "18446744073709551615s"] {
        let (code, out, err) = run(
            &scratch.tree(),
            &state,
            &[
                "--outcome-file",
                refused.to_str().expect("utf-8"),
                "--every",
                every,
            ],
        );
        assert_eq!(code, 2, "{every}: stdout {out} stderr {err}");
        assert!(err.contains("seven days"), "{every}: the rule: {err}");
        assert_eq!(json(&refused)["exit"], 2, "{every}");
        fs::remove_file(&refused).expect("removed");
        assert!(!state.join("schedule").exists(), "{every}: nothing started");
    }
    let helped = scratch.0.join("helped.json");
    let (code, out, err) = run(
        &scratch.tree(),
        &state,
        &["--outcome-file", helped.to_str().expect("utf-8"), "--help"],
    );
    assert_eq!(code, 0, "stderr {err}");
    assert!(out.contains("--outcome-file"), "the help: {out}");
    assert!(!helped.exists(), "--help writes no outcome file");
}

/// A `schedule.json` that names a live pid no start holds the lock for blocks nothing: the
/// lock, not the pid, decides; the schedule starts again from its files.
#[test]
fn schedule_stale_pid_does_not_block_a_start() {
    let scratch = scratch("stale-pid");
    let config = scratch.config(NOTHING);
    let state = scratch.state();
    let args = [
        "--config",
        config.to_str().expect("utf-8"),
        "--every",
        "1s",
        "--until",
        "exit:0",
    ];
    let (code, out, err) = run(&scratch.tree(), &state, &args);
    assert_eq!(code, 0, "stdout {out} stderr {err}");
    let dir = scratch.schedule_dir();
    // A pid that lives (this test's own) and no `end.json`: what a crash leaves after its pid
    // was reused.
    let mut recorded = json(&dir.join("schedule.json"));
    recorded["pid"] = serde_json::json!(std::process::id());
    fs::write(dir.join("schedule.json"), recorded.to_string()).expect("written");
    fs::remove_file(dir.join("end.json")).expect("removed");
    let (code, out, err) = run(&scratch.tree(), &state, &args);
    assert_eq!(code, 0, "stdout {out} stderr {err}");
    assert_eq!(ticks(&dir).len(), 2, "it ran again");
    let started = json(&dir.join("schedule.json"));
    assert_ne!(started["pid"], serde_json::json!(std::process::id()));
}

/// The outcome file is written through a new file renamed over FILE: a symbolic link at FILE
/// is replaced, its target untouched; a directory at FILE is an error on stderr.
#[test]
fn outcome_file_replaces_a_link_and_refuses_a_directory() {
    let scratch = scratch("outcome-link");
    let config = scratch.config(NOTHING);
    let state = scratch.state();
    let target = scratch.0.join("target.txt");
    fs::write(&target, "kept\n").expect("a target");
    let link = scratch.0.join("link.json");
    std::os::unix::fs::symlink(&target, &link).expect("a link");
    let (code, out, err) = run(
        &scratch.tree(),
        &state,
        &[
            "--config",
            config.to_str().expect("utf-8"),
            "--outcome-file",
            link.to_str().expect("utf-8"),
        ],
    );
    assert_eq!(code, 0, "stdout {out} stderr {err}");
    assert_eq!(fs::read_to_string(&target).expect("reads"), "kept\n");
    assert!(
        !fs::symlink_metadata(&link)
            .expect("there")
            .file_type()
            .is_symlink(),
        "the link was replaced"
    );
    assert_eq!(json(&link)["exit"], 0);

    let directory = scratch.0.join("directory");
    fs::create_dir_all(&directory).expect("a directory");
    let (code, out, err) = run(
        &scratch.tree(),
        &state,
        &[
            "--config",
            config.to_str().expect("utf-8"),
            "--outcome-file",
            directory.to_str().expect("utf-8"),
        ],
    );
    assert_eq!(code, 0, "the run's own exit: stdout {out} stderr {err}");
    assert!(err.contains("the outcome file"), "{err}");
    assert!(directory.is_dir());
    let left: Vec<_> = fs::read_dir(&scratch.0)
        .expect("reads")
        .filter_map(|entry| entry.ok())
        .filter(|entry| entry.file_name().to_string_lossy().ends_with(".partial"))
        .collect();
    assert!(left.is_empty(), "no partial file left: {left:?}");
}
