//! The loop scorecard and the three validators that drive a `loop.improvement/1` case
//! (`protocols/loop-improvement/1.yaml`): `loop.measure` over the recorded `self.improvement`
//! cases under `.engineering/cases/`, one row per turn and the figures computed from the rows;
//! `loop.definition.check` comparing the metric definitions at the candidate with those at the
//! baseline; `loop.compare` comparing the figures of the turns filed after the candidate merged
//! with those of the turns before. The turns here are fixture cases written file by file: a
//! `case.yaml`, the decisions Canon wrote for them, a run record, the story they were opened for.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::anyhow;
use takt::harness::{Harness, StepContext, StepReport};
use takt::loop_scorecard::{
    DEFINITIONS, Definitions, LoopCompare, LoopDefinitionCheck, LoopMeasure, LoopScorecard,
};
use takt::validator::Validator;
use takt::{CaseDir, LoadedProtocol, Run, RunOptions};

const LOOP_IMPROVEMENT: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../protocols/loop-improvement/1.yaml"
);

/// The recorded first improvement turn, as the run record copied it.
const TURN_1: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../docs/runs/2026-10-18-improvement-turn-1/case"
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
    assert!(output.status.success(), "git {args:?}");
    String::from_utf8(output.stdout)
        .expect("utf-8")
        .trim()
        .to_owned()
}

/// A scratch git tree under this crate's build directory, removed on drop.
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

/// A scratch repository whose first commit holds `definitions` at [`DEFINITIONS`]; its `HEAD`.
fn scratch(name: &str, definitions: &str) -> (Scratch, String) {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("loop-scorecard-{}-{name}", std::process::id()));
    if dir.exists() {
        fs::remove_dir_all(&dir).expect("a stale scratch tree is removed");
    }
    fs::create_dir_all(&dir).expect("scratch dir");
    fs::write(dir.join("README.md"), "takt\n").expect("a file");
    git(&dir, &["init", "-q", "-b", "main"]);
    let head = commit(&dir, definitions, "start");
    (Scratch(dir), head)
}

/// Commits `definitions` at [`DEFINITIONS`] in `tree`; the commit.
fn commit(tree: &Path, definitions: &str, message: &str) -> String {
    let path = tree.join(DEFINITIONS);
    fs::create_dir_all(path.parent().expect("a parent")).expect("the directory");
    fs::write(&path, definitions).expect("the definitions");
    git(tree, &["add", "-A", "--", ".", ":!.engineering/cases"]);
    git(tree, &["commit", "-q", "--allow-empty", "-m", message]);
    git(tree, &["rev-parse", "HEAD"])
}

/// One decision of a fixture turn: its instant, the claims true at it, the outcome legitimate.
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

