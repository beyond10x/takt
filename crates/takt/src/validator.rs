//! Validators: programs that turn an observation into evidence: `tests.run`, `gate.run`,
//! `pr.open`, `ci.read`, `repository.merge` and `change.read`.

use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::atomic::{AtomicU64, Ordering};

use anyhow::{Context as _, Result, anyhow, bail};
use loom_sdk::commission::model::json as cjson;
use loom_sdk::commission::model::primitives::{Timestamp, Uuid};
use loom_sdk::commission::model::responsibility::{
    ActionRequestId, AgentRevisionId, AuthorityContext, CaseId, Commission, CommissionData,
    CommissionId, EffectOutcome, EffectOutcomePending, EffectOutcomePerformed,
    EffectOutcomeRefused, ExecutorOutcome, ExecutorOutcomeProposedAction, Frontier, ObservationId,
    PrincipalId, ProposedActionArguments, RunId, commission_state, frontier_state,
};
use loom_sdk::commission::outcome::RunStore;
use loom_sdk::commission::ports::authority::AuthorityProvider;
use loom_sdk::commission::ports::effect::{AdmittedRequest, EffectError};
use loom_sdk::commission::ports::executor::AgentExecutor;
use loom_sdk::{CanonGovernor, EffectPort, LoopContext, LoopEnd, run_until_blocked};

use crate::case::{BRANCH_FILE, Branch, CaseFiles, current};
use crate::confinement::Command;
use crate::evidence::{Record, uuid_from};
use crate::forge::{BotRoute, Checks, Forge, Limits, PullRequest, PullRequestState};
use crate::git::Mirror;
use crate::protocol::LoadedProtocol;

/// What a validator observed, and the records it wrote from it.
#[derive(Debug)]
pub struct Observation {
    /// One line on what was observed.
    pub summary: String,
    /// The evidence records the observation yields.
    pub records: Vec<Record>,
    /// What the validator retains beside its records, when it keeps something: the
    /// observation as its provider wrote it, kept under the case as `artifacts/<name>`.
    pub artifact: Option<Artifact>,
}

/// One file a validator retains with an observation.
#[derive(Clone)]
pub struct Artifact {
    /// The file name under the case's `artifacts/`.
    pub name: String,
    /// The file's bytes.
    pub bytes: Vec<u8>,
}

impl std::fmt::Debug for Artifact {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Artifact")
            .field("name", &self.name)
            .field("bytes", &self.bytes.len())
            .finish()
    }
}

/// A program that performs one deterministic action and answers evidence.
pub trait Validator {
    /// The validator's name, the producer of its records.
    fn name(&self) -> &str;
    /// The protocol action it performs.
    fn action(&self) -> &str;
    /// The evidence kind it produces.
    fn kind(&self) -> &str;
    /// The artifact its records are about.
    fn subject(&self) -> &str;
    /// Performs the action in `tree` about `revision`; `seq` makes record ids unique.
    fn observe(&self, tree: &Path, revision: &str, seq: usize) -> Result<Observation>;
    /// Observes as [`Self::observe`] does, and says whether an observation that wrote no record
    /// waits: on a state outside the tree that moves without a commit (on the forge: checks
    /// still running, a pull request's head not yet at the head pushed, a merge held for a
    /// required check that has not reported; or locally, [`Self::waits_locally`]), so the same
    /// observation made again after a while may write one. On the forge the loop then observes
    /// again within its budget and offers no judgement step meanwhile. Not waiting by default:
    /// an observation without a record is final at its revision.
    fn observe_waiting(
        &self,
        tree: &Path,
        revision: &str,
        seq: usize,
    ) -> Result<(Observation, bool)> {
        Ok((self.observe(tree, revision, seq)?, false))
    }
    /// Whether what it waits on ([`Self::observe_waiting`]) is local: a file or a ref on this
    /// machine that only someone outside the run moves (a saved scorecard, a promoted binary,
    /// the local `main`). Observing it again within the run would spend the budget on the same
    /// observation, so the loop stops [`crate::run::Stop::Waiting`] at once after the step, naming
    /// what it waits for, and the next run observes again. Not local by default: a wait is on the
    /// forge, and the loop pauses and observes it again within its budget.
    fn waits_locally(&self) -> bool {
        false
    }
    /// Observes as [`Self::observe_waiting`] does, a child it runs held to `deadline` (the
    /// instant the run's hour budget ends, the run's own; `None` for a run without one) as well
    /// as its own timeout. By default `deadline` is not read: the validator runs no child of its
    /// own.
    fn observe_within(
        &self,
        tree: &Path,
        revision: &str,
        seq: usize,
        deadline: Option<std::time::Instant>,
    ) -> Result<(Observation, bool)> {
        let _ = deadline;
        self.observe_waiting(tree, revision, seq)
    }
    /// The files under the case's directory the validator itself writes while it observes,
    /// relative to it. Nothing else under the cases directory may change during an
    /// observation: the loop puts back whatever else changed and refuses the observation,
    /// because what a validator runs (a test command, a gate's commands) may be code a model
    /// wrote. None by default.
    fn writes(&self) -> Vec<PathBuf> {
        Vec::new()
    }
    /// The words of the command the validator runs from the tree, the test command's: what
    /// names the command's own script on the run's judge surface
    /// ([`crate::run::JudgeSurface`]), which no judgement step changes. None by default.
    fn runs(&self) -> Vec<String> {
        Vec::new()
    }
    /// What the checks of the last observation that failed printed that names what failed
    /// ([`FailedCheck`]), taken once: the run keeps it on the run record's evidence line of the
    /// observation's record and shows it to the next judgement step. None by default.
    fn take_failed(&self) -> Vec<FailedCheck> {
        Vec::new()
    }
    /// The repository's obligations the last observation judged ([`crate::obligations`]): each
    /// gate with its status at the revision observed, taken once; the run records them as an
    /// `obligations` line. None by default, and for an observation that judged no gate.
    fn take_obligations(&self) -> Option<Vec<crate::obligations::Obligation>> {
        None
    }
    /// The gates document this validator evaluates, relative to the tree: its gates are the
    /// repository's obligations, read from the case's base revision. None by default.
    fn gates_document(&self) -> Option<&Path> {
        None
    }
    /// Checks, when the case at `case_dir` in `tree` has just opened, what the validator reads
    /// from the base revision the case pinned and would refuse when it observes, running
    /// nothing it would run: a refusal is the error, and the case is not run. Nothing to check
    /// by default.
    fn check_at_open(&self, tree: &Path, case_dir: &Path) -> Result<()> {
        let _ = (tree, case_dir);
        Ok(())
    }
}

/// The most bytes [`failing_lines`] keeps of one failed check's output, 4 KiB, each kept line
/// counted with its line end.
pub const FAILED_LINES_PER_CHECK: usize = 4 << 10;

/// The most bytes one observation keeps of its failed checks' lines over all of them, 8 KiB: a
/// check keeps the smaller of [`FAILED_LINES_PER_CHECK`] and what the checks before it left.
pub const FAILED_LINES_TOTAL: usize = 8 << 10;

/// The most bytes of one kept line: a longer one is cut there and ends in `…`.
const FAILED_LINE_BYTES: usize = 512;

/// What a check that failed printed that names what failed: the check's id (a gate's), the
/// lines kept of its output ([`failing_lines`]), and how many of its non-blank lines were left
/// out.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct FailedCheck {
    pub id: String,
    pub lines: Vec<String>,
    pub omitted: u64,
}

/// `line` without its terminal escape sequences (CSI, such as a colour, `ESC [ … <final>`; OSC,
/// `ESC ] … BEL` or `ESC ] … ESC \`; any other `ESC <char>`) and without any other control
/// character but a tab, so a colour code before a token cannot hide its shape.
fn plain(line: &str) -> String {
    let mut out = String::with_capacity(line.len());
    let mut chars = line.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\u{1b}' {
            match chars.next() {
                Some('[') => {
                    for c in chars.by_ref() {
                        if ('\u{40}'..='\u{7e}').contains(&c) {
                            break;
                        }
                    }
                }
                Some(']') => {
                    while let Some(c) = chars.next() {
                        if c == '\u{7}' || (c == '\u{1b}' && chars.next_if_eq(&'\\').is_some()) {
                            break;
                        }
                    }
                }
                _ => {}
            }
        } else if c == '\t' || !c.is_control() {
            out.push(c);
        }
    }
    out
}

/// The lines of a failed command's output `printed` that name what failed, within `budget`
/// bytes (each line counted with its line end), and how many non-blank lines were left out.
/// Every non-blank line, its terminal escape sequences and every other control character but a
/// tab taken out ([`plain`]), its trailing white space cut, a line past 512 bytes cut there and
/// ended in `…`. Whatever the room, so none reaches a record or a prompt, a line of a
/// credential's shape ([`crate::run::credential_shape`], read on the line so cleaned) is left
/// out, and so is a private key's block, from a `-----BEGIN … PRIVATE KEY-----` line through
/// its `-----END` line, or to the end without one; each counted as left out. When they do not
/// all fit: the last line first (a checker's summary), then the
/// lines that do not start with white space, from the start in order until one does not fit,
/// and, once all of those fit, the indented detail lines the same way; kept lines stay in their
/// order. No rule of any one command's: `<document>:<line>: unresolved: <cited>` lines of a
/// citation checker and `error:`/`warning:` lines of a compiler are kept before their indented
/// detail alike.
#[must_use]
pub fn failing_lines(printed: &str, budget: usize) -> (Vec<String>, u64) {
    let mut withheld = 0u64;
    let mut in_key = false;
    let mut lines: Vec<String> = Vec::new();
    for line in printed.lines() {
        let line = plain(line);
        let line = line.trim_end();
        if line.is_empty() {
            continue;
        }
        // A private key's block, from its `BEGIN` line through its `END` line or the end.
        if line.contains("-----BEGIN ") && line.contains("PRIVATE KEY-----") {
            in_key = true;
        }
        if in_key || crate::run::credential_shape(line).is_some() {
            in_key = in_key && !line.contains("-----END ");
            withheld += 1;
            continue;
        }
        if line.len() <= FAILED_LINE_BYTES {
            lines.push(line.to_owned());
            continue;
        }
        let mut end = FAILED_LINE_BYTES - '…'.len_utf8();
        while !line.is_char_boundary(end) {
            end -= 1;
        }
        lines.push(format!("{}…", &line[..end]));
    }
    let cost = |line: &String| line.len() + 1;
    if lines.iter().map(cost).sum::<usize>() <= budget {
        return (lines, withheld);
    }
    let mut keep = vec![false; lines.len()];
    let mut left = budget;
    if let Some(last) = lines.last()
        && cost(last) <= left
    {
        keep[lines.len() - 1] = true;
        left -= cost(last);
    }
    let body = lines.len().saturating_sub(1);
    'passes: for detail in [false, true] {
        for (index, line) in lines[..body].iter().enumerate() {
            if line.starts_with(char::is_whitespace) != detail {
                continue;
            }
            if cost(line) > left {
                break 'passes;
            }
            keep[index] = true;
            left -= cost(line);
        }
    }
    let omitted = keep.iter().filter(|kept| !**kept).count() as u64 + withheld;
    let kept = lines
        .into_iter()
        .zip(keep)
        .filter_map(|(line, kept)| kept.then_some(line))
        .collect();
    (kept, omitted)
}

/// How an effect performs the request it was handed: the validator's observation, taken where
/// the run guards it, its records submitted through the case's evidence port
/// ([`crate::evidence::CaseEvidence`]), answered as the effect's outcome ([`outcome`]).
pub trait Step {
    /// Performs `request` with `validator`.
    ///
    /// # Errors
    /// A failure to observe or to record; the run ends on it.
    fn perform(
        &mut self,
        validator: &dyn Validator,
        request: &AdmittedRequest,
    ) -> Result<EffectOutcome>;
}

/// An effect the run hands Commission's runtime, which keeps the error that failed it: the
/// runtime reports a failed effect as text alone ([`EffectError`]), and the run ends on the error
/// itself.
pub trait Effect: EffectPort {
    /// The error the last invocation failed on, if it failed.
    fn take_failure(&self) -> Option<anyhow::Error>;
}

/// A validator as an [`EffectPort`]: it performs its validator's action and no other, and only
/// what Commission's runtime hands it, an [`AdmittedRequest`] the governor admitted and, for an
/// action that requires a capability, the authority provider allowed. It performs the request
/// through `step`; the gates a validator keeps itself (the merge's grant of `repository.merge`
/// at the case's revision, a judge surface's grant) are inside the effect, whatever the authority
/// provider answered.
pub struct ValidatorEffect<'v> {
    validator: &'v dyn Validator,
    step: RefCell<&'v mut dyn Step>,
    failed: RefCell<Option<anyhow::Error>>,
}

impl<'v> ValidatorEffect<'v> {
    /// The effect of `validator`, performing through `step`.
    pub fn new(validator: &'v dyn Validator, step: &'v mut dyn Step) -> Self {
        Self {
            validator,
            step: RefCell::new(step),
            failed: RefCell::new(None),
        }
    }
}

impl EffectPort for ValidatorEffect<'_> {
    fn performs(&self, action: &str) -> bool {
        action == self.validator.action()
    }

    fn invoke(
        &self,
        _: &Commission<commission_state::Assigned>,
        request: &AdmittedRequest,
    ) -> Result<EffectOutcome, EffectError> {
        if !self.performs(&request.data().action) {
            return Ok(EffectOutcome::Refused(EffectOutcomeRefused {
                reason: format!(
                    "{} performs {}, not {}",
                    self.validator.name(),
                    self.validator.action(),
                    request.data().action
                ),
                ends_run: true,
            }));
        }
        let performed = self.step.borrow_mut().perform(self.validator, request);
        performed.map_err(|error| {
            let message = format!("{error:#}");
            self.failed.replace(Some(error));
            EffectError::new(message)
        })
    }
}

impl Effect for ValidatorEffect<'_> {
    fn take_failure(&self) -> Option<anyhow::Error> {
        self.failed.take()
    }
}

/// What writes a choice record: a request for the choice action, performed.
pub type ChoiceWrite<'c> = dyn FnMut(&AdmittedRequest) -> Result<EffectOutcome> + 'c;

/// The effect that writes a judgement step's choice records (producer
/// [`crate::run::CHOICE_PRODUCER`]): it performs the one action the harness chose, which
/// records a choice, and only what Commission's runtime hands it, through `write`.
pub struct ChoiceEffect<'c> {
    action: &'c str,
    write: RefCell<&'c mut ChoiceWrite<'c>>,
    failed: RefCell<Option<anyhow::Error>>,
}

impl<'c> ChoiceEffect<'c> {
    /// The effect performing `action` through `write`.
    pub fn new(action: &'c str, write: &'c mut ChoiceWrite<'c>) -> Self {
        Self {
            action,
            write: RefCell::new(write),
            failed: RefCell::new(None),
        }
    }
}

impl EffectPort for ChoiceEffect<'_> {
    fn performs(&self, action: &str) -> bool {
        action == self.action
    }

    fn invoke(
        &self,
        _: &Commission<commission_state::Assigned>,
        request: &AdmittedRequest,
    ) -> Result<EffectOutcome, EffectError> {
        if !self.performs(&request.data().action) {
            return Ok(EffectOutcome::Refused(EffectOutcomeRefused {
                reason: format!(
                    "the choice effect performs {}, not {}",
                    self.action,
                    request.data().action
                ),
                ends_run: true,
            }));
        }
        let written = (self.write.borrow_mut())(request);
        written.map_err(|error| {
            let message = format!("{error:#}");
            self.failed.replace(Some(error));
            EffectError::new(message)
        })
    }
}

impl Effect for ChoiceEffect<'_> {
    fn take_failure(&self) -> Option<anyhow::Error> {
        self.failed.take()
    }
}

/// The outcome of an effect that observed `observation`: `Performed` when it wrote a record,
/// reporting its summary and the records' ids; `Pending` when it wrote none and waits
/// ([`Validator::observe_waiting`]); `Refused` otherwise, naming what it observed, with the run
/// going on.
#[must_use]
pub fn outcome(observation: &Observation, waits: bool) -> EffectOutcome {
    let report = cjson::Value::Object(vec![
        (
            "summary".to_owned(),
            cjson::Value::Text(observation.summary.clone()),
        ),
        (
            "records".to_owned(),
            cjson::Value::Array(
                observation
                    .records
                    .iter()
                    .map(|record| cjson::Value::Text(record.id.clone()))
                    .collect(),
            ),
        ),
    ]);
    if !observation.records.is_empty() {
        EffectOutcome::Performed(EffectOutcomePerformed {
            report,
            attempt: None,
            audit: None,
            usage: None,
        })
    } else if waits {
        EffectOutcome::Pending(EffectOutcomePending {
            report,
            ends_run: false,
        })
    } else {
        EffectOutcome::Refused(EffectOutcomeRefused {
            reason: observation.summary.clone(),
            ends_run: false,
        })
    }
}

/// The executor of a deterministic step: it proposes the one action takt found due, which no
/// model chose.
struct Proposes(String);

impl AgentExecutor for Proposes {
    fn run(
        &self,
        _: &Commission<commission_state::Assigned>,
        _: &Frontier<frontier_state::Issued>,
    ) -> ExecutorOutcome {
        ExecutorOutcome::ProposedAction(ExecutorOutcomeProposedAction {
            action: self.0.clone(),
            arguments: ProposedActionArguments(cjson::Value::Object(Vec::new())),
        })
    }
}

/// The runtime's ids, derived from `seed`, the time from takt's clock, and a budget of one step.
struct Clock {
    seed: String,
    n: u64,
}

impl Clock {
    fn next(&mut self, what: &str) -> Uuid {
        self.n += 1;
        Uuid(uuid_from(&format!("{}/{what}/{}", self.seed, self.n)))
    }
}

impl LoopContext for Clock {
    fn action_request_id(&mut self) -> ActionRequestId {
        ActionRequestId(self.next("action-request"))
    }

    fn observation_id(&mut self) -> ObservationId {
        ObservationId(self.next("observation"))
    }

    fn now(&mut self) -> Timestamp {
        Timestamp(crate::now())
    }

    fn step_budget(&self) -> Option<usize> {
        Some(1)
    }
}

/// Every [`run_effect`] of this process, so no two derive the same ids.
static RUNS: AtomicU64 = AtomicU64::new(0);

/// One step of Commission's runtime of loom-sdk (`run_until_blocked`) on the case `id` at `dir`:
/// its governor over the case's files under `protocol`, the executor proposing `action`, the
/// request revalidated, its capability (if the action requires one) asked of `authority`, and
/// the admitted request handed to `effect`. Ids derive from `seed`. Answers how the runtime
/// ended, with the effects it invoked; an effect that failed ends it on the effect's own error.
///
/// # Errors
/// The runtime's failure, or the effect's.
pub fn run_effect<A, F>(
    dir: &Path,
    id: &str,
    protocol: &LoadedProtocol,
    authority: &A,
    effect: &F,
    action: &str,
    seed: &str,
) -> Result<LoopEnd>
where
    A: AuthorityProvider + ?Sized,
    F: Effect + ?Sized,
{
    let n = RUNS.fetch_add(1, Ordering::Relaxed);
    let seed = format!("{id}/{seed}/{action}/{}/{n}", crate::now());
    let at = crate::now();
    let governor = CanonGovernor::new(CaseFiles::new(dir, id, &protocol.id))
        .with_protocol_yaml(&protocol.id, &protocol.yaml)
        .map_err(|error| anyhow!("register the protocol {}: {error}", protocol.id))?
        .with_evaluation_time(move |_: &CaseId| Ok(Some(at.clone())));
    let mut clock = Clock {
        seed: seed.clone(),
        n: 0,
    };
    let commission = Commission::new(CommissionData {
        commission_id: CommissionId(clock.next("commission")),
        agent_revision_id: AgentRevisionId(clock.next("agent-revision")),
        case_id: CaseId(id.to_owned()),
        principal: PrincipalId("takt".to_owned()),
        authority_context: AuthorityContext(cjson::Value::Null),
    });
    let mut runs = RunStore::new({
        let seed = seed.clone();
        let mut n = 0u64;
        move || {
            n += 1;
            RunId(Uuid(uuid_from(&format!("{seed}/run/{n}"))))
        }
    });
    let executor = Proposes(action.to_owned());
    run_until_blocked(
        &governor,
        &executor,
        authority,
        effect,
        &commission,
        &mut runs,
        &mut clock,
    )
    .map_err(|error| {
        effect
            .take_failure()
            .unwrap_or_else(|| anyhow!("Commission's runtime failed on {action}: {error}"))
    })
}

/// What the case's branch did to the judge surface between its base and a revision.
enum Judged {
    /// Nothing on it differs.
    Unchanged,
    /// These paths differ from the case's base, shown as the summary names it.
    Changed { base: String, paths: Vec<String> },
    /// The case pins no base commit to compare with.
    NoBase,
}

