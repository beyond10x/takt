//! Security review of the cluster-delivery wave (`4ec3ae24..4def3240`): conformance cases for
//! the invariants an unattended run in a pod rests on when it merges to `main` with no person's
//! step. Each case drives the code the wave ships and states the invariant it checks; a red case
//! is a gap between the contract (`AGENTS.md` § Rules, story:an-unattended-run-merges-under-a-
//! delegated-grant) and the implementation.

#[path = "support/own_home.rs"]
mod own_home;

use std::cell::{Cell, RefCell};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::rc::Rc;

use anyhow::anyhow;
use takt::case::{Authority, AuthorityDecision, Grant};
use takt::evidence::Record;
use takt::forge::{
    Checks, Forge, Memory, MergeMessage, Outgoing, Process, PullRequest, PullRequestView, Runner,
};
use takt::harness::{Harness, StepContext, StepReport};
use takt::validator::{CiRead, PrMerged, PrOpen, Validator};
use takt::{Run, RunOptions, Stop};

const PROTOCOL: &str = "story.delivery/1";
const ROLE: &str = "agent:takt-delivery";
const MERGE: &str = "repository.merge";
const JUDGE_SURFACE: &str = "repository.merge.judge_surface";
const ALLOW: &str = "paths:\n  - docs/**\n  - README.md\n  - CHANGELOG.md\n";
const DELEGATIONS: &str = ".engineering/delegations.yaml";
const DOCS_EDIT: (&str, &str) = ("docs/guide.md", "a guide, changed\n");

fn git(tree: &Path, args: &[&str]) -> String {
    git_env(tree, args, &[])
}

fn git_env(tree: &Path, args: &[&str], env: &[(&str, &str)]) -> String {
    let output = Command::new("git")
        .args(["-c", "user.name=t", "-c", "user.email=t@localhost"])
        .args(args)
        .envs(env.iter().copied())
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

fn entry(capability: &str, revoked: bool) -> String {
    let mut text = format!(
        "  - owner: human:owner\n    role: {ROLE}\n    capability: {capability}\n    given_at: 2026-10-29T08:00:00Z\n"
    );
    if revoked {
        text.push_str("    revoked_at: 2026-10-29T09:00:00Z\n");
    }
    text
}

fn delegations(entries: &[String]) -> String {
    format!("delegations:\n{}", entries.concat())
}

fn both_delegated() -> String {
    delegations(&[entry(MERGE, false), entry(JUDGE_SURFACE, false)])
}

/// A scratch repository whose `main` holds a README, a doc, a protocol file, the allow-list
/// and, when given, the delegation record. The work tree is `tree/` under the scratch dir.
fn repository(name: &str, delegation: Option<&str>) -> (Scratch, PathBuf) {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("goal-a-sec1-{}-{name}", std::process::id()));
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
            "protocols/story-delivery/3.yaml",
            "format: protocol/1\n".to_owned(),
        ),
        (".engineering/judge-allow.yaml", ALLOW.to_owned()),
    ];
    for (path, text) in files
        .into_iter()
        .chain(delegation.map(|text| (DELEGATIONS, text.to_owned())))
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

/// The built-in delivery protocol under its own id with `pr.open` needing neither `tests.pass`
/// nor `gate.green`: the pull request opens and CI is read whatever the tests say, so the merge
/// is approval-required with CI green and what alone decides the run's own grant is its check of
/// `tests.pass` and `gate.green` at the revision.
fn pr_open_without_tests(scratch: &Path) -> String {
    let builtin = canon_engineering::registry::get("story-delivery", 1)
        .expect("the protocol")
        .yaml;
    let strict = "      all:\n        - claim: tests.pass\n        - claim: gate.green\n        - claim: change.handed_over\n";
    let loose = "      all:\n        - claim: change.handed_over\n";
    assert!(
        builtin.contains(strict),
        "the built-in's pr.open precondition"
    );
    assert!(
        builtin.contains("  id: story.delivery\n"),
        "the built-in's id"
    );
    let text = builtin
        .replace(strict, loose)
        .replace("  id: story.delivery\n", "  id: pr_open.without_tests\n");
    let path = scratch.join("pr-open-without-tests.yaml");
    fs::write(&path, text).expect("the protocol is written");
    path.to_str().expect("utf-8").to_owned()
}

