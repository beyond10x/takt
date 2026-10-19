//! Adversary pass 1 over story:approval-record-reaches-base: what `repository.merge` merges
//! beyond the granted revision, and what a real forge does around the push of the approval's
//! commit that the double does not.

use std::cell::Cell;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::rc::Rc;

use anyhow::{anyhow, bail};
use takt::case::{Branch, CASES_DIR};
use takt::evidence::Record;
use takt::forge::{Checks, Forge, Memory, PullRequest, PullRequestView};
use takt::frontier::{Aep, approve};
use takt::harness::{Harness, StepContext, StepReport};
use takt::validator::{CiRead, PrMerged, PrOpen, Validator};
use takt::{Run, RunOptions, Stop};

const PROTOCOL: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../protocols/story-delivery/1.yaml"
);

const FIXTURES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures");

/// The story's own file in the fixture store.
const STORY: &str = ".engineering/planning/story/ready-one.md";

/// The acceptance a forged approve-subject commit writes into the story.
const FORGED_ACCEPTANCE: &str = "Nothing; it is accepted as is.";

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

fn repository(name: &str) -> Scratch {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!(
        "approval-record-adv1-{}-{name}",
        std::process::id()
    ));
    if dir.exists() {
        fs::remove_dir_all(&dir).expect("a stale scratch tree is removed");
    }
    copy_dir(&Path::new(FIXTURES).join("one-ready"), &dir);
    fs::write(dir.join("check.txt"), "broken\n").expect("a file");
    git(&dir, &["init", "-q", "-b", "main"]);
    git(&dir, &["add", "-A"]);
    git(&dir, &["commit", "-q", "-m", "start"]);
    Scratch(dir)
}

struct Requester;

impl Harness for Requester {
    fn name(&self) -> &str {
        "requester"
    }

    fn step(&self, context: &StepContext<'_>) -> Result<StepReport, anyhow::Error> {
        if context.approvals.contains(&"repository.merge".to_owned()) {
            return Ok(StepReport {
                action: "repository.merge".to_owned(),
                summary: "merge it".to_owned(),
                tokens: None,
            });
        }
        Err(anyhow!("the gate was not offered"))
    }
}

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

fn options() -> RunOptions {
    RunOptions {
        protocol: PROTOCOL.to_owned(),
        intent: String::new(),
        inputs: Vec::new(),
        iterations: Some(8),
        hours: None,
        tokens: None,
        case_id: None,
        gate: Some("repository.merge".to_owned()),
    }
}

fn validators(forge: Rc<dyn Forge>) -> Vec<Box<dyn Validator>> {
    vec![
        Box::new(PrOpen::new(forge.clone())),
        Box::new(CiRead::new(forge.clone())),
        Box::new(PrMerged::new(forge)),
    ]
}

fn record(n: usize, kind: &str, revision: &str) -> Record {
    Record::new(
        format!("e{n}"),
        kind,
        Some("pass".to_owned()),
        "implementation",
        revision,
        "approval-record-adv1",
    )
}

fn double() -> Rc<Memory> {
    Rc::new(Memory {
        checks: Checks::Pass,
        ..Memory::default()
    })
}

/// The case at the merge gate through the double: its id and the revision a grant authorises.
fn at_the_gate(repo: &Path, memory: &Rc<Memory>) -> (String, String) {
    let forge: Rc<dyn Forge> = memory.clone();
    let validators = validators(forge);
    let mut out = Vec::new();
    let opened = Run::open_next(
        repo,
        options(),
        &Aep::default(),
        &Requester,
        &validators,
        &mut out,
    )
    .expect("the frontier answers")
    .expect("story:ready-one is taken");
    let id = opened.case.id.clone();
    drop(opened);
    fs::write(repo.join("check.txt"), "fixed\n").expect("a file");
    git(repo, &["commit", "-q", "-am", "the work"]);
    let worked = git(repo, &["rev-parse", "HEAD"]);
    let mut run =
        Run::resume(repo, options(), &id, &Requester, &validators).expect("the case resumes");
    run.case
        .add_evidence(&record(1, "test_result", &worked))
        .expect("written");
    run.case
        .add_evidence(&record(2, "gate_result", &worked))
        .expect("written");
    run.case
        .add_evidence(&takt::evidence::Record::new(
            format!("handover-{}", &worked[..8]),
            "handover",
            None,
            "implementation",
            &worked,
            takt::run::CHOICE_PRODUCER,
        ))
        .expect("written");
    // A change to publish at the revision, as `change.read` records it.
    run.case
        .add_evidence(&takt::evidence::Record::new(
            format!("change-record-{}", &worked[..8]),
            "change_record",
            Some("ahead".to_owned()),
            "implementation",
            &worked,
            "takt.change-read",
        ))
        .expect("written");
    let mut out = Vec::new();
    let stop = run.run(&mut out).expect("the run stops without an error");
    assert_eq!(
        stop,
        Stop::ApprovalRequired(vec!["repository.merge".to_owned()]),
        "{}",
        String::from_utf8_lossy(&out)
    );
    (id, worked)
}

