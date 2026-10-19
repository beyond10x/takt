//! The authority over a case, hardened: one holder at a time, taken atomically; a grant bound to
//! the implementation revision the case reached the gate at, enforced by takt both in the
//! evaluation that admits `repository.merge` and in the merge itself; a grant only for a case at
//! the gate; `approve` records on the story before it writes the grant, so a failed record
//! leaves nothing to undo; case ids that name no single directory under the cases directory
//! refused wherever an operator gives one; evidence that does not read stops the run naming the
//! file; the latest decision is the one with the highest sequence number, however many digits.

use std::cell::{Cell, RefCell};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::rc::Rc;
use std::sync::{Arc, Barrier};

use anyhow::anyhow;
use takt::case::{
    AUTHORITY_FILE, Approval, Authority, CASES_DIR, CaseDir, LOCK_FILE, check_id, grant,
    latest_decision,
};
use takt::evidence::Record;
use takt::forge::{Forge, Memory, PullRequest, PullRequestState};
use takt::frontier::{Frontier, Story, Take, approve};
use takt::harness::{Harness, StepContext, StepReport};
use takt::validator::{PrMerged, Validator};
use takt::{Decision, LoadedProtocol, Run, RunOptions};

/// The protocol under test, relative to this crate.
const PROTOCOL: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../protocols/story-delivery/1.yaml"
);

/// Runs `git <args>` in `tree`; its trimmed standard output.
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

/// A scratch git tree under this crate's build directory, removed when dropped.
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

/// A scratch tree with one commit on `main`.
fn scratch(name: &str) -> Scratch {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("authority-{}-{name}", std::process::id()));
    if dir.exists() {
        fs::remove_dir_all(&dir).expect("a stale scratch tree is removed");
    }
    fs::create_dir_all(&dir).expect("scratch dir");
    fs::write(dir.join("README.md"), "a story\n").expect("a file");
    git(&dir, &["init", "-q", "-b", "main"]);
    git(&dir, &["add", "-A"]);
    git(&dir, &["commit", "-q", "-m", "start"]);
    Scratch(dir)
}

fn protocol() -> LoadedProtocol {
    LoadedProtocol::from_file(Path::new(PROTOCOL)).expect("the protocol loads")
}

fn record(n: usize, kind: &str, result: &str, revision: &str) -> Record {
    Record::new(
        format!("e{n}"),
        kind,
        Some(result.to_owned()),
        "implementation",
        revision,
        "authority-test",
    )
}

/// One commit on the case's branch, the case's `implementation` moved to it; the revision.
fn commit(tree: &Path, case: &mut CaseDir, file: &str) -> String {
    fs::write(tree.join(file), "done\n").expect("a file");
    git(tree, &["add", file]);
    git(tree, &["commit", "-q", "-m", file]);
    let revision = git(tree, &["rev-parse", "HEAD"]);
    case.set_revision("implementation", &revision)
        .expect("the revision moves");
    revision
}

fn action<'a>(decision: &'a Decision, id: &str) -> &'a str {
    decision
        .actions
        .get(id)
        .map(String::as_str)
        .unwrap_or("absent")
}

/// Brings `case` to the gate at its current revision: the four records the delivery owes up to
/// the merge, then one evaluation, which finds `repository.merge` approval-required.
fn to_the_gate(case: &mut CaseDir, first: usize) -> Decision {
    let revision = case
        .revision("implementation")
        .expect("implementation has a revision");
    for (n, (kind, result)) in [
        ("test_result", "pass"),
        ("gate_result", "pass"),
        ("pull_request", "open"),
        ("ci_result", "pass"),
    ]
    .iter()
    .enumerate()
    {
        case.add_evidence(&record(first + n, kind, result, &revision))
            .expect("the record is written");
    }
    let seq = case.evaluations().expect("counts") + 1;
    let decision = case.evaluate(seq).expect("evaluates");
    assert_eq!(
        action(&decision, "repository.merge"),
        "approval-required",
        "{}",
        decision.json
    );
    decision
}

