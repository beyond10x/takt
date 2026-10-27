//! The loop: evaluate, frontier, one step, effect, observation, validator, evidence, evaluate;
//! until an outcome is legitimate, an approval is needed, nothing is admissible, a step is
//! refused, or the stop criteria are met. A run opens a case, or resumes one a run before it
//! opened: a case stopped at the gate is resumed once the operator has granted it, and the
//! merge is then a deterministic step like any other.
//!
//! Beside each printed line the run appends an event to the case's `events.jsonl`
//! ([`crate::events`]); a line about a judgement step is written once the step's change is
//! kept or put back, so putting the case directory back never takes a line with it. A story's
//! run given the planning store ([`Run::record_on`]) records its test result and its move to
//! `implemented` there: just before the step a grant authorises (the merge), so both ride to the
//! base with it above the granted revision, the move last ([`crate::case::approvals_only`]); or
//! once its case terminates `accepted`, when there was no such step.

use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use anyhow::{Context as _, Result, anyhow, bail};

use loom_sdk::commission::model::json as cjson;
use loom_sdk::commission::model::primitives::{Timestamp, Uuid};
use loom_sdk::commission::model::responsibility::{
    ActionRequestId, ActionStatus, AgentRevisionId, AuthorityContext, BreachStep, CaseId,
    Commission, CommissionData, CommissionId, EffectOutcome, EffectOutcomePerformed,
    EffectOutcomeRefused, ExecutorOutcome, ExecutorOutcomeProposedAction, ExecutorOutcomeSuspended,
    ExecutorStep, ExecutorUsage, Frontier as LoomFrontier, HostStop as LoomHostStop, ObservationId,
    PrincipalId, ProposedActionArguments, RunId, RunOutcome, StopPolicy, SuspensionReason,
    Unit as LoomUnit, commission_state, frontier_state,
};
use loom_sdk::commission::outcome::RunStore;
use loom_sdk::commission::ports::effect::{AdmittedRequest, EffectError};
use loom_sdk::commission::ports::executor::AgentExecutor;
use loom_sdk::{CanonGovernor, EffectPort, LoopContext, run_until_blocked};

use crate::case::{
    CASE_ROOTS, CaseAuthority, CaseDir, CaseFiles, CommitAs, KEPT_MOVE, KEPT_TEST_RESULT, Records,
    is_under_cases,
};
use crate::events::{BY_STORE, BY_TAKT, Event, Events};
use crate::evidence::{CaseEvidence, uuid_from};
use crate::frontier::{Frontier, IMPLEMENTED, Story, TEST_RESULT, Take, Unit};
use crate::git::{BoundedDiff, DiffContext};
use crate::harness::{CHANGE_SHOWN_BYTES, ChangeSoFar, Harness, StepContext, is_test_path};
use crate::protocol::LoadedProtocol;
use crate::scan::Scan;
pub use crate::scan::gates::ChangeScan;
use crate::validator::{ChoiceEffect, Effect, Step, Validator, ValidatorEffect, outcome};

/// What a run is given.
#[derive(Debug, Clone)]
pub struct RunOptions {
    /// The protocol id, `software.change/1`, or a `protocol/1` file path.
    pub protocol: String,
    /// The intent.
    pub intent: String,
    /// Case inputs, `name=value`.
    pub inputs: Vec<(String, String)>,
    /// Stop after this many steps.
    pub iterations: Option<u64>,
    /// Stop after this many hours.
    pub hours: Option<f64>,
    /// Stop once the harness reported this many tokens.
    pub tokens: Option<u64>,
    /// The case id; generated from the time when absent.
    pub case_id: Option<String>,
    /// The approval gate: the one approval-required action the harness may request to end the
    /// run, `repository.merge` for a software change. Other approval-required actions are not
    /// offered.
    pub gate: Option<String>,
}

/// Why a run stopped.
#[derive(Debug, Clone, PartialEq)]
pub enum Stop {
    /// An outcome is legitimate: the case is complete through it.
    Completed(String),
    /// Only actions that need an approval are left.
    ApprovalRequired(Vec<String>),
    /// No action is left to take and none needs an approval: none is admissible, or the only
    /// admissible ones read and the harness keeps no state between steps.
    NothingAdmissible,
    /// The step budget is spent.
    Iterations(u64),
    /// The time budget is spent.
    Hours(f64),
    /// The token budget is spent.
    Tokens(u64),
    /// The active-time budget is spent ([`Run::cap_active_time`]): the run's wall time less its
    /// waits on the forge reached the budget, read before a step.
    ActiveTime {
        /// The budget, in seconds.
        budget: u64,
        /// The run's active time when it stopped, in seconds.
        active: f64,
        /// The time it waited on the forge, left out of the active time, in seconds.
        waited: f64,
    },
    /// The harness did something the frontier did not admit; its change was discarded.
    StepRefused(String),
    /// A judgement step changed no file and the decision did not move: another step would not either.
    NoProgress,
    /// The run waits, and may wait no longer. On the forge (checks still running, a pull
    /// request's head behind the head pushed): its step or hour budget is spent, or it has
    /// neither to wait in. On something local ([`crate::validator::Validator::waits_locally`]: a
    /// saved scorecard, a promoted binary, the local `main`): at once after the step that
    /// observed it, with no pause and no decision after it, since only someone outside the run
    /// moves it. Nothing is recorded for what is awaited; the case stays open, and the next run
    /// resumes it and observes again.
    Waiting {
        /// The actions whose observation waits.
        on: Vec<String>,
        /// The budget spent; `None` for a run with no step or hour budget, which does not wait,
        /// and for a wait on something local.
        budget: Option<Box<Stop>>,
        /// What each wait on something local waits for, as its observation named it (the file
        /// it reads); empty for a wait on the forge.
        local: Vec<String>,
    },
    /// No offered or due action produces evidence an open obligation's claim reads, and an
    /// admissible action nobody in the run performs is the only producer of a claim that does
    /// not hold: no step can move the case, and none is taken. The case stays open. Only a run
    /// that turned it on stops so ([`Run::stop_when_unperformable`]): the CLI does, for every
    /// run; the library's default is off.
    Unperformable {
        /// The claim only the missing performer decides.
        claim: String,
        /// What the run lacks to perform it: the forge, the gates document.
        performer: String,
    },
    /// The file system holding the case's tree, or the one holding the build directory, has
    /// less free space than the run was given as the least a step needs: read before every step
    /// the run offers or runs, once one follows (a stop that takes no step, for a grant, a
    /// missing performer or nothing admissible, comes first), never at open, and the run stops
    /// before the step instead of failing in it for want of space. The case stays open. Only a
    /// run given a threshold stops so ([`Run::stop_when_disk_below`]): the CLI gives one for
    /// every run, `--min-free-gib`; the library's default reads nothing.
    DiskLow {
        /// The path whose file system was read: the tree, or the build directory.
        path: PathBuf,
        /// The bytes free there for a writer that is not root.
        free_bytes: u64,
        /// The threshold: the least free space a step needs, in bytes.
        min_bytes: u64,
    },
    /// The case's first `tests.run` failed before any effect: at a revision its branch reaches
    /// from the base commit it pinned through takt's store commits alone (the take), so no step
    /// has changed anything yet. The run stops before the first judgement step, no model token
    /// spent; the failure is recorded as the case's test result like any other, and the case
    /// stays open.
    RedBase {
        /// The revision the tests failed at.
        revision: String,
        /// The tests the output names as failed ([`crate::validator::failures_in`]); empty
        /// when it names none.
        tests: Vec<String>,
        /// The runner's hint for running them again, when its output had one.
        rerun: Option<String>,
    },
    /// [`REPEATED_FAILURES`] `tests.run` results in a row failed the same set of tests, each at
    /// a revision the one before was not at: the edits between them moved nothing the tests
    /// decide. The run stops after the last, before another judgement step; the case stays open.
    RepeatedTestFailure {
        /// The tests every one of the results failed, never empty.
        tests: Vec<String>,
        /// The results in a row.
        count: u64,
    },
}

/// How many `tests.run` results in a row, each at a new revision, failing the same set of tests
/// stop the run ([`Stop::RepeatedTestFailure`]).
pub const REPEATED_FAILURES: u64 = 2;

/// The outcome a run record gives each stop but `Completed`, whose outcome is the one it names:
/// the word its display opens with. `tests/stop_kinds.rs` matches every variant into this list
/// and fails to compile when one is added without its name here.
pub const STOP_KINDS: [&str; 13] = [
    "ApprovalRequired",
    "NothingAdmissible",
    "Iterations",
    "Hours",
    "Tokens",
    "StepRefused",
    "NoProgress",
    "Waiting",
    "Unperformable",
    "DiskLow",
    "ActiveTime",
    "RedBase",
    "RepeatedTestFailure",
];

/// `bytes` as a [`Stop::DiskLow`] names them: the count, then in GiB to two places.
fn bytes_shown(bytes: u64) -> String {
    format!(
        "{bytes} bytes ({:.2} GiB)",
        bytes as f64 / f64::from(1_u32 << 30)
    )
}

impl std::fmt::Display for Stop {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Stop::Completed(outcome) => write!(f, "Completed ({outcome})"),
            Stop::ApprovalRequired(actions) => {
                write!(f, "ApprovalRequired ({})", actions.join(", "))
            }
            Stop::NothingAdmissible => write!(f, "NothingAdmissible"),
            Stop::Iterations(n) => write!(f, "Iterations ({n})"),
            Stop::Hours(h) => write!(f, "Hours ({h})"),
            Stop::Tokens(t) => write!(f, "Tokens ({t})"),
            Stop::ActiveTime {
                budget,
                active,
                waited,
            } => write!(
                f,
                "ActiveTime ({budget} s; active {active:.1} s, waited {waited:.1} s on the forge)"
            ),
            Stop::StepRefused(why) => write!(f, "StepRefused ({why})"),
            Stop::NoProgress => write!(f, "NoProgress"),
            Stop::Waiting { on, budget, local } => match budget {
                _ if !local.is_empty() => {
                    write!(
                        f,
                        "Waiting ({}; locally: {})",
                        on.join(", "),
                        local.join("; ")
                    )
                }
                Some(budget) => write!(f, "Waiting ({}; {budget})", on.join(", ")),
                None => write!(f, "Waiting ({}; no step or hour budget)", on.join(", ")),
            },
            Stop::Unperformable { claim, performer } => write!(
                f,
                "Unperformable ({claim}; nothing performs it without {performer})"
            ),
            Stop::DiskLow {
                path,
                free_bytes,
                min_bytes,
            } => write!(
                f,
                "DiskLow ({}: {} free, under {})",
                path.display(),
                bytes_shown(*free_bytes),
                bytes_shown(*min_bytes)
            ),
            Stop::RedBase {
                revision,
                tests,
                rerun,
            } => {
                write!(
                    f,
                    "RedBase (the tests fail at the case's base {}, before any step changed it: ",
                    short(revision)
                )?;
                if tests.is_empty() {
                    write!(f, "the output names no failed test")?;
                } else {
                    write!(f, "{}", tests.join(", "))?;
                }
                match rerun {
                    Some(rerun) => write!(f, "; {rerun})"),
                    None => write!(f, ")"),
                }
            }
            Stop::RepeatedTestFailure { tests, count } => write!(
                f,
                "RepeatedTestFailure ({count} tests.run results in a row, each at a new revision, failed the same tests: {})",
                tests.join(", ")
            ),
        }
    }
}

impl Stop {
    /// Whether the run hands its change over as a candidate here ([`Run::candidate`]): the
    /// [`Stop::Unperformable`] at `pr.opened`, a delivery whose change reads ahead with no forge
    /// to open its pull request.
    #[must_use]
    pub fn hands_over(&self) -> bool {
        matches!(self, Stop::Unperformable { claim, .. } if claim == "pr.opened")
    }
}

/// Where a case's candidate is named: `refs/takt/candidates/<case id>` ([`Run::candidate`]).
pub const CANDIDATE_REFS: &str = "refs/takt/candidates/";

/// Whether the commit `commit` of the case delivering `story` is one of takt's store commits,
/// left out of its candidate: its subject is the take's, the `test_result` record's, the move's
/// or an approval's for that story, and every path it changes lies in the planning store
/// ([`crate::frontier::is_store_path`]). A commit a step made is never one, whatever it
/// changes: its subject is `<action>: <summary>`.
fn is_store_commit(commit: &crate::git::Commit, story: &str) -> bool {
    let subject = commit.subject.as_str();
    (subject == take_subject(story)
        || subject == crate::frontier::test_result_subject(story)
        || subject == crate::frontier::implemented_subject(story)
        || subject.starts_with(&crate::frontier::approve_subject(story, "")))
        && commit
            .paths
            .iter()
            .all(|path| crate::frontier::is_store_path(path))
}

/// The outcome of a case whose story is recorded and moved to `implemented` in the store.
pub const ACCEPTED: &str = "accepted";

/// The producer of a record takt writes from a judgement step that records a choice: the
/// record attests the harness's choice, not an observation's outcome.
pub const CHOICE_PRODUCER: &str = "takt.choice";

/// The first eight characters of a revision, as the loop prints one.
fn short(revision: &str) -> &str {
    revision.get(..8).unwrap_or(revision)
}

/// Who took a step, with what its event records of it: a validator, and the revision it
/// observed when it observed; or the harness, the tokens it reported, their components and
/// what it reported about the call besides them, and the path of the transcript takt kept or
/// why it kept none.
enum SteppedBy<'r> {
    Validator(Option<&'r str>),
    Harness(
        Option<u64>,
        Option<crate::events::TokenUsage>,
        Option<Box<crate::harness::CallReport>>,
        Option<std::result::Result<String, String>>,
    ),
}

/// What [`Run::record_before`] found before a step: no grant authorises it, so nothing was
/// written; the story's records for the granted revision are both written; or one is missing,
/// with why, and the step is not taken.
enum Before {
    Ungranted,
    Recorded,
    Missing(String),
}

/// What a validator's step came to ([`Run::validator_step`]): a stop that ends the run, or
/// the observation at `revision`, its records written, and whether it waits.
enum Taken {
    Stop(Stop),
    Observed {
        revision: String,
        observation: crate::validator::Observation,
        waits: bool,
    },
}

/// The `tests.run` results in a row that failed the same tests ([`Run::tests_stop`]): the set,
/// the tests in the order the first result named them, the revision of the last, and how many.
struct Failing {
    set: BTreeSet<String>,
    tests: Vec<String>,
    revision: String,
    count: u64,
}

impl Failing {
    /// `failing` after a `tests.run` result at `revision`: `failed` the tests it failed, none
    /// for a pass. A failure of the set the row failed adds one at a revision the last was not
    /// at and none at the same; another set starts a row of one; a pass, and a failure that
    /// names no test, end the row.
    fn after(failing: &mut Option<Failing>, revision: &str, failed: Option<Vec<String>>) {
        let Some(tests) = failed.filter(|tests| !tests.is_empty()) else {
            *failing = None;
            return;
        };
        let set: BTreeSet<String> = tests.iter().cloned().collect();
        match failing {
            Some(row) if row.set == set => {
                if row.revision != revision {
                    row.revision = revision.to_owned();
                    row.count += 1;
                }
            }
            _ => {
                *failing = Some(Failing {
                    set,
                    tests,
                    revision: revision.to_owned(),
                    count: 1,
                });
            }
        }
    }
}

/// What the case's record holds of its `tests.run` results as a run opens ([`TestsSoFar::read`]),
/// for [`Run::tests_stop`].
struct TestsSoFar {
    /// No `tests.run` result is recorded in the case yet: its next is the case's first.
    first: bool,
    /// The results in a row that failed the same tests, as the record ends.
    failing: Option<Failing>,
    /// The case's last run stopped on its tests: its next run observes `tests.run` again before
    /// any judgement step ([`Again`]).
    again: Option<Again>,
}

/// The stop on its tests a case's last run ended with, which its next run checks again at once:
/// `RedBase`, with the tests its result failed, or `RepeatedTestFailure`.
enum Again {
    RedBase(BTreeSet<String>),
    Repeated,
}

impl TestsSoFar {
    /// What `record`, the case's record before this run's own lines, holds: whether any
    /// `tests.run` step wrote a `test_result` record; the row of failures of the same tests its
    /// results end with ([`Failing::after`], each result's tests read from its step's summary,
    /// [`crate::validator::failures_in`]); and the stop of its last run, from its last `stop`
    /// line after the last `opened` one, when it is `RedBase` or `RepeatedTestFailure`.
    fn read(record: &[serde_json::Value]) -> Self {
        let mut first = true;
        let mut failing = None;
        let mut last_failed: BTreeSet<String> = BTreeSet::new();
        let mut stop: Option<&str> = None;
        // The `tests.run` step the lines that follow belong to: its number, revision, summary.
        let mut current: Option<(&serde_json::Value, &str, &str)> = None;
        for line in record {
            match line["event"].as_str() {
                Some("opened") => stop = None,
                Some("stop") => stop = line["stop"].as_str(),
                Some("step") => {
                    current = (line["by"] == "validator"
                        && line["action"] == crate::validator::TestsRun::ACTION)
                        .then(|| {
                            (
                                &line["step"],
                                line["revision"].as_str().unwrap_or_default(),
                                line["summary"].as_str().unwrap_or_default(),
                            )
                        });
                }
                _ if line["event"] == "evidence" => {
                    let Some((step, revision, summary)) = current else {
                        continue;
                    };
                    if line["step"] != *step || line["kind"] != crate::validator::TestsRun::KIND {
                        continue;
                    }
                    first = false;
                    let failed = (line["result"] == "fail")
                        .then(|| crate::validator::failures_in(summary).tests);
                    last_failed = failed.iter().flatten().cloned().collect();
                    Failing::after(&mut failing, revision, failed);
                }
                _ => {}
            }
        }
        let again = match stop.and_then(|stop| stop.split([' ', '(']).next()) {
            Some("RedBase") => Some(Again::RedBase(last_failed)),
            Some("RepeatedTestFailure") => Some(Again::Repeated),
            _ => None,
        };
        TestsSoFar {
            first,
            failing,
            again,
        }
    }
}

/// Where a validator's step is taken: the run's record and output, the evaluation it follows,
/// and whether the story's records were written before a granted step this run.
struct StepPlace<'p> {
    events: &'p Events,
    out: &'p mut dyn Write,
    seq: usize,
    recorded_before: &'p mut bool,
}

/// The step a [`crate::validator::ValidatorEffect`] performs in the run: the run's validator
/// step ([`Run::validator_step`]), what it came to kept for the loop in `taken`, and answered
/// to Commission's runtime as the effect's outcome: a stop as a refusal that ends its run, an
/// observation as [`crate::validator::outcome`] finds it.
struct GuardedStep<'g, 'a> {
    run: &'g mut Run<'a>,
    events: &'g Events,
    out: &'g mut dyn Write,
    seq: usize,
    recorded_before: &'g mut bool,
    taken: Option<Taken>,
}

impl Step for GuardedStep<'_, '_> {
    fn perform(
        &mut self,
        validator: &dyn Validator,
        request: &AdmittedRequest,
    ) -> Result<EffectOutcome> {
        let place = StepPlace {
            events: self.events,
            out: &mut *self.out,
            seq: self.seq,
            recorded_before: &mut *self.recorded_before,
        };
        let taken =
            self.run
                .validator_step(place, validator, &request.data().action_request_id.0.0)?;
        let answer = match &taken {
            Taken::Stop(stop) => EffectOutcome::Refused(EffectOutcomeRefused {
                reason: format!("{} ended the run: {stop:?}", validator.action()),
                ends_run: true,
            }),
            Taken::Observed {
                observation, waits, ..
            } => outcome(observation, *waits),
        };
        self.taken = Some(taken);
        Ok(answer)
    }
}

/// What a judgement step came to before Commission's runtime hands its action on
/// ([`Run::harness_turn`]): a stop that ends the run, or the action proposed, its change held;
/// or a session that ended without the line naming its action
/// ([`crate::harness::NoActionLine`]), its change discarded and nothing recorded of it yet: the
/// error the run ends on unless the step is tried again ([`NO_ACTION_TRIES`]), and the
/// transcript kept of it.
enum Judgement {
    Stop(Stop),
    Propose(Box<Proposed>),
    NoAction {
        error: anyhow::Error,
        transcript: Option<std::result::Result<String, String>>,
    },
}

/// A judgement step's proposal: what the harness reported, the transcript kept of it, the paths
/// it changed, whether it requests an approval (`ask`), and what puts its change back.
struct Proposed {
    report: crate::harness::StepReport,
    transcript: Option<std::result::Result<String, String>>,
    changed: Vec<String>,
    ask: bool,
    held: StepKept,
}

/// What a judgement step is put back to, owned until its proposal is kept or discarded
/// ([`Kept`] borrows it).
struct StepKept {
    checkpoint: crate::git::Checkpoint,
    tree: PathBuf,
    cases_kept: BTreeMap<PathBuf, Entry>,
    surface: JudgeSurface,
    surface_kept: BTreeMap<PathBuf, Entry>,
    outside: Outside,
    outside_dirs: OutsideDirs,
}

impl StepKept {
    fn kept(&self) -> Kept<'_> {
        Kept {
            checkpoint: &self.checkpoint,
            tree: &self.tree,
            cases_kept: &self.cases_kept,
            surface: &self.surface,
            surface_kept: &self.surface_kept,
            outside: &self.outside,
            outside_dirs: &self.outside_dirs,
        }
    }
}

/// Who takes the step a call of Commission's runtime makes: the validator due, else the
/// harness when an action is offered to it, else nobody.
#[derive(Clone, Copy)]
enum Taker<'v> {
    Validator(&'v dyn Validator),
    Harness,
    Nobody,
}

/// What the step came to, for the loop: a validator's step ([`Taken`]), or a judgement step
/// kept, with whether it moved the case.
enum Came {
    Taken(Taken),
    Kept(bool),
}

/// One call of Commission's runtime of loom-sdk as the run takes it: the run, its record and
/// output, the evaluation the call follows, who takes the step, and what came of it. The
/// executor, the effect and the context of the call share it ([`TurnExecutor`], [`TurnEffect`],
/// [`TurnContext`]); the runtime calls them one at a time.
struct Turn<'t, 'a> {
    run: &'t mut Run<'a>,
    events: &'t Events,
    out: &'t mut dyn Write,
    seq: usize,
    recorded_before: &'t mut bool,
    taker: Taker<'a>,
    decision: &'t crate::Decision,
    offered: &'t [String],
    approvals: &'t [String],
    shape: &'t Shape,
    run_number: u64,
    /// The host's stop the context answered, with whether it is a budget's.
    host: Option<(Stop, bool)>,
    came: Option<Came>,
    /// A judgement step's proposal, until its effect keeps it.
    proposed: Option<Box<Proposed>>,
    /// The error a part of the call failed on, which the run ends on.
    failed: Option<anyhow::Error>,
}

/// The composite executor of a call: the due validator's action, proposed as found due, which
/// no model chose; else a judgement step of the harness ([`Run::harness_turn`]), its usage
/// reported to the runtime; else no useful action.
struct TurnExecutor<'r, 't, 'a>(&'r RefCell<Turn<'t, 'a>>);

impl AgentExecutor for TurnExecutor<'_, '_, '_> {
    fn run(
        &self,
        commission: &Commission<commission_state::Assigned>,
        frontier: &LoomFrontier<frontier_state::Issued>,
    ) -> ExecutorOutcome {
        self.run_with_usage(commission, frontier).outcome
    }

    fn run_with_usage(
        &self,
        _: &Commission<commission_state::Assigned>,
        frontier: &LoomFrontier<frontier_state::Issued>,
    ) -> ExecutorStep {
        let mut guard = self.0.borrow_mut();
        let turn = &mut *guard;
        let proposal = |action: &str| {
            ExecutorOutcome::ProposedAction(ExecutorOutcomeProposedAction {
                action: action.to_owned(),
                arguments: ProposedActionArguments(cjson::Value::Object(Vec::new())),
            })
        };
        let step = |outcome| ExecutorStep {
            outcome,
            usage: None,
            transcript_digest: None,
            summary: None,
        };
        match turn.taker {
            Taker::Validator(validator) => step(proposal(validator.action())),
            Taker::Nobody => step(ExecutorOutcome::NoUsefulAction(LoomUnit(true))),
            Taker::Harness => {
                let mut tried = 0;
                let (started, judged) = loop {
                    tried += 1;
                    let started = Instant::now();
                    let judged = turn.run.harness_turn(
                        turn.events,
                        &mut *turn.out,
                        turn.decision,
                        turn.offered,
                        turn.approvals,
                        turn.shape,
                        turn.run_number,
                    );
                    // The session Loom's executor recorded for the step, whatever became of its
                    // proposal, is observed into the case before the step's own outcome.
                    let judged = match turn.run.harness.take_session() {
                        Some(session) => {
                            observe_session(&turn.run.case, frontier, &session).and(judged)
                        }
                        None => judged,
                    };
                    // A session that ended without the line naming its action, its change put
                    // back, is a try, recorded with its tokens; the step is tried again from the
                    // tree it was put back to, a step of its own, unless the run's own stop (a
                    // budget, the free disk) comes first, and after [`NO_ACTION_TRIES`] tries the
                    // run ends on its error. A refused step, or any other failure, is not retried.
                    let Ok(Judgement::NoAction { error, transcript }) = judged else {
                        break (started, judged);
                    };
                    if let Err(unrecorded) =
                        turn.run.charge_no_action(turn.events, transcript, tried)
                    {
                        break (
                            started,
                            Err(error.context(format!(
                                "and the tokens its session reported could not be recorded: {unrecorded:#}"
                            ))),
                        );
                    }
                    // The run's own stop is asked before the try limit: a try that spent the
                    // budget stops the run on the budget, the last try included. Said once it was
                    // asked: the step is told as tried again only when it is.
                    let stopped = turn.host_stop();
                    if stopped.is_none() && tried >= NO_ACTION_TRIES {
                        break (
                            started,
                            Err(error.context(format!(
                                "{tried} tries of the step ended without the line naming its action, {NO_ACTION_TRIES} at most"
                            ))),
                        );
                    }
                    let then = match (&stopped, tried >= NO_ACTION_TRIES) {
                        (Some(_), true) => "the run stops",
                        (Some(_), false) => "the run stops before another try",
                        (None, _) => "the step is tried again",
                    };
                    if let Err(unsaid) = writeln!(
                        turn.out,
                        "step {}: the session named no action (try {tried} of {NO_ACTION_TRIES}); {then}",
                        turn.run.steps
                    ) {
                        break (
                            started,
                            Err(error.context(format!("and the try could not be told: {unsaid}"))),
                        );
                    }
                    if let Some(stop) = stopped {
                        return step(ExecutorOutcome::Suspended(ExecutorOutcomeSuspended {
                            reason: SuspensionReason::HostStop(stop),
                        }));
                    }
                };
                let suspended = |detail: String| {
                    step(ExecutorOutcome::Suspended(ExecutorOutcomeSuspended {
                        reason: SuspensionReason::HostStop(LoomHostStop {
                            name: "takt".to_owned(),
                            detail: cjson::Value::Text(detail),
                        }),
                    }))
                };
                match judged {
                    Err(error) | Ok(Judgement::NoAction { error, .. }) => {
                        let detail = format!("{error:#}");
                        turn.failed = Some(error);
                        suspended(detail)
                    }
                    Ok(Judgement::Stop(stop)) => {
                        let detail = stop.to_string();
                        turn.came = Some(Came::Taken(Taken::Stop(stop)));
                        suspended(detail)
                    }
                    Ok(Judgement::Propose(proposed)) => {
                        let usage = match turn.run.harness.last_usage() {
                            Some(usage) => (
                                usage
                                    .input
                                    .saturating_add(usage.cache_read)
                                    .saturating_add(usage.cache_creation),
                                usage.output,
                            ),
                            None => (proposed.report.tokens.unwrap_or(0), 0),
                        };
                        let outcome = proposal(&proposed.report.action);
                        let summary = Some(proposed.report.summary.clone());
                        turn.proposed = Some(proposed);
                        ExecutorStep {
                            outcome,
                            usage: Some(ExecutorUsage {
                                input_tokens: i64::try_from(usage.0).unwrap_or(i64::MAX),
                                output_tokens: i64::try_from(usage.1).unwrap_or(i64::MAX),
                                duration_ms: i64::try_from(started.elapsed().as_millis())
                                    .unwrap_or(i64::MAX),
                                cost_micro_usd: None,
                            }),
                            transcript_digest: None,
                            summary,
                        }
                    }
                }
            }
        }
    }
}

/// The source of the observation that holds the `AgentSession` Loom's session executor recorded
/// for a judgement step ([`observe_session`]).
pub const SESSION_SOURCE: &str = "loom.session";

/// Observes `session`, the `AgentSession` Loom's session executor recorded for a judgement step
/// at `frontier`, into `case` (`observations.json`), as source [`SESSION_SOURCE`]: its session
/// id, kind, commission and frontier ids, the vendor's session id, its action, each file it
/// proposed by path with the SHA-256 of its proposed bytes (null for a removal), and its state.
/// No file's bytes enter the case.
///
/// # Errors
/// The case's refusal of the observation.
fn observe_session(
    case: &CaseDir,
    frontier: &LoomFrontier<frontier_state::Issued>,
    session: &crate::harness::AgentSession,
) -> Result<()> {
    use loom_sdk::commission::model::responsibility::{Observation, ObservationData};
    use loom_sdk::commission::ports::evidence::ObservationPort as _;
    use sha2::{Digest as _, Sha256};
    let text = |value: &str| cjson::Value::Text(value.to_owned());
    let id = &session.session_id.0.0;
    let files = session
        .files
        .iter()
        .map(|file| {
            cjson::Value::Object(vec![
                ("path".to_owned(), text(&file.path)),
                (
                    "sha256".to_owned(),
                    file.content.as_ref().map_or(cjson::Value::Null, |content| {
                        let digest = Sha256::digest(content.as_bytes());
                        cjson::Value::Text(
                            digest.iter().map(|byte| format!("{byte:02x}")).collect(),
                        )
                    }),
                ),
            ])
        })
        .collect();
    let observed_at = crate::now();
    let observation = ObservationData {
        observation_id: ObservationId(Uuid(uuid_from(&format!(
            "{}/observation/session/{id}/{observed_at}",
            case.id
        )))),
        source: SESSION_SOURCE.to_owned(),
        subject: format!("{}@{}", case.id, frontier.data().case_revision),
        observed_at: Timestamp(observed_at),
        payload: cjson::Value::Object(vec![
            ("record".to_owned(), text("loom.session.AgentSession")),
            ("session_id".to_owned(), text(id)),
            ("kind".to_owned(), text(&format!("{:?}", session.kind))),
            ("commission_id".to_owned(), text(&session.commission_id.0.0)),
            ("frontier_id".to_owned(), text(&session.frontier_id.0.0)),
            (
                "vendor_session_id".to_owned(),
                session
                    .vendor_session_id
                    .as_deref()
                    .map_or(cjson::Value::Null, text),
            ),
            ("action".to_owned(), text(&session.action)),
            ("files".to_owned(), cjson::Value::Array(files)),
            ("state".to_owned(), text(&format!("{:?}", session.state))),
        ]),
    };
    CanonGovernor::new(CaseFiles::new(&case.dir, &case.id, &case.protocol.id))
        .observe(Observation::new(observation))
        .map_err(|error| {
            anyhow!(
                "the case {} did not receive the session {id}: {error:?}",
                case.id
            )
        })
}

/// The composite effect of a call: it hands a validator's request to that validator's
/// [`ValidatorEffect`], performing through the run's validator step, and keeps the harness's
/// proposed action ([`Run::keep`]), handing an action that records a choice to the
/// [`ChoiceEffect`]; a request for an approval it answers with a refusal that ends the run,
/// nothing performed. It performs only what Commission's runtime hands it.
struct TurnEffect<'r, 't, 'a>(&'r RefCell<Turn<'t, 'a>>);

impl EffectPort for TurnEffect<'_, '_, '_> {
    fn performs(&self, action: &str) -> bool {
        let turn = self.0.borrow();
        turn.run.validators.iter().any(|v| v.action() == action)
            || turn.offered.iter().any(|offered| offered == action)
            || turn.approvals.iter().any(|approval| approval == action)
    }

    fn invoke(
        &self,
        commission: &Commission<commission_state::Assigned>,
        request: &AdmittedRequest,
    ) -> Result<EffectOutcome, EffectError> {
        let mut guard = self.0.borrow_mut();
        let turn = &mut *guard;
        let action = request.data().action.clone();
        let performed = match turn.taker {
            Taker::Validator(validator) if validator.action() == action => {
                let mut step = GuardedStep {
                    run: &mut *turn.run,
                    events: turn.events,
                    out: &mut *turn.out,
                    seq: turn.seq,
                    recorded_before: &mut *turn.recorded_before,
                    taken: None,
                };
                let (answered, failure) = {
                    let effect = ValidatorEffect::new(validator, &mut step);
                    let answered = effect.invoke(commission, request);
                    (answered, effect.take_failure())
                };
                turn.came = step.taken.take().map(Came::Taken);
                if let Some(error) = failure {
                    turn.failed = Some(error);
                }
                return answered;
            }
            Taker::Harness
                if turn
                    .proposed
                    .as_ref()
                    .is_some_and(|proposed| proposed.report.action == action) =>
            {
                let Some(proposed) = turn.proposed.take() else {
                    unreachable!("the proposal was just found");
                };
                if proposed.ask {
                    let stop = Stop::ApprovalRequired(vec![action.clone()]);
                    turn.came = Some(Came::Taken(Taken::Stop(stop)));
                    Ok(EffectOutcome::Refused(EffectOutcomeRefused {
                        reason: format!(
                            "{action} requests an approval; nothing is performed for it"
                        ),
                        ends_run: true,
                    }))
                } else {
                    let kept = turn.run.keep(
                        turn.events,
                        &mut *turn.out,
                        turn.seq,
                        turn.shape,
                        &proposed,
                        commission,
                        request,
                    );
                    kept.map(|(moved, answered)| {
                        turn.came = Some(Came::Kept(moved));
                        answered.unwrap_or_else(|| {
                            EffectOutcome::Performed(EffectOutcomePerformed {
                                report: cjson::Value::Object(vec![(
                                    "paths".to_owned(),
                                    cjson::Value::Array(
                                        proposed
                                            .changed
                                            .iter()
                                            .map(|path| cjson::Value::Text(path.clone()))
                                            .collect(),
                                    ),
                                )]),
                                attempt: None,
                                audit: None,
                                usage: None,
                            })
                        })
                    })
                }
            }
            _ => Ok(EffectOutcome::Refused(EffectOutcomeRefused {
                reason: format!("{action} is not the action this step proposed"),
                ends_run: true,
            })),
        };
        performed.map_err(|error| {
            let message = format!("{error:#}");
            turn.failed = Some(error);
            EffectError::new(message)
        })
    }
}

/// Every call of Commission's runtime a run of this process makes, so no two derive the same ids.
static TURNS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// The ids of one call, derived from `seed`.
struct TurnIds {
    seed: String,
    n: u64,
}

impl TurnIds {
    fn next(&mut self, what: &str) -> Uuid {
        self.n += 1;
        Uuid(uuid_from(&format!("{}/{what}/{}", self.seed, self.n)))
    }
}

/// The context of one call: its ids, takt's clock, a budget of one step, no approval gate and
/// an idle bound of one, so the call ends after the one step whatever it came to; and the
/// host's stop, asked before the step: the run's budget spent ([`Run::budget_spent`]), then,
/// for a step somebody takes, a file system under its threshold ([`Run::disk_low`]).
struct TurnContext<'r, 't, 'a> {
    turn: &'r RefCell<Turn<'t, 'a>>,
    ids: TurnIds,
}

impl LoopContext for TurnContext<'_, '_, '_> {
    fn action_request_id(&mut self) -> ActionRequestId {
        ActionRequestId(self.ids.next("action-request"))
    }

    fn observation_id(&mut self) -> ObservationId {
        ObservationId(self.ids.next("observation"))
    }

    fn now(&mut self) -> Timestamp {
        Timestamp(crate::now())
    }

    fn step_budget(&self) -> Option<usize> {
        Some(1)
    }

    fn stop_policy(&self) -> StopPolicy {
        StopPolicy {
            approval_gate: false,
            idle_bound: Some(1),
            on_breach: BreachStep::DiscardStep,
            breach_by_limit: Vec::new(),
        }
    }

    fn host_stop(&mut self) -> Option<LoomHostStop> {
        self.turn.borrow_mut().host_stop()
    }
}

impl Turn<'_, '_> {
    /// The host's stop before a step ([`TurnContext::host_stop`]), asked again before a judgement
    /// step is tried again ([`NO_ACTION_TRIES`]): the run's budget spent, then, for a step
    /// somebody takes, a file system under its threshold; kept in `host`, or the error the check
    /// failed on in `failed`.
    fn host_stop(&mut self) -> Option<LoomHostStop> {
        let turn = self;
        let stop = match turn.run.budget_spent() {
            Some(stop) => (stop, true),
            None if matches!(turn.taker, Taker::Nobody) => return None,
            None => match turn.run.disk_low(&mut *turn.out) {
                Ok(Some(stop)) => (stop, false),
                Ok(None) => return None,
                Err(error) => {
                    let detail = format!("{error:#}");
                    turn.failed = Some(error);
                    return Some(LoomHostStop {
                        name: "takt".to_owned(),
                        detail: cjson::Value::Text(detail),
                    });
                }
            },
        };
        let detail = stop.0.to_string();
        turn.host = Some(stop);
        Some(LoomHostStop {
            name: "takt".to_owned(),
            detail: cjson::Value::Text(detail),
        })
    }
}

/// Whether `validator` is due at a decision: its action is admissible, its evidence is missing
/// at its subject's current revision, and it has not already observed there without a record.
/// An evidence record that does not read is an error naming its file, never an answer.
fn is_due(
    case: &CaseDir,
    admissible: &[String],
    observed_without_record: &BTreeSet<(String, String)>,
    validator: &dyn Validator,
) -> Result<bool> {
    if !admissible.iter().any(|a| a == validator.action()) {
        return Ok(false);
    }
    if case.has_evidence(validator.kind(), validator.subject())? {
        return Ok(false);
    }
    Ok(!case.revision(validator.subject()).is_some_and(|revision| {
        observed_without_record.contains(&(validator.name().to_owned(), revision))
    }))
}

/// What the step choice reads of the case's protocol, as the protocol declares it: each
/// action's effect, the evidence kinds each action may produce, the kinds each claim is decided
/// from (through the claims it reads), and the kinds that record a choice.
///
/// A kind records a choice when no built-in validator writes it, every action that may produce
/// it declares `effect: read` and none of them is an action a built-in validator performs
/// ([`crate::validator::BUILT_IN`], whether or not the run has that validator), and every
/// predicate that reads it (a claim's, an obligation's, a precondition, an outcome's) reads it
/// without a `result`: what it attests is that the harness took the action, not an
/// observation's outcome. Such a record is written by takt from the step itself ([`Run::run`]);
/// every other kind only a validator writes. Refused when the protocol is read, before any case
/// is opened on it: a kind that records a choice and that the protocol reads about no artifact
/// (its record could be bound to no revision); a kind a built-in validator writes that an
/// action no built-in validator performs may produce with `effect: read` (it would pass for a
/// choice); a protocol whose outcome or obligation reads a choice kind through a form that is
/// not monotone in it (`not`, a claim tested `is: false` or `is: unknown`, an order form naming
/// it), directly or through the claims it tests ([`non_monotone_choice`]); and, with only
/// monotone forms left, a protocol where an outcome or an obligation holds with every claim
/// choice records decide true and every other claim unknown ([`under_choices`]), the most
/// choice records alone can make of it. A choice record may gate an action, never an outcome or
/// an obligation alone.
struct Shape {
    effects: BTreeMap<String, String>,
    produces: BTreeMap<String, BTreeSet<String>>,
    reads: BTreeMap<String, BTreeSet<String>>,
    /// The kinds that record a choice, each with the artifacts the predicates reading it name.
    choices: BTreeMap<String, BTreeSet<String>>,
    /// The actions whose precondition the case's inputs make false for good: false whatever any
    /// claim or evidence becomes.
    blocked_for_good: BTreeSet<String>,
    /// The kinds each obligation's discharge reads, directly or through the claims it tests.
    discharges: BTreeMap<String, BTreeSet<String>>,
}

/// The value of `predicate` from the case's `inputs` alone, every claim and every evidence
/// match unknown: `Some(false)` only when no claim or evidence could make it true.
fn by_inputs(
    predicate: &b10x_canon::model::Predicate,
    inputs: &[(String, String)],
) -> Option<bool> {
    use b10x_canon::model::{InputValue, Predicate};
    match predicate {
        Predicate::All(members) => {
            let values: Vec<Option<bool>> = members.iter().map(|m| by_inputs(m, inputs)).collect();
            if values.contains(&Some(false)) {
                Some(false)
            } else if values.iter().all(|v| *v == Some(true)) {
                Some(true)
            } else {
                None
            }
        }
        Predicate::Any(members) => {
            let values: Vec<Option<bool>> = members.iter().map(|m| by_inputs(m, inputs)).collect();
            if values.contains(&Some(true)) {
                Some(true)
            } else if values.iter().all(|v| *v == Some(false)) {
                Some(false)
            } else {
                None
            }
        }
        Predicate::Not(inner) => by_inputs(inner, inputs).map(|value| !value),
        Predicate::Input(test) => inputs
            .iter()
            .find(|(name, _)| name == test.input.as_str())
            .map(|(_, value)| test.holds_for(&InputValue::new(value.clone()))),
        _ => None,
    }
}

/// The value of `predicate` when every kind in `choices` has its record, every other evidence
/// match and every order form is unknown, and every claim has the value its own predicate takes
/// so ([`claim_under_choices`]): `Some(true)` when choice records alone would make it hold.
/// Inputs are the case's.
fn under_choices(
    predicate: &b10x_canon::model::Predicate,
    model: &b10x_canon::model::Protocol,
    choices: &BTreeMap<String, BTreeSet<String>>,
    inputs: &[(String, String)],
    values: &mut BTreeMap<String, Option<Option<bool>>>,
) -> Option<bool> {
    use b10x_canon::model::{Predicate, Truth};
    match predicate {
        Predicate::All(members) => {
            let found: Vec<Option<bool>> = members
                .iter()
                .map(|m| under_choices(m, model, choices, inputs, values))
                .collect();
            if found.contains(&Some(false)) {
                Some(false)
            } else if found.iter().all(|v| *v == Some(true)) {
                Some(true)
            } else {
                None
            }
        }
        Predicate::Any(members) => {
            let found: Vec<Option<bool>> = members
                .iter()
                .map(|m| under_choices(m, model, choices, inputs, values))
                .collect();
            if found.contains(&Some(true)) {
                Some(true)
            } else if found.iter().all(|v| *v == Some(false)) {
                Some(false)
            } else {
                None
            }
        }
        Predicate::Not(inner) => {
            under_choices(inner, model, choices, inputs, values).map(|value| !value)
        }
        Predicate::Evidence(matched) => choices.contains_key(matched.kind.as_str()).then_some(true),
        Predicate::Input(_) => by_inputs(predicate, inputs),
        Predicate::Claim(test) => {
            let value = claim_under_choices(test.claim.as_str(), model, choices, inputs, values);
            match test.is {
                Truth::True => value,
                Truth::False => value.map(|value| !value),
                Truth::Unknown => Some(value.is_none()),
            }
        }
        _ => None,
    }
}

/// The value claim `id` takes under [`under_choices`], kept in `values`; a claim that reads
/// itself, through others, is unknown.
fn claim_under_choices(
    id: &str,
    model: &b10x_canon::model::Protocol,
    choices: &BTreeMap<String, BTreeSet<String>>,
    inputs: &[(String, String)],
    values: &mut BTreeMap<String, Option<Option<bool>>>,
) -> Option<bool> {
    match values.get(id) {
        Some(Some(value)) => return *value,
        Some(None) => return None,
        None => {}
    }
    values.insert(id.to_owned(), None);
    let value = model
        .claims
        .iter()
        .find(|(claim, _)| claim.as_str() == id)
        .and_then(|(_, claim)| under_choices(&claim.true_when, model, choices, inputs, values));
    values.insert(id.to_owned(), Some(value));
    value
}

/// The kinds `predicate` reads, directly or through the claims it tests (`reads`: each claim's
/// kinds through the claims it reads).
fn kinds_read(
    predicate: &b10x_canon::model::Predicate,
    reads: &BTreeMap<String, BTreeSet<String>>,
) -> BTreeSet<String> {
    use b10x_canon::model::Predicate;
    let mut kinds = BTreeSet::new();
    predicate.visit(&mut |inner| match inner {
        Predicate::Evidence(matched) => {
            kinds.insert(matched.kind.as_str().to_owned());
        }
        Predicate::First(first) => {
            kinds.insert(first.kind.as_str().to_owned());
        }
        Predicate::Before(order) => {
            kinds.insert(order.first.as_str().to_owned());
            kinds.insert(order.then.as_str().to_owned());
        }
        Predicate::Claim(test) => {
            if let Some(through) = reads.get(test.claim.as_str()) {
                kinds.extend(through.iter().cloned());
            }
        }
        _ => {}
    });
    kinds
}

/// The first form in `predicate`, or in a claim it tests `is: true`, that is not monotone in a
/// choice kind: `not` over a predicate that reads one, a claim that reads one tested `is: false`
/// or `is: unknown`, or an order form (`first`, `before`) that names one. With none, a choice
/// record can only make the predicate truer, and its value with every choice record present
/// ([`under_choices`]) is the most choice records alone can make of it.
fn non_monotone_choice(
    predicate: &b10x_canon::model::Predicate,
    model: &b10x_canon::model::Protocol,
    choices: &BTreeMap<String, BTreeSet<String>>,
    reads: &BTreeMap<String, BTreeSet<String>>,
    seen: &mut BTreeSet<String>,
) -> Option<String> {
    use b10x_canon::model::{Predicate, Truth};
    let choice_in =
        |kinds: &BTreeSet<String>| kinds.iter().find(|k| choices.contains_key(*k)).cloned();
    match predicate {
        Predicate::All(members) | Predicate::Any(members) => members
            .iter()
            .find_map(|member| non_monotone_choice(member, model, choices, reads, seen)),
        Predicate::Not(inner) => choice_in(&kinds_read(inner, reads))
            .map(|kind| format!("`not` over a predicate that reads {kind}")),
        Predicate::First(first) => choices
            .contains_key(first.kind.as_str())
            .then(|| format!("an order form naming {}", first.kind.as_str())),
        Predicate::Before(order) => [&order.first, &order.then]
            .into_iter()
            .find(|kind| choices.contains_key(kind.as_str()))
            .map(|kind| format!("an order form naming {}", kind.as_str())),
        Predicate::Claim(test) => {
            let claim = test.claim.as_str();
            if test.is != Truth::True {
                let empty = BTreeSet::new();
                return choice_in(reads.get(claim).unwrap_or(&empty)).map(|kind| {
                    format!(
                        "the claim {claim}, which reads {kind}, tested is: {}",
                        if test.is == Truth::False {
                            "false"
                        } else {
                            "unknown"
                        }
                    )
                });
            }
            if !seen.insert(claim.to_owned()) {
                return None;
            }
            model
                .claims
                .iter()
                .find(|(id, _)| id.as_str() == claim)
                .and_then(|(_, found)| {
                    non_monotone_choice(&found.true_when, model, choices, reads, seen)
                })
        }
        _ => None,
    }
}

impl Shape {
    fn of(protocol: &LoadedProtocol, inputs: &[(String, String)]) -> Result<Self> {
        use b10x_canon::model::Predicate;
        let model = b10x_canon::model::parse(&protocol.yaml)
            .map_err(|error| anyhow::anyhow!("read the protocol {}: {error}", protocol.id))?;
        let mut effects = BTreeMap::new();
        let mut produces: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
        for (id, action) in model.actions.iter() {
            let id = id.as_str().to_owned();
            if let Some(effect) = &action.effect {
                effects.insert(id.clone(), effect.as_str().to_owned());
            }
            produces.insert(
                id,
                action
                    .may_produce
                    .iter()
                    .map(|production| production.evidence.as_str().to_owned())
                    .collect(),
            );
        }
        // How each kind is read anywhere: whether with a result, and about which artifacts.
        let mut with_result: BTreeMap<String, bool> = BTreeMap::new();
        let mut subjects: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
        let mut note = |predicate: &Predicate| match predicate {
            Predicate::Evidence(matched) => {
                let kind = matched.kind.as_str().to_owned();
                *with_result.entry(kind.clone()).or_default() |= matched.result.is_some();
                if let Some(subject) = &matched.subject {
                    subjects
                        .entry(kind)
                        .or_default()
                        .insert(subject.as_str().to_owned());
                }
            }
            Predicate::First(first) => {
                with_result.insert(first.kind.as_str().to_owned(), true);
            }
            Predicate::Before(order) => {
                for kind in [&order.first, &order.then] {
                    with_result.entry(kind.as_str().to_owned()).or_default();
                    if let Some(subject) = &order.subject {
                        subjects
                            .entry(kind.as_str().to_owned())
                            .or_default()
                            .insert(subject.as_str().to_owned());
                    }
                }
            }
            _ => {}
        };
        for (_, claim) in model.claims.iter() {
            claim.true_when.visit(&mut note);
        }
        for (_, obligation) in model.obligations.iter() {
            obligation.discharged_when.visit(&mut note);
            if let Some(applicable) = &obligation.applicable_when {
                applicable.visit(&mut note);
            }
        }
        for (_, action) in model.actions.iter() {
            if let Some(precondition) = &action.precondition {
                precondition.visit(&mut note);
            }
        }
        for (_, outcome) in model.outcomes.iter() {
            if let Some(predicate) = outcome.requires.predicate() {
                predicate.visit(&mut note);
            }
        }
        // A kind a built-in validator writes is an observation whatever action a protocol names
        // as its producer: one a read action outside the built-in validators may produce would
        // pass for a choice, and the protocol is refused.
        for (action, produced) in &produces {
            if crate::validator::is_built_in_action(action)
                || effects.get(action).is_none_or(|effect| effect != "read")
            {
                continue;
            }
            if let Some(kind) = produced
                .iter()
                .find(|kind| crate::validator::is_built_in_kind(kind))
            {
                bail!(
                    "the protocol {} has {action}, an action no built-in validator performs, produce {kind}, which only a built-in validator writes: a record of it from that action would pass for an observation",
                    protocol.id
                );
            }
        }
        let mut choices = BTreeMap::new();
        for (kind, read_with_result) in &with_result {
            let producers: Vec<&String> = produces
                .iter()
                .filter(|(_, produced)| produced.contains(kind))
                .map(|(action, _)| action)
                .collect();
            if !*read_with_result
                && !crate::validator::is_built_in_kind(kind)
                && !producers.is_empty()
                && producers
                    .iter()
                    .all(|action| effects.get(*action).is_some_and(|e| e == "read"))
                && !producers
                    .iter()
                    .any(|action| crate::validator::is_built_in_action(action))
            {
                let about = subjects.get(kind).cloned().unwrap_or_default();
                if about.is_empty() {
                    bail!(
                        "the protocol {} reads {kind}, a kind that records a choice ({} may produce it), about no artifact: its record could be bound to no revision; name the artifact (subject) where it is read",
                        protocol.id,
                        producers
                            .iter()
                            .map(|action| action.as_str())
                            .collect::<Vec<_>>()
                            .join(", ")
                    );
                }
                choices.insert(kind.clone(), about);
            }
        }
        // No outcome and no obligation may hold on choice records alone.
        let mut values = BTreeMap::new();
        for (id, _) in model.claims.iter() {
            claim_under_choices(id.as_str(), &model, &choices, inputs, &mut values);
        }
        for (id, outcome) in model.outcomes.iter() {
            if let Some(predicate) = outcome.requires.predicate()
                && under_choices(predicate, &model, &choices, inputs, &mut values) == Some(true)
            {
                bail!(
                    "the protocol {} lets its outcome {} hold with every claim a choice record decides true and every other claim unknown: a choice record may gate an action, never an outcome alone",
                    protocol.id,
                    id.as_str()
                );
            }
        }
        for (id, obligation) in model.obligations.iter() {
            if under_choices(
                &obligation.discharged_when,
                &model,
                &choices,
                inputs,
                &mut values,
            ) == Some(true)
            {
                bail!(
                    "the protocol {} discharges its obligation {} with every claim a choice record decides true and every other claim unknown: a choice record may gate an action, never an obligation alone",
                    protocol.id,
                    id.as_str()
                );
            }
        }
        let blocked_for_good = model
            .actions
            .iter()
            .filter(|(_, action)| {
                action
                    .precondition
                    .as_ref()
                    .is_some_and(|precondition| by_inputs(precondition, inputs) == Some(false))
            })
            .map(|(id, _)| id.as_str().to_owned())
            .collect();
        let mut direct: BTreeMap<String, (BTreeSet<String>, BTreeSet<String>)> = BTreeMap::new();
        for (id, claim) in model.claims.iter() {
            let mut kinds = BTreeSet::new();
            let mut claims = BTreeSet::new();
            claim.true_when.visit(&mut |predicate| match predicate {
                Predicate::Evidence(matched) => {
                    kinds.insert(matched.kind.as_str().to_owned());
                }
                Predicate::First(first) => {
                    kinds.insert(first.kind.as_str().to_owned());
                }
                Predicate::Before(order) => {
                    kinds.insert(order.first.as_str().to_owned());
                    kinds.insert(order.then.as_str().to_owned());
                }
                Predicate::Claim(test) => {
                    claims.insert(test.claim.as_str().to_owned());
                }
                _ => {}
            });
            direct.insert(id.as_str().to_owned(), (kinds, claims));
        }
        let mut reads = BTreeMap::new();
        for id in direct.keys() {
            let mut kinds = BTreeSet::new();
            let mut seen = BTreeSet::new();
            let mut todo = vec![id.clone()];
            while let Some(claim) = todo.pop() {
                if !seen.insert(claim.clone()) {
                    continue;
                }
                if let Some((own, through)) = direct.get(&claim) {
                    kinds.extend(own.iter().cloned());
                    todo.extend(through.iter().cloned());
                }
            }
            reads.insert(id.clone(), kinds);
        }
        // No outcome and no obligation may read a choice kind through a form a choice record
        // could make falser: with only monotone forms left, the check above is the worst case.
        let mut checked: Vec<(String, &Predicate)> = Vec::new();
        for (id, outcome) in model.outcomes.iter() {
            if let Some(predicate) = outcome.requires.predicate() {
                checked.push((format!("outcome {}", id.as_str()), predicate));
            }
        }
        for (id, obligation) in model.obligations.iter() {
            checked.push((
                format!("obligation {}", id.as_str()),
                &obligation.discharged_when,
            ));
            if let Some(applicable) = &obligation.applicable_when {
                checked.push((format!("obligation {}", id.as_str()), applicable));
            }
        }
        for (what, predicate) in checked {
            if let Some(form) =
                non_monotone_choice(predicate, &model, &choices, &reads, &mut BTreeSet::new())
            {
                bail!(
                    "the protocol {} reads a choice kind in its {what} through {form}: a choice record could then make it truer by its absence or its order, and a choice may gate an action, never an outcome or an obligation",
                    protocol.id
                );
            }
        }
        let discharges = model
            .obligations
            .iter()
            .map(|(id, obligation)| {
                (
                    id.as_str().to_owned(),
                    kinds_read(&obligation.discharged_when, &reads),
                )
            })
            .collect();
        Ok(Self {
            effects,
            produces,
            reads,
            choices,
            blocked_for_good,
            discharges,
        })
    }

    /// Whether `action`'s declared effect is `effect`.
    fn is(&self, action: &str, effect: &str) -> bool {
        self.effects.get(action).is_some_and(|e| e == effect)
    }

    /// The kinds `action` may produce that record a choice, each with the artifacts the
    /// protocol reads it about.
    fn choices_of(&self, action: &str) -> Vec<(&String, &BTreeSet<String>)> {
        self.produces
            .get(action)
            .map(|produced| {
                self.choices
                    .iter()
                    .filter(|(kind, _)| produced.contains(*kind))
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Whether a harness that keeps no state between steps may be offered `action`: one that
    /// only reads is offered only when it records a choice, so that taking it moves the
    /// decision; any other read would be gone at the next step, and the step, changing nothing,
    /// would end the run.
    fn for_stateless(&self, action: &str) -> bool {
        !self.is(action, "read") || !self.choices_of(action).is_empty()
    }

    /// Whether `action` may be offered to the harness at all: never one that may produce a kind
    /// a built-in validator writes (`change.read` among them), whether or not the run has that
    /// validator. Only a validator writes such a kind, so the action is the run loop's to take,
    /// and a harness that took it could move nothing.
    fn for_harness(&self, action: &str) -> bool {
        self.produces.get(action).is_none_or(|produced| {
            !produced
                .iter()
                .any(|kind| crate::validator::is_built_in_kind(kind))
        })
    }

    /// The actions that may produce evidence `claim` is decided from, but those the case's
    /// inputs block for good.
    fn producers(&self, claim: &str) -> BTreeSet<String> {
        let Some(kinds) = self.reads.get(claim) else {
            return BTreeSet::new();
        };
        self.produces
            .iter()
            .filter(|(action, produced)| {
                !produced.is_disjoint(kinds) && !self.blocked_for_good.contains(*action)
            })
            .map(|(action, _)| action.clone())
            .collect()
    }

    /// The actions that stand between `decision` and its outcome when nothing else does: every
    /// claim that does not hold, and that some action can decide, is decided only by evidence
    /// an action needing an approval (`approvals`) produces. No judgement step brings the
    /// outcome nearer then: an edit only moves the revision and returns every claim to
    /// unknown. `None` when some such claim can still be decided another way.
    fn only_behind(&self, decision: &crate::Decision, approvals: &[String]) -> Option<Vec<String>> {
        let mut behind = BTreeSet::new();
        for (claim, value) in &decision.claims {
            if value == "true" {
                continue;
            }
            let producers = self.producers(claim);
            if producers.is_empty() {
                continue;
            }
            if !producers.iter().all(|action| approvals.contains(action)) {
                return None;
            }
            behind.extend(producers);
        }
        (!behind.is_empty()).then(|| behind.into_iter().collect())
    }

    /// The claim no step can move the case past, with the action that alone would decide it,
    /// when none of the `offered` actions produces a kind an open obligation reads, and some
    /// claim that does not hold is decided only by built-in actions the run has no validator for
    /// (`performed` false), one of them admissible. An edit produces nothing and only moves the
    /// revision. `None` while an offered action could still move an open obligation, or every
    /// claim left has a performer or waits on an action not yet admissible: `--forge none` works
    /// up to `pr.open` admissible, its change read ahead, before this names `pr.opened`.
    fn unperformable(
        &self,
        decision: &crate::Decision,
        offered: &[String],
        performed: impl Fn(&str) -> bool,
    ) -> Option<(String, String)> {
        let open: BTreeSet<&String> = decision
            .obligations
            .iter()
            .filter(|(_, status)| status.as_str() == "open")
            .filter_map(|(id, _)| self.discharges.get(id))
            .flatten()
            .collect();
        if offered.iter().any(|action| {
            self.produces
                .get(action)
                .is_some_and(|produced| produced.iter().any(|kind| open.contains(kind)))
        }) {
            return None;
        }
        let admissible = decision.admissible();
        for (claim, value) in &decision.claims {
            if value == "true" {
                continue;
            }
            let producers = self.producers(claim);
            if producers.is_empty()
                || producers.iter().any(|action| {
                    !crate::validator::is_built_in_action(action) || performed(action)
                })
            {
                continue;
            }
            if let Some(action) = producers.iter().find(|action| admissible.contains(action)) {
                return Some((claim.clone(), action.clone()));
            }
        }
        None
    }
}

/// What a run lacks to perform the built-in `action`, as [`Stop::Unperformable`] names it.
fn performer_of(action: &str) -> String {
    let validator = crate::validator::BUILT_IN
        .iter()
        .find(|built_in| built_in.action == action)
        .map(|built_in| built_in.validator);
    match validator {
        Some("GateRun") => "the gates document".to_owned(),
        Some("PrOpen" | "CiRead" | "PrMerged") => "the forge".to_owned(),
        Some("TestsRun") => "a test command".to_owned(),
        Some(validator) => format!("its validator {validator}"),
        None => format!("a validator for {action}"),
    }
}

/// One entry of a snapshot: a regular file with its bytes and whether it is executable, a
/// symbolic link with its target, or anything else (a FIFO, a socket, a device) by its kind
/// alone. A link is an entry by itself and is never followed: what it points at may lie outside
/// the tree, and nothing the kernel does for a case reaches outside the tree. Anything else is
/// never opened: reading a FIFO no one writes would block the run for good. A file outside the
/// tree too large to keep a copy of ([`Outside`]) is its fingerprint alone ([`fingerprint`]),
/// which tells a change but cannot put the file back.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Entry {
    File { bytes: Vec<u8>, executable: bool },
    Link(PathBuf),
    Other(&'static str),
    Large(String),
}

/// The size past which [`Outside`] keeps a file's fingerprint, not its bytes.
const OUTSIDE_BYTES_AT_MOST: u64 = 1 << 20;

/// What tells a change to the file `meta` describes without reading it: its size, mode, device
/// and inode, and its modification and change times to the nanosecond. The kernel sets the
/// change time on every write and every change of mode or times, and no user can set it back,
/// so a file whose fingerprint is the same holds the same bytes.
fn fingerprint(meta: &std::fs::Metadata) -> String {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt as _;
        format!(
            "{} bytes, mode {:o}, inode {}:{}, modified {}.{:09}, changed {}.{:09}",
            meta.size(),
            meta.mode(),
            meta.dev(),
            meta.ino(),
            meta.mtime(),
            meta.mtime_nsec(),
            meta.ctime(),
            meta.ctime_nsec()
        )
    }
    #[cfg(not(unix))]
    {
        format!("{} bytes, modified {:?}", meta.len(), meta.modified().ok())
    }
}

/// What a file that is not a regular file, a directory or a link is.
fn other_kind(kind: std::fs::FileType) -> &'static str {
    #[cfg(unix)]
    {
        use std::os::unix::fs::FileTypeExt as _;
        if kind.is_fifo() {
            return "fifo";
        }
        if kind.is_socket() {
            return "socket";
        }
        if kind.is_block_device() {
            return "block device";
        }
        if kind.is_char_device() {
            return "character device";
        }
    }
    let _ = kind;
    "other"
}

/// Whether `meta` gives its file an execute bit.
#[cfg(unix)]
fn is_executable(meta: &std::fs::Metadata) -> bool {
    use std::os::unix::fs::PermissionsExt as _;
    meta.permissions().mode() & 0o111 != 0
}

/// Whether `meta` gives its file an execute bit: never on this platform.
#[cfg(not(unix))]
fn is_executable(_: &std::fs::Metadata) -> bool {
    false
}

/// Every file, link and other entry under `dir`, by path relative to it: the cases directory
/// as it is before a judgement step, which the step may not touch.
fn snapshot(dir: &Path) -> Result<BTreeMap<PathBuf, Entry>> {
    snapshot_without(dir, None)
}

/// Whether `path`, relative to the tree and as git names it, is the directory that holds the cases
/// root (`.takt`) or the ignore file that keeps it out of version control (`.takt/.gitignore`).
/// An observation may not change either, whatever the judge surface: git ignores what they keep,
/// so the surface does not show a change to them.
fn takt_guard(path: &str) -> bool {
    let path = path.trim_end_matches('/');
    path == crate::case::TAKT_DIR
        || path
            .strip_prefix(crate::case::TAKT_DIR)
            .and_then(|rest| rest.strip_prefix('/'))
            == Some(crate::case::TAKT_GITIGNORE)
}

/// [`snapshot`] of both cases roots under `tree` ([`CASE_ROOTS`]), each entry keyed by its path
/// relative to `tree`: one map holds a case under either root, and [`restore`] puts it back from
/// `tree`. A root that is absent contributes nothing.
fn snapshot_cases(tree: &Path) -> Result<BTreeMap<PathBuf, Entry>> {
    let mut entries = BTreeMap::new();
    for root in CASE_ROOTS {
        for (path, entry) in snapshot(&tree.join(root))? {
            entries.insert(Path::new(root).join(path), entry);
        }
    }
    Ok(entries)
}

/// [`snapshot`] of `dir` without what lies under `without`, a path relative to it.
fn snapshot_without(dir: &Path, without: Option<&Path>) -> Result<BTreeMap<PathBuf, Entry>> {
    fn walk(
        root: &Path,
        dir: &Path,
        without: Option<&Path>,
        entries: &mut BTreeMap<PathBuf, Entry>,
    ) -> Result<()> {
        for entry in std::fs::read_dir(dir).with_context(|| format!("read {}", dir.display()))? {
            let path = entry?.path();
            let relative = path.strip_prefix(root).unwrap_or(&path).to_path_buf();
            if without.is_some_and(|without| relative.starts_with(without)) {
                continue;
            }
            let meta = std::fs::symlink_metadata(&path)
                .with_context(|| format!("read {}", path.display()))?;
            let kind = meta.file_type();
            if kind.is_symlink() {
                let target = std::fs::read_link(&path)
                    .with_context(|| format!("read the link {}", path.display()))?;
                entries.insert(relative, Entry::Link(target));
            } else if kind.is_dir() {
                walk(root, &path, without, entries)?;
            } else if kind.is_file() {
                let bytes =
                    std::fs::read(&path).with_context(|| format!("read {}", path.display()))?;
                entries.insert(
                    relative,
                    Entry::File {
                        bytes,
                        executable: is_executable(&meta),
                    },
                );
            } else {
                entries.insert(relative, Entry::Other(other_kind(kind)));
            }
        }
        Ok(())
    }
    let mut entries = BTreeMap::new();
    if std::fs::symlink_metadata(dir).is_ok_and(|meta| meta.is_dir()) {
        walk(dir, dir, without, &mut entries)?;
    }
    Ok(entries)
}

/// The paths at which two snapshots differ: added, removed or changed, in path order.
fn differing(before: &BTreeMap<PathBuf, Entry>, after: &BTreeMap<PathBuf, Entry>) -> Vec<PathBuf> {
    before
        .keys()
        .chain(after.keys())
        .filter(|path| before.get(*path) != after.get(*path))
        .cloned()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

/// Makes the link `at` pointing at `target`.
#[cfg(unix)]
fn make_link(target: &Path, at: &Path) -> Result<()> {
    std::os::unix::fs::symlink(target, at)
        .with_context(|| format!("put back the link {}", at.display()))
}

/// Makes the link `at` pointing at `target`: not on this platform.
#[cfg(not(unix))]
fn make_link(target: &Path, at: &Path) -> Result<()> {
    bail!(
        "the link {} to {} cannot be put back on this platform",
        at.display(),
        target.display()
    )
}

/// Writes `bytes` to `at` and gives it an execute bit or takes it away, as `executable` says.
fn put_back_file(at: &Path, bytes: &[u8], executable: bool) -> Result<()> {
    std::fs::write(at, bytes).with_context(|| format!("put back {}", at.display()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        let mut permissions = std::fs::metadata(at)
            .with_context(|| format!("read {}", at.display()))?
            .permissions();
        let mode = permissions.mode();
        permissions.set_mode(if executable {
            mode | ((mode & 0o444) >> 2)
        } else {
            mode & !0o111
        });
        std::fs::set_permissions(at, permissions)
            .with_context(|| format!("put back the mode of {}", at.display()))?;
    }
    #[cfg(not(unix))]
    let _ = executable;
    Ok(())
}

/// Puts `dir` back to `before` at the paths `changed`: whatever is at the path now is removed
/// (a link as the link itself, never what it points at; a directory the step made in a file's
/// place with everything in it), then the file is written again as it was, its mode with it,
/// or the link made again; a path the step added stays removed, and an entry that was neither
/// a file nor a link (a FIFO) cannot be made again and stays removed too. The paths go in
/// order, so a link or file the step put in place of a directory is gone before anything is
/// written below it. Something in `dir`'s own place (a link, as the link itself, or a file) is
/// removed first and every path of `before` put back: nothing is ever put back through it.
/// `dir` lies under `root`, the tree, and before anything is removed or written each part of the
/// way from `root` down is read with `symlink_metadata`: a link on the way, which would take the
/// write wherever it points, refuses it, naming the link.
fn restore(
    root: &Path,
    dir: &Path,
    before: &BTreeMap<PathBuf, Entry>,
    changed: &[PathBuf],
) -> Result<()> {
    no_link_on_the_way(root, dir)?;
    let mut changed = changed.to_vec();
    if std::fs::symlink_metadata(dir).is_ok_and(|meta| !meta.is_dir()) {
        std::fs::remove_file(dir).with_context(|| format!("remove {}", dir.display()))?;
        changed = before.keys().cloned().collect();
    }
    // A cases root that is no longer a directory (a file, a link, another kind) is removed, and
    // every path of `before` under it put back, whichever root the step replaced.
    for case_root in CASE_ROOTS {
        let at = dir.join(case_root);
        if std::fs::symlink_metadata(&at).is_ok_and(|meta| !meta.is_dir()) {
            no_link_on_the_way(root, &at)?;
            std::fs::remove_file(&at).with_context(|| format!("remove {}", at.display()))?;
            for path in before.keys().filter(|path| path.starts_with(case_root)) {
                if !changed.contains(path) {
                    changed.push(path.clone());
                }
            }
        }
    }
    for path in &changed {
        let full = dir.join(path);
        no_link_on_the_way(root, &full)?;
        if let Ok(meta) = std::fs::symlink_metadata(&full) {
            if meta.is_dir() {
                std::fs::remove_dir_all(&full)
            } else {
                std::fs::remove_file(&full)
            }
            .with_context(|| format!("remove {}", full.display()))?;
        }
        if let Some(parent) = full.parent()
            && before.contains_key(path)
        {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("create {}", parent.display()))?;
        }
        match before.get(path) {
            Some(Entry::File { bytes, executable }) => put_back_file(&full, bytes, *executable)?,
            Some(Entry::Link(target)) => make_link(target, &full)?,
            Some(Entry::Other(_) | Entry::Large(_)) | None => {}
        }
    }
    Ok(())
}

/// Refuses a write at `at` when a part of the way from `root` down to it, `at` itself left out,
/// is a link ([`Outside::link_on_the_way`]): the refusal names the link, relative to `root`.
fn no_link_on_the_way(root: &Path, at: &Path) -> Result<()> {
    if let Some(link) = Outside::link_on_the_way(root, at) {
        bail!(
            "{} is a link on the way to {}; nothing is put back through it",
            link.strip_prefix(root).unwrap_or(&link).display(),
            at.strip_prefix(root).unwrap_or(at).display()
        );
    }
    Ok(())
}

/// The directory under the tree that holds the planning store, the project file that says
/// where the store is kept, and the cases: everything in it but the cases is the store's, which
/// no judgement step writes.
const ENGINEERING: &str = ".engineering";

/// The files that decide what pytest collects and how it reports, at any depth: a
/// `conftest.py` can rewrite every outcome. `pyproject.toml`'s `[tool.pytest*]` tables are
/// compared by the manifest scanner ([`pytest_config_changes`]). And Python's start-up modules,
/// which the interpreter imports from its import path before it runs anything: a
/// `sitecustomize.py` that exits 0 passes every run.
const PYTHON_FILES: &[&str] = &[
    "conftest.py",
    "pytest.ini",
    ".pytest.ini",
    "pytest.toml",
    ".pytest.toml",
    "tox.ini",
    "setup.cfg",
    "sitecustomize.py",
    "usercustomize.py",
];

/// The CI definition of a repository on a GitLab host, at the tree's root: on the judge surface
/// as `.github/**` is, so a step that edits what the checks run leaves `ci.read` nothing to read.
pub const GITLAB_CI: &str = ".gitlab-ci.yml";

/// The directory at the tree's root where a GitLab repository keeps what its CI definition
/// includes (`include: local: .gitlab/ci/*.yml`) and its review rules (`.gitlab/CODEOWNERS`): on
/// the judge surface with [`GITLAB_CI`], everything under it. A file `include: local:` names
/// outside both is not.
pub const GITLAB_DIR: &str = ".gitlab";

/// The judge surface every run guards, as a refusal names it.
pub const JUDGE_SURFACE: &str = "`.cargo/**` and `rust-toolchain*` and `build.rs` at any depth, `.github/**`, `.gitlab-ci.yml` at the root, `.gitlab/**`, `.engineering/**` but `.engineering/cases/**`, `conftest.py`, `pytest.ini`, `.pytest.ini`, `pytest.toml`, `.pytest.toml`, `tox.ini`, `setup.cfg`, `sitecustomize.py` and `usercustomize.py` at any depth, `*.pth` at the root and directly in a `site-packages` or `dist-packages` directory, `[tool.pytest*]` in `pyproject.toml`, the module and package `python -m` runs and the `*.pth` files in every directory it imports from (where it runs and what `PYTHONPATH` names), a module, compiled module or package named like one the test command's Python imports from outside the tree (asked when the case opens; the standard library's and the test runners' `unittest`, `pytest`, `_pytest`, `pluggy`, `nose2` when no Python answers) in every directory Python imports from under any program (where it runs, each `PYTHONPATH` entry, a `unittest discover` or `nose2` start and top-level directory, a test file's directory when pytest runs, a Python script's own directory), every executable entry (at the base, before or after the step) directly in a tree directory on `PATH`, the script's or the one takt runs with, the test command's own script, the scripts it runs, read one level deep (every file beside a script deeper than that), and its build tool's file, every Python file, shell file and program of the tree while a line of such a script cannot be read or a build tool's recipe runs, and the repository's own `.engineering/judge-surface.yaml` list";

/// How many scripts deep the guard reads: the test command's own script, and the scripts it
/// runs. A script deeper than that is not read; everything in its directory, and every Python
/// file of the tree, is guarded instead ([`script_paths`]).
const SCRIPTS_AT_MOST: usize = 2;

/// What a line the guard cannot read guards ([`fail_closed`]), since what it runs, and from
/// where, is unknown: every Python module, compiled module and path file of the tree, every
/// shell file, and every program ([`PROGRAMS`]: a file starting with `#!` or with an exec bit).
pub(crate) const FAIL_CLOSED: &[&str] = &[
    "**/*.py",
    "**/*.pyc",
    "**/*.pth",
    "**/*.sh",
    "**/*.bash",
    PROGRAMS,
];

/// Guards what a script line the guard cannot read could load or run ([`FAIL_CLOSED`]), and
/// names `reason` beside it ([`FAILS_CLOSED_MARK`]), so a refusal of a path it put on the
/// surface says why ([`JudgeSurface::modules_note`]).
fn fail_closed(named: &mut Vec<String>, reason: impl std::fmt::Display) {
    named.extend(FAIL_CLOSED.iter().map(|glob| (*glob).to_owned()));
    named.push(format!("{FAILS_CLOSED_MARK}{reason}"));
}

/// `text` as a reason quotes it: on one line, in backticks, cut at 80 characters.
fn quoted(text: &str) -> String {
    let line = text.split_whitespace().collect::<Vec<_>>().join(" ");
    let cut: String = line.chars().take(80).collect();
    if cut.len() < line.len() {
        format!("`{cut}…`")
    } else {
        format!("`{cut}`")
    }
}

/// What [`scripts_of`] names for every program of the tree, by what the file holds rather
/// than by its name: a file whose first line starts with `#!`, or one with an exec bit, as the
/// tree holds it now or held it when the surface was read ([`JudgeSurface::contains`]).
const PROGRAMS: &str = "**/(programs)";

/// The last part of what [`scripts_of`] names for a directory Python imports from: in it, a
/// module, compiled module or package with a name the Python imports from outside the tree
/// ([`Modules`]) is guarded, since Python finds it there before the installed one.
pub(crate) const IMPORTS: &str = "(imports)";

/// The last part of what [`scripts_of`] names for a directory of the tree on `PATH`: every
/// entry directly in it is guarded, since a program run by its name may be found there.
const ON_PATH: &str = "(on PATH)";

/// `dir`, relative to the tree (`""` its root), marked with `mark` ([`IMPORTS`], [`ON_PATH`]).
fn marked(dir: &str, mark: &str) -> String {
    if dir.is_empty() {
        mark.to_owned()
    } else {
        format!("{dir}/{mark}")
    }
}

/// The test runners' packages, refused in every import directory whatever Python answered, and
/// with the standard library's ([`STDLIB_MODULES`]) the fallback when none did ([`Modules`]).
const RUNNER_PACKAGES: &[&str] = &["unittest", "pytest", "_pytest", "pluggy", "nose2"];

/// The standard library's module names: `sys.stdlib_module_names` of CPython 3.14.7. A module of
/// this name in a directory Python imports from replaces the installed one for the test runner
/// and for the tests alike. The fallback when no Python answers at case open ([`Modules`]).
const STDLIB_MODULES: &[&str] = &[
    "__future__",
    "_abc",
    "_aix_support",
    "_android_support",
    "_apple_support",
    "_ast",
    "_ast_unparse",
    "_asyncio",
    "_bisect",
    "_blake2",
    "_bz2",
    "_codecs",
    "_codecs_cn",
    "_codecs_hk",
    "_codecs_iso2022",
    "_codecs_jp",
    "_codecs_kr",
    "_codecs_tw",
    "_collections",
    "_collections_abc",
    "_colorize",
    "_compat_pickle",
    "_contextvars",
    "_csv",
    "_ctypes",
    "_curses",
    "_curses_panel",
    "_datetime",
    "_dbm",
    "_decimal",
    "_elementtree",
    "_frozen_importlib",
    "_frozen_importlib_external",
    "_functools",
    "_gdbm",
    "_hashlib",
    "_heapq",
    "_hmac",
    "_imp",
    "_interpchannels",
    "_interpqueues",
    "_interpreters",
    "_io",
    "_ios_support",
    "_json",
    "_locale",
    "_lsprof",
    "_lzma",
    "_markupbase",
    "_md5",
    "_multibytecodec",
    "_multiprocessing",
    "_opcode",
    "_opcode_metadata",
    "_operator",
    "_osx_support",
    "_overlapped",
    "_pickle",
    "_posixshmem",
    "_posixsubprocess",
    "_py_abc",
    "_py_warnings",
    "_pydatetime",
    "_pydecimal",
    "_pyio",
    "_pylong",
    "_pyrepl",
    "_queue",
    "_random",
    "_remote_debugging",
    "_scproxy",
    "_sha1",
    "_sha2",
    "_sha3",
    "_signal",
    "_sitebuiltins",
    "_socket",
    "_sqlite3",
    "_sre",
    "_ssl",
    "_stat",
    "_statistics",
    "_string",
    "_strptime",
    "_struct",
    "_suggestions",
    "_symtable",
    "_sysconfig",
    "_thread",
    "_threading_local",
    "_tkinter",
    "_tokenize",
    "_tracemalloc",
    "_types",
    "_typing",
    "_uuid",
    "_warnings",
    "_weakref",
    "_weakrefset",
    "_winapi",
    "_wmi",
    "_zoneinfo",
    "_zstd",
    "abc",
    "annotationlib",
    "antigravity",
    "argparse",
    "array",
    "ast",
    "asyncio",
    "atexit",
    "base64",
    "bdb",
    "binascii",
    "bisect",
    "builtins",
    "bz2",
    "cProfile",
    "calendar",
    "cmath",
    "cmd",
    "code",
    "codecs",
    "codeop",
    "collections",
    "colorsys",
    "compileall",
    "compression",
    "concurrent",
    "configparser",
    "contextlib",
    "contextvars",
    "copy",
    "copyreg",
    "csv",
    "ctypes",
    "curses",
    "dataclasses",
    "datetime",
    "dbm",
    "decimal",
    "difflib",
    "dis",
    "doctest",
    "email",
    "encodings",
    "ensurepip",
    "enum",
    "errno",
    "faulthandler",
    "fcntl",
    "filecmp",
    "fileinput",
    "fnmatch",
    "fractions",
    "ftplib",
    "functools",
    "gc",
    "genericpath",
    "getopt",
    "getpass",
    "gettext",
    "glob",
    "graphlib",
    "grp",
    "gzip",
    "hashlib",
    "heapq",
    "hmac",
    "html",
    "http",
    "idlelib",
    "imaplib",
    "importlib",
    "inspect",
    "io",
    "ipaddress",
    "itertools",
    "json",
    "keyword",
    "linecache",
    "locale",
    "logging",
    "lzma",
    "mailbox",
    "marshal",
    "math",
    "mimetypes",
    "mmap",
    "modulefinder",
    "msvcrt",
    "multiprocessing",
    "netrc",
    "nt",
    "ntpath",
    "nturl2path",
    "numbers",
    "opcode",
    "operator",
    "optparse",
    "os",
    "pathlib",
    "pdb",
    "pickle",
    "pickletools",
    "pkgutil",
    "platform",
    "plistlib",
    "poplib",
    "posix",
    "posixpath",
    "pprint",
    "profile",
    "pstats",
    "pty",
    "pwd",
    "py_compile",
    "pyclbr",
    "pydoc",
    "pydoc_data",
    "pyexpat",
    "queue",
    "quopri",
    "random",
    "re",
    "readline",
    "reprlib",
    "resource",
    "rlcompleter",
    "runpy",
    "sched",
    "secrets",
    "select",
    "selectors",
    "shelve",
    "shlex",
    "shutil",
    "signal",
    "site",
    "smtplib",
    "socket",
    "socketserver",
    "sqlite3",
    "sre_compile",
    "sre_constants",
    "sre_parse",
    "ssl",
    "stat",
    "statistics",
    "string",
    "stringprep",
    "struct",
    "subprocess",
    "symtable",
    "sys",
    "sysconfig",
    "syslog",
    "tabnanny",
    "tarfile",
    "tempfile",
    "termios",
    "textwrap",
    "this",
    "threading",
    "time",
    "timeit",
    "tkinter",
    "token",
    "tokenize",
    "tomllib",
    "trace",
    "traceback",
    "tracemalloc",
    "tty",
    "turtle",
    "turtledemo",
    "types",
    "typing",
    "unicodedata",
    "unittest",
    "urllib",
    "uuid",
    "venv",
    "warnings",
    "wave",
    "weakref",
    "webbrowser",
    "winreg",
    "winsound",
    "wsgiref",
    "xml",
    "xmlrpc",
    "zipapp",
    "zipfile",
    "zipimport",
    "zlib",
    "zoneinfo",
];

/// The module the file `name` in an import directory loads as: `m.py`, `m.pyc`, `m.pyw`, `m.pyd`,
/// `m.so` and `m.<tag>.so` are `m`, and a name with no suffix (a package) is itself.
pub(crate) fn module_of(name: &str) -> Option<&str> {
    let Some((stem, suffix)) = name.split_once('.') else {
        return Some(name);
    };
    (["py", "pyc", "pyw", "pyd", "so"].contains(&suffix)
        || suffix.ends_with(".so")
        || suffix.ends_with(".pyd"))
    .then_some(stem)
}

/// Whether the file at `full` is a program: a regular file with an exec bit, or one whose first
/// two bytes are `#!`. A link is not followed.
fn is_program(full: &Path) -> bool {
    use std::io::Read as _;
    let Ok(meta) = std::fs::symlink_metadata(full) else {
        return false;
    };
    if !meta.is_file() {
        return false;
    }
    if is_executable(&meta) {
        return true;
    }
    let mut start = [0_u8; 2];
    std::fs::File::open(full)
        .and_then(|mut file| file.read_exact(&mut start))
        .is_ok_and(|()| &start == b"#!")
}

/// The interpreter the first line of the file at `full` names (`#!/usr/bin/env python3` is
/// `python`), folded and without its version ([`program_of`]); `None` for a file with no `#!`.
fn shebang_of(full: &Path) -> Option<String> {
    use std::io::Read as _;
    let meta = std::fs::symlink_metadata(full).ok()?;
    if !meta.is_file() {
        return None;
    }
    let mut start = Vec::new();
    std::fs::File::open(full)
        .and_then(|file| file.take(256).read_to_end(&mut start))
        .ok()?;
    let text = String::from_utf8_lossy(&start);
    interpreter_of(text.strip_prefix("#!")?)
}

/// The interpreter a `#!` line (after the `#!`) names, `env` and its options skipped.
pub(crate) fn interpreter_of(shebang: &str) -> Option<String> {
    let line = shebang.lines().next().unwrap_or_default();
    let mut words = shell_words(line).into_iter().map(|word| program_of(&word));
    let interpreter = words.next()?;
    if interpreter == "env" {
        return words.find(|word| !word.starts_with('-') && !word.contains('='));
    }
    Some(interpreter)
}

/// The first part of what [`scripts_of`] names for a program a command runs, which no
/// [`JudgeSurface`] guards: the case asks the Python it is, or its `#!` line names, for the
/// module names that Python imports from outside the tree ([`Modules::ask`]). The rest is the
/// program's name, its absolute path outside the tree, or `tree:` and its path in the tree.
const PROGRAM_MARK: &str = "(program) ";

/// The separator between a program's spec and the directories the script prepended to `PATH`
/// in a [`PROGRAM_MARK`], and between those directories.
const MARK_SEP: char = '\u{1f}';

/// What [`scripts_of`] names for a directory outside the tree the script prepends to `PATH`
/// ([`OUTSIDE_PATH_MARK`]): a step may write nothing into it, since the test command would run
/// a program it finds there.
const OUTSIDE_PATH_MARK: &str = "(outside path) ";

/// What [`scripts_of`] names for a directory or file outside the tree the script makes Python
/// load code from: a `PYTHONPATH` entry, or what a variable of [`PYTHON_DIRS`] names. A step
/// may write nothing in it, since the test command's Python would load what it finds there.
const OUTSIDE_IMPORTS_MARK: &str = "(outside imports) ";

/// What [`scripts_of`] names, beside failing closed, for why a script fails closed at open: the
/// refusal names it ([`JudgeSurface::modules_note`]).
pub(crate) const FAILS_CLOSED_MARK: &str = "(fails closed) ";

/// What [`read_command`] names, beside the path itself, for what a Python loads as code: a
/// script a Python runs (`python3 check.py`, a file whose `#!` line names a Python), and what
/// `python -m` runs and the `*.pth` files beside it. The gate compares them at the base
/// ([`crate::validator::GateRun`]) wherever they were named, in a `sh -c` string or a shell
/// script as well; [`scripts_of`] leaves the mark out.
pub(crate) const PYTHON_MARK: &str = "(python) ";

/// `path` named with [`PYTHON_MARK`] as well as by itself.
fn python_named(named: &mut Vec<String>, path: String) {
    named.push(format!("{PYTHON_MARK}{path}"));
    named.push(path);
}

/// What [`scripts_of`] names for the program `word` run from `dir` with `env` ([`PROGRAM_MARK`]):
/// the program (a bare name, an absolute path, or `tree:` and its path in the tree), then the
/// directories the script prepended to `PATH`, so [`Modules::ask`] resolves a bare name against
/// them ahead of the inherited `PATH`. Nothing for a program the guard cannot name.
fn program_mark(tree: &Path, dir: Option<&str>, word: &str, env: &Env) -> Option<String> {
    if word.is_empty() || opaque(word) {
        return None;
    }
    let spec = if !word.contains('/') {
        word.to_owned()
    } else if let Some(path) = named_from(tree, dir, word) {
        format!("tree:{path}")
    } else {
        let full = if Path::new(word).is_absolute() {
            PathBuf::from(word)
        } else {
            std::path::absolute(tree).ok()?.join(dir?).join(word)
        };
        full.display().to_string()
    };
    let mut mark = format!("{PROGRAM_MARK}{spec}");
    for prefix in &env.path_prefix {
        mark.push(MARK_SEP);
        mark.push_str(prefix);
    }
    Some(mark)
}

/// The absolute path of the `PATH` entry `entry`, named from the directory `dir` of the tree
/// (`""` its root): an absolute entry as it is, a relative one resolved against the shell's
/// working directory, an empty one the working directory itself; `None` when the directory is
/// not known.
fn absolute_entry(tree: &Path, dir: Option<&str>, entry: &str) -> Option<String> {
    if Path::new(entry).is_absolute() {
        return Some(entry.to_owned());
    }
    let dir = dir.filter(|dir| *dir != ANYWHERE && !dir.starts_with("**/"))?;
    let base = std::path::absolute(tree).ok()?.join(dir);
    let joined = if entry.is_empty() {
        base
    } else {
        base.join(entry)
    };
    Some(joined.display().to_string())
}

/// The directories outside the tree the test command `runs` prepends to `PATH`
/// ([`OUTSIDE_PATH_MARK`]): a step may write nothing into them, so takt snapshots them with the
/// rest of what it reads outside the tree ([`Outside`]).
#[must_use]
pub fn outside_path_dirs(runs: &[String], tree: &Path) -> Vec<PathBuf> {
    scripts_of(runs, tree)
        .iter()
        .filter_map(|named| named.strip_prefix(OUTSIDE_PATH_MARK))
        .map(PathBuf::from)
        .collect()
}

/// What Python loads code from outside the tree when the test command `runs` in `tree`, so a
/// step may write nothing in it ([`Outside`]): what the script names ([`OUTSIDE_IMPORTS_MARK`]),
/// each absolute `PYTHONPATH` entry and [`PYTHON_DIRS`] value takt runs the command with, and
/// the directories the case's Pythons import from ([`Modules::sites`]).
#[must_use]
pub fn outside_import_dirs(runs: &[String], tree: &Path, modules: &Modules) -> Vec<PathBuf> {
    let mut dirs: BTreeSet<PathBuf> = scripts_of(runs, tree)
        .iter()
        .filter_map(|named| named.strip_prefix(OUTSIDE_IMPORTS_MARK))
        .map(PathBuf::from)
        .collect();
    if let Some(path) = std::env::var_os("PYTHONPATH") {
        dirs.extend(std::env::split_paths(&path).filter(|dir| dir.is_absolute()));
    }
    for name in PYTHON_DIRS {
        dirs.extend(
            std::env::var_os(name)
                .map(PathBuf::from)
                .filter(|dir| dir.is_absolute()),
        );
    }
    dirs.extend(modules.sites.iter().map(PathBuf::from));
    dirs.into_iter().filter(|dir| !in_tree(tree, dir)).collect()
}

/// The file a case keeps its module names in when a Python answered at open ([`Modules`]), under
/// its directory.
pub const MODULES_FILE: &str = "python-modules.json";

/// What a Python asked at case open prints ([`Modules::ask`]): its path and version (`V`),
/// then each top-level module name it can import from outside the tree given as its first
/// argument, `S` for the standard library's and the built-in ones and `N` for the rest: what
/// every `sys.path` entry outside the tree holds (the standard library, site-packages, what a
/// `.pth` file adds) and what start-up imported (`sys.modules`, a `.pth` file's imports among
/// them), but a module loaded from the tree. Last, `P` and each directory outside the tree it
/// imports from: every `sys.path` entry, its site directories, and the user site under the home
/// given as its second argument (the caller's; it runs in a home of its own).
const NAMES_PY: &str = r#"
import os, sys
tree = os.path.realpath(sys.argv[1])
def inside(path):
    if not path:
        return False
    real = os.path.realpath(path)
    return real == tree or real.startswith(tree + os.sep)
strict = set(sys.stdlib_module_names) | set(sys.builtin_module_names)
names = set()
for name, module in list(sys.modules.items()):
    if not inside(getattr(module, "__file__", None) or ""):
        names.add(name.split(".")[0])
for entry in sys.path:
    if not entry or not os.path.isdir(entry) or inside(entry):
        continue
    try:
        for found in os.scandir(entry):
            stem, dot, suffix = found.name.partition(".")
            if found.is_dir() and not dot:
                names.add(stem)
            elif suffix in ("py", "pyc", "pyd", "so") or suffix.endswith((".so", ".pyd")):
                names.add(stem)
    except OSError:
        pass
print("V", sys.executable, sys.version.split()[0])
for name in sorted(strict):
    print("S", name)
for name in sorted(names - strict):
    if name.isidentifier() and name != "__pycache__":
        print("N", name)
import site
try:
    sites = list(site.getsitepackages())
except Exception:
    sites = []
if len(sys.argv) > 2 and sys.argv[2]:
    os.environ["HOME"] = sys.argv[2]
site.USER_BASE = None
site.USER_SITE = None
try:
    sites.append(site.getusersitepackages())
except Exception:
    pass
for entry in sys.path + sites:
    if entry and "\n" not in entry and not inside(entry):
        print("P", os.path.abspath(entry))
"#;

/// The top-level module names no step may put in a directory Python imports from: asked at case
/// open of the Python the test command runs, and kept with the case ([`MODULES_FILE`]) only when
/// a Python answered or could not be established; the built-in lists ([`STDLIB_MODULES`],
/// [`RUNNER_PACKAGES`]) when none answered, and then the case keeps no file.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Modules {
    /// Where the names came from: each Python asked, with its version, or the built-in lists.
    pub from: Vec<String>,
    /// The names refused in every import directory, folded: the standard library's, the
    /// built-in modules' and the test runners' packages.
    pub strict: BTreeSet<String>,
    /// The other names the Python imports from outside the tree, folded: refused in an import
    /// directory unless the tree held a module of that name there before the step (the
    /// project's own, which already replaces the installed one).
    pub names: BTreeSet<String>,
    /// Set with its reason when the interpreter the test command runs could not be established
    /// at case open (a `PATH` the guard cannot follow, a program it cannot resolve): every
    /// Python file of the tree is then on the surface ([`JudgeSurface::contains`]), since the
    /// real interpreter's start-up imports are unknown.
    #[serde(default)]
    pub fail_closed: Option<String>,
    /// The directories outside the tree each Python asked imports from, absolute: every
    /// `sys.path` entry, its site directories and the user site under the caller's home. No
    /// step may write in them ([`Outside`]).
    #[serde(default)]
    pub sites: BTreeSet<String>,
}

/// What one Python answered at case open ([`Modules::answer`]): where it is and its version,
/// its strict names and the rest, folded, and the directories it imports from.
struct Answer {
    from: String,
    strict: BTreeSet<String>,
    names: BTreeSet<String>,
    sites: BTreeSet<String>,
}

impl Default for Modules {
    /// The built-in lists, which stand in when no Python answered.
    fn default() -> Self {
        Self {
            from: vec![format!(
                "the built-in lists: CPython 3.14.7's standard library and the test runners' packages ({}); no Python interpreter answered",
                RUNNER_PACKAGES.join(", ")
            )],
            strict: STDLIB_MODULES
                .iter()
                .chain(RUNNER_PACKAGES)
                .map(|name| fold(name))
                .collect(),
            names: BTreeSet::new(),
            fail_closed: None,
            sites: BTreeSet::new(),
        }
    }
}

/// Whether `name`, a program's name without its directory, is a Python ([`INTERPRETERS`]).
pub(crate) fn python_like(name: &str) -> bool {
    spec_of(INTERPRETERS, &program_of(name)).is_some_and(|spec| !spec.short_module.is_empty())
}

/// Whether `path` lies in `tree`, both at their real paths.
fn in_tree(tree: &Path, path: &Path) -> bool {
    match (std::fs::canonicalize(tree), std::fs::canonicalize(path)) {
        (Ok(tree), Ok(path)) => path.starts_with(tree),
        // One that does not resolve is taken as written.
        (Ok(tree), Err(_)) => std::path::absolute(path).is_ok_and(|path| path.starts_with(tree)),
        _ => false,
    }
}

/// The executable file `name` names on `PATH`, as the test command inherits it, outside `tree`:
/// a relative entry, the tree's own, is passed over.
fn lookup(name: &str, tree: &Path) -> Option<PathBuf> {
    lookup_in(name, &[], tree)
}

/// The executable file `name` names, looked up in `prefix` (absolute directories the script
/// prepended to `PATH`) and then the `PATH` the test command inherits, outside `tree`.
fn lookup_in(name: &str, prefix: &[PathBuf], tree: &Path) -> Option<PathBuf> {
    let inherited = std::env::var_os("PATH").unwrap_or_default();
    prefix
        .iter()
        .cloned()
        .chain(std::env::split_paths(&inherited))
        .filter(|dir| dir.is_absolute())
        .map(|dir| dir.join(name))
        .find(|candidate| {
            std::fs::metadata(candidate).is_ok_and(|meta| meta.is_file() && is_executable(&meta))
                && !in_tree(tree, candidate)
        })
}

/// The Python the `#!` line of the program at `path` names, outside `tree`: `#!/usr/bin/python3`,
/// or `#!/usr/bin/env python3` looked up on `PATH`.
fn shebang_python(path: &Path, tree: &Path) -> Option<PathBuf> {
    use std::io::Read as _;
    let mut start = Vec::new();
    std::fs::File::open(path)
        .and_then(|file| file.take(256).read_to_end(&mut start))
        .ok()?;
    let text = String::from_utf8_lossy(&start);
    let line = text.strip_prefix("#!")?.lines().next()?;
    let words = shell_words(line);
    let first = words.first()?;
    if program_of(first) == "env" {
        let name = words[1..]
            .iter()
            .find(|word| !word.starts_with('-') && !word.contains('='))?;
        return python_like(name).then(|| lookup(name, tree)).flatten();
    }
    let interpreter = PathBuf::from(first);
    (python_like(first) && interpreter.is_absolute() && !in_tree(tree, &interpreter))
        .then_some(interpreter)
}

/// The top-level names in the `site-packages` of a virtual environment in the tree whose
/// interpreter is at `python` (`<venv>/bin/python`), read from the directory, never run.
fn venv_names(python: &Path) -> BTreeSet<String> {
    let mut names = BTreeSet::new();
    let Some(venv) = python.parent().and_then(Path::parent) else {
        return names;
    };
    for lib in ["lib", "lib64"] {
        let Ok(versions) = std::fs::read_dir(venv.join(lib)) else {
            continue;
        };
        for version in versions.flatten() {
            let Ok(entries) = std::fs::read_dir(version.path().join("site-packages")) else {
                continue;
            };
            for entry in entries.flatten() {
                let name = entry.file_name().to_string_lossy().into_owned();
                let is_dir = entry.file_type().is_ok_and(|kind| kind.is_dir());
                let module = if name == "__pycache__" {
                    None
                } else if is_dir && !name.contains('.') {
                    Some(name.as_str())
                } else if is_dir {
                    None
                } else {
                    module_of(&name).filter(|_| name.contains('.'))
                };
                names.extend(module.map(fold));
            }
        }
    }
    names
}

impl Modules {
    /// Asks every Python the test command `runs` may run in `tree` for its module names and the
    /// directories it imports from ([`NAMES_PY`]): each program its command lines run that is a
    /// Python, or whose `#!` line names one, resolved outside the tree on the `PATH` the script
    /// leaves (its own prepends first, then the one it inherits), never takt's own `python3` in
    /// its place. Each is started with `-I`, in an empty directory of its own outside the tree
    /// with `HOME` there and `PYTHONNOUSERSITE=1`, for ten seconds at most; a file in the tree
    /// is never run, but a virtual environment's `site-packages` there is read ([`venv_names`]).
    /// The built-in lists when none answered.
    #[must_use]
    pub fn ask(runs: &[String], tree: &Path) -> Self {
        let mut pythons: Vec<PathBuf> = Vec::new();
        let mut venv = BTreeSet::new();
        let mut python_will_run = false;
        let mut unresolved: Vec<String> = Vec::new();
        // The Pythons of the tree the test command runs, which are never run here.
        let mut in_tree_pythons: Vec<String> = Vec::new();
        for mark in scripts_of(runs, tree)
            .iter()
            .filter_map(|named| named.strip_prefix(PROGRAM_MARK))
        {
            // The program's spec, then the directories the script prepended to `PATH`.
            let mut parts = mark.split(MARK_SEP);
            let spec = parts.next().unwrap_or_default();
            let prefix: Vec<PathBuf> = parts.map(PathBuf::from).collect();
            // A program in the tree: run as it is, its `#!` line names the interpreter. A
            // virtual environment inside the tree is read, not run (a writable one is
            // story:confinement-seam's).
            if let Some(rel) = spec.strip_prefix("tree:") {
                let full = tree.join(rel);
                if python_like(rel) {
                    python_will_run = true;
                    venv.extend(venv_names(&full));
                    in_tree_pythons.push(rel.to_owned());
                } else if let Some(python) = shebang_python(&full, tree) {
                    python_will_run = true;
                    pythons.push(python);
                }
                continue;
            }
            // An absolute program outside the tree, or a bare name resolved on the effective
            // `PATH` (the prepends first); the tree's own is passed over.
            let path = if spec.contains('/') {
                Some(PathBuf::from(spec)).filter(|path| !in_tree(tree, path))
            } else {
                lookup_in(spec, &prefix, tree)
            };
            if python_like(spec) {
                python_will_run = true;
                match path {
                    Some(python) => pythons.push(python),
                    None => unresolved.push(spec.to_owned()),
                }
            } else if let Some(python) = path.and_then(|path| shebang_python(&path, tree)) {
                python_will_run = true;
                pythons.push(python);
            }
        }
        let mut seen = BTreeSet::new();
        pythons
            .retain(|python| seen.insert(std::fs::canonicalize(python).unwrap_or(python.clone())));
        let mut asked = Self {
            from: Vec::new(),
            strict: RUNNER_PACKAGES.iter().map(|name| fold(name)).collect(),
            names: venv,
            fail_closed: None,
            sites: BTreeSet::new(),
        };
        for python in &pythons {
            if let Some(answer) = Self::answer(python, tree) {
                asked.from.push(answer.from);
                asked.strict.extend(answer.strict);
                asked.names.extend(answer.names);
                asked.sites.extend(answer.sites);
            } else {
                unresolved.push(python.display().to_string());
            }
        }
        // Why a Python the test command runs was not asked: one of the tree is never run here,
        // and one that could not be found or did not answer is not known. Either fails closed.
        let mut reasons = Vec::new();
        if !in_tree_pythons.is_empty() {
            let verb = if in_tree_pythons.len() == 1 {
                "is"
            } else {
                "are"
            };
            reasons.push(format!(
                "{} {verb} inside the tree, and takt does not run a file of the tree at case open to ask what the test command's Python imports",
                in_tree_pythons.join(", ")
            ));
        }
        if asked.from.is_empty() {
            // No interpreter answered. A test command that runs a Python whose interpreter
            // could not be established fails closed; one that runs none keeps the built-in
            // lists, which guard nothing a Python here would not.
            let mut fallback = Self::default();
            fallback.names.extend(asked.names);
            if python_will_run {
                if reasons.is_empty() || !unresolved.is_empty() {
                    reasons.push(format!(
                        "the interpreter the test command runs could not be established at case open{}",
                        if unresolved.is_empty() {
                            String::new()
                        } else {
                            format!(": {}", unresolved.join(", "))
                        }
                    ));
                }
                fallback.fail_closed = Some(reasons.join("; "));
            }
            return fallback;
        }
        if !unresolved.is_empty() {
            reasons.push(format!(
                "a Python the test command runs could not be queried at case open: {}",
                unresolved.join(", ")
            ));
        }
        if !reasons.is_empty() {
            asked.fail_closed = Some(reasons.join("; "));
        }
        let strict = asked.strict.clone();
        asked
            .names
            .retain(|name| !strict.contains(name) && name != "__pycache__");
        asked
    }

    /// What `python` answers ([`NAMES_PY`]); `None` when it could not be run or did not answer
    /// in time. Its user site is the one under the caller's home, which it is given.
    fn answer(python: &Path, tree: &Path) -> Option<Answer> {
        let home = crate::validator::FreshHome::new().ok()?;
        let tree = std::path::absolute(tree).ok()?;
        let mut command = crate::confinement::Command::new(python);
        command
            .args(["-I", "-c", NAMES_PY])
            .arg(&tree)
            .arg(std::env::var_os("HOME").unwrap_or_default())
            .current_dir(home.dir());
        crate::forge::without_credentials(&mut command, |_| false);
        home.apply(&mut command);
        let limits = crate::forge::Limits {
            timeout: Duration::from_secs(10),
            output: 4 << 20,
        };
        let finished = crate::forge::run_child(&mut command, limits).ok()?;
        if !finished.success() || finished.stdout_cut {
            return None;
        }
        let text = String::from_utf8_lossy(&finished.stdout);
        let mut from = None;
        let mut strict = BTreeSet::new();
        let mut names = BTreeSet::new();
        let mut sites = BTreeSet::new();
        for line in text.lines() {
            match line.split_once(' ') {
                Some(("V", rest)) => {
                    let version = rest.rsplit(' ').next().unwrap_or(rest);
                    from = Some(format!("{} (Python {version})", python.display()));
                }
                Some(("S", name)) => {
                    strict.insert(fold(name));
                }
                Some(("N", name)) => {
                    names.insert(fold(name));
                }
                Some(("P", dir)) if Path::new(dir).is_absolute() => {
                    sites.insert(dir.to_owned());
                }
                _ => {}
            }
        }
        Some(Answer {
            from: from?,
            strict,
            names,
            sites,
        })
    }

    /// The names the case at `case_dir` keeps ([`MODULES_FILE`]); else asked of the Python the test
    /// command `runs` in `tree`, and kept there only when the answer is not the built-in lists
    /// ([`Modules::default`]): when a Python answered, or one could not be established (it fails
    /// closed). Otherwise no file is written, and [`Modules::read`] reads the built-in lists.
    pub fn of_case(case_dir: &Path, runs: &[String], tree: &Path) -> Result<Self> {
        let file = case_dir.join(MODULES_FILE);
        if file.is_file() {
            return Ok(Self::read(case_dir));
        }
        let modules = Self::ask(runs, tree);
        if modules != Self::default() {
            std::fs::write(&file, serde_json::to_vec_pretty(&modules)?)
                .with_context(|| format!("write {}", file.display()))?;
        }
        Ok(modules)
    }

    /// The names a resumed case guards: the ones it keeps ([`MODULES_FILE`]), else asked again
    /// and kept by no one, so the file exists only as the case opened with it and a replay of
    /// any step rebuilds the case as that step found it.
    #[must_use]
    pub fn at_resume(case_dir: &Path, runs: &[String], tree: &Path) -> Self {
        if case_dir.join(MODULES_FILE).is_file() {
            return Self::read(case_dir);
        }
        Self::ask(runs, tree)
    }

    /// The names the case at `case_dir` keeps ([`MODULES_FILE`]); the built-in lists when it
    /// keeps none it can read.
    #[must_use]
    pub fn read(case_dir: &Path) -> Self {
        std::fs::read(case_dir.join(MODULES_FILE))
            .ok()
            .and_then(|bytes| serde_json::from_slice(&bytes).ok())
            .unwrap_or_default()
    }

    /// Where the names came from, as a refusal says it.
    #[must_use]
    pub fn described(&self) -> String {
        self.from.join("; ")
    }
}

/// The build tools whose recipe the guard does not read: as the test command or a command of a
/// script it reads, each fails closed ([`fail_closed`]) beside guarding its build file.
const UNREAD_RECIPES: &[&str] = &["make", "gmake", "just", "task", "go-task", "tox", "nox"];

/// The directory marker for a working directory the guard could not follow (`cd "$x"`): a path
/// named from it is guarded in every directory of the tree ([`landing`]).
const ANYWHERE: &str = "**";

/// The glob for everything under `dir` of the tree, `""` its root.
fn under(dir: &str) -> String {
    if dir.is_empty() {
        "**".to_owned()
    } else {
        format!("{dir}/**")
    }
}

/// The shells: a script they run is read line by line ([`script_paths`]).
const SHELLS: &[&str] = &["sh", "bash", "dash", "zsh", "ksh", "mksh", "ash", "fish"];

/// The shell's words that open or close a compound command, which the guard does not follow: a
/// line starting with one is not read ([`statement`]).
const COMPOUND: &[&str] = &[
    "if", "then", "else", "elif", "fi", "for", "while", "until", "do", "done", "case", "esac",
    "select", "function", "coproc", "{", "}", "[[", "]]",
];

/// The shell's built-ins that run code given as text, or undo a directory change the guard
/// cannot replay: a line with one is not read ([`statement`]).
const OPAQUE: &[&str] = &["eval", "trap", "alias", "popd"];

/// The variables naming a directory Python loads code from beside `PYTHONPATH`: what each names
/// in the tree is guarded whole.
const PYTHON_DIRS: &[&str] = &[
    "PYTHONHOME",
    "PYTHONUSERBASE",
    "PYTHONPYCACHEPREFIX",
    "PYTHONSTARTUP",
];

/// How a program of a test command takes its options. A short option is a character of a
/// cluster (`-eo`); one that takes an argument takes the rest of its cluster or, when that is
/// empty, the next word. A long option (`--name`) takes `=value` or the next word.
#[derive(Clone, Copy)]
struct Spec {
    /// Options that take an argument and name nothing guarded.
    short_arg: &'static str,
    long_arg: &'static [&'static str],
    /// Options after which the rest is code, not a file (`python -c`, `python -m`).
    short_stop: &'static str,
    long_stop: &'static [&'static str],
    /// Options whose argument is a command line of its own (`sh -c`, `flock -c`).
    short_command: &'static str,
    long_command: &'static [&'static str],
    /// Options that change the directory the rest is named from (`make -C`, `env -C`).
    short_chdir: &'static str,
    long_chdir: &'static [&'static str],
    /// Options that name the file the program runs (`make -f`, `pwsh -File`).
    short_file: &'static str,
    long_file: &'static [&'static str],
    /// Whole words that name the file the program runs, or stop as `short_stop` does
    /// (`pwsh -File`, `pwsh -Command`), matched whatever their case.
    word_file: &'static [&'static str],
    word_stop: &'static [&'static str],
    /// Whether `+o` is an option as `-o` is (a shell's).
    plus: bool,
    /// Operands before the script or the command (`timeout DURATION`, `flock FILE`), or the
    /// subcommands skipped before the script (`deno run`).
    operands: usize,
    subcommands: &'static [&'static str],
    /// Options that name a module the program runs from the directory it runs in
    /// (`python -m MODULE`), which a file or a package there replaces.
    short_module: &'static str,
}

/// A program with no option of note.
const PLAIN: Spec = Spec {
    short_arg: "",
    long_arg: &[],
    short_stop: "",
    long_stop: &[],
    short_command: "",
    long_command: &[],
    short_chdir: "",
    long_chdir: &[],
    short_file: "",
    long_file: &[],
    word_file: &[],
    word_stop: &[],
    plus: false,
    operands: 0,
    subcommands: &[],
    short_module: "",
};

/// A shell: `-o OPT`, `-O OPT`, `+o OPT`, `--rcfile FILE`; `-c` runs a command line.
const SHELL: Spec = Spec {
    short_arg: "oO",
    long_arg: &["rcfile", "init-file"],
    short_command: "c",
    plus: true,
    ..PLAIN
};

/// The programs that run a script named after their options: the script is the test command's
/// own as much as `./check.sh` is.
const INTERPRETERS: &[(&[&str], Spec)] = &[
    (SHELLS, SHELL),
    (
        &["python", "pypy"],
        Spec {
            short_arg: "WX",
            long_arg: &["check-hash-based-pycs"],
            short_stop: "c",
            short_module: "m",
            ..PLAIN
        },
    ),
    (
        &["perl"],
        Spec {
            short_arg: "I",
            short_stop: "eE",
            ..PLAIN
        },
    ),
    (
        &["ruby"],
        Spec {
            short_arg: "IrEF",
            long_arg: &["encoding"],
            short_stop: "e",
            short_chdir: "C",
            ..PLAIN
        },
    ),
    (
        &["node", "nodejs"],
        Spec {
            short_arg: "rC",
            long_arg: &[
                "require",
                "import",
                "loader",
                "experimental-loader",
                "conditions",
                "env-file",
                "input-type",
                "title",
            ],
            short_stop: "ep",
            long_stop: &["eval", "print"],
            ..PLAIN
        },
    ),
    (
        &["deno"],
        Spec {
            short_arg: "cL",
            long_arg: &[
                "config",
                "import-map",
                "cert",
                "location",
                "seed",
                "log-level",
            ],
            subcommands: &["run", "test"],
            ..PLAIN
        },
    ),
    (
        &["bun"],
        Spec {
            short_arg: "r",
            long_arg: &["preload", "config"],
            long_chdir: &["cwd"],
            short_stop: "e",
            long_stop: &["eval", "print"],
            subcommands: &["run", "test"],
            ..PLAIN
        },
    ),
    (
        &["php"],
        Spec {
            short_arg: "cdz",
            short_stop: "rBRE",
            ..PLAIN
        },
    ),
    (
        &["rscript"],
        Spec {
            short_stop: "e",
            ..PLAIN
        },
    ),
    (
        &["lua"],
        Spec {
            short_arg: "l",
            short_stop: "e",
            ..PLAIN
        },
    ),
    (&["tclsh"], PLAIN),
    (
        &["pwsh", "powershell"],
        Spec {
            word_file: &["-file", "-f"],
            word_stop: &["-command", "-c", "-encodedcommand", "-e", "-ec"],
            ..PLAIN
        },
    ),
];

/// The programs that run the command after their options and operands.
const WRAPPERS: &[(&[&str], Spec)] = &[
    (
        &["timeout"],
        Spec {
            short_arg: "sk",
            long_arg: &["signal", "kill-after"],
            operands: 1,
            ..PLAIN
        },
    ),
    (
        &["nice"],
        Spec {
            short_arg: "n",
            long_arg: &["adjustment"],
            ..PLAIN
        },
    ),
    (
        &["ionice"],
        Spec {
            short_arg: "cnpPu",
            long_arg: &["class", "classdata", "pid", "pgid", "uid"],
            ..PLAIN
        },
    ),
    (
        &["stdbuf"],
        Spec {
            short_arg: "ioe",
            long_arg: &["input", "output", "error"],
            ..PLAIN
        },
    ),
    (&["nohup", "chronic", "setsid", "busybox"], PLAIN),
    (
        &["time"],
        Spec {
            short_arg: "fo",
            long_arg: &["format", "output"],
            ..PLAIN
        },
    ),
    (
        &["xvfb-run"],
        Spec {
            short_arg: "nsfepw",
            long_arg: &[
                "server-num",
                "server-args",
                "auth-file",
                "error-file",
                "xauth-protocol",
                "wait",
            ],
            ..PLAIN
        },
    ),
    (
        &["flock"],
        Spec {
            short_arg: "wE",
            long_arg: &["timeout", "wait", "conflict-exit-code"],
            short_command: "c",
            long_command: &["command"],
            operands: 1,
            ..PLAIN
        },
    ),
    // `taskset MASK CMD`, `chrt [-f|-r|…] PRIORITY CMD`: one operand before the command.
    (
        &["taskset"],
        Spec {
            operands: 1,
            ..PLAIN
        },
    ),
    (
        &["chrt"],
        Spec {
            short_arg: "TPD",
            long_arg: &["sched-runtime", "sched-period", "sched-deadline"],
            operands: 1,
            ..PLAIN
        },
    ),
    (&["unbuffer"], PLAIN),
    // `script -qec CMD FILE` runs the command string through a shell.
    (
        &["script"],
        Spec {
            short_arg: "TIOBEmo",
            long_arg: &[
                "log-timing",
                "log-in",
                "log-out",
                "log-io",
                "echo",
                "logging-format",
                "output-limit",
            ],
            short_command: "c",
            long_command: &["command"],
            ..PLAIN
        },
    ),
    // `xargs [options] CMD` runs the command with the words of its standard input appended,
    // which the guard does not read: the line fails closed besides ([`command_paths`]).
    (
        &["xargs"],
        Spec {
            short_arg: "adEILnPs",
            long_arg: &[
                "arg-file",
                "delimiter",
                "eof",
                "replace",
                "max-lines",
                "max-args",
                "max-procs",
                "max-chars",
                "process-slot-var",
            ],
            ..PLAIN
        },
    ),
    // `uv run [options] CMD`, or a Python script or `-m MODULE` run by the project's Python
    // ([`command_paths`]).
    (
        &["uv"],
        Spec {
            short_arg: "pw",
            long_arg: &[
                "python",
                "with",
                "with-editable",
                "with-requirements",
                "group",
                "only-group",
                "no-group",
                "extra",
                "package",
                "env-file",
                "project",
                "index",
                "default-index",
                "find-links",
                "cache-dir",
                "config-file",
                "exclude-newer",
                "python-preference",
                "link-mode",
                "resolution",
                "prerelease",
            ],
            long_chdir: &["directory"],
            subcommands: &["run"],
            ..PLAIN
        },
    ),
];

/// Whether `word` names a program the reader knows how to read: a shell or interpreter
/// ([`INTERPRETERS`]), a wrapper ([`WRAPPERS`]), a build tool ([`BUILD_TOOLS`]), `env` or
/// `pytest`. A program the reader does not know that takes one as an argument may run it, which
/// the reader cannot see: a file in the tree, compared whole at the base, is read as running
/// that program with the words after it, and fails the line closed besides when a word stands
/// before that program or the file sets what it runs with ([`hand_off_unread`]); any other
/// fails the line closed ([`command_paths`]).
fn known_program(word: &str) -> bool {
    let name = program_of(word);
    name == "env"
        || name == "pytest"
        || name == "py.test"
        || spec_of(INTERPRETERS, &name).is_some()
        || spec_of(WRAPPERS, &name).is_some()
        || BUILD_TOOLS
            .iter()
            .any(|(names, _, _, _)| names.contains(&name.as_str()))
}

/// Why the line fails closed when the program of the tree at `path` is given the program `given`
/// and hands the command off to it ([`command_paths`]): what the file sets before it runs that
/// program is not read. A Python file is read for the forms [`python_hand_off_form`] names; a
/// script a shell reads was read line by line ([`script_paths`]), and an expansion of the words
/// it is given (`"$@"`) failed closed there; any other program is not read. `None` when the file
/// is read and sets nothing the reader does not follow.
fn hand_off_unread(tree: &Path, path: &str, given: &str) -> Option<String> {
    use std::io::Read as _;
    let full = tree.join(path);
    let python = shebang_of(&full).is_some_and(|interpreter| {
        spec_of(INTERPRETERS, &interpreter).is_some_and(|spec| !spec.short_module.is_empty())
    });
    if !python {
        if shell_text(&full, false, &mut Vec::new()).is_some() {
            return None;
        }
        return Some(format!(
            "`{path}` runs {}, and the guard does not read what `{path}` sets before it does",
            quoted(given)
        ));
    }
    let mut bytes = Vec::new();
    let read = std::fs::File::open(&full)
        .and_then(|file| file.take(SCRIPT_BYTES_AT_MOST + 1).read_to_end(&mut bytes));
    if read.is_err() || bytes.len() as u64 > SCRIPT_BYTES_AT_MOST {
        return Some(format!(
            "`{path}` runs {}, and cannot be read or is larger than {SCRIPT_BYTES_AT_MOST} bytes",
            quoted(given)
        ));
    }
    python_hand_off_form(&String::from_utf8_lossy(&bytes)).map(|form| {
        format!(
            "`{path}` sets {} before it runs {}, which the guard does not follow",
            quoted(&form),
            quoted(given)
        )
    })
}

/// The first form in the Python `text` of a program of the tree that hands off the command it
/// is given (`os.execvp(sys.argv[1], sys.argv[1:])`) by which it sets what that command runs
/// with, as the text writes it: the environment (a write to `os.environ` or `os.environb`, or a
/// `putenv(` or `setenv(`, of a variable whose name is not a string literal or is one Python
/// reads, `PYTHON…`; any other call on the mapping than a read, such as `.update(`; a whole
/// environment given, `env=`, `os.execve(` and the other `e` calls of `exec*` and `spawn*`,
/// `posix_spawn(`; any `PYTHON…` name at all), the directory (`chdir(`, `cwd=`), the import path
/// (`sys.path`, `sys.meta_path`, `addsitedir(`), and a name that hides one (`from sys import
/// path`, `from os import … as`, `import sys as`, `getattr(`, `__import__(`, `exec(`). The text
/// is read whole, white space between words kept and the rest removed, comments and strings
/// among it: a form a comment names counts as well. A write of another variable, `PATH` among
/// them, is none: the shell reader follows only `PYTHONPATH`, `PATH` and [`PYTHON_DIRS`] in an
/// assignment ([`assign`]), and `PATH` decides which program runs, not what it imports.
fn python_hand_off_form(text: &str) -> Option<String> {
    let ident = |c: char| c.is_alphanumeric() || c == '_';
    let text = text.replace("\\\n", " ");
    let mut compact = String::new();
    let mut spaced = false;
    for c in text.chars() {
        if c.is_whitespace() {
            spaced = true;
            continue;
        }
        if spaced && compact.ends_with(ident) && ident(c) {
            compact.push(' ');
        }
        spaced = false;
        compact.push(c);
    }
    // The dotted name before `at` as written: `os.` of `os.environ`.
    let prefix = |at: usize| {
        let start = compact[..at]
            .char_indices()
            .rev()
            .take_while(|(_, c)| ident(*c) || *c == '.')
            .last()
            .map_or(at, |(k, _)| k);
        compact[start..at].to_owned()
    };
    let literal = |key: &str| {
        let key = key.strip_prefix('b').unwrap_or(key);
        ['\'', '"']
            .into_iter()
            .find_map(|quote| key.strip_prefix(quote)?.strip_suffix(quote))
            .filter(|name| name.chars().all(ident))
            .map(str::to_owned)
    };
    let unfollowed = |key: &str| literal(key).is_none_or(|name| name.starts_with("PYTHON"));
    let assigns = |after: &str| {
        (after.starts_with('=') && !after.starts_with("=="))
            || [
                "+=", "-=", "*=", "/=", "%=", "&=", "|=", "^=", "@=", "//=", "**=", "<<=", ">>=",
            ]
            .iter()
            .any(|op| after.starts_with(op))
    };
    // The environment.
    for (at, _) in compact.match_indices("environ") {
        if compact[..at].ends_with(ident) {
            continue;
        }
        let (name, rest) = match compact[at + "environ".len()..].strip_prefix('b') {
            Some(rest) => ("environb", rest),
            None => ("environ", &compact[at + "environ".len()..]),
        };
        if rest.starts_with(ident) {
            continue;
        }
        let shown = format!("{}{name}", prefix(at));
        if let Some(inside) = rest.strip_prefix('[') {
            let (key, after) = inside.split_once(']').unwrap_or((inside, ""));
            if assigns(after) && unfollowed(key) {
                return Some(format!("{shown}[{key}] ="));
            }
        } else if let Some(call) = rest.strip_prefix('.') {
            let method: String = call.chars().take_while(|c| ident(*c)).collect();
            if ![
                "get",
                "copy",
                "items",
                "keys",
                "values",
                "__getitem__",
                "__contains__",
            ]
            .contains(&method.as_str())
            {
                return Some(format!("{shown}.{method}("));
            }
        } else if assigns(rest) {
            return Some(format!("{shown} ="));
        }
    }
    for call in ["putenv(", "setenv("] {
        for (at, _) in compact.match_indices(call) {
            let key = compact[at + call.len()..]
                .split([',', ')'])
                .next()
                .unwrap_or_default();
            if unfollowed(key) {
                return Some(format!("{}{call}", prefix(at)));
            }
        }
    }
    // A whole environment given to the program it runs.
    for call in [
        "execve(",
        "execle(",
        "execlpe(",
        "execvpe(",
        "spawnve(",
        "spawnle(",
        "spawnlpe(",
        "spawnvpe(",
        "posix_spawn(",
        "posix_spawnp(",
    ] {
        if let Some(at) = compact.find(call) {
            return Some(format!("{}{call}", prefix(at)));
        }
    }
    // A keyword of `subprocess`: its environment, the directory it runs in.
    for keyword in ["env=", "cwd="] {
        for (at, _) in compact.match_indices(keyword) {
            if compact[..at].ends_with(['(', ','])
                && !compact[at + keyword.len()..].starts_with('=')
            {
                return Some(keyword.to_owned());
            }
        }
    }
    // The directory, the import path, and a name that hides one of them.
    for form in [
        "chdir(",
        "sys.path",
        "sys.meta_path",
        "addsitedir(",
        "getattr(",
        "setattr(",
        "__dict__",
        "vars(",
        "__import__(",
        "importlib",
        "exec(",
        "eval(",
    ] {
        if let Some(at) = compact.find(form) {
            return Some(format!("{}{form}", prefix(at)));
        }
    }
    for (at, _) in text.match_indices("import") {
        let line = text[..at].rsplit('\n').next().unwrap_or_default();
        let words: Vec<&str> = line.split_whitespace().collect();
        let names = &text[at + "import".len()..];
        let names = match names.trim_start().strip_prefix('(') {
            Some(inside) => inside.split(')').next().unwrap_or_default(),
            None => names.split(['\n', ';']).next().unwrap_or_default(),
        };
        let names: Vec<&str> = names
            .split(|c: char| c == ',' || c.is_whitespace())
            .filter(|name| !name.is_empty())
            .collect();
        match words.as_slice() {
            ["from", module] if ["os", "sys", "posix", "nt", "site"].contains(module) => {
                if names.iter().any(|name| {
                    ["*", "as"].contains(name)
                        || (*module == "sys" && ["path", "meta_path", "path_hooks"].contains(name))
                }) {
                    return Some(format!("from {module} import"));
                }
            }
            [] if names.windows(2).any(|pair| pair == ["sys", "as"]) => {
                return Some("import sys as".to_owned());
            }
            _ => {}
        }
    }
    // A name of the variables Python reads, wherever it stands: `env={"PYTHONPATH": …}`.
    for (at, _) in compact.match_indices("PYTHON") {
        if compact[at + "PYTHON".len()..].starts_with(|c: char| c.is_ascii_uppercase() || c == '_')
        {
            let name: String = compact[at..].chars().take_while(|c| ident(*c)).collect();
            return Some(name);
        }
    }
    None
}

/// The top-level names the Python `text` imports: `import a.b as c, d` names `a` and `d`,
/// `from a.b import c` names `a`; a relative import (`from . import x`) names nothing, and so
/// does a line the reader cannot take as one statement (a parenthesised or continued list is
/// read as far as its first line). A name is found in the directories Python imports from as
/// `-m` finds its module ([`PYTHON_MARK`]).
pub(crate) fn imports_of(text: &str) -> BTreeSet<String> {
    let identifier = |name: &str| {
        !name.is_empty()
            && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
            && !name.starts_with(|c: char| c.is_ascii_digit())
    };
    let mut names = BTreeSet::new();
    let statements = text.lines().flat_map(|line| {
        let line = line.split_once('#').map_or(line, |(code, _)| code);
        line.split(';')
    });
    for line in statements {
        let line = line.trim();
        if let Some(rest) = line.strip_prefix("import ") {
            for item in rest.split(',') {
                let item = item.trim();
                let name = item.split_whitespace().next().unwrap_or("");
                let name = name.split('.').next().unwrap_or("");
                if identifier(name) {
                    names.insert(name.to_owned());
                }
            }
        } else if let Some(rest) = line.strip_prefix("from ") {
            let module = rest.split_whitespace().next().unwrap_or("");
            let name = module.split('.').next().unwrap_or("");
            if identifier(name) {
                names.insert(name.to_owned());
            }
        }
    }
    names
}

/// The forms in the Python `text` by which it imports what no import line names, which
/// [`imports_of`] cannot follow: `importlib` in any use (`import importlib`, `from importlib`,
/// `importlib.import_module(`, named as written), `__import__(`, and a call of `exec(` or
/// `eval(`. Read as [`imports_of`] reads, line by line with a comment stripped; each form once,
/// in the order found. A file holding one fails closed in the gate's comparison.
pub(crate) fn dynamic_imports_of(text: &str) -> Vec<String> {
    let is_identifier = |c: char| c.is_ascii_alphanumeric() || c == '_';
    let mut forms: Vec<String> = Vec::new();
    let statements = text.lines().flat_map(|line| {
        let line = line.split_once('#').map_or(line, |(code, _)| code);
        line.split(';')
    });
    for statement in statements {
        let statement = statement.trim();
        let chars: Vec<(usize, char)> = statement.char_indices().collect();
        let end_of = |k: usize| chars.get(k).map_or(statement.len(), |(at, _)| *at);
        let mut i = 0;
        while i < chars.len() {
            if !is_identifier(chars[i].1) {
                i += 1;
                continue;
            }
            let begin = chars[i].0;
            let mut j = i;
            while j < chars.len() && is_identifier(chars[j].1) {
                j += 1;
            }
            let name = &statement[begin..end_of(j)];
            let form = match name {
                "importlib" if statement.starts_with("import ") => {
                    Some("import importlib".to_owned())
                }
                "importlib"
                    if statement
                        .strip_prefix("from ")
                        .and_then(|rest| rest.split_whitespace().next())
                        .is_some_and(|module| module.split('.').next() == Some("importlib")) =>
                {
                    Some("from importlib".to_owned())
                }
                "importlib" => {
                    // The attribute chain after it, and the call's parenthesis when one follows.
                    let mut k = j;
                    while chars.get(k).is_some_and(|(_, c)| *c == '.')
                        && chars.get(k + 1).is_some_and(|(_, c)| is_identifier(*c))
                    {
                        k += 1;
                        while k < chars.len() && is_identifier(chars[k].1) {
                            k += 1;
                        }
                    }
                    let chain = &statement[begin..end_of(k)];
                    let called = statement[end_of(k)..].trim_start().starts_with('(');
                    Some(if called {
                        format!("{chain}(")
                    } else {
                        chain.to_owned()
                    })
                }
                "__import__" | "exec" | "eval"
                    if statement[end_of(j)..].trim_start().starts_with('(') =>
                {
                    Some(format!("{name}("))
                }
                _ => None,
            };
            if let Some(form) = form.filter(|form| !forms.contains(form)) {
                forms.push(form);
            }
            i = j;
        }
    }
    forms
}

/// Names, behind [`PYTHON_MARK`] alone (the gate's to compare, not the step guard's), what the
/// top-level imports of the Python `code` load from each of `imports`, the directories Python
/// imports from: `name.py`, a compiled `name.*` and the package `name/`, as `-m name` is named.
fn python_imports_named(tree: &Path, code: &str, imports: &[String], named: &mut Vec<String>) {
    for name in imports_of(code) {
        for import in imports {
            let import = Some(import.as_str());
            let loads = [
                named_from(tree, import, &format!("{name}.py")),
                named_from(tree, import, &format!("{name}.*")),
                named_from(tree, import, &name).map(|package| format!("{package}/**")),
            ];
            for path in loads.into_iter().flatten() {
                named.push(format!("{PYTHON_MARK}{path}"));
            }
        }
    }
}

/// The build tools whose own file decides what their target runs: the files they read in the
/// directory they run in, unless an option names another; and the configuration they read
/// there beside it, which decides how they run it (`.npmrc`'s `script-shell`).
type BuildTool = (
    &'static [&'static str],
    Spec,
    &'static [&'static str],
    &'static [&'static str],
);

/// The build tools ([`BuildTool`]).
const BUILD_TOOLS: &[BuildTool] = &[
    (
        &["make", "gmake"],
        Spec {
            short_arg: "IoWE",
            long_arg: &[
                "include-dir",
                "old-file",
                "assume-old",
                "what-if",
                "new-file",
                "assume-new",
                "eval",
            ],
            short_chdir: "C",
            long_chdir: &["directory"],
            short_file: "f",
            long_file: &["file", "makefile"],
            ..PLAIN
        },
        &["Makefile", "makefile", "GNUmakefile"],
        &[],
    ),
    (
        &["task", "go-task"],
        Spec {
            short_chdir: "d",
            long_chdir: &["dir"],
            short_file: "t",
            long_file: &["taskfile"],
            ..PLAIN
        },
        &[
            "Taskfile.yml",
            "Taskfile.yaml",
            "Taskfile.dist.yml",
            "Taskfile.dist.yaml",
        ],
        &[],
    ),
    (
        &["npm", "yarn", "pnpm"],
        Spec {
            short_chdir: "C",
            long_chdir: &["prefix", "dir", "cwd"],
            ..PLAIN
        },
        &["package.json"],
        &[".npmrc", ".yarnrc", ".yarnrc.yml", ".pnpmfile.cjs"],
    ),
    (
        &["just"],
        Spec {
            short_chdir: "d",
            long_chdir: &["working-directory"],
            short_file: "f",
            long_file: &["justfile"],
            ..PLAIN
        },
        &["justfile", ".justfile"],
        &[],
    ),
    (
        &["tox"],
        Spec {
            short_arg: "e",
            long_arg: &["env", "workdir", "installpkg"],
            long_chdir: &["root"],
            short_file: "c",
            long_file: &["conf"],
            ..PLAIN
        },
        &["tox.ini", "tox.toml"],
        &[],
    ),
    (
        &["nox"],
        Spec {
            short_arg: "spkte",
            long_arg: &[
                "session", "sessions", "python", "keywords", "tags", "envdir",
            ],
            short_file: "f",
            long_file: &["noxfile"],
            ..PLAIN
        },
        &["noxfile.py"],
        &[],
    ),
];

/// How deep a command line inside a command line (`sh -c`, `env -S`) is read.
const COMMANDS_AT_MOST: usize = 16;

/// `path` with its case folded as a file system that ignores case folds it: lower case, and the
/// two letters outside ASCII that fold into the ASCII letters of the guarded names.
fn fold(path: &str) -> String {
    path.chars()
        .flat_map(char::to_lowercase)
        .map(|c| match c {
            '\u{17f}' => 's',
            '\u{212a}' => 'k',
            c => c,
        })
        .collect()
}

/// `path` relative to the tree with `.` and `..` resolved and empty parts dropped, as git names
/// a path; `None` when it leads out of the tree, `""` for the tree itself.
fn normalise(path: &str) -> Option<String> {
    let mut parts: Vec<&str> = Vec::new();
    for part in path.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                parts.pop()?;
            }
            part => parts.push(part),
        }
    }
    Some(parts.join("/"))
}

/// Where `word`, named from the directory `dir` of the tree (relative to it), lands: resolved
/// as a path from the tree's absolute path, `.` and `..` lexically, so a path that climbs above
/// the tree and comes back (`../repo/check.sh`) lands inside it; relative to the tree, `""` for
/// the tree itself, `None` outside it.
fn landing(tree: &Path, dir: &str, word: &str) -> Option<String> {
    // From a directory the guard could not follow ([`ANYWHERE`]), a relative path lands in any
    // directory of the tree: a glob, which `..` never climbs out of.
    if (dir == ANYWHERE || dir.starts_with("**/")) && !Path::new(word).is_absolute() {
        let mut parts: Vec<&str> = Vec::new();
        for part in dir.split('/').chain(word.split('/')) {
            match part {
                "" | "." => {}
                ".." => {
                    if parts.last() != Some(&ANYWHERE) {
                        parts.pop();
                    }
                }
                part => parts.push(part),
            }
        }
        return Some(parts.join("/"));
    }
    let absolute = std::path::absolute(tree).ok()?;
    let joined = if Path::new(word).is_absolute() {
        PathBuf::from(word)
    } else {
        absolute.join(dir).join(word)
    };
    let mut parts: Vec<std::path::Component> = Vec::new();
    for component in joined.components() {
        match component {
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                if matches!(parts.last(), Some(std::path::Component::Normal(_))) {
                    parts.pop();
                }
            }
            component => parts.push(component),
        }
    }
    let resolved: PathBuf = parts.iter().collect();
    [Some(absolute), std::fs::canonicalize(tree).ok()]
        .iter()
        .flatten()
        .find_map(|root| resolved.strip_prefix(root).ok())
        .and_then(|inside| inside.to_str())
        .and_then(normalise)
}

/// The program a word names, without its directory and its version: `python3.12` is `python`.
fn program_of(word: &str) -> String {
    let name = word.rsplit('/').next().unwrap_or(word);
    fold(name.trim_end_matches(|c: char| c.is_ascii_digit() || c == '.'))
}

/// `word` named from `dir` (relative to the tree; `None` once a directory change left it), as a
/// path in the tree; `None` outside it.
fn named_from(tree: &Path, dir: Option<&str>, word: &str) -> Option<String> {
    if word.is_empty() {
        return None;
    }
    let dir = if Path::new(word).is_absolute() {
        ""
    } else {
        dir?
    };
    landing(tree, dir, word).filter(|path| !path.is_empty())
}

/// The directory `dir` changes to by `to`: `None` outside the tree.
fn changed_dir(tree: &Path, dir: Option<&str>, to: &str) -> Option<String> {
    let dir = if Path::new(to).is_absolute() {
        ""
    } else {
        dir?
    };
    landing(tree, dir, to)
}

/// What starts a redirection among the words [`shell_words`] splits a line into: a character no
/// word of a command line holds, so a quoted `"2>"` stays a word.
const REDIRECT: char = '\u{0}';

/// The redirection `word` is ([`shell_words`]): the descriptor written before its operator (`""`
/// for none) and the operator; `None` for any other word.
fn redirection(word: &str) -> Option<(&str, &str)> {
    let rest = word.strip_prefix(REDIRECT)?;
    Some(rest.split_at(rest.find(['<', '>', '&']).unwrap_or(rest.len())))
}

/// `words` as a shell line shows them: each redirection's operator without its [`REDIRECT`].
fn shown(words: &[impl AsRef<str>]) -> String {
    words
        .iter()
        .map(|word| word.as_ref().trim_start_matches(REDIRECT))
        .collect::<Vec<_>>()
        .join(" ")
}

/// `line` split into words as a POSIX shell splits a command line: white space between words,
/// `'…'` taken as it is, `"…"` with `\` escaping `"`, `\`, `$` and a backquote, `\` outside quotes
/// escaping the next character, and each of `;`, `&`, `|`, `(`, `)` a word by itself, so the
/// commands of a list are told apart. A redirection is one word, behind [`REDIRECT`]: its
/// operator (`<`, `>`, `>>`, `<>`, `>|`, `<&`, `>&`, `<<`, `<<-`, `<<<`, `&>`, `&>>`) with the
/// descriptor an unquoted run of digits (or bash's `{NAME}`) right before it names, so neither
/// `2>&1` is split at `&` nor its `2` left as a word; one followed at once by another `<` or `>`
/// is kept whole, a form [`redirection`] does not know. Its target is the next word. Expansions
/// are not made; an unclosed quote ends at the end of the line.
fn shell_words(line: &str) -> Vec<String> {
    let mut words = Vec::new();
    let mut word = String::new();
    let mut started = false;
    // Whether the word so far is unquoted characters only, so it may be a descriptor.
    let mut bare = true;
    // A shell drops a null byte of its input, and no other word starts with [`REDIRECT`].
    let mut chars = line.chars().filter(|c| *c != REDIRECT).peekable();
    while let Some(c) = chars.next() {
        match c {
            '<' | '>' => {
                let descriptor = started
                    && bare
                    && (word.chars().all(|c| c.is_ascii_digit())
                        || word
                            .strip_prefix('{')
                            .and_then(|name| name.strip_suffix('}'))
                            .is_some_and(|name| {
                                name.starts_with(|c: char| c.is_ascii_alphabetic() || c == '_')
                                    && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
                            }));
                let mut op = String::from(REDIRECT);
                if descriptor {
                    op.push_str(&std::mem::take(&mut word));
                } else if started {
                    words.push(std::mem::take(&mut word));
                }
                started = false;
                bare = true;
                op.push(c);
                match (c, chars.peek()) {
                    ('<', Some('<')) => {
                        op.extend(chars.next());
                        if matches!(chars.peek(), Some('<' | '-')) {
                            op.extend(chars.next());
                        }
                    }
                    ('<', Some('>' | '&')) | ('>', Some('>' | '|' | '&')) => {
                        op.extend(chars.next());
                    }
                    _ => {}
                }
                while let Some(&more) = chars.peek().filter(|c| matches!(c, '<' | '>')) {
                    op.push(more);
                    chars.next();
                }
                words.push(op);
            }
            '&' if chars.peek() == Some(&'>') => {
                if started {
                    words.push(std::mem::take(&mut word));
                    started = false;
                    bare = true;
                }
                let mut op = String::from(REDIRECT);
                op.push('&');
                op.extend(chars.next());
                while let Some(&more) = chars.peek().filter(|c| matches!(c, '<' | '>')) {
                    op.push(more);
                    chars.next();
                }
                words.push(op);
            }
            '\'' => {
                started = true;
                bare = false;
                for c in chars.by_ref() {
                    if c == '\'' {
                        break;
                    }
                    word.push(c);
                }
            }
            '"' => {
                started = true;
                bare = false;
                while let Some(c) = chars.next() {
                    match c {
                        '"' => break,
                        '\\' if matches!(chars.peek(), Some('"' | '\\' | '$' | '`')) => {
                            word.extend(chars.next());
                        }
                        c => word.push(c),
                    }
                }
            }
            '\\' => {
                started = true;
                bare = false;
                word.extend(chars.next());
            }
            ';' | '&' | '|' | '(' | ')' => {
                if started {
                    words.push(std::mem::take(&mut word));
                    started = false;
                    bare = true;
                }
                words.push(c.to_string());
            }
            c if c.is_whitespace() => {
                if started {
                    words.push(std::mem::take(&mut word));
                    started = false;
                    bare = true;
                }
            }
            c => {
                started = true;
                word.push(c);
            }
        }
    }
    if started {
        words.push(word);
    }
    words
}

/// What the shell that runs a command has set that decides what the command loads, and where the
/// guard is among the scripts it reads.
#[derive(Debug, Clone, Default)]
struct Env {
    /// The `PYTHONPATH` entries: each as written, named from where Python runs, or as an
    /// absolute path once a variable in it was expanded.
    pythonpath: Vec<String>,
    /// The shell variables whose value the guard knows, expanded.
    vars: BTreeMap<String, String>,
    /// The absolute directory of the script being read: what `$(dirname "$0")` stands for.
    script_dir: Option<String>,
    /// The absolute directory of the file being read, a sourced one or the script itself: what
    /// `$(dirname "${BASH_SOURCE[0]}")` stands for.
    source_dir: Option<String>,
    /// The directories the script has prepended to `PATH`, absolute, in the order a lookup by
    /// name would try them, ahead of the `PATH` the test command inherits: where the guard
    /// resolves the interpreter a bare `python` names ([`Modules::ask`]), and, for the outside
    /// ones, directories a step may not write to ([`OUTSIDE_PATH_MARK`]).
    path_prefix: Vec<String>,
    /// How many scripts deep the guard is ([`SCRIPTS_AT_MOST`]).
    level: usize,
    /// What the command reads on its standard input, when a redirection or a pipe of the line
    /// set it: what a shell given no script reads as its script ([`stdin_script`]). `None` is
    /// the test command's own.
    stdin: Option<Stdin>,
}

/// What a command reads on its standard input ([`Env::stdin`]).
#[derive(Debug, Clone)]
enum Stdin {
    /// A file of the tree, relative to it: `< FILE`, or `cat FILE |`.
    File(String),
    /// Nothing: `< /dev/null`.
    Empty,
    /// What the guard cannot name, and why.
    Unknown(String),
}

/// What `word`, the file a redirection or `cat` feeds a command, read from `dir`, gives it on
/// its standard input ([`Stdin`]).
fn stdin_of(tree: &Path, word: &str, dir: Option<&str>, env: &Env) -> Stdin {
    let Some(file) = expand(tree, word, dir, env).filter(|file| !globbed(file)) else {
        return Stdin::Unknown(format!(
            "{} holds what the guard cannot expand",
            quoted(word)
        ));
    };
    if file == "/dev/null" {
        return Stdin::Empty;
    }
    match named_from(tree, dir, &file) {
        Some(path) => Stdin::File(path),
        None => Stdin::Unknown(format!("{}, which is not a file of the tree", quoted(word))),
    }
}

/// What the command `producer` of a pipeline writes to the next one's standard input, as far as
/// the guard reads it: `cat FILE` feeds it `FILE` (`Ok`), as written; anything else is output the
/// guard cannot name (`Err`, the command).
fn piped_from(producer: &[String]) -> Result<String, String> {
    match producer {
        [cat, file] | [cat, _, file]
            if cat == "cat"
                && !file.starts_with('-')
                && redirection(file).is_none()
                && (producer.len() == 2 || producer[1] == "--") =>
        {
            Ok(file.clone())
        }
        _ => Err(shown(producer)),
    }
}

/// A shell given no script, or `-s`, reads its script from its standard input ([`Env::stdin`]):
/// a file of the tree is read as the script a shell runs ([`script_paths`]), nothing is nothing,
/// and anything else fails closed. The test command's own input is not the line's to name.
fn stdin_script(tree: &Path, dir: Option<&str>, env: &Env, named: &mut Vec<String>, depth: usize) {
    match &env.stdin {
        None | Some(Stdin::Empty) => {}
        Some(Stdin::File(path)) => {
            named.push(path.clone());
            let env = Env {
                stdin: None,
                ..env.clone()
            };
            script_paths(tree, path, dir, &env, named, depth, true);
        }
        Some(Stdin::Unknown(why)) => fail_closed(
            named,
            format!(
                "a shell reads its script from its standard input, {why}, which the guard cannot read"
            ),
        ),
    }
}

/// A shell reading a script: the directories it may be in (a `cd` that fails leaves it where it
/// was, so every directory it may have changed to is kept beside the ones before; `None` outside
/// the tree) and what it has set.
struct Shell {
    dirs: Vec<Option<String>>,
    env: Env,
}

/// The size past which a script is not read: one that large fails closed ([`fail_closed`]).
const SCRIPT_BYTES_AT_MOST: u64 = 1 << 20;

/// The command substitutions the guard reads, each with what it stands for: the script's own
/// directory (`\u{1}`), the directory the shell is in (`\u{2}`), or the directory of the file
/// being read, a sourced one or the script (`\u{3}`, `BASH_SOURCE`). A line with any other
/// substitution is not read.
const SUBSTITUTIONS: &[(&str, &str)] = &[
    ("$(dirname \"$(realpath \"$0\")\")", "\u{1}"),
    ("$(dirname \"$(realpath -- \"$0\")\")", "\u{1}"),
    ("$(dirname \"$(readlink -f \"$0\")\")", "\u{1}"),
    ("$(dirname -- \"$0\")", "\u{1}"),
    ("$(dirname \"$0\")", "\u{1}"),
    ("$(dirname $0)", "\u{1}"),
    ("`dirname \"$0\"`", "\u{1}"),
    ("`dirname $0`", "\u{1}"),
    ("$(dirname \"$(realpath \"${BASH_SOURCE[0]}\")\")", "\u{3}"),
    (
        "$(dirname \"$(readlink -f \"${BASH_SOURCE[0]}\")\")",
        "\u{3}",
    ),
    ("$(dirname -- \"${BASH_SOURCE[0]}\")", "\u{3}"),
    ("$(dirname \"${BASH_SOURCE[0]}\")", "\u{3}"),
    ("$(dirname \"${BASH_SOURCE}\")", "\u{3}"),
    ("$(dirname \"$BASH_SOURCE\")", "\u{3}"),
    ("${BASH_SOURCE[0]%/*}", "\u{3}"),
    ("${BASH_SOURCE%/*}", "\u{3}"),
    ("${0%/*}", "\u{1}"),
    ("$(pwd -P)", "\u{2}"),
    ("$(pwd)", "\u{2}"),
    ("`pwd`", "\u{2}"),
];

/// `line` with the substitutions the guard reads ([`SUBSTITUTIONS`]) put in their place, and
/// `$(cd DIR && pwd)` as `DIR`.
fn substituted(line: &str) -> String {
    let mut line = line.to_owned();
    for (form, by) in SUBSTITUTIONS {
        line = line.replace(form, by);
    }
    while let Some(start) = line.find("$(cd ") {
        let rest = &line[start + "$(cd ".len()..];
        let Some(end) = rest.find(')') else {
            break;
        };
        let inner = rest[..end].trim_end();
        let Some(dir) = ["&& pwd -P", "&& pwd", "; pwd -P", "; pwd"]
            .iter()
            .find_map(|tail| inner.strip_suffix(tail))
            .map(str::trim)
            .filter(|dir| !dir.contains("$(") && !dir.contains('`'))
        else {
            break;
        };
        let dir = ["-P ", "-L ", "-- "]
            .iter()
            .fold(dir, |dir, option| dir.strip_prefix(option).unwrap_or(dir));
        line = format!("{}{dir}{}", &line[..start], &rest[end + 1..]);
    }
    line
}

/// Whether `word` holds an expansion the guard could not make: a variable it does not know, a
/// command substitution, a home directory, a glob or a brace expansion ([`globbed`]).
fn opaque(word: &str) -> bool {
    word.contains(['$', '`', '\u{1}', '\u{2}', '\u{3}']) || globbed(word)
}

/// Whether `word` holds what a shell expands into the names of files, which the guard does not
/// expand: `*`, `?`, a bracket expression (`[` with a `]` after it), or a brace expansion (`{`
/// with a `}` after it). A word the guard reads as a path is unquoted already, so a glob quoted
/// to stand for itself counts too: the guard fails closed rather than guess.
fn globbed(word: &str) -> bool {
    let closes = |open: char, close: char| {
        word.find(open)
            .is_some_and(|at| word[at + open.len_utf8()..].contains(close))
    };
    word.contains(['*', '?']) || closes('[', ']') || closes('{', '}')
}

/// `word` with its expansions made, from the directory `dir` of the tree: `$PWD` and the
/// substitutions [`substituted`] put in place, and the variables `env` knows, `${X:-y}`,
/// `${X-y}`, `${X:=y}` and `${X=y}` among them. `None` when one is not known (`$1`, `${X:-y}` of
/// an `X` it does not know, `~`): the caller fails closed, and may take the default
/// ([`expand_default`]).
fn expand(tree: &Path, word: &str, dir: Option<&str>, env: &Env) -> Option<String> {
    expand_with(tree, word, dir, env, false)
}

/// `word` expanded as [`expand`] does, but a variable it does not know with a default
/// (`${X:-y}`, `${X-y}`, `${X:=y}`, `${X=y}`) taken as that default: the path the line names when
/// the variable is not set, which the guard follows and guards while the line fails closed.
/// `None` when no default stands in for every unknown variable.
fn expand_default(tree: &Path, word: &str, dir: Option<&str>, env: &Env) -> Option<String> {
    expand_with(tree, word, dir, env, true)
}

/// `word` expanded, with `defaults` the default of a variable it does not know taken
/// ([`expand`], [`expand_default`]).
fn expand_with(
    tree: &Path,
    word: &str,
    dir: Option<&str>,
    env: &Env,
    defaults: bool,
) -> Option<String> {
    let here = || -> Option<String> {
        let dir = dir.filter(|dir| *dir != ANYWHERE && !dir.starts_with("**/"))?;
        Some(
            std::path::absolute(tree)
                .ok()?
                .join(dir)
                .display()
                .to_string(),
        )
    };
    let mut out = String::new();
    let mut chars = word.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '\u{1}' => out.push_str(env.script_dir.as_deref()?),
            '\u{2}' => out.push_str(&here()?),
            '\u{3}' => out.push_str(env.source_dir.as_deref().or(env.script_dir.as_deref())?),
            '`' => return None,
            '~' if out.is_empty() => return None,
            '$' => {
                let braced = chars.next_if_eq(&'{').is_some();
                let mut name = String::new();
                while let Some(c) = chars.next_if(|c| c.is_ascii_alphanumeric() || *c == '_') {
                    name.push(c);
                }
                if name.is_empty() {
                    return None;
                }
                // `${NAME:-word}`, `${NAME-word}`, `${NAME:=word}`, `${NAME=word}`: the default
                // runs to the first `}`, and whether an empty value takes it is the colon's.
                let mut default = None;
                if braced {
                    let colon = chars.next_if_eq(&':').is_some();
                    if chars.next_if(|c| *c == '-' || *c == '=').is_some() {
                        let mut word = String::new();
                        loop {
                            match chars.next()? {
                                '}' => break,
                                c => word.push(c),
                            }
                        }
                        default = Some((colon, word));
                    } else if colon || chars.next() != Some('}') {
                        return None;
                    }
                }
                let value = if name == "PWD" {
                    here()
                } else {
                    env.vars.get(&name).cloned()
                };
                match (value, default) {
                    (Some(value), Some((true, word))) if value.is_empty() => {
                        out.push_str(&expand_with(tree, &word, dir, env, defaults)?);
                    }
                    (Some(value), _) => out.push_str(&value),
                    (None, Some((_, word))) if defaults => {
                        out.push_str(&expand_with(tree, &word, dir, env, defaults)?);
                    }
                    (None, _) => return None,
                }
            }
            c => out.push(c),
        }
    }
    Some(out)
}

/// `word` as a shell assignment, `NAME=VALUE`.
fn assignment(word: &str) -> Option<(&str, &str)> {
    let (name, value) = word.split_once('=')?;
    let mut chars = name.chars();
    (chars
        .next()
        .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_'))
    .then_some((name, value))
}

/// The values `word` may take from the directory `dir`: as expanded, or, when a variable in it
/// is not known, as its default ([`expand_default`]) after failing closed; none when there is
/// no default either.
fn values_of(
    tree: &Path,
    word: &str,
    dir: Option<&str>,
    env: &Env,
    named: &mut Vec<String>,
) -> Option<String> {
    expand(tree, word, dir, env).or_else(|| {
        fail_closed(
            named,
            format!("{} holds what the guard cannot expand", quoted(word)),
        );
        expand_default(tree, word, dir, env)
    })
}

/// Sets `name` to `value` in `env`, made in each of `dirs`: a `PYTHONPATH` adds its entries; a
/// `PATH` entry in the tree guards every entry directly in it ([`ON_PATH`]); a variable naming
/// where Python loads from ([`PYTHON_DIRS`]) guards what it names in the tree, whole; a value the
/// guard cannot expand fails closed, and a default it gives (`${X:-dir}`) is followed besides.
fn assign(
    tree: &Path,
    dirs: &[Option<String>],
    name: &str,
    value: &str,
    env: &mut Env,
    named: &mut Vec<String>,
) {
    let values: Vec<Option<String>> = dirs
        .iter()
        .map(|dir| expand(tree, value, dir.as_deref(), env))
        .collect();
    if name == "PYTHONPATH" || name == "PATH" {
        let mut entries = Vec::new();
        for dir in dirs {
            for entry in value.split(':') {
                if [format!("${name}"), format!("${{{name}}}")].contains(&entry.to_owned()) {
                    continue;
                }
                match values_of(tree, entry, dir.as_deref(), env, named) {
                    Some(entry) if entry.is_empty() => entries.push((dir, ".".to_owned())),
                    Some(entry) => entries.push((dir, entry)),
                    None => {}
                }
            }
        }
        if name == "PATH" {
            for (dir, entry) in entries {
                // A tree directory on `PATH`: every entry in it is on the surface. An outside
                // one: a directory a step may not write ([`OUTSIDE_PATH_MARK`]). Either way,
                // where the guard resolves a bare program name ([`Env::path_prefix`]).
                if let Some(path) = changed_dir(tree, dir.as_deref(), &entry) {
                    named.push(marked(&path, ON_PATH));
                } else if let Some(absolute) = absolute_entry(tree, dir.as_deref(), &entry) {
                    named.push(format!("{OUTSIDE_PATH_MARK}{absolute}"));
                }
                if let Some(absolute) = absolute_entry(tree, dir.as_deref(), &entry)
                    && !env.path_prefix.contains(&absolute)
                {
                    env.path_prefix.push(absolute);
                }
            }
        } else {
            // An entry outside the tree: a directory a step may not write ([`OUTSIDE_IMPORTS_MARK`]).
            for (dir, entry) in &entries {
                if changed_dir(tree, dir.as_deref(), entry).is_none()
                    && let Some(absolute) = absolute_entry(tree, dir.as_deref(), entry)
                {
                    named.push(format!("{OUTSIDE_IMPORTS_MARK}{absolute}"));
                }
            }
            env.pythonpath
                .extend(entries.into_iter().map(|(_, entry)| entry));
        }
    } else if PYTHON_DIRS.contains(&name) {
        for dir in dirs {
            let Some(value) = values_of(tree, value, dir.as_deref(), env, named) else {
                continue;
            };
            if let Some(path) = changed_dir(tree, dir.as_deref(), &value) {
                named.push(under(&path));
                if !path.is_empty() {
                    named.push(path);
                }
            } else if let Some(absolute) = absolute_entry(tree, dir.as_deref(), &value) {
                named.push(format!("{OUTSIDE_IMPORTS_MARK}{absolute}"));
            }
        }
    }
    match values.first() {
        Some(Some(first)) if values.iter().all(|value| value.as_ref() == Some(first)) => {
            env.vars.insert(name.to_owned(), first.clone());
        }
        _ => {
            env.vars.remove(name);
        }
    }
}

/// The directories of the tree Python imports from when it runs in `dir`: `dir` itself (where a
/// `python -m` and the start-up modules are found first) and every `PYTHONPATH` entry, named from
/// `dir`.
fn import_dirs(tree: &Path, dir: Option<&str>, env: &Env) -> Vec<String> {
    let mut dirs: Vec<String> = dir.map(str::to_owned).into_iter().collect();
    for entry in &env.pythonpath {
        dirs.extend(changed_dir(tree, dir, entry));
    }
    dirs.sort();
    dirs.dedup();
    dirs
}

/// Marks the directories Python imports from ([`IMPORTS`]) for the command `words` run from
/// `dir` with `env`, whatever its program: `dir` and every `PYTHONPATH` entry
/// ([`import_dirs`]), and the start and top-level directories of a `unittest discover` among
/// the words, which unittest puts first on the import path before it imports the tests.
fn mark_imports(
    tree: &Path,
    words: &[&str],
    dir: Option<&str>,
    env: &Env,
    named: &mut Vec<String>,
) {
    for import in import_dirs(tree, dir, env) {
        named.push(marked(&import, IMPORTS));
    }
    let mut pytest = false;
    for (k, word) in words.iter().enumerate() {
        let name = word.rsplit('/').next().unwrap_or(word);
        // pytest, run by its name or as a module through any wrapper, puts each test file's
        // directory first on the import path.
        if !pytest && (["pytest", "py.test"].contains(&name) || *word == "-mpytest") {
            pytest = true;
            for test_dir in pytest_dirs(tree) {
                named.push(marked(&test_dir, IMPORTS));
            }
        }
        // `nose2 -s DIR` puts its start directory there, as `unittest discover -s` does.
        if name == "nose2" || *word == "-mnose2" {
            for start in nose2_dirs(&words[k + 1..]) {
                if opaque(start) {
                    fail_closed(
                        named,
                        format!("nose2's start directory {} cannot be named", quoted(start)),
                    );
                } else if let Some(path) = changed_dir(tree, dir, start) {
                    named.push(marked(&path, IMPORTS));
                }
            }
        }
    }
    for (k, word) in words.iter().enumerate() {
        let unittest = *word == "unittest" || word.strip_prefix("-m") == Some("unittest");
        if !unittest
            || !words
                .get(k + 1)
                .is_some_and(|next| next.eq_ignore_ascii_case("discover"))
        {
            continue;
        }
        for start in discover_dirs(&words[k + 2..]) {
            if opaque(start) {
                fail_closed(
                    named,
                    format!(
                        "a `unittest discover` directory {} cannot be named",
                        quoted(start)
                    ),
                );
                continue;
            }
            if let Some(path) = changed_dir(tree, dir, start) {
                named.push(marked(&path, IMPORTS));
            }
        }
    }
}

/// The directories pytest's default import mode (`prepend`) puts first on the import path for
/// the tree's tests: for each test file (`test_*.py`, `*_test.py`) and `conftest.py` git tracks
/// or would add, its directory, or the first one up from it without an `__init__.py`; `""` for
/// the tree's root. One the step adds is not among them: what it imports is the step's own code.
fn pytest_dirs(tree: &Path) -> BTreeSet<String> {
    let Ok(listed) = crate::git::run(
        tree,
        &[
            "ls-files",
            "-z",
            "--cached",
            "--others",
            "--exclude-standard",
        ],
    ) else {
        return BTreeSet::new();
    };
    let files: BTreeSet<&str> = listed
        .split('\0')
        .filter(|path| !path.is_empty() && !crate::case::is_under_cases(path))
        .collect();
    let has_init = |dir: &str| {
        let init = if dir.is_empty() {
            "__init__.py".to_owned()
        } else {
            format!("{dir}/__init__.py")
        };
        files.contains(init.as_str())
    };
    let mut dirs = BTreeSet::new();
    for path in &files {
        let name = path.rsplit('/').next().unwrap_or(path);
        let test = name == "conftest.py"
            || (name.starts_with("test_") && name.ends_with(".py"))
            || name.ends_with("_test.py");
        if !test {
            continue;
        }
        let mut dir = parent_of(path);
        loop {
            if !has_init(dir) {
                dirs.insert(dir.to_owned());
                break;
            }
            if dir.is_empty() {
                break;
            }
            dir = parent_of(dir);
        }
    }
    dirs
}

/// The directories `nose2` with the arguments `args` puts on the import path: its start
/// directory (`-s`, `--start-dir`; `.` when none is given) and its top-level directory (`-t`,
/// `--top-level-directory`).
fn nose2_dirs<'w>(args: &[&'w str]) -> Vec<&'w str> {
    let mut dirs = Vec::new();
    let mut start = false;
    let mut at = 0;
    while let Some(&word) = args.get(at) {
        at += 1;
        if matches!(word, "-s" | "--start-dir" | "-t" | "--top-level-directory") {
            start |= matches!(word, "-s" | "--start-dir");
            dirs.extend(args.get(at).copied());
            at += 1;
        } else if let Some((option, value)) = word.split_once('=')
            && ["--start-dir", "--top-level-directory"].contains(&option)
        {
            start |= option == "--start-dir";
            dirs.push(value);
        } else if let Some(value) = word.strip_prefix("-s").filter(|v| !v.is_empty()) {
            start = true;
            dirs.push(value);
        } else if let Some(value) = word.strip_prefix("-t").filter(|v| !v.is_empty()) {
            dirs.push(value);
        }
    }
    if !start {
        dirs.push(".");
    }
    dirs
}

/// The directories `unittest discover` with the arguments `args` puts on the import path: its
/// start directory (`-s`, `--start-directory`, or the first operand; `.` when none is given)
/// and its top-level directory (`-t`, `--top-level-directory`, or the third operand).
fn discover_dirs<'w>(args: &[&'w str]) -> Vec<&'w str> {
    let mut dirs = Vec::new();
    let mut operands = Vec::new();
    let mut start = false;
    let mut at = 0;
    while let Some(&word) = args.get(at) {
        at += 1;
        match word {
            "-s" | "--start-directory" | "-t" | "--top-level-directory" => {
                start |= matches!(word, "-s" | "--start-directory");
                dirs.extend(args.get(at).copied());
                at += 1;
            }
            "-p" | "--pattern" | "-k" | "--durations" => at += 1,
            _ => {
                if let Some((option, value)) = word.split_once('=')
                    && ["--start-directory", "--top-level-directory"].contains(&option)
                {
                    start |= option == "--start-directory";
                    dirs.push(value);
                } else if let Some(value) = word.strip_prefix("-s").filter(|v| !v.is_empty()) {
                    start = true;
                    dirs.push(value);
                } else if let Some(value) = word.strip_prefix("-t").filter(|v| !v.is_empty()) {
                    dirs.push(value);
                } else if !word.starts_with('-') {
                    operands.push(word);
                }
            }
        }
    }
    if let Some(first) = operands.first() {
        start = true;
        dirs.push(first);
    }
    dirs.extend(operands.get(2));
    if !start {
        dirs.push(".");
    }
    dirs
}

/// The text of the script at `full` when a shell reads it: a file whose first line names a shell
/// (`#!/bin/sh`, `#!/usr/bin/env bash`), or names none (an `exec` hands it to `sh`), or any file
/// when a shell named it (`sh check.sh`). `None` for anything else: a link (never followed), a
/// program, a script another interpreter runs. A script too large or not readable fails closed.
fn shell_text(full: &Path, by_shell: bool, named: &mut Vec<String>) -> Option<String> {
    use std::io::Read as _;
    let meta = std::fs::symlink_metadata(full).ok()?;
    if !meta.is_file() {
        return None;
    }
    let mut bytes = Vec::new();
    let read = std::fs::File::open(full)
        .and_then(|file| file.take(SCRIPT_BYTES_AT_MOST + 1).read_to_end(&mut bytes));
    if read.is_err() || bytes.len() as u64 > SCRIPT_BYTES_AT_MOST {
        fail_closed(
            named,
            format!(
                "the script {} cannot be read or is larger than {SCRIPT_BYTES_AT_MOST} bytes",
                full.display()
            ),
        );
        return None;
    }
    if bytes.contains(&0) {
        return None;
    }
    let text = String::from_utf8_lossy(&bytes).into_owned();
    if let Some(shebang) = text.strip_prefix("#!") {
        let interpreter = interpreter_of(shebang)?;
        if !by_shell && !SHELLS.contains(&interpreter.as_str()) {
            return None;
        }
    }
    Some(text)
}

/// `text` split into the lines a shell reads: a line ends at a newline outside quotes, a
/// backslash before a newline joins two, and a `#` starting a word comments out the rest.
fn logical_lines(text: &str) -> Vec<String> {
    let mut lines = Vec::new();
    let mut line = String::new();
    let mut word_start = true;
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '\'' => {
                line.push(c);
                for c in chars.by_ref() {
                    line.push(c);
                    if c == '\'' {
                        break;
                    }
                }
                word_start = false;
            }
            '"' => {
                line.push(c);
                while let Some(c) = chars.next() {
                    if c == '\\' {
                        match chars.next() {
                            Some('\n') | None => {}
                            Some(next) => {
                                line.push(c);
                                line.push(next);
                            }
                        }
                        continue;
                    }
                    line.push(c);
                    if c == '"' {
                        break;
                    }
                }
                word_start = false;
            }
            '\\' => match chars.next() {
                Some('\n') | None => {}
                Some(next) => {
                    line.push(c);
                    line.push(next);
                    word_start = false;
                }
            },
            '#' if word_start => while chars.next_if(|c| *c != '\n').is_some() {},
            '\n' => {
                lines.push(std::mem::take(&mut line));
                word_start = true;
            }
            c => {
                line.push(c);
                word_start = c.is_whitespace() || ";&|()<>".contains(c);
            }
        }
    }
    lines.push(line);
    lines.retain(|line| !line.trim().is_empty());
    lines
}

/// Reads the shell script `text` line by line in `shell` ([`read_line`]).
fn read_text(tree: &Path, text: &str, shell: &mut Shell, named: &mut Vec<String>, depth: usize) {
    for line in logical_lines(text) {
        read_line(tree, &line, shell, named, depth, false);
    }
}

/// Reads `line`, a command line a shell reads, in `shell`: each command of it by the rules of
/// the test command ([`statement`]), and with `every_word` (a line given to `sh -c`) every word
/// that names a path in the tree guarded too. A line holding what the guard does not follow (a
/// command substitution it does not know, a subshell or a function's parentheses, a
/// here-document) fails closed, and what it can read of it is read all the same.
fn read_line(
    tree: &Path,
    line: &str,
    shell: &mut Shell,
    named: &mut Vec<String>,
    depth: usize,
    every_word: bool,
) {
    let raw = line;
    let line = substituted(line);
    let words = shell_words(&line);
    if line.contains("$(")
        || line.contains('`')
        || words.iter().any(|word| word == "(" || word == ")")
        || words
            .iter()
            .any(|word| redirection(word).is_some_and(|(_, operator)| operator.starts_with("<<")))
    {
        fail_closed(
            named,
            format!(
                "the line {} holds a command substitution, a subshell, a function or a here-document, which the guard does not follow",
                quoted(raw)
            ),
        );
    }
    if every_word {
        for dir in &shell.dirs {
            for word in words.iter().filter(|word| redirection(word).is_none()) {
                let word =
                    expand(tree, word, dir.as_deref(), &shell.env).unwrap_or_else(|| word.clone());
                named.extend(named_from(tree, dir.as_deref(), &word));
            }
        }
    }
    // The commands of the line, each with what a pipe before it feeds its standard input: a
    // `|` (or `|&`) alone, not one of `||`.
    let mut piped: Option<Result<String, String>> = None;
    let mut command: Vec<String> = Vec::new();
    let mut k = 0;
    while k <= words.len() {
        match words.get(k) {
            Some(word) if [";", "&", "|", "(", ")"].contains(&word.as_str()) => {
                let pipe = word == "|"
                    && words.get(k + 1).is_none_or(|next| next != "|")
                    && (k == 0 || words[k - 1] != "|");
                let next = pipe.then(|| piped_from(&command));
                statement(tree, &command, shell, named, depth, piped.take());
                piped = next;
                command.clear();
                if pipe && words.get(k + 1).is_some_and(|next| next == "&") {
                    k += 1;
                }
            }
            Some(word) => command.push(word.clone()),
            None => statement(tree, &command, shell, named, depth, piped.take()),
        }
        k += 1;
    }
}

/// Reads one command of a shell line in `shell`: a redirection's input is guarded, the target
/// of an output is the shell's to write; `NAME=VALUE` before nothing sets the shell's variable
/// ([`assign`]), and before a program that program's; `cd` and `pushd` add the directory they
/// change to; `export` and its kin set; `source` and `.` read the file in the same shell, one
/// level deeper; a compound command's word (`if`, `for`, `{`, [`COMPOUND`]) and code given as
/// text (`eval`, `trap`, `alias`, [`OPAQUE`]) fail closed and the rest is read as a command; any
/// other is a command line of its own, from every directory the shell may be in
/// ([`command_paths`]), with what a `<` or the pipe before it (`piped`, [`piped_from`]) feeds its
/// standard input ([`Env::stdin`]).
fn statement(
    tree: &Path,
    command: &[String],
    shell: &mut Shell,
    named: &mut Vec<String>,
    depth: usize,
    piped: Option<Result<String, String>>,
) {
    let dirs = shell.dirs.clone();
    let mut words: Vec<&str> = Vec::new();
    // What a redirection gives the command's standard input, the last one winning: a file
    // (`Ok`), or what the guard knows of it otherwise (`Err`).
    let mut input: Option<Result<String, Stdin>> = None;
    let mut at = 0;
    while let Some(word) = command.get(at) {
        at += 1;
        let Some((descriptor, operator)) = redirection(word) else {
            words.push(word);
            continue;
        };
        // A redirection's target is a word: another redirection (`< <(…)`) is read as itself.
        let target = command
            .get(at)
            .filter(|target| redirection(target).is_none());
        if target.is_some() {
            at += 1;
        }
        let shown_redirection = || {
            quoted(&format!(
                "{descriptor}{operator}{}",
                target.map_or("", String::as_str)
            ))
        };
        // The descriptor it names; none is 0 for `<`, 1 for `>`, both 1 and 2 for `&>`.
        let fd: Option<u32> = if descriptor.is_empty() {
            Some(if operator.starts_with('<') { 0 } else { 1 })
        } else {
            descriptor.parse().ok()
        };
        let known = matches!(
            operator,
            "<" | "<>" | "<&" | "<<" | "<<-" | "<<<" | ">" | ">>" | ">|" | ">&"
        ) || (descriptor.is_empty() && matches!(operator, "&>" | "&>>"));
        let (Some(fd), true, Some(target)) = (fd, known, target) else {
            fail_closed(
                named,
                format!(
                    "the redirection {} in {} is a form the guard does not know",
                    shown_redirection(),
                    quoted(&shown(command))
                ),
            );
            continue;
        };
        // A file opened for reading is one the command may read: guarded, whatever descriptor.
        if matches!(operator, "<" | "<>") {
            for dir in &dirs {
                let target = expand(tree, target, dir.as_deref(), &shell.env)
                    .unwrap_or_else(|| target.clone());
                named.extend(named_from(tree, dir.as_deref(), &target));
            }
        }
        if fd != 0 {
            continue;
        }
        input = Some(match operator {
            "<" | "<>" => Ok(target.clone()),
            "<&" | ">&" if target == "0" => continue,
            "<&" | ">&" if target == "-" => Err(Stdin::Empty),
            "<&" | ">&" => Err(Stdin::Unknown(format!(
                "{}, a descriptor the guard does not follow",
                shown_redirection()
            ))),
            "<<" | "<<-" | "<<<" => Err(Stdin::Unknown(format!(
                "{}, a here-document",
                shown_redirection()
            ))),
            _ => Err(Stdin::Unknown(format!(
                "{}, opened for writing",
                shown_redirection()
            ))),
        });
    }
    let mut at = 0;
    while words
        .get(at)
        .is_some_and(|word| *word == "!" || COMPOUND.contains(word))
    {
        if words[at] != "!" {
            fail_closed(
                named,
                format!(
                    "the command {} opens or closes a compound command (`{}`), which the guard does not follow",
                    quoted(&shown(command)),
                    words[at]
                ),
            );
        }
        at += 1;
    }
    let mut assignments = Vec::new();
    while let Some(pair) = words.get(at).and_then(|word| assignment(word)) {
        assignments.push(pair);
        at += 1;
    }
    // `exec`, `command` and `builtin` run the program after their options: `exec -c`, `-l` and
    // `-a NAME` (the name the program is given, not the program), `command -p`, `-v`, `-V`. An
    // option the guard does not know fails closed: it cannot tell where the program is.
    let mut exec = false;
    while let Some(&builtin) = words
        .get(at)
        .filter(|word| matches!(**word, "exec" | "command" | "builtin"))
    {
        exec |= builtin == "exec";
        at += 1;
        let (flags, with_argument) = match builtin {
            "exec" => ("cl", "a"),
            "command" => ("pvV", ""),
            _ => ("", ""),
        };
        while let Some(word) = words.get(at) {
            if *word == "--" {
                at += 1;
                break;
            }
            let Some(cluster) = word.strip_prefix('-').filter(|cluster| !cluster.is_empty()) else {
                break;
            };
            at += 1;
            for (k, c) in cluster.char_indices() {
                if with_argument.contains(c) {
                    if cluster[k + c.len_utf8()..].is_empty() {
                        at += 1;
                    }
                    break;
                }
                if !flags.contains(c) {
                    fail_closed(
                        named,
                        format!(
                            "`{builtin}` takes an option the guard does not know, `-{c}`, in {}",
                            quoted(&shown(command))
                        ),
                    );
                    break;
                }
            }
        }
    }
    let rest = &words[at..];
    let Some(&first) = rest.first() else {
        for (name, value) in assignments {
            assign(tree, &dirs, name, value, &mut shell.env, named);
        }
        // `exec < FILE` with no program: the shell's own standard input, which the commands
        // after it read, is the file from then on; a shell among them given no script reads it
        // as its script ([`stdin_script`]).
        if exec && let Some(input) = input {
            shell.env.stdin = Some(match input {
                Ok(target) => {
                    let mut stdins: Vec<Stdin> = dirs
                        .iter()
                        .map(|dir| stdin_of(tree, &target, dir.as_deref(), &shell.env))
                        .collect();
                    match stdins.len() {
                        1 => stdins.remove(0),
                        _ => Stdin::Unknown(format!(
                            "{}, read from a directory the guard cannot name",
                            quoted(&target)
                        )),
                    }
                }
                Err(stdin) => stdin,
            });
        }
        return;
    };
    match first {
        "cd" | "pushd" => {
            let target = rest[1..]
                .iter()
                .find(|word| !word.starts_with('-') && !word.starts_with('+'));
            for dir in &dirs {
                let mut to = Vec::new();
                match target {
                    // Home, or a directory taken off the stack: home is outside the tree, and
                    // a directory on the stack is one the shell may be in already.
                    None => to.push(None),
                    Some(target) if target.starts_with('~') => to.push(None),
                    Some(target) => match expand(tree, target, dir.as_deref(), &shell.env)
                        .filter(|target| !globbed(target))
                    {
                        Some(target) => to.push(changed_dir(tree, dir.as_deref(), &target)),
                        None => {
                            // Anywhere, and where the default (`${X:-dir}`) leads.
                            fail_closed(
                                named,
                                format!(
                                    "`{first} {target}` changes to a directory the guard cannot name"
                                ),
                            );
                            to.push(Some(ANYWHERE.to_owned()));
                            if let Some(target) =
                                expand_default(tree, target, dir.as_deref(), &shell.env)
                            {
                                to.push(changed_dir(tree, dir.as_deref(), &target));
                            }
                        }
                    },
                }
                for to in to {
                    if !shell.dirs.contains(&to) {
                        shell.dirs.push(to);
                    }
                }
            }
        }
        "export" | "readonly" | "declare" | "typeset" | "local" => {
            for (name, value) in rest[1..].iter().filter_map(|word| assignment(word)) {
                assign(tree, &dirs, name, value, &mut shell.env, named);
            }
        }
        "source" | "." => {
            let Some(target) = rest.get(1) else {
                return;
            };
            for dir in &dirs {
                // A file named through a variable with a default (`${X:-file}`) fails closed
                // and is read where the default leads.
                let Some(target) = values_of(tree, target, dir.as_deref(), &shell.env, named)
                else {
                    continue;
                };
                if globbed(&target) {
                    fail_closed(
                        named,
                        format!(
                            "the file sourced, {}, is a pattern the guard does not expand",
                            quoted(&target)
                        ),
                    );
                    continue;
                }
                let Some(path) = named_from(tree, dir.as_deref(), &target) else {
                    // A file outside the tree is code the guard cannot see, and what it does to
                    // `PATH` or the environment it cannot follow: fail closed, and say why.
                    if !target.is_empty() {
                        fail_closed(
                            named,
                            format!(
                                "the test command's script sources {target}, outside the tree, whose effect on `PATH` and the interpreter takt cannot follow"
                            ),
                        );
                    }
                    continue;
                };
                named.push(path.clone());
                if shell.env.level >= SCRIPTS_AT_MOST {
                    fail_closed(
                        named,
                        format!(
                            "{path} is sourced deeper than the {SCRIPTS_AT_MOST} scripts the guard reads"
                        ),
                    );
                    named.push(under(parent_of(&path)));
                    continue;
                }
                // A file not in the tree to read (a link, nothing yet) is code the guard cannot
                // see. In the file read, `BASH_SOURCE` is that file; `$0` stays the script.
                match shell_text(&tree.join(&path), true, named) {
                    Some(text) => {
                        let source_dir = std::path::absolute(tree)
                            .ok()
                            .map(|tree| tree.join(parent_of(&path)).display().to_string());
                        let outer = std::mem::replace(&mut shell.env.source_dir, source_dir);
                        shell.env.level += 1;
                        read_text(tree, &text, shell, named, depth + 1);
                        shell.env.level -= 1;
                        shell.env.source_dir = outer;
                    }
                    None => fail_closed(
                        named,
                        format!(
                            "the sourced file {path} is not a file of the tree the guard can read"
                        ),
                    ),
                }
            }
        }
        first if OPAQUE.contains(&first) => {
            fail_closed(
                named,
                format!(
                    "the command {} runs `{first}`, which runs code given as text or changes what the guard follows",
                    quoted(&rest.join(" "))
                ),
            );
            for word in &rest[1..] {
                let code = word.split_once('=').map_or(*word, |(_, code)| code);
                for line in [*word, code] {
                    let every_word = shell.env.level == 0;
                    read_line(tree, line, shell, named, depth + 1, every_word);
                }
            }
        }
        _ => {
            for dir in &dirs {
                let mut env = shell.env.clone();
                // A redirection wins over the pipe; with neither, the shell's own input.
                match (&input, &piped) {
                    (Some(Ok(target)), _) | (None, Some(Ok(target))) => {
                        env.stdin = Some(stdin_of(tree, target, dir.as_deref(), &shell.env));
                    }
                    (Some(Err(stdin)), _) => env.stdin = Some(stdin.clone()),
                    (None, Some(Err(producer))) => {
                        env.stdin = Some(Stdin::Unknown(format!(
                            "the output of {}",
                            quoted(producer)
                        )));
                    }
                    (None, None) => {}
                }
                for (name, value) in &assignments {
                    assign(
                        tree,
                        std::slice::from_ref(dir),
                        name,
                        value,
                        &mut env,
                        named,
                    );
                }
                // A word with a variable it does not know but a default for (`${X:-./run}`) is
                // taken as the default, and the line fails closed unless the default is empty
                // outside the program's place (`"${1:-}"`); any other stays as written.
                let expanded: Vec<String> = rest
                    .iter()
                    .enumerate()
                    .map(|(k, word)| {
                        expand(tree, word, dir.as_deref(), &env)
                            .or_else(|| {
                                let default = expand_default(tree, word, dir.as_deref(), &env);
                                if default.as_ref().is_some_and(|d| k == 0 || !d.is_empty()) {
                                    fail_closed(
                                        named,
                                        format!(
                                            "{} holds a variable the guard does not know and is taken as its default",
                                            quoted(word)
                                        ),
                                    );
                                }
                                default
                            })
                            .unwrap_or_else(|| (*word).to_owned())
                    })
                    .collect();
                let words: Vec<&str> = expanded.iter().map(String::as_str).collect();
                command_paths(tree, &words, dir.clone(), named, depth + 1, &env);
            }
        }
    }
}

/// The directory `path`, relative to the tree, is in: `""` for the tree's root.
fn parent_of(path: &str) -> &str {
    path.rsplit_once('/').map_or("", |(parent, _)| parent)
}

/// Reads the shell script at `script` (relative to the tree), run from `dir` with `env`, by the
/// rules of the test command ([`read_text`]); a script another interpreter runs is guarded by
/// itself and not read ([`shell_text`]), and with `by_shell` (`sh check.sh`) any file is read as
/// a shell script. A script deeper than [`SCRIPTS_AT_MOST`] is not read: everything in its
/// directory, and every Python file of the tree, is guarded instead.
fn script_paths(
    tree: &Path,
    script: &str,
    dir: Option<&str>,
    env: &Env,
    named: &mut Vec<String>,
    depth: usize,
    by_shell: bool,
) {
    let Some(text) = shell_text(&tree.join(script), by_shell, named) else {
        return;
    };
    let parent = parent_of(script);
    if env.level >= SCRIPTS_AT_MOST {
        fail_closed(
            named,
            format!("{script} runs deeper than the {SCRIPTS_AT_MOST} scripts the guard reads"),
        );
        named.push(under(parent));
        return;
    }
    let script_dir = std::path::absolute(tree)
        .ok()
        .map(|tree| tree.join(parent).display().to_string());
    let mut shell = Shell {
        dirs: vec![dir.map(str::to_owned)],
        env: Env {
            source_dir: script_dir.clone(),
            script_dir,
            level: env.level + 1,
            ..env.clone()
        },
    };
    read_text(tree, &text, &mut shell, named, depth);
}

/// Where the options of a program end.
enum End {
    /// At the operand at this index, or past the last word.
    Operand,
    /// The rest is code, not a file.
    Stop,
    /// The command line of its own starting at this index.
    Command(usize),
}

/// What a program's options said, read from the word at `at` on.
struct Scanned<'w> {
    at: usize,
    end: End,
    chdirs: Vec<&'w str>,
    files: Vec<&'w str>,
    /// The modules an option names to run (`python -m MODULE`).
    modules: Vec<&'w str>,
}

/// Reads the options of a program from `words[at..]` by `spec`, to the first operand.
fn scan<'w>(words: &[&'w str], mut at: usize, spec: &Spec) -> Scanned<'w> {
    let mut chdirs = Vec::new();
    let mut files = Vec::new();
    let mut modules = Vec::new();
    let done = |at, end, chdirs, files| Scanned {
        at,
        end,
        chdirs,
        files,
        modules: Vec::new(),
    };
    while let Some(word) = words.get(at).copied() {
        if word == "--" {
            return done(at + 1, End::Operand, chdirs, files);
        }
        let folded = fold(word);
        if spec.word_stop.contains(&folded.as_str()) {
            return done(at, End::Stop, chdirs, files);
        }
        if spec.word_file.contains(&folded.as_str()) {
            at += 1;
            files.extend(words.get(at).copied());
            at += 1;
            continue;
        }
        if let Some(long) = word.strip_prefix("--") {
            let (name, value) = match long.split_once('=') {
                Some((name, value)) => (name, Some(value)),
                None => (long, None),
            };
            if spec.long_stop.contains(&name) {
                return done(at, End::Stop, chdirs, files);
            }
            if spec.long_command.contains(&name) {
                return done(
                    at,
                    if value.is_some() {
                        End::Stop
                    } else {
                        End::Command(at + 1)
                    },
                    chdirs,
                    files,
                );
            }
            let chdir = spec.long_chdir.contains(&name);
            let file = spec.long_file.contains(&name);
            if chdir || file || spec.long_arg.contains(&name) {
                let argument = match value {
                    Some(value) => Some(value),
                    None => {
                        at += 1;
                        words.get(at).copied()
                    }
                };
                if chdir {
                    chdirs.extend(argument);
                }
                if file {
                    files.extend(argument);
                }
            }
            at += 1;
            continue;
        }
        let cluster = word
            .strip_prefix('-')
            .or_else(|| word.strip_prefix('+').filter(|_| spec.plus))
            .filter(|cluster| !cluster.is_empty());
        let Some(cluster) = cluster else {
            return done(at, End::Operand, chdirs, files);
        };
        for (k, c) in cluster.char_indices() {
            let rest = &cluster[k + c.len_utf8()..];
            if spec.short_stop.contains(c) {
                return done(at, End::Stop, chdirs, files);
            }
            if spec.short_command.contains(c) {
                return done(at, End::Command(at + 1), chdirs, files);
            }
            if spec.short_module.contains(c) {
                let module = if rest.is_empty() {
                    words.get(at + 1).copied()
                } else {
                    Some(rest)
                };
                modules.extend(module);
                return Scanned {
                    modules,
                    ..done(at, End::Stop, chdirs, files)
                };
            }
            let chdir = spec.short_chdir.contains(c);
            let file = spec.short_file.contains(c);
            if chdir || file || spec.short_arg.contains(c) {
                let argument = if rest.is_empty() {
                    at += 1;
                    words.get(at).copied()
                } else {
                    Some(rest)
                };
                if chdir {
                    chdirs.extend(argument);
                }
                if file {
                    files.extend(argument);
                }
                break;
            }
        }
        at += 1;
    }
    done(at, End::Operand, chdirs, files)
}

/// The spec of `name` in `table`.
fn spec_of<T: Copy>(table: &[(&[&str], T)], name: &str) -> Option<T> {
    table
        .iter()
        .find(|(names, _)| names.contains(&name))
        .map(|(_, spec)| *spec)
}

/// The paths of `tree` the command line `words`, run from `dir` with what the shell set in
/// `env`, runs as its own: added to `named`. See [`scripts_of`].
fn command_paths(
    tree: &Path,
    words: &[&str],
    mut dir: Option<String>,
    named: &mut Vec<String>,
    depth: usize,
    env: &Env,
) {
    if depth > COMMANDS_AT_MOST {
        return;
    }
    let mut env = env.clone();
    let mut at = 0;
    while let Some(program) = words.get(at).copied() {
        // `NAME=VALUE` before the program: the program's environment.
        if let Some((name, value)) = assignment(program) {
            assign(
                tree,
                std::slice::from_ref(&dir),
                name,
                value,
                &mut env,
                named,
            );
            at += 1;
            continue;
        }
        // Whatever the program, Python may run under it: where it runs and what `PYTHONPATH`
        // names are import directories, and so are a `unittest discover`'s.
        mark_imports(tree, &words[at..], dir.as_deref(), &env, named);
        // The program as the case asks for the module names its Python imports ([`Modules`]).
        named.extend(program_mark(tree, dir.as_deref(), program, &env));
        // A program the guard cannot name runs what it cannot see.
        if opaque(program) {
            fail_closed(
                named,
                format!("the program {} cannot be named", quoted(program)),
            );
        }
        // A program in the tree that a shell reads is read line by line; one Python runs
        // imports from its own directory first.
        let in_tree = named_from(tree, dir.as_deref(), program);
        if let Some(path) = in_tree.clone() {
            script_paths(tree, &path, dir.as_deref(), &env, named, depth, false);
            if shebang_of(&tree.join(&path)).is_some_and(|interpreter| {
                spec_of(INTERPRETERS, &interpreter)
                    .is_some_and(|spec| !spec.short_module.is_empty())
            }) {
                named.push(marked(parent_of(&path), IMPORTS));
                named.push(format!("{PYTHON_MARK}{path}"));
            }
            named.push(path);
        }
        let name = program_of(program);
        at += 1;
        if name == "env" {
            // `NAME=VALUE` pairs and the options: `-u NAME`, `-C DIR`, `-S STRING`, whose
            // words are the command, `--unset=`, `--chdir=`, `--split-string=`.
            while let Some(word) = words.get(at).copied() {
                if word == "--" {
                    at += 1;
                    break;
                }
                if let Some(long) = word.strip_prefix("--") {
                    let (option, value) = match long.split_once('=') {
                        Some((option, value)) => (option, Some(value)),
                        None => (long, None),
                    };
                    let argument = |at: &mut usize| match value {
                        Some(value) => Some(value),
                        None => {
                            *at += 1;
                            words.get(*at).copied()
                        }
                    };
                    match option {
                        "unset" => {
                            argument(&mut at);
                        }
                        "chdir" => {
                            if let Some(to) = argument(&mut at) {
                                dir = changed_dir(tree, dir.as_deref(), to);
                            }
                        }
                        "split-string" => {
                            let (string, after) = match value {
                                Some(value) => (Some(value), at + 1),
                                None => (words.get(at + 1).copied(), at + 2),
                            };
                            let Some(string) = string else {
                                return;
                            };
                            return env_split(
                                tree,
                                string,
                                &words[after.min(words.len())..],
                                dir,
                                named,
                                depth,
                                &env,
                            );
                        }
                        _ => {}
                    }
                    at += 1;
                    continue;
                }
                if let Some(cluster) = word.strip_prefix('-').filter(|c| !c.is_empty()) {
                    let mut split = None;
                    for (k, c) in cluster.char_indices() {
                        let rest = &cluster[k + c.len_utf8()..];
                        match c {
                            'u' => {
                                if rest.is_empty() {
                                    at += 1;
                                }
                                break;
                            }
                            'C' => {
                                let to = if rest.is_empty() {
                                    at += 1;
                                    words.get(at).copied()
                                } else {
                                    Some(rest)
                                };
                                if let Some(to) = to {
                                    dir = changed_dir(tree, dir.as_deref(), to);
                                }
                                break;
                            }
                            'S' => {
                                split = Some(rest);
                                break;
                            }
                            _ => {}
                        }
                    }
                    match split {
                        Some("") => {
                            let Some(string) = words.get(at + 1).copied() else {
                                return;
                            };
                            let after = (at + 2).min(words.len());
                            return env_split(
                                tree,
                                string,
                                &words[after..],
                                dir,
                                named,
                                depth,
                                &env,
                            );
                        }
                        Some(rest) => {
                            return env_split(
                                tree,
                                rest,
                                &words[at + 1..],
                                dir,
                                named,
                                depth,
                                &env,
                            );
                        }
                        None => {}
                    }
                    at += 1;
                    continue;
                }
                if word.contains('=') {
                    if let Some((name, value)) = assignment(word) {
                        assign(
                            tree,
                            std::slice::from_ref(&dir),
                            name,
                            value,
                            &mut env,
                            named,
                        );
                    }
                    at += 1;
                    continue;
                }
                break;
            }
            continue;
        }
        if let Some(spec) = spec_of(WRAPPERS, &name) {
            // `xargs` appends the words of its standard input (or of `-a FILE`) to the command
            // it runs, which the guard does not read: the command is read, and the line fails
            // closed for what it is given.
            if name == "xargs" {
                fail_closed(
                    named,
                    format!(
                        "`xargs` runs {} with the words of its input appended, which the guard does not read",
                        quoted(&words[at..].join(" "))
                    ),
                );
            }
            // `uv run` has options of its own before the command; a `*.py` operand or `-m
            // MODULE` after them runs under the project's Python.
            if name == "uv" {
                if words.get(at) != Some(&"run") {
                    return;
                }
                let scanned = scan(words, at + 1, &spec);
                for to in scanned.chdirs {
                    dir = changed_dir(tree, dir.as_deref(), to);
                }
                let End::Operand = scanned.end else {
                    return;
                };
                let rest = &words[scanned.at.min(words.len())..];
                let python = rest.first().is_some_and(|word| {
                    word.ends_with(".py") || matches!(*word, "-m" | "--module")
                });
                if python {
                    let mut under_python = vec!["python"];
                    under_python.extend(
                        rest.iter()
                            .map(|word| if *word == "--module" { "-m" } else { word }),
                    );
                    return command_paths(tree, &under_python, dir, named, depth + 1, &env);
                }
                return command_paths(tree, rest, dir, named, depth + 1, &env);
            }
            // Options may stand before the operands and after them (`flock FILE -c CMD`).
            let mut operands = spec.operands;
            loop {
                let scanned = scan(words, at, &spec);
                for to in scanned.chdirs {
                    dir = changed_dir(tree, dir.as_deref(), to);
                }
                match scanned.end {
                    End::Stop => return,
                    End::Command(from) => {
                        return command_line(tree, words, from, dir.as_deref(), named, depth, &env);
                    }
                    End::Operand if operands > 0 && scanned.at < words.len() => {
                        at = scanned.at + 1;
                        operands -= 1;
                    }
                    End::Operand => {
                        at = scanned.at;
                        break;
                    }
                }
            }
            continue;
        }
        if let Some(spec) = spec_of(INTERPRETERS, &name) {
            let scanned = scan(words, at, &spec);
            for to in scanned.chdirs {
                dir = changed_dir(tree, dir.as_deref(), to);
            }
            for file in scanned.files {
                named.extend(named_from(tree, dir.as_deref(), file));
            }
            // Python imports from the directory it runs in and from every `PYTHONPATH` entry
            // before the installed modules: `-m pkg.module` imports `pkg`, and `pkg.py`, a
            // compiled `pkg.*` and the package `pkg/` in any of them replace the installed one;
            // a `*.pth` file there is a start-up file.
            if !spec.short_module.is_empty() {
                let imports = import_dirs(tree, dir.as_deref(), &env);
                for module in scanned.modules {
                    if opaque(module) {
                        fail_closed(
                            named,
                            format!("the module `-m` runs, {}, cannot be named", quoted(module)),
                        );
                    }
                    let first = module.split('.').next().unwrap_or(module);
                    // A build tool run as a module reads its recipe as it does by its name.
                    if let Some((_, _, defaults, _)) = BUILD_TOOLS
                        .iter()
                        .find(|(names, _, _, _)| names.contains(&first))
                        .filter(|_| UNREAD_RECIPES.contains(&first))
                    {
                        fail_closed(
                            named,
                            format!(
                                "`-m {first}` runs a build tool whose recipe the guard does not read"
                            ),
                        );
                        for file in *defaults {
                            named.extend(named_from(tree, dir.as_deref(), file));
                        }
                    }
                    for import in &imports {
                        let import = Some(import.as_str());
                        let loads = [
                            named_from(tree, import, &format!("{first}.py")),
                            named_from(tree, import, &format!("{first}.*")),
                            named_from(tree, import, first).map(|package| format!("{package}/**")),
                        ];
                        for path in loads.into_iter().flatten() {
                            python_named(named, path);
                        }
                    }
                }
                for import in &imports {
                    if let Some(path) = named_from(tree, Some(import.as_str()), "*.pth") {
                        python_named(named, path);
                    }
                }
            }
            match scanned.end {
                End::Stop => {
                    // `python -c CODE`: what the code imports is loaded from the directories
                    // Python imports from, as `-m` loads its module; the gate compares it.
                    if !spec.short_module.is_empty()
                        && let Some(word) = words.get(scanned.at)
                        && let Some(cluster) = word.strip_prefix('-')
                        && let Some((_, rest)) = cluster.split_once('c')
                    {
                        let code = if rest.is_empty() {
                            words.get(scanned.at + 1).copied()
                        } else {
                            Some(rest)
                        };
                        if let Some(code) = code {
                            let imports = import_dirs(tree, dir.as_deref(), &env);
                            python_imports_named(tree, code, &imports, named);
                        }
                    }
                }
                End::Command(from) => {
                    command_line(tree, words, from, dir.as_deref(), named, depth, &env);
                }
                End::Operand => {
                    let mut script = scanned.at;
                    // A shell given no script, `-` or `-s` reads it from its standard input.
                    if SHELLS.contains(&name.as_str())
                        && (words.get(script).is_none_or(|word| *word == "-")
                            || words[at..script.min(words.len())].iter().any(|word| {
                                word.strip_prefix('-').is_some_and(|cluster| {
                                    !cluster.starts_with('-') && cluster.contains('s')
                                })
                            }))
                    {
                        stdin_script(tree, dir.as_deref(), &env, named, depth);
                    }
                    // A Python given no script or `-` reads it from its standard input: a file
                    // of the tree is the script it runs (`python3 - < check.py`), with the
                    // directory it runs in first on its import path, not the file's own.
                    if !spec.short_module.is_empty()
                        && words.get(script).is_none_or(|word| *word == "-")
                    {
                        match &env.stdin {
                            Some(Stdin::File(path)) => python_named(named, path.clone()),
                            Some(Stdin::Unknown(why)) => fail_closed(
                                named,
                                format!(
                                    "a Python reads its script from its standard input, {why}, which the guard cannot read"
                                ),
                            ),
                            None | Some(Stdin::Empty) => {}
                        }
                    }
                    // A subcommand (`deno run`) has options of its own before the script.
                    if words
                        .get(script)
                        .is_some_and(|word| spec.subcommands.contains(word))
                    {
                        let after = scan(words, script + 1, &spec);
                        for to in after.chdirs {
                            dir = changed_dir(tree, dir.as_deref(), to);
                        }
                        for file in after.files {
                            named.extend(named_from(tree, dir.as_deref(), file));
                        }
                        match after.end {
                            End::Operand => script = after.at,
                            End::Stop => return,
                            End::Command(from) => {
                                return command_line(
                                    tree,
                                    words,
                                    from,
                                    dir.as_deref(),
                                    named,
                                    depth,
                                    &env,
                                );
                            }
                        }
                    }
                    if let Some(script) = words.get(script).filter(|word| **word != "-") {
                        if opaque(script) {
                            fail_closed(
                                named,
                                format!("the script {} cannot be named", quoted(script)),
                            );
                        }
                        // A script a shell runs is read line by line, whatever its first line;
                        // one Python runs imports from its own directory first.
                        if let Some(path) = named_from(tree, dir.as_deref(), script) {
                            if SHELLS.contains(&name.as_str()) {
                                script_paths(tree, &path, dir.as_deref(), &env, named, depth, true);
                            }
                            if !spec.short_module.is_empty() {
                                named.push(marked(parent_of(&path), IMPORTS));
                                named.push(format!("{PYTHON_MARK}{path}"));
                            }
                            named.push(path);
                        }
                    }
                }
            }
            return;
        }
        if let Some((spec, defaults, configs)) = BUILD_TOOLS
            .iter()
            .find(|(names, _, _, _)| names.contains(&name.as_str()))
            .map(|(_, spec, defaults, configs)| (*spec, *defaults, *configs))
        {
            // Options and targets interleave: every option up to the last word is read.
            let mut files = Vec::new();
            loop {
                let scanned = scan(words, at, &spec);
                for to in scanned.chdirs {
                    dir = changed_dir(tree, dir.as_deref(), to);
                }
                files.extend(scanned.files);
                match scanned.end {
                    End::Operand if scanned.at < words.len() => at = scanned.at + 1,
                    _ => break,
                }
            }
            if files.is_empty() {
                files.extend(defaults);
            }
            files.extend(configs);
            // No recipe parser reads what the target runs.
            if UNREAD_RECIPES.contains(&name.as_str()) {
                fail_closed(
                    named,
                    format!("`{name}` runs a recipe the guard does not read"),
                );
            }
            for file in files {
                named.extend(named_from(tree, dir.as_deref(), file));
            }
            return;
        }
        // `pytest FILE…`: each Python file given runs as a test module, its directory first on
        // the import path; the gate compares it as a script a Python runs ([`PYTHON_MARK`]).
        if name == "pytest" || name == "py.test" {
            for word in words[at..].iter().filter(|word| !word.starts_with('-')) {
                let file = word.split_once("::").map_or(*word, |(file, _)| file);
                if !file.ends_with(".py") || opaque(file) {
                    continue;
                }
                if let Some(path) = named_from(tree, dir.as_deref(), file) {
                    named.push(marked(parent_of(&path), IMPORTS));
                    named.push(format!("{PYTHON_MARK}{path}"));
                }
            }
        }
        // A program in none of the tables that is given a program the guard knows (`taskset 1
        // python3 check.py` before `taskset` was listed) may run it in a way the guard cannot
        // see. A file in the tree is named above, so the gate compares it whole at the base
        // (`scripts/gate-exec python3 -I scripts/check-citations`, which runs what it is given,
        // and a file a step made is a change): the most it runs is that program with the words
        // after it, read as a command from here with the line's environment and directory. A
        // word before that program, which the file may take as an option's value
        // (`--interpreter sh`) or for anything else, and a form by which the file sets the
        // environment, directory or import path of what it runs ([`hand_off_unread`]) fail the
        // line closed, naming them; the command is read besides. One the tree does not hold,
        // which the guard has not read: the line fails closed, naming both.
        if let Some(position) = words[at..]
            .iter()
            .position(|word| !word.starts_with('-') && known_program(word))
        {
            let given = words[at + position];
            match in_tree.as_ref().filter(|path| tree.join(path).is_file()) {
                Some(path) => {
                    if position > 0 {
                        fail_closed(
                            named,
                            format!(
                                "`{path}` is given {} before {}, a program, and the guard does not know what `{path}` takes it for",
                                quoted(&words[at..at + position].join(" ")),
                                quoted(given)
                            ),
                        );
                    }
                    if let Some(why) = hand_off_unread(tree, path, given) {
                        fail_closed(named, why);
                    }
                    command_paths(
                        tree,
                        &words[at + position..],
                        dir.clone(),
                        named,
                        depth + 1,
                        &env,
                    );
                }
                None => fail_closed(
                    named,
                    format!(
                        "`{name}` is given {}, a program, and the guard does not know how `{name}` runs it",
                        quoted(given)
                    ),
                ),
            }
        }
        // A program in none of the tables: what it reads is not known, so every word of the
        // rest that names a path in the tree is guarded, as written and as a shell would unquote
        // it, an option's value after `=` among them. Whether the path is there yet does not
        // matter: a step could make it. Not in a script the test command runs: what a line
        // there names (`grep -q fixed check.txt`) is the data the tests check, the code under
        // test's, and only the programs, scripts and modules it runs are guarded.
        if env.level > 0 {
            return;
        }
        for word in &words[at..] {
            let word = if word.starts_with('-') {
                match word.split_once('=') {
                    Some((_, value)) => value,
                    None => continue,
                }
            } else {
                word
            };
            named.extend(named_from(tree, dir.as_deref(), word));
            for unquoted in shell_words(word)
                .into_iter()
                .filter(|word| redirection(word).is_none())
            {
                named.extend(named_from(tree, dir.as_deref(), &unquoted));
            }
        }
        return;
    }
}

/// The command `env -S STRING` runs: the string split into words as `env` splits it (at white
/// space, quotes kept together), with the words after it, `rest`, appended, read as a command
/// line from `dir` ([`command_paths`]).
fn env_split(
    tree: &Path,
    string: &str,
    rest: &[&str],
    dir: Option<String>,
    named: &mut Vec<String>,
    depth: usize,
    env: &Env,
) {
    let split: Vec<String> = shell_words(string)
        .into_iter()
        .map(|word| word.trim_start_matches(REDIRECT).to_owned())
        .collect();
    let mut words_of: Vec<&str> = split.iter().map(String::as_str).collect();
    words_of.extend(rest);
    command_paths(tree, &words_of, dir, named, depth + 1, env);
}

/// The command line an option such as `sh -c` or `flock -c` runs: the word at `from`, after a
/// `--` that ends the options, read as a shell reads it ([`read_line`]); in the test command
/// itself, not a script it runs, every word naming a path in the tree is guarded besides.
fn command_line(
    tree: &Path,
    words: &[&str],
    mut from: usize,
    dir: Option<&str>,
    named: &mut Vec<String>,
    depth: usize,
    env: &Env,
) {
    if words.get(from) == Some(&"--") {
        from += 1;
    }
    // The command line is the one word at `from`; the words after it are its `$0` and
    // arguments. A test command is split at white space, so a line meant to be quoted
    // (`sh -c 'a; b'`) arrives in pieces: each piece, and the pieces joined, are read as well.
    let rest = words.get(from..).unwrap_or_default();
    let joined = rest.join(" ");
    let lines = rest
        .iter()
        .copied()
        .chain((rest.len() > 1).then_some(joined.as_str()));
    for line in lines {
        let mut shell = Shell {
            dirs: vec![dir.map(str::to_owned)],
            env: env.clone(),
        };
        read_line(tree, line, &mut shell, named, depth, env.level == 0);
    }
}

/// The paths in `tree` the command `words` runs as its own: the program when it names a path;
/// the script an interpreter ([`INTERPRETERS`]) runs, past the options that take an argument
/// (`sh -o OPT`), or nothing more for code given inline (`python -c`, `python -m`); what a
/// command line given to `sh -c`, `flock -c` or `env -S` runs, read the same way; the command a
/// wrapper ([`WRAPPERS`]: `timeout DURATION`, `nice -n N`, …) or `env` runs, past its options,
/// their arguments, its operands and `NAME=VALUE` pairs; and the file a build tool
/// ([`BUILD_TOOLS`]: make, task, npm, yarn, pnpm, just) reads, in the directory `-C` names or as
/// `-f` names it. A word that names nothing yet is named all the same: a step could make it.
///
/// A shell script among them is read line by line, by these same rules, and the scripts it runs
/// one level further ([`script_paths`]); `NAME=VALUE` before a program, `export`, `cd` and
/// `source` are followed. What `python -m MODULE` imports is named in the directory it runs in
/// and in every `PYTHONPATH` entry, with the `*.pth` start-up files there. Under any program,
/// where it runs, each `PYTHONPATH` entry, a `unittest discover`'s directories and a Python
/// script's own directory are named as import directories ([`IMPORTS`]), and a `PATH` entry in
/// the tree as one whose entries are programs ([`ON_PATH`]). A word with an expansion the guard
/// cannot make, a line it cannot read, a script too deep and a build tool whose recipe it does
/// not read ([`UNREAD_RECIPES`]) add every Python file, shell file and program of the tree
/// ([`FAIL_CLOSED`]), which [`JudgeSurface`] guards whole.
#[must_use]
pub fn scripts_of(words: &[String], tree: &Path) -> Vec<String> {
    read_command(words, tree)
        .into_iter()
        .filter(|path| !path.starts_with(PYTHON_MARK))
        .collect()
}

/// What [`scripts_of`] names for the command `words`, with what a Python loads as code named
/// once more behind [`PYTHON_MARK`]: one reader for the step guard and the gate.
#[must_use]
pub(crate) fn read_command(words: &[String], tree: &Path) -> Vec<String> {
    let words: Vec<&str> = words.iter().map(String::as_str).collect();
    let mut named = Vec::new();
    // The `PATH` takt runs the command with, which it inherits: an entry that resolves inside
    // the tree (an empty one is where the command runs, its root) holds programs it may run.
    if let Some(path) = std::env::var_os("PATH") {
        for entry in std::env::split_paths(&path) {
            let entry = entry.to_string_lossy();
            let entry = if entry.is_empty() { "." } else { &entry };
            if let Some(dir) = landing(tree, "", entry) {
                named.push(marked(&dir, ON_PATH));
            }
        }
    }
    command_paths(
        tree,
        &words,
        Some(String::new()),
        &mut named,
        0,
        &Env::default(),
    );
    named.sort();
    named.dedup();
    named
}

/// A run's judge surface: what decides its validators, which no judgement step changes
/// ([`JUDGE_SURFACE`]). Paths are matched whatever their case, as a file system that folds case
/// would open them. A link is never followed: a guarded path whose resolution crosses a link
/// anywhere on the way, or leaves the tree, is not guarded through it, and the step guard refuses
/// a step while there is one ([`JudgeSurface::links`]). The cases directory is the kernel's and
/// not on it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct JudgeSurface {
    /// The test command's own scripts and its build tool's files, each by itself.
    scripts: BTreeSet<String>,
    /// Whether the planning store is on it: always, but for what decides a validator alone
    /// ([`JudgeSurface::without_store`]).
    store: bool,
    /// The repository's own additions ([`JUDGE_LIST`]), path globs folded, each guarding what it
    /// matches and everything under it.
    globs: Vec<String>,
    /// The directories Python imports from ([`IMPORTS`]), folded, `**` for any: in each, the
    /// module names of [`JudgeSurface::modules`].
    imports: BTreeSet<String>,
    /// The module names no step may put in an import directory: the case's ([`Modules`]), the
    /// built-in lists until it is given them.
    modules: Modules,
    /// Why the test command's script fails closed, as it says it ([`FAILS_CLOSED_MARK`]): named
    /// in every refusal of the surface while there is one.
    closed: Vec<String>,
    /// Each top-level module the tree held when the surface was read, as `<dir>/<name>` folded
    /// (`/<name>` at the root): a name of [`Modules::names`] it already held there is the
    /// project's own.
    provided: BTreeSet<String>,
    /// The tree's directories on `PATH` ([`ON_PATH`]), folded: every executable entry directly
    /// in each, since a lookup by name finds nothing else.
    on_path: BTreeSet<String>,
    /// The entries directly in a directory on `PATH` that were executable when the surface was
    /// read, folded; one executable now is guarded besides.
    on_path_before: BTreeSet<String>,
    /// While a line fails closed ([`PROGRAMS`]): the tree's programs as it held them when the
    /// surface was read, folded; every file that is a program now is guarded besides.
    programs: Option<BTreeSet<String>>,
    /// The tree, where what a file is now is read.
    tree: PathBuf,
}

/// The top-level modules `paths` (relative to the tree) hold in each directory, as
/// `<dir>/<name>` folded ([`JudgeSurface::provided`]): every directory on a path's way by its
/// name, and the file by the module it loads as ([`module_of`]).
fn provided_by<'p>(paths: impl Iterator<Item = &'p str>) -> BTreeSet<String> {
    let mut provided = BTreeSet::new();
    for path in paths {
        let folded = fold(path);
        let parts: Vec<&str> = folded.split('/').collect();
        for k in 0..parts.len() {
            let name = if k + 1 == parts.len() {
                module_of(parts[k])
            } else {
                Some(parts[k])
            };
            if let Some(name) = name {
                provided.insert(format!("{}/{name}", parts[..k].join("/")));
            }
        }
    }
    provided
}

/// The paths git tracks or would add in `tree`, the cases left out; none when git cannot list
/// them.
fn listed_files(tree: &Path) -> Vec<String> {
    let Ok(listed) = crate::git::run(
        tree,
        &[
            "ls-files",
            "-z",
            "--cached",
            "--others",
            "--exclude-standard",
        ],
    ) else {
        return Vec::new();
    };
    listed
        .split('\0')
        .filter(|path| !path.is_empty() && !crate::case::is_under_cases(path))
        .map(str::to_owned)
        .collect()
}

/// Whether the entry at `full` is executable: a file with an exec bit, or a link (which a lookup
/// on `PATH` follows). Nothing there is not.
fn is_executable_entry(full: &Path) -> bool {
    std::fs::symlink_metadata(full)
        .is_ok_and(|meta| meta.file_type().is_symlink() || (meta.is_file() && is_executable(&meta)))
}

/// Whether a `*.pth` file at the folded path parts `parts` is read at start-up: at the tree's
/// root, or directly in a `site-packages` or `dist-packages` directory (a virtual environment's
/// in the tree), which Python reads as a site directory.
fn site_dir(parts: &[&str]) -> bool {
    match parts {
        [_] => true,
        [.., parent, _] => *parent == "site-packages" || *parent == "dist-packages",
        [] => false,
    }
}

/// Whether the directory glob `dir` (`""` the tree's root, `**` any) matches the path parts
/// `parts` of a directory.
fn dir_matches(dir: &str, parts: &[&str]) -> bool {
    if dir.is_empty() {
        return parts.is_empty();
    }
    glob_matches(&dir.split('/').collect::<Vec<_>>(), parts)
}

/// The programs of `tree` ([`is_program`]): the files git tracks or would add, folded, the
/// cases left out.
fn programs_in(tree: &Path) -> BTreeSet<String> {
    listed_files(tree)
        .iter()
        .filter(|path| is_program(&tree.join(path)))
        .map(|path| fold(path))
        .collect()
}

/// The repository's own list of judge-surface path globs, which extends the default and never
/// shrinks it: read at the case's base revision, never from the tree a step edits, and itself on
/// the surface.
pub const JUDGE_LIST: &str = ".engineering/judge-surface.yaml";

/// The globs of a judge-surface list: a YAML sequence of path globs relative to the tree
/// (`*` and `?` within a part, `**` across parts), each matched whatever its case. Anything else
/// (not a sequence of strings, an empty glob, an absolute one, one with `..` or a backslash) is
/// the error: the caller fails closed.
pub fn parse_judge_list(text: &str) -> std::result::Result<Vec<String>, String> {
    let globs: Vec<String> = serde_yaml_ng::from_str(text)
        .map_err(|error| format!("{JUDGE_LIST} is not a list of path globs: {error}"))?;
    globs
        .into_iter()
        .map(|glob| {
            let trimmed = glob.trim().trim_end_matches('/');
            if trimmed.is_empty()
                || trimmed.starts_with('/')
                || trimmed.contains('\\')
                || trimmed
                    .split('/')
                    .any(|part| part == ".." || part.is_empty())
            {
                return Err(format!(
                    "{JUDGE_LIST} holds `{glob}`, which is no path glob relative to the tree"
                ));
            }
            Ok(fold(trimmed))
        })
        .collect()
}

/// The judge-surface list at the commit `revision` ([`JUDGE_LIST`]): none when the commit has no
/// such file; the inner error when it does not parse ([`parse_judge_list`]).
pub fn judge_list_at(
    tree: &Path,
    revision: &str,
) -> Result<std::result::Result<Vec<String>, String>> {
    let spec = format!("{revision}:./{JUDGE_LIST}");
    if crate::git::run(tree, &["cat-file", "-e", &spec]).is_err() {
        return Ok(Ok(Vec::new()));
    }
    let text = crate::git::blob(tree, &spec)?;
    Ok(String::from_utf8(text)
        .map_err(|_| format!("{JUDGE_LIST} is not UTF-8"))
        .and_then(|text| parse_judge_list(&text)))
}

/// Whether `path`, relative to the tree as git names it, is matched whole by the path glob
/// `glob` ([`parse_judge_list`]'s dialect: `*` and `?` within a part, `**` across parts), byte
/// for byte and case and all: an allow-list's match, which admits nothing a fold would add and
/// nothing under a path it names but does not match.
#[must_use]
pub fn glob_admits(glob: &str, path: &str) -> bool {
    let glob: Vec<&str> = glob.split('/').collect();
    let path: Vec<&str> = path.split('/').collect();
    glob_matches(&glob, &path)
}

/// Whether the part `name` matches the glob part `glob`: `*` any run of characters, `?` one.
fn part_matches(glob: &[char], name: &[char]) -> bool {
    match glob.split_first() {
        None => name.is_empty(),
        Some(('*', rest)) => (0..=name.len()).any(|skip| part_matches(rest, &name[skip..])),
        Some(('?', rest)) => !name.is_empty() && part_matches(rest, &name[1..]),
        Some((c, rest)) => name.first() == Some(c) && part_matches(rest, &name[1..]),
    }
}

/// Whether the path parts `path` lead towards what the glob parts `glob` can match: each part of
/// the path matches the glob's part at its place, up to the path's end, or the glob reaches a
/// `**` first. A link at such a path is on the way to what the glob guards.
fn glob_towards(glob: &[&str], path: &[&str]) -> bool {
    match (glob.split_first(), path.split_first()) {
        (_, None) => true,
        (None, Some(_)) => false,
        (Some((&"**", _)), Some(_)) => true,
        (Some((part, glob)), Some((name, path))) => {
            part_matches(
                &part.chars().collect::<Vec<_>>(),
                &name.chars().collect::<Vec<_>>(),
            ) && glob_towards(glob, path)
        }
    }
}

/// Whether the path parts `path` match the glob parts `glob`, `**` matching any number of parts.
fn glob_matches(glob: &[&str], path: &[&str]) -> bool {
    match glob.split_first() {
        None => path.is_empty(),
        Some((&"**", rest)) => {
            glob_matches(rest, path) || (!path.is_empty() && glob_matches(glob, &path[1..]))
        }
        Some((part, rest)) => {
            !path.is_empty()
                && part_matches(
                    &part.chars().collect::<Vec<_>>(),
                    &path[0].chars().collect::<Vec<_>>(),
                )
                && glob_matches(rest, &path[1..])
        }
    }
}

impl JudgeSurface {
    /// The surface of a run whose validators run `runs` ([`Validator::runs`]) in `tree`.
    #[must_use]
    pub fn new(runs: &[String], tree: &Path) -> Self {
        // The marks first: the programs, the import directories and the directories on `PATH`.
        // What [`scripts_of`] names with a wildcard (a module's package, `pkg/**`) is guarded
        // as a glob, everything under it with it; the rest each by itself.
        let mut imports = BTreeSet::new();
        let mut on_path = BTreeSet::new();
        let mut programs = false;
        let mut closed = Vec::new();
        let mut rest = Vec::new();
        for path in scripts_of(runs, tree) {
            if path == PROGRAMS {
                programs = true;
            } else if path.starts_with(PROGRAM_MARK) {
                // Read at case open ([`Modules::ask`]), guarded by nothing.
            } else if path.starts_with(OUTSIDE_PATH_MARK) || path.starts_with(OUTSIDE_IMPORTS_MARK)
            {
                // A directory outside the tree ([`Outside`]), guarded by nothing tree-relative.
            } else if let Some(reason) = path.strip_prefix(FAILS_CLOSED_MARK) {
                closed.push(reason.to_owned());
            } else if let Some(dir) = path.strip_suffix(IMPORTS) {
                imports.insert(fold(dir.trim_end_matches('/')));
            } else if let Some(dir) = path.strip_suffix(ON_PATH) {
                on_path.insert(fold(dir.trim_end_matches('/')));
            } else {
                rest.push(path);
            }
        }
        let (globs, scripts): (Vec<String>, Vec<String>) =
            rest.into_iter().partition(|path| path.contains(['*', '?']));
        let mut surface = Self {
            scripts: scripts.into_iter().collect(),
            store: true,
            globs: globs.iter().map(|glob| fold(glob)).collect(),
            imports,
            on_path,
            on_path_before: BTreeSet::new(),
            programs: programs.then(|| programs_in(tree)),
            tree: tree.to_path_buf(),
            modules: Modules::default(),
            closed,
            provided: provided_by(listed_files(tree).iter().map(String::as_str)),
        };
        if !surface.on_path.is_empty() {
            surface.on_path_before = listed_files(tree)
                .iter()
                .map(|path| (path, fold(path)))
                .filter(|(_, folded)| surface.in_path_dir(&folded.split('/').collect::<Vec<_>>()))
                .filter(|(path, _)| is_executable_entry(&tree.join(path)))
                .map(|(_, folded)| folded)
                .collect();
        }
        surface
    }

    /// Whether the folded path parts `parts` name, in a directory Python imports from, a module,
    /// compiled module or package of a name the Python imports from outside the tree
    /// ([`Modules`]), or what is under such a package: a strict name always, any other unless
    /// the tree held a module of that name there when the surface was read.
    fn shadowing(&self, parts: &[&str]) -> bool {
        (0..parts.len()).any(|k| {
            let name = if k + 1 == parts.len() {
                module_of(parts[k])
            } else {
                Some(parts[k])
            };
            let Some(name) = name else {
                return false;
            };
            self.imports.iter().any(|dir| dir_matches(dir, &parts[..k]))
                && (self.modules.strict.contains(name)
                    || (self.modules.names.contains(name)
                        && !self
                            .provided
                            .contains(&format!("{}/{name}", parts[..k].join("/")))))
        })
    }

    /// With the module names `modules`, the case's ([`Modules::of_case`]).
    #[must_use]
    pub fn with_modules(mut self, modules: Modules) -> Self {
        self.modules = modules;
        self
    }

    /// With the top-level modules the tree held taken from `paths` (a commit's files, relative
    /// to the tree) rather than from the tree as it is ([`JudgeSurface::provided`]).
    #[must_use]
    pub fn provided_by<'p>(mut self, paths: impl Iterator<Item = &'p str>) -> Self {
        self.provided = provided_by(paths);
        self
    }

    /// What a refusal that names `touched` adds: why the guard fails closed, while it does (the
    /// script's reasons always, the interpreter's when a Python file is among them); else, when
    /// one of them is a module of such a name in an import directory, where the names came from.
    #[must_use]
    pub fn modules_note(&self, touched: &[String]) -> String {
        let shadowed = touched.iter().any(|path| {
            let folded = fold(path.trim_end_matches('/'));
            self.shadowing(&folded.split('/').collect::<Vec<_>>())
        });
        // What failing closed puts on the surface: a line of the script, every Python file,
        // shell file and program ([`FAIL_CLOSED`]), so its reasons go with every refusal of the
        // surface (a program the step wrote is gone by the time the refusal is written); an
        // interpreter not asked, every Python file.
        let python = touched.iter().any(|path| {
            matches!(
                fold(path.trim_end_matches('/'))
                    .rsplit_once('.')
                    .map(|(_, suffix)| suffix),
                Some("py" | "pyc" | "pth")
            )
        });
        let mut reasons: Vec<&str> = self.closed.iter().map(String::as_str).collect();
        let script_closed = !reasons.is_empty();
        if python {
            reasons.extend(self.modules.fail_closed.as_deref());
        }
        if !reasons.is_empty() {
            let what = if script_closed {
                "every Python file, shell file and program"
            } else {
                "every Python file"
            };
            return format!(
                "; the guard fails closed, {what} of the tree on the surface: {}",
                reasons.join("; ")
            );
        }
        if shadowed {
            format!(
                "; a module name the Python imports from outside the tree, from {}",
                self.modules.described()
            )
        } else {
            String::new()
        }
    }

    /// Whether the folded path parts `parts` name an entry directly in a directory on `PATH`.
    fn in_path_dir(&self, parts: &[&str]) -> bool {
        parts
            .split_last()
            .is_some_and(|(_, parent)| self.on_path.iter().any(|dir| dir_matches(dir, parent)))
    }

    /// Whether `path` is a program a lookup on `PATH` may find: an entry directly in a directory
    /// on `PATH` that was executable when the surface was read or is now (a step that makes one
    /// executable among them).
    fn on_path(&self, path: &str, folded: &str, parts: &[&str]) -> bool {
        self.in_path_dir(parts)
            && (self.on_path_before.contains(folded) || is_executable_entry(&self.tree.join(path)))
    }

    /// Whether `path`, relative to the tree, is in a directory on `PATH`, where an entry a commit
    /// held executable is on the surface: the caller reads the commit's mode.
    #[must_use]
    pub fn in_a_path_dir(&self, path: &str) -> bool {
        let folded = fold(path.trim_end_matches('/'));
        self.in_path_dir(&folded.split('/').collect::<Vec<_>>())
    }

    /// Whether `path` is a program while a line fails closed: a program when the surface was
    /// read, or now ([`is_program`]).
    fn program(&self, path: &str, folded: &str) -> bool {
        self.programs
            .as_ref()
            .is_some_and(|before| before.contains(folded) || is_program(&self.tree.join(path)))
    }

    /// Whether every program of the tree is on the surface ([`PROGRAMS`]): a line of the test
    /// command's script failed closed. What a commit held is the caller's to read.
    #[must_use]
    pub fn guards_programs(&self) -> bool {
        self.programs.is_some()
    }

    /// With the repository's own globs `globs` ([`JUDGE_LIST`]) added to the default.
    #[must_use]
    pub fn with_globs(mut self, globs: Vec<String>) -> Self {
        self.globs.extend(globs);
        self
    }

    /// The surface without the planning store: what decides `tests.run` and `ci.read`. The
    /// store decides no test and no check, and takt writes it on a case's branch itself; a
    /// step is still refused for writing it.
    #[must_use]
    pub fn without_store(mut self) -> Self {
        self.store = false;
        self
    }

    /// Whether `path` is the store's: under `.engineering/`, whatever its case, and not a case's.
    #[must_use]
    pub fn is_store(path: &str) -> bool {
        let path = path.trim_end_matches('/');
        let folded = fold(path);
        (folded == ENGINEERING || folded.starts_with(&format!("{ENGINEERING}/")))
            && !crate::case::is_under_cases(path)
    }

    /// Whether `path`, relative to the tree as git names it, is on the surface.
    #[must_use]
    pub fn contains(&self, path: &str) -> bool {
        let path = path.trim_end_matches('/');
        if crate::case::is_under_cases(path) {
            return false;
        }
        let folded = fold(path);
        let parts: Vec<&str> = folded.split('/').collect();
        let name = parts.last().copied().unwrap_or_default();
        (self.store && Self::is_store(path))
            || folded == fold(JUDGE_LIST)
            || folded == GITLAB_CI
            || folded == GITLAB_DIR
            || folded.starts_with(".gitlab/")
            || folded == ".github"
            || folded.starts_with(".github/")
            || parts.contains(&".cargo")
            || name.starts_with("rust-toolchain")
            || name == "build.rs"
            || PYTHON_FILES.contains(&name)
            || (name.ends_with(".pth") && site_dir(&parts))
            || self.scripts.iter().any(|script| fold(script) == folded)
            || self.globs.iter().any(|glob| {
                let glob: Vec<&str> = glob.split('/').collect();
                (1..=parts.len()).any(|len| glob_matches(&glob, &parts[..len]))
            })
            || self.shadowing(&parts)
            || self.on_path(path, &folded, &parts)
            || self.program(path, &folded)
            || (self.modules.fail_closed.is_some()
                && matches!(
                    name.rsplit_once('.').map(|(_, suffix)| suffix),
                    Some("py" | "pyc" | "pth")
                ))
    }

    /// Whether a validator or the guarded command would read through a link at `link`: the link
    /// is, or is on the way to, the cargo configuration (`.cargo/` at any depth), the CI
    /// workflows (`.github/workflows/`, `.gitlab-ci.yml` and `.gitlab/` at the root), the store's directories and project file (while the
    /// store is on the surface), a manifest, a build script, a toolchain file, a Python start-up
    /// file (`sitecustomize.py`, `usercustomize.py`, a `*.pth` at the root or in a
    /// `site-packages` or `dist-packages` directory), or the test
    /// command's own script, recipe or build-tool configuration. Any other link (an
    /// instructions file under `.github/`) is read by none of them.
    #[must_use]
    pub fn reads_through(&self, link: &str) -> bool {
        let folded = fold(link.trim_end_matches('/'));
        let parts: Vec<&str> = folded.split('/').collect();
        let name = parts.last().copied().unwrap_or_default();
        let is_or_holds = |path: &str| {
            folded == path
                || folded
                    .strip_prefix(path)
                    .is_some_and(|rest| rest.starts_with('/'))
                || path
                    .strip_prefix(folded.as_str())
                    .is_some_and(|rest| rest.starts_with('/'))
        };
        parts.contains(&".cargo")
            || is_or_holds(".github/workflows")
            || is_or_holds(GITLAB_CI)
            || is_or_holds(GITLAB_DIR)
            || (self.store
                && [
                    ".engineering/planning",
                    ".engineering/evidence",
                    ".engineering/project.yaml",
                ]
                .into_iter()
                .any(is_or_holds))
            || name == "cargo.toml"
            || name == "build.rs"
            || name.starts_with("rust-toolchain")
            || PYTHON_FILES.contains(&name)
            || (name.ends_with(".pth") && site_dir(&parts))
            || folded == fold(JUDGE_LIST)
            || self.scripts.iter().any(|script| is_or_holds(&fold(script)))
            // A path the repository's own list (or a module's package) names: the link is on
            // it, or on the way towards it.
            || self.globs.iter().any(|glob| {
                let glob: Vec<&str> = glob.split('/').collect();
                (1..=parts.len()).any(|len| glob_matches(&glob, &parts[..len]))
                    || glob_towards(&glob, &parts)
            })
            // A module of a guarded name in an import directory, an entry in a directory on
            // `PATH`, or the way to either directory.
            || self.shadowing(&parts)
            || self.in_path_dir(&parts)
            || self.imports.iter().chain(&self.on_path).any(|dir| {
                !dir.is_empty() && glob_towards(&dir.split('/').collect::<Vec<_>>(), &parts)
            })
    }

    /// Where the surface is read in `tree`: the guarded names at its root, whatever their case,
    /// each with what is under it; the scripts and build files, each by itself.
    fn roots(&self, tree: &Path) -> Result<Vec<(PathBuf, bool)>> {
        let mut roots = Vec::new();
        for entry in std::fs::read_dir(tree).with_context(|| format!("read {}", tree.display()))? {
            let name = entry?.file_name();
            let Some(name) = name.to_str() else {
                continue;
            };
            let folded = fold(name);
            if [
                ENGINEERING,
                ".cargo",
                ".github",
                GITLAB_CI,
                GITLAB_DIR,
                "build.rs",
            ]
            .contains(&folded.as_str())
                || folded.starts_with("rust-toolchain")
                || PYTHON_FILES.contains(&folded.as_str())
                || folded.ends_with(".pth")
            {
                roots.push((PathBuf::from(name), true));
            }
        }
        roots.extend(
            self.scripts
                .iter()
                .map(|script| (PathBuf::from(script), false)),
        );
        // A glob naming a path outright is read like a root name; one with a wildcard is
        // guarded through git's view of the change.
        roots.extend(
            self.globs
                .iter()
                .filter(|glob| !glob.contains(['*', '?']))
                .map(|glob| (PathBuf::from(glob), true)),
        );
        Ok(roots)
    }

    /// Every file, link and other entry on the surface in `tree`, by path relative to it, as
    /// [`snapshot`] takes them: a link is never followed, and a root whose way crosses a link,
    /// checked component by component, is that link.
    fn snapshot(&self, tree: &Path) -> Result<BTreeMap<PathBuf, Entry>> {
        fn walk(
            tree: &Path,
            at: &Path,
            entries: &mut BTreeMap<PathBuf, Entry>,
            descend: bool,
        ) -> Result<()> {
            if at.to_str().is_some_and(crate::case::is_under_cases) {
                return Ok(());
            }
            let full = tree.join(at);
            let meta = match std::fs::symlink_metadata(&full) {
                Ok(meta) => meta,
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
                Err(error) => {
                    return Err(error).with_context(|| format!("read {}", full.display()));
                }
            };
            let kind = meta.file_type();
            if kind.is_symlink() {
                let target = std::fs::read_link(&full)
                    .with_context(|| format!("read the link {}", full.display()))?;
                entries.insert(at.to_path_buf(), Entry::Link(target));
            } else if kind.is_dir() {
                if descend {
                    for entry in std::fs::read_dir(&full)
                        .with_context(|| format!("read {}", full.display()))?
                    {
                        walk(tree, &at.join(entry?.file_name()), entries, true)?;
                    }
                }
            } else if kind.is_file() {
                let bytes =
                    std::fs::read(&full).with_context(|| format!("read {}", full.display()))?;
                entries.insert(
                    at.to_path_buf(),
                    Entry::File {
                        bytes,
                        executable: is_executable(&meta),
                    },
                );
            } else {
                entries.insert(at.to_path_buf(), Entry::Other(other_kind(kind)));
            }
            Ok(())
        }
        let mut entries = BTreeMap::new();
        for (root, descend) in self.roots(tree)? {
            // The way to a root, component by component from the tree: the first link on it is
            // the entry, and nothing under it is read.
            let mut way = PathBuf::new();
            let mut linked = None;
            for component in root.components() {
                way.push(component);
                if way == root {
                    break;
                }
                if std::fs::symlink_metadata(tree.join(&way))
                    .is_ok_and(|meta| meta.file_type().is_symlink())
                {
                    linked = Some(way.clone());
                    break;
                }
            }
            match linked {
                Some(link) => walk(tree, &link, &mut entries, false)?,
                None => walk(tree, &root, &mut entries, descend)?,
            }
        }
        Ok(entries)
    }

    /// The surface of a run whose validators run `runs`, with the case's module names `modules`,
    /// and what is on it in `tree` now.
    fn read(
        runs: &[String],
        globs: Vec<String>,
        modules: &Modules,
        tree: &Path,
    ) -> Result<(Self, BTreeMap<PathBuf, Entry>)> {
        let surface = Self::new(runs, tree)
            .with_globs(globs)
            .with_modules(modules.clone());
        let kept = surface.snapshot(tree)?;
        Ok((surface, kept))
    }

    /// The links a validator or the guarded command would read through
    /// ([`JudgeSurface::reads_through`]), each with where it leads: on the surface as `kept`
    /// holds it, among the links git tracks, at the root manifest, and a tracked link to a
    /// directory that holds a manifest (a workspace member reached through it). What they guard
    /// would be read wherever they point, inside the tree or out of it, so the step guard takes
    /// no judgement step while there is one.
    fn locking_links(&self, tree: &Path, kept: &BTreeMap<PathBuf, Entry>) -> Result<Vec<String>> {
        let mut links: BTreeMap<String, PathBuf> = kept
            .iter()
            .filter_map(|(path, entry)| match entry {
                Entry::Link(target) => Some((path.display().to_string(), target.clone())),
                _ => None,
            })
            .filter(|(path, _)| self.reads_through(path))
            .collect();
        let tracked = crate::git::run(tree, &["ls-files", "-s", "-z"])?;
        let mut candidates: Vec<String> = tracked
            .split('\0')
            .filter_map(|entry| {
                let (meta, path) = entry.split_once('\t')?;
                meta.starts_with("120000 ").then(|| path.to_owned())
            })
            .collect();
        candidates.push("Cargo.toml".to_owned());
        for path in candidates {
            let full = tree.join(&path);
            if !std::fs::symlink_metadata(&full).is_ok_and(|meta| meta.file_type().is_symlink()) {
                continue;
            }
            if self.reads_through(&path) || full.join("Cargo.toml").exists() {
                let target = std::fs::read_link(&full)
                    .with_context(|| format!("read the link {}", full.display()))?;
                links.insert(path, target);
            }
        }
        Ok(links
            .into_iter()
            .map(|(path, target)| format!("{path} -> {}", target.display()))
            .collect())
    }

    /// Puts the surface in `tree` back to `kept`, where it still differs; through the tree's
    /// real path, so a tree named through a link is not taken for a directory a step replaced.
    fn put_back(&self, tree: &Path, kept: &BTreeMap<PathBuf, Entry>) -> Result<()> {
        let again = differing(kept, &self.snapshot(tree)?);
        if !again.is_empty() {
            let real = std::fs::canonicalize(tree)
                .with_context(|| format!("resolve {}", tree.display()))?;
            restore(&real, &real, kept, &again)?;
        }
        Ok(())
    }
}

/// The files outside the tree that decide how cargo and rustup build and run the tests from it:
/// `.cargo/config` and `.cargo/config.toml` in every directory above the tree, `rust-toolchain`
/// and `rust-toolchain.toml` there, and every `config*` in `$CARGO_HOME` (`~/.cargo` when it is
/// not set), each by its absolute path with its anchor (the directory it is found from) and what
/// is at it, `None` for nothing; each part of the way between the anchor and the file (`.cargo`,
/// `$CARGO_HOME`) is a path of its own, so a link there is seen as a link. Read around each
/// judgement step and each observation: a step that changed one is refused and the file put
/// back, and no validator records while one differs from what the run found when it began. A
/// file behind a link is read through it, so its change is seen, but nothing is ever written or
/// removed through a link. Confining the step to the tree is not this.
///
/// Beside them, the programs a command may find by name ([`program_dirs`]): every directory on
/// the `PATH` the test command inherits and on the shell's default `PATH` a gate's command has,
/// every directory outside the tree the script itself prepends to `PATH`
/// ([`outside_path_dirs`]), `$CARGO_HOME/bin` and every `bin` of a rustup toolchain, that the
/// user can write (one that does not exist yet when its nearest existing directory can be
/// written), each directory by itself and each executable file and link directly in it; one too
/// large to keep is its [`fingerprint`]. A directory the user cannot write no step can change.
///
/// And what Python loads code from outside the tree ([`outside_import_dirs`]): each `PYTHONPATH`
/// entry and `PYTHONHOME`, `PYTHONUSERBASE`, `PYTHONPYCACHEPREFIX` or `PYTHONSTARTUP` value the
/// script sets or takt inherits, and every `sys.path` entry, site directory and the user site of
/// each Python asked at case open ([`Modules::sites`]), that the user can write by the same
/// rule: everything under each, the tree left out, each entry by its [`fingerprint`].
///
/// takt never deletes or rewrites a file outside the tree except the named config files the
/// step guard snapshots; any outside change still stops the run and names it. So a changed
/// configuration file is written back as it was and the run stops naming it, while a change in
/// a program or import directory (a step's, or another session's install) is left exactly as it
/// is and the run stops naming it ([`Self::put_back`]).
#[derive(Debug, Clone, PartialEq, Eq)]
struct Outside(BTreeMap<PathBuf, (PathBuf, Option<Entry>, bool)>);

/// Why [`Outside`] reads a path: a configuration file cargo or rustup reads, which takt puts
/// back; a program directory's, or what Python imports from, which it never writes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Held {
    Config,
    Program,
    Import,
}

/// The directories outside the tree a run's test command reaches beside what every run reads
/// ([`Outside`]): read when the case opens, the same around every step.
#[derive(Debug, Clone, Default)]
struct OutsideDirs {
    /// The directories outside the tree the script prepends to `PATH` ([`outside_path_dirs`]).
    programs: Vec<PathBuf>,
    /// What Python loads code from outside the tree ([`outside_import_dirs`]).
    imports: Vec<PathBuf>,
}

impl OutsideDirs {
    /// What the test command `runs` in `tree` reaches, with the case's `modules`.
    fn of(runs: &[String], tree: &Path, modules: &Modules) -> Self {
        Self {
            programs: outside_path_dirs(runs, tree),
            imports: outside_import_dirs(runs, tree, modules),
        }
    }
}

/// The shell's default `PATH`, which a gate's command has when its binding sets none.
const SHELL_PATH: &[&str] = &[
    "/usr/local/sbin",
    "/usr/local/bin",
    "/usr/sbin",
    "/usr/bin",
    "/sbin",
    "/bin",
];

/// Whether the user can write the directory `dir`, which exists.
fn writable(dir: &Path) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStrExt as _;
        let Ok(path) = std::ffi::CString::new(dir.as_os_str().as_bytes()) else {
            return false;
        };
        // SAFETY: `path` is a valid NUL-terminated string that outlives the call.
        unsafe { libc::access(path.as_ptr(), libc::W_OK) == 0 }
    }
    #[cfg(not(unix))]
    {
        std::fs::metadata(dir).is_ok_and(|meta| !meta.permissions().readonly())
    }
}

/// The directories outside `tree` a command may find a program in by its name, which the user
/// can write, each with the path it is read at and its anchor: an existing one at its real path
/// (a linked directory resolved), anchored at its parent; one that does not exist yet at its
/// nearest existing directory's real path joined with the rest, anchored there, with every
/// directory between, when that existing directory can be written. Which directories: every
/// absolute entry of `PATH`, the shell's default `PATH` ([`SHELL_PATH`]), `$CARGO_HOME/bin`
/// (`~/.cargo/bin`) and `$RUSTUP_HOME/toolchains/*/bin` (`~/.rustup`).
fn program_dirs(tree: &Path) -> Vec<(PathBuf, PathBuf)> {
    let home = std::env::var_os("HOME").map(PathBuf::from);
    let mut dirs: Vec<PathBuf> = std::env::var_os("PATH")
        .map(|path| std::env::split_paths(&path).collect())
        .unwrap_or_default();
    dirs.extend(SHELL_PATH.iter().map(PathBuf::from));
    let cargo_home = std::env::var_os("CARGO_HOME")
        .map(PathBuf::from)
        .or_else(|| home.as_ref().map(|home| home.join(".cargo")));
    dirs.extend(cargo_home.map(|cargo| cargo.join("bin")));
    let rustup_home = std::env::var_os("RUSTUP_HOME")
        .map(PathBuf::from)
        .or_else(|| home.as_ref().map(|home| home.join(".rustup")));
    if let Some(Ok(toolchains)) =
        rustup_home.map(|rustup| std::fs::read_dir(rustup.join("toolchains")))
    {
        dirs.extend(
            toolchains
                .flatten()
                .map(|toolchain| toolchain.path().join("bin")),
        );
    }
    writable_outside(tree, dirs, false)
}

/// Of `dirs`, absolute, the ones outside `tree` the user can write, each with the path it is
/// read at and its anchor, as [`program_dirs`] resolves them: an existing one at its real path,
/// anchored at its parent; one that does not exist yet at its nearest existing directory's real
/// path joined with the rest, anchored there, with every directory between, when that
/// directory can be written. An existing file (a zip archive on `PYTHONPATH`) is kept with
/// `files`, passed over without.
fn writable_outside(
    tree: &Path,
    dirs: impl IntoIterator<Item = PathBuf>,
    files: bool,
) -> Vec<(PathBuf, PathBuf)> {
    let roots: Vec<PathBuf> = [
        std::path::absolute(tree).ok(),
        std::fs::canonicalize(tree).ok(),
    ]
    .into_iter()
    .flatten()
    .collect();
    let mut found = BTreeMap::new();
    for dir in dirs.into_iter().filter(|dir| dir.is_absolute()) {
        // The nearest existing directory, at its real path, and what is below it.
        let mut existing = dir.as_path();
        let mut below = Vec::new();
        let real = loop {
            if let Ok(real) = std::fs::canonicalize(existing) {
                break Some(real);
            }
            match (existing.parent(), existing.file_name()) {
                (Some(parent), Some(name)) => {
                    below.push(name.to_owned());
                    existing = parent;
                }
                _ => break None,
            }
        };
        let Some(real) = real else {
            continue;
        };
        let kept = real.is_dir() || (files && below.is_empty() && real.is_file());
        if !kept || !writable(&real) {
            continue;
        }
        let whole: PathBuf = below
            .iter()
            .rev()
            .fold(real.clone(), |path, name| path.join(name));
        if roots.iter().any(|root| whole.starts_with(root)) {
            continue;
        }
        if below.is_empty() {
            let anchor = real
                .parent()
                .map_or_else(|| real.clone(), Path::to_path_buf);
            found.insert(real, anchor);
        } else {
            let mut path = real.clone();
            for name in below.iter().rev() {
                path.push(name);
                found.insert(path.clone(), real.clone());
            }
        }
    }
    found.into_iter().collect()
}

impl Outside {
    /// The paths [`Outside`] reads for `tree`, each with its anchor and whether it is a program
    /// directory's (one takt never writes to, [`Self::put_back`]). The configuration files cargo
    /// and rustup read: the directory above the tree it is in, or the parent of `$CARGO_HOME`;
    /// every part between the anchor and the path (`.cargo`, `$CARGO_HOME` itself) is a path of
    /// its own too, so a link made there is seen as the link it is. The program directories: the
    /// ones [`program_dirs`] finds and `extra`'s (the outside directories the script prepends to
    /// `PATH`), each with the executable files and links directly in them. What Python imports
    /// from outside the tree (`extra`'s, [`OutsideDirs::imports`]) that the user can write
    /// ([`writable_outside`]): each directory with everything under it, a link not followed.
    fn paths(tree: &Path, extra: &OutsideDirs) -> Vec<(PathBuf, PathBuf, Held)> {
        // Each path to its (anchor, what holds it); a configuration path wins a collision, and
        // a program's an import's.
        let mut paths: BTreeMap<PathBuf, (PathBuf, Held)> = BTreeMap::new();
        let config =
            |paths: &mut BTreeMap<PathBuf, (PathBuf, Held)>, path: PathBuf, anchor: PathBuf| {
                paths.insert(path, (anchor, Held::Config));
            };
        let roots = [
            std::path::absolute(tree).ok(),
            std::fs::canonicalize(tree).ok(),
        ];
        for root in roots.iter().flatten() {
            for above in root.ancestors().skip(1) {
                for name in [
                    ".cargo",
                    ".cargo/config",
                    ".cargo/config.toml",
                    "rust-toolchain",
                    "rust-toolchain.toml",
                ] {
                    config(&mut paths, above.join(name), above.to_path_buf());
                }
            }
        }
        let home = std::env::var_os("CARGO_HOME")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".cargo")));
        if let Some(home) = home
            && let Some(anchor) = home.parent()
        {
            config(&mut paths, home.clone(), anchor.to_path_buf());
            config(&mut paths, home.join("config"), anchor.to_path_buf());
            config(&mut paths, home.join("config.toml"), anchor.to_path_buf());
            if !std::fs::symlink_metadata(&home).is_ok_and(|meta| meta.file_type().is_symlink())
                && let Ok(entries) = std::fs::read_dir(&home)
            {
                for entry in entries.flatten() {
                    if entry.file_name().to_string_lossy().starts_with("config") {
                        config(&mut paths, entry.path(), anchor.to_path_buf());
                    }
                }
            }
        }
        // The programs a lookup by name may find: each directory, and each executable file and
        // link directly in it. A configuration path is not re-tagged as a program's.
        let program =
            |paths: &mut BTreeMap<PathBuf, (PathBuf, Held)>, path: PathBuf, anchor: PathBuf| {
                paths.entry(path).or_insert((anchor, Held::Program));
            };
        let extra_dirs = extra.programs.iter().filter_map(|dir| {
            let real = std::fs::canonicalize(dir).ok()?;
            if !real.is_dir() || roots.iter().flatten().any(|root| real.starts_with(root)) {
                return None;
            }
            let anchor = real
                .parent()
                .map(Path::to_path_buf)
                .unwrap_or_else(|| real.clone());
            Some((real, anchor))
        });
        let dirs = program_dirs(tree).into_iter().chain(extra_dirs);
        for (dir, anchor) in dirs {
            if let Ok(entries) = std::fs::read_dir(&dir) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if std::fs::symlink_metadata(&path).is_ok_and(|meta| {
                        meta.file_type().is_symlink() || (meta.is_file() && is_executable(&meta))
                    }) {
                        program(&mut paths, path, dir.clone());
                    }
                }
            }
            program(&mut paths, dir, anchor);
        }
        // What Python imports from: everything under each directory, by itself, but the tree
        // (an entry above it, `..`), which the step guard reads; a link is an entry and is not
        // followed.
        fn under(
            paths: &mut BTreeMap<PathBuf, (PathBuf, Held)>,
            path: &Path,
            anchor: &Path,
            trees: &[PathBuf],
        ) {
            if trees.iter().any(|tree| path.starts_with(tree)) {
                return;
            }
            paths
                .entry(path.to_path_buf())
                .or_insert((anchor.to_path_buf(), Held::Import));
            if std::fs::symlink_metadata(path).is_ok_and(|meta| meta.is_dir())
                && let Ok(entries) = std::fs::read_dir(path)
            {
                for entry in entries.flatten() {
                    under(paths, &entry.path(), path, trees);
                }
            }
        }
        let trees: Vec<PathBuf> = roots.iter().flatten().cloned().collect();
        for (dir, anchor) in writable_outside(tree, extra.imports.iter().cloned(), true) {
            under(&mut paths, &dir, &anchor, &trees);
        }
        paths
            .into_iter()
            .map(|(path, (anchor, held))| (path, anchor, held))
            .collect()
    }

    /// The first part between `anchor` and `path`, the path itself left out, that is a link:
    /// checked part by part with `symlink_metadata`, never followed.
    fn link_on_the_way(anchor: &Path, path: &Path) -> Option<PathBuf> {
        let below = path.strip_prefix(anchor).ok()?;
        let mut way = anchor.to_path_buf();
        let parts: Vec<_> = below.components().collect();
        for part in parts.iter().take(parts.len().saturating_sub(1)) {
            way.push(part);
            if std::fs::symlink_metadata(&way).is_ok_and(|meta| meta.file_type().is_symlink()) {
                return Some(way);
            }
        }
        None
    }

    /// What is at each of the paths now. A path whose way crosses a link is read through it,
    /// so a change there is seen, and is never written or removed through it ([`Self::put_back`]).
    /// What Python imports from is read by its [`fingerprint`] alone, since takt never puts it
    /// back.
    fn read(tree: &Path, extra: &OutsideDirs) -> Result<Self> {
        let mut entries = BTreeMap::new();
        for (path, anchor, held) in Self::paths(tree, extra) {
            let entry = Self::entry_at_with(&path, held != Held::Import)?;
            entries.insert(path, (anchor, entry, held != Held::Config));
        }
        Ok(Self(entries))
    }

    /// Takes each of the paths `made`, exactly those, which takt itself made or wrote for the
    /// transcripts (the transcripts directory's way when the run opens, [`Run::transcripts_in`];
    /// a step's transcript and the directories made for it, [`crate::harness::keep_transcript`]),
    /// as it is now, as if this had read it so: a directory as one, a file by its
    /// [`fingerprint`]. A path this reads already takes what is there now, when that is a
    /// directory; a path this does not hold is taken only where [`Self::read`] would read it,
    /// under a directory Python imports from (`PYTHONUSERBASE` at `~/.local`). Anything else is
    /// passed over: a path this neither reads nor would read, one that cannot be read, one no
    /// longer as takt made it. What a step writes beside them, under the same directory, is
    /// still charged to it.
    fn took(&mut self, made: &[PathBuf], tree: &Path, extra: &OutsideDirs) {
        let trees: Vec<PathBuf> = [
            std::path::absolute(tree).ok(),
            std::fs::canonicalize(tree).ok(),
        ]
        .into_iter()
        .flatten()
        .collect();
        let imports: Vec<PathBuf> = writable_outside(tree, extra.imports.iter().cloned(), true)
            .into_iter()
            .map(|(dir, _)| dir)
            .collect();
        for path in made {
            if let Some((_, entry, _)) = self.0.get_mut(path) {
                if let Ok(now @ Some(Entry::Other("directory"))) = Self::entry_at(path) {
                    *entry = now;
                }
                continue;
            }
            let read = !trees.iter().any(|tree| path.starts_with(tree))
                && imports.iter().any(|dir| path.starts_with(dir));
            if let (true, Some(anchor), Ok(now @ Some(_))) =
                (read, path.parent(), Self::entry_at_with(path, false))
            {
                self.0
                    .insert(path.clone(), (anchor.to_path_buf(), now, true));
            }
        }
    }

    /// What is at `path` now, `None` for nothing; a link is not followed.
    fn entry_at(path: &Path) -> Result<Option<Entry>> {
        Self::entry_at_with(path, true)
    }

    /// What is at `path` now ([`Self::entry_at`]); a file by its [`fingerprint`] alone without
    /// `bytes`.
    fn entry_at_with(path: &Path, bytes: bool) -> Result<Option<Entry>> {
        Ok(match std::fs::symlink_metadata(path) {
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
            // A path whose way is not a directory (`.cargo` a file) holds nothing.
            Err(error) if error.raw_os_error() == Some(20) => None,
            // One the run may not read is compared by that alone.
            Err(error) if error.kind() == std::io::ErrorKind::PermissionDenied => {
                Some(Entry::Other("unreadable"))
            }
            Err(error) => {
                return Err(error).with_context(|| format!("read {}", path.display()));
            }
            Ok(meta) if meta.file_type().is_symlink() => Some(Entry::Link(
                std::fs::read_link(path)
                    .with_context(|| format!("read the link {}", path.display()))?,
            )),
            Ok(meta) if meta.is_file() && (!bytes || meta.len() > OUTSIDE_BYTES_AT_MOST) => {
                Some(Entry::Large(fingerprint(&meta)))
            }
            Ok(meta) if meta.is_file() => Some(Entry::File {
                bytes: std::fs::read(path).with_context(|| format!("read {}", path.display()))?,
                executable: is_executable(&meta),
            }),
            Ok(meta) if meta.is_dir() => Some(Entry::Other("directory")),
            Ok(meta) => Some(Entry::Other(other_kind(meta.file_type()))),
        })
    }

    /// The paths at which `now` differs from this, in path order: a part of the way (`.cargo`)
    /// before what is under it. A path this holds and `now` does not list (a directory that
    /// did not exist, `~/.local` on the way to `~/.local/bin`, now made; a program that is no
    /// longer executable) is compared with what is at it now. A directory made where there was
    /// nothing, on the way to other paths this or `now` reads (`~/.cargo`, which cargo makes for
    /// its cache, before `~/.cargo/bin` and `~/.cargo/config.toml`), holds nothing read of its
    /// own: it differs only with a path under it that differs.
    fn differing(&self, now: &Self) -> Vec<PathBuf> {
        use std::ops::Bound::{Excluded, Unbounded};
        let entry = |outside: &Self, path: &PathBuf| {
            outside.0.get(path).and_then(|(_, entry, _)| entry.clone())
        };
        let current = |path: &PathBuf| match now.0.get(path) {
            Some((_, entry, _)) => entry.clone(),
            None => Self::entry_at(path).unwrap_or(Some(Entry::Other("unreadable"))),
        };
        // Whether the first path after `path` in path order is under it: a path's descendants
        // follow it directly.
        let read_under = |outside: &Self, path: &PathBuf| {
            outside
                .0
                .range::<PathBuf, _>((Excluded(path), Unbounded))
                .next()
                .is_some_and(|(next, _)| next.starts_with(path))
        };
        let differ: BTreeSet<PathBuf> = self
            .0
            .keys()
            .chain(now.0.keys())
            .filter(|path| entry(self, path) != current(path))
            .cloned()
            .collect();
        differ
            .iter()
            .filter(|path| {
                let made_on_the_way = entry(self, path).is_none()
                    && current(path) == Some(Entry::Other("directory"))
                    && (read_under(self, path) || read_under(now, path));
                !made_on_the_way
                    || differ
                        .range::<PathBuf, _>((Excluded(*path), Unbounded))
                        .next()
                        .is_some_and(|next| next.starts_with(path))
            })
            .cloned()
            .collect()
    }

    /// Puts each configuration path where the tree's outside now differs from this back, in path
    /// order: what was created is removed (a link as the link itself), what was changed or
    /// removed is written again (a link made again, a directory made again empty). A program
    /// directory's content is never touched: a change there, a step's or another session's,
    /// stops the run and is left exactly as it is, named for the refusal. Nothing is ever written
    /// or removed through a link either: a path whose way crosses one, checked part by part, is
    /// left as it is and answered, and so is a file too large to have been kept ([`Entry::Large`])
    /// that was changed or removed. A path only `now` holds (a program made in a directory on
    /// `PATH`) is anchored as `now` anchors it.
    fn put_back(&self, tree: &Path, extra: &OutsideDirs) -> Result<Vec<PathBuf>> {
        let mut left = Vec::new();
        let now = Self::read(tree, extra)?;
        for path in self.differing(&now) {
            let (anchor, is_program) = match self.0.get(&path).or_else(|| now.0.get(&path)) {
                Some((anchor, _, is_program)) => (anchor.clone(), *is_program),
                None => continue,
            };
            // A program directory's content is never written or removed by takt.
            if is_program {
                left.push(path);
                continue;
            }
            if Self::link_on_the_way(&anchor, &path).is_some()
                || matches!(self.0.get(&path), Some((_, Some(Entry::Large(_)), _)))
            {
                left.push(path);
                continue;
            }
            if let Ok(meta) = std::fs::symlink_metadata(&path) {
                if meta.is_dir() {
                    std::fs::remove_dir_all(&path)
                } else {
                    std::fs::remove_file(&path)
                }
                .with_context(|| format!("remove {}", path.display()))?;
            }
            match self.0.get(&path).and_then(|(_, entry, _)| entry.clone()) {
                Some(Entry::File { bytes, executable }) => {
                    if let Some(parent) = path.parent() {
                        std::fs::create_dir_all(parent)
                            .with_context(|| format!("create {}", parent.display()))?;
                    }
                    put_back_file(&path, &bytes, executable)?;
                }
                Some(Entry::Link(target)) => make_link(&target, &path)?,
                Some(Entry::Other("directory")) => std::fs::create_dir_all(&path)
                    .with_context(|| format!("create {}", path.display()))?,
                Some(Entry::Other(_) | Entry::Large(_)) | None => {}
            }
        }
        Ok(left)
    }
}

/// How a refusal names the paths outside the tree that differ.
fn outside_named(paths: &[PathBuf]) -> String {
    paths
        .iter()
        .map(|path| path.display().to_string())
        .collect::<Vec<_>>()
        .join(", ")
}

/// A key path of a manifest read strictly: bare keys (`A-Za-z0-9_-`) and quoted keys holding no
/// escape, joined by dots with optional white space; `None` for anything else, an escape in a
/// key among it, which the scanner cannot read with certainty.
fn strict_key(key: &str) -> Option<Vec<String>> {
    let mut parts = Vec::new();
    let mut chars = key.chars().peekable();
    let skip = |chars: &mut std::iter::Peekable<std::str::Chars<'_>>| {
        while chars.peek().is_some_and(|c| *c == ' ' || *c == '\t') {
            chars.next();
        }
    };
    loop {
        skip(&mut chars);
        let mut part = String::new();
        match chars.peek().copied() {
            Some(quote @ ('"' | '\'')) => {
                chars.next();
                loop {
                    match chars.next()? {
                        c if c == quote => break,
                        '\\' | '\n' | '\r' => return None,
                        c => part.push(c),
                    }
                }
            }
            Some(c) if c.is_ascii_alphanumeric() || c == '_' || c == '-' => {
                while let Some(c) = chars.peek().copied() {
                    if c.is_ascii_alphanumeric() || c == '_' || c == '-' {
                        part.push(c);
                        chars.next();
                    } else {
                        break;
                    }
                }
            }
            _ => return None,
        }
        parts.push(part);
        skip(&mut chars);
        match chars.next() {
            None => return Some(parts),
            Some('.') => {}
            Some(_) => return None,
        }
    }
}

/// The kind of string a manifest value is inside.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Quoted {
    Basic,
    Literal,
    MultiBasic,
    MultiLiteral,
}

/// A manifest value read strictly, line by line: strings of the four kinds, arrays and inline
/// tables, bare scalars. Anything the scanner cannot read with certainty makes it uncertain: an
/// escape inside an array or inline table (it could spell a key), any `\` in a multi-line basic
/// string, a multi-line string whose delimiter is followed by another quote, a basic or literal
/// string left open at the end of a line, a bracket closed that was not open, a character a
/// value does not hold, anything after a complete value but a comment.
#[derive(Default)]
struct StrictValue {
    text: String,
    quoted: Option<Quoted>,
    depth: i64,
    ended: bool,
    uncertain: bool,
}

impl StrictValue {
    /// Reads `line`; whether the value is complete after it.
    fn line(&mut self, line: &str) -> bool {
        let mut rest = line;
        while !rest.is_empty() && !self.uncertain {
            let c = rest.chars().next().unwrap_or_default();
            let width = c.len_utf8();
            match self.quoted {
                Some(Quoted::Basic) => {
                    if c == '\\' {
                        if self.depth > 0 {
                            self.uncertain = true;
                            break;
                        }
                        let escaped: usize = rest.chars().take(2).map(char::len_utf8).sum();
                        self.text.push_str(&rest[..escaped]);
                        rest = &rest[escaped..];
                        continue;
                    }
                    if c == '"' {
                        self.close();
                    }
                }
                Some(Quoted::Literal) => {
                    if c == '\'' {
                        self.close();
                    }
                }
                Some(quoted @ (Quoted::MultiBasic | Quoted::MultiLiteral)) => {
                    let delimiter = if quoted == Quoted::MultiBasic {
                        "\"\"\""
                    } else {
                        "'''"
                    };
                    if quoted == Quoted::MultiBasic && c == '\\' {
                        self.uncertain = true;
                        break;
                    }
                    if rest.starts_with(delimiter) {
                        if rest[3..].starts_with(&delimiter[..1]) {
                            self.uncertain = true;
                            break;
                        }
                        self.text.push_str(delimiter);
                        rest = &rest[3..];
                        self.close();
                        continue;
                    }
                }
                None => {
                    if self.ended && !c.is_whitespace() && c != '#' {
                        self.uncertain = true;
                        break;
                    }
                    if c == '#' {
                        if self.text.trim().is_empty() {
                            self.uncertain = true;
                        }
                        break;
                    }
                    if let Some((quoted, opening)) = [
                        (Quoted::MultiBasic, "\"\"\""),
                        (Quoted::MultiLiteral, "'''"),
                        (Quoted::Basic, "\""),
                        (Quoted::Literal, "'"),
                    ]
                    .into_iter()
                    .find(|(_, opening)| rest.starts_with(opening))
                    {
                        self.text.push_str(opening);
                        rest = &rest[opening.len()..];
                        self.quoted = Some(quoted);
                        continue;
                    }
                    match c {
                        '[' | '{' => self.depth += 1,
                        ']' | '}' => {
                            self.depth -= 1;
                            if self.depth < 0 {
                                self.uncertain = true;
                                break;
                            }
                            if self.depth == 0 {
                                self.ended = true;
                            }
                        }
                        ',' | '=' if self.depth > 0 => {}
                        c if c.is_whitespace() => {
                            if self.depth == 0 && !self.text.trim().is_empty() {
                                self.ended = true;
                            }
                        }
                        c if c.is_ascii_alphanumeric() || "_-+.:".contains(c) => {}
                        _ => {
                            self.uncertain = true;
                            break;
                        }
                    }
                }
            }
            self.text.push_str(&rest[..width]);
            rest = &rest[width..];
        }
        match self.quoted {
            Some(Quoted::Basic | Quoted::Literal) => {
                self.uncertain = true;
                true
            }
            Some(Quoted::MultiBasic | Quoted::MultiLiteral) => {
                self.text.push('\n');
                false
            }
            None => {
                self.text.push(' ');
                self.uncertain || (self.depth == 0 && !self.text.trim().is_empty())
            }
        }
    }

    /// A string closed: at the outermost level, the value is complete.
    fn close(&mut self) {
        self.quoted = None;
        if self.depth == 0 {
            self.ended = true;
        }
    }
}

/// Every entry of the manifest `text`, by its full key path, its value as written with white
/// space outside strings collapsed: a strict scanner of sections and keys, not a TOML parser. An
/// entry of an array of tables (`[[bin]]`) is told apart by its index. A line it cannot read with
/// certainty (a header with anything after it, an escape in a key, an odd string, anything that
/// is no header, key or comment) is the error, by its number and text: the caller refuses.
fn manifest_entries(text: &str) -> std::result::Result<BTreeMap<Vec<String>, String>, String> {
    let mut entries = BTreeMap::new();
    let mut table: Vec<String> = Vec::new();
    let mut arrays: BTreeMap<Vec<String>, usize> = BTreeMap::new();
    let mut pending: Option<(Vec<String>, StrictValue, usize)> = None;
    let uncertain = |number: usize, line: &str| format!("line {number}: {}", line.trim());
    for (index, line) in text.lines().enumerate() {
        let number = index + 1;
        let line = line.strip_suffix('\r').unwrap_or(line);
        if let Some((key, mut value, from)) = pending.take() {
            let complete = value.line(line);
            if value.uncertain {
                return Err(uncertain(number, line));
            }
            if complete {
                entries.insert(key, collapse(&value.text));
            } else {
                pending = Some((key, value, from));
            }
            continue;
        }
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        if let Some(header) = trimmed.strip_prefix("[[") {
            let path = header
                .strip_suffix("]]")
                .and_then(strict_key)
                .ok_or_else(|| uncertain(number, line))?;
            let index = arrays.entry(path.clone()).or_insert(0);
            table = path;
            table.push(index.to_string());
            *index += 1;
            continue;
        }
        if let Some(header) = trimmed.strip_prefix('[') {
            table = header
                .strip_suffix(']')
                .and_then(strict_key)
                .ok_or_else(|| uncertain(number, line))?;
            continue;
        }
        // `key = value`: the first `=` outside a quoted key.
        let mut quote = None;
        let split = trimmed.char_indices().find(|&(_, c)| match (quote, c) {
            (None, '"' | '\'') => {
                quote = Some(c);
                false
            }
            (Some(q), c) if c == q => {
                quote = None;
                false
            }
            (None, '=') => true,
            _ => false,
        });
        let (at, _) = split.ok_or_else(|| uncertain(number, line))?;
        let mut key = table.clone();
        key.extend(strict_key(&trimmed[..at]).ok_or_else(|| uncertain(number, line))?);
        let mut value = StrictValue::default();
        let complete = value.line(&trimmed[at + 1..]);
        if value.uncertain {
            return Err(uncertain(number, line));
        }
        if complete {
            entries.insert(key, collapse(&value.text));
        } else {
            pending = Some((key, value, number));
        }
    }
    if let Some((_, _, from)) = pending {
        return Err(format!("line {from}: a value that does not end"));
    }
    Ok(entries)
}

/// `value` with runs of white space made one space, trimmed.
fn collapse(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Whether the manifest entry at `key`, with `value`, decides how cargo builds or runs the tests:
/// `package.build`, `package.links`, a target's `harness`, any entry of `[profile.*]`,
/// `[patch.*]`, `[replace]`, `[build-dependencies]`, and of `[target.*]` but a target's
/// `dependencies` and `dev-dependencies`; and an inline table that could hold one of those.
fn builds(key: &[String], value: &str) -> bool {
    let parts: Vec<String> = key.iter().map(|part| fold(part)).collect();
    let first = parts.first().map(String::as_str).unwrap_or_default();
    let names = |words: &[&str]| words.iter().any(|word| fold(value).contains(word));
    match first {
        "profile" | "patch" | "replace" | "build-dependencies" | "build_dependencies" => true,
        "target" => !matches!(
            parts.get(2).map(String::as_str),
            Some("dependencies" | "dev-dependencies" | "dev_dependencies")
        ),
        "package" => match parts.get(1).map(String::as_str) {
            Some("build" | "links") => true,
            None => names(&["build", "links"]),
            Some(_) => false,
        },
        "lib" | "bin" | "test" | "bench" | "example" => {
            parts.iter().skip(1).any(|part| part == "harness") || names(&["harness"])
        }
        _ => false,
    }
}

/// The entries that decide how cargo builds or runs the tests ([`builds`]) and differ between the
/// manifests `before` and `after`, by key path: added, changed or removed. A manifest the
/// scanner cannot read with certainty is the error, naming the line: the caller refuses.
pub fn manifest_build_changes(
    before: &str,
    after: &str,
) -> std::result::Result<Vec<String>, String> {
    let before = manifest_entries(before).map_err(|line| format!("before, {line}"))?;
    let after = manifest_entries(after).map_err(|line| format!("after, {line}"))?;
    Ok(before
        .keys()
        .chain(after.keys())
        .filter(|key| {
            let (was, is) = (before.get(*key), after.get(*key));
            was != is
                && (was.is_some_and(|value| builds(key, value))
                    || is.is_some_and(|value| builds(key, value)))
        })
        .map(|key| key.join("."))
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect())
}

/// The build scripts and `links` of each package cargo reads, by manifest path; `None` for a
/// tree with no manifest at its root; the error is cargo's failure.
type CargoBuilds =
    std::result::Result<Option<BTreeMap<String, (Vec<String>, Option<String>)>>, String>;

/// What cargo itself reads as each package's build script and `links`, by manifest path:
/// `cargo metadata --offline --no-deps --format-version 1` run in `tree` as a confined child (no
/// credential, no standard input, a timeout, a bounded output), packages with neither left out.
/// `Ok(None)` when the tree has no manifest at its root; the error is cargo's failure.
fn cargo_builds(tree: &Path) -> CargoBuilds {
    if !std::fs::symlink_metadata(tree.join("Cargo.toml")).is_ok_and(|meta| meta.is_file()) {
        return Ok(None);
    }
    let mut command = crate::confinement::Command::new("cargo");
    command
        .args([
            "metadata",
            "--offline",
            "--no-deps",
            "--format-version",
            "1",
        ])
        .current_dir(tree);
    crate::forge::without_credentials(&mut command, |_| false);
    let limits = crate::forge::Limits {
        timeout: Duration::from_secs(120),
        output: 64 << 20,
    };
    let output = crate::forge::run_child(&mut command, limits)
        .map_err(|error| format!("cargo metadata did not run: {error:#}"))?;
    if !output.success() {
        let said = String::from_utf8_lossy(&output.stderr);
        return Err(format!(
            "cargo metadata exited {}: {}",
            output
                .code
                .map_or("on a signal".to_owned(), |code| code.to_string()),
            said.trim().chars().take(300).collect::<String>()
        ));
    }
    let metadata: serde_json::Value = serde_json::from_slice(&output.stdout)
        .map_err(|error| format!("cargo metadata wrote what is not its JSON: {error}"))?;
    let mut builds = BTreeMap::new();
    for package in metadata["packages"].as_array().into_iter().flatten() {
        let scripts: Vec<String> = package["targets"]
            .as_array()
            .into_iter()
            .flatten()
            .filter(|target| {
                target["kind"]
                    .as_array()
                    .is_some_and(|kinds| kinds.iter().any(|kind| kind == "custom-build"))
            })
            .filter_map(|target| target["src_path"].as_str().map(str::to_owned))
            .collect();
        let links = package["links"].as_str().map(str::to_owned);
        if !scripts.is_empty() || links.is_some() {
            let manifest = package["manifest_path"].as_str().unwrap_or_default();
            builds.insert(manifest.to_owned(), (scripts, links));
        }
    }
    Ok(Some(builds))
}

/// How the build scripts and `links` cargo reads changed across a step, `before` and `after`
/// it ([`cargo_builds`]): each difference named; cargo failing after the step but not before is
/// one.
fn cargo_build_changes(before: &CargoBuilds, after: &CargoBuilds) -> Vec<String> {
    match (before, after) {
        (_, Err(failed)) if before.is_ok() => {
            vec![format!(
                "cargo cannot read the manifests after the step: {failed}"
            )]
        }
        (Err(_), Err(_)) => Vec::new(),
        (before, Ok(after)) => {
            let empty = BTreeMap::new();
            let before = before
                .as_ref()
                .ok()
                .and_then(Option::as_ref)
                .unwrap_or(&empty);
            let after = after.as_ref().unwrap_or(&empty);
            before
                .keys()
                .chain(after.keys())
                .collect::<BTreeSet<_>>()
                .into_iter()
                .filter(|manifest| before.get(*manifest) != after.get(*manifest))
                .map(|manifest| {
                    let shown = |builds: Option<&(Vec<String>, Option<String>)>| {
                        builds.map_or("none".to_owned(), |(scripts, links)| {
                            format!(
                                "build script {}, links {}",
                                if scripts.is_empty() {
                                    "none".to_owned()
                                } else {
                                    scripts.join(" ")
                                },
                                links.as_deref().unwrap_or("none")
                            )
                        })
                    };
                    format!(
                        "cargo reads {manifest}: {} before, {} after",
                        shown(before.get(manifest)),
                        shown(after.get(manifest))
                    )
                })
                .collect()
        }
        (Ok(_), Err(_)) => Vec::new(),
    }
}

/// The text of the manifest at `path`, relative to the tree, in the commit `revision`: empty
/// when the commit has none there.
pub fn manifest_at(tree: &Path, revision: &str, path: &str) -> Result<String> {
    let spec = format!("{revision}:./{path}");
    if crate::git::run(tree, &["cat-file", "-e", &spec]).is_err() {
        return Ok(String::new());
    }
    Ok(String::from_utf8_lossy(&crate::git::blob(tree, &spec)?).into_owned())
}

/// The entries of `pyproject.toml`'s `[tool.pytest*]` tables that differ between `before` and
/// `after`, by key path: pytest's configuration, read by the same strict scanner as a cargo
/// manifest ([`manifest_entries`]). A file the scanner cannot read with certainty is the error,
/// naming the line: the caller refuses.
pub fn pytest_config_changes(
    before: &str,
    after: &str,
) -> std::result::Result<Vec<String>, String> {
    let before = manifest_entries(before).map_err(|line| format!("before, {line}"))?;
    let after = manifest_entries(after).map_err(|line| format!("after, {line}"))?;
    let pytest = |key: &Vec<String>, value: &String| {
        let parts: Vec<String> = key.iter().map(|part| fold(part)).collect();
        match (parts.first().map(String::as_str), parts.get(1)) {
            (Some("tool"), Some(table)) => table.starts_with("pytest"),
            (Some("tool"), None) => fold(value).contains("pytest"),
            _ => false,
        }
    };
    Ok(before
        .keys()
        .chain(after.keys())
        .filter(|key| {
            let (was, is) = (before.get(*key), after.get(*key));
            was != is
                && (was.is_some_and(|value| pytest(key, value))
                    || is.is_some_and(|value| pytest(key, value)))
        })
        .map(|key| key.join("."))
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect())
}

/// Whether `path`, relative to the tree, is a `pyproject.toml`, whatever its case.
#[must_use]
pub fn is_pyproject(path: &str) -> bool {
    fold(path.rsplit('/').next().unwrap_or(path)) == "pyproject.toml"
}

/// Whether `path`, relative to the tree, is a cargo manifest, whatever its case.
#[must_use]
pub fn is_manifest(path: &str) -> bool {
    fold(path.rsplit('/').next().unwrap_or(path)) == "cargo.toml"
}

/// How a refusal names what a step changed on the judge surface: the commit identity file
/// ([`crate::git::COMMIT_IDENTITY_FILE`]) first, as the identity, then the store, as the store's,
/// then the rest.
fn surface_refusal(action: &str, touched: &[String]) -> String {
    let (identity, touched): (Vec<&String>, Vec<&String>) = touched.iter().partition(|path| {
        fold(path.trim_end_matches('/')) == fold(crate::git::COMMIT_IDENTITY_FILE)
    });
    let (store, other): (Vec<&String>, Vec<&String>) = touched
        .into_iter()
        .partition(|path| JudgeSurface::is_store(path));
    let join = |paths: &[&String]| {
        paths
            .iter()
            .map(|path| path.as_str())
            .collect::<Vec<_>>()
            .join(", ")
    };
    let mut parts = Vec::new();
    if !identity.is_empty() {
        parts.push(format!(
            "changed the commit identity file, what decides whose name a candidate leaves the machine under, read at the base and on the judge surface: {}",
            join(&identity)
        ));
    }
    if !store.is_empty() {
        parts.push(format!(
            "wrote into the planning store, which only takt writes, each write its own commit (the take when a case opens, a grant's record, the story's test result and its move to implemented): {}",
            join(&store)
        ));
    }
    if !other.is_empty() {
        parts.push(format!(
            "changed the judge surface, what decides its validators ({JUDGE_SURFACE}): {}",
            join(&other)
        ));
    }
    format!("{action} {}", parts.join("; and "))
}

/// What a judgement step or an observation is checked against and put back to when it is
/// discarded: the checkpoint, the cases roots (`cases_kept`, keyed relative to `tree`), the judge
/// surface and what lies outside the tree, each as it was just before.
struct Kept<'k> {
    checkpoint: &'k crate::git::Checkpoint,
    tree: &'k Path,
    cases_kept: &'k BTreeMap<PathBuf, Entry>,
    surface: &'k JudgeSurface,
    surface_kept: &'k BTreeMap<PathBuf, Entry>,
    outside: &'k Outside,
    outside_dirs: &'k OutsideDirs,
}

/// What discarding a step or an observation did besides putting the tree back: the files inside
/// an ignored directory a harness step rewrote, moved out of the tree ([`crate::git::SetAside`];
/// none for an observation), and the paths outside the tree left as they were, their way
/// crossing a link.
struct Undone {
    aside: crate::git::SetAside,
    left: Vec<PathBuf>,
}

/// Discards everything a judgement step or an observation did, in this order: the reset to the
/// checkpoint; the judge surface put back, which removes a link the step put at `.engineering`
/// as the link itself; the cases directory put back, after the reset, which can remove a case
/// file the step had staged; and what lies outside the tree. Every part is attempted whatever
/// another part's error, and the errors are joined in that order. Last, once every part put
/// back what it holds, the files inside an ignored directory the step rewrote are moved aside
/// ([`crate::git::Checkpoint::set_aside`]): their way runs through the tree, so they move only
/// out of a tree put back whole; when any part failed, none is moved, and the error names them
/// as not moved, with why. Answers what was set aside and the paths outside the tree left as
/// they were, their way crossing a link.
fn undo_step(tree: &Path, kept: &Kept<'_>) -> Result<Undone> {
    let reset = kept
        .checkpoint
        .restore(tree)
        .context("discard the step's change");
    let surface = kept.surface.put_back(tree, kept.surface_kept);
    let cases = snapshot_cases(kept.tree).and_then(|now| {
        let again = differing(kept.cases_kept, &now);
        if again.is_empty() {
            Ok(())
        } else {
            restore(tree, kept.tree, kept.cases_kept, &again)
        }
    });
    let outside = kept.outside.put_back(tree, kept.outside_dirs);
    let (due, reset) = match reset {
        Ok(due) => (Some(due), Ok(())),
        Err(error) => (None, Err(error)),
    };
    let mut errors: Vec<String> = [reset, surface, cases]
        .into_iter()
        .filter_map(|part| part.err().map(|error| format!("{error:#}")))
        .collect();
    let left = outside.unwrap_or_else(|error| {
        errors.push(format!("{error:#}"));
        Vec::new()
    });
    if errors.is_empty() {
        let due = due.unwrap_or_default();
        // The way to the cases directory and to every case is read again first, as a run reads
        // it before it writes there ([`crate::case::CaseDir::check_way`]): past a link or a part that
        // is not a directory, nothing is moved out of the tree, and each file is named with why.
        let aside = match cases_way(tree, kept.cases_kept) {
            Ok(()) => kept.checkpoint.set_aside(tree, &due),
            Err(way) => crate::git::SetAside::not_moved(
                &due,
                &format!("{way:#}; so nothing was moved out of the tree"),
            ),
        };
        // The tree is read once more after the move: a process the step left behind may have
        // written into it, or moved what was set aside back into it, while the files moved.
        // Anything it names is the error, which still says where the files went.
        let after = kept.checkpoint.change(tree).with_context(|| {
            format!(
                "read the tree after the step was discarded{}",
                aside.named()
            )
        })?;
        if !after.paths.is_empty()
            || after.staged
            || after.head.is_some()
            || !after.refs.is_empty()
            || !after.ignored_added.is_empty()
        {
            bail!(
                "the tree could not be put back as it was before the step, as read after the files inside an ignored directory were moved: {after:?}{}",
                aside.named()
            );
        }
        Ok(Undone { aside, left })
    } else {
        // The files that were due to move, or, when the reset itself failed, those the
        // checkpoint sees rewritten now: none is moved out of a tree not put back whole.
        let due = due.unwrap_or_else(|| {
            kept.checkpoint
                .change(tree)
                .map(|change| change.ignored_set_aside)
                .unwrap_or_default()
        });
        let aside = crate::git::SetAside::not_moved(
            &due,
            "the tree could not be put back whole, so nothing was moved out of it",
        );
        bail!("{}{}", errors.join("; and "), aside.named())
    }
}

/// That every part of the way from `tree` down to each cases root that is there, and down to each
/// directory a file of `kept` lies in, is a real directory, none a link, each read with
/// `symlink_metadata` and never followed; the error names the first that is not. The keys of
/// `kept` are relative to `tree` ([`snapshot_cases`]).
fn cases_way(tree: &Path, kept: &BTreeMap<PathBuf, Entry>) -> Result<()> {
    let mut dirs: BTreeSet<PathBuf> = CASE_ROOTS
        .iter()
        .filter(|root| std::fs::symlink_metadata(tree.join(root)).is_ok())
        .map(PathBuf::from)
        .collect();
    for path in kept.keys() {
        let mut at = path.parent();
        while let Some(dir) = at.filter(|dir| !dir.as_os_str().is_empty()) {
            dirs.insert(dir.to_path_buf());
            at = dir.parent();
        }
    }
    for dir in &dirs {
        let full = tree.join(dir);
        no_link_on_the_way(tree, &full)?;
        let meta =
            std::fs::symlink_metadata(&full).with_context(|| format!("read {}", full.display()))?;
        if !meta.is_dir() {
            bail!("{} is not a directory", dir.display());
        }
    }
    Ok(())
}

/// The paths under the cases roots of `tree` that a step or an observation changed, as a refusal
/// names them: each root that is not a directory first (a link, with where it leads, or a file),
/// then each of `paths`, relative to `tree`, one that is a link now with where it leads. Read
/// before the roots are put back.
fn cases_named<'p>(tree: &Path, paths: impl IntoIterator<Item = &'p PathBuf>) -> Vec<String> {
    let link = |at: &Path| {
        std::fs::symlink_metadata(at)
            .ok()
            .filter(|meta| meta.file_type().is_symlink())
            .map(|_| match std::fs::read_link(at) {
                Ok(target) => format!(", replaced by a link to {}", target.display()),
                Err(_) => ", replaced by a link".to_owned(),
            })
    };
    let mut named = Vec::new();
    let mut replaced: Vec<&str> = Vec::new();
    for root in CASE_ROOTS {
        let at = tree.join(root);
        if let Ok(meta) = std::fs::symlink_metadata(&at)
            && !meta.is_dir()
        {
            named.push(format!(
                "{root}{}",
                link(&at).unwrap_or_else(|| ", replaced by a file".to_owned())
            ));
            replaced.push(root);
        }
    }
    named.extend(paths.into_iter().map(|path| {
        let shown = path.display().to_string();
        let under_replaced = replaced.iter().any(|root| path.starts_with(root));
        match (!under_replaced).then(|| link(&tree.join(path))).flatten() {
            Some(link) => format!("{shown}{link}"),
            None => shown,
        }
    }));
    named
}

/// What a refusal says after an untracked `Cargo.lock`: a cargo test command writes one into a
/// repository that commits none, and each of the three ways out is named on the same line.
const CARGO_LOCK_REMEDY: &str = "commit Cargo.lock, or ignore it (.gitignore), or give --test-cmd a command that does not write it";

/// Whether one of the `paths` a step changed is a `Cargo.lock` at any directory level that the
/// repository does not track: git lists it under neither the index nor a commit. The path is
/// looked up literally (`--literal-pathspecs`): a directory named `:` or holding a glob
/// character is no pathspec magic (story:untracked-cargo-lock-remedy).
fn untracked_cargo_lock(tree: &Path, paths: &[String]) -> bool {
    paths.iter().any(|path| {
        Path::new(path)
            .file_name()
            .is_some_and(|name| name == "Cargo.lock")
            && crate::git::run(tree, &["--literal-pathspecs", "ls-files", "-z", "--", path])
                .is_ok_and(|listed| listed.is_empty())
    })
}

/// How a refusal names ignored paths ([`crate::git::paths_named`]).
fn ignored_named(paths: &[std::ffi::OsString]) -> String {
    crate::git::paths_named(paths)
}

/// Why a harness step is refused before it is taken in a run with no set-aside root.
const NO_SET_ASIDE: &str = "no set-aside directory: neither XDG_STATE_HOME nor HOME is set, so a harness step's rewrite of a file inside an ignored directory would have nowhere to go; no judgement step is taken";

/// The set-aside root of a run on `tree` that is given none: [`default_set_aside`], checked
/// ([`crate::git::set_aside_root`]): one that resolves but cannot be used (relative, inside
/// the tree, through a link into it, a file in the way) is the error, and the run does not
/// start. None when nothing names one (neither `XDG_STATE_HOME` nor `HOME` is set): the run
/// opens, its observations and a run with no harness need none, and its first harness step is
/// refused before it is taken ([`NO_SET_ASIDE`]).
fn opening_set_aside(tree: &Path) -> Result<Option<PathBuf>> {
    default_set_aside()
        .map(|dir| crate::git::set_aside_root(&dir, tree).context("the run does not start"))
        .transpose()
}

/// The set-aside root a run on `tree` opens with: the directory `given` names, checked as
/// [`opening_set_aside`] checks the default, when the caller gives one; the default otherwise.
/// A run given a directory reads neither `XDG_STATE_HOME` nor `HOME`.
fn opening_set_aside_given(tree: &Path, given: Option<&Path>) -> Result<Option<PathBuf>> {
    match given {
        Some(dir) => crate::git::set_aside_root(dir, tree)
            .context("the run does not start")
            .map(Some),
        None => opening_set_aside(tree),
    }
}

/// Where discarding a harness step moves the files inside an ignored directory it rewrote,
/// outside every repository and every case directory: `$XDG_STATE_HOME/takt/set-aside`
/// (`~/.local/state/takt/set-aside` without it), state and not cache, since what is moved
/// there may be the operator's own file, kept until the operator removes it. None when
/// neither is set.
#[must_use]
pub fn default_set_aside() -> Option<PathBuf> {
    crate::promotion::state_dir()
        .ok()
        .map(|dir| dir.join("takt").join("set-aside"))
}

/// Where `takt run` keeps each harness step's transcript ([`Run::transcripts_in`]), outside
/// every repository and every case directory: `$XDG_STATE_HOME/takt/transcripts`
/// (`~/.local/state/takt/transcripts` without it). None when neither is set.
#[must_use]
pub fn default_transcripts() -> Option<PathBuf> {
    crate::promotion::state_dir()
        .ok()
        .map(|dir| dir.join("takt").join("transcripts"))
}

/// Where `takt replay` records each replay of a step when it is given no directory
/// ([`crate::replay`]), outside every repository and every case directory:
/// `$XDG_STATE_HOME/takt/replays` (`~/.local/state/takt/replays` without it). None when
/// neither is set.
#[must_use]
pub fn default_replays() -> Option<PathBuf> {
    crate::promotion::state_dir()
        .ok()
        .map(|dir| dir.join("takt").join("replays"))
}

/// What a judgement step at one decision is offered, as [`Run::run`] offers it: the actions,
/// the approval it may request, and the actions among them that record a choice.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Offer {
    /// The actions offered ([`StepContext::offered`]).
    pub offered: Vec<String>,
    /// The approval-required actions it may request ([`StepContext::approvals`]): the gate's,
    /// when it needs an approval at the decision.
    pub approvals: Vec<String>,
    /// Of the offered actions, those that record a choice ([`StepContext::choices`]).
    pub choices: Vec<String>,
}

/// The offer [`Run::run`] makes a judgement step at `decision` of a case under `protocol` with
/// `inputs`, to a harness that keeps state between steps or not (`keeps_state`), with the
/// approval gate `gate`: the admissible actions the harness may take ([`Shape::for_harness`],
/// [`Shape::for_stateless`]), the gate when it is approval-required, and the choices among the
/// offered. A run also leaves out the actions its validators own; every built-in validator's
/// action produces a kind only a validator writes and is left out here too, but a run's own
/// validator of another action is not known from the record, which is why `takt replay`
/// compares the prompt it builds with the recorded one before it runs anything.
pub fn judgement_offer(
    protocol: &LoadedProtocol,
    inputs: &[(String, String)],
    decision: &crate::Decision,
    keeps_state: bool,
    gate: Option<&str>,
) -> Result<Offer> {
    let shape = Shape::of(protocol, inputs)?;
    let offered: Vec<String> = decision
        .admissible()
        .into_iter()
        .filter(|a| shape.for_harness(a))
        .filter(|a| keeps_state || shape.for_stateless(a))
        .collect();
    let approvals = decision
        .approval_required()
        .into_iter()
        .filter(|a| gate == Some(a.as_str()))
        .collect();
    let choices = offered
        .iter()
        .filter(|a| !shape.choices_of(a).is_empty())
        .cloned()
        .collect();
    Ok(Offer {
        offered,
        approvals,
        choices,
    })
}

/// The subject of the commit that records a take on the case's branch: `takt: take <story>`.
#[must_use]
pub fn take_subject(story: &str) -> String {
    format!("takt: take {story}")
}

/// The stories whose case is under `tree` and whose take is committed on that case's branch,
/// in case order: the stories a case is being worked for, wherever the store's own copy says
/// otherwise (the base's copy lists a story `proposed` until its case's branch is merged).
fn taken_on_case_branches(tree: &Path) -> Result<Vec<String>> {
    let mut taken = Vec::new();
    for (case, story) in cases_with_story(tree)? {
        let branch = format!("refs/heads/{}", crate::case::Branch::name_of(&case));
        if crate::git::has_subject(tree, &branch, &take_subject(&story))? && !taken.contains(&story)
        {
            taken.push(story);
        }
    }
    Ok(taken)
}

/// Every case under `tree` that records a story, as `(case id, story id)` in case order.
fn cases_with_story(tree: &Path) -> Result<Vec<(String, String)>> {
    let mut found = Vec::new();
    for (id, dir) in crate::case::case_dirs(tree)? {
        if !dir.join("case.yaml").is_file() {
            continue;
        }
        if let Some(story) = crate::case::story_of(&dir)? {
            found.push((id, story));
        }
    }
    Ok(found)
}

/// Whether the tree is on a case's branch, `takt/<id>` of an existing case: where [`to_base`]
/// moves it, or refuses. A branch under the prefix that is no case's is refused by name.
fn on_a_case_branch(tree: &Path) -> Result<bool> {
    let Some(branch) = crate::git::current_branch(tree)? else {
        return Ok(false);
    };
    Ok(crate::case::case_of_branch(tree, &branch)?.is_some())
}

/// Puts the tree on the base of the case whose branch it is on, so that the store is read as
/// the base has it, never as a case's branch, which carries a model's commits, has it. A tree
/// on any other branch, or detached, stays where it is. Refused when the branch is under takt's
/// reserved prefix and is no case's ([`crate::case::case_of_branch`]), when it names no case
/// ([`crate::case::check_id`], checked before the name is joined to a path), when the case's
/// base is not known, or when the tree has uncommitted work, which the checkout would carry
/// along.
fn to_base(tree: &Path) -> Result<()> {
    let Some(branch) = crate::git::current_branch(tree)? else {
        return Ok(());
    };
    let Some(case) = crate::case::case_of_branch(tree, &branch)? else {
        return Ok(());
    };
    let Some(base) = crate::case::Branch::read(&CaseDir::locate(tree, case))
        .ok()
        .and_then(|recorded| recorded.base)
    else {
        bail!(
            "the tree {} is on {branch}, a case's branch with no base recorded; the store is read from the base, never from a case's branch: check out the branch to take the next story from",
            tree.display()
        );
    };
    let uncommitted = crate::git::uncommitted(tree)?;
    if !uncommitted.is_empty() {
        bail!(
            "the tree {} is on {branch} with uncommitted work: {}; the next story is read from {base}, and checking it out would carry the work along: commit it, or move it out of the tree, first",
            tree.display(),
            uncommitted.join(", ")
        );
    }
    crate::git::run(tree, &["checkout", "-q", &base])
        .with_context(|| format!("check out {base} to read the store from"))?;
    Ok(())
}

/// The free space of a file system as a run reads it before a step
/// ([`Run::stop_when_disk_below`]): the device the file system is, by which the run compares
/// each file system once, and the bytes free on it for a writer that is not root.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FreeSpace {
    /// The file system's device, as `stat` gives it for a path on it.
    pub device: u64,
    /// The bytes free for a writer that is not root: `statvfs`'s `f_bavail * f_frsize`.
    pub bytes: u64,
}

/// What a run reads a path's free space with: [`free_space`] unless
/// [`Run::read_free_space_with`] gives another.
type FreeSpaceReader = Box<dyn Fn(&Path) -> std::io::Result<FreeSpace>>;

/// The free space of the file system holding `path`, the reader a run uses unless given another:
/// the device `stat` gives and `statvfs`'s `f_bavail * f_frsize`. A path that does not exist yet
/// (a build directory not made) is read at its nearest ancestor that does.
pub fn free_space(path: &Path) -> std::io::Result<FreeSpace> {
    let mut at = path;
    let metadata = loop {
        match std::fs::metadata(at) {
            Ok(metadata) => break metadata,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => match at.parent() {
                Some(parent) if !parent.as_os_str().is_empty() => at = parent,
                _ => return Err(error),
            },
            Err(error) => return Err(error),
        }
    };
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStrExt as _;
        use std::os::unix::fs::MetadataExt as _;
        let name = std::ffi::CString::new(at.as_os_str().as_bytes())
            .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidInput, error))?;
        let mut stat = std::mem::MaybeUninit::<libc::statvfs>::uninit();
        // SAFETY: `name` is a valid NUL-terminated string that outlives the call, and `stat` a
        // buffer of the type `statvfs` fills.
        if unsafe { libc::statvfs(name.as_ptr(), stat.as_mut_ptr()) } != 0 {
            return Err(std::io::Error::last_os_error());
        }
        // SAFETY: `statvfs` returned 0, so it filled `stat`.
        let stat = unsafe { stat.assume_init() };
        // The field types are `u64` on 64-bit Linux and narrower on some other targets.
        #[allow(clippy::useless_conversion)]
        let bytes = u64::from(stat.f_bavail).saturating_mul(u64::from(stat.f_frsize));
        Ok(FreeSpace {
            device: metadata.dev(),
            bytes,
        })
    }
    #[cfg(not(unix))]
    {
        let _ = metadata;
        Err(std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            "free space is read with statvfs, which this platform does not have",
        ))
    }
}

/// How [`scan_change`] says a scan that never ran.
pub(crate) const SCAN_DID_NOT_RUN: &str = "the scan could not run";

/// The summary a harness step's line carries in place of the session's own when its proposal
/// did not pass the content scan before the edit effect: what the session wrote is never
/// recorded.
pub const PROPOSAL_NOT_RECORDED: &str =
    "not recorded: the step's proposal did not pass the secret scan";

/// The summary of the step line that charges a harness step which failed and proposed nothing
/// with the tokens its session reported ([`Run::charge_unproposed`]).
pub const UNPROPOSED_SUMMARY: &str =
    "the harness failed and proposed nothing; the tokens its session reported are charged";

/// How many times one judgement step is tried in a run while its session ends without the line
/// naming its action ([`crate::harness::NoActionLine`]): each try is a step, counted by
/// `--iterations`, its step line written with the tokens its session reported
/// ([`Run::charge_no_action`]), and starts from the tree the try before it was discarded to; the
/// run ends on the error (exit 5) after the last. A run whose budget a try spent stops on the
/// budget instead (exit 3).
pub const NO_ACTION_TRIES: u32 = 3;

/// The summary of the step line of a try whose session named no action
/// ([`Run::charge_no_action`]), followed by the try and the bound.
pub const NO_ACTION_SUMMARY: &str = "the session named no action: it ended without the line naming its action; everything it changed was discarded and the tokens it reported are charged";

/// The shapes of a credential [`credential_shape`] knows, each a login a session could read in
/// the user's home: a private key block (`~/.ssh`); a JWT (Codex's `auth.json` tokens); an
/// Anthropic token (`sk-ant-`, Claude Code's `.credentials.json` OAuth tokens and API keys); a
/// GitHub token (`gho_`, `ghp_`, `ghu_`, `ghs_`, `ghr_`, `github_pat_`; gh's `hosts.yml`); an
/// OpenAI key (`sk-`); a GitLab token (each of [`GITLAB_TOKEN_PREFIXES`] before 20 token
/// characters or more; glab's `config.yml`); and a token field (`access_token`,
/// `refresh_token`, `id_token`, `oauth_token`, `accessToken`, `refreshToken`, `idToken`, and
/// the key `token:` that starts a line of glab's `config.yml`) set to a value of 20 token
/// characters or more holding a letter and a digit. Checked beside the gates policy's scan,
/// which holds no rule for them.
pub const CREDENTIAL_SHAPES: [&str; 7] = [
    "private-key",
    "jwt",
    "anthropic-token",
    "github-token",
    "gitlab-token",
    "openai-key",
    "token-field",
];

/// The prefixes GitLab gives its tokens, as GitLab's documentation lists them under "Token
/// prefixes" (<https://docs.gitlab.com/security/tokens/#token-prefixes>): personal,
/// impersonation, project and group access tokens (`glpat-`), an OAuth application secret
/// (`gloas-`), deploy tokens (`gldt-`), runner authentication tokens (`glrt-`, `glrtr-`), CI/CD
/// job tokens (`glcbt-`), trigger tokens (`glptt-`), feed tokens (`glft-`), incoming mail tokens
/// (`glimt-`), GitLab agent for Kubernetes tokens (`glagent-`), workspace tokens (`glwt-`),
/// SCIM tokens (`glsoat-`), feature flags client tokens (`glffct-`) and the session cookie
/// (`_gitlab_session=`).
pub const GITLAB_TOKEN_PREFIXES: [&str; 14] = [
    "glpat-",
    "gloas-",
    "gldt-",
    "glrt-",
    "glrtr-",
    "glcbt-",
    "glptt-",
    "glft-",
    "glimt-",
    "glagent-",
    "glwt-",
    "glsoat-",
    "glffct-",
    "_gitlab_session=",
];

/// The first of [`CREDENTIAL_SHAPES`] `text` holds, by its name; none when it holds none. Read
/// in process, so it always runs; the matched text is never returned.
#[must_use]
pub fn credential_shape(text: &str) -> Option<&'static str> {
    let bytes = text.as_bytes();
    let word = |c: u8| c.is_ascii_alphanumeric() || c == b'_' || c == b'-';
    let run = |from: usize, within: &dyn Fn(u8) -> bool| {
        bytes
            .get(from..)
            .map_or(0, |rest| rest.iter().take_while(|c| within(**c)).count())
    };
    let mut rest = text;
    while let Some(at) = rest.find("-----BEGIN ") {
        let after = &rest[at + "-----BEGIN ".len()..];
        if let Some(end) = after.find("-----")
            && end <= 40
            && after[..end].contains("PRIVATE KEY")
        {
            return Some("private-key");
        }
        rest = after;
    }
    // A JWT: three base64url parts joined by dots, the first two encoding a JSON object.
    let jwt = |from: usize| {
        let first = run(from, &word);
        let second = from + first + 1;
        let third = second + run(second, &word) + 1;
        first >= 13
            && bytes.get(from + first) == Some(&b'.')
            && bytes.get(second..).is_some_and(|at| at.starts_with(b"eyJ"))
            && run(second, &word) >= 13
            && bytes.get(third - 1) == Some(&b'.')
            && run(third, &word) >= 10
    };
    for at in 0..bytes.len() {
        if at > 0 && word(bytes[at - 1]) {
            continue;
        }
        let here = &bytes[at..];
        let shape = if here.starts_with(b"sk-ant-") && run(at + 7, &word) >= 20 {
            Some("anthropic-token")
        } else if here.starts_with(b"sk-") && run(at + 3, &word) >= 32 {
            Some("openai-key")
        } else if (here.len() > 4
            && here.starts_with(b"gh")
            && b"pousr".contains(&here[2])
            && here[3] == b'_'
            && run(at + 4, &|c: u8| c.is_ascii_alphanumeric()) >= 36)
            || (here.starts_with(b"github_pat_") && run(at + 11, &word) >= 22)
        {
            Some("github-token")
        } else if GITLAB_TOKEN_PREFIXES.iter().any(|prefix| {
            here.starts_with(prefix.as_bytes()) && run(at + prefix.len(), &word) >= 20
        }) {
            Some("gitlab-token")
        } else if here.starts_with(b"eyJ") && jwt(at) {
            Some("jwt")
        } else {
            None
        };
        if shape.is_some() {
            return shape;
        }
    }
    const FIELDS: [&str; 7] = [
        "access_token",
        "refresh_token",
        "id_token",
        "oauth_token",
        "accessToken",
        "refreshToken",
        "idToken",
    ];
    let token = |c: u8| c.is_ascii_alphanumeric() || b"._~+/=-".contains(&c);
    for field in FIELDS {
        for (at, _) in text.match_indices(field) {
            let mut from = at + field.len();
            let gap = run(from, &|c: u8| b"\"' \t:=".contains(&c));
            let assigned = bytes[from..from + gap]
                .iter()
                .any(|c| *c == b':' || *c == b'=');
            from += gap;
            let value = &bytes[from..from + run(from, &token)];
            if assigned
                && gap <= 4
                && value.len() >= 20
                && value.iter().any(u8::is_ascii_digit)
                && value.iter().any(u8::is_ascii_alphabetic)
            {
                return Some("token-field");
            }
        }
    }
    // glab's `config.yml` keeps a host's token under the key `token:`, one to a line and the
    // value the rest of it; a line of code that only names a token (`token: read_token(),`)
    // holds more than token characters after the key.
    for line in text.lines() {
        let Some(value) = line.trim_start().strip_prefix("token:") else {
            continue;
        };
        let value = value.trim().trim_matches(['"', '\'']);
        if value.len() >= 20
            && value.bytes().all(token)
            && value.bytes().any(|c| c.is_ascii_digit())
            && value.bytes().any(|c| c.is_ascii_alphabetic())
        {
            return Some("token-field");
        }
    }
    None
}

/// Why a path `proposal` names may not reach the tree, the record or a refusal: it holds text of
/// the shape of a credential ([`credential_shape`]). Read before any refusal that names a path,
/// and naming none. None when every path passes.
fn proposed_path_refusal(proposal: &crate::harness::Proposal) -> Option<String> {
    let shape = proposal
        .files
        .iter()
        .find_map(|file| credential_shape(&file.path))?;
    Some(format!(
        "{} proposes a path that holds text of the shape of a credential ({shape})",
        proposal.action
    ))
}

/// Why the content of `proposal` may not reach the tree or the record, read before the edit
/// effect writes a byte of it: each proposed file's content and the session's summary, each
/// apart so the refusal names which, hold no shape of [`CREDENTIAL_SHAPES`] and pass `scan`, the
/// scan of the run's adapter `adapter` ([`Scan::change_scan`], [`ChangeScan`]); a scan that
/// cannot run (no program, a policy that does not load, any exit but 0) refuses as a match does.
/// With no scan set, the proposal of a harness whose session may have read outside its copy
/// (`unscanned_refused`, [`crate::harness::Harness::reads_outside_its_copy`]) is refused, as
/// [`Scan::unscanned`] says: not scanned is never passed. A run under a declared `none` passes
/// `unscanned_refused` false, and its proposals are checked for the shape of a credential alone.
/// The paths the proposal names are read first ([`proposed_path_refusal`]) and, under a scan,
/// scanned as one text. A refusal names the file or the summary and the shape or the scan's
/// exit, never the text matched. None when it passes.
fn proposal_scan_refusal(
    tree: &Path,
    adapter: &Scan,
    scan: Option<&ChangeScan>,
    unscanned_refused: bool,
    proposal: &crate::harness::Proposal,
) -> Option<String> {
    if let Some(why) = proposed_path_refusal(proposal) {
        return Some(why);
    }
    let action = &proposal.action;
    let paths: Vec<&str> = proposal
        .files
        .iter()
        .map(|file| file.path.as_str())
        .collect();
    let paths = paths.join("\n");
    let mut texts: Vec<(&str, &str)> = Vec::new();
    if !paths.is_empty() {
        texts.push(("the paths it names", paths.as_str()));
    }
    texts.extend(
        proposal
            .files
            .iter()
            .filter_map(|file| Some((file.path.as_str(), file.content.as_deref()?))),
    );
    texts.push(("the step's summary", proposal.summary.as_str()));
    for (what, text) in &texts {
        if let Some(shape) = credential_shape(text) {
            return Some(format!(
                "{action} proposes {what}, which holds text of the shape of a credential ({shape})"
            ));
        }
    }
    let Some(scan) = scan else {
        return unscanned_refused.then(|| adapter.unscanned(action));
    };
    texts.iter().find_map(|(what, text)| {
        scan_bytes(tree, scan, "proposed.txt", text.as_bytes())
            .err()
            .map(|why| {
                if why.starts_with(SCAN_DID_NOT_RUN) {
                    format!(
                        "the secret scan of {what}, which {action} proposes, could not run: {why}"
                    )
                } else {
                    format!("{action} proposes {what}, which did not pass the secret scan: {why}")
                }
            })
    })
}

/// Scans the change so far, `diff`, with `scan` ([`scan_bytes`]). The one scan both the change
/// so far and each scope file a prompt attaches pass ([`crate::harness::scope_files`]).
pub(crate) fn scan_change(
    tree: &Path,
    scan: &ChangeScan,
    diff: &[u8],
) -> std::result::Result<(), String> {
    scan_bytes(tree, scan, "change.diff", diff)
}

/// Scans `bytes` with `scan`: written to a file `name` of its own (`0600`, in a directory of its
/// own, `0700`, under the temporary directory, outside the tree, removed after), then
/// `<program> scan-text --repo <tree> --policy <policy> <file>` in the tree. `Ok` only when it
/// exits 0; else why, naming its exit and never what it printed, which may quote the bytes.
fn scan_bytes(
    tree: &Path,
    scan: &ChangeScan,
    name: &str,
    bytes: &[u8],
) -> std::result::Result<(), String> {
    use std::io::Write as _;
    use std::os::unix::fs::{DirBuilderExt as _, OpenOptionsExt as _};
    static MADE: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    struct Removed(PathBuf);
    impl Drop for Removed {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    let dir = std::env::temp_dir().join(format!(
        "takt-change-{}-{}",
        std::process::id(),
        MADE.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    ));
    std::fs::DirBuilder::new()
        .mode(0o700)
        .create(&dir)
        .map_err(|error| format!("{SCAN_DID_NOT_RUN}: make {}: {error}", dir.display()))?;
    let dir = Removed(dir);
    let file = dir.0.join(name);
    std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .custom_flags(libc::O_NOFOLLOW)
        .open(&file)
        .and_then(|mut handle| handle.write_all(bytes))
        .map_err(|error| format!("{SCAN_DID_NOT_RUN}: write {}: {error}", file.display()))?;
    let ran = crate::confinement::Command::new(&scan.program)
        .args(scan.args(tree, &file))
        .current_dir(tree)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map_err(|error| format!("{SCAN_DID_NOT_RUN}: {}: {error}", scan.program.display()))?;
    if ran.success() {
        Ok(())
    } else {
        Err(format!(
            "{} scan-text exited {ran}: not passed",
            scan.program.display()
        ))
    }
}

/// What the last observation of each validator's action failed, as `record` (the case's record
/// so far) holds it: for each action a validator's `step` line names, the `failed` entries of
/// the `evidence` lines of that action's last such step ([`crate::validator::FailedCheck`]), in
/// the record's order. A later observation of the same action that failed nothing, or wrote no
/// record, leaves its action nothing; a judgement step's line leaves it as it was. A run reads
/// it before each judgement step; a replay reads it the same way for the step it rebuilds.
#[must_use]
pub fn failed_checks(record: &[serde_json::Value]) -> Vec<crate::validator::FailedCheck> {
    let mut last: Vec<(&str, Vec<crate::validator::FailedCheck>)> = Vec::new();
    // The action of the validator's step the lines that follow belong to, and its step.
    let mut current: Option<(&str, &serde_json::Value)> = None;
    for line in record {
        if line["event"] == "step" {
            current = None;
            if line["by"] == "validator"
                && let Some(action) = line["action"].as_str()
            {
                last.retain(|(seen, _)| *seen != action);
                last.push((action, Vec::new()));
                current = Some((action, &line["step"]));
            }
        } else if line["event"] == "evidence"
            && let Some((action, step)) = current
            && line["step"] == *step
            && let Ok(checks) =
                serde_json::from_value::<Vec<crate::validator::FailedCheck>>(line["failed"].clone())
            && let Some((_, failed)) = last.iter_mut().find(|(seen, _)| *seen == action)
        {
            failed.extend(checks);
        }
    }
    last.into_iter().flat_map(|(_, failed)| failed).collect()
}

/// The change the case at `case_dir` has made so far, as a judgement step is shown it
/// ([`ChangeSoFar`]), read in the repository of `tree` at the revision `at` (its `HEAD` when none
/// is given): not known for a case with no `base_commit`; none yet while no accepted step has
/// committed a path; else the diff from `base_commit` to that revision over the paths the
/// `effect` lines of `record` (the case's record so far) name as committed, `.engineering/` left
/// out, so a commit on the branch that no accepted step made stays out
/// ([`crate::git::diff_bounded`]), shown up to [`CHANGE_SHOWN_BYTES`] and, under `scan`, only
/// when the scan passes it. A diff that cannot be read and a scan that refuses or cannot run show
/// nothing, and `out` says why, never what the scan said; neither is an error. A run reads it
/// before each judgement step, under its scan ([`Run::scan_change_with`]); a replay reads it the
/// same way for the step it rebuilds ([`crate::replay`]).
pub fn change_so_far(
    tree: &Path,
    case_dir: &Path,
    record: impl FnOnce() -> Result<Vec<serde_json::Value>>,
    at: Option<&str>,
    scan: Option<&ChangeScan>,
    out: &mut dyn Write,
) -> Result<ChangeSoFar> {
    let read = |out: &mut dyn Write| -> Result<ChangeSoFar> {
        let Some(base) = crate::case::Branch::read(case_dir)?.base_commit else {
            return Ok(ChangeSoFar::NotKnown);
        };
        let mut paths = BTreeSet::new();
        for line in record()? {
            if line["event"] != "effect" || !line["revision"].is_string() {
                continue;
            }
            for path in line["paths"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(serde_json::Value::as_str)
            {
                if path != ENGINEERING && !path.starts_with(&format!("{ENGINEERING}/")) {
                    paths.insert(path.to_owned());
                }
            }
        }
        if paths.is_empty() {
            return Ok(ChangeSoFar::NoneYet);
        }
        let paths: Vec<String> = paths.into_iter().collect();
        let head = match at {
            None => crate::git::head(tree)?,
            Some(revision) => crate::git::run(
                tree,
                &["rev-parse", "--verify", &format!("{revision}^{{commit}}")],
            )?,
        };
        let diff_of = |parts: &[(&[String], DiffContext)], count_to: u64| {
            crate::git::diff_bounded(
                tree,
                (&base, &head),
                parts,
                ENGINEERING,
                CHANGE_SHOWN_BYTES,
                count_to,
            )
        };
        // Each file that is not a test whole, each test file as its changed lines: a step
        // that hands over reads the implementation for what the changed lines alone do not
        // show (the rest of the path they are on, what calls them). Past the bound, every
        // file as its changed lines; past that, the size alone.
        let (tests, others): (Vec<String>, Vec<String>) =
            paths.iter().cloned().partition(|path| is_test_path(path));
        let mut whole = None;
        if !others.is_empty()
            && let BoundedDiff::Within(diff) = diff_of(
                &[(&others, DiffContext::Whole), (&tests, DiffContext::Hunks)],
                0,
            )?
        {
            whole = Some(diff);
        }
        let shows_whole = whole.is_some();
        let diff = match whole {
            Some(diff) => BoundedDiff::Within(diff),
            None => diff_of(
                &[(&paths, DiffContext::Hunks)],
                crate::git::DIFF_COUNTED_BYTES,
            )?,
        };
        Ok(match diff {
            BoundedDiff::Within(diff) if diff.is_empty() => ChangeSoFar::NoneYet,
            BoundedDiff::Within(diff) => match scan {
                None => ChangeSoFar::shown(&diff, shows_whole),
                Some(scan) => match scan_change(tree, scan, &diff) {
                    Ok(()) => ChangeSoFar::shown(&diff, shows_whole),
                    Err(why) => {
                        writeln!(out, "  the change so far is not shown: {why}")?;
                        ChangeSoFar::NotShown(if why.starts_with(SCAN_DID_NOT_RUN) {
                            "the secret scan could not run"
                        } else {
                            "it did not pass the secret scan"
                        })
                    }
                },
            },
            BoundedDiff::Past { bytes, whole } => ChangeSoFar::TooLarge { bytes, whole },
        })
    };
    Ok(match read(out) {
        Ok(change) => change,
        Err(error) => {
            writeln!(out, "  the change so far is not shown: {error:#}")?;
            ChangeSoFar::NotShown("it could not be read")
        }
    })
}

/// A run over one case. Its stop [`Stop::Unperformable`] is off by default in the library: a
/// library caller opts in with [`Run::stop_when_unperformable`], and the CLI turns it on for
/// every run in its `finish`, so the library's default is not the product's behaviour.
///
/// Every commit the run makes carries the identity its case was opened with, as author and
/// committer, never the operator's git configuration ([`Run::committer`]). A run given no
/// identity, through [`Run::open`], [`Run::open_unit`] or [`Run::take_next`], opens its case
/// with none, and its commits are takt's own fixed `takt <takt@localhost>`: an opt-in, as
/// [`Run::stop_when_unperformable`] is. [`Run::open_unit_as`] and [`Run::take_next_as`] give
/// one ([`CommitAs`]); the CLI does for every case it opens.
pub struct Run<'a> {
    /// The worktree.
    pub tree: PathBuf,
    /// The case.
    pub case: CaseDir,
    /// What the run works on: the intent given, or the story the frontier took.
    pub unit: Unit,
    /// The judgement actor.
    pub harness: &'a dyn Harness,
    /// The validators, one per deterministic action.
    pub validators: &'a [Box<dyn Validator>],
    /// Steps taken.
    pub steps: u64,
    /// Tokens the harness reported.
    pub tokens: u64,
    /// How long the run waits on the forge before it observes again: [`Run::POLL`] unless set.
    pub poll: Duration,
    /// The revision the running binary was built from, as its `--version` prints it, recorded
    /// on the `opened` line; none unless the caller sets it, since this library does not know
    /// the build it is linked into.
    pub binary: Option<String>,
    /// The digest of the `llm.routes/1` document the run's configuration file gave, recorded on
    /// the `opened` line as `llm_digest`; none unless the caller sets it, as it does
    /// [`Run::binary`].
    pub llm_digest: Option<String>,
    /// Where discarding a harness step moves the files inside an ignored directory it rewrote,
    /// under the case's id and the step's number: [`default_set_aside`], checked when the run
    /// opens, unless [`Run::set_aside_in`] gives another; none when nothing names one, and then
    /// no harness step is taken. A validator's observation moves nothing aside.
    set_aside: Option<PathBuf>,
    /// Where each harness step's transcript is kept ([`crate::harness::keep_transcript`]), with
    /// the repository's key there ([`crate::harness::repo_key`]): none until
    /// [`Run::transcripts_in`] gives one (`takt run` gives [`default_transcripts`]), and then no
    /// transcript is kept.
    transcripts: Option<(PathBuf, String)>,
    /// The planning store a story's run records its test result and its move in; none until
    /// [`Run::record_on`] gives one, and only a story's run writes to it.
    store: Option<&'a dyn Frontier>,
    started: Instant,
    /// The active-time budget, in seconds: none until [`Run::cap_active_time`] gives one.
    active_cap: Option<u64>,
    /// How long the run has paused on the forge so far: the waits its active time leaves out.
    waited: Duration,
    options: RunOptions,
    /// The files outside the tree that decide how cargo and rustup build and run the tests
    /// ([`Outside`]), as the run found them when it began.
    outside: Outside,
    /// The directories outside the tree the test command prepends to `PATH` and what its Python
    /// imports from there, which [`Outside`] snapshots beside what cargo and rustup read.
    outside_dirs: OutsideDirs,
    /// The module names no step may put in an import directory, kept with the case when a Python
    /// answered ([`Modules::of_case`]).
    modules: Modules,
    /// Whether the run stops [`Stop::Unperformable`]; off until [`Run::stop_when_unperformable`].
    stops_unperformable: bool,
    /// Whether the run stops [`Stop::RedBase`]; on until [`Run::continue_past_red_base`].
    stops_red_base: bool,
    /// The records profile the case keeps its decisions under ([`Records`]): the profile the
    /// case was opened under on a resume ([`CaseDir::opened_profile`]), `trial` on a new run,
    /// until [`Run::keep_records`]; what the case keeps is [`Records::for_case`] of it. The CLI
    /// asks for `default` on a new run unless told otherwise; a library caller keeps every decision.
    records: Records,
    /// The least free space, in bytes, a step needs on the file system of the tree and of the
    /// build directory; 0 reads nothing, and is the default until [`Run::stop_when_disk_below`].
    min_free: u64,
    /// The build directory whose file system is read beside the tree's; none until
    /// [`Run::stop_when_disk_below`] names one.
    build_dir: Option<PathBuf>,
    /// What free space is read with ([`FreeSpaceReader`]).
    read_free: FreeSpaceReader,
    /// Whether a free-space read that failed was said: once a run.
    free_unread_said: bool,
    /// The author and committer of every commit the run makes, the case's
    /// ([`CaseDir::committer`]), read when the run is opened or resumed.
    committer: crate::git::CommitIdentity,
    /// The run's scan adapter ([`Scan`]), named on its `opened` line: [`Scan::from_env`] when
    /// the run opens, unless [`Run::scan_with`] gives another.
    scan: Scan,
    /// What the change so far and each proposal pass: the scan adapter's
    /// ([`Scan::change_scan`]), unless [`Run::scan_change_with`] gives another.
    change_scan: Option<ChangeScan>,
    /// What the run is configured to write as a step commit's message ([`Run::commit_settings`]).
    commit_settings: crate::commit_style::Settings,
    /// The commit style in effect, decided when the run starts ([`Run::commit_plan_now`]).
    commit_plan: Option<crate::commit_style::Plan>,
}

impl<'a> Run<'a> {
    /// How long a run waits on the forge before it observes again, by default: thirty seconds.
    pub const POLL: Duration = Duration::from_secs(30);

    /// Opens the case on the intent in `options` and prepares the run.
    pub fn open(
        tree: &Path,
        options: RunOptions,
        harness: &'a dyn Harness,
        validators: &'a [Box<dyn Validator>],
    ) -> Result<Self> {
        let unit = Unit::Intent(options.intent.clone());
        Self::open_unit(tree, options, unit, harness, validators)
    }

    /// Opens the case on `unit` and prepares the run. For a story, `intent.md` is the story's
    /// title followed by its body, and the case id, unless `options` gives one, is derived from
    /// the story id and the instant. Its commits are takt's own ([`CommitAs::Takt`]).
    pub fn open_unit(
        tree: &Path,
        options: RunOptions,
        unit: Unit,
        harness: &'a dyn Harness,
        validators: &'a [Box<dyn Validator>],
    ) -> Result<Self> {
        Self::open_unit_as(tree, options, unit, &CommitAs::Takt, harness, validators)
    }

    /// [`Run::open_unit`], the case opened with the identity `commit_as` says
    /// ([`CaseDir::open_as`]); a case that must have one and finds none is refused before
    /// anything is written.
    pub fn open_unit_as(
        tree: &Path,
        options: RunOptions,
        unit: Unit,
        commit_as: &CommitAs,
        harness: &'a dyn Harness,
        validators: &'a [Box<dyn Validator>],
    ) -> Result<Self> {
        Self::open_unit_in(tree, options, unit, commit_as, None, harness, validators)
    }

    /// [`Run::open_unit_as`], the discarded steps' set-aside directory given as `set_aside`
    /// (checked as [`Run::set_aside_in`] checks one) in place of the default
    /// ([`opening_set_aside`]), which is then not read. None is the default.
    pub fn open_unit_in(
        tree: &Path,
        mut options: RunOptions,
        unit: Unit,
        commit_as: &CommitAs,
        set_aside: Option<&Path>,
        harness: &'a dyn Harness,
        validators: &'a [Box<dyn Validator>],
    ) -> Result<Self> {
        match &unit {
            Unit::Intent(intent) => options.intent = intent.clone(),
            Unit::Story(story) => {
                options.intent = story.intent();
                if options.case_id.is_none() {
                    options.case_id = Some(story.case_id(&crate::now()));
                }
            }
        }
        let protocol = if Path::new(&options.protocol).is_file() {
            LoadedProtocol::from_file(Path::new(&options.protocol))?
        } else {
            LoadedProtocol::builtin(&options.protocol)?
        };
        // What the step choice will refuse is refused here, before a case directory or a
        // branch exists for it.
        Shape::of(&protocol, &options.inputs)?;
        // Nor does a run start that a harness step's rewrite could not be moved out of.
        let set_aside = opening_set_aside_given(tree, set_aside)?;
        let id = options
            .case_id
            .clone()
            .unwrap_or_else(|| format!("takt-{}", crate::now().replace([':', '-'], "")));
        let case = CaseDir::open_as(
            tree,
            protocol,
            &id,
            &options.intent,
            &options.inputs,
            commit_as,
        )?;
        let committer = match case.committer() {
            Ok(committer) => committer,
            Err(error) => {
                case.discard()?;
                return Err(error);
            }
        };
        // A case with no base cannot open its pull request. With a validator for `pr.open`
        // registered that is known now, so the run is refused before its first step rather
        // than after the tests, the gate and the harness have run; the case is discarded.
        if validators.iter().any(|v| v.action() == "pr.open")
            && crate::case::Branch::read(&case.dir)?.base.is_none()
        {
            case.discard()?;
            bail!(
                "the case {id} has no base branch to open its pull request against: the tree is on a detached HEAD; check out the branch the pull request is against, or run without a forge"
            );
        }
        // A case opened for a story records it: the next run on the repository finds the story
        // `active` with this case open and resumes it, never opens a second.
        if let Unit::Story(story) = &unit
            && let Err(error) = case.set_story(&story.id)
        {
            case.discard()?;
            return Err(error);
        }
        // The module names the test command's Python imports from outside the tree, asked once;
        // kept with the case when a Python answered, else the built-in lists stand in.
        let runs: Vec<String> = validators.iter().flat_map(|v| v.runs()).collect();
        let modules = match Modules::of_case(&case.dir, &runs, tree) {
            Ok(modules) => modules,
            Err(error) => {
                case.discard()?;
                return Err(error);
            }
        };
        let outside_dirs = OutsideDirs::of(&runs, tree, &modules);
        Ok(Self {
            tree: tree.to_path_buf(),
            case,
            unit,
            harness,
            validators,
            steps: 0,
            tokens: 0,
            poll: Self::POLL,
            binary: None,
            llm_digest: None,
            set_aside,
            transcripts: None,
            store: None,
            started: Instant::now(),
            active_cap: None,
            waited: Duration::ZERO,
            outside: Outside::read(tree, &outside_dirs)?,
            outside_dirs,
            options,
            modules,
            stops_unperformable: false,
            stops_red_base: true,
            records: Records::Trial,
            min_free: 0,
            build_dir: None,
            read_free: Box::new(free_space),
            free_unread_said: false,
            committer,
            scan: Scan::from_env(),
            change_scan: ChangeScan::from_env(),
            commit_settings: crate::commit_style::Settings::default(),
            commit_plan: None,
        })
    }

    /// The author and committer of every commit the run makes: the identity its case was opened
    /// with, else takt's own ([`CaseDir::committer`]).
    #[must_use]
    pub fn committer(&self) -> &crate::git::CommitIdentity {
        &self.committer
    }

    /// The case's candidate, a commit a turn takes as it is: the case's code commits, every
    /// commit from the base commit it pinned at open to its implementation revision but takt's
    /// store commits ([`is_store_commit`]: the take, the `test_result` record, the move and the
    /// approvals), replayed in order on its base branch's tip, else on that pinned commit
    /// ([`crate::git::replay`]), under the identity the case was opened with, else the one
    /// [`crate::git::COMMIT_IDENTITY_FILE`] names at that tip; named by
    /// `refs/takt/candidates/<case id>` ([`CANDIDATE_REFS`]), so it is fetched by its id from the
    /// repository whatever protocol the fetch speaks. Neither the work tree, the index nor a
    /// branch is touched, and the same case on the same base gives the same commit. Refused,
    /// with no ref written and the one an earlier candidate of the case wrote deleted, so no
    /// ref names a candidate the refusal says is none, when no identity is configured (takt
    /// makes no candidate as [`crate::git::CommitIdentity::takt`]), when the case pins no base,
    /// holds a merge or no code commit, and when replaying a code commit on the base's tip
    /// conflicts, naming the base, its tip, the commit and the paths.
    pub fn candidate(&self) -> Result<String> {
        self.make_candidate().map_err(|why| {
            let named = format!("{CANDIDATE_REFS}{}", self.case.id);
            match crate::git::run(&self.tree, &["update-ref", "-d", &named]) {
                Ok(_) => why,
                Err(stale) => why.context(format!(
                    "{named} could not be deleted and may still name an earlier candidate: {stale:#}"
                )),
            }
        })
    }

    /// [`Run::candidate`] but deleting the ref on a refusal.
    fn make_candidate(&self) -> Result<String> {
        let branch = crate::case::Branch::read(&self.case.dir)?;
        let Some(pinned) = branch.base_commit else {
            bail!("the case {} pins no base commit", self.case.id);
        };
        let onto = match &branch.base {
            Some(base) => crate::git::run(
                &self.tree,
                &[
                    "rev-parse",
                    "--verify",
                    "--end-of-options",
                    &format!("refs/heads/{base}^{{commit}}"),
                ],
            )
            .with_context(|| format!("the tip of the case's base branch {base}"))?,
            None => pinned.clone(),
        };
        let tip = match self.case.revision("implementation") {
            Some(revision) => revision.to_owned(),
            None => crate::git::run(
                &self.tree,
                &[
                    "rev-parse",
                    "--verify",
                    &format!("refs/heads/{}", branch.name),
                ],
            )?,
        };
        let identity = match self.case.commit_identity()? {
            Some(identity) => identity,
            None => crate::git::configured_identity(&self.tree, &onto)?.ok_or_else(|| {
                anyhow!(
                    "no commit identity: the case {} was opened with none, and {} holds no {} giving `identity.name` and `identity.email`; a candidate is a commit that leaves the machine, and takt makes none as {}: commit that file on {} or open the case with --commit-identity '<name> <email>'",
                    self.case.id,
                    short(&onto),
                    crate::git::COMMIT_IDENTITY_FILE,
                    crate::git::CommitIdentity::takt(),
                    branch.base.as_deref().unwrap_or("the base")
                )
            })?,
        };
        let story = self.case.story()?;
        let listed = crate::git::run(
            &self.tree,
            &[
                "rev-list",
                "--reverse",
                "--topo-order",
                "--end-of-options",
                &format!("{pinned}..{tip}"),
            ],
        )?;
        let mut code = Vec::new();
        for id in listed.lines().filter(|id| !id.is_empty()) {
            let commit = crate::git::commit(&self.tree, id)?;
            if commit.parents.len() != 1 {
                bail!(
                    "the case's branch holds the merge {id} ({}); a candidate is replayed from commits with one parent",
                    commit.subject
                );
            }
            if !story
                .as_deref()
                .is_some_and(|story| is_store_commit(&commit, story))
            {
                code.push(id.to_owned());
            }
        }
        if code.is_empty() {
            bail!(
                "the case's branch holds no code commit from {} to {}",
                short(&pinned),
                short(&tip)
            );
        }
        match crate::git::replay(&self.tree, &onto, &code, &identity)? {
            crate::git::Replayed::Made(candidate) => {
                crate::git::run(
                    &self.tree,
                    &[
                        "update-ref",
                        &format!("{CANDIDATE_REFS}{}", self.case.id),
                        &candidate,
                    ],
                )?;
                Ok(candidate)
            }
            crate::git::Replayed::Conflict { commit, paths } => bail!(
                "replaying the case's code commits on {} at {} (the case pinned {}) conflicted: the code commit {commit} does not apply there; it conflicts at {}",
                branch.base.as_deref().unwrap_or("the base"),
                short(&onto),
                short(&pinned),
                paths.join(", ")
            ),
        }
    }

    /// Gives the run the planning store `store`: a story's run records there the `test_result`
    /// pass record a validator of the case wrote, at the record's own `observed_at`, and moves
    /// the story to `implemented`, each a commit on the case's branch
    /// ([`crate::frontier::write_on_case_branch`]), before the step the case's grant authorises
    /// or, without one, once the case terminates `accepted`; a refusal is relayed and the stop
    /// is unchanged. A run on an intent has no story and writes nothing there; nor does an
    /// improvement turn ([`crate::case::TURN_PROTOCOL`]), whose `story.yaml` names the finding it
    /// verifies, a story it does not deliver: a turn writes nothing to the planning store.
    pub fn record_on(&mut self, store: &'a dyn Frontier) {
        if self.case.protocol.id != crate::case::TURN_PROTOCOL {
            self.store = Some(store);
        }
    }

    /// Where discarding a harness step moves the files inside an ignored directory it rewrote
    /// ([`Run::set_aside_in`]); none when nothing names one.
    #[must_use]
    pub fn set_aside(&self) -> Option<&Path> {
        self.set_aside.as_deref()
    }

    /// Moves what discarding a harness step sets aside to `dir` instead, once
    /// [`crate::git::set_aside_root`] accepts it: absolute, outside the tree, and makeable;
    /// else the error, and the run keeps the directory it had.
    pub fn set_aside_in(&mut self, dir: PathBuf) -> Result<()> {
        self.set_aside = Some(crate::git::set_aside_root(&dir, &self.tree)?);
        Ok(())
    }

    /// Where each harness step's transcript is kept; none until [`Run::transcripts_in`].
    #[must_use]
    pub fn transcripts(&self) -> Option<&Path> {
        self.transcripts.as_ref().map(|(root, _)| root.as_path())
    }

    /// Keeps each harness step's transcript under `dir`, in the tree's own segment
    /// ([`crate::harness::keep_transcript`], [`crate::harness::repo_key`]), once `dir` is
    /// accepted ([`crate::harness::transcripts_root`]): absolute, outside the tree, and
    /// makeable, every link on the way resolved now; else the error, which names the
    /// transcripts directory, and the run keeps what it had. The directory is made now, with
    /// every part of the way to it that is not there (`0700`, a link or a file on the way
    /// refused: [`crate::harness::make_transcripts_root`]), and what the run found outside the
    /// tree when it began ([`Outside`]) takes the parts it made as they are now, those made
    /// before a part that failed too: they are takt's, made before any step, so no step is
    /// charged with them (`~/.local` on an empty home, on the way to `~/.local/bin` or the user
    /// site; `~/.local/state` under a `PYTHONUSERBASE` of `~/.local`). Nothing else the run
    /// found is read again.
    pub fn transcripts_in(&mut self, dir: PathBuf) -> Result<()> {
        let root = crate::harness::transcripts_root(&dir, &self.tree)?;
        let key = crate::harness::repo_key(&self.tree)?;
        let mut made = Vec::new();
        let making = crate::harness::make_transcripts_root(&root, &mut made);
        self.outside.took(&made, &self.tree, &self.outside_dirs);
        making.with_context(|| {
            format!("the transcripts directory {} cannot be made", dir.display())
        })?;
        self.transcripts = Some((root, key));
        Ok(())
    }

    /// Scans the change so far with `scan` before a prompt shows it, or, given none, shows it
    /// unscanned; the run opens with [`ChangeScan::from_env`]. The run's scan adapter stays.
    pub fn scan_change_with(&mut self, scan: Option<ChangeScan>) {
        self.change_scan = scan;
    }

    /// Runs under the scan adapter `scan`: its scan ([`Scan::change_scan`]) for the change so
    /// far and each proposal, and its name on the `opened` line. Under a declared `none`
    /// ([`Scan::None`]) nothing is scanned, the proposal of a harness whose session may read
    /// outside its copy is not refused as unscanned, and each is still checked for the shape of
    /// a credential ([`credential_shape`]). The run opens under [`Scan::from_env`].
    pub fn scan_with(&mut self, scan: Scan) {
        self.change_scan = scan.change_scan();
        self.scan = scan;
    }

    /// Prepares a run on the case `id` a run before this one opened under `tree`: the case's
    /// files as they are, its `authority.yaml` read into every evaluation, its intent and inputs
    /// the run's. The unit is the story the case records, or the intent it was opened on.
    /// Nothing is opened and nothing discarded; the protocol, inputs and case id in `options`
    /// are not read: the case's own rule.
    pub fn resume(
        tree: &Path,
        options: RunOptions,
        id: &str,
        harness: &'a dyn Harness,
        validators: &'a [Box<dyn Validator>],
    ) -> Result<Self> {
        Self::resume_in(tree, options, id, None, harness, validators)
    }

    /// [`Run::resume`], the discarded steps' set-aside directory given as `set_aside` (checked as
    /// [`Run::set_aside_in`] checks one) in place of the default ([`opening_set_aside`]), which
    /// is then not read. None is the default.
    pub fn resume_in(
        tree: &Path,
        mut options: RunOptions,
        id: &str,
        set_aside: Option<&Path>,
        harness: &'a dyn Harness,
        validators: &'a [Box<dyn Validator>],
    ) -> Result<Self> {
        let set_aside = opening_set_aside_given(tree, set_aside)?;
        let case = CaseDir::resume(tree, id)?;
        let intent = case.intent()?;
        let unit = match case.story()? {
            Some(story) => {
                // `intent.md` of a story's case is its title, a blank line, its body.
                let (title, body) = intent.split_once("\n\n").unwrap_or((intent.as_str(), ""));
                Unit::Story(Story {
                    id: story,
                    title: title.to_owned(),
                    body: body.to_owned(),
                })
            }
            None => Unit::Intent(intent.clone()),
        };
        options.intent = intent;
        options.inputs = case.inputs();
        options.case_id = Some(id.to_owned());
        options.protocol = case.protocol.id.clone();
        // A case with no module file (none kept at open: no Python answered, or the case predates
        // the file) is asked again for this run, and nothing is written.
        let runs: Vec<String> = validators.iter().flat_map(|v| v.runs()).collect();
        let modules = Modules::at_resume(&case.dir, &runs, tree);
        let outside_dirs = OutsideDirs::of(&runs, tree, &modules);
        // The identity the case was opened with, whatever the repository configures by now.
        let committer = case.committer()?;
        // The profile the case was opened under: a resume keeps it unless the caller asks for
        // another ([`Run::keep_records`]).
        let records = case.opened_profile()?;
        Ok(Self {
            tree: tree.to_path_buf(),
            case,
            unit,
            harness,
            validators,
            steps: 0,
            tokens: 0,
            poll: Self::POLL,
            binary: None,
            llm_digest: None,
            set_aside,
            transcripts: None,
            store: None,
            started: Instant::now(),
            active_cap: None,
            waited: Duration::ZERO,
            outside: Outside::read(tree, &outside_dirs)?,
            outside_dirs,
            options,
            modules,
            stops_unperformable: false,
            stops_red_base: true,
            records,
            min_free: 0,
            build_dir: None,
            read_free: Box::new(free_space),
            free_unread_said: false,
            committer,
            scan: Scan::from_env(),
            change_scan: ChangeScan::from_env(),
            commit_settings: crate::commit_style::Settings::default(),
            commit_plan: None,
        })
    }

    /// Resumes the open case [`Run::next_to_resume`] names, if it names one; else opens the case
    /// on the next ready story, the store told last ([`Run::take_next`]). One case directory per
    /// taken story, none for a refused one, never a second for a story with one.
    pub fn open_next(
        tree: &Path,
        options: RunOptions,
        frontier: &dyn Frontier,
        harness: &'a dyn Harness,
        validators: &'a [Box<dyn Validator>],
        out: &mut dyn Write,
    ) -> Result<Option<Self>> {
        Self::open_next_in(tree, options, frontier, None, harness, validators, out)
    }

    /// [`Run::open_next`], the discarded steps' set-aside directory given as `set_aside` (checked
    /// as [`Run::set_aside_in`] checks one) in place of the default, for the case it resumes or
    /// opens ([`Run::open_unit_in`]). None is the default.
    pub fn open_next_in(
        tree: &Path,
        options: RunOptions,
        frontier: &dyn Frontier,
        set_aside: Option<&Path>,
        harness: &'a dyn Harness,
        validators: &'a [Box<dyn Validator>],
        out: &mut dyn Write,
    ) -> Result<Option<Self>> {
        if let Some((case_id, story_id)) = Self::next_to_resume(tree, frontier, out)? {
            return Self::resumed(
                tree, options, &case_id, &story_id, set_aside, harness, validators, out,
            )
            .map(Some);
        }
        Self::take_next_as_in(
            tree,
            options,
            frontier,
            &CommitAs::Takt,
            set_aside,
            harness,
            validators,
            out,
        )
    }

    /// The open case [`Run::open_next`] resumes, as `(case id, story id)`, answered with nothing
    /// changed: no checkout, no write, so a caller can refuse what was given for that case
    /// before anything is resumed, and then resume exactly it. The case of a story being
    /// worked: first one whose take is committed on its case's branch, whatever branch the
    /// tree is on; then one whose story the store lists `active`, read only from a tree that is
    /// not on a case's branch, so a status never comes from a branch a model committed on (a
    /// tree on a case's branch is put on its base by [`Run::take_next`], and the next run reads
    /// the base's list there). Among those, the first in case order that is not finished; a
    /// decision that does not read is said on `out` and the case taken as open. `None` when
    /// there is nothing to resume.
    pub fn next_to_resume(
        tree: &Path,
        frontier: &dyn Frontier,
        out: &mut dyn Write,
    ) -> Result<Option<(String, String)>> {
        let taken = taken_on_case_branches(tree)?;
        if let Some(found) = crate::case::resumable(tree, &taken, out)? {
            return Ok(Some(found));
        }
        if on_a_case_branch(tree)? {
            return Ok(None);
        }
        let active = frontier.active(tree)?;
        crate::case::resumable(tree, &active, out)
    }

    /// Opens the case on the next ready story, the store told last, and never resumes one: for
    /// a caller that asked [`Run::next_to_resume`] first and was told there is nothing to
    /// resume. The store is read from the base: a tree on a case's branch is put on that case's
    /// base first, so neither a story's text nor its status ever comes from a branch a model
    /// committed on. For each candidate in order, the case is opened first (on a clean tree,
    /// the protocol loaded, the inputs checked), then the story is taken, and the take
    /// committed on the case's branch, exactly the paths the store changed, as `takt: take
    /// <story>`: a refused step that discards the tree's change keeps it, and the case's
    /// implementation revision is that commit, the one its validators observe. A refusal
    /// removes the case just opened, is printed on `out` verbatim, and the next candidate is
    /// tried; a take or its commit that fails removes it too and is the error. A candidate a
    /// case already records is not taken again, and one whose minted case id the case id rule
    /// refuses ([`crate::case::check_id`]) is not opened: each is skipped with a line on `out`
    /// naming it. `None`,
    /// with no case opened, when nothing else is ready, such a skipped case included. The case
    /// commits as takt's own ([`CommitAs::Takt`]).
    pub fn take_next(
        tree: &Path,
        options: RunOptions,
        frontier: &dyn Frontier,
        harness: &'a dyn Harness,
        validators: &'a [Box<dyn Validator>],
        out: &mut dyn Write,
    ) -> Result<Option<Self>> {
        Self::take_next_as(
            tree,
            options,
            frontier,
            &CommitAs::Takt,
            harness,
            validators,
            out,
        )
    }

    /// [`Run::take_next`], each case opened with the identity `commit_as` says
    /// ([`Run::open_unit_as`]), the take committed with it: a case that must have one and finds
    /// none is refused, and that refusal is the error, with nothing taken.
    pub fn take_next_as(
        tree: &Path,
        options: RunOptions,
        frontier: &dyn Frontier,
        commit_as: &CommitAs,
        harness: &'a dyn Harness,
        validators: &'a [Box<dyn Validator>],
        out: &mut dyn Write,
    ) -> Result<Option<Self>> {
        Self::take_next_as_in(
            tree, options, frontier, commit_as, None, harness, validators, out,
        )
    }

    /// [`Run::take_next_as`], each case opened with the discarded steps' set-aside directory
    /// `set_aside` ([`Run::open_unit_in`]); None is the default.
    #[allow(clippy::too_many_arguments)]
    pub fn take_next_as_in(
        tree: &Path,
        options: RunOptions,
        frontier: &dyn Frontier,
        commit_as: &CommitAs,
        set_aside: Option<&Path>,
        harness: &'a dyn Harness,
        validators: &'a [Box<dyn Validator>],
        out: &mut dyn Write,
    ) -> Result<Option<Self>> {
        to_base(tree)?;
        let cased = cases_with_story(tree)?;
        for id in frontier.candidates(tree)? {
            if let Some((case, _)) = cased.iter().find(|(_, story)| *story == id) {
                writeln!(
                    out,
                    "frontier: {id} not taken: the case {case} was opened for it already"
                )?;
                continue;
            }
            let story = frontier.show(tree, &id)?;
            let title = story.title.clone();
            // The case id takt mints from the story's name can fail the case id rule (a name
            // long enough takes it past the limit): that story is skipped, named, and the next
            // tried. An id the operator gave is theirs, and its refusal stays the error.
            let mut options = options.clone();
            if options.case_id.is_none() {
                let minted = story.case_id(&crate::now());
                if let Err(refusal) = crate::case::check_id(&minted) {
                    writeln!(out, "frontier: {id} not taken: {refusal:#}")?;
                    continue;
                }
                options.case_id = Some(minted);
            }
            let mut run = Self::open_unit_in(
                tree,
                options,
                Unit::Story(story),
                commit_as,
                set_aside,
                harness,
                validators,
            )?;
            let taken = match frontier.take(tree, &id) {
                Ok(taken) => taken,
                Err(error) => return Err(Self::discarded(run, error)),
            };
            match taken {
                Take::Taken => {
                    let changed = match crate::git::uncommitted(tree) {
                        Ok(changed) => changed,
                        Err(error) => return Err(Self::discarded(run, error)),
                    };
                    let head = match crate::git::commit_paths(
                        tree,
                        &run.committer,
                        &changed,
                        &take_subject(&id),
                    ) {
                        Ok(head) => head,
                        Err(error) => return Err(Self::discarded(run, error)),
                    };
                    if run.case.revision("implementation").is_some() {
                        run.case.set_revision("implementation", &head)?;
                    }
                    writeln!(out, "frontier: took {id}: {title}")?;
                    return Ok(Some(run));
                }
                Take::Refused(refusal) => {
                    // The case goes whole: its directory, the cases directory when it was the
                    // last, and its branch, the tree back on the branch the case left.
                    let dir = run.case.dir.clone();
                    run.case.discard().with_context(|| {
                        format!("remove the case {} the store would not take", dir.display())
                    })?;
                    writeln!(out, "frontier: {id} not taken: {refusal}")?;
                }
            }
        }
        Ok(None)
    }

    /// Resumes the case `case_id` of the story `story_id` and says so on `out`.
    #[allow(clippy::too_many_arguments)]
    fn resumed(
        tree: &Path,
        options: RunOptions,
        case_id: &str,
        story_id: &str,
        set_aside: Option<&Path>,
        harness: &'a dyn Harness,
        validators: &'a [Box<dyn Validator>],
        out: &mut dyn Write,
    ) -> Result<Self> {
        let run = Self::resume_in(tree, options, case_id, set_aside, harness, validators)?;
        let title = match &run.unit {
            Unit::Story(story) => story.title.clone(),
            Unit::Intent(intent) => intent.clone(),
        };
        writeln!(
            out,
            "frontier: resumed {story_id}: {title} (case {case_id})"
        )?;
        Ok(run)
    }

    /// Removes the case `run` just opened, whose take failed with `error`, and answers the
    /// error; a removal that fails too is added to it.
    fn discarded(run: Self, error: anyhow::Error) -> anyhow::Error {
        let dir = run.case.dir.clone();
        match run.case.discard() {
            Ok(()) => error,
            Err(removal) => error.context(format!(
                "and the case {} could not be removed: {removal:#}",
                dir.display()
            )),
        }
    }

    /// The checkpoint of the tree before the step being taken, the branches of
    /// [`Self::kept_branches`] guarded; for a harness step (`harness`), what discarding it moves
    /// aside going to `<set-aside>/<case id>/step-<n>` ([`Run::set_aside`]), made new when
    /// something is moved: one that exists already, a run before this one under the same case
    /// id and step, gets `-2`, `-3` and so on after it, never shared
    /// ([`crate::git::Checkpoint::setting_aside_in`]). A validator's observation moves nothing
    /// aside: what its command rewrote inside an ignored directory is its own build output, kept
    /// in place as a pass keeps it.
    fn checkpoint(&self, harness: bool) -> Result<crate::git::Checkpoint> {
        let checkpoint = crate::git::Checkpoint::take_guarding(&self.tree, &self.kept_branches())?;
        let (Some(id), Some(root)) = (
            self.case.dir.file_name().filter(|_| harness),
            self.set_aside.clone(),
        ) else {
            return Ok(checkpoint);
        };
        Ok(checkpoint.setting_aside_in(root, id.to_owned(), format!("step-{}", self.steps)))
    }

    /// The branches a checkpoint guards even when another worktree has them checked out: the
    /// case's own branch and its base, where `take_next` reads the store from and the next case
    /// forks from.
    fn kept_branches(&self) -> Vec<String> {
        let mut kept = vec![self.case.branch.clone()];
        kept.extend(
            crate::case::Branch::read(&self.case.dir)
                .ok()
                .and_then(|branch| branch.base),
        );
        kept
    }

    /// The words of every command the run's validators run from the tree ([`Validator::runs`]):
    /// what names the test command's own script on the judge surface.
    fn runs(&self) -> Vec<String> {
        self.validators.iter().flat_map(|v| v.runs()).collect()
    }

    /// The repository's own judge-surface globs ([`JUDGE_LIST`]) at the case's pinned base
    /// revision, never the tree's; none for a case that pins no base. The inner error is a list
    /// that does not parse.
    fn judge_list(&self) -> Result<std::result::Result<Vec<String>, String>> {
        match crate::case::Branch::read(&self.case.dir)?.base_commit {
            Some(base) => judge_list_at(&self.tree, &base),
            None => Ok(Ok(Vec::new())),
        }
    }

    fn budget_spent(&self) -> Option<Stop> {
        if let Some(n) = self.options.iterations
            && self.steps >= n
        {
            return Some(Stop::Iterations(n));
        }
        // An hour budget that is not a number is spent: no comparison would ever end it.
        if let Some(h) = self.options.hours
            && (h.is_nan() || self.started.elapsed().as_secs_f64() >= h * 3600.0)
        {
            return Some(Stop::Hours(h));
        }
        if let Some(t) = self.options.tokens
            && self.tokens >= t
        {
            return Some(Stop::Tokens(t));
        }
        if let Some(budget) = self.active_cap {
            let active = self.active_time();
            if active.as_secs_f64() >= budget as f64 {
                return Some(Stop::ActiveTime {
                    budget,
                    active: active.as_secs_f64(),
                    waited: self.waited.as_secs_f64(),
                });
            }
        }
        None
    }

    /// Caps the run's active time ([`Run::active_time`]) at `seconds`: read before every step,
    /// as the token budget is, so a step under way runs to its end and the run stops
    /// [`Stop::ActiveTime`] before the next. Waits on the forge spend none of it, and it is no
    /// bound on them: a run with no step or hour budget still has nothing to wait in. Without
    /// this call the run's active time is not capped. The CLI calls it with `--active-seconds`.
    pub fn cap_active_time(&mut self, seconds: u64) {
        self.active_cap = Some(seconds);
    }

    /// The run's active time: its wall time since it was opened or resumed, less the time it
    /// paused on the forge.
    #[must_use]
    pub fn active_time(&self) -> Duration {
        self.started.elapsed().saturating_sub(self.waited)
    }

    /// How long the run has paused on the forge since it was opened or resumed.
    #[must_use]
    pub fn waited(&self) -> Duration {
        self.waited
    }

    /// For a run given no approval gate, the gate its case's protocol names for the case's
    /// inputs ([`crate::LoadedProtocol::default_gate`]); a run given one keeps it. Without this
    /// call a run with no gate offers the harness none.
    pub fn take_default_gate(&mut self) {
        if self.options.gate.is_none() {
            self.options.gate = self.case.protocol.default_gate(&self.case.inputs());
        }
    }

    /// Stops the run [`Stop::Unperformable`] at the first step no step can move the case past:
    /// a claim left is decided only by an admissible built-in action no validator of the run
    /// performs (no forge, no gates document). The CLI turns it on for every run; a library
    /// caller opts in here, and without this call a run offers the harness its step as before.
    pub fn stop_when_unperformable(&mut self) {
        self.stops_unperformable = true;
    }

    /// Goes on past a red base: the case's first `tests.run` failing before any effect is
    /// recorded like any other result, and the harness is offered its step, instead of the run
    /// stopping [`Stop::RedBase`]. For a caller whose base fails on purpose; every run stops so
    /// without this call.
    pub fn continue_past_red_base(&mut self) {
        self.stops_red_base = false;
    }

    /// Writes each step commit's message as `settings` say ([`crate::commit_style`]); without
    /// this call, `semantic` with the trailer `Refs`. The repository's own commit rule, read at
    /// the case's base, wins over the style given here.
    pub fn commit_settings(&mut self, settings: crate::commit_style::Settings) {
        self.commit_settings = settings;
        self.commit_plan = None;
    }

    /// The commit style in effect for this run, decided on first use from the settings, the
    /// instruction files at the case's base and the story's tags there, and kept.
    fn commit_plan_now(&mut self) -> crate::commit_style::Plan {
        if let Some(plan) = &self.commit_plan {
            return plan.clone();
        }
        let base = crate::case::Branch::read(&self.case.dir)
            .ok()
            .and_then(|branch| branch.base_commit);
        let rule = base.as_deref().and_then(|base| {
            crate::commit_style::rule_in(&self.tree, base, &self.harness.instruction_paths())
        });
        let bug = self.case.story().ok().flatten().is_some_and(|story| {
            crate::case::story_front_at(&self.tree, base.as_deref().unwrap_or("HEAD"), &story)
                .ok()
                .flatten()
                .is_some_and(|front| {
                    front["tags"].as_sequence().is_some_and(|tags| {
                        tags.iter()
                            .any(|tag| tag.as_str() == Some(crate::commit_style::BUG_TAG))
                    })
                })
        });
        let plan = crate::commit_style::Plan::new(&self.commit_settings, rule, bug);
        self.commit_plan = Some(plan.clone());
        plan
    }

    /// Asks for the records profile the case keeps its decisions under: `default` keeps the
    /// latest decision alone, after each decision is written; `trial` keeps every one. A scenario
    /// case or an improvement case keeps every one whatever is asked ([`Records::for_case`]).
    /// A resumed case runs under the profile it was opened under until this call; a new one
    /// runs under `trial` until this call.
    pub fn keep_records(&mut self, records: Records) {
        self.records = records;
    }

    /// The records profile this run's case keeps: the one asked, or every decision for a scenario
    /// case or an improvement case ([`Records::for_case`]), a new run and a resumed one alike.
    #[must_use]
    pub fn kept_records(&self) -> Records {
        self.records.for_case(&self.case.protocol.id, &self.case.id)
    }

    /// Stops the run [`Stop::DiskLow`] before a step (a harness step, a deterministic step, the
    /// test command, a gate) when the file system holding the case's tree, or the one holding
    /// `build_dir` (relative, the tree's), has fewer than `min_bytes` free; each file system is
    /// compared once. `0` turns the check off, as without this call, and nothing is read. The CLI
    /// calls it for every run with `--min-free-gib` and `CARGO_TARGET_DIR`.
    pub fn stop_when_disk_below(&mut self, min_bytes: u64, build_dir: Option<PathBuf>) {
        self.min_free = min_bytes;
        self.build_dir = build_dir;
    }

    /// Reads free space with `reader` instead of [`free_space`]: a test's free space, which no
    /// disk has to be filled for. The reader owns what it captures (`'static`), so a run keeps
    /// no borrow alive longer than without one.
    pub fn read_free_space_with(
        &mut self,
        reader: impl Fn(&Path) -> std::io::Result<FreeSpace> + 'static,
    ) {
        self.read_free = Box::new(reader);
    }

    /// The [`Stop::DiskLow`] for the first file system under the threshold, the tree's then the
    /// build directory's, each read once; none with no threshold or when each has room. A read
    /// that fails stops nothing: the run goes on unchecked there, and the first such failure of
    /// the run is printed to `out`.
    fn disk_low(&mut self, out: &mut dyn Write) -> Result<Option<Stop>> {
        if self.min_free == 0 {
            return Ok(None);
        }
        let build_dir = self.build_dir.as_ref().map(|dir| self.tree.join(dir));
        let mut read: BTreeSet<u64> = BTreeSet::new();
        for path in std::iter::once(self.tree.clone()).chain(build_dir) {
            match (self.read_free)(&path) {
                Ok(free) => {
                    if read.insert(free.device) && free.bytes < self.min_free {
                        return Ok(Some(Stop::DiskLow {
                            path,
                            free_bytes: free.bytes,
                            min_bytes: self.min_free,
                        }));
                    }
                }
                Err(error) if !self.free_unread_said => {
                    self.free_unread_said = true;
                    writeln!(
                        out,
                        "  free space: {} could not be read, so it is not checked: {error}",
                        path.display()
                    )?;
                }
                Err(_) => {}
            }
        }
        Ok(None)
    }

    /// How long the run waits on the forge before it observes again: [`Run::poll`], cut to
    /// what is left of the hour budget. `None` for a run with neither a step nor an hour
    /// budget: it has nothing to wait in, and a wait without a bound would never end. What is
    /// left of an hour budget too large for a [`Duration`], infinite among them, saturates and
    /// leaves the poll as it is; nothing here panics.
    fn pause(&self) -> Option<Duration> {
        if self.options.iterations.is_none() && self.options.hours.is_none() {
            return None;
        }
        Some(match self.time_left() {
            Some(left) => self.poll.min(left),
            None => self.poll,
        })
    }

    /// The instant the hour budget ends: the run's start and the budget. `None` for a run
    /// without one, or with one too large for an [`Instant`] (infinite among them), which never
    /// ends; a budget that is not a number ends at the start. Nothing here panics.
    fn deadline(&self) -> Option<Instant> {
        let h = self.options.hours?;
        let budget = Duration::try_from_secs_f64((h * 3600.0).max(0.0)).ok()?;
        self.started.checked_add(budget)
    }

    /// What is left of the hour budget now ([`Run::deadline`]); `None` for a run without an
    /// end, zero once it is spent.
    fn time_left(&self) -> Option<Duration> {
        self.deadline()
            .map(|deadline| deadline.saturating_duration_since(Instant::now()))
    }

    /// Runs until a stop; prints one line per frontier, step and evidence to `out`, and appends
    /// one event per such fact to the case's `events.jsonl`, the stop, or the error the run
    /// ended on, last. At an `accepted` stop a story's run given the store records there first
    /// ([`Run::record_on`]); what goes wrong there is recorded as an `error` line and the stop
    /// is kept. Every run's record ends on its `stop` line, or on an `error` line when the run
    /// ends on an error, a failing `out` among them. The run holds the case while it runs
    /// (`run.lock` under the case directory), so no grant lands and no second run starts on it
    /// meanwhile; it lets go at every stop and on error. Nothing under the case directory is
    /// written, appended to or removed while a part of the way to it is not a real directory
    /// ([`CaseDir::check_way`], read before the hold, before the stop and before the error
    /// line): a run that ends that way records nothing and keeps its hold, says on standard
    /// error what it could not record, and ends on the error.
    pub fn run(&mut self, out: &mut dyn Write) -> Result<Stop> {
        self.case.check_way()?;
        self.case.hold()?;
        let events = Events::new(&self.case.dir);
        // A panic in the loop (a harness or a validator that broke) is recorded as the run's
        // `error` line and the case let go before the panic goes on: the record ends, and the
        // case is not left held; unless the way to the case directory holds a link or a part
        // that is not a directory, when neither is done (`unrecorded`).
        let outcome = match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            self.run_held(&events, out).and_then(|stop| {
                self.case.check_way()?;
                self.close(&events, &stop, out).map(|()| stop)
            })
        })) {
            Ok(outcome) => outcome,
            Err(panic) => {
                let what = panic
                    .downcast_ref::<&str>()
                    .map(|text| (*text).to_owned())
                    .or_else(|| panic.downcast_ref::<String>().cloned())
                    .unwrap_or_else(|| "a panic with no message".to_owned());
                let error = format!("the run panicked: {what}");
                match self.case.check_way() {
                    Ok(()) => {
                        let _ = events.emit(self.steps, &Event::Error { error });
                        let _ = self.case.release();
                    }
                    Err(way) => self.unrecorded(&error, &way),
                }
                std::panic::resume_unwind(panic);
            }
        };
        let outcome = match outcome {
            Ok(stop) => Ok(stop),
            Err(error) => {
                if let Err(way) = self.case.check_way() {
                    self.unrecorded(&format!("{error:#}"), &way);
                    return Err(error.context(format!(
                        "nothing of the run was recorded and the case was not let go: {way:#}"
                    )));
                }
                Err(
                    match events.emit(
                        self.steps,
                        &Event::Error {
                            error: format!("{error:#}"),
                        },
                    ) {
                        Ok(()) => error,
                        Err(unrecorded) => error.context(format!(
                            "and the error could not be recorded: {unrecorded:#}"
                        )),
                    },
                )
            }
        };
        let released = self.case.release();
        let stop = outcome?;
        released?;
        Ok(stop)
    }

    /// Says on standard error what a run that ended on `error` did not record because the way to
    /// its case directory is not a real directory (`way`, [`CaseDir::check_way`]): the steps and
    /// the tokens it took, the refusal, and the error. Its record and its hold stay as they are.
    fn unrecorded(&self, error: &str, way: &anyhow::Error) {
        eprintln!(
            "takt: case {}: nothing of this run was recorded and the case was not let go: {way:#}; the run took {} step(s) and {} token(s); it ended on: {error}",
            self.case.id, self.steps, self.tokens
        );
    }

    /// After the loop stopped: the store's writes of a story's case that terminated
    /// [`ACCEPTED`], then the stop recorded. Anything that goes wrong with the store's writes
    /// (an `events.jsonl` that does not read, an `out` that fails) is recorded as an `error`
    /// line before the stop, which stays the one the case reached.
    fn close(&self, events: &Events, stop: &Stop, out: &mut dyn Write) -> Result<()> {
        if let Stop::Completed(outcome) = stop
            && outcome == ACCEPTED
            && let (Unit::Story(story), Some(store)) = (&self.unit, self.store)
            && let Err(error) = self.record_accepted(events, store, &story.id, out)
        {
            events.emit(
                self.steps,
                &Event::Error {
                    error: format!(
                        "recording the accepted case on {} in the store: {error:#}",
                        story.id
                    ),
                },
            )?;
        }
        // A delivery stopped at `pr.opened` for want of a forge hands its change over as a
        // candidate a turn takes as it is; one that cannot be made is named, and the stop stays.
        if stop.hands_over() {
            match self.candidate() {
                Ok(candidate) => writeln!(out, "candidate: {candidate}")?,
                Err(why) => writeln!(out, "candidate: none: {why:#}")?,
            }
        }
        events.emit(
            self.steps,
            &Event::Stop {
                stop: stop.to_string(),
                steps: self.steps,
                tokens: self.tokens,
                completed: Some(matches!(stop, Stop::Completed(_))),
            },
        )
    }

    /// Records the case's story in `store`, before the step the grant authorises or once the
    /// case is accepted: the `test_result` pass record a validator of the case wrote
    /// ([`Run::observed_pass`]) as `test_result` evidence at its own `observed_at`, then the move
    /// to `implemented`, which the store judges, the last; each a commit on the case's branch
    /// with what it committed kept in the case directory ([`crate::case::keep_rider`]) and its
    /// line in the case's record, written with the commit: either that cannot be written takes
    /// the commit back ([`crate::frontier::write_on_case_branch`]). A write kept at the case's
    /// current revision and committed above it is not made again; one of an earlier revision
    /// is ([`Run::store_done`]). A refusal, the store's verbatim or takt's own, is recorded, then
    /// printed, and nothing is retried; the refusals are the answer. What is printed comes after
    /// what is recorded: an `out` that fails skips no line and no write, and is the error once
    /// both writes were tried.
    fn record_accepted(
        &self,
        events: &Events,
        store: &dyn Frontier,
        story: &str,
        out: &mut dyn Write,
    ) -> Result<Vec<String>> {
        let recorded = events.read()?;
        let branch = &self.case.branch;
        let identity = &self.committer;
        let mut refusals: Vec<String> = Vec::new();
        let committed = |commit: &Option<String>| match commit {
            Some(commit) => format!(" (committed {} on {branch})", short(commit)),
            None => String::new(),
        };
        let mut unprinted: Option<std::io::Error> = None;
        let mut say = |text: String| {
            if unprinted.is_none()
                && let Err(error) = writeln!(out, "{text}")
            {
                unprinted = Some(error);
            }
        };
        let evidence_subject = crate::frontier::test_result_subject(story);
        if !self.store_done(story, KEPT_TEST_RESULT, &evidence_subject)? {
            let written = self.observed_pass(&recorded).and_then(|observed| {
                let Some((reference, at)) = observed else {
                    return Err(anyhow::anyhow!(
                        "no {TEST_RESULT} pass record a validator of the case wrote is about the current revision; nothing was recorded"
                    ));
                };
                crate::frontier::write_on_case_branch(
                    &self.tree,
                    branch,
                    identity,
                    story,
                    &evidence_subject,
                    || store.record_test_result(&self.tree, story, &reference, &at),
                    |commit| {
                        self.kept_and_recorded(commit, KEPT_TEST_RESULT, || {
                            events.emit(
                                self.steps,
                                &Event::StoreEvidence {
                                    story: story.to_owned(),
                                    kind: TEST_RESULT.to_owned(),
                                    reference: reference.clone(),
                                    observed_at: at.clone(),
                                    commit: commit.map(str::to_owned),
                                },
                            )
                        })
                    },
                )
                .map(|written| written.map(|commit| (reference, at, commit)))
            });
            match written {
                Ok(Ok((reference, at, commit))) => say(format!(
                    "store: {TEST_RESULT} recorded on {story} at {at} from {reference}{}",
                    committed(&commit)
                )),
                Ok(Err(refusal)) => {
                    self.refused(events, BY_STORE, &refusal)?;
                    say(format!(
                        "store: {TEST_RESULT} not recorded on {story}: {refusal}"
                    ));
                    refusals.push(refusal);
                }
                Err(error) => {
                    let why = format!("{error:#}");
                    self.refused(events, BY_TAKT, &why)?;
                    say(format!(
                        "store: {TEST_RESULT} not recorded on {story}: {why}"
                    ));
                    refusals.push(why);
                }
            }
        }
        let move_subject = crate::frontier::implemented_subject(story);
        if !self.store_done(story, KEPT_MOVE, &move_subject)? {
            match crate::frontier::write_on_case_branch(
                &self.tree,
                branch,
                identity,
                story,
                &move_subject,
                || store.implemented(&self.tree, story),
                |commit| {
                    self.kept_and_recorded(commit, KEPT_MOVE, || {
                        events.emit(
                            self.steps,
                            &Event::StoreMove {
                                story: story.to_owned(),
                                to: IMPLEMENTED.to_owned(),
                                commit: commit.map(str::to_owned),
                            },
                        )
                    })
                },
            ) {
                Ok(Ok(commit)) => say(format!(
                    "store: {story} moved to {IMPLEMENTED}{}",
                    committed(&commit)
                )),
                Ok(Err(refusal)) => {
                    self.refused(events, BY_STORE, &refusal)?;
                    say(format!(
                        "store: {story} not moved to {IMPLEMENTED}: {refusal}"
                    ));
                    refusals.push(refusal);
                }
                Err(error) => {
                    let why = format!("{error:#}");
                    self.refused(events, BY_TAKT, &why)?;
                    say(format!("store: {story} not moved to {IMPLEMENTED}: {why}"));
                    refusals.push(why);
                }
            }
        }
        match unprinted {
            Some(error) => Err(anyhow::Error::new(error).context("print the store's writes")),
            None => Ok(refusals),
        }
    }

    /// Whether the store write on `story` the case keeps as `name`, committed as `subject`, is
    /// done at the case's current implementation revision: kept under it and committed above it
    /// on the case's branch. One made at an earlier revision is no record of this one. The move
    /// ([`KEPT_MOVE`]) is done as well when the story at the revision is already `implemented`,
    /// moved below it at an earlier revision: the store would refuse it again, so nothing is due.
    fn store_done(&self, story: &str, name: &str, subject: &str) -> Result<bool> {
        let Some(revision) = self.case.revision("implementation") else {
            return Ok(false);
        };
        let above = format!("{revision}..refs/heads/{}", self.case.branch);
        if crate::case::is_kept_at(&self.case.dir, &revision, name)
            && crate::git::has_subject(&self.tree, &above, subject)?
        {
            return Ok(true);
        }
        Ok(name == KEPT_MOVE
            && crate::case::story_status_at(&self.tree, &revision, story)?.as_deref()
                == Some(IMPLEMENTED))
    }

    /// Keeps in the case directory, as `name` at the implementation revision, what the store's
    /// write `commit` committed ([`crate::case::keep_rider`]), then writes its line with
    /// `record`: a line that cannot be written takes the kept file back. A write that changed
    /// nothing keeps nothing.
    fn kept_and_recorded(
        &self,
        commit: Option<&str>,
        name: &str,
        record: impl FnOnce() -> Result<()>,
    ) -> Result<()> {
        let kept = match (commit, self.case.revision("implementation")) {
            (Some(commit), Some(revision)) => Some(crate::case::keep_rider(
                &self.tree,
                &self.case.dir,
                &revision,
                commit,
                name,
            )?),
            _ => None,
        };
        let recorded = record();
        if recorded.is_err()
            && let Some(kept) = &kept
        {
            for kept in kept {
                let _ = std::fs::remove_file(kept);
            }
        }
        recorded
    }

    /// Before the step for `action`: when the case holds the grant that authorises it
    /// ([`CaseDir::grants`]), a story's run given the store records the story there
    /// ([`Run::record_accepted`]), so the `test_result` and the move, the last, ride to the base
    /// with the step's merge. It fails closed: unless both are done at the granted revision
    /// ([`Run::store_done`]), the step is not taken, and the answer is why, the store's
    /// refusals relayed, for the run to stop on ([`Stop::StepRefused`]). What goes wrong
    /// writing them is recorded as an `error` line and is such a refusal too.
    fn record_before(&self, events: &Events, action: &str, out: &mut dyn Write) -> Result<Before> {
        let (Unit::Story(story), Some(store)) = (&self.unit, self.store) else {
            return Ok(Before::Ungranted);
        };
        if !self.case.grants(action)? {
            return Ok(Before::Ungranted);
        }
        let refusals = match self.record_accepted(events, store, &story.id, out) {
            Ok(refusals) => refusals,
            Err(error) => {
                let why = format!(
                    "recording {} in the store before {action}: {error:#}",
                    story.id
                );
                events.emit(self.steps, &Event::Error { error: why.clone() })?;
                vec![why]
            }
        };
        let story = story.id.as_str();
        let test_result = self.store_done(
            story,
            KEPT_TEST_RESULT,
            &crate::frontier::test_result_subject(story),
        )?;
        let moved = self.store_done(
            story,
            KEPT_MOVE,
            &crate::frontier::implemented_subject(story),
        )?;
        if test_result && moved {
            return Ok(Before::Recorded);
        }
        let state = |done: bool| if done { "written" } else { "missing" };
        let mut why = format!(
            "{action} not taken: the store's records of {story} at {} are not both written ({TEST_RESULT} {}, the move to {IMPLEMENTED} {}), so they would not reach the base with it",
            self.case
                .revision("implementation")
                .as_deref()
                .map_or("no revision", short),
            state(test_result),
            state(moved)
        );
        if !refusals.is_empty() {
            why.push_str(&format!("; {}", refusals.join("; ")));
        }
        Ok(Before::Missing(why))
    }

    /// The latest `test_result` pass record a validator of the case wrote that is about the
    /// current revision of its subject: the reference the store records for it (its path, sha256
    /// and fields, [`crate::frontier::test_result_reference`]) and its own `observed_at`.
    /// A record counts only when an `evidence` line of the case's record, which the kernel
    /// writes and a step may not, names it under the case's `evidence/`: a record put there by
    /// any other hand is never one the run observed. `None` when there is none.
    fn observed_pass(&self, recorded: &[serde_json::Value]) -> Result<Option<(String, String)>> {
        let mine = self
            .case
            .dir
            .strip_prefix(&self.tree)
            .with_context(|| format!("locate the case {}", self.case.id))?
            .join("evidence");
        for line in recorded.iter().rev() {
            if line["event"] != "evidence"
                || line["kind"] != TEST_RESULT
                || line["result"] != "pass"
            {
                continue;
            }
            let Some(path) = line["path"].as_str() else {
                continue;
            };
            let relative = Path::new(path);
            if relative.parent() != Some(mine.as_path()) {
                continue;
            }
            let full = self.tree.join(relative);
            let bytes = std::fs::read(&full)
                .with_context(|| format!("read the record {}", full.display()))?;
            let record: serde_json::Value = serde_json::from_slice(&bytes)
                .with_context(|| format!("read the record {}", full.display()))?;
            let current = record["subject"]
                .as_str()
                .and_then(|subject| self.case.revision(subject));
            if record["kind"] == TEST_RESULT
                && let Some(result) = record["result"].as_str().filter(|result| *result == "pass")
                && let Some(revision) = record["subject_revision"].as_str()
                && current.as_deref() == Some(revision)
                && let Some(producer) = record["producer"].as_str()
                && let Some(at) = record["observed_at"].as_str()
            {
                let reference = crate::frontier::test_result_reference(
                    path, &bytes, result, revision, producer,
                );
                return Ok(Some((reference, at.to_owned())));
            }
        }
        Ok(None)
    }

    /// Records a refusal by `by`, its reason verbatim.
    fn refused(&self, events: &Events, by: &str, reason: &str) -> Result<()> {
        events.emit(
            self.steps,
            &Event::Refusal {
                by: by.to_owned(),
                reason: reason.to_owned(),
            },
        )
    }

    /// Records a step: by a validator, with the revision it observed, or by the harness, with
    /// the tokens it reported, their components and what it reported about the call (the
    /// model, cost, duration, API time, turns and start, each only when stated); which one,
    /// the action, what it reported.
    fn stepped(
        &self,
        events: &Events,
        by: SteppedBy<'_>,
        name: &str,
        action: &str,
        summary: &str,
    ) -> Result<()> {
        let (by, tokens, usage, revision, call, kept) = match by {
            SteppedBy::Validator(revision) => ("validator", None, None, revision, None, None),
            SteppedBy::Harness(tokens, usage, call, kept) => {
                ("harness", tokens, usage, None, call, kept)
            }
        };
        let (transcript, transcript_refused) = match kept {
            Some(Ok(path)) => (Some(path), None),
            Some(Err(why)) => (None, Some(why)),
            None => (None, None),
        };
        let call = call.map(|call| *call).unwrap_or_default();
        events.emit(
            self.steps,
            &Event::Step {
                by: by.to_owned(),
                name: name.to_owned(),
                action: action.to_owned(),
                summary: summary.to_owned(),
                tokens,
                usage,
                revision: revision.map(str::to_owned),
                model: call.model,
                version: call.version,
                cost_usd: call.cost_usd,
                duration_ms: call.duration_ms,
                api_ms: call.api_ms,
                turns: call.turns,
                started_at: call.started_at,
                transcript,
                transcript_refused,
            },
        )
    }

    /// Charges a harness step that failed, proposing nothing, with the tokens its session
    /// reported nonetheless (a session the hermetic floor held back ran to its end): the run's
    /// count, and a harness step line carrying them, their components and what the harness
    /// reported of the call, with no action and [`UNPROPOSED_SUMMARY`]; the error or refusal the
    /// run ends on says why. Nothing when the harness reported no tokens for the step. Written
    /// after the step's change was put back, as every step line is.
    fn charge_unproposed(
        &mut self,
        events: &Events,
        transcript: Option<std::result::Result<String, String>>,
    ) -> Result<()> {
        if self.harness.last_usage().is_none() {
            return Ok(());
        }
        self.charge_failed(events, transcript, UNPROPOSED_SUMMARY)
    }

    /// Records try `tried` of a judgement step whose session ended without the line naming its
    /// action ([`Judgement::NoAction`]): a harness step line with no action and
    /// [`NO_ACTION_SUMMARY`], naming the try and the bound, charged the tokens its session
    /// reported, as [`Run::charge_unproposed`] charges them. Nothing when the session reported
    /// no tokens, as for any failed step: a harness step line without a count leaves the run's
    /// tokens unknown to the scorecard, and one counting 0 would claim what nobody reported. Such
    /// a try is still a step of the record, which its stop line counts and the next line's step
    /// skips, with its transcript kept under its own step, and on the run's output. Written after
    /// the step's change was put back.
    fn charge_no_action(
        &mut self,
        events: &Events,
        transcript: Option<std::result::Result<String, String>>,
        tried: u32,
    ) -> Result<()> {
        if self.harness.last_usage().is_none() {
            return Ok(());
        }
        let summary = format!("{NO_ACTION_SUMMARY} (try {tried} of {NO_ACTION_TRIES})");
        self.charge_failed(events, transcript, &summary)
    }

    /// The harness step line of a step that failed and proposed nothing, with `summary`: the
    /// tokens its session reported, when it reported any, charged to the run and carried with
    /// their components and what the harness reported of the call.
    fn charge_failed(
        &mut self,
        events: &Events,
        transcript: Option<std::result::Result<String, String>>,
        summary: &str,
    ) -> Result<()> {
        let usage = self.harness.last_usage();
        let tokens = usage.map(|usage| usage.total());
        if let Some(tokens) = tokens {
            self.tokens += tokens;
        }
        self.stepped(
            events,
            SteppedBy::Harness(
                tokens,
                usage,
                self.harness.last_call().map(Box::new),
                transcript,
            ),
            self.harness.name(),
            "",
            summary,
        )
    }

    /// Why the step guard refuses `proposal` on its face, before the edit effect writes a byte
    /// of it to `tree`: an action neither `offered` nor among the `approvals`; a file changed
    /// for an approval, `repository.inspect` or an action that records a choice (`shape`),
    /// which change none, each file named; a path that is no plain path of the tree (absolute,
    /// with a `.` or `..` part, a part named `.git`); a path in the cases directory or on the
    /// judge `surface`; a non-test path for `tests.write`; a path reached through a link, or
    /// one at which the tree holds no regular file but something else; a path git ignores
    /// ([`crate::git::ignored_among`]), each named, which no commit would carry and the scratch
    /// copy never held, so what the tree holds there is the operator's and is never written,
    /// or the proposal whole when what git ignores could not be read. None when it passes.
    fn proposal_refusal(
        tree: &Path,
        proposal: &crate::harness::Proposal,
        offered: &[String],
        approvals: &[String],
        surface: &JudgeSurface,
        shape: &Shape,
    ) -> Option<String> {
        let action = &proposal.action;
        let approval = approvals.contains(action);
        if !offered.contains(action) && !approval {
            return Some(format!("{action} was not offered"));
        }
        let records_choice = !shape.choices_of(action).is_empty();
        if (approval || records_choice || action == "repository.inspect")
            && !proposal.files.is_empty()
        {
            let named: Vec<std::ffi::OsString> = proposal
                .files
                .iter()
                .map(|file| file.path.clone().into())
                .collect();
            return Some(format!(
                "{action}{} proposes {} changed path(s), and it changes none: {}",
                if records_choice {
                    " records a choice and"
                } else {
                    ""
                },
                proposal.files.len(),
                crate::git::paths_named(&named)
            ));
        }
        for file in &proposal.files {
            let path = file.path.as_str();
            if !loom_sdk::session::plain_relative(path) {
                return Some(format!(
                    "{action} proposes {path:?}, which is no plain path of the tree"
                ));
            }
            if is_under_cases(path) {
                return Some(format!(
                    "{action} proposes a path in the cases directory: {path}"
                ));
            }
            if surface.contains(path) {
                return Some(surface_refusal(action, &[path.to_owned()]));
            }
            if action == "tests.write" && !is_test_path(path) {
                return Some(format!("tests.write proposes a non-test path: {path}"));
            }
            let at = tree.join(path);
            if let Err(why) = no_link_on_the_way(tree, &at) {
                return Some(format!("{action} proposes {path}: {why:#}"));
            }
            if let Ok(meta) = std::fs::symlink_metadata(&at)
                && !meta.is_file()
            {
                return Some(format!(
                    "{action} proposes {path}, where the tree holds no regular file"
                ));
            }
        }
        let paths: Vec<&str> = proposal
            .files
            .iter()
            .map(|file| file.path.as_str())
            .collect();
        match crate::git::ignored_among(tree, &paths) {
            Ok(ignored) if ignored.is_empty() => None,
            Ok(ignored) => {
                let named: Vec<std::ffi::OsString> = ignored.into_iter().map(Into::into).collect();
                Some(format!(
                    "{action} proposes {} ignored by git, which no commit would carry and the scratch copy did not hold: {}",
                    if named.len() == 1 { "a path" } else { "paths" },
                    crate::git::paths_named(&named)
                ))
            }
            Err(error) => Some(format!(
                "{action} proposes {} path(s), and which of them git ignores could not be read: {error:#}",
                paths.len()
            )),
        }
    }

    /// The edit effect: writes each file of `proposal` to `tree`, its content whole, never
    /// through a link (`O_NOFOLLOW`), or removes it when the proposal carries none. Called
    /// only on a proposal [`Self::proposal_refusal`] passed.
    fn apply_proposal(tree: &Path, proposal: &crate::harness::Proposal) -> Result<()> {
        use std::io::Write as _;
        use std::os::unix::fs::OpenOptionsExt as _;
        for file in &proposal.files {
            let at = tree.join(&file.path);
            match &file.content {
                Some(text) => {
                    if let Some(parent) = at.parent() {
                        std::fs::create_dir_all(parent)
                            .with_context(|| format!("create {}", parent.display()))?;
                    }
                    let mut written = std::fs::OpenOptions::new()
                        .write(true)
                        .create(true)
                        .truncate(true)
                        .custom_flags(libc::O_NOFOLLOW)
                        .open(&at)
                        .with_context(|| format!("open {}", file.path))?;
                    written
                        .write_all(text.as_bytes())
                        .with_context(|| format!("write {}", file.path))?;
                }
                None => match std::fs::remove_file(&at) {
                    Ok(()) => {}
                    Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                    Err(error) => {
                        return Err(error).with_context(|| format!("remove {}", file.path));
                    }
                },
            }
        }
        Ok(())
    }

    /// The step of `validator`, as its effect performs the request `request` Commission's
    /// runtime handed it: the story's records in the store just before the step a grant
    /// authorises ([`Run::record_before`]), the observation guarded as a judgement step is, and
    /// each record it wrote submitted through the case's evidence port
    /// ([`crate::evidence::CaseEvidence`]) under the validator's name. Answers the stop that
    /// ends the run, or what was observed.
    fn validator_step(
        &mut self,
        place: StepPlace<'_>,
        validator: &dyn Validator,
        request: &str,
    ) -> Result<Taken> {
        let StepPlace {
            events,
            out,
            seq,
            recorded_before,
        } = place;
        // The story's records in the store, once a run, just before the step its grant
        // authorises: they ride to the base with the merge, and without both of them the
        // step is not taken.
        if !*recorded_before {
            match self.record_before(events, validator.action(), out)? {
                Before::Ungranted => {}
                Before::Recorded => *recorded_before = true,
                Before::Missing(why) => {
                    self.refused(events, BY_TAKT, &why)?;
                    writeln!(out, "  refused: {why}")?;
                    return Ok(Taken::Stop(Stop::StepRefused(why)));
                }
            }
        }
        self.steps += 1;
        let revision = self
            .case
            .revision(validator.subject())
            .context("the validator's subject has a revision")?;
        // A record is bound to the revision, `HEAD`: a tree with uncommitted changes
        // outside the cases directory would be observed on other content than the one
        // the record names. Nothing is observed, and the changes are left as they are.
        let uncommitted = crate::git::uncommitted(&self.tree)?;
        if !uncommitted.is_empty() {
            let why = format!(
                "{} would observe {} with uncommitted changes: {}",
                validator.action(),
                &revision[..8.min(revision.len())],
                uncommitted.join(", ")
            );
            self.stepped(
                events,
                SteppedBy::Validator(None),
                validator.name(),
                validator.action(),
                "nothing observed",
            )?;
            self.refused(events, BY_TAKT, &why)?;
            writeln!(out, "step {}: {} {{}}", self.steps, validator.action())?;
            writeln!(out, "  refused: {why}; nothing observed")?;
            return Ok(Taken::Stop(Stop::StepRefused(why)));
        }
        // What a validator runs may be code a model wrote (the test command, a gate's
        // commands): the cases directory is guarded as around a judgement step, and the
        // tree must be as it was before the observation: the guarded part of `.git`
        // (hooks, config, `info/`), `HEAD`, every branch and tag, the index and every path outside the cases
        // directory; ignored files it made (a build's output) may stay. The validator's
        // own writes under its case are the only change admitted; anything else is put
        // back, and the observation refused with nothing recorded. `.git` is put back
        // before any git runs again: git runs what was planted there (a hook,
        // `core.fsmonitor`). The judge surface is snapshotted as around a judgement
        // step: what the observation left there that git does not list (a FIFO, a file
        // in an ignored directory) or that git lists as ignored is put back and the
        // observation refused like any other foreign write. So are the files outside
        // the tree that decide how cargo and rustup build and run the tests
        // ([`Outside`]): one that differs from what the run found when it began (a
        // change no step could keep, by another route) and nothing is observed or
        // recorded; one the observation changed is put back and the observation refused.
        let outside_kept = Outside::read(&self.tree, &self.outside_dirs)?;
        let outside_moved = self.outside.differing(&outside_kept);
        if !outside_moved.is_empty() {
            let why = format!(
                "{} would observe {} while what cargo or rustup reads outside the tree, a program on `PATH`, or what Python imports from there differs from what the run found when it began: {}",
                validator.action(),
                &revision[..8.min(revision.len())],
                outside_named(&outside_moved)
            );
            self.stepped(
                events,
                SteppedBy::Validator(None),
                validator.name(),
                validator.action(),
                "nothing observed",
            )?;
            self.refused(events, BY_TAKT, &why)?;
            writeln!(out, "step {}: {} {{}}", self.steps, validator.action())?;
            writeln!(out, "  refused: {why}; nothing observed")?;
            return Ok(Taken::Stop(Stop::StepRefused(why)));
        }
        let checkpoint = self.checkpoint(false)?;
        let kept = snapshot_cases(&self.tree)?;
        let globs = self.judge_list()?.unwrap_or_default();
        let (surface, surface_kept) =
            JudgeSurface::read(&self.runs(), globs, &self.modules, &self.tree)?;
        let observed = validator
            .observe_within(&self.tree, &revision, seq, self.deadline())
            .with_context(|| format!("{} ({})", validator.action(), validator.name()));
        let failed = validator.take_failed();
        let returned = Instant::now();
        // The way to the case's directory is read before anything is written under it: a test
        // command that put a link on it (`.takt`, a cases root) is refused and undone below.
        let way = self.case.check_way().err();
        // What the observation left running is killed before anything is looked at;
        // one that could not be killed is named with the rest of what it left.
        let swept = crate::forge::sweep_left();
        let git_dir = checkpoint.put_back_git_dir(&self.tree);
        let mut planted = git_dir.touched;
        planted.extend(
            git_dir
                .failed
                .into_iter()
                .map(|path| format!("not put back: {path}")),
        );
        if let Err(error) = swept {
            planted.push(format!("processes still running: {error:#}"));
        }
        // What the observation left: on the judge surface, in the cases directory, in the
        // tree as git sees it and outside the tree. A read that fails (a file the run may
        // not read) discards everything the observation did before the run ends on the
        // error.
        let kept_all = Kept {
            checkpoint: &checkpoint,
            tree: &self.tree,
            cases_kept: &kept,
            surface: &surface,
            surface_kept: &surface_kept,
            outside: &outside_kept,
            outside_dirs: &self.outside_dirs,
        };
        let seen = (|| -> Result<_> {
            let on_surface = differing(&surface_kept, &surface.snapshot(&self.tree)?);
            let touched = differing(&kept, &snapshot_cases(&self.tree)?);
            let change = checkpoint.change(&self.tree)?;
            let moved = outside_kept.differing(&Outside::read(&self.tree, &self.outside_dirs)?);
            Ok((on_surface, touched, change, moved))
        })();
        let (on_surface, touched, change, outside_moved) = match seen {
            Ok(seen) => seen,
            Err(error) => {
                return Err(match undo_step(&self.tree, &kept_all) {
                    Ok(_) => error.context(format!(
                        "{}: what the observation left could not be read; everything it changed was discarded",
                        validator.action()
                    )),
                    Err(undo) => error.context(format!(
                        "{}: what the observation left could not be read, and its change could not be discarded: {undo:#}",
                        validator.action()
                    )),
                });
            }
        };
        let own: Vec<PathBuf> = match self.case.dir.strip_prefix(&self.tree) {
            Ok(case) => validator
                .writes()
                .iter()
                .map(|path| case.join(path))
                .collect(),
            Err(_) => Vec::new(),
        };
        let mut foreign = cases_named(
            &self.tree,
            touched.iter().filter(|path| !own.contains(path)),
        );
        let mut outside: Vec<String> = planted;
        outside.extend(change.paths.iter().cloned());
        if change.staged {
            outside.push("the index".to_owned());
        }
        if let Some(moved) = &change.head {
            outside.push(format!("HEAD, moved {moved}"));
        }
        outside.extend(change.refs.iter().cloned());
        for path in on_surface
            .iter()
            .map(|path| path.display().to_string())
            .chain(
                change
                    .ignored_added
                    .iter()
                    .chain(&change.ignored_changed)
                    .chain(&change.ignored_removed)
                    .filter(|path| {
                        let text = path.to_string_lossy();
                        surface.contains(&text) || takt_guard(&text)
                    })
                    .map(|path| crate::git::shown(path)),
            )
        {
            if !outside.contains(&path) {
                outside.push(path);
            }
        }
        outside.extend(outside_moved.iter().map(|path| path.display().to_string()));
        if let Some(way) = &way {
            outside.push(format!("{way:#}"));
        }
        if !foreign.is_empty() || !outside.is_empty() {
            let lock_remedy = untracked_cargo_lock(&self.tree, &change.paths);
            let left = undo_step(&self.tree, &kept_all)?.left;
            foreign.extend(outside);
            foreign.extend(left.iter().map(|path| {
                format!("not put back (a program or import directory takt never writes, a link on its way, or a file too large to keep a copy of): {}", path.display())
            }));
            let mut why = format!(
                "{} wrote what its validator does not: {}",
                validator.action(),
                foreign.join(", ")
            );
            if lock_remedy {
                why.push_str(&format!("; {CARGO_LOCK_REMEDY}"));
            }
            if let Err(error) = &observed {
                why.push_str(&format!("; the observation failed too: {error:#}"));
            }
            // Written after the cases directory was put back, which would take a
            // line written before it along.
            self.stepped(
                events,
                SteppedBy::Validator(Some(&revision)),
                validator.name(),
                validator.action(),
                "nothing recorded",
            )?;
            self.refused(events, BY_TAKT, &why)?;
            writeln!(out, "step {}: {} {{}}", self.steps, validator.action())?;
            writeln!(
                out,
                "  refused: {why}; the cases directory was put back, the change discarded and nothing recorded"
            )?;
            return Ok(Taken::Stop(Stop::StepRefused(why)));
        }
        // An observation that failed when the hour budget ended, its child killed by it,
        // records nothing, and the run stops on the budget.
        if let Err(error) = &observed
            && self.deadline().is_some_and(|deadline| returned >= deadline)
            && let Some(h) = self.options.hours
        {
            restore(&self.tree, &self.tree, &kept, &touched)?;
            let why = format!(
                "step {}: the hour budget ran out during {}; nothing recorded: {error:#}",
                self.steps,
                validator.action()
            );
            self.stepped(
                events,
                SteppedBy::Validator(Some(&revision)),
                validator.name(),
                validator.action(),
                "nothing recorded",
            )?;
            self.refused(events, BY_TAKT, &why)?;
            writeln!(out, "step {}: {} {{}}", self.steps, validator.action())?;
            writeln!(out, "  {why}")?;
            return Ok(Taken::Stop(Stop::Hours(h)));
        }
        // A test command that changed or removed `.takt/.gitignore` or `.takt` was refused and
        // undone above, so this writes the ignore file only when it is absent, as at open, which
        // keeps the case out of `git status`; an observation that failed is written the same way
        // before its error ends the run.
        let ignore = if self
            .case
            .dir
            .starts_with(self.tree.join(crate::case::CASES_DIR))
        {
            crate::case::write_takt_files(&self.tree)
        } else {
            Ok(())
        };
        let (observation, waits) = observed?;
        ignore?;
        self.stepped(
            events,
            SteppedBy::Validator(Some(&revision)),
            validator.name(),
            validator.action(),
            &observation.summary,
        )?;
        writeln!(out, "step {}: {} {{}}", self.steps, validator.action())?;
        // A `tests.run` summary keeps its failed tests whole; its printed line is bounded.
        if validator.action() == crate::validator::TestsRun::ACTION {
            let line = crate::validator::tests_line(&observation.summary);
            writeln!(out, "  effect: {line}")?;
        } else {
            writeln!(out, "  effect: {}", observation.summary)?;
        }
        // What the validator retains lands before the records that refer to it: a
        // retention that fails ends the run with nothing written for the step.
        if let Some(artifact) = &observation.artifact {
            let dir = self.case.dir.join("artifacts");
            std::fs::create_dir_all(&dir).with_context(|| format!("create {}", dir.display()))?;
            let path = dir.join(&artifact.name);
            std::fs::write(&path, &artifact.bytes)
                .with_context(|| format!("write {}", path.display()))?;
            writeln!(
                out,
                "  artifact: {}",
                path.strip_prefix(&self.tree).unwrap_or(&path).display()
            )?;
        }
        let evidence = CaseEvidence::new(&mut self.case, vec![validator.name().to_owned()])?;
        for record in &observation.records {
            let written = evidence.submit(validator.name(), record, Some(request))?;
            events.emit(
                self.steps,
                &Event::Evidence {
                    id: record.id.clone(),
                    kind: record.kind.clone(),
                    result: record.result.clone(),
                    path: written
                        .strip_prefix(&self.tree)
                        .unwrap_or(&written)
                        .display()
                        .to_string(),
                    failed: if record.result.as_deref() == Some("fail") {
                        failed.clone()
                    } else {
                        Vec::new()
                    },
                },
            )?;
            writeln!(
                out,
                "  evidence: {} {}",
                record.kind,
                record.result.as_deref().unwrap_or("-")
            )?;
        }
        Ok(Taken::Observed {
            revision,
            observation,
            waits,
        })
    }

    /// What the `tests.run` observation `observation` at `revision` stops, from its record's
    /// result and the failed tests its summary names ([`crate::validator::failures_in`]), with
    /// `so_far` what the case's record held as the run opened ([`TestsSoFar`]), kept up to date:
    /// [`Stop::RedBase`] when it failed before any effect, at a revision no step has changed
    /// ([`Run::before_any_effect`]), and is the case's first result (no `tests.run` result
    /// recorded in the case before it) or the check again after a `RedBase` the case's last run
    /// stopped on, failing the same tests; unless the run goes on past it
    /// ([`Run::continue_past_red_base`]). Else [`Stop::RepeatedTestFailure`] when it leaves the
    /// row of failures of the same tests ([`Failing::after`], seeded from the case's record) at
    /// [`REPEATED_FAILURES`] or more: after a `RepeatedTestFailure` the check again at the same
    /// revision failing the same set stops the same way, and a pass or another set goes on. An
    /// observation that wrote no record changes nothing but the check again, which it uses up.
    fn tests_stop(
        &self,
        revision: &str,
        observation: &crate::validator::Observation,
        so_far: &mut TestsSoFar,
    ) -> Result<Option<Stop>> {
        let again = so_far.again.take();
        let Some(result) = observation
            .records
            .iter()
            .find(|record| record.kind == crate::validator::TestsRun::KIND)
            .and_then(|record| record.result.as_deref())
        else {
            return Ok(None);
        };
        let first = std::mem::replace(&mut so_far.first, false);
        if result != "fail" {
            so_far.failing = None;
            return Ok(None);
        }
        let failures = crate::validator::failures_in(&observation.summary);
        let red_again = match &again {
            Some(Again::RedBase(set)) => {
                *set == failures.tests.iter().cloned().collect::<BTreeSet<_>>()
            }
            _ => false,
        };
        if (first || red_again) && self.stops_red_base && self.before_any_effect(revision)? {
            return Ok(Some(Stop::RedBase {
                revision: revision.to_owned(),
                tests: failures.tests,
                rerun: failures.rerun,
            }));
        }
        Failing::after(&mut so_far.failing, revision, Some(failures.tests));
        Ok(so_far
            .failing
            .as_ref()
            .filter(|row| row.count >= REPEATED_FAILURES)
            .map(|row| Stop::RepeatedTestFailure {
                tests: row.tests.clone(),
                count: row.count,
            }))
    }

    /// Whether `revision` is before any effect of the case: its branch reaches it from the base
    /// commit the case pinned at open through takt's store commits alone ([`is_store_commit`]:
    /// the take), so no step has changed a file. False for a case that pins no base.
    fn before_any_effect(&self, revision: &str) -> Result<bool> {
        let Some(base) = crate::case::Branch::read(&self.case.dir)
            .ok()
            .and_then(|branch| branch.base_commit)
        else {
            return Ok(false);
        };
        let story = self.case.story()?;
        let listed = crate::git::run(
            &self.tree,
            &[
                "rev-list",
                "--end-of-options",
                &format!("{base}..{revision}"),
            ],
        )?;
        for id in listed.lines().filter(|id| !id.is_empty()) {
            let commit = crate::git::commit(&self.tree, id)?;
            if !story
                .as_deref()
                .is_some_and(|story| is_store_commit(&commit, story))
            {
                return Ok(false);
            }
        }
        Ok(true)
    }

    /// The loop, the case held.
    fn run_held(&mut self, events: &Events, out: &mut dyn Write) -> Result<Stop> {
        let inputs: Vec<String> = self
            .options
            .inputs
            .iter()
            .map(|(k, v)| format!("{k}={v}"))
            .collect();
        // This run's number in the case, which names its step transcripts' directory: one
        // more than the `opened` lines the case's record holds before this run writes its own.
        let run_number = crate::harness::run_number(events.path())?;
        // What the case's record holds of its tests before this run's own lines
        // ([`Run::tests_stop`]); a record that does not read names no first result and no row,
        // so it stops nothing, and a `RedBase` the run goes on past is not checked again.
        let mut tests_so_far = events.read().map_or(
            TestsSoFar {
                first: false,
                failing: None,
                again: None,
            },
            |record| TestsSoFar::read(&record),
        );
        if !self.stops_red_base && matches!(tests_so_far.again, Some(Again::RedBase(_))) {
            tests_so_far.again = None;
        }
        // The layers the run can read as it opens (ADR 0101), each from what ran: takt's own
        // revision as the caller gave it. The harness's version and the model are read from
        // the harness's stream at each step, so their layers are on the step lines: takt runs
        // the harness program in a judgement step and nowhere else, never for its version.
        let mut layers = std::collections::BTreeMap::new();
        if let Some(binary) = &self.binary {
            layers.insert("takt".to_owned(), binary.clone());
        }
        // The adapter every child of this run starts through, named on the first line and
        // in the record so a reader knows what confined the evidence.
        let confinement = crate::confinement::current().name().to_owned();
        let commit_record = self.commit_plan_now().record();
        events.emit_opened_with(
            self.steps,
            &Event::Opened {
                case: self.case.id.clone(),
                protocol: self.case.protocol.id.clone(),
                story: match &self.unit {
                    Unit::Story(story) => Some(story.id.clone()),
                    Unit::Intent(_) => None,
                },
                inputs: inputs.clone(),
                base: crate::case::Branch::read(&self.case.dir)
                    .ok()
                    .and_then(|branch| branch.base_commit),
                binary: self.binary.clone(),
                layers: Some(layers),
                confinement: Some(confinement.clone()),
                // The endpoint the harness is pointed at, by reference: never a token.
                endpoint: self.harness.endpoint_reference(),
                records: Some(self.kept_records().name().to_owned()),
                // The scan adapter every proposal and the change so far pass.
                scan: Some(self.scan.name().to_owned()),
            },
            self.llm_digest.as_deref(),
            &commit_record,
        )?;
        writeln!(
            out,
            "case {} on {} in {}; inputs {}; confinement {confinement}",
            self.case.id,
            self.case.protocol.id,
            self.tree.display(),
            if inputs.is_empty() {
                "none".to_owned()
            } else {
                inputs.join(" ")
            }
        )?;
        // One sequence per case, continued across its runs: no decision file is overwritten,
        // so the latest by name is the latest written, and a record id, numbered by the same
        // sequence, never repeats.
        let mut seq = self.case.evaluations()?;
        // The first decision this run writes: the `default` profile removes only the decisions
        // from here on, never one an earlier run wrote.
        let first_decision = seq + 1;
        let mut last_idle: Option<String> = None;
        let shape = Shape::of(&self.case.protocol, &self.options.inputs)?;
        let keeps_state = self.harness.keeps_state();
        // The validators that observed without writing a record, each with the revision it
        // observed at: not due again there, because the next observation would end the same
        // way; a commit moves the revision and makes them due again.
        let mut observed_without_record: BTreeSet<(String, String)> = BTreeSet::new();
        // Of those, the ones whose observation waits, as `(name, action, revision, local)`,
        // `local` what a wait on something local waits for ([`Validator::waits_locally`]): on
        // the forge, observed again after a pause, within the budget, and meanwhile no
        // judgement step is offered, because what is missing is not the harness's to make; when
        // every wait is local, the run stops at once instead.
        let mut awaited: Vec<(String, String, String, Option<String>)> = Vec::new();
        // Whether the story's records were written before a granted step this run.
        let mut recorded_before = false;
        let all_local = |awaited: &[(String, String, String, Option<String>)]| {
            awaited.iter().all(|(_, _, _, local)| local.is_some())
        };
        let waiting = |awaited: &[(String, String, String, Option<String>)],
                       budget: Option<Stop>| {
            let mut on: Vec<String> = Vec::new();
            for (_, action, _, _) in awaited {
                if !on.contains(action) {
                    on.push(action.clone());
                }
            }
            Stop::Waiting {
                on,
                budget: budget.map(Box::new),
                local: if all_local(awaited) {
                    awaited
                        .iter()
                        .filter_map(|(_, _, _, local)| local.clone())
                        .collect()
                } else {
                    Vec::new()
                },
            }
        };
        loop {
            seq += 1;
            let decision = self.case.evaluate(seq)?;
            // Each fact is recorded before it is printed, here and below: an `out` that fails
            // ends the run on an error, never with a fact it printed missing from the record.
            events.emit(
                self.steps,
                &Event::Decision {
                    seq,
                    file: format!("decisions/{seq:04}.json"),
                    frontier: decision.frontier_line(),
                },
            )?;
            // The `default` profile keeps the decision just written and none this run wrote before
            // it, once its `decision` line is in the record; the sequence goes on from it, so no
            // later decision reuses a number.
            if self.kept_records() == Records::Default {
                self.case.keep_only_decision(first_decision, seq)?;
            }
            writeln!(out, "{}", decision.frontier_line())?;
            let state = format!(
                "{:?}{:?}{:?}",
                decision.claims, decision.obligations, decision.actions
            );
            if last_idle.as_deref() == Some(state.as_str()) {
                return Ok(Stop::NoProgress);
            }
            // A case with a legitimate outcome is not stepped: Commission's runtime ends the run
            // completed at once, before any check below.
            let open = decision.legitimate().is_empty();
            let admissible = decision.admissible();
            // The judgement actions: the admissible ones no validator owns, but none whose evidence
            // only a validator writes ([`Shape::for_harness`]). A harness that keeps nothing
            // between steps is offered an action that only reads only when the action records a
            // choice ([`Shape::for_stateless`]).
            let offered: Vec<String> = admissible
                .iter()
                .filter(|a| !self.validators.iter().any(|v| v.action() == a.as_str()))
                .filter(|a| shape.for_harness(a))
                .filter(|a| keeps_state || shape.for_stateless(a))
                .cloned()
                .collect();
            // A deterministic step first: a validator due at this decision.
            let mut due = None;
            // After a stop on the case's tests, its tests are observed again first, its record
            // there or not, before any judgement step; once ([`TestsSoFar`]).
            if open && tests_so_far.again.is_some() {
                due = self.validators.iter().find(|validator| {
                    validator.action() == crate::validator::TestsRun::ACTION
                        && admissible.iter().any(|a| a == validator.action())
                });
                if due.is_none() {
                    tests_so_far.again = None;
                }
            }
            if open && due.is_none() {
                for validator in self.validators {
                    if is_due(
                        &self.case,
                        &admissible,
                        &observed_without_record,
                        validator.as_ref(),
                    )? {
                        due = Some(validator);
                        break;
                    }
                }
            }
            // No validator is due, and one waits on the forge whose action is still admissible:
            // the run pauses and observes it again, offering no judgement step meanwhile. A run
            // with no step or hour budget has nothing to wait in and stops waiting at once; the
            // case stays open for the next run, as it does when every wait left is on something
            // local. A budget spent stops the wait first.
            if open && due.is_none() {
                awaited.retain(|(_, action, _, _)| admissible.contains(action));
                if !awaited.is_empty() {
                    if let Some(stop) = self.budget_spent() {
                        return Ok(waiting(&awaited, Some(stop)));
                    }
                    let pause = self.pause().filter(|_| !all_local(&awaited));
                    let Some(pause) = pause else {
                        return Ok(waiting(&awaited, None));
                    };
                    let on: Vec<&str> = awaited
                        .iter()
                        .map(|(_, action, _, _)| action.as_str())
                        .collect();
                    events.emit(
                        self.steps,
                        &Event::Waiting {
                            on: on.iter().map(|action| (*action).to_owned()).collect(),
                            seconds: pause.as_secs_f64(),
                        },
                    )?;
                    writeln!(
                        out,
                        "  waiting: {:.1} s on the forge, then {} again",
                        pause.as_secs_f64(),
                        on.join(", ")
                    )?;
                    let paused = Instant::now();
                    std::thread::sleep(pause);
                    self.waited += paused.elapsed();
                    for (name, _, revision, _) in &awaited {
                        observed_without_record.remove(&(name.clone(), revision.clone()));
                    }
                    continue;
                }
            }
            // Two checks of takt's own come before every step it hands Commission's runtime with
            // no validator due, ahead of the runtime's budget and host stops: Loom's frontier
            // relates no action to a claim, so neither stop is the runtime's to find (loom
            // `AGENTS.md` § Commission; approval-record:loop-pre-checks-ahead-of-loom). A due
            // validator's step goes first: what it observes may still decide a claim.
            if open && due.is_none() {
                // No step when only a grant stands between the case and its outcome: every claim
                // still open is decided by evidence only an action waiting for an approval
                // produces. The run stops there at once and spends nothing on asking.
                let waiting_for_grant = decision.approval_required();
                if !waiting_for_grant.is_empty()
                    && let Some(behind) = shape.only_behind(&decision, &waiting_for_grant)
                {
                    return Ok(Stop::ApprovalRequired(behind));
                }
                // No step when no step can move the case: nothing offered produces what an open
                // obligation reads, and a claim left is decided only by an admissible action the
                // run has no performer for (no forge, no gates document). Checked at each step,
                // never at open, so a `--forge none` run works up to its change read ahead first.
                // On when the caller asked for it ([`Run::stop_when_unperformable`]): the CLI does.
                if self.stops_unperformable
                    && let Some((claim, action)) =
                        shape.unperformable(&decision, &offered, |action| {
                            self.validators.iter().any(|v| v.action() == action)
                        })
                {
                    return Ok(Stop::Unperformable {
                        claim,
                        performer: performer_of(&action),
                    });
                }
            }
            // The step itself is one call of Commission's runtime of loom-sdk
            // (`run_until_blocked`), with a budget of one step: the composite executor proposes
            // the due validator's action, else asks the harness for a judgement step, else
            // proposes nothing; the governor admits the request, the case's authority allows
            // one that requires a capability, and the effect performs it. The run's own budget
            // and the free disk are the host's stop, asked before the step
            // ([`TurnContext::host_stop`]).
            let taker = match due {
                Some(validator) => Taker::Validator(validator.as_ref()),
                None if open && !offered.is_empty() => Taker::Harness,
                None => Taker::Nobody,
            };
            let approvals: Vec<String> = decision
                .approval_required()
                .into_iter()
                .filter(|a| self.options.gate.as_deref() == Some(a.as_str()))
                .collect();
            let (dir, id, protocol) = (
                self.case.dir.clone(),
                self.case.id.clone(),
                self.case.protocol.clone(),
            );
            let authority = CaseAuthority::new(&dir, &id);
            let n = TURNS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            let seed = format!("{id}/{seq}/{}/{n}", crate::now());
            let at = crate::now();
            let governor = CanonGovernor::new(CaseFiles::new(&dir, &id, &protocol.id))
                .with_protocol_yaml(&protocol.id, &protocol.yaml)
                .map_err(|error| anyhow!("register the protocol {}: {error}", protocol.id))?
                .with_evaluation_time(move |_: &CaseId| Ok(Some(at.clone())));
            let mut ids = TurnIds { seed, n: 0 };
            let commission = Commission::new(CommissionData {
                commission_id: CommissionId(ids.next("commission")),
                agent_revision_id: AgentRevisionId(ids.next("agent-revision")),
                case_id: CaseId(id.clone()),
                principal: PrincipalId("takt".to_owned()),
                authority_context: AuthorityContext(cjson::Value::Null),
            });
            let mut runs = RunStore::new({
                let seed = ids.seed.clone();
                let mut n = 0u64;
                move || {
                    n += 1;
                    RunId(Uuid(uuid_from(&format!("{seed}/run/{n}"))))
                }
            });
            let turn = std::cell::RefCell::new(Turn {
                run: &mut *self,
                events,
                out: &mut *out,
                seq,
                recorded_before: &mut recorded_before,
                taker,
                decision: &decision,
                offered: &offered,
                approvals: &approvals,
                shape: &shape,
                run_number,
                host: None,
                came: None,
                proposed: None,
                failed: None,
            });
            let ended = {
                let executor = TurnExecutor(&turn);
                let effect = TurnEffect(&turn);
                let mut context = TurnContext { turn: &turn, ids };
                run_until_blocked(
                    &governor,
                    &executor,
                    &authority,
                    &effect,
                    &commission,
                    &mut runs,
                    &mut context,
                )
            };
            let Turn {
                host,
                came,
                proposed,
                failed,
                ..
            } = turn.into_inner();
            if let Some(error) = failed {
                return Err(error);
            }
            let ended = ended.map_err(|error| anyhow!("Commission's runtime failed: {error}"))?;
            match came {
                Some(Came::Taken(Taken::Stop(stop))) => return Ok(stop),
                Some(Came::Taken(Taken::Observed {
                    revision,
                    observation,
                    waits,
                })) => {
                    let Some(validator) = due else {
                        bail!("an observation came of a step no validator took");
                    };
                    // A validator that wrote no record (the gate was unknown) is not due again at
                    // this revision. The pass counts as idle only when nothing else can move the
                    // decision, no judgement step offered, no other validator due and none
                    // waiting on the forge: then the next pass would see the same decision, and
                    // the run stops there as no progress. One that waits on the forge is due
                    // again after the pause above; when every wait is on something local, the
                    // run stops here, with no pause and no further decision.
                    awaited.retain(|(name, _, _, _)| name != validator.name());
                    // A failing `tests.run` before any effect, or the last of a run of failures
                    // on the same tests, stops the run before another judgement step.
                    if validator.action() == crate::validator::TestsRun::ACTION
                        && let Some(stop) =
                            self.tests_stop(&revision, &observation, &mut tests_so_far)?
                    {
                        return Ok(stop);
                    }
                    if observation.records.is_empty() {
                        observed_without_record
                            .insert((validator.name().to_owned(), revision.clone()));
                        if waits {
                            awaited.push((
                                validator.name().to_owned(),
                                validator.action().to_owned(),
                                revision,
                                validator
                                    .waits_locally()
                                    .then(|| observation.summary.clone()),
                            ));
                            if all_local(&awaited) {
                                return Ok(waiting(&awaited, None));
                            }
                            last_idle = None;
                            continue;
                        }
                        let mut another_due = false;
                        for v in self.validators {
                            if is_due(
                                &self.case,
                                &admissible,
                                &observed_without_record,
                                v.as_ref(),
                            )? {
                                another_due = true;
                                break;
                            }
                        }
                        last_idle = (offered.is_empty() && !another_due && awaited.is_empty())
                            .then_some(state);
                    } else {
                        last_idle = None;
                    }
                    continue;
                }
                Some(Came::Kept(moved)) => {
                    last_idle = (!moved).then_some(state);
                    continue;
                }
                None => {}
            }
            // A judgement step proposed and never handed to its effect: a request for an
            // approval ends the run there, whatever the authority answered; any other is put
            // back whole and refused.
            if let Some(proposed) = proposed {
                if proposed.ask {
                    return Ok(Stop::ApprovalRequired(vec![proposed.report.action.clone()]));
                }
                let moved = undo_step(&self.tree, &proposed.held.kept())
                    .map(|undone| undone.aside.named())?;
                self.record_proposed(events, &proposed)?;
                let why = format!(
                    "{} was not handed to its effect: Commission's runtime ended {:?}{moved}",
                    proposed.report.action, ended.outcome
                );
                self.refused(events, BY_TAKT, &why)?;
                writeln!(out, "  refused: {why}; the change was discarded")?;
                return Ok(Stop::StepRefused(why));
            }
            // The host's stop the runtime ended on: a budget spent while a validator waits on the
            // forge is that wait's.
            if let Some((stop, budget)) = host {
                return Ok(if budget && !awaited.is_empty() {
                    waiting(&awaited, Some(stop))
                } else {
                    stop
                });
            }
            if let Some(validator) = due {
                let why = format!(
                    "{} was not handed to its validator {}: Commission's runtime ended {:?}",
                    validator.action(),
                    validator.name(),
                    ended.outcome
                );
                self.refused(events, BY_TAKT, &why)?;
                writeln!(out, "  refused: {why}")?;
                return Ok(Stop::StepRefused(why));
            }
            return Ok(match ended.outcome {
                RunOutcome::Completed(completed) => Stop::Completed(completed.outcome),
                RunOutcome::EffectRefused(refused) => Stop::StepRefused(refused.reason),
                // Nothing to step: what waits for an approval on the last frontier, or nothing
                // admissible.
                RunOutcome::NoAdmissibleAction(_)
                | RunOutcome::NoPerformableAction(_)
                | RunOutcome::NeedsExternalEvidence(_)
                | RunOutcome::NeedsAuthority(_)
                | RunOutcome::AwaitingApproval(_)
                | RunOutcome::IdleBound(_) => {
                    let approvals: Vec<String> = match &ended.last_frontier {
                        Some(frontier) => frontier
                            .actions
                            .iter()
                            .filter(|listed| listed.status == ActionStatus::ApprovalRequired)
                            .map(|listed| listed.action.clone())
                            .collect(),
                        None => decision.approval_required(),
                    };
                    if approvals.is_empty() {
                        Stop::NothingAdmissible
                    } else {
                        Stop::ApprovalRequired(approvals)
                    }
                }
                other => {
                    let why = format!("Commission's runtime ended {other:?}");
                    self.refused(events, BY_TAKT, &why)?;
                    writeln!(out, "  refused: {why}")?;
                    Stop::StepRefused(why)
                }
            });
        }
    }

    /// A judgement step up to its proposal, as the executor of Commission's runtime of loom-sdk
    /// takes it ([`TurnExecutor`]): the harness called on the actions `offered`, the guards that
    /// judge what it did to the tree, and either a stop (a refusal, the hour budget ended in the
    /// step) or the action it proposes, its change held in the tree with what puts it back. The
    /// change is kept only once the runtime hands the request to the effect ([`Run::keep`]).
    ///
    /// # Errors
    /// A failure to read, put back or record the step; the run ends on it.
    #[allow(clippy::too_many_arguments)]
    fn harness_turn(
        &mut self,
        events: &Events,
        out: &mut dyn Write,
        decision: &crate::Decision,
        offered: &[String],
        approvals: &[String],
        shape: &Shape,
        run_number: u64,
    ) -> Result<Judgement> {
        self.steps += 1;
        let choices: Vec<String> = offered
            .iter()
            .filter(|a| !shape.choices_of(a).is_empty())
            .cloned()
            .collect();
        // The offered actions the case holds a grant for at its current revision: admissible now
        // because they were granted, where each needed an approval before ([`CaseDir::grants`]).
        let mut granted: Vec<String> = Vec::new();
        for action in offered {
            if self.case.grants(action)? {
                granted.push(action.clone());
            }
        }
        let change = change_so_far(
            &self.tree,
            &self.case.dir,
            || events.read(),
            None,
            self.change_scan.as_ref(),
            out,
        )?;
        let failed = failed_checks(&events.read()?);
        // The repository's instruction files are read at the commit the case forked from.
        let base = crate::case::Branch::read(&self.case.dir)?.base_commit;
        let ask = self.commit_plan_now().ask();
        let mut context = StepContext {
            tree: &self.tree,
            intent: &self.options.intent,
            offered,
            approvals,
            choices: &choices,
            granted: &granted,
            decision,
            protocol: &self.case.protocol,
            step: self.steps as usize,
            time_left: None,
            change: &change,
            failed: &failed,
            scope_revision: None,
            base_revision: base.as_deref(),
            commit: ask.as_ref(),
        };
        // The step is judged by what it did to the tree, against a checkpoint taken just
        // before it: `HEAD`, the index, every path already uncommitted (a validator's run
        // may leave one) with what is at it, the ignored paths, and the guarded part of the
        // git directory. An accepted step commits exactly the paths it changed; a refused
        // one, or one whose harness failed, is put back to the checkpoint whole.
        //
        // The cases directory is the kernel's, every case under it: its files are what
        // the evaluations read, and a harness that wrote into it (a grant, a record, a
        // branch number, the protocol, for this case or a sibling waiting for its grant)
        // would decide what only a validator or the operator decides. It is snapshotted
        // before the step and put back after, whatever the step reported and however it
        // ended; a step that touched it is refused.
        let kept = snapshot_cases(&self.tree)?;
        // The judge surface likewise ([`JudgeSurface`]): the planning store and everything
        // else under `.engineering/` but the cases (the project file that says where the
        // store is kept, the gates and the assertions project file among it), whose
        // artifacts and evidence the store's lifecycle moves a story on and which only
        // takt's own writes change, each its own commit on the case's branch: the take when
        // the case opens ([`Run::take_next`]); and, riding above the implementation revision
        // to the base with the pull request ([`crate::case::approvals_only`]), a grant's
        // record (`takt approve`) and the story's `test_result` and move to `implemented`,
        // written before the step the case's grant authorises or, without one, once the
        // case is accepted ([`Run::record_on`]); and what
        // decides the validators, which a step that changed it would decide its own
        // verdict with: the cargo configuration, the toolchain, build scripts, the CI
        // workflows, pytest's configuration, the test command's own script and its build
        // tool's file, and what the repository's own list adds. Matched
        // whatever the case. A step whose change git sees there (a file, a link, a mode, a
        // rename), or that left anything there the snapshot sees differ (a FIFO, which git
        // does not list, a file in an ignored directory), is refused, and put back to the
        // snapshot taken here.
        // A run with no set-aside root takes no judgement step: a rewrite of a file inside an
        // ignored directory would have nowhere to go when the step is discarded.
        if self.set_aside.is_none() {
            let why = NO_SET_ASIDE.to_owned();
            self.refused(events, BY_TAKT, &why)?;
            writeln!(
                out,
                "step {}: refused before the harness: {why}",
                self.steps
            )?;
            return Ok(Judgement::Stop(Stop::StepRefused(why)));
        }
        let runs = self.runs();
        // The repository's own list extends the surface, read at the case's base: a list
        // that does not parse takes no judgement step at all.
        let globs = match self.judge_list()? {
            Ok(globs) => globs,
            Err(why) => {
                let why = format!(
                    "{why}; no judgement step is taken while the repository's judge-surface list does not parse"
                );
                self.refused(events, BY_TAKT, &why)?;
                writeln!(
                    out,
                    "step {}: refused before the harness: {why}",
                    self.steps
                )?;
                return Ok(Judgement::Stop(Stop::StepRefused(why)));
            }
        };
        let (surface, surface_kept) = JudgeSurface::read(&runs, globs, &self.modules, &self.tree)?;
        // A link is never followed: while what a validator or the guarded command reads is
        // reached through one, what it guards could be anywhere, in the tree or out of it,
        // and no judgement step is taken ([`JudgeSurface::locking_links`]). Any other link
        // on the surface is an entry by itself: changing it is refused like any change.
        let links = surface.locking_links(&self.tree, &surface_kept)?;
        if !links.is_empty() {
            let why = format!(
                "the judge surface is reached through a link, which takt never follows: {}; no judgement step is taken while one is there",
                links.join(", ")
            );
            self.refused(events, BY_TAKT, &why)?;
            writeln!(
                out,
                "step {}: refused before the harness: {why}",
                self.steps
            )?;
            return Ok(Judgement::Stop(Stop::StepRefused(why)));
        }
        let surface_dirty_before: BTreeSet<String> = crate::git::changed_paths(&self.tree)?
            .into_iter()
            .filter(|path| surface.contains(path))
            .collect();
        // The cargo manifests as `HEAD` has them before the step: what decides how cargo
        // builds and runs the tests is compared with them after it.
        let head_before = crate::git::head(&self.tree)?;
        let cargo_before = cargo_builds(&self.tree);
        // What cargo and rustup read outside the tree ([`Outside`]), put back with the rest
        // of a step that is discarded.
        let mut outside_before = Outside::read(&self.tree, &self.outside_dirs)?;
        let outside_dirs = self.outside_dirs.clone();
        // Every branch and tag is guarded beside `HEAD` and the case's branch: the case's
        // base, and a sibling case's branch, which a step could otherwise move under it; a
        // branch another worktree has checked out is that session's, but never the case's
        // base or its own branch.
        let checkpoint = self.checkpoint(true)?;
        // What is left of the hour budget is read last, just before the call, so the step
        // ends by the run's end and not by the time its preparation took after it.
        context.time_left = self.time_left();
        // A harness that panics is put back like one that failed, then the panic goes on
        // to `run`, which records it and lets go of the case.
        let stepped =
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| self.harness.step(&context)));
        let returned = Instant::now();
        // The step's transcript is kept now, before any way out of the step below, the
        // panic's and the errors' too: under the transcripts directory, outside the tree
        // and every case directory ([`Run::transcripts_in`]), so no snapshot of the step
        // reads it and no put-back takes it along. The step's line names it, or why none
        // was kept: a file at its path is never overwritten, and the run goes on. A way out
        // below that writes no step line (the harness failed, the hour budget ended it,
        // what it left could not be killed or read) names it in the error or refusal line
        // instead (`note`) and on the run's output (`told`); a record names it relative to
        // the transcripts directory, the output whole.
        let kept_transcript = crate::harness::keep_step_transcript(
            self.harness,
            self.transcripts
                .as_ref()
                .map(|(root, key)| (root.as_path(), key.as_str())),
            &self.case.id,
            run_number,
            self.steps,
        );
        // What takt wrote for the transcript, each exact path, is takt's: taken as found
        // before the step and when the run began ([`Outside::took`]), so neither this step
        // nor a later observation is charged with it (the transcripts directory under a
        // `PYTHONUSERBASE` of `~/.local`); what the step wrote beside it still is.
        if let Some(kept) = &kept_transcript {
            outside_before.took(kept.made(), &self.tree, &outside_dirs);
            self.outside.took(kept.made(), &self.tree, &outside_dirs);
        }
        let transcript = kept_transcript
            .as_ref()
            .map(crate::harness::KeptTranscript::recorded);
        let note = kept_transcript
            .as_ref()
            .map(crate::harness::KeptTranscript::note)
            .unwrap_or_default();
        let told = kept_transcript
            .as_ref()
            .map(crate::harness::KeptTranscript::told);
        let tell = |out: &mut dyn Write| -> std::io::Result<()> {
            match &told {
                Some(told) => writeln!(out, "{told}"),
                None => Ok(()),
            }
        };
        // What the step left running (a writer in a process group of its own) is killed
        // before anything it did is looked at, so nothing lands after the step is judged.
        let swept = crate::forge::sweep_left();
        // What git runs or reads as configuration first: every git call after this one
        // would read what the step planted there. Nothing in it ends early; a path it could
        // not put back is named in the refusal below.
        let git_dir = checkpoint.put_back_git_dir(&self.tree);
        let tree = self.tree.clone();
        let kept_all = Kept {
            checkpoint: &checkpoint,
            tree: &self.tree,
            cases_kept: &kept,
            surface: &surface,
            surface_kept: &surface_kept,
            outside: &outside_before,
            outside_dirs: &outside_dirs,
        };
        // Every way a harness step is discarded goes through here, and what it says of the
        // files it set aside ([`crate::git::SetAside::named`]) is what every message about the
        // discard carries.
        let undo =
            || -> Result<String> { undo_step(&tree, &kept_all).map(|undone| undone.aside.named()) };
        let stepped = match stepped {
            Ok(stepped) => stepped,
            Err(panic) => {
                match undo() {
                    Ok(moved) if !moved.is_empty() => eprintln!(
                        "step {}: the harness panicked; its change was discarded{moved}",
                        self.steps
                    ),
                    Ok(_) => {}
                    Err(undo) => eprintln!(
                        "step {}: the harness panicked, and the step's change could not be discarded: {undo:#}",
                        self.steps
                    ),
                }
                let _ = tell(out);
                std::panic::resume_unwind(panic);
            }
        };
        // A step whose leftovers could not all be killed is not judged: it is discarded
        // whole and the run ends on the error.
        if let Err(error) = swept {
            let _ = tell(out);
            return Err(match undo() {
                Ok(moved) => error.context(format!(
                    "step {}: what the step left running could not be killed; everything it changed was discarded{moved}{note}",
                    self.steps
                )),
                Err(undo) => error.context(format!(
                    "step {}: what the step left running could not be killed, and its change could not be discarded: {undo:#}{note}",
                    self.steps
                )),
            });
        }
        // The edit effect: a harness that works in a scratch copy and never in the tree
        // ([`Harness::take_proposal`]) hands its change over as a proposal, and takt writes
        // it here, after the checkpoint and before the step is judged. A proposal the guard
        // refuses on its face (an action not offered, a file changed for an action that
        // changes none or records a choice, a path that is no plain path of the tree, under
        // `.git`, in the cases directory, on the judge surface, a non-test path for
        // `tests.write`, a path reached through a link, a path git ignores) is refused before
        // a byte of it reaches the tree; one that passes is written, and judged below as any
        // step's change.
        //
        // Its content is read before that, whatever the guard says of it: each file and the
        // summary checked for the shape of a credential and scanned by the run's scan adapter
        // ([`proposal_scan_refusal`]). One that does not pass is refused, and its summary is
        // not recorded ([`PROPOSAL_NOT_RECORDED`]); a refusal on its face is said first, but
        // after a path of the shape of a credential ([`proposed_path_refusal`]), since a
        // refusal on its face names the path.
        let stepped = match stepped {
            Ok(report) => match self.harness.take_proposal() {
                None => Ok(report),
                Some(proposal) => {
                    let unscanned = proposal_scan_refusal(
                        &self.tree,
                        &self.scan,
                        self.change_scan.as_ref(),
                        self.harness.reads_outside_its_copy() && !self.scan.declared_none(),
                        &proposal,
                    );
                    let summary = match unscanned {
                        Some(_) => PROPOSAL_NOT_RECORDED.to_owned(),
                        None => report.summary.clone(),
                    };
                    match proposed_path_refusal(&proposal)
                        .or_else(|| {
                            Self::proposal_refusal(
                                &self.tree, &proposal, offered, approvals, &surface, shape,
                            )
                        })
                        .or(unscanned)
                    {
                        Some(why) => {
                            let moved = undo()?;
                            let why = format!("{}{moved}", crate::trace::escape(&why));
                            if let Some(tokens) = report.tokens {
                                self.tokens += tokens;
                            }
                            self.stepped(
                                events,
                                SteppedBy::Harness(
                                    report.tokens,
                                    self.harness.last_usage(),
                                    self.harness.last_call().map(Box::new),
                                    transcript.clone(),
                                ),
                                self.harness.name(),
                                &report.action,
                                &summary,
                            )?;
                            self.refused(events, BY_TAKT, &why)?;
                            writeln!(
                                out,
                                "step {}: {} ({})",
                                self.steps,
                                report.action,
                                self.harness.name()
                            )?;
                            writeln!(
                                out,
                                "  refused before the edit effect: {why}; nothing of the proposal was written to the tree"
                            )?;
                            tell(out)?;
                            return Ok(Judgement::Stop(Stop::StepRefused(why)));
                        }
                        None => Self::apply_proposal(&self.tree, &proposal)
                            .map(|()| report)
                            .context("the edit effect could not write the proposal"),
                    }
                }
            },
            Err(error) => Err(error),
        };
        // What the step left, read now; a read that fails (a file the run may not read)
        // discards the step before the run ends on the error.
        let seen = (|| -> Result<(Vec<PathBuf>, Vec<String>)> {
            let touched = differing(&kept, &snapshot_cases(&self.tree)?);
            let mut surface_touched: BTreeSet<String> =
                differing(&surface_kept, &surface.snapshot(&tree)?)
                    .iter()
                    .map(|path| path.display().to_string())
                    .collect();
            surface_touched.extend(
                crate::git::changed_paths(&tree)?
                    .into_iter()
                    .filter(|path| surface.contains(path) && !surface_dirty_before.contains(path)),
            );
            Ok((touched, surface_touched.into_iter().collect()))
        })();
        let (touched, surface_touched) = match seen {
            Ok(seen) => seen,
            Err(error) => {
                let _ = tell(out);
                return Err(match undo() {
                    Ok(moved) => error.context(format!(
                        "step {}: what the step left could not be read; everything it changed was discarded{moved}{note}",
                        self.steps
                    )),
                    Err(undo) => error.context(format!(
                        "step {}: what the step left could not be read, and its change could not be discarded: {undo:#}{note}",
                        self.steps
                    )),
                });
            }
        };
        // Named as the step left them; the cases directory is put back with the rest of the
        // step's change by `undo`, after the reset, on every path below that discards it.
        let touched_named = cases_named(&self.tree, &touched);
        // A step that returned when the hour budget ended, failed (killed by it) or not, is
        // discarded whole like any failed step, recorded as a refusal, and the run stops on
        // the budget. One that reported keeps its step line, with its tokens and their
        // components and a summary saying it was discarded, and one that failed is charged
        // what its session reported ([`Run::charge_unproposed`]), so the steps' tokens add up
        // to the run's; what of the git directory could not be put back is named. One that
        // failed by itself before then failed as any other.
        if let Some(h) = self.options.hours
            && self.deadline().is_some_and(|deadline| returned >= deadline)
        {
            let ended = match &stepped {
                Ok(report) => {
                    self.tokens += report.tokens.unwrap_or(0);
                    format!("it reported {} at or after the run's end", report.action)
                }
                Err(error) => format!("{error:#}"),
            };
            let not_put_back = if git_dir.failed.is_empty() {
                String::new()
            } else {
                format!("; not put back: {}", git_dir.failed.join("; "))
            };
            let moved = match undo() {
                Ok(moved) => moved,
                Err(undo) => {
                    return Err(anyhow::anyhow!(
                        "step {}: the hour budget ran out during the step ({ended}), and the step's change could not be discarded: {undo:#}{not_put_back}{note}",
                        self.steps
                    ));
                }
            };
            let why = format!(
                "step {}: the hour budget ran out during the step; everything the step changed was discarded: {ended}{not_put_back}{moved}{note}",
                self.steps
            );
            // Written after the cases directory was put back, which would take a line
            // written before it along.
            if let Ok(report) = &stepped {
                self.stepped(
                    events,
                    SteppedBy::Harness(
                        report.tokens,
                        self.harness.last_usage(),
                        self.harness.last_call().map(Box::new),
                        transcript.clone(),
                    ),
                    self.harness.name(),
                    &report.action,
                    &format!("discarded at the run's end: {}", report.summary),
                )?;
            } else {
                self.charge_unproposed(events, transcript.clone())?;
            }
            self.refused(events, BY_TAKT, &why)?;
            writeln!(out, "{why}")?;
            tell(out)?;
            return Ok(Judgement::Stop(Stop::Hours(h)));
        }
        let report = match stepped {
            Ok(report) => report,
            Err(error) => {
                let _ = tell(out);
                // An instruction file the base tracks that cannot be given: the step was not
                // started, so nothing is charged; the run stops with the files named, in its
                // output and its record, and the case stays open.
                if let Some(refusal) =
                    error.downcast_ref::<crate::harness::InstructionFilesRefused>()
                {
                    let moved = match undo() {
                        Ok(moved) => moved,
                        Err(undo) => {
                            return Err(error.context(format!(
                                "step {}: the step could not be given its instruction files, and its change could not be discarded: {undo:#}{note}",
                                self.steps
                            )));
                        }
                    };
                    let why = format!("step {}: {refusal}{moved}{note}", self.steps);
                    self.refused(events, BY_TAKT, &why)?;
                    writeln!(out, "{why}")?;
                    return Ok(Judgement::Stop(Stop::StepRefused(why)));
                }
                // A session that ended without the line naming its action, its change put back
                // whole, is handed to the executor, which records the try and tries again
                // ([`NO_ACTION_TRIES`]); one whose change could not be put back is not.
                let no_action = error
                    .downcast_ref::<crate::harness::NoActionLine>()
                    .is_some();
                return Err(match undo() {
                    Ok(moved) if no_action => {
                        return Ok(Judgement::NoAction {
                            error: error.context(format!(
                                "step {}: the harness failed; everything the step changed was discarded{moved}{note}",
                                self.steps
                            )),
                            transcript: transcript.clone(),
                        });
                    }
                    Ok(moved) => {
                        let error = error.context(format!(
                            "step {}: the harness failed; everything the step changed was discarded{moved}{note}",
                            self.steps
                        ));
                        match self.charge_unproposed(events, transcript.clone()) {
                            Ok(()) => error,
                            Err(unrecorded) => error.context(format!(
                                "and the tokens its session reported could not be recorded: {unrecorded:#}"
                            )),
                        }
                    }
                    Err(undo) => error.context(format!(
                        "step {}: the harness failed, and the step's change could not be discarded: {undo:#}{note}",
                        self.steps
                    )),
                });
            }
        };
        if let Some(tokens) = report.tokens {
            self.tokens += tokens;
        }
        // The step's line is written once its change is kept or put back: putting the
        // cases directory back to `kept` would take a line written before it along.
        let harness = self.harness;
        let record_step = |run: &Self| {
            run.stepped(
                events,
                SteppedBy::Harness(
                    report.tokens,
                    harness.last_usage(),
                    harness.last_call().map(Box::new),
                    transcript.clone(),
                ),
                harness.name(),
                &report.action,
                &report.summary,
            )
        };
        // A step that cannot be told on `out` is not judged: it is discarded whole, as
        // one whose harness failed, and the run ends on the error.
        let told = writeln!(
            out,
            "step {}: {} ({})",
            self.steps,
            report.action,
            self.harness.name()
        )
        .and_then(|()| writeln!(out, "  said: {}", report.summary));
        if let Err(error) = told {
            let moved = undo()?;
            record_step(self)?;
            return Err(anyhow::Error::new(error).context(format!(
                "step {}: {} could not be printed; the change was discarded{moved}",
                self.steps, report.action
            )));
        }
        let change = match checkpoint.change(&self.tree) {
            Ok(change) => change,
            Err(error) => {
                let moved = undo()?;
                record_step(self)?;
                return Err(error.context(format!(
                    "step {}: what {} changed could not be read; the change was discarded{moved}",
                    self.steps, report.action
                )));
            }
        };
        let changed = &change.paths;
        if !touched.is_empty() {
            let moved = undo()?;
            let why = format!(
                "{} wrote into the cases directory: {}",
                report.action,
                touched_named.join(", ")
            );
            record_step(self)?;
            let why = format!("{why}{moved}");
            self.refused(events, BY_TAKT, &why)?;
            writeln!(
                out,
                "  refused: {why}; the cases directory was put back and the change discarded"
            )?;
            return Ok(Judgement::Stop(Stop::StepRefused(why)));
        }
        if !surface_touched.is_empty() {
            let moved = undo()?;
            let why = surface_refusal(&report.action, &surface_touched)
                + &surface.modules_note(&surface_touched);
            record_step(self)?;
            let why = format!("{why}{moved}");
            self.refused(events, BY_TAKT, &why)?;
            writeln!(
                out,
                "  refused: {why}; the judge surface was put back and the change discarded"
            )?;
            return Ok(Judgement::Stop(Stop::StepRefused(why)));
        }
        // A cargo manifest anywhere in the tree decides how cargo builds and runs the
        // tests through some of its keys ([`manifest_build_changes`]): a build script
        // under another name, a test harness, a profile, a patch, a build dependency, a
        // target's own entries but its dependencies. A step that adds, changes or removes
        // one is refused, and so is one that changed a manifest the scanner cannot read
        // with certainty, or what cargo itself reads as a build script or `links`
        // ([`cargo_builds`]). `.cargo/` is guarded above, so cargo reads the base's
        // configuration here.
        // pytest's configuration in a `pyproject.toml` likewise ([`pytest_config_changes`]).
        let manifests = (|| -> Result<Vec<String>> {
            let mut changed_keys = Vec::new();
            for path in changed
                .iter()
                .filter(|path| is_manifest(path) || is_pyproject(path))
            {
                let before = manifest_at(&self.tree, &head_before, path)?;
                let full = self.tree.join(path);
                let after = match std::fs::symlink_metadata(&full) {
                    Err(error) if error.kind() == std::io::ErrorKind::NotFound => String::new(),
                    Err(error) => {
                        return Err(error).with_context(|| format!("read {}", full.display()));
                    }
                    Ok(meta) if meta.is_file() => String::from_utf8_lossy(
                        &std::fs::read(&full)
                            .with_context(|| format!("read {}", full.display()))?,
                    )
                    .into_owned(),
                    Ok(_) => {
                        changed_keys.push(format!("{path}: not a regular file"));
                        continue;
                    }
                };
                let compared = if is_manifest(path) {
                    manifest_build_changes(&before, &after)
                } else {
                    pytest_config_changes(&before, &after)
                };
                match compared {
                    Ok(keys) => {
                        changed_keys.extend(keys.into_iter().map(|key| format!("{path}: {key}")));
                    }
                    Err(line) => changed_keys.push(format!(
                        "{path}: a line the manifest scanner cannot read with certainty, {line}"
                    )),
                }
            }
            // Cargo itself as the parser, the scanner's cross-check: the build scripts and
            // `links` it reads, before the step and after it.
            changed_keys.extend(cargo_build_changes(
                &cargo_before,
                &cargo_builds(&self.tree),
            ));
            Ok(changed_keys)
        })();
        let manifests = match manifests {
            Ok(manifests) => manifests,
            Err(error) => {
                let moved = undo()?;
                record_step(self)?;
                return Err(error.context(format!(
                    "step {}: the cargo manifests {} changed could not be read; the change was discarded{moved}",
                    self.steps, report.action
                )));
            }
        };
        if !manifests.is_empty() {
            let moved = undo()?;
            // Each entry starts with the path it is about: pytest's tables in a
            // `pyproject.toml`, else cargo's manifests and what cargo reads.
            let (pytest, cargo): (Vec<&String>, Vec<&String>) =
                manifests.iter().partition(|entry| {
                    entry
                        .split_once(": ")
                        .is_some_and(|(path, _)| is_pyproject(path))
                });
            let join = |entries: &[&String]| {
                entries
                    .iter()
                    .map(|entry| entry.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            };
            let mut parts = Vec::new();
            if !cargo.is_empty() {
                parts.push(format!(
                    "changed how cargo builds or runs the tests: {}",
                    join(&cargo)
                ));
            }
            if !pytest.is_empty() {
                parts.push(format!(
                    "changed how pytest collects or reports the tests: {}",
                    join(&pytest)
                ));
            }
            let why = format!("{} {}", report.action, parts.join("; and "));
            record_step(self)?;
            let why = format!("{why}{moved}");
            self.refused(events, BY_TAKT, &why)?;
            writeln!(out, "  refused: {why}; the change was discarded")?;
            return Ok(Judgement::Stop(Stop::StepRefused(why)));
        }
        // What cargo and rustup read outside the tree ([`Outside`]): a step that changed it
        // is refused, and what it created there removed, what it changed or removed put back.
        let outside_changed = match Outside::read(&self.tree, &self.outside_dirs) {
            Ok(now) => outside_before.differing(&now),
            Err(error) => {
                let moved = undo()?;
                record_step(self)?;
                return Err(error.context(format!(
                    "step {}: what cargo and rustup read outside the tree could not be read after {}; the change was discarded{moved}",
                    self.steps, report.action
                )));
            }
        };
        if !outside_changed.is_empty() {
            let moved = undo()?;
            // What is still not as it was is behind a link, never written through.
            let left = outside_before.differing(&Outside::read(&self.tree, &self.outside_dirs)?);
            let mut why = format!(
                "{} changed what cargo or rustup reads outside the tree, a program on `PATH`, or what Python imports from there: {}",
                report.action,
                outside_named(&outside_changed)
            );
            if !left.is_empty() {
                why.push_str(&format!(
                    "; not put back (a program or import directory takt never writes, a link on its way, or a file too large to keep a copy of): {}",
                    outside_named(&left)
                ));
            }
            record_step(self)?;
            let why = format!("{why}{moved}");
            self.refused(events, BY_TAKT, &why)?;
            writeln!(
                out,
                "  refused: {why}; the named configuration files were put back and the change discarded"
            )?;
            return Ok(Judgement::Stop(Stop::StepRefused(why)));
        }
        // What only takt does, or what nobody may: `.git/`, `HEAD`, the index, ignored
        // paths, and a path that was uncommitted before the step, whose change cannot be
        // told apart from what was there.
        let action = &report.action;
        let overreach = if !git_dir.touched.is_empty() {
            let mut why = format!(
                "{action} wrote under .git, where git runs or reads it: {}",
                git_dir.touched.join(", ")
            );
            if !git_dir.failed.is_empty() {
                why.push_str(&format!("; not put back: {}", git_dir.failed.join("; ")));
            }
            Some(why)
        } else if let Some(moved) = &change.head {
            Some(format!("{action} moved HEAD {moved}"))
        } else if !change.refs.is_empty() {
            Some(format!("{action} moved {}", change.refs.join(", ")))
        } else if change.staged {
            Some(format!(
                "{action} changed the index; takt stages and commits a step's change, the step does not"
            ))
        } else if !change.ignored_added.is_empty()
            || !change.ignored_changed.is_empty()
            || !change.ignored_removed.is_empty()
        {
            let mut what = Vec::new();
            if !change.ignored_added.is_empty() {
                what.push(format!("wrote {}", ignored_named(&change.ignored_added)));
            }
            if !change.ignored_changed.is_empty() {
                what.push(format!(
                    "rewrote {}",
                    ignored_named(&change.ignored_changed)
                ));
            }
            if !change.ignored_removed.is_empty() {
                what.push(format!(
                    "removed {}",
                    ignored_named(&change.ignored_removed)
                ));
            }
            let mut why = format!(
                "{action} {} ignored by git, which no commit would carry",
                what.join(" and ")
            );
            if !change.ignored_lost.is_empty() {
                why.push_str(&format!(
                    "; nothing was kept to put back of {}",
                    ignored_named(&change.ignored_lost)
                ));
            }
            Some(why)
        } else if !change.already_uncommitted.is_empty() {
            Some(format!(
                "{action} changed {}, uncommitted before the step already",
                change.already_uncommitted.join(", ")
            ))
        } else {
            None
        };
        // A path is the step's to name: each control character in it escaped, so no name
        // shapes a line of the output.
        let overreach = overreach.map(|why| crate::trace::escape(&why));
        if let Some(why) = overreach {
            let moved = undo()?;
            record_step(self)?;
            let why = format!("{why}{moved}");
            self.refused(events, BY_TAKT, &why)?;
            writeln!(out, "  refused: {why}; the change was discarded")?;
            return Ok(Judgement::Stop(Stop::StepRefused(why)));
        }
        if approvals.contains(&report.action) {
            if !changed.is_empty() {
                let moved = undo()?;
                let why = format!("{} requested with changes", report.action);
                record_step(self)?;
                let why = format!("{why}{moved}");
                self.refused(events, BY_TAKT, &why)?;
                writeln!(
                    out,
                    "  refused: {} was requested with {} changed path(s); the change was discarded{moved}",
                    report.action,
                    changed.len()
                )?;
                return Ok(Judgement::Stop(Stop::StepRefused(why)));
            }
            record_step(self)?;
            writeln!(out, "  effect: none; {} needs an approval", report.action)?;
            // The request for the approval is the proposal: the runtime asks the case's
            // authority, and whatever it answers nothing is performed for it.
            return Ok(Judgement::Propose(Box::new(Proposed {
                report,
                transcript,
                changed: Vec::new(),
                ask: true,
                held: StepKept {
                    checkpoint,
                    tree: self.tree.clone(),
                    cases_kept: kept,
                    surface,
                    surface_kept,
                    outside: outside_before,
                    outside_dirs,
                },
            })));
        }
        let refusal = if !offered.contains(&report.action) {
            Some(format!("{} was not offered", report.action))
        } else if report.action == "repository.inspect" && !changed.is_empty() {
            Some(format!(
                "repository.inspect changed {} path(s)",
                changed.len()
            ))
        } else if report.action == "tests.write"
            && let Some(path) = changed.iter().find(|p| !is_test_path(p))
        {
            Some(format!("tests.write changed a non-test path: {path}"))
        } else {
            None
        };
        if let Some(why) = refusal {
            let moved = undo()?;
            record_step(self)?;
            let why = format!("{why}{moved}");
            self.refused(events, BY_TAKT, &why)?;
            writeln!(out, "  refused: {why}; the change was discarded")?;
            return Ok(Judgement::Stop(Stop::StepRefused(why)));
        }
        // An action whose evidence records a choice ([`Shape`]) is taken with no file changed: a
        // step that changed one is refused, as a request for an approval with changes is.
        if !shape.choices_of(&report.action).is_empty() && !changed.is_empty() {
            let moved = undo()?;
            let why = format!(
                "{} records a choice and changed {} path(s)",
                report.action,
                changed.len()
            );
            record_step(self)?;
            let why = format!("{why}{moved}");
            self.refused(events, BY_TAKT, &why)?;
            writeln!(out, "  refused: {why}; the change was discarded")?;
            return Ok(Judgement::Stop(Stop::StepRefused(why)));
        }
        let changed = changed.clone();
        Ok(Judgement::Propose(Box::new(Proposed {
            report,
            transcript,
            changed,
            ask: false,
            held: StepKept {
                checkpoint,
                tree: self.tree.clone(),
                cases_kept: kept,
                surface,
                surface_kept,
                outside: outside_before,
                outside_dirs,
            },
        })))
    }

    /// The step line of the judgement step `proposed`: its tokens, their components and what
    /// the harness reported about the call, and its transcript.
    fn record_proposed(&self, events: &Events, proposed: &Proposed) -> Result<()> {
        self.stepped(
            events,
            SteppedBy::Harness(
                proposed.report.tokens,
                self.harness.last_usage(),
                self.harness.last_call().map(Box::new),
                proposed.transcript.clone(),
            ),
            self.harness.name(),
            &proposed.report.action,
            &proposed.report.summary,
        )
    }

    /// The effect of a judgement step, once Commission's runtime of loom-sdk handed it the
    /// `request` for the proposed action: an action whose evidence records a choice is handed
    /// to the choice effect ([`ChoiceEffect`]), which writes the records the case lacks through
    /// the case's evidence port under [`CHOICE_PRODUCER`]; one that changed no file is kept as
    /// such; else the change is committed, and the implementation moves to the commit. Answers
    /// whether the step moved the case, and the choice effect's answer when it had one.
    ///
    /// # Errors
    /// A failure to commit, record or write; a commit that fails is discarded whole first.
    #[allow(clippy::too_many_arguments)]
    fn keep(
        &mut self,
        events: &Events,
        out: &mut dyn Write,
        seq: usize,
        shape: &Shape,
        proposed: &Proposed,
        commission: &Commission<commission_state::Assigned>,
        request: &AdmittedRequest,
    ) -> Result<(bool, Option<EffectOutcome>)> {
        let report = &proposed.report;
        let changed = &proposed.changed;
        // An action whose evidence records a choice ([`Shape`]): takt writes that record,
        // with no result, bound to the revision of each artifact the protocol reads it about.
        // Taken again where its record is already written, it moves nothing, and the run sees
        // no progress.
        let choices = shape.choices_of(&report.action);
        if !choices.is_empty() {
            self.record_proposed(events, proposed)?;
            events.emit(
                self.steps,
                &Event::Effect {
                    paths: Vec::new(),
                    revision: None,
                },
            )?;
            writeln!(out, "  effect: no file changed")?;
            // The records missing, each about an artifact at its revision: written by the
            // choice effect ([`crate::validator::ChoiceEffect`]), which Commission's runtime
            // handed the request for the action the harness chose, through the case's
            // evidence port under [`CHOICE_PRODUCER`].
            let mut missing: Vec<(String, String, String, String)> = Vec::new();
            for (kind, subjects) in choices {
                for subject in subjects {
                    if self.case.has_evidence(kind, subject)? {
                        continue;
                    }
                    let revision = self.case.revision(subject).with_context(|| {
                        format!("{kind} is read about {subject}, which has no revision")
                    })?;
                    let id = if subjects.len() == 1 {
                        format!("{}-{seq}", kind.replace('_', "-"))
                    } else {
                        format!("{}-{subject}-{seq}", kind.replace('_', "-"))
                    };
                    missing.push((id, kind.clone(), subject.clone(), revision));
                }
            }
            // A step whose choice records the case already holds asks the choice effect for
            // none: the request is answered as any step that changed no file is.
            if missing.is_empty() {
                return Ok((false, None));
            }
            let mut written: Vec<(crate::evidence::Record, PathBuf)> = Vec::new();
            let (answered, failure) = {
                let case = &mut self.case;
                let written = &mut written;
                let mut write = |request: &AdmittedRequest| -> Result<EffectOutcome> {
                    let evidence = CaseEvidence::new(&mut *case, vec![CHOICE_PRODUCER.to_owned()])?;
                    let mut ids = Vec::new();
                    for (id, kind, subject, revision) in &missing {
                        let record = crate::evidence::Record::new(
                            id,
                            kind,
                            None,
                            subject,
                            revision,
                            CHOICE_PRODUCER,
                        );
                        let path = evidence.submit(
                            CHOICE_PRODUCER,
                            &record,
                            Some(&request.data().action_request_id.0.0),
                        )?;
                        ids.push(cjson::Value::Text(record.id.clone()));
                        written.push((record, path));
                    }
                    Ok(EffectOutcome::Performed(EffectOutcomePerformed {
                        report: cjson::Value::Object(vec![(
                            "records".to_owned(),
                            cjson::Value::Array(ids),
                        )]),
                        attempt: None,
                        audit: None,
                        usage: None,
                    }))
                };
                let effect = ChoiceEffect::new(&report.action, &mut write);
                let answered = effect.invoke(commission, request);
                (answered, effect.take_failure())
            };
            if let Some(error) = failure {
                return Err(error);
            }
            let answered =
                answered.map_err(|error| anyhow!("the choice effect failed: {error}"))?;
            for (record, path) in &written {
                events.emit(
                    self.steps,
                    &Event::Evidence {
                        id: record.id.clone(),
                        kind: record.kind.clone(),
                        result: None,
                        path: path
                            .strip_prefix(&self.tree)
                            .unwrap_or(path)
                            .display()
                            .to_string(),
                        failed: Vec::new(),
                    },
                )?;
                writeln!(out, "  evidence: {} -", record.kind)?;
            }
            return Ok((!written.is_empty(), Some(answered)));
        }
        if changed.is_empty() {
            self.record_proposed(events, proposed)?;
            events.emit(
                self.steps,
                &Event::Effect {
                    paths: Vec::new(),
                    revision: None,
                },
            )?;
            writeln!(out, "  effect: no file changed")?;
            return Ok((false, None));
        }
        // The message is the harness's `commit` when it passes its check, else the fallback,
        // said in the record before the step's effect; the trailer is takt's, from the case's
        // story, never from the reply.
        let plan = self.commit_plan_now();
        let reply = self.harness.take_commit();
        let story = self.case.story()?;
        let title = match &self.unit {
            Unit::Story(story) => story.title.clone(),
            Unit::Intent(intent) => intent.lines().next().unwrap_or_default().to_owned(),
        };
        let message = crate::commit_style::message(
            &plan,
            &report.action,
            &report.summary,
            &title,
            reply.as_ref(),
            story.as_deref(),
        );
        // A commit that fails (a hook of the repository's own refused it) leaves nothing
        // of the step behind either.
        let head =
            match crate::git::commit_paths(&self.tree, &self.committer, changed, &message.text) {
                Ok(head) => head,
                Err(error) => {
                    let moved = undo_step(&self.tree, &proposed.held.kept())
                        .map(|undone| undone.aside.named())?;
                    self.record_proposed(events, proposed)?;
                    return Err(error.context(format!(
                    "step {}: the commit of {}'s change failed; the change was discarded{moved}",
                    self.steps, report.action
                )));
                }
            };
        // The step is on record before anything that can end the run, so the tokens it spent
        // are kept whatever follows: a case that declares no implementation artifact ends
        // the run here.
        self.record_proposed(events, proposed)?;
        self.case.set_revision("implementation", &head)?;
        if let Some(reason) = message.fallback {
            events.emit(self.steps, &Event::CommitFallback { reason })?;
        }
        events.emit(
            self.steps,
            &Event::Effect {
                paths: changed.clone(),
                revision: Some(head.clone()),
            },
        )?;
        writeln!(
            out,
            "  effect: committed {} path(s); implementation is now {}",
            changed.len(),
            &head[..8.min(head.len())]
        )?;
        Ok((true, None))
    }
}
