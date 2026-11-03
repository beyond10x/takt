//! story:fenced-forge-dispatch: `pr.open` and `repository.merge` record the outcome class of the
//! forge write they attempt on the step's line in `events.jsonl` (`forge_write`): `applied`,
//! `refused`, `not_attempted` or `unknown`. A write whose request may have reached the forge and
//! whose answer never came is `unknown`, and the next step resolves it by reading the forge,
//! never by sending the merge again on that ground.
//!
//! Every case but the last runs the built-in `story.delivery/1` with the in-memory forge
//! ([`Memory`]), its checks passing: a first run that stops at the gate, the operator's grant,
//! and the runs that follow; each reads the step's line of the record. The last holds the
//! command forge to each way a merge fails, through a scripted runner and the process runner on
//! a local program. Nothing leaves the machine.

#[path = "support/own_home.rs"]
mod own_home;

use std::cell::RefCell;
use std::collections::VecDeque;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::rc::Rc;
use std::time::Duration;

use anyhow::anyhow;
use takt::case::{AUTHORITY_FILE, grant};
use takt::evidence::Record;
use takt::forge::command::{CommandForge, ForgeCommands};
use takt::forge::{
    Checks, Exit, Forge, Limits, Memory, MergeAnswer, Process, Runner, TimedOut, WriteOutcome,
};
use takt::harness::{Harness, StepContext, StepReport};
use takt::scan::Scan;
use takt::validator::{CiRead, PrMerged, PrOpen, Validator};
use takt::{Run, RunOptions, Stop};

/// The protocol under test, the engineering protocols' built-in.
const PROTOCOL: &str = "story.delivery/1";

/// What the forge says when it refuses a merge, as GitHub's API words a pull request it will not
/// merge.
const NOT_MERGEABLE: &str = "405 Method Not Allowed: Pull Request is not mergeable";

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

/// A scratch git tree under this crate's build directory. Dropping it removes the tree; a
/// removal that fails fails the case, unless the case is already failing.
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

/// A scratch tree with one commit on `main`.
fn scratch(name: &str) -> Scratch {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("fenced-{}-{name}", std::process::id()));
    if dir.exists() {
        fs::remove_dir_all(&dir).expect("a stale scratch tree is removed");
    }
    fs::create_dir_all(&dir).expect("scratch dir");
    fs::write(dir.join("README.md"), "a story\n").expect("a file");
    git(&dir, &["init", "-q", "-b", "main"]);
    git(&dir, &["add", "-A"]);
    git(&dir, &["commit", "-q", "-m", "start"]);
    Scratch(dir)
}

/// A harness the runs after the gate never need: every step they take is a validator's.
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

/// A harness that requests the merge gate, changing nothing, as soon as it is offered.
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
        Err(anyhow!(
            "the gate was not offered; a judgement step among {} is left",
            context.offered.join(", ")
        ))
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

/// The three forge validators over `forge`, in the order the binary registers them.
fn validators(forge: Rc<dyn Forge>) -> Vec<Box<dyn Validator>> {
    vec![
        Box::new(PrOpen::new(forge.clone())),
        Box::new(CiRead::new(forge.clone())),
        Box::new(PrMerged::new(forge)),
    ]
}

