//! `scripts/file-finding` and `scripts/check-finding`, run against scratch stores. A finding is
//! filed into the planning store as a draft story or a blocker carrying its `expected_before`
//! predicate, its source and the reference `finding:<key>`; a key already in the store, or a
//! finding without a predicate that parses, is refused with nothing written. `check-finding`
//! builds takt from a revision of a scratch takt source, in a checkout of that revision so the
//! binary names it, confined in bubblewrap so the build reads nothing of the caller's home or
//! environment and writes nothing outside its own build directory, runs the predicate on a
//! fresh checkout of the scenario's seed, or of the repository and seed the predicate names
//! itself, with every host path read-only but its own run directory, the caller's home hidden
//! and a PID namespace of its own, and prints `reproduced`, `not_reproduced` (only when the
//! command ended with the exit the predicate names as the healthy one) or `error`, the results
//! of the `scenario_result` evidence kind (`protocols/self-improvement/1.yaml`). The scratch
//! takt is a workspace with no dependencies, so each build takes seconds; nothing reaches a
//! network.

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

/// The predicate of the default-protocol finding against scenario 1, citing [`SOURCE`]. The
/// fixed takt opens its case on the right protocol and exits 3 as the defect's does, so the
/// predicate names 3 as the healthy exit too and the output tells the two apart.
fn predicate() -> String {
    format!(
        "scenario: 1\ncommand: \"$TAKT\" run --repo . --harness none\nexit: 3\nhealthy_exit: 3\noutput: on software.change/1\nsource: {SOURCE}\n"
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

/// A predicate's command line that writes `script` to `$HOME/<name>`, in the predicate's own
/// home, through base64: the scratch this suite writes may lie under a home the check hides
/// from the command, so a script there is out of its sight.
fn carried(name: &str, script: &str) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut encoded = String::new();
    for chunk in script.as_bytes().chunks(3) {
        let bytes = [
            chunk[0],
            chunk.get(1).copied().unwrap_or(0),
            chunk.get(2).copied().unwrap_or(0),
        ];
        let word = (u32::from(bytes[0]) << 16) | (u32::from(bytes[1]) << 8) | u32::from(bytes[2]);
        for at in 0..4 {
            if at <= chunk.len() {
                encoded.push(ALPHABET[((word >> (18 - 6 * at)) & 63) as usize] as char);
            } else {
                encoded.push('=');
            }
        }
    }
    format!("echo {encoded} | base64 -d > \"$HOME/{name}\"")
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
            "a healthy exit that is no number",
            body(Some(&replaced("healthy_exit", "zero"))),
        ),
        (
            "a healthy exit past 255",
            body(Some(&replaced("healthy_exit", "256"))),
        ),
        (
            "a healthy exit the check reads as its timeout",
            body(Some(&replaced("healthy_exit", "124"))),
        ),
        (
            "a command that never runs the takt under test",
            body(Some(&replaced(
                "command",
                "echo on software.change/1; exit 3",
            ))),
        ),
        (
            "a command naming only a variable that starts like TAKT",
            body(Some(&replaced("command", "\"$TAKTX\" run --repo ."))),
        ),
        (
            "an output shorter than 8 characters",
            body(Some(&replaced("output", "on soft"))),
        ),
        (
            "a defect's exit that is the healthy one, with no output to tell them apart",
            body(Some(&without("output"))),
        ),
        (
            "a defect's exit 0, the healthy exit when none is named, with no output",
            body(Some(
                &without("output")
                    .lines()
                    .filter(|line| !line.starts_with("healthy_exit:"))
                    .map(|line| {
                        if line.starts_with("exit:") {
                            "exit: 0\n".to_owned()
                        } else {
                            format!("{line}\n")
                        }
                    })
                    .collect::<String>(),
            )),
        ),
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
    // At the edges: `${TAKT}` runs the takt under test, and 8 characters are output enough.
    let edges = replaced("command", "${TAKT} run --repo . --harness none")
        .replace("output: on software.change/1", "output: change/1");
    let output = file(&repo, "edges", &body(Some(&edges)), &[]);
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "filed\n",
        "{}",
        shown(&output)
    );
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
    /// reached it. The fixed takt exits 3 as well, its healthy exit.
    fn repository_predicate(&self) -> String {
        format!(
            "repository: {REPOSITORY_URL}\nseed: {}\ntest: ./check.sh\ncommand: test \"$TEST\" = ./check.sh && \"$TAKT\" run --repo .\nexit: 3\nhealthy_exit: 3\noutput: on software.change/1\nsource: {SOURCE}\n",
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

/// Runs `check-finding` as [`World::check`] does, with `held` open for reading on descriptor 7
/// of its process, as a caller that leaves a descriptor open would.
fn check_holding(
    world: &World,
    finding: &str,
    revision: &str,
    env: &[(&str, &str)],
    held: &Path,
) -> Output {
    Command::new("bash")
        .args(["-c", "exec 7<\"$1\" && shift && exec \"$@\"", "bash"])
        .arg(held)
        .arg(Path::new(SCRIPTS).join("check-finding"))
        .args([
            "--scenarios",
            world.scenarios.to_str().expect("UTF-8"),
            finding,
            revision,
        ])
        .current_dir(&world.takt)
        .env("XDG_CACHE_HOME", world.cache())
        .envs(env.iter().copied())
        .output()
        .expect("check-finding runs")
}

/// The run directory a check names on standard error (`check-finding: record: <dir>`).
fn record(output: &Output) -> PathBuf {
    let stderr = String::from_utf8_lossy(&output.stderr);
    let line = stderr
        .lines()
        .find_map(|line| line.strip_prefix("check-finding: record: "))
        .unwrap_or_else(|| panic!("no record named: {}", shown(output)));
    PathBuf::from(line)
}

/// story:check-finding-positive-control: a check says `not_reproduced` only when the command
/// ended with the exit the predicate names as the healthy one (`healthy_exit`, 0 when the
/// predicate names none) and the defect's expectations did not all hold. A command that panics,
/// rejects its flags, never starts, or ends with any other status the predicate does not name
/// as the healthy one gives `error`, exit 0, the status named on standard error and in the
/// check's record; `reproduced` still needs every expectation, and an exit the predicate names.
#[test]
fn a_command_that_does_not_end_healthy_or_with_the_defect_gives_error_naming_its_status() {
    let world = world("positive-control");
    // Every command runs the takt under test first, as a filed predicate must.
    let defect = |command: &str| {
        format!(
            "scenario: 1\ncommand: \"$TAKT\" --version >/dev/null; {command}\nexit: 3\noutput: on software.change/1\nsource: {SOURCE}\n"
        )
    };
    let cases: Vec<(&str, String, &str, &str, i32)> = vec![
        (
            "panic",
            defect("echo \"thread 'main' panicked at src/main.rs:9:5\" >&2; exit 101"),
            world.defect.as_str(),
            "error",
            101,
        ),
        (
            "flag-rejected",
            defect("echo \"error: unexpected argument '--harness' found\" >&2; exit 2"),
            world.defect.as_str(),
            "error",
            2,
        ),
        (
            "not-started",
            defect("no-such-command-of-check-finding --repo ."),
            world.defect.as_str(),
            "error",
            127,
        ),
        (
            "defect-exit-without-its-output",
            defect("echo case on story.delivery/1; exit 3"),
            world.defect.as_str(),
            "error",
            3,
        ),
        (
            "healthy-without-the-output",
            defect("echo case on story.delivery/1"),
            world.defect.as_str(),
            "not_reproduced",
            0,
        ),
        (
            "healthy-with-the-output",
            defect("echo case on software.change/1"),
            world.defect.as_str(),
            "not_reproduced",
            0,
        ),
        (
            "the-defect",
            defect("echo case on software.change/1; exit 3"),
            world.defect.as_str(),
            "reproduced",
            3,
        ),
        (
            "a-named-healthy-exit",
            format!(
                "scenario: 1\ncommand: \"$TAKT\" --version >/dev/null; echo case on story.delivery/1; exit 3\nexit: 3\nhealthy_exit: 3\noutput: on software.change/1\nsource: {SOURCE}\n"
            ),
            world.defect.as_str(),
            "not_reproduced",
            3,
        ),
        (
            "output-only-crash",
            format!(
                "scenario: 1\ncommand: \"$TAKT\" --version >/dev/null; echo case on software.change/1; exit 101\noutput: on software.change/1\nsource: {SOURCE}\n"
            ),
            world.defect.as_str(),
            "error",
            101,
        ),
        // The takt the fix builds opens its case on the right protocol and exits 3, as the
        // defect's does: a predicate that names no healthy exit cannot call that fixed.
        (
            "fixed-takt-no-healthy-exit",
            defect("\"$TAKT\" run --repo . --harness none"),
            world.fixed.as_str(),
            "error",
            3,
        ),
        (
            "defect-takt-no-healthy-exit",
            defect("\"$TAKT\" run --repo . --harness none"),
            world.defect.as_str(),
            "reproduced",
            3,
        ),
    ];
    for (key, predicate, revision, result, status) in &cases {
        let filed = file(&world.takt, key, &body(Some(predicate)), &[]);
        assert!(filed.status.success(), "{key}: {}", shown(&filed));
        let output = world.check(&format!("story:{key}"), revision, &[]);
        assert!(output.status.success(), "{key}: {}", shown(&output));
        assert_eq!(
            String::from_utf8_lossy(&output.stdout),
            format!("{result}\n"),
            "{key}: {}",
            shown(&output)
        );
        let recorded = fs::read_to_string(record(&output).join("result"))
            .unwrap_or_else(|error| panic!("{key}: the check's record: {error}"));
        assert!(
            recorded.contains(&format!("exit: {status}\nresult: {result}\n")),
            "{key}: {recorded}"
        );
        if *result == "error" {
            let stderr = String::from_utf8_lossy(&output.stderr);
            assert!(
                stderr.contains(&format!("exit {status}, which the predicate names")),
                "{key}: the status is named: {}",
                shown(&output)
            );
        }
    }
}

/// The `build.rs` of a revision that probes its build's confinement, the paths it tries filled
/// in by [`probe_build`]: it writes `$HOME/canary`, writes and reads under the caller's home by
/// its path, overwrites another revision's kept binary and plants a file beside it, looks for
/// the caller's secret in a descriptor it holds, and copies its whole environment; each
/// attempt and the environment go to `probe.txt` in its `OUT_DIR`,
/// which the binary prints for `--probe`. Every attempt is recorded, none fails the build.
const PROBE_BUILD: &str = r#"use std::fmt::Write as _;

fn attempt(what: &str, result: std::io::Result<()>, out: &mut String) {
    match result {
        Ok(()) => writeln!(out, "{what}: done").unwrap(),
        Err(error) => writeln!(out, "{what}: refused: {error}").unwrap(),
    }
}

/// Done when a descriptor this process holds is a regular file naming the planted secret.
fn held_secret() -> std::io::Result<()> {
    for entry in std::fs::read_dir("/proc/self/fd")? {
        let path = entry?.path();
        let regular = std::fs::metadata(&path).is_ok_and(|meta| meta.is_file());
        if regular
            && std::fs::read_to_string(&path).is_ok_and(|text| text.contains("PLANTED-HOME-SECRET"))
        {
            return Ok(());
        }
    }
    Err(std::io::Error::other("no descriptor held names the secret"))
}

fn main() {
    let mut out = String::new();
    let home = std::env::var("HOME").unwrap_or_default();
    attempt("home-canary", std::fs::write(format!("{home}/canary"), "build.rs\n"), &mut out);
    attempt("callers-home-canary", std::fs::write("@HOME@/canary", "build.rs\n"), &mut out);
    attempt(
        "callers-home-secret",
        std::fs::read_to_string("@HOME@/.secret").map(|_| ()),
        &mut out,
    );
    attempt("other-build", std::fs::write("@OTHER@/takt", "build.rs\n"), &mut out);
    attempt("other-build-planted", std::fs::write("@OTHER@/planted", "build.rs\n"), &mut out);
    attempt("inherited-fd", held_secret(), &mut out);
    for (name, value) in std::env::vars() {
        writeln!(out, "env {name}={value:?}").unwrap();
    }
    let dir = std::env::var("OUT_DIR").unwrap();
    std::fs::write(format!("{dir}/probe.txt"), out).unwrap();
    println!("cargo:rustc-env=TAKT_REVISION=probe");
}
"#;

/// The probing revision's `main.rs`: `--probe` prints what its build script recorded.
const PROBE_MAIN: &str = r#"fn main() {
    if std::env::args().any(|arg| arg == "--probe") {
        print!("{}", include_str!(concat!(env!("OUT_DIR"), "/probe.txt")));
        return;
    }
    println!("takt 0.1.0 ({})", env!("TAKT_REVISION"));
}
"#;

/// [`PROBE_BUILD`] with the caller's home `home` and the other revision's kept build directory
/// `other`.
fn probe_build(home: &Path, other: &Path) -> String {
    PROBE_BUILD
        .replace("@HOME@", home.to_str().expect("UTF-8"))
        .replace("@OTHER@", other.to_str().expect("UTF-8"))
}

/// The variables a confined build's script may see: those the check sets (`HOME`, `TMPDIR`,
/// `PATH`, `LANG`, `TERM`, `CARGO_HOME`, `RUSTC`), the working directory bubblewrap sets, and
/// those cargo sets for a build script (`CARGO`, `CARGO_*`, `DEP_*` and the names below; the
/// two `SSL_CERT_*` cargo sets in its own environment, naming the system's certificates).
fn allowed_in_a_build(name: &str) -> bool {
    const SET: [&str; 22] = [
        "CARGO",
        "SSL_CERT_DIR",
        "SSL_CERT_FILE",
        "HOME",
        "TMPDIR",
        "PATH",
        "LANG",
        "TERM",
        "PWD",
        "RUSTC",
        "RUSTDOC",
        "RUSTC_LINKER",
        "RUSTC_WRAPPER",
        "RUSTC_WORKSPACE_WRAPPER",
        "OUT_DIR",
        "TARGET",
        "HOST",
        "NUM_JOBS",
        "OPT_LEVEL",
        "PROFILE",
        "DEBUG",
        "LD_LIBRARY_PATH",
    ];
    SET.contains(&name) || name.starts_with("CARGO_") || name.starts_with("DEP_")
}

/// story:check-finding-builds-confined: the build of the revision under check runs confined.
/// A revision whose `build.rs` writes `$HOME/canary`, writes and reads under the caller's home,
/// overwrites another revision's kept binary and copies its environment still builds, and
/// leaves no canary in the caller's home and the other revision's build as it was; its
/// environment holds the allow-list only, nothing the caller set, and not the caller's home;
/// and the secret file the caller left open on a descriptor reaches it on none.
#[test]
fn a_revisions_build_reads_nothing_of_the_callers_home_or_environment_and_writes_only_its_own() {
    let world = world("confined");
    let filed = file(
        &world.takt,
        "default-protocol",
        &body(Some(&predicate())),
        &[],
    );
    assert!(filed.status.success(), "{}", shown(&filed));
    let output = world.check("story:default-protocol", &world.defect, &[]);
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "reproduced\n",
        "the ordinary build works: {}",
        shown(&output)
    );
    let other = world
        .cache()
        .join("takt/check-finding/builds")
        .join(&world.defect);
    let kept = fs::read(other.join("takt")).expect("the defect's kept binary");

    let home = world.outer.0.join("callers-home");
    write(&home.join(".secret"), "PLANTED-HOME-SECRET\n");
    write(
        &world.takt.join("crates/takt-cli/build.rs"),
        &probe_build(&home, &other),
    );
    write(&world.takt.join("crates/takt-cli/src/main.rs"), PROBE_MAIN);
    git(&world.takt, &["add", "-A"]);
    git(&world.takt, &["commit", "-q", "-m", "probe"]);
    let probe = git_out(&world.takt, &["rev-parse", "HEAD"]);
    let filed = file(
        &world.takt,
        "probe",
        &body(Some(&format!(
            "scenario: 1\ncommand: \"$TAKT\" --probe\nexit: 0\noutput: other-build:\nsource: {SOURCE}\n"
        ))),
        &[],
    );
    assert!(filed.status.success(), "{}", shown(&filed));
    let shared = world.outer.0.join("shared-target");
    let output = check_holding(
        &world,
        "story:probe",
        &probe,
        &[
            ("HOME", home.to_str().expect("UTF-8")),
            ("CHECK_FINDING_PLANTED", "PLANTED-CREDENTIAL"),
            ("GITHUB_TOKEN", "PLANTED-CREDENTIAL"),
            ("CARGO_TARGET_DIR", shared.to_str().expect("UTF-8")),
        ],
        &home.join(".secret"),
    );
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "reproduced\n",
        "the probing revision builds: {}",
        shown(&output)
    );
    let printed = Command::new(
        world
            .cache()
            .join("takt/check-finding/builds")
            .join(&probe)
            .join("takt"),
    )
    .arg("--probe")
    .output()
    .expect("the probe's binary runs");
    let printed = String::from_utf8_lossy(&printed.stdout).into_owned();
    // A message names the build's variables, never their values: under a confinement that
    // fails, a value is the caller's, a credential among them.
    let attempts: String = printed
        .lines()
        .filter(|line| !line.starts_with("env "))
        .map(|line| format!("{line}\n"))
        .collect();
    let environment: Vec<(&str, &str)> = printed
        .lines()
        .filter_map(|line| line.strip_prefix("env "))
        .filter_map(|line| line.split_once('='))
        .collect();
    let names: Vec<&str> = environment.iter().map(|(name, _)| *name).collect();

    assert!(
        !home.join("canary").exists(),
        "the build wrote into the caller's home:\n{attempts}"
    );
    assert_eq!(
        fs::read(other.join("takt")).expect("the defect's kept binary"),
        kept,
        "the build overwrote another revision's kept binary:\n{attempts}"
    );
    assert!(
        !other.join("planted").exists(),
        "the build wrote into another revision's build directory:\n{attempts}"
    );
    assert!(
        !shared.exists(),
        "the build used the caller's CARGO_TARGET_DIR"
    );
    for attempt in [
        "callers-home-canary",
        "callers-home-secret",
        "other-build",
        "other-build-planted",
        "inherited-fd",
    ] {
        assert!(
            printed
                .lines()
                .any(|line| line.starts_with(&format!("{attempt}: refused: "))),
            "{attempt} is refused, and the record says why:\n{attempts}"
        );
    }
    assert!(
        names.contains(&"OUT_DIR"),
        "the environment was copied: {names:?}"
    );
    let outside: Vec<_> = environment
        .iter()
        .filter(|(name, _)| !allowed_in_a_build(name))
        .map(|(name, _)| *name)
        .collect();
    assert!(
        outside.is_empty(),
        "variables outside the allow-list reached the build: {outside:?}"
    );
    let home_text = home.to_str().expect("UTF-8");
    let leaked: Vec<_> = environment
        .iter()
        .filter(|(_, value)| value.contains("PLANTED") || value.contains(home_text))
        .map(|(name, _)| *name)
        .collect();
    assert!(
        leaked.is_empty(),
        "the caller's environment or home reached the build: {leaked:?}"
    );
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("confined in bwrap"),
        "the check says the build is confined: {}",
        shown(&output)
    );
}

