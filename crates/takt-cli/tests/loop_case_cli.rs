//! `takt run` on `loop.improvement/1` (story:loop-case-opener, folded into story:loop-scorecard):
//! a case opens with `loop_baseline` at the commit `--input loop_baseline=<full commit id>`
//! names and the baseline's metric definitions snapshotted before any step, so `loop.measure`
//! records over the recorded turns through the binary alone; a missing value, or one that is no
//! full commit id of a commit of the repository, is refused before anything is written.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

/// The loop-improvement protocol of this repository.
const LOOP_PROTOCOL: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../protocols/loop-improvement/1.yaml"
);

/// Definitions with one target: accepted share at least three quarters.
const DEFINED: &str = "format: takt-loop-metrics/1
metrics:
  turns:
    version: 1
    definition: the improvement turns recorded
  accepted_share:
    version: 1
    definition: accepted rows over all rows, open turns included
    target:
      at_least: 0.75
";

fn git(tree: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .args(["-c", "user.name=t", "-c", "user.email=t@localhost"])
        .args(args)
        .current_dir(tree)
        .output()
        .expect("git runs");
    assert!(output.status.success(), "git {args:?}");
    String::from_utf8_lossy(&output.stdout).trim().to_owned()
}

/// A scratch repository under this crate's build directory, removed on drop.
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

/// A committed repository holding the loop's metric definitions and three recorded turns
/// (one accepted, two declined) under its cases directory; its `HEAD`.
fn repository(name: &str) -> (Scratch, String) {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("loop-case-cli-{}-{name}", std::process::id()));
    if dir.exists() {
        fs::remove_dir_all(&dir).expect("a stale scratch tree is removed");
    }
    let metrics = dir.join("protocols/loop-improvement/metrics.yaml");
    fs::create_dir_all(metrics.parent().expect("a parent")).expect("the directory");
    fs::write(&metrics, DEFINED).expect("the definitions");
    git(&dir, &["init", "-q", "-b", "main"]);
    git(&dir, &["add", "-A"]);
    git(&dir, &["commit", "-q", "-m", "start"]);
    let head = git(&dir, &["rev-parse", "HEAD"]);
    for (id, verdict) in [
        ("turn-a", "accepted"),
        ("turn-b", "declined"),
        ("turn-c", "declined"),
    ] {
        let case = dir.join(".engineering/cases").join(id);
        fs::create_dir_all(case.join("decisions")).expect("the case directory");
        fs::write(
            case.join("case.yaml"),
            format!(
                "format: canon-case/1\nid: {id}\nprotocol: self.improvement\nartifacts:\n  finding:\n    revision: {head}\n  baseline:\n    revision: {head}\n  candidate:\n    revision: {head}\n  promotion:\n    revision: {head}\ninputs:\n  surface: non_judge\n"
            ),
        )
        .expect("case.yaml");
        fs::write(
            case.join("decisions/0000.json"),
            format!(
                r#"{{"claims":{{"finding.cited":{{"value":"true"}}}},"outcomes":{{"{verdict}":{{"status":"legitimate"}}}},"explanation":{{"computed_from":{{"at":"2026-10-18T10:00:00Z"}}}}}}"#
            ),
        )
        .expect("a decision");
    }
    (Scratch(dir), head)
}