/// A case `id` opened under `tree`, one commit on its branch, at the gate there; its revision.
fn at_the_gate(tree: &Path, id: &str) -> (CaseDir, String) {
    let mut case =
        CaseDir::open(tree, protocol(), id, "deliver the story", &[]).expect("the case opens");
    let revision = commit(tree, &mut case, "implemented.txt");
    to_the_gate(&mut case, 1);
    (case, revision)
}

fn evaluate(case: &CaseDir) -> Decision {
    let seq = case.evaluations().expect("counts") + 1;
    case.evaluate(seq).expect("evaluates")
}

// ---------------------------------------------------------------------------------------------
// One holder at a time.
// ---------------------------------------------------------------------------------------------

/// Two threads holding one case at once: exactly one succeeds, in every one of 300 rounds.
#[test]
fn two_threads_holding_one_case_at_once_exactly_one_succeeds_in_300_of_300_rounds() {
    let tree = scratch("hold");
    let case =
        CaseDir::open(&tree, protocol(), "hold", "deliver the story", &[]).expect("the case opens");
    let lock = case.dir.join(LOCK_FILE);
    let mut exactly_one = 0;
    for round in 0..300 {
        let a = CaseDir::resume(&tree, "hold").expect("the case resumes");
        let b = CaseDir::resume(&tree, "hold").expect("the case resumes");
        let barrier = Arc::new(Barrier::new(2));
        let (ba, bb) = (barrier.clone(), barrier);
        let ta = std::thread::spawn(move || {
            ba.wait();
            a.hold().is_ok()
        });
        let tb = std::thread::spawn(move || {
            bb.wait();
            b.hold().is_ok()
        });
        let held = [ta.join().expect("joins"), tb.join().expect("joins")];
        if held.iter().filter(|held| **held).count() == 1 {
            exactly_one += 1;
        }
        assert!(lock.is_file(), "round {round}: nobody holds the case");
        fs::remove_file(&lock).expect("the lock goes");
    }
    assert_eq!(exactly_one, 300);
}

/// A grant written while a run holds the case is refused, naming the lock, with nothing
/// written; raced against a hold 300 times, a grant that went through always finished before
/// the hold was taken.
#[test]
fn a_grant_written_while_a_run_holds_the_case_is_refused() {
    let tree = scratch("grant-hold");
    let (case, _) = at_the_gate(&tree, "grant-hold");
    let dir = case.dir.clone();
    case.hold().expect("the run holds the case");
    let refusal = format!(
        "{:#}",
        grant(&tree, "grant-hold", "repository.merge", "human:operator").expect_err("held")
    );
    assert!(refusal.contains(LOCK_FILE), "{refusal}");
    assert!(!dir.join(AUTHORITY_FILE).exists());
    case.release().expect("lets go");

    let mut after_the_hold = 0;
    for _ in 0..300 {
        let _ = fs::remove_file(dir.join(AUTHORITY_FILE));
        let holder = CaseDir::resume(&tree, "grant-hold").expect("the case resumes");
        let t = tree.0.clone();
        let barrier = Arc::new(Barrier::new(2));
        let (bh, bg) = (barrier.clone(), barrier);
        let th = std::thread::spawn(move || {
            bh.wait();
            holder.hold().is_ok()
        });
        let tg = std::thread::spawn(move || {
            bg.wait();
            grant(&t, "grant-hold", "repository.merge", "human:operator").is_ok()
        });
        let (held, granted) = (th.join().expect("joins"), tg.join().expect("joins"));
        assert!(
            held || granted,
            "neither the hold nor the grant went through"
        );
        if held && granted {
            let locked = fs::metadata(dir.join(LOCK_FILE))
                .and_then(|meta| meta.modified())
                .expect("the lock");
            let written = fs::metadata(dir.join(AUTHORITY_FILE))
                .and_then(|meta| meta.modified())
                .expect("the grant");
            if written > locked {
                after_the_hold += 1;
            }
        }
        let _ = fs::remove_file(dir.join(LOCK_FILE));
    }
    assert_eq!(after_the_hold, 0, "a grant was written while the run held");
}

