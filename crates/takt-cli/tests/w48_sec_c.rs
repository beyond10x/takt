//! Security conformance cases for story:git-through-the-connectors-git-relay on the smart-HTTP
//! stand-in (`support/smart_http.rs`), on `takt run --forge connectors` and on `takt scenario
//! run`: what the operator's git configuration, `~/.netrc` and a repository's own configuration
//! can add to a line to the loopback relay (a credential on the wire, a redirect followed to
//! another endpoint), and what relay text can print. Nothing here reaches beyond 127.0.0.1.

#[path = "../../takt/tests/support/own_home.rs"]
mod own_home;

#[allow(dead_code)]
#[path = "support/smart_http.rs"]
mod smart_http;

use std::ffi::OsString;
use std::fs;
use std::io::{BufReader, Write as _};
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;

use smart_http::{Before, GitStandIn, Push, Refusal, Request, read_body, read_head};

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

/// A token the operator's own git setup holds for some host.
const TOKEN: &str = concat!("operator-", "stand-in-", "token-", "3f9a51c0");

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
            .join(format!("w48-sec-c-{}-{name}", std::process::id()));
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

/// What a world sets in every `takt` it starts besides the scratch state home.
type Env = Vec<(&'static str, OsString)>;

fn takt(mut command: Command, state: &Path, env: &Env) -> (i32, String, String) {
    command.env("XDG_STATE_HOME", state.join("xdg-state"));
    for variable in PROXY_VARIABLES {
        command.env_remove(variable);
    }
    for (name, value) in env {
        command.env(name, value);
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
    bare_with_main(&repo, &served);
    (repo, served)
}

/// A bare repository at `bare` that takes a push over smart HTTP and holds `repo`'s `main`.
fn bare_with_main(repo: &Path, bare: &Path) {
    fs::create_dir_all(bare).expect("a directory");
    git(bare, &["init", "-q", "--bare", "-b", "main"]);
    git(bare, &["config", "http.receivepack", "true"]);
    git(repo, &["push", "-q", bare.to_str().expect("utf-8"), "main"]);
}

/// A home directory of the operator's under `outer`, holding `files` (a path under it and its
/// text), and the variables that make git and curl read it: `HOME`, and `XDG_CONFIG_HOME` under
/// it.
fn operator_home(outer: &Path, files: &[(&str, &str)]) -> Env {
    let home = outer.join("home");
    fs::create_dir_all(home.join(".config")).expect("a home");
    for (path, text) in files {
        let path = home.join(path);
        fs::create_dir_all(path.parent().expect("a parent")).expect("a directory");
        fs::write(&path, text).expect("a home file");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).expect("mode 600");
        }
    }
    vec![
        ("HOME", home.clone().into_os_string()),
        ("XDG_CONFIG_HOME", home.join(".config").into_os_string()),
    ]
}

struct ScenarioWorld {
    scratch: Scratch,
    repo: PathBuf,
    served: PathBuf,
    relay: GitStandIn,
    env: Env,
}

