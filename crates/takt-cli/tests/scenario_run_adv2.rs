//! Adversary pass 2 against `takt scenario run`, after correction 1: the fetch of the remote's
//! `trial/*` branches, the `--porcelain` read of the push, the number checked before and after
//! the fetch, and what a failure after the push leaves. The bot route's programs and the
//! harness are doubles on the program path; the bot route double runs a real `git fetch` and a
//! real, unforced `git push` (every option takt gives passed through) against a bare repository
//! standing in for the scratch repository. Nothing reaches a network or a model.

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
    repo: PathBuf,
    bin: PathBuf,
    state: PathBuf,
    remote: PathBuf,
    /// The first commit: the fixture store with `story:ready-one` `proposed`. Scenario 1's seed.
    seed: String,
    /// A commit after `seed` on `main`, the store unchanged: scenario 2's seed.
    later: String,
}

/// The scratch repository: `seed`, then `later` (one more file, the store as it was), then the
/// declaration of scenario 1 (seed `seed`) and scenario 2 (seed `later`) on `main`; `origin` on
/// github.com, and a bare `remote.git` the bot route double fetches from and pushes into.
fn world(name: &str) -> World {
    let outer = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("scenario-adv2-{}-{name}", std::process::id()));
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
    fs::write(repo.join("later.txt"), "later\n").expect("a file");
    git(&repo, &["add", "later.txt"]);
    git(&repo, &["commit", "-q", "-m", "later"]);
    let later = git_out(&repo, &["rev-parse", "HEAD"]);
    fs::write(
        repo.join(SCENARIOS),
        format!(
            "[scenario \"1\"]\n\tstory = story:ready-one\n\tseed = {seed}\n\
             [scenario \"2\"]\n\tstory = story:ready-one\n\tseed = {later}\n"
        ),
    )
    .expect("the declaration");
    git(&repo, &["add", SCENARIOS]);
    git(&repo, &["commit", "-q", "-m", "declare the scenarios"]);
    git(
        &repo,
        &["remote", "add", "origin", "git@github.com:owner/repo.git"],
    );
    let remote = outer.join("remote.git");
    git(
        &outer,
        &["init", "-q", "-b", "main", "--bare", "remote.git"],
    );
    let bin = outer.join("stub-bin");
    let state = outer.join("forge");
    fs::create_dir_all(&state).expect("a state directory");
    doubles(&bin, &state, &remote);
    World {
        _scratch: Scratch(outer),
        repo,
        bin,
        state,
        remote,
        seed,
        later,
    }
}

/// `b10x-gates bot -- fetch <remote> <refspec>`: a real `git fetch` from the bare `remote`
/// (standing in for the HTTPS server, so without the HTTPS-only transport takt sets), or
/// nothing when `state/no-fetch` exists. `b10x-gates bot -- push [options] <remote> <spec>`: when
/// `state/race` exists, first the push it holds (a refspec) is made into `remote` and the file
/// removed, standing in for another checkout's run landing between this run's fetch and its
/// push; then a real, unforced `git push` of `<spec>` with every option takt gave, its report on
/// stdout as git prints it. `api --method POST` answers pull request `k`. `claude` answers
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
    shift 2
    verb="$1"; shift
    options=''
    while [ $# -gt 0 ]; do
      case "$1" in
        -*) options="$options $1"; shift ;;
        *) break ;;
      esac
    done
    spec="$2"
    case "$verb" in
      fetch)
        if [ -e "$state/no-fetch" ]; then exit 0; fi
        env -u GIT_ALLOW_PROTOCOL git fetch -q "$remote" "$spec"; exit $? ;;
      push)
        if [ -e "$state/race" ]; then
          git push -q "$remote" "$(cat "$state/race")" || exit 3
          rm "$state/race"
        fi
        git push $options "$remote" "$spec" 2>/dev/null; exit $? ;;
    esac ;;
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
            .env("XDG_STATE_HOME", self.state.join("xdg-state"))
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

    fn remote_ref(&self, name: &str) -> String {
        git_out(&self.remote, &["rev-parse", name])
    }

    fn calls(&self) -> Vec<String> {
        fs::read_to_string(self.state.join("calls"))
            .unwrap_or_default()
            .lines()
            .map(str::to_owned)
            .collect()
    }
}

/// Two checkouts run scenarios at once: both fetch, both find no trial branch, both choose
/// `trial/0`. The other run (scenario 1, seed `seed`) pushes first; this run (scenario 2, seed
/// `later`, a descendant of `seed`) pushes `trial/0` unforced, and git fast-forwards the
/// remote's `trial/0` from `seed` to `later`, exit 0, porcelain flag ` `. takt reads no `*` and
/// refuses, but the other run's base has already been moved under its open pull request: its
/// trial no longer starts at its seed. The push must create or do nothing
/// (`--force-with-lease=refs/heads/trial/<n>:` refuses a ref that exists, as "stale info").
#[test]
fn a_trial_branch_another_run_creates_after_the_fetch_is_not_moved_by_this_runs_push() {
    let world = world("race");
    fs::write(
        world.state.join("race"),
        format!("{}:refs/heads/trial/0", world.seed),
    )
    .expect("the other run's push");

    let (code, out, err) = world.scenario_run("2", &["--iterations", "0"]);
    assert_eq!(
        world.remote_ref("refs/heads/trial/0"),
        world.seed,
        "the other run's trial/0 was moved from its seed to this run's seed {}: exit {code}, stdout {out} stderr {err}",
        world.later
    );
    assert_eq!(code, 1, "stdout {out} stderr {err}");
}

