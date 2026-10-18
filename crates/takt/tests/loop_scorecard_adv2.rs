//! Adversary pass 2 on the loop scorecard (`crates/takt/src/loop_scorecard.rs`) and its first
//! correction: the reads at the base a step can still redirect past the checkpoint, `filed_at`
//! read from the commit that added the finding, the comparison's sides, the definition check and
//! the token rule. Each case is named for the defect it shows.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use takt::git::Checkpoint;
use takt::loop_scorecard::{
    DEFINITIONS, DefinitionCheck, Definitions, LoopCompare, LoopDefinitionCheck, LoopMeasure,
    LoopScorecard, check,
};
use takt::validator::Validator;
use takt::{CaseDir, LoadedProtocol};

const LOOP_IMPROVEMENT: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../protocols/loop-improvement/1.yaml"
);

/// Definitions with two targets: accepted share at least three quarters, finding to verified
/// at most a day.
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
    target:
      at_most: 86400
";

/// [`DEFINED`] with accepted share redefined under the same version and its target lowered.
fn silent() -> String {
    let silent = DEFINED.replace(
        "accepted rows over all rows, open turns included\n    target:\n      at_least: 0.75",
        "accepted rows over closed rows, open turns left out\n    target:\n      at_least: 0.1",
    );
    assert_ne!(silent, DEFINED, "the redefinition applies");
    silent
}

/// Definitions with one target: finding to verified at most a day.
const DAY_ONLY: &str = "format: takt-loop-metrics/1
metrics:
  turns:
    version: 1
    definition: the improvement turns recorded
  accepted_share:
    version: 1
    definition: accepted rows over all rows, open turns included
  finding_to_verified:
    version: 1
    definition: the median seconds from filed_at to verified_at over the rows with both
    target:
      at_most: 86400
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
        if self.0.exists()
            && let Err(error) = fs::remove_dir_all(&self.0)
        {
            eprintln!("remove the scratch tree {}: {error}", self.0.display());
        }
    }
}

fn scratch_dir(name: &str) -> Scratch {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("loop-scorecard-adv2-{}-{name}", std::process::id()));
    if dir.exists() {
        fs::remove_dir_all(&dir).expect("a stale scratch tree is removed");
    }
    Scratch(dir)
}

/// A scratch repository whose first commit holds `definitions` at [`DEFINITIONS`]; its `HEAD`.
fn scratch(name: &str, definitions: &str) -> (Scratch, String) {
    let dir = scratch_dir(name);
    fs::create_dir_all(&*dir).expect("scratch dir");
    fs::write(dir.join("README.md"), "takt\n").expect("a file");
    git(&dir, &["init", "-q", "-b", "main"]);
    let head = commit(&dir, definitions, "start");
    (dir, head)
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

/// Files the finding `key` in `tree`'s planning store, its store file committed at `at`.
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
/// story it was opened for, its decisions, and the run record lines `events` when given.
fn turn(
    tree: &Path,
    id: &str,
    finding: Option<&str>,
    decisions: &[At<'_>],
    events: &[serde_json::Value],
) {
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
    if !events.is_empty() {
        let text: String = events.iter().map(|line| format!("{line}\n")).collect();
        fs::write(dir.join("events.jsonl"), text).expect("events.jsonl");
    }
}

/// A turn whose finding was filed at `filed`, verified at `verified` and closed `accepted`.
fn accepted(tree: &Path, id: &str, filed: &str, verified: &str) {
    file(tree, id, filed);
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
                holds: &[
                    "finding.cited",
                    "scenario.passes_after",
                    "fix.merged",
                    "promoted",
                ],
                legitimate: Some("accepted"),
            },
        ],
        &[],
    );
}

/// Opens a loop case `id` on `tree`, the loop validators' producers declared independent.
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

/// Writes a `merge_observation` record of the loop candidate into `case`.
fn merged(case: &mut CaseDir) {
    let revision = case.revision("loop_candidate").expect("a candidate");
    case.add_evidence(&takt::evidence::Record::new(
        "merged",
        "merge_observation",
        Some("merged".to_owned()),
        "loop_candidate",
        revision,
        "developer-run",
    ))
    .expect("written");
}

/// Moves the turn `id`'s `baseline` revision to `revision`.
fn on_baseline(tree: &Path, id: &str, revision: &str) {
    let path = tree.join(".engineering/cases").join(id).join("case.yaml");
    let text = fs::read_to_string(&path).expect("case.yaml").replace(
        &format!("revision: b-{id}"),
        &format!("revision: {revision}"),
    );
    fs::write(&path, text).expect("case.yaml");
}

