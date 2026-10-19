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
use takt::harness::{Harness, StepContext, StepReport};
use takt::validator::{CiRead, Observation, PrMerged, PrOpen, Validator};
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
    for files in commits {
        for (path, text) in *files {
            let path = tree.join(path);
            fs::create_dir_all(path.parent().expect("a parent")).expect("a directory");
            fs::write(path, text).expect("a file");
        }
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
/// `agent:planner` with no delegation.
#[test]
fn a_base_commit_that_is_not_an_ancestor_of_the_target_refuses_the_merge_naming_the_base() {
    let (_scratch, tree) = repository("moved-base", None);
    let widened = format!("{ALLOW}  - crates/**\n");
    let (revision, made) = at_the_gate(
        &tree,
        "moved-base",
        &[
            &[(".engineering/judge-allow.yaml", widened.as_str())],
            JUDGE_EDIT,
        ],
    );
    grant(&tree, "moved-base", "repository.merge", "agent:planner").expect("granted");
    let dir = case_dir(&tree, "moved-base");
    let mut branch = Branch::read(&dir).expect("the branch record");
    branch.base_commit = Some(made[0].clone());
    branch.write(&dir).expect("written");

    let (observation, calls) = merge(&tree, "moved-base", &revision);
    assert_refused(&observation, &calls, "moved base", &["base", &made[0][..8]]);
}

/// The added acceptance's second half: a head that carries only takt's approval riders above
/// the granted revision merges as before (here under the owner's grant).
#[test]
fn a_head_with_only_approval_records_above_the_grant_merges_as_before() {
    let (_scratch, tree) = repository("riders", None);
    let (revision, _) = at_the_gate(&tree, "riders", &[JUDGE_EDIT]);
    grant(&tree, "riders", "repository.merge", "human:owner").expect("granted");
    let (observation, calls) = merge(&tree, "riders", &revision);
    assert_merged(&observation, &calls, "the revision itself");
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
