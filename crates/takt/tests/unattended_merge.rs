//! story:an-unattended-run-merges-under-a-delegated-grant: a run with no person present grants
//! `repository.merge` itself, as the agent role `agent:takt-delivery`, at the case's revision,
//! when the frontier finds the merge approval-required, `.engineering/delegations.yaml` at the
//! merge target's tip records an unrevoked delegation of `repository.merge` (and, for a change to
//! a judge surface, `repository.merge.judge_surface`) to that role, and the case holds validator
//! evidence at the revision for `tests.pass`, `gate.green` and `ci.green`. Without any of them
//! nothing is granted, the run stops `ApprovalRequired (repository.merge)` as before, and the
//! forge is asked for no merge. The merge itself is the merge validator's, under the grant it
//! reads from `authority.yaml` like a person's.

#[path = "support/own_home.rs"]
mod own_home;

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::rc::Rc;

use anyhow::anyhow;
use takt::case::Authority;
use takt::evidence::Record;
use takt::forge::{Checks, Forge, Memory};
use takt::harness::{Harness, StepContext, StepReport};
use takt::validator::{CiRead, PrMerged, PrOpen, Validator};
use takt::{Run, RunOptions, Stop};

/// The delivery protocol, the engineering protocols' built-in.
const PROTOCOL: &str = "story.delivery/1";

/// The role an unattended run grants the merge as.
const ROLE: &str = "agent:takt-delivery";

/// The merge's action and the capability it requires.
const MERGE: &str = "repository.merge";

/// The capability a judge-surface merge's delegation names.
const JUDGE_SURFACE: &str = "repository.merge.judge_surface";

/// The allow-list every scratch repository's `main` starts with.
const ALLOW: &str = "paths:\n  - docs/**\n  - README.md\n  - CHANGELOG.md\n";

/// A change to a judge surface: the judge's own source.
const JUDGE_EDIT: (&str, &str) = (
    "crates/takt/src/validator.rs",
    "pub fn judge() { /* a candidate */ }\n",
);

/// A change to no judge surface: a doc the allow-list admits.
const DOCS_EDIT: (&str, &str) = ("docs/guide.md", "a guide, changed\n");

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

/// A scratch directory under this crate's build directory, removed when dropped.
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

/// One delegation entry of `capability` to [`ROLE`] by the owner, revoked when `revoked`.
fn entry(capability: &str, revoked: bool) -> String {
    let mut text = format!(
        "  - owner: human:owner\n    role: {ROLE}\n    capability: {capability}\n    given_at: 2026-10-29T08:00:00Z\n"
    );
    if revoked {
        text.push_str("    revoked_at: 2026-10-29T09:00:00Z\n");
    }
    text
}

/// A delegation record of the given entries.
fn delegations(entries: &[String]) -> String {
    format!("delegations:\n{}", entries.concat())
}

/// The record the acceptance's fixture holds: `repository.merge` and
/// `repository.merge.judge_surface` delegated to [`ROLE`], neither revoked.
fn both_delegated() -> String {
    delegations(&[entry(MERGE, false), entry(JUDGE_SURFACE, false)])
}

/// A scratch repository whose `main` holds a README, a doc, the judge's own source, the
/// allow-list and, when given, `delegations` as `.engineering/delegations.yaml`: the merge
/// target's tip the in-memory forge reports. The work tree is `tree/` under the scratch
/// directory, which is free for files outside the tree.
fn repository(name: &str, delegations: Option<&str>) -> (Scratch, PathBuf) {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("unattended-merge-{}-{name}", std::process::id()));
    if dir.exists() {
        fs::remove_dir_all(&dir).expect("a stale scratch tree is removed");
    }
    fs::create_dir_all(&dir).expect("scratch dir");
    let scratch = Scratch(dir);
    let tree = scratch.join("tree");
    let files: Vec<(&str, String)> = vec![
        ("README.md", "a repository\n".to_owned()),
        ("docs/guide.md", "a guide\n".to_owned()),
        (
            "crates/takt/src/validator.rs",
            "pub fn judge() {}\n".to_owned(),
        ),
        (".engineering/judge-allow.yaml", ALLOW.to_owned()),
    ];
    for (path, text) in files
        .into_iter()
        .chain(delegations.map(|text| (".engineering/delegations.yaml", text.to_owned())))
    {
        let path = tree.join(path);
        fs::create_dir_all(path.parent().expect("a parent")).expect("a directory");
        fs::write(path, text).expect("a file");
    }
    git(&tree, &["init", "-q", "-b", "main"]);
    git(&tree, &["add", "-A"]);
    git(&tree, &["commit", "-q", "-m", "start"]);
    (scratch, tree)
}

