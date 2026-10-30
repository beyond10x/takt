//! story:git-through-the-connectors-git-relay, acceptance 3 and 4, against a smart-HTTP stand-in
//! on 127.0.0.1 (`support/smart_http.rs`) that answers a push as a Connectors git relay does.
//!
//! Acceptance 3: a push the relay refuses is answered with a report-status refusing every command
//! with the relay's reason (`ng <ref> <reason>`), which git shows as `! [remote rejected] <ref>
//! (<reason>)`. The reasons are the relay's own words: `<ref> is not under an allowed branch
//! prefix (<prefixes>)` for a ref outside `ref_prefixes`, and `commit <id> is not authored and
//! committed by the configured identity` for a commit that does not carry `commit_identity`. Both
//! `takt run`'s push of the case's branch (`--forge connectors`, `origin` the relay) and
//! `takt scenario run`'s push of `trial/<n>` end naming the ref and the reason; the push is sent
//! once, and the served repository does not get the ref.
//!
//! Acceptance 4: every git takt starts, the fetch and the push among them, is recorded by a `git`
//! first on takt's `PATH` that writes its environment and arguments and then runs the real git.
//! takt's own environment holds a token under each credential's name; no git child holds one in
//! its environment or its arguments, and the relay is sent no `Authorization` header.
//!
//! The served Connectors instance `--forge connectors` needs is a stand-in on 127.0.0.1 that
//! describes the github connector's operations and refuses every invocation. Nothing here
//! reaches beyond 127.0.0.1.

#[path = "../../takt/tests/support/own_home.rs"]
mod own_home;

#[allow(dead_code)]
#[path = "support/smart_http.rs"]
mod smart_http;

use std::fs;
use std::io::{BufReader, Write as _};
use std::net::TcpListener;
use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Arc, Mutex};

use smart_http::{GitStandIn, Push, Refusal, read_body, read_head};

/// The fixture stores of the library crate, one directory each.
const FIXTURES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../takt/tests/fixtures");

/// A gates document whose one assertion is true in a repository with a manifest.
const GATES: &str = "format: engineering-gates/1\nlanguage: canon-expr/1\ngates:\n  - id: manifest\n    \
                     text: The repository has its manifest.\n    assertions:\n      \
                     - 'fs.file.exists(\"Cargo.toml\")'\n";

/// The delivery protocol, the engineering protocols' built-in, which declares `pr.open`.
const DELIVERY_PROTOCOL: &str = "story.delivery/1";

/// The proxy variables a git child reads; every run here clears them.
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

/// The token takt's environment holds under every credential's name in these runs.
const TOKEN: &str = concat!("relay-", "stand-in-", "token-", "4e19a7c2");

/// The variables takt's own environment holds [`TOKEN`] under: names git, a forge client or a
/// relay's caller reads a credential from.
const CREDENTIAL_VARIABLES: [&str; 8] = [
    "GITHUB_TOKEN",
    "GH_TOKEN",
    "GIT_PASSWORD",
    "GITLAB_TOKEN",
    "GIT_TOKEN",
    "CONNECTORS_TOKEN",
    "RELAY_BEARER_TOKEN",
    "ANTHROPIC_API_KEY",
];

/// The relay's own reason for a ref outside `ref_prefixes`, with `agent/` the one prefix.
fn outside_prefixes(name: &str) -> String {
    format!("{name} is not under an allowed branch prefix (agent/)")
}

/// The relay's own reason for a commit that does not carry `commit_identity`.
fn not_the_identity(commit: &str) -> String {
    format!("commit {commit} is not authored and committed by the configured identity")
}

/// A relay whose `ref_prefixes` is `agent/` alone.
fn agent_prefix_only() -> Push {
    let refusal: Refusal = Arc::new(|name: &str, _: &str| {
        (!name.starts_with("refs/heads/agent/")).then(|| outside_prefixes(name))
    });
    Push::Refused(refusal)
}