/// `resume` takes the lock before it writes anything: a case another holder holds is refused
/// naming the lock, and the tree is left on the branch it was on.
#[test]
fn resume_writes_nothing_while_the_case_is_held() {
    let tree = scratch("resume-held");
    let case = CaseDir::open(&tree, protocol(), "resume-held", "deliver the story", &[])
        .expect("the case opens");
    git(&tree, &["checkout", "-q", "main"]);
    fs::write(case.dir.join(LOCK_FILE), "2026-01-01T00:00:00Z\n").expect("a lock");
    let snapshot = fs::read_to_string(case.dir.join("case.yaml")).expect("the snapshot");
    let refusal = format!(
        "{:#}",
        CaseDir::resume(&tree, "resume-held").expect_err("held")
    );
    assert!(refusal.contains(LOCK_FILE), "{refusal}");
    assert_eq!(git(&tree, &["rev-parse", "--abbrev-ref", "HEAD"]), "main");
    assert_eq!(
        fs::read_to_string(case.dir.join("case.yaml")).expect("the snapshot"),
        snapshot
    );
    assert_eq!(
        fs::read_to_string(case.dir.join(LOCK_FILE)).expect("the lock"),
        "2026-01-01T00:00:00Z\n",
        "the other holder's lock was touched"
    );
}

// ---------------------------------------------------------------------------------------------
// A grant authorises one revision.
// ---------------------------------------------------------------------------------------------

/// A grant records the implementation revision it was given at; the merge is admitted at that
/// revision and at no other: a commit after the grant needs a new grant, which replaces the
/// one for the old revision.
#[test]
fn a_grant_records_its_revision_and_the_merge_is_admitted_at_no_other() {
    let tree = scratch("bound");
    let (mut case, first) = at_the_gate(&tree, "bound");
    let approval =
        grant(&tree, "bound", "repository.merge", "human:operator").expect("the grant is written");
    assert_eq!(approval.revision, first);
    let authority = Authority::read(&case.dir)
        .expect("reads")
        .expect("the file is there");
    assert_eq!(authority.decisions.len(), 1, "{authority:?}");
    assert_eq!(
        authority.decisions[0].revision.as_deref(),
        Some(first.as_str())
    );
    let decision = evaluate(&case);
    assert_eq!(
        action(&decision, "repository.merge"),
        "admissible",
        "{}",
        decision.json
    );

    // A commit after the grant: the claims re-established at the new revision, the merge is
    // not admitted under the grant given at the old one.
    let second = commit(&tree, &mut case, "after-the-grant.txt");
    let decision = to_the_gate(&mut case, 5);
    assert_ne!(
        action(&decision, "repository.merge"),
        "admissible",
        "{}",
        decision.json
    );

    // A new grant, at the new revision, replaces the old one.
    let approval = grant(&tree, "bound", "repository.merge", "human:operator")
        .expect("the new grant is written");
    assert_eq!(approval.revision, second);
    let authority = Authority::read(&case.dir)
        .expect("reads")
        .expect("the file is there");
    assert_eq!(authority.decisions.len(), 1, "{authority:?}");
    assert_eq!(
        authority.decisions[0].revision.as_deref(),
        Some(second.as_str())
    );
    let decision = evaluate(&case);
    assert_eq!(
        action(&decision, "repository.merge"),
        "admissible",
        "{}",
        decision.json
    );
}

/// An `authority.yaml` that grants the merge without a revision (one written before grants were
/// bound) admits nothing.
#[test]
fn a_grant_with_no_revision_admits_nothing() {
    let tree = scratch("unbound");
    let (case, _) = at_the_gate(&tree, "unbound");
    fs::write(
        case.dir.join(AUTHORITY_FILE),
        "- capability: repository.merge\n  decision: granted\n  approver: operator\n  granted_at: 2026-01-01T00:00:00Z\n",
    )
    .expect("an unbound grant");
    let decision = evaluate(&case);
    assert_eq!(
        action(&decision, "repository.merge"),
        "approval-required",
        "{}",
        decision.json
    );
}

