//! story:run-flags-spell-metaharness-and-llm: `takt run` and `takt scenario run` take the endpoint and
//! llm settings under the names `metaharness run` and llm use, and the `--endpoint-*` names keep
//! working for one minor release, each with one line on standard error naming the new one. No run
//! here reaches a network: `--iterations 0` opens the case and stops before any step, a
//! `b10x-harness` double leaves a marker behind if it runs, HOME is an empty scratch directory, and
//! each credential is a secret's name (takt reads no secret) or a scratch token file.

#[path = "../../takt/tests/support/own_home.rs"]
mod own_home;

use std::fs;
use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

const SECRET: &str = "llm/opencode-go-api-key";
const BASE_URL: &str = "https://opencode.ai/inference/go/anthropic/v1";
const FIXTURES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../takt/tests/fixtures");

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

fn git_out(tree: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .args(args)
        .current_dir(tree)
        .output()
        .expect("git runs");
    assert!(output.status.success(), "git {args:?}");
    String::from_utf8_lossy(&output.stdout).trim().to_owned()
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

fn files_under(dir: &Path) -> Vec<PathBuf> {
    let mut found = Vec::new();
    let Ok(entries) = fs::read_dir(dir) else {
        return found;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let Ok(meta) = fs::symlink_metadata(&path) else {
            continue;
        };
        if meta.is_dir() {
            found.extend(files_under(&path));
        } else if meta.is_file() {
            found.push(path);
        }
    }
    found
}

/// The `opened` records of the one case under `cases`.
fn opened_lines(cases: &Path) -> Vec<serde_json::Value> {
    let events: Vec<PathBuf> = files_under(cases)
        .into_iter()
        .filter(|path| path.file_name().is_some_and(|name| name == "events.jsonl"))
        .collect();
    assert_eq!(events.len(), 1, "one case record: {events:?}");
    fs::read_to_string(&events[0])
        .expect("the run record")
        .lines()
        .map(|line| serde_json::from_str::<serde_json::Value>(line).expect("a JSON line"))
        .filter(|line| line["event"] == "opened")
        .collect()
}

fn text(output: &Output) -> String {
    format!(
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
}

/// A scratch tree under this crate's build directory, removed on drop: a repository with one commit
/// (a scenario repository declaring scenario `1` when `scenario`), an empty HOME, a program directory
/// holding a `b10x-harness` double that leaves `started` behind, and a config directory.
struct World {
    outer: PathBuf,
    scenario: bool,
}

impl Drop for World {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.outer);
    }
}

impl World {
    fn new(name: &str, scenario: bool) -> Self {
        let outer = Path::new(env!("CARGO_TARGET_TMPDIR"))
            .join(format!("run-flags-{name}-{}", std::process::id()));
        if outer.exists() {
            fs::remove_dir_all(&outer).expect("a stale scratch tree is removed");
        }
        let world = Self { outer, scenario };
        for dir in [world.repo(), world.home(), world.bin(), world.config_dir()] {
            fs::create_dir_all(dir).expect("a directory");
        }
        if scenario {
            copy_dir(&Path::new(FIXTURES).join("one-ready"), &world.repo());
        } else {
            fs::write(world.repo().join("README.md"), "a story\n").expect("a file");
        }
        git(&world.repo(), &["init", "-q", "-b", "main"]);
        git(&world.repo(), &["add", "-A"]);
        git(&world.repo(), &["commit", "-q", "-m", "start"]);
        if scenario {
            let seed = git_out(&world.repo(), &["rev-parse", "HEAD"]);
            fs::write(
                world.repo().join(".engineering/scenarios.conf"),
                format!("[scenario \"1\"]\n\tstory = story:ready-one\n\tseed = {seed}\n"),
            )
            .expect("the declaration");
            git(&world.repo(), &["add", "-A"]);
            git(
                &world.repo(),
                &["commit", "-q", "-m", "declare the scenarios"],
            );
        }
        let double = world.bin().join("b10x-harness");
        fs::write(
            &double,
            format!("#!/bin/sh\nif [ \"$1\" = \"--version\" ]; then echo double; exit 0; fi\ntouch '{}'\nexit 1\n", world.started().display()),
        )
        .expect("a double");
        fs::set_permissions(&double, fs::Permissions::from_mode(0o755)).expect("exec bit");
        world
    }

    fn repo(&self) -> PathBuf {
        self.outer.join("repo")
    }

    fn home(&self) -> PathBuf {
        self.outer.join("home")
    }

    fn bin(&self) -> PathBuf {
        self.outer.join("bin")
    }

    fn config_dir(&self) -> PathBuf {
        self.outer.join("config")
    }