/// A relay whose `commit_identity` no commit pushed here carries.
fn another_identity() -> Push {
    let refusal: Refusal = Arc::new(|_: &str, new: &str| Some(not_the_identity(new)));
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

/// The first line of `text` that holds `needle`; a failure naming `text` when none does.
fn line_with<'a>(text: &'a str, needle: &str) -> &'a str {
    text.lines()
        .find(|line| line.contains(needle))
        .unwrap_or_else(|| panic!("no line holds `{needle}`:\n{text}"))
}

/// The commit `bare` holds for `branch`; none when it holds no such branch.
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

/// A scratch directory under this crate's build directory, removed on drop.
struct Scratch(PathBuf);

impl Scratch {
    fn new(name: &str) -> Self {
        let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
            .join(format!("git-relay-refusals-{}-{name}", std::process::id()));
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

/// A served Connectors instance on 127.0.0.1: it describes the github connector's operations as
/// the instance `github`, and refuses every invocation, logging each operation.
struct ConnectorsStandIn {
    url: String,
    invoked: Arc<Mutex<Vec<String>>>,
}

impl ConnectorsStandIn {
    fn serve() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("a loopback port");
        let url = format!("http://{}", listener.local_addr().expect("an address"));
        let invoked = Arc::new(Mutex::new(Vec::new()));
        let log = Arc::clone(&invoked);
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
                    ("POST", "/v1alpha2/invoke") => {
                        log.lock()
                            .expect("the log")
                            .push(body["operation"].as_str().unwrap_or_default().to_owned());
                        (
                            404,
                            serde_json::json!({
                                "version": "v1alpha2",
                                "request_id": body["request_id"],
                                "status": "error",
                                "error": {"code": "not_found", "message": "a stand-in"},
                                "audit_ref": "audit-1",
                                "audit_status": "complete",
                            }),
                        )
                    }
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
        Self { url, invoked }
    }

    fn invoked(&self) -> Vec<String> {
        self.invoked.lock().expect("the log").clone()
    }
}

/// The descriptor of the instance `github`: every operation the github connector's map needs.
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

/// A `git` first on takt's `PATH` that writes, for each time it is started, its environment
/// (`<n>.env`) and its arguments one per line (`<n>.args`) under `log`, and then runs the real
/// git with the same arguments.
struct GitRecorder {
    bin: PathBuf,
    log: PathBuf,
}

impl GitRecorder {
    fn new(outer: &Path) -> Self {
        let real = std::env::split_paths(&std::env::var_os("PATH").unwrap_or_default())
            .map(|dir| dir.join("git"))
            .find(|path| path.is_file())
            .expect("a git on the program path");
        let bin = outer.join("recorder-bin");
        let log = outer.join("recorded");
        fs::create_dir_all(&bin).expect("a directory");
        fs::create_dir_all(&log).expect("a directory");
        let program = bin.join("git");
        fs::write(
            &program,
            format!(
                "#!/bin/sh\nf=$(mktemp '{log}/git.XXXXXXXX') || exit 125\nenv > \"$f.env\"\nfor a in \"$@\"; do printf '%s\\n' \"$a\"; done > \"$f.args\"\nexec '{real}' \"$@\"\n",
                log = log.display(),
                real = real.display()
            ),
        )
        .expect("the recorder");
        fs::set_permissions(&program, fs::Permissions::from_mode(0o755)).expect("exec bit");
        Self { bin, log }
    }

    /// takt's `PATH`: the recorder first, then this process's.
    fn path(&self) -> std::ffi::OsString {
        let mut dirs = vec![self.bin.clone()];
        dirs.extend(std::env::split_paths(
            &std::env::var_os("PATH").unwrap_or_default(),
        ));
        std::env::join_paths(dirs).expect("a PATH")
    }

    /// Each git started: its arguments and its environment.
    fn started(&self) -> Vec<(Vec<String>, String)> {
        let mut found = Vec::new();
        for entry in fs::read_dir(&self.log).expect("the log").flatten() {
            let path = entry.path();
            if path.extension().is_some_and(|ext| ext == "args") {
                let args = fs::read_to_string(&path)
                    .expect("the arguments")
                    .lines()
                    .map(str::to_owned)
                    .collect();
                let environment =
                    fs::read_to_string(path.with_extension("env")).expect("the environment");
                found.push((args, environment));
            }
        }
        found
    }

