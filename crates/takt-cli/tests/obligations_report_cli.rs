//! story:the-run-shows-each-repository-obligation through the `takt` binary: a run whose gate
//! fails prints the obligations line before its stop line, and `takt report --case` prints the
//! table, in text and in `--format json`, with the same row.
//!
//! story:an-accepted-run-reports-the-tested-revisions-obligations through the binary: the
//! obligations are read at the case's implementation revision, not at takt's store commits above
//! it, so an accepted run whose gate passed there prints no obligations line, one whose gate
//! failed there still names it, a code commit after the gate reads it `not yet run`, and a stop at
//! the store commits does not hide the gate's table from a resume or from `takt report`.

#[path = "../../takt/tests/support/own_home.rs"]
mod own_home;

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

const GATE_PROTOCOL: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../takt/tests/fixtures/gate-only/1.yaml"
);

const GATES: &str = "format: engineering-gates/1\nlanguage: canon-expr/1\ngates:\n  - id: release-notes\n    \
                     text: A change records a release note in CHANGELOG.md.\n    assertions:\n      \
                     - 'git.changed(\"CHANGELOG.md\")'\n";

fn git(tree: &Path, args: &[&str]) {
    let status = Command::new("git")
        .args(["-c", "user.name=t", "-c", "user.email=t@localhost"])
        .args(args)
        .current_dir(tree)
        .status()
        .expect("git runs");
    assert!(status.success(), "git {args:?}");
}

struct Scratch(PathBuf);

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
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

#[test]
fn a_failed_obligation_is_in_the_run_line_and_in_the_report() {
    let repo = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("obligations-report-cli-{}", std::process::id()));
    let _ = fs::remove_dir_all(&repo);
    fs::create_dir_all(&repo).expect("a directory");
    let _scratch = Scratch(repo.clone());
    fs::write(repo.join("CHANGELOG.md"), "# Changelog\n").expect("a file");
    fs::write(repo.join("gates.yaml"), GATES).expect("a file");
    git(&repo, &["init", "-q", "-b", "main"]);
    git(&repo, &["add", "-A"]);
    git(&repo, &["commit", "-q", "-m", "start"]);
    let repo_arg = repo.to_str().expect("utf-8");

    let (code, out, err) = takt(&[
        "run",
        "--repo",
        repo_arg,
        "--intent",
        "record a note",
        "--case-id",
        "note",
        "--harness",
        "none",
        "--protocol",
        GATE_PROTOCOL,
        "--test-cmd",
        "true",
        "--gates",
        "gates.yaml",
    ]);
    assert_eq!(code, 3, "stdout {out} stderr {err}");
    let obligations = out
        .lines()
        .position(|line| line == "obligations: 1 obligation open: release-notes fail")
        .unwrap_or_else(|| panic!("the obligations line: {out}"));
    let stopped = out
        .lines()
        .position(|line| line.starts_with("stopped: "))
        .unwrap_or_else(|| panic!("the stop line: {out}"));
    assert!(obligations < stopped, "{out}");

    let case = repo.join(".takt/cases/note");
    let case_arg = case.to_str().expect("utf-8");
    let (code, out, err) = takt(&["report", "--case", case_arg]);
    assert_eq!(code, 0, "stdout {out} stderr {err}");
    assert!(
        out.lines()
            .any(|line| line.starts_with("| release-notes | fail |")),
        "{out}"
    );
    let (code, out, err) = takt(&["report", "--case", case_arg, "--format", "json"]);
    assert_eq!(code, 0, "stdout {out} stderr {err}");
    let json: serde_json::Value = serde_json::from_str(&out).expect("one JSON document");
    assert_eq!(json["runs"][0]["obligations"][0]["id"], "release-notes");
    assert_eq!(json["runs"][0]["obligations"][0]["status"], "fail");
}

/// The fixture stores of the library crate, one directory each.
const FIXTURES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../takt/tests/fixtures");

/// A gates document whose one gate holds in a repository with a manifest.
const MANIFEST_GATES: &str = "format: engineering-gates/1\nlanguage: canon-expr/1\ngates:\n  - id: manifest\n    \
                              text: The repository has its manifest.\n    assertions:\n      \
                              - 'fs.file.exists(\"Cargo.toml\")'\n";