/// Whether the case's branch changed what decides `tests.run`, `ci.read` and `gate.run` between
/// the case's base at `dir` and `revision`: the run's judge surface
/// ([`crate::run::JudgeSurface`]) but the planning store, a link on it or on the way to a script
/// at either commit, and the entries of a cargo manifest that decide how cargo builds or runs the
/// tests ([`crate::run::manifest_build_changes`]) and of `pyproject.toml`'s pytest tables, the
/// surface extended by the repository's own list ([`crate::run::JUDGE_LIST`]) read at the base,
/// which records nothing when it does not parse, and by the caller's folded globs `extra`; with
/// `gate`, the tool configuration by name, a link there included ([`is_tool_config`]), and the
/// lint tables of every cargo manifest ([`lint_changes`]) besides. A change no step
/// could make, because the step guard refuses it, but one that reached the branch by another
/// route (a commit made outside the loop) would decide the verdict all the same. The store is
/// guarded at the step too, but it decides no test and no check, and takt itself writes it on
/// the case's branch (the take, an accepted case's test result and move): it is not compared.
fn judged(
    tree: &Path,
    dir: &Path,
    revision: &str,
    runs: &[String],
    extra: Vec<String>,
    gate: bool,
) -> Result<Judged> {
    let Some((base, at)) = base_revision(tree, dir)? else {
        return Ok(Judged::NoBase);
    };
    // The repository's own list, read at the base: one that does not parse records nothing.
    let mut paths: Vec<String> = Vec::new();
    let globs = match crate::run::judge_list_at(tree, &at)? {
        Ok(globs) => globs,
        Err(why) => {
            paths.push(why);
            Vec::new()
        }
    };
    // The case's module names, and the top-level modules the base held: a name the base held
    // in an import directory is the project's own, one the branch added there is not.
    let at_base = crate::git::run(
        tree,
        &["ls-tree", "-r", "-z", "--name-only", "--full-tree", &at],
    )?;
    let surface = crate::run::JudgeSurface::new(runs, tree)
        .without_store()
        .with_globs(globs)
        .with_globs(extra)
        .with_modules(crate::run::Modules::read(dir))
        .provided_by(at_base.split('\0').filter(|path| !path.is_empty()));
    // A link is never followed: one the base or the revision holds on the surface, or on the
    // way to a script, is named, and nothing is recorded while it is there.
    for commit in [at.as_str(), revision] {
        let listed = crate::git::bytes(tree, &["ls-tree", "-r", "-z", "--full-tree", commit])?;
        for entry in nul_fields(&listed, "ls-tree", true)? {
            let Some((meta, path)) = entry.split_once('\t') else {
                continue;
            };
            if meta.starts_with("120000 ")
                && (surface.reads_through(path) || (gate && is_tool_config(path)))
            {
                let named = format!("{path} (a link)");
                if !paths.contains(&named) {
                    paths.push(named);
                }
            }
        }
    }
    // Paths, read byte for byte: the first path's leading white space is part of it.
    let diff = crate::git::bytes(
        tree,
        &[
            "diff-tree",
            "-r",
            "-z",
            "--no-renames",
            "--name-only",
            &at,
            revision,
            "--",
        ],
    )?;
    for path in nul_fields(&diff, "diff-tree", false)? {
        // While a line of the test command's script fails closed every program is on the
        // surface: one the base held is as much one as the tree's now.
        // In a directory on `PATH` an entry the base held executable is a program a lookup
        // finds, as one executable now is.
        if surface.contains(path)
            || (gate && is_tool_config(path))
            || (surface.guards_programs() && program_at(tree, &at, path)?)
            || (surface.in_a_path_dir(path) && executable_at(tree, &at, path)?)
        {
            paths.push(path.to_owned());
        } else if crate::run::is_manifest(path) || crate::run::is_pyproject(path) {
            let before = crate::run::manifest_at(tree, &at, path)?;
            let after = crate::run::manifest_at(tree, revision, path)?;
            let compared = if crate::run::is_manifest(path) {
                crate::run::manifest_build_changes(&before, &after)
            } else {
                crate::run::pytest_config_changes(&before, &after)
            };
            match compared {
                Ok(keys) => paths.extend(keys.into_iter().map(|key| format!("{path}: {key}"))),
                Err(line) => paths.push(format!(
                    "{path}: a line the manifest scanner cannot read with certainty, {line}"
                )),
            }
            if gate && crate::run::is_manifest(path) {
                match lint_changes(&before, &after) {
                    Ok(keys) => {
                        for key in keys {
                            let named = format!("{path}: {key}");
                            if !paths.contains(&named) {
                                paths.push(named);
                            }
                        }
                    }
                    Err(side) => paths.push(format!(
                        "{path}: a line the lints scanner cannot read with certainty, {side}"
                    )),
                }
            }
        }
    }
    // Where the module names came from, when one of the paths is such a module.
    let note = surface.modules_note(&paths);
    if let Some(last) = paths.last_mut() {
        last.push_str(&note);
    }
    Ok(if paths.is_empty() {
        Judged::Unchanged
    } else {
        Judged::Changed { base, paths }
    })
}

/// The home directory this user's passwd entry names (`getpwuid_r`), when it is an absolute path:
/// where a command's toolchain lives when takt itself has no absolute `HOME`.
#[cfg(unix)]
pub(crate) fn passwd_home() -> Option<PathBuf> {
    use std::os::unix::ffi::OsStrExt as _;
    let mut buffer = vec![0_u8; 16 * 1024];
    loop {
        // SAFETY: `passwd` is plain data, all zeroes a valid value before the call fills it.
        let mut entry: libc::passwd = unsafe { std::mem::zeroed() };
        let mut found: *mut libc::passwd = std::ptr::null_mut();
        // SAFETY: the entry, the buffer and its length, and the result pointer all outlive the
        // call, which writes the entry's strings into the buffer.
        let code = unsafe {
            libc::getpwuid_r(
                libc::getuid(),
                &raw mut entry,
                buffer.as_mut_ptr().cast(),
                buffer.len(),
                &raw mut found,
            )
        };
        if code == libc::ERANGE && buffer.len() < 1 << 20 {
            buffer.resize(buffer.len() * 2, 0);
            continue;
        }
        if code != 0 || found.is_null() || entry.pw_dir.is_null() {
            return None;
        }
        // SAFETY: a NUL-terminated string in the buffer, which outlives this read.
        let dir = unsafe { std::ffi::CStr::from_ptr(entry.pw_dir) };
        let dir = PathBuf::from(std::ffi::OsStr::from_bytes(dir.to_bytes()));
        return dir.is_absolute().then_some(dir);
    }
}

/// No passwd entry to read where there is no unix.
#[cfg(not(unix))]
pub(crate) fn passwd_home() -> Option<PathBuf> {
    None
}

/// The directories a gate's command finds a program by name on, when its binding declares no
/// `PATH` of its own. The user's `PATH` is never read: credentials never enter a child's
/// environment (`AGENTS.md` § Rules), and a program found on it would differ from one machine to
/// the next.
pub(crate) const FIXED_PATH: &str = "/usr/local/bin:/usr/bin:/bin";

/// The file `program` in the first absolute directory of `searched`, a colon-separated `PATH`,
/// that holds one its owner, group or others may execute, as a shell's `PATH` lookup finds it;
/// a relative entry is not searched (story:gate-commands-find-programs-on-a-system-path).
fn found_on(program: &str, searched: &str) -> Option<PathBuf> {
    searched
        .split(':')
        .map(Path::new)
        .filter(|dir| dir.is_absolute())
        .map(|dir| dir.join(program))
        .find(|candidate| executable(candidate))
}

/// Whether `path` is a file with an execute bit set.
#[cfg(unix)]
fn executable(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt as _;
    path.metadata()
        .is_ok_and(|meta| meta.is_file() && meta.permissions().mode() & 0o111 != 0)
}

/// Whether `path` is a file.
#[cfg(not(unix))]
fn executable(path: &Path) -> bool {
    path.is_file()
}

/// Whether `program`, a bare name, is a file at the root of `tree` that git tracks: one the
/// gate's comparison covers. An ignored or untracked file of that name is in no commit, so it
/// does not keep the name bare (story:gate-commands-find-programs-on-a-system-path).
fn tracked_at_root(tree: &Path, program: &str) -> bool {
    tree.join(program).is_file()
        && crate::git::run(
            tree,
            &["--literal-pathspecs", "ls-files", "-z", "--", program],
        )
        .is_ok_and(|listed| listed.split('\0').any(|path| path == program))
}

/// An empty home made for one run of a test command or a gate's commands, outside the tree,
/// removed when dropped. The command runs with `HOME` there and `PYTHONNOUSERSITE=1`, so
/// nothing a step wrote in the caller's home (Python's user site, its start-up files) runs
/// with it, and `PYTHONDONTWRITEBYTECODE=1`, so its Python writes no byte code into a directory
/// outside the tree takt snapshots; cargo and rustup keep the caller's homes ([`FreshHome::vars`]).
pub(crate) struct FreshHome(PathBuf);

impl FreshHome {
    /// A new empty directory under the system's temporary directory, readable by its owner
    /// alone.
    pub(crate) fn new() -> Result<Self> {
        static MADE: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let made = MADE.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |since| since.subsec_nanos());
        let dir =
            std::env::temp_dir().join(format!("takt-home-{}-{made}-{nanos}", std::process::id()));
        let mut builder = std::fs::DirBuilder::new();
        #[cfg(unix)]
        std::os::unix::fs::DirBuilderExt::mode(&mut builder, 0o700);
        builder
            .create(&dir)
            .with_context(|| format!("create a home for the command at {}", dir.display()))?;
        Ok(Self(dir))
    }

    /// What the command runs with, given what it would inherit (`inherited`): `HOME` this
    /// directory, `PYTHONNOUSERSITE=1`, `PYTHONDONTWRITEBYTECODE=1`, and `CARGO_HOME` and
    /// `RUSTUP_HOME`, when it would inherit none, the caller's: its own variable when it is an
    /// absolute path, else `.cargo` and `.rustup` in its home, which is `HOME` when that is an
    /// absolute path and else the home this user's passwd entry names ([`passwd_home`]). A
    /// relative one is never taken, so neither resolves from the command's directory, the tree;
    /// with no home at all they stay unset.
    pub(crate) fn vars(
        &self,
        inherited: impl Fn(&str) -> bool,
    ) -> Vec<(&'static str, std::ffi::OsString)> {
        let caller_home = std::env::var_os("HOME")
            .map(PathBuf::from)
            .filter(|home| home.is_absolute())
            .or_else(passwd_home);
        let mut vars = vec![
            ("HOME", self.0.clone().into_os_string()),
            ("PYTHONNOUSERSITE", "1".into()),
            ("PYTHONDONTWRITEBYTECODE", "1".into()),
        ];
        for (name, under) in [("CARGO_HOME", ".cargo"), ("RUSTUP_HOME", ".rustup")] {
            if inherited(name) {
                continue;
            }
            let caller = std::env::var_os(name)
                .filter(|value| Path::new(value).is_absolute())
                .or_else(|| caller_home.as_ref().map(|home| home.join(under).into()));
            vars.extend(caller.map(|value| (name, value)));
        }
        vars
    }

    /// The assertions project file `project` with every command it admits, and every
    /// provider's, run in this home ([`FreshHome::vars`]): the gate's commands inherit nothing,
    /// so `CARGO_HOME` and `RUSTUP_HOME` are set unless the binding sets them. A bare program
    /// name (no `/`) that is not a file `tree` tracks is written absolute, found on the `PATH` the
    /// binding's `env` declares or else on [`FIXED_PATH`], and a binding that declares no `PATH`
    /// runs with [`FIXED_PATH`]; a name found nowhere is refused, naming it and the path searched.
    fn admitting(&self, project: &str, tree: &Path) -> Result<String> {
        let mut doc: serde_yaml_ng::Value =
            serde_yaml_ng::from_str(project).context("read the project file")?;
        let Some(top) = doc.as_mapping_mut() else {
            bail!("the project file is not a mapping");
        };
        let mut bindings: Vec<&mut serde_yaml_ng::Mapping> = Vec::new();
        for (key, value) in top.iter_mut() {
            let Some(entries) = value.as_mapping_mut() else {
                continue;
            };
            match key.as_str() {
                Some("commands") => bindings.extend(
                    entries
                        .values_mut()
                        .filter_map(serde_yaml_ng::Value::as_mapping_mut),
                ),
                Some("providers") => {
                    bindings.extend(entries.values_mut().filter_map(|provider| {
                        provider
                            .get_mut("command")
                            .and_then(serde_yaml_ng::Value::as_mapping_mut)
                    }));
                }
                _ => {}
            }
        }
        for binding in bindings {
            let declared = binding
                .get("env")
                .and_then(|env| env.get("PATH"))
                .and_then(serde_yaml_ng::Value::as_str)
                .map(str::to_owned);
            if let Some(program) = binding
                .get("executable")
                .and_then(serde_yaml_ng::Value::as_str)
                .map(str::to_owned)
                .filter(|program| !program.contains('/') && !tracked_at_root(tree, program))
            {
                let searched = declared.clone().unwrap_or_else(|| FIXED_PATH.to_owned());
                let Some(found) = found_on(&program, &searched) else {
                    bail!(
                        "the command runs {program}, which is on no directory of the path searched, {searched}; a bare program name is found on the PATH the command's env declares, or else on the fixed path {FIXED_PATH}, never on the user's PATH"
                    );
                };
                let inside = match (found.canonicalize(), tree.canonicalize()) {
                    (Ok(at), Ok(root)) => at.starts_with(root),
                    _ => true,
                };
                if inside {
                    bail!(
                        "the command runs {program} from {}, inside the tree, which the gate's comparison does not cover; a program a gate runs is outside the tree",
                        found.display()
                    );
                }
                binding.insert(
                    "executable".into(),
                    found.to_string_lossy().into_owned().into(),
                );
            }
            let env = binding
                .entry("env".into())
                .or_insert_with(|| serde_yaml_ng::Value::Mapping(serde_yaml_ng::Mapping::new()));
            let Some(env) = env.as_mapping_mut() else {
                bail!("the project file binds an `env` that is not a mapping");
            };
            let set: Vec<String> = env
                .keys()
                .filter_map(|key| key.as_str().map(str::to_owned))
                .collect();
            for (name, value) in self.vars(|name| set.iter().any(|key| key == name)) {
                env.insert(name.into(), value.to_string_lossy().into_owned().into());
            }
            if !env.contains_key("PATH") {
                env.insert("PATH".into(), FIXED_PATH.into());
            }
        }
        serde_yaml_ng::to_string(&doc).context("write the project file")
    }

    /// The directory.
    pub(crate) fn dir(&self) -> &Path {
        &self.0
    }

    /// Runs `command` in this home ([`FreshHome::vars`]).
    pub(crate) fn apply(&self, command: &mut Command) {
        for (name, value) in self.vars(|name| std::env::var_os(name).is_some()) {
            command.env(name, value);
        }
    }
}

impl Drop for FreshHome {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// Whether the commit `at` holds an executable entry at `path`: a file with an exec bit, or a
/// link.
fn executable_at(tree: &Path, at: &str, path: &str) -> Result<bool> {
    Ok(object_at(tree, at, path)?
        .is_some_and(|object| object.starts_with("100755 ") || object.starts_with("120000 ")))
}

/// Whether the commit `at` holds a program at `path`: a file with an exec bit, or one whose
/// first two bytes are `#!`.
fn program_at(tree: &Path, at: &str, path: &str) -> Result<bool> {
    let Some(object) = object_at(tree, at, path)? else {
        return Ok(false);
    };
    let mut fields = object.split_whitespace();
    let (mode, kind, id) = (fields.next(), fields.next(), fields.next());
    if mode == Some("100755") {
        return Ok(true);
    }
    match (kind, id) {
        (Some("blob"), Some(id)) => Ok(crate::git::blob(tree, id)?.starts_with(b"#!")),
        _ => Ok(false),
    }
}

/// The NUL-terminated fields of `listed`, what `git <command> -z` printed, each decoded as
/// UTF-8. A field that is not UTF-8 is never decoded to a stand-in, which could read as another
/// path: it is the error, naming the path with its bytes escaped (after the first tab when
/// `after_tab`, as `ls-tree` prints the mode, type and object before it).
fn nul_fields<'a>(listed: &'a [u8], command: &str, after_tab: bool) -> Result<Vec<&'a str>> {
    listed
        .split(|byte| *byte == 0)
        .filter(|field| !field.is_empty())
        .map(|field| {
            std::str::from_utf8(field).map_err(|_| {
                let path = if after_tab {
                    field
                        .splitn(2, |byte| *byte == b'\t')
                        .last()
                        .unwrap_or(field)
                } else {
                    field
                };
                anyhow!(
                    "git {command} named a path that is not UTF-8: {}",
                    path.escape_ascii()
                )
            })
        })
        .collect()
}

/// An observation that recorded nothing, its summary `summary`.
fn nothing_recorded(summary: String) -> Observation {
    Observation {
        summary,
        records: Vec::new(),
        artifact: None,
    }
}

/// `tests.run`: runs the test command without a shell; exit 0 is `pass`, anything else `fail`.
/// The command is code from the tree: it gets no credential, no standard input, a timeout past
/// which it is killed with what it started, and only a bounded summary of its output is kept
/// ([`summarise_tests`]); under `none` its environment names no login of the user's
/// ([`crate::confinement::unconfined`]), and under `substrate` it runs confined
/// ([`crate::confinement::Substrate`]).
/// What decides it besides the code under test is on the run's judge surface, which no step
/// changes: the cargo configuration, the toolchain, build scripts and the command's own script
/// ([`crate::run::JudgeSurface`]). In a case, nothing is run and no record written while the
/// case's branch changed one of those since its base, or when the case pins no base to compare
/// with; the summary names the paths.
#[derive(Debug, Clone)]
pub struct TestsRun {
    /// The command and its arguments.
    pub command: Vec<String>,
    /// What the command is held to.
    pub limits: Limits,
}

impl TestsRun {
    /// The action it performs.
    pub const ACTION: &str = "tests.run";
    /// The evidence kind it writes.
    pub const KIND: &str = "test_result";

    /// Splits `command` at white space.
    pub fn new(command: &str) -> Result<Self> {
        let command: Vec<String> = command.split_whitespace().map(str::to_owned).collect();
        if command.is_empty() {
            bail!("the test command is empty");
        }
        Ok(Self {
            command,
            limits: Self::LIMITS,
        })
    }

    /// The limits a test command is held to by default: thirty minutes, 64 KiB of output.
    pub const LIMITS: Limits = Limits {
        timeout: std::time::Duration::from_secs(1800),
        output: 64 << 10,
    };

    /// Held to `limits`.
    #[must_use]
    pub fn with_limits(mut self, limits: Limits) -> Self {
        self.limits = limits;
        self
    }
}

/// What the test command's output says about its tests, for the step's summary. With one or
/// more `test result:` lines (cargo's, one per test binary and doc-test run): passed, failed and
/// ignored summed over every one and the number of such lines; so a workspace run whose last
/// block is an empty doc-test run is not read as `0 passed`, and a block that failed is not
/// hidden by a later one that passed. The failed tests by name follow it outside its bound
/// ([`failed_tests`], [`TestsRun`]). Without one: the first non-empty line beside the
/// last four when the output has more than five, so a summary a command prints first is kept,
/// else every non-empty line; lines joined by ` | `.
fn summarise_tests(printed: &str, success: bool) -> String {
    let lines: Vec<&str> = printed.lines().collect();
    let kept: Vec<&str> = lines
        .iter()
        .copied()
        .filter(|line| !line.trim().is_empty())
        .collect();
    let (mut passed, mut failed, mut ignored, mut results) = (0u64, 0u64, 0u64, 0usize);
    for line in &lines {
        let Some(rest) = line.trim_start().strip_prefix("test result:") else {
            continue;
        };
        results += 1;
        for part in rest.split(';') {
            let mut words = part.split_whitespace();
            let Some(count) = words.by_ref().find_map(|word| word.parse::<u64>().ok()) else {
                continue;
            };
            // Saturating: a count a test command prints is not trusted to fit, and a sum that
            // wraps would read a failure as none.
            match words.next() {
                Some("passed") => passed = passed.saturating_add(count),
                Some("failed") => failed = failed.saturating_add(count),
                Some("ignored") => ignored = ignored.saturating_add(count),
                _ => {}
            }
        }
    }
    if results > 0 {
        let plural = if results == 1 { "" } else { "s" };
        let mut summary = format!(
            "{passed} passed, {failed} failed, {ignored} ignored over {results} result line{plural}"
        );
        // A failure the result lines do not show (a binary killed by a signal, a runner whose
        // `test result:` line carries no counts): the last line says what the counts cannot.
        if !success
            && failed == 0
            && let Some(last) = kept.last()
        {
            summary.push_str(" | ");
            summary.push_str(last.trim());
        }
        return summary;
    }
    if kept.len() <= 5 {
        return kept.join(" | ");
    }
    let mut shown = vec![kept[0], "…"];
    shown.extend_from_slice(&kept[kept.len() - 4..]);
    shown.join(" | ")
}

/// Adds `name`, trimmed, to `names` unless it is empty or there already.
fn push_once(names: &mut Vec<String>, name: &str) {
    let name = name.trim();
    if !name.is_empty() && !names.iter().any(|named| named == name) {
        names.push(name.to_owned());
    }
}

/// The tests a failing test command's output names as failed, in output order, each once:
/// cargo's `test <name> ... FAILED` lines, or, under `cargo test --quiet`, which prints none,
/// the names listed under the last `failures:` heading, one indented a line, when a `test
/// result: FAILED` line says one failed; Python unittest's `FAIL: <test>` and `ERROR: <test>`
/// headings; pytest's `FAILED <id>` lines, up to ` - `. Empty for output that names none.
fn failed_tests(printed: &str) -> Vec<String> {
    let lines: Vec<&str> = printed.lines().collect();
    let mut names = Vec::new();
    for line in &lines {
        let line = line.trim();
        if let Some(name) = line
            .strip_prefix("test ")
            .and_then(|rest| rest.strip_suffix(" ... FAILED"))
        {
            push_once(&mut names, name);
        } else if let Some(name) = line
            .strip_prefix("FAIL: ")
            .or_else(|| line.strip_prefix("ERROR: "))
        {
            push_once(&mut names, name);
        } else if let Some(id) = line.strip_prefix("FAILED ")
            && !id.starts_with('(')
        {
            push_once(&mut names, id.split(" - ").next().unwrap_or(id));
        }
    }
    if names.is_empty()
        && lines
            .iter()
            .any(|line| line.trim_start().starts_with("test result: FAILED"))
        && let Some(heading) = lines.iter().rposition(|line| line.trim() == "failures:")
    {
        for line in lines[heading + 1..]
            .iter()
            .take_while(|line| line.starts_with("    ") || line.trim().is_empty())
        {
            push_once(&mut names, line);
        }
    }
    names
}

/// The runner's hint for running a failure again, from `to rerun` on, on the first line that
/// says it: cargo's `error: test failed, to rerun pass `-p <crate> --lib``.
fn rerun_hint(printed: &str) -> Option<String> {
    printed
        .lines()
        .find_map(|line| line.find("to rerun").map(|at| line[at..].trim().to_owned()))
}

/// Where a `tests.run` summary lists the failed tests ([`TestsRun`]): after the last of these.
const FAILED_MARK: &str = "; failed: ";

/// The most characters a `tests.run` summary keeps of what the runner said, and the most its
/// printed line shows after its `the test command exited` part ([`tests_line`]); a part cut to
/// it ends in `…`.
const SUMMARY_LIMIT: usize = 300;

/// The fewest characters of what the runner said that a failing `tests.run` line shows before
/// its rerun hint and failed tests ([`tests_line`]): room the two leave over goes to it as well.
const SUMMARY_LEAST: usize = 100;

/// The most characters of the rerun hint a `tests.run` summary keeps.
const RERUN_LIMIT: usize = 200;

/// The most failed tests a `tests.run` summary names, 64, and the most bytes of their list,
/// 8 KiB: a set past either is cut and ends in `…`, and names no set ([`failures_in`]).
const FAILED_NAMES: usize = 64;
const FAILED_BYTES: usize = 8 << 10;

/// The failed tests `failed` as a `tests.run` summary lists them, joined by `, `: the whole set
/// within [`FAILED_NAMES`] and [`FAILED_BYTES`], or the names that fit and `…`.
fn failed_list(failed: &[String]) -> String {
    let mut list = String::new();
    for (at, name) in failed.iter().enumerate() {
        let sep = if at == 0 { "" } else { ", " };
        if at == FAILED_NAMES || list.len() + sep.len() + name.len() > FAILED_BYTES {
            list.push_str(sep);
            list.push('…');
            return list;
        }
        list.push_str(sep);
        list.push_str(name);
    }
    list
}

