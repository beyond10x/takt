//! A case's pull request that is mergeable but behind its base: the base moved after the case's
//! base commit. Before the merge takt reads whether the pull request is behind
//! ([`Forge::behind_base`]); when it is, takt merges the base's tip, the one the judge guard
//! judged, into the case's head as its own merge commit (first parent the head, second the tip),
//! records it in the case (`artifacts/base-update.json`), pushes it as it pushes its riders,
//! reads the checks on the new head and merges at that head. The rider rule takes exactly such a
//! recorded merge and nothing else: an unrecorded merge commit, one whose tree is not the clean
//! merge of its parents, and one whose second parent is not the recorded tip on the base ride on
//! nothing.
//!
//! The local forge's cases run over real git repositories under this crate's build directory;
//! the github route's over a loopback stand-in of a served Connectors instance whose pull read
//! says `mergeable_state: "behind"` until the head holds the base's tip, and which answers a merge
//! of a head behind its base `outcome_unknown`, as a strict "up to date" rule makes it. Nothing
//! here reaches a host.

#[path = "support/own_home.rs"]
mod own_home;

use std::collections::BTreeMap;
use std::fs;
use std::io::{BufRead as _, BufReader, Read as _, Write as _};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::rc::Rc;
use std::sync::{Arc, Mutex};

use anyhow::anyhow;
use serde_json::{Value, json};
use takt::case::approvals_only;
use takt::evidence::Record;
use takt::forge::connectors::ConnectorsForge;
use takt::forge::local::Local;
use takt::forge::{Checks, Exit, Forge, Memory, Runner};
use takt::git::CommitIdentity;
use takt::harness::{Harness, StepContext, StepReport};
use takt::run_config::RunConfig;
use takt::scan::Scan;
use takt::validator::{CiRead, PrMerged, PrOpen, Validator};
use takt::{Run, RunOptions, Stop};

/// The record of the base updates takt made, under the case's directory.
const BASE_UPDATE: &str = "artifacts/base-update.json";

/// `git <args>` in `dir` with no configuration of the user's or the system's, no signing and no
/// hook, never looking for a repository above the build's scratch directory; its trimmed
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

/// Whether `ancestor` is an ancestor of `descendant` in `dir`.
fn is_ancestor(dir: &Path, ancestor: &str, descendant: &str) -> bool {
    git_output(dir, &["merge-base", "--is-ancestor", ancestor, descendant])
        .status
        .success()
}

/// A scratch directory under this crate's build directory, removed when dropped.
struct Scratch(PathBuf);

impl Scratch {
    /// A directory of its own for every call, so tests running at once never share one.
    fn new(name: &str) -> Self {
        static MADE: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let made = MADE.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
            .join(format!("w48-unit-e-{}-{made}-{name}", std::process::id()));
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

/// Writes the files every repository here starts with into `dir`: a README, a guide, the judge
/// allow-list and the owner's delegation of the merge to `agent:takt-delivery`.
fn seed_files(dir: &Path) {
    let files = [
        ("README.md", "a repository\n".to_owned()),
        ("docs/guide.md", "a guide\n".to_owned()),
        (
            ".engineering/judge-allow.yaml",
            "paths:\n  - docs/**\n  - README.md\n  - CHANGELOG.md\n".to_owned(),
        ),
        (
            ".engineering/delegations.yaml",
            ["repository.merge", "repository.merge.judge_surface"]
                .iter()
                .fold("delegations:\n".to_owned(), |text, capability| {
                    text + &format!(
                        "  - owner: human:owner\n    role: agent:takt-delivery\n    capability: {capability}\n    given_at: 2026-10-30T08:00:00Z\n"
                    )
                }),
        ),
    ];
    for (path, text) in files {
        let path = dir.join(path);
        fs::create_dir_all(path.parent().expect("a parent")).expect("a directory");
        fs::write(path, text).expect("a file");
    }
}

/// Commits `file` as `text` on a branch `branch` made at `main` in `tree`, then checks `main`
/// out again: the commit, which no branch the case uses holds.
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

/// A harness that asks for the merge gate whenever it is offered and hands the change over when
/// that is offered: the run's grant is never its choice.
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

/// The options of a run of `story.delivery/1` for the case `id`, to the merge gate.
fn options(id: &str) -> RunOptions {
    RunOptions {
        protocol: "story.delivery/1".to_owned(),
        intent: "change the guide\n\nA guide that says more.".to_owned(),
        inputs: Vec::new(),
        iterations: Some(12),
        hours: None,
        tokens: None,
        case_id: Some(id.to_owned()),
        gate: Some("repository.merge".to_owned()),
    }
}

/// The three forge validators over `forge`.
fn validators(forge: Rc<dyn Forge>) -> Vec<Box<dyn Validator>> {
    vec![
        Box::new(PrOpen::new(forge.clone())),
        Box::new(CiRead::new(forge.clone())),
        Box::new(PrMerged::new(forge)),
    ]
}

/// Opens the case `id` on `tree`'s `main`, commits `guide` as the guide on its branch and writes
/// the tests, gate, handover and change records at that revision: the run and the revision.
fn opened<'a>(
    tree: &Path,
    id: &str,
    guide: &str,
    validators: &'a [Box<dyn Validator>],
) -> (Run<'a>, String) {
    let mut run = Run::open(tree, options(id), &Requester, validators).expect("the case opens");
    run.poll = std::time::Duration::from_millis(5);
    fs::write(tree.join("docs/guide.md"), guide).expect("a file");
    git(tree, &["add", "-A", "--", ".", ":!.takt"]);
    git(tree, &["commit", "-q", "-m", "change the guide"]);
    let revision = git(tree, &["rev-parse", "HEAD"]);
    run.case
        .set_revision("implementation", &revision)
        .expect("the revision moves");
    let short = &revision[..8];
    for (id, kind, result, producer) in [
        ("e0".to_owned(), "test_result", Some("pass"), "w48-unit-e"),
        ("e1".to_owned(), "gate_result", Some("pass"), "w48-unit-e"),
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
                id,
                kind,
                result.map(str::to_owned),
                "implementation",
                &revision,
                producer,
            ))
            .expect("the record is written");
    }
    (run, revision)
}

