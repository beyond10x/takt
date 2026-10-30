//! Adversarial cases for story:git-and-model-through-loopback-relays, acceptance 2 to 4, against
//! the Loom 0.52.0 pin: every host form takt calls loopback starts a keyless session (takt's and
//! the session executor's loopback predicates agree), every form that only looks like one is
//! refused before a case opens, a credential flag on a loopback endpoint still reaches the
//! harness as a file and never as a value, and, with the real b10x-harness, a loopback endpoint
//! that redirects elsewhere does not carry the session's credential to the other host.

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
use std::process::{Command, Output};
use std::sync::{Arc, Mutex};

use smart_http::{GitStandIn, Request, read_body, read_head};

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

const CREDENTIAL_FLAGS: [&str; 6] = [
    "--api-key-file",
    "--api-key-env",
    "--api-key-secret",
    "--oauth-token-file",
    "--oauth-token-env",
    "--oauth-token-pointer",
];

const TOKEN: &str = concat!("adv-a-", "loopback-", "token-", "5e1f07");

fn git(tree: &Path, args: &[&str]) {
    let status = Command::new("git")
        .args(["-c", "user.name=t", "-c", "user.email=t@localhost"])
        .args(["-c", "commit.gpgsign=false"])
        .args(args)
        .current_dir(tree)
        .status()
        .expect("git runs");
    assert!(status.success(), "git {args:?}");
}

/// A stand-in on `host`:0 that logs every request with its headers and answers each with
/// `response`, a whole HTTP/1.1 response.
struct Recorder {
    port: u16,
    log: Arc<Mutex<Vec<Request>>>,
}

impl Recorder {
    fn serve(host: &str, response: String) -> Self {
        let listener = TcpListener::bind(format!("{host}:0")).expect("a loopback port");
        let port = listener.local_addr().expect("an address").port();
        let log = Arc::new(Mutex::new(Vec::new()));
        let seen = Arc::clone(&log);
        std::thread::spawn(move || {
            for stream in listener.incoming().flatten() {
                let Ok(clone) = stream.try_clone() else {
                    continue;
                };
                let mut reader = BufReader::new(clone);
                let Some(request) = read_head(&mut reader) else {
                    continue;
                };
                let _ = read_body(&mut reader, &request);
                seen.lock().expect("the log").push(request);
                let mut out = stream;
                let _ = out.write_all(response.as_bytes());
            }
        });
        Self { port, log }
    }

    fn requests(&self) -> Vec<Request> {
        self.log.lock().expect("the log").clone()
    }
}