/// The `tests.run` summary `summary` as the run prints it on its `effect:` line: its `the test
/// command exited` part, then the rest within [`SUMMARY_LIMIT`] characters, what the runner said
/// giving way to the rerun hint and the failed tests down to [`SUMMARY_LEAST`]. The summary the
/// run records keeps the whole set ([`failures_in`]).
#[must_use]
pub fn tests_line(summary: &str) -> String {
    let Some(at) = summary.find("; ") else {
        return summary.to_owned();
    };
    let (exited, rest) = summary.split_at(at + 2);
    let named_at = [rest.rfind("; to rerun"), rest.rfind(FAILED_MARK)]
        .into_iter()
        .flatten()
        .min()
        .unwrap_or(rest.len());
    let (said, named) = rest.split_at(named_at);
    let room = SUMMARY_LIMIT
        .saturating_sub(named.chars().count())
        .max(SUMMARY_LEAST);
    let shown = bounded(&format!("{}{named}", bounded(said, room)), SUMMARY_LIMIT);
    format!("{exited}{shown}")
}

/// What a failing `tests.run` summary names ([`TestsRun`] writes it, [`failures_in`] reads it):
/// the failed tests, and the runner's hint for running them again.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Failures {
    /// The failed tests in output order; empty when the summary names none, or its list was cut
    /// short, so a set a reader compares is never a part of one.
    pub tests: Vec<String>,
    /// The runner's rerun hint (`to rerun pass ...`), when its output had one.
    pub rerun: Option<String>,
}

/// What the `tests.run` summary `summary` names as failed: the list after its last `; failed: `
/// and the hint after its `; to rerun`, as [`TestsRun`] writes them.
#[must_use]
pub fn failures_in(summary: &str) -> Failures {
    let (head, list) = match summary.rfind(FAILED_MARK) {
        Some(at) => (&summary[..at], Some(&summary[at + FAILED_MARK.len()..])),
        None => (summary, None),
    };
    let tests = match list {
        Some(list) if !list.ends_with('…') => list
            .split(", ")
            .filter(|name| !name.is_empty())
            .map(str::to_owned)
            .collect(),
        _ => Vec::new(),
    };
    let rerun = head.rfind("; to rerun").map(|at| head[at + 2..].to_owned());
    Failures { tests, rerun }
}

/// `text` within `limit` characters: the whole of it, or its first `limit - 1` characters and
/// `…`, so a cut summary says it was cut.
fn bounded(text: &str, limit: usize) -> String {
    if text.chars().count() <= limit {
        return text.to_owned();
    }
    let mut cut: String = text.chars().take(limit.saturating_sub(1)).collect();
    cut.push('…');
    cut
}

impl Validator for TestsRun {
    fn name(&self) -> &str {
        "takt.tests-run"
    }

    fn action(&self) -> &str {
        Self::ACTION
    }

    fn kind(&self) -> &str {
        Self::KIND
    }

    fn subject(&self) -> &str {
        "implementation"
    }

    fn runs(&self) -> Vec<String> {
        self.command.clone()
    }

    fn observe(&self, tree: &Path, revision: &str, seq: usize) -> Result<Observation> {
        self.observe_by(tree, revision, seq, None)
    }

    fn observe_within(
        &self,
        tree: &Path,
        revision: &str,
        seq: usize,
        deadline: Option<std::time::Instant>,
    ) -> Result<(Observation, bool)> {
        let observed = self.observe_by(tree, revision, seq, deadline);
        Ok((ended_by(deadline, self.action(), observed)?, false))
    }
}

/// What an observation held to `deadline` answers: an error once it returned at or after it,
/// whatever it observed, so the budget that ended it records nothing; else `observed`.
fn ended_by(
    deadline: Option<std::time::Instant>,
    action: &str,
    observed: Result<Observation>,
) -> Result<Observation> {
    if !deadline.is_some_and(|deadline| std::time::Instant::now() >= deadline) {
        return observed;
    }
    match observed {
        Err(error) => Err(error),
        Ok(_) => {
            bail!("{action} returned at or after the run's end; what it observed is not recorded")
        }
    }
}

impl TestsRun {
    /// Observes, the test command held to the smaller of its own timeout and what is left
    /// before `deadline` when it starts.
    fn observe_by(
        &self,
        tree: &Path,
        revision: &str,
        seq: usize,
        deadline: Option<std::time::Instant>,
    ) -> Result<Observation> {
        let shown = self.command.join(" ");
        // A tree on no case's branch is no case's observation and has no base to compare with.
        if crate::git::current_branch(tree)?
            .is_some_and(|branch| branch.starts_with(crate::case::BRANCH_PREFIX))
        {
            let dir = current(tree)?;
            match judged(tree, &dir, revision, &self.command, Vec::new(), false)? {
                Judged::Unchanged => {}
                Judged::NoBase => {
                    return Ok(nothing_recorded(format!(
                        "the test command `{shown}`: not run; the case pins no base commit ({BRANCH_FILE} has no base_commit) to compare its judge surface with; no record, the claim stays unknown; abandon the case and open it again"
                    )));
                }
                Judged::Changed { base, paths } => {
                    return Ok(nothing_recorded(format!(
                        "the test command `{shown}`: not run; the case's branch at {} changed the judge surface since {base}: {}; no record, the claim stays unknown",
                        short(revision),
                        paths.join(", ")
                    )));
                }
            }
        }
        let mut limits = self.limits;
        if let Some(deadline) = deadline {
            limits.timeout = limits
                .timeout
                .min(deadline.saturating_duration_since(std::time::Instant::now()));
        }
        // The adapter in use runs it: under `none` as [`crate::confinement::unconfined`] starts
        // it, under another as that adapter confines it, or refuses it.
        let output = crate::confinement::current()
            .run_test(&crate::confinement::TestRun {
                argv: &self.command,
                tree,
                limits,
            })
            .with_context(|| format!("run {}", self.command.join(" ")))?;
        let result = if output.success() { "pass" } else { "fail" };
        let printed =
            String::from_utf8_lossy(&output.stdout) + String::from_utf8_lossy(&output.stderr);
        let tail = summarise_tests(&printed, output.success());
        let record = Record::new(
            format!("test-result-{seq}"),
            self.kind(),
            Some(result.to_owned()),
            self.subject(),
            revision,
            self.name(),
        );
        // What the runner said, bounded; then a failure's rerun hint and the failed tests by
        // name, the names last ([`failures_in`] reads them back), within their own bounds: the
        // printed line is cut apart ([`tests_line`]), so the set never depends on it.
        let mut summary = format!(
            "the test command exited {}: {result}; {}",
            output.code.map_or("signal".to_owned(), |c| c.to_string()),
            bounded(&tail, SUMMARY_LIMIT),
        );
        if !output.success() {
            if let Some(hint) = rerun_hint(&printed) {
                summary.push_str("; ");
                summary.push_str(&bounded(&hint, RERUN_LIMIT));
            }
            let failed = failed_tests(&printed);
            if !failed.is_empty() {
                summary.push_str(FAILED_MARK);
                summary.push_str(&failed_list(&failed));
            }
        }
        Ok(Observation {
            summary,
            records: vec![record],
            artifact: None,
        })
    }
}

/// The project file a gates document is evaluated with when the validator is given none and
/// the tree has it: `engineering-assertions-project/1`, at the path the assertions guide names.
pub const DEFAULT_PROJECT: &str = ".engineering/assertions-project.yaml";

/// `gate.run`: evaluates the repository's `engineering-gates/1` document through the
/// engineering assertions library, as `canon-engineering gates run` does. Every gate true is
/// `pass`, any gate false is `fail`; an unknown evaluation writes no record, because a claim
/// that could not be observed stays unknown; an invalid document or provider datum is an error.
/// The project file admits the commands and providers the gates bind. Every observation
/// retains the bundle the library evaluated, `engineering-gate-evidence/1`, as
/// `<record-id>.bundle.json`.
///
/// A false evaluation keeps, for each gate with a false assertion, what the commands that
/// assertion names printed that names what failed ([`FailedCheck`], [`failing_lines`]), taken
/// once with [`Validator::take_failed`]: the output the observation kept (`process.run`,
/// `ess.run`), or, for `process.succeeded`, which keeps only whether the command exited 0, the
/// command run again once, only when it failed, as the library ran it, after the evaluation,
/// held to the project file's timeout and output bound and to `deadline`; a run again that
/// passes keeps one line saying so ([`PASSED_AGAIN`]) and none of its output ([`printed_by`]).
///
/// The gates document and the project file are read from the case's base revision, the tip of
/// the branch in `branch.yaml`, never from the tree: the tree is the case's branch, which a
/// model edits, and a gate it rewrote would decide its own verdict. What the gates observe is
/// the tree. The two files are copied for the evaluation into the case's directory, which the
/// tree's source identity leaves out, and removed again before the observation ends.
///
/// What the gates observe runs from the tree only while the base revision and the observed one
/// agree on it: with a project file, the executables and catalogs it admits, every file its
/// commands take as an argument, the tool configuration ([`GATE_TOOL_CONFIG`]), the `[lints]`
/// and `[workspace.lints]` tables of every cargo manifest ([`lint_changes`]), what Python loads
/// from the tree for its commands, a `sh -c` string or shell script of theirs among them, and the
/// paths such a string or script names ([`python_paths`]; every Python file, shell file and program
/// while one cannot be read with certainty) and the judge surface the test command's is
/// compared on. One that differs, was added or was deleted runs
/// nothing and writes no record; the summary names it.
///
/// Code that runs while cargo builds (a build script, a procedural macro, a dependency's) is
/// not closed by comparing files: the step writes the code under test, and a gate whose
/// commands build it runs what the step wrote. `gate.green` cannot vouch for such code; the
/// merge of the change stays the owner's to grant.
#[derive(Debug, Clone)]
pub struct GateRun {
    /// The gates document, relative to the tree.
    pub gates: PathBuf,
    /// The `engineering-assertions-project/1` file that admits the gates' commands and
    /// providers, relative to the tree; `None` takes [`DEFAULT_PROJECT`] when the base
    /// revision has it.
    pub project: Option<PathBuf>,
    /// The failed gates of the last observation ([`Validator::take_failed`]).
    failed: RefCell<Vec<FailedCheck>>,
    /// The obligations of the last observation ([`Validator::take_obligations`]).
    obligations: RefCell<Option<Vec<crate::obligations::Obligation>>>,
}

/// The directory under the case's that holds the base revision's gate inputs while the gate
/// validator evaluates them.
const GATE_INPUTS: &str = "gate-inputs";

/// The case's base revision: the commit pinned in `branch.yaml` when the case opened, shown
/// with the base branch's name, and the commit. `None` for a case that pins none (one opened
/// before the pin was recorded): no branch name stands in for it, because a test command can
/// move any branch, even onto `HEAD`, without touching the tree.
fn base_revision(tree: &Path, case_dir: &Path) -> Result<Option<(String, String)>> {
    let branch = Branch::read(case_dir)?;
    let shown = branch.base.as_deref().unwrap_or("a detached HEAD");
    let Some(pinned) = &branch.base_commit else {
        return Ok(None);
    };
    let at = crate::git::run(
        tree,
        &[
            "rev-parse",
            "--verify",
            "--quiet",
            "--end-of-options",
            &format!("{pinned}^{{commit}}"),
        ],
    )
    .with_context(|| format!("resolve the case's base commit {pinned}"))?;
    Ok(Some((
        format!("{shown} at {}, pinned at open", short(&at)),
        at,
    )))
}

/// What the gate runner is told of the change: its `base` the case's pinned base commit `at`, its
/// `subject` the `revision` observed, and the story the case works (`story_id`) with the tags the
/// planning store gives it at the base; a case with no story, or one whose story file the base
/// does not hold, tells the runner no tags, so a gate that asks for one is unknown, not false.
fn change_of(
    tree: &Path,
    case_dir: &Path,
    at: &str,
    revision: &str,
) -> Result<canon_engineering_assertions::Change> {
    let story = crate::case::story_of(case_dir)?;
    let tags = match &story {
        Some(story) => crate::case::story_front_at(tree, at, story)?.map(|front| {
            front["tags"]
                .as_sequence()
                .map(|tags| {
                    tags.iter()
                        .filter_map(|tag| tag.as_str().map(str::to_owned))
                        .collect()
                })
                .unwrap_or_default()
        }),
        None => None,
    };
    Ok(canon_engineering_assertions::Change {
        base: Some(at.to_owned()),
        subject: Some(revision.to_owned()),
        story_id: story,
        story_tags: tags,
    })
}

/// The files a project file names that decide what a gate runs: the executables of the
/// commands and providers it admits, and the catalogs it extends the functions with; those
/// relative to the tree, which a model can edit, as they are named.
fn admitted_files(project: &str) -> Result<Vec<String>> {
    let doc: serde_yaml_ng::Value =
        serde_yaml_ng::from_str(project).context("read the project file")?;
    let mut paths = Vec::new();
    let named = |value: &serde_yaml_ng::Value| value.as_str().map(str::to_owned);
    if let Some(commands) = doc["commands"].as_mapping() {
        paths.extend(commands.values().filter_map(|c| named(&c["executable"])));
    }
    if let Some(providers) = doc["providers"].as_mapping() {
        paths.extend(
            providers
                .values()
                .filter_map(|p| named(&p["command"]["executable"])),
        );
    }
    if let Some(catalogs) = doc["catalogs"].as_sequence() {
        paths.extend(catalogs.iter().filter_map(named));
    }
    Ok(inside_tree(&paths))
}

/// `paths` relative to the tree and normalised, sorted and each once; one that leads out of the
/// tree is not the tree's.
fn inside_tree(paths: &[String]) -> Vec<String> {
    let mut inside: Vec<String> = paths
        .iter()
        .filter(|path| Path::new(path).is_relative())
        .filter_map(|path| {
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
            (!parts.is_empty()).then(|| parts.join("/"))
        })
        .collect();
    inside.sort();
    inside.dedup();
    inside
}

/// The arguments of the commands and providers a project file admits, as paths relative to the
/// tree ([`inside_tree`]): each argument as it is written, and an option's value after its `=`
/// (`--path=dir`). Which of them a commit holds is the caller's to read.
fn argument_paths(project: &str) -> Result<Vec<String>> {
    let doc: serde_yaml_ng::Value =
        serde_yaml_ng::from_str(project).context("read the project file")?;
    let mut bindings: Vec<&serde_yaml_ng::Value> = Vec::new();
    if let Some(commands) = doc["commands"].as_mapping() {
        bindings.extend(commands.values());
    }
    if let Some(providers) = doc["providers"].as_mapping() {
        bindings.extend(providers.values().map(|provider| &provider["command"]));
    }
    let mut paths = Vec::new();
    for argument in bindings
        .iter()
        .filter_map(|binding| binding["args"].as_sequence())
        .flatten()
        .filter_map(serde_yaml_ng::Value::as_str)
    {
        paths.push(argument.to_owned());
        if let Some((_, value)) = argument
            .strip_prefix('-')
            .and_then(|option| option.split_once('='))
        {
            paths.push(value.to_owned());
        }
    }
    Ok(inside_tree(&paths))
}

/// What Python loads from the tree for the commands and providers a project file admits, read
/// as the step guard reads a test command ([`crate::run::scripts_of`]): each binding's words,
/// its `env` before them as `NAME=VALUE`, and again from each word that names a Python on (a
/// wrapper's command). The first of the pair is paths, the second globs ([`crate::run::parse_judge_list`]):
///
/// - In the directory a Python script runs from, which Python puts first on its import path
///   (the script's own directory; the directory itself for `python3 <dir>`, which runs its
///   `__main__.py`): every module, compiled module and `*.pth` file directly in it, and every
///   package, a directory there holding a module at any depth, whole ([`python_dir`]). A script
///   is a path the guard names, in an import directory of its own, that the base or the revision
///   holds as a file run by a Python (its `#!` line names one, or a Python runs the command and
///   no other `#!` line does) or as a directory with a `__main__.py`. A script whose directory is
///   the one the command runs in, the tree's root, is taken as `-m` is below: the script itself
///   and the `*.pth` files there, not every module and package of the code the gate checks.
/// - What `python -m a.b` runs, in the directory the command runs in and in every `PYTHONPATH`
///   entry the binding sets: `a.py`, `a.*` and the package `a/` whole, and the `*.pth` files
///   there, the globs the guard names for it. Not every module of those directories: the
///   directory a gate runs in is the tree's root and a `PYTHONPATH` entry is as often the code
///   the gate checks (`src`), and comparing all of it would leave every gate that runs Python
///   over that code not run once a step changed it. A name the Python imports from outside the
///   tree stays the step guard's to refuse there.
///
/// The reader is the step guard's whole ([`crate::run::read_command`]): a Python run through a
/// shell (`sh -c "python3 check.py"`, a shell script the binding runs, a `sh -c` string running
/// one) is a Python script as one the binding runs directly is, and what its `-m` runs is
/// compared as above, the shell script and every path the string names besides.
///
/// What cannot be read with certainty fails closed, as in the step guard: the third of the
/// result is why ([`crate::run::FAIL_CLOSED`]'s reasons), and while there is one the caller
/// compares every Python file, shell file and program of the tree.
fn python_paths(
    tree: &Path,
    at: &str,
    revision: &str,
    project: &str,
) -> Result<(Vec<String>, Vec<String>, Vec<String>)> {
    let doc: serde_yaml_ng::Value =
        serde_yaml_ng::from_str(project).context("read the project file")?;
    let mut bindings: Vec<&serde_yaml_ng::Value> = Vec::new();
    if let Some(commands) = doc["commands"].as_mapping() {
        bindings.extend(commands.values());
    }
    if let Some(providers) = doc["providers"].as_mapping() {
        bindings.extend(providers.values().map(|provider| &provider["command"]));
    }
    let mut dirs: Vec<String> = Vec::new();
    let mut scripts: Vec<String> = Vec::new();
    let mut globs: Vec<String> = Vec::new();
    let mut closed: Vec<String> = Vec::new();
    for binding in bindings {
        let Some(executable) = binding["executable"].as_str() else {
            continue;
        };
        let mut words: Vec<String> = vec![executable.to_owned()];
        words.extend(
            binding["args"]
                .as_sequence()
                .into_iter()
                .flatten()
                .filter_map(serde_yaml_ng::Value::as_str)
                .map(str::to_owned),
        );
        let env: Vec<String> = binding["env"]
            .as_mapping()
            .into_iter()
            .flatten()
            .filter_map(|(name, value)| Some(format!("{}={}", name.as_str()?, value.as_str()?)))
            .collect();
        let pythons: Vec<usize> = (0..words.len())
            .filter(|&k| crate::run::python_like(&words[k]))
            .collect();
        let mut commands: Vec<Vec<String>> = vec![[env.clone(), words.clone()].concat()];
        commands.extend(
            pythons
                .iter()
                .filter(|&&k| k > 0)
                .map(|&k| [env.clone(), words[k..].to_vec()].concat()),
        );
        for (k, command) in commands.iter().enumerate() {
            let read = crate::run::read_command(command, tree);
            // What a Python loads as code, wherever the reader found it: in the binding's words,
            // a `sh -c` string or a shell script it runs.
            let python: Vec<&str> = read
                .iter()
                .filter_map(|path| path.strip_prefix(crate::run::PYTHON_MARK))
                .collect();
            closed.extend(
                read.iter()
                    .filter_map(|path| path.strip_prefix(crate::run::FAILS_CLOSED_MARK))
                    .map(str::to_owned),
            );
            let named: Vec<String> = read
                .iter()
                .filter(|path| !path.starts_with(crate::run::PYTHON_MARK))
                .cloned()
                .collect();
            // A path a `sh -c` string or a shell script names (a shell script it runs, the
            // Python script), compared as an argument of the binding is; not a directory the
            // base holds, which is the data a command checks.
            for path in named.iter().filter(|path| !path.starts_with('(')) {
                if path.contains(['*', '?']) || path.contains("/(") {
                    continue;
                }
                if !object_at(tree, at, path)?.is_some_and(|o| o.starts_with("040000 ")) {
                    scripts.push(path.clone());
                }
            }
            globs.extend(
                python
                    .iter()
                    .filter(|path| path.contains('*'))
                    .map(|path| (*path).to_owned()),
            );
            let imports: Vec<&str> = named
                .iter()
                .filter_map(|path| path.strip_suffix(crate::run::IMPORTS))
                .map(|dir| dir.trim_end_matches('/'))
                .collect();
            // The globs of a command a Python runs: what `-m` runs and the start-up files.
            if k > 0 || pythons.first() == Some(&0) {
                globs.extend(
                    named
                        .iter()
                        .filter(|path| path.contains('*') && !path.contains('('))
                        .filter(|path| !crate::run::FAIL_CLOSED.contains(&path.as_str()))
                        .cloned(),
                );
            }
            // What the reader marked as code a Python loads and named nowhere else (what a
            // `-c` string imports, the script Python reads from its standard input): compared
            // as the script is.
            for path in python
                .iter()
                .filter(|path| !path.contains(['*', '?', '(']))
                .filter(|path| !named.iter().any(|name| name == *path))
            {
                if !object_at(tree, at, path)?.is_some_and(|o| o.starts_with("040000 ")) {
                    scripts.push((*path).to_owned());
                }
            }
            // What each Python file the reader marked imports, at the base or at the revision,
            // followed through the files it finds in the tree: in every directory the command's
            // Python imports from, `name.py`, a compiled `name.*` and the package `name/`, as
            // `-m name` is compared. Not every module of those directories (wave 18's rule).
            let (imported_paths, imported_globs, imports_closed) =
                python_imports(tree, at, revision, &python, &imports)?;
            scripts.extend(imported_paths);
            globs.extend(imported_globs);
            closed.extend(imports_closed);
            let runs_python = !pythons.is_empty();
            for path in named.iter().filter(|path| !path.contains(['*', '?', '('])) {
                let parent = path.rsplit_once('/').map_or("", |(parent, _)| parent);
                // What `-m` runs is named with its compiled forms beside it (`a.py`, `a.*`): a
                // module the directory it runs in holds, not a script.
                let module = path
                    .strip_suffix(".py")
                    .is_some_and(|stem| named.contains(&format!("{stem}.*")));
                if !imports.contains(&parent) || module {
                    continue;
                }
                match python_script(
                    tree,
                    at,
                    revision,
                    path,
                    runs_python,
                    python.contains(&path.as_str()),
                )? {
                    // The directory the gate runs in, the tree's root, holds the code the gate
                    // checks: as for `-m`, the script itself and the `*.pth` files there.
                    Some(dir) if dir.is_empty() => {
                        scripts.push(path.clone());
                        globs.push("*.pth".to_owned());
                    }
                    Some(dir) => dirs.push(dir),
                    None => {}
                }
            }
        }
    }
    dirs.sort();
    dirs.dedup();
    let mut paths = scripts;
    for dir in dirs {
        paths.extend(python_dir(tree, at, revision, &dir)?);
    }
    paths.sort();
    paths.dedup();
    globs.sort();
    globs.dedup();
    closed.sort();
    closed.dedup();
    Ok((paths, globs, closed))
}