/// Asks for the merge gate when offered, hands over when offered, fails otherwise.
struct Requester;

impl Harness for Requester {
    fn name(&self) -> &str {
        "requester"
    }

    fn step(&self, context: &StepContext<'_>) -> Result<StepReport, anyhow::Error> {
        if context.approvals.contains(&MERGE.to_owned()) {
            return Ok(StepReport {
                action: MERGE.to_owned(),
                summary: "merge it".to_owned(),
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
        Err(anyhow!("offered {}", context.offered.join(", ")))
    }
}

fn options(id: &str, protocol: &str) -> RunOptions {
    RunOptions {
        protocol: protocol.to_owned(),
        intent: "change the guide\n\nA candidate.".to_owned(),
        inputs: Vec::new(),
        iterations: Some(12),
        hours: None,
        tokens: None,
        case_id: Some(id.to_owned()),
        gate: Some(MERGE.to_owned()),
    }
}

fn validators(forge: &Rc<dyn Forge>) -> Vec<Box<dyn Validator>> {
    vec![
        Box::new(PrOpen::new(forge.clone())),
        Box::new(CiRead::new(forge.clone())),
        Box::new(PrMerged::new(forge.clone())),
    ]
}

/// Commits `change` on the case's branch and moves the case's implementation to it.
fn commit_change(run: &mut Run<'_>, tree: &Path, change: (&str, &str)) -> String {
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
    revision
}

fn add(run: &mut Run<'_>, id: &str, kind: &str, result: Option<&str>, at: &str, producer: &str) {
    run.case
        .add_evidence(&Record::new(
            format!("{id}-{}", &at[..8]),
            kind,
            result.map(str::to_owned),
            "implementation",
            at,
            producer,
        ))
        .expect("the record is written");
}

/// The tests and the gate green at `at`.
fn green(run: &mut Run<'_>, at: &str) {
    add(run, "tests", "test_result", Some("pass"), at, "sec1-test");
    add(run, "gate", "gate_result", Some("pass"), at, "sec1-test");
}

/// Handed over, with a change to publish, at `at`.
fn handed_over(run: &mut Run<'_>, at: &str) {
    add(
        run,
        "handover",
        "handover",
        None,
        at,
        takt::run::CHOICE_PRODUCER,
    );
    add(
        run,
        "change-record",
        "change_record",
        Some("ahead"),
        at,
        "takt.change-read",
    );
}

fn grant_of(case_dir: &Path) -> Option<AuthorityDecision> {
    Authority::read(case_dir)
        .expect("authority.yaml reads")
        .and_then(|authority| authority.decision(MERGE).cloned())
}

/// Opens a case on `main` of `tree`, commits `change`, records the tests, the gate, the
/// handover and the change record at the revision, and runs once through an in-memory forge
/// whose checks pass. Answers the stop (or the error), the console, the forge's calls and the
/// grant left in `authority.yaml`.
fn run_once(
    tree: &Path,
    id: &str,
    protocol: &str,
    change: (&str, &str),
) -> (
    Result<Stop, String>,
    String,
    Vec<String>,
    Option<AuthorityDecision>,
) {
    let memory = Rc::new(Memory {
        checks: Checks::Pass,
        ..Memory::default()
    });
    let forge: Rc<dyn Forge> = memory.clone();
    let validators = validators(&forge);
    let mut run = Run::open(tree, options(id, protocol), &Requester, &validators).expect("opens");
    let revision = commit_change(&mut run, tree, change);
    green(&mut run, &revision);
    handed_over(&mut run, &revision);
    let mut out = Vec::new();
    let stop = run.run(&mut out).map_err(|error| format!("{error:#}"));
    let dir = run.case.dir.clone();
    let calls = memory.calls.borrow().clone();
    (
        stop,
        String::from_utf8_lossy(&out).into_owned(),
        calls,
        grant_of(&dir),
    )
}

// ---------------------------------------------------------------------------------------------
// Invariant 1: no grant without an unrevoked delegation at the merge target's tip.
// ---------------------------------------------------------------------------------------------

/// The in-memory forge whose first merge fails as a forge outage would (a 5xx, a lost
/// connection), and every later one goes through.
struct FirstMergeFails {
    inner: Rc<Memory>,
    failed: Cell<bool>,
}

impl Forge for FirstMergeFails {
    fn name(&self) -> &str {
        self.inner.name()
    }
    fn scan_outgoing(
        &self,
        tree: &Path,
        dir: &Path,
        outgoing: &Outgoing<'_>,
    ) -> anyhow::Result<()> {
        self.inner.scan_outgoing(tree, dir, outgoing)
    }
    fn push(&self, tree: &Path, branch: &str) -> anyhow::Result<()> {
        self.inner.push(tree, branch)
    }
    fn push_revision(&self, tree: &Path, branch: &str, revision: &str) -> anyhow::Result<()> {
        self.inner.push_revision(tree, branch, revision)
    }
    fn open_pull_request(
        &self,
        tree: &Path,
        dir: &Path,
        request: &PullRequest,
    ) -> anyhow::Result<u64> {
        self.inner.open_pull_request(tree, dir, request)
    }
    fn pull_request(&self, tree: &Path, number: u64) -> anyhow::Result<PullRequestView> {
        self.inner.pull_request(tree, number)
    }
    fn checks(&self, tree: &Path, number: u64) -> anyhow::Result<Checks> {
        self.inner.checks(tree, number)
    }
    fn merge(&self, tree: &Path, dir: &Path, number: u64, head: &str) -> anyhow::Result<()> {
        if !self.failed.replace(true) {
            return Err(anyhow!("the forge answered 502 Bad Gateway"));
        }
        self.inner.merge(tree, dir, number, head)
    }
    fn merge_with(
        &self,
        tree: &Path,
        dir: &Path,
        number: u64,
        head: &str,
        _message: &MergeMessage,
    ) -> anyhow::Result<()> {
        self.merge(tree, dir, number, head)
    }
    fn target_tip(&self, tree: &Path, base: &str) -> anyhow::Result<String> {
        self.inner.target_tip(tree, base)
    }
}

/// Commits `text` as the delegation record on `main` without touching the work tree, as the
/// owner's commit on the forge would land: plumbing into a private index.
fn owner_commits_delegations(scratch: &Path, tree: &Path, text: &str) {
    let record = scratch.join("delegations-owner.yaml");
    fs::write(&record, text).expect("the record");
    let blob = git(
        tree,
        &["hash-object", "-w", record.to_str().expect("utf-8")],
    );
    let index = scratch.join("owner.index");
    let env = [("GIT_INDEX_FILE", index.to_str().expect("utf-8"))];
    git_env(tree, &["read-tree", "refs/heads/main"], &env);
    git_env(
        tree,
        &[
            "update-index",
            "--add",
            "--cacheinfo",
            &format!("100644,{blob},{DELEGATIONS}"),
        ],
        &env,
    );
    let written = git_env(tree, &["write-tree"], &env);
    let commit = git(
        tree,
        &[
            "commit-tree",
            &written,
            "-p",
            "refs/heads/main",
            "-m",
            "the owner revokes the delegation",
        ],
    );
    git(tree, &["update-ref", "refs/heads/main", &commit]);
}

/// AGENTS.md § Rules: a delegation "is revocable, and a revoked one grants nothing from then
/// on". An unattended run grants the merge as `agent:takt-delivery` under the owner's
/// delegation, and the forge fails the merge (an outage); the owner then revokes the delegation
/// on `main`; the pod's next run resumes the case. The grant in `authority.yaml` is still at the
/// revision, so the run does not grant again, and the merge validator judges the base with
/// `delegated: None` (`validator.rs:5393`), never reading the `repository.merge` delegation
/// again: the change merges under a delegation revoked before the merge.
#[test]
fn inv1_a_delegation_revoked_after_the_grant_stops_the_resumed_merge() {
    let (scratch, tree) = repository("revoked-after", Some(&delegations(&[entry(MERGE, false)])));
    let memory = Rc::new(Memory {
        checks: Checks::Pass,
        ..Memory::default()
    });
    let forge: Rc<dyn Forge> = Rc::new(FirstMergeFails {
        inner: memory.clone(),
        failed: Cell::new(false),
    });
    let validators = validators(&forge);
    let id = "revoked-after";
    let (first, case_dir) = {
        let mut run =
            Run::open(&tree, options(id, PROTOCOL), &Requester, &validators).expect("opens");
        let revision = commit_change(&mut run, &tree, DOCS_EDIT);
        green(&mut run, &revision);
        handed_over(&mut run, &revision);
        let mut out = Vec::new();
        let stop = run.run(&mut out);
        (
            format!("{stop:?}\n{}", String::from_utf8_lossy(&out)),
            run.case.dir.clone(),
        )
    };
    let granted = grant_of(&case_dir);
    assert_eq!(
        granted.as_ref().map(|grant| grant.approver.as_str()),
        Some(ROLE),
        "the first run granted as the role before the merge failed:\n{first}"
    );
    assert!(
        memory.merged.borrow().is_empty(),
        "the first merge failed:\n{first}"
    );

    owner_commits_delegations(&scratch, &tree, &delegations(&[entry(MERGE, true)]));

    let mut run =
        Run::resume(&tree, options(id, PROTOCOL), id, &Requester, &validators).expect("resumes");
    let mut out = Vec::new();
    let stop = run.run(&mut out);
    let second = format!("{stop:?}\n{}", String::from_utf8_lossy(&out));
    assert!(
        memory.merged.borrow().is_empty(),
        "the resumed run merged pull request {:?} under agent:takt-delivery's grant although {DELEGATIONS} at main now revokes the delegation of {MERGE}:\n--- first run ---\n{first}\n--- resumed run ---\n{second}",
        memory.merged.borrow()
    );
}

/// A delegation record the case's own diff adds grants nothing: the tip has none, the change
/// writes one delegating both capabilities to the role.
#[test]
fn inv1_a_delegation_record_the_case_adds_grants_nothing() {
    let (_scratch, tree) = repository("adds-record", None);
    let (stop, out, calls, grant) = run_once(
        &tree,
        "adds-record",
        PROTOCOL,
        (DELEGATIONS, &both_delegated()),
    );
    assert!(grant.is_none(), "granted: {grant:?}\n{out}");
    assert!(
        !calls.iter().any(|call| call.starts_with("merge ")),
        "{calls:?}\n{out}"
    );
    assert!(
        out.contains("not granted as agent:takt-delivery: ")
            && out.contains(".engineering/delegations.yaml is not there"),
        "{stop:?}\n{out}"
    );
}

// ---------------------------------------------------------------------------------------------
// Invariant 2: a judge-surface diff needs the judge-surface delegation.
// ---------------------------------------------------------------------------------------------

/// A change to the delegation record itself grants nothing even with both capabilities
/// delegated at the tip: it is owner-only.
#[test]
fn inv2_a_change_to_the_delegation_record_grants_nothing_though_delegated() {
    let (_scratch, tree) = repository("edits-record", Some(&both_delegated()));
    let widened = format!(
        "{}{}",
        both_delegated(),
        entry("repository.merge.owner_only", false)
    );
    let (stop, out, calls, grant) =
        run_once(&tree, "edits-record", PROTOCOL, (DELEGATIONS, &widened));
    assert!(grant.is_none(), "granted: {grant:?}\n{out}");
    assert!(
        !calls.iter().any(|call| call.starts_with("merge ")),
        "{calls:?}\n{out}"
    );
    assert!(
        out.contains("not granted as agent:takt-delivery: ")
            && out.contains("merges under the owner's grant (human:<id>) alone"),
        "{stop:?}\n{out}"
    );
}

/// A change to a protocol file is a judge surface: with `repository.merge` alone delegated
/// the run grants nothing and names the judge-surface delegation it lacks.
#[test]
fn inv2_a_change_to_a_protocol_file_needs_the_judge_surface_delegation() {
    let (_scratch, tree) = repository("edits-protocol", Some(&delegations(&[entry(MERGE, false)])));
    let (stop, out, calls, grant) = run_once(
        &tree,
        "edits-protocol",
        PROTOCOL,
        (
            "protocols/story-delivery/3.yaml",
            "format: protocol/1\n# widened\n",
        ),
    );
    assert!(grant.is_none(), "granted: {grant:?}\n{out}");
    assert!(
        !calls.iter().any(|call| call.starts_with("merge ")),
        "{calls:?}\n{out}"
    );
    assert!(
        out.contains("not granted as agent:takt-delivery: ")
            && out.contains("protocols/story-delivery/3.yaml")
            && out.contains(
                "records no unrevoked delegation of repository.merge.judge_surface to agent:takt-delivery"
            ),
        "{stop:?}\n{out}"
    );
}

// ---------------------------------------------------------------------------------------------
// Invariant 3: tests.pass, gate.green and ci.green at the exact revision.
// ---------------------------------------------------------------------------------------------

/// The tests and the gate green at an earlier revision, CI green at the current one: the run
/// grants nothing, though the merge is approval-required and the delegation stands.
#[test]
fn inv3_tests_and_gate_at_an_earlier_revision_grant_nothing() {
    let (scratch, tree) = repository("earlier", Some(&both_delegated()));
    let protocol = pr_open_without_tests(&scratch);
    let memory = Rc::new(Memory {
        checks: Checks::Pass,
        ..Memory::default()
    });
    let forge: Rc<dyn Forge> = memory.clone();
    let validators = validators(&forge);
    let mut run = Run::open(
        &tree,
        options("earlier", &protocol),
        &Requester,
        &validators,
    )
    .expect("opens");
    let first = commit_change(&mut run, &tree, DOCS_EDIT);
    green(&mut run, &first);
    let second = commit_change(&mut run, &tree, ("docs/other.md", "another doc\n"));
    handed_over(&mut run, &second);
    let mut out = Vec::new();
    let stop = run.run(&mut out);
    let out = String::from_utf8_lossy(&out).into_owned();
    eprintln!("{stop:?}\n{out}");
    let calls = memory.calls.borrow().clone();
    assert!(
        calls.contains(&"checks #1".to_owned()),
        "CI was read at the revision: {calls:?}\n{out}"
    );
    assert!(
        matches!(&stop, Ok(Stop::ApprovalRequired(gate)) if gate == &vec![MERGE.to_owned()]),
        "{stop:?}\n{out}"
    );
    let grant = grant_of(&run.case.dir);
    assert!(grant.is_none(), "granted: {grant:?}\n{out}");
    assert!(
        !calls.iter().any(|call| call.starts_with("merge ")),
        "{calls:?}\n{out}"
    );
}

// ---------------------------------------------------------------------------------------------
// Invariant 4: the model never writes the grant, the evidence or the delegation.
// ---------------------------------------------------------------------------------------------

/// On its step, writes into the case what would let the run merge: `test_result` and
/// `gate_result` records of `pass` at the revision, a grant of `repository.merge` as the role
/// in `authority.yaml`, and a delegation record in the tree; then asks for a read.
struct Forger {
    case_dir: RefCell<Option<PathBuf>>,
    stepped: Cell<u32>,
}

impl Harness for Forger {
    fn name(&self) -> &str {
        "forger"
    }

    fn step(&self, context: &StepContext<'_>) -> Result<StepReport, anyhow::Error> {
        let calls = self.stepped.get();
        self.stepped.set(calls + 1);
        if calls > 0 {
            return Ok(StepReport {
                action: MERGE.to_owned(),
                summary: "merge it".to_owned(),
                tokens: None,
            });
        }
        let dir = self
            .case_dir
            .borrow()
            .clone()
            .ok_or_else(|| anyhow!("no case dir"))?;
        let head = git(context.tree, &["rev-parse", "HEAD"]);
        for (n, kind) in ["test_result", "gate_result"].into_iter().enumerate() {
            let record = Record::new(
                format!("forged-{n}"),
                kind,
                Some("pass".to_owned()),
                "implementation",
                &head,
                "takt.tests-run",
            );
            fs::write(
                dir.join("evidence")
                    .join(format!("9{n:03}-forged-{n}.json")),
                record.to_json(),
            )?;
        }
        Authority {
            decisions: vec![AuthorityDecision {
                capability: MERGE.to_owned(),
                decision: Grant::Granted,
                approver: ROLE.to_owned(),
                granted_at: "2026-10-29T10:00:00Z".to_owned(),
                revision: Some(head),
                mainline_tip: None,
            }],
        }
        .write(&dir)?;
        fs::write(context.tree.join(DELEGATIONS), both_delegated())?;
        Ok(StepReport {
            action: "repository.inspect".to_owned(),
            summary: "read the repository".to_owned(),
            tokens: None,
        })
    }
}

/// A harness step that writes evidence, a grant and a delegation is refused and put back, and
/// nothing is granted or merged: the tip has no delegation, the tests and the gate never ran.
#[test]
fn inv4_a_step_that_forges_evidence_a_grant_and_a_delegation_grants_nothing() {
    let (scratch, tree) = repository("forger", None);
    let protocol = pr_open_without_tests(&scratch);
    let memory = Rc::new(Memory {
        checks: Checks::Pass,
        ..Memory::default()
    });
    let forge: Rc<dyn Forge> = memory.clone();
    let validators = validators(&forge);
    let harness = Forger {
        case_dir: RefCell::new(None),
        stepped: Cell::new(0),
    };
    let mut run =
        Run::open(&tree, options("forger", &protocol), &harness, &validators).expect("opens");
    harness.case_dir.replace(Some(run.case.dir.clone()));
    let revision = commit_change(&mut run, &tree, DOCS_EDIT);
    handed_over(&mut run, &revision);
    let mut out = Vec::new();
    let stop = run.run(&mut out);
    let out = String::from_utf8_lossy(&out).into_owned();
    eprintln!("{stop:?}\n{out}");
    let calls = memory.calls.borrow().clone();
    assert!(
        harness.stepped.get() >= 1,
        "the harness was asked: {stop:?}\n{out}"
    );
    let grant = grant_of(&run.case.dir);
    assert!(grant.is_none(), "granted: {grant:?}\n{stop:?}\n{out}");
    assert!(
        !calls.iter().any(|call| call.starts_with("merge ")),
        "{calls:?}\n{out}"
    );
    let forged: Vec<String> = fs::read_dir(run.case.dir.join("evidence"))
        .expect("evidence")
        .map(|entry| {
            entry
                .expect("entry")
                .file_name()
                .to_string_lossy()
                .into_owned()
        })
        .filter(|name| name.contains("forged"))
        .collect();
    assert!(forged.is_empty(), "forged records kept: {forged:?}\n{out}");
    assert!(
        !tree.join(DELEGATIONS).exists(),
        "the step's delegation record was kept in the tree\n{out}"
    );
}

// ---------------------------------------------------------------------------------------------
// Invariant 5: no credential a child does not read.
// ---------------------------------------------------------------------------------------------

/// The GitHub tokens `gh` read, which no forge program reads now that `gh` is gone.
const GH_TOKENS: [&str; 4] = [
    "GH_TOKEN",
    "GITHUB_TOKEN",
    "GH_ENTERPRISE_TOKEN",
    "GITHUB_ENTERPRISE_TOKEN",
];

/// No forge program takt starts reads a GitHub token: a forge child started by [`Process`]
/// receives none of the four. Run in a child of this binary whose environment holds them.
#[test]
fn inv5_forge_children_receive_no_github_token() {
    const MARK: &str = "TAKT_GOAL_A_SEC1_CHILD";
    if std::env::var_os(MARK).is_none() {
        let mut command = Command::new(std::env::current_exe().expect("the test binary"));
        command
            .args([
                "inv5_forge_children_receive_no_github_token",
                "--exact",
                "--test-threads",
                "1",
            ])
            .env_clear()
            .env(MARK, "1");
        for name in ["PATH", "HOME"] {
            if let Some(value) = std::env::var_os(name) {
                command.env(name, value);
            }
        }
        for name in GH_TOKENS {
            command.env(name, "fake-not-a-credential");
        }
        let output = command.output().expect("the child test runs");
        assert!(
            output.status.success(),
            "the child test failed:\n{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        return;
    }
    let exit = Process::default()
        .run(Path::new(env!("CARGO_TARGET_TMPDIR")), &["env".to_owned()])
        .expect("env runs");
    assert!(exit.success(), "{exit:?}");
    let leaked: Vec<&str> = GH_TOKENS
        .iter()
        .copied()
        .filter(|name| {
            exit.stdout
                .lines()
                .any(|line| line.starts_with(&format!("{name}=")))
        })
        .collect();
    assert!(
        leaked.is_empty(),
        "a forge child received {leaked:?}, which no forge program reads"
    );
}
