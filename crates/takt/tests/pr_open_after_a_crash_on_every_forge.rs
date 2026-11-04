//! story:pr-open-cut-off-by-a-crash-is-read-back: a run whose `pr.open` sent the open and ended
//! before it read the answer leaves the case with no number and an open recorded `unknown`. The
//! resumed run reads the forge's open pull request for the case's branch before it opens one
//! ([`Forge::find_open_pull_request`]), on every shipped forge: the command forge through its
//! `find` entry, GitLab through the merge requests open from the branch into the base, and the
//! local forge from the request records in its hub. A command forge without `find` cannot read
//! one back, and sends no second open while the first stands unanswered.
//!
//! The first run goes through [`Cut`], which lets the forge's open through and then ends the way
//! a process that died after the send does: the answer is never read, and nothing of the forge is
//! reached again. The resumed run talks to the forge itself. The GitLab host and the command
//! forge's program are doubles of the forge [`Runner`] that keep their requests and every call;
//! the local forge is a real bare repository under this crate's build directory. Nothing leaves
//! the machine.

#[path = "support/own_home.rs"]
mod own_home;

use std::cell::{Cell, RefCell};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::rc::Rc;

use anyhow::{anyhow, bail};
use takt::case::Branch;
use takt::forge::command::CommandForge;
use takt::forge::gitlab::Glab;
use takt::forge::local::Local;
use takt::forge::{
    Checks, Exit, Forge, IdentityScan, MergeMessage, Outgoing, PullRequest, PullRequestView,
    Runner, WriteError, Written,
};
use takt::git::CommitIdentity;
use takt::harness::{Harness, StepContext, StepReport};
use takt::run_config::RunConfig;
use takt::scan::Scan;
use takt::validator::{CiRead, PrOpen, Validator};
use takt::{Run, RunOptions, Stop};

/// The protocol under test, the engineering protocols' built-in.
const PROTOCOL: &str = "story.delivery/1";

/// Runs `git <args>` in `dir` with no configuration of the user's or the system's; its trimmed
/// standard output.
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

/// A scratch directory under this crate's build directory, removed on drop.
struct Scratch(PathBuf);

impl Scratch {
    fn new(name: &str) -> Self {
        let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
            .join(format!("pr-open-crash-{}-{name}", std::process::id()));
        if dir.exists() {
            fs::remove_dir_all(&dir).expect("a stale scratch directory is removed");
        }
        fs::create_dir_all(&dir).expect("a scratch directory");
        Self(dir)
    }

    /// A tree with one commit on `main` and `origin` at `url`.
    fn tree(&self, url: &str) -> PathBuf {
        let tree = self.0.join("tree");
        fs::create_dir_all(&tree).expect("a tree");
        fs::write(tree.join("README.md"), "a story\n").expect("a file");
        git(&tree, &["init", "-q", "-b", "main"]);
        git(&tree, &["add", "-A"]);
        git(&tree, &["commit", "-q", "-m", "start"]);
        git(&tree, &["remote", "add", "origin", url]);
        tree
    }

    /// A bare hub `<scratch>/hub.git` whose `main` holds one commit, and a tree cloned from it
    /// with `--no-local`, on `main`.
    fn hub_and_tree(&self) -> PathBuf {
        let seed = self.0.join("seed");
        fs::create_dir_all(&seed).expect("a directory");
        fs::write(seed.join("README.md"), "a story\n").expect("a file");
        git(&seed, &["init", "-q", "-b", "main"]);
        git(&seed, &["add", "-A"]);
        git(&seed, &["commit", "-q", "-m", "start"]);
        git(&self.0, &["clone", "-q", "--bare", "seed", "hub.git"]);
        fs::remove_dir_all(&seed).expect("the seed is removed");
        git(&self.0, &["clone", "-q", "--no-local", "hub.git", "tree"]);
        self.0.join("tree")
    }

