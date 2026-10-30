//! Adversarial cases on story:an-accepted-run-reports-the-tested-revisions-obligations through
//! the `takt` binary: a code commit above takt's store commits, a store look-alike carrying code,
//! the revision every table of an accepted run is recorded at, a resume after a stop that is not
//! accepted, and a stop-time table at the store commits as takt 0.16 recorded it.

#[path = "../../takt/tests/support/own_home.rs"]
mod own_home;

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

/// The fixture stores of the library crate, one directory each.
const FIXTURES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../takt/tests/fixtures");

/// A gates document whose one gate holds in a repository with a manifest.
const MANIFEST_GATES: &str = "format: engineering-gates/1\nlanguage: canon-expr/1\ngates:\n  - id: manifest\n    \
                              text: The repository has its manifest.\n    assertions:\n      \
                              - 'fs.file.exists(\"Cargo.toml\")'\n";

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
/// its origin is the bare hub `<scratch>/hub.git`, holding its `main`, of the local forge.
fn store_repository(name: &str, manifest: bool) -> (Dir, PathBuf) {
    let outer = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!(
        "obligations-report-adv1-{}-{name}",
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
    git_out(&outer, &["init", "-q", "--bare", "-b", "main", "hub.git"]);
    let hub = outer.join("hub.git");
    git_out(
        &repo,
        &["remote", "add", "origin", hub.to_str().expect("utf-8")],
    );
    git_out(&repo, &["push", "-q", "origin", "main"]);
    (Dir(outer), repo)
}

// story:takt-carries-no-organisations-own-route: the bot forge double is gone; these cases run on
// the local forge.

/// The run file of the local forge in `dir`, `run.yaml`: its checks pass unless the file
/// `<dir>/checks-fail` exists. The run file and that flag's path.
fn local_forge(dir: &Path) -> (PathBuf, PathBuf) {
    let flag = dir.join("checks-fail");
    let config = dir.join("run.yaml");
    fs::write(
        &config,
        format!(
            "forge: local\nlocal_checks: [\"sh\", \"-c\", \"test ! -e '{}'\"]\n",
            flag.display()
        ),
    )
    .expect("the run file");
    (config, flag)
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

/// The `obligations:` lines of a run's output.
fn obligation_lines(out: &str) -> Vec<&str> {
    out.lines()
        .filter(|line| line.starts_with("obligations:"))
        .collect()
}

/// The status `takt report --case <case> --format json` gives the gate `id` in the case's last
/// run, and the text report's row for it.
fn reported(case: &Path, id: &str) -> (String, String) {
    let case_arg = case.to_str().expect("utf-8");
    let (code, out, err) = takt(&["report", "--case", case_arg, "--format", "json"]);
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
    let (code, text, err) = takt(&["report", "--case", case_arg]);
    assert_eq!(code, 0, "stdout {text} stderr {err}");
    let row = text
        .lines()
        .rfind(|line| line.starts_with(&format!("| {id} |")))
        .unwrap_or_else(|| panic!("{id}'s row: {text}"))
        .to_owned();
    (status, row)
}

/// A delivery of the `one-ready` story on `story.delivery/1` through the local forge, up to
/// the grant: a run takes the story, the operator commits the work on the case's branch, the
/// next run runs the tests and the gate at that commit, opens the pull request, reads its checks
/// and stops at the gate. The scratch tree, the repository, the forge's run file and
/// the flag that fails its checks, the case's id and the work's commit.
struct Delivery {
    _scratch: Dir,
    repo: PathBuf,
    config: PathBuf,
    flag: PathBuf,
    id: String,
    worked: String,
}

impl Delivery {
    fn to_the_grant(name: &str) -> Delivery {
        let (scratch, repo) = store_repository(name, true);
        let (config, flag) = local_forge(&scratch);
        let delivery = Delivery {
            _scratch: scratch,
            repo,
            config,
            flag,
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
            "--config",
            self.config.to_str().expect("utf-8"),
        ];
        args.extend(extra);
        takt(&args)
    }

    fn approve(&self) {
        let (code, out, err) = takt(&[
            "approve",
            "--repo",
            self.repo.to_str().expect("utf-8"),
            &self.id,
            "repository.merge",
            "--by",
            "human:operator",
        ]);
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

/// The case's `obligations` lines, in record order: each line's revision and its rows' statuses.
fn tables(case: &Path) -> Vec<(String, Vec<String>)> {
    let text = fs::read_to_string(case.join("events.jsonl")).expect("the record");
    text.lines()
        .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
        .filter(|line| line["event"] == "obligations")
        .map(|line| {
            let revision = line["revision"].as_str().unwrap_or_default().to_owned();
            let statuses = line["obligations"]
                .as_array()
                .map(|rows| {
                    rows.iter()
                        .map(|row| row["status"].as_str().unwrap_or_default().to_owned())
                        .collect()
                })
                .unwrap_or_default();
            (revision, statuses)
        })
        .collect()
}

/// A delivery approved and stopped, its checks failing on the new head, at takt's store commits
/// (the `test_result` record and the move), the gate having passed at the work's commit: nothing
/// is merged.
fn stopped_at_the_store_commits(name: &str) -> Delivery {
    let delivery = Delivery::to_the_grant(name);
    delivery.approve();
    fs::write(&delivery.flag, "").expect("the flag");
    let (code, out, err) = delivery.run(&[]);
    assert_ne!(code, 0, "stdout {out} stderr {err}");
    assert!(
        out.contains("through local: fail; nothing merged"),
        "stdout {out} stderr {err}"
    );
    assert!(
        !out.contains("merge_observation"),
        "stdout {out} stderr {err}"
    );
    assert!(
        git_out(&delivery.repo, &["log", "-1", "--format=%s"]).starts_with("takt: implemented "),
        "the run stops at takt's store commits: {out}"
    );
    assert_eq!(delivery.implementation(), delivery.worked, "{out}");
    fs::remove_file(&delivery.flag).expect("the flag goes");
    delivery
}

#[test]
fn a_code_commit_above_the_store_commits_reads_not_yet_run() {
    let delivery = stopped_at_the_store_commits("code-above-store");
    fs::write(delivery.repo.join("more.txt"), "more work\n").expect("a file");
    git_out(&delivery.repo, &["add", "more.txt"]);
    git_out(&delivery.repo, &["commit", "-q", "-m", "more work"]);
    let changed = git_out(&delivery.repo, &["rev-parse", "HEAD"]);
    let (_, out, err) = delivery.run(&["--iterations", "0"]);
    assert_eq!(
        delivery.implementation(),
        changed,
        "stdout {out} stderr {err}"
    );
    assert_eq!(
        obligation_lines(&out),
        ["obligations: 1 obligation open: manifest not yet run"],
        "the gate's pass below the store commits does not carry over to the code above them: stdout {out} stderr {err}"
    );
    let (status, row) = reported(&delivery.case(), "manifest");
    assert_eq!(status, "not_yet_run", "{row}");
}

#[test]
fn a_store_look_alike_commit_carrying_code_reads_not_yet_run() {
    let delivery = stopped_at_the_store_commits("look-alike");
    let subjects = git_out(&delivery.repo, &["log", "--format=%s", "-n", "5"]);
    let test_result = subjects
        .lines()
        .find(|subject| subject.starts_with("takt: test_result "))
        .unwrap_or_else(|| panic!("a test_result commit: {subjects}"))
        .to_owned();
    // A commit with the subject of takt's `test_result` record that changes code.
    fs::write(delivery.repo.join("feature.txt"), "changed work\n").expect("a file");
    git_out(&delivery.repo, &["add", "feature.txt"]);
    git_out(&delivery.repo, &["commit", "-q", "-m", &test_result]);
    let changed = git_out(&delivery.repo, &["rev-parse", "HEAD"]);
    let (_, out, err) = delivery.run(&["--iterations", "0"]);
    assert_eq!(
        delivery.implementation(),
        changed,
        "stdout {out} stderr {err}"
    );
    assert_eq!(
        obligation_lines(&out),
        ["obligations: 1 obligation open: manifest not yet run"],
        "stdout {out} stderr {err}"
    );
    let (status, row) = reported(&delivery.case(), "manifest");
    assert_eq!(status, "not_yet_run", "{row}");
}

#[test]
fn every_table_of_an_accepted_run_is_at_the_tested_revision() {
    let delivery = Delivery::to_the_grant("tables-at-tested");
    delivery.approve();
    let (code, out, err) = delivery.run(&[]);
    assert_eq!(code, 0, "stdout {out} stderr {err}");
    let head = git_out(&delivery.repo, &["rev-parse", "HEAD"]);
    assert_ne!(head, delivery.worked, "{out}");
    let tables = tables(&delivery.case());
    assert!(!tables.is_empty(), "the record holds tables: {out}");
    for (revision, _) in &tables {
        assert_ne!(revision, &head, "a table at the store commits: {tables:?}");
    }
    let (revision, statuses) = tables.last().expect("a table");
    assert_eq!(revision, &delivery.worked, "{tables:?}");
    assert_eq!(statuses, &["pass"], "{tables:?}");
    // The stop's line and the report read the same table.
    assert_eq!(obligation_lines(&out), Vec::<&str>::new(), "stdout {out}");
    let (status, _) = reported(&delivery.case(), "manifest");
    assert_eq!(status, "pass");
}

#[test]
fn a_resume_after_a_stop_that_is_not_accepted_keeps_the_gates_status() {
    let delivery = Delivery::to_the_grant("resume-unaccepted");
    // The grant stop's own line: the gate passed at the work's commit.
    let (status, row) = reported(&delivery.case(), "manifest");
    assert_eq!(status, "pass", "{row}");
    // Resumed without the grant: the run stops again before the merge.
    let (_, out, err) = delivery.run(&["--iterations", "0"]);
    assert_eq!(delivery.implementation(), delivery.worked, "{out}");
    assert_eq!(
        obligation_lines(&out),
        Vec::<&str>::new(),
        "stdout {out} stderr {err}"
    );
    let (revision, statuses) = tables(&delivery.case()).pop().expect("a table");
    assert_eq!(revision, delivery.worked);
    assert_eq!(statuses, ["pass"]);
    let (status, row) = reported(&delivery.case(), "manifest");
    assert_eq!(status, "pass", "{row}");
}

#[test]
fn a_stop_table_at_the_store_commits_does_not_hide_the_gates_table_in_the_report() {
    let delivery = Delivery::to_the_grant("legacy-stop-table");
    delivery.approve();
    let (code, out, err) = delivery.run(&[]);
    assert_eq!(code, 0, "stdout {out} stderr {err}");
    assert!(out.contains("stopped: Completed (accepted)"), "{out}");
    let head = git_out(&delivery.repo, &["rev-parse", "HEAD"]);
    // The record as takt 0.16.0 wrote an accepted run's: the stop's table at the store commits,
    // the head, every gate `not_yet_run`, after the gate's `pass` at the work's commit. An
    // accepted case is not resumed, so only `takt report` reads it again.
    let path = delivery.case().join("events.jsonl");
    let text = fs::read_to_string(&path).expect("the record");
    let mut lines: Vec<serde_json::Value> = text
        .lines()
        .map(|line| serde_json::from_str(line).expect("a JSON line"))
        .collect();
    let last = lines
        .iter_mut()
        .rev()
        .find(|line| line["event"] == "obligations")
        .expect("the stop's table");
    last["revision"] = serde_json::Value::String(head);
    for row in last["obligations"].as_array_mut().expect("rows") {
        row["status"] = serde_json::Value::String("not_yet_run".to_owned());
    }
    let rewritten: String = lines.iter().map(|line| format!("{line}\n")).collect();
    fs::write(&path, rewritten).expect("the record is rewritten");
    assert!(
        tables(&delivery.case())
            .iter()
            .any(|(revision, statuses)| revision == &delivery.worked && statuses == &["pass"]),
        "the gate's table at the work's commit stays in the record"
    );
    let (status, row) = reported(&delivery.case(), "manifest");
    assert_eq!(
        status, "pass",
        "the report reads the gate's table at the tested revision, not the stop's at the store commits: {row}"
    );
}
