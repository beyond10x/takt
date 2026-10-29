//! `takt scenario run`: each run of a scenario starts from a fresh base branch `trial/<n>`, cut
//! from the scenario's seed commit, pushed through the bot route and checked out before the case
//! opens, so the case's pull request targets that branch and the store on it shows the scenario's
//! story `proposed` again; no hand reset between runs. The bot route's programs and the harness
//! are doubles on the program path; nothing reaches a network or a model.

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

/// Where the scratch repository declares its scenarios.
const SCENARIOS: &str = ".engineering/scenarios.conf";

/// The story file of the fixture's one ready story.
const STORY_FILE: &str = ".engineering/planning/story/ready-one.md";

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

/// Writes `name` into `bin`, executable, with `text`.
fn executable(bin: &Path, name: &str, text: &str) {
    let path = bin.join(name);
    fs::write(&path, text).expect("the stub is written");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).expect("executable");
    }
}

/// One scratch world: the repository at `repo` with an `origin` on github.com, the doubles of
/// the bot route and the harness in `bin`, their state in `state`.
struct World {
    _scratch: Scratch,
    repo: PathBuf,
    bin: PathBuf,
    state: PathBuf,
    /// The bare repository the bot route double fetches from and pushes into, standing in for
    /// the scratch repository on its host.
    remote: PathBuf,
    /// The seed commit: the fixture store with its one story `proposed`.
    seed: String,
}

/// The scratch repository: the seed commit holds the fixture store with `story:ready-one`
/// `proposed`, a manifest and a gates document; `main` moves on past it with the scenario
/// declaration, so a base cut from `main` instead of the seed is told apart.
fn world(name: &str) -> World {
    let outer = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("scenario-run-{}-{name}", std::process::id()));
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
    configure_identity(&repo);
    git(
        &repo,
        &["remote", "add", "origin", "git@github.com:owner/repo.git"],
    );
    let bin = outer.join("stub-bin");
    let state = outer.join("forge");
    fs::create_dir_all(&state).expect("a state directory");
    let remote = outer.join("remote.git");
    git(
        &outer,
        &["init", "-q", "-b", "main", "--bare", "remote.git"],
    );
    doubles(&bin, &state, &remote);
    World {
        _scratch: Scratch(outer),
        repo,
        bin,
        state,
        remote,
        seed,
    }
}

/// The identity the scratch repository configures for takt's commits.
const IDENTITY: &str = "b10x-bot[bot] <316511680+b10x-bot[bot]@users.noreply.github.com>";

/// Commits `.engineering/commit-identity.conf` naming [`IDENTITY`] on the branch the tree is on:
/// a scenario run reads it at `--declared-on`, and through the bot route needs it.
fn configure_identity(repo: &Path) {
    let file = ".engineering/commit-identity.conf";
    fs::write(
        repo.join(file),
        "[identity]\n\tname = b10x-bot[bot]\n\temail = 316511680+b10x-bot[bot]@users.noreply.github.com\n",
    )
    .expect("the identity is written");
    git(repo, &["add", file]);
    git(
        repo,
        &["commit", "-q", "-m", "configure the commit identity"],
    );
}

/// Commits `text` as the scenario declaration on the branch the tree is on.
fn declare(repo: &Path, text: &str) {
    fs::write(repo.join(SCENARIOS), text).expect("the declaration is written");
    git(repo, &["add", SCENARIOS]);
    git(repo, &["commit", "-q", "-m", "declare the scenarios"]);
}

/// The doubles in `bin`, their state in `state`. Every `b10x-gates` line is logged to
/// `state/calls`. `b10x-gates bot -- fetch <remote> <refspec>` records the prompt settings, the
/// SSH agent socket, its standard input, `GIT_ASKPASS` and `GIT_ALLOW_PROTOCOL` in
/// `state/fetch-env`, then runs a real `git fetch` of
/// the refspec from the bare `remote` (standing in for the HTTPS server, so without the
/// HTTPS-only transport takt sets for the route's own git), or does nothing when `state/no-fetch` exists, sleeps 45 s
/// and fails when `state/hang-fetch` exists, standing in for a fetch waiting on a prompt, and
/// fails at once with git's access-rights message when `state/refuse-fetch` exists; `b10x-gates bot
/// -- push [options] <remote> <spec>` appends its spec and its `GIT_ALLOW_PROTOCOL` to
/// `state/push-env`, runs a real `git push` of the spec into `remote` (again standing in for the
/// HTTPS server, so without the HTTPS-only transport) with every
/// option takt gave (`--porcelain`, `--force-with-lease`), its report on stdout as git prints it,
/// keeps the commit pushed per branch (`pushed-<branch, / as _>`), or exits 1 when
/// `state/refuse-push` exists; `scan-text` passes;
/// `api --method POST` keeps the request as `pr-<k>.json` and answers pull request `k`,
/// counting from 1. `gh pr view` reports the pull request open with the head it was opened
/// for, at the commit last pushed for it; `gh pr checks` one check, passing. `claude` appends a
/// line to `work.txt` and answers `implementation.edit`, or, once an approval is offered,
/// answers `repository.merge`, or, once its `work.txt` exists and `change.handover` is offered,
/// takes it, changing nothing.
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
        {{
          printf 'GIT_TERMINAL_PROMPT=%s\n' "${{GIT_TERMINAL_PROMPT-unset}}"
          printf 'GIT_SSH_COMMAND=%s\n' "${{GIT_SSH_COMMAND-unset}}"
          printf 'SSH_AUTH_SOCK=%s\n' "${{SSH_AUTH_SOCK-unset}}"
          if [ /dev/stdin -ef /dev/null ]; then stdin=/dev/null; else stdin=$(readlink /proc/self/fd/0 2>/dev/null || echo 'not /dev/null'); fi
          printf 'stdin=%s\n' "$stdin"
          printf 'GIT_ASKPASS=%s\n' "${{GIT_ASKPASS-unset}}"
          printf 'GIT_ALLOW_PROTOCOL=%s\n' "${{GIT_ALLOW_PROTOCOL-unset}}"
        }} > "$state/fetch-env"
        if [ -e "$state/hang-fetch" ]; then sleep 45; exit 1; fi
        if [ -e "$state/refuse-fetch" ]; then
          echo "git@github.com: Permission denied (publickey)." >&2
          echo "fatal: Could not read from remote repository." >&2
          echo "Please make sure you have the correct access rights" >&2
          exit 1
        fi
        if [ -e "$state/no-fetch" ]; then exit 0; fi
        env -u GIT_ALLOW_PROTOCOL git fetch -q "$remote" "$spec"; exit $? ;;
      push)
        printf '%s GIT_ALLOW_PROTOCOL=%s\n' "$spec" "${{GIT_ALLOW_PROTOCOL-unset}}" >> "$state/push-env"
        if [ -e "$state/refuse-push" ]; then echo "refused by the double" >&2; exit 1; fi
        branch="${{spec#*:refs/heads/}}"; revision="${{spec%%:*}}"
        if [ "$branch" = "$spec" ]; then revision="refs/heads/$spec"; fi
        git rev-parse "$revision" > "$state/pushed-$(printf '%s' "$branch" | tr / _)" || exit 1
        env -u GIT_ALLOW_PROTOCOL git push $options "$remote" "$spec" 2>/dev/null; exit $? ;;
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
      sed -n 's/.*"head": "\(.*\)".*/\1/p' "$input" > "$state/pr-$k.head"
      printf '{{"number": %s}}' "$k" > "$output"
      exit 0
    fi ;;
