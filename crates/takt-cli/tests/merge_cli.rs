//! The `takt` binary around the merge under authority, at its edges: the run after a story's
//! case completed, and a run with no harness on a case that holds everything but the grant.
//! Adversary pass 1 over story:merge-under-authority.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use takt::CaseDir;
use takt::evidence::Record;

/// The fixture stores of the library crate, one directory each.
const FIXTURES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../takt/tests/fixtures");

/// The fixture protocol of the library crate that declares `gate_result` and `gate.green`.
const GATE_PROTOCOL: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../takt/tests/fixtures/gate-only/1.yaml"
);

/// The story-delivery protocol, the engineering protocols' built-in.
const DELIVERY_PROTOCOL: &str = "story.delivery/1";

/// A gates document whose one assertion is true in a repository with a manifest.
const GATES: &str = "format: engineering-gates/1\nlanguage: canon-expr/1\ngates:\n  - id: manifest\n    \
                     text: The repository has its manifest.\n    assertions:\n      \
                     - 'fs.file.exists(\"Cargo.toml\")'\n";

fn git(tree: &Path, args: &[&str]) {
    let status = Command::new("git")
        .args(["-c", "user.name=t", "-c", "user.email=t@localhost"])
        .args(args)
        .current_dir(tree)
        .status()
        .expect("git runs");
    assert!(status.success(), "git {args:?}");
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

/// A committed copy of the fixture store `fixture` with a manifest and a gates document whose
/// one assertion is true, in a scratch tree of its own: `name` keeps the cases of this process
/// apart.
fn repository(fixture: &str, name: &str) -> Scratch {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("merge-cli-{}-{name}", std::process::id()));
    if dir.exists() {
        fs::remove_dir_all(&dir).expect("a stale scratch tree is removed");
    }
    copy_dir(&Path::new(FIXTURES).join(fixture), &dir);
    fs::write(dir.join("Cargo.toml"), "[package]\nname = \"scratch\"\n").expect("a file");
    fs::write(dir.join(".engineering/gates.yaml"), GATES).expect("a file");
    fs::write(
        dir.join(".engineering/commit-identity.conf"),
        "[identity]\n\tname = Test Bot\n\temail = bot@example.invalid\n",
    )
    .expect("the commit identity is written");
    git(&dir, &["init", "-q", "-b", "main"]);
    git(&dir, &["add", "-A"]);
    git(&dir, &["commit", "-q", "-m", "start"]);
    Scratch(dir)
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

/// `takt run --repo R` with no intent and no case, twice: the first takes the ready story and
/// completes its case through the gate; the second finds the story `active` with a finished
/// case and takes nothing else. A finished case is not an open case, so the second run does not
/// resume it: it says nothing is ready, or takes the next ready story.
#[test]
fn the_run_after_a_story_case_completed_does_not_resume_it() {
    let repo = repository("one-ready", "completed");
    let path = repo.to_str().expect("utf-8");
    let args = [
        "run",
        "--repo",
        path,
        "--harness",
        "none",
        "--protocol",
        GATE_PROTOCOL,
        "--test-cmd",
        "true",
    ];
    let (code, out, err) = takt(&args);
    assert_eq!(code, 0, "stdout {out} stderr {err}");
    assert!(
        out.contains("frontier: took story:ready-one"),
        "stdout {out} stderr {err}"
    );
    assert!(
        out.contains("stopped: Completed (accepted)"),
        "stdout {out} stderr {err}"
    );
    let (code, out, err) = takt(&args);
    assert!(
        !out.contains("frontier: resumed story:ready-one"),
        "the finished case is resumed by every later run on the repository, and no other story is ever taken: exit {code} stdout {out} stderr {err}"
    );
    assert!(
        out.contains("frontier: nothing ready"),
        "exit {code} stdout {out} stderr {err}"
    );
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

/// A double of the bot route's two programs in `<dir>/stub-bin`, its state in `<dir>/forge`:
/// `b10x-gates bot -- push <remote> <branch>` (or `<revision>:refs/heads/<branch>`) keeps the branch and the commit pushed,
/// `b10x-gates bot -- fetch <url> <refspec> --dry-run …` reports the local branch as the repository's,
/// `scan-text` passes, `api --method POST` opens pull request #7, `api --method PUT` merges it
/// and counts the merge; `gh pr view` reports the pull request's state with the pushed head,
/// `gh pr checks` one check, passing. Nothing reaches a network.
fn forge_double(dir: &Path) -> (PathBuf, PathBuf) {
    let bin = dir.join("stub-bin");
    let state = dir.join("forge");
    fs::create_dir_all(&bin).expect("a stub directory");
    fs::create_dir_all(&state).expect("a state directory");
    let state_text = state.display().to_string();
    executable(
        &bin,
        "b10x-gates",
        &format!(
            r#"#!/bin/sh
state='{state_text}'
case "$1" in
  bot)
    if [ "$3" = fetch ]; then
      printf '  %s %s %s\n' 0000000000000000000000000000000000000000 "$(git rev-parse "${{5%%:*}}")" "${{5#*:}}"
      exit 0
    fi
    spec="$5"; branch="${{spec#*:refs/heads/}}"; revision="${{spec%%:*}}"
    if [ "$branch" = "$spec" ]; then revision="refs/heads/$spec"; fi
    printf '%s' "$branch" > "$state/branch"
    git rev-parse "$revision" > "$state/pushed" || exit 1
    exit 0 ;;
  scan-text) exit 0 ;;
  api)
    method=''; output=''
    while [ $# -gt 0 ]; do
      case "$1" in
        --method) method="$2"; shift ;;
        --output) output="$2"; shift ;;
      esac
      shift
    done
    case "$method" in
      POST) printf 'OPEN' > "$state/state"; printf '{{"number": 7}}' > "$output"; exit 0 ;;
      PUT) printf 'MERGED' > "$state/state"; echo merged >> "$state/merges"; printf '{{"merged": true}}' > "$output"; exit 0 ;;
    esac ;;