/// Accepted once the tests pass and the gate has judged the implementation revision, whatever
/// it found: an accepted run whose gate failed.
const TESTS_AND_A_JUDGED_GATE: &str = "format: protocol/1

protocol:
  id: tests.and.a.judged.gate
  revision: 1
  description: >-
    Run the tests and the gate against the implementation at its current revision; accepted once
    the tests pass and the gate has judged it.

artifacts:
  implementation:
    description: a revision of the code

evidence_kinds:
  test_result:
    description: the result of running tests against an implementation revision
  gate_result:
    description: the result of the repository's gate against an implementation revision

claims:
  tests.pass:
    description: the tests pass for the current implementation revision
    true_when:
      evidence:
        kind: test_result
        result: pass
        subject: implementation
  gate.judged:
    description: the gate has judged the current implementation revision
    true_when:
      evidence:
        kind: gate_result
        subject: implementation

obligations:
  tests.required:
    description: the tests pass for the current implementation revision
    discharged_when:
      claim: tests.pass

actions:
  tests.run:
    description: run the tests against an implementation revision
    effect: read
    may_produce:
      - evidence: test_result
  gate.run:
    description: run the repository's gate against an implementation revision
    effect: read
    may_produce:
      - evidence: gate_result

outcomes:
  accepted:
    description: the tests pass and the gate has judged the current implementation revision
    requires:
      all:
        - claim: tests.pass
        - claim: gate.judged
";

/// A scratch directory under this crate's build directory, removed on drop.
struct Dir(PathBuf);

impl std::ops::Deref for Dir {
    type Target = Path;

    fn deref(&self) -> &Path {
        &self.0
    }
}

impl Drop for Dir {
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

/// `git <args>` in `tree`; its standard output, trimmed.
fn git_out(tree: &Path, args: &[&str]) -> String {
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

/// A committed copy of the `one-ready` store under `<scratch>/repo`, with the gates document
/// [`MANIFEST_GATES`], a commit identity and, when `manifest`, the manifest the gate asks for;
/// its origin is on github.com, so a run's forge is the bot route.
fn store_repository(name: &str, manifest: bool) -> (Dir, PathBuf) {
    let outer = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!(
        "obligations-report-cli-{}-{name}",
        std::process::id()
    ));
    if outer.exists() {
        fs::remove_dir_all(&outer).expect("a stale scratch tree is removed");
    }
    let repo = outer.join("repo");
    copy_dir(&Path::new(FIXTURES).join("one-ready"), &repo);
    if manifest {
        fs::write(repo.join("Cargo.toml"), "[package]\nname = \"scratch\"\n").expect("a file");
    }
    fs::write(repo.join(".engineering/gates.yaml"), MANIFEST_GATES).expect("a file");
    fs::write(
        repo.join(".engineering/commit-identity.conf"),
        "[identity]\n\tname = Test Bot\n\temail = bot@example.invalid\n",
    )
    .expect("the commit identity is written");
    git_out(&repo, &["init", "-q", "-b", "main"]);
    git_out(&repo, &["add", "-A"]);
    git_out(&repo, &["commit", "-q", "-m", "start"]);
    git_out(
        &repo,
        &["remote", "add", "origin", "git@github.com:owner/repo.git"],
    );
    (Dir(outer), repo)
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

/// A double of the bot route's program in `<dir>/stub-bin`, its state in `<dir>/forge`:
/// `b10x-gates bot -- push` keeps the branch and the commit pushed, `bot -- fetch` reports the
/// local branch as the repository's, `scan-text` passes, `api --method POST` opens pull request
/// #7 and `api --method PUT` merges it; `api --method GET` reports the pull request with the
/// pushed head, and one check run, in progress while `<dir>/forge/pending` exists and passing
/// otherwise. Nothing reaches a network.
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
    method=''; path=''; output=''
    while [ $# -gt 0 ]; do
      case "$1" in
        --method) method="$2"; shift ;;
        --path) path="$2"; shift ;;
        --output) output="$2"; shift ;;
      esac
      shift
    done
    case "$method" in
      POST) printf 'OPEN' > "$state/state"; printf '{{"number": 7}}' > "$output"; exit 0 ;;
      PUT) printf 'MERGED' > "$state/state"; printf '{{"merged": true}}' > "$output"; exit 0 ;;
      GET)
        case "$path" in
          */pulls\?*) printf '[]' > "$output" ;;
          */check-runs*) if [ -e "$state/pending" ]; then printf '{{"total_count":1,"check_runs":[{{"status":"in_progress","conclusion":null}}]}}' > "$output"; else printf '{{"total_count":1,"check_runs":[{{"status":"completed","conclusion":"success"}}]}}' > "$output"; fi ;;
          */status*) printf '{{"state":"pending","total_count":0,"statuses":[]}}' > "$output" ;;
          *) s=$(cat "$state/state"); st=open; merged=false
             case "$s" in MERGED) st=closed; merged=true ;; CLOSED) st=closed ;; esac
             printf '{{"state":"%s","merged":%s,"head":{{"ref":"%s","sha":"%s"}},"base":{{"ref":"main"}}}}' "$st" "$merged" "$(cat "$state/branch")" "$(tr -d '\n' < "$state/pushed")" > "$output" ;;
        esac
        exit 0 ;;
    esac ;;