/// How many Python files of the tree the gate reads the imports of for one command
/// ([`python_imports`]); a walk that reaches the cap fails closed.
const IMPORTS_READ_AT_MOST: usize = 64;

/// What the Python files `python` (relative to the tree; a glob among them is passed over)
/// import at the base `at` or at `revision`, followed through the files found in `dirs`, the
/// directories the command's Python imports from (`""` the root): for each top-level name
/// ([`crate::run::imports_of`]) in each directory, the path `name.py` and the globs `name.*`
/// and `name/**`, what `-m name` would compare. A name found as a module or a package's
/// `__init__.py` is read in turn, up to [`IMPORTS_READ_AT_MOST`] files. What the walk cannot
/// follow fails closed, as a shell line the reader cannot read does; the third of the result
/// is why: a file past the cap, named, is not read, and a file read that imports by a form no
/// import line names ([`crate::run::dynamic_imports_of`]) loads what the walk cannot see.
fn python_imports(
    tree: &Path,
    at: &str,
    revision: &str,
    python: &[&str],
    dirs: &[&str],
) -> Result<(Vec<String>, Vec<String>, Vec<String>)> {
    let join = |dir: &str, name: &str| {
        if dir.is_empty() {
            name.to_owned()
        } else {
            format!("{dir}/{name}")
        }
    };
    let mut paths = Vec::new();
    let mut globs = Vec::new();
    let mut closed = Vec::new();
    let mut queue: Vec<String> = python
        .iter()
        .filter(|path| !path.contains(['*', '?', '(']))
        .map(|path| (*path).to_owned())
        .collect();
    let mut read = std::collections::BTreeSet::new();
    while let Some(path) = queue.pop() {
        if !read.insert(path.clone()) {
            continue;
        }
        if read.len() > IMPORTS_READ_AT_MOST {
            closed.push(format!(
                "`{path}` is imported deeper than the {IMPORTS_READ_AT_MOST} Python files the gate reads the imports of, so what it imports is not followed"
            ));
            break;
        }
        let mut names = std::collections::BTreeSet::new();
        for commit in [at, revision] {
            if let Some(text) = file_at(tree, commit, Path::new(&path))? {
                names.extend(crate::run::imports_of(&text));
                closed.extend(
                    crate::run::dynamic_imports_of(&text)
                        .into_iter()
                        .map(|form| {
                            format!("`{path}` imports by `{form}`, which the gate does not follow")
                        }),
                );
            }
        }
        for name in names {
            for dir in dirs {
                let module = join(dir, &format!("{name}.py"));
                let init = join(dir, &format!("{name}/__init__.py"));
                for found in [&module, &init] {
                    if object_at(tree, at, found)?.is_some()
                        || object_at(tree, revision, found)?.is_some()
                    {
                        queue.push(found.clone());
                    }
                }
                paths.push(module);
                globs.push(join(dir, &format!("{name}.*")));
                globs.push(join(dir, &format!("{name}/**")));
            }
        }
    }
    Ok((paths, globs, closed))
}

/// The directory Python imports from first when it runs `path` (relative to the tree), when the
/// base `at` or `revision` holds it as a Python script ([`python_paths`]): the script's own
/// directory, or the directory itself when it holds a `__main__.py`. With `runs_python`, a
/// Python runs the command, and a file whose `#!` line names no other program is its script;
/// with `run_by_python`, the reader saw a Python run it ([`crate::run::PYTHON_MARK`]), and a
/// file at the path is its script whatever its first line.
fn python_script(
    tree: &Path,
    at: &str,
    revision: &str,
    path: &str,
    runs_python: bool,
    run_by_python: bool,
) -> Result<Option<String>> {
    let parent = path.rsplit_once('/').map_or("", |(parent, _)| parent);
    let mut script = false;
    for commit in [at, revision] {
        let Some(object) = object_at(tree, commit, path)? else {
            continue;
        };
        if object.starts_with("040000 ") {
            for main in ["__main__.py", "__main__.pyc"] {
                if object_at(tree, commit, &format!("{path}/{main}"))?.is_some() {
                    return Ok(Some(path.to_owned()));
                }
            }
            continue;
        }
        let text = file_at(tree, commit, Path::new(path))?.unwrap_or_default();
        script |= run_by_python
            || match text.strip_prefix("#!") {
                Some(line) => crate::run::interpreter_of(line)
                    .is_some_and(|interpreter| crate::run::python_like(&interpreter)),
                None => runs_python,
            };
    }
    Ok(script.then(|| parent.to_owned()))
}

/// What Python may import from `dir` (relative to the tree, `""` its root) at the base `at` or at
/// `revision`: every module, compiled module and `*.pth` file directly in it, and every directory
/// there that holds a module at any depth, a package (or a namespace package's part, nested or
/// not), by its path, which the comparison takes whole.
fn python_dir(tree: &Path, at: &str, revision: &str, dir: &str) -> Result<Vec<String>> {
    let prefix = if dir.is_empty() {
        String::new()
    } else {
        format!("{dir}/")
    };
    let module = |name: &str| {
        name.contains('.') && (name.ends_with(".pth") || crate::run::module_of(name).is_some())
    };
    let mut paths = Vec::new();
    for commit in [at, revision] {
        let mut args = vec![
            "--literal-pathspecs",
            "ls-tree",
            "-r",
            "-z",
            "--name-only",
            "--full-tree",
            commit,
        ];
        if !dir.is_empty() {
            args.extend(["--", dir]);
        }
        let listed = crate::git::bytes(tree, &args)?;
        for path in nul_fields(&listed, "ls-tree", false)? {
            let Some(rest) = path.strip_prefix(&prefix) else {
                continue;
            };
            match rest.split('/').collect::<Vec<_>>().as_slice() {
                [name] if module(name) => paths.push(path.to_owned()),
                [package, .., name] if module(name) => paths.push(format!("{prefix}{package}")),
                _ => {}
            }
        }
    }
    paths.sort();
    paths.dedup();
    Ok(paths)
}

/// The tool configuration a gate's commands read from the tree, by file name at any depth:
/// cargo's (in a `.cargo` directory), clippy's, rustfmt's and task's. The toolchain file is on
/// every judge surface already.
const GATE_TOOL_CONFIG: [&str; 10] = [
    ".cargo/config",
    ".cargo/config.toml",
    "clippy.toml",
    ".clippy.toml",
    "rustfmt.toml",
    ".rustfmt.toml",
    "Taskfile.yml",
    "Taskfile.yaml",
    "Taskfile.dist.yml",
    "Taskfile.dist.yaml",
];

/// Whether `path`, relative to the tree, is tool configuration ([`GATE_TOOL_CONFIG`]): its last
/// part (its last two for cargo's) is one of the names, whatever its case. Matched by name and
/// not by a glob, so a link elsewhere in the tree is not on the way to it.
fn is_tool_config(path: &str) -> bool {
    let fold = |text: &str| -> String {
        text.chars()
            .flat_map(char::to_lowercase)
            .map(|c| if c == '\u{17f}' { 's' } else { c })
            .collect()
    };
    let folded = fold(path.trim_end_matches('/'));
    GATE_TOOL_CONFIG.iter().any(|name| {
        let name = fold(name);
        folded == name || folded.ends_with(&format!("/{name}"))
    })
}

/// The parts of the TOML key `key` (`a.b`, `"a".'b'`): bare, or quoted without an escape. `None`
/// for anything else, which the caller does not read with certainty.
fn key_parts(key: &str) -> Option<Vec<String>> {
    let mut parts = Vec::new();
    let mut rest = key.trim();
    loop {
        let (part, after) = if let Some(quoted) = rest.strip_prefix('"') {
            let end = quoted.find('"')?;
            if quoted[..end].contains('\\') {
                return None;
            }
            (&quoted[..end], &quoted[end + 1..])
        } else if let Some(quoted) = rest.strip_prefix('\'') {
            let end = quoted.find('\'')?;
            (&quoted[..end], &quoted[end + 1..])
        } else {
            let end = rest
                .find(|c: char| !(c.is_ascii_alphanumeric() || c == '-' || c == '_'))
                .unwrap_or(rest.len());
            if end == 0 {
                return None;
            }
            (&rest[..end], &rest[end..])
        };
        parts.push(part.to_owned());
        let after = after.trim_start();
        if after.is_empty() {
            return Some(parts);
        }
        rest = after.strip_prefix('.')?.trim_start();
    }
}

/// `line` up to its comment, and how many brackets and braces it opens less those it closes,
/// strings left out. `None` for a multi-line string or a string the line does not close.
fn toml_code(line: &str) -> Option<(&str, i32)> {
    let mut depth = 0;
    let mut quote: Option<char> = None;
    let mut chars = line.char_indices().peekable();
    while let Some((at, c)) = chars.next() {
        match (quote, c) {
            (Some('"'), '\\') => {
                chars.next();
            }
            (Some(q), c) if c == q => quote = None,
            (Some(_), _) => {}
            (None, '"' | '\'') => {
                if line[at..].starts_with("\"\"\"") || line[at..].starts_with("'''") {
                    return None;
                }
                quote = Some(c);
            }
            (None, '#') => return Some((&line[..at], depth)),
            (None, '[' | '{') => depth += 1,
            (None, ']' | '}') => depth -= 1,
            (None, _) => {}
        }
    }
    quote.is_none().then_some((line, depth))
}

/// The entries of the cargo manifest `text` that decide its lints, `[lints]` and
/// `[workspace.lints]` as a table, a dotted key or an inline table, each by its key path with its
/// value as written: a strict scanner of one-line headers and keys whose values may span lines in
/// brackets, not a TOML parser. `None` for a line it cannot read with certainty (a multi-line
/// string, an escape in a key, a header with anything after it).
fn lint_entries(text: &str) -> Option<Vec<(String, String)>> {
    // Under `lints` or `workspace.lints`; or the root key `workspace` itself, an inline table.
    let under = |key: &[String]| {
        key.first().is_some_and(|first| first == "lints")
            || (key.first().is_some_and(|first| first == "workspace")
                && key.get(1).is_some_and(|second| second == "lints"))
    };
    let lints = |key: &[String]| under(key) || key == ["workspace"];
    let mut table: Vec<String> = Vec::new();
    let mut entries = Vec::new();
    let mut open: Option<(Vec<String>, String, i32)> = None;
    for line in text.lines() {
        let (code, delta) = toml_code(line)?;
        if let Some((key, mut value, depth)) = open.take() {
            value.push(' ');
            value.push_str(code.trim());
            match depth + delta {
                0 => entries.push((key, value)),
                depth if depth > 0 => open = Some((key, value, depth)),
                _ => return None,
            }
            continue;
        }
        let code = code.trim();
        if code.is_empty() {
            continue;
        }
        if code.starts_with('[') {
            let inner = code
                .strip_prefix("[[")
                .and_then(|inner| inner.strip_suffix("]]"))
                .or_else(|| code.strip_prefix('[')?.strip_suffix(']'))?;
            table = key_parts(inner)?;
            if under(&table) {
                entries.push((table.clone(), String::new()));
            }
            continue;
        }
        // The first `=` outside a string ends the key.
        let at = code
            .char_indices()
            .find(|(at, c)| *c == '=' && toml_code(&code[..*at]).is_some())?
            .0;
        let mut key = table.clone();
        key.extend(key_parts(&code[..at])?);
        let value = code[at + 1..].trim().to_owned();
        match delta {
            0 => entries.push((key, value)),
            depth if depth > 0 => open = Some((key, value, depth)),
            _ => return None,
        }
    }
    if open.is_some() {
        return None;
    }
    let mut kept: Vec<(String, String)> = entries
        .into_iter()
        .filter(|(key, _)| lints(key))
        .map(|(key, value)| {
            let value = value.split_whitespace().collect::<Vec<_>>().join(" ");
            (key.join("."), value)
        })
        .collect();
    kept.sort();
    Some(kept)
}

/// The lint entries ([`lint_entries`]) that differ between the manifests `before` and `after`,
/// by key path; the error names a side the scanner cannot read with certainty.
fn lint_changes(before: &str, after: &str) -> std::result::Result<Vec<String>, &'static str> {
    let before = lint_entries(before).ok_or("before")?;
    let after = lint_entries(after).ok_or("after")?;
    let mut keys: Vec<String> = before
        .iter()
        .filter(|entry| !after.contains(entry))
        .chain(after.iter().filter(|entry| !before.contains(entry)))
        .map(|(key, _)| key.clone())
        .collect();
    keys.sort();
    keys.dedup();
    Ok(keys)
}

/// What git stores at `path` in the commit `at`: its mode and object, or `None` when the commit
/// has no such path. The path is looked up literally (`--literal-pathspecs`): a path a gate's
/// command names may start with `:`, which git reads as pathspec magic
/// (story:a-gate-script-with-a-colon-is-run).
fn object_at(tree: &Path, at: &str, path: &str) -> Result<Option<String>> {
    let listed = crate::git::run(
        tree,
        &[
            "--literal-pathspecs",
            "ls-tree",
            "--full-tree",
            at,
            "--",
            path,
        ],
    )?;
    Ok(listed
        .split('\t')
        .next()
        .filter(|entry| !entry.is_empty())
        .map(str::to_owned))
}

/// The text of `path`, relative to the tree, at the commit `at`; `None` when it has no such
/// file.
pub(crate) fn file_at(tree: &Path, at: &str, path: &Path) -> Result<Option<String>> {
    let relative = path
        .to_str()
        .filter(|path| path.split('/').all(|part| part != ".."))
        .filter(|_| path.is_relative())
        .ok_or_else(|| anyhow!("{} is not a path inside the tree", path.display()))?;
    let spec = format!("{at}:./{relative}");
    if crate::git::run(tree, &["cat-file", "-e", &spec]).is_err() {
        return Ok(None);
    }
    crate::git::run(tree, &["show", &spec]).map(|text| Some(format!("{text}\n")))
}

impl GateRun {
    /// The action it performs.
    pub const ACTION: &str = "gate.run";
    /// The evidence kind it writes.
    pub const KIND: &str = "gate_result";

    /// Over the gates document at `gates`, relative to the tree.
    #[must_use]
    pub fn new(gates: impl Into<PathBuf>) -> Self {
        Self {
            gates: gates.into(),
            project: None,
            failed: RefCell::new(Vec::new()),
            obligations: RefCell::new(None),
        }
    }

    /// With the project file at `project`, relative to the tree.
    #[must_use]
    pub fn with_project(mut self, project: impl Into<PathBuf>) -> Self {
        self.project = Some(project.into());
        self
    }
}

impl Validator for GateRun {
    fn name(&self) -> &str {
        "takt.gate-run"
    }

    fn action(&self) -> &str {
        Self::ACTION
    }

    fn kind(&self) -> &str {
        Self::KIND
    }

    fn subject(&self) -> &str {
        "implementation"
    }

    fn observe(&self, tree: &Path, revision: &str, seq: usize) -> Result<Observation> {
        self.observe_by(tree, revision, seq, None)
    }

    fn observe_within(
        &self,
        tree: &Path,
        revision: &str,
        seq: usize,
        deadline: Option<std::time::Instant>,
    ) -> Result<(Observation, bool)> {
        let observed = self.observe_by(tree, revision, seq, deadline);
        Ok((ended_by(deadline, self.action(), observed)?, false))
    }

    fn take_failed(&self) -> Vec<FailedCheck> {
        self.failed.take()
    }

    fn take_obligations(&self) -> Option<Vec<crate::obligations::Obligation>> {
        self.obligations.take()
    }

    fn gates_document(&self) -> Option<&Path> {
        Some(&self.gates)
    }

    /// The gates document and the project file the case pinned at open, prepared as an
    /// observation prepares them ([`GateRun::prepare`]) and no command run: a file the gate would
    /// refuse (a `timeout_seconds` outside `1..300`, an unknown format, a program found nowhere)
    /// is the error, naming the file and the reason. A case that pins no base, or whose base
    /// holds no gates document, is left to the observation, as before.
    fn check_at_open(&self, tree: &Path, case_dir: &Path) -> Result<()> {
        let Some((base, at)) = base_revision(tree, case_dir)? else {
            return Ok(());
        };
        if file_at(tree, &at, &self.gates)?.is_none() {
            return Ok(());
        }
        let inputs = self.inputs_at(tree, &at, base)?;
        let change = change_of(tree, case_dir, &at, &at)?;
        let dir = case_dir.join(GATE_INPUTS);
        clear_inputs(&dir)?;
        let home = FreshHome::new()?;
        let prepared = self.prepare(tree, case_dir, &home, &inputs, change);
        let cleared = clear_inputs(&dir);
        prepared.map(drop).context(
            "the case is refused at open: the gate would refuse what it reads from the base revision",
        )?;
        cleared
    }
}

/// What [`GateRun`] reads from the case's base revision: the revision shown (`base`), the gates
/// document as an error names it (`document`) and its text, and the project file's path and text
/// when there is one.
struct BaseInputs {
    base: String,
    document: String,
    gates: String,
    project: Option<(PathBuf, String)>,
}

/// Removes `inputs`, the case's [`GATE_INPUTS`], when it is there.
fn clear_inputs(inputs: &Path) -> Result<()> {
    match std::fs::remove_dir_all(inputs) {
        Err(error) if error.kind() != std::io::ErrorKind::NotFound => {
            Err(error).with_context(|| format!("remove {}", inputs.display()))
        }
        _ => Ok(()),
    }
}

/// The admitted command bindings a failed assertion of a gate names: each request of the plan
/// that runs one (`process.succeeded`, `process.run`, `ess.run`) whose `binding` the assertion's
/// expression names as a quoted string, with the outcome observed for it.
fn bound_requests<'b>(
    bundle: &'b canon_engineering_assertions::Bundle,
    expression: &str,
) -> Vec<(&'b str, &'b str, Option<&'b canon_expr::Outcome>)> {
    bundle
        .plan
        .requests
        .iter()
        .filter(|request| {
            matches!(
                request.operation.as_str(),
                "process.succeeded" | "process.run" | "ess.run"
            )
        })
        .filter_map(|request| match request.arguments.get("binding") {
            Some(canon_expr::Value::String(binding))
                if expression.contains(&format!("\"{binding}\"")) =>
            {
                let outcome = bundle
                    .observations
                    .iter()
                    .find(|observation| observation.request_id == request.id)
                    .map(|observation| &observation.outcome);
                Some((request.operation.as_str(), binding.as_str(), outcome))
            }
            _ => None,
        })
        .collect()
}

/// What a failed `process.succeeded` command keeps when, run again once, it exits 0: the
/// failure did not repeat (a flaky command), so what failed is not known.
pub const PASSED_AGAIN: &str =
    "the command passed when run again (flaky): what failed the first time is not known";

/// An admitted command as the project file binds it, after the library resolved its executable.
struct Bound {
    executable: PathBuf,
    args: Vec<String>,
    env: std::collections::BTreeMap<String, String>,
}

/// What `binding`'s command printed, standard output then standard error: from the observation
/// when it kept them (`process.run`, `ess.run`), or the reason it was not observed; for
/// `process.succeeded`, which keeps only whether it exited 0, from running it again once, only
/// on its failure, as the library ran it (no inherited environment, the binding's own, in
/// `tree`), held to `limits`; when that run exits 0 its output names nothing that failed, so
/// the one line [`PASSED_AGAIN`] stands for it. None for an outcome that failed nothing: a
/// `process.succeeded` that is not false.
fn printed_by(
    operation: &str,
    binding: &str,
    outcome: Option<&canon_expr::Outcome>,
    bound: &std::collections::BTreeMap<String, Bound>,
    tree: &Path,
    limits: Limits,
) -> Option<String> {
    fn field(fields: &std::collections::BTreeMap<String, canon_expr::Value>, name: &str) -> String {
        match fields.get(name) {
            Some(canon_expr::Value::String(text)) => text.clone(),
            _ => String::new(),
        }
    }
    match (operation, outcome) {
        ("process.succeeded", Some(canon_expr::Outcome::Known(canon_expr::Value::Bool(false)))) => {
        }
        ("process.succeeded", _) => return None,
        (_, Some(canon_expr::Outcome::Known(canon_expr::Value::Record(fields)))) => {
            return Some(field(fields, "stdout") + &field(fields, "stderr"));
        }
        (_, Some(canon_expr::Outcome::Unavailable(why))) => return Some(why.clone()),
        _ => return None,
    }
    let command = bound.get(binding)?;
    if limits.timeout.is_zero() {
        return None;
    }
    let mut run = crate::confinement::Command::new(&command.executable);
    run.args(&command.args)
        .env_clear()
        .envs(&command.env)
        .current_dir(tree);
    Some(match crate::forge::run_child(&mut run, limits) {
        Ok(output) if output.code == Some(0) => PASSED_AGAIN.to_owned(),
        Ok(output) => {
            String::from_utf8_lossy(&output.stdout).into_owned()
                + &String::from_utf8_lossy(&output.stderr)
        }
        Err(error) => format!("(what it printed could not be read again: {error:#})"),
    })
}

impl GateRun {
    /// The gates document and the project file at the base revision `at`, shown as `base`: the
    /// gates document, which must be there, and the project file given, which must be there too,
    /// or else [`DEFAULT_PROJECT`] when the base holds it.
    fn inputs_at(&self, tree: &Path, at: &str, base: String) -> Result<BaseInputs> {
        let document = format!("{} from {base}", self.gates.display());
        let gates = file_at(tree, at, &self.gates)?
            .ok_or_else(|| anyhow!("the gates document {document} is not in the base revision"))?;
        let project = match &self.project {
            Some(path) => Some((
                path.clone(),
                file_at(tree, at, path)?.ok_or_else(|| {
                    anyhow!(
                        "the project file {} is not in the base revision, {base}",
                        path.display()
                    )
                })?,
            )),
            None => file_at(tree, at, Path::new(DEFAULT_PROJECT))?
                .map(|text| (PathBuf::from(DEFAULT_PROJECT), text)),
        };
        Ok(BaseInputs {
            base,
            document,
            gates,
            project,
        })
    }