    fn started(&self) -> PathBuf {
        self.outer.join("started")
    }

    fn cases(&self) -> PathBuf {
        self.repo().join(takt::case::CASES_DIR)
    }

    /// Writes `text` as `config/<name>` and returns its path.
    fn file(&self, name: &str, text: &str) -> PathBuf {
        let path = self.config_dir().join(name);
        fs::write(&path, text).expect("a file beside the run file");
        path
    }

    /// The token file every credential test names: a JSON object whose `/token` is the token.
    fn token(&self) -> PathBuf {
        self.file("token.json", "{\"token\": \"TOKEN-marker\"}\n")
    }

    /// The route key file of mode 0600, which no output or case file may carry.
    fn key(&self) -> PathBuf {
        let path = self.outer.join("route-key");
        fs::write(&path, "KEY-marker").expect("a key file");
        fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).expect("mode 0600");
        path
    }

    /// `takt run` (or `takt scenario run 1`) on the repository, `args` after the fixed ones, HOME the
    /// scratch one, the double first on the program path, and no session bus.
    fn run(&self, args: &[&str]) -> Output {
        let repo = self.repo();
        let repo = repo.to_str().expect("utf-8");
        let mut command = Command::new(env!("CARGO_BIN_EXE_takt"));
        if self.scenario {
            command.args([
                "scenario",
                "run",
                "--repo",
                repo,
                "1",
                "--test-cmd",
                "true",
                "--forge",
                "none",
            ]);
        } else {
            command.args([
                "run", "--repo", repo, "--intent", "probe", "--forge", "none",
            ]);
        }
        command
            .args(args)
            .env("HOME", self.home())
            .env(
                "PATH",
                format!(
                    "{}:{}",
                    self.bin().display(),
                    std::env::var("PATH").unwrap_or_default()
                ),
            )
            .env("XDG_STATE_HOME", self.outer.join("xdg-state"))
            .env_remove("DBUS_SESSION_BUS_ADDRESS")
            .env_remove("XDG_RUNTIME_DIR")
            // A harness run opens only under a policy or a declared `scan: none`; no step here
            // reaches a scan.
            .env("B10X_GATES_POLICY", self.outer.join("gates-policy.json"))
            .env_remove("ANTHROPIC_API_KEY")
            .output()
            .expect("takt runs")
    }

    /// A run that opens its case and stops at `--iterations 0`: exit 3, the double not started, and
    /// the case's `opened` record returned with the run's standard error. The cases directory is
    /// removed, so the next run in this tree opens its own record alone.
    fn opened_run(&self, args: &[&str]) -> (String, serde_json::Value) {
        git(&self.repo(), &["checkout", "-q", "-f", "main"]);
        let output = self.run(args);
        let said = text(&output);
        assert_eq!(output.status.code(), Some(3), "{said}");
        assert!(
            !self.started().exists(),
            "b10x-harness was not started: {said}"
        );
        let mut opened = opened_lines(&self.cases());
        assert_eq!(opened.len(), 1, "{said}");
        let record = opened.remove(0);
        fs::remove_dir_all(self.cases()).expect("the case is removed");
        git(&self.repo(), &["checkout", "-q", "-f", "main"]);
        let branches = git_out(&self.repo(), &["branch", "--format=%(refname:short)"]);
        for branch in branches
            .lines()
            .filter(|branch| branch.starts_with("takt/"))
        {
            git(&self.repo(), &["branch", "-D", branch]);
        }
        (String::from_utf8_lossy(&output.stderr).into_owned(), record)
    }
}

/// The lines of `stderr` that name `new` as the flag a renamed `old` now is.
fn renamed_lines<'a>(stderr: &'a str, new: &str) -> Vec<&'a str> {
    stderr
        .lines()
        .filter(|line| line.contains("renamed") && line.contains(new))
        .collect()
}

/// The `harness` run arguments that stop at once: the harness, and no iteration.
fn harness_args<'a>(extra: &[&'a str]) -> Vec<&'a str> {
    join(&["--harness", "harness", "--iterations", "0"], extra)
}

