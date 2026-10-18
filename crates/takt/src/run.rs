//! The loop: evaluate, frontier, one step, effect, observation, validator, evidence, evaluate;
//! until an outcome is legitimate, an approval is needed, nothing is admissible, a step is
//! refused, or the stop criteria are met. A run opens a case, or resumes one a run before it
//! opened: a case stopped at the gate is resumed once the operator has granted it, and the
//! merge is then a deterministic step like any other.

use std::collections::BTreeSet;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::Instant;

use anyhow::{Context as _, Result, bail};

use crate::case::{CASES_DIR, CaseDir};
use crate::frontier::{Frontier, Story, Take, Unit};
use crate::harness::{Harness, StepContext, is_test_path};
use crate::protocol::LoadedProtocol;
use crate::validator::Validator;

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
    /// No action is admissible and none needs an approval.
    NothingAdmissible,
    /// The step budget is spent.
    Iterations(u64),
    /// The time budget is spent.
    Hours(f64),
    /// The token budget is spent.
    Tokens(u64),
    /// The harness did something the frontier did not admit; its change was discarded.
    StepRefused(String),
    /// A judgement step changed no file and the decision did not move: another step would not either.
    NoProgress,
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
            Stop::StepRefused(why) => write!(f, "StepRefused ({why})"),
            Stop::NoProgress => write!(f, "NoProgress"),
        }
    }
}

/// Whether `validator` is due at a decision: its action is admissible, its evidence is missing
/// at its subject's current revision, and it has not already observed there without a record.
fn is_due(
    case: &CaseDir,
    admissible: &[String],
    observed_without_record: &BTreeSet<(String, String)>,
    validator: &dyn Validator,
) -> bool {
    admissible.iter().any(|a| a == validator.action())
        && !case
            .has_evidence(validator.kind(), validator.subject())
            .unwrap_or(true)
        && !case.revision(validator.subject()).is_some_and(|revision| {
            observed_without_record.contains(&(validator.name().to_owned(), revision))
        })
}

/// A run over one case.
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
    started: Instant,
    options: RunOptions,
}

impl<'a> Run<'a> {
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
    /// the story id and the instant.
    pub fn open_unit(
        tree: &Path,
        mut options: RunOptions,
        unit: Unit,
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
        let id = options
            .case_id
            .clone()
            .unwrap_or_else(|| format!("takt-{}", crate::now().replace([':', '-'], "")));
        let case = CaseDir::open(tree, protocol, &id, &options.intent, &options.inputs)?;
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
        Ok(Self {
            tree: tree.to_path_buf(),
            case,
            unit,
            harness,
            validators,
            steps: 0,
            tokens: 0,
            started: Instant::now(),
            options,
        })
    }

    /// Prepares a run on the case `id` a run before this one opened under `tree`: the case's
    /// files as they are, its `authority.yaml` read into every evaluation, its intent and inputs
    /// the run's. The unit is the story the case records, or the intent it was opened on.
    /// Nothing is opened and nothing discarded; the protocol, inputs and case id in `options`
    /// are not read: the case's own rule.
    pub fn resume(
        tree: &Path,
        mut options: RunOptions,
        id: &str,
        harness: &'a dyn Harness,
        validators: &'a [Box<dyn Validator>],
    ) -> Result<Self> {
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
        Ok(Self {
            tree: tree.to_path_buf(),
            case,
            unit,
            harness,
            validators,
            steps: 0,
            tokens: 0,
            started: Instant::now(),
            options,
        })
    }

