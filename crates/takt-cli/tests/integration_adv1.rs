//! Adversary pass 1 over the wave 06 integration fixes, through the binary: the approval's own
//! commit (`approvals_only`) and the case a plain run resumes (`Run::next_to_resume`).

#[path = "../../takt/tests/support/own_home.rs"]
mod own_home;

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

/// The fixture stores of the library crate, one directory each.
const FIXTURES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../takt/tests/fixtures");

/// A gates document whose one assertion is true in a repository with a manifest.
const GATES: &str = "format: engineering-gates/1\nlanguage: canon-expr/1\ngates:\n  - id: manifest\n    \
                     text: The repository has its manifest.\n    assertions:\n      \
                     - 'fs.file.exists(\"Cargo.toml\")'\n";

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

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
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

/// `<outer>/repo`, a committed copy of the `one-ready` store with a manifest and a true gates
/// document; with `second`, a second ready story, `story:ready-two`, beside `story:ready-one`.
fn repository(name: &str, second: bool) -> (Scratch, PathBuf) {
    let outer = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("integration-adv1-{}-{name}", std::process::id()));
    if outer.exists() {
        fs::remove_dir_all(&outer).expect("a stale scratch tree is removed");
    }
    let repo = outer.join("repo");
    copy_dir(&Path::new(FIXTURES).join("one-ready"), &repo);
    fs::write(repo.join("Cargo.toml"), "[package]\nname = \"scratch\"\n").expect("a file");
    fs::write(repo.join(".engineering/gates.yaml"), GATES).expect("a file");
    fs::write(
        repo.join(".engineering/commit-identity.conf"),
        "[identity]\n\tname = Test Bot\n\temail = bot@example.invalid\n",
    )
    .expect("the commit identity is written");
    if second {
        let stories = repo.join(".engineering/planning/story");
        let one = fs::read_to_string(stories.join("ready-one.md")).expect("ready-one");
        let two = one
            .replace("story:ready-one", "story:ready-two")
            .replace("Make the check pass", "Make the second check pass");
        fs::write(stories.join("ready-two.md"), two).expect("ready-two");
    }
    git(&repo, &["init", "-q", "-b", "main"]);
    git(&repo, &["add", "-A"]);
    git(&repo, &["commit", "-q", "-m", "start"]);
    (Scratch(outer), repo)
}

/// Runs the built `takt` binary with `args`: the exit code, stdout and stderr.
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

// story:takt-carries-no-organisations-own-route: the bot forge double is gone; these cases run on
// the local forge.
/// The local forge for `repo`: a bare hub `<dir>/hub.git` holding its `main`, named by its
/// `origin`, and a run file `<dir>/run.yaml` whose forge is `local` with `true` as its checks;
/// the run file. Nothing reaches a network.
fn local_hub(dir: &Path, repo: &Path) -> PathBuf {
    git(dir, &["init", "-q", "--bare", "-b", "main", "hub.git"]);
    let hub = dir.join("hub.git");
    git(
        repo,
        &["remote", "add", "origin", hub.to_str().expect("utf-8")],
    );
    git(repo, &["push", "-q", "origin", "main"]);
    let config = dir.join("run.yaml");
    fs::write(&config, "forge: local\nlocal_checks: [\"true\"]\n").expect("the run file");
    config
}

/// The one case directory under `repo`, by name.
fn the_case(repo: &Path) -> String {
    let names: Vec<String> = fs::read_dir(repo.join(".takt/cases"))
        .expect("a cases directory")
        .map(|entry| {
            entry
                .expect("an entry")
                .file_name()
                .to_string_lossy()
                .into_owned()
        })
        .collect();
    assert_eq!(names.len(), 1, "{names:?}");
    names[0].clone()
}