fn base_after_merging(repo: &Path, head: &str) -> String {
    git(repo, &["merge-tree", "--write-tree", "main", head])
        .lines()
        .next()
        .expect("a tree")
        .to_owned()
}

/// Runs the resumed case once through `forge`: the stop, or the error, and what it printed.
fn merge_run(repo: &Path, id: &str, forge: Rc<dyn Forge>) -> (Result<Stop, String>, String) {
    merge_run_with(repo, id, forge, &NoHarness)
}

fn merge_run_with(
    repo: &Path,
    id: &str,
    forge: Rc<dyn Forge>,
    harness: &dyn Harness,
) -> (Result<Stop, String>, String) {
    let validators = validators(forge);
    let mut run = Run::resume(repo, options(), id, harness, &validators).expect("the case resumes");
    // A run that waits on the forge pauses between its observations; here only briefly.
    run.poll = std::time::Duration::from_millis(5);
    let mut out = Vec::new();
    let stop = run.run(&mut out).map_err(|error| format!("{error:#}"));
    (stop, String::from_utf8_lossy(&out).into_owned())
}

/// The double, with what a real forge does around a push that the double does not: a push the
/// forge applied and then reported as failed (`fail_next_push`), and required checks on the
/// head a merge names (`require_checks`): checks are passing only on heads in `green`, pending
/// on any other, and a merge of a head whose checks are not passing is refused the way the
/// API refuses it.
struct Real {
    memory: Rc<Memory>,
    fail_next_push: Cell<bool>,
    require_checks: bool,
    green: std::cell::RefCell<Vec<String>>,
}

impl Real {
    fn head_of(&self, number: u64) -> anyhow::Result<String> {
        Ok(self.memory.pull_request(Path::new("."), number)?.head.sha)
    }
}

impl Forge for Real {
    /// Scans nothing: the double publishes nothing off the machine.
    fn scan_outgoing(
        &self,
        _: &std::path::Path,
        _: &std::path::Path,
        _: &takt::forge::Outgoing<'_>,
    ) -> anyhow::Result<()> {
        Ok(())
    }

    fn name(&self) -> &str {
        "real"
    }

    fn push(&self, tree: &Path, branch: &str) -> anyhow::Result<()> {
        self.memory.push(tree, branch)?;
        if self.fail_next_push.replace(false) {
            bail!("b10x-gates bot -- push exited on a signal: the child outlived its time limit");
        }
        Ok(())
    }

    fn push_revision(&self, tree: &Path, branch: &str, revision: &str) -> anyhow::Result<()> {
        self.memory.push_revision(tree, branch, revision)?;
        if self.fail_next_push.replace(false) {
            bail!("b10x-gates bot -- push exited on a signal: the child outlived its time limit");
        }
        Ok(())
    }

    fn open_pull_request(
        &self,
        tree: &Path,
        dir: &Path,
        request: &PullRequest,
    ) -> anyhow::Result<u64> {
        self.memory.open_pull_request(tree, dir, request)
    }

    fn pull_request(&self, tree: &Path, number: u64) -> anyhow::Result<PullRequestView> {
        self.memory.pull_request(tree, number)
    }

    fn checks(&self, tree: &Path, number: u64) -> anyhow::Result<Checks> {
        let answer = self.memory.checks(tree, number)?;
        if !self.require_checks {
            return Ok(answer);
        }
        let head = self.head_of(number)?;
        Ok(if self.green.borrow().contains(&head) {
            answer
        } else {
            Checks::Pending
        })
    }