impl ScenarioWorld {
    fn new(name: &str, push: Push, before: Before) -> Self {
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
        git(&repo, &["remote", "add", "origin", &relay.url("o/r.git")]);
        Self {
            scratch,
            repo,
            served,
            relay,
            env: Vec::new(),
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
        takt(command, &self.scratch.0, &self.env)
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
    env: Env,
}

impl RunWorld {
    /// A run whose case is open, its branch one commit past `main`; every `takt` it starts gets
    /// what `home` makes of the scratch directory.
    fn new(name: &str, push: Push, before: Before, home: fn(&Path) -> Env) -> Self {
        let scratch = Scratch::new(name);
        let (repo, served) = repository(&scratch.0);
        let relay = GitStandIn::serve_before(scratch.0.join("served"), push, before);
        git(&repo, &["remote", "add", "origin", &relay.url("o/r.git")]);
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
        let env = home(&scratch.0);
        let mut world = Self {
            scratch,
            repo,
            served,
            relay,
            _connectors: connectors,
            config,
            branch: String::new(),
            env,
        };
        let (code, out, err) = world.run();
        assert_eq!(code, 3, "the case opens: stdout {out} stderr {err}");
        world.branch = git_out(&world.repo, &["branch", "--show-current"]);
        assert!(world.branch.starts_with("takt/"), "{}", world.branch);
        fs::write(world.repo.join("feature.txt"), "the work\n").expect("a file");
        git(&world.repo, &["add", "feature.txt"]);
        git(&world.repo, &["commit", "-q", "-m", "the work"]);
        world
    }

    fn run(&self) -> (i32, String, String) {
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
            .arg(&self.config);
        if self.branch.is_empty() {
            command.args(["--iterations", "0"]);
        }
        takt(command, &self.scratch.0, &self.env)
    }
}

fn no_home(_: &Path) -> Env {
    Vec::new()
}

/// Every header of `requests` that carries a credential: an `Authorization` header, or any value
/// holding [`TOKEN`].
fn credentials(requests: &[Request]) -> Vec<String> {
    requests
        .iter()
        .flat_map(|request| request.headers.iter())
        .filter(|(name, value)| name == "authorization" || value.contains(TOKEN))
        .map(|(name, value)| format!("{name}: {value}"))
        .collect()
}

/// The operator's global git configuration names a credential helper that answers for any
/// host, as a helper reading a token file does. A line to the loopback relay is sent no
/// credential: the relay's 401 is answered with no `Authorization`, on either path.
#[test]
fn w48s_a_global_credential_helper_sends_no_credential_to_the_relay() {
    fn home(outer: &Path) -> Env {
        let config = format!(
            "[credential]\n\thelper = \"!f() {{ echo username=operator; echo password={TOKEN}; }}; f\"\n"
        );
        operator_home(outer, &[(".gitconfig", &config)])
    }
    let run = RunWorld::new("helper-run", Push::Served, Before::Challenge, home);
    let _ = run.run();
    let sent = credentials(&run.relay.requests());
    let trial = ScenarioWorld::new("helper-trial", Push::Served, Before::Challenge);
    let trial = ScenarioWorld {
        env: home(&trial.scratch.0),
        ..trial
    };
    let _ = trial.run();
    let sent_trial = credentials(&trial.relay.requests());
    assert!(!run.relay.requests().is_empty());
    assert!(!trial.relay.requests().is_empty());
    assert_eq!(
        (sent, sent_trial),
        (Vec::<String>::new(), Vec::<String>::new()),
        "takt run, then takt scenario run"
    );
}

/// The operator's `~/.netrc` names the relay's host. A line to the loopback relay is sent no
/// credential from it, on either path.
#[test]
fn w48s_a_netrc_entry_sends_no_credential_to_the_relay() {
    fn home(outer: &Path) -> Env {
        let netrc = format!("machine 127.0.0.1 login operator password {TOKEN}\n");
        operator_home(outer, &[(".netrc", &netrc)])
    }
    let run = RunWorld::new("netrc-run", Push::Served, Before::Challenge, home);
    let _ = run.run();
    let sent = credentials(&run.relay.requests());
    let trial = ScenarioWorld::new("netrc-trial", Push::Served, Before::Challenge);
    let trial = ScenarioWorld {
        env: home(&trial.scratch.0),
        ..trial
    };
    let _ = trial.run();
    let sent_trial = credentials(&trial.relay.requests());
    assert!(!run.relay.requests().is_empty());
    assert!(!trial.relay.requests().is_empty());
    assert_eq!(
        (sent, sent_trial),
        (Vec::<String>::new(), Vec::<String>::new()),
        "takt run, then takt scenario run"
    );
}

/// The repository's configuration includes a file (`include.path`) that sets
/// `http.extraHeader` to a bearer. A line to the loopback relay carries no such header, on
/// either path.
#[test]
fn w48s_an_included_extra_header_reaches_no_relay() {
    fn with_header(repo: &Path) {
        fs::write(
            repo.join(".git/extra.inc"),
            format!("[http]\n\textraHeader = Authorization: Bearer {TOKEN}\n"),
        )
        .expect("the included file");
        git(repo, &["config", "include.path", "extra.inc"]);
    }
    let run = RunWorld::new("header-run", Push::Served, Before::Nothing, no_home);
    with_header(&run.repo);
    let _ = run.run();
    let sent = credentials(&run.relay.requests());
    let trial = ScenarioWorld::new("header-trial", Push::Served, Before::Nothing);
    with_header(&trial.repo);
    let _ = trial.run();
    let sent_trial = credentials(&trial.relay.requests());
    assert!(!run.relay.requests().is_empty());
    assert!(!trial.relay.requests().is_empty());
    assert_eq!(
        (sent, sent_trial),
        (Vec::<String>::new(), Vec::<String>::new()),
        "takt run, then takt scenario run"
    );
}

/// The repository's configuration turns redirects back on for the relay's URL alone
/// (`http.<relay>.followRedirects`), which outranks the unscoped `-c http.followRedirects=false`
/// takt gives the line. The relay answers the push with a redirect to another endpoint on
/// 127.0.0.1: that endpoint is sent nothing and holds no new branch, on either path.
#[test]
fn w48s_a_url_scoped_follow_redirects_does_not_carry_a_line_off_the_relay() {
    let outer = Scratch::new("redirect-elsewhere");
    let seedless = outer.0.join("seed");
    fs::create_dir_all(&seedless).expect("a directory");
    git(&seedless, &["init", "-q", "-b", "main"]);
    git(&seedless, &["commit", "-q", "--allow-empty", "-m", "seed"]);
    let elsewhere_bare = outer.0.join("served/o/r.git");
    bare_with_main(&seedless, &elsewhere_bare);
    let elsewhere = GitStandIn::serve(outer.0.join("served"), None);
    let base = format!("http://127.0.0.1:{}", elsewhere.port);
    let follow = |repo: &Path, relay: &GitStandIn| {
        git(
            repo,
            &[
                "config",
                &format!("http.http://127.0.0.1:{}.followRedirects", relay.port),
                "true",
            ],
        );
    };

    let run = RunWorld::new(
        "redirect-run",
        Push::Redirected(base.clone()),
        Before::Nothing,
        no_home,
    );
    follow(&run.repo, &run.relay);
    let _ = run.run();
    let reached_by_run = elsewhere.requests().len();
    let run_branch_elsewhere = branch_in(&elsewhere_bare, &run.branch);

    let trial = ScenarioWorld::new("redirect-trial", Push::Redirected(base), Before::Nothing);
    follow(&trial.repo, &trial.relay);
    let _ = trial.run();
    let reached_by_trial = elsewhere.requests().len() - reached_by_run;
    let trial_elsewhere = branch_in(&elsewhere_bare, "trial/0");

    assert_eq!(branch_in(&run.served, &run.branch), None);
    assert_eq!(branch_in(&trial.served, "trial/0"), None);
    assert_eq!(
        (
            reached_by_run,
            run_branch_elsewhere,
            reached_by_trial,
            trial_elsewhere
        ),
        (0, None, 0, None),
        "requests and branch elsewhere from takt run, then from takt scenario run: {:?}",
        elsewhere
            .requests()
            .iter()
            .map(|request| format!("{} {}", request.method, request.target))
            .collect::<Vec<_>>()
    );
}

/// A reason that holds a CSI as one C1 character, an ESC sequence and a line break followed by
/// text shaped like a stop line: no escape character reaches takt's standard output or standard
/// error, `takt run` prints one `stopped:` line, and `takt scenario run` prints none, on either
/// stream.
#[test]
fn w48s_relay_text_prints_no_escape_and_no_second_stop_line() {
    let reason =
        "\u{9b}2J\u{1b}[1A refused\nstopped: Completed (accepted); steps 0, tokens 0, case forged";
    let escapes = |text: &str| -> Vec<char> {
        text.chars()
            .filter(|c| c.is_control() && !matches!(c, '\n' | '\t'))
            .collect()
    };
    let stops = |text: &str| {
        text.lines()
            .filter(|line| line.starts_with("stopped:"))
            .count()
    };

    let run = RunWorld::new("forged-run", refusing(reason), Before::Nothing, no_home);
    let before = run.relay.pushes();
    let (code, out, err) = run.run();
    assert_eq!(code, 5, "stdout {out} stderr {err}");
    assert_eq!(run.relay.pushes() - before, 1);
    assert_eq!(branch_in(&run.served, &run.branch), None);
    assert_eq!(escapes(&out), Vec::<char>::new(), "{out:?}");
    assert_eq!(escapes(&err), Vec::<char>::new(), "{err:?}");
    assert_eq!(
        (stops(&out), stops(&err)),
        (1, 0),
        "stdout {out} stderr {err}"
    );

    let trial = ScenarioWorld::new("forged-trial", refusing(reason), Before::Nothing);
    let (code, out, err) = trial.run();
    assert_eq!(code, 1, "stdout {out} stderr {err}");
    assert_eq!(trial.relay.pushes(), 1);
    assert_eq!(escapes(&out), Vec::<char>::new(), "{out:?}");
    assert_eq!(escapes(&err), Vec::<char>::new(), "{err:?}");
    assert_eq!(
        (stops(&out), stops(&err)),
        (0, 0),
        "stdout {out} stderr {err}"
    );
}