esac
echo "b10x-gates double: $*" >&2
exit 2
"#
        ),
    );
    executable(
        bin,
        "gh",
        &format!(
            r#"#!/bin/sh
state='{state}'
case "$2" in
  view)
    head=$(cat "$state/pr-$3.head")
    oid=$(tr -d '\n' < "$state/pushed-$(printf '%s' "$head" | tr / _)")
    printf '{{"state":"OPEN","mergedAt":null,"headRefName":"%s","headRefOid":"%s","baseRefName":"main"}}' "$head" "$oid"
    exit 0 ;;
  checks) printf '[{{"bucket":"pass"}}]'; exit 0 ;;
esac
echo "gh double: $*" >&2
exit 2
"#
        ),
    );
    executable(
        bin,
        "claude",
        r#"#!/bin/sh
case $1 in --version) echo '2.1.309 (Claude Code)'; exit 0 ;; esac
case "$2" in
  *"These need an approval"*) action=repository.merge ;;
  *"- change.handover:"*)
    if [ -e work.txt ]; then action=change.handover;
    else echo "the work" >> work.txt; action=implementation.edit; fi ;;
  *) echo "the work" >> work.txt; action=implementation.edit ;;
esac
printf '{"result":"{\\"action\\": \\"%s\\", \\"summary\\": \\"the double\\"}","usage":{"input_tokens":1}}' "$action"
"#,
    );
}

impl World {
    /// Runs the built `takt` binary with `args` in this world: the doubles first on the program
    /// path, no `b10x-gates` policy. The exit code, stdout and stderr.
    fn takt(&self, args: &[&str]) -> (i32, String, String) {
        self.takt_env(args, &[])
    }

    /// [`Self::takt`] with `env` set in takt's own environment.
    fn takt_env(&self, args: &[&str], env: &[(&str, &str)]) -> (i32, String, String) {
        let output = Command::new(env!("CARGO_BIN_EXE_takt"))
            .args(args)
            .env("XDG_STATE_HOME", self.state.join("xdg-state"))
            .envs(env.iter().copied())
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
            output.status.code().expect("an exit code"),
            String::from_utf8_lossy(&output.stdout).into_owned(),
            String::from_utf8_lossy(&output.stderr).into_owned(),
        )
    }

    /// `takt scenario run --repo <repo> <scenario> --test-cmd true`, `extra` last.
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
        self.takt(&args)
    }

    /// The lines the `b10x-gates` double was run with, in order.
    fn calls(&self) -> Vec<String> {
        fs::read_to_string(self.state.join("calls"))
            .unwrap_or_default()
            .lines()
            .map(str::to_owned)
            .collect()
    }

    /// The commit the double holds as pushed for `branch`.
    fn pushed(&self, branch: &str) -> Option<String> {
        fs::read_to_string(
            self.state
                .join(format!("pushed-{}", branch.replace('/', "_"))),
        )
        .ok()
        .map(|text| text.trim().to_owned())
    }

    /// The field `name` of pull request request `k` the double was sent, read from takt's
    /// pretty-printed JSON (`"name": "value"`, one field a line).
    fn request(&self, k: u32, name: &str) -> String {
        let text = fs::read_to_string(self.state.join(format!("pr-{k}.json")))
            .unwrap_or_else(|_| panic!("pull request {k} was opened"));
        let key = format!("\"{name}\": \"");
        text.lines()
            .find_map(|line| line.trim().strip_prefix(&key))
            .and_then(|rest| rest.strip_suffix("\",").or_else(|| rest.strip_suffix('"')))
            .unwrap_or_else(|| panic!("pull request {k} has no {name}: {text}"))
            .to_owned()
    }

    /// The local branches named `trial/…`.
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

    /// `git status --porcelain` outside the cases directory: what a person would have to reset.
    fn changed_outside_cases(&self) -> String {
        git_out(
            &self.repo,
            &["status", "--porcelain", "--", ".", ":!.takt/cases"],
        )
    }

    /// The status the store at `revision` gives `story:ready-one`, read from its file.
    fn status_at(&self, revision: &str) -> String {
        let text = git_out(&self.repo, &["show", &format!("{revision}:{STORY_FILE}")]);
        text.lines()
            .find_map(|line| line.strip_prefix("status: "))
            .expect("the story has a status")
            .to_owned()
    }
}