    fn scan(&self) -> Scan {
        Scan::None {
            declared_in: self.0.join("run.yaml"),
        }
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
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

/// A forge whose process ends after the open is sent: the open goes through to the forge it
/// wraps, its answer is dropped and the write reported `unknown`, and from then on nothing of
/// that forge is reached, as nothing is from a process that is gone.
struct Cut {
    forge: Rc<dyn Forge>,
    gone: Cell<bool>,
}

impl Cut {
    fn over(forge: Rc<dyn Forge>) -> Rc<dyn Forge> {
        Rc::new(Self {
            forge,
            gone: Cell::new(false),
        })
    }

    fn alive(&self) -> anyhow::Result<()> {
        if self.gone.get() {
            bail!("the process ended after the open was sent");
        }
        Ok(())
    }
}

impl Forge for Cut {
    fn name(&self) -> &str {
        self.forge.name()
    }

    fn scan_outgoing(
        &self,
        tree: &Path,
        dir: &Path,
        outgoing: &Outgoing<'_>,
    ) -> anyhow::Result<()> {
        self.alive()?;
        self.forge.scan_outgoing(tree, dir, outgoing)
    }

    fn push(&self, tree: &Path, branch: &str) -> anyhow::Result<()> {
        self.alive()?;
        self.forge.push(tree, branch)
    }

    fn push_revision(&self, tree: &Path, branch: &str, revision: &str) -> anyhow::Result<()> {
        self.alive()?;
        self.forge.push_revision(tree, branch, revision)
    }

    fn open_pull_request(&self, tree: &Path, dir: &Path, request: &PullRequest) -> Written<u64> {
        self.alive().map_err(WriteError::not_attempted)?;
        let answer = self.forge.open_pull_request(tree, dir, request);
        self.gone.set(true);
        Err(WriteError::unknown(anyhow!(
            "the process ended after the open was sent, its answer never read ({})",
            if answer.is_ok() { "opened" } else { "failed" }
        )))
    }

    fn find_open_pull_request(
        &self,
        tree: &Path,
        branch: &str,
        base: &str,
    ) -> anyhow::Result<Option<u64>> {
        self.alive()?;
        self.forge.find_open_pull_request(tree, branch, base)
    }

    fn pull_request(&self, tree: &Path, number: u64) -> anyhow::Result<PullRequestView> {
        self.alive()?;
        self.forge.pull_request(tree, number)
    }

    fn behind_base(&self, tree: &Path, number: u64) -> anyhow::Result<Option<bool>> {
        self.alive()?;
        self.forge.behind_base(tree, number)
    }

    fn checks(&self, tree: &Path, number: u64) -> anyhow::Result<Checks> {
        self.alive()?;
        self.forge.checks(tree, number)
    }

    fn merge(&self, tree: &Path, dir: &Path, number: u64, head: &str) -> Written<()> {
        self.alive().map_err(WriteError::not_attempted)?;
        self.forge.merge(tree, dir, number, head)
    }

    fn merge_with(
        &self,
        tree: &Path,
        dir: &Path,
        number: u64,
        head: &str,
        message: &MergeMessage,
    ) -> Written<()> {
        self.alive().map_err(WriteError::not_attempted)?;
        self.forge.merge_with(tree, dir, number, head, message)
    }

    fn scan_identity(
        &self,
        tree: &Path,
        dir: &Path,
        identity: &CommitIdentity,
    ) -> anyhow::Result<IdentityScan> {
        self.forge.scan_identity(tree, dir, identity)
    }

    fn target_tip(&self, tree: &Path, base: &str) -> anyhow::Result<String> {
        self.alive()?;
        self.forge.target_tip(tree, base)
    }
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

/// What a run said, its stop included, for an assertion's message.
fn said(ran: &Ran) -> String {
    format!("{:?}\n{}", ran.stop, ran.text)
}

/// The first run: the case `id` opened, one commit beyond the base, the tests and the gate
/// green and the change handed over at it, one step, `pr.open`, through `forge`. The case
/// directory and the revision.
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
        ("e1", "test_result", Some("pass"), "crash-test"),
        ("e2", "gate_result", Some("pass"), "crash-test"),
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

/// The case `id` resumed, as a new process would, with `pr.open` and `ci.read` over `forge`:
/// one step.
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

/// The first run sent one open and never learnt its number: the case holds none and no record.
fn assert_cut_off(first: &Ran, case_dir: &Path) {
    assert_eq!(
        Branch::read(case_dir).expect("branch.yaml").pull_request,
        None,
        "the first run never learnt the number: {}",
        said(first)
    );
    assert!(pull_request_records(case_dir).is_empty(), "{}", said(first));
}

/// The resumed run took the open pull request `number` the forge holds: the case holds its
/// number and a `pull_request` record `open` at the revision.
fn assert_taken(second: &Ran, case_dir: &Path, number: u64, revision: &str) {
    assert!(second.stop.is_ok(), "{}", said(second));
    assert_eq!(
        Branch::read(case_dir).expect("branch.yaml").pull_request,
        Some(number),
        "{}",
        said(second)
    );
    assert_eq!(
        pull_request_records(case_dir),
        [("open".to_owned(), revision.to_owned())],
        "{}",
        said(second)
    );
}

// ---------------------------------------------------------------------------------------------
// The doubles of a host behind the forge `Runner`.

/// One request a double holds, open.
#[derive(Debug, Clone)]
struct Held {
    number: u64,
    branch: String,
    base: String,
}

/// What a double of a host keeps: its open requests, the head last pushed for each branch, and
/// every call as a line (`push <branch>`, `open <branch> <base>`, `find ...`, `read <n>`).
#[derive(Default)]
struct Host {
    held: RefCell<Vec<Held>>,
    pushed: RefCell<Vec<(String, String)>>,
    lines: RefCell<Vec<String>>,
    /// What a read of the open requests answers instead of the list, when set.
    garbled: RefCell<Option<String>>,
    /// The next `open` the command program runs takes the request and the process ends before
    /// the program answers: nothing of the forge is reached from then on ([`Self::dark`]).
    die_after_open: Cell<bool>,
    /// The process is gone: every line run fails.
    dark: Cell<bool>,
    /// The next `open` the command program runs is refused, `refused_exit` 3, nothing taken.
    refuse: Cell<bool>,
}

impl Host {
    fn opens(&self) -> usize {
        self.lines
            .borrow()
            .iter()
            .filter(|line| line.starts_with("open "))
            .count()
    }

    fn finds(&self) -> Vec<String> {
        self.lines
            .borrow()
            .iter()
            .filter(|line| line.starts_with("find "))
            .cloned()
            .collect()
    }

    fn log(&self, line: String) {
        self.lines.borrow_mut().push(line);
    }

    /// Records the push `argv` names (`<sha>:refs/heads/<branch>`).
    fn push(&self, argv: &[String]) -> Exit {
        let Some((sha, branch)) = argv.iter().find_map(|word| word.split_once(":refs/heads/"))
        else {
            return exit(1, "", "no refspec");
        };
        self.log(format!("push {branch}"));
        let mut pushed = self.pushed.borrow_mut();
        pushed.retain(|(held, _)| held != branch);
        pushed.push((branch.to_owned(), sha.to_owned()));
        exit(0, "", "")
    }

    fn head_of(&self, branch: &str) -> String {
        self.pushed
            .borrow()
            .iter()
            .find(|(held, _)| held == branch)
            .map(|(_, sha)| sha.clone())
            .unwrap_or_default()
    }

    /// Opens a request for `branch` into `base`, refused while one is open for the branch.
    fn open(&self, branch: &str, base: &str) -> Result<u64, String> {
        self.log(format!("open {branch} {base}"));
        if let Some(held) = self.held.borrow().iter().find(|held| held.branch == branch) {
            return Err(format!(
                "Another open merge request already exists for this source branch: !{}",
                held.number
            ));
        }
        let number = 7 + self.held.borrow().len() as u64;
        self.held.borrow_mut().push(Held {
            number,
            branch: branch.to_owned(),
            base: base.to_owned(),
        });
        Ok(number)
    }

    fn found(&self, branch: &str, base: &str) -> Vec<Held> {
        self.held
            .borrow()
            .iter()
            .filter(|held| held.branch == branch && held.base == base)
            .cloned()
            .collect()
    }
}

fn exit(code: i32, stdout: &str, stderr: &str) -> Exit {
    Exit {
        code: Some(code),
        stdout: stdout.to_owned(),
        stderr: stderr.to_owned(),
    }
}

/// `text` with each `%XX` decoded.
fn decoded(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = Vec::new();
    let mut at = 0;
    while at < bytes.len() {
        if bytes[at] == b'%'
            && at + 2 < bytes.len()
            && let Ok(byte) = u8::from_str_radix(&text[at + 1..at + 3], 16)
        {
            out.push(byte);
            at += 3;
        } else {
            out.push(bytes[at]);
            at += 1;
        }
    }
    String::from_utf8(out).expect("utf-8")
}

/// A GitLab host behind `glab api`: merge requests opened, read one by one and listed by
/// state, source branch and target branch, as the API answers them.
#[derive(Clone, Default)]
struct GitLabHost(Rc<Host>);

impl Runner for GitLabHost {
    fn run(&self, tree: &Path, argv: &[String]) -> anyhow::Result<Exit> {
        self.run_env(tree, argv, &[])
    }

    fn run_env(&self, _: &Path, argv: &[String], _: &[(&str, &str)]) -> anyhow::Result<Exit> {
        let host = &self.0;
        if argv.first().is_some_and(|program| program == "git") {
            return Ok(host.push(argv));
        }
        let method = argv
            .iter()
            .position(|word| word == "--method")
            .map_or("GET", |at| argv[at + 1].as_str());
        let path = argv
            .iter()
            .find(|word| word.starts_with("projects/"))
            .ok_or_else(|| anyhow!("no API path in {argv:?}"))?;
        let base = "projects/group%2Frepo/merge_requests";
        let not_found = || exit(1, r#"{"message":"404 Not found"}"#, "(HTTP 404)");
        let Some(rest) = path.strip_prefix(base) else {
            return Ok(not_found());
        };
        Ok(match (method, rest) {
            ("POST", "") => {
                let input = argv
                    .iter()
                    .position(|word| word == "--input")
                    .map(|at| argv[at + 1].clone())
                    .ok_or_else(|| anyhow!("no input in {argv:?}"))?;
                let sent: serde_json::Value = serde_json::from_slice(&fs::read(input)?)?;
                let branch = sent["source_branch"].as_str().unwrap_or_default();
                let target = sent["target_branch"].as_str().unwrap_or_default();
                match host.open(branch, target) {
                    Ok(number) => exit(0, &serde_json::json!({ "iid": number }).to_string(), ""),
                    Err(message) => exit(
                        1,
                        &serde_json::json!({ "message": [message] }).to_string(),
                        "(HTTP 409)",
                    ),
                }
            }
            ("GET", query) if query.starts_with('?') => {
                let mut state = None;
                let mut source = None;
                let mut target = None;
                for pair in query[1..].split('&') {
                    let (key, value) = pair.split_once('=').unwrap_or((pair, ""));
                    match key {
                        "state" => state = Some(decoded(value)),
                        "source_branch" => source = Some(decoded(value)),
                        "target_branch" => target = Some(decoded(value)),
                        _ => {}
                    }
                }
                host.log(format!(
                    "find {} {} {}",
                    source.clone().unwrap_or_default(),
                    target.clone().unwrap_or_default(),
                    state.clone().unwrap_or_default()
                ));
                if let Some(garbled) = host.garbled.borrow().as_ref() {
                    return Ok(exit(0, garbled, ""));
                }
                let listed: Vec<serde_json::Value> = host
                    .held
                    .borrow()
                    .iter()
                    .filter(|held| {
                        state.as_deref().is_none_or(|state| state == "opened")
                            && source.as_deref().is_none_or(|source| source == held.branch)
                            && target.as_deref().is_none_or(|target| target == held.base)
                    })
                    .map(|held| {
                        serde_json::json!({
                            "iid": held.number,
                            "state": "opened",
                            "source_branch": held.branch,
                            "target_branch": held.base,
                            "sha": host.head_of(&held.branch),
                        })
                    })
                    .collect();
                exit(0, &serde_json::Value::Array(listed).to_string(), "")
            }
            ("GET", one) => {
                let Some(number) = one
                    .strip_prefix('/')
                    .and_then(|number| number.parse::<u64>().ok())
                else {
                    return Ok(not_found());
                };
                host.log(format!("read {number}"));
                let held = host.held.borrow();
                let Some(held) = held.iter().find(|held| held.number == number) else {
                    return Ok(not_found());
                };
                let document = serde_json::json!({
                    "iid": held.number,
                    "state": "opened",
                    "source_branch": held.branch,
                    "target_branch": held.base,
                    "sha": host.head_of(&held.branch),
                    "head_pipeline": null,
                });
                exit(0, &document.to_string(), "")
            }
            _ => not_found(),
        })
    }
}

/// The program of a command forge: `tool open|read|find ...`, answering from the host.
#[derive(Clone, Default)]
struct CommandHost(Rc<Host>);

impl Runner for CommandHost {
    fn run(&self, tree: &Path, argv: &[String]) -> anyhow::Result<Exit> {
        self.run_env(tree, argv, &[])
    }

    fn run_env(&self, _: &Path, argv: &[String], _: &[(&str, &str)]) -> anyhow::Result<Exit> {
        let host = &self.0;
        if host.dark.get() {
            bail!("the process ended after the open was sent");
        }
        let words: Vec<&str> = argv.iter().map(String::as_str).collect();
        Ok(match words.as_slice() {
            ["git", ..] => host.push(argv),
            ["tool", "open", branch, base] if host.refuse.replace(false) => {
                host.log(format!("open {branch} {base}"));
                exit(3, "", "the forge refused the request")
            }
            ["tool", "open", branch, base] => match host.open(branch, base) {
                Ok(_) if host.die_after_open.replace(false) => {
                    host.dark.set(true);
                    bail!("the process ended after the open was sent");
                }
                Ok(number) => exit(
                    0,
                    &format!("https://forge.test/group/repo/-/merge_requests/{number}\n"),
                    "",
                ),
                Err(message) => exit(1, "", &message),
            },
            ["tool", "read", number] => {
                host.log(format!("read {number}"));
                let held = host.held.borrow();
                let Some(held) = held.iter().find(|held| held.number.to_string() == *number) else {
                    return Ok(exit(1, "", "no such request"));
                };
                let document = serde_json::json!({
                    "state": "opened",
                    "head": host.head_of(&held.branch),
                    "branch": held.branch,
                    "base": held.base,
                });
                exit(0, &document.to_string(), "")
            }
            ["tool", "find", branch, base] => {
                host.log(format!("find {branch} {base}"));
                if let Some(garbled) = host.garbled.borrow().as_ref() {
                    return Ok(exit(0, garbled, ""));
                }
                let found = host.found(branch, base);
                exit(
                    0,
                    &found
                        .first()
                        .map(|held| format!("{}\n", held.number))
                        .unwrap_or_default(),
                    "",
                )
            }
            _ => exit(1, "", &format!("the double does not know {argv:?}")),
        })
    }
}

/// The command forge of the run file `forge_commands` section `section`, its program the
/// double over `host`.
fn command_forge(
    scratch: &Scratch,
    tree: &Path,
    section: &str,
    host: &CommandHost,
) -> CommandForge {
    let path = scratch.0.join("run.yaml");
    fs::write(
        &path,
        format!("forge: command\nscan: none\nforge_commands:\n{section}"),
    )
    .expect("a run file");
    let config = RunConfig::read(&path).expect("the run file reads");
    CommandForge::for_tree(
        tree,
        config.forge_commands.expect("the section"),
        &[
            "pr.open".to_owned(),
            "ci.read".to_owned(),
            "repository.merge".to_owned(),
        ],
        scratch.0.join("state"),
        scratch.scan(),
        Box::new(host.clone()),
    )
    .expect("the command forge")
}

/// The commands of a command forge, `find` among them when `find` is set.
fn section(find: bool) -> String {
    let mut text = String::from(
        "  open: [tool, open, \"{branch}\", \"{base}\"]\n  read: [tool, read, \"{number}\"]\n  checks: [tool, checks, \"{number}\", \"{head}\"]\n  merge: [tool, merge, \"{number}\", \"{head}\"]\n  refused_exit: [3]\n",
    );
    if find {
        text.push_str("  find: [tool, find, \"{branch}\", \"{base}\"]\n");
    }
    text
}

// ---------------------------------------------------------------------------------------------
// The cases.

/// The command forge with a `find` entry: the resumed run runs `find` for the case's branch into
/// its base, takes the number it prints and reads it through `read`; one `open` across both runs.
#[test]
fn the_command_forge_finds_the_open_request_through_find() {
    let scratch = Scratch::new("command");
    let tree = scratch.tree("https://forge.test/group/repo.git");
    let host = CommandHost::default();
    let forge: Rc<dyn Forge> = Rc::new(command_forge(&scratch, &tree, &section(true), &host));
    let (first, case_dir, revision) = first_run(&tree, "command", Cut::over(forge.clone()));
    assert_eq!(host.0.opens(), 1, "{}", said(&first));
    assert_cut_off(&first, &case_dir);

    let second = resumed(&tree, "command", forge);
    assert_eq!(
        host.0.opens(),
        1,
        "one open across both runs: {:#?}\n{}",
        host.0.lines.borrow(),
        said(&second)
    );
    assert_taken(&second, &case_dir, 7, &revision);
    assert_eq!(
        host.0.finds(),
        ["find takt/command main", "find takt/command main"],
        "one find before the first open, one on the resume"
    );
}

/// The command forge without `find` cannot read the open request back: the resumed run sends no
/// second `open` while the first stands unanswered, and says what it lacks, naming `find`.
///
/// The process is cut inside the `open` command here, not at the forge's answer as [`Cut`] cuts
/// it: the forge settles its own record of the open once the command answers, and a process
/// that died after the send never gets that far.
#[test]
fn a_command_forge_without_find_sends_no_second_open_while_one_stands_unanswered() {
    let scratch = Scratch::new("command-no-find");
    let tree = scratch.tree("https://forge.test/group/repo.git");
    let host = CommandHost::default();
    let forge: Rc<dyn Forge> = Rc::new(command_forge(&scratch, &tree, &section(false), &host));
    host.0.die_after_open.set(true);
    let (first, case_dir, _) = first_run(&tree, "nofind", forge.clone());
    assert_eq!(host.0.opens(), 1, "{}", said(&first));
    assert_eq!(host.0.held.borrow().len(), 1, "the forge took the open");
    assert_cut_off(&first, &case_dir);
    assert!(
        case_dir
            .join("artifacts")
            .join(CommandForge::UNANSWERED)
            .exists(),
        "the open stands unanswered: {}",
        said(&first)
    );

    host.0.dark.set(false);
    let second = resumed(&tree, "nofind", forge.clone());
    assert_eq!(
        host.0.opens(),
        1,
        "no second open: {:#?}\n{}",
        host.0.lines.borrow(),
        said(&second)
    );
    let text = said(&second);
    assert!(text.contains("`find`"), "{text}");
    assert_eq!(
        Branch::read(&case_dir).expect("branch.yaml").pull_request,
        None,
        "{text}"
    );
    assert!(pull_request_records(&case_dir).is_empty(), "{text}");

    // Resumed again, it still sends none.
    let third = resumed(&tree, "nofind", forge);
    assert_eq!(host.0.opens(), 1, "{}", said(&third));
}

/// The command forge without `find` opens as before when no open stands unanswered: a case's
/// first open is sent, and an open the forge refused is no open that may stand, so the resumed
/// run sends it again and takes its number; nothing of the open is left standing.
#[test]
fn a_command_forge_without_find_opens_when_no_open_stands_unanswered() {
    let scratch = Scratch::new("command-no-find-fresh");
    let tree = scratch.tree("https://forge.test/group/repo.git");
    let host = CommandHost::default();
    let forge: Rc<dyn Forge> = Rc::new(command_forge(&scratch, &tree, &section(false), &host));
    host.0.refuse.set(true);
    let (first, case_dir, revision) = first_run(&tree, "fresh", forge.clone());
    assert!(
        said(&first).contains("refused with exit 3"),
        "{}",
        said(&first)
    );
    assert_eq!(host.0.opens(), 1, "{}", said(&first));
    let unanswered = case_dir.join("artifacts").join(CommandForge::UNANSWERED);
    assert!(!unanswered.exists(), "{}", said(&first));

    let second = resumed(&tree, "fresh", forge);
    assert_eq!(host.0.opens(), 2, "{}", said(&second));
    assert_taken(&second, &case_dir, 7, &revision);
    assert!(!unanswered.exists(), "{}", said(&second));
}

/// What `find` prints that is neither a number nor nothing is an error, never none.
#[test]
fn what_find_prints_that_is_no_number_is_an_error() {
    let scratch = Scratch::new("command-garbled");
    let tree = scratch.tree("https://forge.test/group/repo.git");
    let host = CommandHost::default();
    let forge = command_forge(&scratch, &tree, &section(true), &host);
    assert_eq!(
        forge
            .find_open_pull_request(&tree, "takt/x", "main")
            .expect("nothing printed is none"),
        None
    );
    for garbled in ["seven\n", "7 8\n", "-7\n", "{\"iid\": 7}\n"] {
        host.0.garbled.replace(Some(garbled.to_owned()));
        let error = forge
            .find_open_pull_request(&tree, "takt/x", "main")
            .expect_err(garbled);
        assert!(format!("{error:#}").contains("find"), "{error:#}");
    }
}

/// GitLab: the resumed run lists the merge requests opened from the case's branch into its base
/// through the API and takes the one there; one `POST` across both runs.
#[test]
fn gitlab_finds_the_open_merge_request_through_the_api() {
    let scratch = Scratch::new("gitlab");
    let tree = scratch.tree("https://forge.test/group/repo.git");
    let host = GitLabHost::default();
    let forge: Rc<dyn Forge> = Rc::new(Glab::new(
        "forge.test",
        "group/repo",
        scratch.scan(),
        Box::new(host.clone()),
    ));
    let (first, case_dir, revision) = first_run(&tree, "gitlab", Cut::over(forge.clone()));
    assert_eq!(host.0.opens(), 1, "{}", said(&first));
    assert_cut_off(&first, &case_dir);

    let second = resumed(&tree, "gitlab", forge);
    assert_eq!(
        host.0.opens(),
        1,
        "one POST across both runs: {:#?}\n{}",
        host.0.lines.borrow(),
        said(&second)
    );
    assert_taken(&second, &case_dir, 7, &revision);
    assert_eq!(
        host.0.finds(),
        [
            "find takt/gitlab main opened",
            "find takt/gitlab main opened"
        ],
        "one list before the first POST, one on the resume"
    );
}

/// GitLab's answer to the list is read strictly: anything but a list of merge requests, each
/// with its number, state and branches, is an error, never none; two open for one branch and
/// base is an error too.
#[test]
fn a_gitlab_answer_that_does_not_read_is_an_error() {
    let scratch = Scratch::new("gitlab-garbled");
    let tree = scratch.tree("https://forge.test/group/repo.git");
    let host = GitLabHost::default();
    let forge = Glab::new(
        "forge.test",
        "group/repo",
        scratch.scan(),
        Box::new(host.clone()),
    );
    assert_eq!(
        forge
            .find_open_pull_request(&tree, "takt/x", "main")
            .expect("an empty list is none"),
        None
    );
    let one =
        r#"{"iid": 7, "state": "opened", "source_branch": "takt/x", "target_branch": "main"}"#;
    let two =
        r#"{"iid": 8, "state": "opened", "source_branch": "takt/x", "target_branch": "main"}"#;
    for garbled in [
        "not json".to_owned(),
        r#"{"message": "a list was expected"}"#.to_owned(),
        r#"[{"state": "opened", "source_branch": "takt/x", "target_branch": "main"}]"#.to_owned(),
        r#"[{"iid": 7, "source_branch": "takt/x", "target_branch": "main"}]"#.to_owned(),
        r#"[{"iid": 7, "state": "opened", "target_branch": "main"}]"#.to_owned(),
        r#"[{"iid": "7", "state": "opened", "source_branch": "takt/x", "target_branch": "main"}]"#
            .to_owned(),
        format!("[{one}, {two}]"),
    ] {
        host.0.garbled.replace(Some(garbled.clone()));
        let found = forge.find_open_pull_request(&tree, "takt/x", "main");
        assert!(found.is_err(), "{garbled}: {found:?}");
    }
    host.0.garbled.replace(Some(format!("[{one}]")));
    assert_eq!(
        forge
            .find_open_pull_request(&tree, "takt/x", "main")
            .expect("one there"),
        Some(7)
    );
}

/// The local forge: the resumed run reads the request records in the hub, takes the open one
/// for the case's branch into its base, and writes no second record.
#[test]
fn the_local_forge_finds_the_open_request_in_the_hub() {
    let scratch = Scratch::new("local");
    let tree = scratch.hub_and_tree();
    let identity = CommitIdentity::new("Test Bot", "bot@example.invalid").expect("an identity");
    let forge: Rc<dyn Forge> = Rc::new(
        Local::for_tree(
            &tree,
            vec!["true".to_owned()],
            scratch.0.join("state"),
            identity,
        )
        .expect("the hub is a local bare repository outside the tree"),
    );
    let requests = scratch.0.join("hub.git").join(Local::REQUESTS);
    let records = || {
        fs::read_dir(&requests)
            .map(|entries| entries.count())
            .unwrap_or(0)
    };
    let (first, case_dir, revision) = first_run(&tree, "local", Cut::over(forge.clone()));
    assert_eq!(records(), 1, "{}", said(&first));
    assert_cut_off(&first, &case_dir);

    let second = resumed(&tree, "local", forge);
    assert_eq!(
        records(),
        1,
        "one request across both runs: {}",
        said(&second)
    );
    assert_taken(&second, &case_dir, 1, &revision);
}

/// A record in the hub that does not read is an error, never none; requests for other branches
/// and bases, or no longer open, are passed over.
#[test]
fn the_local_forge_reads_its_records_strictly() {
    let scratch = Scratch::new("local-strict");
    let tree = scratch.hub_and_tree();
    let identity = CommitIdentity::new("Test Bot", "bot@example.invalid").expect("an identity");
    let forge = Local::for_tree(
        &tree,
        vec!["true".to_owned()],
        scratch.0.join("state"),
        identity,
    )
    .expect("the hub is a local bare repository outside the tree");
    assert_eq!(
        forge
            .find_open_pull_request(&tree, "takt/x", "main")
            .expect("no record is none"),
        None
    );
    let requests = scratch.0.join("hub.git").join(Local::REQUESTS);
    fs::create_dir_all(&requests).expect("the records directory");
    let record = |branch: &str, base: &str, state: &str| {
        serde_json::json!({
            "branch": branch, "base": base, "title": "t", "body": "b", "state": state,
        })
        .to_string()
    };
    fs::write(requests.join("1.json"), record("takt/x", "main", "merged")).expect("a record");
    fs::write(requests.join("2.json"), record("takt/y", "main", "open")).expect("a record");
    fs::write(requests.join("3.json"), record("takt/x", "other", "open")).expect("a record");
    fs::write(requests.join("4.json"), record("takt/x", "main", "open")).expect("a record");
    fs::write(requests.join("4.json.123.next"), "half").expect("a write in flight");
    assert_eq!(
        forge
            .find_open_pull_request(&tree, "takt/x", "main")
            .expect("one open"),
        Some(4)
    );
    fs::write(requests.join("5.json"), "").expect("a record never written");
    assert!(
        forge
            .find_open_pull_request(&tree, "takt/x", "main")
            .is_err()
    );
    fs::write(requests.join("5.json"), record("takt/x", "main", "open")).expect("a record");
    assert!(
        forge
            .find_open_pull_request(&tree, "takt/x", "main")
            .is_err(),
        "two open for one branch and base"
    );
}
