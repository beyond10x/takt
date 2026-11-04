//! The adversary pass on wave 53: cases that try to break each of the wave's four units.
//!
//! - `slow_runner`: story:a-slow-runner-is-no-late-ci-result, the one-cap allowance of
//!   `ci_wait_backoff.rs` against a run one whole cap late, judged on measurements.
//! - `merge_read`: story:merge-is-not-sent-after-a-merged-read, the read right before the merge
//!   finding the pull request merged or closed at a head that is not takt's.
//! - `found`: story:pr-open-cut-off-by-a-crash-is-read-back, what `find_open_pull_request`
//!   answers and what `pr.open` does with it.
//! - `through`: story:program-rewritten-through-a-link-refuses-the-step, links in a program
//!   directory that reach their program by another way than one absolute link.
//!
//! Every forge here is a double or a local bare repository; nothing leaves the machine.

#[path = "support/one_cap.rs"]
mod one_cap;
#[path = "support/own_home.rs"]
mod own_home;

use std::path::Path;
use std::process::Command;
use std::sync::{Mutex, MutexGuard};

/// Every case runs steps under the guard, and the `through` cases write links into this
/// binary's one program directory, which every other case's guard reads: they run one at a time.
static ONE_AT_A_TIME: Mutex<()> = Mutex::new(());

fn one_at_a_time() -> MutexGuard<'static, ()> {
    ONE_AT_A_TIME
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

fn git(tree: &Path, args: &[&str]) -> String {
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
        .current_dir(tree)
        .output()
        .expect("git runs");
    assert!(
        output.status.success(),
        "git {args:?} in {}: {}",
        tree.display(),
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout)
        .expect("utf-8")
        .trim()
        .to_owned()
}

/// A scratch directory under this crate's build directory, removed on drop.
struct Scratch(std::path::PathBuf);

impl Scratch {
    fn new(name: &str) -> Self {
        let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
            .join(format!("w53-adv-{}-{name}", std::process::id()));
        if dir.exists() {
            std::fs::remove_dir_all(&dir).expect("a stale scratch directory is removed");
        }
        std::fs::create_dir_all(&dir).expect("a scratch directory");
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
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

// ---------------------------------------------------------------------------------------------
// Unit d: the one-cap allowance on a slow runner.

mod slow_runner {
    //! The one-cap rule `ci_wait_backoff.rs` ships (`support/one_cap.rs`), judged on the
    //! measurements of a run, not on a run itself: a grid of real runs measures the runner as
    //! much as the rule, and on a loaded host a run one cap late can fall inside `cap + own` at
    //! every offset, so such a case fails or passes by the host's load. The measurements below
    //! are the ones the finding and its runners produced.

    use std::time::Duration;

    use super::one_cap::Measured;

    const CAP: Duration = Duration::from_millis(225);

    /// A run that saw the checks pass at the read after the green, `own` its own time between
    /// two reads, and wrote the record `write` after that read; `late` more wait than the rule
    /// allows before that read.
    fn run(own: Duration, late: Duration, write: Duration) -> Measured {
        Measured {
            cap: CAP,
            own,
            detected: CAP + own + late,
            written: write,
        }
    }

    /// The allowance before 6e2641054, at 112943ab2: `cap + own + 20 ms` from the green to the
    /// record.
    fn before(measured: &Measured) -> bool {
        measured.detected + measured.written
            > measured.cap + measured.own + Duration::from_millis(20)
    }

    /// The allowance of 6e2641054: `cap + own + max(own, 20 ms)` from the green to the record.
    fn first_fix(measured: &Measured) -> bool {
        measured.detected + measured.written
            > measured.cap + measured.own + measured.own.max(Duration::from_millis(20))
    }

    /// story:a-slow-runner-is-no-late-ci-result, Outcome: "a run that waits a whole interval
    /// too long still does" turn the case red, on a runner of no delay and on one 300 ms and
    /// 524 ms slow between its reads.
    #[test]
    fn a_whole_cap_late_result_is_flagged_on_any_runner() {
        for own in [0, 300, 524].map(Duration::from_millis) {
            let late = run(own, CAP, Duration::from_millis(5));
            assert!(
                late.late(),
                "a run one whole cap late on a runner {own:?} slow between its reads is not flagged: {late:?}, story:a-slow-runner-is-no-late-ci-result"
            );
        }
    }

    /// The first fix let a run one cap late through once the runner's own time passed about one
    /// interval (the adversary's finding); the allowance before it flagged that run.
    #[test]
    fn the_first_fix_let_a_whole_cap_late_result_through_and_the_old_rule_did_not() {
        let late = run(Duration::from_millis(300), CAP, Duration::from_millis(5));
        assert!(!first_fix(&late), "the first fix flags {late:?} after all");
        assert!(
            before(&late),
            "the rule before the change does not flag {late:?}"
        );
    }

    /// The finding's darwin runner: own 524 ms, the record 863 ms after the green. On time, it
    /// passes under the shipped rule, though the rule before the change turned it red.
    #[test]
    fn the_darwin_run_on_time_passes() {
        let own = Duration::from_millis(524);
        let on_time = Measured {
            cap: CAP,
            own,
            detected: CAP + own,
            written: Duration::from_millis(863) - (CAP + own),
        };
        assert!(
            !on_time.late(),
            "the darwin run on time is flagged: {on_time:?}"
        );
        assert!(
            before(&on_time),
            "the rule before the change passes the darwin run"
        );
    }

    /// A write that held a wait of one first interval (2 s by default) is flagged, however fast
    /// the read before it was.
    #[test]
    fn a_write_that_holds_a_wait_is_flagged() {
        let slow_write = run(Duration::ZERO, Duration::ZERO, Duration::from_secs(2));
        assert!(
            slow_write.late(),
            "a write of 2 s is not flagged: {slow_write:?}"
        );
    }
}

// ---------------------------------------------------------------------------------------------
// Unit a: the read right before the merge.

mod merge_read {
    use std::cell::{Cell, RefCell};
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::rc::Rc;
    use std::time::Duration;

    use anyhow::anyhow;
    use takt::case::grant;
    use takt::evidence::Record;
    use takt::forge::{
        Checks, Forge, IdentityScan, Memory, MergeMessage, Outgoing, PullRequest, PullRequestState,
        PullRequestView, Written,
    };
    use takt::harness::{Harness, StepContext, StepReport};
    use takt::validator::{CiRead, PrMerged, PrOpen, Validator};
    use takt::{Run, RunOptions, Stop};

    use super::{Scratch, git};

    const PROTOCOL: &str = "story.delivery/1";

    fn scratch(name: &str) -> Scratch {
        let dir = Scratch::new(&format!("merge-{name}"));
        fs::write(dir.join("README.md"), "a story\n").expect("a file");
        git(&dir, &["init", "-q", "-b", "main"]);
        git(&dir, &["add", "-A"]);
        git(&dir, &["commit", "-q", "-m", "start"]);
        dir
    }

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
            Err(anyhow!("the gate was not offered"))
        }
    }