/// Writes a `self.improvement` case `id` under `tree`'s cases directory: its snapshot, the
/// story it was opened for (`story:<finding>`), its decisions in order, and a run record whose
/// one step reported `tokens` when given.
fn turn(tree: &Path, id: &str, finding: Option<&str>, decisions: &[At<'_>], tokens: Option<u64>) {
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
    if let Some(finding) = finding {
        fs::write(dir.join("story.yaml"), format!("id: story:{finding}\n")).expect("story.yaml");
    }
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
    if let Some(tokens) = tokens {
        let first = decisions.first().map_or("2026-10-18T09:00:00Z", |d| d.at);
        let last = decisions.last().map_or(first, |d| d.at);
        let lines = [
            serde_json::json!({ "event": "opened", "at": first, "step": 0 }),
            serde_json::json!({ "event": "step", "at": first, "step": 1, "tokens": tokens }),
            serde_json::json!({ "event": "stop", "at": last, "step": 1, "stop": "Completed (accepted)" }),
        ];
        let text: String = lines.iter().map(|line| format!("{line}\n")).collect();
        fs::write(dir.join("events.jsonl"), text).expect("events.jsonl");
    }
}

/// A turn that was filed at `filed`, verified at `verified` and closed with `verdict`, promoted
/// when the verdict is `accepted`.
fn closed(tree: &Path, id: &str, filed: &str, verified: &str, verdict: &str, tokens: Option<u64>) {
    let promoted: &[&str] = if verdict == "accepted" {
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
        Some(id),
        &[
            At {
                at: filed,
                holds: &["finding.cited"],
                legitimate: None,
            },
            At {
                at: verified,
                holds: &["finding.cited", "scenario.passes_after"],
                legitimate: None,
            },
            At {
                at: verified,
                holds: promoted,
                legitimate: Some(verdict),
            },
        ],
        tokens,
    );
}

/// A harness the runs below never reach.
struct NoHarness;

impl Harness for NoHarness {
    fn name(&self) -> &str {
        "none"
    }

    fn step(&self, context: &StepContext<'_>) -> Result<StepReport, anyhow::Error> {
        Err(anyhow!(
            "no harness: a judgement step among {} is left",
            context.offered.join(", ")
        ))
    }
}

/// Opens a loop case `id` on `tree` with `loop_baseline` at `baseline` and `loop_candidate` at
/// `candidate`, the loop validators' producers declared independent of the case's work.
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
    let snapshot = case.dir.join("case.yaml");
    let mut doc: serde_yaml_ng::Value =
        serde_yaml_ng::from_str(&fs::read_to_string(&snapshot).expect("case.yaml")).expect("yaml");
    let mut independence = serde_yaml_ng::Mapping::new();
    for producer in [
        LoopMeasure.name(),
        LoopDefinitionCheck.name(),
        LoopCompare.name(),
    ] {
        independence.insert(producer.into(), vec!["different_agent_run"].into());
    }
    doc["independence"] = independence.into();
    fs::write(&snapshot, serde_yaml_ng::to_string(&doc).expect("yaml")).expect("case.yaml");
    CaseDir::resume(tree, id).expect("the case resumes")
}

/// The evidence records under the case, in name order.
fn records(case: &CaseDir) -> Vec<serde_json::Value> {
    let mut paths: Vec<PathBuf> = fs::read_dir(case.dir.join("evidence"))
        .expect("an evidence directory")
        .map(|entry| entry.expect("an entry").path())
        .collect();
    paths.sort();
    paths
        .iter()
        .map(|path| {
            serde_json::from_str(&fs::read_to_string(path).expect("a record")).expect("json")
        })
        .collect()
}

fn ids(value: &serde_json::Value) -> Vec<String> {
    value
        .as_array()
        .expect("a list")
        .iter()
        .map(|id| id.as_str().expect("an id").to_owned())
        .collect()
}