/// The built-in delivery protocol with the merge's precondition narrowed to `pr.opened`, under
/// its own id, written outside the tree: a protocol under which the merge is approval-required
/// while continuous integration is not green, so what decides the grant there is the grant's own
/// condition on the evidence.
fn merge_without_ci(scratch: &Path) -> String {
    let builtin = canon_engineering::registry::get("story-delivery", 1)
        .expect("the protocol")
        .yaml;
    let narrowed = "    precondition:\n      claim: pr.opened\n    requires:\n      - capability: repository.merge\n";
    let widened = "    precondition:\n      all:\n        - claim: ci.green\n        - claim: pr.opened\n    requires:\n      - capability: repository.merge\n";
    assert!(
        builtin.contains(widened),
        "the built-in's merge precondition"
    );
    assert!(
        builtin.contains("  id: story.delivery\n"),
        "the built-in's id"
    );
    let text = builtin
        .replace(widened, narrowed)
        .replace("  id: story.delivery\n", "  id: merge.without_ci\n");
    let path = scratch.join("merge-without-ci.yaml");
    fs::write(&path, text).expect("the protocol is written");
    path.to_str().expect("utf-8").to_owned()
}

/// A harness that asks for the merge gate whenever it is offered, hands the change over when
/// that is offered, and fails otherwise: the run's grant is never its choice.
struct Requester;

impl Harness for Requester {
    fn name(&self) -> &str {
        "requester"
    }

    fn step(&self, context: &StepContext<'_>) -> Result<StepReport, anyhow::Error> {
        if context.approvals.contains(&MERGE.to_owned()) {
            return Ok(StepReport {
                action: MERGE.to_owned(),
                summary: "the intent holds; merge it".to_owned(),
                tokens: None,
            });
        }
        if context.offered.contains(&"change.handover".to_owned()) {
            return Ok(StepReport {
                action: "change.handover".to_owned(),
                summary: "hand it over".to_owned(),
                tokens: None,
            });
        }
        Err(anyhow!(
            "neither the gate nor the handover was offered; among {}",
            context.offered.join(", ")
        ))
    }
}

/// The `merge_observation` records under the case at `dir`, each as its JSON.
fn merge_observations(dir: &Path) -> Vec<serde_json::Value> {
    let Ok(entries) = fs::read_dir(dir.join("evidence")) else {
        return Vec::new();
    };
    entries
        .map(|entry| {
            let path = entry.expect("an entry").path();
            serde_json::from_str::<serde_json::Value>(&fs::read_to_string(&path).expect("a record"))
                .expect("json")
        })
        .filter(|record| record["kind"] == "merge_observation")
        .collect()
}

/// What one unattended run of a case ends with.
struct Ran {
    revision: String,
    stop: Stop,
    out: String,
    calls: Vec<String>,
    authority: Option<Authority>,
    merged: Vec<serde_json::Value>,
}

