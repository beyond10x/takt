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

/// Files the finding `key` in `tree`'s planning store as `scripts/file-finding` does, its store
/// file committed at `at`; the commit.
fn file(tree: &Path, key: &str, at: &str) -> String {
    let relative = format!(".engineering/planning/story/{key}.md");
    let path = tree.join(&relative);
    fs::create_dir_all(path.parent().expect("a parent")).expect("the store");
    fs::write(&path, format!("# finding {key}\n")).expect("the finding");
    git(tree, &["add", "--", &relative]);
    let status = Command::new("git")
        .args(["-c", "user.name=t", "-c", "user.email=t@localhost"])
        .args(["commit", "-q", "-m", "file the finding", "--", &relative])
        .env("GIT_AUTHOR_DATE", at)
        .env("GIT_COMMITTER_DATE", at)
        .current_dir(tree)
        .status()
        .expect("git runs");
    assert!(status.success(), "commit {relative}");
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

/// Writes a `self.improvement` case `id` under `tree`'s cases directory: its snapshot, its
/// `finding` and `baseline` at `tree`'s `HEAD` (which holds every finding filed so far), the
/// story it was opened for (`story:<finding>`), its decisions in order, and a run record whose
/// one step reported `tokens` when given.
fn turn(tree: &Path, id: &str, finding: Option<&str>, decisions: &[At<'_>], tokens: Option<u64>) {
    let dir = tree.join(".engineering/cases").join(id);
    fs::create_dir_all(dir.join("decisions")).expect("the case directory");
    fs::create_dir_all(dir.join("evidence")).expect("the evidence directory");
    let baseline = git(tree, &["rev-parse", "HEAD"]);
    fs::write(
        dir.join("case.yaml"),
        format!(
            "format: canon-case/1\nid: {id}\nprotocol: self.improvement\nartifacts:\n  finding:\n    revision: {baseline}\n  baseline:\n    revision: {baseline}\n  candidate:\n    revision: c-{id}\n  promotion:\n    revision: p-{id}\ninputs:\n  surface: non_judge\n"
        ),
    )
    .expect("case.yaml");
    fs::write(
        dir.join("protocol.yaml"),
        "format: protocol/1\nprotocol:\n  id: self.improvement\n  revision: 1\n",
    )
    .expect("protocol.yaml");
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

/// A turn whose finding was filed in the store at `filed`, verified at `verified` and closed with
/// `verdict`, promoted when the verdict is `accepted`.
fn closed(tree: &Path, id: &str, filed: &str, verified: &str, verdict: &str, tokens: Option<u64>) {
    file(tree, id, filed);
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
    let case = CaseDir::resume(tree, id).expect("the case resumes");
    takt::loop_scorecard::snapshot_baseline(tree, &case).expect("the baseline is snapshotted");
    case
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
    let (tree, _) = scratch("acceptance", DEFINED);
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
    let head = git(&tree, &["rev-parse", "HEAD"]);
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
    let _case = loop_case(&tree, "loop-two-turns", &head, &head);
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
    let head = file(&tree, "turn-c", "2026-10-18T12:00:00Z");
    turn(
        &tree,
        "turn-c",
        Some("turn-c"),
        &[At {
            at: "2026-10-18T12:05:00Z",
            holds: &["finding.cited"],
            legitimate: None,
        }],
        None,
    );
    let card = LoopScorecard::read_at(&tree, Some(&head)).expect("reads");
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
    file(&tree, "turn-c", "2026-10-18T08:00:00Z");
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
    // Verified, undone, verified again: verified from the last time it began to hold. Filed in
    // the store at 07:00, its case first evaluated at 07:05: filed at 07:00.
    let head = file(&tree, "turn-d", "2026-10-18T07:00:00Z");
    turn(
        &tree,
        "turn-d",
        Some("turn-d"),
        &[
            At {
                at: "2026-10-18T07:05:00Z",
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
    let card = LoopScorecard::read_at(&tree, Some(&head)).expect("reads");
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
    assert_eq!(
        row("turn-d").filed_at.as_deref(),
        Some("2026-10-18T07:00:00Z")
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
/// snapshot, verified when its decisions first held the claim, and no tokens, finding key or
/// filing time, because the hand recording wrote no run record and no story record.
#[test]
fn the_recorded_first_turn_reads_as_an_accepted_promoted_row() {
    let (tree, head) = scratch("turn-1", DEFINED);
    let into = tree.join(".engineering/cases/turn-1-scenario-help");
    copy(Path::new(TURN_1), &into);
    let card = LoopScorecard::read_at(&tree, Some(&head)).expect("reads");
    assert_eq!(card.rows.len(), 1);
    let row = &card.rows[0];
    assert_eq!(row.turn, "turn-1-scenario-help");
    assert_eq!(row.verdict.as_deref(), Some("accepted"));
    assert!(row.promoted);
    assert_eq!(row.baseline, "0a776d2");
    assert_eq!(row.candidate, "447423c");
    assert_eq!(row.filed_at, None);
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
    // Targets are the owner's: a candidate that moves one is redefined, even under a new
    // version with the old definition kept beside it.
    let lowered = with_accepted_share(
        "  accepted_share:
    version: 2
    definition: accepted rows over closed rows, open turns left out
    target:
      at_least: 0.5
    previous:
      - version: 1
        definition: accepted rows over all rows, open turns included
        target:
          at_least: 0.75
",
    );
    assert_eq!(
        checked("lowered-target", Some(&lowered)),
        ("redefined".to_owned(), false)
    );
    // A metric whose figure no code computes cannot print: added, or as an uncomputed version.
    let added = format!(
        "{DEFINED}  escaped_defects:\n    version: 1\n    definition: escaped defects, human-sourced\n"
    );
    assert_eq!(
        checked("uncomputed-metric", Some(&added)),
        ("redefined".to_owned(), false)
    );
    let uncomputed = with_accepted_share(
        "  accepted_share:
    version: 3
    definition: accepted rows over the rows of the last week
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
        checked("uncomputed-version", Some(&uncomputed)),
        ("redefined".to_owned(), false)
    );
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

/// Moves the turn `id`'s `baseline` revision to `revision`.
fn on_baseline(tree: &Path, id: &str, revision: &str) {
    at_revision(tree, id, "baseline", revision);
}

/// Moves the turn `id`'s `artifact` to `revision`.
fn at_revision(tree: &Path, id: &str, artifact: &str, revision: &str) {
    let path = tree.join(".engineering/cases").join(id).join("case.yaml");
    let mut doc: serde_yaml_ng::Value =
        serde_yaml_ng::from_str(&fs::read_to_string(&path).expect("case.yaml")).expect("yaml");
    doc["artifacts"][artifact]["revision"] = revision.into();
    fs::write(&path, serde_yaml_ng::to_string(&doc).expect("yaml")).expect("case.yaml");
}

/// A turn closed with `verdict` on a baseline that holds everything committed so far.
fn after(tree: &Path, id: &str, at: &str, verdict: &str) {
    closed(tree, id, at, at, verdict, None);
    on_baseline(tree, id, &git(tree, &["rev-parse", "HEAD"]));
}

/// `loop.compare`: the turns whose baseline holds the merged candidate against the turns the
/// case's measurement of the baseline read, on the metrics below their target there: `improved`
/// when each is better after, else `not_improved`. Fewer than three turns with a verdict on such
/// a baseline record nothing, open turns and turns on another baseline not counted; so does a
/// case with no measurement of the baseline, or no merge of the candidate.
#[test]
fn the_loop_comparison_sets_the_turns_on_the_candidate_against_those_measured() {
    let (tree, _) = scratch("compare", DEFINED);
    for (id, at, verdict) in [
        ("before-a", "2026-10-18T09:00:00Z", "accepted"),
        ("before-b", "2026-10-18T09:10:00Z", "declined"),
        ("before-c", "2026-10-18T09:20:00Z", "declined"),
    ] {
        closed(&tree, id, at, at, verdict, None);
    }
    let base = git(&tree, &["rev-parse", "HEAD"]);
    let candidate = commit(&tree, DEFINED, "the loop candidate");
    let mut case = loop_case(&tree, "loop-compare", &base, &candidate);

    let observation = LoopCompare.observe(&tree, &candidate, 1).expect("observes");
    assert!(observation.records.is_empty(), "{}", observation.summary);
    assert!(
        observation.summary.contains("no merge"),
        "{}",
        observation.summary
    );
    merged(&mut case, "2026-10-18T10:00:00Z");
    let observation = LoopCompare.observe(&tree, &candidate, 2).expect("observes");
    assert!(observation.records.is_empty(), "{}", observation.summary);
    assert!(
        observation.summary.contains("no loop measurement"),
        "{}",
        observation.summary
    );

    let measurement = LoopMeasure.observe(&tree, &base, 3).expect("measures");
    assert_eq!(measurement.records.len(), 1, "{}", measurement.summary);
    let retained = measurement.artifact.expect("the measurement retained");
    fs::create_dir_all(case.dir.join("artifacts")).expect("artifacts");
    fs::write(
        case.dir.join("artifacts").join(&retained.name),
        &retained.bytes,
    )
    .expect("retained");
    case.add_evidence(&measurement.records[0]).expect("written");

    after(&tree, "after-a", "2026-10-18T11:00:00Z", "accepted");
    after(&tree, "after-b", "2026-10-18T11:10:00Z", "accepted");
    turn(
        &tree,
        "after-open",
        None,
        &[At {
            at: "2026-10-18T11:15:00Z",
            holds: &["finding.cited"],
            legitimate: None,
        }],
        None,
    );
    on_baseline(&tree, "after-open", &git(&tree, &["rev-parse", "HEAD"]));
    // Accepted, but on the baseline, which does not hold the candidate: not a turn after.
    closed(
        &tree,
        "elsewhere",
        "2026-10-18T11:16:00Z",
        "2026-10-18T11:16:00Z",
        "accepted",
        None,
    );
    on_baseline(&tree, "elsewhere", &base);
    let observation = LoopCompare.observe(&tree, &candidate, 4).expect("observes");
    assert!(observation.records.is_empty(), "{}", observation.summary);
    assert!(
        observation.summary.contains("2 turns with a verdict"),
        "{}",
        observation.summary
    );

    after(&tree, "after-c", "2026-10-18T11:20:00Z", "declined");
    let observation = LoopCompare.observe(&tree, &candidate, 5).expect("observes");
    assert_eq!(observation.records.len(), 1, "{}", observation.summary);
    let record = &observation.records[0];
    assert_eq!(record.kind, "loop_comparison");
    assert_eq!(record.subject, "loop_candidate");
    assert_eq!(record.subject_revision, candidate);
    // Accepted share 1/3 before, 2/4 after, the open turn counted: improved.
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
    assert_eq!(
        ids(&retained["before"]["turns"]),
        ["before-a", "before-b", "before-c"],
        "{retained}"
    );
    assert_eq!(
        ids(&retained["after"]["turns"]),
        ["after-a", "after-b", "after-c", "after-open"],
        "{retained}"
    );

    // Accepted share 2/6 after against 1/3 before is no better.
    after(&tree, "after-d", "2026-10-18T11:30:00Z", "declined");
    after(&tree, "after-e", "2026-10-18T11:40:00Z", "declined");
    let observation = LoopCompare.observe(&tree, &candidate, 6).expect("observes");
    assert_eq!(
        observation.records[0].result.as_deref(),
        Some("not_improved"),
        "{}",
        observation.summary
    );
}

/// The definitions this repository ships parse, every figure they define is one the code
/// computes, current and previous versions alike, and they hold the planner's targets: accepted
/// and promoted share at least 0.8, finding to verified at most a day, none for the turns and
/// the median tokens.
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
    let target = |metric: &str| {
        definitions.metrics[metric]
            .target
            .map(|target| (target.at_least, target.at_most))
    };
    assert_eq!(target("accepted_share"), Some((Some(0.8), None)));
    assert_eq!(target("promoted_share"), Some((Some(0.8), None)));
    assert_eq!(target("finding_to_verified"), Some((None, Some(86400.0))));
    assert_eq!(target("median_tokens_per_accepted_turn"), None);
    assert_eq!(target("turns"), None);
    LoopScorecard::default()
        .figures(&definitions)
        .expect("every figure is computed");
}

/// A judgement step plants `refs/replace/<base>` pointing the baseline at the candidate. While it
/// is there, what takt reads at the base is still the base's: the run's judge-surface list
/// (`takt::run::judge_list_at`) and the baseline's metric definitions in
/// `loop.definition.check`. The checkpoint the step was measured against names the ref as a
/// change, and its restore removes it.
#[test]
fn a_planted_replace_ref_changes_no_read_at_the_base_and_the_restore_removes_it() {
    let (tree, _) = scratch("replace-ref", DEFINED);
    fs::create_dir_all(tree.join(".engineering")).expect("the directory");
    fs::write(tree.join(takt::run::JUDGE_LIST), "- src/**\n").expect("the list");
    let base = commit(&tree, DEFINED, "the base");
    fs::write(tree.join(takt::run::JUDGE_LIST), "- docs/**\n").expect("the list");
    let silent = with_accepted_share(
        "  accepted_share:
    version: 1
    definition: accepted rows over closed rows, open turns left out
    target:
      at_least: 0.1
",
    );
    let head = commit(&tree, &silent, "the candidate");
    let _case = loop_case(&tree, "loop-replace-ref", &base, &head);

    let checkpoint = takt::git::Checkpoint::take(&tree).expect("a checkpoint");
    git(&tree, &["replace", &base, &head]);
    assert_eq!(
        git(
            &tree,
            &["for-each-ref", "--format=%(refname)", "refs/replace/"]
        ),
        format!("refs/replace/{base}"),
        "the step planted its ref"
    );
    assert_eq!(
        takt::run::judge_list_at(&tree, &base).expect("reads"),
        Ok(vec!["src/**".to_owned()]),
        "the judge-surface list at the base is the base's"
    );
    let observation = LoopDefinitionCheck
        .observe(&tree, &head, 1)
        .expect("observes");
    assert_eq!(
        observation.records[0].result.as_deref(),
        Some("redefined"),
        "the baseline's definitions are the base's: {}",
        observation.summary
    );

    let change = checkpoint.change(&tree).expect("the change");
    assert!(
        change
            .refs
            .iter()
            .any(|moved| moved.starts_with(&format!("refs/replace/{base}"))),
        "{change:?}"
    );
    checkpoint.restore(&tree).expect("restored");
    assert_eq!(
        git(
            &tree,
            &["for-each-ref", "--format=%(refname)", "refs/replace/"]
        ),
        "",
        "the restore removed the planted ref"
    );
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

/// A linked worktree of a scratch repository: the repository, the worktree, and the worktree's
/// own git directory.
fn linked(name: &str) -> (Scratch, Scratch, PathBuf) {
    let (main, _) = scratch(&format!("{name}-main"), DEFINED);
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("loop-scorecard-{}-{name}-tree", std::process::id()));
    if dir.exists() {
        fs::remove_dir_all(&dir).expect("a stale scratch tree is removed");
    }
    git(
        &main,
        &[
            "worktree",
            "add",
            "-q",
            "-b",
            "work",
            dir.to_str().expect("utf-8"),
        ],
    );
    let own = PathBuf::from(git(&dir, &["rev-parse", "--absolute-git-dir"]));
    (main, Scratch(dir), own)
}

/// What a step does to `file` between a checkpoint of `tree` and its put-back and restore; the
/// file's bytes after them, none when it is absent.
fn after_step(tree: &Path, file: &Path, step: impl FnOnce()) -> Option<Vec<u8>> {
    let checkpoint = takt::git::Checkpoint::take(tree).expect("a checkpoint");
    step();
    let put_back = checkpoint.put_back_git_dir(tree);
    assert!(put_back.failed.is_empty(), "{put_back:?}");
    assert!(
        put_back
            .touched
            .iter()
            .any(|path| path.ends_with(&*file.file_name().expect("a name").to_string_lossy())),
        "the put-back names {}: {put_back:?}",
        file.display()
    );
    checkpoint.restore(tree).expect("restored");
    fs::read(file).ok()
}

/// A linked worktree's `commondir` a step points at a copy of the common git directory is put
/// back: every later read of the worktree is the repository's again.
#[test]
fn the_checkpoint_puts_back_a_linked_worktrees_commondir() {
    let (main, tree, own) = linked("guard-commondir");
    let file = own.join("commondir");
    let before = fs::read(&file).expect("commondir");
    let copy = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!(
        "loop-scorecard-{}-guard-commondir-copy",
        std::process::id()
    ));
    let copy = Scratch(copy);
    let after = after_step(&tree, &file, || {
        let status = Command::new("cp")
            .arg("-r")
            .arg(main.join(".git"))
            .arg(&*copy)
            .status()
            .expect("cp runs");
        assert!(status.success(), "the copy is made");
        fs::write(&file, format!("{}\n", copy.display())).expect("commondir");
    });
    assert_eq!(after, Some(before));
}

/// A linked worktree's `gitdir` a step rewrites is put back.
#[test]
fn the_checkpoint_puts_back_a_linked_worktrees_gitdir() {
    let (_main, tree, own) = linked("guard-gitdir");
    let file = own.join("gitdir");
    let before = fs::read(&file).expect("gitdir");
    let after = after_step(&tree, &file, || {
        fs::write(&file, "/elsewhere/.git\n").expect("gitdir");
    });
    assert_eq!(after, Some(before));
}

/// A `shallow` file a step writes, which would cut the history every read of a filing or an
/// ancestry rests on, is removed.
#[test]
fn the_checkpoint_removes_a_shallow_file_a_step_writes() {
    let (tree, head) = scratch("guard-shallow", DEFINED);
    let file = tree.join(".git/shallow");
    let after = after_step(&tree, &file, || {
        fs::write(&file, format!("{head}\n")).expect("shallow");
    });
    assert_eq!(after, None);
}

/// An `objects/info/alternates` file a step writes, which would read another object directory
/// as the repository's, is removed.
#[test]
fn the_checkpoint_removes_an_alternates_file_a_step_writes() {
    let (tree, _) = scratch("guard-alternates", DEFINED);
    let (other, _) = scratch("guard-alternates-other", DEFINED);
    let file = tree.join(".git/objects/info/alternates");
    let after = after_step(&tree, &file, || {
        fs::create_dir_all(file.parent().expect("a parent")).expect("objects/info");
        fs::write(&file, format!("{}\n", other.join(".git/objects").display()))
            .expect("alternates");
    });
    assert_eq!(after, None);
}

/// A finding whose store commit comes after its verification has no filing time, never a
/// negative time from finding to verified.
#[test]
fn a_filing_later_than_its_verification_is_no_filing_time() {
    let (tree, _) = scratch("late-filing", DEFINED);
    closed(
        &tree,
        "turn-a",
        "2026-10-18T09:00:00Z",
        "2026-10-18T10:00:00Z",
        "accepted",
        None,
    );
    file(&tree, "turn-late", "2026-10-18T19:00:00Z");
    turn(
        &tree,
        "turn-late",
        Some("turn-late"),
        &[At {
            at: "2026-10-18T16:00:00Z",
            holds: &["finding.cited", "scenario.passes_after"],
            legitimate: None,
        }],
        None,
    );
    let head = git(&tree, &["rev-parse", "HEAD"]);
    let card = LoopScorecard::read_at(&tree, Some(&head)).expect("reads");
    let late = card
        .rows
        .iter()
        .find(|row| row.turn == "turn-late")
        .expect("the row");
    assert_eq!(late.verified_at.as_deref(), Some("2026-10-18T16:00:00Z"));
    assert_eq!(late.filed_at, None);
    let figures = card
        .figures(&Definitions::parse(DEFINED).expect("parses"))
        .expect("computes");
    let time = figures
        .iter()
        .find(|figure| figure.metric == "finding_to_verified")
        .expect("the figure");
    assert_eq!(time.cases, ["turn-a"]);
    assert_eq!(time.value, Some(3600.0));
}

/// A figure with a target and no value fails closed: no data is below target, and a
/// measurement over turns that give none answers `below_target`.
#[test]
fn a_target_with_no_figure_to_hold_to_it_is_below_target() {
    let definitions = Definitions::parse(
        "format: takt-loop-metrics/1
metrics:
  finding_to_verified:
    version: 1
    definition: the median seconds from filed_at to verified_at over the rows with both
    target:
      at_most: 86400
  turns:
    version: 1
    definition: the improvement turns recorded
",
    )
    .expect("parses");
    let figures = LoopScorecard::default()
        .figures(&definitions)
        .expect("computes");
    let time = figures
        .iter()
        .find(|figure| figure.metric == "finding_to_verified")
        .expect("the figure");
    assert_eq!(time.value, None);
    assert!(time.below_target, "{time:?}");
    let turns = figures
        .iter()
        .find(|figure| figure.metric == "turns")
        .expect("the figure");
    assert!(!turns.below_target, "no target, never below it: {turns:?}");
}

/// `loop.definition.check` reads the baseline's definitions only from the snapshot taken when
/// the case opened: none, or one of another baseline, records nothing; with one, a base whose
/// objects a step rewrote afterwards is still judged as it was.
#[test]
fn the_definition_check_reads_the_baseline_only_from_the_snapshot_taken_at_open() {
    let (tree, base) = scratch("snapshot", DEFINED);
    let silent = with_accepted_share(
        "  accepted_share:
    version: 1
    definition: accepted rows over closed rows, open turns left out
    target:
      at_least: 0.75
",
    );
    let head = commit(&tree, &silent, "candidate");
    let case = loop_case(&tree, "loop-snapshot", &base, &head);
    let snapshot = case.dir.join(takt::loop_scorecard::BASELINE_SNAPSHOT);
    let taken = fs::read(&snapshot).expect("the snapshot was taken at open");

    fs::remove_file(&snapshot).expect("removed");
    let observation = LoopDefinitionCheck
        .observe(&tree, &head, 1)
        .expect("observes");
    assert!(observation.records.is_empty(), "{}", observation.summary);
    assert!(
        observation.summary.contains("no snapshot"),
        "{}",
        observation.summary
    );

    let mut other: serde_json::Value = serde_json::from_slice(&taken).expect("json");
    other["revision"] = head.clone().into();
    fs::write(&snapshot, serde_json::to_vec(&other).expect("json")).expect("written");
    let observation = LoopDefinitionCheck
        .observe(&tree, &head, 2)
        .expect("observes");
    assert!(observation.records.is_empty(), "{}", observation.summary);

    fs::write(&snapshot, &taken).expect("written");
    let observation = LoopDefinitionCheck
        .observe(&tree, &head, 3)
        .expect("observes");
    assert_eq!(
        observation.records[0].result.as_deref(),
        Some("redefined"),
        "{}",
        observation.summary
    );
}

/// The reviewer's forging program: rewrites one commit's first parent in a commit-graph file
/// and fixes its checksum.
const FORGE_GRAPH: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/fixtures/forge_commit_graph.py"
);

/// Whether plain git, which reads the commit-graph, takes `ancestor` for an ancestor of
/// `revision` in `tree`.
fn plain_ancestor(tree: &Path, ancestor: &str, revision: &str) -> bool {
    Command::new("git")
        .args(["merge-base", "--is-ancestor", ancestor, revision])
        .current_dir(tree)
        .status()
        .expect("git runs")
        .success()
}

/// The reviewer's probe C. A step forges `objects/info/commit-graph`, re-pointing a commit's
/// parent at a commit off its history, its checksum fixed: plain git then takes the side commit
/// for an ancestor. The ancestry takt reads (a turn's baseline descending from the loop
/// candidate, the history a filing is read in) never reads the commit-graph, even while the
/// forged file is there, and the checkpoint puts the file back.
#[test]
fn probe_c_forged_commit_graph_survives_the_restore() {
    let (tree, _) = scratch("commit-graph", DEFINED);
    git(&tree, &["checkout", "-q", "-b", "side"]);
    let side = commit(&tree, DEFINED, "a commit off the history");
    git(&tree, &["checkout", "-q", "main"]);
    let target = commit(&tree, DEFINED, "the commit whose parent is forged");
    git(&tree, &["commit-graph", "write", "--reachable"]);
    let graph = tree.join(".git/objects/info/commit-graph");
    let before = fs::read(&graph).expect("the commit-graph is written");
    assert!(
        !plain_ancestor(&tree, &side, &target),
        "side is off the history"
    );

    let checkpoint = takt::git::Checkpoint::take(&tree).expect("a checkpoint");
    let forged = Command::new("python3")
        .arg("-I")
        .arg(FORGE_GRAPH)
        .arg(&graph)
        .arg(&target)
        .arg(&side)
        .status()
        .expect("python3 runs");
    assert!(forged.success(), "the commit-graph is forged");
    assert!(
        plain_ancestor(&tree, &side, &target),
        "the forged commit-graph makes plain git take side for an ancestor"
    );
    assert!(
        takt::git::run(&tree, &["merge-base", "--is-ancestor", &side, &target]).is_err(),
        "the ancestry takt reads ignores the forged commit-graph"
    );

    let put_back = checkpoint.put_back_git_dir(&tree);
    assert!(put_back.failed.is_empty(), "{put_back:?}");
    assert!(
        put_back
            .touched
            .iter()
            .any(|path| path.ends_with("commit-graph")),
        "the put-back names the commit-graph: {put_back:?}"
    );
    checkpoint.restore(&tree).expect("restored");
    assert_eq!(
        fs::read(&graph).ok(),
        Some(before),
        "the commit-graph is put back"
    );
    assert!(
        !plain_ancestor(&tree, &side, &target),
        "side is off the history again"
    );
}

/// A split commit-graph a step writes (`objects/info/commit-graphs/`) is removed.
#[test]
fn the_checkpoint_removes_a_split_commit_graph_a_step_writes() {
    let (tree, _) = scratch("commit-graphs", DEFINED);
    commit(&tree, DEFINED, "a second commit");
    let dir = tree.join(".git/objects/info/commit-graphs");
    let checkpoint = takt::git::Checkpoint::take(&tree).expect("a checkpoint");
    git(
        &tree,
        &["commit-graph", "write", "--reachable", "--split=replace"],
    );
    assert!(dir.is_dir(), "the split commit-graph is written");
    let put_back = checkpoint.put_back_git_dir(&tree);
    assert!(put_back.failed.is_empty(), "{put_back:?}");
    checkpoint.restore(&tree).expect("restored");
    assert!(!dir.exists(), "the split commit-graph is removed");
}

/// A target that is not a finite number is refused: `.nan` would never be missed.
#[test]
fn a_target_that_is_not_a_finite_number_is_refused() {
    for bound in ["at_least: .nan", "at_most: .inf", "at_least: -.inf"] {
        let text = format!(
            "format: takt-loop-metrics/1\nmetrics:\n  accepted_share:\n    version: 1\n    definition: accepted rows over all rows\n    target:\n      {bound}\n"
        );
        assert!(Definitions::parse(&text).is_err(), "{bound} is refused");
    }
}

/// A target no figure of its metric can take is refused: a share outside 0 to 1, a negative
/// time, count or token figure. One inside the range reads.
#[test]
fn a_target_outside_its_metrics_range_is_refused() {
    let definitions = |metric: &str, bound: &str| {
        format!(
            "format: takt-loop-metrics/1\nmetrics:\n  {metric}:\n    version: 1\n    definition: the figure\n    target:\n      {bound}\n"
        )
    };
    for (metric, bound) in [
        ("accepted_share", "at_least: 1.5"),
        ("accepted_share", "at_least: -0.1"),
        ("promoted_share", "at_most: 2"),
        ("finding_to_verified", "at_most: -60"),
        ("median_tokens_per_accepted_turn", "at_most: -1"),
        ("turns", "at_least: -3"),
    ] {
        assert!(
            Definitions::parse(&definitions(metric, bound)).is_err(),
            "{metric} {bound} is refused"
        );
    }
    for (metric, bound) in [
        ("accepted_share", "at_least: 1"),
        ("promoted_share", "at_least: 0"),
        ("finding_to_verified", "at_most: 0"),
        ("median_tokens_per_accepted_turn", "at_most: 200000"),
    ] {
        Definitions::parse(&definitions(metric, bound))
            .unwrap_or_else(|error| panic!("{metric} {bound}: {error:#}"));
    }
}

/// `loop.measure` and `loop.compare` take the baseline's definitions from the snapshot the case
/// took when it opened, never from the object store: a snapshot whose target differs from the
/// base's file decides the measurement, and with no snapshot neither records anything.
#[test]
fn measure_and_compare_read_the_baseline_only_from_the_snapshot() {
    let (tree, _) = scratch("measure-snapshot", DEFINED);
    for (id, verdict) in [
        ("turn-a", "accepted"),
        ("turn-b", "accepted"),
        ("turn-c", "declined"),
    ] {
        closed(
            &tree,
            id,
            "2026-10-18T09:00:00Z",
            "2026-10-18T10:00:00Z",
            verdict,
            None,
        );
    }
    let base = git(&tree, &["rev-parse", "HEAD"]);
    let case = loop_case(&tree, "loop-measure-snapshot", &base, &base);
    let path = case.dir.join(takt::loop_scorecard::BASELINE_SNAPSHOT);
    // At the base, accepted share 2/3 misses its 0.75: below target.
    let observation = LoopMeasure.observe(&tree, &base, 1).expect("measures");
    assert_eq!(
        observation.records[0].result.as_deref(),
        Some("below_target"),
        "{}",
        observation.summary
    );
    // The snapshot, not the object store, holds the base: one whose target is 0.5 is at target.
    let mut snapshot: serde_json::Value =
        serde_json::from_slice(&fs::read(&path).expect("the snapshot")).expect("json");
    snapshot["definitions"] = DEFINED.replace("at_least: 0.75", "at_least: 0.5").into();
    fs::write(&path, serde_json::to_vec(&snapshot).expect("json")).expect("written");
    let observation = LoopMeasure.observe(&tree, &base, 2).expect("measures");
    assert_eq!(
        observation.records[0].result.as_deref(),
        Some("at_target"),
        "{}",
        observation.summary
    );

    fs::remove_file(&path).expect("removed");
    let observation = LoopMeasure.observe(&tree, &base, 3).expect("measures");
    assert!(observation.records.is_empty(), "{}", observation.summary);
    let mut case = case;
    merged(&mut case, "2026-10-18T11:00:00Z");
    let observation = LoopCompare.observe(&tree, &base, 4).expect("compares");
    assert!(observation.records.is_empty(), "{}", observation.summary);
    assert!(
        observation.summary.contains("no snapshot"),
        "{}",
        observation.summary
    );
}

/// A finding is filed after the baseline it was observed on and before the candidate that
/// fixes it: its filing is read in the history the turn's `finding` artifact was opened at,
/// or, while that names no commit, in the candidate's; never in the baseline's, which does not
/// hold it.
#[test]
fn a_finding_filed_after_the_baseline_is_read_at_the_finding_or_the_candidate() {
    let (tree, baseline) = scratch("filed-after-baseline", DEFINED);
    file(&tree, "turn-a", "2026-10-18T09:30:00Z");
    let filed = file(&tree, "turn-b", "2026-10-18T09:40:00Z");
    file(&tree, "turn-c", "2026-10-18T09:50:00Z");
    let candidate = commit(&tree, DEFINED, "the candidate");
    let decided = [At {
        at: "2026-10-18T10:00:00Z",
        holds: &["finding.cited", "scenario.passes_after"],
        legitimate: None,
    }];
    for id in ["turn-a", "turn-b", "turn-c"] {
        turn(&tree, id, Some(id), &decided, None);
        on_baseline(&tree, id, &baseline);
    }
    // turn-a: its finding opened at no commit (`r0`), its candidate after the filing.
    at_revision(&tree, "turn-a", "finding", "r0");
    at_revision(&tree, "turn-a", "candidate", &candidate);
    // turn-b: its finding opened at the filing, its candidate at the baseline.
    at_revision(&tree, "turn-b", "finding", &filed);
    at_revision(&tree, "turn-b", "candidate", &baseline);
    // turn-c: no finding commit, its candidate before the filing: nothing to read.
    at_revision(&tree, "turn-c", "finding", "r0");
    at_revision(&tree, "turn-c", "candidate", &baseline);
    let card = LoopScorecard::read_at(&tree, Some(&candidate)).expect("reads");
    let filed_at = |id: &str| {
        card.rows
            .iter()
            .find(|row| row.turn == id)
            .expect("the row")
            .filed_at
            .clone()
    };
    assert_eq!(filed_at("turn-a").as_deref(), Some("2026-10-18T09:30:00Z"));
    assert_eq!(filed_at("turn-b").as_deref(), Some("2026-10-18T09:40:00Z"));
    assert_eq!(filed_at("turn-c"), None);
}

/// `data` as a zlib stream of stored deflate blocks, as git reads a loose object.
fn zlib_stored(data: &[u8]) -> Vec<u8> {
    let mut out = vec![0x78, 0x01];
    let chunks: Vec<&[u8]> = if data.is_empty() {
        vec![&[]]
    } else {
        data.chunks(65535).collect()
    };
    for (n, chunk) in chunks.iter().enumerate() {
        out.push(u8::from(n + 1 == chunks.len()));
        let len = u16::try_from(chunk.len()).expect("a stored block");
        out.extend(len.to_le_bytes());
        out.extend((!len).to_le_bytes());
        out.extend(*chunk);
    }
    let (mut a, mut b) = (1u32, 0u32);
    for &byte in data {
        a = (a + u32::from(byte)) % 65521;
        b = (b + a) % 65521;
    }
    out.extend(((b << 16) | a).to_be_bytes());
    out
}

/// Writes, under the id `oid` in `tree`'s object directory, a loose object of `kind` holding
/// `content`: other bytes than the ones the id names.
fn plant_object(tree: &Path, kind: &str, oid: &str, content: &[u8]) {
    let path = tree.join(".git/objects").join(&oid[..2]).join(&oid[2..]);
    fs::create_dir_all(path.parent().expect("a parent")).expect("the object directory");
    if path.exists() {
        fs::remove_file(&path).expect("the object is removed");
    }
    let mut raw = format!("{kind} {}\0", content.len()).into_bytes();
    raw.extend_from_slice(content);
    fs::write(&path, zlib_stored(&raw)).expect("the object is written");
}

/// The reviewer's H1: a loose object planted over the definitions blob under its id. Plain git
/// reads the planted bytes; `Definitions::at` checks every object on the way from the commit and
/// refuses, naming the object.
#[test]
fn definitions_read_from_a_planted_object_are_refused_naming_it() {
    let (tree, head) = scratch("planted-definitions", DEFINED);
    let blob = git(&tree, &["rev-parse", &format!("{head}:{DEFINITIONS}")]);
    plant_object(
        &tree,
        "blob",
        &blob,
        DEFINED
            .replace("at_least: 0.75", "at_least: 0.25")
            .as_bytes(),
    );
    assert!(
        git(&tree, &["show", &blob]).contains("at_least: 0.25"),
        "plain git reads the planted bytes"
    );
    let error = Definitions::at(&tree, &head).expect_err("the planted object is refused");
    assert!(format!("{error:#}").contains(&blob), "{error:#}");
}

/// The reviewer's H1 for ancestry: a commit object rewritten under its id with a parent off its
/// history makes plain git take the side commit for an ancestor; `verified_descends` checks the
/// commits on the way and refuses, naming the commit.
#[test]
fn an_ancestry_through_a_forged_commit_is_refused_naming_it() {
    let (tree, _) = scratch("forged-commit", DEFINED);
    git(&tree, &["checkout", "-q", "-b", "side"]);
    let side = commit(&tree, DEFINED, "a commit off the history");
    git(&tree, &["checkout", "-q", "main"]);
    let target = commit(&tree, DEFINED, "the commit whose parent is forged");
    assert!(
        !takt::loop_scorecard::verified_descends(&tree, &side, &target).expect("reads"),
        "side is off the history"
    );
    let real = git(&tree, &["cat-file", "commit", &target]);
    let parent = real
        .lines()
        .find_map(|line| line.strip_prefix("parent "))
        .expect("a parent")
        .to_owned();
    let forged = format!("{}\n", real.replace(&parent, &side));
    plant_object(&tree, "commit", &target, forged.as_bytes());
    let plain = Command::new("git")
        .args(["merge-base", "--is-ancestor", &side, &target])
        .current_dir(&*tree)
        .status()
        .expect("git runs");
    assert!(plain.success(), "plain git takes side for an ancestor");
    let error = takt::loop_scorecard::verified_descends(&tree, &side, &target)
        .expect_err("the forged commit is refused");
    assert!(format!("{error:#}").contains(&target), "{error:#}");
}

/// A share's target is 0 to 1 however the metric's id is spelled: a target of 5 is refused for
/// each share metric.
#[test]
fn a_share_target_of_five_is_refused_however_the_share_is_spelled() {
    for metric in [
        "accepted_share",
        "promoted_share",
        "ACCEPTED_SHARE",
        "promoted_shares",
        "Promoted_Share",
    ] {
        let text = format!(
            "format: takt-loop-metrics/1\nmetrics:\n  {metric}:\n    version: 1\n    definition: the share\n    target:\n      at_least: 5\n"
        );
        assert!(
            Definitions::parse(&text).is_err(),
            "{metric} at_least 5 is refused"
        );
    }
}

/// The reviewer's M3 (rv6_probe): a commit on the way rewritten under its id with no parent
/// cuts the history, so plain git no longer finds the true ancestor. The walk reads that commit
/// through the checked reader and refuses it, naming it: an error, never `false`.
#[test]
fn a_forged_parentless_commit_on_the_way_is_an_error_not_false() {
    let (tree, root) = scratch("forged-parentless", DEFINED);
    let middle = commit(&tree, DEFINED, "the commit whose parent is cut");
    let tip = commit(&tree, DEFINED, "the tip");
    assert!(
        takt::loop_scorecard::verified_descends(&tree, &root, &tip).expect("reads"),
        "the root is an ancestor of the tip"
    );
    let real = git(&tree, &["cat-file", "commit", &middle]);
    let cut: String = real
        .lines()
        .filter(|line| !line.starts_with("parent "))
        .map(|line| format!("{line}\n"))
        .collect();
    plant_object(&tree, "commit", &middle, cut.as_bytes());
    let plain = Command::new("git")
        .args(["merge-base", "--is-ancestor", &root, &tip])
        .current_dir(&*tree)
        .status()
        .expect("git runs");
    assert!(
        !plain.success(),
        "plain git lost the root from the tip's history"
    );
    let error = takt::loop_scorecard::verified_descends(&tree, &root, &tip)
        .expect_err("the forged commit is an error, not false");
    assert!(format!("{error:#}").contains(&middle), "{error:#}");
}

/// The reviewer's L3: the walk reads a 20,000-commit history through one `git cat-file --batch`
/// and checks each commit in this process, in a debug build under 10 s, both when the ancestor
/// is the root (found at the end) and when it is off the history (the whole history walked).
#[test]
fn a_twenty_thousand_commit_history_is_walked_within_ten_seconds() {
    let (tree, _) = scratch("long-history", DEFINED);
    let mut stream = String::new();
    for n in 1..=20_000 {
        stream.push_str(&format!(
            "commit refs/heads/long\nmark :{n}\ncommitter t <t@localhost> {} +0000\ndata 2\n{}\n\n",
            1_700_000_000 + n,
            n % 10
        ));
        if n > 1 {
            stream.push_str(&format!("from :{}\n\n", n - 1));
        }
    }
    let mut import = Command::new("git")
        .args(["fast-import", "--quiet"])
        .current_dir(&*tree)
        .stdin(std::process::Stdio::piped())
        .spawn()
        .expect("git fast-import runs");
    {
        use std::io::Write as _;
        let mut stdin = import.stdin.take().expect("stdin");
        stdin.write_all(stream.as_bytes()).expect("the stream");
    }
    assert!(
        import.wait().expect("waits").success(),
        "the history is imported"
    );
    let tip = git(&tree, &["rev-parse", "long"]);
    let root = git(&tree, &["rev-list", "--max-parents=0", "long"]);
    let main = git(&tree, &["rev-parse", "main"]);
    let started = std::time::Instant::now();
    assert!(
        takt::loop_scorecard::verified_descends(&tree, &root, &tip).expect("walks"),
        "the root is an ancestor of the tip"
    );
    assert!(
        !takt::loop_scorecard::verified_descends(&tree, &main, &tip).expect("walks"),
        "main's commit is off the long history"
    );
    let took = started.elapsed();
    assert!(
        took < std::time::Duration::from_secs(10),
        "two walks of 20,000 commits took {took:?}"
    );
}

/// The reviewer's L5: a loose object planted over the definitions blob whose header gives a
/// size of `u64::MAX`. The read refuses it before allocating anything, naming the object and
/// its size, instead of overflowing.
#[test]
fn an_object_whose_header_gives_the_largest_size_is_refused_naming_it() {
    let (tree, head) = scratch("largest-size", DEFINED);
    let blob = git(&tree, &["rev-parse", &format!("{head}:{DEFINITIONS}")]);
    let path = tree.join(".git/objects").join(&blob[..2]).join(&blob[2..]);
    fs::remove_file(&path).expect("the object is removed");
    let mut raw = format!("blob {}\0", u64::MAX).into_bytes();
    raw.extend_from_slice(DEFINED.as_bytes());
    fs::write(&path, zlib_stored(&raw)).expect("the object is written");
    let error = Definitions::at(&tree, &head).expect_err("the object is refused");
    let shown = format!("{error:#}");
    assert!(shown.contains(&blob), "{shown}");
    assert!(shown.contains(&u64::MAX.to_string()), "{shown}");
}

/// The planner's low finding: a loop validator is held to the run's deadline as well as its own
/// 10 s per read. The candidate's definitions blob is planted with a header that overstates its
/// size, so reading it blocks; with the run ending 1 s from now, `loop.definition.check` ends
/// with an error naming the blob within 5 s, not at the 10 s read deadline.
#[test]
fn a_loop_validator_is_held_to_the_runs_deadline() {
    let (tree, base) = scratch("run-deadline", DEFINED);
    let candidate = commit(
        &tree,
        &DEFINED.replace("at_least: 0.75", "at_least: 0.8"),
        "candidate",
    );
    let _case = loop_case(&tree, "loop-run-deadline", &base, &candidate);
    let blob = git(&tree, &["rev-parse", &format!("{candidate}:{DEFINITIONS}")]);
    let path = tree.join(".git/objects").join(&blob[..2]).join(&blob[2..]);
    fs::remove_file(&path).expect("the object is removed");
    let mut raw = b"blob 100000\0".to_vec();
    raw.extend_from_slice(DEFINED.as_bytes());
    fs::write(&path, zlib_stored(&raw)).expect("the object is written");
    let started = std::time::Instant::now();
    let observed = LoopDefinitionCheck.observe_within(
        &tree,
        &candidate,
        1,
        Some(started + std::time::Duration::from_secs(1)),
    );
    let took = started.elapsed();
    assert!(
        took < std::time::Duration::from_secs(5),
        "the observation ended by the run's deadline: it took {took:?}"
    );
    let error = observed.expect_err("the read past the run's deadline is an error");
    assert!(format!("{error:#}").contains(&blob), "{error:#}");
}

/// The second review's planter of a commit bomb: writes, under a commit's id, a loose commit
/// whose body is the commit's bytes and `n` bytes more, compressed.
const PLANT_BOMB: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/fixtures/plant_commit_bomb.py"
);

/// A repository whose `HEAD` filed the finding `turn-a` (its store file committed at 09:00)
/// and one recorded turn on it: the tree, the commit that added the store file, and `HEAD`.
fn filed_turn(name: &str) -> (Scratch, String, String) {
    let (tree, _) = scratch(name, DEFINED);
    closed(
        &tree,
        "turn-a",
        "2026-10-18T09:00:00Z",
        "2026-10-18T10:00:00Z",
        "accepted",
        None,
    );
    // `closed` files the finding: `HEAD` is the commit that added its store file.
    let head = git(&tree, &["rev-parse", "HEAD"]);
    let filing = head.clone();
    (tree, filing, head)
}

/// The second review's first high finding: the commit that added the finding's store file is
/// rewritten under its id with another committer time. Plain `git log` reads the new time; the
/// filing is read through the checked history and refused as a hash mismatch, naming the
/// commit.
#[test]
fn a_filing_commit_rewritten_under_its_id_is_refused() {
    let (tree, filing, head) = filed_turn("forged-filing");
    let real = git(&tree, &["cat-file", "commit", &filing]);
    let forged: String = real
        .lines()
        .map(|line| {
            if line.starts_with("committer ") {
                let mut words: Vec<&str> = line.split(' ').collect();
                let at = words.len() - 2;
                words[at] = "1791000000";
                words.join(" ")
            } else {
                line.to_owned()
            }
        })
        .map(|line| format!("{line}\n"))
        .collect();
    plant_object(&tree, "commit", &filing, forged.as_bytes());
    let error =
        LoopScorecard::read_at(&tree, Some(&head)).expect_err("the forged commit is refused");
    let shown = format!("{error:#}");
    assert!(shown.contains(&filing), "{shown}");
    assert!(shown.contains("hash") || shown.contains("id is"), "{shown}");
}

/// The second review's second high finding: a small loose commit that inflates to 64 MiB,
/// planted under the filing commit's id. It is refused at the commit cap, naming the commit and
/// the cap, before any of it is read.
#[test]
fn a_commit_bomb_is_refused_at_the_cap() {
    let (tree, filing, head) = filed_turn("commit-bomb");
    let content = tree.join(".git/bomb-content");
    fs::write(&content, git(&tree, &["cat-file", "commit", &filing])).expect("the content");
    let planted = Command::new("python3")
        .arg("-I")
        .arg(PLANT_BOMB)
        .arg(tree.join(".git"))
        .arg(&filing)
        .arg(&content)
        .arg((64u64 << 20).to_string())
        .stdout(std::process::Stdio::null())
        .status()
        .expect("python3 runs");
    assert!(planted.success(), "the bomb is planted");
    let started = std::time::Instant::now();
    let error = LoopScorecard::read_at(&tree, Some(&head)).expect_err("the bomb is refused");
    let shown = format!("{error:#}");
    assert!(shown.contains(&filing), "{shown}");
    assert!(
        shown.contains(&takt::git::MAX_COMMIT_OR_TREE.to_string()),
        "the cap is named: {shown}"
    );
    assert!(
        started.elapsed() < std::time::Duration::from_secs(10),
        "refused before reading"
    );
}

/// The second review's low finding on `parent` lines: git takes as parents only the `parent`
/// lines right after the tree. A commit whose `parent` line comes after its author line names
/// no parent, so the commit it points at is no ancestor.
#[test]
fn a_parent_line_after_another_header_names_no_parent() {
    let (tree, root) = scratch("misplaced-parent", DEFINED);
    let tree_id = git(&tree, &["rev-parse", &format!("{root}^{{tree}}")]);
    let bytes = format!(
        "tree {tree_id}\nauthor a <a@b> 1700000000 +0000\nparent {root}\ncommitter a <a@b> 1700000000 +0000\n\nmisplaced\n"
    );
    let mut hash = Command::new("git")
        .args([
            "hash-object",
            "-t",
            "commit",
            "-w",
            "--literally",
            "--stdin",
        ])
        .current_dir(&*tree)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .spawn()
        .expect("git runs");
    {
        use std::io::Write as _;
        hash.stdin
            .take()
            .expect("stdin")
            .write_all(bytes.as_bytes())
            .expect("the commit");
    }
    let output = hash.wait_with_output().expect("git ends");
    assert!(output.status.success(), "the commit is written");
    let misplaced = String::from_utf8(output.stdout)
        .expect("utf-8")
        .trim()
        .to_owned();
    assert!(
        !takt::loop_scorecard::verified_descends(&tree, &root, &misplaced).expect("reads"),
        "a parent line after the author line is no parent"
    );
}

/// The second review's YAML bomb (yaml_bomb.py): a 128 KiB anchored string and a `previous`
/// list of aliases to it. Read, the aliases would expand without bound (yaml_bomb.py's 30,000
/// to about 3.7 GiB; this case keeps 300 so a red run cannot exhaust memory). Definitions
/// holding an anchor or an alias are refused before they are parsed, naming the line.
#[test]
fn definitions_holding_yaml_aliases_are_refused_naming_the_line() {
    let big = "a".repeat(128 * 1024);
    let aliases = vec!["*p"; 300].join(", ");
    let bomb = format!(
        "format: takt-loop-metrics/1\nmetrics:\n  turns:\n    version: 1\n    definition: &s {big}\n    previous: [&p {{version: 2, definition: *s}}, {aliases}]\n"
    );
    let error = Definitions::parse(&bomb).expect_err("the anchors are refused");
    let shown = format!("{error:#}");
    assert!(shown.contains("line 5"), "{shown}");
    Definitions::parse(DEFINED).expect("definitions without anchors read");
}

/// The fourth review's hidden bomb (yaml_hidden.py, 32,000 aliases to a 128 KiB string, 259 KB
/// in all, under the definitions' cap): the anchor and the aliases sit on one flow line after
/// the plain scalar `a 'b`, whose `'` is no quote. A lexical scan took it for one and read on;
/// the aliases expanded to an out-of-memory abort. Read from the parser's events, the first
/// anchored node is refused, naming its line, within a second and before anything expands.
/// Run in a child test process under a 1 GiB address-space cap, so a red run aborts the child
/// and never exhausts the machine.
#[test]
fn the_hidden_alias_bomb_after_a_plain_scalar_quote_is_refused_within_a_second() {
    if std::env::var_os("TAKT_YAML_HIDDEN_CHILD").is_none() {
        use std::os::unix::process::CommandExt as _;
        let mut command = Command::new(std::env::current_exe().expect("the test binary"));
        command
            .args([
                "the_hidden_alias_bomb_after_a_plain_scalar_quote_is_refused_within_a_second",
                "--exact",
                "--test-threads",
                "1",
            ])
            .env("TAKT_YAML_HIDDEN_CHILD", "1");
        // SAFETY: setrlimit is async-signal-safe, and the closure touches nothing else. An
        // abort under the cap leaves no core file in the tree.
        unsafe {
            command.pre_exec(|| {
                for (resource, limit) in [(libc::RLIMIT_AS, 1 << 30), (libc::RLIMIT_CORE, 0)] {
                    let cap = libc::rlimit {
                        rlim_cur: limit,
                        rlim_max: limit,
                    };
                    if libc::setrlimit(resource, &raw const cap) != 0 {
                        return Err(std::io::Error::last_os_error());
                    }
                }
                Ok(())
            });
        }
        let output = command.output().expect("the child test runs");
        assert!(
            output.status.success(),
            "the child test failed ({}):\n{}\n{}",
            output.status,
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        return;
    }
    let big = "a".repeat(128 * 1024);
    let aliases = vec!["*p"; 32_000].join(", ");
    let bomb = format!(
        "format: takt-loop-metrics/1\nmetrics:\n  turns: {{version: 1, definition: a 'b, previous: [&p {{version: 2, definition: {big}}}, {aliases}]}}\n"
    );
    assert!(bomb.len() < 256 * 1024, "{} bytes", bomb.len());
    let started = std::time::Instant::now();
    let error = Definitions::parse(&bomb).expect_err("the anchor is refused");
    let took = started.elapsed();
    let shown = format!("{error:#}");
    assert!(shown.contains("line 3"), "{shown}");
    assert!(took < std::time::Duration::from_secs(1), "{took:?}");
}

/// Every anchored node and every alias is refused as the parser reads it, naming its line,
/// wherever it stands: after a plain scalar holding a quote of either kind, on a mapping, on a
/// sequence, after a tag, an alias whose anchor is never defined, an anchor never aliased. A
/// plain scalar holding `&` or `*` is text, not an anchor, and reads.
#[test]
fn every_anchor_and_alias_is_refused_from_the_parsers_events_naming_its_line() {
    let head = "format: takt-loop-metrics/1\nmetrics:\n";
    for (case, body, line) in [
        (
            "after a single quote in a plain scalar",
            "  turns: {version: 1, definition: a 'b, previous: [&p {version: 2, definition: x}]}\n",
            3,
        ),
        (
            "after a double quote in a plain scalar",
            "  turns: {version: 1, definition: a \"b, previous: [&p {version: 2, definition: x}]}\n",
            3,
        ),
        (
            "on a mapping",
            "  turns:\n    version: 1\n    definition: x\n    target: &t\n      at_least: 1\n",
            6,
        ),
        (
            "on a sequence",
            "  turns:\n    version: 2\n    definition: x\n    previous: &s\n      - {version: 1, definition: y}\n",
            6,
        ),
        (
            "after a tag",
            "  turns:\n    version: 1\n    definition: !!str &d x\n",
            5,
        ),
        (
            "an alias to no anchor",
            "  turns:\n    version: 1\n    definition: x\n    previous: [*nowhere]\n",
            6,
        ),
        (
            "an anchor never aliased",
            "  turns:\n    version: 1\n    definition: &d x\n",
            5,
        ),
    ] {
        let text = format!("{head}{body}");
        let error = Definitions::parse(&text).expect_err(case);
        let shown = format!("{error:#}");
        assert!(
            shown.contains(&format!("YAML anchor or alias on line {line};")),
            "{case}: {shown}"
        );
    }
    let text =
        format!("{head}  turns:\n    version: 1\n    definition: rows & cases *weighted, a 'b\n");
    let definitions = Definitions::parse(&text).expect("a plain scalar holding & and * reads");
    assert_eq!(
        definitions.metrics["turns"].definition,
        "rows & cases *weighted, a 'b"
    );
}

/// Every other YAML file the loop validators read is held to the same rule as the definitions:
/// a turn's `case.yaml` and `protocol.yaml`, and the loop case's evidence records, holding an
/// anchor or an alias are refused, naming the file and the line, before anything expands.
#[test]
fn every_yaml_file_the_loop_reads_is_refused_with_an_anchor_naming_the_file_and_line() {
    let (tree, head) = scratch("yaml-reads", DEFINED);
    closed(
        &tree,
        "turn-a",
        "2026-10-18T09:00:00Z",
        "2026-10-18T10:00:00Z",
        "accepted",
        None,
    );
    for name in ["case.yaml", "protocol.yaml"] {
        let path = tree.join(".engineering/cases/turn-a").join(name);
        let original = fs::read_to_string(&path).expect("the file");
        let line = original.lines().count() + 1;
        fs::write(&path, format!("{original}extra: &x [1]\n")).expect("the anchor");
        let error = LoopScorecard::read(&tree).expect_err(name);
        let shown = format!("{error:#}");
        assert!(shown.contains(name), "{shown}");
        assert!(
            shown.contains(&format!("YAML anchor or alias on line {line};")),
            "{shown}"
        );
        fs::write(&path, original).expect("restored");
    }
    LoopScorecard::read(&tree).expect("the turn reads without the anchor");
    let case = loop_case(&tree, "loop-yaml", &head, &head);
    fs::write(
        case.dir.join("evidence").join("zz.yaml"),
        "kind: merge_observation\nresult: *nowhere\n",
    )
    .expect("a record");
    let error = LoopCompare
        .observe(&tree, &head, 1)
        .expect_err("the record is refused");
    let shown = format!("{error:#}");
    assert!(shown.contains("zz.yaml"), "{shown}");
    assert!(shown.contains("YAML anchor or alias on line 2;"), "{shown}");
}

/// An abbreviated commit id is refused, naming it, and never resolved by a git subprocess
/// without the reader's deadline: a revision is read only as a full id of the repository's
/// object format, through the checked reader. A full id the repository does not hold, an id of
/// another format's length and a revision that is no id at all name no commit.
#[test]
fn an_abbreviated_revision_is_refused_naming_it() {
    let (tree, head) = scratch("abbreviated", DEFINED);
    let mut history = takt::loop_scorecard::History::open(&tree, None).expect("opens");
    assert_eq!(
        history.resolve(&head).expect("resolves").as_deref(),
        Some(head.as_str())
    );
    for short in [&head[..7], &head[..12], &head[..39]] {
        let error = history
            .resolve(short)
            .expect_err("an abbreviated id is refused");
        let shown = format!("{error:#}");
        assert!(shown.contains(short), "{shown}");
        assert!(shown.contains("abbreviated"), "{shown}");
        let error = Definitions::at(&tree, short).expect_err("an abbreviated id is refused");
        assert!(format!("{error:#}").contains(short), "{error:#}");
        let error = takt::loop_scorecard::verified_descends(&tree, short, &head)
            .expect_err("an abbreviated ancestor is refused");
        assert!(format!("{error:#}").contains(short), "{error:#}");
    }
    for none in ["0".repeat(40), "a".repeat(64), "r0".to_owned()] {
        assert_eq!(history.resolve(&none).expect("reads"), None, "{none}");
    }
}

/// The second review's medium finding on the definition check: the figures come from code
/// chosen by each metric's id and version, so a candidate could change a figure without
/// touching the YAML. The code counts as part of the definitions: a candidate that changes it
/// is `redefined`.
#[test]
fn a_candidate_that_changes_the_figures_code_is_redefined() {
    let (tree, _) = scratch("figures-code", DEFINED);
    let code = tree.join(takt::loop_scorecard::FIGURES_CODE);
    fs::create_dir_all(code.parent().expect("a parent")).expect("the directory");
    fs::write(&code, "// the figures as the baseline computes them\n").expect("the code");
    let base = commit(&tree, DEFINED, "the baseline");
    fs::write(&code, "// the accepted share computed otherwise\n").expect("the code");
    let candidate = commit(&tree, DEFINED, "the candidate");
    let _case = loop_case(&tree, "loop-figures-code", &base, &candidate);
    let observation = LoopDefinitionCheck
        .observe(&tree, &candidate, 1)
        .expect("observes");
    assert_eq!(
        observation.records[0].result.as_deref(),
        Some("redefined"),
        "{}",
        observation.summary
    );
}

/// The second review's medium finding on the walk: in a `--depth 2` clone of a merge, the
/// mainline parent is found first (first parent first), and an ancestor beyond the shallow
/// boundary is unknown, not an error.
#[test]
fn a_shallow_clone_of_a_merge_walks_first_parents_and_answers_unknown_past_its_boundary() {
    let (origin, _) = scratch("shallow-merge-origin", DEFINED);
    for n in 1..=4 {
        commit(&origin, DEFINED, &format!("c{n}"));
    }
    let early = git(&origin, &["rev-parse", "HEAD~3"]);
    git(&origin, &["checkout", "-q", "-b", "side", "HEAD~3"]);
    commit(&origin, DEFINED, "s1");
    commit(&origin, DEFINED, "s2");
    git(&origin, &["checkout", "-q", "main"]);
    let mainline = commit(&origin, DEFINED, "b");
    git(&origin, &["merge", "-q", "--no-ff", "-m", "m", "side"]);
    let merge = git(&origin, &["rev-parse", "HEAD"]);
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!(
        "loop-scorecard-{}-shallow-merge-clone",
        std::process::id()
    ));
    if dir.exists() {
        fs::remove_dir_all(&dir).expect("a stale clone is removed");
    }
    let url = format!("file://{}", origin.display());
    git(
        &origin,
        &[
            "clone",
            "-q",
            "--depth",
            "2",
            &url,
            dir.to_str().expect("utf-8"),
        ],
    );
    let clone = Scratch(dir);
    assert!(clone.join(".git/shallow").is_file(), "the clone is shallow");
    let mut history = takt::loop_scorecard::History::open(&clone, None).expect("opens");
    assert_eq!(
        history.descends(&mainline, &merge).expect("walks"),
        takt::loop_scorecard::Ancestry::Yes,
        "the mainline parent is an ancestor"
    );
    assert_eq!(
        history
            .descends(&early, &merge)
            .expect("no error at the boundary"),
        takt::loop_scorecard::Ancestry::Unknown,
        "past the shallow boundary the answer is unknown"
    );
}

/// The second review's low finding on the comparison: its before side is the rows the
/// measurement retained, not today's records; a measured turn gone from the cases is refused,
/// with nothing recorded.
#[test]
fn a_measured_turn_gone_from_the_cases_makes_the_comparison_record_nothing() {
    let (tree, _) = scratch("measured-gone", DEFINED);
    for id in ["before-a", "before-b", "before-c"] {
        closed(
            &tree,
            id,
            "2026-10-18T09:00:00Z",
            "2026-10-18T09:00:00Z",
            "declined",
            None,
        );
    }
    let base = git(&tree, &["rev-parse", "HEAD"]);
    let candidate = commit(&tree, DEFINED, "the loop candidate");
    let mut case = loop_case(&tree, "loop-measured-gone", &base, &candidate);
    let measurement = LoopMeasure.observe(&tree, &base, 1).expect("measures");
    let retained = measurement.artifact.expect("the measurement retained");
    let snapshot: serde_json::Value = serde_json::from_slice(&retained.bytes).expect("json");
    assert_eq!(
        snapshot["rows"].as_array().map(Vec::len),
        Some(3),
        "the measurement retains its rows"
    );
    fs::create_dir_all(case.dir.join("artifacts")).expect("artifacts");
    fs::write(
        case.dir.join("artifacts").join(&retained.name),
        &retained.bytes,
    )
    .expect("retained");
    case.add_evidence(&measurement.records[0]).expect("written");
    merged(&mut case, "2026-10-18T10:00:00Z");
    for id in ["after-a", "after-b", "after-c"] {
        after(&tree, id, "2026-10-18T11:00:00Z", "accepted");
    }
    fs::remove_dir_all(tree.join(".engineering/cases/before-c")).expect("a measured turn goes");
    let observation = LoopCompare.observe(&tree, &candidate, 2).expect("observes");
    assert!(observation.records.is_empty(), "{}", observation.summary);
    assert!(
        observation.summary.contains("before-c"),
        "the missing turn is named: {}",
        observation.summary
    );
}

/// The second review's low finding on protocols: a turn is a case on `self.improvement` at
/// major 1, as its own `protocol.yaml` names it; one on another major is no row.
#[test]
fn a_case_on_another_major_of_the_protocol_is_no_turn() {
    let (tree, _) = scratch("other-major", DEFINED);
    closed(
        &tree,
        "turn-a",
        "2026-10-18T09:00:00Z",
        "2026-10-18T10:00:00Z",
        "accepted",
        None,
    );
    closed(
        &tree,
        "turn-b",
        "2026-10-18T09:00:00Z",
        "2026-10-18T10:00:00Z",
        "accepted",
        None,
    );
    fs::write(
        tree.join(".engineering/cases/turn-b/protocol.yaml"),
        "format: protocol/1\nprotocol:\n  id: self.improvement\n  revision: 2\n",
    )
    .expect("protocol.yaml");
    let card = LoopScorecard::read(&tree).expect("reads");
    let turns: Vec<&str> = card.rows.iter().map(|row| row.turn.as_str()).collect();
    assert_eq!(turns, ["turn-a"]);
}
