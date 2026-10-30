//! What a step may not leave behind, and what `repository.merge` does before it pushes takt's
//! riders, whatever the forge: a step that moves or makes a ref, or sets the configuration a
//! push or takt's commit reads, is named and put back; takt commits no new gitlink; and the
//! forge's scan of what the rider push would publish runs before that push, a refusal pushing
//! and merging nothing. A forge double stands in for the host; nothing leaves the machine.

#[path = "support/own_home.rs"]
mod own_home;

use std::cell::{Cell, RefCell};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::rc::Rc;

use anyhow::{anyhow, bail};
use takt::forge::{Checks, Forge, Memory, Outgoing, PullRequest, PullRequestView};
use takt::frontier::{Aep, approve};
use takt::harness::{Harness, StepContext, StepReport};
use takt::validator::{ChangeRead, CiRead, PrMerged, PrOpen, TestsRun, Validator};
use takt::{Run, RunOptions, Stop};

/// The protocol under test, the engineering protocols' built-in.
const PROTOCOL: &str = "story.delivery/1";

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

/// A scratch git tree with one commit on `main` and an `origin` on github.com, under this
/// crate's build directory. Dropping it removes the tree.
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

fn scratch(name: &str) -> Scratch {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("step-guards-{}-{name}", std::process::id()));
    if dir.exists() {
        fs::remove_dir_all(&dir).expect("a stale scratch tree is removed");
    }
    fs::create_dir_all(&dir).expect("scratch dir");
    fs::write(dir.join("README.md"), "a story\n").expect("a file");
    git(&dir, &["init", "-q", "-b", "main"]);
    git(&dir, &["add", "-A"]);
    git(&dir, &["commit", "-q", "-m", "start"]);
    git(
        &dir,
        &[
            "remote",
            "add",
            "origin",
            "https://github.com/owner/repo.git",
        ],
    );
    Scratch(dir)
}

/// A harness the runs below never reach: every step they take is a validator's.
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

/// Commits `path` holding `text` with the message `message`, as a step leaves it.
fn commit(tree: &Path, path: &str, text: &str, message: &str) {
    fs::write(tree.join(path), text).expect("a file");
    git(tree, &["add", "-A", "--", path]);
    git(tree, &["commit", "-q", "-m", message]);
}

/// git's output is read no further than the limit: past it the answer is none.
#[test]
fn git_output_past_the_limit_is_not_read() {
    let tree = scratch("bounded");
    commit(&tree, "work.txt", &"x\n".repeat(100), "work");
    let all = takt::git::run_bounded(&tree, &["log", "--patch"], 1 << 20)
        .expect("git runs")
        .expect("under the limit");
    assert!(all.len() > 64, "{all}");
    assert_eq!(
        takt::git::run_bounded(&tree, &["log", "--patch"], 64).expect("git runs"),
        None
    );
}

/// The fixture stores, one directory each, relative to this crate.
const FIXTURES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures");

/// A forge over [`Memory`] that logs `scan <branch>` for each scan and the pushes as the
/// double does, and refuses a scan while `refuse` is set.
#[derive(Default)]
struct Scanning {
    memory: Memory,
    refuse: Cell<bool>,
    log: RefCell<Vec<String>>,
}

impl Forge for Scanning {
    fn target_tip(&self, tree: &Path, base: &str) -> anyhow::Result<String> {
        self.memory.target_tip(tree, base)
    }

    fn name(&self) -> &str {
        "scanning"
    }

    fn scan_outgoing(&self, _: &Path, _: &Path, outgoing: &Outgoing<'_>) -> anyhow::Result<()> {
        self.log
            .borrow_mut()
            .push(format!("scan {} {}", outgoing.branch, outgoing.head));
        if self.refuse.get() {
            bail!("the scanning double refuses it");
        }
        Ok(())
    }

    fn push(&self, tree: &Path, branch: &str) -> anyhow::Result<()> {
        self.log.borrow_mut().push(format!("push {branch}"));
        self.memory.push(tree, branch)
    }

    fn push_revision(&self, tree: &Path, branch: &str, revision: &str) -> anyhow::Result<()> {
        self.log
            .borrow_mut()
            .push(format!("push {branch} {revision}"));
        self.memory.push_revision(tree, branch, revision)
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
        self.memory.checks(tree, number)
    }

