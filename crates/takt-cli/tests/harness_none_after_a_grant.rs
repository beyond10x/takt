//! story:harness-none-takes-a-granted-action: `takt run --harness none` after a grant. Resumed
//! with a granted action admissible, the harness takes it instead of ending the run on "no
//! harness: a judgement step among ... is left"; with no grant it behaves as it always has. The
//! forge is the bot route's double on the program path (`forge_double`), so nothing reaches a
//! network.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

/// The fixture stores of the library crate, one directory each.
const FIXTURES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../takt/tests/fixtures");

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
        .join(format!("harness-none-grant-{}-{name}", std::process::id()));
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
/// `b10x-gates bot -- push <remote> <branch>` keeps the branch and the commit pushed,
/// `b10x-gates bot -- fetch` reports the local branch as the repository's, `scan-text` passes,
/// `api --method POST` opens pull request #7 and `api --method PUT` merges it and counts the
/// merge; `gh pr view` reports the pull request's state with the pushed head, `gh pr checks`
/// one check, passing. Nothing reaches a network.
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

/// A repository whose origin is a github.com repository, so the default forge is the bot route
/// the double answers for, and the story's first run: the case is open, its branch checked out.
/// Returns the repository, the double's directory, the case id and the path of the double's bin.
fn delivery_at_the_gate(name: &str) -> (Scratch, Scratch, String, PathBuf) {
    let repo = repository("one-ready", name);
    git(
        &repo,
        &["remote", "add", "origin", "git@github.com:owner/repo.git"],
    );
    let outer = PathBuf::from(format!("{}-forge", repo.display()));
    if outer.exists() {
        fs::remove_dir_all(&outer).expect("a stale double is removed");
    }
    let double = Scratch(outer.clone());
    let (bin, _) = forge_double(&outer);
    let path = repo.to_str().expect("utf-8");
    let (code, out, err) = takt_through(
        &bin,
        &[
            "run",
            "--repo",
            path,
            "--harness",
            "none",
            "--test-cmd",
            "true",
            "--protocol",
            "story.delivery/1",
            "--iterations",
            "0",
        ],
    );
    assert_eq!(code, 3, "stdout {out} stderr {err}");
    assert!(
        out.contains("frontier: took story:ready-one"),
        "stdout {out} stderr {err}"
    );
    assert!(out.contains("forge: bot (owner/repo)"), "{out}");
    let id = git_out(&repo, &["branch", "--show-current"])
        .strip_prefix("takt/")
        .expect("the tree is on the case's branch")
        .to_owned();
    fs::write(repo.join("feature.txt"), "the work\n").expect("a file");
    git(&repo, &["add", "feature.txt"]);
    git(&repo, &["commit", "-q", "-m", "the work"]);
    (repo, double, id, bin)
}

/// The plain resumed run every case here makes: no intent, the harness none, the bot route's
/// double for the forge.
fn resume_args(repo: &Path) -> Vec<&str> {
    vec![
        "run",
        "--repo",
        repo.to_str().expect("utf-8"),
        "--harness",
        "none",
        "--test-cmd",
        "true",
        "--protocol",
        "story.delivery/1",
    ]
}

/// scenario `harness-none/takes-the-granted-merge`: a case granted `repository.merge` at its
/// revision, resumed with `--harness none` and a forge double that merges, takes
/// `repository.merge` and stops `Completed`, exit 0. The merge is taken and performed by the
/// forge's merge validator, which the forge registers and which `--harness none` never has to
/// offer; the run leaves nothing to the harness and names no missing step.
#[test]
fn harness_none_takes_the_granted_merge() {
    let (repo, double, id, bin) = delivery_at_the_gate("granted-merge");
    let path = repo.to_str().expect("utf-8");
    let resume = resume_args(&repo);
    let (code, out, err) = takt_through(&bin, &resume);
    assert_eq!(code, 4, "stdout {out} stderr {err}");
    assert!(
        out.contains("stopped: ApprovalRequired (repository.merge)"),
        "{out}"
    );
    assert!(
        !double.join("forge/merges").exists(),
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

    let (code, out, err) = takt_through(&bin, &resume);
    assert_eq!(code, 0, "stdout {out} stderr {err}");
    assert!(
        out.contains(&format!(
            "frontier: resumed story:ready-one: Make the check pass (case {id})"
        )),
        "{out}"
    );
    assert!(!out.contains("refused"), "stdout {out} stderr {err}");
    assert!(
        !out.contains("no harness:"),
        "the granted merge is taken, not left to the harness: stdout {out} stderr {err}"
    );
    assert!(
        out.contains("evidence: merge_observation merged"),
        "stdout {out} stderr {err}"
    );
    assert!(out.contains("stopped: Completed"), "{out}");
    assert_eq!(
        fs::read_to_string(double.join("forge/merges")).expect("the double merged"),
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

/// scenario `harness-none/no-grant-unchanged`: with no grant, `--harness none` behaves as it did
/// before a grant existed. At the gate it requests the pending approval, `ApprovalRequired`,
/// exit 4, and merges nothing; it writes no authority file; and a run that leaves a judgement
/// step with nothing to take ends `stopped: error:`, exit 5, naming the steps left.
#[test]
fn harness_none_no_grant_unchanged() {
    let (repo, double, id, bin) = delivery_at_the_gate("no-grant");
    let resume = resume_args(&repo);
    let (code, out, err) = takt_through(&bin, &resume);
    assert_eq!(
        code, 4,
        "the pending approval is requested, not an error: stdout {out} stderr {err}"
    );
    assert!(
        out.contains("stopped: ApprovalRequired (repository.merge)"),
        "{out}"
    );
    assert!(
        !out.contains("no harness: a judgement step"),
        "the approval is requested, not left: {out}"
    );
    assert!(
        !double.join("forge/merges").exists(),
        "nothing merged without a grant"
    );
    assert!(
        !repo
            .join(takt::case::CASES_DIR)
            .join(&id)
            .join("authority.yaml")
            .exists(),
        "no grant was written"
    );

    // A case with no forge and no grant: the work committed by hand is handed over as it stands,
    // and the judgement steps left, with nothing to take them, end the run on an error.
    let plain = repository("one-ready", "no-grant-plain");
    let plain_path = plain.to_str().expect("utf-8");
    let (code, out, err) = takt(&[
        "run",
        "--repo",
        plain_path,
        "--harness",
        "none",
        "--forge",
        "none",
        "--test-cmd",
        "true",
        "--protocol",
        "story.delivery/1",
        "--iterations",
        "0",
    ]);
    assert_eq!(code, 3, "stdout {out} stderr {err}");
    let (code, out, err) = takt(&[
        "run",
        "--repo",
        plain_path,
        "--harness",
        "none",
        "--forge",
        "none",
        "--test-cmd",
        "true",
        "--protocol",
        "story.delivery/1",
    ]);
    assert!(
        out.contains("change.handover (none)"),
        "the choice is taken as it always was: {out}"
    );
    assert_eq!(code, 5, "stdout {out} stderr {err}");
    assert!(out.contains("stopped: error: step "), "{out}");
    assert!(
        out.contains("no harness: a judgement step among implementation.edit, tests.write is left"),
        "the steps left are named: {out}"
    );
}
