//! story:takt-commits-with-the-repositorys-identity through the binary: every commit `takt run`
//! makes carries the identity `.engineering/commit-identity.conf` names at the base, or the one
//! `--commit-identity` gives, which wins, as author and committer, whatever the operator's git
//! configuration or the `GIT_AUTHOR_*` and `GIT_COMMITTER_*` variables in takt's own environment
//! say. With none configured a run that pushes nothing commits as `takt <takt@localhost>`; a run
//! whose forge pushes is refused before its case opens, naming the setting. A resume commits
//! with the identity its case was opened with. The harness and the command forge's program are
//! doubles on the program path; nothing reaches a network or a model.

#[path = "../../takt/tests/support/own_home.rs"]
mod own_home;

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

/// The fixture stores of the library crate, one directory each.
const FIXTURES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../takt/tests/fixtures");

/// A gates document whose one assertion is true in a repository with a manifest, so the gate
/// has a performer and the harness is reached.
const GATES: &str = "format: engineering-gates/1\nlanguage: canon-expr/1\ngates:\n  - id: manifest\n    \
                     text: The repository has its manifest.\n    assertions:\n      \
                     - 'fs.file.exists(\"Cargo.toml\")'\n";

/// Where a repository configures the identity takt commits with.
const IDENTITY_FILE: &str = ".engineering/commit-identity.conf";

/// The identity the repository configures in these cases, its name and its email.
const CONFIGURED: (&str, &str) = ("Repository Bot", "repository-bot@example.invalid");

/// The program every entry of the command forge runs: a double that logs each call and fails.
const FORGE_PROGRAM: &str = "forge-double";

/// The operator's own identity, in every place git's configuration can give one.
const OPERATOR_CONFIG: [(&str, &str); 6] = [
    ("user.name", "Operator"),
    ("user.email", "operator@example.invalid"),
    ("author.name", "Operator Author"),
    ("author.email", "author@example.invalid"),
    ("committer.name", "Operator Committer"),
    ("committer.email", "committer@example.invalid"),
];

/// The identity variables in takt's own environment, which git would take over any
/// configuration.
const OPERATOR_ENV: [(&str, &str); 4] = [
    ("GIT_AUTHOR_NAME", "Env Author"),
    ("GIT_AUTHOR_EMAIL", "env-author@example.invalid"),
    ("GIT_COMMITTER_NAME", "Env Committer"),
    ("GIT_COMMITTER_EMAIL", "env-committer@example.invalid"),
];