    fn merge(&self, tree: &Path, dir: &Path, number: u64, head: &str) -> anyhow::Result<()> {
        self.log.borrow_mut().push(format!("merge #{number}"));
        self.memory.merge(tree, dir, number, head)
    }
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
        // The work committed is complete: hand it over, changing nothing.
        if context.offered.contains(&"change.handover".to_owned()) {
            return Ok(StepReport {
                action: "change.handover".to_owned(),
                summary: "hand it over".to_owned(),
                tokens: None,
            });
        }
        Err(anyhow!("the gate was not offered"))
    }
}

/// A delivery run's validators over `forge`: tests (`true`), the gate as `TestsRun` would not
/// do, so a gate that always passes, and the forge's three.
fn delivery(forge: &Rc<Scanning>) -> Vec<Box<dyn Validator>> {
    let forge: Rc<dyn Forge> = forge.clone();
    vec![
        Box::new(TestsRun::new("true").expect("a command")),
        Box::new(ChangeRead),
        Box::new(GatePass),
        Box::new(PrOpen::new(forge.clone())),
        Box::new(CiRead::new(forge.clone())),
        Box::new(PrMerged::new(forge)),
    ]
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

    fn observe(
        &self,
        _: &Path,
        revision: &str,
        seq: usize,
    ) -> Result<takt::validator::Observation, anyhow::Error> {
        Ok(takt::validator::Observation {
            summary: "pass".to_owned(),
            records: vec![takt::evidence::Record::new(
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

/// The options of a delivery run of the fixture store's story.
fn delivery_options() -> RunOptions {
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

/// Copies the directory `from` to `to`.
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

/// The fixture store's story taken, worked, opened through `forge`, its checks passing, and
/// granted the merge: the tree, the case id. The next run pushes takt's riders before the merge.
fn granted(name: &str, forge: &Rc<Scanning>) -> (Scratch, String) {
    let tree = scratch(name);
    copy_dir(&Path::new(FIXTURES).join("one-ready"), &tree);
    fs::write(tree.join("check.txt"), "broken\n").expect("a file");
    git(&tree, &["add", "-A"]);
    git(&tree, &["commit", "-q", "-m", "the store"]);
    let validators = delivery(forge);
    let store = Aep::default();
    let mut out = Vec::new();
    let opened = Run::open_next(
        &tree,
        delivery_options(),
        &store,
        &Requester,
        &validators,
        &mut out,
    )
    .expect("the frontier answers")
    .expect("the story is taken");
    let id = opened.case.id.clone();
    drop(opened);
    fs::write(tree.join("check.txt"), "fixed\n").expect("a file");
    git(&tree, &["commit", "-q", "-am", "the work"]);
    let mut run = Run::resume(&tree, delivery_options(), &id, &Requester, &validators)
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
    approve(&tree, &store, &id, "repository.merge", "human:operator")
        .expect("the approval is given");
    (tree, id)
}

/// Runs the granted case once more, past the gate: how it ended and what it printed.
fn merge_run(tree: &Path, id: &str, forge: &Rc<Scanning>) -> (Result<Stop, String>, String) {
    let validators = delivery(forge);
    let store = Aep::default();
    let mut run = Run::resume(tree, delivery_options(), id, &NoHarness, &validators)
        .expect("the case resumes");
    run.record_on(&store);
    run.poll = std::time::Duration::from_millis(5);
    let mut out = Vec::new();
    let stop = run.run(&mut out).map_err(|error| format!("{error:#}"));
    (stop, String::from_utf8_lossy(&out).into_owned())
}

/// `repository.merge` pushes takt's riders above the reviewed head before it merges: the forge
/// scans that push before it, and the merge follows.
#[test]
fn the_rider_push_before_the_merge_is_scanned_first() {
    let forge = Rc::new(Scanning {
        memory: Memory {
            checks: Checks::Pass,
            ..Memory::default()
        },
        ..Scanning::default()
    });
    let (tree, id) = granted("riders", &forge);
    let before = forge.log.borrow().len();
    let (stop, text) = merge_run(&tree, &id, &forge);
    assert_eq!(stop, Ok(Stop::Completed("accepted".to_owned())), "{text}");
    let log = forge.log.borrow()[before..].to_vec();
    let push = log
        .iter()
        .position(|line| line.starts_with("push "))
        .unwrap_or_else(|| panic!("the riders were pushed: {log:#?}"));
    let pushed = log[push].rsplit(' ').next().expect("a revision").to_owned();
    assert!(
        log[..push].contains(&format!("scan takt/{id} {pushed}")),
        "{log:#?}"
    );
    assert!(log.contains(&"merge #1".to_owned()), "{log:#?}");
}

/// A scan that refuses the riders' push: nothing is pushed, nothing merged, no record written.
#[test]
fn a_refused_scan_of_the_rider_push_pushes_and_merges_nothing() {
    let forge = Rc::new(Scanning {
        memory: Memory {
            checks: Checks::Pass,
            ..Memory::default()
        },
        ..Scanning::default()
    });
    let (tree, id) = granted("riders-refused", &forge);
    let before = forge.log.borrow().len();
    forge.refuse.set(true);
    let (stop, text) = merge_run(&tree, &id, &forge);
    let error = stop.expect_err("the scan refused");
    assert!(
        error.contains("the scanning double refuses it"),
        "{error}\n{text}"
    );
    assert!(
        error.contains(&format!(
            "scan takt/{id} before the push through the forge scanning"
        )),
        "not the merge's scan refusal: {error}\n{text}"
    );
    let log = forge.log.borrow()[before..].to_vec();
    assert!(
        !log.iter().any(|line| line.starts_with("push ")),
        "{log:#?}"
    );
    assert!(forge.memory.merged.borrow().is_empty(), "{log:#?}");
    let dir = tree.join(".takt/cases").join(&id);
    assert!(
        !kinds(&dir).contains(&"merge_observation".to_owned()),
        "{:?}",
        kinds(&dir)
    );
}

/// A harness whose step writes a file and moves a tracking ref and makes another: what a push
/// reads as what the remote holds.
struct Retracking;

impl Harness for Retracking {
    fn name(&self) -> &str {
        "retracking"
    }

    fn step(&self, context: &StepContext<'_>) -> Result<StepReport, anyhow::Error> {
        fs::write(context.tree.join("work.txt"), "work\n")?;
        let head = git(context.tree, &["rev-parse", "HEAD"]);
        git(
            context.tree,
            &["update-ref", "refs/remotes/origin/main", &head],
        );
        git(
            context.tree,
            &["update-ref", "refs/remotes/origin/planted", &head],
        );
        Ok(StepReport {
            action: "implementation.edit".to_owned(),
            summary: "acted".to_owned(),
            tokens: None,
        })
    }
}

/// A step may not move or make a tracking ref: the checkpoint guards `refs/remotes/` as it
/// guards branches and tags, so the step is refused, the ref it moved is put back and the one it
/// made removed.
#[test]
fn a_step_that_moves_a_tracking_ref_is_refused_and_the_refs_put_back() {
    let tree = scratch("retracking");
    let start = git(&tree, &["rev-parse", "HEAD"]);
    git(&tree, &["update-ref", "refs/remotes/origin/main", &start]);
    let options = RunOptions {
        protocol: "software.change/1".to_owned(),
        intent: "make the change".to_owned(),
        inputs: vec![
            ("risk".to_owned(), "trivial".to_owned()),
            ("change_kind".to_owned(), "feature".to_owned()),
            ("affects_security_boundary".to_owned(), "false".to_owned()),
        ],
        iterations: Some(4),
        hours: None,
        tokens: None,
        case_id: Some("retracking".to_owned()),
        gate: Some("repository.merge".to_owned()),
    };
    let validators: Vec<Box<dyn Validator>> =
        vec![Box::new(TestsRun::new("true").expect("a command"))];
    let mut run = Run::open(&tree, options, &Retracking, &validators).expect("the case opens");
    let mut out = Vec::new();
    let stop = run.run(&mut out).expect("the run stops");
    let text = String::from_utf8_lossy(&out);
    assert!(
        matches!(&stop, Stop::StepRefused(why) if why.contains("refs/remotes/origin/")),
        "{stop:?}\n{text}"
    );
    assert_eq!(
        git(
            &tree,
            &[
                "for-each-ref",
                "--format=%(refname) %(objectname)",
                "refs/remotes/"
            ]
        ),
        format!("refs/remotes/origin/main {start}"),
        "{text}"
    );
}

/// takt does not commit a nested repository a step left as a gitlink the case's `HEAD` does not
/// have: the commit is refused naming the path, and `HEAD` stays.
#[test]
fn takt_refuses_to_commit_a_new_gitlink() {
    let tree = scratch("new-gitlink");
    git(&tree, &["checkout", "-q", "-b", "takt/case"]);
    let before = git(&tree, &["rev-parse", "HEAD"]);
    let sub = tree.join("nested");
    fs::create_dir_all(&sub).expect("a nested directory");
    git(&sub, &["init", "-q", "-b", "main"]);
    fs::write(sub.join("f"), "inner\n").expect("a file");
    git(&sub, &["add", "-A"]);
    git(&sub, &["commit", "-q", "-m", "inner"]);
    fs::write(tree.join("work.txt"), "work\n").expect("a file");
    let error = takt::git::commit_paths(
        &tree,
        &takt::git::CommitIdentity::takt(),
        &["nested".to_owned(), "work.txt".to_owned()],
        "implementation.edit: work",
    )
    .expect_err("a new gitlink is refused");
    let error = format!("{error:#}");
    assert!(error.contains("nested"), "{error}");
    assert!(error.contains("gitlink"), "{error}");
    assert_eq!(git(&tree, &["rev-parse", "HEAD"]), before);
}

/// A step may not set the configuration a push or takt's commit reads: `i18n.*` (the encoding a
/// commit records), `push.*` (what a push sends besides the refspec), `http.*` (where it goes).
/// The checkpoint puts each back and names it.
#[test]
fn a_step_that_sets_push_http_or_i18n_configuration_is_put_back() {
    for (n, (key, value)) in [
        ("i18n.commitEncoding", "UTF-16LE"),
        ("push.followTags", "true"),
        ("http.proxy", "http://127.0.0.1:9"),
    ]
    .into_iter()
    .enumerate()
    {
        let tree = scratch(&format!("config-{n}"));
        let checkpoint = takt::git::Checkpoint::take(&tree).expect("a checkpoint");
        git(&tree, &["config", key, value]);
        let back = checkpoint.put_back_git_dir(&tree);
        assert!(!back.touched.is_empty(), "{key}: nothing put back");
        assert!(back.failed.is_empty(), "{key}: {:?}", back.failed);
        let config = fs::read_to_string(tree.join(".git/config")).expect("the config");
        let name = key.rsplit('.').next().expect("a name");
        assert!(!config.contains(name), "{key} stayed:\n{config}");
    }
}

/// A step may not make a ref anywhere: outside `refs/heads`, `refs/tags` and `refs/remotes`
/// too (a ref named like a tracking ref, a note, a replacement), each is a change the
/// checkpoint names.
#[test]
fn a_step_that_makes_any_ref_is_named() {
    let tree = scratch("any-ref");
    let head = git(&tree, &["rev-parse", "HEAD"]);
    let checkpoint = takt::git::Checkpoint::take(&tree).expect("a checkpoint");
    for name in [
        "refs/refs/remotes/origin/main",
        "refs/notes/commits",
        "refs/replace/x",
    ] {
        git(&tree, &["update-ref", name, &head]);
    }
    let refs = checkpoint.change(&tree).expect("the change").refs;
    for name in [
        "refs/refs/remotes/origin/main",
        "refs/notes/commits",
        "refs/replace/x",
    ] {
        assert!(
            refs.iter().any(|reference| reference.contains(name)),
            "{name} not named: {refs:?}"
        );
    }
}

/// The kinds of the records the case holds.
fn kinds(case_dir: &Path) -> Vec<String> {
    let Ok(entries) = fs::read_dir(case_dir.join("evidence")) else {
        return Vec::new();
    };
    entries
        .map(|entry| {
            let text = fs::read_to_string(entry.expect("an entry").path()).expect("a record");
            let record: serde_json::Value = serde_json::from_str(&text).expect("json");
            record["kind"].as_str().unwrap_or_default().to_owned()
        })
        .collect()
}
