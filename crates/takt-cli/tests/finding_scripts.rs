//! `scripts/file-finding` and `scripts/check-finding`, run against scratch stores. A finding is
//! filed into the planning store as a draft story or a blocker carrying its `expected_before`
//! predicate, its source and the reference `finding:<key>`; a key already in the store, or a
//! finding without a predicate that parses, is refused with nothing written. `check-finding`
//! builds takt from a revision of a scratch takt source, in a checkout of that revision so the
//! binary names it, runs the predicate on a fresh checkout of the scenario's seed, or of the
//! repository and seed the predicate names itself, and prints `reproduced` or `not_reproduced`,
//! the results of the `scenario_result` evidence kind (`protocols/self-improvement/1.yaml`). The
//! scratch takt is a workspace with no dependencies, so each build takes seconds; nothing
//! reaches a network.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

/// The scripts under test.
const SCRIPTS: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../scripts");

/// The fixture stores of the library crate, one directory each.
const FIXTURES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../takt/tests/fixtures");

/// The run record a finding cites, relative to the repository it is filed in.
const SOURCE: &str = "docs/runs/trial-1.md";

/// The URL a repository-form predicate names; a `.invalid` host, so a check that clones it
/// instead of the `--checkout` standing in for it fails without a network.
const REPOSITORY_URL: &str = "https://example.invalid/scenarios";

/// A scratch directory under this crate's build directory, removed on drop.
struct Scratch(PathBuf);

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

fn scratch(name: &str) -> Scratch {
    let outer = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("finding-scripts-{}-{name}", std::process::id()));
    if outer.exists() {
        fs::remove_dir_all(&outer).expect("a stale scratch tree is removed");
    }
    fs::create_dir_all(&outer).expect("a scratch directory");
    Scratch(outer)
}

fn git(tree: &Path, args: &[&str]) {
    let status = Command::new("git")
        .args(["-c", "user.name=t", "-c", "user.email=t@localhost"])
        .args(args)
        .current_dir(tree)
        .status()
        .expect("git runs");
    assert!(status.success(), "git {args:?}");
}

/// `git <args>` in `tree`; its standard output, trimmed.
fn git_out(tree: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .args(args)
        .current_dir(tree)
        .output()
        .expect("git runs");
    assert!(output.status.success(), "git {args:?}");
    String::from_utf8_lossy(&output.stdout).trim().to_owned()
}

