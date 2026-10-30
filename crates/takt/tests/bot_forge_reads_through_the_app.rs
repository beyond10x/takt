//! story:bot-forge-reads-through-the-app: the bot forge reads a pull request and the checks on
//! its head through `b10x-gates api --method GET`, under the App it writes with, and runs no
//! `gh`. The forge's lines run as processes, `PATH` holding nothing but a directory with a
//! `b10x-gates` double, so a `gh` line would fail to start. The double logs each line it is
//! given and answers the three `GET` paths (the pull request, the check runs and the combined
//! status of its head) from the fixture files under `fixtures/bot-reads`, one set per case.

use std::fs;
use std::path::{Path, PathBuf};

use takt::forge::{
    BotRoute, Checks, Exit, Forge as _, Head, Process, PullRequestState, PullRequestView, Runner,
};

/// The head the pull request fixtures name.
const HEAD: &str = "0123456789abcdef0123456789abcdef01234567";

/// A directory of this case's own under the crate's build directory, removed on drop.
struct Scratch(PathBuf);

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// Runs every line as a process whose `PATH` is `bin` alone.
struct OnPath(PathBuf);

impl Runner for OnPath {
    fn run(&self, tree: &Path, argv: &[String]) -> anyhow::Result<Exit> {
        self.run_env(tree, argv, &[])
    }

    fn run_env(&self, tree: &Path, argv: &[String], env: &[(&str, &str)]) -> anyhow::Result<Exit> {
        let path = self.0.to_str().expect("utf-8");
        let mut env = env.to_vec();
        env.push(("PATH", path));
        Process::default().run_with(tree, argv, &env)
    }
}

/// The double, in `<scratch>/bin` and alone on the forge's `PATH`, answering from the fixture
/// set `set` (its `pull.json` when it has one, else the open pull request's).
fn double(name: &str, set: &str) -> (Scratch, BotRoute) {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("bot-reads-{}-{name}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    let bin = dir.join("bin");
    fs::create_dir_all(&bin).expect("the double's directory");
    let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/bot-reads");
    let script = format!(
        r#"#!/bin/sh
log='{log}'
fixtures='{fixtures}'
set='{set}'
printf '%s\n' "$*" >> "$log"
[ "$1" = api ] || {{ echo "b10x-gates double: $*" >&2; exit 2; }}
shift
method=''; path=''; output=''
while [ $# -gt 0 ]; do
  case "$1" in
    --method) method="$2"; shift ;;
    --path) path="$2"; shift ;;
    --output) output="$2"; shift ;;
    --input) echo "b10x-gates double: a read takes no --input" >&2; exit 2 ;;
  esac
  shift
done
[ "$method" = GET ] || {{ echo "b10x-gates double: $method is no read" >&2; exit 2; }}
case "$path" in
  /repos/owner/repo/pulls/7)
    file="$fixtures/$set/pull.json"; [ -f "$file" ] || file="$fixtures/pull.json" ;;
  /repos/owner/repo/commits/{HEAD}/check-runs|/repos/owner/repo/commits/{HEAD}/check-runs\?*)
    file="$fixtures/$set/check-runs.json" ;;
  /repos/owner/repo/commits/{HEAD}/status|/repos/owner/repo/commits/{HEAD}/status\?*)
    file="$fixtures/$set/status.json" ;;
  *) echo "b10x-gates double: no fixture for $path" >&2; exit 2 ;;
esac
while IFS= read -r line || [ -n "$line" ]; do printf '%s\n' "$line"; done < "$file" > "$output" || exit 1
echo "bot API operation completed; response retained locally"
"#,
        log = dir.join("calls.log").display(),
        fixtures = fixtures.display(),
    );
    let program = bin.join("b10x-gates");
    fs::write(&program, script).expect("the double");
    {
        use std::os::unix::fs::PermissionsExt as _;
        fs::set_permissions(&program, fs::Permissions::from_mode(0o755)).expect("executable");
    }
    let forge = BotRoute::new(
        "owner/repo",
        Some(PathBuf::from("/policies/gates.json")),
        Box::new(OnPath(bin)),
    );
    (Scratch(dir), forge)
}

/// The lines the double was given, one per call.
fn calls(scratch: &Scratch) -> Vec<String> {
    fs::read_to_string(scratch.0.join("calls.log"))
        .unwrap_or_default()
        .lines()
        .map(str::to_owned)
        .collect()
}