/// Acceptance: over three recorded turns, `loop.measure` writes one `loop_measurement` record,
/// and the figures it retains each name the case ids they were computed from.
#[test]
fn three_recorded_turns_give_one_loop_measurement_whose_figures_name_their_cases() {
    let (tree, head) = scratch("acceptance", DEFINED);
    closed(
        &tree,
        "turn-a",
        "2026-10-18T09:00:00Z",
        "2026-10-18T10:00:00Z",
        "accepted",
        Some(100),
    );
    closed(
        &tree,
        "turn-b",
        "2026-10-18T11:00:00Z",
        "2026-10-18T11:30:00Z",
        "accepted",
        Some(300),
    );
    closed(
        &tree,
        "turn-c",
        "2026-10-18T12:00:00Z",
        "2026-10-18T13:00:00Z",
        "declined",
        Some(50),
    );
    let case = loop_case(&tree, "loop-1", &head, &head);
    let validators: Vec<Box<dyn Validator>> = vec![
        Box::new(LoopMeasure),
        Box::new(LoopDefinitionCheck),
        Box::new(LoopCompare),
    ];
    let options = RunOptions {
        protocol: String::new(),
        intent: String::new(),
        inputs: Vec::new(),
        iterations: Some(2),
        hours: None,
        tokens: None,
        case_id: None,
        gate: None,
    };
    let mut run =
        Run::resume(&tree, options, &case.id, &NoHarness, &validators).expect("the run resumes");
    let mut out = Vec::new();
    run.run(&mut out).expect("the run stops without an error");
    let text = String::from_utf8(out).expect("utf-8");

    let measured: Vec<serde_json::Value> = records(&run.case)
        .into_iter()
        .filter(|record| record["kind"] == "loop_measurement")
        .collect();
    assert_eq!(measured.len(), 1, "{text}");
    let record = &measured[0];
    assert_eq!(record["result"], "below_target", "{text}");
    assert_eq!(record["subject"], "loop_baseline", "{text}");
    assert_eq!(record["subject_revision"], head.as_str(), "{text}");
    assert_eq!(record["producer"], LoopMeasure.name(), "{text}");

    let artifacts: Vec<PathBuf> = fs::read_dir(run.case.dir.join("artifacts"))
        .expect("the retained measurement")
        .map(|entry| entry.expect("an entry").path())
        .filter(|path| {
            path.file_name()
                .is_some_and(|name| name.to_string_lossy().starts_with("loop-measurement-"))
        })
        .collect();
    assert_eq!(artifacts.len(), 1, "{text}");
    let snapshot: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&artifacts[0]).expect("the snapshot"))
            .expect("json");
    assert_eq!(snapshot["result"], "below_target");
    assert_eq!(snapshot["definitions_revision"], head.as_str());
    let figures = snapshot["figures"].as_array().expect("figures");
    let all = ["turn-a", "turn-b", "turn-c"];
    for figure in figures {
        let cases = ids(&figure["cases"]);
        assert!(!cases.is_empty(), "a figure names its cases: {figure}");
        assert!(
            cases.iter().all(|case| all.contains(&case.as_str())),
            "a figure names only turns: {figure}"
        );
    }
    let figure = |metric: &str| {
        figures
            .iter()
            .find(|figure| figure["metric"] == metric)
            .unwrap_or_else(|| panic!("a figure {metric}: {snapshot}"))
    };
    assert_eq!(ids(&figure("turns")["cases"]), all);
    assert_eq!(figure("turns")["value"], 3.0);
    assert_eq!(ids(&figure("accepted_share")["cases"]), all);
    assert_eq!(figure("accepted_share")["shown"], "2/3");
    assert_eq!(figure("accepted_share")["below_target"], true);
    assert_eq!(
        ids(&figure("median_tokens_per_accepted_turn")["cases"]),
        ["turn-a", "turn-b"]
    );
    assert_eq!(figure("median_tokens_per_accepted_turn")["value"], 200.0);
    assert_eq!(ids(&figure("finding_to_verified")["cases"]), all);

    let decision = run.case.evaluate(99).expect("evaluates");
    assert!(
        decision.holds("loop.metric_below_target"),
        "{}",
        decision.json
    );
    assert!(text.contains("loop.measure"), "{text}");
}

/// With fewer than three recorded turns, nothing is measured.
#[test]
fn fewer_than_three_turns_record_no_measurement() {
    let (tree, head) = scratch("two-turns", DEFINED);
    closed(
        &tree,
        "turn-a",
        "2026-10-18T09:00:00Z",
        "2026-10-18T10:00:00Z",
        "accepted",
        Some(100),
    );
    closed(
        &tree,
        "turn-b",
        "2026-10-18T11:00:00Z",
        "2026-10-18T11:30:00Z",
        "declined",
        None,
    );
    let observation = LoopMeasure.observe(&tree, &head, 1).expect("observes");
    assert!(observation.records.is_empty(), "{}", observation.summary);
    assert!(
        observation.summary.contains("at least 3"),
        "{}",
        observation.summary
    );
}

