//! story:local-forge-merges-into-a-target-branch, second adversary pass: under `--confinement
//! substrate` the checks are run as the test command is, and Substrate serves `cargo` alone. A
//! `local_checks` it does not serve is known before anything runs, so it is refused before a case
//! opens, as `--forge local` without `local_checks` is; otherwise the run takes the story, spends
//! its steps, and learns only at `ci.read` that the forge can never run its checks. Where no
//! adapter of this host serves the run, the case prints `skipped: missing prerequisite: …`.

#[path = "../../takt/tests/support/own_home.rs"]
mod own_home;

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

struct Scratch(PathBuf);

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn git(dir: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
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
        .output()
        .expect("git runs");
    assert!(output.status.success(), "git {args:?}");
    String::from_utf8_lossy(&output.stdout).trim().to_owned()
}

#[test]
fn local_checks_substrate_cannot_run_are_refused_before_a_case_opens() {
    let outer = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("local-forge-adv-substrate-{}", std::process::id()));
    let _ = fs::remove_dir_all(&outer);
    let _scratch = Scratch(outer.clone());
    let seed = outer.join("seed");
    fs::create_dir_all(seed.join(".engineering")).expect("a directory");
    fs::write(
        seed.join("Cargo.toml"),
        "[package]\nname = \"probe\"\nversion = \"0.1.0\"\nedition = \"2021\"\n\n[workspace]\n",
    )
    .expect("a file");
    fs::create_dir_all(seed.join("src")).expect("a directory");
    fs::write(seed.join("src/lib.rs"), "").expect("a file");
    fs::write(seed.join(".gitignore"), "target/\n").expect("a file");
    fs::write(
        seed.join(".engineering/commit-identity.conf"),
        "[identity]\n\tname = Test Bot\n\temail = bot@example.invalid\n",
    )
    .expect("the commit identity is written");
    git(&seed, &["init", "-q", "-b", "main"]);
    git(&seed, &["add", "-A"]);
    git(&seed, &["commit", "-q", "-m", "start"]);
    git(&outer, &["clone", "-q", "--bare", "seed", "hub.git"]);
    git(&outer, &["clone", "-q", "hub.git", "repo"]);
    let repo = outer.join("repo");
    let config = outer.join("run.yaml");
    fs::write(
        &config,
        "forge: local\nlocal_checks: [\"sh\", \"-c\", \"true\"]\n",
    )
    .expect("a file");
    let output = Command::new(env!("CARGO_BIN_EXE_takt"))
        .args([
            "run",
            "--repo",
            repo.to_str().expect("utf-8"),
            "--intent",
            "deliver",
            "--harness",
            "none",
            "--protocol",
            "story.delivery/1",
            "--test-cmd",
            "cargo --version",
            "--confinement",
            "substrate",
            "--iterations",
            "0",
            "--config",
            config.to_str().expect("utf-8"),
        ])
        .env_remove("B10X_GATES_POLICY")
        .output()
        .expect("takt runs");
    let (out, err) = (
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
    if err.contains("no Substrate adapter on this host serves") {
        eprintln!("skipped: missing prerequisite: {}", err.trim());
        return;
    }
    assert_eq!(
        output.status.code(),
        Some(1),
        "a local_checks Substrate does not serve opened the case: stdout {out} stderr {err}"
    );
    assert!(err.contains("local_checks"), "stderr {err}");
    assert_eq!(
        git(&repo, &["branch", "--show-current"]),
        "main",
        "no case branch: stdout {out} stderr {err}"
    );
}

const FIXTURES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../takt/tests/fixtures");

const GATES: &str = "format: engineering-gates/1\nlanguage: canon-expr/1\ngates:\n  - id: manifest\n    \
                     text: The repository has its manifest.\n    assertions:\n      \
                     - 'fs.file.exists(\"Cargo.toml\")'\n";

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

/// The served case: `local_checks` of `cargo`, which Substrate serves, run as the test command
/// is through `ci.read`; a story delivered to the gate reads `ci_result pass`, exit 4.
#[test]
fn cargo_local_checks_run_under_substrate_to_the_gate() {
    let outer = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!(
        "local-forge-adv-substrate-e2e-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&outer);
    let _scratch = Scratch(outer.clone());
    let seed = outer.join("seed");
    copy_dir(&Path::new(FIXTURES).join("one-ready"), &seed);
    fs::write(
        seed.join("Cargo.toml"),
        "[package]\nname = \"probe\"\nversion = \"0.1.0\"\nedition = \"2021\"\n\n[workspace]\n",
    )
    .expect("a file");
    fs::create_dir_all(seed.join("src")).expect("a directory");
    fs::write(seed.join("src/lib.rs"), "").expect("a file");
    fs::write(seed.join(".gitignore"), "target/\n").expect("a file");
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
    let repo = outer.join("repo");
    let config = outer.join("run.yaml");
    fs::write(
        &config,
        "forge: local\nlocal_checks: [\"cargo\", \"--version\"]\n",
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
        "cargo --version",
        "--confinement",
        "substrate",
        "--protocol",
        "story.delivery/1",
        "--config",
        config.to_str().expect("utf-8"),
    ];
    let takt = |args: &[&str]| {
        let output = Command::new(env!("CARGO_BIN_EXE_takt"))
            .args(args)
            .env_remove("B10X_GATES_POLICY")
            .output()
            .expect("takt runs");
        (
            output.status.code().expect("an exit code"),
            String::from_utf8_lossy(&output.stdout).into_owned(),
            String::from_utf8_lossy(&output.stderr).into_owned(),
        )
    };
    let mut first = run.to_vec();
    first.extend(["--iterations", "0"]);
    let (code, out, err) = takt(&first);
    if err.contains("no Substrate adapter on this host serves") {
        eprintln!("skipped: missing prerequisite: {}", err.trim());
        return;
    }
    assert_eq!(code, 3, "stdout {out} stderr {err}");
    fs::write(repo.join("feature.txt"), "the work\n").expect("a file");
    git(&repo, &["add", "feature.txt"]);
    git(&repo, &["commit", "-q", "-m", "the work"]);
    let (code, out, err) = takt(&run);
    assert_eq!(code, 4, "stdout {out} stderr {err}");
    assert!(
        out.contains("evidence: ci_result pass"),
        "stdout {out} stderr {err}"
    );
}
