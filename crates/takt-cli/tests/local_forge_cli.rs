//! story:local-forge-merges-into-a-target-branch at the binary's edge: `--forge local` is refused
//! before a case opens, exit 1, without `local_checks` in the run file, or with an `origin` that
//! is no bare repository on this machine outside the tree; `local_checks` under another forge is
//! refused naming the key; with both, the run opens on the hub and says so in its forge line,
//! and a granted case merges into the hub and records `pr.merged`, its checks run without the
//! credentials in `takt`'s environment. `takt run --help` names it.

#[path = "../../takt/tests/support/own_home.rs"]
mod own_home;

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

const DELIVERY_PROTOCOL: &str = "story.delivery/1";

/// Runs `git <args>` in `dir` with no configuration of the user's or the system's, no signing
/// and no hook.
fn git(dir: &Path, args: &[&str]) {
    let status = Command::new("git")
        .args(["-c", "user.name=t", "-c", "user.email=t@localhost"])
        .args([
            "-c",
            "commit.gpgsign=false",
            "-c",
            "core.hooksPath=/dev/null",
        ])
        .args(args)
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .current_dir(dir)
        .status()
        .expect("git runs");
    assert!(status.success(), "git {args:?}");
}

/// A scratch directory under this crate's build directory, removed on drop.
struct Scratch(PathBuf);

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// A hub `<scratch>/hub.git` whose `main` holds one commit with a commit identity, and a clone
/// of it at `<scratch>/repo`, its `origin` the hub.
fn repository(name: &str) -> (Scratch, PathBuf, PathBuf) {
    let outer = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("local-forge-cli-{}-{name}", std::process::id()));
    let _ = fs::remove_dir_all(&outer);
    let seed = outer.join("seed");
    fs::create_dir_all(seed.join(".engineering")).expect("a directory");
    fs::write(seed.join("README.md"), "a story\n").expect("a file");
    fs::write(
        seed.join(".engineering/commit-identity.conf"),
        "[identity]\n\tname = Test Bot\n\temail = bot@example.invalid\n",
    )
    .expect("the commit identity is written");
    git(&seed, &["init", "-q", "-b", "main"]);
    git(&seed, &["add", "-A"]);
    git(&seed, &["commit", "-q", "-m", "start"]);
    git(&outer, &["clone", "-q", "--bare", "seed", "hub.git"]);
    git(&outer, &["clone", "-q", "--no-local", "hub.git", "repo"]);
    fs::remove_dir_all(&seed).expect("the seed is removed");
    let hub = outer.join("hub.git");
    let repo = outer.join("repo");
    (Scratch(outer), repo, hub)
}