/// `first` then `second`, as one argument list.
fn join<'a>(first: &[&'a str], second: &[&'a str]) -> Vec<&'a str> {
    let mut args = first.to_vec();
    args.extend_from_slice(second);
    args
}

/// An endpoint file's text for `--endpoint-file`, the same endpoint a flag run names.
fn endpoint_document(base_url: &str, wire: &str) -> String {
    format!("base_url = \"{base_url}\"\nwire = \"{wire}\"\n\n[credential]\nsecret = \"{SECRET}\"\n")
}

/// The `llm.routes/1` document A: route `small`, alias `fast`, on endpoint `e` at port 9.
fn document_a(key: &Path) -> serde_json::Value {
    document("http://127.0.0.1:9/v1", key)
}

/// The `llm.routes/1` document B: the same routes on endpoint `e` at port 8, so its digest differs.
fn document_b(key: &Path) -> serde_json::Value {
    document("http://127.0.0.1:8/v1", key)
}

fn document(base_url: &str, key: &Path) -> serde_json::Value {
    serde_json::json!({
        "format": "llm.routes/1",
        "endpoints": {
            "e": {
                "base_url": base_url,
                "protocol": "responses",
                "auth": {"kind": "bearer", "secret": format!("file:{}", key.display())}
            }
        },
        "routes": {"small": {"model": "e/upstream-small", "aliases": ["fast"]}}
    })
}

/// Document B as TOML: the same document, for `--llm-config` with a `.toml` name.
fn document_b_toml(key: &Path) -> String {
    format!(
        "format = \"llm.routes/1\"\n\n[endpoints.e]\nbase_url = \"http://127.0.0.1:8/v1\"\nprotocol = \"responses\"\n\n[endpoints.e.auth]\nkind = \"bearer\"\nsecret = \"file:{}\"\n\n[routes.small]\nmodel = \"e/upstream-small\"\naliases = [\"fast\"]\n",
        key.display()
    )
}

/// A run file on `harness: harness` with `model: small` and the `llm` section `llm`.
fn llm_file(llm: &serde_json::Value) -> String {
    format!("iterations: 0\nharness: harness\nforge: none\nmodel: small\nllm: {llm}\n")
}

/// Acceptance 1, for `takt run`: each new endpoint flag, given alone with the credential it needs,
/// opens the record its endpoint file gives, and prints no rename line.
#[test]
fn each_new_endpoint_flag_opens_the_record_of_its_endpoint_file() {
    let world = World::new("new-names", false);
    let token = world.token();
    let token = token.to_str().expect("utf-8");
    let cases: [(&str, Vec<String>, String); 5] = [
        (
            "model-endpoint",
            vec![
                "--model-endpoint".to_owned(),
                BASE_URL.to_owned(),
                "--api-key-secret".to_owned(),
                SECRET.to_owned(),
            ],
            format!("base_url = \"{BASE_URL}\"\n\n[credential]\nsecret = \"{SECRET}\"\n"),
        ),
        (
            "model-wire",
            vec![
                "--model-wire".to_owned(),
                "openai-responses".to_owned(),
                "--api-key-secret".to_owned(),
                SECRET.to_owned(),
            ],
            format!("wire = \"openai-responses\"\n\n[credential]\nsecret = \"{SECRET}\"\n"),
        ),
        (
            "subscription-token-file",
            vec!["--subscription-token-file".to_owned(), token.to_owned()],
            format!("[credential]\ntoken_file = \"{token}\"\n"),
        ),
        (
            "subscription-token-pointer",
            vec![
                "--subscription-token-file".to_owned(),
                token.to_owned(),
                "--subscription-token-pointer".to_owned(),
                "/token".to_owned(),
            ],
            format!("[credential]\ntoken_file = \"{token}\"\ntoken_pointer = \"/token\"\n"),
        ),
        (
            "api-key-secret",
            vec!["--api-key-secret".to_owned(), SECRET.to_owned()],
            format!("[credential]\nsecret = \"{SECRET}\"\n"),
        ),
    ];
    for (name, flags, file_text) in cases {
        let flags: Vec<&str> = flags.iter().map(String::as_str).collect();
        let (stderr, by_flag) = world.opened_run(&harness_args(&flags));
        assert!(!stderr.contains("renamed"), "{name}: {stderr}");
        let file = world.file(&format!("{name}.toml"), &file_text);
        let file = file.to_str().expect("utf-8");
        let (stderr, by_file) = world.opened_run(&harness_args(&["--endpoint-file", file]));
        assert!(!stderr.contains("renamed"), "{name}: {stderr}");
        assert_eq!(by_flag["endpoint"], by_file["endpoint"], "{name}");
    }
}

/// Acceptance 1 for the endpoint document's own model: `--model` names it under `--harness harness`,
/// and the record's endpoint model is that name, not the file's default.
#[test]
fn model_flag_names_the_endpoint_model() {
    let world = World::new("model-flag", false);
    let (_, record) = world.opened_run(&harness_args(&[
        "--model",
        "claude-haiku-5-5",
        "--api-key-secret",
        SECRET,
    ]));
    assert_eq!(
        record["endpoint"]["model"], "claude-haiku-5-5",
        "{record:?}"
    );
}

/// Acceptance 2, for `takt run`: each of the six old names opens the record its new name opens, and
/// prints exactly one line on standard error naming the new one; the new name prints none.
#[test]
fn each_old_name_opens_the_record_of_its_new_name_and_warns_once() {
    let world = World::new("aliases", false);
    let token = world.token();
    let token = token.to_str().expect("utf-8");
    let pairs: [(&str, &str, &str, Vec<&str>); 6] = [
        (
            "--endpoint-base-url",
            "--model-endpoint",
            BASE_URL,
            vec!["--api-key-secret", SECRET],
        ),
        (
            "--endpoint-wire",
            "--model-wire",
            "openai-responses",
            vec!["--api-key-secret", SECRET],
        ),
        (
            "--endpoint-model",
            "--model",
            "claude-haiku-5-5",
            vec!["--api-key-secret", SECRET],
        ),
        (
            "--endpoint-token-file",
            "--subscription-token-file",
            token,
            vec![],
        ),
        (
            "--endpoint-token-pointer",
            "--subscription-token-pointer",
            "/token",
            vec!["--subscription-token-file", token],
        ),
        ("--endpoint-secret", "--api-key-secret", SECRET, vec![]),
    ];
    for (old, new, value, rest) in pairs {
        let (old_stderr, by_old) = world.opened_run(&harness_args(&join(&[old, value], &rest)));
        let warnings = renamed_lines(&old_stderr, new);
        assert_eq!(warnings.len(), 1, "{old}: {old_stderr}");
        assert!(warnings[0].contains(old), "{old}: {old_stderr}");
        assert_eq!(
            old_stderr.matches("renamed").count(),
            1,
            "{old}: {old_stderr}"
        );
        let (new_stderr, by_new) = world.opened_run(&harness_args(&join(&[new, value], &rest)));
        assert!(!new_stderr.contains("renamed"), "{new}: {new_stderr}");
        assert_eq!(
            by_old["endpoint"], by_new["endpoint"],
            "{old} against {new}"
        );
    }
}

/// Acceptance 3: `--llm-config` over a run file's `llm` section gives the document's digest in the
/// `opened` record, the section's own digest is not recorded; and `--llm-config` alone, without
/// `--config`, does the same from a TOML document.
/// Linux only: llm 0.16.0 reads a `{file: ...}` key on Linux alone and refuses it elsewhere
/// (`llm-credentials` `file.rs`, `SecretError::UnsupportedPlatform`).
#[cfg(target_os = "linux")]
#[test]
fn llm_config_replaces_the_run_files_section() {
    let world = World::new("llm-config", false);
    let key = world.key();
    let b_yaml = world.file("b.yaml", &document_b(&key).to_string());
    let b_yaml = b_yaml.to_str().expect("utf-8");
    let inline_b = world.file("inline-b.yaml", &llm_file(&document_b(&key)));
    let inline_b = inline_b.to_str().expect("utf-8");
    let (_, expected) = world.opened_run(&["--config", inline_b]);
    let digest_b = expected["llm_digest"]
        .as_str()
        .expect("a digest")
        .to_owned();

    let section_a = world.file("a.yaml", &llm_file(&document_a(&key)));
    let section_a = section_a.to_str().expect("utf-8");
    let (_, by_section) = world.opened_run(&["--config", section_a]);
    let digest_a = by_section["llm_digest"]
        .as_str()
        .expect("a digest")
        .to_owned();
    assert_ne!(digest_a, digest_b);

    let (_, over) = world.opened_run(&["--config", section_a, "--llm-config", b_yaml]);
    assert_eq!(over["llm_digest"], digest_b.as_str(), "{over:?}");

    let b_toml = world.file("b.toml", &document_b_toml(&key));
    let b_toml = b_toml.to_str().expect("utf-8");
    let (_, alone) = world.opened_run(&[
        "--harness",
        "harness",
        "--iterations",
        "0",
        "--model",
        "small",
        "--llm-config",
        b_toml,
    ]);
    assert_eq!(alone["llm_digest"], digest_b.as_str(), "{alone:?}");
}

/// Acceptance 4, the names `takt run --help` lists: the five endpoint flags and `--llm-config`.
#[test]
fn run_help_lists_the_new_names() {
    let output = Command::new(env!("CARGO_BIN_EXE_takt"))
        .args(["run", "--help"])
        .output()
        .expect("takt runs");
    let said = text(&output);
    assert_eq!(output.status.code(), Some(0), "{said}");
    for flag in [
        "--model-endpoint",
        "--model-wire",
        "--subscription-token-file",
        "--subscription-token-pointer",
        "--api-key-secret",
        "--llm-config",
    ] {
        assert!(said.contains(flag), "takt run --help names {flag}: {said}");
    }
}

/// The new endpoint flags are endpoint sources: beside an `llm` section, refused before any case
/// opens, naming the flag, as the old names are.
#[test]
fn a_new_endpoint_flag_beside_an_llm_section_is_refused() {
    let world = World::new("section-refused", false);
    let key = world.key();
    let token = world.token();
    let token = token.to_str().expect("utf-8");
    let config = world.file("run.yaml", &llm_file(&document_a(&key)));
    let config = config.to_str().expect("utf-8");
    let output = world.run(&["--config", config, "--subscription-token-file", token]);
    let said = text(&output);
    assert_eq!(output.status.code(), Some(1), "{said}");
    assert!(
        said.contains("--subscription-token-file") && said.contains("llm"),
        "{said}"
    );
    assert!(!world.cases().exists(), "no case was opened");
    assert!(!world.started().exists(), "b10x-harness was not started");
}

/// `--llm-config` beside a new endpoint flag is refused the same way: the document is an `llm`
/// section, and an endpoint source sits beside it.
#[test]
fn llm_config_beside_a_new_endpoint_flag_is_refused() {
    let world = World::new("config-refused", false);
    let key = world.key();
    let b_yaml = world.file("b.yaml", &document_b(&key).to_string());
    let b_yaml = b_yaml.to_str().expect("utf-8");
    let output = world.run(&[
        "--harness",
        "harness",
        "--llm-config",
        b_yaml,
        "--model",
        "small",
        "--model-endpoint",
        "http://127.0.0.1:9/v1",
    ]);
    let said = text(&output);
    assert_eq!(output.status.code(), Some(1), "{said}");
    assert!(
        said.contains("--model-endpoint") && said.contains("llm"),
        "{said}"
    );
    assert!(!world.cases().exists(), "no case was opened");
}

/// `--llm-config` under `--harness claude` is refused before any case opens, naming `claude`.
#[test]
fn llm_config_under_claude_is_refused() {
    let world = World::new("config-claude", false);
    let key = world.key();
    let b_yaml = world.file("b.yaml", &document_b(&key).to_string());
    let b_yaml = b_yaml.to_str().expect("utf-8");
    let output = world.run(&["--harness", "claude", "--llm-config", b_yaml]);
    let said = text(&output);
    assert_eq!(output.status.code(), Some(1), "{said}");
    assert!(said.contains("claude"), "{said}");
    assert!(!world.cases().exists(), "no case was opened");
}

/// A new name and its old name given together, and `--model` with `--endpoint-model`, are refused by
/// the argument parser before anything runs.
#[test]
fn a_flag_and_its_old_name_together_are_refused() {
    let world = World::new("both-names", false);
    let pairs: [&[&str]; 2] = [
        &[
            "--model-endpoint",
            "http://127.0.0.1:9/v1",
            "--endpoint-base-url",
            "http://127.0.0.1:8/v1",
        ],
        &[
            "--model",
            "a",
            "--endpoint-model",
            "b",
            "--harness",
            "harness",
        ],
    ];
    for pair in pairs {
        let output = world.run(pair);
        let said = text(&output);
        assert_eq!(output.status.code(), Some(2), "{said}");
        assert!(said.contains("cannot be used with"), "{said}");
        assert!(!world.cases().exists(), "no case was opened");
    }
}

/// Acceptance 1 and 2 for `takt scenario run`: the new endpoint flag, its old name (one warning) and
/// the endpoint file each open the same record.
#[test]
fn scenario_run_takes_the_new_names_and_their_old_ones() {
    let world = World::new("scenario", true);
    let (_, by_new) = world.opened_run(&harness_args(&[
        "--model-endpoint",
        BASE_URL,
        "--api-key-secret",
        SECRET,
    ]));
    let (old_stderr, by_old) = world.opened_run(&harness_args(&[
        "--endpoint-base-url",
        BASE_URL,
        "--api-key-secret",
        SECRET,
    ]));
    assert_eq!(
        renamed_lines(&old_stderr, "--model-endpoint").len(),
        1,
        "{old_stderr}"
    );
    let file = world.file(
        "endpoint.toml",
        &endpoint_document(BASE_URL, "anthropic-messages"),
    );
    let file = file.to_str().expect("utf-8");
    let (_, by_file) = world.opened_run(&harness_args(&["--endpoint-file", file]));
    assert_eq!(by_new["endpoint"], by_old["endpoint"]);
    assert_eq!(by_new["endpoint"], by_file["endpoint"]);
}
