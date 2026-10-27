//! story:takt-commits-with-the-repositorys-identity through the binary: every commit `takt run`
//! makes carries the identity `.engineering/commit-identity.conf` names at the base, or the one
//! `--commit-identity` gives, which wins, as author and committer, whatever the operator's git
//! configuration or the `GIT_AUTHOR_*` and `GIT_COMMITTER_*` variables in takt's own environment
//! say. With none configured a run that pushes nothing commits as `takt <takt@localhost>`; a run
//! that pushes (the bot route) is refused before its case opens, naming the setting. A resume
//! commits with the identity its case was opened with. The harness is a double on the program
//! path; nothing reaches a network or a model.

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

/// The identity the repository configures in these cases.
const BOT: &str = "b10x-bot[bot] <316511680+b10x-bot[bot]@users.noreply.github.com>";

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
/// `work.txt` and answers `implementation.edit`, and `b10x-gates` and `gh` doubles that log each
/// call and fail.
struct World {
    _scratch: Scratch,
    repo: PathBuf,
    bin: PathBuf,
    /// Where the route's doubles log each call.
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
    // The bot route's programs: each call logged and failed, so nothing reaches a network.
    let calls = outer.join("route-calls");
    let route = format!(
        "#!/bin/sh\necho \"$0 $*\" >> '{}'\necho 'the route double refuses' >&2\nexit 1\n",
        calls.display()
    );
    for name in ["b10x-gates", "gh"] {
        fs::write(bin.join(name), &route).expect("the double is written");
    }
    #[cfg(unix)]
    for name in ["claude", "b10x-gates", "gh"] {
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

impl World {
    /// The built `takt` with `args`, the doubles first on the program path, the operator's
    /// identity variables ([`OPERATOR_ENV`]) in its environment: the exit code, stdout, stderr.
    fn takt(&self, args: &[&str]) -> (i32, String, String) {
        let output = Command::new(env!("CARGO_BIN_EXE_takt"))
            .args(args)
            .envs(OPERATOR_ENV)
            .env_remove("B10X_GATES_POLICY")
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

    /// `takt run --repo <repo> --test-cmd true`, `extra` after it.
    fn run(&self, extra: &[&str]) -> (i32, String, String) {
        let repo = self.repo.to_str().expect("utf-8");
        let mut args = vec!["run", "--repo", repo, "--test-cmd", "true"];
        args.extend_from_slice(extra);
        self.takt(&args)
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

    /// The calls the route's doubles were given, one a line.
    fn route_calls(&self) -> String {
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
    let world = world(
        "configured",
        Some(&identity_file(
            "b10x-bot[bot]",
            "316511680+b10x-bot[bot]@users.noreply.github.com",
        )),
    );
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
    world.all_by(BOT, 2, &said);
}

/// `--commit-identity` wins over the file.
#[test]
fn the_flag_wins_over_the_file() {
    let world = world(
        "flag",
        Some(&identity_file(
            "b10x-bot[bot]",
            "316511680+b10x-bot[bot]@users.noreply.github.com",
        )),
    );
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
    let world = world(
        "resume",
        Some(&identity_file(
            "b10x-bot[bot]",
            "316511680+b10x-bot[bot]@users.noreply.github.com",
        )),
    );
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

/// A case opened with no identity under `--forge none` commits as takt's own; once the
/// repository has an `origin` on github.com, a run that would resume it through the bot route,
/// by `--case` or as the open case the frontier resumes, is refused before it resumes: exit 1,
/// nothing committed, the bot route and `gh` never called, and the refusal says how to get past
/// the case: finish it with `--forge none`, or drop its branch and its directory.
#[test]
fn a_case_opened_with_no_identity_is_not_resumed_through_the_bot_route() {
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
    git(
        &world.repo,
        &["remote", "add", "origin", "git@github.com:owner/repo.git"],
    );
    let head = git(&world.repo, &["rev-parse", "HEAD"]);
    for by_case in [true, false] {
        let mut args = vec!["--harness", "claude", "--iterations", "3"];
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
        assert_eq!(world.route_calls(), "", "{said}");
    }
}

/// Acceptance 2, where commits leave the machine: with no identity configured, a run whose
/// forge is the bot route (by default for an `origin` on github.com) is refused before its case
/// opens, naming the file, its keys and the flag: no case directory, no case branch, no commit,
/// the story not taken, and the bot route never called.
#[test]
fn a_pushing_run_with_no_identity_is_refused_before_its_case_opens() {
    let world = world("pushing", None);
    git(
        &world.repo,
        &["remote", "add", "origin", "git@github.com:owner/repo.git"],
    );
    let start = git(&world.repo, &["rev-parse", "HEAD"]);
    for forge in [&["--forge", "bot"][..], &[][..]] {
        let mut args = vec!["--harness", "claude", "--iterations", "3"];
        args.extend_from_slice(forge);
        let (code, out, err) = world.run(&args);
        let said = format!("{forge:?}: stdout {out} stderr {err}");
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
        assert_eq!(world.route_calls(), "", "{said}");
    }
}

/// The same for a scenario run through the bot route: refused before anything is fetched, cut
/// or pushed, naming the setting, when `--declared-on` holds no identity and no flag is given.
#[test]
fn a_scenario_run_through_the_bot_route_with_no_identity_is_refused_before_anything_is_cut() {
    let world = world("scenario", None);
    let seed = git(&world.repo, &["rev-parse", "HEAD"]);
    fs::write(
        world.repo.join(".engineering/scenarios.conf"),
        format!("[scenario \"1\"]\n\tstory = story:ready-one\n\tseed = {seed}\n"),
    )
    .expect("a file");
    git(&world.repo, &["add", "-A"]);
    git(&world.repo, &["commit", "-q", "-m", "declare"]);
    git(
        &world.repo,
        &["remote", "add", "origin", "git@github.com:owner/repo.git"],
    );
    let start = git(&world.repo, &["rev-parse", "HEAD"]);
    let repo = world.repo.to_str().expect("utf-8");
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
                "refs/heads/takt/"
            ]
        ),
        "",
        "{said}"
    );
    assert!(!world.repo.join(".takt/cases").exists(), "{said}");
    assert_eq!(world.route_calls(), "", "{said}");
}

// story:a-run-refuses-a-scanned-identity-at-open: a run whose commits leave the machine scans
// its commit identity with the bot route's own `scan-text` when it opens, before step 1.

/// The name the scan double refuses wherever a file it scans holds it.
const REFUSED_NAME: &str = "Refused Person";

/// An identity the scan double refuses.
const REFUSED: &str = "Refused Person <refused@example.invalid>";

/// An identity the scan double passes.
const PASSING: &str = "Release Bot <release@example.invalid>";

/// What `b10x-gates` 0.1.21 says, exiting 1, when no policy file is given and its default one
/// is not there.
const NO_POLICY: &str =
    "b10x-gates: protected file /var/empty/.config/b10x/gates/policy.json unavailable: not found";

/// How the scan double words a refusal, as `b10x-gates scan-text` does after the path.
const RULE_BROKEN: &str =
    "breaks 1 private rule(s): line 1 person-name; private details remain local";

impl World {
    /// Replaces the `b10x-gates` double with one whose `scan-text` appends every file it is
    /// given to `scanned` and refuses one holding [`REFUSED_NAME`] ([`RULE_BROKEN`]), passing the
    /// rest; with `no_policy`, every `scan-text` says [`NO_POLICY`] and fails, scanning nothing.
    /// Every call is logged as before; a `bot -- fetch` (the scenario run's read of the
    /// remote's trial branches) passes, fetching nothing; every other line is refused.
    fn scanning_gates(&self, no_policy: bool) {
        let scanned = self.scanned_path();
        let policy = if no_policy {
            format!("echo '{NO_POLICY}' >&2\nexit 1\n")
        } else {
            String::new()
        };
        let script = format!(
            "#!/bin/sh\necho \"$0 $*\" >> '{calls}'\nif [ \"$1 $3\" = \"bot fetch\" ]; then\n  exit 0\nfi\nif [ \"$1\" != scan-text ]; then\n  echo 'the route double refuses' >&2\n  exit 1\nfi\n{policy}for arg in \"$@\"; do\n  if [ -f \"$arg\" ]; then\n    cat \"$arg\" >> '{scanned}'\n    if grep -q '{REFUSED_NAME}' \"$arg\"; then\n      echo \"$arg {RULE_BROKEN}\" >&2\n      exit 1\n    fi\n  fi\ndone\necho \"$# file(s) carry no private rule\"\nexit 0\n",
            calls = self.calls.display(),
            scanned = scanned.display(),
        );
        let path = self.bin.join("b10x-gates");
        fs::write(&path, script).expect("the double is written");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).expect("executable");
        }
    }

    fn scanned_path(&self) -> PathBuf {
        self.calls.with_file_name("scanned")
    }

    /// Everything the scan double was given to read.
    fn scanned(&self) -> String {
        fs::read_to_string(self.scanned_path()).unwrap_or_default()
    }

    /// An `origin` on github.com: the forge is the bot route by default.
    fn with_origin(&self) {
        git(
            &self.repo,
            &["remote", "add", "origin", "git@github.com:owner/repo.git"],
        );
    }

    /// Declares scenario `1` on story:ready-one, seeded at the current commit, and commits it.
    fn declare_scenario(&self) {
        let seed = git(&self.repo, &["rev-parse", "HEAD"]);
        fs::write(
            self.repo.join(".engineering/scenarios.conf"),
            format!("[scenario \"1\"]\n\tstory = story:ready-one\n\tseed = {seed}\n"),
        )
        .expect("a file");
        git(&self.repo, &["add", "-A"]);
        git(&self.repo, &["commit", "-q", "-m", "declare"]);
    }

    /// `takt scenario run --repo <repo> 1 --test-cmd true --harness claude`, `extra` after it.
    fn scenario_run(&self, extra: &[&str]) -> (i32, String, String) {
        let repo = self.repo.to_str().expect("utf-8");
        let mut args = vec![
            "scenario",
            "run",
            "--repo",
            repo,
            "1",
            "--test-cmd",
            "true",
            "--harness",
            "claude",
        ];
        args.extend_from_slice(extra);
        self.takt(&args)
    }

    /// Asserts nothing was opened, committed or pushed, no trial base is left, and the route ran
    /// one `scan-text`, which read `identity` as a commit object carries it, and no line but it
    /// and the read of the trial branches.
    fn refused_at_open(&self, start: &str, identity: &str, said: &str) {
        assert_eq!(git(&self.repo, &["rev-parse", "HEAD"]), start, "{said}");
        assert_eq!(
            git(&self.repo, &["branch", "--show-current"]),
            "main",
            "{said}"
        );
        assert_eq!(
            git(
                &self.repo,
                &[
                    "for-each-ref",
                    "--format=%(refname)",
                    "refs/heads/trial/",
                    "refs/heads/takt/"
                ]
            ),
            "",
            "{said}"
        );
        assert_eq!(git(&self.repo, &["status", "--porcelain"]), "", "{said}");
        assert!(!self.repo.join(".takt/cases").exists(), "{said}");
        assert!(!self.repo.join("work.txt").exists(), "{said}");
        let calls = self.route_calls();
        let scans: Vec<&str> = calls
            .lines()
            .filter(|line| line.contains("b10x-gates scan-text "))
            .collect();
        assert_eq!(scans.len(), 1, "one scan: {calls}\n{said}");
        assert!(
            scans[0].contains("b10x-gates scan-text --repository owner/repo "),
            "{calls}\n{said}"
        );
        for line in calls.lines() {
            assert!(
                line.contains("b10x-gates scan-text ") || line.contains("b10x-gates bot -- fetch "),
                "nothing but the scan and a read: {calls}\n{said}"
            );
        }
        let scanned = self.scanned();
        for side in ["author", "committer"] {
            assert!(
                scanned.contains(&format!("{side} {identity}")),
                "the scan read the identity as a commit's {side}: {scanned}\n{said}"
            );
        }
    }
}

/// Acceptance 1 and 2 for `takt run`: through the bot route, an identity the scan refuses,
/// given by `--commit-identity` or read from `.engineering/commit-identity.conf`, ends the run
/// with exit 1 before step 1, naming the scan's refusal and where the identity came from: no
/// case, no branch, no commit, the harness never called, nothing pushed.
#[test]
fn a_run_through_the_bot_route_refuses_an_identity_the_scan_refuses_before_step_one() {
    let file = identity_file(REFUSED_NAME, "refused@example.invalid");
    for (name, configured, flag, source) in [
        ("scan-flag", None, Some(REFUSED), "--commit-identity"),
        ("scan-file", Some(file.as_str()), None, IDENTITY_FILE),
    ] {
        let world = world(name, configured);
        world.with_origin();
        world.scanning_gates(false);
        let start = git(&world.repo, &["rev-parse", "HEAD"]);
        let mut args = vec!["--harness", "claude", "--iterations", "3"];
        if let Some(flag) = flag {
            args.extend(["--commit-identity", flag]);
        }
        let (code, out, err) = world.run(&args);
        let said = format!("{name}: stdout {out} stderr {err}");
        assert_eq!(code, 1, "{said}");
        for named in [RULE_BROKEN, "scan-text", REFUSED, source] {
            assert!(err.contains(named), "{named}: {said}");
        }
        assert!(!out.contains("frontier: took"), "{said}");
        world.refused_at_open(&start, REFUSED, &said);
    }
}

/// Acceptance 1 and 2 for `takt scenario run`: the same refusal before the trial base is pushed
/// or any step taken; the trial base cut locally is removed again.
#[test]
fn a_scenario_run_through_the_bot_route_refuses_an_identity_the_scan_refuses() {
    let file = identity_file(REFUSED_NAME, "refused@example.invalid");
    for (name, configured, flag, source) in [
        (
            "scan-scenario-flag",
            None,
            Some(REFUSED),
            "--commit-identity",
        ),
        (
            "scan-scenario-file",
            Some(file.as_str()),
            None,
            IDENTITY_FILE,
        ),
    ] {
        let world = world(name, configured);
        world.declare_scenario();
        world.with_origin();
        world.scanning_gates(false);
        let start = git(&world.repo, &["rev-parse", "HEAD"]);
        let mut args = Vec::new();
        if let Some(flag) = flag {
            args.extend(["--commit-identity", flag]);
        }
        let (code, out, err) = world.scenario_run(&args);
        let said = format!("{name}: stdout {out} stderr {err}");
        assert_eq!(code, 1, "{said}");
        for named in [RULE_BROKEN, "scan-text", REFUSED, source] {
            assert!(err.contains(named), "{named}: {said}");
        }
        world.refused_at_open(&start, REFUSED, &said);
    }
}

/// Acceptance 3: an identity the scan passes opens the run as before, the scan its first route
/// line; under `--forge none` an identity the scan would refuse opens the run and no scan runs.
#[test]
fn an_identity_the_scan_passes_opens_and_forge_none_scans_nothing() {
    let passing = world("scan-passes", None);
    passing.with_origin();
    passing.scanning_gates(false);
    let (_, out, err) = passing.run(&[
        "--harness",
        "none",
        "--iterations",
        "0",
        "--commit-identity",
        PASSING,
    ]);
    let said = format!("stdout {out} stderr {err}");
    assert!(out.contains("frontier: took story:ready-one"), "{said}");
    passing.all_by(PASSING, 1, &said);
    let calls = passing.route_calls();
    assert!(
        calls
            .lines()
            .next()
            .is_some_and(|line| line.contains("b10x-gates scan-text ")),
        "{calls}\n{said}"
    );
    assert!(
        passing.scanned().contains(&format!("author {PASSING}")),
        "{said}"
    );
    assert!(!format!("{out}{err}").contains("not scanned"), "{said}");

    let unscanned = world("scan-none", None);
    unscanned.with_origin();
    unscanned.scanning_gates(false);
    let (_, out, err) = unscanned.run(&[
        "--harness",
        "none",
        "--forge",
        "none",
        "--iterations",
        "0",
        "--commit-identity",
        REFUSED,
    ]);
    let said = format!("stdout {out} stderr {err}");
    assert!(out.contains("frontier: took story:ready-one"), "{said}");
    unscanned.all_by(REFUSED, 1, &said);
    assert_eq!(unscanned.route_calls(), "", "{said}");
}

/// Acceptance 4: with no policy the scan cannot run; the run says "not scanned", never that the
/// identity passed, and opens as it does today, its scan left to `pr.open`.
#[test]
fn with_no_policy_the_identity_is_not_scanned_and_the_run_opens() {
    let world = world("scan-no-policy", None);
    world.with_origin();
    world.scanning_gates(true);
    let (_, out, err) = world.run(&[
        "--harness",
        "none",
        "--iterations",
        "0",
        "--commit-identity",
        REFUSED,
    ]);
    let said = format!("stdout {out} stderr {err}");
    assert!(out.contains("frontier: took story:ready-one"), "{said}");
    world.all_by(REFUSED, 1, &said);
    let both = format!("{out}{err}");
    assert!(both.contains("not scanned"), "{said}");
    assert!(both.contains("unavailable: not found"), "{said}");
    assert!(!both.contains("passed"), "{said}");
    assert_eq!(world.scanned(), "", "{said}");
}