/// Without bubblewrap on `PATH` a revision is never built unconfined: the check gives no
/// result, says why, and keeps no binary.
#[test]
fn a_check_without_bubblewrap_builds_nothing_and_gives_no_result() {
    use std::os::unix::fs::symlink;

    let world = world("no-bwrap");
    let filed = file(
        &world.takt,
        "default-protocol",
        &body(Some(&predicate())),
        &[],
    );
    assert!(filed.status.success(), "{}", shown(&filed));
    // A PATH holding every program of the caller's but bubblewrap.
    let bin = world.outer.0.join("bin-without-bwrap");
    fs::create_dir_all(&bin).expect("a directory");
    let path = std::env::var_os("PATH").expect("a PATH");
    for dir in std::env::split_paths(&path) {
        let Ok(entries) = fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let name = entry.file_name();
            if name == "bwrap" || bin.join(&name).symlink_metadata().is_ok() {
                continue;
            }
            symlink(entry.path(), bin.join(&name)).expect("a link");
        }
    }
    let output = world.check(
        "story:default-protocol",
        &world.defect,
        &[("PATH", bin.to_str().expect("UTF-8"))],
    );
    assert_eq!(output.status.code(), Some(1), "{}", shown(&output));
    assert!(output.stdout.is_empty(), "{}", shown(&output));
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("bubblewrap"),
        "{}",
        shown(&output)
    );
    assert!(
        !world
            .cache()
            .join("takt/check-finding/builds")
            .join(&world.defect)
            .join("takt")
            .exists(),
        "a binary was built without bubblewrap"
    );
}