/// Moves the turn `id`'s `finding` revision to `revision`.
fn on_finding(tree: &Path, id: &str, revision: &str) {
    let path = tree.join(".engineering/cases").join(id).join("case.yaml");
    let text = fs::read_to_string(&path).expect("case.yaml").replace(
        &format!("revision: f-{id}"),
        &format!("revision: {revision}"),
    );
    fs::write(&path, text).expect("case.yaml");
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

/// What a step can do with write access to an object directory: replace the loose object `oid`
/// with a blob holding `content`, under the same name.
fn rewrite_blob(objects: &Path, oid: &str, content: &[u8]) {
    let path = objects.join(&oid[..2]).join(&oid[2..]);
    assert!(path.is_file(), "{} is a loose object", path.display());
    fs::remove_file(&path).expect("the object is removed");
    let mut raw = format!("blob {}\0", content.len()).into_bytes();
    raw.extend_from_slice(content);
    fs::write(&path, zlib_stored(&raw)).expect("the object is written");
}

/// A step rewrites the loose object of the base's metric definitions with the candidate's bytes,
/// as any process of the step can. The checkpoint guards no object, so after its put-back and
/// restore every read at the base returns the candidate's definitions: a same-version
/// redefinition with its target lowered is checked against itself and recorded `unchanged`. The
/// replace-ref fix closes one way to redirect a read at the base; this is another.
#[test]
fn a_loose_object_a_step_rewrites_makes_the_baselines_definitions_the_candidates() {
    let (tree, _) = scratch("loose-object", DEFINED);
    let base = commit(&tree, DEFINED, "the base");
    let silent = silent();
    let head = commit(&tree, &silent, "the candidate");
    let _case = loop_case(&tree, "loop-loose-object", &base, &head);

    let checkpoint = Checkpoint::take(&tree).expect("a checkpoint");
    let blob = git(&tree, &["rev-parse", &format!("{base}:{DEFINITIONS}")]);
    rewrite_blob(&tree.join(".git/objects"), &blob, silent.as_bytes());
    let _ = checkpoint.put_back_git_dir(&tree);
    checkpoint.restore(&tree).expect("restored");

    let observation = LoopDefinitionCheck
        .observe(&tree, &head, 1)
        .expect("observes");
    assert_eq!(
        observation.records[0].result.as_deref(),
        Some("redefined"),
        "the baseline's definitions after the restore are the candidate's: {}",
        observation.summary
    );
}

/// The tree is a linked worktree, as a managed worktree is. A step copies the repository's
/// common git directory, rewrites the base's definitions in the copy, and points the worktree's
/// `commondir` at it. Nothing guards `commondir`: the put-back and the restore leave it, every
/// later read of the worktree (refs, objects, the base's definitions) is the copy's, and the
/// candidate's redefinition is recorded `unchanged`.
#[test]
fn a_commondir_a_step_rewrites_redirects_every_read_of_a_linked_worktree_past_the_restore() {
    let (main, _) = scratch("commondir-main", DEFINED);
    let base = commit(&main, DEFINED, "the base");
    let silent = silent();
    let head = commit(&main, &silent, "the candidate");
    let tree = scratch_dir("commondir-tree");
    let copy = scratch_dir("commondir-copy");
    git(
        &main,
        &[
            "worktree",
            "add",
            "-q",
            "-b",
            "work",
            tree.to_str().expect("utf-8"),
            &head,
        ],
    );
    let _case = loop_case(&tree, "loop-commondir", &base, &head);
    let common = git(
        &tree,
        &["rev-parse", "--path-format=absolute", "--git-common-dir"],
    );
    let own = PathBuf::from(git(&tree, &["rev-parse", "--absolute-git-dir"]));

    let checkpoint = Checkpoint::take(&tree).expect("a checkpoint");
    let copied = Command::new("cp")
        .arg("-r")
        .arg(main.join(".git"))
        .arg(&*copy)
        .status()
        .expect("cp runs");
    assert!(copied.success(), "the copy is made");
    let blob = git(&tree, &["rev-parse", &format!("{base}:{DEFINITIONS}")]);
    rewrite_blob(&copy.join("objects"), &blob, silent.as_bytes());
    fs::write(own.join("commondir"), format!("{}\n", copy.display())).expect("commondir");
    let put_back = checkpoint.put_back_git_dir(&tree);
    let restored = checkpoint.restore(&tree);

    let observation = LoopDefinitionCheck
        .observe(&tree, &head, 1)
        .expect("observes");
    assert_eq!(
        observation.records[0].result.as_deref(),
        Some("redefined"),
        "the baseline's definitions are read from the step's copy (put back: {put_back:?}, restore: {restored:?}): {}",
        observation.summary
    );
    assert_eq!(
        git(
            &tree,
            &["rev-parse", "--path-format=absolute", "--git-common-dir"]
        ),
        common,
        "the restore put the common git directory back"
    );
}

/// A step writes `.git/shallow` naming the base. Nothing guards the file, so after the put-back
/// and the restore the base has no parents: the commit that added the finding is no longer in
/// its history, the base itself is taken for it, and `filed_at` moves from the filing to the
/// base's commit time. The same file cuts `merge-base --is-ancestor`, which `loop.compare`'s
/// after side rests on.
#[test]
fn a_shallow_file_a_step_writes_survives_the_restore_and_moves_filed_at() {
    let (tree, _) = scratch("shallow", DEFINED);
    file(&tree, "finding-x", "2026-10-01T10:00:00Z");
    let base = commit(&tree, DEFINED, "the base");
    turn(
        &tree,
        "turn-x",
        Some("finding-x"),
        &[At {
            at: "2026-10-01T12:00:00Z",
            holds: &["finding.cited", "scenario.passes_after"],
            legitimate: None,
        }],
        &[],
    );
    on_finding(&tree, "turn-x", &base);
    let filed = |tree: &Path| {
        LoopScorecard::read_at(tree, Some(&base))
            .expect("reads")
            .rows[0]
            .filed_at
            .clone()
    };
    assert_eq!(filed(&tree).as_deref(), Some("2026-10-01T10:00:00Z"));

    let checkpoint = Checkpoint::take(&tree).expect("a checkpoint");
    fs::write(tree.join(".git/shallow"), format!("{base}\n")).expect("the shallow file");
    let _ = checkpoint.put_back_git_dir(&tree);
    checkpoint.restore(&tree).expect("restored");

    assert_eq!(
        filed(&tree).as_deref(),
        Some("2026-10-01T10:00:00Z"),
        "filed_at after the restore (shallow file left: {})",
        tree.join(".git/shallow").exists()
    );
}

/// The planning store is committed after the run that used it (the recorded first turn's case
/// directory and the story filed beside it were committed hours after its last decision). A
/// finding whose store commit comes after its verification gives a negative time from finding to
/// verified, which meets an `at_most` target by any margin.
#[test]
fn a_finding_committed_after_its_verification_gives_a_negative_finding_to_verified() {
    let (tree, _) = scratch("negative", DEFINED);
    file(&tree, "late", "2026-10-18T19:17:01Z");
    let base = git(&tree, &["rev-parse", "HEAD"]);
    turn(
        &tree,
        "turn-late",
        Some("late"),
        &[
            At {
                at: "2026-10-18T14:40:03Z",
                holds: &["finding.cited"],
                legitimate: None,
            },
            At {
                at: "2026-10-18T16:00:22Z",
                holds: &["finding.cited", "scenario.passes_after"],
                legitimate: None,
            },
            At {
                at: "2026-10-18T16:02:08Z",
                holds: &[
                    "finding.cited",
                    "scenario.passes_after",
                    "fix.merged",
                    "promoted",
                ],
                legitimate: Some("accepted"),
            },
        ],
        &[],
    );
    let definitions = Definitions::parse(DEFINED).expect("parses");
    let card = LoopScorecard::read_at(&tree, Some(&base)).expect("reads");
    let figure = card
        .figures(&definitions)
        .expect("computed")
        .into_iter()
        .find(|figure| figure.metric == "finding_to_verified")
        .expect("the figure");
    assert!(
        figure.value.is_none_or(|value| value >= 0.0),
        "finding to verified is {:?} s (filed_at {:?}, verified_at {:?}), below_target {}",
        figure.value,
        card.rows[0].filed_at,
        card.rows[0].verified_at,
        figure.below_target
    );
}

/// `loop.compare` reads every row's `filed_at` at the case's `loop_baseline`. The turns after
/// the merge are opened on findings filed after it, which the baseline's history does not hold,
/// so none of them has a `filed_at` and the after side has no time from finding to verified:
/// a candidate that cut it from two days to one hour on every turn is `not_improved`.
#[test]
fn turns_after_the_merge_have_no_filed_at_so_finding_to_verified_can_never_improve() {
    let (tree, _) = scratch("after-filed", DAY_ONLY);
    for id in ["before-a", "before-b", "before-c"] {
        accepted(&tree, id, "2026-10-10T09:00:00Z", "2026-10-12T09:00:00Z");
    }
    let base = commit(&tree, DAY_ONLY, "the base");
    let candidate = commit(&tree, DAY_ONLY, "the loop candidate");
    let mut case = loop_case(&tree, "loop-after-filed", &base, &candidate);
    merged(&mut case);
    let measurement = LoopMeasure.observe(&tree, &base, 2).expect("measures");
    assert_eq!(
        measurement.records[0].result.as_deref(),
        Some("below_target"),
        "{}",
        measurement.summary
    );
    let retained = measurement.artifact.expect("the measurement retained");
    fs::create_dir_all(case.dir.join("artifacts")).expect("artifacts");
    fs::write(
        case.dir.join("artifacts").join(&retained.name),
        &retained.bytes,
    )
    .expect("retained");
    case.add_evidence(&measurement.records[0]).expect("written");

    for id in ["after-a", "after-b", "after-c"] {
        accepted(&tree, id, "2026-10-13T09:00:00Z", "2026-10-13T10:00:00Z");
        on_baseline(&tree, id, &git(&tree, &["rev-parse", "HEAD"]));
        on_finding(&tree, id, &git(&tree, &["rev-parse", "HEAD"]));
    }
    let observation = LoopCompare.observe(&tree, &candidate, 3).expect("observes");
    assert_eq!(observation.records.len(), 1, "{}", observation.summary);
    assert_eq!(
        observation.records[0].result.as_deref(),
        Some("improved"),
        "{}",
        observation.summary
    );
}

/// The base prints accepted share v2 and, beside it, the v1 figure it replaced. A candidate that
/// drops the v1 definition stops printing that figure, the change the definition check exists to
/// catch, and is found `unchanged`.
#[test]
fn a_candidate_that_drops_a_previous_definition_the_base_prints_is_unchanged() {
    let base = Definitions::parse(
        "format: takt-loop-metrics/1
metrics:
  accepted_share:
    version: 2
    definition: accepted rows over rows with a verdict
    target:
      at_least: 0.8
    previous:
      - version: 1
        definition: accepted rows over all rows, open turns included
        target:
          at_least: 0.8
",
    )
    .expect("parses");
    let candidate = Definitions::parse(
        "format: takt-loop-metrics/1
metrics:
  accepted_share:
    version: 2
    definition: accepted rows over rows with a verdict
    target:
      at_least: 0.8
",
    )
    .expect("parses");
    let (result, each) = check(&base, Some(&candidate));
    assert_ne!(result, DefinitionCheck::Unchanged, "{each:?}");
}

/// A turn whose run record holds two harness steps, one that reported 1000 tokens and one that
/// reported none, is given 1000 tokens: a partial sum enters the median tokens per accepted turn
/// as though it were the turn's whole spend.
#[test]
fn a_turn_whose_harness_reported_tokens_for_some_steps_only_has_a_partial_sum_as_its_tokens() {
    let (tree, _) = scratch("partial-tokens", DEFINED);
    let at = "2026-10-18T09:00:00Z";
    turn(
        &tree,
        "turn-partial",
        None,
        &[At {
            at,
            holds: &["scenario.passes_after", "promoted"],
            legitimate: Some("accepted"),
        }],
        &[
            serde_json::json!({ "at": at, "step": 0, "event": "opened" }),
            serde_json::json!({ "at": at, "step": 1, "event": "step", "by": "harness", "name": "a", "action": "implementation.edit", "summary": "edited", "tokens": 1000 }),
            serde_json::json!({ "at": at, "step": 2, "event": "step", "by": "harness", "name": "b", "action": "implementation.edit", "summary": "edited" }),
            serde_json::json!({ "at": at, "step": 2, "event": "stop", "stop": "Completed (accepted)", "steps": 2, "tokens": 1000 }),
        ],
    );
    let card = LoopScorecard::read(&tree).expect("reads");
    assert_eq!(
        card.rows[0].tokens, None,
        "a harness step reported no count, so the turn's tokens are unknown"
    );
}