fn git(tree: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .args(["-c", "user.name=t", "-c", "user.email=t@localhost"])
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

/// A scratch directory under this crate's build directory, removed on drop.
struct Scratch(PathBuf);

impl std::ops::Deref for Scratch {
    type Target = Path;

    fn deref(&self) -> &Path {
        &self.0
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        if let Err(error) = fs::remove_dir_all(&self.0) {
            let message = format!("remove the scratch tree {}: {error}", self.0.display());
            if std::thread::panicking() {
                eprintln!("{message}");
            } else {
                panic!("{message}");
            }
        }
    }
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

/// One scratch world: `repo`, a committed copy of the `one-ready` store with a manifest, a gates
/// document, `identity` (the file's text) as `.engineering/commit-identity.conf` when given, and
/// the operator's identity in its configuration; and `bin`, a `claude` double that appends to
/// `work.txt` and answers `implementation.edit`, and [`FORGE_PROGRAM`], which logs each call
/// and fails.
struct World {
    _scratch: Scratch,
    repo: PathBuf,
    bin: PathBuf,
    /// Where the forge double logs each call.
    calls: PathBuf,
}

fn world(name: &str, identity: Option<&str>) -> World {
    let outer = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("commit-identity-cli-{}-{name}", std::process::id()));
    if outer.exists() {
        fs::remove_dir_all(&outer).expect("a stale scratch tree is removed");
    }
    let repo = outer.join("repo");
    copy_dir(&Path::new(FIXTURES).join("one-ready"), &repo);
    fs::write(repo.join("check.txt"), "broken\n").expect("a file");
    fs::write(repo.join("Cargo.toml"), "[package]\nname = \"scratch\"\n").expect("a file");
    fs::write(repo.join(".engineering/gates.yaml"), GATES).expect("a file");
    if let Some(text) = identity {
        fs::write(repo.join(IDENTITY_FILE), text).expect("a file");
    }
    git(&repo, &["init", "-q", "-b", "main"]);
    git(&repo, &["add", "-A"]);
    git(&repo, &["commit", "-q", "-m", "start"]);
    for (key, value) in OPERATOR_CONFIG {
        git(&repo, &["config", key, value]);
    }
    let bin = outer.join("stub-bin");
    fs::create_dir_all(&bin).expect("a stub directory");
    fs::write(
        bin.join("claude"),
        "#!/bin/sh\ncase $1 in --version) echo '2.1.309 (Claude Code)'; exit 0 ;; esac\necho \"the work\" >> work.txt\nprintf '{\"result\":\"{\\\\\"action\\\\\": \\\\\"implementation.edit\\\\\", \\\\\"summary\\\\\": \\\\\"the double\\\\\"}\",\"usage\":{\"input_tokens\":1}}'\n",
    )
    .expect("the double is written");
    // The command forge's program: each call logged and failed, so nothing reaches a network.
    let calls = outer.join("forge-calls");
    let forge = format!(
        "#!/bin/sh\necho \"$0 $*\" >> '{}'\necho 'the forge double refuses' >&2\nexit 1\n",
        calls.display()
    );
    fs::write(bin.join(FORGE_PROGRAM), &forge).expect("the double is written");
    #[cfg(unix)]
    for name in ["claude", FORGE_PROGRAM] {
        use std::os::unix::fs::PermissionsExt as _;
        fs::set_permissions(bin.join(name), fs::Permissions::from_mode(0o755)).expect("executable");
    }
    World {
        _scratch: Scratch(outer),
        repo,
        bin,
        calls,
    }
}

/// `[identity]` naming `name` and `email`.
fn identity_file(name: &str, email: &str) -> String {
    format!("[identity]\n\tname = {name}\n\temail = {email}\n")
}

/// The configured identity as `.engineering/commit-identity.conf` gives it.
fn configured_file() -> String {
    identity_file(CONFIGURED.0, CONFIGURED.1)
}

/// The configured identity as a commit carries it, `<name> <email>`.
fn configured() -> String {
    format!("{} <{}>", CONFIGURED.0, CONFIGURED.1)
}

impl World {
    /// The built `takt` with `args`, the doubles first on the program path and the operator's
    /// identity variables ([`OPERATOR_ENV`]) in its environment: the exit code, stdout, stderr.
    fn takt(&self, args: &[&str]) -> (i32, String, String) {
        let output = Command::new(env!("CARGO_BIN_EXE_takt"))
            .args(args)
            .envs(OPERATOR_ENV)
            .env("XDG_STATE_HOME", self.repo.with_file_name("xdg-state"))
            .env(
                "PATH",
                format!(
                    "{}:{}",
                    self.bin.display(),
                    std::env::var("PATH").unwrap_or_default()
                ),
            )
            .output()
            .expect("takt runs");
        (
            output.status.code().expect("an exit code"),
            String::from_utf8_lossy(&output.stdout).into_owned(),
            String::from_utf8_lossy(&output.stderr).into_owned(),
        )
    }

    /// `takt run --repo <repo> --test-cmd true`, `extra` after it, through [`Self::takt`].
    fn run(&self, extra: &[&str]) -> (i32, String, String) {
        self.takt(&self.run_args(extra))
    }

    /// A run file beside the repository for `forge`, `--config` and its path: `command` runs
    /// [`FORGE_PROGRAM`] for each entry; `local` runs `true` as its checks, its hub the bare
    /// `origin` [`Self::with_bare_origin`] adds. Both declare `scan: none`, so a forge that
    /// publishes is not refused for want of a scan adapter.
    fn forge_config(&self, forge: &str) -> [String; 2] {
        let text = match forge {
            "command" => format!(
                "forge: command\nscan: none\nforge_commands:\n  open: [{FORGE_PROGRAM}, open]\n  read: [{FORGE_PROGRAM}, read, \"{{number}}\"]\n  checks: [{FORGE_PROGRAM}, checks]\n  merge: [{FORGE_PROGRAM}, merge]\n  id_pattern: '(\\d+)'\n"
            ),
            "local" => "forge: local\nscan: none\nlocal_checks: [\"true\"]\n".to_owned(),
            other => panic!("no run file for the forge {other}"),
        };
        let path = self.repo.with_file_name(format!("forge-{forge}.yaml"));
        fs::write(&path, text).expect("the run file is written");
        ["--config".to_owned(), path.display().to_string()]
    }

    /// A bare `origin` beside the repository, the local forge's hub and the remote a scenario
    /// run pushes its trial base to.
    fn with_bare_origin(&self) -> PathBuf {
        let remote = self.repo.with_file_name("remote.git");
        git(
            self.repo.parent().expect("the scratch directory"),
            &["init", "-q", "--bare", "remote.git"],
        );
        git(
            &self.repo,
            &["remote", "add", "origin", remote.to_str().expect("utf-8")],
        );
        remote
    }

    fn run_args<'a>(&'a self, extra: &[&'a str]) -> Vec<&'a str> {
        let repo = self.repo.to_str().expect("utf-8");
        let mut args = vec!["run", "--repo", repo, "--test-cmd", "true"];
        if !extra.contains(&"--protocol") {
            args.extend(["--protocol", "story.delivery/1"]);
        }
        args.extend_from_slice(extra);
        args
    }

    /// Each commit since `main`, newest first, as `<subject>|<author>|<committer>`, each side
    /// `<name> <email>`.
    fn commits(&self) -> Vec<String> {
        git(
            &self.repo,
            &["log", "--format=%s|%an <%ae>|%cn <%ce>", "main..HEAD", "--"],
        )
        .lines()
        .map(str::to_owned)
        .collect()
    }

    /// The calls the forge double was given, one a line.
    fn forge_calls(&self) -> String {
        fs::read_to_string(&self.calls).unwrap_or_default()
    }

    /// Asserts there are `count` commits since `main`, each by `who` on both sides.
    fn all_by(&self, who: &str, count: usize, said: &str) {
        let commits = self.commits();
        assert_eq!(commits.len(), count, "{commits:#?}\n{said}");
        for commit in &commits {
            let (_, sides) = commit.split_once('|').expect("a subject");
            assert_eq!(sides, format!("{who}|{who}"), "{commits:#?}\n{said}");
        }
    }
}

/// Acceptance 2, where nothing leaves the machine: no identity configured and `--forge none`,
/// the take commits as takt's own `takt <takt@localhost>` on both sides, though takt's
/// environment sets `GIT_AUTHOR_*` and `GIT_COMMITTER_*` and the repository configures the
/// operator's `user.*`, `author.*` and `committer.*`.
#[test]
fn with_no_identity_and_no_forge_commits_are_takts_own_on_both_sides() {
    let world = world("unconfigured", None);
    let (_, out, err) = world.run(&["--harness", "none", "--forge", "none", "--iterations", "1"]);
    let said = format!("stdout {out} stderr {err}");
    assert!(out.contains("frontier: took story:ready-one"), "{said}");
    world.all_by("takt <takt@localhost>", 1, &said);
}

/// Acceptance 1: with `.engineering/commit-identity.conf` at the base, the take and the
/// harness's edit both carry it as author and committer, over takt's environment and the
/// operator's configuration.
#[test]
fn the_configured_identity_carries_every_commit_over_the_environment_and_the_config() {
    let world = world("configured", Some(&configured_file()));
    let (_, out, err) = world.run(&[
        "--harness",
        "claude",
        "--forge",
        "none",
        "--iterations",
        "3",
    ]);
    let said = format!("stdout {out} stderr {err}");
    assert!(out.contains("frontier: took story:ready-one"), "{said}");
    world.all_by(&configured(), 2, &said);
}

/// `--commit-identity` wins over the file.
#[test]
fn the_flag_wins_over_the_file() {
    let world = world("flag", Some(&configured_file()));
    let (_, out, err) = world.run(&[
        "--harness",
        "claude",
        "--forge",
        "none",
        "--iterations",
        "3",
        "--commit-identity",
        "Release Bot <release@example.invalid>",
    ]);
    let said = format!("stdout {out} stderr {err}");
    assert!(out.contains("frontier: took story:ready-one"), "{said}");
    world.all_by("Release Bot <release@example.invalid>", 2, &said);
}

/// A resume commits with the identity its case was opened with: opened under the flag, which
/// names another identity than the file, it is resumed without one and the harness's edit
/// carries the flag's. A flag naming another identity for the resumed case is refused, nothing
/// resumed.
#[test]
fn a_resume_commits_with_the_identity_the_case_was_opened_with() {
    let world = world("resume", Some(&configured_file()));
    let given = "Release Bot <release@example.invalid>";
    let (_, out, err) = world.run(&[
        "--harness",
        "none",
        "--forge",
        "none",
        "--iterations",
        "0",
        "--case-id",
        "resumed",
        "--commit-identity",
        given,
    ]);
    let said = format!("stdout {out} stderr {err}");
    assert!(out.contains("frontier: took story:ready-one"), "{said}");
    world.all_by(given, 1, &said);

    let (code, out, err) = world.run(&[
        "--case",
        "resumed",
        "--harness",
        "claude",
        "--forge",
        "none",
        "--iterations",
        "3",
        "--commit-identity",
        "Someone Else <else@example.invalid>",
    ]);
    let said = format!("stdout {out} stderr {err}");
    assert_eq!(code, 1, "{said}");
    assert!(err.contains("--commit-identity"), "{said}");
    assert!(err.contains(given), "{said}");
    world.all_by(given, 1, &said);

    let (_, out, err) = world.run(&[
        "--case",
        "resumed",
        "--harness",
        "claude",
        "--forge",
        "none",
        "--iterations",
        "3",
    ]);
    let said = format!("stdout {out} stderr {err}");
    world.all_by(given, 2, &said);
}

/// A case opened with no identity under `--forge none` commits as takt's own; a run that would
/// resume it through a forge that pushes (`forge: command`), by `--case` or as the open case the
/// frontier resumes, is refused before it resumes: exit 1, nothing committed, the forge never
/// called, and the refusal says how to get past the case: finish it with `--forge none`, or
/// drop its branch and its directory.
#[test]
fn a_case_opened_with_no_identity_is_not_resumed_through_a_forge_that_pushes() {
    let world = world("resume-pushing", None);
    let (_, out, err) = world.run(&[
        "--harness",
        "none",
        "--forge",
        "none",
        "--iterations",
        "0",
        "--case-id",
        "local",
    ]);
    let said = format!("stdout {out} stderr {err}");
    assert!(out.contains("frontier: took story:ready-one"), "{said}");
    world.all_by("takt <takt@localhost>", 1, &said);
    let config = world.forge_config("command");
    let head = git(&world.repo, &["rev-parse", "HEAD"]);
    for by_case in [true, false] {
        let mut args = vec!["--harness", "claude", "--iterations", "3"];
        args.extend(config.iter().map(String::as_str));
        if by_case {
            args.extend(["--case", "local"]);
        }
        let (code, out, err) = world.run(&args);
        let said = format!("--case given: {by_case}: stdout {out} stderr {err}");
        assert_eq!(code, 1, "{said}");
        for named in [
            "the case local was opened with none",
            "--forge none",
            ".takt/cases/local",
            "branch -D takt/local",
            IDENTITY_FILE,
            "--commit-identity",
        ] {
            assert!(err.contains(named), "{named}: {said}");
        }
        assert_eq!(git(&world.repo, &["rev-parse", "HEAD"]), head, "{said}");
        assert!(!world.repo.join("work.txt").exists(), "{said}");
        assert_eq!(world.forge_calls(), "", "{said}");
    }
}

/// Acceptance 2, where commits leave the machine: with no identity configured, a run whose
/// forge pushes (`forge: command`, and `forge: local` into its hub) is refused before its case
/// opens, naming the file, its keys and the flag: no case directory, no case branch, no commit,
/// the story not taken, the forge never called and nothing in the hub.
#[test]
fn a_pushing_run_with_no_identity_is_refused_before_its_case_opens() {
    let world = world("pushing", None);
    let remote = world.with_bare_origin();
    let start = git(&world.repo, &["rev-parse", "HEAD"]);
    for forge in ["command", "local"] {
        let config = world.forge_config(forge);
        let mut args = vec!["--harness", "claude", "--iterations", "3"];
        args.extend(config.iter().map(String::as_str));
        let (code, out, err) = world.run(&args);
        let said = format!("{forge}: stdout {out} stderr {err}");
        assert_eq!(code, 1, "{said}");
        for named in [
            IDENTITY_FILE,
            "identity.name",
            "identity.email",
            "--commit-identity",
        ] {
            assert!(err.contains(named), "{named}: {said}");
        }
        assert!(!world.repo.join(".takt/cases").exists(), "{said}");
        assert_eq!(git(&world.repo, &["rev-parse", "HEAD"]), start, "{said}");
        assert_eq!(
            git(&world.repo, &["branch", "--show-current"]),
            "main",
            "{said}"
        );
        assert_eq!(
            git(
                &world.repo,
                &["for-each-ref", "--format=%(refname)", "refs/heads/takt/"]
            ),
            "",
            "{said}"
        );
        assert_eq!(git(&world.repo, &["status", "--porcelain"]), "", "{said}");
        assert!(!world.repo.join("work.txt").exists(), "{said}");
        assert_eq!(world.forge_calls(), "", "{said}");
        assert_eq!(
            git(&remote, &["for-each-ref", "--format=%(refname)"]),
            "",
            "{said}"
        );
    }
}

/// The same for a scenario run through `--forge local`, whose trial base and pull request go to
/// `origin`: refused before anything is fetched, cut or pushed, naming the setting, when
/// `--declared-on` holds no identity and no flag is given.
#[test]
fn a_scenario_run_through_a_forge_with_no_identity_is_refused_before_anything_is_cut() {
    let world = world("scenario", None);
    let seed = git(&world.repo, &["rev-parse", "HEAD"]);
    fs::write(
        world.repo.join(".engineering/scenarios.conf"),
        format!("[scenario \"1\"]\n\tstory = story:ready-one\n\tseed = {seed}\n"),
    )
    .expect("a file");
    git(&world.repo, &["add", "-A"]);
    git(&world.repo, &["commit", "-q", "-m", "declare"]);
    let remote = world.with_bare_origin();
    let start = git(&world.repo, &["rev-parse", "HEAD"]);
    let repo = world.repo.to_str().expect("utf-8");
    let config = world.forge_config("local");
    let (code, out, err) = world.takt(&[
        "scenario",
        "run",
        "--repo",
        repo,
        "1",
        "--test-cmd",
        "true",
        "--harness",
        "claude",
        "--forge",
        "local",
        &config[0],
        &config[1],
    ]);
    let said = format!("stdout {out} stderr {err}");
    assert_eq!(code, 1, "{said}");
    for named in [
        IDENTITY_FILE,
        "identity.name",
        "identity.email",
        "--commit-identity",
    ] {
        assert!(err.contains(named), "{named}: {said}");
    }
    assert_eq!(git(&world.repo, &["rev-parse", "HEAD"]), start, "{said}");
    assert_eq!(
        git(
            &world.repo,
            &[
                "for-each-ref",
                "--format=%(refname)",
                "refs/heads/trial/",
                "refs/heads/takt/",
                "refs/remotes/"
            ]
        ),
        "",
        "{said}"
    );
    assert!(!world.repo.join(".takt/cases").exists(), "{said}");
    assert_eq!(
        git(&remote, &["for-each-ref", "--format=%(refname)"]),
        "",
        "{said}"
    );
}

// story:a-run-refuses-a-scanned-identity-at-open: a run whose commits leave the machine reads
// its commit identity through its forge's scan when it opens, before step 1. No forge takt
// ships scans an identity, so the run opens and says the identity was not scanned.

/// An identity given to a run through a forge.
const GIVEN: &str = "Release Bot <release@example.invalid>";

/// Through a forge that pushes, the run opens under the given identity, says once that it was
/// not scanned and why (`scan: none` declared in the run file), and calls the forge for nothing
/// at open; under `--forge none` nothing leaves the machine and no such line is said.
#[test]
fn an_identity_through_a_forge_opens_the_run_and_is_said_not_scanned() {
    let through = world("not-scanned", None);
    let config = through.forge_config("command");
    let (_, out, err) = through.run(&[
        "--harness",
        "none",
        "--iterations",
        "0",
        "--commit-identity",
        GIVEN,
        &config[0],
        &config[1],
    ]);
    let said = format!("stdout {out} stderr {err}");
    assert!(out.contains("frontier: took story:ready-one"), "{said}");
    through.all_by(GIVEN, 1, &said);
    let line = format!("commit identity: {GIVEN} from --commit-identity not scanned (");
    assert_eq!(out.matches(&line).count(), 1, "{said}");
    assert!(out.contains("`scan: none` declared in"), "{said}");
    assert_eq!(through.forge_calls(), "", "{said}");

    let local = world("forge-none", None);
    let (_, out, err) = local.run(&[
        "--harness",
        "none",
        "--forge",
        "none",
        "--iterations",
        "0",
        "--commit-identity",
        GIVEN,
    ]);
    let said = format!("stdout {out} stderr {err}");
    assert!(out.contains("frontier: took story:ready-one"), "{said}");
    local.all_by(GIVEN, 1, &said);
    assert!(!format!("{out}{err}").contains("not scanned"), "{said}");
}