/// `takt run` with no harness on the delivery protocol over `repo`, `extra` after it.
fn takt(repo: &Path, extra: &[&str]) -> (i32, String, String) {
    let output = Command::new(env!("CARGO_BIN_EXE_takt"))
        .args([
            "run",
            "--repo",
            repo.to_str().expect("utf-8"),
            "--intent",
            "deliver the story",
            "--case-id",
            "c1",
            "--harness",
            "none",
            "--protocol",
            DELIVERY_PROTOCOL,
            "--test-cmd",
            "true",
            "--iterations",
            "1",
        ])
        .args(extra)
        .env_remove("B10X_GATES_POLICY")
        .output()
        .expect("takt runs");
    (
        output.status.code().expect("an exit code"),
        String::from_utf8_lossy(&output.stdout).into_owned(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
    )
}

/// Acceptance 5: `--forge local` without `local_checks`, from the flag or the run file, is
/// refused before a case opens, naming the key; nothing is written to the hub.
#[test]
fn local_without_local_checks_is_refused_before_a_case_opens() {
    let (outer, repo, hub) = repository("no-checks");
    let config = outer.0.join("run.yaml");
    fs::write(&config, "forge: local\n").expect("a file");
    for extra in [
        vec!["--forge", "local"],
        vec!["--config", config.to_str().expect("utf-8")],
    ] {
        let (code, out, err) = takt(&repo, &extra);
        assert_eq!(code, 1, "{extra:?}: stdout {out} stderr {err}");
        assert!(err.contains("local_checks"), "{extra:?}: stderr {err}");
        assert!(!out.contains("step 1"), "{extra:?}: stdout {out}");
        assert!(!repo.join(".takt/cases").exists(), "{extra:?}");
    }
    assert!(!hub.join("takt").exists());
}

/// Acceptance 5: with `local_checks`, an `origin` that is no local path, one inside the tree, or
/// a repository that is not bare is refused before a case opens.
#[test]
fn an_origin_that_is_no_hub_is_refused_before_a_case_opens() {
    let (outer, repo, _hub) = repository("origin");
    let config = outer.0.join("run.yaml");
    fs::write(&config, "forge: local\nlocal_checks: [\"true\"]\n").expect("a file");
    git(&repo, &["init", "-q", "--bare", "inner.git"]);
    fs::write(repo.join(".git/info/exclude"), "inner.git/\n").expect("excluded");
    let inside = repo.join("inner.git");
    for (url, words) in [
        ("https://forge.example/group/project.git", "no local path"),
        (inside.to_str().expect("utf-8"), "inside the tree"),
        (
            outer.0.join("repo").to_str().expect("utf-8"),
            "inside the tree",
        ),
    ] {
        git(&repo, &["remote", "set-url", "origin", url]);
        let (code, out, err) = takt(&repo, &["--config", config.to_str().expect("utf-8")]);
        assert_eq!(code, 1, "{url}: stdout {out} stderr {err}");
        assert!(err.contains(words), "{url}: stderr {err}");
        assert!(!repo.join(".takt/cases").exists(), "{url}");
    }
}

/// `local_checks` belongs to `forge: local`: under another forge it is refused naming the key.
#[test]
fn local_checks_under_another_forge_is_refused() {
    let (outer, repo, _hub) = repository("other");
    let config = outer.0.join("run.yaml");
    fs::write(&config, "forge: none\nlocal_checks: [\"true\"]\n").expect("a file");
    let (code, out, err) = takt(&repo, &["--config", config.to_str().expect("utf-8")]);
    assert_eq!(code, 1, "stdout {out} stderr {err}");
    assert!(err.contains("local_checks"), "stderr {err}");
    assert!(!repo.join(".takt/cases").exists());
}

/// With `local_checks` and a hub for `origin`, the run opens on the local forge and names the hub
/// in its forge line; without the flag or the run file's `forge`, the default is never `local`.
#[test]
fn the_run_opens_on_the_hub_and_local_is_never_the_default() {
    let (outer, repo, hub) = repository("opens");
    let config = outer.0.join("run.yaml");
    fs::write(&config, "forge: local\nlocal_checks: [\"true\"]\n").expect("a file");
    let (_, out, err) = takt(&repo, &["--config", config.to_str().expect("utf-8")]);
    let hub = fs::canonicalize(&hub).expect("the hub");
    assert!(
        out.contains(&format!("forge: local ({})", hub.display())),
        "stdout {out} stderr {err}"
    );
    let (_outer, repo, _hub) = repository("default");
    let (_, out, err) = takt(&repo, &[]);
    assert!(
        out.contains("forge: none (origin names no <owner>/<repo> on a host)"),
        "stdout {out} stderr {err}"
    );
}

/// The fixture stores of the library crate, one directory each.
const FIXTURES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../takt/tests/fixtures");

/// A gates document whose one assertion is true in a repository with a manifest.
const GATES: &str = "format: engineering-gates/1\nlanguage: canon-expr/1\ngates:\n  - id: manifest\n    \
                     text: The repository has its manifest.\n    assertions:\n      \
                     - 'fs.file.exists(\"Cargo.toml\")'\n";

/// A value no credential holds, set in `takt`'s environment under each credential's name.
const SENTINEL: &str = "local-forge-cli-sentinel";

/// The `CARGO_TARGET_DIR` `takt` runs with, which the checks share under `--confinement none`.
const SHARED_TARGET: &str = concat!(
    env!("CARGO_TARGET_TMPDIR"),
    "/local-forge-cli-shared-target"
);

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

/// `git <args>` in `dir`; its standard output, trimmed.
fn git_out(dir: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .args(args)
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .current_dir(dir)
        .output()
        .expect("git runs");
    assert!(output.status.success(), "git {args:?}");
    String::from_utf8_lossy(&output.stdout).trim().to_owned()
}

/// The built `takt` with `args`, a credential's value under each credential's name in its
/// environment: the exit code, stdout and stderr.
fn takt_with_credentials(args: &[&str]) -> (i32, String, String) {
    let output = Command::new(env!("CARGO_BIN_EXE_takt"))
        .args(args)
        .env_remove("B10X_GATES_POLICY")
        .env("GH_TOKEN", SENTINEL)
        .env("GITHUB_TOKEN", SENTINEL)
        .env("B10X_GATES_TOKEN", SENTINEL)
        .env("CARGO_TARGET_DIR", SHARED_TARGET)
        .output()
        .expect("takt runs");
    (
        output.status.code().expect("an exit code"),
        String::from_utf8_lossy(&output.stdout).into_owned(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
    )
}

/// Acceptance 2 through the binary: the store fixture with one ready story in a hub, and a
/// `--no-local` clone of it as the tree; a plain run takes the story, the work is committed on
/// the case's branch, the next run pushes it to the hub, opens its request, runs `local_checks`
/// and stops at the gate, exit 4; `takt approve` grants the merge; the next run merges the request into
/// the hub's `main` and records `pr.merged`, exit 0. The checks saw none of the credentials in
/// `takt`'s environment, had a home of their own and built into `takt`'s `CARGO_TARGET_DIR`.
#[test]
fn a_granted_case_merges_into_the_hub_and_records_pr_merged() {
    let outer = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("local-forge-cli-{}-e2e", std::process::id()));
    let _ = fs::remove_dir_all(&outer);
    let _scratch = Scratch(outer.clone());
    let seed = outer.join("seed");
    copy_dir(&Path::new(FIXTURES).join("one-ready"), &seed);
    fs::write(seed.join("Cargo.toml"), "[package]\nname = \"scratch\"\n").expect("a file");
    fs::write(seed.join(".engineering/gates.yaml"), GATES).expect("a file");
    fs::write(
        seed.join(".engineering/commit-identity.conf"),
        "[identity]\n\tname = Test Bot\n\temail = bot@example.invalid\n",
    )
    .expect("the commit identity is written");
    git(&seed, &["init", "-q", "-b", "main"]);
    git(&seed, &["add", "-A"]);
    git(&seed, &["commit", "-q", "-m", "start"]);
    git(&outer, &["clone", "-q", "--bare", "seed", "hub.git"]);
    git(&outer, &["clone", "-q", "--no-local", "hub.git", "repo"]);
    let hub = outer.join("hub.git");
    let repo = outer.join("repo");
    let start = git_out(&hub, &["rev-parse", "refs/heads/main"]);
    let seen = outer.join("seen-env");
    let config = outer.join("run.yaml");
    fs::write(
        &config,
        format!(
            "forge: local\nlocal_checks: [\"sh\", \"-c\", \"env > '{}'\"]\n",
            seen.display()
        ),
    )
    .expect("a file");
    let path = repo.to_str().expect("utf-8");
    let run = [
        "run",
        "--repo",
        path,
        "--harness",
        "none",
        "--test-cmd",
        "true",
        "--protocol",
        DELIVERY_PROTOCOL,
        "--config",
        config.to_str().expect("utf-8"),
    ];
    let mut first = run.to_vec();
    first.extend(["--iterations", "0"]);
    let (code, out, err) = takt_with_credentials(&first);
    assert_eq!(code, 3, "stdout {out} stderr {err}");
    assert!(out.contains("forge: local ("), "{out}");
    let id = git_out(&repo, &["branch", "--show-current"])
        .strip_prefix("takt/")
        .expect("the tree is on the case's branch")
        .to_owned();

    fs::write(repo.join("feature.txt"), "the work\n").expect("a file");
    git(&repo, &["add", "feature.txt"]);
    git(&repo, &["commit", "-q", "-m", "the work"]);

    let (code, out, err) = takt_with_credentials(&run);
    assert_eq!(code, 4, "stdout {out} stderr {err}");
    for line in [
        "evidence: pull_request open",
        "evidence: ci_result pass",
        "stopped: ApprovalRequired (repository.merge)",
    ] {
        assert!(out.contains(line), "{line}: stdout {out} stderr {err}");
    }
    assert_eq!(
        git_out(&hub, &["rev-parse", "refs/heads/main"]),
        start,
        "nothing merged before the grant"
    );

    let (code, out, err) = takt_with_credentials(&[
        "approve",
        "--repo",
        path,
        &id,
        "repository.merge",
        "--by",
        "human:operator",
    ]);
    assert_eq!(code, 0, "stdout {out} stderr {err}");

    let (code, out, err) = takt_with_credentials(&run);
    assert_eq!(code, 0, "stdout {out} stderr {err}");
    for line in ["evidence: merge_observation merged", "stopped: Completed"] {
        assert!(out.contains(line), "{line}: stdout {out} stderr {err}");
    }
    let merged = git_out(&hub, &["rev-parse", "refs/heads/main"]);
    let head = git_out(&hub, &["rev-parse", &format!("refs/heads/takt/{id}")]);
    assert_eq!(
        git_out(&hub, &["rev-list", "--parents", "-n", "1", &merged]),
        format!("{merged} {start} {head}"),
        "the hub's main is a merge of the case's head onto where it was"
    );
    assert_eq!(
        git_out(&hub, &["log", "-1", "--format=%an <%ae>", &merged]),
        "Test Bot <bot@example.invalid>"
    );

    let environment = fs::read_to_string(&seen).expect("the checks ran");
    assert!(
        !environment.contains(SENTINEL),
        "a credential reached the checks: {environment}"
    );
    let home = std::env::var("HOME").unwrap_or_default();
    assert!(
        environment
            .lines()
            .any(|line| line.starts_with("HOME=") && line != format!("HOME={home}")),
        "the checks had a home of their own: {environment}"
    );
    assert!(
        environment
            .lines()
            .any(|line| line == format!("CARGO_TARGET_DIR={SHARED_TARGET}")),
        "the checks build into the run's CARGO_TARGET_DIR: {environment}"
    );
}

/// Acceptance 6, the help half: `takt run --help` names `local` in its `--forge` entry.
#[test]
fn run_help_names_the_local_forge() {
    let output = Command::new(env!("CARGO_BIN_EXE_takt"))
        .args(["run", "--help"])
        .output()
        .expect("takt runs");
    assert!(output.status.success());
    let help = String::from_utf8_lossy(&output.stdout);
    let flat = help.split_whitespace().collect::<Vec<_>>().join(" ");
    assert!(
        flat.contains("`local` (the bare repository on this machine")
            && flat.contains("local_checks"),
        "{help}"
    );
}