/// The records of `kind` under the case at `dir`, each as its JSON.
fn records_of(dir: &Path, kind: &str) -> Vec<Value> {
    let Ok(entries) = fs::read_dir(dir.join("evidence")) else {
        return Vec::new();
    };
    entries
        .map(|entry| {
            let path = entry.expect("an entry").path();
            serde_json::from_str::<Value>(&fs::read_to_string(&path).expect("a record"))
                .expect("json")
        })
        .filter(|record| record["kind"] == kind)
        .collect()
}

/// The base updates the case at `dir` recorded, `[]` when it recorded none.
fn base_updates(dir: &Path) -> Vec<Value> {
    match fs::read_to_string(dir.join(BASE_UPDATE)) {
        Ok(text) => serde_json::from_str::<Vec<Value>>(&text).expect("the record is a list"),
        Err(_) => Vec::new(),
    }
}

/// The parents of `commit` in `dir`, in order.
fn parents(dir: &Path, commit: &str) -> Vec<String> {
    git(dir, &["rev-list", "--parents", "-n", "1", commit])
        .split_whitespace()
        .skip(1)
        .map(str::to_owned)
        .collect()
}

// ------------------------------------------------------------------------------------------
// The local forge.

/// A hub with `main` holding the seed files and a tree cloned from it with `--no-local`, on
/// `main`: the hub and the tree.
fn hub_and_tree(scratch: &Scratch) -> (PathBuf, PathBuf) {
    let seed = scratch.join("seed");
    fs::create_dir_all(&seed).expect("a directory");
    seed_files(&seed);
    git(&seed, &["init", "-q", "-b", "main"]);
    git(&seed, &["add", "-A"]);
    git(&seed, &["commit", "-q", "-m", "start"]);
    git(scratch, &["clone", "-q", "--bare", "seed", "hub.git"]);
    fs::remove_dir_all(&seed).expect("the seed is removed");
    git(scratch, &["clone", "-q", "--no-local", "hub.git", "tree"]);
    (scratch.join("hub.git"), scratch.join("tree"))
}

/// The local forge over `tree`, its checks `true`.
fn local(scratch: &Scratch, tree: &Path) -> Local {
    Local::for_tree(
        tree,
        vec!["true".to_owned()],
        scratch.join("state"),
        CommitIdentity::new("Test Bot", "bot@example.invalid").expect("an identity"),
    )
    .expect("the hub is a local bare repository outside the tree")
}

/// The local forge: the hub's `main` gets a commit after the case's pull request opens. The
/// pull request reads behind; takt merges `main`'s tip into the case's head as its own commit,
/// records it, pushes it, reads the checks on it and merges it, so the hub's `main` holds the
/// base's commit and the case's change, and the case's branch on the hub is takt's merge commit
/// with the head first and the base's tip second.
#[test]
fn a_base_that_moved_is_merged_into_the_case_branch_before_the_merge() {
    let scratch = Scratch::new("local-behind");
    let (hub, tree) = hub_and_tree(&scratch);
    let moved = side_commit(&tree, "upstream", "docs/news.md", "news\n");
    let forge: Rc<dyn Forge> = Rc::new(local(&scratch, &tree));
    let validators = validators(forge);
    let (mut run, revision) = opened(&tree, "behind", "a guide, changed\n", &validators);
    let dir = run.case.dir.clone();
    let branch = run.case.branch.clone();
    // The base moves on the hub before the run reaches the pull request.
    git(
        &tree,
        &["push", "-q", "origin", &format!("{moved}:refs/heads/main")],
    );

    let mut out = Vec::new();
    let stop = run.run(&mut out);
    let out = String::from_utf8_lossy(&out).into_owned();
    drop(run);
    assert_eq!(
        stop.as_ref().ok(),
        Some(&Stop::Completed("accepted".to_owned())),
        "{stop:?}\n{out}"
    );
    let merged = records_of(&dir, "merge_observation");
    assert_eq!(merged.len(), 1, "{out}");
    assert_eq!(merged[0]["result"], "merged", "{out}");
    assert_eq!(
        merged[0]["subject_revision"], revision,
        "the record is about the granted revision"
    );

    let updates = base_updates(&dir);
    assert_eq!(
        updates.len(),
        1,
        "one base update recorded: {updates:?}\n{out}"
    );
    let update = &updates[0];
    assert_eq!(update["previous_head"], revision, "{update}");
    assert_eq!(update["base_tip"], moved.as_str(), "{update}");
    assert!(
        update["at"].as_str().is_some_and(|at| !at.is_empty()),
        "{update}"
    );
    let merge = update["merge"]
        .as_str()
        .expect("the merge commit")
        .to_owned();
    assert_eq!(
        git(&hub, &["rev-parse", &format!("refs/heads/{branch}")]),
        merge,
        "the case's branch on the hub is takt's merge commit"
    );
    assert_eq!(parents(&hub, &merge), [revision.clone(), moved.clone()]);
    assert_eq!(
        git(&hub, &["log", "-1", "--format=%cn <%ce>", &merge]),
        "takt <takt@localhost>",
        "takt's own commit"
    );

    let main = git(&hub, &["rev-parse", "refs/heads/main"]);
    assert!(
        is_ancestor(&hub, &moved, &main),
        "main holds the base's commit"
    );
    assert!(
        is_ancestor(&hub, &revision, &main),
        "main holds the case's change"
    );
    assert!(
        is_ancestor(&hub, &merge, &main),
        "main holds takt's merge commit"
    );
    assert_eq!(
        git(&hub, &["show", &format!("{main}:docs/news.md")]),
        "news"
    );
    assert_eq!(
        git(&hub, &["show", &format!("{main}:docs/guide.md")]),
        "a guide, changed"
    );
    assert_eq!(
        git(&tree, &["rev-parse", &format!("refs/heads/{branch}")]),
        revision,
        "the tree's branch stays at the revision: a validator moves no ref of the tree"
    );
}

