//! Adversarial cases against story:git-through-the-connectors-git-relay, acceptance 3 and 4, on
//! the smart-HTTP stand-in (`support/smart_http.rs`): reasons with odd bytes, a long reason,
//! HTTP statuses instead of a refusal report, a refused fetch, and an `origin` whose URL holds a
//! credential in its userinfo, on `takt run --forge connectors` and on `takt scenario run`.
//! Nothing here reaches beyond 127.0.0.1.

#[path = "../../takt/tests/support/own_home.rs"]
mod own_home;

#[allow(dead_code)]
#[path = "support/smart_http.rs"]
mod smart_http;

use std::fs;
use std::io::{BufReader, Write as _};
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;

use smart_http::{Before, GitStandIn, Push, Refusal, read_body, read_head};

const FIXTURES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../takt/tests/fixtures");

const GATES: &str = "format: engineering-gates/1\nlanguage: canon-expr/1\ngates:\n  - id: manifest\n    \
                     text: The repository has its manifest.\n    assertions:\n      \
                     - 'fs.file.exists(\"Cargo.toml\")'\n";

const DELIVERY_PROTOCOL: &str = "story.delivery/1";

const PROXY_VARIABLES: [&str; 8] = [
    "http_proxy",
    "HTTP_PROXY",
    "https_proxy",
    "HTTPS_PROXY",
    "all_proxy",
    "ALL_PROXY",
    "no_proxy",
    "NO_PROXY",
];

/// A token an operator put in `origin`'s userinfo.
const TOKEN: &str = concat!("userinfo-", "stand-in-", "token-", "77c0e1d5");

/// A relay that refuses every command with `reason`.
fn refusing(reason: &str) -> Push {
    let reason = reason.to_owned();
    let refusal: Refusal = Arc::new(move |_: &str, _: &str| Some(reason.clone()));
    Push::Refused(refusal)
}

fn git(tree: &Path, args: &[&str]) {
    let output = Command::new("git")
        .args(["-c", "user.name=t", "-c", "user.email=t@localhost"])
        .args(["-c", "commit.gpgsign=false"])
        .args(args)
        .current_dir(tree)
        .output()
        .expect("git runs");
    assert!(
        output.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

fn git_out(tree: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .args(args)
        .current_dir(tree)
        .output()
        .expect("git runs");
    assert!(output.status.success(), "git {args:?}");
    String::from_utf8_lossy(&output.stdout).trim().to_owned()
}

fn line_with<'a>(text: &'a str, needle: &str) -> &'a str {
    text.lines()
        .find(|line| line.contains(needle))
        .unwrap_or_else(|| panic!("no line holds `{needle}`:\n{text}"))
}