fn write(path: &Path, text: &str) {
    fs::create_dir_all(path.parent().expect("a parent")).expect("a directory");
    fs::write(path, text).expect("a file");
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

/// A repository with an empty planning store and the run record findings cite, committed.
fn store(at: &Path) -> PathBuf {
    let repo = at.to_path_buf();
    fs::create_dir_all(&repo).expect("a directory");
    fs::copy(
        Path::new(FIXTURES).join("one-ready/.engineering/project.yaml"),
        {
            fs::create_dir_all(repo.join(".engineering")).expect("a directory");
            repo.join(".engineering/project.yaml")
        },
    )
    .expect("the project file");
    write(
        &repo.join(SOURCE),
        "# Trial 1\n\nThe story ran on the wrong protocol.\n",
    );
    git(&repo, &["init", "-q", "-b", "main"]);
    git(&repo, &["add", "-A"]);
    git(&repo, &["commit", "-q", "-m", "seed"]);
    repo
}

/// A finding's body: a heading, a sentence and, when given, the `expected_before` block holding
/// `predicate`, one `key: value` a line.
fn body(predicate: Option<&str>) -> String {
    let mut text = "# The default protocol takes a store story\n\nA story taken from the store \
                    runs on the wrong protocol.\n"
        .to_owned();
    if let Some(predicate) = predicate {
        text.push_str(&format!("\n```expected_before\n{predicate}```\n"));
    }
    text
}

/// The predicate of the default-protocol finding against scenario 1, citing [`SOURCE`].
fn predicate() -> String {
    format!(
        "scenario: 1\ncommand: \"$TAKT\" run --repo . --harness none\nexit: 3\noutput: on software.change/1\nsource: {SOURCE}\n"
    )
}

/// Runs `scripts/<name>` with `args` in `cwd`, `env` added to the environment.
fn script(name: &str, cwd: &Path, args: &[&str], env: &[(&str, &str)]) -> Output {
    Command::new(Path::new(SCRIPTS).join(name))
        .args(args)
        .current_dir(cwd)
        .envs(env.iter().copied())
        .output()
        .unwrap_or_else(|error| panic!("scripts/{name} runs: {error}"))
}

fn shown(output: &Output) -> String {
    format!(
        "exit {:?}\nstdout:\n{}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
}

/// Files `text` as the finding `key` in `repo`, with `extra` arguments.
fn file(repo: &Path, key: &str, text: &str, extra: &[&str]) -> Output {
    let path = repo.parent().expect("a parent").join(format!(
        "body-{}.md",
        key.replace(|c: char| !c.is_ascii_alphanumeric(), "_")
    ));
    fs::write(&path, text).expect("the body file");
    let from = path.to_str().expect("a UTF-8 path");
    let mut args = vec!["--key", key, "--title", "A finding", "--from", from];
    args.extend_from_slice(extra);
    script("file-finding", repo, &args, &[])
}

/// The artifacts `aep plan artifact list --ref finding:<key>` lists in `repo`, one line each.
fn listed(repo: &Path, key: &str) -> Vec<String> {
    let output = Command::new("aep")
        .args([
            "plan",
            "artifact",
            "list",
            "--ref",
            &format!("finding:{key}"),
        ])
        .current_dir(repo)
        .output()
        .expect("aep runs");
    assert!(output.status.success(), "aep list: {}", shown(&output));
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(str::to_owned)
        .collect()
}

fn show(repo: &Path, id: &str) -> String {
    let output = Command::new("aep")
        .args(["plan", "artifact", "show", id])
        .current_dir(repo)
        .output()
        .expect("aep runs");
    assert!(output.status.success(), "aep show {id}: {}", shown(&output));
    String::from_utf8_lossy(&output.stdout).into_owned()
}

/// `git status --porcelain --untracked-files=all` in `repo`.
fn status(repo: &Path) -> String {
    git_out(repo, &["status", "--porcelain", "--untracked-files=all"])
}

#[test]
fn a_finding_is_filed_as_a_draft_story_with_its_predicate_source_and_reference() {
    let outer = scratch("filed");
    let repo = store(&outer.0.join("repo"));
    let output = file(&repo, "default-protocol", &body(Some(&predicate())), &[]);
    assert!(output.status.success(), "{}", shown(&output));
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "filed\n",
        "{}",
        shown(&output)
    );
    let listed = listed(&repo, "default-protocol");
    assert_eq!(listed.len(), 1, "{listed:?}");
    assert!(
        listed[0].starts_with("story:default-protocol ") && listed[0].contains(" draft "),
        "{listed:?}"
    );
    let shown = show(&repo, "story:default-protocol");
    assert!(shown.contains("finding:default-protocol"), "{shown}");
    for line in predicate().lines() {
        assert!(shown.contains(line), "{line} missing from\n{shown}");
    }
    assert!(shown.contains("```expected_before\n"), "{shown}");
}

#[test]
fn a_finding_is_filed_as_a_blocker_when_asked() {
    let outer = scratch("blocker");
    let repo = store(&outer.0.join("repo"));
    let output = file(
        &repo,
        "no-trial-repository",
        &body(Some(&predicate())),
        &["--kind", "blocker"],
    );
    assert!(output.status.success(), "{}", shown(&output));
    assert_eq!(String::from_utf8_lossy(&output.stdout), "filed\n");
    let listed = listed(&repo, "no-trial-repository");
    assert_eq!(listed.len(), 1, "{listed:?}");
    assert!(
        listed[0].starts_with("blocker:no-trial-repository "),
        "{listed:?}"
    );
}

#[test]
fn a_second_filing_of_a_key_is_refused_and_the_store_keeps_one() {
    let outer = scratch("twice");
    let repo = store(&outer.0.join("repo"));
    let first = file(&repo, "default-protocol", &body(Some(&predicate())), &[]);
    assert!(first.status.success(), "{}", shown(&first));
    let before = status(&repo);
    // As a blocker the artifact would be another file, which the store accepts beside the story:
    // the refusal is the key's, not the file name's.
    for extra in [&[][..], &["--kind", "blocker"][..]] {
        let again = file(&repo, "default-protocol", &body(Some(&predicate())), extra);
        assert!(!again.status.success(), "{}", shown(&again));
        assert_ne!(String::from_utf8_lossy(&again.stdout), "filed\n");
        assert!(
            String::from_utf8_lossy(&again.stderr).contains("story:default-protocol"),
            "the refusal names the artifact holding the key: {}",
            shown(&again)
        );
        assert_eq!(listed(&repo, "default-protocol").len(), 1);
        assert_eq!(status(&repo), before, "nothing was written");
    }
}

#[test]
fn a_finding_without_an_expected_before_block_is_refused() {
    let outer = scratch("no-block");
    let repo = store(&outer.0.join("repo"));
    let before = status(&repo);
    let output = file(&repo, "default-protocol", &body(None), &[]);
    assert!(!output.status.success(), "{}", shown(&output));
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "malformed\n",
        "{}",
        shown(&output)
    );
    assert!(listed(&repo, "default-protocol").is_empty());
    assert_eq!(status(&repo), before, "nothing was written");
}

