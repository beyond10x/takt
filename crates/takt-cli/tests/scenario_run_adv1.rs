//! Adversary pass 1 against `takt scenario run`: where a scenario run does not start from a
//! state of its own. The bot route's programs and the harness are doubles on the program path;
//! one double pushes for real into a bare repository standing in for the scratch repository's
//! remote. Nothing reaches a network or a model.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

/// The fixture stores of the library crate, one directory each.
const FIXTURES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../takt/tests/fixtures");

/// A gates document whose one assertion is true in a repository with a manifest.
const GATES: &str = "format: engineering-gates/1\nlanguage: canon-expr/1\ngates:\n  - id: manifest\n    \
                     text: The repository has its manifest.\n    assertions:\n      \
                     - 'fs.file.exists(\"Cargo.toml\")'\n";

/// Where the scratch repository declares its scenarios.
const SCENARIOS: &str = ".engineering/scenarios.conf";

fn git(tree: &Path, args: &[&str]) {
    let status = Command::new("git")
        .args(["-c", "user.name=t", "-c", "user.email=t@localhost"])
        .args(args)
        .current_dir(tree)
        .status()
        .expect("git runs");
    assert!(status.success(), "git {args:?}");
}

fn git_out(tree: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .args(args)
        .current_dir(tree)
        .output()
        .expect("git runs");
    assert!(output.status.success(), "git {args:?}");
    String::from_utf8_lossy(&output.stdout).trim().to_owned()
}

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

fn executable(bin: &Path, name: &str, text: &str) {
    let path = bin.join(name);
    fs::write(&path, text).expect("the stub is written");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).expect("executable");
    }
}

struct World {
    _scratch: Scratch,
    outer: PathBuf,
    repo: PathBuf,
    bin: PathBuf,
    state: PathBuf,
    remote: PathBuf,
    seed: String,
}

/// The scratch repository as the implementor's suite builds it, plus a bare repository
/// `remote.git` the bot route double really pushes into.
fn world(name: &str) -> World {
    let outer = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("scenario-adv1-{}-{name}", std::process::id()));
    if outer.exists() {
        fs::remove_dir_all(&outer).expect("a stale scratch tree is removed");
    }
    let repo = outer.join("repo");
    copy_dir(&Path::new(FIXTURES).join("one-ready"), &repo);
    fs::write(repo.join("Cargo.toml"), "[package]\nname = \"scratch\"\n").expect("a file");
    fs::write(repo.join(".engineering/gates.yaml"), GATES).expect("a file");
    git(&repo, &["init", "-q", "-b", "main"]);
    git(&repo, &["add", "-A"]);
    git(&repo, &["commit", "-q", "-m", "seed"]);
    let seed = git_out(&repo, &["rev-parse", "HEAD"]);
    declare(
        &repo,
        &format!("[scenario \"1\"]\n\tstory = story:ready-one\n\tseed = {seed}\n"),
    );
    git(
        &repo,
        &["remote", "add", "origin", "git@github.com:owner/repo.git"],
    );
    let remote = outer.join("remote.git");
    git(&outer, &["init", "-q", "--bare", "remote.git"]);
    let bin = outer.join("stub-bin");
    let state = outer.join("forge");
    fs::create_dir_all(&state).expect("a state directory");
    doubles(&bin, &state, &remote);
    World {
        _scratch: Scratch(outer.clone()),
        outer,
        repo,
        bin,
        state,
        remote,
        seed,
    }
}

fn declare(repo: &Path, text: &str) {
    fs::write(repo.join(SCENARIOS), text).expect("the declaration is written");
    git(repo, &["add", SCENARIOS]);
    git(repo, &["commit", "-q", "-m", "declare the scenarios"]);
}

