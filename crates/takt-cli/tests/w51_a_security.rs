//! Security review of story:harness-section-is-metaharness-session-request: integrity checks of
//! the contract the run file's `harness` mapping and the session flags carry. A mapping or a flag
//! reaches none of Loom's reserved members; `context` hands the session no file outside the
//! repository, through a link, untracked, or holding text of a credential's shape; every refusal
//! comes before any case opens and starts no harness. The harness programs are doubles that
//! leave their argv behind and exit; no run reaches a network.

#[path = "../../takt/tests/support/own_home.rs"]
mod own_home;

use std::fs;
use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

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

fn text(output: &Output) -> String {
    format!(
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
}

fn run_file(harness: &str) -> String {
    format!("iterations: 2\nforge: none\ntest_cmd: \"true\"\nscan: none\n{harness}")
}

/// A fake GitHub token: `ghp_` and 36 letters and digits, the shape takt's own
/// `credential_shape` reads as a credential.
fn token() -> String {
    format!("ghp_{}", "a1B2c3D4e5F6g7H8i9J0k1L2m3N4o5P6q7R8")
}

/// A scratch tree under this crate's build directory, removed on drop: a repository whose first
/// commit holds `README.md` and the files `extra` names, an empty HOME, and claude and codex
/// doubles that write their argv to a file and exit 1.
struct World(PathBuf);

impl Drop for World {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

impl World {
    fn new(name: &str, extra: &[(&str, &str)]) -> Self {
        let outer = Path::new(env!("CARGO_TARGET_TMPDIR"))
            .join(format!("w51-a-security-{name}-{}", std::process::id()));
        if outer.exists() {
            fs::remove_dir_all(&outer).expect("a stale scratch tree is removed");
        }
        let world = Self(outer);
        for dir in [world.repo(), world.home(), world.bin(), world.config_dir()] {
            fs::create_dir_all(dir).expect("a directory");
        }
        fs::write(world.repo().join("README.md"), "a story\n").expect("a file");
        for (path, body) in extra {
            let at = world.repo().join(path);
            fs::create_dir_all(at.parent().expect("a parent")).expect("a directory");
            fs::write(at, body).expect("a file");
        }
        git(&world.repo(), &["init", "-q", "-b", "main"]);
        git(&world.repo(), &["add", "-A"]);
        git(&world.repo(), &["commit", "-q", "-m", "start"]);
        for program in ["claude", "codex"] {
            let dir = if program == "codex" {
                world.home().join(".local/bin")
            } else {
                world.bin()
            };
            fs::create_dir_all(&dir).expect("a directory");
            let double = dir.join(program);
            fs::write(
                &double,
                format!(
                    "#!/bin/sh\ncase $1 in --version) echo '2.1.309 (Claude Code)'; exit 0 ;; esac\nfor arg in \"$@\"; do printf '%s\\n' \"$arg\"; done > '{}'\nexit 1\n",
                    world.started(program).display()
                ),
            )
            .expect("a double");
            fs::set_permissions(&double, fs::Permissions::from_mode(0o755)).expect("exec bit");
        }
        world
    }

    fn repo(&self) -> PathBuf {
        self.0.join("repo")
    }

    fn home(&self) -> PathBuf {
        self.0.join("home")
    }

    fn bin(&self) -> PathBuf {
        self.0.join("bin")
    }

    fn config_dir(&self) -> PathBuf {
        self.0.join("config")
    }

    fn started(&self, program: &str) -> PathBuf {
        self.0.join(format!("started-{program}"))
    }

    fn cases(&self) -> PathBuf {
        self.repo().join(".takt/cases")
    }

    fn file(&self, name: &str, text: &str) -> PathBuf {
        let path = self.config_dir().join(name);
        fs::write(&path, text).expect("a file beside the run file");
        path
    }

    /// Clears the cases and each double's argv file, and puts the repository back on `main`.
    fn fresh(&self) {
        if self.cases().exists() {
            fs::remove_dir_all(self.cases()).expect("an earlier case is removed");
        }
        for program in ["claude", "codex"] {
            let _ = fs::remove_file(self.started(program));
        }
        git(&self.repo(), &["checkout", "-q", "-f", "main"]);
    }

    fn run(&self, config: &Path, flags: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_takt"))
            .args([
                "run",
                "--repo",
                self.repo().to_str().expect("utf-8"),
                "--intent",
                "probe",
                "--config",
                config.to_str().expect("utf-8"),
            ])
            .args(flags)
            .env("HOME", self.home())
            .env(
                "PATH",
                format!(
                    "{}:{}",
                    self.bin().display(),
                    std::env::var("PATH").unwrap_or_default()
                ),
            )
            .env("XDG_STATE_HOME", self.0.join("xdg-state"))
            .env_remove("DBUS_SESSION_BUS_ADDRESS")
            .env_remove("XDG_RUNTIME_DIR")
            .env_remove("ANTHROPIC_API_KEY")
            .output()
            .expect("takt runs")
    }

    /// The argv `program`'s double was started with, as one text, when it was started.
    fn argv(&self, program: &str) -> Option<String> {
        fs::read_to_string(self.started(program)).ok()
    }

    /// Runs `harness` as the run file's `harness` member with `flags`, and asserts the refusal:
    /// exit 1, no case opened, no double started. Returns what the run said.
    fn refused_with(&self, harness: &str, flags: &[&str]) -> String {
        self.fresh();
        let config = self.file("run.yaml", &run_file(harness));
        let output = self.run(&config, flags);
        let said = text(&output);
        assert!(
            self.argv("claude").is_none() && self.argv("codex").is_none(),
            "no harness was started: {said}"
        );
        assert!(!self.cases().exists(), "no case was opened: {said}");
        assert_eq!(output.status.code(), Some(1), "{said}");
        said
    }

    /// Every regular file under the cases directory whose bytes hold `needle`.
    fn case_files_holding(&self, needle: &str) -> Vec<PathBuf> {
        fn walk(dir: &Path, needle: &[u8], into: &mut Vec<PathBuf>) {
            let Ok(entries) = fs::read_dir(dir) else {
                return;
            };
            for entry in entries.flatten() {
                let path = entry.path();
                let Ok(meta) = fs::symlink_metadata(&path) else {
                    continue;
                };
                if meta.is_dir() {
                    walk(&path, needle, into);
                } else if meta.is_file()
                    && fs::read(&path)
                        .is_ok_and(|bytes| bytes.windows(needle.len()).any(|w| w == needle))
                {
                    into.push(path);
                }
            }
        }
        let mut found = Vec::new();
        walk(&self.cases(), needle.as_bytes(), &mut found);
        found
    }
}

/// Every key of metaharness's session-request document outside the admitted six (Loom's reserved
/// members: credentials, working directory, scratch copy, hermetic mode, decision mode, endpoint,
/// secret route, and the rest) is refused before any case opens under each kind, naming the key,
/// never printing the value, and starting no harness. The list is metaharness 0.23.0's
/// `RunSpec::DOCUMENT_KEYS`.
#[test]
fn every_reserved_session_key_is_refused_before_any_case_opens() {
    let world = World::new("reserved", &[]);
    let reserved = [
        "hermetic",
        "prompt",
        "frame",
        "decisions",
        "actor",
        "hooks",
        "driver",
        "tool-surface",
        "catalogue",
        "allow-program",
        "credentials",
        "model",
        "model-endpoint",
        "model-wire",
        "subscription-token-file",
        "subscription-token-env",
        "subscription-token-pointer",
        "api-key-secret",
        "plugin-dir",
        "plugin",
        "cwd",
        "scratch-cwd",
        "retain-dir",
        "audit",
        "spec",
        "substrate",
        "substrate-embedded",
        "cgroup-root",
        "toolchain",
        "prices",
        "write-scope",
        "process-write-subtree",
        "scope-announce",
        "auditor",
        "auditor-args",
    ];
    for kind in ["claude", "codex", "b10x"] {
        for key in reserved {
            let said = world.refused_with(
                &format!("harness: {{kind: {kind}, {key}: MARKER-reserved-value}}\n"),
                &[],
            );
            assert!(said.contains(&format!("`{key}`")), "{kind} {key}: {said}");
            assert!(
                !said.contains("MARKER-reserved-value"),
                "{kind} {key}: {said}"
            );
        }
    }
}

/// A `context` path reached through a tracked symbolic link, to a file outside the repository or
/// through a linked directory, is refused before any case opens, naming it, and the outside file's
/// text reaches no harness.
#[test]
fn a_context_path_through_a_tracked_link_is_refused() {
    let world = World::new("link", &[]);
    fs::write(world.0.join("outside.txt"), "MARKER-outside-text\n").expect("an outside file");
    std::os::unix::fs::symlink("../outside.txt", world.repo().join("linked.md"))
        .expect("a file link");
    std::os::unix::fs::symlink("..", world.repo().join("up")).expect("a directory link");
    git(&world.repo(), &["add", "linked.md", "up"]);
    git(&world.repo(), &["commit", "-q", "-m", "links"]);
    for path in ["linked.md", "up/outside.txt", "up/repo/README.md"] {
        let said = world.refused_with(
            &format!("harness: {{kind: claude, context: [\"{path}\"]}}\n"),
            &[],
        );
        assert!(said.contains(path), "{path}: {said}");
        let said = world.refused_with("harness: claude\n", &["--context", path]);
        assert!(said.contains(path), "--context {path}: {said}");
    }
}

/// A `context` path the index does not track, untracked in the work tree or inside `.git`, is
/// refused before any case opens, naming it, whether the file or a flag names it.
#[test]
fn an_untracked_or_git_internal_context_path_is_refused() {
    let world = World::new("untracked", &[]);
    fs::write(world.repo().join("scratch.md"), "MARKER-untracked\n").expect("an untracked file");
    for path in ["scratch.md", ".git/config", "README.md/"] {
        let said = world.refused_with(
            &format!("harness: {{kind: claude, context: [\"{path}\"]}}\n"),
            &[],
        );
        assert!(said.contains(path.trim_end_matches('/')), "{path}: {said}");
        let said = world.refused_with("harness: {kind: codex}\n", &["--context", path]);
        assert!(
            said.contains(path.trim_end_matches('/')),
            "--context {path}: {said}"
        );
    }
}

/// A tracked file holding text of a credential's shape, named in the mapping's `context`, is not
/// handed to the session: takt refuses its own instruction files and leaves out scope files of
/// that shape (`crates/takt/src/harness.rs` `instruction_files`, `scope_context_files`), and the
/// run is refused before any case opens, as every other context refusal is.
#[test]
fn a_context_file_of_a_credentials_shape_from_the_mapping_reaches_no_session() {
    let secret = token();
    let world = World::new(
        "credential-mapping",
        &[("deploy/notes.txt", &format!("GH_TOKEN={secret}\n"))],
    );
    world.fresh();
    let config = world.file(
        "run.yaml",
        &run_file("harness: {kind: claude, context: [deploy/notes.txt]}\n"),
    );
    let output = world.run(&config, &[]);
    let said = text(&output);
    let argv = world.argv("claude").unwrap_or_default();
    assert!(
        !argv.contains(&secret),
        "the credential-shaped context file's text was handed to the session's argv"
    );
    assert!(!world.cases().exists(), "no case was opened: {said}");
    assert_eq!(output.status.code(), Some(1), "{said}");
}

/// The same file named by `--context` on the command line reaches no session either.
#[test]
fn a_context_file_of_a_credentials_shape_from_the_flag_reaches_no_session() {
    let secret = token();
    let world = World::new(
        "credential-flag",
        &[("deploy/notes.txt", &format!("GH_TOKEN={secret}\n"))],
    );
    world.fresh();
    let config = world.file("run.yaml", &run_file("harness: claude\n"));
    let output = world.run(&config, &["--context", "deploy/notes.txt"]);
    let said = text(&output);
    let argv = world.argv("claude").unwrap_or_default();
    assert!(
        !argv.contains(&secret),
        "the credential-shaped context file's text was handed to the session's argv"
    );
    assert!(!world.cases().exists(), "no case was opened: {said}");
    assert_eq!(output.status.code(), Some(1), "{said}");
}

/// No case record holds the text of a credential-shaped file named in `context`, whatever the
/// session was handed.
#[test]
fn no_case_record_holds_a_credential_shaped_context_files_text() {
    let secret = token();
    let world = World::new(
        "credential-record",
        &[("deploy/notes.txt", &format!("GH_TOKEN={secret}\n"))],
    );
    world.fresh();
    let config = world.file(
        "run.yaml",
        &run_file("harness: {kind: claude, context: [deploy/notes.txt]}\n"),
    );
    let output = world.run(&config, &[]);
    let holding = world.case_files_holding(&secret);
    assert!(
        holding.is_empty(),
        "case records hold the credential-shaped context file's text: {holding:?}\n{}",
        text(&output).replace(&secret, "<token>")
    );
}