    /// The base revision's two files (`inputs`) prepared as the engineering assertions library
    /// prepares them for the case at `dir`, about `change`, the project file's commands admitted
    /// in `home` ([`FreshHome::admitting`]): nothing is run. The case directory, which the loop
    /// writes every pass, stays out of the bundle's source identity: that identity is the
    /// repository tree's. The two files are copied under the case's [`GATE_INPUTS`] and
    /// prepared from there, the paths relative to the tree as the library takes them; the
    /// caller removes them again ([`clear_inputs`]).
    fn prepare(
        &self,
        tree: &Path,
        dir: &Path,
        home: &FreshHome,
        inputs: &BaseInputs,
        change: canon_engineering_assertions::Change,
    ) -> Result<canon_engineering_assertions::Prepared> {
        let BaseInputs {
            base,
            document,
            gates,
            project,
        } = inputs;
        // The external preparation takes one excluded path: the cases root the case is in.
        let cases = dir
            .parent()
            .and_then(|root| root.strip_prefix(tree).ok())
            .unwrap_or(Path::new(crate::case::CASES_DIR));
        let copies = dir.join(GATE_INPUTS);
        let relative = copies
            .strip_prefix(tree)
            .with_context(|| format!("{} is not under the tree", copies.display()))?
            .to_path_buf();
        let copy = |kind: &str, path: &Path, text: &str| -> Result<PathBuf> {
            let name = path
                .file_name()
                .ok_or_else(|| anyhow!("{} names no file", path.display()))?;
            let at = copies.join(kind);
            std::fs::create_dir_all(&at).with_context(|| format!("create {}", at.display()))?;
            std::fs::write(at.join(name), text)
                .with_context(|| format!("write {}", at.join(name).display()))?;
            Ok(relative.join(kind).join(name))
        };
        let gates = copy("gates", &self.gates, gates)?;
        let copied = project
            .as_ref()
            .map(|(path, text)| {
                let admitted = home.admitting(text, tree).with_context(|| {
                    format!(
                        "admit the commands of the project file {} from {base}",
                        path.display()
                    )
                })?;
                copy("project", path, &admitted)
            })
            .transpose()?;
        canon_engineering_assertions::prepare_with(
            tree,
            &gates,
            copied.as_deref(),
            Some(cases),
            &canon_engineering_assertions::Admission {
                change,
                ..Default::default()
            },
        )
        .with_context(|| match project {
            Some((project, _)) => format!(
                "prepare the gates document {document} with the project file {} from {base}",
                project.display()
            ),
            None => format!("prepare the gates document {document}"),
        })
    }

    /// Observes, each command the gate runs held to the smaller of the project file's timeout
    /// and what is left before `deadline` when the gate's commands are collected.
    fn observe_by(
        &self,
        tree: &Path,
        revision: &str,
        seq: usize,
        deadline: Option<std::time::Instant>,
    ) -> Result<Observation> {
        self.failed.take();
        self.obligations.take();
        let dir = current(tree)?;
        let unpinned = || {
            Ok(nothing_recorded(format!(
                "the gates document {}: not run; the case pins no base commit ({BRANCH_FILE} has no base_commit; it was opened before takt recorded one), and the gate is read from nothing else; no record, the claim stays unknown; abandon the case and open it again",
                self.gates.display()
            )))
        };
        let Some((base, at)) = base_revision(tree, &dir)? else {
            return unpinned();
        };
        let inputs = self.inputs_at(tree, &at, base)?;
        let document = inputs.document.clone();
        let project = inputs.project.clone();
        // What the gate's commands run or read from the tree: a file the case's branch changed
        // would let a model's edit decide the gate the base revision defines. Compared at the
        // base as the test command's surface is ([`judged`]), with what the project file admits
        // (its executables and catalogs) and every file its commands take as an argument added
        // as globs, and the tool configuration by name ([`is_tool_config`]) and every manifest's
        // lint tables ([`lint_changes`]) besides. A directory the base holds at an argument is
        // the data a command checks, not code it runs, and is not compared; a name the glob
        // dialect cannot hold as written is compared as a path by itself. Nothing is run and no
        // record written while one differs, is added or is deleted. Without a project file the
        // gate admits no command, and runs nothing from the tree.
        if let Some((path, text)) = &project {
            let read = || format!("read the project file {}", path.display());
            let mut globs: Vec<String> = Vec::new();
            let mut changed = Vec::new();
            let mut files = admitted_files(text).with_context(read)?;
            for argument in argument_paths(text).with_context(read)? {
                if !object_at(tree, &at, &argument)?.is_some_and(|o| o.starts_with("040000 ")) {
                    files.push(argument);
                }
            }
            // What Python loads from the tree for the gate's commands ([`python_paths`]).
            let (python, python_globs, closed) =
                python_paths(tree, &at, revision, text).with_context(read)?;
            files.extend(python);
            globs.extend(python_globs);
            // A line the reader cannot read with certainty fails closed, as in the step guard:
            // every Python file and shell file by its glob, and every program, a file the base
            // or the revision holds with an exec bit or a `#!` line.
            if !closed.is_empty() {
                globs.extend(
                    crate::run::FAIL_CLOSED
                        .iter()
                        .filter(|glob| !glob.contains('('))
                        .map(|glob| (*glob).to_owned()),
                );
                let diff = crate::git::bytes(
                    tree,
                    &[
                        "diff-tree",
                        "-r",
                        "-z",
                        "--no-renames",
                        "--name-only",
                        &at,
                        revision,
                        "--",
                    ],
                )?;
                for path in nul_fields(&diff, "diff-tree", false)? {
                    if program_at(tree, &at, path)? || program_at(tree, revision, path)? {
                        changed.push(path.to_owned());
                    }
                }
            }
            for file in files {
                if file.contains(['*', '?', '\\']) || file.trim() != file {
                    if object_at(tree, &at, &file)? != object_at(tree, revision, &file)? {
                        changed.push(file);
                    }
                } else {
                    globs.push(file);
                }
            }
            let globs = crate::run::parse_judge_list(&serde_yaml_ng::to_string(&globs)?)
                .map_err(|why| anyhow!("the paths the gate compares at the base: {why}"))?;
            match judged(tree, &dir, revision, &[], globs, true)? {
                Judged::Unchanged => {}
                Judged::NoBase => return unpinned(),
                Judged::Changed { paths, .. } => changed.extend(paths),
            }
            if !changed.is_empty() {
                let mut seen = std::collections::BTreeSet::new();
                changed.retain(|path| seen.insert(path.clone()));
                let why = if closed.is_empty() {
                    String::new()
                } else {
                    format!(
                        "; its commands fail closed, so every Python file, shell file and program of the tree is compared: {}",
                        closed.join("; ")
                    )
                };
                return Ok(nothing_recorded(format!(
                    "the gates document {document}: not run; what its commands run or read from the tree differs from the base revision at {}: {}{why}; no record, the claim stays unknown",
                    short(revision),
                    changed.join(", ")
                )));
            }
        }
        let now = time::OffsetDateTime::now_utc().unix_timestamp();
        let change = change_of(tree, &dir, &at, revision)?;
        let gate_inputs = dir.join(GATE_INPUTS);
        let clear = || clear_inputs(&gate_inputs);
        clear()?;
        // The gate's commands run in a home of their own, as the test command does.
        let home = FreshHome::new()?;
        let collected = (|| -> Result<_> {
            let mut prepared = self.prepare(tree, &dir, &home, &inputs, change.clone())?;
            if let Some(deadline) = deadline {
                prepared.context.timeout = prepared
                    .context
                    .timeout
                    .min(deadline.saturating_duration_since(std::time::Instant::now()));
            }
            // The library starts the commands and providers the project file binds itself,
            // outside [`crate::confinement::Command`]: each is asked of the adapter in use
            // first, and one refused starts none of them.
            let confinement = crate::confinement::current();
            for executable in prepared
                .context
                .commands
                .values()
                .map(|binding| &binding.executable)
                .chain(
                    prepared
                        .context
                        .providers
                        .values()
                        .map(|provider| &provider.command.executable),
                )
            {
                confinement
                    .admit_delegated(executable.as_os_str())
                    .with_context(|| format!("start the commands of {document}"))?;
            }
            // What a failed gate's command printed is read again from these
            // ([`printed_by`]), held as the library held them.
            let bound: std::collections::BTreeMap<String, Bound> = prepared
                .context
                .commands
                .iter()
                .map(|(name, binding)| {
                    let bound = Bound {
                        executable: binding.executable.clone(),
                        args: binding.args.clone(),
                        env: binding.env.clone(),
                    };
                    (name.clone(), bound)
                })
                .collect();
            let limits = Limits {
                timeout: prepared.context.timeout,
                output: prepared.context.max_output_bytes,
            };
            // The provider starts the gate's commands in process groups of their own and waits
            // on them itself: no sweep on another thread may take one meanwhile.
            let held = crate::forge::hold_sweeps();
            let collected = canon_engineering_assertions::collect(prepared, now)
                .with_context(|| format!("collect the observations of {document}"));
            drop(held);
            collected.map(|bundle| (bundle, bound, limits))
        })();
        let cleared = clear();
        let (bundle, bound, limits) = collected?;
        cleared?;
        let report = canon_engineering_assertions::evaluate(&bundle, now, None, None)
            .with_context(|| format!("evaluate {document}"))?;
        // Each gate with a false assertion keeps what the commands that assertion names printed
        // that names what failed ([`failing_lines`]), within [`FAILED_LINES_TOTAL`] over all of
        // them; the run keeps it on the record's evidence line and shows it to the next step.
        let mut failed = Vec::new();
        if report.truth == canon_expr::Truth::False {
            let mut left = FAILED_LINES_TOTAL;
            let mut evaluations = report.assertions.iter();
            for gate in &bundle.document.gates {
                let mut printed = String::new();
                let mut seen = std::collections::BTreeSet::new();
                for evaluation in evaluations.by_ref().take(gate.assertions.len()) {
                    if evaluation.truth != canon_expr::Truth::False {
                        continue;
                    }
                    for (operation, binding, outcome) in
                        bound_requests(&bundle, &evaluation.expression)
                    {
                        if !seen.insert(binding) {
                            continue;
                        }
                        let mut limits = limits;
                        if let Some(deadline) = deadline {
                            limits.timeout = limits
                                .timeout
                                .min(deadline.saturating_duration_since(std::time::Instant::now()));
                        }
                        if let Some(text) =
                            printed_by(operation, binding, outcome, &bound, tree, limits)
                        {
                            printed.push_str(&text);
                            printed.push('\n');
                        }
                    }
                }
                if printed.trim().is_empty() {
                    continue;
                }
                let (lines, omitted) = failing_lines(&printed, FAILED_LINES_PER_CHECK.min(left));
                left -= lines.iter().map(|line| line.len() + 1).sum::<usize>();
                failed.push(FailedCheck {
                    id: gate.id.clone(),
                    lines,
                    omitted,
                });
            }
        }
        drop(home);
        *self.failed.borrow_mut() = failed;
        let results = canon_engineering_assertions::evaluate_gates(&bundle, now, None, None)
            .with_context(|| format!("evaluate the gates of {document}"))?;
        let mut obligations = Vec::new();
        for (gate, outcome) in bundle.document.gates.iter().zip(&results.gates) {
            let result = serde_json::to_value(outcome.result)?;
            obligations.push(crate::obligations::Obligation {
                id: gate.id.clone(),
                text: gate.text.split_whitespace().collect::<Vec<_>>().join(" "),
                status: crate::obligations::status_of(result.as_str().unwrap_or_default())?,
            });
        }
        *self.obligations.borrow_mut() = Some(obligations);
        // The report holds the gates' assertions in declaration order; the gate that decided
        // is the first whose assertion shares the report's truth.
        let decided = bundle
            .document
            .gates
            .iter()
            .flat_map(|gate| std::iter::repeat_n(gate.id.as_str(), gate.assertions.len()))
            .zip(&report.assertions)
            .find(|(_, evaluation)| evaluation.truth == report.truth)
            .map(|(gate, evaluation)| {
                (
                    gate,
                    evaluation.expression.as_str(),
                    evaluation.reasons.join("; "),
                )
            });
        let decided = |truth: &str| match &decided {
            Some((gate, expression, reasons)) if reasons.is_empty() => {
                format!("gate {gate} {truth}: {expression}")
            }
            Some((gate, expression, reasons)) => format!(
                "gate {gate} {truth}: {expression} ({})",
                reasons.chars().take(300).collect::<String>()
            ),
            None => format!("no assertion is {truth}"),
        };
        let artifact = format!("gate-result-{seq}.bundle.json");
        let (result, summary) = match report.truth {
            canon_expr::Truth::True => {
                let gates: Vec<&str> = bundle
                    .document
                    .gates
                    .iter()
                    .map(|gate| gate.id.as_str())
                    .collect();
                (
                    Some("pass"),
                    format!(
                        "the gates document {document}: pass; every gate true: {}; retained as {artifact}",
                        gates.join(", ")
                    ),
                )
            }
            canon_expr::Truth::False => (
                Some("fail"),
                format!(
                    "the gates document {document}: fail; {}; retained as {artifact}",
                    decided("false")
                ),
            ),
            canon_expr::Truth::Unknown => (
                None,
                format!(
                    "the gates document {document}: unknown; {}; no record, the claim stays unknown; retained as {artifact}",
                    decided("unknown")
                ),
            ),
        };
        let records = result
            .map(|result| {
                vec![Record::new(
                    format!("gate-result-{seq}"),
                    self.kind(),
                    Some(result.to_owned()),
                    self.subject(),
                    revision,
                    self.name(),
                )]
            })
            .unwrap_or_default();
        let bytes = serde_json::to_vec_pretty(&bundle).context("serialize the evaluated bundle")?;
        Ok(Observation {
            summary,
            records,
            artifact: Some(Artifact {
                name: artifact,
                bytes,
            }),
        })
    }
}

/// The first eight characters of a revision, as the loop prints one.
fn short(revision: &str) -> &str {
    revision.get(..8).unwrap_or(revision)
}

/// Whether the pull request head `head` carries the implementation revision `revision` of the
/// case at `case_dir`: it is the revision, or it reaches the revision only through takt's
/// riders for the story ([`crate::case::approvals_only`]: the approval's commits, the
/// `test_result` and the move to `implemented`, each file byte-equal to the record the case
/// keeps), which are how those records reach the base with the pull request. A head that is
/// not a commit of the repository carries nothing.
fn carries(tree: &Path, case_dir: &Path, revision: &str, head: &str) -> Result<bool> {
    if head == revision {
        return Ok(true);
    }
    if head.len() < 7
        || !head.bytes().all(|byte| byte.is_ascii_hexdigit())
        || crate::git::run(tree, &["cat-file", "-e", &format!("{head}^{{commit}}")]).is_err()
    {
        return Ok(false);
    }
    crate::case::approvals_only(tree, revision, head, case_dir)
}

/// How a summary names a head that carries takt's riders above the revision; nothing
/// for a head that is the revision.
fn above(revision: &str, head: &str) -> String {
    if head == revision {
        String::new()
    } else {
        format!("; its head {} carries takt's records above it", short(head))
    }
}

/// `pr.open`: pushes the case's branch through the forge and opens its one pull request, the
/// head at the implementation revision; writes one `pull_request` record with the state the
/// forge reports, `open` for a pull request just opened. At a later revision the branch is
/// pushed again and the pull request's state read: one pull request per case. The tree's
/// `HEAD` must be the revision, or the revision with only takt's riders for the story above it
/// ([`crate::case::approvals_only`]), which are pushed with it so the store's records reach the
/// base with the pull request; anything else would not be what the claim is about, and nothing
/// is pushed. A number the case directory carries is trusted only once the forge reports the pull
/// request's head as the case's branch at the head pushed, else no record is written. A case
/// with no base is refused before anything is pushed. Before a pull request is opened, a case
/// whose base branch (`refs/heads/<base>`, never a tag of its name) is not there, or whose
/// branch has no commit beyond it but the take, pushes nothing and writes no record, the
/// summary saying why. The title is the intent's first line, the body the intent and the claims
/// that hold. Before every push the forge scans what it would publish
/// ([`Forge::scan_outgoing`]): every commit of the head pushed the remote does not hold, with
/// its author, committer, message and diff, and, when the pull request is opened, its title and
/// body; a refused scan pushes nothing, opens nothing and writes no record.
pub struct PrOpen {
    /// The forge the push and the pull request go through.
    pub forge: Rc<dyn Forge>,
}

impl PrOpen {
    /// The action it performs.
    pub const ACTION: &str = "pr.open";
    /// The evidence kind it writes.
    pub const KIND: &str = "pull_request";

    /// Over `forge`.
    #[must_use]
    pub fn new(forge: Rc<dyn Forge>) -> Self {
        Self { forge }
    }
}

impl std::fmt::Debug for PrOpen {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PrOpen")
            .field("forge", &self.forge.name())
            .finish()
    }
}

/// The pull request's title: the intent's first line.
fn title_of(intent: &str) -> String {
    intent
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .unwrap_or("takt")
        .chars()
        .take(120)
        .collect()
}

/// The claims the case's latest decision holds, in id order; none before a decision.
fn claims_held(case_dir: &Path) -> Result<Vec<String>> {
    Ok(crate::case::latest_decision(case_dir)?
        .map(|decision| {
            decision
                .claims
                .iter()
                .filter(|(_, value)| value.as_str() == "true")
                .map(|(id, _)| id.clone())
                .collect()
        })
        .unwrap_or_default())
}

/// The figure a body states as this when the case's record holds none of it.
const NOT_STATED: &str = "not stated";
/// The most bytes a pull request body takes; the forge's limit is 65,536 characters.
const BODY_LIMIT: usize = 60_000;
/// The most bytes of the intent a body carries; a longer one is cut, and the body says so.
const INTENT_LIMIT: usize = 20_000;
/// The most bytes of the evidence list; the records past it are left out, and the body counts them.
const EVIDENCE_LIMIT: usize = 30_000;
/// The bytes a body keeps back from its list for the heading and the count of records left out.
const EVIDENCE_RESERVE: usize = 400;

/// The longest prefix of `text` of at most `limit` bytes, cut on a character boundary.
fn prefix_within(text: &str, limit: usize) -> &str {
    let mut end = limit.min(text.len());
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    &text[..end]
}

/// The evidence records under the case's `evidence/`, each as JSON, in file-name order. A file
/// that is not JSON reads as `null`, so its figures are [`NOT_STATED`].
fn evidence_of(case_dir: &Path) -> Result<Vec<serde_json::Value>> {
    let dir = case_dir.join("evidence");
    let entries = match std::fs::read_dir(&dir) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(error).with_context(|| format!("read {}", dir.display())),
    };
    let mut paths = Vec::new();
    for entry in entries {
        let path = entry
            .with_context(|| format!("read {}", dir.display()))?
            .path();
        if path
            .extension()
            .is_some_and(|extension| extension == "json")
        {
            paths.push(path);
        }
    }
    paths.sort();
    Ok(paths
        .iter()
        .map(|path| {
            std::fs::read(path)
                .ok()
                .and_then(|bytes| serde_json::from_slice(&bytes).ok())
                .unwrap_or(serde_json::Value::Null)
        })
        .collect())
}

/// The steps and the tokens `events.jsonl` in the case directory states so far: the step lines
/// counted, and the `tokens` they state summed. Each is [`NOT_STATED`] when the file is missing,
/// and the tokens when no step line states any. When only some step lines state tokens, the sum
/// says how many of the steps stated them.
fn run_figures_of(case_dir: &Path) -> (String, String) {
    let Ok(text) = std::fs::read_to_string(case_dir.join(crate::events::EVENTS_FILE)) else {
        return (NOT_STATED.to_owned(), NOT_STATED.to_owned());
    };
    let mut steps = 0u64;
    let mut stated_steps = 0u64;
    let mut tokens: Option<u64> = None;
    for line in text.lines() {
        let Ok(event) = serde_json::from_str::<serde_json::Value>(line) else {
            continue;
        };
        if event["event"] != "step" {
            continue;
        }
        steps += 1;
        if let Some(stated) = event["tokens"].as_u64() {
            stated_steps += 1;
            tokens = Some(tokens.unwrap_or(0).saturating_add(stated));
        }
    }
    let tokens = match tokens {
        None => NOT_STATED.to_owned(),
        Some(sum) if stated_steps == steps => sum.to_string(),
        Some(sum) => format!("{sum} (stated by {stated_steps} of {steps} steps)"),
    };
    (steps.to_string(), tokens)
}

/// One evidence record as a body line: its kind, result, short revision and producer, each
/// [`NOT_STATED`] when the record does not hold it.
fn evidence_line(record: &serde_json::Value) -> String {
    let figure = |name: &str| {
        record[name]
            .as_str()
            .map_or_else(|| NOT_STATED.to_owned(), str::to_owned)
    };
    let revision = record["subject_revision"].as_str().map_or_else(
        || NOT_STATED.to_owned(),
        |revision| short(revision).to_owned(),
    );
    format!(
        "- `{}`, result `{}`, revision `{revision}`, producer `{}`.\n",
        figure("kind"),
        figure("result"),
        figure("producer")
    )
}

/// The pull request's body: the intent, then the case, the revision and the claims that hold, the
/// steps and tokens `events.jsonl` states, and the evidence records of the case. The body stays
/// under [`BODY_LIMIT`] bytes: a longer intent is cut, and the records that do not fit are left
/// out, the body saying how many.
fn body_of(case_dir: &Path, intent: &str, revision: &str) -> Result<String> {
    let case = case_dir
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default();
    let claims = claims_held(case_dir)?;
    let claims = if claims.is_empty() {
        "none yet".to_owned()
    } else {
        claims.join(", ")
    };
    let intent = intent.trim();
    let kept = prefix_within(intent, INTENT_LIMIT);
    let cut = if kept.len() < intent.len() {
        format!(
            "\n\n(The intent is cut at {INTENT_LIMIT} bytes; the rest is left out of this body.)"
        )
    } else {
        String::new()
    };
    let (steps, tokens) = run_figures_of(case_dir);
    let head = format!(
        "{kept}{cut}\n\n---\n\nOpened by takt for the case `{case}` at `{revision}`. Claims that hold: {claims}.\n\nSteps: {steps}. Tokens: {tokens}.\n"
    );
    let records = evidence_of(case_dir)?;
    let budget = BODY_LIMIT
        .saturating_sub(head.len())
        .saturating_sub(EVIDENCE_RESERVE)
        .min(EVIDENCE_LIMIT);
    let lines: Vec<String> = records.iter().map(evidence_line).collect();
    let keep = kept_within(&records, &lines, revision, budget);
    let listed: String = lines
        .iter()
        .zip(&keep)
        .filter(|(_, kept)| **kept)
        .map(|(line, _)| line.as_str())
        .collect();
    let count = keep.iter().filter(|kept| **kept).count();
    let mut body = head;
    body.push_str(&format!(
        "\nEvidence: {} records in the case.\n\n{listed}",
        records.len()
    ));
    if count < records.len() {
        body.push_str(&format!(
            "\n{} of {} evidence records are left out of this body, the list being bounded at {EVIDENCE_LIMIT} bytes.\n",
            records.len() - count,
            records.len()
        ));
    }
    Ok(body)
}