/// `b10x-gates bot -- push <remote> <spec>` runs a real, unforced `git push` of `<spec>` into
/// the bare `remote`, as the bot route's push does into the scratch repository, and logs the
/// line; `api --method POST` keeps the request as `pr-<k>.json`. `claude` answers
/// `implementation.edit`.
fn doubles(bin: &Path, state: &Path, remote: &Path) {
    fs::create_dir_all(bin).expect("a stub directory");
    let state = state.display().to_string();
    let remote = remote.display().to_string();
    executable(
        bin,
        "b10x-gates",
        &format!(
            r#"#!/bin/sh
state='{state}'
remote='{remote}'
echo "$*" >> "$state/calls"
case "$1" in
  bot)
    git push -q "$remote" "$5" || exit 1
    exit 0 ;;
  scan-text) exit 0 ;;
  api)
    method=''; input=''; output=''
    while [ $# -gt 0 ]; do
      case "$1" in
        --method) method="$2"; shift ;;
        --input) input="$2"; shift ;;
        --output) output="$2"; shift ;;
      esac
      shift
    done
    if [ "$method" = POST ]; then
      k=$(( $(ls "$state" | grep -c '^pr-.*\.json$') + 1 ))
      cp "$input" "$state/pr-$k.json"
      printf '{{"number": %s}}' "$k" > "$output"
      exit 0
    fi ;;
esac
echo "b10x-gates double: $*" >&2
exit 2
"#
        ),
    );
    executable(bin, "gh", "#!/bin/sh\necho \"gh double: $*\" >&2\nexit 2\n");
    executable(
        bin,
        "claude",
        r#"#!/bin/sh
echo "the work" >> work.txt
printf '{"result":"{\\"action\\": \\"implementation.edit\\", \\"summary\\": \\"the double\\"}","usage":{"input_tokens":1}}'
"#,
    );
}

impl World {
    fn scenario_run(&self, scenario: &str, extra: &[&str]) -> (i32, String, String) {
        let repo = self.repo.to_str().expect("utf-8");
        let mut args = vec![
            "scenario",
            "run",
            "--repo",
            repo,
            scenario,
            "--test-cmd",
            "true",
        ];
        args.extend_from_slice(extra);
        let output = Command::new(env!("CARGO_BIN_EXE_takt"))
            .args(&args)
            .env_remove("B10X_GATES_POLICY")
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
            output.status.code().unwrap_or(-1),
            String::from_utf8_lossy(&output.stdout).into_owned(),
            String::from_utf8_lossy(&output.stderr).into_owned(),
        )
    }

    fn calls(&self) -> Vec<String> {
        fs::read_to_string(self.state.join("calls"))
            .unwrap_or_default()
            .lines()
            .map(str::to_owned)
            .collect()
    }

    fn trial_branches(&self) -> Vec<String> {
        git_out(
            &self.repo,
            &[
                "for-each-ref",
                "--format=%(refname:short)",
                "refs/heads/trial/",
            ],
        )
        .lines()
        .map(str::to_owned)
        .collect()
    }
}

/// `git::config_at` says the declaration is "read with `git config --blob`, so nothing is
/// checked out and no include is followed" (crates/takt/src/git.rs). Git follows an
/// `include.path` in a blob (git 2.55: `git config --blob <rev>:<path> --list` lists the
/// included file's entries). A `scenarios.conf` on `main` that declares nothing itself and
/// includes a file outside the repository must not yield a scenario: the declaration is the
/// repository's, at `--declared-on`, and nothing else.
#[test]
fn a_scenario_declared_only_through_an_include_outside_the_repository_is_refused() {
    let world = world("include");
    let outside = world.outer.join("outside.conf");
    fs::write(
        &outside,
        format!(
            "[scenario \"1\"]\n\tstory = story:ready-one\n\tseed = {}\n",
            world.seed
        ),
    )
    .expect("the outside file");
    declare(
        &world.repo,
        &format!("[include]\n\tpath = {}\n", outside.display()),
    );

    let (code, out, err) = world.scenario_run("1", &["--iterations", "0"]);
    assert_eq!(
        code, 1,
        "a scenario only an included file outside the repository declares was run: stdout {out} stderr {err}"
    );
    assert!(err.contains("declares no scenario 1"), "stderr {err}");
    assert!(
        world.trial_branches().is_empty(),
        "{:?}",
        world.trial_branches()
    );
    assert!(world.calls().is_empty(), "{:?}", world.calls());
}

