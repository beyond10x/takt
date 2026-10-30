//! story:pr-open-not-idempotent-on-resume: a run whose `pr.open` reached the forge, which
//! created the pull request, and then lost its answer (a 502, or the process ended before the
//! number was recorded) does not open a second one when it is resumed. Before a `POST`, and
//! after a `POST` that failed, `pr.open` reads the open pull requests whose head is the case's
//! branch through the forge ([`Forge::find_open_pull_request`]), and takes an existing one's number.
//!
//! A forge double keeps a pull request list the way GitHub does: an open for a head that already
//! has an open pull request is refused with GitHub's 422, and an open it is told to lose
//! creates the pull request and then fails with a 502. Every call is kept, so the cases count
//! the `POST`s across both runs. Nothing leaves the machine.

#[path = "support/own_home.rs"]
mod own_home;

use std::cell::{Cell, RefCell};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::rc::Rc;

use anyhow::{anyhow, bail};
use takt::case::Branch;
use takt::forge::{Checks, Forge, Head, PullRequest, PullRequestState, PullRequestView};
use takt::harness::{Harness, StepContext, StepReport};
use takt::validator::{CiRead, PrOpen, Validator};
use takt::{Run, RunOptions, Stop};

/// The protocol under test, the engineering protocols' built-in.
const PROTOCOL: &str = "story.delivery/1";

/// What GitHub says to a second `POST` for a head that has an open pull request.
const EXISTS: &str = "A pull request already exists";

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
        .join(format!("pr-open-lost-{}-{name}", std::process::id()));
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

/// One pull request the double holds, open.
#[derive(Debug, Clone)]
struct Pull {
    number: u64,
    head: String,
    base: String,
}

/// A forge of one repository that keeps its open pull requests the way GitHub does: an open for
/// a head that already has an open pull request is refused with GitHub's 422, an open it is told
/// to lose creates the pull request and then fails with a 502, and the read of the open pull
/// requests for a head and a base answers the one it holds. Every call is kept as a line.
#[derive(Default)]
struct GitHub {
    lines: RefCell<Vec<String>>,
    pushed: RefCell<Option<(String, String)>>,
    pulls: RefCell<Vec<Pull>>,
    /// The next open creates the pull request and then fails with a 502, its answer lost.
    lose: Cell<bool>,
    /// The next open fails with a 502 and creates nothing.
    fail: Cell<bool>,
    /// Every read of the open pull requests fails with a 502, as a connection gone would.
    lists_fail: Cell<bool>,
    /// The open that loses its answer takes the forge out of reach: every read of the open pull
    /// requests after it fails ([`Self::lists_fail`]), as for a process that ended before it
    /// could read anything back.
    dark_after_lost: Cell<bool>,
}

impl GitHub {
    fn posts(&self) -> usize {
        self.lines
            .borrow()
            .iter()
            .filter(|line| line.starts_with("open "))
            .count()
    }

    fn lists(&self) -> Vec<String> {
        self.lines
            .borrow()
            .iter()
            .filter(|line| line.starts_with("find "))
            .cloned()
            .collect()
    }
}

/// Lends a shared [`GitHub`] to the run as its forge.
struct Shared(Rc<GitHub>);

impl Forge for Shared {
    fn name(&self) -> &str {
        "github-double"
    }

    fn scan_outgoing(
        &self,
        _: &Path,
        _: &Path,
        _: &takt::forge::Outgoing<'_>,
    ) -> anyhow::Result<()> {
        Ok(())
    }

    fn push(&self, tree: &Path, branch: &str) -> anyhow::Result<()> {
        self.0.lines.borrow_mut().push(format!("push {branch}"));
        let revision = git(tree, &["rev-parse", &format!("refs/heads/{branch}")]);
        *self.0.pushed.borrow_mut() = Some((branch.to_owned(), revision));
        Ok(())
    }

    fn open_pull_request(&self, _: &Path, _: &Path, request: &PullRequest) -> anyhow::Result<u64> {
        let github = &self.0;
        github
            .lines
            .borrow_mut()
            .push(format!("open {} {}", request.branch, request.base));
        let head = request.branch.clone();
        if github.pulls.borrow().iter().any(|pull| pull.head == head) {
            bail!("422 Unprocessable Entity: Validation Failed; {EXISTS} for owner:{head}.");
        }
        if github.fail.replace(false) {
            bail!("502 Bad Gateway: the response was lost");
        }
        let number = 7 + github.pulls.borrow().len() as u64;
        github.pulls.borrow_mut().push(Pull {
            number,
            head,
            base: request.base.clone(),
        });
        if github.lose.replace(false) {
            if github.dark_after_lost.get() {
                github.lists_fail.set(true);
            }
            bail!("502 Bad Gateway: the response was lost");
        }
        Ok(number)
    }

    fn find_open_pull_request(
        &self,
        _: &Path,
        branch: &str,
        base: &str,
    ) -> anyhow::Result<Option<u64>> {
        let github = &self.0;
        github
            .lines
            .borrow_mut()
            .push(format!("find {branch} {base}"));
        if github.lists_fail.get() {
            bail!("502 Bad Gateway: no answer");
        }
        Ok(github
            .pulls
            .borrow()
            .iter()
            .find(|pull| pull.head == branch && pull.base == base)
            .map(|pull| pull.number))
    }

