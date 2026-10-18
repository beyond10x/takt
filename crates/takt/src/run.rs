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

use std::collections::{BTreeMap, BTreeSet};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use anyhow::{Context as _, Result, bail};

use crate::case::{CASES_DIR, CaseDir, KEPT_MOVE, KEPT_TEST_RESULT};
use crate::events::{BY_STORE, BY_TAKT, Event, Events};
use crate::frontier::{Frontier, IMPLEMENTED, Story, TEST_RESULT, Take, Unit};
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
    /// No action is left to take and none needs an approval: none is admissible, or the only
    /// admissible ones read and the harness keeps no state between steps.
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
    /// The run waited on the forge (checks still running, a pull request's head behind the head
    /// pushed) and may wait no longer: its step or hour budget is spent, or it has neither to
    /// wait in. Nothing is recorded for what is awaited; the case stays open, and the next run
    /// resumes it and observes again.
    Waiting {
        /// The actions whose observation waits on the forge.
        on: Vec<String>,
        /// The budget spent; `None` for a run with no step or hour budget, which does not wait.
        budget: Option<Box<Stop>>,
    },
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
            Stop::Waiting { on, budget } => match budget {
                Some(budget) => write!(f, "Waiting ({}; {budget})", on.join(", ")),
                None => write!(f, "Waiting ({}; no step or hour budget)", on.join(", ")),
            },
        }
    }
}

/// The outcome of a case whose story is recorded and moved to `implemented` in the store.
pub const ACCEPTED: &str = "accepted";

/// The producer of a record takt writes from a judgement step that records a choice: the
/// record attests the harness's choice, not an observation's outcome.
pub const CHOICE_PRODUCER: &str = "takt.choice";

/// `text` on one line: every run of white space, line breaks among them, as one space. A
/// judgement step's commit message is `<action>: <summary>`, the summary the harness reported;
/// a line of its own there could read as one of takt's (`takt: take`, `takt: approve`) to what
/// reads commit messages.
fn one_line(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// The first eight characters of a revision, as the loop prints one.
fn short(revision: &str) -> &str {
    revision.get(..8).unwrap_or(revision)
}

/// Who took a step, with what its event records of it: a validator, and the revision it
/// observed when it observed; or the harness, the tokens it reported and their components.
enum SteppedBy<'r> {
    Validator(Option<&'r str>),
    Harness(Option<u64>, Option<crate::events::TokenUsage>),
}

/// What [`Run::record_before`] found before a step: no grant authorises it, so nothing was
/// written; the story's records for the granted revision are both written; or one is missing,
/// with why, and the step is not taken.
enum Before {
    Ungranted,
    Recorded,
    Missing(String),
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
        Ok(Self {
            effects,
            produces,
            reads,
            choices,
            blocked_for_good,
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
}

/// One entry of a snapshot: a regular file with its bytes and whether it is executable, a
/// symbolic link with its target, or anything else (a FIFO, a socket, a device) by its kind
/// alone. A link is an entry by itself and is never followed: what it points at may lie outside
/// the tree, and nothing the kernel does for a case reaches outside the tree. Anything else is
/// never opened: reading a FIFO no one writes would block the run for good.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Entry {
    File { bytes: Vec<u8>, executable: bool },
    Link(PathBuf),
    Other(&'static str),
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
/// written below it.
fn restore(dir: &Path, before: &BTreeMap<PathBuf, Entry>, changed: &[PathBuf]) -> Result<()> {
    if std::fs::symlink_metadata(dir).is_ok_and(|meta| meta.file_type().is_symlink()) {
        bail!(
            "the directory {} was replaced by a link; nothing is put back through it",
            dir.display()
        );
    }
    for path in changed {
        let full = dir.join(path);
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
            Some(Entry::Other(_)) | None => {}
        }
    }
    Ok(())
}

/// The directory under the tree that holds the planning store, the project file that says
/// where the store is kept, and the cases: everything in it but the cases is the store's, which
/// no judgement step writes.
const ENGINEERING: &str = ".engineering";

/// The files that decide what pytest collects and how it reports, at any depth: a
/// `conftest.py` can rewrite every outcome. `pyproject.toml`'s `[tool.pytest*]` tables are
/// compared by the manifest scanner ([`pytest_config_changes`]).
const PYTHON_FILES: &[&str] = &[
    "conftest.py",
    "pytest.ini",
    ".pytest.ini",
    "pytest.toml",
    ".pytest.toml",
    "tox.ini",
    "setup.cfg",
];

/// The judge surface every run guards, as a refusal names it.
pub const JUDGE_SURFACE: &str = "`.cargo/**` and `rust-toolchain*` and `build.rs` at any depth, `.github/**`, `.engineering/**` but `.engineering/cases/**`, `conftest.py`, `pytest.ini`, `.pytest.ini`, `pytest.toml`, `.pytest.toml`, `tox.ini` and `setup.cfg` at any depth, `[tool.pytest*]` in `pyproject.toml`, the module and package `python -m` runs, the test command's own script and its build tool's file, and the repository's own `.engineering/judge-surface.yaml` list";

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
    (
        &["sh", "bash", "dash", "zsh", "ksh", "mksh", "ash", "fish"],
        SHELL,
    ),
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
];

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

/// `line` split into words as a POSIX shell splits a command line: white space between words,
/// `'…'` taken as it is, `"…"` with `\` escaping `"`, `\`, `$` and a backquote, `\` outside quotes
/// escaping the next character, and each of `;`, `&`, `|`, `(`, `)`, `<`, `>` a word by itself,
/// so the commands of a list are told apart. Expansions are not made; an unclosed quote ends at
/// the end of the line.
fn shell_words(line: &str) -> Vec<String> {
    let mut words = Vec::new();
    let mut word = String::new();
    let mut started = false;
    let mut chars = line.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '\'' => {
                started = true;
                for c in chars.by_ref() {
                    if c == '\'' {
                        break;
                    }
                    word.push(c);
                }
            }
            '"' => {
                started = true;
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
                word.extend(chars.next());
            }
            ';' | '&' | '|' | '(' | ')' | '<' | '>' => {
                if started {
                    words.push(std::mem::take(&mut word));
                    started = false;
                }
                words.push(c.to_string());
            }
            c if c.is_whitespace() => {
                if started {
                    words.push(std::mem::take(&mut word));
                    started = false;
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

/// Guards every word of `line`, a command line a shell reads, that names a path in the tree,
/// and reads each command of it as a command line of its own.
fn shell_line_paths(
    tree: &Path,
    line: &str,
    dir: Option<&str>,
    named: &mut Vec<String>,
    depth: usize,
) {
    let words = shell_words(line);
    for word in &words {
        named.extend(named_from(tree, dir, word));
    }
    for command in words.split(|word| [";", "&", "|", "(", ")"].contains(&word.as_str())) {
        // A redirection's target is a file the shell opens, named above; the command is the rest.
        let mut kept: Vec<&str> = Vec::new();
        let mut skip = false;
        for word in command {
            if skip {
                skip = false;
                continue;
            }
            if word == "<" || word == ">" {
                skip = true;
                continue;
            }
            kept.push(word);
        }
        command_paths(tree, &kept, dir.map(str::to_owned), named, depth + 1);
    }
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

/// The paths of `tree` the command line `words`, run from `dir`, runs as its own: added to
/// `named`. See [`scripts_of`].
fn command_paths(
    tree: &Path,
    words: &[&str],
    mut dir: Option<String>,
    named: &mut Vec<String>,
    depth: usize,
) {
    if depth > COMMANDS_AT_MOST {
        return;
    }
    let mut at = 0;
    while let Some(program) = words.get(at).copied() {
        named.extend(named_from(tree, dir.as_deref(), program));
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
                            let Some(value) = value else {
                                at += 1;
                                break;
                            };
                            let mut rest = vec![value];
                            rest.extend(&words[at + 1..]);
                            return command_paths(tree, &rest, dir, named, depth + 1);
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
                            at += 1;
                            break;
                        }
                        Some(rest) => {
                            let mut words_of = vec![rest];
                            words_of.extend(&words[at + 1..]);
                            return command_paths(tree, &words_of, dir, named, depth + 1);
                        }
                        None => {}
                    }
                    at += 1;
                    continue;
                }
                if word.contains('=') {
                    at += 1;
                    continue;
                }
                break;
            }
            continue;
        }
        if let Some(spec) = spec_of(WRAPPERS, &name) {
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
                        return command_line(tree, words, from, dir.as_deref(), named, depth);
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
            // `-m pkg.module` imports `pkg` from the directory it runs in first: `pkg.py` and
            // the package `pkg/` there replace the installed one.
            for module in scanned.modules {
                let first = module.split('.').next().unwrap_or(module);
                named.extend(named_from(tree, dir.as_deref(), &format!("{first}.py")));
                named.extend(
                    named_from(tree, dir.as_deref(), first).map(|package| format!("{package}/**")),
                );
            }
            match scanned.end {
                End::Stop => {}
                End::Command(from) => {
                    command_line(tree, words, from, dir.as_deref(), named, depth);
                }
                End::Operand => {
                    let mut script = scanned.at;
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
                                );
                            }
                        }
                    }
                    if let Some(script) = words.get(script) {
                        named.extend(named_from(tree, dir.as_deref(), script));
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
            for file in files {
                named.extend(named_from(tree, dir.as_deref(), file));
            }
            return;
        }
        // A program in none of the tables: what it reads is not known, so every word of the
        // rest that names a path in the tree is guarded, as written and as a shell would unquote
        // it, an option's value after `=` among them. Whether the path is there yet does not
        // matter: a step could make it.
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
            for unquoted in shell_words(word) {
                named.extend(named_from(tree, dir.as_deref(), &unquoted));
            }
        }
        return;
    }
}