/// The local forge: a pull request whose head holds the base's tip is not behind, and the merge
/// goes as before, with no base update recorded.
#[test]
fn a_pull_request_up_to_date_merges_with_no_base_update() {
    let scratch = Scratch::new("local-current");
    let (hub, tree) = hub_and_tree(&scratch);
    let forge: Rc<dyn Forge> = Rc::new(local(&scratch, &tree));
    let validators = validators(forge);
    let (mut run, revision) = opened(&tree, "current", "a guide, changed\n", &validators);
    let dir = run.case.dir.clone();
    let mut out = Vec::new();
    let stop = run.run(&mut out);
    let out = String::from_utf8_lossy(&out).into_owned();
    drop(run);
    assert_eq!(
        stop.as_ref().ok(),
        Some(&Stop::Completed("accepted".to_owned())),
        "{stop:?}\n{out}"
    );
    assert!(base_updates(&dir).is_empty(), "{out}");
    let main = git(&hub, &["rev-parse", "refs/heads/main"]);
    assert!(is_ancestor(&hub, &revision, &main), "{out}");
}

/// The local forge: the base's new commit and the case change the same lines. Nothing is merged
/// and nothing pushed: the judge guard, on the moved base, grants nothing and the run stops for
/// the merge's approval, naming the conflicting file; the hub's `main` and the case's branch on
/// the hub stay where they were, no base update is recorded and the tree is clean, its branch at
/// the revision.
#[test]
fn a_base_that_conflicts_stops_naming_the_file_and_leaves_everything_as_it_was() {
    let scratch = Scratch::new("local-conflict");
    let (hub, tree) = hub_and_tree(&scratch);
    let moved = side_commit(&tree, "upstream", "docs/guide.md", "a guide, theirs\n");
    let forge: Rc<dyn Forge> = Rc::new(local(&scratch, &tree));
    let validators = validators(forge);
    let (mut run, revision) = opened(&tree, "conflict", "a guide, ours\n", &validators);
    let dir = run.case.dir.clone();
    let branch = run.case.branch.clone();
    git(
        &tree,
        &["push", "-q", "origin", &format!("{moved}:refs/heads/main")],
    );

    let mut out = Vec::new();
    let stop = run.run(&mut out);
    let mut text = String::from_utf8_lossy(&out).into_owned();
    drop(run);
    if let Err(error) = &stop {
        text.push_str(&format!("\nerror: {error:#}"));
    }
    assert_eq!(
        stop.as_ref().ok(),
        Some(&Stop::ApprovalRequired(vec!["repository.merge".to_owned()])),
        "{text}"
    );
    assert!(
        text.contains("conflicts at docs/guide.md"),
        "the stop names the conflicting file: {stop:?}\n{text}"
    );
    assert!(records_of(&dir, "merge_observation").is_empty(), "{text}");
    assert!(base_updates(&dir).is_empty(), "{text}");
    assert_eq!(git(&hub, &["rev-parse", "refs/heads/main"]), moved);
    assert_eq!(
        git(&hub, &["rev-parse", &format!("refs/heads/{branch}")]),
        revision,
        "nothing was pushed over the case's branch"
    );
    assert_eq!(
        git(&tree, &["rev-parse", &format!("refs/heads/{branch}")]),
        revision
    );
    assert_eq!(git(&tree, &["rev-parse", "HEAD"]), revision);
    assert_eq!(
        git(&tree, &["status", "--porcelain", "--untracked-files=all"]),
        "",
        "no half merge in the checkout"
    );
}

/// The local forge's read of whether a request is behind: its base's tip not an ancestor of its
/// head is behind; once the head holds the tip it is not.
#[test]
fn the_local_forge_reads_a_request_behind_its_base() {
    let scratch = Scratch::new("local-read");
    let (_hub, tree) = hub_and_tree(&scratch);
    let forge = local(&scratch, &tree);
    git(&tree, &["checkout", "-q", "-b", "takt/read"]);
    fs::write(tree.join("docs/guide.md"), "a guide, changed\n").expect("a file");
    git(&tree, &["commit", "-q", "-am", "change"]);
    forge.push(&tree, "takt/read").expect("pushed");
    let number = forge
        .open_pull_request(
            &tree,
            &scratch.join("artifacts"),
            &takt::forge::PullRequest {
                branch: "takt/read".to_owned(),
                base: "main".to_owned(),
                title: "read".to_owned(),
                body: String::new(),
            },
        )
        .expect("opened");
    assert_eq!(forge.behind_base(&tree, number).expect("read"), Some(false));
    let moved = side_commit(&tree, "upstream", "docs/news.md", "news\n");
    git(
        &tree,
        &["push", "-q", "origin", &format!("{moved}:refs/heads/main")],
    );
    assert_eq!(forge.behind_base(&tree, number).expect("read"), Some(true));
    git(&tree, &["checkout", "-q", "takt/read"]);
    git(&tree, &["merge", "-q", "--no-edit", &moved]);
    forge.push(&tree, "takt/read").expect("pushed");
    assert_eq!(forge.behind_base(&tree, number).expect("read"), Some(false));
}

/// A forge that cannot tell answers not known, and the merge goes as before: the in-memory
/// double has no such read.
#[test]
fn a_forge_that_cannot_tell_answers_not_known() {
    let scratch = Scratch::new("memory-read");
    let (_hub, tree) = hub_and_tree(&scratch);
    let memory = Memory::default();
    assert_eq!(memory.behind_base(&tree, 1).expect("read"), None);
}

// ------------------------------------------------------------------------------------------
// The rider rule.