/// `takt run --protocol <loop-improvement/1> --intent ... --input loop_baseline=<value>` (no
/// input when `value` is none) in `repo`, one step at most; its exit code and standard error.
fn run(repo: &Path, value: Option<&str>) -> (i32, String, String) {
    let input = value.map(|value| format!("loop_baseline={value}"));
    let mut args = vec![
        "run",
        "--repo",
        repo.to_str().expect("utf-8"),
        "--protocol",
        LOOP_PROTOCOL,
        "--intent",
        "raise the loop's accepted share",
        "--case-id",
        "loop-1",
        "--harness",
        "none",
        "--forge",
        "none",
        "--test-cmd",
        "true",
        "--iterations",
        "1",
    ];
    if let Some(input) = &input {
        args.extend(["--input", input.as_str()]);
    }
    let output = Command::new(env!("CARGO_BIN_EXE_takt"))
        .args(&args)
        .env_remove("B10X_GATES_POLICY")
        .output()
        .expect("takt runs");
    (
        output.status.code().expect("an exit code"),
        String::from_utf8_lossy(&output.stdout).into_owned(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
    )
}

/// The case directories under `repo`, by name.
fn cases(repo: &Path) -> Vec<String> {
    let mut names: Vec<String> = fs::read_dir(repo.join(".engineering/cases"))
        .expect("the cases directory")
        .map(|entry| {
            entry
                .expect("an entry")
                .file_name()
                .to_string_lossy()
                .into_owned()
        })
        .collect();
    names.sort();
    names
}

/// Acceptance of story:loop-case-opener: the case opens with `loop_baseline` at the commit given
/// and its snapshot taken, and `loop.measure` records over the three recorded turns without a
/// hand-written `case.yaml`.
#[test]
fn takt_run_opens_a_loop_case_whose_measure_records_over_the_turns() {
    let (repo, head) = repository("opens");
    let (code, out, err) = run(&repo, Some(&head));
    let case = repo.join(".engineering/cases/loop-1");
    assert!(
        case.join("case.yaml").is_file(),
        "exit {code}\n{out}\n{err}"
    );
    let opened = takt::CaseDir::resume(&repo, "loop-1").expect("the case reads");
    assert_eq!(
        opened.revision("loop_baseline").as_deref(),
        Some(head.as_str()),
        "{out}\n{err}"
    );
    let taken = fs::read_to_string(case.join("loop-baseline.json"))
        .unwrap_or_else(|error| panic!("the snapshot ({error})\n{out}\n{err}"));
    assert!(
        taken.contains(&format!("\"revision\": \"{head}\"")),
        "{taken}"
    );
    assert!(taken.contains("takt-loop-metrics/1"), "{taken}");
    let measured: Vec<String> = fs::read_dir(case.join("evidence"))
        .expect("the evidence directory")
        .map(|entry| fs::read_to_string(entry.expect("an entry").path()).expect("a record"))
        .filter(|record| record.contains("\"kind\": \"loop_measurement\""))
        .collect();
    assert_eq!(measured.len(), 1, "exit {code}\n{out}\n{err}");
    let record = &measured[0];
    assert!(record.contains("\"result\": \"below_target\""), "{record}");
    assert!(
        record.contains("\"subject\": \"loop_baseline\""),
        "{record}"
    );
    assert!(
        record.contains(&format!("\"subject_revision\": \"{head}\"")),
        "{record}"
    );
}

/// A loop case without `--input loop_baseline`, or with a value that is no full commit id of a
/// commit of the repository (a short id, an id the repository does not hold, a tree's id, a
/// branch name), is refused before anything is written: no case directory, no case branch.
#[test]
fn a_loop_case_without_a_commit_for_its_baseline_is_refused_before_anything_is_written() {
    let (repo, head) = repository("refused");
    let tree = git(&repo, &["rev-parse", "HEAD^{tree}"]);
    let short = head[..12].to_owned();
    let unknown = "0".repeat(40);
    for value in [
        None,
        Some(short.as_str()),
        Some(unknown.as_str()),
        Some(tree.as_str()),
        Some("main"),
    ] {
        let (code, out, err) = run(&repo, value);
        assert_eq!(code, 1, "{value:?}: {out}\n{err}");
        assert!(err.contains("loop_baseline"), "{value:?}: {err}");
        assert_eq!(
            cases(&repo),
            ["turn-a", "turn-b", "turn-c"],
            "{value:?}: nothing was written"
        );
        assert_eq!(
            git(&repo, &["branch", "--list", "takt/*"]),
            "",
            "{value:?}: no case branch"
        );
        assert_eq!(git(&repo, &["rev-parse", "--abbrev-ref", "HEAD"]), "main");
    }
}

/// The reviewer's planter: writes a loose object under an id with other bytes than the ones the
/// id names, as any process with write access to the object directory can.
const PLANT: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../takt/tests/fixtures/plant_loose_object.py"
);

/// The definitions of [`DEFINED`] with the accepted share's target lowered to 0.25.
fn forged() -> String {
    DEFINED.replace("at_least: 0.75", "at_least: 0.25")
}

/// Plants, under the id of the definitions blob at `HEAD` of `repo`, a loose object holding the
/// forged definitions; the blob's id.
fn plant(repo: &Path) -> String {
    let blob = git(
        repo,
        &["rev-parse", "HEAD:protocols/loop-improvement/metrics.yaml"],
    );
    let forged_file = repo.join(".git/forged.yaml");
    fs::write(&forged_file, forged()).expect("the forged definitions");
    let status = Command::new("python3")
        .arg("-I")
        .arg(PLANT)
        .arg(repo.join(".git"))
        .arg(&blob)
        .arg(&forged_file)
        .stdout(std::process::Stdio::null())
        .status()
        .expect("python3 runs");
    assert!(status.success(), "the object is planted");
    assert!(
        git(repo, &["show", &blob]).contains("at_least: 0.25"),
        "plain git reads the planted bytes under the blob's id"
    );
    blob
}

