//! Adversarial cases for wave 48 unit e: a pull request behind its base is brought up to date by
//! takt's own merge commit before the merge.
//!
//! A pull request whose head is already an earlier base update, the base moved on again: the
//! second update is made, recorded, named in `branch.yaml` and pushed. When that push is not
//! taken by the forge, the pull request's head stays the earlier update, which is neither the
//! revision nor the head `branch.yaml` names, and every later pass stops at "nothing to merge"
//! before it reaches the update again. The same check stops a pass that observes again while the
//! forge has not yet moved the pull request to the push, which the update itself said was waited
//! on.
//!
//! And a head that already holds the tip the judge guard judged gets no second update while the
//! forge still reads it behind, which no case of the unit pins.
//!
//! Every repository here is a real git repository under this crate's build directory; the forge
//! is the in-memory double behind a wrapper. Nothing here reaches a host.

#[path = "support/own_home.rs"]
mod own_home;

use std::cell::{Cell, RefCell};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::rc::Rc;

use anyhow::anyhow;
use serde_json::{Value, json};
use takt::evidence::Record;
use takt::forge::{Checks, Forge, Head, Memory, PullRequestView};
use takt::harness::{Harness, StepContext, StepReport};
use takt::validator::{CiRead, PrMerged, PrOpen, Validator};
use takt::{Run, RunOptions, Stop};

/// The record of the base updates takt made, under the case's directory.
const BASE_UPDATE: &str = "artifacts/base-update.json";

/// `git <args>` in `dir` with no configuration of the user's or the system's; its trimmed
/// standard output.
fn git(dir: &Path, args: &[&str]) -> String {
    let output = git_output(dir, args);
    assert!(
        output.status.success(),
        "git {args:?} in {}: {}",
        dir.display(),
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout)
        .expect("utf-8")
        .trim()
        .to_owned()
}

/// `git <args>` in `dir` as [`git`] runs it, whatever its exit status.
fn git_output(dir: &Path, args: &[&str]) -> std::process::Output {
    Command::new("git")
        .args(["-c", "user.name=t", "-c", "user.email=t@localhost"])
        .args([
            "-c",
            "commit.gpgsign=false",
            "-c",
            "core.hooksPath=/dev/null",
        ])
        .args(args)
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CEILING_DIRECTORIES", env!("CARGO_TARGET_TMPDIR"))
        .current_dir(dir)
        .output()
        .expect("git runs")
}

/// A scratch directory under this crate's build directory, removed when dropped.
struct Scratch(PathBuf);

