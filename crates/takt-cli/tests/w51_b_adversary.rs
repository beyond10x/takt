//! Adversary pass on wave 51 unit b (story:harness-flags-spell-metaharness-names) and second pass
//! on unit a's correction (story:harness-section-is-metaharness-session-request): the session's
//! `context` files checked before any case opens against the tree the session's copy is made of,
//! within metaharness's caps, and the help text of the five flags. Each case drives `takt` with
//! harness doubles on the program path that leave their argv behind; nothing reaches a network
//! or a model.

#[path = "../../takt/tests/support/own_home.rs"]
mod own_home;

use std::fs;
use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

/// The fixture stores of the library crate, one directory each.
const FIXTURES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../takt/tests/fixtures");

/// A gates document whose one assertion holds in a repository with a manifest.
const GATES: &str = "format: engineering-gates/1\nlanguage: canon-expr/1\ngates:\n  - id: manifest\n    \
                     text: The repository has its manifest.\n    assertions:\n      \
                     - 'fs.file.exists(\"Cargo.toml\")'\n";

/// metaharness 0.23.0's cap on the context files of one session (`CONTEXT_BYTES_MAX`), each
/// costing its bytes and its path rendered as a JSON string (`context_cost`).
const CAP: usize = 65_536;

/// Linux's limit on one argument string (`ARGUMENT_BYTES_MAX` of metaharness 0.23.0).
const ARGUMENT_MAX: usize = 131_072;

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

/// `len` bytes of plain prose that starts with `marker`: UTF-8, no NUL, nothing of a
/// credential's shape.
fn prose(marker: &str, len: usize) -> String {
    let mut text = format!("{marker}\n");
    while text.len() < len {
        text.push_str("lorem ipsum dolor sit amet consectetur adipiscing elit\n");
    }
    text.truncate(len);
    text
}

/// What one file costs against [`CAP`]: its bytes and its path as a JSON string.
fn cost(path: &str, len: usize) -> usize {
    path.len() + 2 + len
}

/// A scratch world under this crate's build directory, removed on drop: a repository, an empty
/// HOME whose `.local/bin` holds the codex double, a program directory holding the claude
/// double, and a config directory. Each double answers `--version` with `2.1.309`, writes its
/// argv one argument a line when started for a session, and exits 1.
struct World(PathBuf);