/// The command line an option such as `sh -c` or `flock -c` runs: the word at `from`, after a
/// `--` that ends the options, read as a shell reads it ([`shell_line_paths`]).
fn command_line(
    tree: &Path,
    words: &[&str],
    mut from: usize,
    dir: Option<&str>,
    named: &mut Vec<String>,
    depth: usize,
) {
    if words.get(from) == Some(&"--") {
        from += 1;
    }
    // The command line is the one word at `from`; the words after it are its `$0` and
    // arguments. A test command is split at white space, so a line meant to be quoted
    // (`sh -c 'a; b'`) arrives in pieces: each piece, and the pieces joined, are read as well.
    let rest = words.get(from..).unwrap_or_default();
    for line in rest {
        shell_line_paths(tree, line, dir, named, depth);
    }
    if rest.len() > 1 {
        shell_line_paths(tree, &rest.join(" "), dir, named, depth);
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
#[must_use]
pub fn scripts_of(words: &[String], tree: &Path) -> Vec<String> {
    let words: Vec<&str> = words.iter().map(String::as_str).collect();
    let mut named = Vec::new();
    command_paths(tree, &words, Some(String::new()), &mut named, 0);
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
        // What [`scripts_of`] names with a wildcard (a module's package, `pkg/**`) is guarded
        // as a glob, everything under it with it; the rest each by itself.
        let (globs, scripts): (Vec<String>, Vec<String>) = scripts_of(runs, tree)
            .into_iter()
            .partition(|path| path.contains(['*', '?']));
        Self {
            scripts: scripts.into_iter().collect(),
            store: true,
            globs: globs.iter().map(|glob| fold(glob)).collect(),
        }
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
            || folded == ".github"
            || folded.starts_with(".github/")
            || parts.contains(&".cargo")
            || name.starts_with("rust-toolchain")
            || name == "build.rs"
            || PYTHON_FILES.contains(&name)
            || self.scripts.iter().any(|script| fold(script) == folded)
            || self.globs.iter().any(|glob| {
                let glob: Vec<&str> = glob.split('/').collect();
                (1..=parts.len()).any(|len| glob_matches(&glob, &parts[..len]))
            })
    }

    /// Whether a validator or the guarded command would read through a link at `link`: the link
    /// is, or is on the way to, the cargo configuration (`.cargo/` at any depth), the CI
    /// workflows (`.github/workflows/`), the store's directories and project file (while the
    /// store is on the surface), a manifest, a build script, a toolchain file, or the test
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
            || folded == fold(JUDGE_LIST)
            || self.scripts.iter().any(|script| is_or_holds(&fold(script)))
            // A path the repository's own list (or a module's package) names: the link is on
            // it, or on the way towards it.
            || self.globs.iter().any(|glob| {
                let glob: Vec<&str> = glob.split('/').collect();
                (1..=parts.len()).any(|len| glob_matches(&glob, &parts[..len]))
                    || glob_towards(&glob, &parts)
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
            if [ENGINEERING, ".cargo", ".github", "build.rs"].contains(&folded.as_str())
                || folded.starts_with("rust-toolchain")
                || PYTHON_FILES.contains(&folded.as_str())
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

    /// The surface of a run whose validators run `runs`, and what is on it in `tree` now.
    fn read(
        runs: &[String],
        globs: Vec<String>,
        tree: &Path,
    ) -> Result<(Self, BTreeMap<PathBuf, Entry>)> {
        let surface = Self::new(runs, tree).with_globs(globs);
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
            restore(&real, kept, &again)?;
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
#[derive(Debug, Clone, PartialEq, Eq)]
struct Outside(BTreeMap<PathBuf, (PathBuf, Option<Entry>)>);

impl Outside {
    /// The paths [`Outside`] reads for `tree`, each with its anchor: the directory above the
    /// tree it is in, or the parent of `$CARGO_HOME`. Every part between the anchor and the path
    /// (`.cargo`, `$CARGO_HOME` itself) is a path of its own too, so a link made there is seen
    /// as the link it is.
    fn paths(tree: &Path) -> Vec<(PathBuf, PathBuf)> {
        let mut paths = BTreeMap::new();
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
                    paths.insert(above.join(name), above.to_path_buf());
                }
            }
        }
        let home = std::env::var_os("CARGO_HOME")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".cargo")));
        if let Some(home) = home
            && let Some(anchor) = home.parent()
        {
            paths.insert(home.clone(), anchor.to_path_buf());
            paths.insert(home.join("config"), anchor.to_path_buf());
            paths.insert(home.join("config.toml"), anchor.to_path_buf());
            if !std::fs::symlink_metadata(&home).is_ok_and(|meta| meta.file_type().is_symlink())
                && let Ok(entries) = std::fs::read_dir(&home)
            {
                for entry in entries.flatten() {
                    if entry.file_name().to_string_lossy().starts_with("config") {
                        paths.insert(entry.path(), anchor.to_path_buf());
                    }
                }
            }
        }
        paths.into_iter().collect()
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
    fn read(tree: &Path) -> Result<Self> {
        let mut entries = BTreeMap::new();
        for (path, anchor) in Self::paths(tree) {
            let entry = match std::fs::symlink_metadata(&path) {
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
                    std::fs::read_link(&path)
                        .with_context(|| format!("read the link {}", path.display()))?,
                )),
                Ok(meta) if meta.is_file() => Some(Entry::File {
                    bytes: std::fs::read(&path)
                        .with_context(|| format!("read {}", path.display()))?,
                    executable: is_executable(&meta),
                }),
                Ok(meta) if meta.is_dir() => Some(Entry::Other("directory")),
                Ok(meta) => Some(Entry::Other(other_kind(meta.file_type()))),
            };
            entries.insert(path, (anchor, entry));
        }
        Ok(Self(entries))
    }

    /// The paths at which `now` differs from this, in path order: a part of the way (`.cargo`)
    /// before what is under it.
    fn differing(&self, now: &Self) -> Vec<PathBuf> {
        let entry = |outside: &Self, path: &PathBuf| {
            outside.0.get(path).and_then(|(_, entry)| entry.clone())
        };
        self.0
            .keys()
            .chain(now.0.keys())
            .filter(|path| entry(self, path) != entry(now, path))
            .cloned()
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect()
    }

    /// Puts each path where the tree's outside now differs from this back, in path order: what
    /// was created is removed (a link as the link itself), what was changed or removed is written
    /// again (a link made again, a directory made again empty). Nothing is ever written or
    /// removed through a link: a path whose way crosses one, checked part by part, is left as it
    /// is and answered, for the refusal to name.
    fn put_back(&self, tree: &Path) -> Result<Vec<PathBuf>> {
        let mut left = Vec::new();
        for path in self.differing(&Self::read(tree)?) {
            let anchor = match self.0.get(&path) {
                Some((anchor, _)) => anchor.clone(),
                None => continue,
            };
            if Self::link_on_the_way(&anchor, &path).is_some() {
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
            match self.0.get(&path).and_then(|(_, entry)| entry.clone()) {
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
                Some(Entry::Other(_)) | None => {}
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
    let mut command = std::process::Command::new("cargo");
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

/// How a refusal names what a step changed on the judge surface: the store first, as the store's,
/// then the rest.
fn surface_refusal(action: &str, touched: &[String]) -> String {
    let (store, other): (Vec<&String>, Vec<&String>) = touched
        .iter()
        .partition(|path| JudgeSurface::is_store(path));
    let join = |paths: &[&String]| {
        paths
            .iter()
            .map(|path| path.as_str())
            .collect::<Vec<_>>()
            .join(", ")
    };
    let mut parts = Vec::new();
    if !store.is_empty() {
        parts.push(format!(
            "wrote into the planning store, which only takt writes once a case is accepted: {}",
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

/// Discards everything a judgement step did, back to `checkpoint`, and puts the cases directory
/// back to `kept` once more: a reset can remove a case file the step had staged.
fn undo_step(
    tree: &Path,
    checkpoint: &crate::git::Checkpoint,
    cases: &Path,
    kept: &BTreeMap<PathBuf, Entry>,
) -> Result<()> {
    checkpoint
        .restore(tree)
        .context("discard the step's change")?;
    let again = differing(kept, &snapshot(cases)?);
    if !again.is_empty() {
        restore(cases, kept, &again)?;
    }
    Ok(())
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
    let cases = tree.join(CASES_DIR);
    if !cases.is_dir() {
        return Ok(Vec::new());
    }
    let mut dirs: Vec<PathBuf> = std::fs::read_dir(&cases)
        .with_context(|| format!("read {}", cases.display()))?
        .map(|entry| entry.map(|e| e.path()))
        .collect::<std::io::Result<Vec<_>>>()?;
    dirs.sort();
    let mut found = Vec::new();
    for dir in dirs {
        if !dir.join("case.yaml").is_file() {
            continue;
        }
        if let Some(story) = crate::case::story_of(&dir)? {
            let id = dir
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_default();
            found.push((id, story));
        }
    }
    Ok(found)
}

/// Whether the tree is on a case's branch, `takt/<id>`: where [`to_base`] moves it, or refuses.
fn on_a_case_branch(tree: &Path) -> Result<bool> {
    Ok(crate::git::current_branch(tree)?
        .is_some_and(|branch| branch.starts_with(crate::case::BRANCH_PREFIX)))
}

/// Puts the tree on the base of the case whose branch it is on, so that the store is read as
/// the base has it, never as a case's branch, which carries a model's commits, has it. A tree
/// on any other branch, or detached, stays where it is. Refused when the case's base is not
/// known, or when the tree has uncommitted work, which the checkout would carry along.
fn to_base(tree: &Path) -> Result<()> {
    let Some(branch) = crate::git::current_branch(tree)? else {
        return Ok(());
    };
    let Some(case) = branch.strip_prefix(crate::case::BRANCH_PREFIX) else {
        return Ok(());
    };
    let Some(base) = crate::case::Branch::read(&tree.join(CASES_DIR).join(case))
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
    /// How long the run waits on the forge before it observes again: [`Run::POLL`] unless set.
    pub poll: Duration,
    /// The revision the running binary was built from, as its `--version` prints it, recorded
    /// on the `opened` line; none unless the caller sets it, since this library does not know
    /// the build it is linked into.
    pub binary: Option<String>,
    /// The planning store a story's run records its test result and its move in; none until
    /// [`Run::record_on`] gives one, and only a story's run writes to it.
    store: Option<&'a dyn Frontier>,
    started: Instant,
    options: RunOptions,
    /// The files outside the tree that decide how cargo and rustup build and run the tests
    /// ([`Outside`]), as the run found them when it began.
    outside: Outside,
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
        // What the step choice will refuse is refused here, before a case directory or a
        // branch exists for it.
        Shape::of(&protocol, &options.inputs)?;
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
            poll: Self::POLL,
            binary: None,
            store: None,
            started: Instant::now(),
            outside: Outside::read(tree)?,
            options,
        })
    }

    /// Gives the run the planning store `store`: a story's run records there the `test_result`
    /// pass record a validator of the case wrote, at the record's own `observed_at`, and moves
    /// the story to `implemented`, each a commit on the case's branch
    /// ([`crate::frontier::write_on_case_branch`]), before the step the case's grant authorises
    /// or, without one, once the case terminates `accepted`; a refusal is relayed and the stop
    /// is unchanged. A run on an intent has no story and writes nothing there.
    pub fn record_on(&mut self, store: &'a dyn Frontier) {
        self.store = Some(store);
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
            poll: Self::POLL,
            binary: None,
            store: None,
            started: Instant::now(),
            outside: Outside::read(tree)?,
            options,
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
        if let Some((case_id, story_id)) = Self::next_to_resume(tree, frontier, out)? {
            return Self::resumed(tree, options, &case_id, &story_id, harness, validators, out)
                .map(Some);
        }
        Self::take_next(tree, options, frontier, harness, validators, out)
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
    /// case already records is not taken again: it is skipped with a line on `out`. `None`,
    /// with no case opened, when nothing else is ready, such a skipped case included.
    pub fn take_next(
        tree: &Path,
        options: RunOptions,
        frontier: &dyn Frontier,
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
            let mut run = Self::open_unit(
                tree,
                options.clone(),
                Unit::Story(story),
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
                    let head = match crate::git::commit_paths(tree, &changed, &take_subject(&id)) {
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
    fn resumed(
        tree: &Path,
        options: RunOptions,
        case_id: &str,
        story_id: &str,
        harness: &'a dyn Harness,
        validators: &'a [Box<dyn Validator>],
        out: &mut dyn Write,
    ) -> Result<Self> {
        let run = Self::resume(tree, options, case_id, harness, validators)?;
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
        None
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
    /// meanwhile; it lets go at every stop and on error.
    pub fn run(&mut self, out: &mut dyn Write) -> Result<Stop> {
        self.case.hold()?;
        let events = Events::new(&self.case.dir);
        // A panic in the loop (a harness or a validator that broke) is recorded as the run's
        // `error` line and the case let go before the panic goes on: the record ends, and the
        // case is not left held.
        let outcome = match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            self.run_held(&events, out)
                .and_then(|stop| self.close(&events, &stop, out).map(|()| stop))
        })) {
            Ok(outcome) => outcome,
            Err(panic) => {
                let what = panic
                    .downcast_ref::<&str>()
                    .map(|text| (*text).to_owned())
                    .or_else(|| panic.downcast_ref::<String>().cloned())
                    .unwrap_or_else(|| "a panic with no message".to_owned());
                let _ = events.emit(
                    self.steps,
                    &Event::Error {
                        error: format!("the run panicked: {what}"),
                    },
                );
                let _ = self.case.release();
                std::panic::resume_unwind(panic);
            }
        };
        let outcome = match outcome {
            Ok(stop) => Ok(stop),
            Err(error) => Err(
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
            ),
        };
        let released = self.case.release();
        let stop = outcome?;
        released?;
        Ok(stop)
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
        events.emit(
            self.steps,
            &Event::Stop {
                stop: stop.to_string(),
                steps: self.steps,
                tokens: self.tokens,
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
    /// current revision of its subject: its path in the repository and its own `observed_at`.
    /// A record counts only when an `evidence` line of the case's record, which the kernel
    /// writes and a step may not, names it under the case's `evidence/`: a record put there by
    /// any other hand is never one the run observed. `None` when there is none.
    fn observed_pass(&self, recorded: &[serde_json::Value]) -> Result<Option<(String, String)>> {
        let mine = Path::new(CASES_DIR).join(&self.case.id).join("evidence");
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
            let text = std::fs::read_to_string(&full)
                .with_context(|| format!("read the record {}", full.display()))?;
            let record: serde_json::Value = serde_json::from_str(&text)
                .with_context(|| format!("read the record {}", full.display()))?;
            let current = record["subject"]
                .as_str()
                .and_then(|subject| self.case.revision(subject));
            if record["kind"] == TEST_RESULT
                && record["result"] == "pass"
                && current.is_some()
                && record["subject_revision"].as_str() == current.as_deref()
                && let Some(at) = record["observed_at"].as_str()
            {
                return Ok(Some((path.to_owned(), at.to_owned())));
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
    /// the tokens it reported and their components; which one, the action, what it reported.
    fn stepped(
        &self,
        events: &Events,
        by: SteppedBy<'_>,
        name: &str,
        action: &str,
        summary: &str,
    ) -> Result<()> {
        let (by, tokens, usage, revision) = match by {
            SteppedBy::Validator(revision) => ("validator", None, None, revision),
            SteppedBy::Harness(tokens, usage) => ("harness", tokens, usage, None),
        };
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
            },
        )
    }

    /// The loop, the case held.
    fn run_held(&mut self, events: &Events, out: &mut dyn Write) -> Result<Stop> {
        let inputs: Vec<String> = self
            .options
            .inputs
            .iter()
            .map(|(k, v)| format!("{k}={v}"))
            .collect();
        events.emit(
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
            },
        )?;
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
        // One sequence per case, continued across its runs: no decision file is overwritten,
        // so the latest by name is the latest written, and a record id, numbered by the same
        // sequence, never repeats.
        let mut seq = self.case.evaluations()?;
        let mut last_idle: Option<String> = None;
        let shape = Shape::of(&self.case.protocol, &self.options.inputs)?;
        let keeps_state = self.harness.keeps_state();
        // The validators that observed without writing a record, each with the revision it
        // observed at: not due again there, because the next observation would end the same
        // way; a commit moves the revision and makes them due again.
        let mut observed_without_record: BTreeSet<(String, String)> = BTreeSet::new();
        // Of those, the ones whose observation waits on the forge, as `(name, action,
        // revision)`: observed again after a pause, within the budget, and meanwhile no
        // judgement step is offered, because what is missing is not the harness's to make.
        let mut awaited: Vec<(String, String, String)> = Vec::new();
        // Whether the story's records were written before a granted step this run.
        let mut recorded_before = false;
        let waiting = |awaited: &[(String, String, String)], budget: Option<Stop>| {
            let mut on: Vec<String> = Vec::new();
            for (_, action, _) in awaited {
                if !on.contains(action) {
                    on.push(action.clone());
                }
            }
            Stop::Waiting {
                on,
                budget: budget.map(Box::new),
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
                return Ok(if awaited.is_empty() {
                    stop
                } else {
                    waiting(&awaited, Some(stop))
                });
            }
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
            if let Some(validator) = due {
                // The story's records in the store, once a run, just before the step its grant
                // authorises: they ride to the base with the merge, and without both of them the
                // step is not taken.
                if !recorded_before {
                    match self.record_before(events, validator.action(), out)? {
                        Before::Ungranted => {}
                        Before::Recorded => recorded_before = true,
                        Before::Missing(why) => {
                            self.refused(events, BY_TAKT, &why)?;
                            writeln!(out, "  refused: {why}")?;
                            return Ok(Stop::StepRefused(why));
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
                    return Ok(Stop::StepRefused(why));
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
                let outside_kept = Outside::read(&self.tree)?;
                let outside_moved = self.outside.differing(&outside_kept);
                if !outside_moved.is_empty() {
                    let why = format!(
                        "{} would observe {} while what cargo or rustup reads outside the tree differs from what the run found when it began: {}",
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
                    return Ok(Stop::StepRefused(why));
                }
                let checkpoint =
                    crate::git::Checkpoint::take_guarding(&self.tree, &self.kept_branches())?;
                let cases = self.tree.join(CASES_DIR);
                let kept = snapshot(&cases)?;
                let globs = self.judge_list()?.unwrap_or_default();
                let (surface, surface_kept) = JudgeSurface::read(&self.runs(), globs, &self.tree)?;
                let observed = validator
                    .observe_within(&self.tree, &revision, seq, self.time_left())
                    .with_context(|| format!("{} ({})", validator.action(), validator.name()));
                let returned = Instant::now();
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
                // What the observation left on the judge surface; a read that fails (a file
                // the run may not read) discards everything it did before the run ends on the
                // error.
                let on_surface = match surface.snapshot(&self.tree) {
                    Ok(now) => differing(&surface_kept, &now),
                    Err(error) => {
                        let undone = undo_step(&self.tree, &checkpoint, &cases, &kept)
                            .and_then(|()| surface.put_back(&self.tree, &surface_kept));
                        return Err(match undone {
                            Ok(()) => error.context(format!(
                                "{}: what the observation left on the judge surface could not be read; everything it changed was discarded",
                                validator.action()
                            )),
                            Err(undo) => error.context(format!(
                                "{}: what the observation left on the judge surface could not be read, and its change could not be discarded: {undo:#}",
                                validator.action()
                            )),
                        });
                    }
                };
                let touched = differing(&kept, &snapshot(&cases)?);
                let own: Vec<PathBuf> = match self.case.dir.file_name() {
                    Some(id) => validator
                        .writes()
                        .iter()
                        .map(|path| Path::new(id).join(path))
                        .collect(),
                    None => Vec::new(),
                };
                let mut foreign: Vec<String> = touched
                    .iter()
                    .filter(|path| !own.contains(path))
                    .map(|path| Path::new(CASES_DIR).join(path).display().to_string())
                    .collect();
                let change = checkpoint.change(&self.tree)?;
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
                            .filter(|path| surface.contains(path))
                            .cloned(),
                    )
                {
                    if !outside.contains(&path) {
                        outside.push(path);
                    }
                }
                outside.extend(
                    outside_kept
                        .differing(&Outside::read(&self.tree)?)
                        .iter()
                        .map(|path| path.display().to_string()),
                );
                if !foreign.is_empty() || !outside.is_empty() {
                    restore(&cases, &kept, &touched)?;
                    undo_step(&self.tree, &checkpoint, &cases, &kept)?;
                    surface.put_back(&self.tree, &surface_kept)?;
                    let left = outside_kept.put_back(&self.tree)?;
                    foreign.extend(outside);
                    foreign.extend(left.iter().map(|path| {
                        format!("not put back, its way crosses a link: {}", path.display())
                    }));
                    let mut why = format!(
                        "{} wrote what its validator does not: {}",
                        validator.action(),
                        foreign.join(", ")
                    );
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
                    return Ok(Stop::StepRefused(why));
                }
                // An observation that failed when the hour budget ended, its child killed by it,
                // records nothing, and the run stops on the budget.
                if let Err(error) = &observed
                    && self.deadline().is_some_and(|deadline| returned >= deadline)
                    && let Some(h) = self.options.hours
                {
                    restore(&cases, &kept, &touched)?;
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
                    return Ok(Stop::Hours(h));
                }
                let (observation, waits) = observed?;
                self.stepped(
                    events,
                    SteppedBy::Validator(Some(&revision)),
                    validator.name(),
                    validator.action(),
                    &observation.summary,
                )?;
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
                    let written = self.case.add_evidence(record)?;
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
                        },
                    )?;
                    writeln!(
                        out,
                        "  evidence: {} {}",
                        record.kind,
                        record.result.as_deref().unwrap_or("-")
                    )?;
                }
                // A validator that wrote no record (the gate was unknown) is not due again at
                // this revision. The pass counts as idle only when nothing else can move the
                // decision, no judgement step offered, no other validator due and none waiting
                // on the forge: then the next pass would see the same decision, and the run
                // stops there as no progress. One that waits on the forge is due again after
                // the pause below.
                awaited.retain(|(name, _, _)| name != validator.name());
                if observation.records.is_empty() {
                    observed_without_record.insert((validator.name().to_owned(), revision.clone()));
                    if waits {
                        awaited.push((
                            validator.name().to_owned(),
                            validator.action().to_owned(),
                            revision,
                        ));
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
                    last_idle =
                        (offered.is_empty() && !another_due && awaited.is_empty()).then_some(state);
                } else {
                    last_idle = None;
                }
                continue;
            }
            // No validator is due, and one waits on the forge whose action is still admissible:
            // the run pauses and observes it again, offering no judgement step meanwhile. A run
            // with no step or hour budget has nothing to wait in and stops waiting at once; the
            // case stays open for the next run.
            awaited.retain(|(_, action, _)| admissible.contains(action));
            if !awaited.is_empty() {
                let Some(pause) = self.pause() else {
                    return Ok(waiting(&awaited, None));
                };
                let on: Vec<&str> = awaited
                    .iter()
                    .map(|(_, action, _)| action.as_str())
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
                std::thread::sleep(pause);
                for (name, _, revision) in &awaited {
                    observed_without_record.remove(&(name.clone(), revision.clone()));
                }
                continue;
            }
            // No judgement step when only a grant stands between the case and its outcome:
            // every claim still open is decided by evidence only an action waiting for an
            // approval produces. The run stops there at once and spends nothing on asking.
            let waiting_for_grant = decision.approval_required();
            if !waiting_for_grant.is_empty()
                && let Some(behind) = shape.only_behind(&decision, &waiting_for_grant)
            {
                return Ok(Stop::ApprovalRequired(behind));
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
            let choices: Vec<String> = offered
                .iter()
                .filter(|a| !shape.choices_of(a).is_empty())
                .cloned()
                .collect();
            let mut context = StepContext {
                tree: &self.tree,
                intent: &self.options.intent,
                offered: &offered,
                approvals: &approvals,
                choices: &choices,
                decision: &decision,
                protocol: &self.case.protocol,
                step: self.steps as usize,
                time_left: None,
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
            let cases = self.tree.join(CASES_DIR);
            let kept = snapshot(&cases)?;
            // The judge surface likewise ([`JudgeSurface`]): the planning store and everything
            // else under `.engineering/` but the cases (the project file that says where the
            // store is kept, the gates and the assertions project file among it), whose
            // artifacts and evidence the store's lifecycle moves a story on and which only
            // takt's own writes change once a case is accepted ([`Run::record_on`]); and what
            // decides the validators, which a step that changed it would decide its own
            // verdict with: the cargo configuration, the toolchain, build scripts, the CI
            // workflows, pytest's configuration, the test command's own script and its build
            // tool's file, and what the repository's own list adds. Matched
            // whatever the case. A step whose change git sees there (a file, a link, a mode, a
            // rename), or that left anything there the snapshot sees differ (a FIFO, which git
            // does not list, a file in an ignored directory), is refused, and put back to the
            // snapshot taken here.
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
                    return Ok(Stop::StepRefused(why));
                }
            };
            let (surface, surface_kept) = JudgeSurface::read(&runs, globs, &self.tree)?;
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
                return Ok(Stop::StepRefused(why));
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
            let outside_before = Outside::read(&self.tree)?;
            // Every branch and tag is guarded beside `HEAD` and the case's branch: the case's
            // base, and a sibling case's branch, which a step could otherwise move under it; a
            // branch another worktree has checked out is that session's, but never the case's
            // base or its own branch.
            let checkpoint =
                crate::git::Checkpoint::take_guarding(&self.tree, &self.kept_branches())?;
            // What is left of the hour budget is read last, just before the call, so the step
            // ends by the run's end and not by the time its preparation took after it.
            context.time_left = self.time_left();
            // A harness that panics is put back like one that failed, then the panic goes on
            // to `run`, which records it and lets go of the case.
            let stepped = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                self.harness.step(&context)
            }));
            let returned = Instant::now();
            // What the step left running (a writer in a process group of its own) is killed
            // before anything it did is looked at, so nothing lands after the step is judged.
            let swept = crate::forge::sweep_left();
            // What git runs or reads as configuration first: every git call after this one
            // would read what the step planted there. Nothing in it ends early; a path it could
            // not put back is named in the refusal below.
            let git_dir = checkpoint.put_back_git_dir(&self.tree);
            let tree = self.tree.clone();
            let undo = || -> Result<()> {
                undo_step(&tree, &checkpoint, &cases, &kept)?;
                surface.put_back(&tree, &surface_kept)?;
                outside_before.put_back(&tree).map(|_| ())
            };
            let stepped = match stepped {
                Ok(stepped) => stepped,
                Err(panic) => {
                    if let Err(undo) = undo() {
                        eprintln!(
                            "step {}: the harness panicked, and the step's change could not be discarded: {undo:#}",
                            self.steps
                        );
                    }
                    std::panic::resume_unwind(panic);
                }
            };
            // A step whose leftovers could not all be killed is not judged: it is discarded
            // whole and the run ends on the error.
            if let Err(error) = swept {
                return Err(match undo() {
                    Ok(()) => error.context(format!(
                        "step {}: what the step left running could not be killed; everything it changed was discarded",
                        self.steps
                    )),
                    Err(undo) => error.context(format!(
                        "step {}: what the step left running could not be killed, and its change could not be discarded: {undo:#}",
                        self.steps
                    )),
                });
            }
            // What the step left, read now; a read that fails (a file the run may not read)
            // discards the step before the run ends on the error.
            let seen =
                (|| -> Result<(Vec<PathBuf>, Vec<String>)> {
                    let touched = differing(&kept, &snapshot(&cases)?);
                    let mut surface_touched: BTreeSet<String> =
                        differing(&surface_kept, &surface.snapshot(&tree)?)
                            .iter()
                            .map(|path| path.display().to_string())
                            .collect();
                    surface_touched.extend(crate::git::changed_paths(&tree)?.into_iter().filter(
                        |path| surface.contains(path) && !surface_dirty_before.contains(path),
                    ));
                    Ok((touched, surface_touched.into_iter().collect()))
                })();
            let (touched, surface_touched) = match seen {
                Ok(seen) => seen,
                Err(error) => {
                    return Err(match undo() {
                        Ok(()) => error.context(format!(
                            "step {}: what the step left could not be read; everything it changed was discarded",
                            self.steps
                        )),
                        Err(undo) => error.context(format!(
                            "step {}: what the step left could not be read, and its change could not be discarded: {undo:#}",
                            self.steps
                        )),
                    });
                }
            };
            if !touched.is_empty() {
                restore(&cases, &kept, &touched)?;
            }
            // A step that returned when the hour budget ended, failed (killed by it) or not, is
            // discarded whole like any failed step, recorded as a refusal, and the run stops on
            // the budget; the tokens it reported are counted. One that failed by itself before
            // then failed as any other.
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
                if let Err(undo) = undo() {
                    return Err(anyhow::anyhow!(
                        "step {}: the hour budget ran out during the step ({ended}), and the step's change could not be discarded: {undo:#}",
                        self.steps
                    ));
                }
                let why = format!(
                    "step {}: the hour budget ran out during the step; everything the step changed was discarded: {ended}",
                    self.steps
                );
                // Written after the cases directory was put back, which would take a line
                // written before it along.
                self.refused(events, BY_TAKT, &why)?;
                writeln!(out, "{why}")?;
                return Ok(Stop::Hours(h));
            }
            let report = match stepped {
                Ok(report) => report,
                Err(error) => {
                    return Err(match undo() {
                        Ok(()) => error.context(format!(
                            "step {}: the harness failed; everything the step changed was discarded",
                            self.steps
                        )),
                        Err(undo) => error.context(format!(
                            "step {}: the harness failed, and the step's change could not be discarded: {undo:#}",
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
                    SteppedBy::Harness(report.tokens, harness.last_usage()),
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
                undo()?;
                record_step(self)?;
                return Err(anyhow::Error::new(error).context(format!(
                    "step {}: {} could not be printed; the change was discarded",
                    self.steps, report.action
                )));
            }
            let change = match checkpoint.change(&self.tree) {
                Ok(change) => change,
                Err(error) => {
                    undo()?;
                    record_step(self)?;
                    return Err(error.context(format!(
                        "step {}: what {} changed could not be read; the change was discarded",
                        self.steps, report.action
                    )));
                }
            };
            let changed = &change.paths;
            if !touched.is_empty() {
                undo()?;
                let paths: Vec<String> = touched
                    .iter()
                    .map(|p| Path::new(CASES_DIR).join(p).display().to_string())
                    .collect();
                let why = format!(
                    "{} wrote into the cases directory: {}",
                    report.action,
                    paths.join(", ")
                );
                record_step(self)?;
                self.refused(events, BY_TAKT, &why)?;
                writeln!(
                    out,
                    "  refused: {why}; the cases directory was put back and the change discarded"
                )?;
                return Ok(Stop::StepRefused(why));
            }
            if !surface_touched.is_empty() {
                undo()?;
                let why = surface_refusal(&report.action, &surface_touched);
                record_step(self)?;
                self.refused(events, BY_TAKT, &why)?;
                writeln!(
                    out,
                    "  refused: {why}; the judge surface was put back and the change discarded"
                )?;
                return Ok(Stop::StepRefused(why));
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
                            changed_keys
                                .extend(keys.into_iter().map(|key| format!("{path}: {key}")));
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
                    undo()?;
                    record_step(self)?;
                    return Err(error.context(format!(
                        "step {}: the cargo manifests {} changed could not be read; the change was discarded",
                        self.steps, report.action
                    )));
                }
            };
            if !manifests.is_empty() {
                undo()?;
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
                self.refused(events, BY_TAKT, &why)?;
                writeln!(out, "  refused: {why}; the change was discarded")?;
                return Ok(Stop::StepRefused(why));
            }
            // What cargo and rustup read outside the tree ([`Outside`]): a step that changed it
            // is refused, and what it created there removed, what it changed or removed put back.
            let outside_changed = match Outside::read(&self.tree) {
                Ok(now) => outside_before.differing(&now),
                Err(error) => {
                    undo()?;
                    record_step(self)?;
                    return Err(error.context(format!(
                        "step {}: what cargo and rustup read outside the tree could not be read after {}; the change was discarded",
                        self.steps, report.action
                    )));
                }
            };
            if !outside_changed.is_empty() {
                undo()?;
                // What is still not as it was is behind a link, never written through.
                let left = outside_before.differing(&Outside::read(&self.tree)?);
                let mut why = format!(
                    "{} changed what cargo or rustup reads outside the tree: {}",
                    report.action,
                    outside_named(&outside_changed)
                );
                if !left.is_empty() {
                    why.push_str(&format!(
                        "; not put back, its way crosses a link: {}",
                        outside_named(&left)
                    ));
                }
                record_step(self)?;
                self.refused(events, BY_TAKT, &why)?;
                writeln!(
                    out,
                    "  refused: {why}; it was put back and the change discarded"
                )?;
                return Ok(Stop::StepRefused(why));
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
                    what.push(format!("wrote {}", change.ignored_added.join(", ")));
                }
                if !change.ignored_changed.is_empty() {
                    what.push(format!("rewrote {}", change.ignored_changed.join(", ")));
                }
                if !change.ignored_removed.is_empty() {
                    what.push(format!("removed {}", change.ignored_removed.join(", ")));
                }
                let mut why = format!(
                    "{action} {} ignored by git, which no commit would carry",
                    what.join(" and ")
                );
                if !change.ignored_lost.is_empty() {
                    why.push_str(&format!(
                        "; nothing was kept to put back of {}",
                        change.ignored_lost.join(", ")
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
            if let Some(why) = overreach {
                undo()?;
                record_step(self)?;
                self.refused(events, BY_TAKT, &why)?;
                writeln!(out, "  refused: {why}; the change was discarded")?;
                return Ok(Stop::StepRefused(why));
            }
            if approvals.contains(&report.action) {
                if !changed.is_empty() {
                    undo()?;
                    let why = format!("{} requested with changes", report.action);
                    record_step(self)?;
                    self.refused(events, BY_TAKT, &why)?;
                    writeln!(
                        out,
                        "  refused: {} was requested with {} changed path(s); the change was discarded",
                        report.action,
                        changed.len()
                    )?;
                    return Ok(Stop::StepRefused(why));
                }
                record_step(self)?;
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
                undo()?;
                record_step(self)?;
                self.refused(events, BY_TAKT, &why)?;
                writeln!(out, "  refused: {why}; the change was discarded")?;
                return Ok(Stop::StepRefused(why));
            }
            // An action whose evidence records a choice ([`Shape`]): takt writes that record,
            // with no result, bound to the revision of each artifact the protocol reads it about,
            // for a step that changed no file. A step that changed one is refused, as a request
            // for an approval with changes is. Taken again where its record is already written,
            // it moves nothing, and the run sees no progress.
            let choices = shape.choices_of(&report.action);
            if !choices.is_empty() {
                if !changed.is_empty() {
                    undo()?;
                    let why = format!(
                        "{} records a choice and changed {} path(s)",
                        report.action,
                        changed.len()
                    );
                    record_step(self)?;
                    self.refused(events, BY_TAKT, &why)?;
                    writeln!(out, "  refused: {why}; the change was discarded")?;
                    return Ok(Stop::StepRefused(why));
                }
                record_step(self)?;
                events.emit(
                    self.steps,
                    &Event::Effect {
                        paths: Vec::new(),
                        revision: None,
                    },
                )?;
                writeln!(out, "  effect: no file changed")?;
                let mut recorded = false;
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
                        let record = crate::evidence::Record::new(
                            id,
                            kind,
                            None,
                            subject,
                            &revision,
                            CHOICE_PRODUCER,
                        );
                        let written = self.case.add_evidence(&record)?;
                        events.emit(
                            self.steps,
                            &Event::Evidence {
                                id: record.id.clone(),
                                kind: record.kind.clone(),
                                result: None,
                                path: written
                                    .strip_prefix(&self.tree)
                                    .unwrap_or(&written)
                                    .display()
                                    .to_string(),
                            },
                        )?;
                        writeln!(out, "  evidence: {kind} -")?;
                        recorded = true;
                    }
                }
                last_idle = (!recorded).then_some(state);
                continue;
            }
            if changed.is_empty() {
                record_step(self)?;
                events.emit(
                    self.steps,
                    &Event::Effect {
                        paths: Vec::new(),
                        revision: None,
                    },
                )?;
                writeln!(out, "  effect: no file changed")?;
                last_idle = Some(state);
                continue;
            }
            last_idle = None;
            // A commit that fails (a hook of the repository's own refused it) leaves nothing
            // of the step behind either.
            let head = match crate::git::commit_paths(
                &self.tree,
                changed,
                &format!("{}: {}", report.action, one_line(&report.summary)),
            ) {
                Ok(head) => head,
                Err(error) => {
                    undo()?;
                    record_step(self)?;
                    return Err(error.context(format!(
                        "step {}: the commit of {}'s change failed; the change was discarded",
                        self.steps, report.action
                    )));
                }
            };
            self.case.set_revision("implementation", &head)?;
            record_step(self)?;
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
        }
    }
}