#[test]
fn every_malformed_finding_is_refused_and_nothing_is_written() {
    let outer = scratch("malformed");
    let repo = store(&outer.0.join("repo"));
    let before = status(&repo);
    let good = predicate();
    let without = |key: &str| {
        good.lines()
            .filter(|line| !line.starts_with(&format!("{key}:")))
            .map(|line| format!("{line}\n"))
            .collect::<String>()
    };
    let replaced = |key: &str, value: &str| {
        good.lines()
            .map(|line| {
                if line.starts_with(&format!("{key}:")) {
                    format!("{key}: {value}\n")
                } else {
                    format!("{line}\n")
                }
            })
            .collect::<String>()
    };
    let neither = without("exit")
        .lines()
        .filter(|line| !line.starts_with("output:"))
        .map(|line| format!("{line}\n"))
        .collect::<String>();
    let absolute = outer.0.join("repo").join(SOURCE);
    let cases: Vec<(&str, String)> = vec![
        ("no scenario", body(Some(&without("scenario")))),
        ("no command", body(Some(&without("command")))),
        ("no source", body(Some(&without("source")))),
        ("neither exit nor output", body(Some(&neither))),
        ("an empty command", body(Some(&replaced("command", "")))),
        (
            "an exit that is no number",
            body(Some(&replaced("exit", "three"))),
        ),
        ("an exit past 255", body(Some(&replaced("exit", "256")))),
        (
            "a scenario id a run refuses",
            body(Some(&replaced("scenario", "a b"))),
        ),
        (
            "a source that does not exist",
            body(Some(&replaced("source", "docs/runs/none.md"))),
        ),
        (
            "an absolute source",
            body(Some(&replaced("source", absolute.to_str().expect("UTF-8")))),
        ),
        (
            "a source climbing out",
            body(Some(&replaced("source", "../repo/docs/runs/trial-1.md"))),
        ),
        (
            "a key nobody reads",
            body(Some(&format!("{good}expect: 3\n"))),
        ),
        ("a key given twice", body(Some(&format!("{good}exit: 4\n")))),
        (
            "a line that is no key",
            body(Some(&format!("{good}just words\n"))),
        ),
        (
            "two blocks",
            format!("{}\n```expected_before\n{good}```\n", body(Some(&good))),
        ),
        (
            "a block never closed",
            format!("{}\n```expected_before\n{good}", body(None)),
        ),
    ];
    for (what, text) in &cases {
        let output = file(&repo, "default-protocol", text, &[]);
        assert!(!output.status.success(), "{what}: {}", shown(&output));
        assert_eq!(
            String::from_utf8_lossy(&output.stdout),
            "malformed\n",
            "{what}: {}",
            shown(&output)
        );
        assert!(listed(&repo, "default-protocol").is_empty(), "{what}");
        assert_eq!(status(&repo), before, "{what}: nothing was written");
    }
    for key in ["Default", "a/b", "", "-x", "a b"] {
        let output = file(&repo, key, &body(Some(&good)), &[]);
        assert!(!output.status.success(), "key {key:?}: {}", shown(&output));
        assert_eq!(
            String::from_utf8_lossy(&output.stdout),
            "malformed\n",
            "key {key:?}: {}",
            shown(&output)
        );
        assert_eq!(status(&repo), before, "key {key:?}: nothing was written");
    }
}

/// The scratch takt's `build.rs`: the revision of the checkout the crate is built in, read with
/// git into `TAKT_REVISION`, or `unknown revision` where no checkout holds it, as the real
/// `crates/takt-cli/build.rs` does.
const TAKT_BUILD: &str = r#"use std::process::Command;

fn main() {
    let dir = std::env::var("CARGO_MANIFEST_DIR").unwrap_or_default();
    let revision = Command::new("git")
        .args(["rev-parse", "--verify", "-q", "HEAD"])
        .current_dir(&dir)
        .output()
        .ok()
        .filter(|output| output.status.success())
        .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_owned())
        .filter(|sha| !sha.is_empty())
        .unwrap_or_else(|| "unknown revision".to_owned());
    println!("cargo:rustc-env=TAKT_REVISION={revision}");
}
"#;