    fn options(id: &str) -> RunOptions {
        RunOptions {
            protocol: PROTOCOL.to_owned(),
            intent: "deliver the story\n\nThe story's body.".to_owned(),
            inputs: Vec::new(),
            iterations: Some(8),
            hours: None,
            tokens: None,
            case_id: Some(id.to_owned()),
            gate: Some("repository.merge".to_owned()),
        }
    }

    /// [`Memory`], whose reads answer as the double holds it until `first` more reads are
    /// taken, then with `state` at `sha`, once armed.
    struct Second {
        inner: Rc<Memory>,
        first: Cell<u32>,
        later: RefCell<Option<(PullRequestState, String)>>,
    }

    impl Forge for Second {
        fn name(&self) -> &str {
            "second-read-memory"
        }

        fn scan_outgoing(
            &self,
            tree: &Path,
            dir: &Path,
            outgoing: &Outgoing<'_>,
        ) -> anyhow::Result<()> {
            self.inner.scan_outgoing(tree, dir, outgoing)
        }

        fn push(&self, tree: &Path, branch: &str) -> anyhow::Result<()> {
            self.inner.push(tree, branch)
        }

        fn push_revision(&self, tree: &Path, branch: &str, revision: &str) -> anyhow::Result<()> {
            self.inner.push_revision(tree, branch, revision)
        }

        fn open_pull_request(
            &self,
            tree: &Path,
            dir: &Path,
            request: &PullRequest,
        ) -> Written<u64> {
            self.inner.open_pull_request(tree, dir, request)
        }

        fn pull_request(&self, tree: &Path, number: u64) -> anyhow::Result<PullRequestView> {
            let mut view = self.inner.pull_request(tree, number)?;
            let Some((state, sha)) = self.later.borrow().clone() else {
                return Ok(view);
            };
            if self.first.get() > 0 {
                self.first.set(self.first.get() - 1);
                return Ok(view);
            }
            view.state = state;
            view.head.sha = sha;
            Ok(view)
        }

        fn behind_base(&self, tree: &Path, number: u64) -> anyhow::Result<Option<bool>> {
            self.inner.behind_base(tree, number)
        }

        fn checks(&self, tree: &Path, number: u64) -> anyhow::Result<Checks> {
            self.inner.checks(tree, number)
        }

        fn merge(&self, tree: &Path, dir: &Path, number: u64, head: &str) -> Written<()> {
            self.inner.merge(tree, dir, number, head)
        }

        fn merge_with(
            &self,
            tree: &Path,
            dir: &Path,
            number: u64,
            head: &str,
            _: &MergeMessage,
        ) -> Written<()> {
            self.merge(tree, dir, number, head)
        }

        fn scan_identity(
            &self,
            tree: &Path,
            dir: &Path,
            identity: &takt::git::CommitIdentity,
        ) -> anyhow::Result<IdentityScan> {
            self.inner.scan_identity(tree, dir, identity)
        }

        fn target_tip(&self, tree: &Path, base: &str) -> anyhow::Result<String> {
            self.inner.target_tip(tree, base)
        }
    }

    fn validators(forge: Rc<dyn Forge>) -> Vec<Box<dyn Validator>> {
        vec![
            Box::new(PrOpen::new(forge.clone())),
            Box::new(CiRead::new(forge.clone())),
            Box::new(PrMerged::new(forge)),
        ]
    }

