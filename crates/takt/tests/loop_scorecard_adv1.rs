//! Adversary pass 1 on the loop scorecard (`crates/takt/src/loop_scorecard.rs`): cases that
//! drive the three loop validators and the row reader against what the unit says of them (its
//! module documentation, the ESS prose in `ess/domains/takt.yaml`, the protocol
//! `protocols/loop-improvement/1.yaml`). Each case is named for the defect it shows.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use takt::loop_scorecard::{
    DEFINITIONS, LoopCompare, LoopDefinitionCheck, LoopMeasure, LoopScorecard,
};
use takt::validator::Validator;
use takt::{CaseDir, LoadedProtocol};

const LOOP_IMPROVEMENT: &str = concat!(
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
  promoted_share:
    version: 1
    definition: promoted rows over all rows
  median_tokens_per_accepted_turn:
    version: 1
    definition: the median tokens of the accepted rows with a run record
  finding_to_verified:
    version: 1
    definition: the median seconds from filed_at to verified_at over the rows with both
";

fn git(tree: &Path, args: &[&str]) -> String {
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
    String::from_utf8(output.stdout)
        .expect("utf-8")
        .trim()
        .to_owned()
}

struct Scratch(PathBuf);

impl std::ops::Deref for Scratch {
    type Target = Path;

    fn deref(&self) -> &Path {
        &self.0
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn scratch(name: &str, definitions: &str) -> (Scratch, String) {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("loop-scorecard-adv1-{}-{name}", std::process::id()));
    if dir.exists() {
        fs::remove_dir_all(&dir).expect("a stale scratch tree is removed");
    }
    fs::create_dir_all(&dir).expect("scratch dir");
    fs::write(dir.join("README.md"), "takt\n").expect("a file");
    git(&dir, &["init", "-q", "-b", "main"]);
    let head = commit(&dir, definitions, "start");
    (Scratch(dir), head)
}

fn commit(tree: &Path, definitions: &str, message: &str) -> String {
    let path = tree.join(DEFINITIONS);
    fs::create_dir_all(path.parent().expect("a parent")).expect("the directory");
    fs::write(&path, definitions).expect("the definitions");
    git(tree, &["add", "-A", "--", ".", ":!.engineering/cases"]);
    git(tree, &["commit", "-q", "--allow-empty", "-m", message]);
    git(tree, &["rev-parse", "HEAD"])
}

struct At<'a> {
    at: &'a str,
    holds: &'a [&'a str],
    legitimate: Option<&'a str>,
}

const CLAIMS: [&str; 6] = [
    "finding.cited",
    "scenario.reproduced_before",
    "scenario.passes_after",
    "fix.merged",
    "scorecard.no_regression",
    "promoted",
];
const OUTCOMES: [&str; 4] = ["accepted", "declined", "unreproducible", "abandoned"];