/// Which of `records` the body lists within `budget` bytes, marked by index in `keep`, `lines`
/// their body lines. Every passing `test_result` at `revision`, the record the held claims rest
/// on, is kept first, the newest while they fit; then the newest of the others while they fit.
/// What is kept reads oldest to newest in the body, so the newest records that fit are the ones
/// that are listed.
fn kept_within(
    records: &[serde_json::Value],
    lines: &[String],
    revision: &str,
    budget: usize,
) -> Vec<bool> {
    let rests_on_claims = |record: &serde_json::Value| {
        record["kind"] == "test_result"
            && record["result"] == "pass"
            && record["subject_revision"] == revision
    };
    let mut keep = vec![false; records.len()];
    let mut used = 0;
    for pinned in [true, false] {
        for index in (0..records.len()).rev() {
            if rests_on_claims(&records[index]) != pinned {
                continue;
            }
            if used + lines[index].len() > budget {
                break;
            }
            used += lines[index].len();
            keep[index] = true;
        }
    }
    keep
}

/// What `pr.open` does after the push: opens the case's pull request, or reads the one it has.
enum Next {
    Open(PullRequest),
    Read(u64),
}

/// The case's branch, from the case directory's name: what the validators push and open for,
/// never the `name` a file under the case directory carries, which a harness can write.
fn branch_of(case_dir: &Path) -> Result<String> {
    let id = case_dir
        .file_name()
        .ok_or_else(|| anyhow!("the case directory {} has no name", case_dir.display()))?;
    Ok(Branch::name_of(&id.to_string_lossy()))
}

/// The pull request a case opens: `name` against `base`, the intent's first line as the
/// title, the intent and the claims that hold as the body. A case with no base is refused.
fn request_of(
    case_dir: &Path,
    name: &str,
    base: Option<&str>,
    revision: &str,
) -> Result<PullRequest> {
    let base = base.ok_or_else(|| {
        anyhow!(
            "the case has no base branch: it was opened from a detached HEAD; open it from the branch its pull request is against"
        )
    })?;
    let intent = case_dir.join("intent.md");
    let intent =
        std::fs::read_to_string(&intent).with_context(|| format!("read {}", intent.display()))?;
    Ok(PullRequest {
        branch: name.to_owned(),
        base: base.to_owned(),
        title: title_of(&intent),
        body: body_of(case_dir, &intent, revision)?,
    })
}

impl Validator for PrOpen {
    fn name(&self) -> &str {
        "takt.pr-open"
    }

    fn action(&self) -> &str {
        Self::ACTION
    }

    fn kind(&self) -> &str {
        Self::KIND
    }

    fn subject(&self) -> &str {
        "implementation"
    }

    /// The case's `branch.yaml`, and what the bot route keeps of the request it sent, the text it
    /// scanned as published, the commit messages and files it scanned before the push and the
    /// response it read.
    fn writes(&self) -> Vec<PathBuf> {
        let artifacts = Path::new("artifacts");
        vec![
            PathBuf::from(BRANCH_FILE),
            artifacts.join(BotRoute::REQUEST),
            artifacts.join(BotRoute::RESPONSE),
            artifacts.join(BotRoute::PUBLISHED),
            artifacts.join(crate::forge::command::CommandForge::BODY),
            artifacts.join(BotRoute::MESSAGES),
            artifacts.join(BotRoute::FILES),
        ]
    }

    fn observe(&self, tree: &Path, revision: &str, seq: usize) -> Result<Observation> {
        self.observe_waiting(tree, revision, seq)
            .map(|(observation, _)| observation)
    }

    /// Waits on the forge when the pull request's head, on the case's branch, is not yet at
    /// the head pushed: the forge moves it after the push, without a commit.
    fn observe_waiting(
        &self,
        tree: &Path,
        revision: &str,
        seq: usize,
    ) -> Result<(Observation, bool)> {
        let dir = current(tree)?;
        // `HEAD` is the implementation revision, or takt's riders on top of it
        // (`approvals_only`); `HEAD` is what is pushed, so the pull request carries the
        // store's records to the base, and the record written is about the revision.
        let head = crate::git::head(tree)?;
        if head != revision && !crate::case::approvals_only(tree, revision, &head, &dir)? {
            bail!(
                "the tree's HEAD {head} is not the implementation revision {revision}; the pull request's head would not be the revision the claim is about"
            );
        }
        let mut branch = Branch::read(&dir)?;
        let forge = self.forge.name();
        // The branch pushed and opened for is the case's, from the case id: the `name` in
        // `branch.yaml` is in a file a harness can write, and a refspec there must never reach
        // the forge. A `name` that differs is set right, and the summary says so.
        let name = branch_of(&dir)?;
        let repaired = if branch.name == name {
            String::new()
        } else {
            let was = std::mem::replace(&mut branch.name, name.clone());
            format!("{BRANCH_FILE} named {was}, not the case's branch; set right; ")
        };
        // What follows the push is settled before it: a case with no pull request yet needs its
        // base and its request, and one without a base is refused with nothing pushed.
        let next = match branch.pull_request {
            Some(number) => Next::Read(number),
            None => Next::Open(request_of(&dir, &name, branch.base.as_deref(), revision)?),
        };
        // A pull request needs a commit on the case's branch beyond its base: at the base itself
        // the forge has nothing to compare, and opening one would be refused there. The take
        // committed when the case opened (`takt: take <story>`) is not such a commit: it moves
        // the story's status, not the work. Nothing is pushed and no record written; a commit
        // moves the revision and makes `pr.open` due again.
        if let Next::Open(request) = &next {
            // The base is the branch, `refs/heads/<base>`, never a tag or a remote ref of its
            // name; with the branch gone there is nothing to count against, and nothing to open
            // a pull request against, as for a case with no commit beyond its base.
            let Some(base_tip) = crate::git::run(
                tree,
                &[
                    "rev-parse",
                    "--verify",
                    "--quiet",
                    "--end-of-options",
                    &format!("refs/heads/{}^{{commit}}", request.base),
                ],
            )
            .ok()
            .filter(|tip| !tip.is_empty()) else {
                return Ok((
                    Observation {
                        summary: format!(
                            "{name} at {}: the base branch {} is not there; nothing pushed, no pull request; the claim stays unknown",
                            short(revision),
                            request.base
                        ),
                        records: Vec::new(),
                        artifact: None,
                    },
                    false,
                ));
            };
            let ahead = crate::git::run(
                tree,
                &[
                    "rev-list",
                    "--count",
                    "--invert-grep",
                    "--extended-regexp",
                    "--grep=^takt: take [^ ]+$",
                    &format!("{base_tip}..{revision}"),
                ],
            )?;
            if ahead.trim() == "0" {
                return Ok((
                    Observation {
                        summary: format!(
                            "{name} at {} has no commit beyond {}; nothing pushed, no pull request; the claim stays unknown",
                            short(revision),
                            request.base
                        ),
                        records: Vec::new(),
                        artifact: None,
                    },
                    false,
                ));
            }
        }
        // Everything the push and the pull request would publish is scanned before either
        // leaves: the commits the remote does not hold, and the title and body of a pull request
        // about to be opened. A refusal pushes nothing and records nothing.
        let (base, request) = match &next {
            Next::Open(request) => (request.base.clone(), Some(request)),
            Next::Read(number) => (
                branch.base.clone().ok_or_else(|| {
                    anyhow!(
                        "the case has pull request #{number} but no base branch to count the commits it would push from; nothing pushed"
                    )
                })?,
                None,
            ),
        };
        self.forge
            .scan_outgoing(
                tree,
                &dir.join("artifacts"),
                &crate::forge::Outgoing {
                    branch: &name,
                    base: &base,
                    head: &head,
                    request,
                },
            )
            .with_context(|| format!("scan {name} before the push through the forge {forge}"))?;
        self.forge
            .push_revision(tree, &name, &head)
            .with_context(|| format!("push {name} through the forge {forge}"))?;
        branch.pushed = Some(head.clone());
        let carried = above(revision, &head);
        let mut waits = false;
        let (state, summary) = match next {
            Next::Read(number) => {
                let view = self.forge.pull_request(tree, number).with_context(|| {
                    format!("read pull request #{number} through the forge {forge}")
                })?;
                if view.head.branch == name && view.head.sha == head {
                    (
                        Some(view.state),
                        format!(
                            "{repaired}pushed {name} at {} through {forge}{carried}; pull request #{number} is {}",
                            short(revision),
                            view.state
                        ),
                    )
                } else {
                    // The case's branch at another commit: the forge has not moved the pull
                    // request to the push yet. Another branch is not a lag, and not waited on.
                    waits = view.head.branch == name;
                    (
                        None,
                        format!(
                            "{repaired}pushed {name} at {} through {forge}; pull request #{number} has the head {} at {}, not the case's branch at the revision; no record, the claim stays unknown",
                            short(revision),
                            view.head.branch,
                            short(&view.head.sha)
                        ),
                    )
                }
            }
            Next::Open(request) => {
                let number = self
                    .forge
                    .open_pull_request(tree, &dir.join("artifacts"), &request)
                    .with_context(|| {
                        format!("open the pull request for {name} through the forge {forge}")
                    })?;
                branch.pull_request = Some(number);
                (
                    Some(PullRequestState::Open),
                    format!(
                        "{repaired}pushed {name} at {} through {forge}{carried} and opened pull request #{number}: open",
                        short(revision)
                    ),
                )
            }
        };
        branch.write(&dir)?;
        let records = state
            .map(|state| {
                vec![Record::new(
                    format!("pull-request-{seq}"),
                    self.kind(),
                    Some(state.as_str().to_owned()),
                    self.subject(),
                    revision,
                    self.name(),
                )]
            })
            .unwrap_or_default();
        Ok((
            Observation {
                summary,
                records,
                artifact: None,
            },
            waits,
        ))
    }
}

/// `ci.read`: reads the checks on the head of the case's pull request through the forge and
/// writes one `ci_result` record, `pass` or `fail`. Pending checks write no record, because a
/// claim that could not be observed stays unknown; so does a case with no pull request yet, or
/// one whose pull request the forge does not report as the case's branch at the head pushed,
/// the revision or the revision with only takt's riders above it: a number the
/// case directory carries is not trusted on its own. Nothing but the pull request head is read.
/// Pending checks, and a pull request whose head on the case's branch is not yet at the head
/// pushed, wait on the forge. What the checks run is decided by the run's judge surface (the CI
/// workflows under `.github/`, the cargo configuration, the toolchain, build scripts, the test
/// command's own script; [`crate::run::JudgeSurface`]), which no step changes: nothing is read
/// and no record written while the case's branch changed it since its base, or when the case
/// pins no base to compare with; the summary names the paths.
pub struct CiRead {
    /// The forge the checks are read through.
    pub forge: Rc<dyn Forge>,
    /// The words of the commands the run's validators run from the tree ([`Validator::runs`]):
    /// what names the test command's own script on the judge surface.
    pub runs: Vec<String>,
}

impl CiRead {
    /// The action it performs.
    pub const ACTION: &str = "ci.read";
    /// The evidence kind it writes.
    pub const KIND: &str = "ci_result";

    /// Over `forge`.
    #[must_use]
    pub fn new(forge: Rc<dyn Forge>) -> Self {
        Self {
            forge,
            runs: Vec::new(),
        }
    }

    /// With the run's validators running `runs` from the tree ([`Validator::runs`]).
    #[must_use]
    pub fn with_runs(mut self, runs: Vec<String>) -> Self {
        self.runs = runs;
        self
    }
}

impl std::fmt::Debug for CiRead {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CiRead")
            .field("forge", &self.forge.name())
            .finish()
    }
}

impl Validator for CiRead {
    fn name(&self) -> &str {
        "takt.ci-read"
    }

    fn action(&self) -> &str {
        Self::ACTION
    }

    fn kind(&self) -> &str {
        Self::KIND
    }

    fn subject(&self) -> &str {
        "implementation"
    }

    fn observe(&self, tree: &Path, revision: &str, seq: usize) -> Result<Observation> {
        self.observe_waiting(tree, revision, seq)
            .map(|(observation, _)| observation)
    }

    fn observe_waiting(
        &self,
        tree: &Path,
        revision: &str,
        seq: usize,
    ) -> Result<(Observation, bool)> {
        let dir = current(tree)?;
        let branch = Branch::read(&dir)?;
        // The case's branch, from the case id: not the `name` of a file a harness can write.
        let name = branch_of(&dir)?;
        let nothing = |summary: String, waits: bool| {
            (
                Observation {
                    summary,
                    records: Vec::new(),
                    artifact: None,
                },
                waits,
            )
        };
        match judged(tree, &dir, revision, &self.runs, Vec::new(), false)? {
            Judged::Unchanged => {}
            Judged::NoBase => {
                return Ok(nothing(
                    format!(
                        "the case pins no base commit ({BRANCH_FILE} has no base_commit) to compare its judge surface with; nothing read, the claim stays unknown; abandon the case and open it again"
                    ),
                    false,
                ));
            }
            Judged::Changed { base, paths } => {
                return Ok(nothing(
                    format!(
                        "the case's branch at {} changed the judge surface since {base}: {}; nothing read, the claim stays unknown",
                        short(revision),
                        paths.join(", ")
                    ),
                    false,
                ));
            }
        }
        let Some(number) = branch.pull_request else {
            return Ok(nothing(
                "no pull request is open for the case; nothing to read, the claim stays unknown"
                    .to_owned(),
                false,
            ));
        };
        let pushed = match branch.pushed {
            Some(pushed) if carries(tree, &dir, revision, &pushed)? => pushed,
            pushed => {
                return Ok(nothing(
                    format!(
                        "the head of pull request #{number} is {}, not the implementation revision {}; nothing to read, the claim stays unknown",
                        pushed.as_deref().map_or("not pushed", short),
                        short(revision)
                    ),
                    false,
                ));
            }
        };
        let forge = self.forge.name();
        let view = self
            .forge
            .pull_request(tree, number)
            .with_context(|| format!("read pull request #{number} through the forge {forge}"))?;
        if view.head.branch != name || view.head.sha != pushed {
            return Ok(nothing(
                format!(
                    "pull request #{number} has the head {} at {}, not {name} at {}; nothing to read, the claim stays unknown",
                    view.head.branch,
                    short(&view.head.sha),
                    short(revision)
                ),
                view.head.branch == name,
            ));
        }
        let checks = self.forge.checks(tree, number).with_context(|| {
            format!("read the checks on pull request #{number} through the forge {forge}")
        })?;
        let result = match checks {
            Checks::Pass => Some("pass"),
            Checks::Fail => Some("fail"),
            Checks::Pending => None,
        };
        let records = result
            .map(|result| {
                vec![Record::new(
                    format!("ci-result-{seq}"),
                    self.kind(),
                    Some(result.to_owned()),
                    self.subject(),
                    revision,
                    self.name(),
                )]
            })
            .unwrap_or_default();
        let summary = match result {
            Some(result) => format!(
                "checks on pull request #{number} at {} through {forge}: {result}",
                short(revision)
            ),
            None => format!(
                "checks on pull request #{number} at {} through {forge}: pending; no record, the claim stays unknown",
                short(revision)
            ),
        };
        Ok((
            Observation {
                summary,
                records,
                artifact: None,
            },
            checks == Checks::Pending,
        ))
    }
}

/// `repository.merge`: merges the case's pull request through the forge, the one deterministic
/// step with a write effect, and writes one `merge_observation` record with the state the forge
/// reports afterwards: `merged` once it is, `closed` for a pull request closed unmerged. The
/// loop takes the step only when Canon admits the action, and Canon admits it only under a
/// recorded grant of `repository.merge` read from the case's `authority.yaml`: a connection to
/// the forge is never authority. The grant authorises the implementation revision it was given
/// at and no other, and the step checks that again itself: an open pull request at a revision
/// the case holds no grant for is not merged. Nothing is merged unless the forge reports the
/// pull request's head as the case's branch at the head last pushed, or at the revision, and
/// that head carries the revision: it is the revision, or the revision with only takt's riders
/// for the story above it ([`crate::case::approvals_only`]), the store's records of the grant,
/// the test result and the move to `implemented`, which so reach the base with the merge. Any
/// other head is not merged and not replaced. When the case's branch holds riders above the
/// pull request's head that it descends from and that carry the revision, the branch is pushed
/// first, named in `branch.yaml` before the push, so the head merged carries the records and a
/// push the forge took but reported failed is merged by the next run; the forge scans what that
/// push would publish before it ([`Forge::scan_outgoing`]), and a refused scan pushes nothing,
/// merges nothing and writes no record. A head carrying riders
/// is one CI was not read on: its checks are read before the merge, and pending or failing checks merge nothing and
/// write no record, the summary saying which; a later run merges it. Pending checks, a head
/// pushed that the pull request does not show yet, and a merge the forge refuses for a required
/// check that has not reported ([`crate::forge::refused_for_a_pending_check`]) merge nothing,
/// write no record and wait on the forge. The merge names
/// the head, so a head that moved meanwhile is refused by the forge rather than merged. The
/// record is about the revision the grant authorises; the head merged is retained beside it as
/// `artifacts/merged-head-<seq>.json`. A pull request already merged is recorded without a
/// second merge; one the forge still reports open after the merge writes no record, because a
/// claim that could not be observed stays unknown.
pub struct PrMerged {
    /// The forge the merge goes through and the state is read from.
    pub forge: Rc<dyn Forge>,
}

impl PrMerged {
    /// The action it performs.
    pub const ACTION: &str = "repository.merge";
    /// The evidence kind it writes.
    pub const KIND: &str = "merge_observation";

    /// Over `forge`.
    #[must_use]
    pub fn new(forge: Rc<dyn Forge>) -> Self {
        Self { forge }
    }
}

/// The judge allow-list: the paths that are not judge surfaces, every other path being one,
/// read at the merge target's tip as the forge reports it, never from the case's tree.
pub const JUDGE_ALLOW: &str = ".engineering/judge-allow.yaml";

/// The delegation record: the owner's delegations of a judge-surface merge grant to agent
/// roles, each revocable, read at the merge target's tip like [`JUDGE_ALLOW`].
pub const DELEGATIONS: &str = ".engineering/delegations.yaml";

/// The capability a delegation in [`DELEGATIONS`] gives: granting the merge of a change to a
/// judge surface.
pub const JUDGE_SURFACE_CAPABILITY: &str = "repository.merge.judge_surface";

/// The files a change to which merges under the owner's grant alone, never a delegation's,
/// whatever the allow-list admits: the guard's own inputs, every file it reads at the target's
/// tip. A delegated role that could merge a change to either could widen its own delegation or
/// the allow-list.
pub const OWNER_ONLY: [&str; 2] = [JUDGE_ALLOW, DELEGATIONS];

/// The modes a changed path may have for its name alone to decide whether it is a judge
/// surface: a regular file, plain or executable, or none (the path deleted). Any other, a
/// symbolic link (`120000`) or a submodule's commit (`160000`) above all, is a judge surface
/// whatever its name: what it points at is not the text its name stands for.
pub(crate) const NAMED_BY_PATH: [&str; 3] = ["100644", "100755", "000000"];

/// Each change of `git diff-tree -r -z --raw` output `raw` as its new mode and its path; an
/// error when a field does not read, a path that is not UTF-8 among them.
pub(crate) fn raw_changes(raw: &[u8]) -> Result<Vec<(&str, &str)>> {
    let mut fields = raw
        .split(|byte| *byte == 0)
        .filter(|field| !field.is_empty());
    let mut changes = Vec::new();
    while let Some(change) = fields.next() {
        let path = fields
            .next()
            .ok_or_else(|| anyhow!("git diff-tree gave a change with no path"))?;
        let path = std::str::from_utf8(path).map_err(|_| {
            anyhow!(
                "git diff-tree named a path that is not UTF-8: {}",
                path.escape_ascii()
            )
        })?;
        let change = std::str::from_utf8(change)
            .map_err(|_| anyhow!("git diff-tree gave a change line that is not UTF-8"))?;
        let parts: Vec<&str> = change.trim_start_matches(':').split(' ').collect();
        let [_, to_mode, _, _, _] = parts.as_slice() else {
            bail!("git diff-tree gave a change line it does not document: {change}");
        };
        changes.push((*to_mode, path));
    }
    Ok(changes)
}

/// [`JUDGE_ALLOW`] as it is written: `paths`, a list of path globs.
#[derive(Debug, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct AllowList {
    paths: Vec<String>,
}

/// [`DELEGATIONS`] as it is written: `delegations`, a list of entries.
#[derive(Debug, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct DelegationRecord {
    delegations: Vec<Delegation>,
}

/// One delegation: the delegating owner (`human:<id>`), the role (`agent:<role>`), the
/// capability, when it was given and, once revoked, when it was revoked.
#[derive(Debug, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Delegation {
    owner: String,
    role: String,
    capability: String,
    given_at: String,
    #[serde(default)]
    revoked_at: Option<String>,
}

impl Delegation {
    /// Whether it lets `approver` grant a judge-surface merge now: an owner's delegation of
    /// [`JUDGE_SURFACE_CAPABILITY`] to exactly that role, saying when it was given, with no
    /// revocation on record.
    fn admits(&self, approver: &str) -> bool {
        !self.given_at.trim().is_empty()
            && self.owner.starts_with("human:")
            && crate::case::is_approver(&self.owner)
            && self.role.starts_with("agent:")
            && self.role == approver
            && self.capability == JUDGE_SURFACE_CAPABILITY
            && self.revoked_at.is_none()
    }
}

/// The file `path` at the commit `at` in the judge's copy, read as `T` without anchors or
/// aliases; `Ok(None)` when the commit has no such file, the inner error when it does not read.
fn record_at<T: serde::de::DeserializeOwned>(
    mirror: &Mirror,
    at: &str,
    path: &str,
) -> Result<Option<std::result::Result<T, String>>> {
    Ok(blob_at(mirror, at, path)?.map(|bytes| {
        String::from_utf8(bytes)
            .map_err(|_| format!("{path} is not UTF-8"))
            .and_then(|text| {
                crate::loop_scorecard::read_yaml::<T>(Path::new(path), &text)
                    .map_err(|error| format!("{error:#}"))
            })
    }))
}

/// The globs of [`JUDGE_ALLOW`] at `at`: none when the commit has no such file; the error when
/// it does not read or holds a glob that is not relative to the tree, and the caller then admits
/// nothing.
pub(crate) fn allow_list_at(
    mirror: &Mirror,
    at: &str,
) -> Result<std::result::Result<Vec<String>, String>> {
    let Some(read) = record_at::<AllowList>(mirror, at, JUDGE_ALLOW)? else {
        return Ok(Ok(Vec::new()));
    };
    Ok(read.and_then(|list| {
        list.paths
            .into_iter()
            .map(|glob| {
                if glob.is_empty()
                    || glob.starts_with('/')
                    || glob.contains('\\')
                    || glob.split('/').any(|part| part.is_empty() || part == "..")
                {
                    Err(format!(
                        "{JUDGE_ALLOW} holds `{glob}`, which is no path glob relative to the tree"
                    ))
                } else {
                    Ok(glob)
                }
            })
            .collect()
    }))
}

/// The body of a story's file in the planning store, everything below its frontmatter: the
/// bytes after the line `---` that closes the `---` line the file opens with; `None` for a
/// file that opens with no frontmatter or never closes it.
pub(crate) fn story_body(text: &[u8]) -> Option<&[u8]> {
    let rest = text.strip_prefix(b"---\n")?;
    let mut offset = 0;
    for line in rest.split_inclusive(|byte| *byte == b'\n') {
        offset += line.len();
        if line == b"---\n" || line == b"---" {
            return Some(&rest[offset..]);
        }
    }
    None
}