    /// Resumes the open case of a story `frontier` lists `active`, if there is one; else opens
    /// the case on the next ready story, the store told last: for each candidate in order, the
    /// case is opened first (the protocol loaded, the inputs checked), then the story is taken;
    /// a refusal removes the case just opened, is printed on `out` verbatim, and the next
    /// candidate is tried. `None`, with no case opened, when nothing is open and nothing is
    /// ready. One case directory per taken story, none for a refused one, never a second for a
    /// story with one open.
    pub fn open_next(
        tree: &Path,
        options: RunOptions,
        frontier: &dyn Frontier,
        harness: &'a dyn Harness,
        validators: &'a [Box<dyn Validator>],
        out: &mut dyn Write,
    ) -> Result<Option<Self>> {
        let active = frontier.active(tree)?;
        if let Some((case_id, story_id)) = crate::case::resumable(tree, &active)? {
            let run = Self::resume(tree, options, &case_id, harness, validators)?;
            let title = match &run.unit {
                Unit::Story(story) => story.title.clone(),
                Unit::Intent(intent) => intent.clone(),
            };
            writeln!(
                out,
                "frontier: resumed {story_id}: {title} (case {case_id})"
            )?;
            return Ok(Some(run));
        }
        for id in frontier.candidates(tree)? {
            let story = frontier.show(tree, &id)?;
            let title = story.title.clone();
            let run = Self::open_unit(
                tree,
                options.clone(),
                Unit::Story(story),
                harness,
                validators,
            )?;
            match frontier.take(tree, &id)? {
                Take::Taken => {
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

    fn budget_spent(&self) -> Option<Stop> {
        if let Some(n) = self.options.iterations
            && self.steps >= n
        {
            return Some(Stop::Iterations(n));
        }
        if let Some(h) = self.options.hours
            && self.started.elapsed().as_secs_f64() >= h * 3600.0
        {
            return Some(Stop::Hours(h));
        }
        if let Some(t) = self.options.tokens
            && self.tokens >= t
        {
            return Some(Stop::Tokens(t));
        }
        None
    }

    /// Runs until a stop; prints one line per frontier, step and evidence to `out`.
    pub fn run(&mut self, out: &mut dyn Write) -> Result<Stop> {
        let inputs: Vec<String> = self
            .options
            .inputs
            .iter()
            .map(|(k, v)| format!("{k}={v}"))
            .collect();
        writeln!(
            out,
            "case {} on {} in {}; inputs {}",
            self.case.id,
            self.case.protocol.id,
            self.tree.display(),
            if inputs.is_empty() {
                "none".to_owned()
            } else {
                inputs.join(" ")
            }
        )?;
        let mut seq = 0usize;
        let mut last_idle: Option<String> = None;
        // The validators that observed without writing a record, each with the revision it
        // observed at: not due again there, because the next observation would end the same
        // way; a commit moves the revision and makes them due again.
        let mut observed_without_record: BTreeSet<(String, String)> = BTreeSet::new();
        loop {
            seq += 1;
            let decision = self.case.evaluate(seq)?;
            writeln!(out, "{}", decision.frontier_line())?;
            let state = format!(
                "{:?}{:?}{:?}",
                decision.claims, decision.obligations, decision.actions
            );
            if last_idle.as_deref() == Some(state.as_str()) {
                return Ok(Stop::NoProgress);
            }
            let legitimate = decision.legitimate();
            if let Some(outcome) = legitimate.first() {
                return Ok(Stop::Completed(outcome.clone()));
            }
            if let Some(stop) = self.budget_spent() {
                return Ok(stop);
            }
            let admissible = decision.admissible();
            // The judgement actions: the admissible ones no validator owns.
            let offered: Vec<String> = admissible
                .iter()
                .filter(|a| !self.validators.iter().any(|v| v.action() == a.as_str()))
                .cloned()
                .collect();
            // A deterministic step first: a validator due at this decision.
            let due = self.validators.iter().find(|v| {
                is_due(
                    &self.case,
                    &admissible,
                    &observed_without_record,
                    v.as_ref(),
                )
            });
            if let Some(validator) = due {
                self.steps += 1;
                let revision = self
                    .case
                    .revision(validator.subject())
                    .context("the validator's subject has a revision")?;
                let observation = validator.observe(&self.tree, &revision, seq)?;
                writeln!(out, "step {}: {} {{}}", self.steps, validator.action())?;
                writeln!(out, "  effect: {}", observation.summary)?;
                // What the validator retains lands before the records that refer to it: a
                // retention that fails ends the run with nothing written for the step.
                if let Some(artifact) = &observation.artifact {
                    let dir = self.case.dir.join("artifacts");
                    std::fs::create_dir_all(&dir)
                        .with_context(|| format!("create {}", dir.display()))?;
                    let path = dir.join(&artifact.name);
                    std::fs::write(&path, &artifact.bytes)
                        .with_context(|| format!("write {}", path.display()))?;
                    writeln!(
                        out,
                        "  artifact: {}",
                        path.strip_prefix(&self.tree).unwrap_or(&path).display()
                    )?;
                }
                for record in &observation.records {
                    self.case.add_evidence(record)?;
                    writeln!(
                        out,
                        "  evidence: {} {}",
                        record.kind,
                        record.result.as_deref().unwrap_or("-")
                    )?;
                }
                // A validator that wrote no record (the gate was unknown) is not due again at
                // this revision. The pass counts as idle only when nothing else can move the
                // decision, no judgement step offered and no other validator due: then the
                // next pass would see the same decision, and the run stops there as no
                // progress.
                if observation.records.is_empty() {
                    observed_without_record.insert((validator.name().to_owned(), revision));
                    let another_due = self.validators.iter().any(|v| {
                        is_due(
                            &self.case,
                            &admissible,
                            &observed_without_record,
                            v.as_ref(),
                        )
                    });
                    last_idle = (offered.is_empty() && !another_due).then_some(state);
                } else {
                    last_idle = None;
                }
                continue;
            }
            // Then a judgement step among the offered actions.
            if offered.is_empty() {
                let approvals = decision.approval_required();
                return Ok(if approvals.is_empty() {
                    Stop::NothingAdmissible
                } else {
                    Stop::ApprovalRequired(approvals)
                });
            }
            self.steps += 1;
            let approvals: Vec<String> = decision
                .approval_required()
                .into_iter()
                .filter(|a| self.options.gate.as_deref() == Some(a.as_str()))
                .collect();
            let context = StepContext {
                tree: &self.tree,
                intent: &self.options.intent,
                offered: &offered,
                approvals: &approvals,
                decision: &decision,
                protocol: &self.case.protocol,
                step: self.steps as usize,
            };
            // What the step changed is the difference to what was already uncommitted: a
            // validator's run may leave files (a lock file) that are not the harness's doing.
            let before: Vec<String> = crate::git::changed_paths(&self.tree)?;
            let report = self.harness.step(&context)?;
            if let Some(tokens) = report.tokens {
                self.tokens += tokens;
            }
            writeln!(
                out,
                "step {}: {} ({})",
                self.steps,
                report.action,
                self.harness.name()
            )?;
            writeln!(out, "  said: {}", report.summary)?;
            let changed: Vec<String> = crate::git::changed_paths(&self.tree)?
                .into_iter()
                .filter(|p| !p.starts_with(CASES_DIR) && !before.contains(p))
                .collect();
            if approvals.contains(&report.action) {
                if !changed.is_empty() {
                    crate::git::discard_all(&self.tree)?;
                    writeln!(
                        out,
                        "  refused: {} was requested with {} changed path(s); the change was discarded",
                        report.action,
                        changed.len()
                    )?;
                    return Ok(Stop::StepRefused(format!(
                        "{} requested with changes",
                        report.action
                    )));
                }
                writeln!(out, "  effect: none; {} needs an approval", report.action)?;
                return Ok(Stop::ApprovalRequired(vec![report.action]));
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
                crate::git::discard_all(&self.tree)?;
                writeln!(out, "  refused: {why}; the change was discarded")?;
                return Ok(Stop::StepRefused(why));
            }
            if changed.is_empty() {
                writeln!(out, "  effect: no file changed")?;
                last_idle = Some(state);
                continue;
            }
            last_idle = None;
            let head = crate::git::commit_all(
                &self.tree,
                &format!("{}: {}", report.action, report.summary),
            )?;
            self.case.set_revision("implementation", &head)?;
            writeln!(
                out,
                "  effect: committed {} path(s); implementation is now {}",
                changed.len(),
                &head[..8.min(head.len())]
            )?;
        }
    }
}