/// A `self.improvement` case `id` with its decisions; no run record.
fn turn(tree: &Path, id: &str, decisions: &[At<'_>]) -> PathBuf {
    let dir = tree.join(".engineering/cases").join(id);
    fs::create_dir_all(dir.join("decisions")).expect("the case directory");
    fs::create_dir_all(dir.join("evidence")).expect("the evidence directory");
    fs::write(
        dir.join("case.yaml"),
        format!(
            "format: canon-case/1\nid: {id}\nprotocol: self.improvement\nartifacts:\n  finding:\n    revision: f-{id}\n  baseline:\n    revision: b-{id}\n  candidate:\n    revision: c-{id}\n  promotion:\n    revision: p-{id}\ninputs:\n  surface: non_judge\n"
        ),
    )
    .expect("case.yaml");
    fs::write(dir.join("story.yaml"), format!("id: story:{id}\n")).expect("story.yaml");
    for (n, decision) in decisions.iter().enumerate() {
        let claims: serde_json::Map<String, serde_json::Value> = CLAIMS
            .iter()
            .map(|claim| {
                let value = if decision.holds.contains(claim) {
                    "true"
                } else {
                    "unknown"
                };
                ((*claim).to_owned(), serde_json::json!({ "value": value }))
            })
            .collect();
        let outcomes: serde_json::Map<String, serde_json::Value> = OUTCOMES
            .iter()
            .map(|outcome| {
                let status = if decision.legitimate == Some(*outcome) {
                    "legitimate"
                } else {
                    "blocked"
                };
                (
                    (*outcome).to_owned(),
                    serde_json::json!({ "status": status }),
                )
            })
            .collect();
        let document = serde_json::json!({
            "format": "canon-decision/1",
            "case": id,
            "claims": claims,
            "outcomes": outcomes,
            "actions": {},
            "obligations": [],
            "explanation": { "computed_from": { "at": decision.at } },
        });
        fs::write(
            dir.join("decisions").join(format!("{n:04}.json")),
            serde_json::to_string_pretty(&document).expect("json"),
        )
        .expect("a decision");
    }
    dir
}

/// A closed turn filed and verified at `at`, promoted when accepted.
fn closed(tree: &Path, id: &str, at: &str, verdict: &str) -> PathBuf {
    let holds: &[&str] = if verdict == "accepted" {
        &[
            "finding.cited",
            "scenario.passes_after",
            "fix.merged",
            "promoted",
        ]
    } else {
        &["finding.cited", "scenario.passes_after"]
    };
    turn(
        tree,
        id,
        &[
            At {
                at,
                holds: &["finding.cited"],
                legitimate: None,
            },
            At {
                at,
                holds,
                legitimate: Some(verdict),
            },
        ],
    )
}

/// An open turn: filed at `at`, nothing else yet.
fn opened(tree: &Path, id: &str, at: &str) -> PathBuf {
    turn(
        tree,
        id,
        &[At {
            at,
            holds: &["finding.cited"],
            legitimate: None,
        }],
    )
}

/// A run record at `dir` whose one harness step reported `tokens` (none: the key absent, as
/// `events::Event::Step` serializes a step whose harness reported none).
fn run_record(dir: &Path, tokens: Option<u64>) {
    let mut step = serde_json::json!({
        "event": "step", "at": "2026-10-18T09:00:00Z", "step": 1,
        "by": "harness", "name": "h", "action": "a", "summary": "s",
    });
    if let Some(tokens) = tokens {
        step["tokens"] = tokens.into();
    }
    let lines = [
        serde_json::json!({ "event": "opened", "at": "2026-10-18T09:00:00Z", "step": 0 }),
        step,
        serde_json::json!({ "event": "stop", "at": "2026-10-18T09:05:00Z", "step": 1, "stop": "Completed (accepted)" }),
    ];
    let text: String = lines.iter().map(|line| format!("{line}\n")).collect();
    fs::write(dir.join("events.jsonl"), text).expect("events.jsonl");
}

fn loop_case(tree: &Path, id: &str, baseline: &str, candidate: &str) -> CaseDir {
    let protocol = LoadedProtocol::from_file(Path::new(LOOP_IMPROVEMENT)).expect("loads");
    let mut case =
        CaseDir::open(tree, protocol, id, "one turn on the loop", &[]).expect("the case opens");
    case.set_revision("loop_baseline", baseline)
        .expect("the baseline");
    case.set_revision("loop_candidate", candidate)
        .expect("the candidate");
    case.set_revision("loop_finding", baseline)
        .expect("the finding");
    let case = CaseDir::resume(tree, id).expect("the case resumes");
    takt::loop_scorecard::snapshot_baseline(tree, &case).expect("the baseline is snapshotted");
    case
}

fn merged(case: &mut CaseDir, at: &str) {
    let revision = case.revision("loop_candidate").expect("a candidate");
    let path = case
        .add_evidence(&takt::evidence::Record::new(
            "merged",
            "merge_observation",
            Some("merged".to_owned()),
            "loop_candidate",
            revision,
            "developer-run",
        ))
        .expect("written");
    let mut record: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&path).expect("the record")).expect("json");
    record["observed_at"] = at.into();
    fs::write(&path, serde_json::to_string_pretty(&record).expect("json")).expect("the record");
}