impl Scratch {
    fn new(name: &str) -> Self {
        static MADE: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let made = MADE.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
            .join(format!("w48-adv-e-{}-{made}-{name}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("a scratch directory");
        Self(dir)
    }
}

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

/// The files every repository here starts with: a README, a guide and the judge allow-list.
fn seed_files(dir: &Path) {
    let files = [
        ("README.md", "a repository\n"),
        ("docs/guide.md", "a guide\n"),
        (
            ".engineering/judge-allow.yaml",
            "paths:\n  - docs/**\n  - README.md\n  - CHANGELOG.md\n",
        ),
    ];
    for (path, text) in files {
        let path = dir.join(path);
        fs::create_dir_all(path.parent().expect("a parent")).expect("a directory");
        fs::write(path, text).expect("a file");
    }
}

/// Commits `file` as `text` on a branch `branch` made at `main`, then checks `main` out again.
fn side_commit(tree: &Path, branch: &str, file: &str, text: &str) -> String {
    git(tree, &["checkout", "-q", "-b", branch, "main"]);
    let path = tree.join(file);
    fs::create_dir_all(path.parent().expect("a parent")).expect("a directory");
    fs::write(path, text).expect("a file");
    git(tree, &["add", "-A", "--", file]);
    git(tree, &["commit", "-q", "-m", &format!("{branch}: {file}")]);
    let commit = git(tree, &["rev-parse", "HEAD"]);
    git(tree, &["checkout", "-q", "main"]);
    commit
}

/// Commits `file` as `text` above `at` with no checkout and no ref: the commit.
fn side_commit_at(tree: &Path, name: &str, at: &str, file: &str, text: &str) -> String {
    let index = tree.join(".git").join(format!("index-{name}"));
    let indexed = |args: &[&str]| -> String {
        let output = Command::new("git")
            .args(args)
            .env("GIT_INDEX_FILE", &index)
            .env("GIT_CEILING_DIRECTORIES", env!("CARGO_TARGET_TMPDIR"))
            .current_dir(tree)
            .output()
            .expect("git runs");
        assert!(output.status.success(), "{args:?}");
        String::from_utf8(output.stdout)
            .expect("utf-8")
            .trim()
            .to_owned()
    };
    indexed(&["read-tree", at]);
    let source = tree.join(".git").join(format!("blob-{name}"));
    fs::write(&source, text).expect("a file");
    let blob = git(
        tree,
        &["hash-object", "-w", source.to_str().expect("utf-8")],
    );
    indexed(&[
        "update-index",
        "--add",
        "--cacheinfo",
        &format!("100644,{blob},{file}"),
    ]);
    let object = indexed(&["write-tree"]);
    let _ = fs::remove_file(index);
    let _ = fs::remove_file(source);
    commit_tree(tree, &object, &[at], &format!("{name}: {file}"))
}

/// The tree `git merge-tree --write-tree` makes of `ours` and `theirs`.
fn clean_merge(tree: &Path, ours: &str, theirs: &str) -> String {
    git(tree, &["merge-tree", "--write-tree", ours, theirs])
        .lines()
        .next()
        .expect("a tree")
        .to_owned()
}

/// A commit of the tree object `object` over `parents`, saying `message`.
fn commit_tree(tree: &Path, object: &str, parents: &[&str], message: &str) -> String {
    let mut args = vec!["commit-tree", object];
    for parent in parents {
        args.extend(["-p", parent]);
    }
    args.extend(["-m", message]);
    git(tree, &args)
}

/// Writes the case's record of base updates: each `(previous head, base tip, merge)`.
fn record_updates(dir: &Path, updates: &[(&str, &str, &str)]) {
    let list: Vec<Value> = updates
        .iter()
        .map(|(previous, tip, merge)| {
            json!({
                "previous_head": previous,
                "base_tip": tip,
                "merge": merge,
                "at": "2026-10-30T09:00:00Z",
            })
        })
        .collect();
    let path = dir.join(BASE_UPDATE);
    fs::create_dir_all(path.parent().expect("a parent")).expect("a directory");
    fs::write(path, serde_json::to_vec_pretty(&list).expect("json")).expect("the record");
}

/// The base updates the case at `dir` recorded.
fn base_updates(dir: &Path) -> Vec<Value> {
    match fs::read_to_string(dir.join(BASE_UPDATE)) {
        Ok(text) => serde_json::from_str::<Vec<Value>>(&text).expect("the record is a list"),
        Err(_) => Vec::new(),
    }
}

/// A harness that asks for the merge gate whenever it is offered and hands over otherwise.
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

/// The in-memory double, its pull request read behind its base, whose next `fail` pushes are
/// not taken (an error, the pull request's head where it was) and whose pull request reads, for
/// `lag` reads after each push it takes, still answer the head from before the push.
struct Flaky {
    memory: Rc<Memory>,
    fail: Cell<usize>,
    lag: usize,
    stale: RefCell<Option<(Head, usize)>>,
}

impl Flaky {
    fn new(memory: Rc<Memory>, fail: usize, lag: usize) -> Self {
        Self {
            memory,
            fail: Cell::new(fail),
            lag,
            stale: RefCell::new(None),
        }
    }
}

impl Forge for Flaky {
    fn name(&self) -> &str {
        "flaky"
    }
    fn scan_outgoing(
        &self,
        tree: &Path,
        dir: &Path,
        outgoing: &takt::forge::Outgoing<'_>,
    ) -> anyhow::Result<()> {
        self.memory.scan_outgoing(tree, dir, outgoing)
    }
    fn push(&self, tree: &Path, branch: &str) -> anyhow::Result<()> {
        self.memory.push(tree, branch)
    }
    fn push_revision(&self, tree: &Path, branch: &str, revision: &str) -> anyhow::Result<()> {
        if self.fail.get() > 0 {
            self.fail.set(self.fail.get() - 1);
            self.memory
                .calls
                .borrow_mut()
                .push(format!("push {revision} as {branch}: not taken"));
            anyhow::bail!("the forge did not take the push of {revision} as {branch}");
        }
        let before = self.memory.pull_request(tree, 1)?.head;
        self.memory.push_revision(tree, branch, revision)?;
        if self.lag > 0 {
            *self.stale.borrow_mut() = Some((before, self.lag));
        }
        Ok(())
    }
    fn open_pull_request(
        &self,
        tree: &Path,
        dir: &Path,
        request: &takt::forge::PullRequest,
    ) -> anyhow::Result<u64> {
        self.memory.open_pull_request(tree, dir, request)
    }
    fn pull_request(&self, tree: &Path, number: u64) -> anyhow::Result<PullRequestView> {
        let mut view = self.memory.pull_request(tree, number)?;
        let mut stale = self.stale.borrow_mut();
        if let Some((head, left)) = stale.as_mut() {
            view.head = head.clone();
            *left -= 1;
            if *left == 0 {
                *stale = None;
            }
        }
        Ok(view)
    }
    fn behind_base(&self, _: &Path, _: u64) -> anyhow::Result<Option<bool>> {
        Ok(Some(true))
    }
    fn checks(&self, tree: &Path, number: u64) -> anyhow::Result<Checks> {
        self.memory.checks(tree, number)
    }
    fn merge(&self, tree: &Path, dir: &Path, number: u64, head: &str) -> anyhow::Result<()> {
        self.memory.merge(tree, dir, number, head)
    }
    fn target_tip(&self, tree: &Path, base: &str) -> anyhow::Result<String> {
        self.memory.target_tip(tree, base)
    }
}

/// What [`under_an_update`] made.
struct Fixture {
    tree: PathBuf,
    dir: PathBuf,
    revision: String,
    update: String,
    earlier: String,
    memory: Rc<Memory>,
}

/// A case whose pull request #1's head is takt's recorded base update `update`, which merged
/// `earlier`, a commit above the case's base writing `docs/news.md`, into the revision. The case
/// opens on `main` with no delegation on record, runs to the merge gate, and the owner grants
/// the merge at its revision. With `moved_again`, `main` then moves to a commit above `earlier`
/// writing `docs/other.md`, so the base moved on after the first update, as a busy base does;
/// else `main` is `earlier`, the tip the first update merged.
fn under_an_update(scratch: &Scratch, moved_again: bool) -> Fixture {
    let seed = scratch.join("seed");
    fs::create_dir_all(&seed).expect("a directory");
    seed_files(&seed);
    git(&seed, &["init", "-q", "-b", "main"]);
    git(&seed, &["add", "-A"]);
    git(&seed, &["commit", "-q", "-m", "start"]);
    git(scratch, &["clone", "-q", "--bare", "seed", "hub.git"]);
    fs::remove_dir_all(&seed).expect("the seed is removed");
    git(scratch, &["clone", "-q", "--no-local", "hub.git", "tree"]);
    let tree = scratch.join("tree");

    let earlier = side_commit(&tree, "upstream", "docs/news.md", "news\n");
    let first: Rc<dyn Forge> = Rc::new(Memory {
        checks: Checks::Pass,
        ..Memory::default()
    });
    let validators: Vec<Box<dyn Validator>> = vec![
        Box::new(PrOpen::new(first.clone())),
        Box::new(CiRead::new(first.clone())),
        Box::new(PrMerged::new(first)),
    ];
    let id = "twice";
    let mut run = Run::open(
        &tree,
        RunOptions {
            protocol: "story.delivery/1".to_owned(),
            intent: "change the guide\n\nA guide that says more.".to_owned(),
            inputs: Vec::new(),
            iterations: Some(12),
            hours: None,
            tokens: None,
            case_id: Some(id.to_owned()),
            gate: Some("repository.merge".to_owned()),
        },
        &Requester,
        &validators,
    )
    .expect("the case opens");
    run.poll = std::time::Duration::from_millis(5);
    fs::write(tree.join("docs/guide.md"), "a guide, changed\n").expect("a file");
    git(&tree, &["add", "-A", "--", ".", ":!.takt"]);
    git(&tree, &["commit", "-q", "-m", "change the guide"]);
    let revision = git(&tree, &["rev-parse", "HEAD"]);
    run.case
        .set_revision("implementation", &revision)
        .expect("the revision moves");
    let short = &revision[..8];
    for (record, kind, result, producer) in [
        ("e0".to_owned(), "test_result", Some("pass"), "w48-adv-e"),
        ("e1".to_owned(), "gate_result", Some("pass"), "w48-adv-e"),
        (
            format!("handover-{short}"),
            "handover",
            None,
            takt::run::CHOICE_PRODUCER,
        ),
        (
            format!("change-record-{short}"),
            "change_record",
            Some("ahead"),
            "takt.change-read",
        ),
    ] {
        run.case
            .add_evidence(&Record::new(
                record,
                kind,
                result.map(str::to_owned),
                "implementation",
                &revision,
                producer,
            ))
            .expect("the record is written");
    }
    let dir = run.case.dir.clone();
    let mut out = Vec::new();
    let stop = run.run(&mut out);
    drop(run);
    assert_eq!(
        stop.as_ref().ok(),
        Some(&Stop::ApprovalRequired(vec!["repository.merge".to_owned()])),
        "{stop:?}\n{}",
        String::from_utf8_lossy(&out)
    );
    takt::case::grant(&tree, id, "repository.merge", "human:owner").expect("granted");

    let branch = format!("takt/{id}");
    let merged = clean_merge(&tree, &revision, &earlier);
    let update = commit_tree(&tree, &merged, &[&revision, &earlier], "bring up to date");
    record_updates(&dir, &[(&revision, &earlier, &update)]);
    let mut stored = takt::case::Branch::read(&dir).expect("branch.yaml reads");
    stored.pushed = Some(update.clone());
    stored.pull_request = Some(1);
    stored.write(&dir).expect("branch.yaml is written");
    let tip = if moved_again {
        side_commit_at(&tree, "again", &earlier, "docs/other.md", "other\n")
    } else {
        earlier.clone()
    };
    git(&tree, &["update-ref", "refs/heads/main", &tip]);
    let memory = Rc::new(Memory {
        checks: Checks::Pass,
        opened: RefCell::new(vec![takt::forge::PullRequest {
            branch: branch.clone(),
            base: "main".to_owned(),
            title: String::new(),
            body: String::new(),
        }]),
        pushed: RefCell::new(BTreeMap::from([(branch, update.clone())])),
        ..Memory::default()
    });
    Fixture {
        tree,
        dir,
        revision,
        update,
        earlier,
        memory,
    }
}

/// The base moved again after a first update; the second update's push is not taken (a network
/// error, a forge that answered 5xx before it wrote). `branch.yaml` already names the second
/// update, the pull request still has the first as its head. A later pass, with the forge
/// taking pushes again, must bring the pull request to the base and merge it, as a rider's push
/// that was not taken is pushed again by the next pass. Within three passes the pull request
/// is merged.
#[test]
fn a_second_update_whose_push_was_not_taken_is_pushed_again_by_a_later_pass() {
    let scratch = Scratch::new("push-not-taken");
    let fixture = under_an_update(&scratch, true);
    let forge = Rc::new(Flaky::new(fixture.memory.clone(), 1, 0));
    let merged = PrMerged::new(forge.clone());

    let first = merged.observe_waiting(&fixture.tree, &fixture.revision, 20);
    assert!(
        first.is_err(),
        "the first pass fails on the push: {:?}",
        first.map(|(observation, waits)| (observation.summary, waits))
    );
    assert_eq!(
        fixture.memory.pushed.borrow().values().next().cloned(),
        Some(fixture.update.clone()),
        "the forge did not take the second update: the pull request's head is the first"
    );
    let named = takt::case::Branch::read(&fixture.dir)
        .expect("reads")
        .pushed
        .expect("a head named");
    assert_ne!(named, fixture.update, "branch.yaml names the second update");
    assert_eq!(base_updates(&fixture.dir).len(), 2);

    let mut summaries = Vec::new();
    for seq in 21..24 {
        match merged.observe_waiting(&fixture.tree, &fixture.revision, seq) {
            Ok((observation, waits)) => {
                summaries.push(format!("{waits}: {}", observation.summary));
            }
            Err(error) => summaries.push(format!("error: {error:#}")),
        }
        if !fixture.memory.merged.borrow().is_empty() {
            break;
        }
    }
    assert_eq!(
        *fixture.memory.merged.borrow(),
        vec![1],
        "the case never merges after one push the forge did not take; each pass said:\n{}\ncalls: {:?}",
        summaries.join("\n"),
        fixture.memory.calls.borrow()
    );
}

/// The base moved again after a first update; the second update is pushed and taken, and the
/// forge moves the pull request to it only after two reads (a forge whose pull read lags its
/// push). The update's own read says the pull request is waited on; the pass observed again
/// while the forge still lags must wait too, not settle on "nothing to merge", which is final
/// at the revision.
#[test]
fn a_pass_observed_again_while_the_forge_lags_the_second_update_still_waits() {
    let scratch = Scratch::new("lag");
    let fixture = under_an_update(&scratch, true);
    let forge = Rc::new(Flaky::new(fixture.memory.clone(), 0, 2));
    let merged = PrMerged::new(forge.clone());

    let (observation, waits) = merged
        .observe_waiting(&fixture.tree, &fixture.revision, 20)
        .expect("the first pass observes");
    assert!(
        waits,
        "the update's own read waits on the forge: {}",
        observation.summary
    );
    let (observation, waits) = merged
        .observe_waiting(&fixture.tree, &fixture.revision, 21)
        .expect("the second pass observes");
    // The pass pushes the same update again and, the forge caught up, merges at it; still
    // lagging, it waits. Either way it does not settle on "nothing to merge".
    assert!(
        waits || !fixture.memory.merged.borrow().is_empty(),
        "a pass made while the forge still lags the push is waited on or merges, not final: {}",
        observation.summary
    );
    let updates = base_updates(&fixture.dir);
    assert_eq!(
        updates.len(),
        2,
        "the second pass makes no further merge commit: {updates:?}"
    );
}

/// The forge still reads the pull request behind (its base moved after the guard judged it, or
/// a read the host has not recomputed since the update), and the head already holds the tip the
/// judge guard judged. No second update is made: nothing recorded, nothing pushed, and the merge
/// goes at the head as it is. Pins the update's "the head already holds the tip" condition,
/// which no case of the unit reaches.
#[test]
fn a_head_holding_the_judged_tip_gets_no_second_update_while_the_forge_reads_behind() {
    let scratch = Scratch::new("holds-tip");
    let fixture = under_an_update(&scratch, false);
    let forge = Rc::new(Flaky::new(fixture.memory.clone(), 0, 0));
    let (observation, _) = PrMerged::new(forge)
        .observe_waiting(&fixture.tree, &fixture.revision, 20)
        .expect("repository.merge observes");
    let updates = base_updates(&fixture.dir);
    assert_eq!(
        updates.len(),
        1,
        "no second update: {updates:?}\n{}",
        observation.summary
    );
    assert_eq!(updates[0]["base_tip"], fixture.earlier.as_str());
    let calls = fixture.memory.calls.borrow().clone();
    assert!(
        !calls.iter().any(|call| call.starts_with("push")),
        "nothing pushed: {calls:?}"
    );
    assert_eq!(*fixture.memory.merged.borrow(), vec![1], "{calls:?}");
    assert_eq!(
        fixture.memory.pushed.borrow().values().next().cloned(),
        Some(fixture.update.clone()),
        "merged at the first update"
    );
    assert!(
        calls.iter().any(|call| call.starts_with("checks")),
        "the checks were read at the head above the revision: {calls:?}"
    );
}

// ------------------------------------------------------------------------------------------
// The gitlab read of whether a change request is behind its target, which no case of the unit
// reaches.

/// A double of `connectors` for the gitlab connector: describes every operation, and answers
/// `merge_request.get` with an open change request whose body carries the fields it holds
/// besides.
struct GitlabRead(Rc<RefCell<Value>>);

impl takt::forge::Runner for GitlabRead {
    fn run(&self, _: &Path, argv: &[String]) -> anyhow::Result<takt::forge::Exit> {
        let operation = argv[5].as_str();
        let result = if argv.iter().any(|word| word == "--describe") {
            json!({
                "connector": "gitlab",
                "instance": "forge",
                "operation": operation,
                "description": "",
                "effect": if operation.ends_with(".get") { "read" } else { "write" },
                "revision": "descriptor-r1",
                "schema": format!("gitlab/{operation}/input/descriptor-r1"),
                "input_schema": {},
            })
        } else {
            assert_eq!(operation, "merge_request.get", "{argv:?}");
            let mut body = json!({
                "iid": 17,
                "state": "opened",
                "source_branch": "takt/case-1",
                "target_branch": "main",
                "sha": "0123456789abcdef0123456789abcdef01234567",
                "head_pipeline": null,
            });
            for (key, value) in self.0.borrow().as_object().expect("an object") {
                body[key] = value.clone();
            }
            json!({
                "connector": "gitlab",
                "instance": "forge",
                "operation": operation,
                "result": {"status": 200, "body": body, "provenance": {}},
                "revision": "descriptor-r1",
            })
        };
        Ok(takt::forge::Exit {
            code: Some(0),
            stdout: json!({"ok": true, "result": result}).to_string(),
            stderr: String::new(),
        })
    }

    fn run_env(
        &self,
        tree: &Path,
        argv: &[String],
        _: &[(&str, &str)],
    ) -> anyhow::Result<takt::forge::Exit> {
        self.run(tree, argv)
    }
}

/// The gitlab read answers behind on `need_rebase` whatever the count, behind on a count above
/// 0, not behind on a count of 0 or a settled status, and not known while GitLab still computes
/// or says neither.
#[test]
fn the_gitlab_read_answers_behind_from_its_fields() {
    let scratch = Scratch::new("gitlab-behind");
    let repo = scratch.join("repo");
    fs::create_dir_all(&repo).expect("a directory");
    git(&repo, &["init", "-q", "-b", "main"]);
    fs::write(repo.join("README.md"), "a story\n").expect("a file");
    git(&repo, &["add", "-A"]);
    git(&repo, &["commit", "-q", "-m", "start"]);
    git(&repo, &["checkout", "-q", "-b", "takt/case-1"]);
    git(
        &repo,
        &[
            "remote",
            "add",
            "origin",
            "https://forge.example/group/project.git",
        ],
    );
    let extra = Rc::new(RefCell::new(json!({})));
    let forge = takt::forge::connectors::ConnectorsForge::for_tree(
        &repo,
        "gitlab/forge",
        None,
        takt::scan::Scan::None {
            declared_in: scratch.join("run.yaml"),
        },
        Box::new(GitlabRead(extra.clone())),
    )
    .expect("a forge");
    for (fields, expected) in [
        (json!({"detailed_merge_status": "need_rebase"}), Some(true)),
        (
            json!({"detailed_merge_status": "need_rebase", "diverged_commits_count": 0}),
            Some(true),
        ),
        (
            json!({"detailed_merge_status": "mergeable", "diverged_commits_count": 1}),
            Some(true),
        ),
        (
            json!({"detailed_merge_status": "mergeable", "diverged_commits_count": 0}),
            Some(false),
        ),
        (json!({"detailed_merge_status": "mergeable"}), Some(false)),
        (json!({"detailed_merge_status": "checking"}), None),
        (json!({"detailed_merge_status": "unchecked"}), None),
        (json!({}), None),
    ] {
        *extra.borrow_mut() = fields.clone();
        assert_eq!(
            forge.behind_base(&repo, 17).expect("read"),
            expected,
            "{fields}"
        );
    }
}
