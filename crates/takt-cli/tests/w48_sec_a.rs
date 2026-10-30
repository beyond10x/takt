//! Security conformance for story:git-and-model-through-loopback-relays on the Loom 0.52.0 pin:
//! takt's loopback reading of an endpoint's host and the session executor's (the parsed URL's
//! host) decide the same keyless session. Over `https://` a host the URL parser reads as loopback
//! and takt does not (a short or numeric IPv4 spelling, a trailing dot) is refused by the keyless
//! rule alone, as `http://` already is by the clear-text rule; a credential given for such a host
//! reaches b10x-harness and is not dropped for none; and every further IPv6 and case form takt
//! admits starts a keyless session pointed at exactly that URL, never a case the executor then
//! refuses.

#[path = "../../takt/tests/support/own_home.rs"]
mod own_home;

#[allow(dead_code)]
#[path = "support/smart_http.rs"]
mod smart_http;

use std::fs;
use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use smart_http::GitStandIn;

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

const TOKEN: &str = concat!("sec-a-", "loopback-", "token-", "9c2d41");

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

struct Scratch(PathBuf);

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

struct World {
    scratch: Scratch,
    _git: GitStandIn,
}

impl World {
    /// A repository whose origin is a loopback git stand-in, and a b10x-harness double on the
    /// program path that writes its environment and arguments to `started` and exits 1.
    fn new(name: &str) -> Self {
        let outer = Path::new(env!("CARGO_TARGET_TMPDIR"))
            .join(format!("w48-sec-a-{}-{name}", std::process::id()));
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
        fs::write(
            &program,
            format!(
                "#!/bin/sh\nif [ \"$1\" = --version ]; then echo b10x-harness 0.22.0; exit 0; fi\nexec > '{}' 2>&1\nenv\necho '-- args'\nfor arg in \"$@\"; do echo \"$arg\"; done\necho '-- end'\nexit 1\n",
                outer.join("started").display()
            ),
        )
        .expect("a double");
        fs::set_permissions(&program, fs::Permissions::from_mode(0o755)).expect("exec");
        Self {
            scratch: Scratch(outer),
            _git: git_standin,
        }
    }

    fn outer(&self) -> &Path {
        &self.scratch.0
    }