/// A bubblewrap that forks the command's process and then fails its mount setup never runs the
/// command, and its own exit 1 is not the command's: a predicate naming 1 as the defect's exit,
/// or as the healthy one, gets no result, and the check names bubblewrap's error. Bubblewrap's
/// status names the command's pid in both cases; only an `exit-code` line, its last, says that
/// bubblewrap ran the command to its end.
#[test]
fn a_bubblewrap_that_fails_before_the_command_runs_gives_no_result() {
    use std::os::unix::fs::PermissionsExt;

    let world = world("bwrap-setup-fails");
    let real = Command::new("sh")
        .args(["-c", "command -v bwrap"])
        .output()
        .expect("sh runs");
    let real = String::from_utf8_lossy(&real.stdout).trim().to_owned();
    assert!(!real.is_empty(), "bubblewrap is on PATH");
    // The predicate's call alone (the one with a status fd) gets a bind whose source is absent,
    // so bubblewrap's mount setup fails after it forked; the build's call runs as it is.
    let bin = world.outer.0.join("bin-failing-bwrap");
    write(
        &bin.join("bwrap"),
        &format!(
            "#!/bin/sh\ncase \" $* \" in *' --json-status-fd '*) exec {real} --ro-bind /nonexistent-check-finding /srv/absent \"$@\";; esac\nexec {real} \"$@\"\n"
        ),
    );
    fs::set_permissions(bin.join("bwrap"), fs::Permissions::from_mode(0o755)).expect("executable");
    let path = std::env::var("PATH").expect("a PATH");
    let path = format!("{}:{path}", bin.display());
    let cases = [
        (
            "exit-one",
            format!("scenario: 1\ncommand: \"$TAKT\" --version\nexit: 1\nsource: {SOURCE}\n"),
        ),
        (
            "healthy-one",
            format!(
                "scenario: 1\ncommand: \"$TAKT\" run --repo .\nexit: 3\nhealthy_exit: 1\noutput: on software.change/1\nsource: {SOURCE}\n"
            ),
        ),
    ];
    for (key, predicate) in cases {
        let filed = file(&world.takt, key, &body(Some(&predicate)), &[]);
        assert!(filed.status.success(), "{key}: {}", shown(&filed));
        let output = world.check(&format!("story:{key}"), &world.defect, &[("PATH", &path)]);
        assert_eq!(output.status.code(), Some(1), "{key}: {}", shown(&output));
        assert!(output.stdout.is_empty(), "{key}: {}", shown(&output));
        assert!(
            String::from_utf8_lossy(&output.stderr)
                .contains("bwrap: Can't find source path /nonexistent-check-finding"),
            "{key}: the check names bubblewrap's error: {}",
            shown(&output)
        );
        assert!(
            !record(&output).join("result").exists(),
            "{key}: a result was recorded"
        );
    }
}