/// The scratch takt's `main.rs`: `--version` prints `takt 0.1.0 (<revision>)` as the build
/// script read it; `run --repo <dir>` reports the case it would open on `protocol` when `<dir>`
/// holds the scenario's story, exit 3; else exit 5.
fn takt_main(protocol: &str) -> String {
    format!(
        r#"use std::process::ExitCode;

fn main() -> ExitCode {{
    let args: Vec<String> = std::env::args().collect();
    if args.iter().any(|arg| arg == "--version") {{
        println!("takt 0.1.0 ({{}})", env!("TAKT_REVISION"));
        return ExitCode::SUCCESS;
    }}
    let repo = args
        .iter()
        .position(|arg| arg == "--repo")
        .and_then(|at| args.get(at + 1))
        .cloned()
        .unwrap_or_default();
    let story = std::path::Path::new(&repo).join(".engineering/planning/story/ready-one.md");
    if !story.is_file() {{
        eprintln!("takt: nothing ready in {{repo}}");
        return ExitCode::from(5);
    }}
    println!("case story-ready-one on {protocol} in {{repo}}");
    ExitCode::from(3)
}}
"#
    )
}

/// One world for `check-finding`: a scratch takt source, which is also the repository the
/// findings are filed in, with the commits `defect`, `fixed` and `broken` (a build that fails);
/// the scenario repository, whose seed holds the story `story:ready-one` `proposed` and whose
/// `main` moves past the seed, the story gone, declaring scenario 1, so a predicate run on `main`
/// instead of the seed is told apart.
struct World {
    outer: Scratch,
    takt: PathBuf,
    scenarios: PathBuf,
    seed: String,
    defect: String,
    fixed: String,
    broken: String,
}

fn world(name: &str) -> World {
    let outer = scratch(name);
    let takt = store(&outer.0.join("takt"));
    write(
        &takt.join("Cargo.toml"),
        "[workspace]\nresolver = \"2\"\nmembers = [\"crates/*\"]\n",
    );
    write(
        &takt.join("crates/takt-cli/Cargo.toml"),
        "[package]\nname = \"b10x-takt-cli\"\nversion = \"0.1.0\"\nedition = \"2021\"\n\n[[bin]]\nname = \"takt\"\npath = \"src/main.rs\"\n",
    );
    write(
        &takt.join("Cargo.lock"),
        "# This file is automatically @generated by Cargo.\n# It is not intended for manual editing.\nversion = 4\n\n[[package]]\nname = \"b10x-takt-cli\"\nversion = \"0.1.0\"\n",
    );
    write(&takt.join(".gitignore"), "/target\n");
    write(&takt.join("crates/takt-cli/build.rs"), TAKT_BUILD);
    let main = takt.join("crates/takt-cli/src/main.rs");
    let commit = |text: &str, message: &str| {
        write(&main, text);
        git(&takt, &["add", "-A"]);
        git(&takt, &["commit", "-q", "-m", message]);
        git_out(&takt, &["rev-parse", "HEAD"])
    };
    let defect = commit(&takt_main("software.change/1"), "defect");
    let fixed = commit(&takt_main("story.delivery/1"), "fixed");
    let broken = commit("fn main() { this is no rust }\n", "broken");

    let scenarios = outer.0.join("scenarios");
    copy_dir(&Path::new(FIXTURES).join("one-ready"), &scenarios);
    git(&scenarios, &["init", "-q", "-b", "main"]);
    git(&scenarios, &["add", "-A"]);
    git(&scenarios, &["commit", "-q", "-m", "seed"]);
    let seed = git_out(&scenarios, &["rev-parse", "HEAD"]);
    fs::remove_file(scenarios.join(".engineering/planning/story/ready-one.md"))
        .expect("the story is removed past the seed");
    write(
        &scenarios.join(".engineering/scenarios.conf"),
        &format!("[scenario \"1\"]\n\tstory = story:ready-one\n\tseed = {seed}\n"),
    );
    git(&scenarios, &["add", "-A"]);
    git(&scenarios, &["commit", "-q", "-m", "declare scenario 1"]);
    World {
        outer,
        takt,
        scenarios,
        seed,
        defect,
        fixed,
        broken,
    }
}

impl World {
    fn cache(&self) -> PathBuf {
        self.outer.0.join("cache")
    }

    /// Runs `check-finding` from the takt source on `finding` at `revision`, with `env` added.
    fn check(&self, finding: &str, revision: &str, env: &[(&str, &str)]) -> Output {
        let cache = self.cache();
        let mut all = vec![("XDG_CACHE_HOME", cache.to_str().expect("UTF-8"))];
        all.extend_from_slice(env);
        script(
            "check-finding",
            &self.takt,
            &[
                "--scenarios",
                self.scenarios.to_str().expect("UTF-8"),
                finding,
                revision,
            ],
            &all,
        )
    }