    fn granted(tree: &Path, id: &str, forge: &Rc<Second>) -> PathBuf {
        let dynamic: Rc<dyn Forge> = forge.clone();
        let validators = validators(dynamic);
        let mut run =
            Run::open(tree, options(id), &Requester, &validators).expect("the case opens");
        fs::write(tree.join("implemented.txt"), "done\n").expect("a file");
        git(tree, &["add", "implemented.txt"]);
        git(tree, &["commit", "-q", "-m", "implemented by the test"]);
        let revision = git(tree, &["rev-parse", "HEAD"]);
        run.case
            .set_revision("implementation", &revision)
            .expect("the revision moves");
        for (n, (kind, result, producer)) in [
            ("test_result", Some("pass"), "w53-adversary"),
            ("gate_result", Some("pass"), "w53-adversary"),
            ("handover", None, takt::run::CHOICE_PRODUCER),
            ("change_record", Some("ahead"), "takt.change-read"),
        ]
        .into_iter()
        .enumerate()
        {
            run.case
                .add_evidence(&Record::new(
                    format!("e{n}"),
                    kind,
                    result.map(str::to_owned),
                    "implementation",
                    &revision,
                    producer,
                ))
                .expect("the record is written");
        }
        let mut out = Vec::new();
        let stop = run.run(&mut out).expect("the run stops without an error");
        let text = String::from_utf8(out).expect("utf-8");
        assert_eq!(
            stop,
            Stop::ApprovalRequired(vec!["repository.merge".to_owned()]),
            "{text}"
        );
        grant(tree, id, "repository.merge", "human:operator").expect("the grant is written");
        run.case.dir.clone()
    }

    fn merge_observations(case_dir: &Path) -> Vec<String> {
        let Ok(entries) = fs::read_dir(case_dir.join("evidence")) else {
            return Vec::new();
        };
        entries
            .filter_map(|entry| {
                let text = fs::read_to_string(entry.expect("an entry").path()).expect("a record");
                let record: serde_json::Value = serde_json::from_str(&text).expect("json");
                (record["kind"] == "merge_observation")
                    .then(|| record["result"].as_str().unwrap_or_default().to_owned())
            })
            .collect()
    }