/// The story's acceptance: two scenario 1 runs in a row, with no command between them, each
/// start with the story `proposed` on a fresh `trial/<n>` cut from the seed, pushed through the
/// bot route before the case opens, and each opens its pull request against its own
/// `trial/<n>`; the second resumes nothing of the first.
#[test]
fn two_scenario_runs_in_a_row_each_open_their_pull_request_against_their_own_trial_branch() {
    let world = world("two-runs");
    let seed8 = &world.seed[..8];

    let (code, out, err) = world.scenario_run("1", &[]);
    assert_eq!(code, 4, "stdout {out} stderr {err}");
    assert!(
        out.contains(&format!(
            "scenario 1: story:ready-one proposed on trial/0 at {seed8}, pushed through bot"
        )),
        "stdout {out} stderr {err}"
    );
    assert!(out.contains("frontier: took story:ready-one"), "{out}");
    assert!(
        out.contains("case scenario-1-trial-0 on story.delivery/1"),
        "{out}"
    );
    assert!(out.contains("evidence: pull_request open"), "{out}");
    assert!(
        out.contains("stopped: ApprovalRequired (repository.merge)"),
        "stdout {out} stderr {err}"
    );
    assert_eq!(world.request(1, "base"), "trial/0");
    assert_eq!(world.request(1, "head"), "takt/scenario-1-trial-0");
    assert_eq!(
        world.pushed("trial/0").as_deref(),
        Some(world.seed.as_str())
    );
    // The remote's trial branches are fetched, then the commit identity is scanned at open, then
    // the base is pushed, before anything else. The scan's path names the process, so it is matched
    // by its start and its end.
    let calls = world.calls();
    assert_eq!(calls.first(), Some(&FETCH_TRIALS.to_owned()), "{calls:?}");
    assert!(
        calls.get(1).is_some_and(
            |call| call.starts_with("scan-text --repository owner/repo ")
                && call.ends_with("/commit-identity.txt")
        ),
        "the commit identity is scanned right after the fetch: {calls:?}"
    );
    assert_eq!(
        calls.get(2),
        Some(&"bot -- push --porcelain --force-with-lease=refs/heads/trial/0: origin refs/heads/trial/0:refs/heads/trial/0".to_owned()),
        "the base is pushed right after the scan: {calls:?}"
    );
    assert_eq!(
        git_out(&world.remote, &["rev-parse", "refs/heads/trial/0"]),
        world.seed
    );

    // Nothing between the runs: the tree is clean outside the cases directory.
    assert_eq!(world.changed_outside_cases(), "");

    let (code, out, err) = world.scenario_run("1", &[]);
    assert_eq!(code, 4, "stdout {out} stderr {err}");
    assert!(
        out.contains(&format!(
            "scenario 1: story:ready-one proposed on trial/1 at {seed8}, pushed through bot"
        )),
        "stdout {out} stderr {err}"
    );
    assert!(!out.contains("frontier: resumed"), "{out}");
    assert!(out.contains("frontier: took story:ready-one"), "{out}");
    assert!(
        out.contains("case scenario-1-trial-1 on story.delivery/1"),
        "{out}"
    );
    assert!(
        out.contains("stopped: ApprovalRequired (repository.merge)"),
        "stdout {out} stderr {err}"
    );
    assert_eq!(world.request(2, "base"), "trial/1");
    assert_eq!(world.request(2, "head"), "takt/scenario-1-trial-1");
    assert_eq!(
        world.pushed("trial/1").as_deref(),
        Some(world.seed.as_str())
    );

    // Each trial base is the seed, its store the seed's: the story proposed on both.
    assert_eq!(world.trial_branches(), ["trial/0", "trial/1"]);
    for trial in ["trial/0", "trial/1"] {
        assert_eq!(git_out(&world.repo, &["rev-parse", trial]), world.seed);
        assert_eq!(world.status_at(trial), "proposed");
    }
    // Each case records its own trial branch as its base.
    for n in [0, 1] {
        let branch = fs::read_to_string(
            world
                .repo
                .join(format!(".takt/cases/scenario-1-trial-{n}/branch.yaml")),
        )
        .expect("the case records its branch");
        assert!(branch.contains(&format!("base: trial/{n}")), "{branch}");
    }
    assert_eq!(world.changed_outside_cases(), "");
}

/// story:takt-commits-with-the-repositorys-identity: a trial's commits, the take and the
/// harness's edits, carry the identity `--declared-on` configures as author and committer, over
/// the operator's identity variables in takt's environment and the operator's `author.*` and
/// `committer.*` configuration; and so do the commits the bot route pushed.
#[test]
fn a_trials_commits_carry_the_identity_the_declaring_revision_configures() {
    let world = world("identity");
    git(&world.repo, &["config", "author.name", "Operator Author"]);
    git(
        &world.repo,
        &["config", "committer.email", "committer@example.invalid"],
    );
    let repo = world.repo.to_str().expect("utf-8");
    let (code, out, err) = world.takt_env(
        &["scenario", "run", "--repo", repo, "1", "--test-cmd", "true"],
        &[
            ("GIT_AUTHOR_NAME", "Env Author"),
            ("GIT_AUTHOR_EMAIL", "env-author@example.invalid"),
            ("GIT_COMMITTER_NAME", "Env Committer"),
            ("GIT_COMMITTER_EMAIL", "env-committer@example.invalid"),
        ],
    );
    let said = format!("stdout {out} stderr {err}");
    assert_eq!(code, 4, "{said}");
    let range = "refs/heads/trial/0..refs/heads/takt/scenario-1-trial-0";
    for (place, tree) in [("the checkout", &world.repo), ("the remote", &world.remote)] {
        let commits = git_out(
            tree,
            &["log", "--format=%s|%an <%ae>|%cn <%ce>", range, "--"],
        );
        assert!(commits.lines().count() >= 2, "{place}: {commits}\n{said}");
        for line in commits.lines() {
            let (_, sides) = line.split_once('|').expect("a subject");
            assert_eq!(
                sides,
                format!("{IDENTITY}|{IDENTITY}"),
                "{place}: {commits}\n{said}"
            );
        }
    }
}