    /// Runs `check-finding` from the takt source on `finding` at `revision` without
    /// `--scenarios`: the predicate names its repository and seed itself, and `--checkout`
    /// makes the scenario repository's checkout stand in for the predicate's URL.
    fn check_repository(&self, finding: &str, revision: &str, env: &[(&str, &str)]) -> Output {
        let cache = self.cache();
        let mut all = vec![("XDG_CACHE_HOME", cache.to_str().expect("UTF-8"))];
        all.extend_from_slice(env);
        let checkout = self.checkout();
        script(
            "check-finding",
            &self.takt,
            &["--checkout", &checkout, finding, revision],
            &all,
        )
    }

    /// The `--checkout` value that makes the scenario repository stand in for
    /// [`REPOSITORY_URL`].
    fn checkout(&self) -> String {
        format!("{REPOSITORY_URL}={}", self.scenarios.display())
    }

    /// The default-protocol finding's predicate in its repository form: the repository by its
    /// URL and seed, the task's test command, citing [`SOURCE`]; the command checks that `$TEST`
    /// reached it.
    fn repository_predicate(&self) -> String {
        format!(
            "repository: {REPOSITORY_URL}\nseed: {}\ntest: ./check.sh\ncommand: test \"$TEST\" = ./check.sh && \"$TAKT\" run --repo .\nexit: 3\noutput: on software.change/1\nsource: {SOURCE}\n",
            self.seed
        )
    }

    /// The scenario repository's state a check must leave alone: its head, its branch, its
    /// refs and its status.
    fn scenario_state(&self) -> String {
        [
            git_out(&self.scenarios, &["rev-parse", "HEAD"]),
            git_out(&self.scenarios, &["symbolic-ref", "-q", "HEAD"]),
            git_out(&self.scenarios, &["for-each-ref"]),
            status(&self.scenarios),
        ]
        .join("\n")
    }
}

#[test]
fn the_predicate_is_reproduced_with_a_takt_built_from_the_revision_with_the_defect() {
    let world = world("reproduced");
    let filed = file(
        &world.takt,
        "default-protocol",
        &body(Some(&predicate())),
        &[],
    );
    assert!(filed.status.success(), "{}", shown(&filed));
    let before = world.scenario_state();
    let shared = world.outer.0.join("shared-target");
    let output = world.check(
        "story:default-protocol",
        &world.defect,
        &[("CARGO_TARGET_DIR", shared.to_str().expect("UTF-8"))],
    );
    assert!(output.status.success(), "{}", shown(&output));
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "reproduced\n",
        "{}",
        shown(&output)
    );
    assert_eq!(
        world.scenario_state(),
        before,
        "the scenario checkout is untouched"
    );
    assert!(
        !shared.exists(),
        "the build stays in its own tree, whatever CARGO_TARGET_DIR names"
    );
    let builds = world.cache().join("takt/check-finding/builds");
    assert!(
        builds.join(&world.defect).join("takt").is_file(),
        "the binary is kept per revision"
    );
    let left: Vec<_> = fs::read_dir(&builds)
        .expect("the builds directory")
        .map(|entry| entry.expect("an entry").file_name())
        .collect();
    assert_eq!(
        left,
        vec![std::ffi::OsString::from(&world.defect)],
        "the build directory is removed"
    );
}

#[test]
fn the_predicate_is_not_reproduced_with_a_takt_built_from_the_revision_that_fixes_it() {
    let world = world("not-reproduced");
    let filed = file(
        &world.takt,
        "default-protocol",
        &body(Some(&predicate())),
        &[],
    );
    assert!(filed.status.success(), "{}", shown(&filed));
    let output = world.check("story:default-protocol", &world.fixed, &[]);
    assert!(output.status.success(), "{}", shown(&output));
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "not_reproduced\n",
        "{}",
        shown(&output)
    );
    // The revision is the argument: the same finding, the defect's revision, reproduces.
    let output = world.check("story:default-protocol", &world.defect, &[]);
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "reproduced\n",
        "{}",
        shown(&output)
    );
}