    /// Asserts that git was started for `verb` and that no git started holds [`TOKEN`] in its
    /// environment or its arguments.
    fn assert_no_token(&self, verbs: &[&str]) {
        let started = self.started();
        for verb in verbs {
            assert!(
                started
                    .iter()
                    .any(|(args, _)| args.iter().any(|arg| arg == verb)),
                "no git was started for {verb}: {:?}",
                started.iter().map(|(args, _)| args).collect::<Vec<_>>()
            );
        }
        for (args, environment) in &started {
            let holding: Vec<&str> = environment
                .lines()
                .filter(|line| line.contains(TOKEN))
                .collect();
            assert!(
                holding.is_empty(),
                "git {args:?} was started with a token in its environment: {holding:?}"
            );
            assert!(
                !args.iter().any(|arg| arg.contains(TOKEN)),
                "git was started with a token among its arguments: {args:?}"
            );
        }
    }
}

/// The output of `command` with every proxy variable cleared and the state home under `state`:
/// the exit code, stdout and stderr.
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

/// The repository at `<outer>/repo` from the fixture store with `story:ready-one` `proposed`, a
/// manifest, the gates and a commit identity, committed on `main`; and the bare repository
/// `<outer>/served/o/r.git`, which takes a push over smart HTTP and holds `main`.
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

/// One `takt scenario run` world: [`repository`] with scenario 1 declared at the seed, `origin`
/// the git stand-in answering a push as `push` says.
struct ScenarioWorld {
    scratch: Scratch,
    repo: PathBuf,
    served: PathBuf,
    relay: GitStandIn,
    seed: String,
}

impl ScenarioWorld {
    fn new(name: &str, push: Push) -> Self {
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
        let relay = GitStandIn::serve_with(scratch.0.join("served"), push);
        git(&repo, &["remote", "add", "origin", &relay.url("o/r.git")]);
        Self {
            scratch,
            repo,
            served,
            relay,
            seed,
        }
    }

    /// `takt scenario run --repo <repo> 1 --harness none --iterations 0 --test-cmd true`, with
    /// `env` set besides.
    fn run(&self, env: &[(&str, &std::ffi::OsStr)]) -> (i32, String, String) {
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
            ])
            .envs(env.iter().copied());
        takt(command, &self.scratch.0)
    }

    /// The local branches named `trial/…`.
    fn trial_branches(&self) -> String {
        git_out(
            &self.repo,
            &[
                "for-each-ref",
                "--format=%(refname:short)",
                "refs/heads/trial/",
            ],
        )
    }
}

/// Acceptance 3 on `takt scenario run`: the relay refuses `refs/heads/trial/0` as outside its
/// `ref_prefixes`; the run fails naming `trial/0` and the relay's reason, the push was sent
/// once, and neither the relay's upstream nor the checkout holds `trial/0`.
#[test]
fn a_trial_ref_outside_the_relay_prefixes_ends_the_scenario_run_naming_ref_and_reason() {
    let world = ScenarioWorld::new("trial-prefix", agent_prefix_only());
    let (code, out, err) = world.run(&[]);
    assert_eq!(code, 1, "stdout {out} stderr {err}");
    let refused = line_with(&err, "trial/0 was not created");
    assert!(refused.contains("[remote rejected]"), "{refused}");
    assert!(
        refused.contains(&outside_prefixes("refs/heads/trial/0")),
        "the relay's reason is named: {refused}"
    );
    assert_eq!(world.relay.pushes(), 1, "{:?}", world.relay.requests());
    assert_eq!(branch_in(&world.served, "trial/0"), None);
    assert_eq!(world.trial_branches(), "");
}

