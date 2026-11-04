//! Adversary cases, wave 52 unit b (story:two-trials-started-in-one-second-have-no-case-order):
//! the trial numbers `takt scenario run --forge none` takes without `origin`, through the
//! binary. The world is decline_adv1's: the fixture store, scenario 1, a `claude` double that
//! declines, so each run stops declined (exit 9) on a stop line naming its case. Cases that
//! need a number of the run's own second known beforehand write it for every second of a
//! window around the run, as the run's second cannot be chosen from outside.

#[path = "../../takt/tests/support/own_home.rs"]
mod own_home;

use std::fs;
use std::io::Write as _;
use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

const FIXTURES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../takt/tests/fixtures");

const GATES: &str = "format: engineering-gates/1\nlanguage: canon-expr/1\ngates:\n  - id: manifest\n    \
                     text: The repository has its manifest.\n    assertions:\n      \
                     - 'fs.file.exists(\"Cargo.toml\")'\n";

/// How many nonces a second has (`TRIAL_NONCES` in `crates/takt-cli/src/main.rs`).
const NONCES: u128 = 100_000_000;

/// Seconds of the window, from one before the case starts.
const WINDOW: i64 = 180;

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

/// Every `refs` entry pointing at HEAD, in one `git update-ref --stdin`.
fn refs_at_head(tree: &Path, refs: &[String]) {
    let head = git(tree, &["rev-parse", "HEAD"]);
    let mut child = Command::new("git")
        .args(["update-ref", "--stdin"])
        .current_dir(tree)
        .stdin(Stdio::piped())
        .spawn()
        .expect("git runs");
    {
        let stdin = child.stdin.as_mut().expect("a stdin");
        for name in refs {
            writeln!(stdin, "create {name} {head}").expect("a line");
        }
    }
    assert!(child.wait().expect("git ends").success(), "update-ref");
}

fn copy_dir(from: &Path, to: &Path) {
    fs::create_dir_all(to).expect("a directory");
    for entry in fs::read_dir(from).expect("a readable fixture") {
        let entry = entry.expect("an entry");
        let target = to.join(entry.file_name());
        if entry.file_type().expect("a file type").is_dir() {
            copy_dir(&entry.path(), &target);
        } else {
            fs::copy(entry.path(), &target).expect("a copied file");
        }
    }
}

struct World {
    outer: PathBuf,
    repo: PathBuf,
    bin: PathBuf,
    state: PathBuf,
}

impl Drop for World {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.outer);
    }
}

fn world(name: &str) -> World {
    let outer = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("w52-adversary-{}-{name}", std::process::id()));
    if outer.exists() {
        fs::remove_dir_all(&outer).expect("a stale scratch tree is removed");
    }
    let repo = outer.join("repo");
    copy_dir(&Path::new(FIXTURES).join("one-ready"), &repo);
    copy_dir(&Path::new(FIXTURES).join("scenario-1/base"), &repo);
    for script in ["check.sh", "todo.py"] {
        fs::set_permissions(repo.join(script), fs::Permissions::from_mode(0o755))
            .expect("executable");
    }
    fs::write(repo.join("Cargo.toml"), "[package]\nname = \"scratch\"\n").expect("a file");
    fs::write(repo.join(".engineering/gates.yaml"), GATES).expect("a file");
    fs::write(repo.join(".gitignore"), "__pycache__/\n").expect("a file");
    git(&repo, &["init", "-q", "-b", "main"]);
    git(&repo, &["add", "-A"]);
    git(&repo, &["commit", "-q", "-m", "seed"]);
    let seed = git(&repo, &["rev-parse", "HEAD"]);
    fs::write(
        repo.join(".engineering/scenarios.conf"),
        format!("[scenario \"1\"]\n\tstory = story:ready-one\n\tseed = {seed}\n"),
    )
    .expect("the declaration is written");
    git(&repo, &["add", ".engineering/scenarios.conf"]);
    git(&repo, &["commit", "-q", "-m", "declare the scenarios"]);
    let bin = outer.join("stub-bin");
    fs::create_dir_all(&bin).expect("a stub directory");
    let state = outer.join("state");
    fs::create_dir_all(&state).expect("a state directory");
    let reply = serde_json::json!({
        "result": serde_json::json!({"action": "decline", "summary": "AGENTS.md forbids it"}).to_string(),
        "usage": {"input_tokens": 1}
    })
    .to_string();
    let reply_file = outer.join("reply.json");
    fs::write(&reply_file, reply).expect("the reply");
    let path = bin.join("claude");
    fs::write(
        &path,
        format!(
            "#!/bin/sh\ncase $1 in --version) echo '2.1.309 (Claude Code)'; exit 0 ;; esac\ncat '{}'\n",
            reply_file.display()
        ),
    )
    .expect("the double is written");
    fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).expect("executable");
    World {
        outer,
        repo,
        bin,
        state,
    }
}

impl World {
    /// `takt scenario run 1 --forge none`; the case id its stop line names.
    fn trial(&self) -> String {
        let output = Command::new(env!("CARGO_BIN_EXE_takt"))
            .args([
                "scenario",
                "run",
                "--repo",
                self.repo.to_str().expect("utf-8"),
                "1",
                "--test-cmd",
                "./check.sh",
                "--forge",
                "none",
                "--commit-style",
                "action",
                "--iterations",
                "8",
            ])
            .env("XDG_STATE_HOME", self.state.join("xdg-state"))
            .env(
                "PATH",
                format!(
                    "{}:{}",
                    self.bin.display(),
                    std::env::var("PATH").unwrap_or_default()
                ),
            )
            .output()
            .expect("takt runs");
        let out = String::from_utf8_lossy(&output.stdout);
        let err = String::from_utf8_lossy(&output.stderr);
        assert_eq!(output.status.code(), Some(9), "stdout {out} stderr {err}");
        let stop = out
            .lines()
            .find(|line| line.starts_with("stopped: "))
            .unwrap_or_else(|| panic!("no stop line: stdout {out} stderr {err}"));
        let case = stop
            .rsplit_once(", case ")
            .unwrap_or_else(|| panic!("the stop line names no case: {stop}"))
            .1;
        Path::new(case)
            .file_name()
            .expect("a case directory")
            .to_string_lossy()
            .into_owned()
    }
}

