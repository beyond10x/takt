//! The `self.improvement/1` validators of `crates/takt/src/improvement.rs` on scratch turns
//! (story:self-improvement-protocol, part 1): `finding.file` judges the finding as the case's
//! base holds it with the base's `scripts/file-finding`, and `scorecard.compare` reads the
//! candidate's saved scorecard from `$XDG_STATE_HOME/takt/scorecards/scorecard-<candidate>.json`,
//! waiting and naming that path while it is missing, and compares it with the baseline's copy in
//! the promotion store under the promotion rule. `takt run` driving a turn to `unreproducible`
//! and `declined` is `crates/takt-cli/tests/self_improvement_cli.rs`.

use std::fs;
use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};
use std::process::Command;

use sha2::{Digest as _, Sha256};
use takt::case::TurnOpening;
use takt::improvement::{Checking, FindingFile, ScorecardCompare};
use takt::promotion::{Kind, Promotion};
use takt::scorecard::{HeldOut, Row, Rule, Scorecard};
use takt::{CaseDir, LoadedProtocol, Validator};

const SELF_IMPROVEMENT: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../protocols/self-improvement/1.yaml"
);

/// The finding scripts of this repository, committed into a scratch repository's base.
const SCRIPTS: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../scripts");

const FINDING: &str = "story:probe-shows-the-defect";

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

fn write(path: &Path, text: &str) {
    fs::create_dir_all(path.parent().expect("a parent")).expect("a directory");
    fs::write(path, text).expect("a file");
}

/// A turn opened on a scratch repository: its tree, on the case's branch, its baseline and
/// candidate, and the state and cache directories the validators are given.
struct Turn {
    _scratch: Scratch,
    tree: PathBuf,
    baseline: String,
    candidate: String,
    state: PathBuf,
    cache: PathBuf,
}

/// A repository whose base holds the finding scripts and the finding `finding` (its body), with
/// a candidate commit on a side branch, and the turn `turn` opened on it through the kernel's
/// opening method.
fn turn(name: &str, finding: &str) -> Turn {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("self-improvement-{}-{name}", std::process::id()));
    if dir.exists() {
        fs::remove_dir_all(&dir).expect("a stale scratch tree is removed");
    }
    let tree = dir.join("takt");
    for script in ["check-finding", "file-finding"] {
        let path = tree.join("scripts").join(script);
        write(
            &path,
            &fs::read_to_string(Path::new(SCRIPTS).join(script)).expect("the script"),
        );
        fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).expect("executable");
    }
    write(
        &tree.join(".engineering/planning/story/probe-shows-the-defect.md"),
        finding,
    );
    git(&tree, &["init", "-q", "-b", "main"]);
    git(&tree, &["add", "-A"]);
    git(&tree, &["commit", "-q", "-m", "base"]);
    let baseline = git(&tree, &["rev-parse", "HEAD"]);
    git(&tree, &["checkout", "-q", "-b", "fix"]);
    write(&tree.join("fix.txt"), "the fix\n");
    git(&tree, &["add", "-A"]);
    git(&tree, &["commit", "-q", "-m", "fix"]);
    let candidate = git(&tree, &["rev-parse", "HEAD"]);
    git(&tree, &["checkout", "-q", "main"]);
    let protocol = LoadedProtocol::from_file(Path::new(SELF_IMPROVEMENT)).expect("loads");
    let mut case = CaseDir::open(
        &tree,
        protocol,
        "turn",
        "one improvement turn",
        &[("surface".to_owned(), "non_judge".to_owned())],
    )
    .expect("the case opens");
    case.open_turn(&TurnOpening {
        finding: FINDING.to_owned(),
        candidate: candidate.clone(),
        baseline: baseline.clone(),
    })
    .expect("the turn opens");
    Turn {
        _scratch: Scratch(dir.clone()),
        tree,
        baseline,
        candidate,
        state: dir.join("state"),
        cache: dir.join("cache"),
    }
}

/// A finding's body citing `source`.
fn finding(source: &str) -> String {
    format!(
        "# The probe shows the defect\n\nThe probe prints the defect.\n\n```expected_before\nscenario: 1\ncommand: \"$TAKT\" probe\nexit: 1\noutput: the defect is still here\nsource: {source}\n```\n"
    )
}