    /// The read right before the merge finds the pull request merged or closed, at `main`'s
    /// commit, a head that is not takt's (another hand force-pushed the branch, then merged or
    /// closed it): no merge is sent, no `merge_observation` is written for a head takt never
    /// pushed, and the step says the head is not the case's.
    #[test]
    fn merged_or_closed_right_before_the_merge_at_a_head_not_takts_records_nothing() {
        let _one = super::one_at_a_time();
        for (name, state) in [
            ("merged-foreign", PullRequestState::Merged),
            ("closed-foreign", PullRequestState::Closed),
        ] {
            let tree = scratch(name);
            let memory = Rc::new(Memory {
                checks: Checks::Pass,
                ..Memory::default()
            });
            let forge = Rc::new(Second {
                inner: memory.clone(),
                first: Cell::new(0),
                later: RefCell::new(None),
            });
            let case_dir = granted(&tree, name, &forge);
            let main = git(&tree, &["rev-parse", "main"]);
            forge.first.set(1);
            forge.later.replace(Some((state, main)));
            let validators = validators(forge.clone());
            let mut run = Run::resume(&tree, options(name), name, &NoHarness, &validators)
                .expect("the case resumes");
            run.poll = Duration::from_millis(5);
            let mut out = Vec::new();
            let stop = run.run(&mut out).map_err(|error| format!("{error:#}"));
            let text = format!(
                "{stop:?}\n{}\ncalls: {:#?}",
                String::from_utf8_lossy(&out),
                memory.calls.borrow()
            );
            let merges = memory
                .calls
                .borrow()
                .iter()
                .filter(|call| call.starts_with("merge #"))
                .count();
            assert_eq!(
                merges, 0,
                "{name}: no merge is sent, story:merge-is-not-sent-after-a-merged-read\n{text}"
            );
            assert!(
                merge_observations(&case_dir).is_empty(),
                "{name}: nothing observed for a head takt never pushed, story:merge-is-not-sent-after-a-merged-read\n{text}"
            );
            assert!(
                text.contains("nothing to merge"),
                "{name}: the step says the head is not the case's\n{text}"
            );
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Unit b: what the forges find, and what `pr.open` does with it.

mod found {
    use std::cell::RefCell;
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::rc::Rc;

    use anyhow::anyhow;
    use takt::case::Branch;
    use takt::forge::command::CommandForge;
    use takt::forge::gitlab::Glab;
    use takt::forge::local::Local;
    use takt::forge::{Exit, Forge, Runner};
    use takt::git::CommitIdentity;
    use takt::harness::{Harness, StepContext, StepReport};
    use takt::run_config::RunConfig;
    use takt::scan::Scan;
    use takt::validator::{PrOpen, Validator};
    use takt::{Run, RunOptions, Stop};

    use super::{Scratch, git};

    const PROTOCOL: &str = "story.delivery/1";

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

    fn options(id: &str) -> RunOptions {
        RunOptions {
            protocol: PROTOCOL.to_owned(),
            intent: "deliver the story\n\nThe story's body.".to_owned(),
            inputs: Vec::new(),
            iterations: Some(1),
            hours: None,
            tokens: None,
            case_id: Some(id.to_owned()),
            gate: Some("repository.merge".to_owned()),
        }
    }

    /// A tree with one commit on `main` and `origin` at `url`.
    fn tree_at(scratch: &Scratch, url: &str) -> PathBuf {
        let tree = scratch.join("tree");
        fs::create_dir_all(&tree).expect("a tree");
        fs::write(tree.join("README.md"), "a story\n").expect("a file");
        git(&tree, &["init", "-q", "-b", "main"]);
        git(&tree, &["add", "-A"]);
        git(&tree, &["commit", "-q", "-m", "start"]);
        git(&tree, &["remote", "add", "origin", url]);
        tree
    }

    /// A bare hub `hub.git` with one commit on `main`, and a tree cloned from it.
    fn hub_and_tree(scratch: &Scratch) -> PathBuf {
        let seed = scratch.join("seed");
        fs::create_dir_all(&seed).expect("a directory");
        fs::write(seed.join("README.md"), "a story\n").expect("a file");
        git(&seed, &["init", "-q", "-b", "main"]);
        git(&seed, &["add", "-A"]);
        git(&seed, &["commit", "-q", "-m", "start"]);
        git(scratch, &["clone", "-q", "--bare", "seed", "hub.git"]);
        fs::remove_dir_all(&seed).expect("the seed is removed");
        git(scratch, &["clone", "-q", "--no-local", "hub.git", "tree"]);
        scratch.join("tree")
    }

    /// One `pr.open` step of case `id` through `forge`, as the crash cases of unit b take it:
    /// the stop (an error as its text) with what the run printed, and the case directory.
    fn one_open(tree: &Path, id: &str, forge: Rc<dyn Forge>) -> (String, PathBuf) {
        let validators: Vec<Box<dyn Validator>> = vec![Box::new(PrOpen::new(forge))];
        let mut run =
            Run::open(tree, options(id), &NoHarness, &validators).expect("the case opens");
        fs::write(tree.join("done.txt"), "done\n").expect("a file");
        git(tree, &["add", "done.txt"]);
        git(tree, &["commit", "-q", "-m", "the work"]);
        let revision = git(tree, &["rev-parse", "HEAD"]);
        run.case
            .set_revision("implementation", &revision)
            .expect("the revision moves");
        for (n, (kind, result, producer)) in [
            ("test_result", Some("pass"), "w53-adversary"),
            ("gate_result", Some("pass"), "w53-adversary"),
            ("handover", None, takt::run::CHOICE_PRODUCER),
            ("change_record", Some("ahead"), "takt.change-read"),
        ]
        .into_iter()
        .enumerate()
        {
            run.case
                .add_evidence(&takt::evidence::Record::new(
                    format!("e{n}"),
                    kind,
                    result.map(str::to_owned),
                    "implementation",
                    &revision,
                    producer,
                ))
                .expect("the record is written");
        }
        let mut out = Vec::new();
        let stop: Result<Stop, String> = run.run(&mut out).map_err(|error| format!("{error:#}"));
        (
            format!("{stop:?}\n{}", String::from_utf8_lossy(&out)),
            run.case.dir.clone(),
        )
    }

    fn pull_request_results(case_dir: &Path) -> Vec<String> {
        let Ok(entries) = fs::read_dir(case_dir.join("evidence")) else {
            return Vec::new();
        };
        entries
            .filter_map(|entry| {
                let text = fs::read_to_string(entry.expect("an entry").path()).expect("a record");
                let record: serde_json::Value = serde_json::from_str(&text).expect("json");
                (record["kind"] == "pull_request")
                    .then(|| record["result"].as_str().unwrap_or_default().to_owned())
            })
            .collect()
    }

    fn exit(code: i32, stdout: &str) -> Exit {
        Exit {
            code: Some(code),
            stdout: stdout.to_owned(),
            stderr: String::new(),
        }
    }

    /// The program of a command forge whose `find` prints `9`, request 9 being open from
    /// `other_branch` (the case's own when none) into `other_base`; `open` opens request 7 for
    /// what it is given. Every call is a line in `lines`.
    #[derive(Clone)]
    struct Program {
        other_branch: Option<String>,
        other_base: String,
        pushed: Rc<RefCell<String>>,
        opened: Rc<RefCell<Option<(String, String)>>>,
        lines: Rc<RefCell<Vec<String>>>,
    }

    impl Runner for Program {
        fn run(&self, tree: &Path, argv: &[String]) -> anyhow::Result<Exit> {
            self.run_env(tree, argv, &[])
        }

        fn run_env(&self, _: &Path, argv: &[String], _: &[(&str, &str)]) -> anyhow::Result<Exit> {
            let words: Vec<&str> = argv.iter().map(String::as_str).collect();
            self.lines.borrow_mut().push(words.join(" "));
            Ok(match words.as_slice() {
                ["git", ..] => {
                    if let Some((sha, _)) = argv.iter().find_map(|w| w.split_once(":refs/heads/")) {
                        self.pushed.replace(sha.to_owned());
                    }
                    exit(0, "")
                }
                ["tool", "find", ..] => exit(0, "9\n"),
                ["tool", "open", branch, base] => {
                    self.opened
                        .replace(Some(((*branch).to_owned(), (*base).to_owned())));
                    exit(0, "https://forge.test/group/repo/-/merge_requests/7\n")
                }
                ["tool", "read", "9"] => {
                    let case_branch = self
                        .lines
                        .borrow()
                        .iter()
                        .find_map(|line| {
                            line.strip_prefix("tool find ")
                                .and_then(|rest| rest.split_once(' '))
                                .map(|(branch, _)| branch.to_owned())
                        })
                        .unwrap_or_default();
                    let branch = self.other_branch.clone().unwrap_or(case_branch);
                    exit(
                        0,
                        &serde_json::json!({
                            "state": "opened",
                            "head": self.pushed.borrow().clone(),
                            "branch": branch,
                            "base": self.other_base,
                        })
                        .to_string(),
                    )
                }
                ["tool", "read", "7"] => {
                    let (branch, base) = self.opened.borrow().clone().unwrap_or_default();
                    exit(
                        0,
                        &serde_json::json!({
                            "state": "opened",
                            "head": self.pushed.borrow().clone(),
                            "branch": branch,
                            "base": base,
                        })
                        .to_string(),
                    )
                }
                _ => exit(1, ""),
            })
        }
    }

    fn command_forge(scratch: &Scratch, tree: &Path, program: &Program) -> CommandForge {
        let path = scratch.join("run.yaml");
        fs::write(
            &path,
            "forge: command\nscan: none\nforge_commands:\n  open: [tool, open, \"{branch}\", \"{base}\"]\n  read: [tool, read, \"{number}\"]\n  checks: [tool, checks, \"{number}\", \"{head}\"]\n  merge: [tool, merge, \"{number}\", \"{head}\"]\n  find: [tool, find, \"{branch}\", \"{base}\"]\n  refused_exit: [3]\n",
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
            scratch.join("state"),
            Scan::None {
                declared_in: scratch.join("run.yaml"),
            },
            Box::new(program.clone()),
        )
        .expect("the command forge")
    }

    fn program(other_branch: Option<&str>, other_base: &str) -> Program {
        Program {
            other_branch: other_branch.map(str::to_owned),
            other_base: other_base.to_owned(),
            pushed: Rc::default(),
            opened: Rc::default(),
            lines: Rc::default(),
        }
    }

    /// A command forge's `find` that prints the number of a request open from another branch
    /// (a script that lists by base alone, say): `open` reads its own number back through
    /// `read` and refuses one that is not the case's branch, so `find`'s number should be held
    /// to the same. The case must not take request 9 as its own.
    #[test]
    fn a_number_find_prints_for_another_branch_is_not_taken_as_the_case_s() {
        let _one = super::one_at_a_time();
        let scratch = Scratch::new("find-other-branch");
        let tree = tree_at(&scratch, "https://forge.test/group/repo.git");
        let program = program(Some("takt/someone-else"), "main");
        let forge: Rc<dyn Forge> = Rc::new(command_forge(&scratch, &tree, &program));
        let (said, case_dir) = one_open(&tree, "other-branch", forge);
        let held = Branch::read(&case_dir).expect("branch.yaml").pull_request;
        assert_ne!(
            held,
            Some(9),
            "the case took request 9, open from takt/someone-else, as its own pull request: every later pass reads #9 and stays unknown, story:pr-open-cut-off-by-a-crash-is-read-back\nlines: {:#?}\n{said}",
            program.lines.borrow()
        );
    }

    /// A command forge's `find` that prints the number of a request open from the case's branch
    /// into another base (a script that lists by head branch alone): the case must not record
    /// that request `open` as its pull request into its own base.
    #[test]
    fn a_number_find_prints_for_another_base_is_not_recorded_open() {
        let _one = super::one_at_a_time();
        let scratch = Scratch::new("find-other-base");
        let tree = tree_at(&scratch, "https://forge.test/group/repo.git");
        let program = program(None, "release");
        let forge: Rc<dyn Forge> = Rc::new(command_forge(&scratch, &tree, &program));
        let (said, case_dir) = one_open(&tree, "other-base", forge);
        let held = Branch::read(&case_dir).expect("branch.yaml").pull_request;
        let results = pull_request_results(&case_dir);
        assert!(
            held != Some(9) && results.is_empty(),
            "the case took request 9, open into release, not main, as its pull request (held {held:?}, pull_request records {results:?}), story:pr-open-cut-off-by-a-crash-is-read-back\nlines: {:#?}\n{said}",
            program.lines.borrow()
        );
    }

    /// The local forge: what another case's open leaves when its process is gone mid-write (or
    /// is still writing) is no request the hub holds. Before the fix that was an empty
    /// `<n>.json`, which the strict `find` could not read, so it stopped every `pr.open` on the
    /// hub; an open now writes `<n>.json.<pid>.next` and links it, so the state a crash leaves
    /// is an empty `.next` file. A case of another branch on the same hub still opens its own
    /// pull request. (A hand-made empty `<n>.json` is no state the forge leaves, and the strict
    /// read refuses it: `the_local_forge_reads_its_records_strictly`.)
    #[test]
    fn an_unfinished_record_of_another_case_stops_no_open_on_the_local_hub() {
        let _one = super::one_at_a_time();
        let scratch = Scratch::new("local-empty-other");
        let tree = hub_and_tree(&scratch);
        let requests = scratch.join("hub.git").join(Local::REQUESTS);
        fs::create_dir_all(&requests).expect("the records directory");
        fs::write(requests.join("1.json.4242.next"), "")
            .expect("a record another case never finished");
        let identity = CommitIdentity::new("Test Bot", "bot@example.invalid").expect("an identity");
        let forge: Rc<dyn Forge> = Rc::new(
            Local::for_tree(
                &tree,
                vec!["true".to_owned()],
                scratch.join("state"),
                identity,
            )
            .expect("the local forge"),
        );
        let (said, case_dir) = one_open(&tree, "local-other", forge);
        let held = Branch::read(&case_dir).expect("branch.yaml").pull_request;
        assert_eq!(
            held,
            Some(1),
            "an unfinished record #1 left by another case stopped this case's open on the hub, story:pr-open-cut-off-by-a-crash-is-read-back\n{said}"
        );
    }

    /// A runner that answers every line with `stdout`, exit 0.
    struct Fixed(String);

    impl Runner for Fixed {
        fn run(&self, tree: &Path, argv: &[String]) -> anyhow::Result<Exit> {
            self.run_env(tree, argv, &[])
        }

        fn run_env(&self, _: &Path, _: &[String], _: &[(&str, &str)]) -> anyhow::Result<Exit> {
            Ok(exit(0, &self.0))
        }
    }

    /// GitLab's list, read strictly, passes over a closed or merged request of the branch, one
    /// into another base and one from another branch, and takes the one open from the branch
    /// into the base.
    #[test]
    fn gitlab_passes_over_closed_and_other_base_requests_in_its_list() {
        let _one = super::one_at_a_time();
        let scratch = Scratch::new("gitlab-list");
        let tree = tree_at(&scratch, "https://forge.test/group/repo.git");
        let entry = |iid: u64, state: &str, source: &str, target: &str| {
            serde_json::json!({
                "iid": iid, "state": state, "source_branch": source, "target_branch": target,
            })
        };
        let list = serde_json::json!([
            entry(3, "closed", "takt/x", "main"),
            entry(4, "merged", "takt/x", "main"),
            entry(5, "opened", "takt/x", "release"),
            entry(6, "opened", "takt/y", "main"),
            entry(12, "opened", "takt/x", "main"),
        ]);
        let forge = Glab::new(
            "forge.test",
            "group/repo",
            Scan::None {
                declared_in: scratch.join("run.yaml"),
            },
            Box::new(Fixed(list.to_string())),
        );
        assert_eq!(
            forge
                .find_open_pull_request(&tree, "takt/x", "main")
                .expect("the list reads"),
            Some(12)
        );
        let only_others = serde_json::json!([
            entry(3, "closed", "takt/x", "main"),
            entry(5, "opened", "takt/x", "release"),
        ]);
        let forge = Glab::new(
            "forge.test",
            "group/repo",
            Scan::None {
                declared_in: scratch.join("run.yaml"),
            },
            Box::new(Fixed(only_others.to_string())),
        );
        assert_eq!(
            forge
                .find_open_pull_request(&tree, "takt/x", "main")
                .expect("the list reads"),
            None
        );
    }

    /// A runner whose `find` prints `printed` and whose `read` reports any request open from
    /// `takt/x` into `main`, as the number `find` printed must read back.
    struct Printed(String);

    impl Runner for Printed {
        fn run(&self, tree: &Path, argv: &[String]) -> anyhow::Result<Exit> {
            self.run_env(tree, argv, &[])
        }

        fn run_env(&self, _: &Path, argv: &[String], _: &[(&str, &str)]) -> anyhow::Result<Exit> {
            Ok(match argv.get(1).map(String::as_str) {
                Some("read") => exit(
                    0,
                    r#"{"state": "opened", "head": "0123456789abcdef0123456789abcdef01234567", "branch": "takt/x", "base": "main"}"#,
                ),
                _ => exit(0, &self.0),
            })
        }
    }

    /// What `find` prints: a number with `#` or `!` before it, or surrounded by blank space, is
    /// that number; two lines, or a number after other words, are an error.
    #[test]
    fn command_find_reads_a_marked_number_and_refuses_more_than_one() {
        let _one = super::one_at_a_time();
        let scratch = Scratch::new("find-printed");
        let tree = tree_at(&scratch, "https://forge.test/group/repo.git");
        let make = |printed: &str| {
            let path = scratch.join("run.yaml");
            fs::write(
                &path,
                "forge: command\nscan: none\nforge_commands:\n  open: [tool, open, \"{branch}\", \"{base}\"]\n  read: [tool, read, \"{number}\"]\n  checks: [tool, checks, \"{number}\", \"{head}\"]\n  merge: [tool, merge, \"{number}\", \"{head}\"]\n  find: [tool, find, \"{branch}\", \"{base}\"]\n",
            )
            .expect("a run file");
            let config = RunConfig::read(&path).expect("the run file reads");
            CommandForge::for_tree(
                &tree,
                config.forge_commands.expect("the section"),
                &["pr.open".to_owned()],
                scratch.join("state"),
                Scan::None {
                    declared_in: path.clone(),
                },
                Box::new(Printed(printed.to_owned())),
            )
            .expect("the command forge")
        };
        for (printed, number) in [("#7\n", 7), ("!7", 7), ("  7  \n\n", 7)] {
            assert_eq!(
                make(printed)
                    .find_open_pull_request(&tree, "takt/x", "main")
                    .expect(printed),
                Some(number),
                "{printed:?}"
            );
        }
        for printed in ["7\n8\n", "found 7\n", "#\n", "18446744073709551616\n"] {
            assert!(
                make(printed)
                    .find_open_pull_request(&tree, "takt/x", "main")
                    .is_err(),
                "{printed:?}"
            );
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Unit c: links in a program directory that reach their program another way.

mod through {
    use super::one_at_a_time;
    use std::cell::RefCell;
    use std::fs;
    use std::os::unix::fs::PermissionsExt as _;
    use std::path::{Path, PathBuf};

    use takt::harness::{Harness, StepContext, StepReport};
    use takt::validator::{TestsRun, Validator};
    use takt::{Run, RunOptions, Stop};

    use super::{Scratch, git};

    const EDIT_AND_TEST: &str = "format: protocol/1

protocol:
  id: edit.and.test
  revision: 1
  description: >-
    Edit the implementation and run the tests against it; accepted once they pass.

artifacts:
  implementation:
    description: a revision of the code

evidence_kinds:
  test_result:
    description: the result of running tests against an implementation revision

claims:
  tests.pass:
    description: the tests pass for the current implementation revision
    true_when:
      evidence:
        kind: test_result
        result: pass
        subject: implementation

obligations:
  tests.required:
    description: the tests pass for the current implementation revision
    discharged_when:
      claim: tests.pass

actions:
  implementation.edit:
    description: change the implementation
    effect: write
  tests.run:
    description: run the tests against an implementation revision
    effect: read
    may_produce:
      - evidence: test_result

outcomes:
  accepted:
    description: the tests pass for the current implementation revision
    requires:
      claim: tests.pass
";

    /// This binary's own `$CARGO_HOME/bin`, under the private directory `own_home` made.
    fn own_program_dir() -> PathBuf {
        let own = std::env::var_os("TAKT_TEST_OWN_HOME")
            .map(PathBuf::from)
            .expect("TAKT_TEST_OWN_HOME: this test binary has no program directories of its own");
        let own = fs::canonicalize(own).expect("the private directory exists");
        let cargo = std::env::var_os("CARGO_HOME")
            .map(PathBuf::from)
            .expect("a CARGO_HOME of this binary's own");
        let bin = cargo.join("bin");
        fs::create_dir_all(&bin).expect("the private cargo bin");
        let bin = fs::canonicalize(bin).expect("the private cargo bin exists");
        assert!(
            bin.starts_with(&own),
            "{} outside {}",
            bin.display(),
            own.display()
        );
        bin
    }

    fn install(path: &Path, text: &str) {
        fs::write(path, text).expect("a program");
        fs::set_permissions(path, fs::Permissions::from_mode(0o755)).expect("a mode");
    }

    /// Paths in this binary's program directory a case made, removed when dropped.
    struct Made(Vec<PathBuf>);

    impl Drop for Made {
        fn drop(&mut self) {
            for path in &self.0 {
                let _ = fs::remove_file(path);
                let _ = fs::remove_dir_all(path);
            }
        }
    }

    /// An `implementation.edit` step that, the first time, runs `write` (outside the tree),
    /// and changes nothing in the tree.
    struct Writes {
        write: RefCell<Option<Box<dyn FnOnce()>>>,
    }

    impl Harness for Writes {
        fn name(&self) -> &str {
            "writes"
        }

        fn step(&self, context: &StepContext<'_>) -> Result<StepReport, anyhow::Error> {
            if !context.offered.iter().any(|a| a == "implementation.edit") {
                anyhow::bail!("implementation.edit not offered");
            }
            if let Some(write) = self.write.borrow_mut().take() {
                write();
            }
            Ok(StepReport {
                action: "implementation.edit".to_owned(),
                summary: "changed nothing in the tree".to_owned(),
                tokens: Some(1),
            })
        }
    }

    /// A repository whose test command fails, so the run takes an edit step; and the protocol.
    fn repository(name: &str) -> (Scratch, PathBuf) {
        let scratch = Scratch::new(&format!("through-{name}"));
        let repo = scratch.join("repo");
        fs::create_dir_all(&repo).expect("a directory");
        fs::write(scratch.join("protocol.yaml"), EDIT_AND_TEST).expect("the protocol");
        fs::write(repo.join(".gitignore"), "target/\n").expect("a file");
        install(&repo.join("check.sh"), "#!/bin/sh\nexit 1\n");
        git(&repo, &["init", "-q", "-b", "main"]);
        git(&repo, &["add", "-A"]);
        git(&repo, &["commit", "-q", "-m", "start"]);
        (scratch, repo)
    }

    fn run(
        scratch: &Scratch,
        repo: &Path,
        write: Box<dyn FnOnce()>,
    ) -> (anyhow::Result<Stop>, String) {
        let validators: Vec<Box<dyn Validator>> =
            vec![Box::new(TestsRun::new("./check.sh").expect("a command"))];
        let options = RunOptions {
            protocol: scratch.join("protocol.yaml").display().to_string(),
            intent: "make the check pass".to_owned(),
            inputs: Vec::new(),
            iterations: Some(4),
            hours: None,
            tokens: None,
            case_id: None,
            gate: None,
        };
        let harness = Writes {
            write: RefCell::new(Some(write)),
        };
        let mut run = Run::open(repo, options, &harness, &validators).expect("the case opens");
        run.continue_past_red_base();
        let mut out = Vec::new();
        let stop = run.run(&mut out);
        (stop, String::from_utf8_lossy(&out).into_owned())
    }

    fn assert_refused_naming(stop: &anyhow::Result<Stop>, text: &str, link: &Path, what: &str) {
        let shown = link.display().to_string();
        match stop {
            Ok(Stop::StepRefused(why)) => assert!(
                why.contains(&shown),
                "{what}: the refusal names {shown}, story:program-rewritten-through-a-link-refuses-the-step: {why}"
            ),
            other => panic!(
                "{what}: the run stops StepRefused naming {shown}, not {:?}, story:program-rewritten-through-a-link-refuses-the-step\n{text}",
                other.as_ref().map_err(ToString::to_string)
            ),
        }
    }

    /// A link on `PATH` to a link to the program: the step rewrites the program at the end of
    /// the chain, not through either link.
    #[test]
    fn a_program_at_the_end_of_a_chain_of_links_rewritten_refuses_the_step() {
        let _one = one_at_a_time();
        let (scratch, repo) = repository("chain");
        let end = scratch.join("tool").join("probe");
        fs::create_dir_all(end.parent().expect("a parent")).expect("a directory");
        install(&end, "#!/bin/sh\nexit 0\n");
        let middle = scratch.join("middle");
        std::os::unix::fs::symlink(&end, &middle).expect("the middle link");
        let link = own_program_dir().join(format!("w53-chain-{}", std::process::id()));
        let _ = fs::remove_file(&link);
        std::os::unix::fs::symlink(&middle, &link).expect("a link on PATH");
        let _made = Made(vec![link.clone()]);
        let rewrite = end.clone();
        let (stop, text) = run(
            &scratch,
            &repo,
            Box::new(move || install(&rewrite, "#!/bin/sh\necho rewritten\nexit 0\n")),
        );
        assert_refused_naming(&stop, &text, &link, "a chain of links");
    }

    /// A link on `PATH` to a link whose own target the step points elsewhere: the program a
    /// lookup runs is another, the link on `PATH` unchanged.
    #[test]
    fn a_middle_link_pointed_at_another_program_refuses_the_step() {
        let _one = one_at_a_time();
        let (scratch, repo) = repository("middle");
        let first = scratch.join("first");
        let second = scratch.join("second");
        install(&first, "#!/bin/sh\nexit 0\n");
        install(&second, "#!/bin/sh\necho another\nexit 0\n");
        let middle = scratch.join("middle");
        std::os::unix::fs::symlink(&first, &middle).expect("the middle link");
        let link = own_program_dir().join(format!("w53-middle-{}", std::process::id()));
        let _ = fs::remove_file(&link);
        std::os::unix::fs::symlink(&middle, &link).expect("a link on PATH");
        let _made = Made(vec![link.clone()]);
        let (moved, to) = (middle.clone(), second.clone());
        let (stop, text) = run(
            &scratch,
            &repo,
            Box::new(move || {
                fs::remove_file(&moved).expect("the middle link goes");
                std::os::unix::fs::symlink(&to, &moved).expect("the middle link points elsewhere");
            }),
        );
        assert_refused_naming(&stop, &text, &link, "a middle link pointed elsewhere");
    }

    /// A relative link on `PATH`, resolved against the program directory: the step rewrites
    /// the program it resolves to.
    #[test]
    fn a_program_behind_a_relative_link_rewritten_refuses_the_step() {
        let _one = one_at_a_time();
        let (scratch, repo) = repository("relative");
        let dir = own_program_dir();
        let sub = dir.join(format!("w53-sub-{}", std::process::id()));
        let _ = fs::remove_dir_all(&sub);
        fs::create_dir_all(&sub).expect("a directory beside the programs");
        let end = sub.join("probe");
        install(&end, "#!/bin/sh\nexit 0\n");
        let link = dir.join(format!("w53-relative-{}", std::process::id()));
        let _ = fs::remove_file(&link);
        std::os::unix::fs::symlink(
            Path::new(sub.file_name().expect("a name")).join("probe"),
            &link,
        )
        .expect("a relative link on PATH");
        let _made = Made(vec![link.clone(), sub.clone()]);
        let rewrite = end.clone();
        let (stop, text) = run(
            &scratch,
            &repo,
            Box::new(move || install(&rewrite, "#!/bin/sh\necho rewritten\nexit 0\n")),
        );
        assert_refused_naming(&stop, &text, &link, "a relative link");
    }

    /// A link on `PATH` that resolves to nothing when the run begins: the step makes its target,
    /// so a lookup by the name now runs a program the step wrote.
    #[test]
    fn a_dangling_link_the_step_makes_resolve_refuses_the_step() {
        let _one = one_at_a_time();
        let (scratch, repo) = repository("dangling");
        let end = scratch.join("later");
        let link = own_program_dir().join(format!("w53-dangling-{}", std::process::id()));
        let _ = fs::remove_file(&link);
        std::os::unix::fs::symlink(&end, &link).expect("a dangling link on PATH");
        let _made = Made(vec![link.clone()]);
        let made = end.clone();
        let (stop, text) = run(
            &scratch,
            &repo,
            Box::new(move || install(&made, "#!/bin/sh\necho planted\nexit 0\n")),
        );
        assert_refused_naming(&stop, &text, &link, "a dangling link made to resolve");
    }

    /// A link on `PATH` whose program the step replaces by a link to another program: the end
    /// differs though the link on `PATH` is the same.
    #[test]
    fn a_program_replaced_by_a_link_to_another_refuses_the_step() {
        let _one = one_at_a_time();
        let (scratch, repo) = repository("replaced");
        let end = scratch.join("probe");
        let other = scratch.join("other");
        install(&end, "#!/bin/sh\nexit 0\n");
        install(&other, "#!/bin/sh\necho other\nexit 0\n");
        let link = own_program_dir().join(format!("w53-replaced-{}", std::process::id()));
        let _ = fs::remove_file(&link);
        std::os::unix::fs::symlink(&end, &link).expect("a link on PATH");
        let _made = Made(vec![link.clone()]);
        let (at, to) = (end.clone(), other.clone());
        let (stop, text) = run(
            &scratch,
            &repo,
            Box::new(move || {
                fs::remove_file(&at).expect("the program goes");
                std::os::unix::fs::symlink(&to, &at).expect("a link in its place");
            }),
        );
        assert_refused_naming(&stop, &text, &link, "a program replaced by a link");
    }
}