/// A case `id` opened on `main` of `tree` under `protocol`, whose branch carries one commit
/// writing `change`, with `tests.pass` and `gate.green` decided at that revision, run once
/// through the in-memory forge whose checks answer `checks`, with no person's step anywhere.
fn run_once(tree: &Path, id: &str, protocol: &str, change: (&str, &str), checks: Checks) -> Ran {
    let memory = Rc::new(Memory {
        checks,
        ..Memory::default()
    });
    let forge: Rc<dyn Forge> = memory.clone();
    let validators: Vec<Box<dyn Validator>> = vec![
        Box::new(PrOpen::new(forge.clone())),
        Box::new(CiRead::new(forge.clone())),
        Box::new(PrMerged::new(forge)),
    ];
    let options = RunOptions {
        protocol: protocol.to_owned(),
        intent: "change the judge\n\nA candidate.".to_owned(),
        inputs: Vec::new(),
        iterations: Some(12),
        hours: None,
        tokens: None,
        case_id: Some(id.to_owned()),
        gate: Some(MERGE.to_owned()),
    };
    let mut run = Run::open(tree, options, &Requester, &validators).expect("the case opens");
    let (path, text) = change;
    let at = tree.join(path);
    fs::create_dir_all(at.parent().expect("a parent")).expect("a directory");
    fs::write(at, text).expect("a file");
    git(tree, &["add", "-A", "--", ".", ":!.takt"]);
    git(tree, &["commit", "-q", "-m", "a candidate commit"]);
    let revision = git(tree, &["rev-parse", "HEAD"]);
    run.case
        .set_revision("implementation", &revision)
        .expect("the revision moves");
    for (n, (kind, result)) in [("test_result", "pass"), ("gate_result", "pass")]
        .into_iter()
        .enumerate()
    {
        run.case
            .add_evidence(&Record::new(
                format!("e{n}"),
                kind,
                Some(result.to_owned()),
                "implementation",
                &revision,
                "unattended-merge-test",
            ))
            .expect("the record is written");
    }
    run.case
        .add_evidence(&Record::new(
            format!("handover-{}", &revision[..8]),
            "handover",
            None,
            "implementation",
            &revision,
            takt::run::CHOICE_PRODUCER,
        ))
        .expect("the record is written");
    run.case
        .add_evidence(&Record::new(
            format!("change-record-{}", &revision[..8]),
            "change_record",
            Some("ahead".to_owned()),
            "implementation",
            &revision,
            "takt.change-read",
        ))
        .expect("the record is written");
    let mut out = Vec::new();
    let stop = run.run(&mut out).expect("the run stops without an error");
    let dir = run.case.dir.clone();
    let merged = merge_observations(&dir);
    let calls = memory.calls.borrow().clone();
    Ran {
        revision,
        stop,
        out: String::from_utf8_lossy(&out).into_owned(),
        calls,
        authority: Authority::read(&dir).expect("authority.yaml reads"),
        merged,
    }
}

/// The run stopped at the gate as it does without this story: `ApprovalRequired
/// (repository.merge)`, no grant in `authority.yaml`, no merge asked of the forge, and no
/// `merge_observation` written. `refused` is what the run says it did not grant for, when it
/// asked about the delegation; `None` when it was not to ask at all, and then it says nothing
/// of a grant.
fn assert_ungranted(ran: &Ran, what: &str, refused: Option<&str>) {
    let said = ran
        .out
        .lines()
        .find(|line| line.contains(&format!("not granted as {ROLE}: ")));
    match refused {
        Some(why) => assert!(
            said.is_some_and(|line| line.contains(why)),
            "{what}: the run does not say it granted nothing because {why}: {said:?}\n{}",
            ran.out
        ),
        None => assert!(
            said.is_none() && !ran.out.contains("granted: "),
            "{what}: the run asked about a grant: {}",
            ran.out
        ),
    }
    assert_eq!(
        ran.stop,
        Stop::ApprovalRequired(vec![MERGE.to_owned()]),
        "{what}: {}",
        ran.out
    );
    let granted = ran
        .authority
        .as_ref()
        .and_then(|authority| authority.decision(MERGE));
    assert!(
        granted.is_none(),
        "{what}: authority.yaml holds a grant: {granted:?}\n{}",
        ran.out
    );
    assert!(
        !ran.calls.iter().any(|call| call.starts_with("merge ")),
        "{what}: the forge was asked to merge: {:?}\n{}",
        ran.calls,
        ran.out
    );
    assert!(
        ran.merged.is_empty(),
        "{what}: a merge_observation was written\n{}",
        ran.out
    );
}

/// Acceptance 1: the checks pass and the tests and the gate are green at the revision, under
/// the owner's delegation of both capabilities to `agent:takt-delivery` at the base: the run
/// merges a change to a judge surface with no `takt approve`, `authority.yaml` names
/// `agent:takt-delivery` at the revision, and a `merge_observation` of `merged` is written.
#[test]
fn checks_green_and_delegated_the_run_grants_as_takt_delivery_and_merges() {
    let (_scratch, tree) = repository("green", Some(&both_delegated()));
    let ran = run_once(&tree, "green", PROTOCOL, JUDGE_EDIT, Checks::Pass);
    assert_eq!(
        ran.stop,
        Stop::Completed("accepted".to_owned()),
        "{}",
        ran.out
    );
    let authority = ran.authority.as_ref().expect("authority.yaml is written");
    let grant = authority
        .decision(MERGE)
        .expect("a grant of repository.merge");
    assert_eq!(grant.approver, ROLE, "{}", ran.out);
    assert_eq!(
        grant.revision.as_deref(),
        Some(ran.revision.as_str()),
        "{}",
        ran.out
    );
    assert!(
        authority.grants_at(MERGE, &ran.revision),
        "the grant authorises the revision: {grant:?}"
    );
    assert!(
        ran.calls.contains(&"merge #1".to_owned()),
        "{:?}\n{}",
        ran.calls,
        ran.out
    );
    assert_eq!(ran.merged.len(), 1, "{}", ran.out);
    assert_eq!(ran.merged[0]["result"], "merged", "{}", ran.out);
    assert_eq!(
        ran.merged[0]["subject_revision"],
        ran.revision.as_str(),
        "{}",
        ran.out
    );
    assert!(
        ran.out.contains(ROLE),
        "the console says who granted: {}",
        ran.out
    );
}