/// A saved scorecard of `revision`: scenario 1 run five times, each run's outcome `outcome` and
/// made with the binary `binary`.
fn scorecard(revision: &str, outcome: &str, binary: &str) -> Scorecard {
    Scorecard {
        format: takt::scorecard::FORMAT.to_owned(),
        revision: Some(revision.to_owned()),
        named_by: Some(takt::scorecard::Named::Build),
        metrics: takt::scorecard::METRICS
            .iter()
            .map(|(metric, version)| ((*metric).to_owned(), *version))
            .collect(),
        rule: Rule::default(),
        rows: (1..=5)
            .map(|n| Row {
                case: format!("scenario-1-trial-{n}"),
                scenario: Some("1".to_owned()),
                outcome: outcome.to_owned(),
                stop: "accepted".to_owned(),
                held_out: Some(HeldOut {
                    passed: outcome == "accepted",
                    result: if outcome == "accepted" {
                        "PASS".to_owned()
                    } else {
                        "FAIL".to_owned()
                    },
                }),
                steps: 4,
                tokens: 1000,
                refusals: 0,
                no_progress: 0,
                errors: 0,
                wall_ms: 1000,
                saturated: false,
                binary: None,
                opened_binary: Some(binary.to_owned()),
            })
            .collect(),
        no_record: Vec::new(),
        unstated: Default::default(),
    }
}

/// The promotion store under `state` holding one baseline record of `revision`, its scorecard
/// copy `card`.
fn promoted(state: &Path, revision: &str, tree: &Path, card: &Scorecard) {
    let store = state.join("takt/promotions");
    let text = serde_json::to_vec_pretty(card).expect("the scorecard serializes");
    fs::create_dir_all(&store).expect("the store");
    fs::write(store.join(format!("{revision}.scorecard.json")), &text).expect("the copy");
    let record = Promotion {
        format: takt::promotion::FORMAT.to_owned(),
        revision: revision.to_owned(),
        revision_stated_by: None,
        tree: git(tree, &["rev-parse", &format!("{revision}^{{tree}}")]),
        promoted_at: "2026-01-01T00:00:00Z".to_owned(),
        kind: Kind::Baseline,
        scorecard: revision.to_owned(),
        scorecard_file: PathBuf::from(format!("/srv/scorecards/scorecard-{revision}.json")),
        scorecard_sha256: format!("{:x}", Sha256::digest(&text)),
        against: None,
        rule: Rule::default(),
        worse: 0,
        binary: None,
        binary_sha256: None,
        runs: Vec::new(),
        previous_sha256: None,
        rejudges: None,
        replaces_sha256: None,
        cases: Vec::new(),
    };
    fs::write(
        store.join(format!("{revision}.json")),
        serde_json::to_vec_pretty(&record).expect("the record serializes"),
    )
    .expect("the record");
}

/// The candidate's saved scorecard, where `scorecard.compare` reads it.
fn saved(turn: &Turn, card: &Scorecard) -> PathBuf {
    let path = turn
        .state
        .join(format!("takt/scorecards/scorecard-{}.json", turn.candidate));
    write(
        &path,
        &serde_json::to_string_pretty(card).expect("the scorecard serializes"),
    );
    path
}

/// The kernel's opening method sets the turn's revisions, names the finding in `story.yaml`
/// and declares each of the turn's validators independent on `different_agent_run`, so no test
/// or operator writes `independence` by hand.
#[test]
fn the_turn_opens_with_its_revisions_and_its_validators_independent() {
    let turn = turn("opened", &finding("docs/runs/trial-1.md"));
    let case = CaseDir::resume(&turn.tree, "turn").expect("the case resumes");
    let base = git(&turn.tree, &["rev-parse", "main"]);
    assert_eq!(case.revision("finding"), Some(base));
    assert_eq!(case.revision("baseline"), Some(turn.baseline.clone()));
    assert_eq!(case.revision("candidate"), Some(turn.candidate.clone()));
    assert_eq!(case.revision("promotion"), Some(turn.baseline.clone()));
    assert_eq!(case.story().expect("story.yaml").as_deref(), Some(FINDING));
    let snapshot = fs::read_to_string(case.dir.join("case.yaml")).expect("case.yaml");
    let doc: serde_yaml_ng::Value = serde_yaml_ng::from_str(&snapshot).expect("yaml");
    let declared = doc["independence"].as_mapping().expect("independence");
    assert_eq!(
        declared.len(),
        takt::improvement::PRODUCERS.len(),
        "{snapshot}"
    );
    for producer in takt::improvement::PRODUCERS {
        assert_eq!(
            doc["independence"][producer],
            serde_yaml_ng::Value::from(vec!["different_agent_run"]),
            "{producer}: {snapshot}"
        );
    }
}

/// `finding.file` records `malformed` for a finding whose predicate cites a source the case's
/// base does not hold, and `filed` once it does.
#[test]
fn finding_file_records_malformed_for_a_source_the_base_does_not_hold() {
    let turn = turn("malformed", &finding("docs/runs/no-such-run.md"));
    let case = CaseDir::resume(&turn.tree, "turn").expect("the case resumes");
    let at = case.revision("finding").expect("the finding's revision");
    let validator = FindingFile(Checking::new(turn.cache.clone()));
    let observed = validator
        .observe(&turn.tree, &at, 1)
        .expect("the finding is judged");
    assert_eq!(observed.records.len(), 1, "{}", observed.summary);
    let record = &observed.records[0];
    assert_eq!(
        (
            record.kind.as_str(),
            record.result.as_deref(),
            record.subject.as_str(),
            record.subject_revision.as_str(),
            record.producer.as_str()
        ),
        (
            "finding_record",
            Some("malformed"),
            "finding",
            at.as_str(),
            "takt.finding-file"
        ),
        "{}",
        observed.summary
    );
    assert!(
        observed.summary.contains("docs/runs/no-such-run.md"),
        "{}",
        observed.summary
    );
}