    /// `takt run --harness harness --model-endpoint <endpoint> --test-cmd true` plus `extra`.
    fn run(&self, endpoint: &str, extra: &[&str]) -> Output {
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
            .env("SEC_A_RELAY_TOKEN", TOKEN)
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

    fn case_opened(&self) -> bool {
        self.outer().join("repo/.takt/cases").exists()
    }

    fn recorded_credential(&self) -> Option<serde_json::Value> {
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
                        .find(|line| line["event"] == "opened")
                        .map(|line| line["endpoint"]["credential"].clone());
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

/// (I1, I4) Over `https://`, where the clear-text rule does not apply, every host the session
/// executor's URL parser reads as loopback and takt's reading does not is refused by the keyless
/// rule with no credential flag: exit 1, no case, b10x-harness never started.
#[test]
fn an_https_host_only_the_url_parser_reads_as_loopback_takes_no_keyless_session() {
    let forms = [
        "https://127.1:48113/v1",
        "https://2130706433:48113/v1",
        "https://0x7f.0.0.1:48113/v1",
        "https://0177.0.0.1:48113/v1",
        "https://127.0.0.1.:48113/v1",
        "https://localhost.:48113/v1",
        "https://127.000.000.001:48113/v1",
    ];
    let mut failures = Vec::new();
    for (index, endpoint) in forms.iter().enumerate() {
        let world = World::new(&format!("https-lookalike-{index}"));
        let output = world.run(endpoint, &[]);
        if output.status.code() != Some(1) {
            failures.push(format!("{endpoint}: {}", text(&output)));
        }
        if let Some((_, args)) = world.started() {
            failures.push(format!("{endpoint}: b10x-harness was started: {args:?}"));
        }
        if world.case_opened() {
            failures.push(format!("{endpoint}: a case opened"));
        }
    }
    assert!(failures.is_empty(), "{failures:#?}");
}

/// (I3, I4) A credential given for a host only the URL parser reads as loopback is not dropped
/// for none by the executor's keyless branch: a secret's name reaches b10x-harness as
/// `--api-key-secret`, a token file as `--oauth-token-file`, each recorded as given, and the
/// token's value is in no argument and no variable of the harness's environment.
#[test]
fn a_credential_for_a_host_only_the_url_parser_reads_as_loopback_is_kept() {
    let endpoint = "https://127.1:48113/v1";
    let world = World::new("secret-lookalike");
    let output = world.run(endpoint, &["--endpoint-secret", "relay/model-key"]);
    let (environment, args) = world
        .started()
        .unwrap_or_else(|| panic!("b10x-harness never started: {}", text(&output)));
    assert!(
        args.iter()
            .any(|arg| arg.starts_with("--api-key-secret") || arg == "relay/model-key")
            && args.iter().any(|arg| arg.contains("relay/model-key")),
        "the secret's name is not handed to b10x-harness: {args:?}"
    );
    assert_eq!(
        world
            .recorded_credential()
            .as_ref()
            .and_then(|credential| credential["kind"].as_str()),
        Some("secret"),
        "{:?}",
        world.recorded_credential()
    );
    assert!(!environment.contains(TOKEN), "{environment}");

    let world = World::new("token-file-lookalike");
    let token = world.outer().join("token.json");
    fs::write(&token, format!("{{\"oauth\":{{\"access\":\"{TOKEN}\"}}}}")).expect("a token file");
    fs::set_permissions(&token, fs::Permissions::from_mode(0o600)).expect("a private token file");
    let output = world.run(
        endpoint,
        &[
            "--endpoint-token-file",
            token.to_str().expect("utf-8"),
            "--endpoint-token-pointer",
            "/oauth/access",
        ],
    );
    let (environment, args) = world
        .started()
        .unwrap_or_else(|| panic!("b10x-harness never started: {}", text(&output)));
    assert!(
        args.windows(2)
            .any(|pair| pair[0] == "--oauth-token-file" && pair[1] == token.to_str().unwrap()),
        "the token file is not handed to b10x-harness: {args:?}"
    );
    assert!(!environment.contains(TOKEN), "{environment}");
    assert!(!args.iter().any(|arg| arg.contains(TOKEN)), "{args:?}");
}

/// (I4) Every further spelling takt reads as loopback (IPv6 with an embedded IPv4 tail, leading
/// zero groups, an all-caps name, no path) is read so by the executor too: the session starts,
/// pointed at exactly that URL, with no credential flag and the credential recorded `none`. A
/// disagreement shows as a case takt opened and b10x-harness never started.
#[test]
fn every_further_form_takt_reads_as_loopback_the_executor_starts_keyless() {
    let forms = [
        "http://[::0.0.0.1]:48113/v1",
        "http://[0:0:0:0:0:0:0.0.0.1]:48113/v1",
        "http://[0000:0000:0000:0000:0000:0000:0000:0001]:48113/v1",
        "http://[::0:1]:48113/v1",
        "http://LOCALHOST:48113/v1",
        "http://127.0.0.1:48113",
        "https://[::1]/v1",
    ];
    let mut failures = Vec::new();
    for (index, endpoint) in forms.iter().enumerate() {
        let world = World::new(&format!("further-{index}"));
        let output = world.run(endpoint, &[]);
        let Some((environment, args)) = world.started() else {
            failures.push(format!(
                "{endpoint}: b10x-harness never started (case opened: {}): {}",
                world.case_opened(),
                text(&output)
            ));
            continue;
        };
        if !args
            .windows(2)
            .any(|pair| pair[0] == "--base-url" && pair[1] == *endpoint)
        {
            failures.push(format!("{endpoint}: not pointed at it: {args:?}"));
        }
        if let Some(flag) = CREDENTIAL_FLAGS
            .iter()
            .find(|flag| args.iter().any(|arg| arg.starts_with(*flag)))
        {
            failures.push(format!("{endpoint}: handed {flag}: {args:?}"));
        }
        if environment.contains(TOKEN) {
            failures.push(format!(
                "{endpoint}: the token is in the harness's environment"
            ));
        }
        let credential = world.recorded_credential();
        if credential != Some(serde_json::json!({"kind": "none"})) {
            failures.push(format!("{endpoint}: credential recorded {credential:?}"));
        }
    }
    assert!(failures.is_empty(), "{failures:#?}");
}