#[test]
fn the_predicate_runs_without_the_callers_credentials() {
    let world = world("credentials");
    let seen = format!(
        "scenario: 1\ncommand: env; test -x \"$TAKT\"\nexit: 0\noutput: SCENARIO=\nsource: {SOURCE}\n"
    );
    let leaked =
        format!("scenario: 1\ncommand: env\noutput: PLANTED-CREDENTIAL\nsource: {SOURCE}\n");
    for (key, predicate) in [("env-seen", &seen), ("env-leaked", &leaked)] {
        let filed = file(&world.takt, key, &body(Some(predicate)), &[]);
        assert!(filed.status.success(), "{}", shown(&filed));
    }
    let planted = [
        ("GITHUB_TOKEN", "PLANTED-CREDENTIAL"),
        ("GH_TOKEN", "PLANTED-CREDENTIAL"),
        ("ANTHROPIC_API_KEY", "PLANTED-CREDENTIAL"),
    ];
    let output = world.check("story:env-seen", &world.defect, &planted);
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "reproduced\n",
        "the predicate ran, with its binary and its scenario: {}",
        shown(&output)
    );
    let output = world.check("story:env-leaked", &world.defect, &planted);
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "not_reproduced\n",
        "no credential of the caller reaches the predicate: {}",
        shown(&output)
    );
}

#[test]
fn a_check_that_cannot_run_its_predicate_prints_no_result() {
    let world = world("refusals");
    let filed = file(
        &world.takt,
        "default-protocol",
        &body(Some(&predicate())),
        &[],
    );
    assert!(filed.status.success(), "{}", shown(&filed));
    let other = predicate().replace("scenario: 1", "scenario: 9");
    let filed = file(&world.takt, "undeclared-scenario", &body(Some(&other)), &[]);
    assert!(filed.status.success(), "{}", shown(&filed));
    let cases = [
        (
            "a finding the store does not hold",
            "story:no-such",
            world.defect.as_str(),
        ),
        (
            "a revision that names no commit",
            "story:default-protocol",
            "no-such-revision",
        ),
        (
            "a scenario the repository does not declare",
            "story:undeclared-scenario",
            world.defect.as_str(),
        ),
        (
            "a revision whose build fails",
            "story:default-protocol",
            world.broken.as_str(),
        ),
    ];
    for (what, finding, revision) in cases {
        let output = world.check(finding, revision, &[]);
        assert!(!output.status.success(), "{what}: {}", shown(&output));
        assert!(output.stdout.is_empty(), "{what}: {}", shown(&output));
    }
    let output = script(
        "check-finding",
        &world.takt,
        &["story:default-protocol", &world.defect],
        &[("XDG_CACHE_HOME", world.cache().to_str().expect("UTF-8"))],
    );
    assert!(
        !output.status.success(),
        "no --scenarios: {}",
        shown(&output)
    );
    assert!(
        output.stdout.is_empty(),
        "no --scenarios: {}",
        shown(&output)
    );
}

#[test]
fn a_finding_on_a_repository_is_filed_with_its_repository_seed_and_test() {
    let world = world("repository-filed");
    let text = body(Some(&world.repository_predicate()));
    let filed = file(&world.takt, "repo-form", &text, &[]);
    assert!(filed.status.success(), "{}", shown(&filed));
    assert_eq!(
        String::from_utf8_lossy(&filed.stdout),
        "filed\n",
        "{}",
        shown(&filed)
    );
    let artifact = show(&world.takt, "story:repo-form");
    for line in [
        format!("repository: {REPOSITORY_URL}"),
        format!("seed: {}", world.seed),
        "test: ./check.sh".to_owned(),
    ] {
        assert!(
            artifact.contains(&line),
            "{line} in the filed body:\n{artifact}"
        );
    }
    let path = world.outer.0.join("body-repo_form.md");
    let output = script(
        "file-finding",
        &world.takt,
        &["--predicate", path.to_str().expect("UTF-8")],
        &[],
    );
    assert!(output.status.success(), "{}", shown(&output));
    let printed = String::from_utf8_lossy(&output.stdout);
    for line in [
        format!("repository\t{REPOSITORY_URL}"),
        format!("seed\t{}", world.seed),
        "test\t./check.sh".to_owned(),
        "command\ttest \"$TEST\" = ./check.sh && \"$TAKT\" run --repo .".to_owned(),
    ] {
        assert!(
            printed.lines().any(|printed| printed == line),
            "{line:?} among the predicate's entries:\n{printed}"
        );
    }
}