    fn merge(&self, tree: &Path, dir: &Path, number: u64, head: &str) -> anyhow::Result<()> {
        if self.require_checks && !self.green.borrow().iter().any(|green| green == head) {
            bail!(
                "b10x-gates api --method PUT --path /repos/o/r/pulls/{number}/merge exited 1: 405 Required status check \"takt / task check\" is expected"
            );
        }
        self.memory.merge(tree, dir, number, head)
    }
}

/// Judge surface. `approvals_only` admits any commit with the approve subject that changes only
/// planning-store paths, whatever it changes there. Before this unit such a commit stayed on the
/// case's branch (the revision was pushed and merged); now `repository.merge` pushes `HEAD` over
/// it and merges it. Here a commit with the approve subject, made after `takt approve`, rewrites
/// the story's own acceptance: the base the merge leaves must not hold that rewrite, which no
/// approval recorded and nobody granted.
#[test]
fn adv1_an_approve_subject_commit_rewriting_the_story_does_not_reach_the_base() {
    let repo = repository("forged");
    let memory = double();
    let (id, granted) = at_the_gate(&repo, &memory);
    let branch = format!("takt/{id}");
    approve(
        &repo,
        &Aep::default(),
        &id,
        "repository.merge",
        "human:operator",
    )
    .expect("the approval is given");
    let approved = git(&repo, &["rev-parse", "HEAD"]);
    let original = git(&repo, &["show", &format!("HEAD:{STORY}")]);

    let rewritten = original.replace("`./check.sh` exits 0.", FORGED_ACCEPTANCE);
    assert_ne!(rewritten, original);
    fs::write(repo.join(STORY), format!("{rewritten}\n")).expect("a file");
    git(
        &repo,
        &[
            "commit",
            "-q",
            "-am",
            "takt: approve story:ready-one repository.merge",
        ],
    );
    let forged = git(&repo, &["rev-parse", "HEAD"]);
    assert_eq!(git(&repo, &["rev-parse", "HEAD^"]), approved);

    let forge: Rc<dyn Forge> = memory.clone();
    let (stop, text) = merge_run(&repo, &id, forge);
    if memory.merged.borrow().is_empty() {
        return;
    }
    let head = memory
        .pushed
        .borrow()
        .get(&branch)
        .cloned()
        .expect("pushed");
    let base = base_after_merging(&repo, &head);
    let on_base = git(&repo, &["show", &format!("{base}:{STORY}")]);
    assert!(
        !on_base.contains(FORGED_ACCEPTANCE) && on_base.contains("`./check.sh` exits 0."),
        "merged head {} (forged {}, approval {}, granted {}) put a rewritten acceptance on the base: {stop:?}\n{text}",
        &head[..8],
        &forged[..8],
        &approved[..8],
        &granted[..8]
    );
}

/// Retry after a partial write. `repository.merge` pushes the approval's commit and records the
/// head pushed only once the push returned. A push the forge applied and then reported failed
/// (the bot route's child killed at its time limit after the remote updated) leaves the forge's
/// head at the approval's commit and `branch.yaml` at the revision; every later run then finds
/// the forge's head differing from the head pushed and merges nothing, and nothing in the run
/// pushes again. The case can never be merged.
#[test]
fn adv1_a_push_the_forge_applied_but_reported_failed_leaves_the_case_mergeable() {
    let repo = repository("partial-push");
    let memory = double();
    let (id, _) = at_the_gate(&repo, &memory);
    approve(
        &repo,
        &Aep::default(),
        &id,
        "repository.merge",
        "human:operator",
    )
    .expect("the approval is given");
    let approved = git(&repo, &["rev-parse", "HEAD"]);
    let real = Rc::new(Real {
        memory: memory.clone(),
        fail_next_push: Cell::new(true),
        require_checks: false,
        green: std::cell::RefCell::new(Vec::new()),
    });

    let (first, first_text) = merge_run(&repo, &id, real.clone());
    assert!(
        first.is_err(),
        "the push reported failure: {first:?}\n{first_text}"
    );
    assert!(memory.merged.borrow().is_empty());

    let mut runs = Vec::new();
    for _ in 0..2 {
        if !memory.merged.borrow().is_empty() {
            break;
        }
        runs.push(merge_run(&repo, &id, real.clone()));
    }
    let pushed = Branch::read(&repo.join(CASES_DIR).join(&id))
        .expect("reads")
        .pushed;
    assert_eq!(
        memory.merged.borrow().as_slice(),
        [1],
        "after the forge took {} and branch.yaml kept {:?}, {} more runs merged nothing: {runs:#?}",
        &approved[..8],
        pushed,
        runs.len()
    );
}

