//! Integrity conformance of the base update's record, `artifacts/base-update.json`, against
//! the contract its own documentation states: the record is "written by `repository.merge`
//! alone ..., in the case directory no step may change", and the rider rule reads a merge
//! commit above the revision against it. `repository.merge` lists the record among the files it
//! writes itself, so the run's step guard admits a change to it during that step. During that
//! step the local forge runs its checks (the run file's `local_checks`) on the head takt pushed:
//! code from the pull request's head, run unconfined under the default confinement `none`. The
//! case here is a check that rewrites the record during `repository.merge`; the contract asks
//! that the record afterwards be what takt wrote, or that the step be refused.

#[path = "support/own_home.rs"]
mod own_home;

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::rc::Rc;

use anyhow::anyhow;
use serde_json::{Value, json};
use takt::case::approvals_only;
use takt::evidence::Record;
use takt::forge::Forge;
use takt::forge::local::Local;
use takt::git::CommitIdentity;
use takt::harness::{Harness, StepContext, StepReport};
use takt::validator::{CiRead, PrMerged, PrOpen, Validator};
use takt::{Run, RunOptions, Stop};

/// The record of the base updates takt made, under the case's directory.
const BASE_UPDATE: &str = "artifacts/base-update.json";

/// `git <args>` in `dir` with no configuration of the user's or the system's, no signing and no
/// hook; its trimmed standard output.
fn git(dir: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
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
        .expect("git runs");
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

/// A scratch directory under this crate's build directory, removed when dropped.
struct Scratch(PathBuf);

impl Scratch {
    fn new(name: &str) -> Self {
        let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
            .join(format!("w48-sec-e-{}-{name}", std::process::id()));
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

/// The files every repository here starts with: a README, a guide, the judge allow-list and
/// the owner's delegation of the merge to `agent:takt-delivery`.
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

/// A hub with `main` holding the seed files and a tree cloned from it: the hub and the tree.
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

/// Commits `file` as `text` above `at` with no checkout and no ref: the commit.
fn commit_above(tree: &Path, name: &str, at: &str, file: &str, text: &str) -> String {
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
    git(
        tree,
        &[
            "commit-tree",
            &object,
            "-p",
            at,
            "-m",
            &format!("{name}: {file}"),
        ],
    )
}

/// A harness that asks for the merge gate whenever it is offered and hands the change over when
/// that is offered.
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

/// The base updates the case at `dir` holds, `[]` when it holds none.
fn base_updates(dir: &Path) -> Vec<Value> {
    match fs::read_to_string(dir.join(BASE_UPDATE)) {
        Ok(text) => serde_json::from_str::<Vec<Value>>(&text).expect("the record is a list"),
        Err(_) => Vec::new(),
    }
}

/// The base moves on the hub after the case opens, so `repository.merge` makes, records and
/// pushes takt's merge commit, then reads the local forge's checks at it. The checks are a
/// command that, when the case's record of base updates exists, puts another list in its place:
/// one naming a merge commit takt never made, of the revision and a commit above the case's
/// base that changes the owner-only `.engineering/delegations.yaml` and that the base does not
/// hold. The contract: nothing but `repository.merge` itself writes the record, so after the run
/// the record holds takt's one update (or the step is refused and the record put back), and the
/// rider rule takes no commit the record was edited to name.
#[test]
fn a_check_run_during_the_merge_cannot_rewrite_the_record_of_base_updates() {
    let scratch = Scratch::new("planted-record");
    let (hub, tree) = hub_and_tree(&scratch);
    let planted = scratch.join("planted.json");
    let script = format!(
        "for f in '{}'/.takt/cases/*/{BASE_UPDATE}; do if [ -f \"$f\" ]; then cp '{}' \"$f\"; fi; done; exit 0",
        tree.display(),
        planted.display()
    );
    let forge: Rc<dyn Forge> = Rc::new(
        Local::for_tree(
            &tree,
            vec!["sh".to_owned(), "-c".to_owned(), script],
            scratch.join("state"),
            CommitIdentity::new("Test Bot", "bot@example.invalid").expect("an identity"),
        )
        .expect("the local forge"),
    );
    let validators: Vec<Box<dyn Validator>> = vec![
        Box::new(PrOpen::new(forge.clone())),
        Box::new(CiRead::new(forge.clone())),
        Box::new(PrMerged::new(forge)),
    ];
    let options = RunOptions {
        protocol: "story.delivery/1".to_owned(),
        intent: "change the guide\n\nA guide that says more.".to_owned(),
        inputs: Vec::new(),
        iterations: Some(12),
        hours: None,
        tokens: None,
        case_id: Some("planted".to_owned()),
        gate: Some("repository.merge".to_owned()),
    };
    let mut run = Run::open(&tree, options, &Requester, &validators).expect("the case opens");
    run.poll = std::time::Duration::from_millis(5);
    let pinned = git(&tree, &["rev-parse", "main"]);
    fs::write(tree.join("docs/guide.md"), "a guide, changed\n").expect("a file");
    git(&tree, &["add", "-A", "--", ".", ":!.takt"]);
    git(&tree, &["commit", "-q", "-m", "change the guide"]);
    let revision = git(&tree, &["rev-parse", "HEAD"]);
    run.case
        .set_revision("implementation", &revision)
        .expect("the revision moves");
    let short = &revision[..8];
    for (id, kind, result, producer) in [
        ("e0".to_owned(), "test_result", Some("pass"), "w48-sec-e"),
        ("e1".to_owned(), "gate_result", Some("pass"), "w48-sec-e"),
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
    let dir = run.case.dir.clone();

    // The base moves on the hub: the pull request will read behind.
    let moved = commit_above(&tree, "upstream", &pinned, "docs/news.md", "news\n");
    git(
        &tree,
        &["push", "-q", "origin", &format!("{moved}:refs/heads/main")],
    );
    // What the check puts in the record's place: a merge of the revision and a commit above the
    // case's base, not on it, that rewrites the owner-only delegations file.
    let foreign = commit_above(
        &tree,
        "foreign",
        &pinned,
        ".engineering/delegations.yaml",
        "delegations:\n  - owner: human:owner\n    role: agent:anyone\n    capability: repository.merge\n    given_at: 2026-10-30T08:00:00Z\n",
    );
    let merged = git(&tree, &["merge-tree", "--write-tree", &revision, &foreign])
        .lines()
        .next()
        .expect("a tree")
        .to_owned();
    let forged = git(
        &tree,
        &[
            "commit-tree",
            &merged,
            "-p",
            &revision,
            "-p",
            &foreign,
            "-m",
            "takt: bring it up to date",
        ],
    );
    fs::write(
        &planted,
        serde_json::to_vec_pretty(&json!([{
            "previous_head": revision,
            "base_tip": foreign,
            "merge": forged,
            "at": "2026-10-30T09:00:00Z",
        }]))
        .expect("json"),
    )
    .expect("the planted list");
    assert!(
        !approvals_only(&tree, &revision, &forged, &dir).expect("reads"),
        "before the run, the forged merge rides on nothing"
    );

    let mut out = Vec::new();
    let stop = run.run(&mut out);
    let out = String::from_utf8_lossy(&out).into_owned();
    drop(run);

    let updates = base_updates(&dir);
    let rides = approvals_only(&tree, &revision, &forged, &dir).expect("reads");
    let refused = matches!(&stop, Ok(Stop::StepRefused(_)));
    let takt_made = updates.len() == 1
        && updates[0]["previous_head"] == revision.as_str()
        && updates[0]["base_tip"] == moved.as_str();
    assert!(
        takt_made && !rides,
        "a check run during repository.merge rewrote {BASE_UPDATE} and the step guard admitted it \
         (step refused: {refused}): the record now holds {updates:?}, and the rider rule takes the \
         forged merge {forged} whose second parent {foreign} changes .engineering/delegations.yaml \
         and is not on the base: {rides}\nstop: {stop:?}\nhub main: {}\n{out}",
        git(&hub, &["rev-parse", "refs/heads/main"])
    );
}