/// The safety fact: a run that stopped before its first commit can leave the store file dirty,
/// and a checkout would carry it onto the next trial branch. The setup refuses a dirty tree,
/// names the path, and changes nothing: the file keeps its change, no branch is cut, nothing is
/// pushed.
#[test]
fn a_dirty_tree_is_refused_and_left_as_it_is() {
    let world = world("dirty");
    let story = world.repo.join(STORY_FILE);
    let dirty = fs::read_to_string(&story)
        .expect("the story file")
        .replace("status: proposed", "status: active");
    fs::write(&story, &dirty).expect("the story file is changed");
    let head = git_out(&world.repo, &["rev-parse", "HEAD"]);

    let (code, out, err) = world.scenario_run("1", &[]);
    assert_eq!(code, 1, "stdout {out} stderr {err}");
    assert!(err.contains(STORY_FILE), "stderr {err}");
    assert!(err.contains("uncommitted"), "stderr {err}");
    assert_eq!(fs::read_to_string(&story).expect("the story file"), dirty);
    assert_eq!(git_out(&world.repo, &["rev-parse", "HEAD"]), head);
    assert_eq!(git_out(&world.repo, &["branch", "--show-current"]), "main");
    assert!(world.trial_branches().is_empty());
    assert!(world.calls().is_empty(), "{:?}", world.calls());
    assert!(!world.repo.join(".takt/cases").exists());
}

/// A push the bot route refuses ends the run before any case is opened: the trial branch it
/// cut is removed again and the tree is back where it was.
#[test]
fn a_refused_push_leaves_no_trial_branch_and_the_tree_where_it_was() {
    let world = world("refused-push");
    fs::write(world.state.join("refuse-push"), "").expect("the double refuses");
    let head = git_out(&world.repo, &["rev-parse", "HEAD"]);

    let (code, out, err) = world.scenario_run("1", &[]);
    assert_eq!(code, 1, "stdout {out} stderr {err}");
    assert!(err.contains("trial/0"), "stderr {err}");
    assert!(err.contains("refused by the double"), "stderr {err}");
    assert!(world.trial_branches().is_empty());
    assert_eq!(git_out(&world.repo, &["branch", "--show-current"]), "main");
    assert_eq!(git_out(&world.repo, &["rev-parse", "HEAD"]), head);
    assert_eq!(world.changed_outside_cases(), "");
    assert!(!world.repo.join(".takt/cases").exists());
}

/// The next trial number follows the highest one the checkout knows, a local branch or one its
/// remote has: a number is never used twice.
#[test]
fn the_next_trial_follows_the_highest_trial_branch_local_or_remote() {
    let world = world("numbering");
    git(&world.repo, &["branch", "trial/2", &world.seed]);
    git(
        &world.repo,
        &["update-ref", "refs/remotes/origin/trial/6", &world.seed],
    );
    git(&world.repo, &["branch", "trial/not-a-number", &world.seed]);

    let (code, out, err) = world.scenario_run("1", &["--iterations", "0"]);
    assert_eq!(code, 3, "stdout {out} stderr {err}");
    assert!(
        out.contains("scenario 1: story:ready-one proposed on trial/7 at "),
        "stdout {out} stderr {err}"
    );
    assert_eq!(git_out(&world.repo, &["rev-parse", "trial/7"]), world.seed);
    assert_eq!(
        git_out(&world.repo, &["branch", "--show-current"]),
        "takt/scenario-1-trial-7"
    );
}

/// A run through the bot route in a checkout that holds a trial number a run without it took,
/// 22 digits (a 14-digit instant and an 8-digit nonce, past `u64`), takes that number plus one:
/// counted, not refused as too large.
#[test]
fn the_next_trial_follows_a_22_digit_trial_number() {
    let world = world("numbering-22");
    git(
        &world.repo,
        &["branch", "trial/2026101914253012345678", &world.seed],
    );

    let (code, out, err) = world.scenario_run("1", &["--iterations", "0"]);
    assert_eq!(code, 3, "stdout {out} stderr {err}");
    assert!(
        out.contains("scenario 1: story:ready-one proposed on trial/2026101914253012345679 at "),
        "stdout {out} stderr {err}"
    );
    assert_eq!(
        world.pushed("trial/2026101914253012345679").as_deref(),
        Some(world.seed.as_str())
    );
    assert_eq!(
        git_out(&world.repo, &["branch", "--show-current"]),
        "takt/scenario-1-trial-2026101914253012345679"
    );
}

/// The top of the range a trial number is counted in, `u128` since a trial number without the
/// bot route is 22 digits: a trial branch at the top has no number after it, and one past the
/// top does not parse. Each is refused in one line naming it, after the fetch and before
/// anything is cut or pushed: never wrapped to a number in use, never a panic.
#[test]
fn a_trial_number_at_the_top_of_u128_is_refused_not_wrapped() {
    let world = world("numbering-top");
    for (number, says) in [
        (
            "340282366920938463463374607431768211456",
            "numbers a trial past the largest a scenario run counts to",
        ),
        (
            "340282366920938463463374607431768211455",
            "is the highest trial branch known, and no number follows it",
        ),
    ] {
        let branch = format!("trial/{number}");
        git(&world.repo, &["branch", &branch, &world.seed]);
        let (code, out, err) = world.scenario_run("1", &["--iterations", "0"]);
        assert!(
            !err.contains("panicked"),
            "takt panicked: exit {code}, stderr {err}"
        );
        assert_eq!(code, 1, "{number}: stdout {out} stderr {err}");
        assert!(err.contains(number), "{number}: stderr {err}");
        assert!(err.contains(says), "{number}: stderr {err}");
        assert_eq!(
            world.trial_branches(),
            std::slice::from_ref(&branch),
            "{number}"
        );
        assert!(
            !world
                .calls()
                .iter()
                .any(|call| call.starts_with("bot -- push")),
            "{:?}",
            world.calls()
        );
        git(&world.repo, &["branch", "-D", &branch]);
    }
}