/// Check: with one turn open, its row has no verdict and the accepted share is accepted rows
/// over all rows, the open one included.
#[test]
fn an_open_turn_has_no_verdict_and_counts_in_the_accepted_share() {
    let (tree, _) = scratch("open-turn", DEFINED);
    closed(
        &tree,
        "turn-a",
        "2026-10-18T09:00:00Z",
        "2026-10-18T10:00:00Z",
        "accepted",
        Some(100),
    );
    closed(
        &tree,
        "turn-b",
        "2026-10-18T11:00:00Z",
        "2026-10-18T11:30:00Z",
        "declined",
        None,
    );
    turn(
        &tree,
        "turn-c",
        Some("turn-c"),
        &[At {
            at: "2026-10-18T12:00:00Z",
            holds: &["finding.cited"],
            legitimate: None,
        }],
        None,
    );
    let card = LoopScorecard::read(&tree).expect("reads");
    assert_eq!(card.rows.len(), 3);
    let open = &card.rows[2];
    assert_eq!(open.turn, "turn-c");
    assert_eq!(open.verdict, None);
    assert!(!open.promoted);
    assert_eq!(open.filed_at.as_deref(), Some("2026-10-18T12:00:00Z"));
    assert_eq!(open.verified_at, None);
    assert_eq!(card.rows[0].verdict.as_deref(), Some("accepted"));
    assert_eq!(card.rows[1].verdict.as_deref(), Some("declined"));
    assert_eq!(card.rows[0].finding.as_deref(), Some("turn-a"));
    assert_eq!(card.rows[0].tokens, Some(100));
    assert_eq!(card.rows[1].tokens, None);

    let definitions = Definitions::parse(DEFINED).expect("parses");
    let figures = card.figures(&definitions).expect("computes");
    let share = figures
        .iter()
        .find(|figure| figure.metric == "accepted_share")
        .expect("the accepted share");
    assert_eq!(share.shown, "1/3");
    assert_eq!(share.cases, ["turn-a", "turn-b", "turn-c"]);
    assert!(share.below_target);
}

/// Check: the time from finding to verified is computed from each row's `filed_at` and
/// `verified_at`; a row not verified, or whose verification no longer holds, is left out.
#[test]
fn the_time_from_finding_to_verified_is_computed_from_each_rows_instants() {
    let (tree, _) = scratch("verified", DEFINED);
    closed(
        &tree,
        "turn-a",
        "2026-10-18T10:00:00Z",
        "2026-10-18T10:30:00Z",
        "accepted",
        None,
    );
    closed(
        &tree,
        "turn-b",
        "2026-10-18T09:00:00Z",
        "2026-10-18T11:00:00Z",
        "declined",
        None,
    );
    // Verified, then a new candidate: the verification no longer holds.
    turn(
        &tree,
        "turn-c",
        Some("turn-c"),
        &[
            At {
                at: "2026-10-18T08:00:00Z",
                holds: &["finding.cited"],
                legitimate: None,
            },
            At {
                at: "2026-10-18T08:10:00Z",
                holds: &["finding.cited", "scenario.passes_after"],
                legitimate: None,
            },
            At {
                at: "2026-10-18T08:20:00Z",
                holds: &["finding.cited"],
                legitimate: None,
            },
        ],
        None,
    );
    // Verified, undone, verified again: verified from the last time it began to hold.
    turn(
        &tree,
        "turn-d",
        Some("turn-d"),
        &[
            At {
                at: "2026-10-18T07:00:00Z",
                holds: &["finding.cited"],
                legitimate: None,
            },
            At {
                at: "2026-10-18T07:10:00Z",
                holds: &["finding.cited", "scenario.passes_after"],
                legitimate: None,
            },
            At {
                at: "2026-10-18T07:20:00Z",
                holds: &["finding.cited"],
                legitimate: None,
            },
            At {
                at: "2026-10-18T08:00:00Z",
                holds: &["finding.cited", "scenario.passes_after"],
                legitimate: None,
            },
        ],
        None,
    );
    let card = LoopScorecard::read(&tree).expect("reads");
    let row = |id: &str| {
        card.rows
            .iter()
            .find(|row| row.turn == id)
            .expect("the row")
    };
    assert_eq!(
        row("turn-a").filed_at.as_deref(),
        Some("2026-10-18T10:00:00Z")
    );
    assert_eq!(
        row("turn-a").verified_at.as_deref(),
        Some("2026-10-18T10:30:00Z")
    );
    assert_eq!(row("turn-c").verified_at, None);
    assert_eq!(
        row("turn-d").verified_at.as_deref(),
        Some("2026-10-18T08:00:00Z")
    );
    let definitions = Definitions::parse(DEFINED).expect("parses");
    let figures = card.figures(&definitions).expect("computes");
    let time = figures
        .iter()
        .find(|figure| figure.metric == "finding_to_verified")
        .expect("the time from finding to verified");
    // 1800 s, 7200 s and 3600 s: the median is 3600 s.
    assert_eq!(time.cases, ["turn-a", "turn-b", "turn-d"]);
    assert_eq!(time.value, Some(3600.0));
}