#[test]
fn a_repository_finding_is_checked_in_a_clone_of_its_repository_at_its_seed() {
    let world = world("repository-checked");
    let filed = file(
        &world.takt,
        "repo-form",
        &body(Some(&world.repository_predicate())),
        &[],
    );
    assert!(filed.status.success(), "{}", shown(&filed));
    let before = world.scenario_state();
    // No --scenarios: the predicate names its repository. At the seed the story is there, so
    // the defect's takt answers exit 3 on software.change/1; past the seed it would exit 5.
    let output = world.check_repository("story:repo-form", &world.defect, &[]);
    assert!(output.status.success(), "{}", shown(&output));
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "reproduced\n",
        "{}",
        shown(&output)
    );
    assert_eq!(
        world.scenario_state(),
        before,
        "the repository the predicate names is untouched"
    );
    let output = world.check_repository("story:repo-form", &world.fixed, &[]);
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "not_reproduced\n",
        "{}",
        shown(&output)
    );
    // --scenarios given as well changes nothing: the predicate's repository is the one cloned.
    let cache = world.cache();
    let checkout = world.checkout();
    let output = script(
        "check-finding",
        &world.takt,
        &[
            "--scenarios",
            world.scenarios.to_str().expect("UTF-8"),
            "--checkout",
            &checkout,
            "story:repo-form",
            &world.defect,
        ],
        &[("XDG_CACHE_HOME", cache.to_str().expect("UTF-8"))],
    );
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "reproduced\n",
        "{}",
        shown(&output)
    );
    // The check's record names the URL the predicate gave and the checkout that stood in.
    let runs = world.cache().join("takt/check-finding/runs");
    let results: Vec<String> = fs::read_dir(&runs)
        .expect("the runs directory")
        .map(|entry| fs::read_to_string(entry.expect("an entry").path().join("result")))
        .filter_map(Result::ok)
        .collect();
    assert!(
        results.iter().all(|result| {
            result.contains(&format!("repository: {REPOSITORY_URL}\n"))
                && result.contains(&format!("checkout: {}\n", world.scenarios.display()))
                && result.contains(&format!("seed: {}\n", world.seed))
        }),
        "{results:?}"
    );
}

#[test]
fn a_repository_finding_the_check_cannot_clone_at_its_seed_gives_no_result() {
    let world = world("repository-refusals");
    let unheld = world
        .repository_predicate()
        .replace(&world.seed, "0123456789abcdef0123456789abcdef01234567");
    let filed = file(&world.takt, "unheld-seed", &body(Some(&unheld)), &[]);
    assert!(filed.status.success(), "{}", shown(&filed));
    let filed = file(
        &world.takt,
        "repo-form",
        &body(Some(&world.repository_predicate())),
        &[],
    );
    assert!(filed.status.success(), "{}", shown(&filed));
    let output = world.check_repository("story:unheld-seed", &world.defect, &[]);
    assert!(
        !output.status.success(),
        "a seed the repository does not hold: {}",
        shown(&output)
    );
    assert!(output.stdout.is_empty(), "{}", shown(&output));
    let empty = world.outer.0.join("no-repository");
    fs::create_dir_all(&empty).expect("a directory");
    let cache = world.cache();
    let env = [("XDG_CACHE_HOME", cache.to_str().expect("UTF-8"))];
    let no_repository = format!("{REPOSITORY_URL}={}", empty.display());
    let no_directory = format!("{REPOSITORY_URL}={}", world.outer.0.join("none").display());
    let cases: Vec<(&str, Vec<&str>, i32)> = vec![
        (
            "a checkout that is no repository",
            vec!["--checkout", &no_repository],
            1,
        ),
        (
            "no checkout: the URL itself, which no network resolves",
            vec![],
            1,
        ),
        (
            "a checkout that is no <url>=<path>",
            vec!["--checkout", "nonsense"],
            64,
        ),
        (
            "a checkout path that is no directory",
            vec!["--checkout", &no_directory],
            64,
        ),
    ];
    for (what, flags, code) in cases {
        let mut args = flags;
        args.extend(["story:repo-form", world.defect.as_str()]);
        let output = script("check-finding", &world.takt, &args, &env);
        assert_eq!(
            output.status.code(),
            Some(code),
            "{what}: {}",
            shown(&output)
        );
        assert!(output.stdout.is_empty(), "{what}: {}", shown(&output));
    }
}