/// The bytes of the regular file `path` (relative to the root) in the commit or tree `at` of
/// the judge's copy, nothing trimmed; `None` when `at` holds no blob there.
fn blob_at(mirror: &Mirror, at: &str, path: &str) -> Result<Option<Vec<u8>>> {
    let spec = format!("{at}:{path}");
    if !mirror.output(&["cat-file", "-e", &spec])?.status.success()
        || mirror.run(&["cat-file", "-t", &spec])? != "blob"
    {
        return Ok(None);
    }
    mirror.bytes(&["cat-file", "blob", &spec]).map(Some)
}

/// The story the case at `dir` was opened for and its file in the planning store
/// (`.engineering/planning/story/<name>.md`); `None` for a case with no story, or a story whose
/// name is no plain file name.
fn story_file(dir: &Path) -> Result<Option<(String, String)>> {
    let Some(story) = crate::case::story_of(dir)? else {
        return Ok(None);
    };
    let Some(name) = story.strip_prefix("story:").filter(|name| {
        !name.is_empty()
            && !name.starts_with('.')
            && name
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || b"-_.".contains(&byte))
    }) else {
        return Ok(None);
    };
    let path = format!(".engineering/planning/story/{name}.md");
    Ok(Some((story, path)))
}

/// The most characters a line of a merge commit's message has ([`merge_message`]).
const MERGE_LINE: usize = 99;

/// `line` cut to [`MERGE_LINE`] characters, ending in `…` when it was longer.
fn merge_line(line: String) -> String {
    if line.chars().count() <= MERGE_LINE {
        return line;
    }
    let mut cut: String = line.chars().take(MERGE_LINE - 1).collect();
    cut.push('…');
    cut
}

/// The evidence kinds a merge rests on, named in its commit's body ([`merge_message`]).
const MERGED_ON: [&str; 3] = ["test_result", "gate_result", "ci_result"];

/// What the merge commit of pull request `number` says: the title `Merge #<number>: ` and the
/// pull request's title (the intent's first line, [`title_of`]); the body the case's story (or
/// that it has none), the case, the head merged (`head`) and, when that is not the granted
/// `revision`, the revision too; the latest `test_result`, `gate_result` and `ci_result` record
/// of the case at `revision`, each by id and result; and the grant of `repository.merge`. Each
/// line is cut to [`MERGE_LINE`] characters; nothing attributes the text.
fn merge_message(
    dir: &Path,
    number: u64,
    revision: &str,
    head: &str,
    grant: Option<&crate::case::AuthorityDecision>,
) -> Result<crate::forge::MergeMessage> {
    let intent = dir.join("intent.md");
    let intent =
        std::fs::read_to_string(&intent).with_context(|| format!("read {}", intent.display()))?;
    let case = dir
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default();
    let mut lines = vec![match crate::case::story_of(dir)? {
        Some(story) => format!("- Story: {story}"),
        None => "- Story: none; the case was opened on an intent".to_owned(),
    }];
    lines.push(format!("- Case: {case}"));
    lines.push(format!("- Merged revision: {head}"));
    if head != revision {
        lines.push(format!(
            "- Granted revision: {revision}; the merged head adds takt's records above it"
        ));
    }
    lines.push(format!("- Evidence at {}:", short(revision)));
    let records = evidence_of(dir)?;
    for kind in MERGED_ON {
        let latest = records
            .iter()
            .rev()
            .find(|record| record["kind"] == kind && record["subject_revision"] == revision);
        lines.push(match latest {
            Some(record) => format!(
                "  - {kind} {}: {}",
                record["id"].as_str().unwrap_or(NOT_STATED),
                record["result"].as_str().unwrap_or(NOT_STATED)
            ),
            None => format!("  - {kind}: none at this revision"),
        });
    }
    lines.push(match grant {
        Some(grant) => format!(
            "- Grant: {} {} at {}, {}",
            grant.capability,
            match grant.decision {
                crate::case::Grant::Granted => "granted",
                crate::case::Grant::Denied => "denied",
            },
            grant.revision.as_deref().map_or("no revision", short),
            grant.granted_at
        ),
        None => format!("- Grant: {} not recorded", PrMerged::ACTION),
    });
    let body: Vec<String> = lines.into_iter().map(merge_line).collect();
    Ok(crate::forge::MergeMessage {
        title: merge_line(format!("Merge #{number}: {}", title_of(&intent))),
        body: body.join("\n") + "\n",
    })
}

impl PrMerged {
    /// The story file of the case at `dir` whose change the guard does not count, `None` when
    /// none is exempt. The case's story is taken by a commit takt makes on the case's branch
    /// (`takt: take <story>`, [`crate::run::take_subject`]), which changes the story's
    /// frontmatter in the planning store, a judge surface; that one path is exempt in the change
    /// the merge makes only when (i) a commit between the case's base commit `at` and
    /// `revision` has `at` as its one parent, exactly that message, and exactly that one path
    /// changed, to a regular file; (ii) the file's body below its frontmatter
    /// ([`story_body`]) is byte-equal at the target's tip `tip` and in the merge's tree
    /// `merged`; and (iii) the file in `merged` is byte-equal to the file at that take commit
    /// (R6), so no change after the take rides on it. Any other path in the planning store
    /// stays guarded.
    fn exempt_take(
        mirror: &Mirror,
        dir: &Path,
        at: &str,
        revision: &str,
        tip: &str,
        merged: &str,
    ) -> Result<Option<String>> {
        let Some((story, path)) = story_file(dir)? else {
            return Ok(None);
        };
        let message = format!("{}\n", crate::run::take_subject(&story));
        let listed = mirror.run(&["rev-list", "--end-of-options", &format!("{at}..{revision}")])?;
        let mut taken = None;
        for id in listed.lines().filter(|id| crate::forge::is_object_id(id)) {
            let bytes = mirror.bytes(&["cat-file", "commit", id])?;
            let header = crate::git::parse_commit(id, &bytes, id.len())?;
            let body = bytes
                .windows(2)
                .position(|pair| pair == b"\n\n")
                .map(|end| &bytes[end + 2..]);
            if header.parents != [at] || body != Some(message.as_bytes()) {
                continue;
            }
            let diff = mirror.bytes(&[
                "diff-tree",
                "-r",
                "-z",
                "--no-renames",
                "--raw",
                at,
                id,
                "--",
            ])?;
            if let [(mode, changed)] = raw_changes(&diff)?.as_slice()
                && *changed == path
                && *mode == "100644"
            {
                taken = Some(id);
                break;
            }
        }
        let Some(taken) = taken else {
            return Ok(None);
        };
        let (Some(at_take), Some(at_tip), Some(at_merge)) = (
            blob_at(mirror, taken, &path)?,
            blob_at(mirror, tip, &path)?,
            blob_at(mirror, merged, &path)?,
        ) else {
            return Ok(None);
        };
        // R6: the merge leaves the file exactly as the take wrote it; any later change to it,
        // in its frontmatter as in its body, is judged like any other path.
        if at_merge != at_take {
            return Ok(None);
        }
        Ok(match (story_body(&at_tip), story_body(&at_merge)) {
            (Some(before), Some(after)) if before == after => Some(path),
            _ => None,
        })
    }

    /// Why a merge granted by `approver` of the case at `revision` is refused, `None` when it
    /// is not. The merge target is `target`, the pull request's base as the forge reports it
    /// ([`PullRequestView::base`](crate::forge::PullRequestView)); a case whose `branch.yaml`
    /// names another base is refused, naming both. The target's tip is read through the forge
    /// ([`Forge::target_tip`]); the case's pinned base commit must be an ancestor of it (D4).
    /// The change judged is what the merge does to the target: the tip's tree against the tree
    /// `git merge-tree --write-tree` makes of the tip and the granted revision
    /// ([`crate::git::merged_tree`], git 2.38 or later), so a revert of the target's own change
    /// hidden by a merge of the target, or by an older base, is a change like any other; a
    /// merge that conflicts is refused. Every change the candidate made from where it meets the
    /// target (each `git merge-base --all` of the tip and the revision) is judged besides (R4),
    /// so no merge driver or attribute can hide one; and a story's file that holds a carriage
    /// return at the tip or in the merge is refused (R5). What lies above the granted revision
    /// is takt's riders alone, which [`carries`] holds the head to. A path in that change that
    /// [`JUDGE_ALLOW`]
    /// at the tip does not admit, or whose new mode is no regular file's ([`NAMED_BY_PATH`]),
    /// is a judge surface, but the story file takt's own take changes ([`Self::exempt_take`]).
    /// A change to one merges under the owner's grant (`human:`), or under an agent role's
    /// (`agent:`) when [`DELEGATIONS`] at the tip names an unrevoked delegation of
    /// [`JUDGE_SURFACE_CAPABILITY`] to that role; any other approver grants it nothing. A change
    /// to a file of [`OWNER_ONLY`] merges under the owner's grant alone, whatever the allow-list
    /// admits. Neither file is read from the tree or at the base commit, so a candidate's own
    /// edit of either counts for nothing. Every object it reads (the lists, the story's file,
    /// the commits and trees it merges and compares) comes from a [`Mirror`]: a copy of the
    /// tip, the revision and the base made through git's transport, each object hashed as it
    /// arrives, never this repository's store, where a step can overwrite one; a copy that does
    /// not arrive as its ids refuses the merge.
    #[expect(
        clippy::too_many_lines,
        reason = "one decision, read in the order it refuses"
    )]
    fn judge_guard(
        &self,
        tree: &Path,
        dir: &Path,
        branch: &Branch,
        target: &str,
        revision: &str,
        approver: &str,
    ) -> Result<Option<String>> {
        let forge = self.forge.name();
        if branch.base.as_deref() != Some(target) {
            return Ok(Some(format!(
                "the forge {forge} reports the pull request's base as {}, but the case names {} as its base: the merge target is not the one the case was opened on",
                target.escape_debug(),
                branch.base.as_deref().map_or_else(
                    || "no branch".to_owned(),
                    |base| base.escape_debug().to_string()
                )
            )));
        }
        let base = target;
        let reported = self.forge.target_tip(tree, base).with_context(|| {
            format!("read the tip of {base}, the merge target, through the forge {forge}")
        })?;
        // The forge's answer is an object id, never a name git resolves; so is the base the
        // case pinned at open.
        if !crate::forge::is_object_id(&reported) {
            return Ok(Some(format!(
                "the forge {forge} reports {base}, the merge target, at `{}`, which is no commit this repository holds",
                reported.escape_debug()
            )));
        }
        let Some(pinned) = branch
            .base_commit
            .as_deref()
            .filter(|at| crate::forge::is_object_id(at))
        else {
            return Ok(Some(format!(
                "the case pins no base commit by its id to compare with {base} at {}",
                short(&reported)
            )));
        };
        // Every object the guard reads comes from a copy made through git's transport, each
        // object hashed as it arrives (H1, H2): none is read from this repository's store,
        // where a step can overwrite one.
        let mirror = match Mirror::of(tree, &[&reported, revision, pinned])? {
            Ok(mirror) => mirror,
            Err(why) => {
                return Ok(Some(format!(
                    "the commits of {base} at {}, the revision {} and the base {} do not arrive from this repository as their ids, every object hashed again ({why})",
                    short(&reported),
                    short(revision),
                    short(pinned)
                )));
            }
        };
        let commit = |name: &str| -> Option<String> {
            mirror
                .run(&[
                    "rev-parse",
                    "--verify",
                    "--quiet",
                    "--end-of-options",
                    &format!("{name}^{{commit}}"),
                ])
                .ok()
                .filter(|at| !at.is_empty())
        };
        let Some(tip) = commit(&reported) else {
            return Ok(Some(format!(
                "the forge {forge} reports {base}, the merge target, at `{}`, which is no commit this repository holds",
                reported.escape_debug()
            )));
        };
        let Some(at) = commit(pinned) else {
            return Ok(Some(format!(
                "the case pins no base commit this repository holds to compare with {base} at {}",
                short(&tip)
            )));
        };
        if !mirror
            .output(&["merge-base", "--is-ancestor", &at, &tip])?
            .status
            .success()
        {
            return Ok(Some(format!(
                "the case's base {} is not an ancestor of {base} at {}, the merge target's tip as the forge {forge} reports it",
                short(&at),
                short(&tip)
            )));
        }
        let merged = mirror.merged_tree(&tip, revision)?;
        if !merged.conflicted.is_empty() {
            return Ok(Some(format!(
                "merging {} into {base} at {} conflicts at {}, so what the merge would change is not known",
                short(revision),
                short(&tip),
                merged.conflicted.join(", ")
            )));
        }
        // R5: takt's take never writes a carriage return into a story's file, and a reader of
        // its frontmatter fence that does not fold one reads another body than the store does.
        if let Some((_, path)) = story_file(dir)? {
            for (at, name) in [(tip.as_str(), base), (merged.tree.as_str(), "the merge")] {
                if blob_at(&mirror, at, &path)?.is_some_and(|text| text.contains(&b'\r')) {
                    return Ok(Some(format!(
                        "the story's file {path} holds a carriage return at {name}, which takt's take never writes"
                    )));
                }
            }
        }
        let (globs, unread) = match allow_list_at(&mirror, &tip)? {
            Ok(globs) => (globs, String::new()),
            Err(why) => (Vec::new(), format!(" ({why}; it admits nothing)")),
        };
        // R4: what the merge does to the target, and every change the candidate made from
        // where it meets the target, so no merge result (a driver, an attribute) hides one.
        let mut ranges = vec![(tip.clone(), merged.tree.clone())];
        for meet in mirror
            .run(&["merge-base", "--all", "--end-of-options", &tip, revision])?
            .lines()
            .filter(|meet| crate::forge::is_object_id(meet))
        {
            ranges.push((meet.to_owned(), revision.to_owned()));
        }
        let mut changes: Vec<(String, String)> = Vec::new();
        for (from, to) in &ranges {
            let diff = mirror.bytes(&[
                "diff-tree",
                "-r",
                "-z",
                "--no-renames",
                "--raw",
                from,
                to,
                "--",
            ])?;
            for (mode, path) in raw_changes(&diff)? {
                let change = (mode.to_owned(), path.to_owned());
                if !changes.contains(&change) {
                    changes.push(change);
                }
            }
        }
        let exempt = Self::exempt_take(&mirror, dir, &at, revision, &tip, &merged.tree)?;
        let mut surfaces: Vec<&str> = Vec::new();
        let mut owner_only: Vec<&str> = Vec::new();
        for (mode, path) in &changes {
            let path = path.as_str();
            if (!NAMED_BY_PATH.contains(&mode.as_str())
                || (exempt.as_deref() != Some(path)
                    && !globs.iter().any(|glob| crate::run::glob_admits(glob, path))))
                && !surfaces.contains(&path)
            {
                surfaces.push(path);
            }
            if OWNER_ONLY.contains(&path) && !owner_only.contains(&path) {
                owner_only.push(path);
            }
        }
        if surfaces.is_empty() && owner_only.is_empty() {
            return Ok(None);
        }
        let shown = if surfaces.len() > 5 {
            format!(
                "{} and {} more",
                surfaces[..5].join(", "),
                surfaces.len() - 5
            )
        } else {
            surfaces.join(", ")
        };
        let change = format!(
            "merging {} into {base} at {} changes",
            short(revision),
            short(&tip)
        );
        let touches = if surfaces.is_empty() {
            format!("{change} {}", owner_only.join(", "))
        } else {
            format!(
                "{change} judge surfaces, paths {JUDGE_ALLOW} at {base} {} does not admit or that are no regular file{unread}: {shown}",
                short(&tip)
            )
        };
        Ok(
            approver_refusal(&mirror, &tip, base, approver, &owner_only)?
                .map(|why| format!("{touches}; {why}")),
        )
    }
}

/// Why `approver`'s grant does not authorise a merge into `base` at `tip` that changes a judge
/// surface, or none when it does: the owner's grant (`human:<id>`) does; an agent role's
/// (`agent:<role>`) does only when no file of [`OWNER_ONLY`] is among `owner_only` (the changed
/// ones) and [`DELEGATIONS`] at `tip`, read from `mirror`, records an unrevoked delegation of
/// [`JUDGE_SURFACE_CAPABILITY`] to that role; any other approver grants nothing (D5). The rule
/// of story:judge-guard, for `repository.merge` and for an improvement turn's
/// `candidate.merge_judge_surface` alike.
pub(crate) fn approver_refusal(
    mirror: &Mirror,
    tip: &str,
    base: &str,
    approver: &str,
    owner_only: &[&str],
) -> Result<Option<String>> {
    let typed = crate::case::is_approver(approver);
    if typed && approver.starts_with("human:") {
        return Ok(None);
    }
    // D5: an approver that is neither the owner nor an agent role the store writes as
    // (untyped, or takt's own actor) grants nothing here, whatever a delegation names.
    if !(typed && approver.starts_with("agent:")) {
        return Ok(Some(format!(
            "the approver `{}` is neither the owner (human:<id>) nor an agent role (agent:<role>), and grants nothing there",
            approver.escape_debug()
        )));
    }
    if !owner_only.is_empty() {
        return Ok(Some(format!(
            "a change to {} merges under the owner's grant (human:<id>) alone, never under a delegation, and {approver} is not the owner",
            owner_only.join(" or ")
        )));
    }
    let delegated = match record_at::<DelegationRecord>(mirror, tip, DELEGATIONS)? {
        None => Err(format!("{DELEGATIONS} is not there")),
        Some(Err(why)) => Err(format!("{why}; it delegates nothing")),
        Some(Ok(record)) => Ok(record
            .delegations
            .iter()
            .any(|delegation| delegation.admits(approver))),
    };
    Ok(match delegated {
        Ok(true) => None,
        Ok(false) => Some(format!(
            "{approver} is not the owner, and {DELEGATIONS} at {base} {} records no unrevoked delegation of {JUDGE_SURFACE_CAPABILITY} to {approver}",
            short(tip)
        )),
        Err(why) => Some(format!(
            "{approver} is not the owner, and no delegation of {JUDGE_SURFACE_CAPABILITY} to it is on record at {base} {} ({why})",
            short(tip)
        )),
    })
}

impl std::fmt::Debug for PrMerged {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PrMerged")
            .field("forge", &self.forge.name())
            .finish()
    }
}

impl Validator for PrMerged {
    fn name(&self) -> &str {
        "takt.pr-merged"
    }

    fn action(&self) -> &str {
        Self::ACTION
    }

    fn kind(&self) -> &str {
        Self::KIND
    }

    fn subject(&self) -> &str {
        "implementation"
    }

    /// The case's `branch.yaml`, when takt's riders are pushed before the merge, the commit
    /// messages and files the bot route scanned before that push, and what it keeps of the merge
    /// request it sent and the response it read.
    fn writes(&self) -> Vec<PathBuf> {
        let artifacts = Path::new("artifacts");
        vec![
            PathBuf::from(BRANCH_FILE),
            artifacts.join(BotRoute::MESSAGES),
            artifacts.join(BotRoute::FILES),
            artifacts.join(BotRoute::MERGE_REQUEST),
            artifacts.join(BotRoute::MERGE_RESPONSE),
        ]
    }

    fn observe(&self, tree: &Path, revision: &str, seq: usize) -> Result<Observation> {
        self.observe_waiting(tree, revision, seq)
            .map(|(observation, _)| observation)
    }

