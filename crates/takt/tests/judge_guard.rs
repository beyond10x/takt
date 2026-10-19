//! story:judge-guard: a merge whose change from the case's base touches a path outside the
//! allow-list `.engineering/judge-allow.yaml` merges only under the owner's grant (`human:`) or
//! a grant from a role an unrevoked delegation in `.engineering/delegations.yaml` names. Both
//! files are read at the merge target's tip as the forge reports it (the in-memory double
//! reports the local `main`), never from the candidate's tree and never at the case's base
//! commit; a base commit that is not an ancestor of that tip refuses the merge.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::rc::Rc;

use anyhow::anyhow;
use takt::case::{AUTHORITY_FILE, Authority, Branch, grant};
use takt::evidence::Record;
use takt::forge::{Checks, Forge, Memory, Outgoing, PullRequest, PullRequestView};
use takt::frontier::{Aep, approve};
use takt::harness::{Harness, StepContext, StepReport};
use takt::validator::{ChangeRead, CiRead, Observation, PrMerged, PrOpen, TestsRun, Validator};
use takt::{Run, RunOptions, Stop};

/// The delivery protocol, relative to this crate.
const PROTOCOL: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../protocols/story-delivery/1.yaml"
);

/// The allow-list every scratch repository's `main` starts with.
const ALLOW: &str = "paths:\n  - docs/**\n  - README.md\n  - CHANGELOG.md\n";

/// The capability a delegation names.
const JUDGE_SURFACE: &str = "repository.merge.judge_surface";