#[test]
fn a_predicate_mixing_or_missing_the_scenario_and_repository_forms_is_refused() {
    let outer = scratch("forms");
    let repo = store(&outer.0.join("repo"));
    let before = status(&repo);
    let scenario_form = predicate();
    let seed = git_out(&repo, &["rev-parse", "HEAD"]);
    let repository_form = format!(
        "repository: {REPOSITORY_URL}\nseed: {seed}\ncommand: \"$TAKT\" run --repo .\nexit: 3\nsource: {SOURCE}\n"
    );
    let replaced = |text: &str, key: &str, value: &str| {
        text.lines()
            .map(|line| {
                if line.starts_with(&format!("{key}:")) {
                    format!("{key}: {value}\n")
                } else {
                    format!("{line}\n")
                }
            })
            .collect::<String>()
    };
    let without = |text: &str, key: &str| {
        text.lines()
            .filter(|line| !line.starts_with(&format!("{key}:")))
            .map(|line| format!("{line}\n"))
            .collect::<String>()
    };
    let cases: Vec<(&str, String)> = vec![
        (
            "both a scenario and a repository",
            format!(
                "{scenario_form}repository: {}\nseed: {seed}\n",
                repo.display()
            ),
        ),
        (
            "a repository without a seed",
            without(&repository_form, "seed"),
        ),
        (
            "a seed without a repository",
            format!("{scenario_form}seed: {seed}\n"),
        ),
        (
            "a test without a repository",
            format!("{scenario_form}test: ./check.sh\n"),
        ),
        (
            "a repository over ssh",
            replaced(
                &repository_form,
                "repository",
                "ssh://git@github.com/beyond10x/canon.git",
            ),
        ),
        (
            "a repository in scp form",
            replaced(
                &repository_form,
                "repository",
                "git@github.com:beyond10x/canon.git",
            ),
        ),
        (
            "a repository over plain http",
            replaced(
                &repository_form,
                "repository",
                "http://example.invalid/canon.git",
            ),
        ),
        (
            "a relative repository path",
            replaced(&repository_form, "repository", "../canon"),
        ),
        (
            "an absolute repository path, one machine's",
            replaced(
                &repository_form,
                "repository",
                repo.to_str().expect("UTF-8"),
            ),
        ),
        (
            "a repository as a file URL",
            replaced(
                &repository_form,
                "repository",
                &format!("file://{}", repo.display()),
            ),
        ),
        (
            "a seed that is no commit id",
            replaced(&repository_form, "seed", "main"),
        ),
        (
            "a seed too short to name a commit",
            replaced(&repository_form, "seed", "abc"),
        ),
        ("an empty test", format!("{repository_form}test:\n")),
    ];
    for (what, text) in &cases {
        let output = file(&repo, "forms", &body(Some(text)), &[]);
        assert!(!output.status.success(), "{what}: {}", shown(&output));
        assert_eq!(
            String::from_utf8_lossy(&output.stdout),
            "malformed\n",
            "{what}: {}",
            shown(&output)
        );
        assert!(listed(&repo, "forms").is_empty(), "{what}");
        assert_eq!(status(&repo), before, "{what}: nothing was written");
    }
    // An https repository with a seed is well formed, whatever the host; filing reads nothing
    // from it.
    let https = replaced(
        &repository_form,
        "repository",
        "https://github.com/beyond10x/canon",
    );
    let output = file(&repo, "forms", &body(Some(&https)), &[]);
    assert!(output.status.success(), "{}", shown(&output));
}

#[test]
fn a_takt_check_finding_builds_names_its_revision() {
    use std::os::unix::fs::PermissionsExt;

    let world = world("revision");
    // The finding of story:check-finding-builds-name-their-revision: a takt built from an
    // export of the tree knows no revision.
    let text = body(Some(&format!(
        "scenario: 1\ncommand: \"$TAKT\" --version\nexit: 0\noutput: (unknown revision)\nsource: {SOURCE}\n"
    )));
    let filed = file(&world.takt, "unknown-revision", &text, &[]);
    assert!(filed.status.success(), "{}", shown(&filed));
    // A binary an earlier check kept, built before builds named their revision, is not
    // trusted: it has no revision record beside it.
    let kept = world
        .cache()
        .join("takt/check-finding/builds")
        .join(&world.defect);
    fs::create_dir_all(&kept).expect("the builds directory");
    let stale = kept.join("takt");
    write(
        &stale,
        "#!/bin/sh\nprintf 'takt 0.1.0 (unknown revision)\\n'\n",
    );
    fs::set_permissions(&stale, fs::Permissions::from_mode(0o755)).expect("executable");
    let output = world.check("story:unknown-revision", &world.defect, &[]);
    assert!(output.status.success(), "{}", shown(&output));
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "not_reproduced\n",
        "the built takt names its revision: {}",
        shown(&output)
    );
    let version = Command::new(&stale)
        .arg("--version")
        .output()
        .expect("the kept binary runs");
    assert_eq!(
        String::from_utf8_lossy(&version.stdout),
        format!("takt 0.1.0 ({})\n", world.defect),
        "the binary kept per revision prints that revision"
    );
    assert_eq!(
        fs::read_to_string(kept.join("revision"))
            .expect("the revision record")
            .trim(),
        world.defect,
        "the build records the revision it was made from"
    );
}