/// A scenario the declaration does not name is refused before anything is changed, naming
/// where it was looked for.
#[test]
fn an_undeclared_scenario_is_refused_before_anything_changes() {
    let world = world("undeclared");
    let (code, out, err) = world.scenario_run("2", &[]);
    assert_eq!(code, 1, "stdout {out} stderr {err}");
    assert!(err.contains("scenario 2"), "stderr {err}");
    assert!(err.contains(SCENARIOS), "stderr {err}");
    assert!(world.trial_branches().is_empty());
    assert!(world.calls().is_empty(), "{:?}", world.calls());
}

/// A declaration with a key takt does not read is refused: a misspelt `seed` is never a
/// scenario without one.
#[test]
fn a_declaration_with_an_unknown_key_is_refused() {
    let world = world("unknown-key");
    declare(
        &world.repo,
        &format!(
            "[scenario \"1\"]\n\tstory = story:ready-one\n\tsead = {}\n",
            world.seed
        ),
    );
    let (code, out, err) = world.scenario_run("1", &[]);
    assert_eq!(code, 1, "stdout {out} stderr {err}");
    assert!(err.contains("sead"), "stderr {err}");
    assert!(world.trial_branches().is_empty());
    assert!(world.calls().is_empty(), "{:?}", world.calls());
}

/// A seed whose store does not show the scenario's story `proposed` is no known starting state:
/// refused once the trial branch is checked out, before it is pushed; the branch is removed
/// and the tree is back where it was.
#[test]
fn a_seed_whose_story_is_not_proposed_is_refused_before_the_push() {
    let world = world("not-proposed");
    // The story taken on the scratch repository's store, as a run before would have.
    let moved = Command::new("aep")
        .args([
            "plan",
            "artifact",
            "move",
            "story:ready-one",
            "--to",
            "active",
            "--executor",
            "agent:test",
        ])
        .current_dir(&world.repo)
        .output()
        .expect("aep runs");
    assert!(
        moved.status.success(),
        "aep plan artifact move: {}",
        String::from_utf8_lossy(&moved.stderr)
    );
    git(&world.repo, &["add", "-A", ".engineering/planning"]);
    git(&world.repo, &["commit", "-q", "-m", "the story taken"]);
    assert_eq!(world.status_at("HEAD"), "active");
    let taken = git_out(&world.repo, &["rev-parse", "HEAD"]);
    declare(
        &world.repo,
        &format!("[scenario \"1\"]\n\tstory = story:ready-one\n\tseed = {taken}\n"),
    );
    let head = git_out(&world.repo, &["rev-parse", "HEAD"]);

    let (code, out, err) = world.scenario_run("1", &[]);
    assert_eq!(code, 1, "stdout {out} stderr {err}");
    assert!(err.contains("story:ready-one"), "stderr {err}");
    assert!(err.contains("active"), "stderr {err}");
    assert!(world.trial_branches().is_empty());
    assert_eq!(world.calls(), [FETCH_TRIALS], "nothing but the fetch");
    assert!(git_out(&world.remote, &["for-each-ref"]).is_empty());
    assert_eq!(git_out(&world.repo, &["branch", "--show-current"]), "main");
    assert_eq!(git_out(&world.repo, &["rev-parse", "HEAD"]), head);
}

/// The line that fetches the remote's trial branches through the bot route: from the remote's
/// HTTPS URL, whatever form `origin` has, so it goes under the bot route's HTTPS credential
/// and never needs the user's SSH agent.
const FETCH_TRIALS: &str = "bot -- fetch https://github.com/owner/repo.git +refs/heads/trial/*:refs/remotes/origin/trial/*";

/// The story's acceptance, the route half: in a clone whose `origin` is an SSH URL, with the
/// user's SSH agent loaded and a prompt allowed in takt's own environment, the trial fetch
/// names the remote's HTTPS URL, and its child gets no agent socket, no git prompt, SSH in
/// batch mode, standard input from `/dev/null`, an empty `GIT_ASKPASS` and HTTPS as the one
/// transport git may use; the run proceeds to cut `trial/0`. The push of the case's branch by
/// `pr.open` is held to HTTPS as the fetch is; the push that creates `trial/0` is not, yet.
#[test]
fn the_trial_fetch_goes_over_https_with_no_user_credential_and_no_prompt() {
    let world = world("https-fetch");
    let agent = world.state.join("agent.sock");
    let agent = agent.to_str().expect("utf-8");
    let repo = world.repo.to_str().expect("utf-8");
    let (code, out, err) = world.takt_env(
        &["scenario", "run", "--repo", repo, "1", "--test-cmd", "true"],
        &[
            ("SSH_AUTH_SOCK", agent),
            ("GIT_TERMINAL_PROMPT", "1"),
            ("GIT_SSH_COMMAND", "ssh"),
            ("GIT_ASKPASS", "/user/askpass"),
        ],
    );
    let pushed = fs::read_to_string(world.state.join("push-env")).expect("a push ran");
    let (case, trial): (Vec<&str>, Vec<&str>) = pushed
        .lines()
        .partition(|line| line.starts_with("takt/") || line.contains(":refs/heads/takt/"));
    assert!(
        !case.is_empty()
            && case
                .iter()
                .all(|line| line.ends_with(" GIT_ALLOW_PROTOCOL=https")),
        "the case's branch pushed over another transport than HTTPS: {pushed}"
    );
    assert!(
        !trial.is_empty()
            && trial
                .iter()
                .all(|line| line.ends_with(" GIT_ALLOW_PROTOCOL=unset")),
        "the trial push: {pushed}"
    );
    let seen = fs::read_to_string(world.state.join("fetch-env")).expect("the fetch ran");
    assert_eq!(
        (
            world.calls().first().cloned(),
            seen.lines().collect::<Vec<_>>()
        ),
        (
            Some(FETCH_TRIALS.to_owned()),
            vec![
                "GIT_TERMINAL_PROMPT=0",
                "GIT_SSH_COMMAND=ssh -o BatchMode=yes",
                "SSH_AUTH_SOCK=unset",
                "stdin=/dev/null",
                "GIT_ASKPASS=",
                "GIT_ALLOW_PROTOCOL=https",
            ]
        ),
        "the fetch line and what its child was started with"
    );
    assert_eq!(code, 4, "stdout {out} stderr {err}");
    assert!(
        out.contains("scenario 1: story:ready-one proposed on trial/0 at "),
        "stdout {out} stderr {err}"
    );
}