/// The merge itself checks the grant: `repository.merge` asked to merge an open pull request at
/// a revision the case's grant was not given at merges nothing and records nothing.
#[test]
fn pr_merged_merges_nothing_under_a_grant_for_another_revision() {
    let tree = scratch("pr-merged");
    let (mut case, first) = at_the_gate(&tree, "pr-merged");
    grant(&tree, "pr-merged", "repository.merge", "human:operator").expect("the grant is written");
    let second = commit(&tree, &mut case, "after-the-grant.txt");
    let mut branch = takt::case::Branch::read(&case.dir).expect("the branch file");
    branch.pushed = Some(second.clone());
    branch.pull_request = Some(1);
    branch.write(&case.dir).expect("the branch file is written");
    let memory = Rc::new(Memory {
        opened: RefCell::new(vec![PullRequest {
            branch: "takt/pr-merged".to_owned(),
            base: "main".to_owned(),
            title: String::new(),
            body: String::new(),
        }]),
        pushed: RefCell::new(BTreeMap::from([(
            "takt/pr-merged".to_owned(),
            second.clone(),
        )])),
        ..Memory::default()
    });
    let forge: Rc<dyn Forge> = memory.clone();
    let observation = PrMerged::new(forge)
        .observe(&tree, &second, 9)
        .expect("observes");
    assert!(observation.records.is_empty(), "{}", observation.summary);
    assert!(memory.merged.borrow().is_empty(), "{}", observation.summary);
    assert!(
        observation.summary.contains(&first[..7]),
        "the summary names the revision the grant is for: {}",
        observation.summary
    );
    assert_eq!(
        memory.pull_request(&tree, 1).expect("reports").state,
        PullRequestState::Open
    );
}

/// `grant` refuses a case that is not at the gate, with nothing written: a case never
/// evaluated, a case whose latest decision does not find the merge approval-required, and a
/// case whose revision moved since the decision that found it so.
#[test]
fn grant_refuses_a_case_that_is_not_at_the_gate() {
    let tree = scratch("gate");
    let mut case =
        CaseDir::open(&tree, protocol(), "gate", "deliver the story", &[]).expect("the case opens");
    let refusal = format!(
        "{:#}",
        grant(&tree, "gate", "repository.merge", "human:operator").expect_err("never evaluated")
    );
    assert!(refusal.contains("gate"), "{refusal}");
    assert!(!case.dir.join(AUTHORITY_FILE).exists());

    commit(&tree, &mut case, "implemented.txt");
    let decision = evaluate(&case);
    assert_eq!(action(&decision, "repository.merge"), "blocked");
    let refusal = format!(
        "{:#}",
        grant(&tree, "gate", "repository.merge", "human:operator").expect_err("merge blocked")
    );
    assert!(refusal.contains("repository.merge"), "{refusal}");
    assert!(!case.dir.join(AUTHORITY_FILE).exists());

    to_the_gate(&mut case, 1);
    commit(&tree, &mut case, "moved.txt");
    let refusal = format!(
        "{:#}",
        grant(&tree, "gate", "repository.merge", "human:operator")
            .expect_err("moved since the gate")
    );
    assert!(refusal.contains("gate"), "{refusal}");
    assert!(!case.dir.join(AUTHORITY_FILE).exists());
    assert!(!case.dir.join(LOCK_FILE).exists(), "the grant let go");
}

// ---------------------------------------------------------------------------------------------
// `approve` records first.
// ---------------------------------------------------------------------------------------------

/// A frontier whose record fails the first time it is asked, and notes whether the grant was
/// already written when the record was asked for.
struct FailsOnce {
    case_dir: PathBuf,
    asked: Cell<usize>,
    written_before_record: Cell<bool>,
    recorded: RefCell<Vec<Approval>>,
}

impl Frontier for FailsOnce {
    fn name(&self) -> &str {
        "fails-once"
    }

    fn candidates(&self, _: &Path) -> anyhow::Result<Vec<String>> {
        Ok(Vec::new())
    }

