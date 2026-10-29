//! story:bot-merge-commit-has-a-body: the merge commit takt makes through the bot forge has a
//! title and a body. `repository.merge` over the bot route, every line it runs answered by a
//! stand-in runner, sends a merge request whose `commit_title` names the pull request and the
//! story and whose `commit_message` names the story, the merged revision and the evidence the
//! merge rests on (the `test_result`, `gate_result` and `ci_result` records at the revision and
//! the grant of `repository.merge`), each line under 100 characters, instead of the forge's
//! default message.

#[path = "support/own_home.rs"]
mod own_home;

use std::cell::Cell;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::rc::Rc;

use anyhow::anyhow;
use takt::case::{Branch, grant};
use takt::evidence::Record;
use takt::forge::{BotRoute, Exit, Runner};
use takt::frontier::{Story, Unit};
use takt::harness::{Harness, StepContext, StepReport};
use takt::validator::{PrMerged, Validator};
use takt::{Run, RunOptions};

/// The pull request the stand-in forge holds open for the case.
const NUMBER: u64 = 18;

/// The story the case is opened for.
const STORY: &str = "story:probe-merge-has-a-body";

/// Its title.
const TITLE: &str = "The probe merge has a body";

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

fn git(tree: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .args(["-c", "user.name=t", "-c", "user.email=t@localhost"])
        .args(args)
        .current_dir(tree)
        .output()
        .expect("git runs");
    assert!(output.status.success(), "git {args:?}");
    String::from_utf8_lossy(&output.stdout).trim().to_owned()
}

/// A harness no step here reaches.
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

/// The stand-in for `gh` and `b10x-gates` under the bot route: `gh pr view` reports pull request
/// [`NUMBER`] open (merged once the merge was asked for) with the head `branch` at `head`
/// against `main`; the dry-run fetch reports `main` at `tip` and the object fetch succeeds; the
/// merge API call answers `merged: true`. Any other line fails.
struct Stand {
    branch: String,
    head: String,
    tip: String,
    merged: Cell<bool>,
}

impl Runner for Stand {
    fn run_env(&self, tree: &Path, argv: &[String], _: &[(&str, &str)]) -> anyhow::Result<Exit> {
        self.run(tree, argv)
    }

    fn run(&self, _: &Path, argv: &[String]) -> anyhow::Result<Exit> {
        let line = argv.join(" ");
        let ok = |stdout: String| Exit {
            code: Some(0),
            stdout,
            stderr: String::new(),
        };
        if line.starts_with(&format!("gh pr view {NUMBER} ")) {
            let state = if self.merged.get() { "MERGED" } else { "OPEN" };
            let view = serde_json::json!({
                "state": state,
                "mergedAt": null,
                "headRefName": self.branch,
                "headRefOid": self.head,
                "baseRefName": "main",
            });
            return Ok(ok(view.to_string()));
        }
        if line.contains("--dry-run") {
            return Ok(ok(format!(
                "* {} {} refs/remotes/origin/main\n",
                "0".repeat(40),
                self.tip
            )));
        }
        if line.contains("--no-write-fetch-head") {
            return Ok(ok(String::new()));
        }
        if argv.get(1).map(String::as_str) == Some("api")
            && line.contains(&format!("--path /repos/owner/repo/pulls/{NUMBER}/merge"))
        {
            let output = argv
                .iter()
                .position(|a| a == "--output")
                .map(|i| &argv[i + 1])
                .expect("--output");
            fs::write(output, r#"{"merged":true,"message":"merged"}"#)?;
            self.merged.set(true);
            return Ok(ok(String::new()));
        }
        Err(anyhow!("the stand-in runs no line {line}"))
    }
}

fn record(id: &str, kind: &str, result: Option<&str>, revision: &str, producer: &str) -> Record {
    Record::new(
        id,
        kind,
        result.map(str::to_owned),
        "implementation",
        revision,
        producer,
    )
}

/// Scenario `bot-merge/carries-a-body`: the merge request the bot forge sends for the case's
/// pull request holds a `commit_title` naming the pull request and the story and a
/// `commit_message` naming the story, the merged revision, the evidence records at it and the
/// grant, each line under 100 characters, beside the head to match.
#[test]
fn the_bot_merge_request_carries_a_title_and_a_body() {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("bot-merge-carries-a-body-{}", std::process::id()));
    if dir.exists() {
        fs::remove_dir_all(&dir).expect("a stale scratch tree is removed");
    }
    fs::create_dir_all(&dir).expect("scratch dir");
    let tree = Scratch(dir);
    fs::write(tree.join("README.md"), "a story\n").expect("a file");
    git(&tree, &["init", "-q", "-b", "main"]);
    git(&tree, &["add", "-A"]);
    git(&tree, &["commit", "-q", "-m", "start"]);
    let tip = git(&tree, &["rev-parse", "main"]);

