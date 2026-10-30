//! story:pr-open-not-idempotent-on-resume: a run whose `pr.open` reached the forge, which
//! created the pull request, and then lost its answer (a 502, or the process ended before the
//! number was recorded) does not open a second one when it is resumed. Before a `POST`, and
//! after a `POST` that failed, `pr.open` reads the open pull requests whose head is the case's
//! branch, through `b10x-gates api --method GET`, and takes an existing one's number.
//!
//! A runner stands in for `b10x-gates` and keeps a pull request list the way GitHub does: a
//! `POST` for a head that already has an open pull request is refused with GitHub's 422, and a
//! `POST` it is told to lose creates the pull request and then fails with a 502. Every line is
//! kept, so the cases count the `POST`s across both runs. Nothing leaves the machine.

#[path = "support/own_home.rs"]
mod own_home;

use std::cell::{Cell, RefCell};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::rc::Rc;

use anyhow::anyhow;
use takt::case::Branch;
use takt::forge::{BotRoute, Exit, Forge, Runner};
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

/// `b10x-gates` over a GitHub of one repository, `owner/repo`: `scan-text` passes, `bot --
/// push` keeps the commit pushed, the dry-run fetch reports the tracking ref's own commit,
/// `api --method POST` opens a pull request (refused with GitHub's 422 for a head that has an
/// open one), `api --method GET` lists the open pull requests its query names or reads one.
#[derive(Default)]
struct GitHub {
    lines: RefCell<Vec<String>>,
    pushed: RefCell<Option<(String, String)>>,
    pulls: RefCell<Vec<Pull>>,
    /// The next `POST` creates the pull request and then fails with a 502, its answer lost.
    lose: Cell<bool>,
    /// The next `POST` fails with a 502 and creates nothing.
    fail: Cell<bool>,
    /// Every list of pull requests fails with a 502, as a connection gone would.
    lists_fail: Cell<bool>,
    /// The `POST` that loses its answer takes the forge out of reach: every list after it fails
    /// ([`Self::lists_fail`]), as for a process that ended before it could read anything back.
    dark_after_lost: Cell<bool>,
    /// Lists every open pull request, whatever head the query names.
    ignores_the_query: Cell<bool>,
}

fn exit(code: i32, stdout: &str, stderr: &str) -> Exit {
    Exit {
        code: Some(code),
        stdout: stdout.to_owned(),
        stderr: stderr.to_owned(),
    }
}

/// The value of `--<name>` in `argv`.
fn option<'a>(argv: &'a [String], name: &str) -> &'a str {
    let at = argv
        .iter()
        .position(|word| word == name)
        .unwrap_or_else(|| panic!("{name} in {argv:?}"));
    &argv[at + 1]
}

/// `%XX` decoded.
fn decoded(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            let hex = std::str::from_utf8(&bytes[i + 1..i + 3]).expect("ascii");
            out.push(u8::from_str_radix(hex, 16).expect("a hex byte"));
            i += 3;
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    String::from_utf8(out).expect("utf-8")
}

impl GitHub {
    fn posts(&self) -> usize {
        self.lines
            .borrow()
            .iter()
            .filter(|line| line.starts_with("b10x-gates api --method POST "))
            .count()
    }

    fn lists(&self) -> Vec<String> {
        self.lines
            .borrow()
            .iter()
            .filter(|line| line.contains(" --method GET --path /repos/owner/repo/pulls?"))
            .cloned()
            .collect()
    }

    fn view(&self, pull: &Pull) -> serde_json::Value {
        let sha = self
            .pushed
            .borrow()
            .as_ref()
            .filter(|(branch, _)| *branch == pull.head)
            .map(|(_, sha)| sha.clone())
            .unwrap_or_default();
        serde_json::json!({
            "number": pull.number,
            "state": "open",
            "merged": false,
            "head": { "ref": pull.head, "sha": sha, "repo": { "full_name": "owner/repo" } },
            "base": { "ref": pull.base },
        })
    }