    fn active(&self, _: &Path) -> anyhow::Result<Vec<String>> {
        Ok(Vec::new())
    }

    fn show(&self, _: &Path, id: &str) -> anyhow::Result<Story> {
        panic!("{id} was shown")
    }

    fn take(&self, _: &Path, id: &str) -> anyhow::Result<Take> {
        panic!("{id} was taken")
    }

    fn record_approval(&self, _: &Path, _: &str, approval: &Approval) -> anyhow::Result<()> {
        if self.case_dir.join(AUTHORITY_FILE).exists() {
            self.written_before_record.set(true);
        }
        let asked = self.asked.get() + 1;
        self.asked.set(asked);
        if asked == 1 {
            return Err(anyhow!("the store is unreachable"));
        }
        self.recorded.borrow_mut().push(approval.clone());
        Ok(())
    }
}

/// `approve` records the approval on the story before it writes the grant: a record that fails
/// leaves no grant and no lock, and the same approval given again goes through, recorded once.
#[test]
fn approve_records_on_the_story_first_and_retries_cleanly_after_a_failed_record() {
    let tree = scratch("approve");
    let (case, revision) = at_the_gate(&tree, "approve");
    case.set_story("story:approve")
        .expect("the story is recorded");
    let frontier = FailsOnce {
        case_dir: case.dir.clone(),
        asked: Cell::new(0),
        written_before_record: Cell::new(false),
        recorded: RefCell::new(Vec::new()),
    };
    let error = format!(
        "{:#}",
        approve(
            &tree,
            &frontier,
            "approve",
            "repository.merge",
            "human:operator"
        )
        .expect_err("the record failed")
    );
    assert!(error.contains("the store is unreachable"), "{error}");
    assert!(!case.dir.join(AUTHORITY_FILE).exists(), "{error}");
    assert!(!case.dir.join(LOCK_FILE).exists(), "{error}");

    let approval = approve(
        &tree,
        &frontier,
        "approve",
        "repository.merge",
        "human:operator",
    )
    .expect("the retry goes through");
    assert_eq!(approval.revision, revision);
    assert!(!frontier.written_before_record.get());
    assert_eq!(frontier.recorded.borrow().as_slice(), [approval]);
    assert!(case.dir.join(AUTHORITY_FILE).is_file());
    assert!(!case.dir.join(LOCK_FILE).exists());
}

// ---------------------------------------------------------------------------------------------
// Case ids.
// ---------------------------------------------------------------------------------------------

/// A harness no run below reaches.
struct NoHarness;

impl Harness for NoHarness {
    fn name(&self) -> &str {
        "none"
    }

    fn step(&self, _: &StepContext<'_>) -> Result<StepReport, anyhow::Error> {
        Err(anyhow!("no harness"))
    }
}

fn options(id: &str) -> RunOptions {
    RunOptions {
        protocol: PROTOCOL.to_owned(),
        intent: "deliver the story".to_owned(),
        inputs: Vec::new(),
        iterations: Some(0),
        hours: None,
        tokens: None,
        case_id: Some(id.to_owned()),
        gate: Some("repository.merge".to_owned()),
    }
}