esac
echo "b10x-gates double: $*" >&2
exit 2
"#
        ),
    );
    executable(
        &bin,
        "gh",
        &format!(
            r#"#!/bin/sh
state='{state_text}'
case "$2" in
  view) printf '{{"state":"%s","mergedAt":null,"headRefName":"%s","headRefOid":"%s","baseRefName":"main"}}' "$(cat "$state/state")" "$(cat "$state/branch")" "$(tr -d '\n' < "$state/pushed")"; exit 0 ;;
  checks) printf '[{{"bucket":"pass"}}]'; exit 0 ;;
esac
echo "gh double: $*" >&2
exit 2
"#
        ),
    );
    (bin, state)
}

/// Runs the built `takt` binary with `args`, `bin` first on the program path and no
/// `b10x-gates` policy: the exit code, stdout and stderr.
fn takt_through(bin: &Path, args: &[&str]) -> (i32, String, String) {
    let output = Command::new(env!("CARGO_BIN_EXE_takt"))
        .args(args)
        .env_remove("B10X_GATES_POLICY")
        .env(
            "PATH",
            format!(
                "{}:{}",
                bin.display(),
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

/// The whole delivery through the binary, on the default protocol for a store story and the
/// bot route as the default forge for a github.com origin, the route's programs a double: a
/// plain run takes the ready story; the operator commits the work on the case's branch; the
/// next plain run resumes the case, runs the tests and the gate, opens the pull request, reads
/// its checks and stops at the gate, exit 4; `takt approve` grants the merge, records it on the
/// story and commits that record on the case's branch; the next plain run resumes the case on a
/// clean tree at the revision the grant was given at and merges the pull request through the
/// forge, once, and the case completes, exit 0.
#[test]
fn approve_then_the_resumed_run_merges() {
    let repo = repository("one-ready", "approve-merge");
    git(
        &repo,
        &["remote", "add", "origin", "git@github.com:owner/repo.git"],
    );
    let outer = PathBuf::from(format!("{}-forge", repo.display()));
    if outer.exists() {
        fs::remove_dir_all(&outer).expect("a stale double is removed");
    }
    let _double = Scratch(outer.clone());
    let (bin, state) = forge_double(&outer);
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
        "story.delivery/1",
    ];
    let mut first = run.to_vec();
    first.extend(["--iterations", "0"]);
    let (code, out, err) = takt_through(&bin, &first);
    assert_eq!(code, 3, "stdout {out} stderr {err}");
    assert!(
        out.contains("frontier: took story:ready-one"),
        "stdout {out} stderr {err}"
    );
    assert!(out.contains(" on story.delivery/1 in "), "{out}");
    assert!(out.contains("forge: bot (owner/repo)"), "{out}");
    let id = git_out(&repo, &["branch", "--show-current"])
        .strip_prefix("takt/")
        .expect("the tree is on the case's branch")
        .to_owned();

    fs::write(repo.join("feature.txt"), "the work\n").expect("a file");
    git(&repo, &["add", "feature.txt"]);
    git(&repo, &["commit", "-q", "-m", "the work"]);
    let worked = git_out(&repo, &["rev-parse", "HEAD"]);

    let (code, out, err) = takt_through(&bin, &run);
    assert_eq!(code, 4, "stdout {out} stderr {err}");
    assert!(
        out.contains(&format!(
            "frontier: resumed story:ready-one: Make the check pass (case {id})"
        )),
        "{out}"
    );
    for line in [
        "evidence: test_result pass",
        "evidence: gate_result pass",
        "evidence: pull_request open",
        "evidence: ci_result pass",
        "stopped: ApprovalRequired (repository.merge)",
    ] {
        assert!(out.contains(line), "{line}: stdout {out} stderr {err}");
    }
    assert!(
        !state.join("merges").exists(),
        "nothing merged before the grant"
    );

    let (code, out, err) = takt_through(
        &bin,
        &[
            "approve",
            "--repo",
            path,
            &id,
            "repository.merge",
            "--by",
            "human:operator",
        ],
    );
    assert_eq!(code, 0, "stdout {out} stderr {err}");
    assert_eq!(
        git_out(&repo, &["log", "-1", "--format=%s"]),
        "takt: approve story:ready-one repository.merge"
    );
    assert_eq!(git_out(&repo, &["rev-parse", "HEAD^"]), worked);
    assert_eq!(
        git_out(
            &repo,
            &["status", "--porcelain", "--", ".", ":!.takt/cases"]
        ),
        ""
    );

    let (code, out, err) = takt_through(&bin, &run);
    assert_eq!(code, 0, "stdout {out} stderr {err}");
    assert!(
        out.contains(&format!(
            "frontier: resumed story:ready-one: Make the check pass (case {id})"
        )),
        "{out}"
    );
    assert!(!out.contains("refused"), "stdout {out} stderr {err}");
    assert!(
        out.contains(&format!(
            "merged pull request #7 at {} through bot: merged",
            &worked[..8]
        )),
        "stdout {out} stderr {err}"
    );
    assert!(
        out.contains("evidence: merge_observation merged"),
        "stdout {out} stderr {err}"
    );
    assert!(
        out.contains("stopped: Completed"),
        "stdout {out} stderr {err}"
    );
    assert_eq!(
        fs::read_to_string(state.join("merges")).expect("the double merged"),
        "merged\n",
        "merged once"
    );
    assert_eq!(
        git_out(
            &repo,
            &["status", "--porcelain", "--", ".", ":!.takt/cases"]
        ),
        ""
    );
}

/// A run with no harness on a case that holds everything but the grant: the four records are
/// true at the revision, `repository.merge` is approval-required, and nothing deterministic is
/// left. The story: the run stops at `ApprovalRequired (repository.merge)` with nothing merged,
/// exit 4, and `takt approve` is the next step.
#[test]
fn a_run_with_no_harness_on_a_case_that_needs_only_the_grant_stops_approval_required() {
    let repo = repository("one-ready", "no-harness");
    let path = repo.to_str().expect("utf-8");
    let (code, out, err) = takt(&[
        "run",
        "--repo",
        path,
        "--intent",
        "deliver the story",
        "--case-id",
        "gated",
        "--protocol",
        DELIVERY_PROTOCOL,
        "--harness",
        "none",
        "--forge",
        "none",
        "--test-cmd",
        "true",
        "--iterations",
        "0",
    ]);
    assert_eq!(code, 3, "stdout {out} stderr {err}");
    assert!(
        out.contains("stopped: Iterations (0)"),
        "stdout {out} stderr {err}"
    );
    // The records the delivery produces up to the merge, as the validators write them.
    let mut case = CaseDir::resume(&repo, "gated").expect("the case reads back");
    let revision = case
        .revision("implementation")
        .expect("implementation has a revision");
    for (n, (kind, result)) in [
        ("test_result", "pass"),
        ("gate_result", "pass"),
        ("pull_request", "open"),
        ("ci_result", "pass"),
    ]
    .iter()
    .enumerate()
    {
        case.add_evidence(&Record::new(
            format!("e{n}"),
            *kind,
            Some((*result).to_owned()),
            "implementation",
            &revision,
            "merge-cli-test",
        ))
        .expect("the record is written");
    }
    // The change handed over at the revision, as takt records a `change.handover` step.
    case.add_evidence(&takt::evidence::Record::new(
        "handover",
        "handover",
        None,
        "implementation",
        &revision,
        takt::run::CHOICE_PRODUCER,
    ))
    .expect("the record is written");
    // A change to publish at the revision, as `change.read` records it.
    case.add_evidence(&takt::evidence::Record::new(
        "change-record",
        "change_record",
        Some("ahead".to_owned()),
        "implementation",
        &revision,
        "takt.change-read",
    ))
    .expect("the record is written");
    let (code, out, err) = takt(&[
        "run",
        "--repo",
        path,
        "--case",
        "gated",
        "--harness",
        "none",
        "--forge",
        "none",
        "--test-cmd",
        "true",
    ]);
    assert_eq!(
        code, 4,
        "ApprovalRequired exits 4, not Completed's 0: stdout {out} stderr {err}"
    );
    assert!(
        out.contains("stopped: ApprovalRequired (repository.merge)"),
        "stdout {out} stderr {err}"
    );
}