/// The result `loop.definition.check` writes for a candidate committing `candidate` over a
/// baseline of [`DEFINED`] in `tree`.
fn check_result(tree: &Path, id: &str, base: &str, head: &str) -> String {
    let _case = loop_case(tree, id, base, head);
    let observation = LoopDefinitionCheck
        .observe(tree, head, 1)
        .expect("observes");
    assert_eq!(observation.records.len(), 1, "{}", observation.summary);
    observation.records[0].result.clone().expect("a result")
}

/// ESS `takt.work.LoopScorecardRow`: "A field no record holds is absent, never guessed". A turn
/// whose run record holds no token count (its harness reported none, as `NoHarness` and a
/// `ClaudeCode` step without `usage` do) gets `tokens: Some(0)`, and the median tokens per
/// accepted turn counts that invented zero and names the turn as read.
#[test]
fn a_run_record_without_token_reports_gives_zero_tokens_instead_of_absent() {
    let (tree, _) = scratch("no-tokens", DEFINED);
    let a = closed(&tree, "turn-a", "2026-10-18T09:00:00Z", "accepted");
    run_record(&a, Some(100));
    let b = closed(&tree, "turn-b", "2026-10-18T10:00:00Z", "accepted");
    run_record(&b, Some(300));
    let c = closed(&tree, "turn-c", "2026-10-18T11:00:00Z", "accepted");
    run_record(&c, None);
    let card = LoopScorecard::read(&tree).expect("reads");
    let row = card
        .rows
        .iter()
        .find(|row| row.turn == "turn-c")
        .expect("turn-c");
    assert_eq!(
        row.tokens, None,
        "a run record that holds no token count gives no tokens, not a guessed 0"
    );
}

/// The protocol: `loop.compare` runs "once at least three self.improvement turns ran under the
/// merged candidate". Three turns merely opened after the merge (filed, no verdict) count as
/// the three, their accepted share is 0/3 by the open-turns-included definition, and the
/// comparison records `not_improved`, which makes `declined` legitimate before any of them ran.
#[test]
fn loop_compare_takes_three_turns_just_opened_after_the_merge_as_three_that_ran() {
    let (tree, head) = scratch("compare-open", DEFINED);
    closed(&tree, "before-a", "2026-10-18T09:00:00Z", "accepted");
    closed(&tree, "before-b", "2026-10-18T09:10:00Z", "declined");
    closed(&tree, "before-c", "2026-10-18T09:20:00Z", "declined");
    let mut case = loop_case(&tree, "loop-compare-open", &head, &head);
    merged(&mut case, "2026-10-18T10:00:00Z");
    opened(&tree, "after-a", "2026-10-18T11:00:00Z");
    opened(&tree, "after-b", "2026-10-18T11:10:00Z");
    opened(&tree, "after-c", "2026-10-18T11:20:00Z");
    let observation = LoopCompare.observe(&tree, &head, 1).expect("observes");
    assert!(
        observation.records.is_empty(),
        "no turn has run under the candidate yet, so nothing is compared; recorded {:?}: {}",
        observation
            .records
            .iter()
            .map(|record| record.result.clone())
            .collect::<Vec<_>>(),
        observation.summary
    );
}

/// ESS `takt.work.LoopFigure`: "a definition cannot name a figure nothing computes". A
/// candidate that adds a metric the code does not compute passes `loop.definition.check` as
/// `unchanged` (honest, mergeable), and once it is the baseline `loop.measure` records nothing
/// at all: every figure, the old ones too, is gone.
#[test]
fn an_added_metric_no_code_computes_is_honest_and_then_stops_every_measurement() {
    let (tree, base) = scratch("added-metric", DEFINED);
    let added = format!(
        "{DEFINED}  escaped_defects:\n    version: 1\n    definition: escaped defects, human-sourced\n"
    );
    let head = commit(&tree, &added, "candidate");
    for (id, at, verdict) in [
        ("turn-a", "2026-10-18T09:00:00Z", "accepted"),
        ("turn-b", "2026-10-18T10:00:00Z", "declined"),
        ("turn-c", "2026-10-18T11:00:00Z", "declined"),
    ] {
        closed(&tree, id, at, verdict);
    }
    let result = check_result(&tree, "loop-added", &base, &head);
    let honest = result == "unchanged" || result == "dual_reported";
    let measured = LoopMeasure.observe(&tree, &head, 1).expect("observes");
    assert!(
        !honest || measured.records.len() == 1,
        "the check called the candidate {result}, yet at it the scorecard measures nothing: {}",
        measured.summary
    );
}