/// Acceptance 2: the forge's checks fail. Under a protocol that still finds the merge
/// approval-required without `ci.green`, the run grants nothing: it stops `ApprovalRequired
/// (repository.merge)`, `authority.yaml` holds no grant, and the forge's call log has no merge.
#[test]
fn checks_failing_grant_nothing() {
    let (scratch, tree) = repository("checks-fail", Some(&both_delegated()));
    let protocol = merge_without_ci(&scratch);
    let ran = run_once(&tree, "checks-fail", &protocol, JUDGE_EDIT, Checks::Fail);
    assert!(
        ran.calls.contains(&"checks #1".to_owned()),
        "the checks were read: {:?}\n{}",
        ran.calls,
        ran.out
    );
    assert_ungranted(&ran, "checks failing", None);
}

/// Acceptance 3: the owner's delegation of `repository.merge` revoked at the base
/// (`revoked_at` set), the judge-surface one still standing: as 2.
#[test]
fn a_revoked_delegation_grants_nothing() {
    let record = delegations(&[entry(MERGE, true), entry(JUDGE_SURFACE, false)]);
    let (_scratch, tree) = repository("revoked", Some(&record));
    let ran = run_once(&tree, "revoked", PROTOCOL, JUDGE_EDIT, Checks::Pass);
    assert_ungranted(
        &ran,
        "revoked",
        Some("records no unrevoked delegation of repository.merge to agent:takt-delivery"),
    );
}

/// Acceptance 4: no delegations file at the base: as 2.
#[test]
fn no_delegations_file_grants_nothing() {
    let (_scratch, tree) = repository("no-file", None);
    let ran = run_once(&tree, "no-file", PROTOCOL, JUDGE_EDIT, Checks::Pass);
    assert_ungranted(
        &ran,
        "no delegations file",
        Some(".engineering/delegations.yaml is not there"),
    );
}

/// A delegation of the judge surface alone is not one of the merge: a change to no judge
/// surface, everything green, is granted nothing without `repository.merge` delegated.
#[test]
fn a_judge_surface_delegation_alone_grants_no_merge() {
    let record = delegations(&[entry(JUDGE_SURFACE, false)]);
    let (_scratch, tree) = repository("surface-only", Some(&record));
    let ran = run_once(&tree, "surface-only", PROTOCOL, DOCS_EDIT, Checks::Pass);
    assert_ungranted(
        &ran,
        "judge surface delegated alone",
        Some("records no unrevoked delegation of repository.merge to agent:takt-delivery"),
    );
}

/// For a change to a judge surface the merge's delegation is not enough: without
/// `repository.merge.judge_surface` delegated the run grants nothing, so no grant is written
/// that the merge's judge guard would then refuse.
#[test]
fn a_judge_surface_change_needs_its_own_delegation() {
    let record = delegations(&[entry(MERGE, false)]);
    let (_scratch, tree) = repository("merge-only", Some(&record));
    let ran = run_once(&tree, "merge-only", PROTOCOL, JUDGE_EDIT, Checks::Pass);
    assert_ungranted(
        &ran,
        "merge delegated alone, judge surface changed",
        Some(
            "records no unrevoked delegation of repository.merge.judge_surface to agent:takt-delivery",
        ),
    );
}

/// The merge's delegation alone is enough for a change to no judge surface.
#[test]
fn a_change_to_no_judge_surface_merges_under_the_merge_delegation_alone() {
    let record = delegations(&[entry(MERGE, false)]);
    let (_scratch, tree) = repository("docs-only", Some(&record));
    let ran = run_once(&tree, "docs-only", PROTOCOL, DOCS_EDIT, Checks::Pass);
    assert_eq!(
        ran.stop,
        Stop::Completed("accepted".to_owned()),
        "{}",
        ran.out
    );
    let grant = ran
        .authority
        .as_ref()
        .and_then(|authority| authority.decision(MERGE))
        .expect("a grant");
    assert_eq!(grant.approver, ROLE);
    assert!(
        ran.calls.contains(&"merge #1".to_owned()),
        "{:?}",
        ran.calls
    );
}