/// (A) `approvals_only` keeps the revision the grant was given at while `HEAD` is the
/// approval's own commit on top of it, on the premise that such a commit "changes nothing a
/// validator observes". `pr.open` observes `HEAD`: it refuses to push when `HEAD` is not the
/// implementation revision. A story's case on a protocol that asks an approval before the push
/// (a `protocol/1` file, `--protocol`, whose `pr.open` requires `repository.push`) reaches that
/// gate; `takt approve` grants it and commits its record on the case's branch; the run after it
/// keeps the revision at the gate and so can never push: every run ends `stopped: error`.
/// What the approval was given for is the push of the revision at the gate, so the run after
/// it pushes and opens the pull request.
#[test]
fn adv1_an_approval_before_the_push_lets_the_resumed_run_open_the_pull_request() {
    let (outer, repo) = repository("push-gate", false);
    let protocol = outer.0.join("push-gated.yaml");
    let yaml = canon_engineering::registry::get("story-delivery", 1)
        .expect("the delivery protocol")
        .yaml;
    let gated = yaml.replace("id: story.delivery", "id: story.push-gated").replace(
        "    may_produce:\n      - evidence: pull_request\n",
        "    requires:\n      - capability: repository.push\n    may_produce:\n      - evidence: pull_request\n",
    );
    assert_ne!(gated, yaml);
    assert!(gated.contains("capability: repository.push"));
    fs::write(&protocol, gated).expect("the protocol file");
    let config = local_hub(&outer.0, &repo);
    let config = config.to_str().expect("utf-8");
    let path = repo.to_str().expect("utf-8");
    let protocol = protocol.to_str().expect("utf-8");

    let (code, out, err) = takt(&[
        "run",
        "--repo",
        path,
        "--harness",
        "none",
        "--test-cmd",
        "true",
        "--protocol",
        protocol,
        "--config",
        config,
        "--iterations",
        "0",
    ]);
    assert_eq!(code, 3, "stdout {out} stderr {err}");
    assert!(out.contains("frontier: took story:ready-one"), "{out}");
    assert!(out.contains("on story.push-gated/1"), "{out}");
    let id = the_case(&repo);

    fs::write(repo.join("feature.txt"), "the work\n").expect("a file");
    git(&repo, &["add", "feature.txt"]);
    git(&repo, &["commit", "-q", "-m", "the work"]);
    let worked = git(&repo, &["rev-parse", "HEAD"]);

    let (code, out, err) = takt(&[
        "run",
        "--repo",
        path,
        "--harness",
        "none",
        "--test-cmd",
        "true",
        "--gate",
        "pr.open",
        "--config",
        config,
    ]);
    assert_eq!(code, 4, "stdout {out} stderr {err}");
    assert!(
        out.contains("stopped: ApprovalRequired (pr.open)"),
        "stdout {out} stderr {err}"
    );

    let (code, out, err) = takt(&[
        "approve",
        "--repo",
        path,
        &id,
        "repository.push",
        "--by",
        "human:operator",
    ]);
    assert_eq!(code, 0, "stdout {out} stderr {err}");
    assert_eq!(
        git(&repo, &["log", "-1", "--format=%s"]),
        "takt: approve story:ready-one repository.push"
    );
    assert_eq!(git(&repo, &["rev-parse", "HEAD^"]), worked);

    let (code, out, err) = takt(&[
        "run",
        "--repo",
        path,
        "--harness",
        "none",
        "--test-cmd",
        "true",
        "--config",
        config,
    ]);
    assert!(
        out.contains("evidence: pull_request open"),
        "the push the approval was given for never happens: exit {code}\nstdout {out}\nstderr {err}"
    );
}

/// (B) The store (on `main`, the base) lists `story:ready-one` active with its case open; the
/// take is on `main` and not on the case's branch, as `git cherry-pick` and `git branch -f`
/// leave it. Coordinator decision B: that state is not one takt supports, and the behaviour is
/// this. A case the base lists active is resumed from the base: a plain run from `main` resumes
/// it. From a tree on a case's branch only a case whose take is committed on its branch is
/// resumed: a plain run from the case's own branch, where the run before left the tree, does not
/// resume it, puts the tree on the base and takes the next ready story, `story:ready-two`,
/// opening no second case for `story:ready-one`. `takt run --help` says so.
#[test]
fn adv1_a_plain_run_on_the_cases_own_branch_resumes_only_a_case_whose_take_is_on_its_branch() {
    let (_outer, repo) = repository("own-branch", true);
    let path = repo.to_str().expect("utf-8");
    let (code, out, err) = takt(&[
        "run",
        "--repo",
        path,
        "--harness",
        "none",
        "--test-cmd",
        "true",
        "--forge",
        "none",
        "--protocol",
        "software.change/1",
        "--iterations",
        "0",
    ]);
    assert_eq!(code, 3, "the setup run: stdout {out} stderr {err}");
    assert!(out.contains("frontier: took story:ready-one"), "{out}");
    let case = the_case(&repo);
    let branch = format!("takt/{case}");
    git(&repo, &["checkout", "-q", "main"]);
    git(&repo, &["cherry-pick", &branch]);
    git(&repo, &["branch", "-f", &branch, "main~1"]);
    let plain = [
        "run",
        "--repo",
        path,
        "--harness",
        "none",
        "--test-cmd",
        "true",
        "--forge",
        "none",
        "--iterations",
        "0",
    ];

    let (code, out, err) = takt(&plain);
    assert!(
        out.contains(&format!(
            "frontier: resumed story:ready-one: Make the check pass (case {case})"
        )),
        "decision B: a case the base lists active is resumed from the base: exit {code}\nstdout {out}\nstderr {err}"
    );
    assert_eq!(git(&repo, &["branch", "--show-current"]), branch);

    let (code, out, err) = takt(&plain);
    assert!(
        !out.contains("frontier: resumed") && out.contains("frontier: took story:ready-two"),
        "decision B: from a tree on a case branch only a case whose take is committed on its branch is resumed: exit {code}\nstdout {out}\nstderr {err}"
    );
    assert_eq!(code, 3, "stdout {out} stderr {err}");
    let cases: Vec<String> = fs::read_dir(repo.join(".takt/cases"))
        .expect("a cases directory")
        .map(|entry| {
            entry
                .expect("an entry")
                .file_name()
                .to_string_lossy()
                .into_owned()
        })
        .filter(|name| name.starts_with("story-ready-one-"))
        .collect();
    assert_eq!(cases, [case], "no second case for story:ready-one");
}