    fn api(&self, argv: &[String]) -> anyhow::Result<Exit> {
        let method = option(argv, "--method");
        let path = option(argv, "--path");
        let output = option(argv, "--output");
        if method == "POST" {
            assert_eq!(path, "/repos/owner/repo/pulls", "{argv:?}");
            let input: serde_json::Value =
                serde_json::from_str(&fs::read_to_string(option(argv, "--input"))?)?;
            let head = input["head"].as_str().expect("a head").to_owned();
            let base = input["base"].as_str().expect("a base").to_owned();
            if self.pulls.borrow().iter().any(|pull| pull.head == head) {
                return Ok(exit(
                    1,
                    "",
                    &format!(
                        "b10x-gates: GitHub operation failed (422 Unprocessable Entity): Validation Failed; PullRequest custom: {EXISTS} for owner:{head}."
                    ),
                ));
            }
            if self.fail.replace(false) {
                return Ok(exit(
                    1,
                    "",
                    "b10x-gates: GitHub operation failed (502 Bad Gateway): the response was lost",
                ));
            }
            let number = 7 + self.pulls.borrow().len() as u64;
            self.pulls.borrow_mut().push(Pull { number, head, base });
            if self.lose.replace(false) {
                if self.dark_after_lost.get() {
                    self.lists_fail.set(true);
                }
                return Ok(exit(
                    1,
                    "",
                    "b10x-gates: GitHub operation failed (502 Bad Gateway): the response was lost",
                ));
            }
            fs::write(output, serde_json::json!({ "number": number }).to_string())?;
            return Ok(exit(0, "", ""));
        }
        assert_eq!(method, "GET", "{argv:?}");
        assert!(!argv.iter().any(|word| word == "--input"), "{argv:?}");
        if let Some(query) = path.strip_prefix("/repos/owner/repo/pulls?") {
            if self.lists_fail.get() {
                return Ok(exit(
                    1,
                    "",
                    "b10x-gates: GitHub operation failed (502 Bad Gateway): no answer",
                ));
            }
            let mut state = None;
            let mut head = None;
            let mut base = None;
            for pair in query.split('&') {
                let (name, value) = pair.split_once('=').expect("name=value");
                match name {
                    "state" => state = Some(decoded(value)),
                    "head" => head = Some(decoded(value)),
                    "base" => base = Some(decoded(value)),
                    _ => {}
                }
            }
            assert_eq!(state.as_deref(), Some("open"), "{path}");
            let listed: Vec<serde_json::Value> = self
                .pulls
                .borrow()
                .iter()
                .filter(|pull| {
                    self.ignores_the_query.get()
                        || (head.as_deref() == Some(&format!("owner:{}", pull.head))
                            && base.as_deref().is_none_or(|base| base == pull.base))
                })
                .map(|pull| self.view(pull))
                .collect();
            fs::write(output, serde_json::Value::Array(listed).to_string())?;
            return Ok(exit(0, "", ""));
        }
        if let Some(number) = path.strip_prefix("/repos/owner/repo/pulls/") {
            let number: u64 = number.parse()?;
            let pulls = self.pulls.borrow();
            let pull = pulls
                .iter()
                .find(|pull| pull.number == number)
                .ok_or_else(|| anyhow!("no pull request #{number}"))?;
            fs::write(output, self.view(pull).to_string())?;
            return Ok(exit(0, "", ""));
        }
        if path.starts_with("/repos/owner/repo/commits/") && path.contains("/check-runs") {
            fs::write(
                output,
                r#"{"total_count":1,"check_runs":[{"status":"completed","conclusion":"success"}]}"#,
            )?;
            return Ok(exit(0, "", ""));
        }
        if path.starts_with("/repos/owner/repo/commits/") && path.contains("/status") {
            fs::write(
                output,
                r#"{"state":"pending","total_count":0,"statuses":[]}"#,
            )?;
            return Ok(exit(0, "", ""));
        }
        Ok(exit(2, "", "the double does not know this read"))
    }
}

impl Runner for GitHub {
    fn run(&self, tree: &Path, argv: &[String]) -> anyhow::Result<Exit> {
        self.run_env(tree, argv, &[])
    }

    fn run_env(&self, tree: &Path, argv: &[String], _: &[(&str, &str)]) -> anyhow::Result<Exit> {
        self.lines.borrow_mut().push(argv.join(" "));
        let words: Vec<&str> = argv.iter().map(String::as_str).collect();
        match words.as_slice() {
            ["b10x-gates", "scan-text", ..] => Ok(exit(0, "no private rule", "")),
            ["b10x-gates", "bot", "--", "push", _, spec, ..] => {
                let (revision, branch) = spec
                    .split_once(":refs/heads/")
                    .expect("a push of a revision");
                *self.pushed.borrow_mut() = Some((branch.to_owned(), revision.to_owned()));
                Ok(exit(0, "", ""))
            }
            ["b10x-gates", "bot", "--", "fetch", _, refspec, ..] => {
                let (_, to) = refspec.split_once(':').expect("a refspec");
                let held = Command::new("git")
                    .args(["rev-parse", "--verify", "-q", to])
                    .current_dir(tree)
                    .output()
                    .expect("git runs");
                let held = String::from_utf8_lossy(&held.stdout).trim().to_owned();
                Ok(exit(0, &format!("= {held} {held} {to}\n"), ""))
            }
            ["b10x-gates", "api", ..] => self.api(argv),
            _ => Ok(exit(2, "", "the double does not know this line")),
        }
    }
}

/// Lends a shared [`GitHub`] to a forge that owns its runner.
struct Shared(Rc<GitHub>);

impl Runner for Shared {
    fn run(&self, tree: &Path, argv: &[String]) -> anyhow::Result<Exit> {
        self.0.run(tree, argv)
    }

    fn run_env(&self, tree: &Path, argv: &[String], env: &[(&str, &str)]) -> anyhow::Result<Exit> {
        self.0.run_env(tree, argv, env)
    }
}