impl Drop for World {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

impl World {
    fn bare(name: &str) -> Self {
        let outer = Path::new(env!("CARGO_TARGET_TMPDIR"))
            .join(format!("w51-b-adv-{name}-{}", std::process::id()));
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

    /// One commit on `main` holding `README.md` and each of `files`.
    fn new(name: &str, files: &[(&str, &str)]) -> Self {
        let world = Self::bare(name);
        fs::write(world.repo().join("README.md"), "a story\n").expect("a file");
        world.write(files);
        git(&world.repo(), &["init", "-q", "-b", "main"]);
        git(&world.repo(), &["add", "-A"]);
        git(&world.repo(), &["commit", "-q", "-m", "start"]);
        world
    }

    /// The world of a scenario run: the seed holds the fixture store with `story:ready-one`
    /// `proposed`, a manifest, a gates document that holds for it and `seed_files`; `main`
    /// moves on past it with the declaration of scenario `1`, and `removed` taken out of it.
    fn scenario(name: &str, seed_files: &[(&str, &str)], removed: &[&str]) -> Self {
        let world = Self::bare(name);
        let repo = world.repo();
        copy_dir(&Path::new(FIXTURES).join("one-ready"), &repo);
        fs::write(repo.join("Cargo.toml"), "[package]\nname = \"scratch\"\n").expect("a file");
        fs::write(repo.join(".engineering/gates.yaml"), GATES).expect("a file");
        world.write(seed_files);
        git(&repo, &["init", "-q", "-b", "main"]);
        git(&repo, &["add", "-A"]);
        git(&repo, &["commit", "-q", "-m", "seed"]);
        let seed = git(&repo, &["rev-parse", "HEAD"]);
        fs::write(
            repo.join(".engineering/scenarios.conf"),
            format!("[scenario \"1\"]\n\tstory = story:ready-one\n\tseed = {seed}\n"),
        )
        .expect("the declaration");
        for path in removed {
            git(&repo, &["rm", "-q", path]);
        }
        git(&repo, &["add", "-A"]);
        git(&repo, &["commit", "-q", "-m", "declare the scenario"]);
        world
    }

    fn write(&self, files: &[(&str, &str)]) {
        for (path, text) in files {
            let at = self.repo().join(path);
            fs::create_dir_all(at.parent().expect("a parent")).expect("a directory");
            fs::write(at, text).expect("a file");
        }
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

    /// `takt run` with `intent` and `--config <config>`, `flags` after them.
    fn run(&self, intent: &str, config: &Path, flags: &[&str]) -> Output {
        let repo = self.repo();
        let mut args = vec![
            "run",
            "--repo",
            repo.to_str().expect("utf-8"),
            "--intent",
            intent,
            "--config",
            config.to_str().expect("utf-8"),
        ];
        args.extend_from_slice(flags);
        self.takt(&args)
    }

    /// `takt scenario run` of scenario `1` with `--config <config>`, no forge and four steps,
    /// enough to reach the first session step; `flags` after them.
    fn scenario_run(&self, config: &Path, flags: &[&str]) -> Output {
        let repo = self.repo();
        let mut args = vec![
            "scenario",
            "run",
            "--repo",
            repo.to_str().expect("utf-8"),
            "1",
            "--config",
            config.to_str().expect("utf-8"),
            "--test-cmd",
            "true",
            "--forge",
            "none",
            "--iterations",
            "4",
        ];
        args.extend_from_slice(flags);
        self.takt(&args)
    }

    fn argv(&self, program: &str) -> Option<Vec<String>> {
        fs::read_to_string(self.started(program))
            .ok()
            .map(|text| text.lines().map(str::to_owned).collect())
    }

    /// Whether `program`'s double was started with `marker` somewhere in its argv.
    fn handed(&self, program: &str, marker: &str) -> bool {
        self.argv(program)
            .is_some_and(|argv| argv.iter().any(|arg| arg.contains(marker)))
    }
}

/// The correction's contract (`SessionRequest::check_context`): a context file the session
/// cannot be handed is refused before any case opens, as metaharness would check it at launch.
/// Under Codex, metaharness puts the context files into the one prompt argument, beside the
/// instruction and scope files takt has already fitted into that argument without counting
/// them. A context list within the byte cap beside a scope file within takt's own scope budget
/// gives an argument past Linux's limit: the case opens and the session is refused at its step.
/// Each half alone starts the codex double.
#[test]
fn codex_context_beside_a_scope_file_is_started_or_refused_before_any_case_opens() {
    let scope = prose("MARKER-scope", 65_400);
    let context = prose("MARKER-context", CAP - cost("ctx.md", 0));
    assert!(scope.len() + context.len() > ARGUMENT_MAX - 400);
    let intent = "probe\n\n## Scope\n\n- `big.md`\n";
    let files = [("big.md", scope.as_str()), ("ctx.md", context.as_str())];

    let alone = World::new("codex-scope-alone", &files);
    let config = alone.file("run.yaml", &run_file(""));
    let output = alone.run(intent, &config, &["--harness", "codex"]);
    assert!(
        alone.handed("codex", "MARKER-scope"),
        "the scope file alone reaches the codex session: {}",
        text(&output)
    );

    let both = World::new("codex-scope-and-context", &files);
    let config = both.file("run.yaml", &run_file(""));
    let output = both.run(
        intent,
        &config,
        &["--harness", "codex", "--context", "ctx.md"],
    );
    let said = text(&output);
    let started = both.argv("codex").is_some();
    assert!(
        started || !both.a_case_opened(),
        "a context list takt admitted opened a case whose codex session was never started: {said}"
    );
}

/// The correction checks `takt run`'s context against the work tree as it is when the command
/// is read. `--case` resumes a case on its own branch, which it checks out after that check: a
/// file the branch the run starts from tracks and the case's branch does not is admitted, and
/// the session's copy, made of the case's branch, cannot hold it.
#[test]
fn a_resumed_cases_context_is_checked_against_the_cases_branch() {
    let world = World::new("resume", &[]);
    let config = world.file("run.yaml", &run_file(""));
    let first = world.run("probe", &config, &["--case-id", "probe"]);
    assert!(
        world.a_case_opened(),
        "the first run opened a case: {}",
        text(&first)
    );
    let _ = fs::remove_file(world.started("claude"));
    git(&world.repo(), &["checkout", "-q", "main"]);
    world.write(&[(
        "later.md",
        "MARKER-later: a note main has and the case does not\n",
    )]);
    git(&world.repo(), &["add", "later.md"]);
    git(&world.repo(), &["commit", "-q", "-m", "a later note"]);
    let repo = world.repo();
    let output = world.takt(&[
        "run",
        "--repo",
        repo.to_str().expect("utf-8"),
        "--case",
        "probe",
        "--config",
        config.to_str().expect("utf-8"),
        "--context",
        "later.md",
    ]);
    let said = text(&output);
    assert!(
        world.handed("claude", "MARKER-later")
            || said.contains("the command line's --context is refused"),
        "the resumed case's session could not be handed later.md, and takt did not refuse it before the case resumed: {said}"
    );
}

/// The correction's byte cap is metaharness's, to the byte: a context list costing exactly the
/// cap reaches the session whole, one byte more is refused before any case opens, naming the
/// file that crosses it.
#[test]
fn a_context_list_at_the_cap_reaches_the_session_and_one_byte_past_it_is_refused() {
    let first = prose("MARKER-first", 30_000);
    let at_cap = prose(
        "MARKER-at-cap",
        CAP - cost("a.md", first.len()) - cost("b.md", 0),
    );
    let past = prose("MARKER-past", at_cap.len() + 1);
    let files = [
        ("a.md", first.as_str()),
        ("b.md", at_cap.as_str()),
        ("c.md", past.as_str()),
    ];

    let fits = World::new("cap-fits", &files);
    let config = fits.file("run.yaml", &run_file(""));
    let output = fits.run(
        "probe",
        &config,
        &["--context", "a.md", "--context", "b.md"],
    );
    assert!(
        fits.handed("claude", "MARKER-first") && fits.handed("claude", "MARKER-at-cap"),
        "a list costing exactly {CAP} bytes reaches the session: {}",
        text(&output)
    );

    let over = World::new("cap-over", &files);
    let config = over.file("run.yaml", &run_file(""));
    let output = over.run(
        "probe",
        &config,
        &["--context", "a.md", "--context", "c.md"],
    );
    let said = text(&output);
    assert_eq!(output.status.code(), Some(1), "{said}");
    assert!(!over.a_case_opened(), "no case opened: {said}");
    assert!(over.argv("claude").is_none(), "no session started: {said}");
    assert!(
        said.contains("the command line's --context is refused") && said.contains("`c.md`"),
        "the refusal names the file past the cap: {said}"
    );
}

/// The reverse of the first pass's seed case: `takt scenario run` hands the session a context
/// file the seed holds, though the repository's current index no longer tracks it.
#[test]
fn scenario_run_hands_a_context_file_the_seed_holds_and_head_does_not() {
    let world = World::scenario(
        "seed-only",
        &[("seed-only.md", "MARKER-seed-only: the seed's note\n")],
        &["seed-only.md"],
    );
    let config = world.file("run.yaml", "scan: none\n");
    let output = world.scenario_run(&config, &["--context", "seed-only.md"]);
    assert!(
        world.handed("claude", "MARKER-seed-only"),
        "the seed's file reaches the session: {}",
        text(&output)
    );
}

/// `--context` wins over the file's `context` on `takt scenario run`: a file's list naming a path
/// the seed does not hold is replaced by the flag's, which reaches the session, and is not
/// checked.
#[test]
fn scenario_run_context_flag_replaces_the_files_list() {
    let world = World::scenario(
        "flag-over-file",
        &[("notes.md", "MARKER-notes: the flag's note\n")],
        &[],
    );
    let config = world.file(
        "run.yaml",
        "scan: none\nharness: {kind: claude, context: [absent.md]}\n",
    );
    let output = world.scenario_run(&config, &["--context", "notes.md"]);
    assert!(
        world.handed("claude", "MARKER-notes"),
        "the flag's file reaches the session: {}",
        text(&output)
    );
}

/// `takt scenario run --help` describes `--context` as a file the repository's index tracks;
/// since the correction a scenario run checks it against the seed and refuses a file the index
/// tracks and the seed does not. The behaviour is asserted first, then the help's wording.
#[test]
fn scenario_run_help_describes_context_as_the_run_checks_it() {
    let world = World::scenario("help-drift", &[], &[]);
    world.write(&[("later.md", "MARKER-later\n")]);
    git(&world.repo(), &["add", "later.md"]);
    git(&world.repo(), &["commit", "-q", "-m", "a later note"]);
    let config = world.file("run.yaml", "scan: none\n");
    let output = world.scenario_run(&config, &["--context", "later.md"]);
    let said = text(&output);
    assert_eq!(output.status.code(), Some(1), "{said}");
    assert!(
        said.contains("the command line's --context is refused") && said.contains("later.md"),
        "a file the index tracks and the seed does not is refused: {said}"
    );
    let help = world.takt(&["scenario", "run", "--help"]);
    let help = text(&help).split_whitespace().collect::<Vec<_>>().join(" ");
    assert!(
        !help.contains("a file the repository's index tracks"),
        "`takt scenario run --help` promises --context takes a file the repository's index tracks, which it refuses: {help}"
    );
}