    fn observe_waiting(
        &self,
        tree: &Path,
        revision: &str,
        seq: usize,
    ) -> Result<(Observation, bool)> {
        let dir = current(tree)?;
        let mut branch = Branch::read(&dir)?;
        // The case's branch, from the case id: not the `name` of a file a harness can write.
        let name = branch_of(&dir)?;
        let forge = self.forge.name();
        let nothing = |summary: String| Observation {
            summary,
            records: Vec::new(),
            artifact: None,
        };
        let Some(number) = branch.pull_request else {
            return Ok((
                nothing(
                    "no pull request is open for the case; nothing to merge, the claim stays unknown"
                        .to_owned(),
                ),
                false,
            ));
        };
        let pushed = match branch.pushed.clone() {
            Some(pushed) if carries(tree, &dir, revision, &pushed)? => pushed,
            pushed => {
                return Ok((
                    nothing(format!(
                        "the head of pull request #{number} is {}, not the implementation revision {}; nothing to merge, the claim stays unknown",
                        pushed.as_deref().map_or("not pushed", short),
                        short(revision)
                    )),
                    false,
                ));
            }
        };
        let mut view = self
            .forge
            .pull_request(tree, number)
            .with_context(|| format!("read pull request #{number} through the forge {forge}"))?;
        // The head last pushed, or the revision itself: `branch.yaml` names the head before it
        // is pushed, so a push the forge never took leaves the pull request at the revision,
        // and the push below is made again.
        if view.head.branch != name || (view.head.sha != pushed && view.head.sha != revision) {
            return Ok((
                nothing(format!(
                    "pull request #{number} has the head {} at {}, not {name} at {}; nothing to merge, the claim stays unknown",
                    view.head.branch,
                    short(&view.head.sha),
                    short(&pushed)
                )),
                false,
            ));
        }
        let (state, merged_now) = match view.state {
            PullRequestState::Open => {
                // The grant authorises the revision the case reached the gate at and no other:
                // checked here as well as in the evaluation that admitted the step.
                let authority = crate::case::Authority::read(&dir)?.unwrap_or_default();
                if !authority.grants_at(self.action(), revision) {
                    let granted = authority
                        .decision(self.action())
                        .filter(|decision| decision.decision == crate::case::Grant::Granted)
                        .map_or_else(
                            || "the case holds no grant of it".to_owned(),
                            |decision| {
                                format!(
                                    "the case's grant is for {}",
                                    decision.revision.as_deref().map_or("no revision", short)
                                )
                            },
                        );
                    return Ok((
                        nothing(format!(
                            "pull request #{number} at {} is open, and {} is not granted at that revision ({granted}); nothing merged, the claim stays unknown",
                            short(revision),
                            self.action()
                        )),
                        false,
                    ));
                }
                // Whose grant it is decides a change to a judge surface, before anything is
                // pushed: the owner's, or a role's the owner delegated to on record.
                let approver = authority
                    .decision(self.action())
                    .map_or("", |decision| decision.approver.as_str());
                // The base judged: the merge goes into it or nowhere (R7).
                let target = view.base.clone();
                if let Some(refusal) =
                    self.judge_guard(tree, &dir, &branch, &target, revision, approver)?
                {
                    return Ok((
                        nothing(format!(
                            "pull request #{number} at {} is open and granted by {approver}, and {refusal}; nothing merged, the claim stays unknown",
                            short(revision)
                        )),
                        false,
                    ));
                }
                // takt's riders above the revision go with the pull request, so the store's
                // records reach the base with the merge: the head is brought up to the
                // branch's `HEAD` when that carries the revision and descends from the head.
                let local = crate::git::head(tree)?;
                if local != view.head.sha
                    && carries(tree, &dir, revision, &local)?
                    && crate::git::run(
                        tree,
                        &["merge-base", "--is-ancestor", &view.head.sha, &local],
                    )
                    .is_ok()
                {
                    // What the push would publish is scanned before it, as `pr.open` scans
                    // its own; a refusal pushes nothing, merges nothing and records nothing.
                    let base = branch.base.clone().ok_or_else(|| {
                        anyhow!(
                            "the case has pull request #{number} but no base branch to count the commits it would push from; nothing pushed, nothing merged"
                        )
                    })?;
                    self.forge
                        .scan_outgoing(
                            tree,
                            &dir.join("artifacts"),
                            &crate::forge::Outgoing {
                                branch: &name,
                                base: &base,
                                head: &local,
                                request: None,
                            },
                        )
                        .with_context(|| {
                            format!("scan {name} before the push through the forge {forge}")
                        })?;
                    // Named before the push: a push the forge took but reported failed leaves
                    // the head it took recorded, and the next run merges it.
                    branch.pushed = Some(local.clone());
                    branch.write(&dir)?;
                    self.forge
                        .push_revision(tree, &name, &local)
                        .with_context(|| format!("push {name} through the forge {forge}"))?;
                    view = self.forge.pull_request(tree, number).with_context(|| {
                        format!("read pull request #{number} through the forge {forge}")
                    })?;
                    if view.head.branch != name || view.head.sha != local {
                        // The case's branch at another commit: the forge has not moved the
                        // pull request to the push yet, and is waited on.
                        return Ok((
                            nothing(format!(
                                "pushed {name} at {} through {forge}, and pull request #{number} has the head {} at {}; nothing merged, the claim stays unknown",
                                short(&local),
                                view.head.branch,
                                short(&view.head.sha)
                            )),
                            view.head.branch == name,
                        ));
                    }
                }
                // CI was read on the revision; a head carrying takt's riders above it
                // is a new head, and a forge that requires checks merges it only once they
                // pass. Its checks are read first: pending or failing, nothing is merged and no
                // record written; pending checks are waited on, and a later run merges once
                // failing ones pass.
                if view.head.sha != revision {
                    let checks = self.forge.checks(tree, number).with_context(|| {
                        format!(
                            "read the checks on pull request #{number} through the forge {forge}"
                        )
                    })?;
                    if checks != Checks::Pass {
                        return Ok((
                            nothing(format!(
                                "checks on pull request #{number} at its head {}, takt's riders above {}, through {forge}: {checks}; nothing merged, the claim stays unknown",
                                short(&view.head.sha),
                                short(revision)
                            )),
                            checks == Checks::Pending,
                        ));
                    }
                }
                // R7: the pull request read again right before the merge; a base the forge
                // moved it to after the guard judged its target merges nothing.
                let last = self.forge.pull_request(tree, number).with_context(|| {
                    format!(
                        "read pull request #{number} through the forge {forge} before the merge"
                    )
                })?;
                if last.base != target {
                    return Ok((
                        nothing(format!(
                            "the forge {forge} reports pull request #{number}'s base as {} right before the merge, not {target}, the target the judge guard judged; nothing merged, the claim stays unknown",
                            last.base.escape_debug()
                        )),
                        false,
                    ));
                }
                // A read of the checks can answer for another head than the one merged (a
                // forge whose checks lag the push): the forge then refuses the merge for its
                // required check, and that refusal is nothing merged, waited on like pending
                // checks. Every other failure of the merge stays an error.
                let message = merge_message(
                    &dir,
                    number,
                    revision,
                    &view.head.sha,
                    authority.decision(self.action()),
                )?;
                if let Err(error) = self.forge.merge_with(
                    tree,
                    &dir.join("artifacts"),
                    number,
                    &view.head.sha,
                    &message,
                ) {
                    if crate::forge::refused_for_a_pending_check(&error) {
                        return Ok((
                            nothing(format!(
                                "the forge {forge} refused the merge of pull request #{number} at its head {} for a required check that has not reported ({}); nothing merged, the claim stays unknown",
                                short(&view.head.sha),
                                format!("{error:#}").replace('\n', " ")
                            )),
                            true,
                        ));
                    }
                    return Err(error.context(format!(
                        "merge pull request #{number} through the forge {forge}"
                    )));
                }
                let after = self.forge.pull_request(tree, number).with_context(|| {
                    format!("read pull request #{number} through the forge {forge} after the merge")
                })?;
                (after.state, true)
            }
            state => (state, false),
        };
        let head = view.head.sha.clone();
        let carried = above(revision, &head);
        let result = match state {
            PullRequestState::Merged => Some("merged"),
            PullRequestState::Closed => Some("closed"),
            PullRequestState::Open => None,
        };
        let summary = match (state, merged_now) {
            (PullRequestState::Merged, true) => format!(
                "merged pull request #{number} at {} through {forge}: merged{carried}",
                short(revision)
            ),
            (PullRequestState::Merged, false) => format!(
                "pull request #{number} at {} is already merged, read through {forge}: merged{carried}",
                short(revision)
            ),
            (PullRequestState::Closed, _) => format!(
                "pull request #{number} at {} is closed unmerged, read through {forge}: closed",
                short(revision)
            ),
            (PullRequestState::Open, _) => format!(
                "merged pull request #{number} at {} through {forge}, which still reports it open; no record, the claim stays unknown",
                short(revision)
            ),
        };
        let records = result
            .map(|result| {
                vec![Record::new(
                    format!("merge-observation-{seq}"),
                    self.kind(),
                    Some(result.to_owned()),
                    self.subject(),
                    revision,
                    self.name(),
                )]
            })
            .unwrap_or_default();
        // The record is about the revision the grant authorises; which head was merged, the
        // revision or the revision with takt's riders above it, is kept beside it.
        let artifact = if state == PullRequestState::Merged {
            let kept = serde_json::json!({
                "pull_request": number,
                "granted": revision,
                "merged_head": head,
            });
            Some(Artifact {
                name: format!("merged-head-{seq}.json"),
                bytes: serde_json::to_vec_pretty(&kept).context("serialize the merged head")?,
            })
        } else {
            None
        };
        Ok((
            Observation {
                summary,
                records,
                artifact,
            },
            false,
        ))
    }
}

/// A built-in validator: its type's name, the action it performs and the evidence kind it
/// writes, each read from the type's own `ACTION` and `KIND`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BuiltIn {
    /// The type's name, as `impl Validator for` names it.
    pub validator: &'static str,
    /// The action it performs.
    pub action: &'static str,
    /// The evidence kind it writes.
    pub kind: &'static str,
}

/// takt's built-in validators, whether or not a run has them configured. An action one of them
/// performs is the run loop's to take, never the harness's, and a kind one of them writes is an
/// observation, never a choice the step choice records (`crate::run`): a protocol in which an
/// action no built-in validator performs may produce one with `effect: read` is refused, and so
/// is one where an outcome or an obligation holds on choice records alone.
pub const BUILT_IN: [BuiltIn; 16] = [
    BuiltIn {
        validator: "TestsRun",
        action: TestsRun::ACTION,
        kind: TestsRun::KIND,
    },
    BuiltIn {
        validator: "GateRun",
        action: GateRun::ACTION,
        kind: GateRun::KIND,
    },
    BuiltIn {
        validator: "PrOpen",
        action: PrOpen::ACTION,
        kind: PrOpen::KIND,
    },
    BuiltIn {
        validator: "CiRead",
        action: CiRead::ACTION,
        kind: CiRead::KIND,
    },
    BuiltIn {
        validator: "PrMerged",
        action: PrMerged::ACTION,
        kind: PrMerged::KIND,
    },
    BuiltIn {
        validator: "ChangeRead",
        action: ChangeRead::ACTION,
        kind: ChangeRead::KIND,
    },
    BuiltIn {
        validator: "LoopMeasure",
        action: crate::loop_scorecard::LoopMeasure::ACTION,
        kind: crate::loop_scorecard::LoopMeasure::KIND,
    },
    BuiltIn {
        validator: "LoopDefinitionCheck",
        action: crate::loop_scorecard::LoopDefinitionCheck::ACTION,
        kind: crate::loop_scorecard::LoopDefinitionCheck::KIND,
    },
    BuiltIn {
        validator: "LoopCompare",
        action: crate::loop_scorecard::LoopCompare::ACTION,
        kind: crate::loop_scorecard::LoopCompare::KIND,
    },
    BuiltIn {
        validator: "FindingFile",
        action: crate::improvement::FindingFile::ACTION,
        kind: crate::improvement::FindingFile::KIND,
    },
    BuiltIn {
        validator: "ScenarioReproduce",
        action: crate::improvement::ScenarioReproduce::ACTION,
        kind: crate::improvement::ScenarioReproduce::KIND,
    },
    BuiltIn {
        validator: "ScenarioVerify",
        action: crate::improvement::ScenarioVerify::ACTION,
        kind: crate::improvement::ScenarioVerify::KIND,
    },
    BuiltIn {
        validator: "ScorecardCompare",
        action: crate::improvement::ScorecardCompare::ACTION,
        kind: crate::improvement::ScorecardCompare::KIND,
    },
    // One type, two actions: one entry each.
    BuiltIn {
        validator: "CandidateMerged",
        action: crate::improvement::CandidateMerged::ACTION,
        kind: crate::improvement::CandidateMerged::KIND,
    },
    BuiltIn {
        validator: "CandidateMerged",
        action: crate::improvement::CandidateMerged::JUDGE_SURFACE_ACTION,
        kind: crate::improvement::CandidateMerged::KIND,
    },
    BuiltIn {
        validator: "BinaryPromote",
        action: crate::improvement::BinaryPromote::ACTION,
        kind: crate::improvement::BinaryPromote::KIND,
    },
];

/// Whether a built-in validator performs `action`.
#[must_use]
pub fn is_built_in_action(action: &str) -> bool {
    BUILT_IN.iter().any(|built_in| built_in.action == action)
}

/// Whether a built-in validator writes `kind`.
#[must_use]
pub fn is_built_in_kind(kind: &str) -> bool {
    BUILT_IN.iter().any(|built_in| built_in.kind == kind)
}

/// `change.read`: observes whether the implementation revision's tree differs from the case's
/// base outside `.engineering/` (the planning store and the cases, which takt's own take,
/// approval and records change), by content, not by its commits or their messages, and writes
/// one `change_record`: `ahead` when merging the revision into the tip, now, of the base branch
/// `branch.yaml` names (`git merge-tree`, nothing checked out) would change that tip's tree
/// outside `.engineering/`, `none` when it would not: the same content landed on the base by
/// another commit, with or without more beside it, is no change to publish; a merge that
/// conflicts at any path outside `.engineering/` is `ahead`, whatever tree git writes for it,
/// and the summary names the conflicted paths. When the base branch is not there, the revision's tree is
/// compared with the commit pinned when the case opened instead; a case with neither writes no
/// record. It reads git only, with no code from configuration: nothing is run from the tree and
/// nothing leaves the machine, so it compares with the local base branch as it stands, and a
/// forge's base that moved without a fetch is not seen.
///
/// When the case's protocol declares [`ChangeRead::PATHS_KIND`] and lists it among what
/// `change.read` may produce (`story.delivery/2`), it writes one `paths_changed` record beside
/// the `change_record`, at the same revision, from the same comparison: the paths outside
/// `.engineering/` at which the merged tree differs from the tip (with every path whose merge
/// conflicts outside it), or the revision's tree from the pinned base; `both` when they hold a
/// test path ([`crate::harness::is_test_path`], what `tests.write` may change) and another,
/// `tests_only`, `implementation_only`, or `none` when there is no such path. A protocol that
/// does not declare the kind gets no such record and the same summary as before.
#[derive(Debug, Clone, Default)]
pub struct ChangeRead;

impl ChangeRead {
    /// The action it performs.
    pub const ACTION: &str = "change.read";
    /// The evidence kind it writes.
    pub const KIND: &str = "change_record";
    /// The second kind it writes, when the case's protocol declares it.
    pub const PATHS_KIND: &str = "paths_changed";
}

/// Whether the protocol of the case at `case_dir`, as its `protocol.yaml` holds it, declares the
/// evidence kind `kind` and lists it among what `action` may produce.
fn produces_kind(case_dir: &Path, action: &str, kind: &str) -> Result<bool> {
    let path = case_dir.join("protocol.yaml");
    let text =
        std::fs::read_to_string(&path).with_context(|| format!("read {}", path.display()))?;
    let doc: serde_yaml_ng::Value =
        serde_yaml_ng::from_str(&text).with_context(|| format!("read {}", path.display()))?;
    let declared = doc["evidence_kinds"]
        .as_mapping()
        .is_some_and(|kinds| kinds.contains_key(kind));
    let produced = doc["actions"][action]["may_produce"]
        .as_sequence()
        .is_some_and(|produced| {
            produced
                .iter()
                .any(|entry| entry["evidence"].as_str() == Some(kind))
        });
    Ok(declared && produced)
}

/// The `paths_changed` result of `paths`, with how many of them are test paths
/// ([`crate::harness::is_test_path`]) and how many are not.
fn paths_result(paths: &[String]) -> (&'static str, usize, usize) {
    let tests = paths
        .iter()
        .filter(|path| crate::harness::is_test_path(path))
        .count();
    let implementation = paths.len() - tests;
    let result = match (tests > 0, implementation > 0) {
        (true, true) => "both",
        (true, false) => "tests_only",
        (false, true) => "implementation_only",
        (false, false) => "none",
    };
    (result, tests, implementation)
}

impl Validator for ChangeRead {
    fn name(&self) -> &str {
        "takt.change-read"
    }

    fn action(&self) -> &str {
        Self::ACTION
    }

    fn kind(&self) -> &str {
        Self::KIND
    }

    fn subject(&self) -> &str {
        "implementation"
    }

    fn observe(&self, tree: &Path, revision: &str, seq: usize) -> Result<Observation> {
        let dir = current(tree)?;
        let pinned = base_revision(tree, &dir)?;
        let tip = Branch::read(&dir)?.base.and_then(|name| {
            crate::git::run(
                tree,
                &[
                    "rev-parse",
                    "--verify",
                    "--quiet",
                    "--end-of-options",
                    &format!("refs/heads/{name}^{{commit}}"),
                ],
            )
            .ok()
            .filter(|tip| !tip.is_empty())
            .map(|tip| (name, tip))
        });
        // The paths the comparison changes, read only for a protocol that declares the kind.
        let paths_declared = produces_kind(&dir, Self::ACTION, Self::PATHS_KIND)?;
        let (differs, summary, paths) = if let Some((name, tip)) = tip {
            // A merge that conflicts at any path outside `.engineering/` is `ahead`, by the paths
            // merge-tree names: the tree it writes may keep the base's side (a file the case
            // deleted that the base changed, a binary file both changed), and there is a change
            // to publish all the same. A conflict only inside `.engineering/` is no change
            // outside it; the summary names the conflicted paths either way.
            let merged = crate::git::merged_tree(tree, &tip, revision)?;
            let outside: Vec<&str> = merged
                .conflicted
                .iter()
                .map(String::as_str)
                .filter(|path| *path != ".engineering" && !path.starts_with(".engineering/"))
                .collect();
            let head = format!(
                "merging {} into {name} at {}, its tip now,",
                short(revision),
                short(&tip)
            );
            let paths = if paths_declared {
                let mut paths =
                    crate::git::paths_differing_outside(tree, &tip, &merged.tree, ".engineering")?;
                for path in &outside {
                    if !paths.iter().any(|known| known == path) {
                        paths.push((*path).to_owned());
                    }
                }
                Some(paths)
            } else {
                None
            };
            if outside.is_empty() {
                let differs =
                    crate::git::trees_differ_outside(tree, &tip, &merged.tree, ".engineering")?;
                let conflicts = if merged.conflicted.is_empty() {
                    String::new()
                } else {
                    format!(
                        " conflicts only inside .engineering/ at {} and",
                        merged.conflicted.join(", ")
                    )
                };
                (
                    differs,
                    format!(
                        "{head}{conflicts} {} its tree outside .engineering/",
                        if differs {
                            "changes"
                        } else {
                            "leaves as it is"
                        }
                    ),
                    paths,
                )
            } else {
                (
                    true,
                    format!(
                        "{head} conflicts outside .engineering/ at {}, a change to publish",
                        outside.join(", ")
                    ),
                    paths,
                )
            }
        } else if let Some((shown, base)) = pinned {
            let differs = crate::git::trees_differ_outside(tree, &base, revision, ".engineering")?;
            let paths = if paths_declared {
                Some(crate::git::paths_differing_outside(
                    tree,
                    &base,
                    revision,
                    ".engineering",
                )?)
            } else {
                None
            };
            (
                differs,
                format!(
                    "the base branch is not there to merge into; the tree of {} {} the tree of {shown} outside .engineering/",
                    short(revision),
                    if differs { "differs from" } else { "is" }
                ),
                paths,
            )
        } else {
            return Ok(Observation {
                summary: "the case records no base; nothing to compare, the claim stays unknown"
                    .to_owned(),
                records: Vec::new(),
                artifact: None,
            });
        };
        let result = if differs { "ahead" } else { "none" };
        let mut summary = format!("{summary}: {result}");
        let mut records = vec![Record::new(
            format!("change-record-{seq}"),
            self.kind(),
            Some(result.to_owned()),
            self.subject(),
            revision,
            self.name(),
        )];
        if let Some(paths) = paths {
            let (changed, tests, implementation) = paths_result(&paths);
            summary.push_str(&format!(
                "; it changes {tests} test path(s) and {implementation} implementation path(s): {changed}"
            ));
            records.push(Record::new(
                format!("paths-changed-{seq}"),
                Self::PATHS_KIND,
                Some(changed.to_owned()),
                self.subject(),
                revision,
                self.name(),
            ));
        }
        Ok(Observation {
            summary,
            records,
            artifact: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn observed() -> Result<Observation> {
        Ok(Observation {
            summary: "observed".to_owned(),
            records: Vec::new(),
            artifact: None,
        })
    }

    /// An observation that returns at the deadline it was given, the run's own, records
    /// nothing, whatever it observed; one before it, or without one, is kept.
    #[test]
    fn an_observation_returning_at_the_deadline_is_not_recorded() {
        let now = std::time::Instant::now();
        let ended = ended_by(Some(now), "tests.run", observed());
        let error = format!(
            "{:#}",
            ended.expect_err("an observation at the deadline ends")
        );
        assert!(error.contains("run's end"), "{error}");
        let later = now + std::time::Duration::from_secs(60);
        assert!(ended_by(Some(later), "tests.run", observed()).is_ok());
        assert!(ended_by(None, "tests.run", observed()).is_ok());
    }

    /// The lint tables of a manifest in each form TOML writes them, and what the scanner does
    /// not read with certainty.
    #[test]
    fn lint_changes_reads_every_form_of_the_lint_tables() {
        let package = "[package]\nname = \"a\"\n";
        let changed = |after: &str| lint_changes(package, &format!("{package}{after}"));
        assert_eq!(
            changed("[lints.clippy]\nall = \"allow\"\n"),
            Ok(vec![
                "lints.clippy".to_owned(),
                "lints.clippy.all".to_owned()
            ])
        );
        assert_eq!(
            changed("[lints]\nworkspace = true\n"),
            Ok(vec!["lints".to_owned(), "lints.workspace".to_owned()])
        );
        assert_eq!(
            lint_changes("", "\"lints\" . 'clippy'.all = \"allow\"\n"),
            Ok(vec!["lints.clippy.all".to_owned()])
        );
        assert_eq!(
            lint_changes(
                "",
                "workspace = { lints = { clippy = { all = \"allow\" } } }\n"
            ),
            Ok(vec!["workspace".to_owned()])
        );
        assert_eq!(
            lint_changes(
                "[workspace]\nmembers = [\n  \"a\",\n]\n",
                "[workspace]\nmembers = [\n  \"a\",\n  [\"b\"],\n]\n[workspace.lints.rust]\nwarnings = \"allow\" # off\n"
            ),
            Ok(vec![
                "workspace.lints.rust".to_owned(),
                "workspace.lints.rust.warnings".to_owned()
            ])
        );
        assert_eq!(
            changed("[dependencies]\nserde = { version = \"1\", features = [\"derive\"] }\n"),
            Ok(Vec::new())
        );
        assert_eq!(
            changed("[\"lint\\u0073\".clippy]\nall = \"allow\"\n"),
            Err("after")
        );
        assert_eq!(changed("[lints] x\n"), Err("after"));
        assert_eq!(changed("d = \"\"\"\n[package]\n\"\"\"\n"), Err("after"));
        assert_eq!(changed("d = [\n"), Err("after"));
    }

    /// story:a-gate-script-with-a-colon-is-run, acceptance 2: a path a gate's command names that
    /// starts with `:` is looked up as the literal path it names. Git reads `:/` as the top of the
    /// tree and refuses `:/__main__.py`; `:x` it reads as a magic-less pattern that matches no
    /// path, so a file of that name would look absent.
    #[test]
    fn a_path_that_starts_with_a_colon_is_looked_up_literally() {
        let dir = std::env::temp_dir().join(format!("takt-object-at-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join(":")).expect("a directory");
        std::fs::write(dir.join(":x"), "x\n").expect("a file");
        std::fs::write(dir.join(":").join("__main__.py"), "print(1)\n").expect("a module");
        crate::git::run(&dir, &["init", "-q", "-b", "main"]).expect("git init");
        crate::git::run(&dir, &["add", "-A"]).expect("git add");
        crate::git::run(
            &dir,
            &[
                "-c",
                "user.name=t",
                "-c",
                "user.email=t@localhost",
                "commit",
                "-q",
                "-m",
                "start",
            ],
        )
        .expect("git commit");
        let file = object_at(&dir, "HEAD", ":x").expect("the lookup runs");
        assert!(
            file.as_deref()
                .is_some_and(|o| o.starts_with("100644 blob ")),
            "story:a-gate-script-with-a-colon-is-run: `:x` is not looked up literally: {file:?}"
        );
        let module = object_at(&dir, "HEAD", ":/__main__.py").expect("the lookup runs");
        assert!(
            module
                .as_deref()
                .is_some_and(|o| o.starts_with("100644 blob ")),
            "story:a-gate-script-with-a-colon-is-run: `:/__main__.py` is not looked up literally: \
             {module:?}"
        );
        assert_eq!(
            object_at(&dir, "HEAD", ":/nothing.py").expect("the lookup runs"),
            None
        );
        assert_eq!(
            python_dir(&dir, "HEAD", "HEAD", ":").expect("the listing runs"),
            vec![":/__main__.py".to_owned()],
            "story:a-gate-script-with-a-colon-is-run: the directory `:` is not listed literally"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }
}