/// The bot route over `github`.
fn bot(github: &Rc<GitHub>) -> Rc<dyn Forge> {
    Rc::new(BotRoute::new(
        "owner/repo",
        Some(PathBuf::from("/policies/gates.json")),
        Box::new(Shared(github.clone())),
    ))
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
    let (first, case_dir, revision) = first_run(&tree, "lost", bot(&github));
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
    let second = resumed(&tree, "lost", bot(&github));
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
        assert!(
            list.starts_with(
                "b10x-gates api --method GET --path /repos/owner/repo/pulls?state=open&head=owner:takt%2Flost&base=main"
            ),
            "{list}"
        );
        assert!(
            list.contains("--repository owner/repo --policy /policies/gates.json"),
            "{list}"
        );
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
    let (first, case_dir, revision) = first_run(&tree, "predicate", bot(&github));
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
    let second = resumed(&tree, "predicate", bot(&github));
    assert!(
        github.lines.borrow()[before..]
            .iter()
            .any(|line| line.contains(" --path /repos/owner/repo/pulls/7 ")),
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
    let (first, case_dir, _) = first_run(&tree, "nothing", bot(&github));
    let text = said(&first);
    assert!(text.contains("502 Bad Gateway"), "{text}");
    assert_eq!(
        Branch::read(&case_dir).expect("branch.yaml").pull_request,
        None,
        "{text}"
    );
    assert!(github.pulls.borrow().is_empty());
    let second = resumed(&tree, "nothing", bot(&github));
    assert!(second.stop.is_ok(), "{}", said(&second));
    assert_eq!(github.posts(), 2, "{:#?}", github.lines.borrow());
    assert_eq!(
        Branch::read(&case_dir).expect("branch.yaml").pull_request,
        Some(7),
        "{}",
        said(&second)
    );
}

/// The read's answer is checked, not trusted: an open pull request whose head is another
/// branch, as a forge that ignored the query would list, is not taken, and the case's own pull
/// request is opened.
#[test]
fn an_open_pull_request_of_another_branch_is_not_taken() {
    let tree = scratch("other");
    let github = Rc::new(GitHub::default());
    github.pulls.borrow_mut().push(Pull {
        number: 7,
        head: "takt/someone-else".to_owned(),
        base: "main".to_owned(),
    });
    github.ignores_the_query.set(true);
    let (first, case_dir, _) = first_run(&tree, "other", bot(&github));
    assert!(first.stop.is_ok(), "{}", said(&first));
    assert_eq!(github.posts(), 1, "{:#?}", github.lines.borrow());
    assert_eq!(
        Branch::read(&case_dir).expect("branch.yaml").pull_request,
        Some(8),
        "{}",
        said(&first)
    );
}

/// The bot route's read of the open pull requests for a head: one `GET` of
/// `/repos/<owner>/<repo>/pulls` naming the state, the head as `<owner>:<branch>` (encoded) and
/// the base; an answer that is not a list is an error, never "none".
#[test]
fn the_bot_route_reads_the_open_pull_requests_for_a_head_through_the_app() {
    let tree = scratch("adapter");
    let github = Rc::new(GitHub::default());
    let forge = bot(&github);
    assert_eq!(
        forge
            .find_open_pull_request(&tree, "takt/c1", "main")
            .expect("reads"),
        None
    );
    github.pulls.borrow_mut().push(Pull {
        number: 7,
        head: "takt/c1".to_owned(),
        base: "main".to_owned(),
    });
    assert_eq!(
        forge
            .find_open_pull_request(&tree, "takt/c1", "main")
            .expect("reads"),
        Some(7)
    );
    assert_eq!(
        forge
            .find_open_pull_request(&tree, "takt/c1", "trunk")
            .expect("reads"),
        None,
        "another base"
    );
    let lists = github.lists();
    assert_eq!(lists.len(), 3, "{lists:#?}");
    let words: Vec<&str> = lists[0].split(' ').collect();
    assert_eq!(
        words[..6],
        [
            "b10x-gates",
            "api",
            "--method",
            "GET",
            "--path",
            "/repos/owner/repo/pulls?state=open&head=owner:takt%2Fc1&base=main&per_page=100"
        ]
    );

    struct NotAList;
    impl Runner for NotAList {
        fn run(&self, tree: &Path, argv: &[String]) -> anyhow::Result<Exit> {
            self.run_env(tree, argv, &[])
        }

        fn run_env(&self, _: &Path, argv: &[String], _: &[(&str, &str)]) -> anyhow::Result<Exit> {
            fs::write(option(argv, "--output"), r#"{"message":"Not Found"}"#)?;
            Ok(exit(0, "", ""))
        }
    }
    let error = format!(
        "{:#}",
        BotRoute::new("owner/repo", None, Box::new(NotAList))
            .find_open_pull_request(&tree, "takt/c1", "main")
            .expect_err("not a list")
    );
    assert!(error.contains("list"), "{error}");
}
