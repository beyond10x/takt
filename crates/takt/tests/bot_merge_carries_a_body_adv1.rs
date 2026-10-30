//! Adversary cases for story:bot-merge-commit-has-a-body. The unit's own case merges a head that
//! is the granted revision; a story's delivery merges a head that carries the approval's record
//! above it (`takt approve` commits it on the case's branch), and its title is the story's.

#[path = "support/own_home.rs"]
mod own_home;

use std::cell::Cell;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::rc::Rc;

use anyhow::anyhow;
use takt::case::{Branch, CASES_DIR, grant};
use takt::evidence::Record;
use takt::forge::{BotRoute, Checks, Exit, Forge, Memory, Runner};
use takt::frontier::{Aep, Story, Unit, approve};
use takt::harness::{Harness, StepContext, StepReport};
use takt::validator::{CiRead, PrMerged, PrOpen, Validator};
use takt::{Run, RunOptions, Stop};

/// The pull request the stand-in forge holds open for the case.
const NUMBER: u64 = 18;

const FIXTURES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures");

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
        let _ = fs::remove_dir_all(&self.0);
    }
}

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
    String::from_utf8_lossy(&output.stdout).trim().to_owned()
}

fn scratch(name: &str) -> Scratch {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("bot-merge-adv1-{}-{name}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("a scratch directory");
    Scratch(dir)
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

/// The stand-in for `b10x-gates` under the bot route, as the unit's own case has it, and
/// answering the reads of the checks on the head with one passing check run and no status.
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
        if line.starts_with("b10x-gates api --method GET --path ") {
            let merged = self.merged.get();
            let response = if line.contains(&format!("--path /repos/owner/repo/pulls/{NUMBER} ")) {
                serde_json::json!({
                    "state": if merged { "closed" } else { "open" },
                    "merged": merged,
                    "head": { "ref": self.branch, "sha": self.head },
                    "base": { "ref": "main" },
                })
            } else if line.contains("/check-runs") {
                serde_json::json!({
                    "total_count": 1,
                    "check_runs": [{ "status": "completed", "conclusion": "success" }],
                })
            } else if line.contains("/status") {
                serde_json::json!({ "state": "pending", "total_count": 0, "statuses": [] })
            } else {
                return Err(anyhow!("the stand-in reads no {line}"));
            };
            let output = argv
                .iter()
                .position(|a| a == "--output")
                .map(|i| &argv[i + 1])
                .expect("--output");
            fs::write(output, response.to_string())?;
            return Ok(ok(String::new()));
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

/// Merges the case at `case_dir` in `tree` at `revision` through the bot route, the pull
/// request's head `head`; the merge request it sent.
fn merge_through_the_bot(
    tree: &Path,
    case_dir: &Path,
    id: &str,
    revision: &str,
    head: &str,
    tip: String,
) -> serde_json::Value {
    let mut branch = Branch::read(case_dir).expect("the branch file");
    branch.pull_request = Some(NUMBER);
    branch.pushed = Some(head.to_owned());
    branch.write(case_dir).expect("the branch file is written");
    let stand = Stand {
        branch: Branch::name_of(id),
        head: head.to_owned(),
        tip,
        merged: Cell::new(false),
    };
    let forge = Rc::new(BotRoute::new("owner/repo", None, Box::new(stand)));
    let observation = PrMerged::new(forge)
        .observe(tree, revision, 40)
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
    serde_json::from_str(&fs::read_to_string(&request).expect("the merge request")).expect("json")
}

/// Requests the merge gate as soon as it is offered, changing nothing.
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

fn delivery() -> RunOptions {
    RunOptions {
        protocol: "story.delivery/1".to_owned(),
        intent: String::new(),
        inputs: Vec::new(),
        iterations: Some(8),
        hours: None,
        tokens: None,
        case_id: None,
        gate: Some("repository.merge".to_owned()),
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

/// A story delivered as takt delivers one: `story:ready-one` taken from the real store, its
/// work committed, run to the merge gate, approved with `takt approve` (whose record is
/// committed on the case's branch above the granted revision), then merged through the bot
/// route at that head. The body names the granted revision on a line of its own template, and
/// that line is cut: every such merge commit ends a line in `…` mid-sentence.
#[test]
fn a_story_merge_above_its_approval_has_no_line_cut_short() {
    let repo = scratch("riders");
    copy_dir(&Path::new(FIXTURES).join("one-ready"), &repo);
    fs::write(repo.join("check.txt"), "broken\n").expect("a file");
    git(&repo, &["init", "-q", "-b", "main"]);
    git(&repo, &["add", "-A"]);
    git(&repo, &["commit", "-q", "-m", "start"]);
    let tip = git(&repo, &["rev-parse", "main"]);

    let memory = Rc::new(Memory {
        checks: Checks::Pass,
        ..Memory::default()
    });
    let forge: Rc<dyn Forge> = memory.clone();
    let validators: Vec<Box<dyn Validator>> = vec![
        Box::new(PrOpen::new(forge.clone())),
        Box::new(CiRead::new(forge.clone())),
        Box::new(PrMerged::new(forge)),
    ];
    let mut out = Vec::new();
    let opened = Run::open_next(
        &repo,
        delivery(),
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
    git(&repo, &["commit", "-q", "-am", "the work"]);
    let worked = git(&repo, &["rev-parse", "HEAD"]);
    let mut run =
        Run::resume(&repo, delivery(), &id, &Requester, &validators).expect("the case resumes");
    for evidence in [
        record("e1", "test_result", Some("pass"), &worked, "adv"),
        record("e2", "gate_result", Some("pass"), &worked, "adv"),
        record(
            &format!("handover-{}", &worked[..8]),
            "handover",
            None,
            &worked,
            takt::run::CHOICE_PRODUCER,
        ),
        record(
            &format!("change-record-{}", &worked[..8]),
            "change_record",
            Some("ahead"),
            &worked,
            "takt.change-read",
        ),
    ] {
        run.case.add_evidence(&evidence).expect("written");
    }
    let stop = run.run(&mut out).expect("the run stops without an error");
    assert_eq!(
        stop,
        Stop::ApprovalRequired(vec!["repository.merge".to_owned()]),
        "{}",
        String::from_utf8_lossy(&out)
    );
    drop(run);
    approve(
        &repo,
        &Aep::default(),
        &id,
        "repository.merge",
        "human:operator",
    )
    .expect("the approval is given");
    let approved = git(&repo, &["rev-parse", "HEAD"]);
    assert_ne!(approved, worked, "the approval is committed above the work");

    let case_dir = repo.join(CASES_DIR).join(&id);
    let sent = merge_through_the_bot(&repo, &case_dir, &id, &worked, &approved, tip);
    println!("merge request: {sent:#}");
    let body = sent["commit_message"].as_str().unwrap_or_default();
    assert!(
        body.contains(&approved),
        "the body names the merged head: {body}"
    );
    let cut: Vec<&str> = body.lines().filter(|line| line.ends_with('…')).collect();
    assert!(
        cut.is_empty(),
        "story:bot-merge-commit-has-a-body: a line of the merge commit's body takt wrote itself is cut short: {cut:?}\n{body}"
    );
}

/// The unit's own story title, as the planning store holds it (91 characters): a story whose
/// title is longer than 88 characters is named in the merge commit's title only up to a `…`,
/// and the title holds neither the whole title nor the story's id.
#[test]
fn the_merge_title_names_a_story_with_a_long_title() {
    const STORY: &str = "story:bot-merge-commit-has-a-body";
    const TITLE: &str = "takt's merge through the bot forge has a title and a body naming the story and its evidence";
    let tree = scratch("long-title");
    fs::write(tree.join("README.md"), "a story\n").expect("a file");
    git(&tree, &["init", "-q", "-b", "main"]);
    git(&tree, &["add", "-A"]);
    git(&tree, &["commit", "-q", "-m", "start"]);
    let tip = git(&tree, &["rev-parse", "main"]);
    let id = "long-title";
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
    let mut run = Run::open_unit(&tree, options, Unit::Story(story), &Requester, &none)
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
        run.case.add_evidence(&evidence).expect("written");
    }
    let seq = run.case.evaluations().expect("counts") + 1;
    run.case.evaluate(seq).expect("evaluates");
    let case_dir = run.case.dir.clone();
    drop(run);
    grant(&tree, id, "repository.merge", "human:operator").expect("the grant is written");
    let sent = merge_through_the_bot(&tree, &case_dir, id, &revision, &revision, tip);
    let title = sent["commit_title"].as_str().unwrap_or_default();
    assert!(
        title.contains(TITLE) || title.contains(STORY),
        "story:bot-merge-commit-has-a-body: the merge commit's title names neither the story's title nor its id: {title}"
    );
}