fn error_400() -> String {
    let body =
        r#"{"type":"error","error":{"type":"invalid_request_error","message":"a stand-in"}}"#;
    format!(
        "HTTP/1.1 400 Bad Request\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    )
}

struct Scratch(PathBuf);

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

enum Harness {
    Double,
    Real(PathBuf),
}

struct World {
    scratch: Scratch,
    _git: GitStandIn,
}

impl World {
    fn new(name: &str, harness: &Harness) -> Self {
        let outer = Path::new(env!("CARGO_TARGET_TMPDIR"))
            .join(format!("w48-adv-a-{}-{name}", std::process::id()));
        if outer.exists() {
            fs::remove_dir_all(&outer).expect("a stale scratch tree is removed");
        }
        let repo = outer.join("repo");
        let program = outer.join("home/.local/bin/b10x-harness");
        let runtime = outer.join("runtime");
        let served = outer.join("served/o/r.git");
        for dir in [
            &repo,
            program.parent().expect("a parent"),
            &runtime,
            &served,
        ] {
            fs::create_dir_all(dir).expect("a directory");
        }
        fs::set_permissions(&runtime, fs::Permissions::from_mode(0o700)).expect("0700");
        git(&served, &["init", "-q", "--bare", "-b", "main"]);
        let git_standin = GitStandIn::serve(outer.join("served"), None);
        fs::write(repo.join("README.md"), "a story\n").expect("a file");
        git(&repo, &["init", "-q", "-b", "main"]);
        git(&repo, &["add", "-A"]);
        git(&repo, &["commit", "-q", "-m", "start"]);
        git(
            &repo,
            &["remote", "add", "origin", &git_standin.url("o/r.git")],
        );
        match harness {
            Harness::Double => {
                fs::write(
                    &program,
                    format!(
                        "#!/bin/sh\nif [ \"$1\" = --version ]; then echo b10x-harness 0.22.0; exit 0; fi\nexec > '{}' 2>&1\nenv\necho '-- args'\nfor arg in \"$@\"; do echo \"$arg\"; done\necho '-- end'\nexit 1\n",
                        outer.join("started").display()
                    ),
                )
                .expect("a double");
                fs::set_permissions(&program, fs::Permissions::from_mode(0o755)).expect("exec");
            }
            Harness::Real(path) => {
                std::os::unix::fs::symlink(path, &program).expect("the real b10x-harness");
            }
        }
        Self {
            scratch: Scratch(outer),
            _git: git_standin,
        }
    }

    fn outer(&self) -> &Path {
        &self.scratch.0
    }

    /// `takt run --harness harness --model-endpoint <endpoint> --test-cmd true` plus `extra`.
    fn run(&self, endpoint: &str, extra: &[&str], env: &[(&str, &str)]) -> Output {
        let scan = self.outer().join("scan-none.yaml");
        fs::write(&scan, "scan: none\n").expect("a run configuration file");
        let mut command = Command::new(env!("CARGO_BIN_EXE_takt"));
        command
            .args([
                "run",
                "--repo",
                self.outer().join("repo").to_str().expect("utf-8"),
                "--intent",
                "probe",
                "--forge",
                "none",
                "--harness",
                "harness",
                "--model-endpoint",
                endpoint,
                "--iterations",
                "2",
                "--test-cmd",
                "true",
                "--config",
                scan.to_str().expect("utf-8"),
            ])
            .args(extra)
            .env("HOME", self.outer().join("home"))
            .env("XDG_STATE_HOME", self.outer().join("xdg-state"))
            .env("XDG_RUNTIME_DIR", self.outer().join("runtime"))
            .env_remove("DBUS_SESSION_BUS_ADDRESS");
        for variable in PROXY_VARIABLES {
            command.env_remove(variable);
        }
        command
            .envs(env.iter().copied())
            .output()
            .expect("takt runs")
    }

    /// `takt run --harness harness --test-cmd true` with a run file declaring `scan: none`, the
    /// model `relay` and an `llm.routes/1` section whose one endpoint, at `base`, is anonymous.
    fn run_route(&self, base: &str) -> Output {
        let llm = serde_json::json!({
            "format": "llm.routes/1",
            "endpoints": {
                "boundary": {"base_url": base, "protocol": "messages", "auth": {"kind": "anonymous"}}
            },
            "routes": {"relay": "boundary/a-model"}
        });
        let config = self.outer().join("route.yaml");
        fs::write(&config, format!("scan: none\nmodel: relay\nllm: {llm}\n")).expect("a run file");
        let mut command = Command::new(env!("CARGO_BIN_EXE_takt"));
        command
            .args([
                "run",
                "--repo",
                self.outer().join("repo").to_str().expect("utf-8"),
                "--intent",
                "probe",
                "--forge",
                "none",
                "--harness",
                "harness",
                "--iterations",
                "2",
                "--test-cmd",
                "true",
                "--config",
                config.to_str().expect("utf-8"),
            ])
            .env("HOME", self.outer().join("home"))
            .env("XDG_STATE_HOME", self.outer().join("xdg-state"))
            .env("XDG_RUNTIME_DIR", self.outer().join("runtime"))
            .env_remove("DBUS_SESSION_BUS_ADDRESS");
        for variable in PROXY_VARIABLES {
            command.env_remove(variable);
        }
        command.output().expect("takt runs")
    }

    fn started(&self) -> Option<(String, Vec<String>)> {
        let text = fs::read_to_string(self.outer().join("started")).ok()?;
        let (environment, rest) = text.split_once("-- args\n")?;
        let args = rest
            .split_once("-- end")
            .map_or(rest, |(args, _)| args)
            .lines()
            .map(str::to_owned)
            .collect();
        Some((environment.to_owned(), args))
    }

    fn opened(&self) -> Option<serde_json::Value> {
        let mut dirs = vec![self.outer().join("repo/.takt/cases")];
        while let Some(dir) = dirs.pop() {
            for entry in fs::read_dir(&dir).into_iter().flatten().flatten() {
                let path = entry.path();
                if path.is_dir() {
                    dirs.push(path);
                } else if path.file_name().is_some_and(|name| name == "events.jsonl") {
                    return fs::read_to_string(&path)
                        .expect("the run record")
                        .lines()
                        .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
                        .find(|line| line["event"] == "opened");
                }
            }
        }
        None
    }
}

fn text(output: &Output) -> String {
    format!(
        "exit {:?}\nstdout {}\nstderr {}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
}

/// Every host form takt reads as loopback starts its session with no credential flag, pointed at
/// exactly the URL given, the case recording the credential `none`: Loom 0.52.0's predicate
/// (the parsed URL's host) admits each form takt's string predicate admits.
#[test]
fn every_loopback_form_takt_admits_starts_a_keyless_session() {
    let forms = [
        "http://localhost:{p}/v1",
        "http://LocalHost:{p}/v1",
        "http://[::1]:{p}/v1",
        "http://[0:0:0:0:0:0:0:1]:{p}/v1",
        "http://127.0.0.2:{p}/v1",
        "http://127.255.255.254:{p}/v1",
        "https://127.0.0.1:{p}/v1",
        "https://localhost:{p}/v1",
        "http://127.0.0.1/v1",
    ];
    let mut failures = Vec::new();
    for (index, form) in forms.iter().enumerate() {
        let world = World::new(&format!("admit-{index}"), &Harness::Double);
        let endpoint = form.replace("{p}", "48113");
        let output = world.run(&endpoint, &[], &[]);
        let Some((_, args)) = world.started() else {
            failures.push(format!(
                "{endpoint}: b10x-harness never started: {}",
                text(&output)
            ));
            continue;
        };
        if !args
            .windows(2)
            .any(|pair| pair[0] == "--base-url" && pair[1] == endpoint)
        {
            failures.push(format!("{endpoint}: not pointed at it: {args:?}"));
        }
        if let Some(flag) = CREDENTIAL_FLAGS
            .iter()
            .find(|flag| args.iter().any(|arg| arg.starts_with(*flag)))
        {
            failures.push(format!("{endpoint}: handed {flag}: {args:?}"));
        }
        let credential = world
            .opened()
            .map(|line| line["endpoint"]["credential"].clone());
        if credential != Some(serde_json::json!({"kind": "none"})) {
            failures.push(format!("{endpoint}: credential recorded {credential:?}"));
        }
    }
    assert!(failures.is_empty(), "{failures:#?}");
}

/// Acceptance 3 and its look-alikes: with no credential flag, every endpoint whose host is not a
/// loopback host by takt's reading (another host, the unspecified address, a loopback-looking
/// name, userinfo, a numeric IPv4 spelling, a trailing dot, a mapped address) is refused with
/// exit 1 before a case opens, b10x-harness is never started, and nothing is sent anywhere.
#[test]
fn an_endpoint_off_loopback_or_disguised_as_loopback_is_refused_before_a_case() {
    let forms = [
        "http://example.com/v1",
        "https://example.com/v1",
        "http://0.0.0.0:48113/v1",
        "https://0.0.0.0:48113/v1",
        "http://127.0.0.1.example.com/v1",
        "https://127.0.0.1.example.com/v1",
        "https://localhost.example.com/v1",
        "http://127.0.0.1@example.com/v1",
        "http://127.0.0.1:48113@example.com/v1",
        "http://example.com\\@127.0.0.1/v1",
        "http://127.0.0.1\\@example.com/v1",
        "http://example.com#@127.0.0.1/v1",
        "http://2130706433/v1",
        "http://0x7f000001/v1",
        "http://0177.0.0.1/v1",
        "http://0x7f.0.0.1/v1",
        "http://127.1/v1",
        "http://127.0.0.1./v1",
        "http://localhost./v1",
        "http://[::ffff:127.0.0.1]/v1",
        "http://[::1]@example.com/v1",
        "http://%6c%6fcalhost/v1",
        "HTTP://example.com/v1",
        "ftp://127.0.0.1/v1",
    ];
    let mut failures = Vec::new();
    for (index, endpoint) in forms.iter().enumerate() {
        let world = World::new(&format!("refuse-{index}"), &Harness::Double);
        let proxy = Recorder::serve(
            "127.0.0.1",
            "HTTP/1.1 502 Bad Gateway\r\nContent-Length: 0\r\nConnection: close\r\n\r\n".into(),
        );
        let url = format!("http://127.0.0.1:{}", proxy.port);
        let output = world.run(
            endpoint,
            &[],
            &[
                ("http_proxy", &url),
                ("https_proxy", &url),
                ("HTTP_PROXY", &url),
                ("HTTPS_PROXY", &url),
            ],
        );
        if output.status.code() != Some(1) {
            failures.push(format!("{endpoint}: {}", text(&output)));
        }
        if world.started().is_some() {
            failures.push(format!("{endpoint}: b10x-harness was started"));
        }
        if world.outer().join("repo/.takt/cases").exists() {
            failures.push(format!("{endpoint}: a case opened"));
        }
        if !proxy.requests().is_empty() {
            failures.push(format!("{endpoint}: sent {:?}", proxy.requests()));
        }
    }
    assert!(failures.is_empty(), "{failures:#?}");
}

/// A credential flag with a loopback endpoint: the credential is used (not dropped for `none`),
/// handed to b10x-harness as a file path and its pointer, and its value is in no variable of the
/// harness's environment and in no argument.
#[test]
fn a_token_file_with_a_loopback_endpoint_reaches_the_harness_as_a_path_only() {
    let world = World::new("token-file", &Harness::Double);
    let token = world.outer().join("token.json");
    fs::write(&token, format!("{{\"oauth\":{{\"access\":\"{TOKEN}\"}}}}")).expect("a token file");
    fs::set_permissions(&token, fs::Permissions::from_mode(0o600)).expect("a private token file");
    let output = world.run(
        "http://127.0.0.1:48113/v1",
        &[
            "--endpoint-token-file",
            token.to_str().expect("utf-8"),
            "--endpoint-token-pointer",
            "/oauth/access",
        ],
        &[
            ("MODEL_RELAY_KEY", TOKEN),
            ("API_KEY", TOKEN),
            ("TOKEN", TOKEN),
        ],
    );
    let (environment, args) = world
        .started()
        .unwrap_or_else(|| panic!("b10x-harness never started: {}", text(&output)));
    assert!(
        args.windows(2)
            .any(|pair| pair[0] == "--oauth-token-file" && pair[1] == token.to_str().unwrap()),
        "{args:?}"
    );
    let credential = world
        .opened()
        .map(|line| line["endpoint"]["credential"].clone());
    assert_eq!(
        credential.as_ref().and_then(|c| c["kind"].as_str()),
        Some("subscription_file"),
        "{credential:?}"
    );
    assert!(!environment.contains(TOKEN), "{environment}");
    assert!(!args.iter().any(|arg| arg.contains(TOKEN)), "{args:?}");
}

/// Acceptance 4 for names the credential filter does not know: takt's environment holds the
/// stand-in's token under names no suffix matches (`API_KEY`, `TOKEN`, `MODEL_RELAY_KEY`,
/// `LLM_AUTH`); none of them reaches the keyless harness's environment or arguments.
#[test]
fn a_token_under_an_unknown_name_does_not_reach_a_keyless_harness() {
    let world = World::new("unknown-names", &Harness::Double);
    let output = world.run(
        "http://127.0.0.1:48113/v1",
        &[],
        &[
            ("API_KEY", TOKEN),
            ("TOKEN", TOKEN),
            ("MODEL_RELAY_KEY", TOKEN),
            ("LLM_AUTH", TOKEN),
            ("ANTHROPIC_BASE_URL", "http://example.com/v1"),
        ],
    );
    let (environment, args) = world
        .started()
        .unwrap_or_else(|| panic!("b10x-harness never started: {}", text(&output)));
    assert!(
        environment.lines().any(|line| line.starts_with("PATH=")),
        "{environment}"
    );
    assert!(!environment.contains(TOKEN), "{environment}");
    assert!(!environment.contains("example.com"), "{environment}");
    assert!(!args.iter().any(|arg| arg.contains(TOKEN)), "{args:?}");
}

/// The story's other keyless route, end to end on Loom 0.52.0: an `llm` route whose endpoint is
/// anonymous on a loopback host starts its session, b10x-harness pointed at the endpoint with no
/// credential flag and the case recording `none`; the same route on another host is refused
/// before a case opens.
#[test]
fn an_anonymous_loopback_route_starts_a_keyless_session_and_another_host_is_refused() {
    let world = World::new("route-loopback", &Harness::Double);
    let output = world.run_route("http://127.0.0.1:48113/v1");
    let (_, args) = world
        .started()
        .unwrap_or_else(|| panic!("b10x-harness never started: {}", text(&output)));
    assert!(
        args.windows(2)
            .any(|pair| pair[0] == "--base-url" && pair[1].starts_with("http://127.0.0.1:48113/v1")),
        "{args:?}"
    );
    for flag in CREDENTIAL_FLAGS {
        assert!(
            !args.iter().any(|arg| arg.starts_with(flag)),
            "{flag}: {args:?}"
        );
    }
    assert_eq!(
        world
            .opened()
            .map(|line| line["endpoint"]["credential"].clone()),
        Some(serde_json::json!({"kind": "none"}))
    );
    let other = World::new("route-other", &Harness::Double);
    let output = other.run_route("http://example.com/v1");
    assert_eq!(output.status.code(), Some(1), "{}", text(&output));
    assert!(other.started().is_none());
    assert!(!other.outer().join("repo/.takt/cases").exists());
}

/// The default subscription token file in HOME is not a fallback for a keyless loopback endpoint:
/// with `~/.claude/.credentials.json` holding a token, the real b10x-harness reaches the stand-in
/// with no `Authorization` or `x-api-key` header and no header holding that token.
#[test]
#[ignore = "needs a real b10x-harness on the program path or at TAKT_TEST_B10X_HARNESS"]
fn the_default_token_file_in_home_is_not_sent_to_a_keyless_loopback_endpoint() {
    let world = World::new("home-token", &Harness::Real(real_harness()));
    let claude = world.outer().join("home/.claude");
    fs::create_dir_all(&claude).expect("a directory");
    let file = claude.join(".credentials.json");
    fs::write(
        &file,
        format!("{{\"claudeAiOauth\":{{\"accessToken\":\"{TOKEN}\"}}}}"),
    )
    .expect("a token file");
    fs::set_permissions(&file, fs::Permissions::from_mode(0o600)).expect("0600");
    let relay = Recorder::serve("127.0.0.1", error_400());
    let output = world.run(&format!("http://127.0.0.1:{}/v1", relay.port), &[], &[]);
    let requests = relay.requests();
    assert!(
        !requests.is_empty(),
        "the stand-in was reached: {}",
        text(&output)
    );
    for request in &requests {
        assert_eq!(request.header("authorization"), None, "{request:?}");
        assert_eq!(request.header("x-api-key"), None, "{request:?}");
        assert!(
            !request
                .headers
                .iter()
                .any(|(_, value)| value.contains(TOKEN)),
            "{request:?}"
        );
    }
    assert_eq!(
        world
            .opened()
            .map(|line| line["endpoint"]["credential"].clone()),
        Some(serde_json::json!({"kind": "none"}))
    );
}

/// Every step transcript of a world's runs, for a failure message.
fn transcripts(outer: &Path) -> String {
    let mut found = String::new();
    let mut dirs = vec![outer.join("xdg-state/takt/transcripts")];
    while let Some(dir) = dirs.pop() {
        for entry in fs::read_dir(&dir).into_iter().flatten().flatten() {
            let path = entry.path();
            if path.is_dir() {
                dirs.push(path);
            } else {
                found.push_str(&fs::read_to_string(&path).unwrap_or_default());
            }
        }
    }
    found
}

fn real_harness() -> PathBuf {
    if let Some(path) = std::env::var_os("TAKT_TEST_B10X_HARNESS") {
        return PathBuf::from(path);
    }
    std::env::split_paths(&std::env::var_os("PATH").unwrap_or_default())
        .map(|dir| dir.join("b10x-harness"))
        .find(|path| path.is_file())
        .expect("a real b10x-harness on the program path or at TAKT_TEST_B10X_HARNESS")
}

/// The real b10x-harness, keyless, against a loopback endpoint that answers every request with a
/// 307 to another host (127.0.0.2 stands in for a host off this one): the other host is sent no
/// request carrying an `Authorization` or `x-api-key` header.
#[test]
#[ignore = "needs a real b10x-harness on the program path or at TAKT_TEST_B10X_HARNESS"]
fn a_keyless_loopback_redirect_carries_no_credential_header_to_another_host() {
    let world = World::new("redirect-keyless", &Harness::Real(real_harness()));
    let other = Recorder::serve("127.0.0.2", error_400());
    let location = format!("http://127.0.0.2:{}/v1/messages", other.port);
    let relay = Recorder::serve(
        "127.0.0.1",
        format!(
            "HTTP/1.1 307 Temporary Redirect\r\nLocation: {location}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
        ),
    );
    let output = world.run(&format!("http://127.0.0.1:{}/v1", relay.port), &[], &[]);
    assert!(
        !relay.requests().is_empty(),
        "the relay was reached: {}",
        text(&output)
    );
    for request in other.requests() {
        assert_eq!(request.header("authorization"), None, "{request:?}");
        assert_eq!(request.header("x-api-key"), None, "{request:?}");
    }
}

/// The same with `--endpoint-token-file` on the loopback endpoint: a relay that redirects to
/// another host over `http://` does not receive the token's value there, in any header.
#[test]
#[ignore = "needs a real b10x-harness on the program path or at TAKT_TEST_B10X_HARNESS"]
fn a_loopback_redirect_does_not_carry_the_token_to_another_host() {
    let world = World::new("redirect-token", &Harness::Real(real_harness()));
    let token = world.outer().join("token.json");
    fs::write(&token, format!("{{\"oauth\":{{\"access\":\"{TOKEN}\"}}}}")).expect("a token file");
    fs::set_permissions(&token, fs::Permissions::from_mode(0o600)).expect("a private token file");
    let other = Recorder::serve("127.0.0.2", error_400());
    let location = format!("http://127.0.0.2:{}/v1/messages", other.port);
    let relay = Recorder::serve(
        "127.0.0.1",
        format!(
            "HTTP/1.1 307 Temporary Redirect\r\nLocation: {location}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
        ),
    );
    let output = world.run(
        &format!("http://127.0.0.1:{}/v1", relay.port),
        &[
            "--endpoint-token-file",
            token.to_str().expect("utf-8"),
            "--endpoint-token-pointer",
            "/oauth/access",
        ],
        &[],
    );
    let at_relay = relay.requests();
    assert!(
        !at_relay.is_empty(),
        "the relay was reached: {}\ntranscripts {}",
        text(&output),
        transcripts(world.outer())
    );
    assert!(
        at_relay.iter().any(|request| request
            .headers
            .iter()
            .any(|(_, value)| value.contains(TOKEN))),
        "the relay itself was sent the token: {at_relay:?}"
    );
    for request in other.requests() {
        assert!(
            !request
                .headers
                .iter()
                .any(|(_, value)| value.contains(TOKEN)),
            "the token reached 127.0.0.2: {request:?}"
        );
    }
}