/// The check's own definition of `dual_reported`: "the figures of both print". A candidate that
/// bumps `accepted_share` to v2 and keeps v1 under `previous` is `dual_reported`, yet no code
/// computes v2, so at the candidate no figure prints at all and `loop.measure` records nothing.
#[test]
fn dual_reported_is_written_for_a_version_whose_figure_cannot_print() {
    let (tree, base) = scratch("dual-v2", DEFINED);
    let start = DEFINED.find("  accepted_share:").expect("the entry");
    let end = DEFINED.find("  promoted_share:").expect("the next entry");
    let dual = format!(
        "{}  accepted_share:
    version: 2
    definition: accepted rows over closed rows, open turns left out
    target:
      at_least: 0.75
    previous:
      - version: 1
        definition: accepted rows over all rows, open turns included
        target:
          at_least: 0.75
{}",
        &DEFINED[..start],
        &DEFINED[end..]
    );
    let head = commit(&tree, &dual, "candidate");
    for (id, at, verdict) in [
        ("turn-a", "2026-10-18T09:00:00Z", "accepted"),
        ("turn-b", "2026-10-18T10:00:00Z", "declined"),
        ("turn-c", "2026-10-18T11:00:00Z", "declined"),
    ] {
        closed(&tree, id, at, verdict);
    }
    let result = check_result(&tree, "loop-dual-v2", &base, &head);
    let measured = LoopMeasure.observe(&tree, &head, 1).expect("observes");
    assert!(
        result != "dual_reported" || measured.records.len() == 1,
        "dual_reported says both figures print; at the candidate none does: {}",
        measured.summary
    );
}

/// The definitions are "read at a revision through git", the baseline's "as it stands at the
/// base". A judgement step can plant `refs/replace/<base>` (the run's checkpoint guards only
/// `refs/heads/`, `refs/tags/` and `.git/{hooks,info,config}`, and its restore leaves the
/// replace ref in place, shown here), and `git cat-file` then reads the candidate's text as the
/// base's: a silent redefinition is checked against itself and written `unchanged`.
#[test]
fn a_replace_ref_a_step_leaves_redirects_the_baseline_definitions_to_the_candidates() {
    let (tree, base) = scratch("replace", DEFINED);
    let start = DEFINED.find("  accepted_share:").expect("the entry");
    let end = DEFINED.find("  promoted_share:").expect("the next entry");
    let silent = format!(
        "{}  accepted_share:
    version: 1
    definition: accepted rows over closed rows, open turns left out
    target:
      at_least: 0.1
{}",
        &DEFINED[..start],
        &DEFINED[end..]
    );
    let head = commit(&tree, &silent, "candidate");
    let _case = loop_case(&tree, "loop-replace", &base, &head);
    // What a judgement step may do, between the checkpoint the run takes and its restore.
    let checkpoint = takt::git::Checkpoint::take_guarding(&tree, &[]).expect("a checkpoint");
    git(&tree, &["replace", &base, &head]);
    checkpoint.restore(&tree).expect("restored");
    let observation = LoopDefinitionCheck
        .observe(&tree, &head, 1)
        .expect("observes");
    assert_eq!(observation.records.len(), 1, "{}", observation.summary);
    assert_eq!(
        observation.records[0].result.as_deref(),
        Some("redefined"),
        "the same version with another definition and target is redefined: {}",
        observation.summary
    );
}