esac
echo "b10x-gates double: $*" >&2
exit 2
"#
        ),
    );
    (bin, state)
}

/// Runs the built `takt` binary with `args`, `bin` first on the program path and a
/// `b10x-gates` policy file the double never reads: the exit code, stdout and stderr.
fn takt_through(bin: &Path, args: &[&str]) -> (i32, String, String) {
    let output = Command::new(env!("CARGO_BIN_EXE_takt"))
        .args(args)
        .env(
            "B10X_GATES_POLICY",
            concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../takt/tests/fixtures/gates-policy-stand-in.json"
            ),
        )
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

/// The `obligations:` lines of a run's output.
fn obligation_lines(out: &str) -> Vec<&str> {
    out.lines()
        .filter(|line| line.starts_with("obligations:"))
        .collect()
}

/// The status `takt report --case <case> --format json` gives the gate `id` in the case's last
/// run, and the text report's row for it.
fn reported(bin: &Path, case: &Path, id: &str) -> (String, String) {
    let case_arg = case.to_str().expect("utf-8");
    let (code, out, err) = takt_through(bin, &["report", "--case", case_arg, "--format", "json"]);
    assert_eq!(code, 0, "stdout {out} stderr {err}");
    let json: serde_json::Value = serde_json::from_str(&out).expect("one JSON document");
    let runs = json["runs"].as_array().expect("the runs");
    let last = runs.last().expect("a run");
    let rows = last["obligations"].as_array().expect("the obligations");
    let status = rows
        .iter()
        .find(|row| row["id"] == id)
        .and_then(|row| row["status"].as_str())
        .unwrap_or_else(|| panic!("{id} in the last run's obligations: {out}"))
        .to_owned();
    let (code, text, err) = takt_through(bin, &["report", "--case", case_arg]);
    assert_eq!(code, 0, "stdout {text} stderr {err}");
    let row = text
        .lines()
        .rfind(|line| line.starts_with(&format!("| {id} |")))
        .unwrap_or_else(|| panic!("{id}'s row: {text}"))
        .to_owned();
    (status, row)
}

/// A delivery of the `one-ready` story on `story.delivery/1` through the bot route's double, up
/// to the grant: a run takes the story, the operator commits the work on the case's branch, the
/// next run runs the tests and the gate at that commit, opens the pull request, reads its checks
/// and stops at the gate. The scratch tree, the repository, the double's programs and state, the
/// case's id and the work's commit.
struct Delivery {
    _scratch: Dir,
    repo: PathBuf,
    bin: PathBuf,
    state: PathBuf,
    id: String,
    worked: String,
}