/// Acceptance 3 on `takt scenario run`: the relay refuses the seed, whose author and committer
/// are not its `commit_identity`; the run fails naming `trial/0` and the relay's reason, which
/// names the commit, the push was sent once, and the upstream does not hold `trial/0`.
#[test]
fn a_trial_commit_of_another_identity_ends_the_scenario_run_naming_ref_and_reason() {
    let world = ScenarioWorld::new("trial-identity", another_identity());
    let (code, out, err) = world.run(&[]);
    assert_eq!(code, 1, "stdout {out} stderr {err}");
    let refused = line_with(&err, "trial/0 was not created");
    assert!(refused.contains("[remote rejected]"), "{refused}");
    assert!(
        refused.contains(&not_the_identity(&world.seed)),
        "the relay's reason is named: {refused}"
    );
    assert_eq!(world.relay.pushes(), 1, "{:?}", world.relay.requests());
    assert_eq!(branch_in(&world.served, "trial/0"), None);
    assert_eq!(world.trial_branches(), "");
}

/// One `takt run --forge connectors` world: [`repository`], `origin` the git stand-in answering
/// a push as `push` says, the served Connectors stand-in, and a run file naming it with
/// `scan: none`. A first run (`--iterations 0`) has opened the case on `story:ready-one`, and
/// the work is committed on its branch by `t <t@localhost>`.
struct RunWorld {
    scratch: Scratch,
    repo: PathBuf,
    served: PathBuf,
    relay: GitStandIn,
    connectors: ConnectorsStandIn,
    config: PathBuf,
    branch: String,
    head: String,
}

impl RunWorld {
    fn new(name: &str, push: Push) -> Self {
        let scratch = Scratch::new(name);
        let (repo, served) = repository(&scratch.0);
        let relay = GitStandIn::serve_with(scratch.0.join("served"), push);
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
        let mut world = Self {
            scratch,
            repo,
            served,
            relay,
            connectors,
            config,
            branch: String::new(),
            head: String::new(),
        };
        let (code, out, err) = world.run(&["--iterations", "0"], &[]);
        assert_eq!(code, 3, "the case opens: stdout {out} stderr {err}");
        world.branch = git_out(&world.repo, &["branch", "--show-current"]);
        assert!(
            world.branch.starts_with("takt/"),
            "the tree is on the case's branch: {}",
            world.branch
        );
        fs::write(world.repo.join("feature.txt"), "the work\n").expect("a file");
        git(&world.repo, &["add", "feature.txt"]);
        git(&world.repo, &["commit", "-q", "-m", "the work"]);
        world.head = git_out(&world.repo, &["rev-parse", "HEAD"]);
        world
    }

    /// `takt run --repo <repo> --harness none --test-cmd true --protocol story.delivery/1
    /// --config <run file>` and `extra`, with `env` set besides.
    fn run(&self, extra: &[&str], env: &[(&str, &std::ffi::OsStr)]) -> (i32, String, String) {
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
            .args(extra)
            .envs(env.iter().copied());
        takt(command, &self.scratch.0)
    }
}

/// Acceptance 3 on `takt run`: the relay refuses the case's branch, `refs/heads/takt/<id>`, as
/// outside its `ref_prefixes`; the run ends naming the branch and the relay's reason, the push
/// was sent once, no pull request was asked for, and the upstream does not hold the branch.
#[test]
fn a_case_branch_outside_the_relay_prefixes_ends_the_run_naming_ref_and_reason() {
    let world = RunWorld::new("case-prefix", agent_prefix_only());
    let before = world.relay.pushes();
    let (code, out, err) = world.run(&[], &[]);
    assert_eq!(code, 5, "stdout {out} stderr {err}");
    let stopped = line_with(&out, "stopped: error:");
    assert!(
        stopped.contains(&format!(
            "[remote rejected] {} -> {}",
            world.head, world.branch
        )),
        "{stopped}"
    );
    assert!(
        stopped.contains(&outside_prefixes(&format!("refs/heads/{}", world.branch))),
        "the relay's reason is named: {stopped}"
    );
    assert_eq!(
        world.relay.pushes() - before,
        1,
        "{:?}",
        world.relay.requests()
    );
    assert_eq!(branch_in(&world.served, &world.branch), None);
    assert!(
        !world
            .connectors
            .invoked()
            .iter()
            .any(|operation| operation == "pull_request.create"),
        "{:?}",
        world.connectors.invoked()
    );
}