/// A case opened on `tree`'s `main` (the in-memory double its forge), its branch one commit
/// above it, and a commit on `upstream` above `main`: the case's directory, the revision and the
/// base's tip.
fn case_with_a_moved_base(scratch: &Scratch) -> (PathBuf, PathBuf, String, String) {
    let (_hub, tree) = hub_and_tree(scratch);
    let moved = side_commit(&tree, "upstream", "docs/news.md", "news\n");
    let memory: Rc<dyn Forge> = Rc::new(Memory {
        checks: Checks::Pass,
        ..Memory::default()
    });
    let validators = validators(memory);
    let (run, revision) = opened(&tree, "riders", "a guide, changed\n", &validators);
    let dir = run.case.dir.clone();
    drop(run);
    (tree, dir, revision, moved)
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

/// The recorded base update rides on the revision: two parents, the head first and the recorded
/// base tip second, the tree the clean merge of the two.
#[test]
fn a_recorded_base_update_rides() {
    let scratch = Scratch::new("rides");
    let (tree, dir, revision, moved) = case_with_a_moved_base(&scratch);
    let merged = clean_merge(&tree, &revision, &moved);
    let merge = commit_tree(&tree, &merged, &[&revision, &moved], "bring up to date");
    assert!(
        !approvals_only(&tree, &revision, &merge, &dir).expect("reads"),
        "an unrecorded merge rides on nothing"
    );
    record_updates(&dir, &[(&revision, &moved, &merge)]);
    assert!(
        approvals_only(&tree, &revision, &merge, &dir).expect("reads"),
        "the recorded base update rides"
    );
}

/// Forged merge commits ride on nothing: an unrecorded one beside a recorded one, one whose tree
/// is not the clean merge of its parents, one whose second parent is not the recorded tip, one
/// whose second parent is not on the base, one whose first parent is not the recorded head, and
/// one with the parents swapped.
#[test]
fn a_forged_base_update_rides_on_nothing() {
    let scratch = Scratch::new("forged");
    let (tree, dir, revision, moved) = case_with_a_moved_base(&scratch);
    let merged = clean_merge(&tree, &revision, &moved);
    let genuine = commit_tree(&tree, &merged, &[&revision, &moved], "bring up to date");

    // Unrecorded: the same tree and parents, another commit.
    let unrecorded = commit_tree(&tree, &merged, &[&revision, &moved], "another message");
    record_updates(&dir, &[(&revision, &moved, &genuine)]);
    assert!(approvals_only(&tree, &revision, &genuine, &dir).expect("reads"));
    assert!(
        !approvals_only(&tree, &revision, &unrecorded, &dir).expect("reads"),
        "an unrecorded merge commit rides"
    );

    // Recorded, its tree not the clean merge: the revision's tree with a file added.
    let extra = {
        let index = scratch.join("index");
        let env_git = |args: &[&str]| -> String {
            let output = Command::new("git")
                .args(args)
                .env("GIT_INDEX_FILE", &index)
                .env("GIT_CEILING_DIRECTORIES", env!("CARGO_TARGET_TMPDIR"))
                .current_dir(&tree)
                .output()
                .expect("git runs");
            assert!(output.status.success(), "{args:?}");
            String::from_utf8(output.stdout)
                .expect("utf-8")
                .trim()
                .to_owned()
        };
        env_git(&["read-tree", &merged]);
        fs::write(scratch.join("evil"), "evil\n").expect("a file");
        let blob = git(
            &tree,
            &[
                "hash-object",
                "-w",
                scratch.join("evil").to_str().expect("utf-8"),
            ],
        );
        env_git(&[
            "update-index",
            "--add",
            "--cacheinfo",
            &format!("100644,{blob},evil.sh"),
        ]);
        env_git(&["write-tree"])
    };
    let wrong_tree = commit_tree(&tree, &extra, &[&revision, &moved], "bring up to date");
    record_updates(&dir, &[(&revision, &moved, &wrong_tree)]);
    assert!(
        !approvals_only(&tree, &revision, &wrong_tree, &dir).expect("reads"),
        "a merge commit whose tree is not the clean merge rides"
    );

    // Recorded, its second parent another commit above the base than the recorded tip.
    let other = side_commit(&tree, "other", "docs/other.md", "other\n");
    let other_merged = clean_merge(&tree, &revision, &other);
    let other_merge = commit_tree(&tree, &other_merged, &[&revision, &other], "up to date");
    record_updates(&dir, &[(&revision, &moved, &other_merge)]);
    assert!(
        !approvals_only(&tree, &revision, &other_merge, &dir).expect("reads"),
        "a merge commit whose second parent is not the recorded tip rides"
    );

    // Recorded as such, its second parent a commit off the base entirely.
    let orphan_tree = git(&tree, &["rev-parse", &format!("{revision}^{{tree}}")]);
    let orphan = commit_tree(&tree, &orphan_tree, &[], "no history");
    let off_base = commit_tree(&tree, &orphan_tree, &[&revision, &orphan], "up to date");
    record_updates(&dir, &[(&revision, &orphan, &off_base)]);
    assert!(
        !approvals_only(&tree, &revision, &off_base, &dir).expect("reads"),
        "a merge commit whose second parent is not on the base rides"
    );

    // Recorded, its first parent not the recorded previous head.
    record_updates(&dir, &[(&moved, &moved, &genuine)]);
    assert!(
        !approvals_only(&tree, &revision, &genuine, &dir).expect("reads"),
        "a merge commit whose first parent is not the recorded head rides"
    );

    // Recorded with the parents swapped.
    let swapped = commit_tree(&tree, &merged, &[&moved, &revision], "bring up to date");
    record_updates(&dir, &[(&moved, &revision, &swapped)]);
    assert!(
        !approvals_only(&tree, &revision, &swapped, &dir).expect("reads"),
        "a merge commit with its parents swapped rides"
    );
}

// ------------------------------------------------------------------------------------------
// `repository.merge` over a double whose pull request reads behind.

/// The in-memory double, its pull request read behind its base.
struct Behind(Rc<Memory>);

impl Forge for Behind {
    fn name(&self) -> &str {
        "behind"
    }
    fn scan_outgoing(
        &self,
        tree: &Path,
        dir: &Path,
        outgoing: &takt::forge::Outgoing<'_>,
    ) -> anyhow::Result<()> {
        self.0.scan_outgoing(tree, dir, outgoing)
    }
    fn push(&self, tree: &Path, branch: &str) -> anyhow::Result<()> {
        self.0.push(tree, branch)
    }
    fn push_revision(&self, tree: &Path, branch: &str, revision: &str) -> anyhow::Result<()> {
        self.0.push_revision(tree, branch, revision)
    }
    fn open_pull_request(
        &self,
        tree: &Path,
        dir: &Path,
        request: &takt::forge::PullRequest,
    ) -> anyhow::Result<u64> {
        self.0.open_pull_request(tree, dir, request)
    }
    fn pull_request(
        &self,
        tree: &Path,
        number: u64,
    ) -> anyhow::Result<takt::forge::PullRequestView> {
        self.0.pull_request(tree, number)
    }
    fn behind_base(&self, _: &Path, _: u64) -> anyhow::Result<Option<bool>> {
        Ok(Some(true))
    }
    fn checks(&self, tree: &Path, number: u64) -> anyhow::Result<Checks> {
        self.0.checks(tree, number)
    }
    fn merge(&self, tree: &Path, dir: &Path, number: u64, head: &str) -> anyhow::Result<()> {
        self.0.merge(tree, dir, number, head)
    }
    fn target_tip(&self, tree: &Path, base: &str) -> anyhow::Result<String> {
        self.0.target_tip(tree, base)
    }
}

/// A case whose pull request's head is an earlier base update. The case opens on `main` with no
/// delegation on record, runs to the merge gate and the owner grants the merge at its revision;
/// then an earlier tip of the base, a commit above `main` that writes `docs/news.md`, is merged
/// into the revision as takt's recorded base update and that pushed as pull request #1's head;
/// then `main` moves to `next`, a commit above the case's base that does not hold the earlier
/// tip, which writes `file` as `text`. The tree, the case's directory, the revision, the base
/// update, the new tip and the double.
fn rebased_under_an_update(
    scratch: &Scratch,
    file: &str,
    text: &str,
) -> (PathBuf, PathBuf, String, String, String, Rc<Memory>) {
    let (_hub, tree) = hub_and_tree(scratch);
    git(&tree, &["rm", "-q", "--", ".engineering/delegations.yaml"]);
    git(&tree, &["commit", "-q", "-m", "no delegation on record"]);
    let earlier = side_commit(&tree, "upstream", "docs/news.md", "news\n");
    let first: Rc<dyn Forge> = Rc::new(Memory {
        checks: Checks::Pass,
        ..Memory::default()
    });
    let validators = validators(first);
    let id = "rebased";
    let (mut run, revision) = opened(&tree, id, "a guide, changed\n", &validators);
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
    let next = side_commit_at(
        &tree,
        "next",
        &stored.base_commit.expect("a base"),
        file,
        text,
    );
    git(&tree, &["update-ref", "refs/heads/main", &next]);
    let memory = Rc::new(Memory {
        checks: Checks::Pass,
        opened: std::cell::RefCell::new(vec![takt::forge::PullRequest {
            branch: branch.clone(),
            base: "main".to_owned(),
            title: String::new(),
            body: String::new(),
        }]),
        pushed: std::cell::RefCell::new(BTreeMap::from([(branch, update.clone())])),
        ..Memory::default()
    });
    (tree, dir, revision, update, next, memory)
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

/// A head that is an earlier base update conflicts with the base's new tip where the revision
/// does not: the judge guard passes, and the base update stops naming the conflicting file,
/// nothing committed, pushed or merged, no update recorded, the tree's branch and checkout as
/// they were.
#[test]
fn a_base_update_that_conflicts_names_the_file_and_changes_nothing() {
    let scratch = Scratch::new("update-conflict");
    let (tree, dir, revision, update, _next, memory) =
        rebased_under_an_update(&scratch, "docs/news.md", "other news\n");
    let head_before = git(&tree, &["rev-parse", "HEAD"]);
    let forge: Rc<dyn Forge> = Rc::new(Behind(memory.clone()));
    let (observation, waits) = PrMerged::new(forge)
        .observe_waiting(&tree, &revision, 20)
        .expect("repository.merge observes");
    assert!(!waits, "{}", observation.summary);
    assert!(observation.records.is_empty(), "{}", observation.summary);
    assert!(
        observation.summary.contains("conflicts at docs/news.md")
            && observation
                .summary
                .contains("no merge commit made, recorded or pushed, and nothing merged"),
        "{}",
        observation.summary
    );
    let calls = memory.calls.borrow().clone();
    assert!(
        !calls
            .iter()
            .any(|call| call.starts_with("push") || call.starts_with("merge")),
        "{calls:?}"
    );
    assert_eq!(base_updates(&dir).len(), 1, "no update recorded");
    assert_eq!(
        takt::case::Branch::read(&dir).expect("reads").pushed,
        Some(update)
    );
    assert_eq!(git(&tree, &["rev-parse", "HEAD"]), head_before);
    assert_eq!(
        git(&tree, &["status", "--porcelain", "--untracked-files=all"]),
        "",
        "no half merge in the checkout"
    );
}

/// A head holding a base update whose tip the base no longer holds (the base was rewritten) is
/// brought up to the new tip, and then not merged: the earlier tip is not on the target the
/// guard judged.
#[test]
fn a_head_holding_a_tip_the_base_no_longer_holds_is_not_merged() {
    let scratch = Scratch::new("update-off-base");
    let (tree, dir, revision, update, next, memory) =
        rebased_under_an_update(&scratch, "docs/other.md", "other\n");
    let forge: Rc<dyn Forge> = Rc::new(Behind(memory.clone()));
    let (observation, _) = PrMerged::new(forge)
        .observe_waiting(&tree, &revision, 20)
        .expect("repository.merge observes");
    assert!(observation.records.is_empty(), "{}", observation.summary);
    assert!(
        observation.summary.contains("does not hold that commit"),
        "{}",
        observation.summary
    );
    assert!(memory.merged.borrow().is_empty(), "nothing merged");
    let updates = base_updates(&dir);
    assert_eq!(updates.len(), 2, "{updates:?}");
    assert_eq!(updates[1]["previous_head"], update.as_str());
    assert_eq!(updates[1]["base_tip"], next.as_str());
}

// ------------------------------------------------------------------------------------------
// The github route of the forge connectors, at a served instance.

/// The connector the run file's `forge_instance` names.
const INSTANCE: &str = "github";

/// The descriptor revision the stand-in describes, which every invocation must carry.
const REVISION: &str = "descriptor-r1";

/// The github connector's operations the stand-in answers.
const OPERATIONS: [&str; 6] = [
    "branch.get",
    "pull_request.create",
    "pull_request.get",
    "pull_request.merge",
    "check_runs.list",
    "commit.status",
];

/// The pull request the stand-in numbers every opened one.
const NUMBER: u64 = 7;

/// The paths of the served wire.
const DESCRIBE: &str = "/v1/describe";
const INVOKE: &str = "/v1alpha2/invoke";

/// The pull request the stand-in holds once one is opened.
#[derive(Clone, Debug)]
struct Pull {
    head: String,
    base: String,
    state: &'static str,
    merged: bool,
}

/// What the stand-in reports: one repository `owner/repo` on GitHub, whose commits are those of
/// the tree `repo`, through which the stand-in tells whether a head holds the base's tip.
#[derive(Debug)]
struct World {
    repo: PathBuf,
    branches: BTreeMap<String, String>,
    pull: Option<Pull>,
    /// The `mergeable_state` the pull read reports when set, whatever the commits say.
    mergeable_state: Option<&'static str>,
}

impl World {
    /// Whether the pull request's head does not hold its base's tip.
    fn behind(&self) -> bool {
        let Some(pull) = &self.pull else {
            return false;
        };
        match (self.branches.get(&pull.head), self.branches.get(&pull.base)) {
            (Some(head), Some(tip)) => !is_ancestor(&self.repo, tip, head),
            _ => false,
        }
    }

    fn view(&self) -> Option<Value> {
        let pull = self.pull.as_ref()?;
        let state = self
            .mergeable_state
            .unwrap_or(if self.behind() { "behind" } else { "clean" });
        Some(json!({
            "number": NUMBER,
            "state": pull.state,
            "merged": pull.merged,
            "mergeable_state": state,
            "head": {"ref": pull.head, "sha": self.branches.get(&pull.head)},
            "base": {"ref": pull.base},
        }))
    }
}

/// A success of the served wire for `request_id`, the operation answering `status` and `body`.
fn success(request_id: &Value, status: u16, body: Value) -> (u16, Value) {
    (
        200,
        json!({
            "version": "v1alpha2",
            "request_id": request_id,
            "status": "success",
            "result": {"status": status, "body": body, "provenance": {}},
            "audit_ref": "audit-1",
            "audit_status": "complete",
        }),
    )
}

/// An error of the served wire for `request_id`, answered with the HTTP status `status`.
fn failure(request_id: &Value, status: u16, code: &str, message: &str) -> (u16, Value) {
    (
        status,
        json!({
            "version": "v1alpha2",
            "request_id": request_id,
            "status": "error",
            "error": {"code": code, "message": message},
            "audit_ref": "audit-2",
            "audit_status": "complete",
        }),
    )
}

/// The operation `envelope` names run on `world`, as the provider answers it; a host without
/// standing grants, so a key is refused as a member it does not take.
fn dispatch(envelope: &Value, world: &mut World) -> (u16, Value) {
    if envelope.get("idempotency_key").is_some() {
        return failure(
            &Value::Null,
            400,
            "unsupported",
            "idempotency_key is not supported",
        );
    }
    let id = &envelope["request_id"];
    if envelope["revision"] != REVISION {
        return failure(id, 409, "stale_description", "the revision changed");
    }
    let input = &envelope["input"];
    match envelope["operation"].as_str().unwrap_or_default() {
        "branch.get" => {
            let name = input["branch"].as_str().unwrap_or_default();
            match world.branches.get(name) {
                Some(sha) => success(id, 200, json!({"name": name, "commit": {"sha": sha}})),
                None => failure(id, 404, "not_found", "no such branch"),
            }
        }
        "pull_request.create" => {
            let head = input["body"]["head"]
                .as_str()
                .unwrap_or_default()
                .to_owned();
            if world.branches.get(&head).map(String::as_str) != input["sha"].as_str() {
                return failure(id, 403, "forbidden", "the head is not at the pinned sha");
            }
            world.pull = Some(Pull {
                head,
                base: input["body"]["base"]
                    .as_str()
                    .unwrap_or_default()
                    .to_owned(),
                state: "open",
                merged: false,
            });
            success(id, 201, world.view().expect("opened"))
        }
        "pull_request.get" if input["pull_number"] == NUMBER => match world.view() {
            Some(view) => success(id, 200, view),
            None => failure(id, 404, "not_found", "no such pull request"),
        },
        "check_runs.list" => success(
            id,
            200,
            json!({"total_count": 1, "check_runs": [{"status": "completed", "conclusion": "success"}]}),
        ),
        "commit.status" => success(
            id,
            200,
            json!({"state": "pending", "total_count": 0, "statuses": []}),
        ),
        "pull_request.merge" if input["pull_number"] == NUMBER => {
            let behind = world.behind();
            let Some(pull) = world.pull.as_mut() else {
                return failure(id, 404, "not_found", "no such pull request");
            };
            if world.branches.get(&pull.head).map(String::as_str) != input["body"]["sha"].as_str() {
                return failure(id, 409, "conflict", "head branch was modified");
            }
            // A strict "up to date" rule refuses the merge; the served instance reports the
            // refusal as an outcome it does not know.
            if behind {
                return failure(
                    id,
                    502,
                    "outcome_unknown",
                    "the provider refused: the head branch is not up to date with the base branch",
                );
            }
            pull.state = "closed";
            pull.merged = true;
            success(
                id,
                200,
                json!({"sha": "a-merge-commit", "merged": true, "message": "merged"}),
            )
        }
        _ => failure(id, 404, "not_found", "no such operation"),
    }
}

/// The stand-in's answer to one request.
fn answer(method: &str, path: &str, body: &Value, world: &mut World) -> (u16, Value) {
    match (method, path) {
        ("GET", DESCRIBE) => (
            200,
            json!({
                "version": "v1alpha1",
                "instance": INSTANCE,
                "adapter": "catalog",
                "revision": REVISION,
                "operations": OPERATIONS.iter().map(|id| json!({
                    "id": id,
                    "description": "",
                    "contract": "operations/v1alpha1",
                    "profile": "github.app",
                    "input_schema": {},
                    "output_schema": {},
                })).collect::<Vec<_>>(),
                "configuration_schema": {"type": "object"},
            }),
        ),
        ("POST", INVOKE) => dispatch(body, world),
        _ => (
            404,
            json!({"code": "not_found", "message": "no such route"}),
        ),
    }
}

/// Reads one request from `stream`, records its body in `seen` and answers it from `world`.
fn serve(stream: TcpStream, seen: &Mutex<Vec<Value>>, world: &Mutex<World>) {
    let mut reader = BufReader::new(stream.try_clone().expect("the stream"));
    let mut line = String::new();
    if reader.read_line(&mut line).is_err() {
        return;
    }
    let mut words = line.split_whitespace();
    let method = words.next().unwrap_or_default().to_owned();
    let path = words.next().unwrap_or_default().to_owned();
    let mut length = 0;
    loop {
        let mut header = String::new();
        if reader.read_line(&mut header).is_err() {
            return;
        }
        let header = header.trim_end();
        if header.is_empty() {
            break;
        }
        if let Some((name, value)) = header.split_once(':')
            && name.trim().eq_ignore_ascii_case("content-length")
        {
            length = value.trim().parse().unwrap_or(0);
        }
    }
    let mut bytes = vec![0; length];
    if reader.read_exact(&mut bytes).is_err() {
        return;
    }
    let body: Value = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
    let (status, answer) = answer(&method, &path, &body, &mut world.lock().expect("the world"));
    seen.lock().expect("the log").push(body);
    let text = answer.to_string();
    let mut stream = stream;
    let _ = write!(
        stream,
        "HTTP/1.1 {status} Answer\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{text}",
        text.len()
    );
}

/// The stand-in of a served Connectors instance, listening on 127.0.0.1 until the process ends.
struct StandIn {
    url: String,
    seen: Arc<Mutex<Vec<Value>>>,
    world: Arc<Mutex<World>>,
}

impl StandIn {
    fn start(world: World) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("a loopback port");
        let url = format!("http://{}", listener.local_addr().expect("an address"));
        let seen = Arc::new(Mutex::new(Vec::new()));
        let world = Arc::new(Mutex::new(world));
        let (log, held) = (seen.clone(), world.clone());
        std::thread::spawn(move || {
            for stream in listener.incoming().flatten() {
                serve(stream, &log, &held);
            }
        });
        Self { url, seen, world }
    }

    /// The input of every invocation of `operation`, in order.
    fn inputs_of(&self, operation: &str) -> Vec<Value> {
        self.seen
            .lock()
            .expect("the log")
            .iter()
            .filter(|body| body["operation"] == operation)
            .map(|body| body["input"].clone())
            .collect()
    }
}

/// The double of the forge's runner: takes a `git push` of `<revision>:refs/heads/<branch>` into
/// the stand-in's branches, as GitHub would, and lists a branch the stand-in holds to a
/// `git ls-remote` of it; every other `git` line succeeds. The commits pushed are the tree's.
#[derive(Clone)]
struct Pushes {
    lines: Rc<std::cell::RefCell<Vec<Vec<String>>>>,
    world: Arc<Mutex<World>>,
}

impl Runner for Pushes {
    fn run(&self, tree: &Path, argv: &[String]) -> anyhow::Result<Exit> {
        self.run_env(tree, argv, &[])
    }

    fn run_env(
        &self,
        _tree: &Path,
        argv: &[String],
        _env: &[(&str, &str)],
    ) -> anyhow::Result<Exit> {
        self.lines.borrow_mut().push(argv.to_vec());
        assert_eq!(argv.first().map(String::as_str), Some("git"), "{argv:?}");
        if argv.iter().any(|word| word == "push") {
            let refspec = argv
                .iter()
                .find(|word| word.contains(":refs/heads/"))
                .ok_or_else(|| anyhow!("a push with no refspec: {argv:?}"))?;
            let (revision, branch) = refspec.split_once(":refs/heads/").expect("a refspec");
            self.world
                .lock()
                .expect("the world")
                .branches
                .insert(branch.to_owned(), revision.to_owned());
        }
        let mut stdout = String::new();
        if argv.iter().any(|word| word == "ls-remote") {
            let reference = argv.last().expect("a reference");
            let branch = reference.strip_prefix("refs/heads/").expect("a branch");
            if let Some(sha) = self.world.lock().expect("the world").branches.get(branch) {
                stdout = format!("{sha}\t{reference}\n");
            }
        }
        Ok(Exit {
            code: Some(0),
            stdout,
            stderr: String::new(),
        })
    }
}

/// A tree at `<scratch>/tree` holding the seed files on `main`, `origin` naming `owner/repo` on
/// a host that does not exist: the tree and `main`'s commit.
fn github_tree(scratch: &Scratch) -> (PathBuf, String) {
    let tree = scratch.join("tree");
    fs::create_dir_all(&tree).expect("a directory");
    seed_files(&tree);
    git(&tree, &["init", "-q", "-b", "main"]);
    git(&tree, &["add", "-A"]);
    git(&tree, &["commit", "-q", "-m", "start"]);
    git(
        &tree,
        &[
            "remote",
            "add",
            "origin",
            "https://github.example/owner/repo.git",
        ],
    );
    let main = git(&tree, &["rev-parse", "HEAD"]);
    (tree, main)
}

/// The forge connectors over the stand-in, for `tree`.
fn github_forge(
    scratch: &Scratch,
    tree: &Path,
    stand_in: &StandIn,
    pushes: &Pushes,
) -> ConnectorsForge {
    let path = scratch.join("run.yaml");
    fs::write(
        &path,
        format!(
            "forge: connectors\nforge_instance: {INSTANCE}\nforge_endpoint: {}\nscan: none\n",
            stand_in.url
        ),
    )
    .expect("the run file");
    let config = RunConfig::read(&path).expect("the run file reads");
    ConnectorsForge::select(
        tree,
        config.forge.as_deref(),
        &config.forge_settings(),
        Scan::None { declared_in: path },
        Box::new(pushes.clone()),
    )
    .expect("the forge is selected")
    .expect("a forge")
}

/// The github route: `main` moves after the pull request opens, the pull read says `behind`
/// until the head holds `main`'s tip, and a merge of a head behind is answered `outcome_unknown`.
/// takt merges the tip into the head as its own commit, records it, pushes it and merges at that
/// head: the merge names takt's merge commit, whose parents are the revision and the tip.
#[test]
fn the_github_route_brings_a_pull_request_behind_up_to_date_and_merges_it() {
    let scratch = Scratch::new("github-behind");
    let (tree, main) = github_tree(&scratch);
    let moved = side_commit(&tree, "upstream", "docs/news.md", "news\n");
    let stand_in = StandIn::start(World {
        repo: tree.clone(),
        branches: BTreeMap::from([("main".to_owned(), main)]),
        pull: None,
        mergeable_state: None,
    });
    let pushes = Pushes {
        lines: Rc::default(),
        world: stand_in.world.clone(),
    };
    let forge: Rc<dyn Forge> = Rc::new(github_forge(&scratch, &tree, &stand_in, &pushes));
    let validators = validators(forge);
    let (mut run, revision) = opened(&tree, "github", "a guide, changed\n", &validators);
    let dir = run.case.dir.clone();
    let branch = run.case.branch.clone();
    // `main` moves on the host before the run reaches the pull request.
    stand_in
        .world
        .lock()
        .expect("the world")
        .branches
        .insert("main".to_owned(), moved.clone());

    let mut out = Vec::new();
    let stop = run.run(&mut out);
    let out = String::from_utf8_lossy(&out).into_owned();
    drop(run);
    assert_eq!(
        stop.as_ref().ok(),
        Some(&Stop::Completed("accepted".to_owned())),
        "{:?}\n{out}",
        stop.as_ref().map_err(|error| format!("{error:#}"))
    );
    let updates = base_updates(&dir);
    assert_eq!(updates.len(), 1, "{updates:?}\n{out}");
    let merge = updates[0]["merge"].as_str().expect("the merge").to_owned();
    assert_eq!(updates[0]["previous_head"], revision);
    assert_eq!(updates[0]["base_tip"], moved.as_str());
    assert_eq!(parents(&tree, &merge), [revision.clone(), moved.clone()]);
    assert_eq!(
        stand_in.inputs_of("pull_request.merge").last(),
        Some(
            &json!({"owner": "owner", "repo": "repo", "pull_number": NUMBER, "body": {"sha": merge}})
        ),
        "the merge is at takt's merge commit"
    );
    assert!(
        pushes
            .lines
            .borrow()
            .iter()
            .any(|argv| argv.contains(&format!("{merge}:refs/heads/{branch}"))),
        "takt's merge commit was pushed as the case's branch"
    );
    let merged = records_of(&dir, "merge_observation");
    assert_eq!(merged.len(), 1, "{out}");
    assert_eq!(merged[0]["result"], "merged", "{out}");
}

/// The github route's read of whether a pull request is behind: `mergeable_state` `behind` is
/// behind, any other state GitHub reports is not, and `unknown` (GitHub still computing) or no
/// state at all is not known.
#[test]
fn the_github_route_reads_behind_from_mergeable_state() {
    let scratch = Scratch::new("github-read");
    let (tree, main) = github_tree(&scratch);
    let stand_in = StandIn::start(World {
        repo: tree.clone(),
        branches: BTreeMap::from([
            ("main".to_owned(), main.clone()),
            ("takt/read".to_owned(), main),
        ]),
        pull: Some(Pull {
            head: "takt/read".to_owned(),
            base: "main".to_owned(),
            state: "open",
            merged: false,
        }),
        mergeable_state: None,
    });
    let pushes = Pushes {
        lines: Rc::default(),
        world: stand_in.world.clone(),
    };
    let forge = github_forge(&scratch, &tree, &stand_in, &pushes);
    for (state, behind) in [
        (Some("behind"), Some(true)),
        (Some("clean"), Some(false)),
        (Some("blocked"), Some(false)),
        (Some("unstable"), Some(false)),
        (Some("dirty"), Some(false)),
        (Some("unknown"), None),
    ] {
        stand_in.world.lock().expect("the world").mergeable_state = state;
        assert_eq!(
            forge.behind_base(&tree, NUMBER).expect("read"),
            behind,
            "{state:?}"
        );
    }
}