impl Delivery {
    fn to_the_grant(name: &str) -> Delivery {
        let (scratch, repo) = store_repository(name, true);
        let (bin, state) = forge_double(&scratch.join("double"));
        let delivery = Delivery {
            _scratch: scratch,
            repo,
            bin,
            state,
            id: String::new(),
            worked: String::new(),
        };
        let (code, out, err) = delivery.run(&["--iterations", "0"]);
        assert_eq!(code, 3, "stdout {out} stderr {err}");
        let id = git_out(&delivery.repo, &["branch", "--show-current"])
            .strip_prefix("takt/")
            .expect("the tree is on the case's branch")
            .to_owned();
        fs::write(delivery.repo.join("feature.txt"), "the work\n").expect("a file");
        git_out(&delivery.repo, &["add", "feature.txt"]);
        git_out(&delivery.repo, &["commit", "-q", "-m", "the work"]);
        let worked = git_out(&delivery.repo, &["rev-parse", "HEAD"]);
        let delivery = Delivery {
            id,
            worked,
            ..delivery
        };
        let (code, out, err) = delivery.run(&[]);
        assert_eq!(code, 4, "stdout {out} stderr {err}");
        for line in [
            "evidence: gate_result pass",
            "stopped: ApprovalRequired (repository.merge)",
        ] {
            assert!(out.contains(line), "{line}: stdout {out} stderr {err}");
        }
        delivery
    }

    /// `takt run` on the repository with no harness, on `story.delivery/1`, with `extra`.
    fn run(&self, extra: &[&str]) -> (i32, String, String) {
        let mut args = vec![
            "run",
            "--repo",
            self.repo.to_str().expect("utf-8"),
            "--harness",
            "none",
            "--test-cmd",
            "true",
            "--protocol",
            "story.delivery/1",
        ];
        args.extend(extra);
        takt_through(&self.bin, &args)
    }

    fn approve(&self) {
        let (code, out, err) = takt_through(
            &self.bin,
            &[
                "approve",
                "--repo",
                self.repo.to_str().expect("utf-8"),
                &self.id,
                "repository.merge",
                "--by",
                "human:operator",
            ],
        );
        assert_eq!(code, 0, "stdout {out} stderr {err}");
    }

    fn case(&self) -> PathBuf {
        self.repo.join(".takt/cases").join(&self.id)
    }

    fn implementation(&self) -> String {
        let case = fs::read_to_string(self.case().join("case.yaml")).expect("the case");
        let yaml: serde_json::Value = serde_json::to_value(
            serde_yaml_ng::from_str::<serde_yaml_ng::Value>(&case).expect("yaml"),
        )
        .expect("json");
        yaml["artifacts"]["implementation"]["revision"]
            .as_str()
            .expect("the implementation revision")
            .to_owned()
    }
}

#[test]
fn an_accepted_run_reports_the_tested_revisions_obligations() {
    let delivery = Delivery::to_the_grant("accepted-pass");
    delivery.approve();
    let (code, out, err) = delivery.run(&[]);
    assert_eq!(code, 0, "stdout {out} stderr {err}");
    assert!(
        out.contains("stopped: Completed (accepted)"),
        "stdout {out} stderr {err}"
    );
    // takt's store commits sit above the tested revision, and do not move it.
    let head = git_out(&delivery.repo, &["rev-parse", "HEAD"]);
    assert_ne!(head, delivery.worked, "{out}");
    assert_eq!(delivery.implementation(), delivery.worked, "{out}");
    assert_eq!(
        obligation_lines(&out),
        Vec::<&str>::new(),
        "the gate passed at the implementation revision: stdout {out}"
    );
    let (status, row) = reported(&delivery.bin, &delivery.case(), "manifest");
    assert_eq!(status, "pass", "{row}");
    assert!(row.starts_with("| manifest | pass |"), "{row}");
}