/// Runs `git <args>` in `tree`, with `env` set; its trimmed standard output.
fn git_env(tree: &Path, args: &[&str], env: &[(&str, &Path)]) -> String {
    let mut command = Command::new("git");
    command
        .args(["-c", "user.name=t", "-c", "user.email=t@localhost"])
        .args(args)
        .current_dir(tree);
    for (name, value) in env {
        command.env(name, value);
    }
    let output = command.output().expect("git runs");
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

fn git(tree: &Path, args: &[&str]) -> String {
    git_env(tree, args, &[])
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

fn fresh(name: &str) -> Scratch {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("judge-guard-{}-{name}", std::process::id()));
    if dir.exists() {
        fs::remove_dir_all(&dir).expect("a stale scratch tree is removed");
    }
    fs::create_dir_all(&dir).expect("scratch dir");
    Scratch(dir)
}

/// A scratch repository whose `main` holds a README, a doc, the judge's own source, the
/// delivery protocol and the allow-list, plus `delegations` as `.engineering/delegations.yaml`
/// when given. The work tree is `tree/`; `tree/../index` is free for plumbing.
fn repository(name: &str, delegations: Option<&str>) -> (Scratch, PathBuf) {
    let scratch = fresh(name);
    let tree = scratch.join("tree");
    let files: Vec<(&str, String)> = vec![
        ("README.md", "a repository\n".to_owned()),
        ("docs/guide.md", "a guide\n".to_owned()),
        (
            "crates/takt/src/validator.rs",
            "pub fn judge() {}\n".to_owned(),
        ),
        (
            "protocols/story-delivery/1.yaml",
            fs::read_to_string(PROTOCOL).expect("the protocol"),
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

/// Commits `files` on top of `parent` without touching the work tree or its index, and
/// answers the commit; `index` is a scratch index file outside the tree.
fn commit_on(tree: &Path, index: &Path, parent: &str, files: &[(&str, &str)]) -> String {
    let env = [("GIT_INDEX_FILE", index)];
    git_env(tree, &["read-tree", parent], &env);
    for (path, text) in files {
        let blob_file = index.with_extension("blob");
        fs::write(&blob_file, text).expect("a blob file");
        let blob = git(
            tree,
            &["hash-object", "-w", blob_file.to_str().expect("utf-8")],
        );
        git_env(
            tree,
            &[
                "update-index",
                "--add",
                "--cacheinfo",
                &format!("100644,{blob},{path}"),
            ],
            &env,
        );
    }
    let written = git_env(tree, &["write-tree"], &env);
    git(
        tree,
        &[
            "commit-tree",
            &written,
            "-p",
            parent,
            "-m",
            "outside the loop",
        ],
    )
}

/// Moves `main` to a commit above its tip holding `files`, as the owner's merge would: the
/// merge target's tip as the in-memory forge reports it.
fn record_on_main(tree: &Path, files: &[(&str, &str)]) -> String {
    let tip = git(tree, &["rev-parse", "refs/heads/main"]);
    let index = tree.parent().expect("a parent").join("index");
    let commit = commit_on(tree, &index, &tip, files);
    git(tree, &["update-ref", "refs/heads/main", &commit, &tip]);
    commit
}

/// A delegation record of `JUDGE_SURFACE` to `role`, revoked when `revoked` says so.
fn delegation(role: &str, revoked: bool) -> String {
    let mut text = format!(
        "delegations:\n  - owner: human:owner\n    role: {role}\n    capability: {JUDGE_SURFACE}\n    given_at: 2026-10-19T08:00:00Z\n"
    );
    if revoked {
        text.push_str("    revoked_at: 2026-10-19T09:00:00Z\n");
    }
    text
}

/// A harness that requests the merge gate as soon as it is offered.
struct Requester;

impl Harness for Requester {
    fn name(&self) -> &str {
        "requester"
    }

    fn step(&self, context: &StepContext<'_>) -> Result<StepReport, anyhow::Error> {
        if context.approvals.contains(&"repository.merge".to_owned()) {
            return Ok(StepReport {
                action: "repository.merge".to_owned(),
                summary: "the intent holds; merge it".to_owned(),
                tokens: None,
            });
        }
        // The work committed is complete: hand it over, changing nothing.
        if context.offered.contains(&"change.handover".to_owned()) {
            return Ok(StepReport {
                action: "change.handover".to_owned(),
                summary: "hand it over".to_owned(),
                tokens: None,
            });
        }
        Err(anyhow!(
            "the gate was not offered; a judgement step among {} is left",
            context.offered.join(", ")
        ))
    }
}

fn options(id: &str, protocol: &str) -> RunOptions {
    RunOptions {
        protocol: protocol.to_owned(),
        intent: "change the judge\n\nA candidate.".to_owned(),
        inputs: Vec::new(),
        iterations: Some(8),
        hours: None,
        tokens: None,
        case_id: Some(id.to_owned()),
        gate: Some("repository.merge".to_owned()),
    }
}

/// A case `id` opened on `main` of `tree` whose branch carries one commit per entry of
/// `commits` (each a list of files written and committed), run until it stops at the merge
/// gate with its pull request open through a double; answers the implementation revision and
/// the commits in order.
fn at_the_gate(tree: &Path, id: &str, commits: &[&[(&str, &str)]]) -> (String, Vec<String>) {
    at_the_gate_on(tree, id, commits, PROTOCOL)
}

fn at_the_gate_on(
    tree: &Path,
    id: &str,
    commits: &[&[(&str, &str)]],
    protocol: &str,
) -> (String, Vec<String>) {
    at_the_gate_with(tree, id, commits.len(), protocol, |n, tree| {
        for (path, text) in commits[n] {
            let path = tree.join(path);
            fs::create_dir_all(path.parent().expect("a parent")).expect("a directory");
            fs::write(path, text).expect("a file");
        }
    })
}

/// As [`at_the_gate_on`], each of the `count` commits written by `write` (given its index and
/// the tree) before it is committed.
fn at_the_gate_with(
    tree: &Path,
    id: &str,
    count: usize,
    protocol: &str,
    write: impl Fn(usize, &Path),
) -> (String, Vec<String>) {
    let memory = Rc::new(Memory {
        checks: Checks::Pass,
        ..Memory::default()
    });
    let forge: Rc<dyn Forge> = memory;
    let validators: Vec<Box<dyn Validator>> = vec![
        Box::new(PrOpen::new(forge.clone())),
        Box::new(CiRead::new(forge.clone())),
        Box::new(PrMerged::new(forge)),
    ];
    let mut run =
        Run::open(tree, options(id, protocol), &Requester, &validators).expect("the case opens");
    let mut made = Vec::new();
    for n in 0..count {
        write(n, tree);
        git(tree, &["add", "-A", "--", ".", ":!.engineering/cases"]);
        git(tree, &["commit", "-q", "-m", "a candidate commit"]);
        made.push(git(tree, &["rev-parse", "HEAD"]));
    }
    let revision = made.last().expect("a commit").clone();
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
                "judge-guard-test",
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
    assert_eq!(
        stop,
        Stop::ApprovalRequired(vec!["repository.merge".to_owned()]),
        "{}",
        String::from_utf8_lossy(&out)
    );
    (revision, made)
}

fn case_dir(tree: &Path, id: &str) -> PathBuf {
    tree.join(".engineering/cases").join(id)
}

/// A double that has pull request #1 open for the case's branch at `head`, checks passing.
fn open_pull_request(id: &str, head: &str) -> Rc<Memory> {
    let branch = format!("takt/{id}");
    Rc::new(Memory {
        checks: Checks::Pass,
        opened: RefCell::new(vec![PullRequest {
            branch: branch.clone(),
            base: "main".to_owned(),
            title: String::new(),
            body: String::new(),
        }]),
        pushed: RefCell::new(BTreeMap::from([(branch, head.to_owned())])),
        ..Memory::default()
    })
}

/// `repository.merge` observed once at `revision` through a fresh double.
fn merge(tree: &Path, id: &str, revision: &str) -> (Observation, Vec<String>) {
    let memory = open_pull_request(id, revision);
    let observation = PrMerged::new(memory.clone())
        .observe(tree, revision, 20)
        .expect("repository.merge observes");
    let calls = memory.calls.borrow().clone();
    (observation, calls)
}

fn assert_merged(observation: &Observation, calls: &[String], what: &str) {
    assert_eq!(
        observation.records.len(),
        1,
        "{what}: {}",
        observation.summary
    );
    assert_eq!(
        observation.records[0].result.as_deref(),
        Some("merged"),
        "{what}: {}",
        observation.summary
    );
    assert!(calls.contains(&"merge #1".to_owned()), "{what}: {calls:?}");
}

fn assert_refused(observation: &Observation, calls: &[String], what: &str, names: &[&str]) {
    assert!(
        observation.records.is_empty(),
        "{what}: a record was written: {}",
        observation.summary
    );
    assert!(
        !calls.contains(&"merge #1".to_owned()),
        "{what}: the merge was asked for: {calls:?}; {}",
        observation.summary
    );
    assert!(
        observation.summary.contains("nothing merged"),
        "{what}: {}",
        observation.summary
    );
    for name in names {
        assert!(
            observation.summary.contains(name),
            "{what}: the summary does not name {name}: {}",
            observation.summary
        );
    }
}

/// The candidate edit of the judge's own source.
const JUDGE_EDIT: &[(&str, &str)] = &[(
    "crates/takt/src/validator.rs",
    "pub fn judge() { /* a candidate */ }\n",
)];

/// The story's acceptance: a case whose change touches `crates/takt/src/validator.rs`, granted
/// by `agent:planner`, is refused at the merge while the delegation record at the target's tip
/// names no unrevoked delegation to `agent:planner`; merges once one is on record; and is refused
/// again once its revocation is on record.
#[test]
fn a_judge_surface_merge_under_agent_planner_follows_the_delegation_on_record() {
    let (_scratch, tree) = repository("acceptance", None);
    let (revision, _) = at_the_gate(&tree, "acceptance", &[JUDGE_EDIT]);
    grant(&tree, "acceptance", "repository.merge", "agent:planner").expect("granted");

    let (observation, calls) = merge(&tree, "acceptance", &revision);
    assert_refused(
        &observation,
        &calls,
        "no delegation",
        &["crates/takt/src/validator.rs", "agent:planner"],
    );

    record_on_main(
        &tree,
        &[(
            ".engineering/delegations.yaml",
            &delegation("agent:planner", false),
        )],
    );
    let (observation, calls) = merge(&tree, "acceptance", &revision);
    assert_merged(&observation, &calls, "delegated");

    record_on_main(
        &tree,
        &[(
            ".engineering/delegations.yaml",
            &delegation("agent:planner", true),
        )],
    );
    let (observation, calls) = merge(&tree, "acceptance", &revision);
    assert_refused(
        &observation,
        &calls,
        "revoked",
        &["crates/takt/src/validator.rs", "agent:planner"],
    );
}

/// The added acceptance: a case whose base commit is set, outside the loop, to a candidate
/// commit that adds `crates/**` to the allow-list is refused at the merge naming the base, under
/// `agent:planner` with no delegation. The case's only judge-surface edit is that commit, below
/// the moved base, so the change from the base is docs alone: only the ancestry refuses it.
#[test]
fn a_base_commit_that_is_not_an_ancestor_of_the_target_refuses_the_merge_naming_the_base() {
    let (_scratch, tree) = repository("moved-base", None);
    let widened = format!("{ALLOW}  - crates/**\n");
    let (revision, made) = at_the_gate(
        &tree,
        "moved-base",
        &[
            &[(".engineering/judge-allow.yaml", widened.as_str())],
            &[("docs/guide.md", "a guide past the moved base\n")],
        ],
    );
    grant(&tree, "moved-base", "repository.merge", "agent:planner").expect("granted");
    let dir = case_dir(&tree, "moved-base");
    let mut branch = Branch::read(&dir).expect("the branch record");
    branch.base_commit = Some(made[0].clone());
    branch.write(&dir).expect("written");
    assert_eq!(
        git(&tree, &["diff", "--name-only", &made[0], &revision]),
        "docs/guide.md",
        "the change from the moved base is docs alone"
    );

    let (observation, calls) = merge(&tree, "moved-base", &revision);
    assert_refused(
        &observation,
        &calls,
        "moved base",
        &[
            &format!(
                "the case's base {} is not an ancestor of main",
                &made[0][..8]
            ),
            "the merge target's tip",
        ],
    );
}

/// The fixture stores, relative to this crate.
const FIXTURES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures");

/// The story's file in the planning store.
const STORY_FILE: &str = ".engineering/planning/story/ready-one.md";

/// The subject of the approval's commit the store makes for `story:ready-one`.
const APPROVE_SUBJECT: &str = "takt: approve story:ready-one repository.merge";

/// The message of takt's take commit for `story:ready-one`.
const TAKE_SUBJECT: &str = "takt: take story:ready-one";

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

/// A scratch repository on `main` holding a committed copy of the `one-ready` store, a doc and
/// `allow` as the allow-list: the work tree is the scratch directory itself.
fn story_repository(name: &str, allow: &str) -> Scratch {
    let scratch = fresh(name);
    copy_dir(&Path::new(FIXTURES).join("one-ready"), &scratch);
    fs::create_dir_all(scratch.join("docs")).expect("a directory");
    fs::write(scratch.join("docs/guide.md"), "a guide\n").expect("a file");
    fs::write(scratch.join(".engineering/judge-allow.yaml"), allow).expect("a file");
    git(&scratch, &["init", "-q", "-b", "main"]);
    git(&scratch, &["add", "-A"]);
    git(&scratch, &["commit", "-q", "-m", "start"]);
    scratch
}

/// `gate.run` without a gate: one `gate_result` pass at the revision.
struct GatePass;

impl Validator for GatePass {
    fn name(&self) -> &str {
        "record.gate-pass"
    }

    fn action(&self) -> &str {
        "gate.run"
    }

    fn kind(&self) -> &str {
        "gate_result"
    }

    fn subject(&self) -> &str {
        "implementation"
    }

    fn observe(&self, _: &Path, revision: &str, seq: usize) -> Result<Observation, anyhow::Error> {
        Ok(Observation {
            summary: "pass".to_owned(),
            records: vec![Record::new(
                format!("gate-result-{seq}"),
                self.kind(),
                Some("pass".to_owned()),
                self.subject(),
                revision,
                self.name(),
            )],
            artifact: None,
        })
    }
}

/// The validators of a delivery run through `forge`: the real tests validator, its command
/// `true`, `change.read`, a gate that passes, and the forge's three.
fn delivery(forge: Rc<dyn Forge>) -> Vec<Box<dyn Validator>> {
    vec![
        Box::new(TestsRun::new("true").expect("a command")),
        Box::new(ChangeRead),
        Box::new(GatePass),
        Box::new(PrOpen::new(forge.clone())),
        Box::new(CiRead::new(forge.clone())),
        Box::new(PrMerged::new(forge)),
    ]
}

fn story_options() -> RunOptions {
    RunOptions {
        protocol: PROTOCOL.to_owned(),
        intent: String::new(),
        inputs: Vec::new(),
        iterations: Some(12),
        hours: None,
        tokens: None,
        case_id: None,
        gate: Some("repository.merge".to_owned()),
    }
}

/// A case of `story:ready-one` taken, its work a change to `docs/guide.md` alone, run to the
/// gate through a forge double whose checks pass with the store recording, and granted the
/// merge through the store by `approver`, whose approval's commit rides above the grant.
struct StoryCase {
    repo: Scratch,
    memory: Rc<Memory>,
    id: String,
    /// The case's pinned base commit.
    base: String,
    /// The revision the grant authorises.
    granted: String,
}

fn story_case(name: &str, approver: &str) -> StoryCase {
    story_case_with(name, approver, |_| {})
}

/// As [`story_case`], `tamper` run on the tree right after the take is committed (its `HEAD`
/// the take commit), before the work is.
fn story_case_with(name: &str, approver: &str, tamper: impl Fn(&Path)) -> StoryCase {
    let repo = story_repository(name, ALLOW);
    let memory = Rc::new(Memory {
        checks: Checks::Pass,
        ..Memory::default()
    });
    let forge: Rc<dyn Forge> = memory.clone();
    let validators = delivery(forge);
    let store = Aep::default();
    let mut out = Vec::new();
    let opened = Run::open_next(
        &repo,
        story_options(),
        &store,
        &Requester,
        &validators,
        &mut out,
    )
    .expect("the frontier answers")
    .expect("story:ready-one is taken");
    let id = opened.case.id.clone();
    let dir = opened.case.dir.clone();
    drop(opened);
    assert_eq!(
        git(&repo, &["log", "-1", "--format=%s"]),
        TAKE_SUBJECT,
        "the take is committed"
    );
    tamper(&repo);
    fs::write(repo.join("docs/guide.md"), "a better guide\n").expect("a file");
    git(&repo, &["commit", "-q", "-am", "the work"]);
    let granted = git(&repo, &["rev-parse", "HEAD"]);
    let mut run = Run::resume(&repo, story_options(), &id, &Requester, &validators)
        .expect("the case resumes");
    run.record_on(&store);
    let mut out = Vec::new();
    let stop = run.run(&mut out).expect("the run stops without an error");
    drop(run);
    assert_eq!(
        stop,
        Stop::ApprovalRequired(vec!["repository.merge".to_owned()]),
        "{}",
        String::from_utf8_lossy(&out)
    );
    approve(&repo, &store, &id, "repository.merge", approver).expect("the approval is given");
    assert_eq!(
        git(&repo, &["log", "--format=%s", &format!("{granted}..HEAD")]),
        APPROVE_SUBJECT,
        "the approval's own commit rides above the grant"
    );
    let base = Branch::read(&dir)
        .expect("the branch record")
        .base_commit
        .expect("a pinned base");
    StoryCase {
        repo,
        memory,
        id,
        base,
        granted,
    }
}

/// The added acceptance's second half: a head that carries only takt's approval riders above
/// the granted revision merges as before. A docs-only change granted by `agent:planner` with no
/// delegation, the allow-list as shipped, the store's approval record committed above the
/// grant: the guard judges what merging the granted revision does, never the rider in
/// `.engineering/evidence/`, and takt's own take of the story is no judge-surface change.
#[test]
fn a_head_with_only_approval_records_above_the_grant_merges_as_before() {
    let case = story_case("riders", "agent:planner");
    let head = git(&case.repo, &["rev-parse", "HEAD"]);
    assert_ne!(head, case.granted, "a rider sits above the grant");
    let (observation, calls) = merge(&case.repo, &case.id, &case.granted);
    assert_merged(
        &observation,
        &calls,
        "approval riders above a docs-only grant",
    );
}

/// A case whose diff touches only `docs/` merges under `agent:planner` with no delegation, on a
/// story's case end to end: taken from the store, run to the gate with the store recording,
/// approved through the store, and run past the gate to the merge. The change up to the grant
/// holds the story's file besides, changed by takt's take commit alone, which is exempt.
#[test]
fn a_docs_only_story_case_merges_under_agent_planner_end_to_end() {
    let case = story_case("story-docs", "agent:planner");
    assert_eq!(
        git(
            &case.repo,
            &["diff", "--name-only", &case.base, &case.granted]
        ),
        format!("{STORY_FILE}\ndocs/guide.md"),
        "the change up to the grant"
    );
    let forge: Rc<dyn Forge> = case.memory.clone();
    let validators = delivery(forge);
    let store = Aep::default();
    let mut run = Run::resume(
        &case.repo,
        story_options(),
        &case.id,
        &Requester,
        &validators,
    )
    .expect("the case resumes");
    run.record_on(&store);
    run.poll = std::time::Duration::from_millis(5);
    let mut out = Vec::new();
    let stop = run.run(&mut out).expect("the run stops without an error");
    let text = String::from_utf8_lossy(&out);
    assert_eq!(stop, Stop::Completed("accepted".to_owned()), "{text}");
    assert_eq!(case.memory.merged.borrow().as_slice(), [1], "{text}");
}

/// The take commit replaced by one whose message is `message`, on `parent` (the take's own
/// parent when `None`), its tree the take's: the branch moved to it, the index already its tree.
fn retake(tree: &Path, parent: Option<&str>, message: &[&str]) {
    let taken = git(tree, &["rev-parse", "HEAD^{tree}"]);
    let parent = parent.map_or_else(|| git(tree, &["rev-parse", "HEAD~1"]), str::to_owned);
    let mut args = vec!["commit-tree", taken.as_str(), "-p", parent.as_str()];
    for paragraph in message {
        args.extend(["-m", paragraph]);
    }
    let commit = git(tree, &args);
    git(tree, &["reset", "-q", "--soft", &commit]);
}

/// R3 (i), the parent: a take commit whose parent is not the case's base commit (an empty
/// commit sits between them) exempts nothing; the story's file is a judge surface.
#[test]
fn a_take_commit_on_another_parent_exempts_nothing() {
    let case = story_case_with("take-parent", "agent:planner", |tree| {
        let base = git(tree, &["rev-parse", "HEAD~1"]);
        let between = git(
            tree,
            &[
                "commit-tree",
                &format!("{base}^{{tree}}"),
                "-p",
                &base,
                "-m",
                "a commit between",
            ],
        );
        retake(tree, Some(&between), &[TAKE_SUBJECT]);
    });
    let (observation, calls) = merge(&case.repo, &case.id, &case.granted);
    assert_refused(&observation, &calls, "another parent", &[STORY_FILE]);
}

/// R3 (i), the message: a take commit whose subject is right but whose message says more
/// exempts nothing.
#[test]
fn a_take_commit_with_another_message_exempts_nothing() {
    let case = story_case_with("take-message", "agent:planner", |tree| {
        retake(tree, None, &[TAKE_SUBJECT, "and more"]);
    });
    let (observation, calls) = merge(&case.repo, &case.id, &case.granted);
    assert_refused(&observation, &calls, "another message", &[STORY_FILE]);
}

/// R3 (i), the one path: a take commit that changes a second path, an admitted doc, exempts
/// nothing.
#[test]
fn a_take_commit_with_a_second_path_exempts_nothing() {
    let case = story_case_with("take-second", "agent:planner", |tree| {
        fs::write(tree.join("docs/extra.md"), "an extra doc\n").expect("a file");
        git(tree, &["add", "docs/extra.md"]);
        git(tree, &["commit", "-q", "--amend", "--no-edit"]);
    });
    let (observation, calls) = merge(&case.repo, &case.id, &case.granted);
    assert_refused(&observation, &calls, "a second path", &[STORY_FILE]);
}

/// R3 (ii): under a valid take commit, the work also edits the story's body below its
/// frontmatter; the body differs between the target's tip and the merge, and the file is a
/// judge surface.
#[test]
fn a_story_body_edit_under_a_valid_take_is_a_judge_surface() {
    let case = story_case_with("take-body", "agent:planner", |tree| {
        let path = tree.join(STORY_FILE);
        let mut text = fs::read_to_string(&path).expect("the story");
        text.push_str("\nA line the candidate adds below the frontmatter.\n");
        fs::write(&path, text).expect("a file");
    });
    let (observation, calls) = merge(&case.repo, &case.id, &case.granted);
    assert_refused(&observation, &calls, "a body edit", &[STORY_FILE]);
}

/// The same case merges under `human:<id>`, with or without a delegation on record.
#[test]
fn the_owner_merges_a_judge_surface_change_with_or_without_a_delegation() {
    for (name, delegations) in [
        ("owner-bare", None),
        ("owner-delegated", Some(delegation("agent:planner", false))),
    ] {
        let (_scratch, tree) = repository(name, delegations.as_deref());
        let (revision, _) = at_the_gate(&tree, name, &[JUDGE_EDIT]);
        grant(&tree, name, "repository.merge", "human:owner").expect("granted");
        let (observation, calls) = merge(&tree, name, &revision);
        assert_merged(&observation, &calls, name);
    }
}

/// A case whose change touches only `docs/` merges under `agent:planner` with no delegation.
#[test]
fn a_docs_only_change_merges_under_agent_planner_without_a_delegation() {
    let (_scratch, tree) = repository("docs-only", None);
    let (revision, _) = at_the_gate(
        &tree,
        "docs-only",
        &[&[("docs/guide.md", "a better guide\n")]],
    );
    grant(&tree, "docs-only", "repository.merge", "agent:planner").expect("granted");
    let (observation, calls) = merge(&tree, "docs-only", &revision);
    assert_merged(&observation, &calls, "docs only");
}

/// A candidate that edits the allow-list or the delegation record to admit its own paths or its
/// own role touches a judge surface, and its own edit counts for nothing: both are read at the
/// target's tip.
#[test]
fn a_candidate_admitting_itself_in_the_lists_is_refused() {
    let widened = format!("{ALLOW}  - crates/**\n");
    let delegated = delegation("agent:planner", false);
    for (name, files) in [
        (
            "own-allow",
            vec![
                (".engineering/judge-allow.yaml", widened.as_str()),
                JUDGE_EDIT[0],
            ],
        ),
        (
            "own-delegation",
            vec![
                (".engineering/delegations.yaml", delegated.as_str()),
                JUDGE_EDIT[0],
            ],
        ),
        (
            "only-the-allow-list",
            vec![(".engineering/judge-allow.yaml", widened.as_str())],
        ),
    ] {
        let (_scratch, tree) = repository(name, None);
        let (revision, _) = at_the_gate(&tree, name, &[files.as_slice()]);
        grant(&tree, name, "repository.merge", "agent:planner").expect("granted");
        let (observation, calls) = merge(&tree, name, &revision);
        assert_refused(&observation, &calls, name, &["agent:planner"]);
    }
}

/// A delegation change merges only on the owner's grant: a case whose change touches
/// `.engineering/judge-allow.yaml` or `.engineering/delegations.yaml` is refused under
/// `agent:planner` although the delegation record at the target's tip delegates
/// `repository.merge.judge_surface` to it, naming the file; and so it is when the allow-list at
/// the tip admits `.engineering/**`, so the file is no judge surface by its name.
#[test]
fn a_change_to_the_lists_merges_under_the_owners_grant_alone() {
    let delegated = delegation("agent:planner", false);
    let widened = format!("{ALLOW}  - crates/**\n");
    let developer = delegation("agent:developer", false);
    let admitting = format!("{ALLOW}  - .engineering/**\n");
    for (name, allow, file, text) in [
        (
            "delegated-allow",
            None,
            ".engineering/judge-allow.yaml",
            widened.as_str(),
        ),
        (
            "delegated-delegations",
            None,
            ".engineering/delegations.yaml",
            developer.as_str(),
        ),
        (
            "admitted-delegations",
            Some(admitting.as_str()),
            ".engineering/delegations.yaml",
            developer.as_str(),
        ),
    ] {
        let (_scratch, tree) = repository(name, Some(&delegated));
        if let Some(allow) = allow {
            fs::write(tree.join(".engineering/judge-allow.yaml"), allow).expect("a file");
            git(
                &tree,
                &["commit", "-q", "-am", "the owner widens the allow-list"],
            );
        }
        let (revision, _) = at_the_gate(&tree, name, &[&[(file, text)]]);
        grant(&tree, name, "repository.merge", "agent:planner").expect("granted");
        let (observation, calls) = merge(&tree, name, &revision);
        assert_refused(
            &observation,
            &calls,
            name,
            &[
                file,
                "agent:planner",
                "under the owner's grant (human:<id>) alone",
            ],
        );
    }
}

/// A symbolic link under `docs/` is a judge surface whatever its name: what it points at is not
/// the doc its name stands for. `agent:planner` with no delegation does not merge it.
#[test]
fn a_symlink_under_docs_is_a_judge_surface() {
    let (_scratch, tree) = repository("docs-link", None);
    let (revision, _) = at_the_gate_with(&tree, "docs-link", 1, PROTOCOL, |_, tree| {
        std::os::unix::fs::symlink(
            "../crates/takt/src/validator.rs",
            tree.join("docs/judge.md"),
        )
        .expect("a link");
    });
    assert_eq!(
        git(&tree, &["ls-tree", &revision, "docs/judge.md"])
            .split_whitespace()
            .next(),
        Some("120000"),
        "committed as a link"
    );
    grant(&tree, "docs-link", "repository.merge", "agent:planner").expect("granted");
    let (observation, calls) = merge(&tree, "docs-link", &revision);
    assert_refused(
        &observation,
        &calls,
        "a link under docs",
        &["docs/judge.md", "agent:planner"],
    );
}

/// A candidate branch's own `protocols/story-delivery/1.yaml` is not the protocol its case is
/// judged by: the case keeps the protocol it opened on, and the edit is a judge surface that
/// `agent:planner` cannot merge without a delegation.
#[test]
fn a_candidate_protocol_is_not_its_judge() {
    let (_scratch, tree) = repository("own-protocol", None);
    let opened_on = tree.join("protocols/story-delivery/1.yaml");
    let base_protocol = fs::read_to_string(&opened_on).expect("the protocol");
    let (revision, _) = at_the_gate_on(
        &tree,
        "own-protocol",
        &[&[(
            "protocols/story-delivery/1.yaml",
            "protocol: the candidate's own\n",
        )]],
        opened_on.to_str().expect("utf-8"),
    );
    let dir = case_dir(&tree, "own-protocol");
    assert_eq!(
        fs::read_to_string(dir.join("protocol.yaml")).expect("the case's protocol"),
        base_protocol
    );
    grant(&tree, "own-protocol", "repository.merge", "agent:planner").expect("granted");
    let (observation, calls) = merge(&tree, "own-protocol", &revision);
    assert_refused(
        &observation,
        &calls,
        "own protocol",
        &["protocols/story-delivery/1.yaml"],
    );
}

/// A delegation naming another role (`agent:developer`) does not admit an `agent:planner`
/// grant.
#[test]
fn a_delegation_to_another_role_admits_nothing() {
    let (_scratch, tree) = repository("other-role", Some(&delegation("agent:developer", false)));
    let (revision, _) = at_the_gate(&tree, "other-role", &[JUDGE_EDIT]);
    grant(&tree, "other-role", "repository.merge", "agent:planner").expect("granted");
    let (observation, calls) = merge(&tree, "other-role", &revision);
    assert_refused(&observation, &calls, "another role", &["agent:planner"]);
}

/// D5: an approver in `authority.yaml` that is neither `human:` nor `agent:` (written there
/// by hand, or before approvers were typed) grants nothing at a judge-surface merge.
#[test]
fn an_untyped_approver_grants_nothing_at_a_judge_surface_merge() {
    let (_scratch, tree) = repository("untyped", Some(&delegation("agent:planner", false)));
    let (revision, _) = at_the_gate(&tree, "untyped", &[JUDGE_EDIT]);
    grant(&tree, "untyped", "repository.merge", "human:owner").expect("granted");
    let dir = case_dir(&tree, "untyped");
    let mut authority = Authority::read(&dir).expect("reads").expect("a grant");
    authority.decisions[0].approver = "operator".to_owned();
    authority.write(&dir).expect("written");
    assert!(dir.join(AUTHORITY_FILE).is_file());
    let (observation, calls) = merge(&tree, "untyped", &revision);
    assert_refused(&observation, &calls, "untyped", &["operator"]);
}

/// The outcome: `takt approve --by` takes only `human:<id>` or `agent:<role>`, on a case opened
/// on an intent as on a story's.
#[test]
fn a_grant_by_an_untyped_approver_is_refused() {
    let (_scratch, tree) = repository("approver", None);
    at_the_gate(&tree, "approver", &[JUDGE_EDIT]);
    for approver in ["operator", "service:bot", "agent:", "human"] {
        let error = grant(&tree, "approver", "repository.merge", approver)
            .expect_err(&format!("{approver} is refused"));
        assert!(
            format!("{error:#}").contains("human:<name> or agent:<role>"),
            "{approver}: {error:#}"
        );
    }
    grant(&tree, "approver", "repository.merge", "agent:planner").expect("granted");
}

/// D5: takt's own actor, `agent:takt`, written into `authority.yaml` by hand, grants nothing at
/// a judge-surface merge, even with a delegation on record naming it: it is no role the store
/// writes a grant as.
#[test]
fn takts_own_actor_grants_nothing_even_when_a_delegation_names_it() {
    let (_scratch, tree) = repository("own-actor", Some(&delegation("agent:takt", false)));
    let (revision, _) = at_the_gate(&tree, "own-actor", &[JUDGE_EDIT]);
    grant(&tree, "own-actor", "repository.merge", "human:owner").expect("granted");
    let dir = case_dir(&tree, "own-actor");
    let mut authority = Authority::read(&dir).expect("reads").expect("a grant");
    authority.decisions[0].approver = "agent:takt".to_owned();
    authority.write(&dir).expect("written");
    let (observation, calls) = merge(&tree, "own-actor", &revision);
    assert_refused(
        &observation,
        &calls,
        "takt's own actor",
        &["`agent:takt` is neither the owner"],
    );
}

/// R1: a candidate whose merge into the target conflicts is refused, naming the path, even a
/// doc under the owner's grant: what the merge would change is not known.
#[test]
fn a_merge_that_conflicts_with_the_target_is_refused() {
    let (_scratch, tree) = repository("conflict", None);
    let (revision, _) = at_the_gate(&tree, "conflict", &[&[("docs/guide.md", "ours\n")]]);
    record_on_main(&tree, &[("docs/guide.md", "theirs\n")]);
    grant(&tree, "conflict", "repository.merge", "human:owner").expect("granted");
    let (observation, calls) = merge(&tree, "conflict", &revision);
    assert_refused(
        &observation,
        &calls,
        "a conflict",
        &["conflicts at docs/guide.md"],
    );
}

/// A forge wrapping the double that reports the target's tip as `answer`.
struct Answering(Rc<Memory>, String);

impl Forge for Answering {
    fn name(&self) -> &str {
        "answering"
    }
    fn scan_outgoing(
        &self,
        tree: &Path,
        dir: &Path,
        outgoing: &Outgoing<'_>,
    ) -> anyhow::Result<()> {
        self.0.scan_outgoing(tree, dir, outgoing)
    }
    fn push(&self, tree: &Path, branch: &str) -> anyhow::Result<()> {
        self.0.push(tree, branch)
    }
    fn open_pull_request(
        &self,
        tree: &Path,
        dir: &Path,
        request: &PullRequest,
    ) -> anyhow::Result<u64> {
        self.0.open_pull_request(tree, dir, request)
    }
    fn pull_request(&self, tree: &Path, number: u64) -> anyhow::Result<PullRequestView> {
        self.0.pull_request(tree, number)
    }
    fn checks(&self, tree: &Path, number: u64) -> anyhow::Result<Checks> {
        self.0.checks(tree, number)
    }
    fn merge(&self, tree: &Path, dir: &Path, number: u64, head: &str) -> anyhow::Result<()> {
        self.0.merge(tree, dir, number, head)
    }
    fn target_tip(&self, _: &Path, _: &str) -> anyhow::Result<String> {
        Ok(self.1.clone())
    }
}

/// The forge's answer for the target's tip is an object id, never a name git would resolve: a
/// branch, a ref, `HEAD`, an abbreviation or the tip's id in capitals merge nothing, even a
/// docs-only change under the owner's grant.
#[test]
fn a_target_tip_that_is_no_object_id_merges_nothing() {
    let (_scratch, tree) = repository("tip-name", None);
    let (revision, _) = at_the_gate(&tree, "tip-name", &[&[("docs/guide.md", "more\n")]]);
    grant(&tree, "tip-name", "repository.merge", "human:owner").expect("granted");
    let main = git(&tree, &["rev-parse", "refs/heads/main"]);
    for answer in [
        "main".to_owned(),
        "refs/heads/main".to_owned(),
        "HEAD".to_owned(),
        main[..12].to_owned(),
        main.to_uppercase(),
    ] {
        let memory = open_pull_request("tip-name", &revision);
        let observation = PrMerged::new(Rc::new(Answering(memory.clone(), answer.clone())))
            .observe(&tree, &revision, 20)
            .expect("repository.merge observes");
        let calls = memory.calls.borrow().clone();
        assert_refused(
            &observation,
            &calls,
            &answer,
            &["which is no commit this repository holds"],
        );
    }
}

/// A forge wrapping the double that does not say what the target's tip is.
struct Silent(Rc<Memory>);

impl Forge for Silent {
    fn name(&self) -> &str {
        "silent"
    }
    fn scan_outgoing(
        &self,
        tree: &Path,
        dir: &Path,
        outgoing: &Outgoing<'_>,
    ) -> anyhow::Result<()> {
        self.0.scan_outgoing(tree, dir, outgoing)
    }
    fn push(&self, tree: &Path, branch: &str) -> anyhow::Result<()> {
        self.0.push(tree, branch)
    }
    fn open_pull_request(
        &self,
        tree: &Path,
        dir: &Path,
        request: &PullRequest,
    ) -> anyhow::Result<u64> {
        self.0.open_pull_request(tree, dir, request)
    }
    fn pull_request(&self, tree: &Path, number: u64) -> anyhow::Result<PullRequestView> {
        self.0.pull_request(tree, number)
    }
    fn checks(&self, tree: &Path, number: u64) -> anyhow::Result<Checks> {
        self.0.checks(tree, number)
    }
    fn merge(&self, tree: &Path, dir: &Path, number: u64, head: &str) -> anyhow::Result<()> {
        self.0.merge(tree, dir, number, head)
    }
}

/// D7: a forge that cannot report the merge target's tip never merges: without the tip there
/// is no allow-list to read, so every path is a judge surface, even under the owner's grant.
#[test]
fn a_forge_that_cannot_report_the_target_tip_merges_nothing() {
    let (_scratch, tree) = repository("silent", None);
    let (revision, _) = at_the_gate(&tree, "silent", &[&[("docs/guide.md", "more\n")]]);
    grant(&tree, "silent", "repository.merge", "human:owner").expect("granted");
    let memory = open_pull_request("silent", &revision);
    let outcome = PrMerged::new(Rc::new(Silent(memory.clone()))).observe(&tree, &revision, 20);
    let summary = match &outcome {
        Ok(observation) => {
            assert!(observation.records.is_empty(), "{}", observation.summary);
            observation.summary.clone()
        }
        Err(error) => format!("{error:#}"),
    };
    assert!(
        !memory.calls.borrow().contains(&"merge #1".to_owned()),
        "{summary}"
    );
    assert!(summary.contains("tip"), "{summary}");
}

/// A delegation of `JUDGE_SURFACE` to `agent:developer`, as one more entry of a record.
const DEVELOPER_ENTRY: &str = "  - owner: human:owner\n    role: agent:developer\n    capability: repository.merge.judge_surface\n    given_at: 2026-10-19T08:00:00Z\n";

/// A record delegating to `agent:planner` and to `agent:developer`.
fn both_delegated() -> String {
    format!("{}{DEVELOPER_ENTRY}", delegation("agent:planner", false))
}

/// The same record with the developer's delegation revoked.
fn developer_revoked() -> String {
    format!(
        "{}{DEVELOPER_ENTRY}    revoked_at: 2026-10-19T09:00:00Z\n",
        delegation("agent:planner", false)
    )
}

/// The paths merging `revision` into `main` changes: the three-way merge's tree against main's.
fn merge_changes(tree: &Path, revision: &str) -> String {
    let merged = git(tree, &["merge-tree", "--write-tree", "main", revision]);
    let merged = merged.lines().next().expect("a tree").to_owned();
    git(tree, &["diff", "--name-only", "main", &merged])
}

/// A hidden revert through a merge of the target: the owner revokes the developer's delegation
/// on `main`; the case, granted by a delegated `agent:planner`, merges `main` and then restores
/// the record to its content at the case's base. The change from the base shows nothing of the
/// record, but merging the head into `main` undoes the revocation: refused, naming the record.
#[test]
fn a_merge_of_the_target_that_restores_a_revoked_delegation_is_refused() {
    let before = both_delegated();
    let revoked = developer_revoked();
    let (_scratch, tree) = repository("revert-revocation", Some(&before));
    let (revision, _) = at_the_gate_with(&tree, "revert-revocation", 2, PROTOCOL, |n, tree| {
        if n == 0 {
            record_on_main(tree, &[(".engineering/delegations.yaml", revoked.as_str())]);
            git(tree, &["merge", "--no-commit", "--no-ff", "main"]);
            fs::write(tree.join("docs/guide.md"), "a guide\nmore\n").expect("a file");
        } else {
            fs::write(tree.join(".engineering/delegations.yaml"), &before).expect("a file");
        }
    });
    assert!(merge_changes(&tree, &revision).contains(".engineering/delegations.yaml"));
    grant(
        &tree,
        "revert-revocation",
        "repository.merge",
        "agent:planner",
    )
    .expect("granted");
    let (observation, calls) = merge(&tree, "revert-revocation", &revision);
    assert_refused(
        &observation,
        &calls,
        "a hidden revert of a revocation",
        &[".engineering/delegations.yaml"],
    );
}

/// The same with no delegation on record at all: the owner narrows the allow-list on `main`;
/// the case merges `main`, changes a doc and restores the wide allow-list it was based on.
#[test]
fn a_merge_of_the_target_that_restores_a_narrowed_allow_list_is_refused() {
    let wide = format!("{ALLOW}  - crates/**\n");
    let (_scratch, tree) = repository("revert-allow", None);
    fs::write(tree.join(".engineering/judge-allow.yaml"), &wide).expect("a file");
    git(&tree, &["commit", "-q", "-am", "an old wide allow-list"]);
    let (revision, _) = at_the_gate_with(&tree, "revert-allow", 2, PROTOCOL, |n, tree| {
        if n == 0 {
            record_on_main(tree, &[(".engineering/judge-allow.yaml", ALLOW)]);
            git(tree, &["merge", "--no-commit", "--no-ff", "main"]);
            fs::write(tree.join("docs/guide.md"), "a guide\nmore\n").expect("a file");
        } else {
            fs::write(tree.join(".engineering/judge-allow.yaml"), &wide).expect("a file");
        }
    });
    assert!(merge_changes(&tree, &revision).contains(".engineering/judge-allow.yaml"));
    grant(&tree, "revert-allow", "repository.merge", "agent:planner").expect("granted");
    let (observation, calls) = merge(&tree, "revert-allow", &revision);
    assert_refused(
        &observation,
        &calls,
        "a hidden revert of the allow-list",
        &[".engineering/judge-allow.yaml"],
    );
}

/// A base commit that is an ancestor of the target's tip but older than the case's fork point:
/// the candidate, branched from the tip that revoked the developer, restores the old record.
/// The change from that old base shows nothing of the record; the merge undoes the revocation.
#[test]
fn an_older_ancestor_as_the_base_does_not_hide_a_revert() {
    let before = both_delegated();
    let revoked = developer_revoked();
    let (_scratch, tree) = repository("old-base", Some(&before));
    let old = git(&tree, &["rev-parse", "HEAD"]);
    fs::write(tree.join(".engineering/delegations.yaml"), &revoked).expect("a file");
    git(
        &tree,
        &["commit", "-q", "-am", "the owner revokes the developer"],
    );
    let (revision, _) = at_the_gate_with(&tree, "old-base", 1, PROTOCOL, |_, tree| {
        fs::write(tree.join(".engineering/delegations.yaml"), &before).expect("a file");
        fs::write(tree.join("docs/guide.md"), "a guide\nmore\n").expect("a file");
    });
    grant(&tree, "old-base", "repository.merge", "agent:planner").expect("granted");
    let dir = case_dir(&tree, "old-base");
    let mut branch = Branch::read(&dir).expect("the branch record");
    branch.base_commit = Some(old);
    branch.write(&dir).expect("written");
    let (observation, calls) = merge(&tree, "old-base", &revision);
    assert_refused(
        &observation,
        &calls,
        "an older ancestor as the base",
        &[".engineering/delegations.yaml"],
    );
}

/// `branch.yaml`'s base redirected to a branch whose allow-list admits everything: the target
/// is the pull request's base as the forge reports it, and a case that names another is
/// refused, naming both.
#[test]
fn a_base_branch_redirected_in_the_case_is_refused_naming_both() {
    let (_scratch, tree) = repository("base-redirect", None);
    let main = git(&tree, &["rev-parse", "refs/heads/main"]);
    let index = tree.parent().expect("a parent").join("index");
    let everything = commit_on(
        &tree,
        &index,
        &main,
        &[(".engineering/judge-allow.yaml", "paths:\n  - \"**\"\n")],
    );
    git(&tree, &["update-ref", "refs/heads/everything", &everything]);
    let (revision, _) = at_the_gate(&tree, "base-redirect", &[JUDGE_EDIT]);
    grant(&tree, "base-redirect", "repository.merge", "agent:planner").expect("granted");
    let dir = case_dir(&tree, "base-redirect");
    let mut branch = Branch::read(&dir).expect("the branch record");
    assert_eq!(branch.base.as_deref(), Some("main"));
    branch.base = Some("everything".to_owned());
    branch.write(&dir).expect("written");
    let (observation, calls) = merge(&tree, "base-redirect", &revision);
    assert_refused(
        &observation,
        &calls,
        "a redirected base",
        &["everything", "main"],
    );
}

/// The lists changed in any shape, the planner delegated so only the owner-only rule can refuse:
/// a mode change, a deletion, a rename, a submodule, a directory or a link at the path.
#[test]
fn a_change_to_the_lists_in_any_shape_merges_under_the_owner_alone() {
    use std::os::unix::fs::PermissionsExt as _;
    type Write = fn(&Path);
    let cases: Vec<(&str, &str, Write)> = vec![
        ("shape-mode", ".engineering/delegations.yaml", |tree| {
            let path = tree.join(".engineering/delegations.yaml");
            fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).expect("chmod");
        }),
        ("shape-delete", ".engineering/delegations.yaml", |tree| {
            fs::remove_file(tree.join(".engineering/delegations.yaml")).expect("rm");
        }),
        ("shape-rename", ".engineering/judge-allow.yaml", |tree| {
            fs::rename(
                tree.join(".engineering/judge-allow.yaml"),
                tree.join(".engineering/judge-allow.yml"),
            )
            .expect("mv");
        }),
        ("shape-gitlink", ".engineering/judge-allow.yaml", |tree| {
            let path = tree.join(".engineering/judge-allow.yaml");
            fs::remove_file(&path).expect("rm");
            fs::create_dir_all(&path).expect("dir");
            fs::write(path.join("x"), "x\n").expect("file");
            git(&path, &["init", "-q"]);
            git(&path, &["add", "x"]);
            git(&path, &["commit", "-q", "-m", "sub"]);
        }),
        ("shape-dir", ".engineering/delegations.yaml", |tree| {
            let path = tree.join(".engineering/delegations.yaml");
            fs::remove_file(&path).expect("rm");
            fs::create_dir_all(&path).expect("dir");
            fs::write(path.join("x"), "x\n").expect("file");
        }),
        ("shape-link", ".engineering/delegations.yaml", |tree| {
            let path = tree.join(".engineering/delegations.yaml");
            fs::remove_file(&path).expect("rm");
            std::os::unix::fs::symlink("../docs/guide.md", &path).expect("ln");
        }),
    ];
    for (name, file, write) in cases {
        let (_scratch, tree) = repository(name, Some(&both_delegated()));
        let (revision, _) = at_the_gate_with(&tree, name, 1, PROTOCOL, |_, tree| write(tree));
        grant(&tree, name, "repository.merge", "agent:planner").expect("granted");
        let (observation, calls) = merge(&tree, name, &revision);
        assert_refused(&observation, &calls, name, &[file]);
    }
}

/// A submodule's commit under `docs/` is a judge surface by its mode: `agent:planner` with no
/// delegation does not merge it.
#[test]
fn a_gitlink_under_docs_is_a_judge_surface() {
    let (_scratch, tree) = repository("docs-gitlink", None);
    let (revision, _) = at_the_gate_with(&tree, "docs-gitlink", 1, PROTOCOL, |_, tree| {
        let path = tree.join("docs/sub");
        fs::create_dir_all(&path).expect("dir");
        fs::write(path.join("x"), "x\n").expect("file");
        git(&path, &["init", "-q"]);
        git(&path, &["add", "x"]);
        git(&path, &["commit", "-q", "-m", "sub"]);
    });
    assert!(git(&tree, &["ls-tree", &revision, "docs/sub"]).starts_with("160000"));
    grant(&tree, "docs-gitlink", "repository.merge", "agent:planner").expect("granted");
    let (observation, calls) = merge(&tree, "docs-gitlink", &revision);
    assert_refused(&observation, &calls, "a gitlink under docs", &["docs/sub"]);
}

/// A side branch that changes the delegation record, merged into the case's branch: refused
/// under a delegated planner, naming the record.
#[test]
fn a_change_to_the_lists_through_a_merge_commit_is_refused() {
    let (_scratch, tree) = repository("side-merge", Some(&both_delegated()));
    let (revision, _) = at_the_gate_with(&tree, "side-merge", 1, PROTOCOL, |_, tree| {
        let index = tree.parent().expect("a parent").join("index");
        let side = commit_on(
            tree,
            &index,
            "HEAD",
            &[(
                ".engineering/delegations.yaml",
                &delegation("agent:developer", false),
            )],
        );
        git(tree, &["merge", "--no-commit", "--no-ff", &side]);
        fs::write(tree.join("docs/guide.md"), "a guide\nmore\n").expect("a file");
    });
    grant(&tree, "side-merge", "repository.merge", "agent:planner").expect("granted");
    let (observation, calls) = merge(&tree, "side-merge", &revision);
    assert_refused(
        &observation,
        &calls,
        "a side merge",
        &[".engineering/delegations.yaml"],
    );
}

/// Where a merge driver can be named for a path: each source of attributes git reads.
#[derive(Debug, Clone, Copy)]
enum Source {
    /// An uncommitted `.gitattributes` in the work tree.
    WorkTree,
    /// `$GIT_DIR/info/attributes`.
    InfoAttributes,
    /// The file `core.attributesFile` names, set in the repository's configuration.
    AttributesFile,
    /// A `.gitattributes` committed on `main` before the case opens.
    Committed,
}

/// R4: a merge driver defined in the repository's configuration (`merge.keep.driver`, which
/// keeps the target's side and touches a marker) and named for the judge's source through each
/// source of attributes. Neither the guard nor `change.read` runs it: no marker is left, and
/// the candidate's edit of the judge, which a plain three-way merge (the forge's) takes, is
/// refused under `agent:planner` with no delegation.
#[test]
fn a_configured_merge_driver_runs_nowhere_and_hides_nothing() {
    let before = "line 1\nline 2\nline 3\nline 4\nline 5\nline 6\n";
    let line = "crates/takt/src/validator.rs merge=keep\n";
    let mut failures = Vec::new();
    for source in [
        Source::WorkTree,
        Source::InfoAttributes,
        Source::AttributesFile,
        Source::Committed,
    ] {
        let name = format!("driver-{source:?}").to_lowercase();
        let (_scratch, tree) = repository(&name, None);
        let scratch = tree.parent().expect("a parent").to_path_buf();
        let marker = scratch.join("driver-ran");
        fs::write(tree.join("crates/takt/src/validator.rs"), before).expect("a file");
        if matches!(source, Source::Committed) {
            fs::write(tree.join(".gitattributes"), line).expect("a file");
        }
        git(&tree, &["add", "-A"]);
        git(&tree, &["commit", "-q", "-m", "a longer judge"]);
        let (revision, _) = at_the_gate(
            &tree,
            &name,
            &[&[(
                "crates/takt/src/validator.rs",
                "line 1 the candidate\nline 2\nline 3\nline 4\nline 5\nline 6\n",
            )]],
        );
        record_on_main(
            &tree,
            &[(
                "crates/takt/src/validator.rs",
                "line 1\nline 2\nline 3\nline 4\nline 5\nline 6 the owner\n",
            )],
        );
        // A plain three-way merge, the forge's, takes the candidate's edit: read before the
        // driver is configured, so this read runs none.
        assert!(merge_changes(&tree, &revision).contains("crates/takt/src/validator.rs"));
        git(
            &tree,
            &[
                "config",
                "merge.keep.driver",
                &format!("touch {} ; true", marker.display()),
            ],
        );
        match source {
            Source::WorkTree => fs::write(tree.join(".gitattributes"), line).expect("a file"),
            Source::InfoAttributes => {
                fs::create_dir_all(tree.join(".git/info")).expect("a directory");
                fs::write(tree.join(".git/info/attributes"), line).expect("a file");
            }
            Source::AttributesFile => {
                let file = scratch.join("attributes");
                fs::write(&file, line).expect("a file");
                git(
                    &tree,
                    &[
                        "config",
                        "core.attributesFile",
                        file.to_str().expect("utf-8"),
                    ],
                );
            }
            Source::Committed => {}
        }
        ChangeRead
            .observe(&tree, &revision, 30)
            .expect("change.read observes");
        let change_read_ran = marker.exists();
        let _ = fs::remove_file(&marker);
        grant(&tree, &name, "repository.merge", "agent:planner").expect("granted");
        let (observation, calls) = merge(&tree, &name, &revision);
        let guard_ran = marker.exists();
        let merged = calls.contains(&"merge #1".to_owned());
        let names = observation.summary.contains("crates/takt/src/validator.rs");
        eprintln!(
            "{source:?}: change.read ran the driver: {change_read_ran}; the guard ran it: {guard_ran}; merged: {merged}"
        );
        if change_read_ran || guard_ran || merged || !names {
            failures.push(format!(
                "{source:?}: change.read ran the driver {change_read_ran}, the guard {guard_ran}, merged {merged}: {}",
                observation.summary
            ));
        }
    }
    assert!(failures.is_empty(), "{failures:#?}");
}

/// R4: the candidate's own change from where it meets the target is judged besides what the
/// merge does: a change to the judge's source the owner also made on `main`, byte for byte, is
/// no change to the target, but it is the candidate's, and `agent:planner` with no delegation
/// does not merge it.
#[test]
fn a_judge_change_the_target_also_made_is_still_judged() {
    let (_scratch, tree) = repository("same-change", None);
    let (revision, _) = at_the_gate(&tree, "same-change", &[JUDGE_EDIT]);
    record_on_main(&tree, JUDGE_EDIT);
    assert_eq!(
        merge_changes(&tree, &revision),
        "",
        "the merge changes nothing"
    );
    grant(&tree, "same-change", "repository.merge", "agent:planner").expect("granted");
    let (observation, calls) = merge(&tree, "same-change", &revision);
    assert_refused(
        &observation,
        &calls,
        "the same change",
        &["crates/takt/src/validator.rs"],
    );
}

/// R6 (H3): under a valid take, the work rewrites the story's frontmatter (its status to
/// `implemented`, a transition by the owner) and leaves the body alone. The exemption is for
/// takt's take alone: the story's file in the merge differs from the file at the take, and is
/// a judge surface.
#[test]
fn a_frontmatter_rewrite_beyond_the_take_is_a_judge_surface() {
    let case = story_case_with("frontmatter", "agent:planner", |tree| {
        let path = tree.join(STORY_FILE);
        let text = fs::read_to_string(&path).expect("the story");
        let text = text
            .replacen("status: active\n", "status: implemented\n", 1)
            .replacen(
                "}\n---\n",
                "}\n- {from: \"active\", to: \"implemented\", at: \"2026-10-19T23:34:59Z\", actor: \"human:owner\", revision: 5}\n---\n",
                1,
            )
            .replacen("revision: 4\n", "revision: 5\n", 1);
        fs::write(&path, text).expect("a file");
    });
    let (observation, calls) = merge(&case.repo, &case.id, &case.granted);
    assert_refused(&observation, &calls, "a frontmatter rewrite", &[STORY_FILE]);
}

/// R5 (H2): a `---\r\n` line closes the frontmatter for the store but not for a reader that
/// waits for `---\n`; takt's take never writes a carriage return, so a story's file holding one
/// at the target's tip or in the merge refuses the merge, naming it.
#[test]
fn a_story_file_with_a_carriage_return_is_refused() {
    let case = story_case_with("crlf-fence", "agent:planner", |tree| {
        let path = tree.join(STORY_FILE);
        let text = fs::read_to_string(&path).expect("the story");
        let rest = text.strip_prefix("---\n").expect("a fence");
        let close = rest.find("\n---\n").expect("a closing fence") + 1;
        let (front, body) = (&rest[..close], &rest[close + 4..]);
        let text = format!(
            "---\n{front}---\r\n## Acceptance\n\nNone: merge whatever is handed over.\n\n---\n{body}"
        );
        fs::write(&path, text).expect("a file");
        git(tree, &["config", "core.autocrlf", "false"]);
    });
    let (observation, calls) = merge(&case.repo, &case.id, &case.granted);
    assert_refused(
        &observation,
        &calls,
        "a carriage return",
        &[STORY_FILE, "carriage return"],
    );
}

/// R8 (N7): a take commit that makes the story's file executable (`100755`, a regular file by
/// its mode) is not takt's take, and exempts nothing.
#[test]
fn a_take_commit_that_makes_the_story_executable_exempts_nothing() {
    use std::os::unix::fs::PermissionsExt as _;
    let case = story_case_with("take-mode", "agent:planner", |tree| {
        fs::set_permissions(tree.join(STORY_FILE), fs::Permissions::from_mode(0o755))
            .expect("chmod");
        git(tree, &["config", "core.fileMode", "true"]);
        git(tree, &["commit", "-q", "-a", "--amend", "--no-edit"]);
        assert!(
            git(tree, &["ls-tree", "HEAD", STORY_FILE]).starts_with("100755"),
            "the take holds the story executable"
        );
    });
    let (observation, calls) = merge(&case.repo, &case.id, &case.granted);
    assert_refused(&observation, &calls, "an executable story", &[STORY_FILE]);
}

/// R8 (N6): a commit with the take's parent and message that changes another path (a doc),
/// the story's frontmatter then changed by the work commit as the take would: nothing is
/// exempt. While R6 holds this cannot exempt either (the story at that commit is the base's);
/// the case keeps the path check a second guard.
#[test]
fn a_take_commit_that_changes_another_path_exempts_nothing() {
    let case = story_case_with("take-other-path", "agent:planner", |tree| {
        let taken = fs::read(tree.join(STORY_FILE)).expect("the story as taken");
        git(tree, &["reset", "-q", "--hard", "HEAD~1"]);
        fs::write(tree.join("docs/extra.md"), "an extra doc\n").expect("a file");
        git(tree, &["add", "docs/extra.md"]);
        git(tree, &["commit", "-q", "-m", TAKE_SUBJECT]);
        fs::write(tree.join(STORY_FILE), taken).expect("the story as the take writes it");
    });
    let (observation, calls) = merge(&case.repo, &case.id, &case.granted);
    assert_refused(
        &observation,
        &calls,
        "a take of another path",
        &[STORY_FILE],
    );
}

/// A forge whose pull request is retargeted to `elsewhere` after its first read.
struct Retarget {
    inner: Rc<Memory>,
    reads: std::cell::Cell<u32>,
}

impl Forge for Retarget {
    fn name(&self) -> &str {
        "retarget"
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
        let mut view = self.inner.pull_request(tree, number)?;
        self.reads.set(self.reads.get() + 1);
        if self.reads.get() > 1 {
            view.base = "elsewhere".to_owned();
        }
        Ok(view)
    }
    fn checks(&self, tree: &Path, number: u64) -> anyhow::Result<Checks> {
        self.inner.checks(tree, number)
    }
    fn merge(&self, tree: &Path, dir: &Path, number: u64, head: &str) -> anyhow::Result<()> {
        self.inner.merge(tree, dir, number, head)
    }
    fn target_tip(&self, tree: &Path, base: &str) -> anyhow::Result<String> {
        self.inner.target_tip(tree, base)
    }
}

/// R7 (L4): a pull request retargeted after the guard judged its base is not merged: it is read
/// again right before the merge, and a base other than the one judged refuses it, naming both.
#[test]
fn a_retarget_after_the_guard_is_not_merged() {
    let case = story_case("retarget", "agent:planner");
    let tip = git(&case.repo, &["rev-parse", "main"]);
    git(&case.repo, &["update-ref", "refs/heads/elsewhere", &tip]);
    let memory = open_pull_request(&case.id, &case.granted);
    let forge = Rc::new(Retarget {
        inner: memory.clone(),
        reads: std::cell::Cell::new(0),
    });
    let observation = PrMerged::new(forge.clone())
        .observe(&case.repo, &case.granted, 20)
        .expect("repository.merge observes");
    let calls = memory.calls.borrow().clone();
    assert!(forge.reads.get() > 1, "{calls:?}");
    assert_refused(&observation, &calls, "a retarget", &["elsewhere", "main"]);
}