/// `origin` in each form git writes for a repository on github.com, SSH or HTTPS, with or
/// without `.git`: the trial fetch names the one HTTPS URL. A fetch that cannot authenticate
/// fails at once, the refusal naming that URL and git's reason, with nothing changed.
#[test]
fn every_origin_form_fetches_from_the_one_https_url_and_a_refusal_changes_nothing() {
    let world = world("origin-forms");
    fs::write(world.state.join("refuse-fetch"), "").expect("the double refuses the fetch");
    let head = git_out(&world.repo, &["rev-parse", "HEAD"]);
    for origin in [
        "git@github.com:owner/repo.git",
        "ssh://git@github.com/owner/repo.git",
        "https://github.com/owner/repo.git",
        "https://github.com/owner/repo",
    ] {
        git(&world.repo, &["remote", "set-url", "origin", origin]);
        let _ = fs::remove_file(world.state.join("calls"));
        let (code, out, err) = world.scenario_run("1", &[]);
        assert_eq!(code, 1, "origin {origin}: stdout {out} stderr {err}");
        assert_eq!(world.calls(), [FETCH_TRIALS], "origin {origin}");
        assert!(
            err.contains("fetch the remote's trial branches; nothing was changed")
                && err.contains("https://github.com/owner/repo.git")
                && err.contains("Please make sure you have the correct access rights"),
            "origin {origin}: stderr {err}"
        );
        assert!(world.trial_branches().is_empty(), "origin {origin}");
        assert_eq!(git_out(&world.repo, &["branch", "--show-current"]), "main");
        assert_eq!(git_out(&world.repo, &["rev-parse", "HEAD"]), head);
        assert!(!world.repo.join(".takt/cases").exists());
    }
}

/// A fetch that does not end, standing in for one waiting on a prompt no one answers, is
/// killed within 30 s, not the forge's five minutes; the refusal names the remote and that it
/// timed out, and nothing is changed.
#[test]
fn a_trial_fetch_that_does_not_end_is_killed_within_30_s() {
    let world = world("hung-fetch");
    fs::write(world.state.join("hang-fetch"), "").expect("the double hangs the fetch");
    let head = git_out(&world.repo, &["rev-parse", "HEAD"]);
    let started = std::time::Instant::now();
    let (code, out, err) = world.scenario_run("1", &[]);
    let took = started.elapsed();
    assert!(
        took < std::time::Duration::from_secs(35),
        "the fetch took {took:?}: stdout {out} stderr {err}"
    );
    assert_eq!(code, 1, "stdout {out} stderr {err}");
    assert!(
        err.contains("https://github.com/owner/repo.git") && err.contains("timed out after 30.0 s"),
        "stderr {err}"
    );
    assert_eq!(world.calls(), [FETCH_TRIALS]);
    assert!(world.trial_branches().is_empty());
    assert_eq!(git_out(&world.repo, &["branch", "--show-current"]), "main");
    assert_eq!(git_out(&world.repo, &["rev-parse", "HEAD"]), head);
    assert!(!world.repo.join(".takt/cases").exists());
}

/// The remote already holds `trial/0`, pushed by a run from another checkout that this one
/// never fetched: the run fetches the remote's trial branches first and takes `trial/1`, its
/// own, and its pull request targets it.
#[test]
fn a_trial_branch_only_the_remote_holds_is_fetched_and_not_reused() {
    let world = world("remote-only");
    git(
        &world.repo,
        &[
            "push",
            "-q",
            world.remote.to_str().expect("utf-8"),
            &format!("{}:refs/heads/trial/0", world.seed),
        ],
    );
    let (code, out, err) = world.scenario_run("1", &[]);
    assert_eq!(code, 4, "stdout {out} stderr {err}");
    assert!(
        out.contains("scenario 1: story:ready-one proposed on trial/1 at "),
        "stdout {out} stderr {err}"
    );
    assert_eq!(world.request(1, "base"), "trial/1");
    assert_eq!(
        git_out(&world.repo, &["rev-parse", "refs/remotes/origin/trial/0"]),
        world.seed
    );
    assert_eq!(world.trial_branches(), ["trial/1"]);
}

/// The fetch alone is not relied on: when the remote holds the trial branch the checkout chose
/// (here the fetch brought nothing), the push reports it up to date rather than new, and the
/// run is refused before any case opens; the branch it cut is removed, the tree back on `main`,
/// the remote's branch as it was.
#[test]
fn a_push_that_finds_the_trial_branch_already_there_is_refused() {
    let world = world("up-to-date");
    git(
        &world.repo,
        &[
            "push",
            "-q",
            world.remote.to_str().expect("utf-8"),
            &format!("{}:refs/heads/trial/0", world.seed),
        ],
    );
    fs::write(world.state.join("no-fetch"), "").expect("the double fetches nothing");
    let head = git_out(&world.repo, &["rev-parse", "HEAD"]);

    let (code, out, err) = world.scenario_run("1", &[]);
    assert_eq!(code, 1, "stdout {out} stderr {err}");
    assert!(err.contains("trial/0"), "stderr {err}");
    assert!(err.contains("not created"), "stderr {err}");
    assert!(!out.contains("on trial/0 at"), "stdout {out}");
    assert!(world.trial_branches().is_empty());
    assert_eq!(git_out(&world.repo, &["branch", "--show-current"]), "main");
    assert_eq!(git_out(&world.repo, &["rev-parse", "HEAD"]), head);
    assert!(!world.repo.join(".takt/cases").exists());
    assert_eq!(
        git_out(&world.remote, &["rev-parse", "refs/heads/trial/0"]),
        world.seed
    );
}