/// The number is checked twice, before the fetch and after, and the first check can refuse a
/// run the second would have numbered past the collision. The checkout holds the case branch
/// `takt/scenario-1-trial-0` of an earlier run whose local `trial/0` was deleted by hand; the
/// remote still holds `trial/0`. Before the fetch the checkout knows no trial, so `n` is 0 and
/// the case branch refuses the run; after the fetch `n` would be 1 and free.
#[test]
fn a_collision_the_fetch_would_number_past_does_not_refuse_the_run() {
    let world = world("pre-check");
    git(
        &world.repo,
        &[
            "push",
            "-q",
            world.remote.to_str().expect("utf-8"),
            &format!("{}:refs/heads/trial/0", world.seed),
        ],
    );
    git(
        &world.repo,
        &["branch", "takt/scenario-1-trial-0", &world.seed],
    );

    let (code, out, err) = world.scenario_run("1", &["--iterations", "0"]);
    assert!(
        out.contains("scenario 1: story:ready-one proposed on trial/1 at "),
        "refused before the fetch on trial/0, which the fetch would have numbered past: exit {code}, stdout {out} stderr {err}; calls {:?}",
        world.calls()
    );
    assert_eq!(world.remote_ref("refs/heads/trial/0"), world.seed);
}

/// The store at the seed lists `story:blocked-one` `proposed`, and an open decision blocks it
/// (`blocked_by` in the same `aep plan artifact list` reading the pre-push check takes its status
/// from). The check reads the status only, not the frontier's ready rule
/// (`takt::frontier::is_ready`: proposed, nothing blocks it, its dependencies done), and `aep`
/// moves a blocked story to `active` without a word: the run pushes `trial/0`, takes the
/// blocked story and runs it, which `takt run` never does. A seed whose story is not ready is no
/// known starting state, to be refused before the push like one whose story is not `proposed`.
/// (A first version of this case assumed `aep` refuses the take; it does not.)
#[test]
fn a_proposed_story_the_frontier_calls_blocked_is_refused_before_the_push() {
    let world = world("blocked");
    fs::write(
        world.repo.join(SCENARIOS),
        format!(
            "[scenario \"3\"]\n\tstory = story:blocked-one\n\tseed = {}\n",
            world.seed
        ),
    )
    .expect("the declaration");
    git(&world.repo, &["add", SCENARIOS]);
    git(&world.repo, &["commit", "-q", "-m", "declare scenario 3"]);

    let (code, out, err) = world.scenario_run("3", &["--iterations", "0"]);
    let pushed = git_out(&world.remote, &["for-each-ref", "--format=%(refname)"]);
    let branch = git_out(&world.repo, &["branch", "--show-current"]);
    assert!(
        pushed.is_empty() && branch == "main",
        "a blocked story was pushed for and taken: exit {code}, the remote holds [{pushed}], the tree is on {branch}; stdout {out} stderr {err}"
    );
    assert_eq!(code, 1, "stdout {out} stderr {err}");
    assert!(err.contains("story:blocked-one"), "stderr {err}");
}

/// The porcelain read must count only the run's own branch. With `push.followTags` in the
/// operator's configuration and an annotated tag on the seed, the push reports `*` for the tag
/// and `=` for a `trial/0` the remote already holds (the fetch brought nothing): the run must be
/// refused. Green against the correction; it is red against the mutant that drops the
/// `ends_with(":refs/heads/<trial>")` condition, which no case of the implementor's suite kills.
#[test]
fn a_new_tag_in_the_push_report_does_not_count_as_the_trial_branch_created() {
    let world = world("follow-tags");
    git(
        &world.repo,
        &[
            "push",
            "-q",
            world.remote.to_str().expect("utf-8"),
            &format!("{}:refs/heads/trial/0", world.seed),
        ],
    );
    git(&world.repo, &["tag", "-a", "v1", "-m", "v1", &world.seed]);
    git(&world.repo, &["config", "push.followTags", "true"]);
    assert!(
        git_out(&world.remote, &["tag", "--list"]).is_empty(),
        "the remote lacks the tag, so the run's push carries it"
    );
    fs::write(world.state.join("no-fetch"), "").expect("the double fetches nothing");

    let (code, out, err) = world.scenario_run("1", &["--iterations", "0"]);
    assert_eq!(code, 1, "stdout {out} stderr {err}");
    assert!(err.contains("not created"), "stderr {err}");
    assert!(!out.contains("on trial/0 at"), "stdout {out}");
    assert!(
        !world.repo.join(".engineering/cases").exists(),
        "a case was opened on the remote's trial/0"
    );
}