/// That a refusal of the loop case wrote nothing: only the recorded turns, no case branch.
fn nothing_written(repo: &Path, what: &str) {
    assert_eq!(
        cases(repo),
        ["turn-a", "turn-b", "turn-c"],
        "{what}: nothing was written"
    );
    assert_eq!(
        git(repo, &["branch", "--list", "takt/*"]),
        "",
        "{what}: no case branch"
    );
}

/// The reviewer's probe 5 (plant): a loose object planted over the base's definitions blob, the
/// real object loose or packed, as a step of an earlier case could leave it. The loop case is
/// refused before anything is written, naming the object, instead of snapshotting the forged
/// target.
#[test]
fn a_definitions_object_planted_before_the_loop_case_opens_is_refused() {
    for packed in [false, true] {
        let (repo, head) = repository(if packed {
            "plant-packed"
        } else {
            "plant-loose"
        });
        if packed {
            git(&repo, &["gc", "-q", "--prune=now"]);
        }
        let blob = plant(&repo);
        let (code, out, err) = run(&repo, Some(&head));
        assert_eq!(code, 1, "packed {packed}: {out}\n{err}");
        assert!(
            err.contains(&blob),
            "packed {packed}: the object is named: {err}"
        );
        nothing_written(&repo, &format!("packed {packed}"));
    }
}

/// The reviewer's probe 5 (test command): an earlier case's test command, code from the tree
/// run by `tests.run`, plants the forged object; the loop case opened afterwards is refused.
#[test]
fn a_definitions_object_an_earlier_test_command_planted_is_refused() {
    let (repo, head) = repository("test-command");
    fs::write(repo.join(".git/forged.yaml"), forged()).expect("the forged definitions");
    fs::write(
        repo.join("test.sh"),
        format!(
            "#!/bin/sh\nblob=$(git rev-parse HEAD:protocols/loop-improvement/metrics.yaml)\npython3 -I '{PLANT}' .git \"$blob\" .git/forged.yaml\nexit 0\n"
        ),
    )
    .expect("test.sh");
    git(&repo, &["add", "test.sh"]);
    git(&repo, &["commit", "-q", "-m", "the test command"]);
    let output = Command::new(env!("CARGO_BIN_EXE_takt"))
        .args([
            "run",
            "--repo",
            repo.to_str().expect("utf-8"),
            "--intent",
            "an earlier case",
            "--case-id",
            "earlier",
            "--harness",
            "none",
            "--forge",
            "none",
            "--test-cmd",
            "sh test.sh",
            "--iterations",
            "3",
        ])
        .env_remove("B10X_GATES_POLICY")
        .output()
        .expect("takt runs");
    let earlier = String::from_utf8_lossy(&output.stdout).into_owned();
    git(&repo, &["checkout", "-q", "main"]);
    let blob = git(
        &repo,
        &["rev-parse", "HEAD:protocols/loop-improvement/metrics.yaml"],
    );
    assert!(
        git(&repo, &["show", &blob]).contains("at_least: 0.25"),
        "the earlier case's test command planted the object: {earlier}"
    );
    let (code, out, err) = run(&repo, Some(&head));
    assert_eq!(code, 1, "{out}\n{err}");
    assert!(err.contains(&blob), "the object is named: {err}");
    assert!(
        !repo.join(".engineering/cases/loop-1").exists(),
        "no loop case was written"
    );
}

/// The reviewer's probe 5 (branch): a baseline that is a commit of the repository but not on
/// the base branch the case opens from, a side branch's with a looser target or an unreachable
/// one, is refused before anything is written.
#[test]
fn a_baseline_off_the_base_branch_is_refused_before_anything_is_written() {
    let (repo, _) = repository("off-base");
    git(&repo, &["checkout", "-q", "-b", "side"]);
    fs::write(
        repo.join("protocols/loop-improvement/metrics.yaml"),
        DEFINED.replace("at_least: 0.75", "at_least: 0.1"),
    )
    .expect("the looser definitions");
    git(&repo, &["commit", "-q", "-am", "loosen"]);
    let side = git(&repo, &["rev-parse", "HEAD"]);
    git(&repo, &["checkout", "-q", "main"]);
    let tree = git(&repo, &["rev-parse", &format!("{side}^{{tree}}")]);
    let dangling = git(
        &repo,
        &["commit-tree", &tree, "-p", &side, "-m", "dangling"],
    );
    for (what, value) in [("side", side.as_str()), ("unreachable", dangling.as_str())] {
        let (code, out, err) = run(&repo, Some(value));
        assert_eq!(code, 1, "{what}: {out}\n{err}");
        assert!(err.contains("loop_baseline"), "{what}: {err}");
        assert!(
            err.contains("main"),
            "{what}: the base branch is named: {err}"
        );
        nothing_written(&repo, what);
        assert_eq!(git(&repo, &["rev-parse", "--abbrev-ref", "HEAD"]), "main");
    }
}