/// The push may only create the trial branch. Another run's `trial/0` lands on the remote
/// after this run's fetch (here the fetch brings nothing) at an ancestor of this run's seed: an
/// unforced push would fast-forward it under that run's pull request. The lease
/// (`--force-with-lease=refs/heads/trial/0:`, the branch must not exist) makes the push
/// rejected instead; the run is refused, the remote's `trial/0` stays where the other run put
/// it, and the refusal says so.
#[test]
fn a_trial_branch_at_an_ancestor_of_the_seed_is_not_moved_by_the_push() {
    let world = world("lease");
    let later = git_out(&world.repo, &["rev-parse", "HEAD"]);
    declare(
        &world.repo,
        &format!("[scenario \"2\"]\n\tstory = story:ready-one\n\tseed = {later}\n"),
    );
    git(
        &world.repo,
        &[
            "push",
            "-q",
            world.remote.to_str().expect("utf-8"),
            &format!("{}:refs/heads/trial/0", world.seed),
        ],
    );
    fs::write(world.state.join("no-fetch"), "").expect("the double fetches nothing");

    let (code, out, err) = world.scenario_run("2", &["--iterations", "0"]);
    assert_eq!(
        git_out(&world.remote, &["rev-parse", "refs/heads/trial/0"]),
        world.seed,
        "the remote's trial/0 was moved: exit {code}, stdout {out} stderr {err}"
    );
    assert_eq!(code, 1, "stdout {out} stderr {err}");
    assert!(err.contains("not created"), "stderr {err}");
    assert!(err.contains("rejected"), "stderr {err}");
    assert!(world.trial_branches().is_empty());
    assert_eq!(git_out(&world.repo, &["branch", "--show-current"]), "main");
    assert!(!world.repo.join(".takt/cases").exists());
}

/// A seed whose store shows the scenario's story `proposed` but not ready, here because it
/// depends on a story still in draft, is no known starting state either: refused before the
/// push, naming what it waits on, as `takt run` would never take it.
#[test]
fn a_seed_whose_story_waits_on_a_dependency_is_refused_before_the_push() {
    let world = world("waits");
    declare(
        &world.repo,
        &format!(
            "[scenario \"4\"]\n\tstory = story:waits-one\n\tseed = {}\n",
            world.seed
        ),
    );
    let (code, out, err) = world.scenario_run("4", &[]);
    assert_eq!(code, 1, "stdout {out} stderr {err}");
    assert!(err.contains("story:waits-one"), "stderr {err}");
    assert!(err.contains("story:draft-dep"), "stderr {err}");
    assert!(world.trial_branches().is_empty());
    assert!(
        !world
            .calls()
            .iter()
            .any(|call| call.starts_with("bot -- push")),
        "{:?}",
        world.calls()
    );
    assert!(git_out(&world.remote, &["for-each-ref"]).is_empty());
    assert_eq!(git_out(&world.repo, &["branch", "--show-current"]), "main");
}

/// The revision of takt the five runs below are saved under: their `opened` lines are made to
/// name it, as a clean build of one revision would write them, so the save's binary check
/// holds whether or not the build under test is `-dirty`. Which binary made the runs is not
/// what the case below is about.
const SAVED_UNDER: &str = "0123456789abcdef0123456789abcdef01234567";

/// `events` with every `opened` line's `binary` naming `revision`.
fn opened_by(events: &str, revision: &str) -> String {
    let key = "\"binary\":\"";
    events
        .lines()
        .map(|line| {
            let Some(at) = line
                .contains("\"event\":\"opened\"")
                .then(|| line.find(key))
                .flatten()
            else {
                return format!("{line}\n");
            };
            let value = at + key.len();
            let end = value + line[value..].find('"').expect("the binary's closing quote");
            format!("{}{revision}{}\n", &line[..value], &line[end..])
        })
        .collect()
}

/// How the five runs of [`five_clones_one_scorecard`] are started.
#[derive(Clone, Copy, PartialEq)]
enum Started {
    /// One after another, each in this process's PID namespace.
    InTurn,
    /// All at once, each in a PID namespace of its own (`bwrap --unshare-pid`), as containers
    /// or confined runners start them: every run has the same process id and host name. macOS
    /// has no PID namespace, so there the five start at once in this one, with one host name.
    AtOnceInOwnPidNamespaces,
}