/// The trial number of the case id `id`, `scenario-1-trial-<n>`.
fn number_of(id: &str) -> u128 {
    id.strip_prefix("scenario-1-trial-")
        .unwrap_or_else(|| panic!("{id} is no case of scenario 1"))
        .parse()
        .unwrap_or_else(|_| panic!("{id} has no trial number"))
}

/// `at`, seconds since the epoch, as the 14 digits `YYYYMMDDHHMMSS` of its UTC instant.
fn digits_of(at: i64) -> u128 {
    let days = at.div_euclid(86_400);
    let of_day = at.rem_euclid(86_400);
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    let text = format!(
        "{year:04}{month:02}{day:02}{:02}{:02}{:02}",
        of_day / 3600,
        of_day / 60 % 60,
        of_day % 60
    );
    text.parse().expect("14 digits")
}

/// Every second of the window as its 14 digits, from one second before now; checked against
/// takt's own clock so a wrong conversion fails here and not in the case's assertion.
fn window() -> Vec<u128> {
    let now = i64::try_from(
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("after the epoch")
            .as_secs(),
    )
    .expect("an i64");
    let takt_now: u128 = takt::now()
        .chars()
        .filter(char::is_ascii_digit)
        .collect::<String>()
        .parse()
        .expect("14 digits");
    assert!(
        takt_now == digits_of(now) || takt_now == digits_of(now + 1),
        "the conversion disagrees with takt::now: {takt_now} against {}",
        digits_of(now)
    );
    (now - 1..now - 1 + WINDOW).map(digits_of).collect()
}

/// The second of the trial number `n`, asserted to be in `window`, so a run slower than the
/// window fails as that and not as the case's finding.
fn second_in(window: &[u128], n: u128) -> u128 {
    let second = n / NONCES;
    assert!(
        window.contains(&second),
        "the trial {n} ran outside the prepared window {}..{}",
        window[0],
        window[window.len() - 1]
    );
    second
}

/// Three trials one after another in one checkout, as fast as the binary runs them: their case
/// ids are in the order they started, whichever seconds they fell in.
#[test]
fn three_trials_in_a_row_are_in_start_order() {
    let world = world("three-in-a-row");
    let ids: Vec<String> = (0..3).map(|_| world.trial()).collect();
    println!("w52 three trials: {ids:?}");
    assert!(
        ids[0] < ids[1] && ids[1] < ids[2],
        "case order is not start order: {ids:?}"
    );
}

/// After the wrap past 99999999 the free search still passes over what is taken: the case
/// branch of nonce 00000000 and the case directory of 00000001. The run takes 00000002 and the
/// directory is left as it was. (The unit test `a_trial_number_without_origin_is_the_instant_and_a_free_nonce`
/// no longer reaches a taken case or case branch in the search.)
#[test]
fn after_the_wrap_a_taken_case_branch_and_case_directory_are_passed_over() {
    let world = world("wrap-passes-over");
    let window = window();
    let mut refs = Vec::new();
    for second in &window {
        refs.push(format!("refs/heads/trial/{second}99999999"));
        refs.push(format!("refs/heads/takt/scenario-1-trial-{second}00000000"));
        let dir = world
            .repo
            .join(format!(".takt/cases/scenario-1-trial-{second}00000001"));
        fs::create_dir_all(&dir).expect("a case directory");
        fs::write(dir.join("w52-marker"), "kept").expect("a marker");
    }
    refs_at_head(&world.repo, &refs);
    let n = number_of(&world.trial());
    let second = second_in(&window, n);
    assert_eq!(n, second * NONCES + 2, "trial {n}");
    let marker = world.repo.join(format!(
        ".takt/cases/scenario-1-trial-{second}00000001/w52-marker"
    ));
    assert_eq!(fs::read_to_string(&marker).ok().as_deref(), Some("kept"));
}

/// The start is one after the largest number of the run's second among the trial branches,
/// local (00000010) and a remote's (00000020), whatever the number of another scenario's case
/// branch (`scenario-10-…`, 50000000), a case branch whose suffix is not all digits, or one of
/// 23 digits (another "second"): the run takes 00000021.
#[test]
fn a_remotes_trial_counts_and_other_names_do_not() {
    let world = world("what-counts");
    let window = window();
    let mut refs = Vec::new();
    for second in &window {
        refs.push(format!("refs/heads/trial/{second}00000010"));
        refs.push(format!("refs/remotes/origin/trial/{second}00000020"));
        refs.push(format!(
            "refs/heads/takt/scenario-10-trial-{second}50000000"
        ));
        refs.push(format!("refs/heads/takt/scenario-1-trial-{second}6000000x"));
        refs.push(format!(
            "refs/heads/takt/scenario-1-trial-1{second}70000000"
        ));
    }
    refs_at_head(&world.repo, &refs);
    let n = number_of(&world.trial());
    let second = second_in(&window, n);
    assert_eq!(n, second * NONCES + 21, "trial {n}");
}