/// The stated residue. A forge that requires checks on the pull request's head holds a merge of
/// the approval's commit that `repository.merge` pushes right before merging: CI was read on the
/// granted revision, never on the head merged. The run must not end in an error there; it waits
/// with nothing merged, and once the checks on that head pass, a later run merges it.
#[test]
fn adv1_required_checks_on_the_pushed_approval_commit_hold_the_merge_without_an_error() {
    let repo = repository("required-checks");
    let memory = double();
    let (id, granted) = at_the_gate(&repo, &memory);
    approve(
        &repo,
        &Aep::default(),
        &id,
        "repository.merge",
        "human:operator",
    )
    .expect("the approval is given");
    let approved = git(&repo, &["rev-parse", "HEAD"]);
    let real = Rc::new(Real {
        memory: memory.clone(),
        fail_next_push: Cell::new(false),
        require_checks: true,
        green: std::cell::RefCell::new(vec![granted.clone()]),
    });

    // The run waits on the forge with no judgement step offered (story:ci-pending-waits): with
    // `NoHarness` an offer would end the run in a harness error. Its step budget ends while the
    // checks are still pending, and it stops `Waiting` on the merge.
    let (first, first_text) = merge_run(&repo, &id, real.clone());
    assert!(memory.merged.borrow().is_empty(), "{first_text}");
    assert!(
        first.is_ok(),
        "checks pending on {} (the approval's commit pushed by repository.merge; CI was read on {}) ended the run in an error: {first:?}\n{first_text}",
        &approved[..8],
        &granted[..8]
    );
    assert_eq!(
        first,
        Ok(Stop::Waiting {
            on: vec!["repository.merge".to_owned()],
            budget: Some(Box::new(Stop::Iterations(8))),
        }),
        "{first_text}"
    );

    real.green.borrow_mut().push(approved.clone());
    let (second, second_text) = merge_run(&repo, &id, real.clone());
    assert_eq!(
        memory.merged.borrow().as_slice(),
        [1],
        "{second:?}\n{second_text}"
    );
}

/// Kills the mutant `carries` -> `Ok(true)`, which `approval_record.rs`'s refusal case does not:
/// there the stored head is the revision and the forge's head the foreign commit, so the
/// equality of the two refuses before `carries` is asked. Here both name the foreign commit,
/// and only `carries` stands between it and the merge.
#[test]
fn adv1_a_stored_and_reported_head_with_a_foreign_commit_is_refused_by_carries() {
    let repo = repository("carries");
    let memory = double();
    let (id, _) = at_the_gate(&repo, &memory);
    let branch = format!("takt/{id}");
    let dir = repo.join(CASES_DIR).join(&id);
    approve(
        &repo,
        &Aep::default(),
        &id,
        "repository.merge",
        "human:operator",
    )
    .expect("the approval is given");

    git(&repo, &["checkout", "-q", "-b", "elsewhere"]);
    fs::write(repo.join("check.txt"), "changed after the grant\n").expect("a file");
    git(&repo, &["commit", "-q", "-am", "after the grant"]);
    let foreign = git(&repo, &["rev-parse", "HEAD"]);
    git(&repo, &["checkout", "-q", &branch]);
    memory
        .pushed
        .borrow_mut()
        .insert(branch.clone(), foreign.clone());
    let mut stored = Branch::read(&dir).expect("reads");
    stored.pushed = Some(foreign.clone());
    stored.write(&dir).expect("writes");

    let forge: Rc<dyn Forge> = memory.clone();
    let (stop, text) = merge_run(&repo, &id, forge);
    assert!(
        memory.merged.borrow().is_empty(),
        "{} merged: {stop:?}\n{text}",
        &foreign[..8]
    );
}