/// The recorded first turn reads as a row: accepted and promoted, its revisions from its
/// snapshot, filed and verified when its decisions first held each claim, and no tokens or
/// finding key, because the hand recording wrote no run record and no story record.
#[test]
fn the_recorded_first_turn_reads_as_an_accepted_promoted_row() {
    let (tree, _) = scratch("turn-1", DEFINED);
    let into = tree.join(".engineering/cases/turn-1-scenario-help");
    copy(Path::new(TURN_1), &into);
    let card = LoopScorecard::read(&tree).expect("reads");
    assert_eq!(card.rows.len(), 1);
    let row = &card.rows[0];
    assert_eq!(row.turn, "turn-1-scenario-help");
    assert_eq!(row.verdict.as_deref(), Some("accepted"));
    assert!(row.promoted);
    assert_eq!(row.baseline, "0a776d2");
    assert_eq!(row.candidate, "447423c");
    assert_eq!(row.filed_at.as_deref(), Some("2026-10-18T14:40:03Z"));
    assert_eq!(row.verified_at.as_deref(), Some("2026-10-18T14:40:03Z"));
    assert_eq!(row.tokens, None);
    assert_eq!(row.wall_ms, None);
    assert_eq!(row.finding, None);
}

fn copy(from: &Path, to: &Path) {
    fs::create_dir_all(to).expect("the directory");
    for entry in fs::read_dir(from).expect("the source") {
        let entry = entry.expect("an entry");
        let target = to.join(entry.file_name());
        if entry.file_type().expect("a type").is_dir() {
            copy(&entry.path(), &target);
        } else {
            fs::copy(entry.path(), &target).expect("a copy");
        }
    }
}

/// The definitions `DEFINED` with the accepted share's entry replaced by `entry`.
fn with_accepted_share(entry: &str) -> String {
    let start = DEFINED.find("  accepted_share:").expect("the entry");
    let end = DEFINED.find("  promoted_share:").expect("the next entry");
    format!("{}{entry}{}", &DEFINED[..start], &DEFINED[end..])
}

/// The result `loop.definition.check` records for a candidate committing `candidate` over a
/// baseline of [`DEFINED`], and whether the loop case then holds `loop.definition_honest`.
fn checked(name: &str, candidate: Option<&str>) -> (String, bool) {
    let (tree, base) = scratch(name, DEFINED);
    let head = match candidate {
        Some(text) => commit(&tree, text, "candidate"),
        None => {
            git(&tree, &["rm", "-q", DEFINITIONS]);
            git(&tree, &["commit", "-q", "-m", "drop the definitions"]);
            git(&tree, &["rev-parse", "HEAD"])
        }
    };
    let mut case = loop_case(&tree, name, &base, &head);
    let observation = LoopDefinitionCheck
        .observe(&tree, &head, 1)
        .expect("observes");
    assert_eq!(observation.records.len(), 1, "{}", observation.summary);
    let record = &observation.records[0];
    assert_eq!(record.kind, "definition_check");
    assert_eq!(record.subject, "loop_candidate");
    assert_eq!(record.subject_revision, head);
    case.add_evidence(record).expect("written");
    let decision = case.evaluate(1).expect("evaluates");
    (
        record.result.clone().expect("a result"),
        decision.holds("loop.definition_honest"),
    )
}