/// Every call was a `GET` through `b10x-gates api` with no `--input`, and `gh` could not have
/// run: it is on no directory of the forge's `PATH`.
fn read_through_the_app_alone(scratch: &Scratch, expected: usize) {
    let calls = calls(scratch);
    assert_eq!(calls.len(), expected, "{calls:#?}");
    for call in &calls {
        assert!(
            call.starts_with("api --method GET --path /repos/owner/repo/"),
            "{call}"
        );
        assert!(!call.contains("--input"), "{call}");
        assert!(
            call.contains("--repository owner/repo --policy /policies/gates.json"),
            "{call}"
        );
    }
    let bin = scratch.0.join("bin");
    let entries: Vec<String> = fs::read_dir(&bin)
        .expect("the double's directory")
        .map(|entry| {
            entry
                .expect("an entry")
                .file_name()
                .to_string_lossy()
                .into_owned()
        })
        .collect();
    assert_eq!(entries, ["b10x-gates"], "no gh on the forge's PATH");
}

/// Acceptance: the pull request reads as `Open` with its head branch, its head's commit and its
/// base, through one `GET` of `/repos/owner/repo/pulls/7`.
#[test]
fn the_bot_forge_reads_an_open_pull_request_through_the_app() {
    let (scratch, forge) = double("open", "pass");
    let view = forge
        .pull_request(&scratch.0, 7)
        .unwrap_or_else(|error| panic!("reads: {error:#}\n{:#?}", calls(&scratch)));
    assert_eq!(
        view,
        PullRequestView {
            state: PullRequestState::Open,
            head: Head {
                branch: "takt/c1".to_owned(),
                sha: HEAD.to_owned(),
            },
            base: "main".to_owned(),
        }
    );
    read_through_the_app_alone(&scratch, 1);
    assert_eq!(
        calls(&scratch)[0].split(' ').nth(4),
        Some("/repos/owner/repo/pulls/7")
    );
}

/// A merged pull request reads as `Merged`, a closed one as `Closed`.
#[test]
fn the_bot_forge_reads_merged_and_closed_pull_requests() {
    for (set, state) in [
        ("merged", PullRequestState::Merged),
        ("closed", PullRequestState::Closed),
    ] {
        let (scratch, forge) = double(set, set);
        let view = forge
            .pull_request(&scratch.0, 7)
            .unwrap_or_else(|error| panic!("{set}: {error:#}"));
        assert_eq!(view.state, state, "{set}");
        read_through_the_app_alone(&scratch, 1);
    }
}

/// Acceptance: the checks on the pull request's head read as `Pass`, `Pending` and `Fail` for
/// the three fixture sets, each through `GET`s of the pull request, the check runs of its head
/// and its head's combined status; no check at all is pending, and a failing status fails
/// whatever the check runs say.
#[test]
fn the_bot_forge_reads_checks_through_the_app() {
    for (set, expected) in [
        ("pass", Checks::Pass),
        ("pending", Checks::Pending),
        ("fail", Checks::Fail),
        ("none", Checks::Pending),
        ("status-fails", Checks::Fail),
    ] {
        let (scratch, forge) = double(&format!("checks-{set}"), set);
        let checks = forge
            .checks(&scratch.0, 7)
            .unwrap_or_else(|error| panic!("{set}: {error:#}\n{:#?}", calls(&scratch)));
        assert_eq!(checks, expected, "{set}");
        read_through_the_app_alone(&scratch, 3);
        let paths: Vec<String> = calls(&scratch)
            .iter()
            .map(|call| call.split(' ').nth(4).expect("a path").to_owned())
            .collect();
        assert_eq!(paths[0], "/repos/owner/repo/pulls/7", "{set}");
        assert!(
            paths[1..]
                .iter()
                .any(|path| path
                    .starts_with(&format!("/repos/owner/repo/commits/{HEAD}/check-runs"))),
            "{set}: {paths:?}"
        );
        assert!(
            paths[1..]
                .iter()
                .any(|path| path.starts_with(&format!("/repos/owner/repo/commits/{HEAD}/status"))),
            "{set}: {paths:?}"
        );
    }
}

/// A read the double cannot answer is an error naming the line, never a reading.
#[test]
fn a_read_that_fails_is_an_error_not_a_reading() {
    let (scratch, forge) = double("missing", "no-such-set");
    let error = format!(
        "{:#}",
        forge
            .checks(&scratch.0, 7)
            .expect_err("no check-runs fixture: no reading")
    );
    assert!(error.contains("b10x-gates api --method GET"), "{error}");
}