/// Case ids that are not one name under the cases directory (a `/`, a `..`, a leading `/`) are
/// refused by `approve`, by `run --case` (`Run::resume`) and by `RunOptions.case_id`
/// (`Run::open`), with nothing written outside the cases directory and no branch made.
#[test]
fn case_ids_reaching_outside_the_cases_directory_are_refused() {
    let tree = scratch("ids");
    let (case, _) = at_the_gate(&tree, "real");
    // A case-shaped directory outside the cases directory, as a `../` id would reach it.
    let outside = tree.join("outside");
    fs::create_dir_all(outside.join("evidence")).expect("a directory");
    for file in [
        "case.yaml",
        "protocol.yaml",
        "protocol.ir.json",
        "intent.md",
        "branch.yaml",
    ] {
        fs::copy(case.dir.join(file), outside.join(file)).expect("a copy");
    }
    let absolute = tree.join("absolute").to_string_lossy().into_owned();
    let none: Vec<Box<dyn Validator>> = Vec::new();
    for id in [
        "../../outside",
        "a/b",
        absolute.as_str(),
        "..",
        ".",
        "",
        "a/../../../outside",
        "x\\..\\y",
    ] {
        let refusal = format!("{:#}", check_id(id).expect_err(id));
        assert!(refusal.contains("case id"), "{id}: {refusal}");

        let refusal = format!(
            "{:#}",
            approve(&tree, &NoStore, id, "repository.merge", "human:operator").expect_err(id)
        );
        assert!(refusal.contains("case id"), "{id}: {refusal}");
        assert!(!outside.join(AUTHORITY_FILE).exists(), "{id}");

        let refusal = format!(
            "{:#}",
            Run::resume(&tree, options(id), id, &NoHarness, &none)
                .err()
                .expect(id)
        );
        assert!(refusal.contains("case id"), "{id}: {refusal}");

        let refusal = format!(
            "{:#}",
            Run::open(&tree, options(id), &NoHarness, &none)
                .err()
                .expect(id)
        );
        assert!(refusal.contains("case id"), "{id}: {refusal}");
    }
    assert!(!Path::new(&absolute).exists());
    assert!(!tree.join(CASES_DIR).join("a").exists());
    assert_eq!(
        git(&tree, &["branch", "--list", "takt/*"]).lines().count(),
        1,
        "a branch was made for a refused id"
    );
    for id in ["real", "story-1-20261018T000000Z", "takt-20261018T000000Z"] {
        check_id(id).unwrap_or_else(|error| panic!("{id}: {error:#}"));
    }
    // A branch that names a nested case resolves none: the validators resolve the case from
    // the branch the tree is on.
    let nested = tree.join(CASES_DIR).join("x").join("nested");
    fs::create_dir_all(&nested).expect("a directory");
    fs::copy(case.dir.join("case.yaml"), nested.join("case.yaml")).expect("a copy");
    git(&tree, &["checkout", "-q", "-b", "takt/x/nested"]);
    let refusal = format!(
        "{:#}",
        takt::case::current(&tree).expect_err("a nested case id")
    );
    assert!(refusal.contains("case id"), "{refusal}");
}

/// A case id carrying an invisible or direction-changing character (a Unicode format
/// character, category Cf, or a whitespace or separator character) is refused, naming the
/// character's code point: such an id displays as another one in every message and in its
/// `takt/<id>` branch.
#[test]
fn case_ids_with_unicode_format_or_separator_characters_are_refused_naming_the_code_point() {
    let ranges: &[(u32, u32)] = &[
        (0x00AD, 0x00AD),
        (0x061C, 0x061C),
        (0x180E, 0x180E),
        (0x200B, 0x200F),
        (0x2028, 0x202E),
        (0x2060, 0x2064),
        (0x2066, 0x206F),
        (0xFEFF, 0xFEFF),
        (0xFFF9, 0xFFFB),
        (0xE0001, 0xE0001),
        (0xE0020, 0xE007F),
        (0x0020, 0x0020),
        (0x00A0, 0x00A0),
        (0x3000, 0x3000),
    ];
    for &(first, last) in ranges {
        for point in first..=last {
            let c = char::from_u32(point).expect("a character");
            let id = format!("case{c}id");
            let refusal = format!("{:#}", check_id(&id).expect_err(&format!("U+{point:04X}")));
            assert!(refusal.contains("case id"), "U+{point:04X}: {refusal}");
            assert!(
                refusal.contains(&format!("U+{point:04X}")),
                "U+{point:04X} is not named: {refusal}"
            );
        }
    }
    for id in ["fix-\u{fc}ber", "caf\u{e9}", "\u{65e5}\u{672c}"] {
        check_id(id).unwrap_or_else(|error| panic!("{id}: {error:#}"));
    }
}

/// A frontier that is never asked anything.
struct NoStore;

impl Frontier for NoStore {
    fn name(&self) -> &str {
        "no-store"
    }