/// A process a predicate leaves running: it leaves `started` in the predicate's home, then
/// waits for another check's bubblewrap status file (beside its own run directory, `$HOME/..`)
/// holding the command's pid and no exit, appends `{ "exit-code": 1 }` to it and ends; it gives
/// up after 30 seconds. Half a second in, it leaves `alive` in the predicate's home.
const STALE: &str = r#"own=$(dirname "$HOME")
runs=$(dirname "$own")
: >"$HOME/started"
i=0
while [ "$i" -lt 600 ]; do
    [ "$i" -eq 10 ] && : >"$HOME/alive"
    for f in "$runs"/*/bwrap-status; do
        [ -f "$f" ] && [ "$f" != "$own/bwrap-status" ] || continue
        grep -q exit-code "$f" && continue
        grep -q child-pid "$f" || continue
        printf '{ "exit-code": 1 }\n' >>"$f"
        exit 0
    done
    sleep 0.05
    i=$((i + 1))
done
"#;

/// story:check-finding-run-step-confined, the review's finding A: a process one check's
/// predicate leaves running dies with that check (the command runs in a PID namespace of its
/// own), and bubblewrap's status reaches the check through a pipe, never a file another check's
/// process can append to. The next check, whose bubblewrap fails its mount setup after a
/// second's delay and so never runs its command, gives no result, though its predicate names
/// the 1 bubblewrap exits with. The command waits until the process it leaves has started.
#[test]
fn a_process_left_running_by_one_check_cannot_change_the_next_checks_result() {
    use std::os::unix::fs::PermissionsExt;

    let world = world("stale-process");
    let (_, env) = callers_home(&world);
    let env: Vec<(&str, &str)> = env
        .iter()
        .map(|(name, value)| (*name, value.as_str()))
        .collect();
    let filed = file(
        &world.takt,
        "leaves-a-process",
        &body(Some(&format!(
            "scenario: 1\ncommand: \"$TAKT\" --version; {} && sh \"$HOME/stale.sh\" </dev/null >/dev/null 2>&1 & i=0; while [ ! -e \"$HOME/started\" ] && [ $i -lt 500 ]; do sleep 0.01; i=$((i + 1)); done\nexit: 0\noutput: takt 0.1.0\nsource: {SOURCE}\n",
            carried("stale.sh", STALE)
        ))),
        &[],
    );
    assert!(filed.status.success(), "{}", shown(&filed));
    let filed = file(
        &world.takt,
        "exit-one",
        &body(Some(&format!(
            "scenario: 1\ncommand: \"$TAKT\" --version\nexit: 1\nsource: {SOURCE}\n"
        ))),
        &[],
    );
    assert!(filed.status.success(), "{}", shown(&filed));
    let first = world.check("story:leaves-a-process", &world.defect, &env);
    assert_eq!(
        String::from_utf8_lossy(&first.stdout),
        "reproduced\n",
        "{}",
        shown(&first)
    );
    let real = Command::new("sh")
        .args(["-c", "command -v bwrap"])
        .output()
        .expect("sh runs");
    let real = String::from_utf8_lossy(&real.stdout).trim().to_owned();
    assert!(!real.is_empty(), "bubblewrap is on PATH");
    let bin = world.outer.0.join("bin-slow-failing-bwrap");
    write(
        &bin.join("bwrap"),
        &format!(
            "#!/bin/sh\ncase \" $* \" in *' --json-status-fd '*) {real} --ro-bind /nonexistent-check-finding /srv/absent \"$@\"; s=$?; sleep 1; exit $s;; esac\nexec {real} \"$@\"\n"
        ),
    );
    fs::set_permissions(bin.join("bwrap"), fs::Permissions::from_mode(0o755)).expect("executable");
    let path = format!(
        "{}:{}",
        bin.display(),
        std::env::var("PATH").expect("a PATH")
    );
    let mut with_path = env.clone();
    with_path.push(("PATH", &path));
    let second = world.check("story:exit-one", &world.defect, &with_path);
    assert_eq!(second.status.code(), Some(1), "{}", shown(&second));
    assert!(second.stdout.is_empty(), "{}", shown(&second));
    assert!(
        String::from_utf8_lossy(&second.stderr)
            .contains("bwrap: Can't find source path /nonexistent-check-finding"),
        "the check names bubblewrap's error: {}",
        shown(&second)
    );
    assert!(
        record(&first).join("home/started").exists(),
        "the first check's predicate started no process"
    );
    assert!(
        !record(&first).join("home/alive").exists(),
        "the process the first check's predicate left was still running a second later"
    );
}

/// The command's exit is bubblewrap's only when the last line bubblewrap writes on its status
/// pipe is `{ "exit-code": N }` and N is the status bubblewrap exited with. A bubblewrap that
/// exits 1 after its command ended 0 gives no result for a predicate naming 1 (the review's
/// mutant M8b, the line read but not compared, takes the 1 as the command's); an `exit-code`
/// line written ahead of bubblewrap's own is not the one read (M8d, the first line for the
/// last, reads it).
#[test]
fn only_the_last_line_bubblewrap_writes_gives_the_commands_exit() {
    use std::os::unix::fs::PermissionsExt;

    let world = world("bwrap-last-word");
    let real = Command::new("sh")
        .args(["-c", "command -v bwrap"])
        .output()
        .expect("sh runs");
    let real = String::from_utf8_lossy(&real.stdout).trim().to_owned();
    assert!(!real.is_empty(), "bubblewrap is on PATH");
    let filed = file(
        &world.takt,
        "exit-one",
        &body(Some(&format!(
            "scenario: 1\ncommand: \"$TAKT\" --version\nexit: 1\nsource: {SOURCE}\n"
        ))),
        &[],
    );
    assert!(filed.status.success(), "{}", shown(&filed));
    let wrapper = |name: &str, predicate_call: &str| {
        let bin = world.outer.0.join(name);
        write(
            &bin.join("bwrap"),
            &format!(
                "#!/bin/sh\ncase \" $* \" in *' --json-status-fd '*) {predicate_call};; esac\nexec {real} \"$@\"\n"
            ),
        );
        fs::set_permissions(bin.join("bwrap"), fs::Permissions::from_mode(0o755))
            .expect("executable");
        format!(
            "{}:{}",
            bin.display(),
            std::env::var("PATH").expect("a PATH")
        )
    };

    let path = wrapper("bin-exits-otherwise", &format!("{real} \"$@\"; exit 1"));
    let output = world.check("story:exit-one", &world.defect, &[("PATH", &path)]);
    assert_eq!(output.status.code(), Some(1), "M8b: {}", shown(&output));
    assert!(output.stdout.is_empty(), "M8b: {}", shown(&output));
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("(exit 1, the command ended 0)"),
        "M8b: the check names both statuses: {}",
        shown(&output)
    );

    let path = wrapper(
        "bin-leading-line",
        &format!("printf '{{ \"exit-code\": 1 }}\\n' >&3; exec {real} \"$@\""),
    );
    let output = world.check("story:exit-one", &world.defect, &[("PATH", &path)]);
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "not_reproduced\n",
        "M8d: {}",
        shown(&output)
    );
}

/// The binary kept for a revision is the one cargo reports building, never a file the revision
/// commits where a build would put it: a revision whose `.cargo/config.toml` sends the build to
/// `target/<host triple>/` and which commits a stand-in `target/debug/takt` is built and its
/// own takt kept, the stand-in never run.
#[test]
fn the_binary_kept_is_the_one_cargo_built_not_one_the_revision_commits() {
    use std::os::unix::fs::PermissionsExt;

    let world = world("committed-binary");
    let rustc = Command::new("rustc")
        .arg("-vV")
        .output()
        .expect("rustc runs");
    let host = String::from_utf8_lossy(&rustc.stdout)
        .lines()
        .find_map(|line| line.strip_prefix("host: ").map(str::to_owned))
        .expect("rustc names its host");
    git(&world.takt, &["checkout", "-q", &world.defect]);
    write(
        &world.takt.join(".cargo/config.toml"),
        &format!("[build]\ntarget = \"{host}\"\n"),
    );
    let fake = world.takt.join("target/debug/takt");
    write(
        &fake,
        "#!/bin/sh\necho FAKE-TAKT-RAN\necho case story-ready-one on story.delivery/1\nexit 3\n",
    );
    fs::set_permissions(&fake, fs::Permissions::from_mode(0o755)).expect("executable");
    git(
        &world.takt,
        &["add", "-f", ".cargo/config.toml", "target/debug/takt"],
    );
    git(&world.takt, &["commit", "-q", "-m", "a committed stand-in"]);
    let planted = git_out(&world.takt, &["rev-parse", "HEAD"]);
    git(&world.takt, &["checkout", "-q", "main"]);
    let filed = file(
        &world.takt,
        "default-protocol",
        &body(Some(&predicate())),
        &[],
    );
    assert!(filed.status.success(), "{}", shown(&filed));
    let output = world.check("story:default-protocol", &planted, &[]);
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "reproduced\n",
        "the revision's own takt ran: {}",
        shown(&output)
    );
    let ran = fs::read_to_string(record(&output).join("output")).expect("the command's output");
    assert!(!ran.contains("FAKE-TAKT-RAN"), "the stand-in ran: {ran}");
    let kept = world
        .cache()
        .join("takt/check-finding/builds")
        .join(&planted)
        .join("takt");
    let version = Command::new(&kept)
        .arg("--version")
        .output()
        .expect("the kept binary runs");
    assert_eq!(
        String::from_utf8_lossy(&version.stdout),
        format!("takt 0.1.0 ({planted})\n"),
        "the kept binary is the one built from the revision"
    );
}

/// The protocol sources `aep` reads are bound read-only into the predicate's home under
/// bubblewrap, neither linked nor copied: a command that writes through them changes nothing in
/// the caller's cache, and the home on disk holds no copy.
#[test]
fn the_predicate_cannot_write_the_callers_protocol_sources() {
    let world = world("protocol-sources");
    let sources = world.cache().join("aep/protocol-sources");
    let original = "kind: protocol\n";
    write(&sources.join("abc/protocol.yaml"), original);
    let command = "\"$TAKT\" --version >/dev/null; d=\"$HOME/.cache/aep/protocol-sources\"; \
                   test -f \"$d/abc/protocol.yaml\" && echo sources-are-there; \
                   { echo changed > \"$d/abc/protocol.yaml\"; } 2>/dev/null || echo file-write-refused; \
                   { echo new > \"$d/abc/planted\"; } 2>/dev/null || echo dir-write-refused; true";
    let filed = file(
        &world.takt,
        "sources",
        &body(Some(&format!(
            "scenario: 1\ncommand: {command}\nexit: 0\noutput: sources-are-there\nsource: {SOURCE}\n"
        ))),
        &[],
    );
    assert!(filed.status.success(), "{}", shown(&filed));
    let output = world.check("story:sources", &world.defect, &[]);
    let ran = fs::read_to_string(record(&output).join("output")).unwrap_or_default();
    let mount_point = record(&output).join("home/.cache/aep/protocol-sources");
    let held: Vec<_> = fs::read_dir(&mount_point)
        .map(|entries| entries.flatten().map(|entry| entry.path()).collect())
        .unwrap_or_default();
    assert!(
        held.is_empty() && !fs::symlink_metadata(&mount_point).is_ok_and(|meta| meta.is_symlink()),
        "the predicate's home holds a copy or a link of the sources: {held:?}"
    );
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "reproduced\n",
        "the predicate sees the sources: {}",
        shown(&output)
    );
    for line in ["file-write-refused", "dir-write-refused"] {
        assert!(ran.lines().any(|ran| ran == line), "{line} in {ran}");
    }
    assert_eq!(
        fs::read_to_string(sources.join("abc/protocol.yaml")).expect("the caller's source"),
        original,
        "the predicate wrote the caller's protocol source"
    );
    assert!(
        !sources.join("abc/planted").exists(),
        "the predicate wrote into the caller's protocol sources"
    );
    assert!(
        fs::symlink_metadata(&sources).is_ok_and(|meta| meta.is_dir()),
        "the caller's protocol sources are still a directory"
    );
}

/// The caller's home a check is run with, holding a planted secret and a `.cache`; the
/// variables that let the check find the toolchain and the registry caches without it.
fn callers_home(world: &World) -> (PathBuf, Vec<(&'static str, String)>) {
    let real_home = std::env::var("HOME").expect("HOME");
    let rustup = std::env::var("RUSTUP_HOME").unwrap_or_else(|_| format!("{real_home}/.rustup"));
    let cargo = std::env::var("CARGO_HOME").unwrap_or_else(|_| format!("{real_home}/.cargo"));
    let home = world.outer.0.join("callers-home");
    write(&home.join(".secret"), "PLANTED-HOME-SECRET\n");
    fs::create_dir_all(home.join(".cache")).expect("a directory");
    let env = vec![
        ("HOME", home.to_str().expect("UTF-8").to_owned()),
        ("RUSTUP_HOME", rustup),
        ("CARGO_HOME", cargo),
    ];
    (home, env)
}

/// What a predicate tries on the host, by real path: `@HOME@` the caller's home, `@REPO@` this
/// repository, `@SOURCES@` the caller's protocol sources. Each write is recorded as `done` or
/// `refused: <the shell's error>`; the secret planted in the caller's home as `read` or
/// `refused`. Carried into the predicate's home by [`carried`] and run from there.
const RUN_PROBE: &str = r#"attempt() {
    if { printf 'written by the predicate\n' >"$2"; } 2>"$HOME/attempt.err"; then
        echo "$1: done"
    else
        echo "$1: refused: $(cat "$HOME/attempt.err")"
    fi
}
attempt callers-cache "@HOME@/.cache/canary"
attempt repository "@REPO@/planted"
attempt repository-file "@REPO@/Cargo.toml"
attempt kept-binary "$TAKT"
attempt kept-build "${TAKT%/takt}/planted"
attempt protocol-source "@SOURCES@/abc/protocol.yaml"
if grep -q PLANTED "@HOME@/.secret" 2>/dev/null; then
    echo "callers-home-secret: read"
else
    echo "callers-home-secret: refused"
fi
echo probe-ran
"#;

/// story:check-finding-run-step-confined: the predicate's command runs with every host path
/// read-only but the clone and its own home, and the caller's home hidden. In the default
/// layout (no XDG_CACHE_HOME, the cache under the caller's home), a command that writes into
/// the caller's `~/.cache`, the kept `builds/<sha>/takt` beside a file of its own, and a
/// protocol source by its real path gets a read-only error on each; one that writes into this
/// repository is refused too, with a read-only error, or with no such file where the
/// repository lies under a home the check hides (the user's own, here, when the scratch lies
/// under it); one that reads a secret planted in the caller's home does not see it; and the
/// check still gives its result.
#[test]
fn the_predicate_writes_no_host_path_and_reads_nothing_of_the_callers_home() {
    let world = world("run-confined");
    let (home, env) = callers_home(&world);
    let sources = home.join(".cache/aep/protocol-sources");
    let original = "kind: protocol\n";
    write(&sources.join("abc/protocol.yaml"), original);
    let manifest = fs::read(world.takt.join("Cargo.toml")).expect("the repository's manifest");
    let probe = RUN_PROBE
        .replace("@HOME@", home.to_str().expect("UTF-8"))
        .replace("@REPO@", world.takt.to_str().expect("UTF-8"))
        .replace("@SOURCES@", sources.to_str().expect("UTF-8"));
    let filed = file(
        &world.takt,
        "run-probe",
        &body(Some(&format!(
            "scenario: 1\ncommand: \"$TAKT\" --version >/dev/null; {} && sh \"$HOME/run-probe.sh\"\nexit: 0\noutput: probe-ran\nsource: {SOURCE}\n",
            carried("run-probe.sh", &probe)
        ))),
        &[],
    );
    assert!(filed.status.success(), "{}", shown(&filed));
    let output = Command::new(Path::new(SCRIPTS).join("check-finding"))
        .args([
            "--scenarios",
            world.scenarios.to_str().expect("UTF-8"),
            "story:run-probe",
            &world.defect,
        ])
        .current_dir(&world.takt)
        .env_remove("XDG_CACHE_HOME")
        .envs(env.iter().map(|(name, value)| (*name, value.as_str())))
        .output()
        .expect("check-finding runs");
    let ran = fs::read_to_string(record(&output).join("output")).unwrap_or_default();
    let builds = home
        .join(".cache/takt/check-finding/builds")
        .join(&world.defect);
    assert!(
        !home.join(".cache/canary").exists(),
        "the predicate wrote into the caller's ~/.cache:\n{ran}"
    );
    assert!(
        !world.takt.join("planted").exists(),
        "the predicate wrote into the repository:\n{ran}"
    );
    assert_eq!(
        fs::read(world.takt.join("Cargo.toml")).expect("the repository's manifest"),
        manifest,
        "the predicate changed a file of the repository:\n{ran}"
    );
    let version = Command::new(builds.join("takt"))
        .arg("--version")
        .output()
        .map(|version| String::from_utf8_lossy(&version.stdout).into_owned())
        .unwrap_or_default();
    assert_eq!(
        version,
        format!("takt 0.1.0 ({})\n", world.defect),
        "the predicate overwrote the kept binary:\n{ran}"
    );
    assert!(
        !builds.join("planted").exists(),
        "the predicate wrote beside the kept binary:\n{ran}"
    );
    assert_eq!(
        fs::read_to_string(sources.join("abc/protocol.yaml")).expect("the caller's source"),
        original,
        "the predicate wrote a protocol source by its real path:\n{ran}"
    );
    for (attempt, hidden) in [
        ("callers-cache", false),
        ("repository", true),
        ("repository-file", true),
        ("kept-binary", false),
        ("kept-build", false),
        ("protocol-source", false),
    ] {
        let line = ran
            .lines()
            .find(|line| line.starts_with(&format!("{attempt}: ")))
            .unwrap_or_default();
        assert!(
            line.starts_with(&format!("{attempt}: refused: "))
                && (line.contains("Read-only file system")
                    || hidden && line.contains("No such file or directory")),
            "{attempt} gets a read-only error{}:\n{ran}",
            if hidden { ", or none such" } else { "" }
        );
    }
    assert!(
        ran.lines()
            .any(|line| line == "callers-home-secret: refused"),
        "the predicate read the caller's home:\n{ran}"
    );
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "reproduced\n",
        "the check still gives its result: {}",
        shown(&output)
    );
}

/// What a predicate reaches of the host beyond its files: its network and IPC namespaces, the
/// network interfaces and abstract unix sockets it sees, and the host's well-known sockets
/// (the user's session bus, agents and runtime directory under `/run/user`, the container
/// engines', the system bus, X11's); each one in sight is printed as `reachable <path>`.
const SOCKET_PROBE: &str = r#"echo "net $(readlink /proc/self/ns/net)"
echo "ipc $(readlink /proc/self/ns/ipc)"
echo "interfaces $(tail -n +3 /proc/net/dev | cut -d: -f1 | tr -d ' ' | tr '\n' ' ')"
echo "abstract $(grep -c ' @' /proc/net/unix)"
for p in "/run/user/$(id -u)" /run/docker.sock /var/run/docker.sock /run/podman/podman.sock \
    /run/dbus/system_bus_socket /tmp/.X11-unix; do
    [ -e "$p" ] && echo "reachable $p"
done
echo probe-ran
"#;

/// The review's correction 1, finding 1: the command shares no network and no IPC namespace
/// with the host, so it reaches no network, no abstract unix socket (X11's among them) and no
/// System V or POSIX IPC of the host, and it sees nothing of `/run` (the user's session bus,
/// ssh and gpg agents, keyring and podman under `/run/user/<uid>`, the docker socket, the
/// system bus), nor `/var/run/docker.sock`, nor the host's `/tmp`.
#[test]
fn the_predicate_reaches_no_network_ipc_or_socket_of_the_host() {
    let world = world("sockets");
    let filed = file(
        &world.takt,
        "socket-probe",
        &body(Some(&format!(
            "scenario: 1\ncommand: \"$TAKT\" --version >/dev/null; {} && sh \"$HOME/socket-probe.sh\"\nexit: 0\noutput: probe-ran\nsource: {SOURCE}\n",
            carried("socket-probe.sh", SOCKET_PROBE)
        ))),
        &[],
    );
    assert!(filed.status.success(), "{}", shown(&filed));
    let output = world.check("story:socket-probe", &world.defect, &[]);
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "reproduced\n",
        "{}",
        shown(&output)
    );
    let ran = fs::read_to_string(record(&output).join("output")).expect("the command's output");
    let host = |kind: &str| {
        fs::read_link(format!("/proc/self/ns/{kind}"))
            .expect("this process's namespace")
            .display()
            .to_string()
    };
    for kind in ["net", "ipc"] {
        assert!(
            ran.lines()
                .any(|line| line.starts_with(&format!("{kind} ")))
                && !ran
                    .lines()
                    .any(|line| line == format!("{kind} {}", host(kind))),
            "the command shares the host's {kind} namespace:\n{ran}"
        );
    }
    assert!(
        ran.lines().any(|line| line == "interfaces lo "),
        "the command sees a network interface but loopback:\n{ran}"
    );
    assert!(
        ran.lines().any(|line| line == "abstract 0"),
        "the command sees abstract unix sockets:\n{ran}"
    );
    let reachable: Vec<_> = ran
        .lines()
        .filter(|line| line.starts_with("reachable "))
        .collect();
    assert!(
        reachable.is_empty(),
        "the command reaches host sockets: {reachable:?}"
    );
}

/// The review's correction 1, finding 2: the check writes its result, and reads the command's
/// output and its predicate, in its run directory after the command ran there. The command
/// can write its clone and its home and nothing else of that directory, so it plants no link
/// where the check writes: a link at `result` to a path outside, a file of its own beside the
/// record, and the removal of `output` and `predicate.tsv` are each refused, and the check
/// writes its result to its own file.
#[test]
fn the_predicate_cannot_plant_a_link_where_the_check_writes() {
    let world = world("record-link");
    let victim = world.outer.0.join("outside-the-record");
    let filed = file(
        &world.takt,
        "record-link",
        &body(Some(&format!(
            "scenario: 1\ncommand: \"$TAKT\" --version; r=\"$HOME/..\"; ln -s {} \"$r/result\" 2>/dev/null && echo linked; touch \"$r/planted\" 2>/dev/null && echo planted; rm -f \"$r/output\" \"$r/predicate.tsv\" 2>/dev/null; true\nexit: 0\noutput: takt 0.1.0\nsource: {SOURCE}\n",
            victim.display()
        ))),
        &[],
    );
    assert!(filed.status.success(), "{}", shown(&filed));
    let output = world.check("story:record-link", &world.defect, &[]);
    let run = record(&output);
    assert!(
        !victim.exists(),
        "the check wrote through a link the command planted: {}",
        shown(&output)
    );
    let result = fs::symlink_metadata(run.join("result"));
    assert!(
        result.as_ref().is_ok_and(|meta| meta.is_file()),
        "the result is no file of the check's own: {result:?}"
    );
    assert!(
        !run.join("planted").exists(),
        "the command wrote into the record"
    );
    for kept in ["output", "predicate.tsv"] {
        assert!(
            fs::symlink_metadata(run.join(kept)).is_ok_and(|meta| meta.is_file()),
            "the command removed {kept}"
        );
    }
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "reproduced\n",
        "{}",
        shown(&output)
    );
}

/// The review's correction 1, finding 3: the check hides the home of the user it runs as
/// (the passwd entry of `id -u`), not only the directory HOME names. Under a HOME of the
/// scratch's, no file directly in the user's passwd home (a shell profile, a git
/// configuration) is in the command's sight.
#[test]
fn the_predicate_sees_no_file_of_the_users_passwd_home() {
    let world = world("passwd-home");
    let (_, env) = callers_home(&world);
    let env: Vec<(&str, &str)> = env
        .iter()
        .map(|(name, value)| (*name, value.as_str()))
        .collect();
    let filed = file(
        &world.takt,
        "passwd-home",
        &body(Some(&format!(
            "scenario: 1\ncommand: \"$TAKT\" --version >/dev/null; pw=$(getent passwd \"$(id -u)\" | cut -d: -f6); echo \"passwd-home $pw\"; for f in \"$pw\"/.* \"$pw\"/*; do [ -f \"$f\" ] && echo \"visible $f\"; done; echo probe-ran\nexit: 0\noutput: probe-ran\nsource: {SOURCE}\n"
        ))),
        &[],
    );
    assert!(filed.status.success(), "{}", shown(&filed));
    let output = world.check("story:passwd-home", &world.defect, &env);
    let ran = fs::read_to_string(record(&output).join("output")).unwrap_or_default();
    assert!(
        ran.lines()
            .any(|line| line.starts_with("passwd-home /") && line != "passwd-home /"),
        "the command names the user's passwd home:\n{ran}"
    );
    let visible: Vec<_> = ran
        .lines()
        .filter(|line| line.starts_with("visible "))
        .collect();
    assert!(
        visible.is_empty(),
        "files of the user's passwd home are in the command's sight: {visible:?}"
    );
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "reproduced\n",
        "{}",
        shown(&output)
    );
}

/// The passwd home is hidden beside HOME, not only when it holds HOME: the user's passwd entry
/// is read from a `getent` first on PATH that names a home of the scratch's, beside the
/// HOME the check is given, so the case reaches that branch whether or not the scratch lies
/// under the user's real passwd home. A file planted there is not in the command's sight.
#[test]
fn a_passwd_home_beside_home_is_hidden_as_well() {
    use std::os::unix::fs::PermissionsExt;

    let world = world("passwd-beside");
    let (home, env) = callers_home(&world);
    let passwd_home = world.outer.0.join("passwd-home");
    write(
        &passwd_home.join(".ssh/id_planted"),
        "PLANTED-PASSWD-HOME\n",
    );
    assert!(
        !home.starts_with(&passwd_home) && !passwd_home.starts_with(&home),
        "the two homes lie beside each other"
    );
    let bin = world.outer.0.join("bin-getent");
    write(
        &bin.join("getent"),
        &format!(
            "#!/bin/sh\n[ \"$1\" = passwd ] || exit 2\nprintf 'u:x:%s:%s::%s:/bin/sh\\n' \"$(id -u)\" \"$(id -g)\" '{}'\n",
            passwd_home.display()
        ),
    );
    fs::set_permissions(bin.join("getent"), fs::Permissions::from_mode(0o755)).expect("executable");
    let path = format!(
        "{}:{}",
        bin.display(),
        std::env::var("PATH").expect("a PATH")
    );
    let mut env: Vec<(&str, &str)> = env
        .iter()
        .map(|(name, value)| (*name, value.as_str()))
        .collect();
    env.push(("PATH", &path));
    let filed = file(
        &world.takt,
        "passwd-beside",
        &body(Some(&format!(
            "scenario: 1\ncommand: \"$TAKT\" --version >/dev/null; cat '{}' 2>/dev/null; echo probe-ran\nexit: 0\noutput: probe-ran\nsource: {SOURCE}\n",
            passwd_home.join(".ssh/id_planted").display()
        ))),
        &[],
    );
    assert!(filed.status.success(), "{}", shown(&filed));
    let output = world.check("story:passwd-beside", &world.defect, &env);
    let ran = fs::read_to_string(record(&output).join("output")).unwrap_or_default();
    assert!(
        !ran.contains("PLANTED-PASSWD-HOME"),
        "the command read a file of the passwd home beside HOME:\n{ran}"
    );
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "reproduced\n",
        "{}",
        shown(&output)
    );
}

/// The review's last finding: a descriptor the caller left open passes to no process the
/// check runs under bubblewrap. A caller holding a file of its hidden home open on descriptor
/// 7 gives the command nothing of it, through `/proc/self/fd/7` or `/dev/fd/7`.
#[test]
fn the_predicate_reads_no_descriptor_the_caller_left_open() {
    let world = world("held-fd");
    let (home, env) = callers_home(&world);
    let env: Vec<(&str, &str)> = env
        .iter()
        .map(|(name, value)| (*name, value.as_str()))
        .collect();
    let filed = file(
        &world.takt,
        "held-fd",
        &body(Some(&format!(
            "scenario: 1\ncommand: \"$TAKT\" --version >/dev/null; for f in /proc/self/fd/7 /dev/fd/7; do cat \"$f\" 2>/dev/null | grep -q PLANTED-HOME-SECRET && echo \"read $f\"; done; echo probe-ran\nexit: 0\noutput: probe-ran\nsource: {SOURCE}\n"
        ))),
        &[],
    );
    assert!(filed.status.success(), "{}", shown(&filed));
    let output = check_holding(
        &world,
        "story:held-fd",
        &world.defect,
        &env,
        &home.join(".secret"),
    );
    let ran = fs::read_to_string(record(&output).join("output")).unwrap_or_default();
    let read: Vec<_> = ran
        .lines()
        .filter(|line| line.starts_with("read "))
        .collect();
    assert!(
        read.is_empty(),
        "the command read the caller's open descriptor: {read:?}"
    );
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
    let leaked = format!(
        "scenario: 1\ncommand: env; test -x \"$TAKT\"\noutput: PLANTED-CREDENTIAL\nsource: {SOURCE}\n"
    );
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
        "exit\t3".to_owned(),
        "healthy_exit\t3".to_owned(),
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
    let other_url = format!(
        "https://other.invalid/scenarios={}",
        world.scenarios.display()
    );
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
        (
            "a checkout for a URL the predicate does not name",
            vec!["--checkout", &other_url],
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
    let mut cases: Vec<(&str, String)> = vec![
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
            "a repository URL carrying a user or a credential",
            replaced(
                &repository_form,
                "repository",
                "https://ci-user:t0k3n-not-real@example.invalid/canon.git",
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
    // The host is a lint on the URL's text, not a boundary: a filed repository names a dotted
    // DNS name or a dotted-decimal IPv4 address outside the local ranges, and nothing else.
    let hosts = [
        ("localhost", "https://localhost/x"),
        ("localhost in capitals", "https://LOCALHOST/x"),
        ("localhost in mixed case", "https://LocalHost/x"),
        ("localhost with a trailing dot", "https://localhost./x"),
        ("localhost with a port", "https://localhost:8443/x"),
        ("a name under localhost", "https://foo.localhost/x"),
        ("the loopback address", "https://127.0.0.1/x"),
        (
            "the loopback address with a trailing dot",
            "https://127.0.0.1./x",
        ),
        ("the loopback address in two parts", "https://127.1/x"),
        ("the loopback address in three parts", "https://127.0.1/x"),
        (
            "the loopback address as one decimal",
            "https://2130706433/x",
        ),
        (
            "the loopback address as one hex number",
            "https://0x7f000001/x",
        ),
        (
            "the loopback address with a hex octet",
            "https://0x7f.0.0.1/x",
        ),
        (
            "the loopback address with an octal octet",
            "https://0177.0.0.1/x",
        ),
        (
            "the loopback address as one octal number",
            "https://017700000001/x",
        ),
        ("the zero address", "https://0.0.0.0/x"),
        ("the zero address as one digit", "https://0/x"),
        ("a 10/8 address", "https://10.0.0.1/x"),
        ("a 10/8 address with an octal octet", "https://012.0.0.1/x"),
        ("a 10/8 address as one decimal", "https://167772161/x"),
        ("the first 172.16/12 address", "https://172.16.0.1/x"),
        ("the last 172.16/12 address", "https://172.31.255.255/x"),
        ("a 192.168/16 address", "https://192.168.1.1/x"),
        (
            "the link-local metadata address",
            "https://169.254.169.254/x",
        ),
        (
            "the link-local metadata address as one decimal",
            "https://2852039166/x",
        ),
        ("a 100.64/10 address", "https://100.64.0.1/x"),
        ("the IPv6 loopback", "https://[::1]/x"),
        (
            "the IPv6 loopback written out",
            "https://[0:0:0:0:0:0:0:1]/x",
        ),
        (
            "the loopback address mapped into IPv6",
            "https://[::ffff:127.0.0.1]/x",
        ),
        (
            "the loopback address mapped into IPv6 in hex",
            "https://[::ffff:7f00:1]/x",
        ),
        ("the IPv6 zero address", "https://[::]/x"),
        ("an IPv6 link-local address", "https://[fe80::1]/x"),
        (
            "an IPv6 link-local address in capitals",
            "https://[FE80::1]/x",
        ),
        (
            "an IPv6 link-local address with a zone",
            "https://[fe80::1%25eth0]/x",
        ),
        ("an IPv6 unique local address", "https://[fc00::1]/x"),
        (
            "another IPv6 unique local address",
            "https://[fd12:3456::1]/x",
        ),
        (
            "an IPv6-compatible loopback address",
            "https://[::127.0.0.1]/x",
        ),
        (
            "the loopback address before an encoded slash and a user",
            "https://127.0.0.1%2f@x/x",
        ),
        ("the loopback address before a query", "https://127.0.0.1?x"),
        (
            "the loopback address before a fragment",
            "https://127.0.0.1#x",
        ),
        (
            "the loopback address before a backslash",
            "https://127.0.0.1\\x",
        ),
        (
            "the loopback address with a port",
            "https://127.0.0.1:443/x",
        ),
    ];
    for (what, url) in hosts {
        cases.push((what, replaced(&repository_form, "repository", url)));
    }
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
    // A public host passes the lint by its text alone, whatever it resolves to: filing reads
    // nothing from it.
    let accepted = [
        (
            "a public repository",
            "https://github.com/beyond10x/todo-app-example",
        ),
        ("a public address", "https://172.32.0.1/x"),
        (
            "a name that resolves to the loopback address",
            "https://localtest.me/x",
        ),
        (
            "a name spelling the loopback address under a public domain",
            "https://127.0.0.1.nip.io/x",
        ),
    ];
    for (index, (what, url)) in accepted.into_iter().enumerate() {
        let text = replaced(&repository_form, "repository", url);
        let key = format!("forms-{index}");
        let output = file(&repo, &key, &body(Some(&text)), &[]);
        assert!(output.status.success(), "{what}: {}", shown(&output));
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

/// The predicate's clone runs as the predicate will, not as the caller: a caller's global git
/// configuration that rewrites the predicate's URL to a local path (`insteadOf`) and names a
/// credential helper reaches neither the clone nor the predicate. Without the rewrite the URL
/// resolves nowhere, so the check has no result; the helper is never called.
#[test]
fn the_predicates_clone_reads_no_git_configuration_or_credential_of_the_caller() {
    use std::os::unix::fs::PermissionsExt;

    let world = world("callers-config");
    let filed = file(
        &world.takt,
        "repo-form",
        &body(Some(&world.repository_predicate())),
        &[],
    );
    assert!(filed.status.success(), "{}", shown(&filed));
    let home = world.outer.0.join("callers-home");
    fs::create_dir_all(&home).expect("a directory");
    let log = world.outer.0.join("helper-calls.log");
    let helper = home.join("helper.sh");
    write(
        &helper,
        &format!("#!/bin/sh\necho called >> {}\nexit 0\n", log.display()),
    );
    fs::set_permissions(&helper, fs::Permissions::from_mode(0o755)).expect("executable");
    write(
        &home.join(".gitconfig"),
        &format!(
            "[url \"{}\"]\n\tinsteadOf = {REPOSITORY_URL}\n[credential]\n\thelper = {}\n",
            world.scenarios.display(),
            helper.display()
        ),
    );
    let cache = world.cache();
    let output = script(
        "check-finding",
        &world.takt,
        &["story:repo-form", &world.defect],
        &[
            ("XDG_CACHE_HOME", cache.to_str().expect("UTF-8")),
            ("HOME", home.to_str().expect("UTF-8")),
        ],
    );
    assert!(
        !output.status.success(),
        "the caller's insteadOf sent the predicate's URL to a local repository: {}",
        shown(&output)
    );
    assert!(output.stdout.is_empty(), "{}", shown(&output));
    assert!(
        !log.exists(),
        "the caller's credential helper was called: {}",
        fs::read_to_string(&log).unwrap_or_default()
    );
    // The scenario form, with the same caller configuration, still runs and still calls no
    // helper: the clone of a local path needs none, and the configuration is not read.
    let filed = file(
        &world.takt,
        "default-protocol",
        &body(Some(&predicate())),
        &[],
    );
    assert!(filed.status.success(), "{}", shown(&filed));
    let output = world.check(
        "story:default-protocol",
        &world.defect,
        &[("HOME", home.to_str().expect("UTF-8"))],
    );
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "reproduced\n",
        "{}",
        shown(&output)
    );
    assert!(!log.exists(), "the caller's credential helper was called");
}

/// Nothing the caller's environment carries reaches the clone or the git calls that set it up:
/// a `GIT_CONFIG_*` and a `GIT_CONFIG_PARAMETERS` `insteadOf` sending the predicate's URL to a
/// local repository, an askpass and an ssh command that log their calls, an agent socket, and a
/// `GIT_DIR` naming another repository.
#[test]
fn the_predicates_clone_reads_no_environment_of_the_caller() {
    use std::os::unix::fs::PermissionsExt;

    let world = world("callers-environment");
    let filed = file(
        &world.takt,
        "repo-form",
        &body(Some(&world.repository_predicate())),
        &[],
    );
    assert!(filed.status.success(), "{}", shown(&filed));
    let log = world.outer.0.join("calls.log");
    let logger = |name: &str| {
        let path = world.outer.0.join(name);
        write(
            &path,
            &format!("#!/bin/sh\necho {name} >> {}\nexit 0\n", log.display()),
        );
        fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).expect("executable");
        path.to_str().expect("UTF-8").to_owned()
    };
    let askpass = logger("askpass.sh");
    let ssh = logger("ssh.sh");
    let instead_of = format!("url.{}.insteadOf", world.scenarios.display());
    let parameters = format!("'{instead_of}={REPOSITORY_URL}'");
    let other = store(&world.outer.0.join("other"));
    let git_dir = other.join(".git");
    let environment = [
        ("GIT_CONFIG_COUNT", "1"),
        ("GIT_CONFIG_KEY_0", instead_of.as_str()),
        ("GIT_CONFIG_VALUE_0", REPOSITORY_URL),
        ("GIT_CONFIG_PARAMETERS", parameters.as_str()),
        ("GIT_ASKPASS", askpass.as_str()),
        ("GIT_SSH_COMMAND", ssh.as_str()),
        ("SSH_AUTH_SOCK", "/nonexistent"),
        ("GIT_DIR", git_dir.to_str().expect("UTF-8")),
    ];
    let cache = world.cache();
    let mut all = vec![("XDG_CACHE_HOME", cache.to_str().expect("UTF-8"))];
    all.extend_from_slice(&environment);
    let output = script(
        "check-finding",
        &world.takt,
        &["story:repo-form", &world.defect],
        &all,
    );
    assert!(
        !output.status.success(),
        "the caller's environment sent the predicate's URL to a local repository: {}",
        shown(&output)
    );
    assert!(output.stdout.is_empty(), "{}", shown(&output));
    assert!(
        !log.exists(),
        "the caller's askpass or ssh command was called: {}",
        fs::read_to_string(&log).unwrap_or_default()
    );
    // With `--checkout`, the same environment still gives the result: the check's own git
    // calls read the repositories the script names, not the caller's `GIT_DIR`.
    let output = world.check_repository("story:repo-form", &world.defect, &environment);
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "reproduced\n",
        "{}",
        shown(&output)
    );
    assert!(
        !log.exists(),
        "the caller's askpass or ssh command was called: {}",
        fs::read_to_string(&log).unwrap_or_default()
    );
}

/// A `--checkout` that is itself a `--shared` clone lends the predicate's clone no
/// `objects/info/alternates`: the clone is dissociated, holding every object it needs itself,
/// and the predicate still answers.
#[test]
fn the_predicates_clone_borrows_no_object_from_its_sources_source() {
    let world = world("dissociate");
    let filed = file(
        &world.takt,
        "repo-form",
        &body(Some(&world.repository_predicate())),
        &[],
    );
    assert!(filed.status.success(), "{}", shown(&filed));
    let shared = world.outer.0.join("shared");
    git(
        &world.outer.0,
        &[
            "clone",
            "-q",
            "--shared",
            "--no-checkout",
            "scenarios",
            "shared",
        ],
    );
    assert!(
        shared.join(".git/objects/info/alternates").is_file(),
        "the --checkout borrows its objects"
    );
    let checkout = format!("{REPOSITORY_URL}={}", shared.display());
    let cache = world.cache();
    let output = script(
        "check-finding",
        &world.takt,
        &["--checkout", &checkout, "story:repo-form", &world.defect],
        &[("XDG_CACHE_HOME", cache.to_str().expect("UTF-8"))],
    );
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "reproduced\n",
        "{}",
        shown(&output)
    );
    let clones: Vec<PathBuf> = fs::read_dir(cache.join("takt/check-finding/runs"))
        .expect("the runs directory")
        .map(|entry| entry.expect("an entry").path().join("scenario"))
        .filter(|clone| clone.is_dir())
        .collect();
    assert_eq!(clones.len(), 1, "{clones:?}");
    let alternates = clones[0].join(".git/objects/info/alternates");
    let borrowed = fs::read_to_string(&alternates).unwrap_or_default();
    assert!(
        borrowed.trim().is_empty(),
        "{} borrows objects from {borrowed}",
        alternates.display()
    );
    let fsck = Command::new("git")
        .args(["fsck", "--strict"])
        .current_dir(&clones[0])
        .output()
        .expect("git runs");
    assert!(
        fsck.status.success(),
        "the clone misses objects of its own: {}",
        String::from_utf8_lossy(&fsck.stderr)
    );
}

/// A local clone shares no object file with the repository it was cloned from: a predicate
/// that damages an object in its clone damages nothing in the `--checkout` or `--scenarios`
/// source, and the source's objects keep one link each.
#[test]
fn the_predicates_clone_hardlinks_no_object_of_its_source() {
    use std::os::unix::fs::MetadataExt;

    let world = world("no-hardlinks");
    // The predicate corrupts every loose object in its clone, then reports the result.
    let predicate = world.repository_predicate().replace(
        "command: test \"$TEST\" = ./check.sh && \"$TAKT\" run --repo .",
        "command: find .git/objects -type f -exec sh -c 'echo damaged > \"$1\"' _ {} \\; && \"$TAKT\" run --repo .",
    );
    let filed = file(&world.takt, "repo-form", &body(Some(&predicate)), &[]);
    assert!(filed.status.success(), "{}", shown(&filed));
    let output = world.check_repository("story:repo-form", &world.defect, &[]);
    assert!(output.status.success(), "{}", shown(&output));
    let fsck = Command::new("git")
        .args(["fsck", "--strict"])
        .current_dir(&world.scenarios)
        .output()
        .expect("git runs");
    assert!(
        fsck.status.success(),
        "the source repository is damaged: {}",
        String::from_utf8_lossy(&fsck.stderr)
    );
    let mut objects = 0;
    for entry in walk(&world.scenarios.join(".git/objects")) {
        let links = fs::metadata(&entry).expect("metadata").nlink();
        assert_eq!(links, 1, "{} has {links} links", entry.display());
        objects += 1;
    }
    assert!(objects > 0, "no object file found");
}

/// Every regular file below `dir`.
fn walk(dir: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    for entry in fs::read_dir(dir).expect("a readable directory") {
        let path = entry.expect("an entry").path();
        if path.is_dir() {
            files.extend(walk(&path));
        } else {
            files.push(path);
        }
    }
    files
}

/// A short seed in the predicate is recorded resolved to the full commit id, as the scenario
/// form records its seed.
#[test]
fn a_repository_findings_seed_is_recorded_as_the_full_commit_id() {
    let world = world("full-seed");
    let short = &world.seed[..12];
    let predicate = world
        .repository_predicate()
        .replace(&format!("seed: {}", world.seed), &format!("seed: {short}"));
    let filed = file(&world.takt, "repo-form", &body(Some(&predicate)), &[]);
    assert!(filed.status.success(), "{}", shown(&filed));
    let output = world.check_repository("story:repo-form", &world.defect, &[]);
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "reproduced\n",
        "{}",
        shown(&output)
    );
    let runs = world.cache().join("takt/check-finding/runs");
    let results: Vec<String> = fs::read_dir(&runs)
        .expect("the runs directory")
        .map(|entry| fs::read_to_string(entry.expect("an entry").path().join("result")))
        .filter_map(Result::ok)
        .collect();
    assert!(
        results
            .iter()
            .all(|result| result.contains(&format!("seed: {}\n", world.seed))),
        "{results:?}"
    );
}

/// `check-finding` builds the revision in a checkout of its own that fetches it through git's
/// transport, every object hashed and checked as it is received. With the loose object of the
/// fixed revision's `main.rs` overwritten in this repository's store by the defect's bytes, the
/// build fails, git naming the object, and there is no result: never the defect's text built as
/// the fixed revision. So too when `GIT_TEMPLATE_DIR` names a template whose
/// `objects/info/alternates` would lend a new repository this store's objects, so that a fetch
/// received none: the checkout is made with no template.
#[test]
fn a_poisoned_object_of_the_revision_fails_the_build_naming_it() {
    use std::os::unix::fs::PermissionsExt as _;
    let loose = |repo: &Path, id: &str| repo.join(".git/objects").join(&id[..2]).join(&id[2..]);
    for template in [false, true] {
        let world = world(if template {
            "poisoned-template"
        } else {
            "poisoned"
        });
        let filed = file(
            &world.takt,
            "default-protocol",
            &body(Some(&predicate())),
            &[],
        );
        assert!(filed.status.success(), "{}", shown(&filed));
        let main = "crates/takt-cli/src/main.rs";
        let blob = git_out(
            &world.takt,
            &["rev-parse", &format!("{}:{main}", world.fixed)],
        );
        let other = git_out(
            &world.takt,
            &["rev-parse", &format!("{}:{main}", world.defect)],
        );
        let target = loose(&world.takt, &blob);
        let bytes = fs::read(loose(&world.takt, &other)).expect("the defect's object");
        fs::set_permissions(&target, fs::Permissions::from_mode(0o644)).expect("writable");
        fs::write(&target, bytes).expect("poisoned");
        let lending = world.outer.0.join("template");
        write(
            &lending.join("objects/info/alternates"),
            &format!("{}\n", world.takt.join(".git/objects").display()),
        );
        let env: Vec<(&str, &str)> = if template {
            vec![("GIT_TEMPLATE_DIR", lending.to_str().expect("UTF-8"))]
        } else {
            Vec::new()
        };
        let output = world.check("story:default-protocol", &world.fixed, &env);
        assert_eq!(
            output.status.code(),
            Some(1),
            "template {template}: {}",
            shown(&output)
        );
        assert!(
            output.stdout.is_empty(),
            "template {template}: {}",
            shown(&output)
        );
        assert!(
            String::from_utf8_lossy(&output.stderr).contains(&blob),
            "template {template}: the failure names {blob}: {}",
            shown(&output)
        );
    }
}

/// The reviewer's RV-R: the caller's environment names an object directory
/// (`GIT_ALTERNATE_OBJECT_DIRECTORIES` or `GIT_OBJECT_DIRECTORY`, which a hook's environment
/// under quarantine carries) holding this repository's store, in which the fixed revision's
/// `main.rs` was overwritten with the defect's bytes. The build's git runs under a cleared
/// environment, so the fetch still receives and hashes every object: the build fails naming the
/// object, and there is no result.
#[test]
fn object_directories_in_the_callers_environment_reach_no_build() {
    use std::os::unix::fs::PermissionsExt as _;
    let loose = |repo: &Path, id: &str| repo.join(".git/objects").join(&id[..2]).join(&id[2..]);
    for var in ["GIT_ALTERNATE_OBJECT_DIRECTORIES", "GIT_OBJECT_DIRECTORY"] {
        let world = world(&format!("caller-{}", var.to_lowercase()));
        let filed = file(
            &world.takt,
            "default-protocol",
            &body(Some(&predicate())),
            &[],
        );
        assert!(filed.status.success(), "{}", shown(&filed));
        let main = "crates/takt-cli/src/main.rs";
        let blob = git_out(
            &world.takt,
            &["rev-parse", &format!("{}:{main}", world.fixed)],
        );
        let other = git_out(
            &world.takt,
            &["rev-parse", &format!("{}:{main}", world.defect)],
        );
        let target = loose(&world.takt, &blob);
        let bytes = fs::read(loose(&world.takt, &other)).expect("the defect's object");
        fs::set_permissions(&target, fs::Permissions::from_mode(0o644)).expect("writable");
        fs::write(&target, bytes).expect("poisoned");
        let objects = world.takt.join(".git/objects");
        let output = world.check(
            "story:default-protocol",
            &world.fixed,
            &[(var, objects.to_str().expect("UTF-8"))],
        );
        assert_eq!(output.status.code(), Some(1), "{var}: {}", shown(&output));
        assert!(output.stdout.is_empty(), "{var}: {}", shown(&output));
        assert!(
            String::from_utf8_lossy(&output.stderr).contains(&blob),
            "{var}: the failure names {blob}: {}",
            shown(&output)
        );
    }
}
