//! story:local-forge-merges-into-a-target-branch, Acceptance 2 through the binary, as the story
//! states it: a hub whose branch is `integration`, two clones each delivering a story through
//! `--forge local`; both merge into `integration` in order, the second on the first's merge, and
//! each case records `pr.merged`. The unit's own end-to-end test delivers one case into `main`.

#[path = "../../takt/tests/support/own_home.rs"]
mod own_home;

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

const FIXTURES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../takt/tests/fixtures");

const GATES: &str = "format: engineering-gates/1\nlanguage: canon-expr/1\ngates:\n  - id: manifest\n    \
                     text: The repository has its manifest.\n    assertions:\n      \
                     - 'fs.file.exists(\"Cargo.toml\")'\n";

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
    assert!(
        output.status.success(),
        "git {args:?} in {}: {}",
        dir.display(),
        String::from_utf8_lossy(&output.stderr)
    );
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

fn takt(args: &[&str]) -> (i32, String, String) {
    let output = Command::new(env!("CARGO_BIN_EXE_takt"))
        .args(args)
        .output()
        .expect("takt runs");
    (
        output.status.code().expect("an exit code"),
        String::from_utf8_lossy(&output.stdout).into_owned(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
    )
}

/// The run of `repo` on the delivery protocol through the hub, `extra` after it.
fn run(repo: &Path, config: &Path, extra: &[&str]) -> (i32, String, String) {
    let mut args = vec![
        "run",
        "--repo",
        repo.to_str().expect("utf-8"),
        "--harness",
        "none",
        "--test-cmd",
        "true",
        "--protocol",
        "story.delivery/1",
        "--config",
        config.to_str().expect("utf-8"),
    ];
    args.extend_from_slice(extra);
    takt(&args)
}

/// Takes `story` in `repo`, commits `file` as the work on its case's branch, and runs on to the
/// gate: the case id.
fn to_the_gate(repo: &Path, config: &Path, story: &str, file: &str) -> String {
    let (code, out, err) = run(repo, config, &["--story", story, "--iterations", "0"]);
    assert_eq!(code, 3, "{story}: stdout {out} stderr {err}");
    let id = git(repo, &["branch", "--show-current"])
        .strip_prefix("takt/")
        .expect("the tree is on the case's branch")
        .to_owned();
    fs::write(repo.join(file), "the work\n").expect("a file");
    git(repo, &["add", file]);
    git(repo, &["commit", "-q", "-m", "the work"]);
    let (code, out, err) = run(repo, config, &[]);
    assert_eq!(code, 4, "{story}: stdout {out} stderr {err}");
    assert!(
        out.contains("stopped: ApprovalRequired (repository.merge)"),
        "{story}: stdout {out} stderr {err}"
    );
    let (code, out, err) = takt(&[
        "approve",
        "--repo",
        repo.to_str().expect("utf-8"),
        &id,
        "repository.merge",
        "--by",
        "human:operator",
    ]);
    assert_eq!(code, 0, "{story}: stdout {out} stderr {err}");
    id
}

#[test]
fn two_clones_deliver_into_integration_and_each_case_records_pr_merged() {
    let outer = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("local-forge-adv-two-{}", std::process::id()));
    let _ = fs::remove_dir_all(&outer);
    let _scratch = Scratch(outer.clone());
    let seed = outer.join("seed");
    copy_dir(&Path::new(FIXTURES).join("one-ready"), &seed);
    let stories = seed.join(".engineering/planning/story");
    let one = fs::read_to_string(stories.join("ready-one.md")).expect("the fixture story");
    fs::write(
        stories.join("ready-two.md"),
        one.replace("story:ready-one", "story:ready-two")
            .replace("path: check.txt", "path: two.txt")
            .replace("# Story: ready-one", "# Story: ready-two"),
    )
    .expect("a second ready story");
    fs::write(seed.join("Cargo.toml"), "[package]\nname = \"scratch\"\n").expect("a file");
    fs::write(seed.join(".engineering/gates.yaml"), GATES).expect("a file");
    fs::write(
        seed.join(".engineering/commit-identity.conf"),
        "[identity]\n\tname = Test Bot\n\temail = bot@example.invalid\n",
    )
    .expect("the commit identity is written");
    git(&seed, &["init", "-q", "-b", "integration"]);
    git(&seed, &["add", "-A"]);
    git(&seed, &["commit", "-q", "-m", "start"]);
    git(&outer, &["clone", "-q", "--bare", "seed", "hub.git"]);
    git(&outer, &["clone", "-q", "--no-local", "hub.git", "a"]);
    git(&outer, &["clone", "-q", "--no-local", "hub.git", "b"]);
    let hub = outer.join("hub.git");
    let (a, b) = (outer.join("a"), outer.join("b"));
    let start = git(&hub, &["rev-parse", "refs/heads/integration"]);
    let config = outer.join("run.yaml");
    fs::write(&config, "forge: local\nlocal_checks: [\"true\"]\n").expect("a file");

    let id_a = to_the_gate(&a, &config, "story:ready-one", "one.txt");
    let id_b = to_the_gate(&b, &config, "story:ready-two", "two.txt");
    assert_eq!(
        git(&hub, &["rev-parse", "refs/heads/integration"]),
        start,
        "nothing merged before a run after the grant"
    );

    let (code, out, err) = run(&a, &config, &[]);
    assert_eq!(code, 0, "a: stdout {out} stderr {err}");
    assert!(
        out.contains("evidence: merge_observation merged"),
        "a: stdout {out} stderr {err}"
    );
    let first = git(&hub, &["rev-parse", "refs/heads/integration"]);
    let head_a = git(&hub, &["rev-parse", &format!("refs/heads/takt/{id_a}")]);
    assert_eq!(
        git(&hub, &["rev-list", "--parents", "-n", "1", &first]),
        format!("{first} {start} {head_a}")
    );

    let (code, out, err) = run(&b, &config, &[]);
    assert_eq!(code, 0, "b: stdout {out} stderr {err}");
    assert!(
        out.contains("evidence: merge_observation merged"),
        "b: stdout {out} stderr {err}"
    );
    let second = git(&hub, &["rev-parse", "refs/heads/integration"]);
    let head_b = git(&hub, &["rev-parse", &format!("refs/heads/takt/{id_b}")]);
    assert_eq!(
        git(&hub, &["rev-list", "--parents", "-n", "1", &second]),
        format!("{second} {first} {head_b}"),
        "the second merge goes onto the first"
    );
}