/// Acceptance 3 on `takt run`: the relay refuses the case's work, committed by `t <t@localhost>`
/// and not by its `commit_identity`; the run ends naming the branch and the relay's reason,
/// which names the commit, the push was sent once, and the upstream does not hold the branch.
#[test]
fn a_case_commit_of_another_identity_ends_the_run_naming_ref_and_reason() {
    let world = RunWorld::new("case-identity", another_identity());
    let before = world.relay.pushes();
    let (code, out, err) = world.run(&[], &[]);
    assert_eq!(code, 5, "stdout {out} stderr {err}");
    let stopped = line_with(&out, "stopped: error:");
    assert!(
        stopped.contains(&format!(
            "[remote rejected] {} -> {}",
            world.head, world.branch
        )),
        "{stopped}"
    );
    assert!(
        stopped.contains(&not_the_identity(&world.head)),
        "the relay's reason is named: {stopped}"
    );
    assert_eq!(
        world.relay.pushes() - before,
        1,
        "{:?}",
        world.relay.requests()
    );
    assert_eq!(branch_in(&world.served, &world.branch), None);
}

/// [`CREDENTIAL_VARIABLES`], each holding [`TOKEN`], and `PATH` with the recorder first.
fn credentials_and(recorder: &GitRecorder) -> Vec<(&'static str, std::ffi::OsString)> {
    let mut env: Vec<(&str, std::ffi::OsString)> = CREDENTIAL_VARIABLES
        .iter()
        .map(|name| (*name, TOKEN.into()))
        .collect();
    env.push(("PATH", recorder.path()));
    env
}

/// Acceptance 4 on `takt scenario run`: with a token under every credential's name in takt's
/// environment, the run fetches and pushes `trial/0` through the relay, and no git it starts
/// holds the token in its environment or its arguments; the relay is sent no `Authorization`.
#[test]
fn no_git_child_of_a_scenario_run_holds_a_token() {
    let world = ScenarioWorld::new("trial-no-token", Push::Served);
    let recorder = GitRecorder::new(&world.scratch.0);
    let env = credentials_and(&recorder);
    let env: Vec<(&str, &std::ffi::OsStr)> = env
        .iter()
        .map(|(name, value)| (*name, value.as_os_str()))
        .collect();
    let (code, out, err) = world.run(&env);
    assert_eq!(code, 3, "stdout {out} stderr {err}");
    assert_eq!(
        branch_in(&world.served, "trial/0").as_deref(),
        Some(world.seed.as_str()),
        "stdout {out} stderr {err}"
    );
    recorder.assert_no_token(&["fetch", "push"]);
    let requests = world.relay.requests();
    assert!(
        requests
            .iter()
            .all(|request| request.header("authorization").is_none()),
        "{requests:?}"
    );
}

/// Acceptance 4 on `takt run`: with a token under every credential's name in takt's
/// environment, the run pushes the case's branch through the relay to its upstream, and no git
/// it starts holds the token in its environment or its arguments; the relay is sent no
/// `Authorization`.
#[test]
fn no_git_child_of_a_case_push_holds_a_token() {
    let world = RunWorld::new("case-no-token", Push::Served);
    let recorder = GitRecorder::new(&world.scratch.0);
    let env = credentials_and(&recorder);
    let env: Vec<(&str, &std::ffi::OsStr)> = env
        .iter()
        .map(|(name, value)| (*name, value.as_os_str()))
        .collect();
    let (_, out, err) = world.run(&[], &env);
    assert_eq!(
        branch_in(&world.served, &world.branch).as_deref(),
        Some(world.head.as_str()),
        "the case's branch reached the upstream: stdout {out} stderr {err}"
    );
    recorder.assert_no_token(&["push"]);
    let requests = world.relay.requests();
    assert!(
        requests
            .iter()
            .all(|request| request.header("authorization").is_none()),
        "{requests:?}"
    );
}