#[test]
fn an_accepted_run_with_a_failed_gate_still_reports_it_open() {
    let (_scratch, repo) = store_repository("accepted-fail", false);
    let protocol = repo.join("tests-and-a-judged-gate.yaml");
    fs::write(&protocol, TESTS_AND_A_JUDGED_GATE).expect("the protocol");
    git_out(&repo, &["add", "-A"]);
    git_out(&repo, &["commit", "-q", "-m", "the protocol"]);
    let bin = repo.join("no-such-bin");
    let (code, out, err) = takt_through(
        &bin,
        &[
            "run",
            "--repo",
            repo.to_str().expect("utf-8"),
            "--harness",
            "none",
            "--forge",
            "none",
            "--test-cmd",
            "true",
            "--protocol",
            protocol.to_str().expect("utf-8"),
        ],
    );
    assert_eq!(code, 0, "stdout {out} stderr {err}");
    assert!(
        out.contains("stopped: Completed (accepted)"),
        "stdout {out} stderr {err}"
    );
    assert!(out.contains("evidence: gate_result fail"), "{out}");
    assert!(
        git_out(&repo, &["log", "-1", "--format=%s"]).starts_with("takt: implemented "),
        "the run stops at takt's store commits: {out}"
    );
    assert_eq!(
        obligation_lines(&out),
        ["obligations: 1 obligation open: manifest fail"],
        "stdout {out}"
    );
    let id = git_out(&repo, &["branch", "--show-current"])
        .strip_prefix("takt/")
        .expect("the tree is on the case's branch")
        .to_owned();
    let (status, row) = reported(&bin, &repo.join(".takt/cases").join(id), "manifest");
    assert_eq!(status, "fail", "{row}");
}

#[test]
fn a_code_change_after_the_gate_still_reads_not_yet_run() {
    let delivery = Delivery::to_the_grant("code-after-gate");
    fs::write(delivery.repo.join("more.txt"), "more work\n").expect("a file");
    git_out(&delivery.repo, &["add", "more.txt"]);
    git_out(&delivery.repo, &["commit", "-q", "-m", "more work"]);
    let changed = git_out(&delivery.repo, &["rev-parse", "HEAD"]);
    let (code, out, err) = delivery.run(&["--iterations", "0"]);
    assert_eq!(code, 3, "stdout {out} stderr {err}");
    assert_eq!(delivery.implementation(), changed, "{out}");
    assert_eq!(
        obligation_lines(&out),
        ["obligations: 1 obligation open: manifest not yet run"],
        "a status the earlier revision earned does not carry over to changed code: stdout {out}"
    );
    let (status, row) = reported(&delivery.bin, &delivery.case(), "manifest");
    assert_eq!(status, "not_yet_run", "{row}");
    assert!(row.starts_with("| manifest | not yet run |"), "{row}");
}

#[test]
fn a_stop_time_table_does_not_hide_the_gates_table() {
    let delivery = Delivery::to_the_grant("stop-at-store");
    delivery.approve();
    // The checks on the head carrying takt's records are pending: the run records the story,
    // pushes, and stops waiting at those store commits.
    fs::write(delivery.state.join("pending"), "").expect("the flag");
    let (code, out, err) = delivery.run(&[]);
    assert_eq!(code, 6, "stdout {out} stderr {err}");
    assert!(
        git_out(&delivery.repo, &["log", "-1", "--format=%s"]).starts_with("takt: implemented "),
        "the run stops at takt's store commits: {out}"
    );
    assert_eq!(delivery.implementation(), delivery.worked, "{out}");
    assert_eq!(obligation_lines(&out), Vec::<&str>::new(), "stdout {out}");
    let (status, row) = reported(&delivery.bin, &delivery.case(), "manifest");
    assert_eq!(status, "pass", "the stop's table: {row}");

    // A stop's table at the store commits, as takt recorded one before this story: the last
    // table of the record, at another revision than the implementation's.
    let head = git_out(&delivery.repo, &["rev-parse", "HEAD"]);
    takt::events::Events::new(&delivery.case())
        .emit(
            1,
            &takt::events::Event::Obligations {
                revision: head,
                obligations: vec![takt::obligations::Obligation {
                    id: "manifest".to_owned(),
                    text: "The repository has its manifest.".to_owned(),
                    status: takt::obligations::Status::NotYetRun,
                    commands: Vec::new(),
                }],
            },
        )
        .expect("the table is recorded");

    // The resume reads the gate's status at the implementation revision.
    fs::remove_file(delivery.state.join("pending")).expect("the flag goes");
    let (code, out, err) = delivery.run(&[]);
    assert_eq!(code, 0, "stdout {out} stderr {err}");
    assert!(
        out.contains("stopped: Completed (accepted)"),
        "stdout {out} stderr {err}"
    );
    assert_eq!(obligation_lines(&out), Vec::<&str>::new(), "stdout {out}");
    let (status, row) = reported(&delivery.bin, &delivery.case(), "manifest");
    assert_eq!(status, "pass", "the resumed run's table: {row}");
}
