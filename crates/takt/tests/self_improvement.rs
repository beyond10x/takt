//! The `self.improvement/1` validators of `crates/takt/src/improvement.rs` on scratch turns
//! (story:self-improvement-protocol, part 1): `finding.file` judges the finding as the case's
//! base holds it with the base's `scripts/file-finding`, and `scorecard.compare` reads the
//! candidate's saved scorecard from `$XDG_STATE_HOME/takt/scorecards/scorecard-<candidate>.json`,
//! waiting and naming that path while it is missing, and compares it with the baseline's copy in
//! the promotion store under the promotion rule. `takt run` driving a turn to `unreproducible`
//! and `declined` is `crates/takt-cli/tests/self_improvement_cli.rs`. Part 2: the judge surface
//! a turn computes at open ([`takt::improvement::judge_surfaces`]), `candidate.merge` observed
//! on the local `main` and through a forge's report of it, and `binary.promote` observed on the
//! promotion store and the installed binary.

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
/// opening method, its surface `non_judge`, recording the forge `none`.
fn turn(name: &str, finding: &str) -> Turn {
    turn_with(name, finding, "non_judge", Some("none"))
}

/// [`turn`], the turn's surface `surface`, recording the forge `forge` or none.
fn turn_with(name: &str, finding: &str, surface: &str, forge: Option<&str>) -> Turn {
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
        &[("surface".to_owned(), surface.to_owned())],
    )
    .expect("the case opens");
    case.open_turn(&TurnOpening {
        finding: FINDING.to_owned(),
        candidate: candidate.clone(),
        baseline: baseline.clone(),
        surface: surface.to_owned(),
        title: "The probe shows the defect".to_owned(),
        judge_sha256: "0".repeat(64),
        install_root: None,
        forge: forge.map(str::to_owned),
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
                completed: None,
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
        series_start: None,
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
    assert_eq!(case.revision("promotion"), Some(turn.candidate.clone()));
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

/// `card` with its rows of scenario 1 trials `trials` only, then, for each trial of `added`, a
/// copy of its first row as a case of scenario 2.
fn reshaped(mut card: Scorecard, trials: usize, added: usize) -> Scorecard {
    let first = card.rows[0].clone();
    card.rows.truncate(trials);
    card.rows.extend((1..=added).map(|n| Row {
        case: format!("scenario-2-trial-{n}"),
        scenario: Some("2".to_owned()),
        ..first.clone()
    }));
    card
}

/// story:the-judge-requires-the-whole-corpus: `scorecard.compare` with a baseline copy of
/// `baseline` (scenario 1 cases, scenario 2 cases) and a candidate scorecard of `candidate`
/// records nothing, never `no_regression`, and does not wait; the summary holds `named`.
fn compare_records_nothing(
    name: &str,
    baseline: (usize, usize),
    candidate: (usize, usize),
    named: &str,
) {
    let turn = turn(name, &finding("docs/runs/trial-1.md"));
    promoted(
        &turn.state,
        &turn.baseline,
        &turn.tree,
        &reshaped(
            scorecard(&turn.baseline, "accepted", &turn.baseline),
            baseline.0,
            baseline.1,
        ),
    );
    saved(
        &turn,
        &reshaped(
            scorecard(&turn.candidate, "accepted", &turn.candidate),
            candidate.0,
            candidate.1,
        ),
    );
    let validator = ScorecardCompare::in_state(&turn.state);
    let (observed, waiting) = validator
        .observe_waiting(&turn.tree, &turn.candidate, 1)
        .expect("observes");
    assert!(
        observed.records.is_empty(),
        "{name}: {:?}: {}",
        observed
            .records
            .iter()
            .map(|record| record.result.clone())
            .collect::<Vec<_>>(),
        observed.summary
    );
    assert!(!waiting, "{name}: {}", observed.summary);
    assert!(
        observed.summary.contains(named) && observed.summary.contains("nothing recorded"),
        "{name}: {named} in {}",
        observed.summary
    );
}

/// story:the-judge-requires-the-whole-corpus: no scenario case on either side is no corpus.
#[test]
fn scorecard_compare_records_nothing_for_empty_against_empty() {
    compare_records_nothing("corpus-empty", (0, 0), (0, 0), "no scenario case");
}

/// story:the-judge-requires-the-whole-corpus: a scenario only the candidate has, with the rule's
/// five cases, is not in the baseline's corpus.
#[test]
fn scorecard_compare_records_nothing_for_a_scenario_only_the_candidate_has() {
    compare_records_nothing("corpus-candidate-only", (5, 0), (5, 5), "scenario 2");
}

/// story:the-judge-requires-the-whole-corpus: a scenario only the baseline has, with the rule's
/// five cases, is missing from the candidate.
#[test]
fn scorecard_compare_records_nothing_for_a_scenario_only_the_baseline_has() {
    compare_records_nothing("corpus-baseline-only", (5, 5), (5, 0), "scenario 2");
}

/// story:the-judge-requires-the-whole-corpus: a scenario with fewer candidate cases than the
/// rule's trials gets no verdict.
#[test]
fn scorecard_compare_records_nothing_for_fewer_cases_than_the_trials() {
    compare_records_nothing("corpus-too-few", (5, 0), (3, 0), "scenario 1");
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

/// A scratch repository for the judge surface: `M` holds the allow-list (`fix.txt`, `docs/**`),
/// `tools/judge.sh` and `fix.txt`; `B`, the baseline, tightens `tools/judge.sh` on top of it.
/// Answers the scratch directory, the repository, `M` and `B`.
fn surface_repo(name: &str) -> (Scratch, PathBuf, String, String) {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!(
        "self-improvement-surface-{}-{name}",
        std::process::id()
    ));
    if dir.exists() {
        fs::remove_dir_all(&dir).expect("a stale scratch tree is removed");
    }
    let repo = dir.join("repo");
    write(
        &repo.join(".engineering/judge-allow.yaml"),
        "paths:\n  - fix.txt\n  - docs/**\n",
    );
    write(&repo.join("tools/judge.sh"), "#!/bin/sh\necho v1\n");
    write(&repo.join("fix.txt"), "a\n");
    git(&repo, &["init", "-q", "-b", "main"]);
    git(&repo, &["add", "-A"]);
    git(&repo, &["commit", "-q", "-m", "M"]);
    let m = git(&repo, &["rev-parse", "HEAD"]);
    write(&repo.join("tools/judge.sh"), "#!/bin/sh\necho v2 strict\n");
    git(&repo, &["commit", "-q", "-am", "B: tighten the judge"]);
    let b = git(&repo, &["rev-parse", "HEAD"]);
    (Scratch(dir), repo, m, b)
}

/// A commit on a new branch `name` off `at` that writes each `(path, text)`; its id.
fn commit_on(repo: &Path, name: &str, at: &str, files: &[(&str, &str)]) -> String {
    git(repo, &["checkout", "-q", "-b", name, at]);
    for (path, text) in files {
        write(&repo.join(path), text);
    }
    git(repo, &["add", "-A"]);
    git(repo, &["commit", "-q", "-m", name]);
    git(repo, &["rev-parse", "HEAD"])
}

/// The reviewer's case: a candidate branched before the baseline's judge change touches only an
/// allowed path, yet its tree differs from the baseline's at `tools/judge.sh`, so promoting it
/// would roll the judge change back: its surface is judge.
#[test]
fn a_candidate_branched_before_a_judge_change_rolls_it_back_and_is_judge() {
    let (_scratch, repo, m, b) = surface_repo("stale");
    let c = commit_on(&repo, "fix", &m, &[("fix.txt", "b\n")]);
    let surfaces = takt::improvement::judge_surfaces(&repo, &b, &c).expect("computes");
    assert_eq!(surfaces, ["tools/judge.sh"]);
}

/// The candidate's own change is judged besides what the promotion installs: a candidate that
/// changes `tools/judge.sh` exactly as the baseline already does is judge, though the baseline's
/// tree and the candidate's agree there.
#[test]
fn a_judge_path_the_candidate_changes_is_judge_where_the_baseline_matches_it() {
    let (_scratch, repo, m, b) = surface_repo("same-change");
    let c = commit_on(
        &repo,
        "fix",
        &m,
        &[("tools/judge.sh", "#!/bin/sh\necho v2 strict\n")],
    );
    assert_eq!(git(&repo, &["diff", "--name-only", &b, &c]), "");
    let surfaces = takt::improvement::judge_surfaces(&repo, &b, &c).expect("computes");
    assert_eq!(surfaces, ["tools/judge.sh"]);
}

/// The reviewer's controls: a rename out of a judge path names the judge path; a link at an
/// allowed path, a change to the allow-list and a judge change hidden behind a replace ref are
/// judge; an executable bit on an allowed file is not.
#[test]
fn the_surface_controls_hold() {
    let (_scratch, repo, _m, b) = surface_repo("controls");
    git(&repo, &["checkout", "-q", "-b", "rename", &b]);
    fs::create_dir_all(repo.join("docs")).expect("docs");
    git(&repo, &["mv", "tools/judge.sh", "docs/judge.sh"]);
    git(&repo, &["commit", "-q", "-m", "rename"]);
    let c = git(&repo, &["rev-parse", "HEAD"]);
    let surfaces = takt::improvement::judge_surfaces(&repo, &b, &c).expect("computes");
    assert_eq!(surfaces, ["tools/judge.sh"], "rename");
    git(&repo, &["checkout", "-q", "-b", "link", &b]);
    fs::create_dir_all(repo.join("docs")).expect("docs");
    std::os::unix::fs::symlink("../tools/judge.sh", repo.join("docs/judge")).expect("a link");
    git(&repo, &["add", "-A"]);
    git(&repo, &["commit", "-q", "-m", "link"]);
    let c = git(&repo, &["rev-parse", "HEAD"]);
    let surfaces = takt::improvement::judge_surfaces(&repo, &b, &c).expect("computes");
    assert_eq!(surfaces, ["docs/judge"], "link");
    let c = commit_on(
        &repo,
        "allow",
        &b,
        &[(".engineering/judge-allow.yaml", "paths:\n  - '**'\n")],
    );
    let surfaces = takt::improvement::judge_surfaces(&repo, &b, &c).expect("computes");
    assert_eq!(surfaces, [".engineering/judge-allow.yaml"], "allow-list");
    git(&repo, &["checkout", "-q", "-b", "exec", &b]);
    fs::set_permissions(repo.join("fix.txt"), fs::Permissions::from_mode(0o755))
        .expect("executable");
    git(&repo, &["commit", "-q", "-am", "exec"]);
    let c = git(&repo, &["rev-parse", "HEAD"]);
    let surfaces = takt::improvement::judge_surfaces(&repo, &b, &c).expect("computes");
    assert!(surfaces.is_empty(), "exec bit: {surfaces:?}");
    let c = commit_on(
        &repo,
        "weaken",
        &b,
        &[("tools/judge.sh", "#!/bin/sh\nexit 0\n")],
    );
    let innocent = commit_on(&repo, "innocent", &b, &[("fix.txt", "c\n")]);
    git(&repo, &["replace", &c, &innocent]);
    let surfaces = takt::improvement::judge_surfaces(&repo, &b, &c).expect("computes");
    assert_eq!(surfaces, ["tools/judge.sh"], "replace ref");
}

/// The delegation record is the owner's alone: a candidate that changes it is judge even where
/// the allow-list admits it.
#[test]
fn a_change_to_the_delegation_record_is_judge_whatever_the_allow_list_admits() {
    let (_scratch, repo, _m, b) = surface_repo("owner-only");
    let wide = commit_on(
        &repo,
        "wide",
        &b,
        &[(
            ".engineering/judge-allow.yaml",
            "paths:\n  - fix.txt\n  - .engineering/**\n",
        )],
    );
    let c = commit_on(
        &repo,
        "delegate",
        &wide,
        &[(".engineering/delegations.yaml", "delegations: []\n")],
    );
    let surfaces = takt::improvement::judge_surfaces(&repo, &wide, &c).expect("computes");
    assert_eq!(surfaces, [".engineering/delegations.yaml"]);
}

/// The allow-list is the baseline's: a candidate that widens it to admit its own judge change
/// still touches that judge path.
#[test]
fn the_allow_list_is_read_at_the_baseline_not_the_candidate() {
    let (_scratch, repo, _m, b) = surface_repo("allow-at-baseline");
    let c = commit_on(
        &repo,
        "widen",
        &b,
        &[
            (
                ".engineering/judge-allow.yaml",
                "paths:\n  - fix.txt\n  - tools/**\n",
            ),
            ("tools/judge.sh", "#!/bin/sh\nexit 0\n"),
        ],
    );
    let surfaces = takt::improvement::judge_surfaces(&repo, &b, &c).expect("computes");
    assert!(
        surfaces.iter().any(|path| path == "tools/judge.sh"),
        "{surfaces:?}"
    );
}

/// With two merge bases, a criss-cross, the change from each counts: `tools/a.sh` changed on one
/// side and `tools/b.sh` on the other, each put back in both merges, show only from the merge
/// base whose side changed it, and both are judge.
#[test]
fn every_merge_base_decides_the_surface() {
    let (_scratch, repo, m, _b) = surface_repo("criss-cross");
    git(&repo, &["checkout", "-q", "-b", "base", &m]);
    write(&repo.join("tools/a.sh"), "a1\n");
    write(&repo.join("tools/b.sh"), "b1\n");
    git(&repo, &["add", "-A"]);
    git(&repo, &["commit", "-q", "-m", "O"]);
    let o = git(&repo, &["rev-parse", "HEAD"]);
    let x = commit_on(&repo, "x", &o, &[("tools/a.sh", "a2\n")]);
    let y = commit_on(&repo, "y", &o, &[("tools/b.sh", "b2\n")]);
    // Each merge takes neither side's change: both files as they were at O.
    let merge = |name: &str, first: &str, second: &str| {
        git(&repo, &["checkout", "-q", "-b", name, first]);
        git(&repo, &["merge", "-q", "--no-ff", "--no-commit", second]);
        write(&repo.join("tools/a.sh"), "a1\n");
        write(&repo.join("tools/b.sh"), "b1\n");
        git(&repo, &["add", "-A"]);
        git(&repo, &["commit", "-q", "-m", name]);
        git(&repo, &["rev-parse", "HEAD"])
    };
    let baseline = merge("left", &x, &y);
    let merged = merge("right", &y, &x);
    let candidate = commit_on(&repo, "fix", &merged, &[("fix.txt", "fixed\n")]);
    let meets = git(&repo, &["merge-base", "--all", &baseline, &candidate]);
    assert_eq!(meets.lines().count(), 2, "{meets}");
    let mut surfaces =
        takt::improvement::judge_surfaces(&repo, &baseline, &candidate).expect("computes");
    surfaces.sort();
    assert_eq!(surfaces, ["tools/a.sh", "tools/b.sh"]);
}

/// Grants `capability` to the turn's case at the candidate revision, as `takt approve` does.
fn granted(turn: &Turn, capability: &str) {
    granted_by(turn, capability, "human:owner");
}

/// Grants `capability` to the turn's case at the candidate revision by `approver`, in place of
/// any grant of it before.
fn granted_by(turn: &Turn, capability: &str, approver: &str) {
    let dir = turn.tree.join(".engineering/cases/turn");
    let mut authority = takt::case::Authority::read(&dir)
        .expect("the authority reads")
        .unwrap_or_default();
    authority
        .decisions
        .retain(|decision| decision.capability != capability);
    authority.decisions.push(takt::case::AuthorityDecision {
        capability: capability.to_owned(),
        decision: takt::case::Grant::Granted,
        approver: approver.to_owned(),
        granted_at: "2026-01-01T00:00:00Z".to_owned(),
        revision: Some(turn.candidate.clone()),
        mainline_tip: Some(git(&turn.tree, &["rev-parse", "main"])),
    });
    authority.write(&dir).expect("the authority is written");
}

/// What `validator` observes at the candidate: its records' results, whether it waits, and its
/// summary.
fn observed(validator: &dyn Validator, turn: &Turn) -> (Vec<String>, bool, String) {
    let (observation, waiting) = validator
        .observe_waiting(&turn.tree, &turn.candidate, 1)
        .expect("observes");
    let results = observation
        .records
        .iter()
        .map(|record| record.result.clone().unwrap_or_default())
        .collect();
    (results, waiting, observation.summary)
}

/// `candidate.merge` on the local `main`: nothing without the case's grant, even with `main` at
/// the candidate; granted, it waits while `main` does not hold the candidate and records
/// `merged` once it does, retaining `forge: none` and the tip.
#[test]
fn the_merge_is_observed_on_local_main_only_with_the_grant() {
    let turn = turn("merge-local", &finding("docs/runs/trial-1.md"));
    let validator = takt::improvement::CandidateMerged::new(takt::improvement::Mainline::Local);
    git(&turn.tree, &["branch", "-f", "main", &turn.candidate]);
    let (results, waiting, summary) = observed(&validator, &turn);
    assert!(results.is_empty() && !waiting, "{summary}");
    git(&turn.tree, &["branch", "-f", "main", &turn.baseline]);
    granted(&turn, "repository.merge");
    let (results, waiting, summary) = observed(&validator, &turn);
    assert!(results.is_empty() && waiting, "{summary}");
    assert!(summary.contains(&turn.baseline[..8]), "{summary}");
    git(&turn.tree, &["branch", "-f", "main", &turn.candidate]);
    let (observation, waiting) = validator
        .observe_waiting(&turn.tree, &turn.candidate, 1)
        .expect("observes");
    assert!(!waiting, "{}", observation.summary);
    assert_eq!(observation.records.len(), 1, "{}", observation.summary);
    assert_eq!(observation.records[0].result.as_deref(), Some("merged"));
    assert_eq!(observation.records[0].producer, "takt.candidate-merge");
    let retained = String::from_utf8(observation.artifact.expect("retained").bytes).expect("text");
    assert!(retained.contains("forge: none"), "{retained}");
    assert!(
        retained.contains(&format!("tip: {}", turn.candidate)),
        "{retained}"
    );
}

/// `candidate.merge` through a forge (the in-memory double, whose `main` is the tree's): it waits
/// while the forge's `main` does not hold the candidate, records `merged` once it does, naming
/// the forge, and a tip the forge cannot report is an error, never a record.
#[test]
fn the_merge_is_observed_through_the_forges_report_of_main() {
    let turn = turn("merge-forge", &finding("docs/runs/trial-1.md"));
    let forge: std::rc::Rc<dyn takt::forge::Forge> =
        std::rc::Rc::new(takt::forge::Memory::default());
    let validator =
        takt::improvement::CandidateMerged::new(takt::improvement::Mainline::Forge(forge));
    granted(&turn, "repository.merge");
    let (results, waiting, summary) = observed(&validator, &turn);
    assert!(results.is_empty() && waiting, "{summary}");
    git(&turn.tree, &["branch", "-f", "main", &turn.candidate]);
    let (observation, _) = validator
        .observe_waiting(&turn.tree, &turn.candidate, 1)
        .expect("observes");
    assert_eq!(observation.records.len(), 1, "{}", observation.summary);
    let retained = String::from_utf8(observation.artifact.expect("retained").bytes).expect("text");
    assert!(retained.contains("forge: memory"), "{retained}");
    git(&turn.tree, &["branch", "-D", "main"]);
    let error = validator
        .observe_waiting(&turn.tree, &turn.candidate, 1)
        .expect_err("no tip, no observation");
    assert!(format!("{error:#}").contains("main"), "{error:#}");
}

/// `candidate.merge_judge_surface` holds its grant to story:judge-guard's rule: an agent role's
/// grant records nothing while `main` records no delegation to it, and `merged` once `main`
/// holds an unrevoked delegation of `repository.merge.judge_surface` to that role; the owner's
/// grant needs none. Each grant is given while `main` is at the baseline, and `main` moves to
/// the candidate after it, as a merge the grant authorises does.
#[test]
fn a_judge_surface_merge_holds_only_the_owners_grant_or_a_delegated_roles() {
    let turn = turn("merge-judge-surface", &finding("docs/runs/trial-1.md"));
    let validator =
        takt::improvement::CandidateMerged::judge_surface(takt::improvement::Mainline::Local);
    granted_by(&turn, "repository.merge.judge_surface", "agent:developer");
    git(&turn.tree, &["branch", "-f", "main", &turn.candidate]);
    let (results, waiting, summary) = observed(&validator, &turn);
    assert!(results.is_empty() && !waiting, "undelegated: {summary}");
    assert!(
        summary.contains(".engineering/delegations.yaml"),
        "{summary}"
    );
    // The owner delegates to the role on `main`, above the candidate.
    let scratch = turn
        .tree
        .parent()
        .expect("a parent")
        .join("delegations.yaml");
    fs::write(
        &scratch,
        "delegations:\n  - owner: human:owner\n    role: agent:developer\n    capability: repository.merge.judge_surface\n    given_at: 2026-01-01T00:00:00Z\n",
    )
    .expect("the delegation record");
    let blob = git(
        &turn.tree,
        &["hash-object", "-w", "--", scratch.to_str().expect("utf-8")],
    );
    let index = turn
        .tree
        .parent()
        .expect("a parent")
        .join("delegation-index");
    let with_index = |args: &[&str]| {
        let output = Command::new("git")
            .args(["-c", "user.name=t", "-c", "user.email=t@localhost"])
            .args(args)
            .env("GIT_INDEX_FILE", &index)
            .current_dir(&turn.tree)
            .output()
            .expect("git runs");
        assert!(output.status.success(), "git {args:?}");
        String::from_utf8_lossy(&output.stdout).trim().to_owned()
    };
    with_index(&["read-tree", &turn.candidate]);
    with_index(&[
        "update-index",
        "--add",
        "--cacheinfo",
        &format!("100644,{blob},.engineering/delegations.yaml"),
    ]);
    let tree = with_index(&["write-tree"]);
    let delegated = git(
        &turn.tree,
        &[
            "commit-tree",
            &tree,
            "-p",
            &turn.candidate,
            "-m",
            "delegate",
        ],
    );
    git(&turn.tree, &["branch", "-f", "main", &delegated]);
    let (results, _, summary) = observed(&validator, &turn);
    assert_eq!(results, ["merged"], "delegated: {summary}");
    git(&turn.tree, &["branch", "-f", "main", &turn.baseline]);
    granted_by(&turn, "repository.merge.judge_surface", "human:owner");
    git(&turn.tree, &["branch", "-f", "main", &turn.candidate]);
    let (results, _, summary) = observed(&validator, &turn);
    assert_eq!(results, ["merged"], "the owner: {summary}");
}

/// The owner's grant of the turn's judge-surface merge written into its `authority.yaml` as
/// `takt approve` writes it: given at `granted_at`, with `tip` the commit `main` was at when it
/// was given, or none recorded. Written as the file's text, the one grant the case holds.
fn judge_grant(turn: &Turn, granted_at: &str, tip: Option<&str>) {
    let dir = turn.tree.join(".engineering/cases/turn");
    let mut text = format!(
        "- capability: repository.merge.judge_surface\n  decision: granted\n  approver: human:owner\n  granted_at: '{granted_at}'\n  revision: {}\n",
        turn.candidate
    );
    if let Some(tip) = tip {
        text.push_str(&format!("  mainline_tip: {tip}\n"));
    }
    fs::write(dir.join("authority.yaml"), text).expect("the authority is written");
}

/// A commit of the tree `tree` with `parents` and `message`, committed at `committed` by the
/// committer's clock and authored at `authored` by the author's; its id.
fn commit_tree(
    turn: &Turn,
    tree: &str,
    parents: &[&str],
    message: &str,
    committed: &str,
    authored: &str,
) -> String {
    let mut args = vec!["commit-tree", tree];
    for parent in parents {
        args.extend(["-p", parent]);
    }
    args.extend(["-m", message]);
    let output = Command::new("git")
        .args(["-c", "user.name=t", "-c", "user.email=t@localhost"])
        .args(&args)
        .env("GIT_COMMITTER_DATE", committed)
        .env("GIT_AUTHOR_DATE", authored)
        .current_dir(&turn.tree)
        .output()
        .expect("git runs");
    assert!(output.status.success(), "git {args:?}");
    String::from_utf8_lossy(&output.stdout).trim().to_owned()
}

/// The first real turn's shape: the candidate reached `main` by a fast-forward before the owner
/// granted its judge-surface merge, so `main` held it when the grant was given. The grant
/// authorises no merge already made: nothing is recorded, and the run does not wait, and the
/// summary names the merge (the candidate itself, a fast-forward having no merge commit) with
/// its committer date, by the committer's clock, and the grant's instant. The grant's instant is
/// set before the candidate's committer date, so a check by date would record `merged`.
#[test]
fn a_candidate_merged_before_the_grant_is_refused_naming_both_instants() {
    let turn = turn("merge-before-grant", &finding("docs/runs/trial-1.md"));
    let validator =
        takt::improvement::CandidateMerged::judge_surface(takt::improvement::Mainline::Local);
    git(&turn.tree, &["branch", "-f", "main", &turn.candidate]);
    let granted_at = "2000-01-01T00:00:00Z";
    judge_grant(&turn, granted_at, Some(&turn.candidate));
    let (results, waiting, summary) = observed(&validator, &turn);
    assert!(results.is_empty() && !waiting, "{summary}");
    let committed = git(&turn.tree, &["log", "-1", "--format=%cI", &turn.candidate]);
    assert!(
        summary.contains(granted_at),
        "the grant's instant: {summary}"
    );
    assert!(
        summary.contains(&committed),
        "the merge's committer date {committed}: {summary}"
    );
    assert!(summary.contains("committer's clock"), "{summary}");
    assert!(summary.contains(&turn.candidate[..8]), "{summary}");
}

/// A candidate merged into `main` by a merge commit, `main` then moved on by one more commit, all
/// before the grant: the refusal names the merge commit, the first commit of `main`'s
/// first-parent chain that holds the candidate, not the tip and not the candidate, with the
/// merge commit's committer date, not its author date, which differs.
#[test]
fn the_refusal_names_the_first_commit_of_mains_first_parent_chain_that_holds_the_candidate() {
    let turn = turn(
        "merge-commit-before-grant",
        &finding("docs/runs/trial-1.md"),
    );
    let validator =
        takt::improvement::CandidateMerged::judge_surface(takt::improvement::Mainline::Local);
    let tree = git(
        &turn.tree,
        &["rev-parse", &format!("{}^{{tree}}", turn.candidate)],
    );
    let merge = commit_tree(
        &turn,
        &tree,
        &[&turn.baseline, &turn.candidate],
        "merge the candidate",
        "2026-10-19T18:02:18Z",
        "2026-10-19T09:30:00Z",
    );
    let after = commit_tree(
        &turn,
        &tree,
        &[&merge],
        "after",
        "2026-10-19T18:04:00Z",
        "2026-10-19T09:45:00Z",
    );
    git(&turn.tree, &["branch", "-f", "main", &after]);
    judge_grant(&turn, "2026-10-19T18:05:26Z", Some(&after));
    let (results, waiting, summary) = observed(&validator, &turn);
    assert!(results.is_empty() && !waiting, "{summary}");
    assert!(summary.contains(&merge[..8]), "the merge commit: {summary}");
    let committed = git(&turn.tree, &["log", "-1", "--format=%cI", &merge]);
    let authored = git(&turn.tree, &["log", "-1", "--format=%aI", &merge]);
    assert!(committed.starts_with("2026-10-19T18:02:18"), "{committed}");
    assert!(authored.starts_with("2026-10-19T09:30:00"), "{authored}");
    assert!(
        summary.contains(&committed),
        "the merge's committer date {committed}: {summary}"
    );
    assert!(
        !summary.contains(&authored),
        "not its author date {authored}: {summary}"
    );
    assert!(summary.contains("2026-10-19T18:05:26Z"), "{summary}");
}

/// The incident's fast-forward shape after the grant: the owner grants while `main` is at the
/// baseline, and `main` then moves to the candidate. It waits until then and records `merged`
/// after, though the candidate's committer date precedes the grant's instant: no date decides it.
#[test]
fn a_candidate_merged_after_the_grant_records_merged() {
    let turn = turn("merge-after-grant", &finding("docs/runs/trial-1.md"));
    let validator =
        takt::improvement::CandidateMerged::judge_surface(takt::improvement::Mainline::Local);
    judge_grant(&turn, "2999-01-01T00:00:00Z", Some(&turn.baseline));
    let (results, waiting, summary) = observed(&validator, &turn);
    assert!(results.is_empty() && waiting, "{summary}");
    git(&turn.tree, &["branch", "-f", "main", &turn.candidate]);
    let (results, waiting, summary) = observed(&validator, &turn);
    assert_eq!(results, ["merged"], "{summary}");
    assert!(!waiting, "{summary}");
}

/// A judge-surface grant that records no tip of `main` read when it was given authorises no
/// merge: whether the candidate reached `main` after it is not known, so nothing is recorded
/// though `main` moved to the candidate after the grant.
#[test]
fn a_judge_surface_grant_with_no_mainline_tip_is_refused() {
    let turn = turn("merge-no-anchor", &finding("docs/runs/trial-1.md"));
    let validator =
        takt::improvement::CandidateMerged::judge_surface(takt::improvement::Mainline::Local);
    judge_grant(&turn, "2026-10-19T18:05:26Z", None);
    git(&turn.tree, &["branch", "-f", "main", &turn.candidate]);
    let (results, waiting, summary) = observed(&validator, &turn);
    assert!(results.is_empty() && !waiting, "{summary}");
    assert!(summary.contains("records no tip of main"), "{summary}");
}

/// A judge-surface grant whose recorded tip of `main` does not arrive from the repository as its
/// id authorises no merge: nothing is recorded, naming the tip.
#[test]
fn a_judge_surface_grant_whose_mainline_tip_cannot_be_fetched_is_refused() {
    let turn = turn("merge-anchor-missing", &finding("docs/runs/trial-1.md"));
    let validator =
        takt::improvement::CandidateMerged::judge_surface(takt::improvement::Mainline::Local);
    let missing = "1".repeat(40);
    judge_grant(&turn, "2026-10-19T18:05:26Z", Some(&missing));
    git(&turn.tree, &["branch", "-f", "main", &turn.candidate]);
    let (results, waiting, summary) = observed(&validator, &turn);
    assert!(results.is_empty() && !waiting, "{summary}");
    assert!(summary.contains(&missing[..8]), "{summary}");
}

/// A judge-surface grant whose recorded tip of `main` is not on `main` now, neither its tip nor
/// an ancestor of it (here a commit with no parent, which holds no candidate), authorises no
/// merge: nothing is recorded though `main` moved to the candidate, naming the recorded tip.
#[test]
fn a_judge_surface_grant_whose_mainline_tip_is_off_the_mainline_is_refused() {
    let turn = turn("merge-anchor-off-main", &finding("docs/runs/trial-1.md"));
    let validator =
        takt::improvement::CandidateMerged::judge_surface(takt::improvement::Mainline::Local);
    let tree = git(
        &turn.tree,
        &["rev-parse", &format!("{}^{{tree}}", turn.baseline)],
    );
    let orphan = commit_tree(
        &turn,
        &tree,
        &[],
        "orphan",
        "2026-10-19T18:00:00Z",
        "2026-10-19T18:00:00Z",
    );
    judge_grant(&turn, "2026-10-19T18:05:26Z", Some(&orphan));
    git(&turn.tree, &["branch", "-f", "main", &turn.candidate]);
    let (results, waiting, summary) = observed(&validator, &turn);
    assert!(results.is_empty() && !waiting, "{summary}");
    assert!(summary.contains(&orphan[..8]), "{summary}");
}

/// The turn's records up to the judge-surface merge's gate, as its validators write them, and
/// the decision that finds `candidate.merge_judge_surface` approval-required.
fn at_the_judge_gate(turn: &Turn) {
    let mut case = CaseDir::resume(&turn.tree, "turn").expect("the case resumes");
    for (n, kind, result, subject, producer) in [
        (1, "finding_record", "filed", "finding", FindingFile::NAME),
        (
            2,
            "scenario_result",
            "reproduced",
            "baseline",
            takt::improvement::ScenarioReproduce::NAME,
        ),
        (
            3,
            "scenario_result",
            "not_reproduced",
            "candidate",
            takt::improvement::ScenarioVerify::NAME,
        ),
    ] {
        let revision = case.revision(subject).expect("the subject has a revision");
        case.add_evidence(&takt::evidence::Record::new(
            format!("e{n}"),
            kind,
            Some(result.to_owned()),
            subject,
            revision,
            producer,
        ))
        .expect("the record is written");
    }
    let decision = case.evaluate(0).expect("evaluates");
    assert_eq!(
        decision
            .actions
            .get("candidate.merge_judge_surface")
            .map(String::as_str),
        Some("approval-required"),
        "{}",
        decision.json
    );
}

/// `takt approve`'s grant of a turn's judge-surface merge records the tip of `main` read through
/// the forge the turn recorded when it opened (`none`: the local `main`); a turn that recorded no
/// forge gets a grant with no tip, which records no merge though `main` moves to the candidate
/// after it.
#[test]
fn a_judge_surface_grant_records_the_mainline_tip_through_the_turns_forge() {
    let turn = turn_with(
        "grant-anchor",
        &finding("docs/runs/trial-1.md"),
        "judge",
        Some("none"),
    );
    at_the_judge_gate(&turn);
    let main = git(&turn.tree, &["rev-parse", "main"]);
    takt::case::grant(
        &turn.tree,
        "turn",
        "repository.merge.judge_surface",
        "human:owner",
    )
    .expect("granted");
    let dir = turn.tree.join(".engineering/cases/turn");
    let authority = takt::case::Authority::read(&dir)
        .expect("the authority reads")
        .expect("an authority");
    let decision = authority
        .decision("repository.merge.judge_surface")
        .expect("the grant");
    assert_eq!(decision.mainline_tip.as_deref(), Some(main.as_str()));

    let turn = turn_with(
        "grant-no-forge",
        &finding("docs/runs/trial-1.md"),
        "judge",
        None,
    );
    at_the_judge_gate(&turn);
    takt::case::grant(
        &turn.tree,
        "turn",
        "repository.merge.judge_surface",
        "human:owner",
    )
    .expect("granted");
    let dir = turn.tree.join(".engineering/cases/turn");
    let authority = takt::case::Authority::read(&dir)
        .expect("the authority reads")
        .expect("an authority");
    let decision = authority
        .decision("repository.merge.judge_surface")
        .expect("the grant");
    assert_eq!(decision.mainline_tip, None);
    git(&turn.tree, &["branch", "-f", "main", &turn.candidate]);
    let validator =
        takt::improvement::CandidateMerged::judge_surface(takt::improvement::Mainline::Local);
    let (results, waiting, summary) = observed(&validator, &turn);
    assert!(results.is_empty() && !waiting, "{summary}");
    assert!(summary.contains("records no tip of main"), "{summary}");
}

/// Writes the promotion store's record of `revision` of `kind`, chained to the file `previous`
/// when given, with `binary_sha256` and a scorecard copy.
fn record(
    state: &Path,
    tree: &Path,
    revision: &str,
    kind: Kind,
    binary_sha256: Option<String>,
    previous: Option<&Path>,
) -> PathBuf {
    let store = state.join("takt/promotions");
    let card = scorecard(revision, "accepted", revision);
    let text = serde_json::to_vec_pretty(&card).expect("the scorecard serializes");
    fs::create_dir_all(&store).expect("the store");
    fs::write(store.join(format!("{revision}.scorecard.json")), &text).expect("the copy");
    let promotion = Promotion {
        format: takt::promotion::FORMAT.to_owned(),
        revision: revision.to_owned(),
        revision_stated_by: None,
        tree: git(tree, &["rev-parse", &format!("{revision}^{{tree}}")]),
        promoted_at: if previous.is_some() {
            "2026-01-02T00:00:00Z"
        } else {
            "2026-01-01T00:00:00Z"
        }
        .to_owned(),
        kind,
        scorecard: revision.to_owned(),
        scorecard_file: PathBuf::from(format!("/srv/scorecards/scorecard-{revision}.json")),
        scorecard_sha256: format!("{:x}", Sha256::digest(&text)),
        against: None,
        rule: Rule::default(),
        worse: 0,
        binary: binary_sha256
            .as_ref()
            .map(|_| PathBuf::from("/srv/install/bin/takt")),
        binary_sha256,
        runs: Vec::new(),
        previous_sha256: previous
            .map(|path| format!("{:x}", Sha256::digest(fs::read(path).expect("previous")))),
        rejudges: None,
        replaces_sha256: None,
        cases: Vec::new(),
        series_start: None,
    };
    let path = store.join(format!("{revision}.json"));
    fs::write(
        &path,
        serde_json::to_vec_pretty(&promotion).expect("the record serializes"),
    )
    .expect("the record");
    path
}

/// `binary.promote` records `promoted` only for a promotion record of the candidate whose
/// digest is the installed binary's, and only with the case's grant: not for a baseline record
/// of the candidate, not for a promotion of the candidate whose binary is not the one installed,
/// not while the latest record is another revision's, even a promotion whose binary is the one
/// installed, and not without the grant.
#[test]
fn binary_promote_observes_only_a_promotion_of_the_candidate_that_is_installed() {
    let turn = turn("promote", &finding("docs/runs/trial-1.md"));
    let root = turn.state.join("install");
    write(
        &root.join("bin/takt"),
        "#!/bin/sh\necho the candidate's takt\n",
    );
    let installed = format!(
        "{:x}",
        Sha256::digest(fs::read(root.join("bin/takt")).expect("installed"))
    );
    let validator =
        takt::improvement::BinaryPromote::in_state(&turn.state).with_install_root(&root);
    let base = record(
        &turn.state,
        &turn.tree,
        &turn.baseline,
        Kind::Baseline,
        None,
        None,
    );
    let promoted = record(
        &turn.state,
        &turn.tree,
        &turn.candidate,
        Kind::Promotion,
        Some(installed.clone()),
        Some(&base),
    );
    let (results, waiting, summary) = observed(&validator, &turn);
    assert!(results.is_empty() && !waiting, "no grant: {summary}");
    granted(&turn, "takt.promote");
    let (results, _, summary) = observed(&validator, &turn);
    assert_eq!(results, ["promoted"], "{summary}");
    // Each record below takes the place of the one before it, the same file.
    record(
        &turn.state,
        &turn.tree,
        &turn.candidate,
        Kind::Promotion,
        Some("0".repeat(64)),
        Some(&base),
    );
    let (results, _, summary) = observed(&validator, &turn);
    assert!(results.is_empty(), "another binary installed: {summary}");
    record(
        &turn.state,
        &turn.tree,
        &turn.candidate,
        Kind::Baseline,
        Some(installed.clone()),
        Some(&base),
    );
    let (results, _, summary) = observed(&validator, &turn);
    assert!(results.is_empty(), "a baseline record: {summary}");
    fs::remove_file(&promoted).expect("removed");
    fs::remove_file(
        turn.state
            .join(format!("takt/promotions/{}.scorecard.json", turn.candidate)),
    )
    .expect("removed");
    // The latest record a promotion of the baseline, whose binary is the one installed.
    record(
        &turn.state,
        &turn.tree,
        &turn.baseline,
        Kind::Promotion,
        Some(installed),
        None,
    );
    let (results, waiting, summary) = observed(&validator, &turn);
    assert!(results.is_empty() && waiting, "the baseline's: {summary}");
}