/// Five scenario runs of scenario 1, each from its own fresh clone of one origin and without
/// the bot route (the origin is a path, no forge), started as `started` says; asserts five
/// distinct trial numbers, all digits, and five distinct case ids, and that, gathered into one
/// repository, `takt scorecard --save` takes all five cases.
fn five_clones_one_scorecard(name: &str, started: Started) {
    let outer = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("scenario-run-{}-{name}", std::process::id()));
    if outer.exists() {
        fs::remove_dir_all(&outer).expect("a stale scratch tree is removed");
    }
    let scratch = Scratch(outer);
    let source = scratch.0.join("source");
    copy_dir(&Path::new(FIXTURES).join("one-ready"), &source);
    fs::write(source.join("Cargo.toml"), "[package]\nname = \"scratch\"\n").expect("a file");
    fs::write(source.join(".engineering/gates.yaml"), GATES).expect("a file");
    git(&source, &["init", "-q", "-b", "main"]);
    git(&source, &["add", "-A"]);
    git(&source, &["commit", "-q", "-m", "seed"]);
    let seed = git_out(&source, &["rev-parse", "HEAD"]);
    declare(
        &source,
        &format!("[scenario \"1\"]\n\tstory = story:ready-one\n\tseed = {seed}\n"),
    );
    git(
        &scratch.0,
        &["init", "-q", "-b", "main", "--bare", "origin.git"],
    );
    git(&source, &["push", "-q", "../origin.git", "main"]);

    let clones: Vec<PathBuf> = (1..=5)
        .map(|k| {
            let name = format!("clone-{k}");
            git(&scratch.0, &["clone", "-q", "origin.git", &name]);
            scratch.0.join(&name)
        })
        .collect();
    let command = |clone: &Path| {
        let mut command = match started {
            Started::InTurn => Command::new(env!("CARGO_BIN_EXE_takt")),
            #[cfg(target_os = "macos")]
            Started::AtOnceInOwnPidNamespaces => Command::new(env!("CARGO_BIN_EXE_takt")),
            #[cfg(not(target_os = "macos"))]
            Started::AtOnceInOwnPidNamespaces => {
                let mut bwrap = Command::new("/usr/bin/bwrap");
                bwrap.args([
                    "--dev-bind",
                    "/",
                    "/",
                    "--unshare-pid",
                    "--die-with-parent",
                    env!("CARGO_BIN_EXE_takt"),
                ]);
                bwrap
            }
        };
        command
            .args(["scenario", "run", "--repo"])
            .arg(clone)
            .args([
                "1",
                "--harness",
                "none",
                "--iterations",
                "1",
                "--test-cmd",
                "true",
            ])
            .env_remove("B10X_GATES_POLICY");
        command
    };
    let outputs: Vec<std::process::Output> = match started {
        Started::InTurn => clones
            .iter()
            .map(|clone| command(clone).output().expect("takt runs"))
            .collect(),
        Started::AtOnceInOwnPidNamespaces => {
            let children: Vec<std::process::Child> = clones
                .iter()
                .map(|clone| {
                    command(clone)
                        .stdout(std::process::Stdio::piped())
                        .stderr(std::process::Stdio::piped())
                        .spawn()
                        .unwrap_or_else(|error| {
                            panic!(
                                "bwrap does not start ({error}): this case needs bubblewrap on the program path, as CI installs it"
                            )
                        })
                })
                .collect();
            children
                .into_iter()
                .map(|child| child.wait_with_output().expect("takt runs"))
                .collect()
        }
    };

    let mut trials: Vec<String> = Vec::new();
    let mut cases: Vec<PathBuf> = Vec::new();
    for (k, (clone, output)) in clones.iter().zip(&outputs).enumerate() {
        let name = format!("clone-{}", k + 1);
        let out = String::from_utf8_lossy(&output.stdout);
        let err = String::from_utf8_lossy(&output.stderr);
        assert!(
            out.contains("not pushed: no forge"),
            "{name}: exit {:?}, stdout {out} stderr {err}",
            output.status.code()
        );
        let trial = out
            .lines()
            .find_map(|line| {
                line.strip_prefix("scenario 1: story:ready-one proposed on trial/")?
                    .split_once(' ')
                    .map(|(n, _)| n.to_owned())
            })
            .unwrap_or_else(|| panic!("{name} names its trial: stdout {out} stderr {err}"));
        assert!(
            !trial.is_empty() && trial.bytes().all(|b| b.is_ascii_digit()),
            "{name}: trial/{trial} is not all digits"
        );
        let case = clone
            .join(".takt/cases")
            .join(format!("scenario-1-trial-{trial}"));
        assert!(
            case.join("events.jsonl").is_file(),
            "{name}: no record at {}: stdout {out} stderr {err}",
            case.display()
        );
        trials.push(trial);
        cases.push(case);
    }
    let mut distinct = trials.clone();
    distinct.sort();
    distinct.dedup();
    assert_eq!(
        distinct.len(),
        5,
        "five fresh clones share a trial number: {trials:?}"
    );

    let scores = scratch.0.join("scores");
    for case in &cases {
        let id = case.file_name().expect("a case id");
        let into = scores.join(".takt/cases").join(id);
        assert!(!into.exists(), "{} twice", into.display());
        copy_dir(case, &into);
        let events = fs::read_to_string(into.join("events.jsonl")).expect("the record");
        fs::write(into.join("events.jsonl"), opened_by(&events, SAVED_UNDER)).expect("written");
        fs::write(into.join("held-out.txt"), "PASS\n").expect("the held-out result");
    }
    let saves = scratch.0.join("saves");
    fs::create_dir_all(&saves).expect("the saves directory");
    let output = Command::new(env!("CARGO_BIN_EXE_takt"))
        .args(["scorecard", "--repo"])
        .arg(&scores)
        .arg("--save")
        .arg(&saves)
        .args(["--revision", SAVED_UNDER])
        .output()
        .expect("takt runs");
    let out = String::from_utf8_lossy(&output.stdout);
    let err = String::from_utf8_lossy(&output.stderr);
    assert_eq!(output.status.code(), Some(0), "stdout {out} stderr {err}");
    let saved = fs::read_to_string(saves.join(format!("scorecard-{SAVED_UNDER}.json")))
        .expect("the saved scorecard");
    for trial in &trials {
        assert!(
            saved.contains(&format!("\"scenario-1-trial-{trial}\"")),
            "the saved scorecard lacks trial {trial}: {saved}"
        );
    }
}

/// story:trial-numbers-are-unique-across-checkouts: five scenario runs, one after another, each
/// from its own fresh clone of one origin and without the bot route, get five distinct trial
/// numbers and case ids, and one scorecard takes all five.
#[test]
fn five_runs_from_five_fresh_clones_get_five_trial_numbers_and_one_scorecard_takes_all() {
    five_clones_one_scorecard("five-clones", Started::InTurn);
}

/// The same five runs started at once, each in a PID namespace of its own (`bwrap
/// --unshare-pid`), where every run has the same process id and host name: the nonce does not
/// rest on either, so they still get five distinct trial numbers. Two runs in one second share
/// a number with a chance of 1 in 10^8 per pair (about 1 in 10^7 for five). Bubblewrap is
/// required on Linux, never skipped: without it the case fails, naming it. macOS has no PID
/// namespace: there the five start at once in its one, with one host name and distinct process
/// ids, and get five distinct trial numbers.
#[test]
fn five_runs_started_at_once_in_their_own_pid_namespaces_get_five_trial_numbers() {
    five_clones_one_scorecard("five-clones-pid-ns", Started::AtOnceInOwnPidNamespaces);
}