/// The first run of the case `id` over `tree`: one commit beyond the base, the tests and the
/// gate green at it and the change handed over, the double's checks passing, the `Requester`
/// harness. It stops at the gate with the pull request open; the revision and the case
/// directory.
fn to_the_gate(tree: &Path, id: &str, memory: &Rc<Memory>) -> (String, PathBuf) {
    let forge: Rc<dyn Forge> = memory.clone();
    let validators = validators(forge);
    let mut run = Run::open(tree, options(id), &Requester, &validators).expect("the case opens");
    fs::write(tree.join("implemented.txt"), "done\n").expect("a file");
    git(tree, &["add", "implemented.txt"]);
    git(tree, &["commit", "-q", "-m", "implemented by the test"]);
    let revision = git(tree, &["rev-parse", "HEAD"]);
    run.case
        .set_revision("implementation", &revision)
        .expect("the revision moves");
    for (n, (kind, result, producer)) in [
        ("test_result", Some("pass"), "fenced-test"),
        ("gate_result", Some("pass"), "fenced-test"),
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
    (revision, run.case.dir.clone())
}

/// What one resumed run left: how it stopped (an error as its text) and what it printed.
struct Ran {
    stop: Result<Stop, String>,
    text: String,
}

impl Ran {
    fn said(&self) -> String {
        format!("{:?}\n{}", self.stop, self.text)
    }
}

/// The case `id` resumed over `memory`, as the next `takt run` on it would be, waiting on the
/// forge for `poll` between observations.
fn resumed(tree: &Path, id: &str, memory: &Rc<Memory>, poll: Duration) -> Ran {
    let forge: Rc<dyn Forge> = memory.clone();
    let validators = validators(forge);
    let mut run =
        Run::resume(tree, options(id), id, &NoHarness, &validators).expect("the case resumes");
    run.poll = poll;
    let mut out = Vec::new();
    let stop = run.run(&mut out).map_err(|error| format!("{error:#}"));
    Ran {
        stop,
        text: String::from_utf8(out).expect("utf-8"),
    }
}

/// The case at the gate, granted `repository.merge` by the operator: its revision and directory.
fn granted(tree: &Path, id: &str, memory: &Rc<Memory>) -> (String, PathBuf) {
    let (revision, case_dir) = to_the_gate(tree, id, memory);
    grant(tree, id, "repository.merge", "human:operator").expect("the grant is written");
    (revision, case_dir)
}

/// The record's lines, in order.
fn lines(case_dir: &Path) -> Vec<serde_json::Value> {
    fs::read_to_string(case_dir.join("events.jsonl"))
        .expect("the run record")
        .lines()
        .map(|line| serde_json::from_str(line).expect("a line of json"))
        .collect()
}

/// The validator step lines of `action` the record holds, in order.
fn steps(case_dir: &Path, action: &str) -> Vec<serde_json::Value> {
    lines(case_dir)
        .into_iter()
        .filter(|line| {
            line["event"] == "step" && line["by"] == "validator" && line["action"] == action
        })
        .collect()
}

/// The `merge_observation` records under the case's `evidence/`, their results.
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

/// How many merges the double was asked for.
fn merge_calls(memory: &Memory) -> usize {
    memory
        .calls
        .borrow()
        .iter()
        .filter(|call| call.starts_with("merge #"))
        .count()
}

/// Acceptance 1: the forge applies the merge and its answer is lost to a timeout. The run records
/// the `repository.merge` step with outcome `unknown`, about pull request #1 at the head the
/// merge named, and no `merge_observation`; the next run on the case reads the pull request
/// merged and writes a `merge_observation` of `merged`, sending no merge: the double's call log
/// holds one merge across both runs.
#[test]
fn a_merge_applied_whose_answer_is_lost_is_unknown_and_the_next_run_reads_it_merged() {
    let tree = scratch("lost");
    let memory = Rc::new(Memory {
        checks: Checks::Pass,
        ..Memory::default()
    });
    let (revision, case_dir) = granted(&tree, "lost", &memory);
    *memory.merge_answer.borrow_mut() = MergeAnswer::MergedAnswerLost;

    let first = resumed(&tree, "lost", &memory, Duration::from_millis(5));
    let merges = steps(&case_dir, "repository.merge");
    assert_eq!(merges.len(), 1, "{}\n{merges:#?}", first.said());
    let write = &merges[0]["forge_write"];
    assert_eq!(write["outcome"], "unknown", "{}\n{merges:#?}", first.said());
    assert_eq!(write["pull_request"], 1, "{merges:#?}");
    assert_eq!(write["head"], revision.as_str(), "{merges:#?}");
    assert!(merge_observations(&case_dir).is_empty(), "{}", first.said());
    assert_eq!(
        memory.merged.borrow().as_slice(),
        [1],
        "the forge applied it"
    );
    assert_eq!(merge_calls(&memory), 1, "{:#?}", memory.calls.borrow());

    let asked = memory.calls.borrow().len();
    let next = resumed(&tree, "lost", &memory, Duration::from_millis(5));
    assert_eq!(
        next.stop,
        Ok(Stop::Completed("accepted".to_owned())),
        "{}",
        next.said()
    );
    assert_eq!(merge_observations(&case_dir), ["merged"], "{}", next.said());
    assert_eq!(
        merge_calls(&memory),
        1,
        "one merge call across both runs: {:#?}",
        memory.calls.borrow()
    );
    assert!(
        memory.calls.borrow()[asked..]
            .iter()
            .any(|call| call == "view #1"),
        "the next run read the pull request: {:#?}",
        memory.calls.borrow()
    );
    let merges = steps(&case_dir, "repository.merge");
    assert_eq!(merges.len(), 2, "{}\n{merges:#?}", next.said());
    assert_eq!(
        merges[1]["forge_write"]["outcome"], "not_attempted",
        "the read sent no merge: {merges:#?}"
    );
}

/// Acceptance 2: the forge refuses the merge with its 405. The step's line records outcome
/// `refused`, nothing is merged and no `merge_observation` is written.
#[test]
fn a_merge_the_forge_refuses_is_refused_and_writes_no_merge_observation() {
    let tree = scratch("refused");
    let memory = Rc::new(Memory {
        checks: Checks::Pass,
        ..Memory::default()
    });
    let (revision, case_dir) = granted(&tree, "refused", &memory);
    *memory.merge_answer.borrow_mut() = MergeAnswer::Refused(NOT_MERGEABLE.to_owned());

    let ran = resumed(&tree, "refused", &memory, Duration::from_millis(5));
    assert!(ran.said().contains(NOT_MERGEABLE), "{}", ran.said());
    let merges = steps(&case_dir, "repository.merge");
    assert_eq!(merges.len(), 1, "{}\n{merges:#?}", ran.said());
    let write = &merges[0]["forge_write"];
    assert_eq!(write["outcome"], "refused", "{}\n{merges:#?}", ran.said());
    assert_eq!(write["pull_request"], 1, "{merges:#?}");
    assert_eq!(write["head"], revision.as_str(), "{merges:#?}");
    assert!(merge_observations(&case_dir).is_empty(), "{}", ran.said());
    assert!(memory.merged.borrow().is_empty());
    assert_eq!(merge_calls(&memory), 1, "{:#?}", memory.calls.borrow());
}

/// Acceptance 3: another hand pushed the case's branch after the grant, so the forge reports the
/// pull request at a head that is not the case's revision. No merge is sent, and the step's line
/// records outcome `not_attempted` with the head the forge reported.
#[test]
fn a_merge_not_sent_because_the_head_moved_is_not_attempted() {
    let tree = scratch("moved");
    let memory = Rc::new(Memory {
        checks: Checks::Pass,
        ..Memory::default()
    });
    let (revision, case_dir) = granted(&tree, "moved", &memory);
    // Someone else's commit on the case's branch, pushed past takt: the double's head for the
    // branch is now that commit, which does not carry takt's revision alone.
    git(&tree, &["checkout", "-q", "-b", "elsewhere"]);
    fs::write(tree.join("elsewhere.txt"), "another hand\n").expect("a file");
    git(&tree, &["add", "elsewhere.txt"]);
    git(&tree, &["commit", "-q", "-m", "another hand's commit"]);
    let moved = git(&tree, &["rev-parse", "HEAD"]);
    git(&tree, &["checkout", "-q", "takt/moved"]);
    memory
        .pushed
        .borrow_mut()
        .insert("takt/moved".to_owned(), moved.clone());

    let ran = resumed(&tree, "moved", &memory, Duration::from_millis(5));
    let merges = steps(&case_dir, "repository.merge");
    assert!(!merges.is_empty(), "{}", ran.said());
    for merge in &merges {
        let write = &merge["forge_write"];
        assert_eq!(
            write["outcome"],
            "not_attempted",
            "{}\n{merges:#?}",
            ran.said()
        );
        assert_eq!(write["pull_request"], 1, "{merges:#?}");
        assert_eq!(write["head"], moved.as_str(), "{merges:#?}");
    }
    assert_ne!(moved, revision);
    assert_eq!(merge_calls(&memory), 0, "{:#?}", memory.calls.borrow());
    assert!(merge_observations(&case_dir).is_empty(), "{}", ran.said());
}

/// Acceptance 4: `pr.open` opens the pull request and the forge answers its number. The step's
/// line records outcome `applied`, with pull request #1 and the head pushed.
#[test]
fn a_pr_open_that_is_applied_records_its_number() {
    let tree = scratch("open");
    let memory = Rc::new(Memory {
        checks: Checks::Pass,
        ..Memory::default()
    });
    let (revision, case_dir) = to_the_gate(&tree, "open", &memory);
    let opens = steps(&case_dir, "pr.open");
    assert_eq!(opens.len(), 1, "{opens:#?}");
    let write = &opens[0]["forge_write"];
    assert_eq!(write["outcome"], "applied", "{opens:#?}");
    assert_eq!(write["pull_request"], 1, "{opens:#?}");
    assert_eq!(write["head"], revision.as_str(), "{opens:#?}");
    // A step that wrote nothing to the forge carries no forge write.
    for line in steps(&case_dir, "ci.read") {
        assert!(line.get("forge_write").is_none(), "{line:#}");
    }
}

/// The security boundary: a merge whose answer was lost without the forge applying it leaves the
/// pull request open at the head it named. The next run's step that reads it so sends no merge,
/// records `not_attempted` saying why and waits on the forge; only a later observation, under
/// the same grant, sends the merge, and it is applied.
#[test]
fn an_unknown_merge_read_open_at_its_head_sends_nothing_in_that_step_and_says_why() {
    let tree = scratch("unapplied");
    let memory = Rc::new(Memory {
        checks: Checks::Pass,
        ..Memory::default()
    });
    let (revision, case_dir) = granted(&tree, "unapplied", &memory);
    *memory.merge_answer.borrow_mut() = MergeAnswer::AnswerLost;

    let first = resumed(&tree, "unapplied", &memory, Duration::from_millis(5));
    assert_eq!(
        steps(&case_dir, "repository.merge")[0]["forge_write"]["outcome"],
        "unknown",
        "{}",
        first.said()
    );
    assert!(memory.merged.borrow().is_empty());
    assert_eq!(merge_calls(&memory), 1);

    let next = resumed(&tree, "unapplied", &memory, Duration::from_millis(5));
    assert_eq!(
        next.stop,
        Ok(Stop::Completed("accepted".to_owned())),
        "{}",
        next.said()
    );
    let merges = steps(&case_dir, "repository.merge");
    let outcomes: Vec<&str> = merges
        .iter()
        .map(|line| line["forge_write"]["outcome"].as_str().unwrap_or("absent"))
        .collect();
    assert_eq!(
        outcomes,
        ["unknown", "not_attempted", "applied"],
        "{}\n{merges:#?}",
        next.said()
    );
    let why = merges[1]["summary"].as_str().expect("a summary");
    assert!(
        why.contains("unknown") && why.contains("nothing sent"),
        "the step says why it sent nothing: {why}"
    );
    assert_eq!(merges[1]["forge_write"]["head"], revision.as_str());
    assert_eq!(merge_calls(&memory), 2, "{:#?}", memory.calls.borrow());
    assert_eq!(merge_observations(&case_dir), ["merged"], "{}", next.said());
}

/// The security boundary: resolving an `unknown` never merges without the grant. With the
/// case's grant gone after the merge whose answer was lost, the next run takes no merge step at
/// all and sends nothing.
#[test]
fn an_unknown_merge_is_never_resolved_into_a_merge_without_the_grant() {
    let tree = scratch("ungranted");
    let memory = Rc::new(Memory {
        checks: Checks::Pass,
        ..Memory::default()
    });
    let (_, case_dir) = granted(&tree, "ungranted", &memory);
    *memory.merge_answer.borrow_mut() = MergeAnswer::AnswerLost;
    let first = resumed(&tree, "ungranted", &memory, Duration::from_millis(5));
    assert_eq!(
        steps(&case_dir, "repository.merge")[0]["forge_write"]["outcome"],
        "unknown",
        "{}",
        first.said()
    );
    fs::remove_file(case_dir.join(AUTHORITY_FILE)).expect("the grant is withdrawn");

    let next = resumed(&tree, "ungranted", &memory, Duration::from_millis(5));
    assert_eq!(merge_calls(&memory), 1, "{:#?}", memory.calls.borrow());
    assert!(memory.merged.borrow().is_empty(), "{}", next.said());
    assert!(merge_observations(&case_dir).is_empty(), "{}", next.said());
    assert_eq!(
        steps(&case_dir, "repository.merge").len(),
        1,
        "{}",
        next.said()
    );
}

/// A command forge runner that answers each line from a script.
struct Scripted(RefCell<VecDeque<anyhow::Result<Exit>>>);

impl Runner for Scripted {
    fn run(&self, _tree: &Path, _argv: &[String]) -> anyhow::Result<Exit> {
        self.0
            .borrow_mut()
            .pop_front()
            .expect("the script has an answer for each line")
    }

    fn run_env(&self, tree: &Path, argv: &[String], _env: &[(&str, &str)]) -> anyhow::Result<Exit> {
        self.run(tree, argv)
    }
}

/// A command forge over `tree` whose `merge` entry is `merge`, `3` its one refusal exit, every
/// line run through `runner`.
fn command_forge(tree: &Path, merge: &[&str], runner: Box<dyn Runner>) -> CommandForge {
    let argv = |words: &[&str]| -> Option<Vec<String>> {
        Some(words.iter().map(|word| (*word).to_owned()).collect())
    };
    CommandForge::for_tree(
        tree,
        ForgeCommands {
            read: argv(&["tool", "read", "{number}"]),
            merge: argv(merge),
            refused_exit: Some(vec![3]),
            ..ForgeCommands::default()
        },
        &["repository.merge".to_owned()],
        tree.join("state"),
        Scan::None {
            declared_in: tree.join("run.yaml"),
        },
        runner,
    )
    .expect("a command forge")
}

/// The class of the merge `forge` fails, and its error.
fn failed_merge(forge: &CommandForge, tree: &Path) -> (WriteOutcome, String) {
    let error = forge
        .merge(tree, &tree.join("artifacts"), 7, "0123456789abcdef")
        .expect_err("the merge fails");
    (error.outcome(), format!("{error:#}"))
}

/// The command forge maps each way a merge fails to one of the four classes: an exit the run
/// file lists in `refused_exit` is `refused`; another exit, an end by a signal, and a program
/// the runner killed past its time limit are `unknown`, since the program may have sent the
/// merge first; a program that could not be started sent nothing, `not_attempted`.
#[test]
fn the_command_forge_maps_each_failed_merge_to_its_class() {
    let tree = scratch("command-classes");
    let exit = |code| {
        Ok(Exit {
            code,
            stdout: String::new(),
            stderr: "the forge said no".to_owned(),
        })
    };
    let scripted = command_forge(
        &tree,
        &["tool", "merge", "{number}", "{head}"],
        Box::new(Scripted(RefCell::new(VecDeque::from([
            exit(Some(3)),
            exit(Some(4)),
            exit(None),
        ])))),
    );
    for (expected, what) in [
        (WriteOutcome::Refused, "a refusal exit"),
        (WriteOutcome::Unknown, "another exit"),
        (WriteOutcome::Unknown, "an end by a signal"),
    ] {
        let (outcome, error) = failed_merge(&scripted, &tree);
        assert_eq!(outcome, expected, "{what}: {error}");
    }

    let timed = command_forge(
        &tree,
        &["sleep", "30"],
        Box::new(Process {
            limits: Limits {
                timeout: Duration::from_millis(200),
                output: 1 << 16,
            },
        }),
    );
    let error = timed
        .merge(&tree, &tree.join("artifacts"), 7, "0123456789abcdef")
        .expect_err("the merge outlives its limit");
    assert!(
        error.chain().any(|cause| cause.is::<TimedOut>()),
        "the runner killed the merge past its limit: {error:#}"
    );
    assert_eq!(error.outcome(), WriteOutcome::Unknown, "{error:#}");

    let missing = tree.join("no-such-program");
    let unstarted = command_forge(
        &tree,
        &[missing.to_str().expect("utf-8")],
        Box::new(Process::default()),
    );
    let (outcome, error) = failed_merge(&unstarted, &tree);
    assert_eq!(outcome, WriteOutcome::NotAttempted, "{error}");
}