/// Check: a changed metric definition is `dual_reported` when both figures print (the new
/// version, with the base's definition kept as a previous one) and `redefined` when only the new
/// one does; unchanged definitions are `unchanged`.
#[test]
fn a_changed_definition_is_dual_reported_when_both_figures_print_and_redefined_otherwise() {
    assert_eq!(
        checked("unchanged", Some(DEFINED)),
        ("unchanged".to_owned(), true)
    );
    let dual = with_accepted_share(
        "  accepted_share:
    version: 2
    definition: accepted rows over closed rows, open turns left out
    target:
      at_least: 0.75
    previous:
      - version: 1
        definition: accepted rows over all rows, open turns included
        target:
          at_least: 0.75
",
    );
    assert_eq!(
        checked("dual", Some(&dual)),
        ("dual_reported".to_owned(), true)
    );
    let silent = with_accepted_share(
        "  accepted_share:
    version: 2
    definition: accepted rows over closed rows, open turns left out
    target:
      at_least: 0.75
",
    );
    assert_eq!(
        checked("silent", Some(&silent)),
        ("redefined".to_owned(), false)
    );
    let same_version = with_accepted_share(
        "  accepted_share:
    version: 1
    definition: accepted rows over all rows, open turns included
    target:
      at_least: 0.5
",
    );
    assert_eq!(
        checked("same-version", Some(&same_version)),
        ("redefined".to_owned(), false)
    );
    let dropped = with_accepted_share("");
    assert_eq!(
        checked("dropped", Some(&dropped)),
        ("redefined".to_owned(), false)
    );
    assert_eq!(checked("removed", None), ("redefined".to_owned(), false));
}

/// Writes a `merge_observation` record of the loop candidate at `at` into `case`.
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

/// `loop.compare`: the turns filed after the candidate merged against those filed before, on
/// the metrics below their target before: `improved` when each is better after, else
/// `not_improved`; fewer than three turns after the merge records nothing.
#[test]
fn the_loop_comparison_sets_the_turns_after_the_merge_against_those_before() {
    let (tree, head) = scratch("compare", DEFINED);
    for (id, at, verdict) in [
        ("before-a", "2026-10-18T09:00:00Z", "accepted"),
        ("before-b", "2026-10-18T09:10:00Z", "declined"),
        ("before-c", "2026-10-18T09:20:00Z", "declined"),
        ("after-a", "2026-10-18T11:00:00Z", "accepted"),
        ("after-b", "2026-10-18T11:10:00Z", "accepted"),
    ] {
        closed(&tree, id, at, at, verdict, None);
    }
    let mut case = loop_case(&tree, "loop-compare", &head, &head);
    merged(&mut case, "2026-10-18T10:00:00Z");
    let observation = LoopCompare.observe(&tree, &head, 1).expect("observes");
    assert!(observation.records.is_empty(), "{}", observation.summary);
    assert!(
        observation.summary.contains("at least 3"),
        "{}",
        observation.summary
    );

    closed(
        &tree,
        "after-c",
        "2026-10-18T11:20:00Z",
        "2026-10-18T11:20:00Z",
        "declined",
        None,
    );
    let observation = LoopCompare.observe(&tree, &head, 2).expect("observes");
    assert_eq!(observation.records.len(), 1, "{}", observation.summary);
    let record = &observation.records[0];
    assert_eq!(record.kind, "loop_comparison");
    assert_eq!(record.subject, "loop_candidate");
    // Accepted share 1/3 before, 2/3 after: improved.
    assert_eq!(
        record.result.as_deref(),
        Some("improved"),
        "{}",
        observation.summary
    );
    let artifact = observation
        .artifact
        .as_ref()
        .expect("the comparison retained");
    let retained: serde_json::Value = serde_json::from_slice(&artifact.bytes).expect("json");
    assert_eq!(ids(&retained["compared"]), ["accepted_share"], "{retained}");

    closed(
        &tree,
        "after-d",
        "2026-10-18T11:30:00Z",
        "2026-10-18T11:30:00Z",
        "declined",
        None,
    );
    closed(
        &tree,
        "after-e",
        "2026-10-18T11:40:00Z",
        "2026-10-18T11:40:00Z",
        "declined",
        None,
    );
    // Accepted share 2/5 after against 1/3 before is still better; 2/6 is not.
    closed(
        &tree,
        "after-f",
        "2026-10-18T11:50:00Z",
        "2026-10-18T11:50:00Z",
        "declined",
        None,
    );
    let observation = LoopCompare.observe(&tree, &head, 3).expect("observes");
    assert_eq!(
        observation.records[0].result.as_deref(),
        Some("not_improved"),
        "{}",
        observation.summary
    );
}

