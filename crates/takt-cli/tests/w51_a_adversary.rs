//! Adversary pass on story:harness-section-is-metaharness-session-request (wave 51, unit a): the
//! run file's `harness` mapping and the session flags over it, carried to the session. Each case
//! drives `takt` with harness doubles on the program path that leave their argv behind; nothing
//! reaches a network or a model.

#[path = "../../takt/tests/support/own_home.rs"]
mod own_home;

use std::fs;
use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

/// The fixture stores of the library crate, one directory each.
const FIXTURES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../takt/tests/fixtures");

fn git(tree: &Path, args: &[&str]) -> String {
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
    String::from_utf8_lossy(&output.stdout).trim().to_owned()
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

/// A scratch world under this crate's build directory, removed on drop: a repository, an empty
/// HOME whose `.local/bin` holds the codex double, a program directory holding the claude
/// double, and a config directory. Each double writes its argv, one argument a line, and exits 1.
struct World(PathBuf);

impl Drop for World {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

impl World {
    /// The world with an empty repository directory; the caller fills and commits it.
    fn bare(name: &str) -> Self {
        let outer = Path::new(env!("CARGO_TARGET_TMPDIR"))
            .join(format!("w51-a-adv-{name}-{}", std::process::id()));
        if outer.exists() {
            fs::remove_dir_all(&outer).expect("a stale scratch tree is removed");
        }
        let world = Self(outer);
        for dir in [world.repo(), world.home(), world.bin(), world.config_dir()] {
            fs::create_dir_all(dir).expect("a directory");
        }
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

    /// The world of `run_config_harness_section.rs`: one commit holding `README.md`, and each
    /// of `extra` beside it.
    fn new(name: &str, extra: &[(&str, &[u8])]) -> Self {
        let world = Self::bare(name);
        fs::write(world.repo().join("README.md"), "a story\n").expect("a file");
        for (path, bytes) in extra {
            let at = world.repo().join(path);
            fs::create_dir_all(at.parent().expect("a parent")).expect("a directory");
            fs::write(at, bytes).expect("a file");
        }
        git(&world.repo(), &["init", "-q", "-b", "main"]);
        git(&world.repo(), &["add", "-A"]);
        git(&world.repo(), &["commit", "-q", "-m", "start"]);
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

    /// Whether any case directory holds a `case.yaml`.
    fn a_case_opened(&self) -> bool {
        fs::read_dir(self.cases()).is_ok_and(|dir| {
            dir.filter_map(Result::ok)
                .any(|entry| entry.path().join("case.yaml").is_file())
        })
    }

    fn file(&self, name: &str, text: &str) -> PathBuf {
        let path = self.config_dir().join(name);
        fs::write(&path, text).expect("a file beside the run file");
        path
    }

    fn fresh(&self) {
        if self.cases().exists() {
            fs::remove_dir_all(self.cases()).expect("an earlier case is removed");
        }
        for program in ["claude", "codex"] {
            let _ = fs::remove_file(self.started(program));
        }
        git(&self.repo(), &["worktree", "prune"]);
        git(&self.repo(), &["checkout", "-q", "-f", "main"]);
        let branches = git(&self.repo(), &["branch", "--format=%(refname:short)"]);
        for branch in branches
            .lines()
            .filter(|branch| branch.starts_with("takt/"))
        {
            git(&self.repo(), &["branch", "-D", branch]);
        }
    }

    /// `takt` with `args`, HOME the scratch one and the doubles first on the program path.
    fn takt(&self, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_takt"))
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
            .env("XDG_STATE_HOME", self.0.join("xdg-state"))
            .env_remove("DBUS_SESSION_BUS_ADDRESS")
            .env_remove("XDG_RUNTIME_DIR")
            .env_remove("ANTHROPIC_API_KEY")
            .output()
            .expect("takt runs")
    }

    /// `takt run` on the repository with an intent and `--config <config>`, `flags` after them.
    fn run(&self, config: &Path, flags: &[&str]) -> Output {
        self.fresh();
        let repo = self.repo();
        let mut args = vec![
            "run",
            "--repo",
            repo.to_str().expect("utf-8"),
            "--intent",
            "probe",
            "--config",
            config.to_str().expect("utf-8"),
        ];
        args.extend_from_slice(flags);
        self.takt(&args)
    }

    fn argv(&self, program: &str) -> Option<Vec<String>> {
        fs::read_to_string(self.started(program))
            .ok()
            .map(|text| text.lines().map(str::to_owned).collect())
    }

    /// The value after `flag` in the argv `program` was started with.
    fn passed(&self, program: &str, flag: &str, said: &str) -> String {
        let argv = self
            .argv(program)
            .unwrap_or_else(|| panic!("{program} was not started: {said}"));
        let at = argv
            .iter()
            .position(|arg| arg == flag)
            .unwrap_or_else(|| panic!("no {flag} in {argv:?}"));
        argv.get(at + 1).cloned().unwrap_or_default()
    }

    /// Asserts the run was refused before any case opened and no harness was started.
    fn assert_refused_before_any_case(&self, output: &Output) -> String {
        let said = text(output);
        assert!(
            !self.a_case_opened(),
            "no case may open for a session request that cannot be started: {said}"
        );
        assert_eq!(output.status.code(), Some(1), "{said}");
        assert!(
            self.argv("claude").is_none() && self.argv("codex").is_none(),
            "no harness was started: {said}"
        );
        said
    }
}

/// The documented floor of `max-budget-usd` is one micro-dollar. Written as a decimal string it
/// reaches the session; written as the same YAML number it must too, since the documentation
/// admits "a number or a decimal string".
#[test]
fn a_one_micro_dollar_budget_written_as_a_number_reaches_the_session() {
    let world = World::new("budget-floor", &[]);
    let config = world.file(
        "string.yaml",
        &run_file("harness: {kind: claude, max-budget-usd: \"0.000001\"}\n"),
    );
    let said = text(&world.run(&config, &[]));
    assert_eq!(
        world.passed("claude", "--max-budget-usd", &said),
        "0.000001",
        "the string form"
    );
    for (written, passed) in [("0.000001", "0.000001"), ("0.000009", "0.000009")] {
        let config = world.file(
            "number.yaml",
            &run_file(&format!(
                "harness: {{kind: claude, max-budget-usd: {written}}}\n"
            )),
        );
        let said = text(&world.run(&config, &[]));
        assert_eq!(
            world.passed("claude", "--max-budget-usd", &said),
            passed,
            "the number {written}: {said}"
        );
    }
}

/// A flag wins over the same key in the file, for each of the keys this unit carries.
#[test]
fn a_flag_wins_over_the_files_budget_and_effort() {
    let world = World::new("flag-wins", &[]);
    let config = world.file(
        "run.yaml",
        &run_file("harness: {kind: claude, max-budget-usd: 1, effort: low}\n"),
    );
    let said = text(&world.run(&config, &["--max-budget-usd", "2.5", "--effort", "high"]));
    assert_eq!(world.passed("claude", "--max-budget-usd", &said), "2.5");
    assert_eq!(world.passed("claude", "--effort", &said), "high");
}

/// A file setting the run's harness does not take, once `--harness` changed the kind, is
/// refused naming the file and the flag that changed the kind.
#[test]
fn a_kind_changed_by_the_flag_refuses_the_files_setting_naming_the_file() {
    let world = World::new("kind-change", &[]);
    let config = world.file(
        "run.yaml",
        &run_file("harness: {kind: claude, max-budget-usd: 2}\n"),
    );
    let output = world.run(&config, &["--harness", "codex"]);
    let said = world.assert_refused_before_any_case(&output);
    assert!(said.contains(&config.display().to_string()), "{said}");
    assert!(said.contains("--harness codex"), "{said}");
}

/// A refusal the flags cause names the flag metaharness refuses, not every session flag given:
/// `--effort` is one Codex takes, so naming it as refused under codex misleads.
#[test]
fn a_refusal_under_codex_names_only_the_flag_codex_does_not_take() {
    let world = World::new("names-culprit", &[]);
    let config = world.file("run.yaml", &run_file(""));
    let output = world.run(
        &config,
        &[
            "--harness",
            "codex",
            "--effort",
            "high",
            "--max-budget-usd",
            "2",
        ],
    );
    let said = world.assert_refused_before_any_case(&output);
    assert!(said.contains("--max-budget-usd"), "{said}");
    assert!(
        !said.contains("--effort"),
        "--effort is taken by codex and is not refused: {said}"
    );
}

/// A tracked text file larger than metaharness's 64 KiB context cap passes takt's check of
/// `context` and is refused only once the case opened, when metaharness reads it at launch. The
/// decision is "refused before any case opens, naming it".
#[test]
fn a_tracked_context_file_over_the_byte_cap_is_refused_before_any_case_opens() {
    let big = "a line of the long file\n".repeat(4000);
    let world = World::new("context-big", &[("docs/long.md", big.as_bytes())]);
    let config = world.file(
        "run.yaml",
        &run_file("harness: {kind: claude, context: [docs/long.md]}\n"),
    );
    let output = world.run(&config, &[]);
    let said = world.assert_refused_before_any_case(&output);
    assert!(said.contains("docs/long.md"), "{said}");
}

/// A tracked file that is not text (it holds a NUL) passes takt's check of `context` and is
/// refused only once the case opened.
#[test]
fn a_tracked_context_file_that_is_not_text_is_refused_before_any_case_opens() {
    let world = World::new("context-binary", &[("logo.bin", b"\x89PNG\0\0\0\rIHDR")]);
    let config = world.file("run.yaml", &run_file(""));
    let output = world.run(&config, &["--context", "logo.bin"]);
    let said = world.assert_refused_before_any_case(&output);
    assert!(said.contains("logo.bin"), "{said}");
}

/// A tracked symbolic link, a tracked file under a linked directory's name, a `.git` part and an
/// untracked file are each refused before any case opens, naming the path.
#[test]
fn context_paths_the_copy_does_not_hold_are_refused_naming_them() {
    let world = World::new("context-shapes", &[("docs/guide.md", b"a guide\n")]);
    std::os::unix::fs::symlink("README.md", world.repo().join("link.md")).expect("a link");
    std::os::unix::fs::symlink("docs", world.repo().join("linked")).expect("a link");
    git(&world.repo(), &["add", "link.md", "linked"]);
    git(&world.repo(), &["commit", "-q", "-m", "links"]);
    fs::write(world.repo().join("untracked.md"), "loose\n").expect("a file");
    let config = world.file("run.yaml", &run_file(""));
    for path in [
        "link.md",
        "linked/guide.md",
        "untracked.md",
        ".git/description",
        "docs//guide.md",
        "docs/guide.md/",
    ] {
        let output = world.run(&config, &["--context", path]);
        let said = world.assert_refused_before_any_case(&output);
        assert!(said.contains(path), "{path}: {said}");
    }
}

/// `takt scenario run` checks `context` against the repository as it stands, then cuts the case
/// from the scenario's seed. A file the repository tracks now and the seed does not is accepted
/// before anything opens and refused by Loom once the case's tree holds the seed: the case opens
/// for a session that can never start.
#[test]
fn scenario_run_checks_context_against_the_tree_the_case_is_cut_from() {
    let world = World::bare("scenario-seed");
    let repo = world.repo();
    copy_dir(&Path::new(FIXTURES).join("one-ready"), &repo);
    copy_dir(&Path::new(FIXTURES).join("scenario-1/base"), &repo);
    for script in ["check.sh", "todo.py"] {
        fs::set_permissions(repo.join(script), fs::Permissions::from_mode(0o755))
            .expect("executable");
    }
    fs::write(repo.join("Cargo.toml"), "[package]\nname = \"scratch\"\n").expect("a file");
    fs::write(
        repo.join(".engineering/gates.yaml"),
        "format: engineering-gates/1\nlanguage: canon-expr/1\ngates:\n  - id: manifest\n    \
         text: The repository has its manifest.\n    assertions:\n      \
         - 'fs.file.exists(\"Cargo.toml\")'\n",
    )
    .expect("a file");
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
    fs::write(repo.join("later.md"), "written after the seed\n").expect("a file");
    git(&repo, &["add", "-A"]);
    git(
        &repo,
        &[
            "commit",
            "-q",
            "-m",
            "declare the scenario and a later file",
        ],
    );
    let output = world.takt(&[
        "scenario",
        "run",
        "--repo",
        repo.to_str().expect("utf-8"),
        "1",
        "--test-cmd",
        "./check.sh",
        "--forge",
        "none",
        "--commit-style",
        "action",
        "--iterations",
        "4",
        "--context",
        "later.md",
    ]);
    let said = text(&output);
    assert!(
        !world.a_case_opened(),
        "the seed does not hold later.md, so the run is refused before any case opens: {said}"
    );
    assert!(said.contains("later.md"), "{said}");
}