    fn candidates(&self, _: &Path) -> anyhow::Result<Vec<String>> {
        Ok(Vec::new())
    }

    fn active(&self, _: &Path) -> anyhow::Result<Vec<String>> {
        Ok(Vec::new())
    }

    fn show(&self, _: &Path, id: &str) -> anyhow::Result<Story> {
        panic!("{id} was shown")
    }

    fn take(&self, _: &Path, id: &str) -> anyhow::Result<Take> {
        panic!("{id} was taken")
    }

    fn record_approval(&self, _: &Path, story: &str, _: &Approval) -> anyhow::Result<()> {
        panic!("an approval was recorded on {story}")
    }
}

// ---------------------------------------------------------------------------------------------
// Evidence and decisions.
// ---------------------------------------------------------------------------------------------

/// An evidence record that does not read stops the run with an error naming the file, and the
/// run lets go of the case; `has_evidence` names it too. A record written as YAML, which Canon
/// reads, is read as one.
#[test]
fn an_unreadable_evidence_record_stops_the_run_naming_the_file() {
    let tree = scratch("unreadable");
    let none: Vec<Box<dyn Validator>> = Vec::new();
    let mut run = Run::open(&tree, options("unreadable"), &NoHarness, &none).expect("opens");
    let revision = run
        .case
        .revision("implementation")
        .expect("implementation has a revision");
    let evidence = run.case.dir.join("evidence");
    fs::write(
        evidence.join("0001-as-yaml.yaml"),
        format!(
            "format: canon-evidence/1\nid: as-yaml\nkind: test_result\nresult: pass\nsubject: implementation\nsubject_revision: \"{revision}\"\nobserved_at: 2026-10-18T00:00:00Z\nproducer: takt.tests-run\n"
        ),
    )
    .expect("a record");
    assert!(
        run.case
            .has_evidence("test_result", "implementation")
            .expect("a YAML record reads")
    );
    assert!(
        !run.case
            .has_evidence("gate_result", "implementation")
            .expect("a YAML record reads")
    );

    fs::write(evidence.join("0002-broken.json"), "{ not a record").expect("a broken record");
    let error = format!(
        "{:#}",
        run.case
            .has_evidence("gate_result", "implementation")
            .expect_err("the record does not read")
    );
    assert!(error.contains("0002-broken.json"), "{error}");
    let error = format!(
        "{:#}",
        run.run(&mut Vec::new())
            .expect_err("the record does not read")
    );
    assert!(error.contains("0002-broken.json"), "{error}");
    assert!(!run.case.dir.join(LOCK_FILE).exists());
}

/// `latest_decision` is the decision with the highest sequence number, past 9999 too, and a
/// file in `decisions/` that is not a numbered decision is not one.
#[test]
fn latest_decision_returns_the_highest_sequence_number_past_9999() {
    let tree = scratch("latest");
    let case = CaseDir::open(&tree, protocol(), "latest", "deliver the story", &[])
        .expect("the case opens");
    case.evaluate(9999).expect("evaluates");
    case.evaluate(10000).expect("evaluates");
    let decisions = case.dir.join("decisions");
    let mut earlier: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(decisions.join("9999.json")).expect("reads"))
            .expect("json");
    earlier["marker"] = "9999".into();
    fs::write(decisions.join("9999.json"), earlier.to_string()).expect("writes");
    let latest = latest_decision(&case.dir)
        .expect("reads")
        .expect("a decision");
    let document: serde_json::Value = serde_json::from_str(&latest.json).expect("json");
    assert!(document.get("marker").is_none(), "{}", latest.json);
    fs::write(decisions.join("notes.json"), r#"{"marker":"notes"}"#).expect("writes");
    let latest = latest_decision(&case.dir)
        .expect("reads")
        .expect("a decision");
    let document: serde_json::Value = serde_json::from_str(&latest.json).expect("json");
    assert!(document.get("marker").is_none(), "{}", latest.json);
    assert_eq!(case.evaluations().expect("counts"), 10000);
}