/// The definitions this repository ships parse, and every figure they define is one the code
/// computes, current and previous versions alike.
#[test]
fn the_shipped_definitions_parse_and_each_figure_is_computed() {
    let text = fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .join(DEFINITIONS),
    )
    .expect("the shipped definitions");
    let definitions = Definitions::parse(&text).expect("parses");
    for metric in [
        "turns",
        "accepted_share",
        "promoted_share",
        "median_tokens_per_accepted_turn",
        "finding_to_verified",
    ] {
        assert!(definitions.metrics.contains_key(metric), "{metric}");
    }
    LoopScorecard::default()
        .figures(&definitions)
        .expect("every figure is computed");
}

/// The three validators perform the protocol's actions and produce its evidence kinds about
/// the artifacts its claims read.
#[test]
fn the_validators_name_the_protocols_actions_kinds_and_subjects() {
    let protocol = LoadedProtocol::from_file(Path::new(LOOP_IMPROVEMENT)).expect("loads");
    for (validator, action, kind, subject) in [
        (
            &LoopMeasure as &dyn Validator,
            "loop.measure",
            "loop_measurement",
            "loop_baseline",
        ),
        (
            &LoopDefinitionCheck,
            "loop.definition.check",
            "definition_check",
            "loop_candidate",
        ),
        (
            &LoopCompare,
            "loop.compare",
            "loop_comparison",
            "loop_candidate",
        ),
    ] {
        assert_eq!(validator.action(), action);
        assert_eq!(validator.kind(), kind);
        assert_eq!(validator.subject(), subject);
        assert!(protocol.actions.contains(&action.to_owned()), "{action}");
    }
}

/// A loop case whose artifacts are not yet set to commits (`r0`, as a case opens) records
/// nothing and ends no run on an error: there are no definitions to read at such a revision.
#[test]
fn a_revision_that_is_no_commit_records_nothing() {
    let (tree, head) = scratch("r0", DEFINED);
    for id in ["turn-a", "turn-b", "turn-c"] {
        closed(
            &tree,
            id,
            "2026-10-18T09:00:00Z",
            "2026-10-18T10:00:00Z",
            "accepted",
            None,
        );
    }
    let _case = loop_case(&tree, "loop-r0", "r0", "r0");
    for validator in [
        &LoopMeasure as &dyn Validator,
        &LoopDefinitionCheck,
        &LoopCompare,
    ] {
        let observation = validator
            .observe(&tree, "r0", 1)
            .unwrap_or_else(|error| panic!("{}: {error:#}", validator.action()));
        assert!(observation.records.is_empty(), "{}", observation.summary);
    }
    let observation = LoopDefinitionCheck
        .observe(&tree, &head, 1)
        .expect("observes");
    assert!(observation.records.is_empty(), "{}", observation.summary);
}