    fn pull_request(&self, _: &Path, number: u64) -> anyhow::Result<PullRequestView> {
        let github = &self.0;
        github.lines.borrow_mut().push(format!("view {number}"));
        let pulls = github.pulls.borrow();
        let pull = pulls
            .iter()
            .find(|pull| pull.number == number)
            .ok_or_else(|| anyhow!("no pull request #{number}"))?;
        let sha = github
            .pushed
            .borrow()
            .as_ref()
            .filter(|(branch, _)| *branch == pull.head)
            .map(|(_, sha)| sha.clone())
            .unwrap_or_default();
        Ok(PullRequestView {
            state: PullRequestState::Open,
            head: Head {
                branch: pull.head.clone(),
                sha,
            },
            base: pull.base.clone(),
        })
    }

    fn checks(&self, _: &Path, _: u64) -> anyhow::Result<Checks> {
        Ok(Checks::Pass)
    }

    fn merge(&self, _: &Path, _: &Path, number: u64, _: &str) -> anyhow::Result<()> {
        bail!("the double merges nothing: #{number}")
    }
}

/// The forge over `github`.
fn forge(github: &Rc<GitHub>) -> Rc<dyn Forge> {
    Rc::new(Shared(github.clone()))
}

fn options() -> RunOptions {
    RunOptions {
        protocol: PROTOCOL.to_owned(),
        intent: "deliver the story\n\nThe story's body.".to_owned(),
        inputs: Vec::new(),
        iterations: Some(1),
        hours: None,
        tokens: None,
        case_id: None,
        gate: Some("repository.merge".to_owned()),
    }
}

/// What one run left: how it stopped (an error as its text) and what it printed.
struct Ran {
    stop: Result<Stop, String>,
    text: String,
}

/// The first run: the case `id` opened, one commit beyond the base, the tests and the gate
/// green and the change handed over at it, one step, `pr.open`. The case directory and the
/// revision.
fn first_run(tree: &Path, id: &str, forge: Rc<dyn Forge>) -> (Ran, PathBuf, String) {
    let validators: Vec<Box<dyn Validator>> = vec![Box::new(PrOpen::new(forge))];
    let mut run = Run::open(
        tree,
        RunOptions {
            case_id: Some(id.to_owned()),
            ..options()
        },
        &NoHarness,
        &validators,
    )
    .expect("the case opens");
    fs::write(tree.join("done.txt"), "done\n").expect("a file");
    git(tree, &["add", "done.txt"]);
    git(tree, &["commit", "-q", "-m", "the work"]);
    let revision = git(tree, &["rev-parse", "HEAD"]);
    run.case
        .set_revision("implementation", &revision)
        .expect("the revision moves");
    for (id, kind, result, producer) in [
        ("e1", "test_result", Some("pass"), "lost-answer-test"),
        ("e2", "gate_result", Some("pass"), "lost-answer-test"),
        ("handover", "handover", None, takt::run::CHOICE_PRODUCER),
        (
            "change-record",
            "change_record",
            Some("ahead"),
            "takt.change-read",
        ),
    ] {
        run.case
            .add_evidence(&takt::evidence::Record::new(
                id,
                kind,
                result.map(str::to_owned),
                "implementation",
                &revision,
                producer,
            ))
            .expect("the record is written");
    }
    let mut out = Vec::new();
    let stop = run.run(&mut out).map_err(|error| format!("{error:#}"));
    let dir = run.case.dir.clone();
    (
        Ran {
            stop,
            text: String::from_utf8(out).expect("utf-8"),
        },
        dir,
        revision,
    )
}

/// The case `id` resumed, as a new process would, with `pr.open` and `ci.read`: one step.
fn resumed(tree: &Path, id: &str, forge: Rc<dyn Forge>) -> Ran {
    let validators: Vec<Box<dyn Validator>> = vec![
        Box::new(PrOpen::new(forge.clone())),
        Box::new(CiRead::new(forge)),
    ];
    let mut run =
        Run::resume(tree, options(), id, &NoHarness, &validators).expect("the case resumes");
    let mut out = Vec::new();
    let stop = run.run(&mut out).map_err(|error| format!("{error:#}"));
    Ran {
        stop,
        text: String::from_utf8(out).expect("utf-8"),
    }
}

/// The `pull_request` records the case holds: their results and revisions.
fn pull_request_records(case_dir: &Path) -> Vec<(String, String)> {
    let Ok(entries) = fs::read_dir(case_dir.join("evidence")) else {
        return Vec::new();
    };
    entries
        .filter_map(|entry| {
            let text = fs::read_to_string(entry.expect("an entry").path()).expect("a record");
            let record: serde_json::Value = serde_json::from_str(&text).expect("json");
            (record["kind"] == "pull_request").then(|| {
                (
                    record["result"].as_str().unwrap_or_default().to_owned(),
                    record["subject_revision"]
                        .as_str()
                        .unwrap_or_default()
                        .to_owned(),
                )
            })
        })
        .collect()
}