/// The remote already has `trial/0` (a run from another checkout of the scratch repository,
/// stopped at its approval, so its base is still the seed), and this checkout has not fetched
/// since. `next_trial` picks `trial/0`; the unforced push of a branch the remote already holds
/// at the same commit succeeds as up to date, so the run says "pushed" and opens its case on a
/// base another run owns. The acceptance asks for a pull request against the run's *own*
/// `trial/<n>`: a run must refuse, or take a number the remote does not hold.
#[test]
fn a_trial_branch_the_remote_holds_but_the_checkout_never_fetched_is_not_reused() {
    let world = world("unfetched");
    git(
        &world.repo,
        &[
            "push",
            "-q",
            world.remote.to_str().expect("utf-8"),
            &format!("{}:refs/heads/trial/0", world.seed),
        ],
    );
    assert!(
        world.trial_branches().is_empty()
            && git_out(&world.repo, &["for-each-ref", "refs/remotes/"]).is_empty(),
        "the checkout knows no trial branch"
    );

    let (code, out, err) = world.scenario_run("1", &["--iterations", "0"]);
    assert!(
        !out.contains("on trial/0 at"),
        "the run took trial/0, which the remote held already: exit {code}, stdout {out} stderr {err}"
    );
    assert!(
        !world
            .repo
            .join(".engineering/cases/scenario-1-trial-0")
            .exists(),
        "a case was opened on the remote's existing trial/0: stdout {out} stderr {err}"
    );
}

/// A refusal is to come before the push ("A refusal before the push removes the branch and
/// puts the tree back"), and the case id is checked for a collision before anything changes,
/// but only as a directory. The case's branch `takt/scenario-1-trial-0` left behind with its
/// directory gone (the hand reset of the live trials moved a case directory) collides only
/// once the case opens, after `trial/0` was pushed: the run ends with a pushed trial branch
/// and the tree left on it.
#[test]
fn a_case_branch_that_collides_is_refused_before_the_trial_branch_is_pushed() {
    let world = world("case-branch");
    git(
        &world.repo,
        &["branch", "takt/scenario-1-trial-0", &world.seed],
    );

    let (code, out, err) = world.scenario_run("1", &["--iterations", "0"]);
    assert_eq!(code, 1, "stdout {out} stderr {err}");
    assert!(
        world.calls().is_empty(),
        "trial/0 was pushed before the case's branch collided: {:?}; stderr {err}",
        world.calls()
    );
    assert_eq!(
        git_out(&world.repo, &["branch", "--show-current"]),
        "main",
        "stderr {err}"
    );
    assert!(
        world.trial_branches().is_empty(),
        "{:?}",
        world.trial_branches()
    );
}

/// `next_trial` answers one past the highest number under `trial/`, `u64` parsed, `n + 1`
/// unchecked. A remote-tracking `trial/18446744073709551615` (any remote's; `refs/remotes/` is
/// outside the step guard's `refs/heads/` and `refs/tags/`) overflows it: a panic in a debug
/// build, `trial/0` again in a release build. A number past the highest is to be refused in
/// one line, not crash the run.
#[test]
fn a_trial_number_at_the_top_of_u64_is_refused_not_a_panic() {
    let world = world("overflow");
    git(
        &world.repo,
        &[
            "update-ref",
            "refs/remotes/origin/trial/18446744073709551615",
            &world.seed,
        ],
    );

    let (code, out, err) = world.scenario_run("1", &["--iterations", "0"]);
    assert!(
        !err.contains("panicked"),
        "takt panicked: exit {code}, stderr {err}"
    );
    assert_eq!(code, 1, "stdout {out} stderr {err}");
    assert!(world.calls().is_empty(), "{:?}", world.calls());
}