/// With no saved scorecard of the candidate, `scorecard.compare` records nothing, waits, and
/// names the path it reads.
#[test]
fn scorecard_compare_waits_naming_the_path_while_the_candidate_has_none() {
    let turn = turn("waits", &finding("docs/runs/trial-1.md"));
    promoted(
        &turn.state,
        &turn.baseline,
        &turn.tree,
        &scorecard(&turn.baseline, "accepted", &turn.baseline),
    );
    let validator = ScorecardCompare::in_state(&turn.state);
    let (observed, waiting) = validator
        .observe_waiting(&turn.tree, &turn.candidate, 1)
        .expect("observes");
    assert!(observed.records.is_empty(), "{}", observed.summary);
    assert!(waiting, "{}", observed.summary);
    let path = turn
        .state
        .join(format!("takt/scorecards/scorecard-{}.json", turn.candidate));
    assert!(
        observed.summary.contains(&path.display().to_string()),
        "{}",
        observed.summary
    );
}

/// The candidate's saved scorecard is compared with the baseline's copy in the promotion
/// store: no scenario worse is `no_regression`, one worse is `regression`, each about the
/// candidate at its revision.
#[test]
fn scorecard_compare_judges_the_saved_scorecard_against_the_baselines_copy() {
    for (name, outcome, result) in [
        ("held", "accepted", "no_regression"),
        ("worse", "failed", "regression"),
    ] {
        let turn = turn(name, &finding("docs/runs/trial-1.md"));
        promoted(
            &turn.state,
            &turn.baseline,
            &turn.tree,
            &scorecard(&turn.baseline, "accepted", &turn.baseline),
        );
        saved(&turn, &scorecard(&turn.candidate, outcome, &turn.candidate));
        let validator = ScorecardCompare::in_state(&turn.state);
        let (observed, waiting) = validator
            .observe_waiting(&turn.tree, &turn.candidate, 1)
            .expect("observes");
        assert!(!waiting, "{name}: {}", observed.summary);
        assert_eq!(observed.records.len(), 1, "{name}: {}", observed.summary);
        let record = &observed.records[0];
        assert_eq!(
            (
                record.kind.as_str(),
                record.result.as_deref(),
                record.subject.as_str(),
                record.subject_revision.as_str(),
                record.producer.as_str()
            ),
            (
                "scorecard_result",
                Some(result),
                "candidate",
                turn.candidate.as_str(),
                "takt.scorecard-compare"
            ),
            "{name}: {}",
            observed.summary
        );
    }
}

/// A saved scorecard whose runs were made with another binary than the candidate's, or that is
/// named by another revision, is judged by nothing: no record, and no wait.
#[test]
fn scorecard_compare_records_nothing_for_runs_of_another_binary() {
    let turn = turn("other-binary", &finding("docs/runs/trial-1.md"));
    promoted(
        &turn.state,
        &turn.baseline,
        &turn.tree,
        &scorecard(&turn.baseline, "accepted", &turn.baseline),
    );
    saved(
        &turn,
        &scorecard(&turn.candidate, "accepted", &turn.baseline),
    );
    let validator = ScorecardCompare::in_state(&turn.state);
    let (observed, waiting) = validator
        .observe_waiting(&turn.tree, &turn.candidate, 1)
        .expect("observes");
    assert!(observed.records.is_empty(), "{}", observed.summary);
    assert!(!waiting, "{}", observed.summary);
    assert!(
        observed.summary.contains(&turn.baseline[..8]),
        "{}",
        observed.summary
    );
}

/// This tree's `scripts/check-finding` is a reviewed confined script: its sha256 is in
/// `takt::improvement::CONFINED_CHECK_FINDING_SHA256S`, the list a promoted baseline's script is
/// checked against. An edit to the script fails here until the list names its digest, so no
/// baseline promoted from this tree is refused for a script nobody reviewed, and none runs one.
#[test]
fn this_trees_check_finding_is_a_reviewed_confined_script() {
    let script = fs::read(Path::new(SCRIPTS).join("check-finding")).expect("the script");
    let digest = format!("{:x}", Sha256::digest(&script));
    assert!(
        takt::improvement::CONFINED_CHECK_FINDING_SHA256S.contains(&digest.as_str()),
        "scripts/check-finding hashes to {digest}, which the reviewed list {:?} does not name",
        takt::improvement::CONFINED_CHECK_FINDING_SHA256S
    );
}