    let id = "merge-body";
    let options = RunOptions {
        protocol: "story.delivery/1".to_owned(),
        intent: String::new(),
        inputs: Vec::new(),
        iterations: Some(1),
        hours: None,
        tokens: None,
        case_id: Some(id.to_owned()),
        gate: Some("repository.merge".to_owned()),
    };
    let story = Story {
        id: STORY.to_owned(),
        title: TITLE.to_owned(),
        body: "The body of the story.".to_owned(),
    };
    let none: Vec<Box<dyn Validator>> = Vec::new();
    let mut run = Run::open_unit(&tree, options, Unit::Story(story), &NoHarness, &none)
        .expect("the case opens");
    fs::write(tree.join("implemented.txt"), "done\n").expect("a file");
    git(&tree, &["add", "implemented.txt"]);
    git(&tree, &["commit", "-q", "-m", "implemented by the test"]);
    let revision = git(&tree, &["rev-parse", "HEAD"]);
    run.case
        .set_revision("implementation", &revision)
        .expect("the revision moves");
    for evidence in [
        record(
            "tests-1",
            "test_result",
            Some("pass"),
            &revision,
            "takt.tests-run",
        ),
        record(
            "gate-2",
            "gate_result",
            Some("pass"),
            &revision,
            "takt.gate-run",
        ),
        record(
            "pr-3",
            "pull_request",
            Some("open"),
            &revision,
            "takt.pr-open",
        ),
        record("ci-4", "ci_result", Some("pass"), &revision, "takt.ci-read"),
        record(
            "handover-5",
            "handover",
            None,
            &revision,
            takt::run::CHOICE_PRODUCER,
        ),
        record(
            "change-6",
            "change_record",
            Some("ahead"),
            &revision,
            "takt.change-read",
        ),
    ] {
        run.case
            .add_evidence(&evidence)
            .expect("the record is written");
    }
    let seq = run.case.evaluations().expect("counts") + 1;
    run.case.evaluate(seq).expect("evaluates");
    let case_dir = run.case.dir.clone();
    drop(run);
    let mut branch = Branch::read(&case_dir).expect("the branch file");
    branch.pull_request = Some(NUMBER);
    branch.pushed = Some(revision.clone());
    branch.write(&case_dir).expect("the branch file is written");
    grant(&tree, id, "repository.merge", "human:operator").expect("the grant is written");

    let stand = Stand {
        branch: Branch::name_of(id),
        head: revision.clone(),
        tip,
        merged: Cell::new(false),
    };
    let forge = Rc::new(BotRoute::new("owner/repo", None, Box::new(stand)));
    let observation = PrMerged::new(forge)
        .observe(&tree, &revision, 7)
        .expect("repository.merge observes");
    assert_eq!(
        observation
            .records
            .first()
            .and_then(|r| r.result.as_deref()),
        Some("merged"),
        "{}",
        observation.summary
    );

    let request = case_dir.join("artifacts").join(BotRoute::MERGE_REQUEST);
    let sent: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&request).expect("the merge request"))
            .expect("json");
    println!("merge request: {sent:#}");
    assert_eq!(sent["sha"], revision.as_str(), "{sent}");
    assert!(
        sent.get("merge_method").is_none(),
        "the merge method stays the forge's default merge commit: {sent}"
    );
    let title = sent["commit_title"].as_str().unwrap_or_default();
    assert!(
        title.contains(&format!("#{NUMBER}")),
        "the title names the pull request: {sent}"
    );
    assert!(title.contains(TITLE), "the title names the story: {sent}");
    let body = sent["commit_message"].as_str().unwrap_or_default();
    for named in [
        STORY,
        revision.as_str(),
        "test_result",
        "tests-1",
        "gate_result",
        "gate-2",
        "ci_result",
        "ci-4",
        "repository.merge",
    ] {
        assert!(body.contains(named), "the body names {named}: {body}");
    }
    for line in std::iter::once(title).chain(body.lines()) {
        assert!(
            line.chars().count() < 100,
            "a line of 100 characters or more: {line}"
        );
    }
    assert!(
        !body.contains("Co-Authored-By") && !body.contains("Generated with"),
        "{body}"
    );
}