fn branch_in(bare: &Path, branch: &str) -> Option<String> {
    let output = Command::new("git")
        .args([
            "rev-parse",
            "--verify",
            "--quiet",
            &format!("refs/heads/{branch}"),
        ])
        .current_dir(bare)
        .output()
        .expect("git runs");
    output
        .status
        .success()
        .then(|| String::from_utf8_lossy(&output.stdout).trim().to_owned())
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

struct Scratch(PathBuf);

impl Scratch {
    fn new(name: &str) -> Self {
        let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
            .join(format!("w48-adv-c-{}-{name}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("a scratch directory");
        Self(dir)
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// A served Connectors instance on 127.0.0.1 that describes the github connector's operations
/// and refuses every invocation.
struct ConnectorsStandIn {
    url: String,
}

impl ConnectorsStandIn {
    fn serve() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("a loopback port");
        let url = format!("http://{}", listener.local_addr().expect("an address"));
        std::thread::spawn(move || {
            for stream in listener.incoming().flatten() {
                let Ok(clone) = stream.try_clone() else {
                    continue;
                };
                let mut reader = BufReader::new(clone);
                let Some(request) = read_head(&mut reader) else {
                    continue;
                };
                let body = read_body(&mut reader, &request).unwrap_or_default();
                let body: serde_json::Value =
                    serde_json::from_slice(&body).unwrap_or(serde_json::Value::Null);
                let (status, answer) = match (request.method.as_str(), request.target.as_str()) {
                    ("GET", "/v1/describe") => (200, describe()),
                    ("POST", "/v1alpha2/invoke") => (
                        404,
                        serde_json::json!({
                            "version": "v1alpha2",
                            "request_id": body["request_id"],
                            "status": "error",
                            "error": {"code": "not_found", "message": "a stand-in"},
                            "audit_ref": "audit-1",
                            "audit_status": "complete",
                        }),
                    ),
                    _ => (404, serde_json::json!({"code": "not_found"})),
                };
                let text = answer.to_string();
                let mut out = stream;
                let _ = write!(
                    out,
                    "HTTP/1.1 {status} Answer\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{text}",
                    text.len()
                );
            }
        });
        Self { url }
    }
}

fn describe() -> serde_json::Value {
    let operations: Vec<serde_json::Value> = [
        "branch.get",
        "pull_request.create",
        "pull_request.get",
        "pull_request.merge",
        "check_runs.list",
        "commit.status",
    ]
    .iter()
    .map(|id| {
        serde_json::json!({
            "id": id,
            "description": "",
            "contract": "operations/v1alpha1",
            "profile": "github.app",
            "input_schema": {},
            "output_schema": {},
        })
    })
    .collect();
    serde_json::json!({
        "version": "v1alpha1",
        "instance": "github",
        "adapter": "catalog",
        "revision": "descriptor-r1",
        "operations": operations,
        "configuration_schema": {"type": "object"},
    })
}

fn takt(mut command: Command, state: &Path) -> (i32, String, String) {
    command.env("XDG_STATE_HOME", state.join("xdg-state"));
    for variable in PROXY_VARIABLES {
        command.env_remove(variable);
    }
    let output = command.output().expect("takt runs");
    (
        output.status.code().expect("an exit code"),
        String::from_utf8_lossy(&output.stdout).into_owned(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
    )
}

fn repository(outer: &Path) -> (PathBuf, PathBuf) {
    let repo = outer.join("repo");
    copy_dir(&Path::new(FIXTURES).join("one-ready"), &repo);
    fs::write(repo.join("Cargo.toml"), "[package]\nname = \"scratch\"\n").expect("a file");
    fs::write(repo.join(".engineering/gates.yaml"), GATES).expect("a file");
    fs::write(
        repo.join(".engineering/commit-identity.conf"),
        "[identity]\n\tname = Test Bot\n\temail = bot@example.invalid\n",
    )
    .expect("the identity is written");
    git(&repo, &["init", "-q", "-b", "main"]);
    git(&repo, &["add", "-A"]);
    git(&repo, &["commit", "-q", "-m", "seed"]);
    let served = outer.join("served/o/r.git");
    fs::create_dir_all(&served).expect("a directory");
    git(&served, &["init", "-q", "--bare", "-b", "main"]);
    git(&served, &["config", "http.receivepack", "true"]);
    git(
        &repo,
        &["push", "-q", served.to_str().expect("utf-8"), "main"],
    );
    (repo, served)
}

/// `origin`'s URL to `relay`: with `TOKEN` in its userinfo when `userinfo`.
fn origin_url(relay: &GitStandIn, userinfo: bool) -> String {
    if userinfo {
        format!("http://x-access-token:{TOKEN}@127.0.0.1:{}/o/r.git", relay.port)
    } else {
        relay.url("o/r.git")
    }
}

struct ScenarioWorld {
    scratch: Scratch,
    repo: PathBuf,
    served: PathBuf,
    relay: GitStandIn,
}

impl ScenarioWorld {
    fn new(name: &str, push: Push, before: Before, userinfo: bool) -> Self {
        let scratch = Scratch::new(name);
        let (repo, served) = repository(&scratch.0);
        let seed = git_out(&repo, &["rev-parse", "HEAD"]);
        fs::write(
            repo.join(".engineering/scenarios.conf"),
            format!("[scenario \"1\"]\n\tstory = story:ready-one\n\tseed = {seed}\n"),
        )
        .expect("the declaration is written");
        git(&repo, &["add", "-A"]);
        git(&repo, &["commit", "-q", "-m", "declare the scenario"]);
        let relay = GitStandIn::serve_before(scratch.0.join("served"), push, before);
        git(
            &repo,
            &["remote", "add", "origin", &origin_url(&relay, userinfo)],
        );
        Self {
            scratch,
            repo,
            served,
            relay,
        }
    }

    fn run(&self) -> (i32, String, String) {
        let mut command = Command::new(env!("CARGO_BIN_EXE_takt"));
        command
            .args(["scenario", "run", "--repo"])
            .arg(&self.repo)
            .args([
                "1",
                "--harness",
                "none",
                "--iterations",
                "0",
                "--test-cmd",
                "true",
            ]);
        takt(command, &self.scratch.0)
    }
}

struct RunWorld {
    scratch: Scratch,
    repo: PathBuf,
    served: PathBuf,
    relay: GitStandIn,
    _connectors: ConnectorsStandIn,
    config: PathBuf,
    branch: String,
    head: String,
}

impl RunWorld {
    fn new(name: &str, push: Push, before: Before, userinfo: bool) -> Self {
        let scratch = Scratch::new(name);
        let (repo, served) = repository(&scratch.0);
        let relay = GitStandIn::serve_before(scratch.0.join("served"), push, before);
        git(
            &repo,
            &["remote", "add", "origin", &origin_url(&relay, userinfo)],
        );
        let connectors = ConnectorsStandIn::serve();
        let config = scratch.0.join("run.yaml");
        fs::write(
            &config,
            format!(
                "forge: connectors\nforge_instance: github\nforge_endpoint: {}\nscan: none\n",
                connectors.url
            ),
        )
        .expect("the run file");
        let mut world = Self {
            scratch,
            repo,
            served,
            relay,
            _connectors: connectors,
            config,
            branch: String::new(),
            head: String::new(),
        };
        let (code, out, err) = world.run(&["--iterations", "0"]);
        assert_eq!(code, 3, "the case opens: stdout {out} stderr {err}");
        world.branch = git_out(&world.repo, &["branch", "--show-current"]);
        assert!(world.branch.starts_with("takt/"), "{}", world.branch);
        fs::write(world.repo.join("feature.txt"), "the work\n").expect("a file");
        git(&world.repo, &["add", "feature.txt"]);
        git(&world.repo, &["commit", "-q", "-m", "the work"]);
        world.head = git_out(&world.repo, &["rev-parse", "HEAD"]);
        world
    }

    fn run(&self, extra: &[&str]) -> (i32, String, String) {
        let mut command = Command::new(env!("CARGO_BIN_EXE_takt"));
        command
            .args(["run", "--repo"])
            .arg(&self.repo)
            .args([
                "--harness",
                "none",
                "--test-cmd",
                "true",
                "--protocol",
                DELIVERY_PROTOCOL,
                "--config",
            ])
            .arg(&self.config)
            .args(extra);
        takt(command, &self.scratch.0)
    }

    /// The pushes the relay was sent by the run, its stop line, after the case was opened.
    fn refused_run(&self) -> (usize, String) {
        let before = self.relay.pushes();
        let (code, out, err) = self.run(&[]);
        assert_eq!(code, 5, "stdout {out} stderr {err}");
        let stopped = line_with(&out, "stopped: error:").to_owned();
        assert_eq!(branch_in(&self.served, &self.branch), None);
        (self.relay.pushes() - before, stopped)
    }
}

fn no_control(line: &str) -> Vec<char> {
    line.chars().filter(|c| c.is_control() && *c != '\t').collect()
}

/// A reason with terminal escapes (a window title, a screen clear): the stop line of
/// `takt run` carries none of the relay's control bytes to the operator's terminal.
#[test]
fn w48c_escape_bytes_in_a_reason_do_not_reach_the_run_stop_line() {
    let reason = "\u{1b}]0;relay-title\u{7}\u{1b}[2Jrefused by policy";
    let world = RunWorld::new("esc-run", refusing(reason), Before::Nothing, false);
    let (pushes, stopped) = world.refused_run();
    assert_eq!(pushes, 1);
    assert!(stopped.contains("refused by policy"), "{stopped:?}");
    assert_eq!(no_control(&stopped), Vec::<char>::new(), "{stopped:?}");
}

/// The same reason on `takt scenario run`: the trial's refusal line carries no control byte.
#[test]
fn w48c_escape_bytes_in_a_reason_do_not_reach_the_trial_refusal_line() {
    let reason = "\u{1b}]0;relay-title\u{7}\u{1b}[2Jrefused by policy";
    let world = ScenarioWorld::new("esc-trial", refusing(reason), Before::Nothing, false);
    let (code, out, err) = world.run();
    assert_eq!(code, 1, "stdout {out} stderr {err}");
    let refused = line_with(&err, "trial/0 was not created");
    assert!(refused.contains("refused by policy"), "{refused:?}");
    assert_eq!(no_control(refused), Vec::<char>::new(), "{refused:?}");
    assert_eq!(world.relay.pushes(), 1);
}

/// A reason with a tab in it: the trial's refusal line names the reason whole.
#[test]
fn w48c_a_reason_with_a_tab_is_named_whole_in_the_trial_refusal_line() {
    let world = ScenarioWorld::new(
        "tab-trial",
        refusing("refs/heads/trial/0 is refused:\tthe prefix is agent/"),
        Before::Nothing,
        false,
    );
    let (code, out, err) = world.run();
    assert_eq!(code, 1, "stdout {out} stderr {err}");
    let refused = line_with(&err, "trial/0 was not created");
    assert!(refused.contains("the prefix is agent/"), "{refused}");
    assert_eq!(world.relay.pushes(), 1);
    assert_eq!(branch_in(&world.served, "trial/0"), None);
}

/// A reason of two lines: the trial's refusal line names both.
#[test]
fn w48c_a_reason_of_two_lines_is_named_whole_in_the_trial_refusal_line() {
    let world = ScenarioWorld::new(
        "newline-trial",
        refusing("refs/heads/trial/0 is refused\nthe prefix is agent/"),
        Before::Nothing,
        false,
    );
    let (code, out, err) = world.run();
    assert_eq!(code, 1, "stdout {out} stderr {err}");
    let refused = line_with(&err, "trial/0 was not created");
    assert!(refused.contains("the prefix is agent/"), "{refused}");
}

/// A reason of two lines on `takt run`: the stop line names both.
#[test]
fn w48c_a_reason_of_two_lines_is_named_whole_in_the_run_stop_line() {
    let world = RunWorld::new(
        "newline-run",
        refusing("the ref is refused\nthe prefix is agent/"),
        Before::Nothing,
        false,
    );
    let (pushes, stopped) = world.refused_run();
    assert_eq!(pushes, 1);
    assert!(stopped.contains("the ref is refused"), "{stopped}");
    assert!(stopped.contains("the prefix is agent/"), "{stopped}");
}

/// A reason of 6000 bytes with parentheses and an arrow in it: both stop lines name it whole,
/// and the full ref.
#[test]
fn w48c_a_long_reason_is_named_whole_on_both_paths() {
    let reason = format!("({}) -> refs/heads/main (x){}", "a".repeat(3000), "b".repeat(3000));
    let run = RunWorld::new("long-run", refusing(&reason), Before::Nothing, false);
    let (pushes, stopped) = run.refused_run();
    assert_eq!(pushes, 1);
    assert!(stopped.contains(&reason), "{}", stopped.len());
    assert!(
        stopped.contains(&format!("[remote rejected] {} -> {}", run.head, run.branch)),
        "{}",
        &stopped[..stopped.len().min(400)]
    );
    let trial = ScenarioWorld::new("long-trial", refusing(&reason), Before::Nothing, false);
    let (code, _, err) = trial.run();
    assert_eq!(code, 1);
    let refused = line_with(&err, "trial/0 was not created");
    assert!(refused.contains(&reason), "{}", refused.len());
    assert_eq!(trial.relay.pushes(), 1);
}

/// Pins today's answer to an HTTP status in place of a report: 401, 403 and 413 on the push
/// each end `takt run` with exit 5 after one push, the upstream without the branch, and a stop
/// line that names the failing push line.
#[test]
fn w48c_an_http_status_on_the_push_ends_the_run_after_one_push() {
    for status in [401_u16, 403, 413] {
        let world = RunWorld::new(
            &format!("status-{status}"),
            Push::Status(status),
            Before::Nothing,
            false,
        );
        let (pushes, stopped) = world.refused_run();
        assert_eq!(pushes, 1, "{status}: {stopped}");
        assert!(stopped.contains(" push "), "{status}: {stopped}");
        assert!(stopped.contains(&world.branch), "{status}: {stopped}");
    }
}

/// A fetch the relay refuses ends `takt scenario run` before any push, and no trial branch is
/// left behind.
#[test]
fn w48c_a_refused_fetch_ends_the_scenario_run_before_any_push() {
    let world = ScenarioWorld::new("fetch-403", Push::Served, Before::RefuseFetch(403), false);
    let (code, out, err) = world.run();
    assert_ne!(code, 0, "stdout {out} stderr {err}");
    assert_ne!(code, 3, "stdout {out} stderr {err}");
    assert_eq!(world.relay.pushes(), 0, "{:?}", world.relay.requests());
    assert_eq!(branch_in(&world.served, "trial/0"), None);
    assert_eq!(
        git_out(
            &world.repo,
            &["for-each-ref", "--format=%(refname:short)", "refs/heads/trial/"]
        ),
        ""
    );
}

/// `origin` holds a token in its userinfo and the relay challenges an unauthenticated request:
/// `takt run`'s lines go to the relay without the userinfo, so the relay is sent no credential.
#[test]
fn w48c_a_run_sends_no_userinfo_credential_to_a_challenging_relay() {
    let world = RunWorld::new("userinfo-run", Push::Served, Before::Challenge, true);
    let _ = world.run(&[]);
    let requests = world.relay.requests();
    assert!(!requests.is_empty());
    assert!(
        requests
            .iter()
            .all(|request| request.header("authorization").is_none()),
        "{requests:?}"
    );
}

/// The same on `takt scenario run`: its fetch and push name `origin`, and the relay is sent no
/// credential either.
#[test]
fn w48c_a_scenario_run_sends_no_userinfo_credential_to_a_challenging_relay() {
    let world = ScenarioWorld::new("userinfo-trial", Push::Served, Before::Challenge, true);
    let _ = world.run();
    let requests = world.relay.requests();
    assert!(!requests.is_empty());
    let sent: Vec<&str> = requests
        .iter()
        .filter_map(|request| request.header("authorization"))
        .collect();
    assert_eq!(sent, Vec::<&str>::new(), "{requests:?}");
}