/// What a run said, its stop included, for an assertion's message.
fn said(ran: &Ran) -> String {
    format!("{:?}\n{}", ran.stop, ran.text)
}

/// Acceptance, the process ending after the `POST`: the forge created the pull request and the
/// first run never learnt its number (the `POST`'s answer lost, and the forge unreachable to
/// read it back), so the case holds none. The resumed run reads the open pull request for the
/// case's branch before it would `POST`, records its number in the case and a `pull_request`
/// record `open` at the revision, and sends no second `POST`: one `POST` across both runs, and
/// no `A pull request already exists`.
#[test]
fn a_resume_after_a_lost_answer_reads_the_pull_request_and_posts_no_second_one() {
    let tree = scratch("resume");
    let github = Rc::new(GitHub::default());
    github.lose.set(true);
    github.dark_after_lost.set(true);
    let (first, case_dir, revision) = first_run(&tree, "lost", forge(&github));
    assert_eq!(github.posts(), 1, "{}", said(&first));
    assert_eq!(
        github.pulls.borrow().len(),
        1,
        "the forge holds the pull request"
    );
    assert_eq!(
        Branch::read(&case_dir).expect("branch.yaml").pull_request,
        None,
        "the first run never learnt the number: {}",
        said(&first)
    );
    assert!(
        pull_request_records(&case_dir).is_empty(),
        "{}",
        said(&first)
    );

    github.lists_fail.set(false);
    let second = resumed(&tree, "lost", forge(&github));
    assert_eq!(
        github.posts(),
        1,
        "one POST across both runs: {:#?}\n{}",
        github.lines.borrow(),
        said(&second)
    );
    assert!(!said(&second).contains(EXISTS), "{}", said(&second));
    assert!(second.stop.is_ok(), "{}", said(&second));
    assert_eq!(
        Branch::read(&case_dir).expect("branch.yaml").pull_request,
        Some(7),
        "{}",
        said(&second)
    );
    assert_eq!(
        pull_request_records(&case_dir),
        [("open".to_owned(), revision)],
        "{}",
        said(&second)
    );
    let lists = github.lists();
    assert!(!lists.is_empty(), "the resume read the open pull requests");
    for list in &lists {
        assert_eq!(list, "find takt/lost main", "{lists:?}");
    }
}

/// Acceptance, the predicate's world: the first `POST` creates the pull request and then fails
/// with a 502. The run reads the open pull request for the case's branch after the failed
/// `POST` and records its number; the resumed run then reads that pull request, and neither
/// run sends a second `POST` or stops on `A pull request already exists`.
#[test]
fn a_post_that_fails_after_creating_the_pull_request_takes_its_number() {
    let tree = scratch("predicate");
    let github = Rc::new(GitHub::default());
    github.lose.set(true);
    let (first, case_dir, revision) = first_run(&tree, "predicate", forge(&github));
    assert!(first.stop.is_ok(), "{}", said(&first));
    assert_eq!(
        Branch::read(&case_dir).expect("branch.yaml").pull_request,
        Some(7),
        "{}",
        said(&first)
    );
    assert_eq!(
        pull_request_records(&case_dir),
        [("open".to_owned(), revision)],
        "{}",
        said(&first)
    );
    let before = github.lines.borrow().len();
    let second = resumed(&tree, "predicate", forge(&github));
    assert!(
        github.lines.borrow()[before..]
            .iter()
            .any(|line| line == "view 7"),
        "the resumed run reads pull request #7: {:#?}",
        github.lines.borrow()
    );
    assert_eq!(
        github.posts(),
        1,
        "one POST across both runs: {:#?}",
        github.lines.borrow()
    );
    assert!(!said(&first).contains(EXISTS), "{}", said(&first));
    assert!(!said(&second).contains(EXISTS), "{}", said(&second));
    assert!(second.stop.is_ok(), "{}", said(&second));
    assert_eq!(github.pulls.borrow().len(), 1);
}

/// A `POST` that failed and created nothing finds no pull request to take: the step is the
/// `POST`'s error, the case holds no number, and the resumed run opens the pull request with
/// its own `POST`. The read never makes a pull request up.
#[test]
fn a_post_that_failed_and_created_nothing_is_the_error_and_the_resume_opens_one() {
    let tree = scratch("nothing");
    let github = Rc::new(GitHub::default());
    github.fail.set(true);
    let (first, case_dir, _) = first_run(&tree, "nothing", forge(&github));
    let text = said(&first);
    assert!(text.contains("502 Bad Gateway"), "{text}");
    assert_eq!(
        Branch::read(&case_dir).expect("branch.yaml").pull_request,
        None,
        "{text}"
    );
    assert!(github.pulls.borrow().is_empty());
    let second = resumed(&tree, "nothing", forge(&github));
    assert!(second.stop.is_ok(), "{}", said(&second));
    assert_eq!(github.posts(), 2, "{:#?}", github.lines.borrow());
    assert_eq!(
        Branch::read(&case_dir).expect("branch.yaml").pull_request,
        Some(7),
        "{}",
        said(&second)
    );
}
