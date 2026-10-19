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
/// observed when it observed; or the harness, the tokens it reported, their components and
/// what it reported about the call besides them.
enum SteppedBy<'r> {
    Validator(Option<&'r str>),
    Harness(
        Option<u64>,
        Option<crate::events::TokenUsage>,
        Option<crate::harness::CallReport>,
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
    let all: Vec<PathBuf>;
    let changed = match std::fs::symlink_metadata(dir) {
        Ok(meta) if !meta.is_dir() => {
            std::fs::remove_file(dir).with_context(|| format!("remove {}", dir.display()))?;
            all = before.keys().cloned().collect();
            &all
        }
        _ => changed,
    };
    for path in changed {
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

/// The judge surface every run guards, as a refusal names it.
pub const JUDGE_SURFACE: &str = "`.cargo/**` and `rust-toolchain*` and `build.rs` at any depth, `.github/**`, `.engineering/**` but `.engineering/cases/**`, `conftest.py`, `pytest.ini`, `.pytest.ini`, `pytest.toml`, `.pytest.toml`, `tox.ini`, `setup.cfg`, `sitecustomize.py` and `usercustomize.py` at any depth, `*.pth` at the root and directly in a `site-packages` or `dist-packages` directory, `[tool.pytest*]` in `pyproject.toml`, the module and package `python -m` runs and the `*.pth` files in every directory it imports from (where it runs and what `PYTHONPATH` names), a module, compiled module or package named like one the test command's Python imports from outside the tree (asked when the case opens; the standard library's and the test runners' `unittest`, `pytest`, `_pytest`, `pluggy`, `nose2` when no Python answers) in every directory Python imports from under any program (where it runs, each `PYTHONPATH` entry, a `unittest discover` or `nose2` start and top-level directory, a test file's directory when pytest runs, a Python script's own directory), every executable entry (at the base, before or after the step) directly in a tree directory on `PATH`, the script's or the one takt runs with, the test command's own script, the scripts it runs, read one level deep (every file beside a script deeper than that), and its build tool's file, every Python file, shell file and program of the tree while a line of such a script cannot be read or a build tool's recipe runs, and the repository's own `.engineering/judge-surface.yaml` list";

/// How many scripts deep the guard reads: the test command's own script, and the scripts it
/// runs. A script deeper than that is not read; everything in its directory, and every Python
/// file of the tree, is guarded instead ([`script_paths`]).
const SCRIPTS_AT_MOST: usize = 2;

/// What a line the guard cannot read guards ([`fail_closed`]), since what it runs, and from
/// where, is unknown: every Python module, compiled module and path file of the tree, every
/// shell file, and every program ([`PROGRAMS`]: a file starting with `#!` or with an exec bit).
const FAIL_CLOSED: &[&str] = &[
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
const IMPORTS: &str = "(imports)";

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
fn module_of(name: &str) -> Option<&str> {
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
fn interpreter_of(shebang: &str) -> Option<String> {
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
const FAILS_CLOSED_MARK: &str = "(fails closed) ";

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

/// The file a case keeps its module names in ([`Modules`]), under its directory.
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
/// open of the Python the test command runs, and kept with the case ([`MODULES_FILE`]); the
/// built-in lists ([`STDLIB_MODULES`], [`RUNNER_PACKAGES`]) when no Python answered.
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
fn python_like(name: &str) -> bool {
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
        let mut command = std::process::Command::new(python);
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

    /// The names the case at `case_dir` keeps ([`MODULES_FILE`]); asked of the Python the test
    /// command `runs` in `tree` and kept there when it keeps none.
    pub fn of_case(case_dir: &Path, runs: &[String], tree: &Path) -> Result<Self> {
        let file = case_dir.join(MODULES_FILE);
        if file.is_file() {
            return Ok(Self::read(case_dir));
        }
        let modules = Self::ask(runs, tree);
        std::fs::write(&file, serde_json::to_vec_pretty(&modules)?)
            .with_context(|| format!("write {}", file.display()))?;
        Ok(modules)
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
/// command substitution, a home directory.
fn opaque(word: &str) -> bool {
    word.contains(['$', '`', '\u{1}', '\u{2}', '\u{3}'])
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
            .windows(2)
            .any(|pair| pair[0] == "<" && pair[1] == "<")
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
            for word in &words {
                let word =
                    expand(tree, word, dir.as_deref(), &shell.env).unwrap_or_else(|| word.clone());
                named.extend(named_from(tree, dir.as_deref(), &word));
            }
        }
    }
    for command in words.split(|word| [";", "&", "|", "(", ")"].contains(&word.as_str())) {
        statement(tree, command, shell, named, depth);
    }
}

/// Reads one command of a shell line in `shell`: a redirection's input is guarded, the target
/// of an output is the shell's to write; `NAME=VALUE` before nothing sets the shell's variable
/// ([`assign`]), and before a program that program's; `cd` and `pushd` add the directory they
/// change to; `export` and its kin set; `source` and `.` read the file in the same shell, one
/// level deeper; a compound command's word (`if`, `for`, `{`, [`COMPOUND`]) and code given as
/// text (`eval`, `trap`, `alias`, [`OPAQUE`]) fail closed and the rest is read as a command; any
/// other is a command line of its own, from every directory the shell may be in
/// ([`command_paths`]).
fn statement(
    tree: &Path,
    command: &[String],
    shell: &mut Shell,
    named: &mut Vec<String>,
    depth: usize,
) {
    let dirs = shell.dirs.clone();
    let mut words: Vec<&str> = Vec::new();
    let mut at = 0;
    while let Some(word) = command.get(at) {
        at += 1;
        if word == "<" || word == ">" {
            while command
                .get(at)
                .is_some_and(|next| next == "<" || next == ">")
            {
                at += 1;
            }
            if let Some(target) = command.get(at) {
                if word == "<" {
                    for dir in &dirs {
                        let target = expand(tree, target, dir.as_deref(), &shell.env)
                            .unwrap_or_else(|| target.clone());
                        named.extend(named_from(tree, dir.as_deref(), &target));
                    }
                }
                at += 1;
            }
            continue;
        }
        words.push(word);
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
                    quoted(&command.join(" ")),
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
    while matches!(words.get(at), Some(&("exec" | "command" | "builtin"))) {
        at += 1;
        while words.get(at).is_some_and(|word| word.starts_with('-')) {
            at += 1;
        }
    }
    let rest = &words[at..];
    let Some(&first) = rest.first() else {
        for (name, value) in assignments {
            assign(tree, &dirs, name, value, &mut shell.env, named);
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
                    Some(target) => match expand(tree, target, dir.as_deref(), &shell.env) {
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
        if let Some(path) = named_from(tree, dir.as_deref(), program) {
            script_paths(tree, &path, dir.as_deref(), &env, named, depth, false);
            if shebang_of(&tree.join(&path)).is_some_and(|interpreter| {
                spec_of(INTERPRETERS, &interpreter)
                    .is_some_and(|spec| !spec.short_module.is_empty())
            }) {
                named.push(marked(parent_of(&path), IMPORTS));
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
                            let Some(value) = value else {
                                at += 1;
                                break;
                            };
                            let mut rest = vec![value];
                            rest.extend(&words[at + 1..]);
                            return command_paths(tree, &rest, dir, named, depth + 1, &env);
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
                            return command_paths(tree, &words_of, dir, named, depth + 1, &env);
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
                        named.extend(named_from(tree, import, &format!("{first}.py")));
                        named.extend(named_from(tree, import, &format!("{first}.*")));
                        named.extend(
                            named_from(tree, import, first).map(|package| format!("{package}/**")),
                        );
                    }
                }
                for import in &imports {
                    named.extend(named_from(tree, Some(import.as_str()), "*.pth"));
                }
            }
            match scanned.end {
                End::Stop => {}
                End::Command(from) => {
                    command_line(tree, words, from, dir.as_deref(), named, depth, &env);
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
                                    &env,
                                );
                            }
                        }
                    }
                    if let Some(script) = words.get(script) {
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
            for unquoted in shell_words(word) {
                named.extend(named_from(tree, dir.as_deref(), &unquoted));
            }
        }
        return;
    }
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
    /// workflows (`.github/workflows/`), the store's directories and project file (while the
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
            if [ENGINEERING, ".cargo", ".github", "build.rs"].contains(&folded.as_str())
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
    /// longer executable) is compared with what is at it now.
    fn differing(&self, now: &Self) -> Vec<PathBuf> {
        let entry = |outside: &Self, path: &PathBuf| {
            outside.0.get(path).and_then(|(_, entry, _)| entry.clone())
        };
        let current = |path: &PathBuf| match now.0.get(path) {
            Some((_, entry, _)) => entry.clone(),
            None => Self::entry_at(path).unwrap_or(Some(Entry::Other("unreadable"))),
        };
        self.0
            .keys()
            .chain(now.0.keys())
            .filter(|path| entry(self, path) != current(path))
            .cloned()
            .collect::<BTreeSet<_>>()
            .into_iter()
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
/// discarded: the checkpoint, the cases directory, the judge surface and what lies outside the
/// tree, each as it was just before.
struct Kept<'k> {
    checkpoint: &'k crate::git::Checkpoint,
    cases: &'k Path,
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
    let cases = snapshot(kept.cases).and_then(|now| {
        let again = differing(kept.cases_kept, &now);
        if again.is_empty() {
            Ok(())
        } else {
            restore(tree, kept.cases, kept.cases_kept, &again)
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
        let aside = match cases_way(tree, kept.cases, kept.cases_kept) {
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

/// That every part of the way from `tree` down to the cases directory `cases`, and down to each
/// directory a file of `kept` lies in, is a real directory, none a link, each read with
/// `symlink_metadata` and never followed; the error names the first that is not.
fn cases_way(tree: &Path, cases: &Path, kept: &BTreeMap<PathBuf, Entry>) -> Result<()> {
    let mut dirs = BTreeSet::from([cases.to_path_buf()]);
    for path in kept.keys() {
        let mut at = path.parent();
        while let Some(dir) = at.filter(|dir| !dir.as_os_str().is_empty()) {
            dirs.insert(cases.join(dir));
            at = dir.parent();
        }
    }
    for dir in &dirs {
        no_link_on_the_way(tree, dir)?;
        let meta =
            std::fs::symlink_metadata(dir).with_context(|| format!("read {}", dir.display()))?;
        if !meta.is_dir() {
            bail!(
                "{} is not a directory",
                dir.strip_prefix(tree).unwrap_or(dir).display()
            );
        }
    }
    Ok(())
}

/// The paths of the cases directory `cases` a step or an observation changed, as a refusal names
/// them, under [`CASES_DIR`]: the directory itself first when something else stands in its
/// place (a link, with where it leads), then each of `paths`, one that is a link now with where
/// it leads. Read before the directory is put back.
fn cases_named<'p>(cases: &Path, paths: impl IntoIterator<Item = &'p PathBuf>) -> Vec<String> {
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
    let replaced = match std::fs::symlink_metadata(cases) {
        Ok(meta) if !meta.is_dir() => {
            named.push(format!(
                "{CASES_DIR}{}",
                link(cases).unwrap_or_else(|| ", replaced by a file".to_owned())
            ));
            true
        }
        _ => false,
    };
    named.extend(paths.into_iter().map(|path| {
        let shown = Path::new(CASES_DIR).join(path).display().to_string();
        match (!replaced).then(|| link(&cases.join(path))).flatten() {
            Some(link) => format!("{shown}{link}"),
            None => shown,
        }
    }));
    named
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
/// on any other branch, or detached, stays where it is. Refused when the branch names no case
/// ([`crate::case::check_id`], checked before the name is joined to a path), when the case's
/// base is not known, or when the tree has uncommitted work, which the checkout would carry
/// along.
fn to_base(tree: &Path) -> Result<()> {
    let Some(branch) = crate::git::current_branch(tree)? else {
        return Ok(());
    };
    let Some(case) = branch.strip_prefix(crate::case::BRANCH_PREFIX) else {
        return Ok(());
    };
    // The name is joined to the cases directory only as a case id: `takt/x/y` names no case.
    crate::case::check_id(case)
        .with_context(|| format!("the branch {} names no case", crate::case::shown(&branch)))?;
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
    /// Where discarding a harness step moves the files inside an ignored directory it rewrote,
    /// under the case's id and the step's number: [`default_set_aside`], checked when the run
    /// opens, unless [`Run::set_aside_in`] gives another; none when nothing names one, and then
    /// no harness step is taken. A validator's observation moves nothing aside.
    set_aside: Option<PathBuf>,
    /// The planning store a story's run records its test result and its move in; none until
    /// [`Run::record_on`] gives one, and only a story's run writes to it.
    store: Option<&'a dyn Frontier>,
    started: Instant,
    options: RunOptions,
    /// The files outside the tree that decide how cargo and rustup build and run the tests
    /// ([`Outside`]), as the run found them when it began.
    outside: Outside,
    /// The directories outside the tree the test command prepends to `PATH` and what its Python
    /// imports from there, which [`Outside`] snapshots beside what cargo and rustup read.
    outside_dirs: OutsideDirs,
    /// The module names no step may put in an import directory, kept with the case
    /// ([`Modules::of_case`]).
    modules: Modules,
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
        // Nor does a run start that a harness step's rewrite could not be moved out of.
        let set_aside = opening_set_aside(tree)?;
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
        // The module names the test command's Python imports from outside the tree, asked once
        // and kept with the case.
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
            set_aside,
            store: None,
            started: Instant::now(),
            outside: Outside::read(tree, &outside_dirs)?,
            outside_dirs,
            options,
            modules,
        })
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
        let set_aside = opening_set_aside(tree)?;
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
        // A case opened before it kept its module names is asked now and keeps them from here.
        let runs: Vec<String> = validators.iter().flat_map(|v| v.runs()).collect();
        let modules = Modules::of_case(&case.dir, &runs, tree)?;
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
            set_aside,
            store: None,
            started: Instant::now(),
            outside: Outside::read(tree, &outside_dirs)?,
            outside_dirs,
            options,
            modules,
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
    /// case already records is not taken again, and one whose minted case id the case id rule
    /// refuses ([`crate::case::check_id`]) is not opened: each is skipped with a line on `out`
    /// naming it. `None`,
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
            let mut run = Self::open_unit(tree, options, Unit::Story(story), harness, validators)?;
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
        None
    }

    /// For a run given no approval gate, the gate its case's protocol names for the case's
    /// inputs ([`crate::LoadedProtocol::default_gate`]); a run given one keeps it. Without this
    /// call a run with no gate offers the harness none.
    pub fn take_default_gate(&mut self) {
        if self.options.gate.is_none() {
            self.options.gate = self.case.protocol.default_gate(&self.case.inputs());
        }
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
        let (by, tokens, usage, revision, call) = match by {
            SteppedBy::Validator(revision) => ("validator", None, None, revision, None),
            SteppedBy::Harness(tokens, usage, call) => ("harness", tokens, usage, None, call),
        };
        let call = call.unwrap_or_default();
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
                cost_usd: call.cost_usd,
                duration_ms: call.duration_ms,
                api_ms: call.api_ms,
                turns: call.turns,
                started_at: call.started_at,
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
                    return Ok(Stop::StepRefused(why));
                }
                let checkpoint = self.checkpoint(false)?;
                let cases = self.tree.join(CASES_DIR);
                let kept = snapshot(&cases)?;
                let globs = self.judge_list()?.unwrap_or_default();
                let (surface, surface_kept) =
                    JudgeSurface::read(&self.runs(), globs, &self.modules, &self.tree)?;
                let observed = validator
                    .observe_within(&self.tree, &revision, seq, self.deadline())
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
                // What the observation left: on the judge surface, in the cases directory, in the
                // tree as git sees it and outside the tree. A read that fails (a file the run may
                // not read) discards everything the observation did before the run ends on the
                // error.
                let kept_all = Kept {
                    checkpoint: &checkpoint,
                    cases: &cases,
                    cases_kept: &kept,
                    surface: &surface,
                    surface_kept: &surface_kept,
                    outside: &outside_kept,
                    outside_dirs: &self.outside_dirs,
                };
                let seen = (|| -> Result<_> {
                    let on_surface = differing(&surface_kept, &surface.snapshot(&self.tree)?);
                    let touched = differing(&kept, &snapshot(&cases)?);
                    let change = checkpoint.change(&self.tree)?;
                    let moved =
                        outside_kept.differing(&Outside::read(&self.tree, &self.outside_dirs)?);
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
                let own: Vec<PathBuf> = match self.case.dir.file_name() {
                    Some(id) => validator
                        .writes()
                        .iter()
                        .map(|path| Path::new(id).join(path))
                        .collect(),
                    None => Vec::new(),
                };
                let mut foreign =
                    cases_named(&cases, touched.iter().filter(|path| !own.contains(path)));
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
                            .filter(|path| surface.contains(&path.to_string_lossy()))
                            .map(|path| crate::git::shown(path)),
                    )
                {
                    if !outside.contains(&path) {
                        outside.push(path);
                    }
                }
                outside.extend(outside_moved.iter().map(|path| path.display().to_string()));
                if !foreign.is_empty() || !outside.is_empty() {
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
                    restore(&self.tree, &cases, &kept, &touched)?;
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
                return Ok(Stop::StepRefused(why));
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
                    return Ok(Stop::StepRefused(why));
                }
            };
            let (surface, surface_kept) =
                JudgeSurface::read(&runs, globs, &self.modules, &self.tree)?;
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
            let outside_before = Outside::read(&self.tree, &self.outside_dirs)?;
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
            let kept_all = Kept {
                checkpoint: &checkpoint,
                cases: &cases,
                cases_kept: &kept,
                surface: &surface,
                surface_kept: &surface_kept,
                outside: &outside_before,
                outside_dirs: &outside_dirs,
            };
            // Every way a harness step is discarded goes through here, and what it says of the
            // files it set aside ([`crate::git::SetAside::named`]) is what every message about the
            // discard carries.
            let undo = || -> Result<String> {
                undo_step(&tree, &kept_all).map(|undone| undone.aside.named())
            };
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
                    std::panic::resume_unwind(panic);
                }
            };
            // A step whose leftovers could not all be killed is not judged: it is discarded
            // whole and the run ends on the error.
            if let Err(error) = swept {
                return Err(match undo() {
                    Ok(moved) => error.context(format!(
                        "step {}: what the step left running could not be killed; everything it changed was discarded{moved}",
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
                        Ok(moved) => error.context(format!(
                            "step {}: what the step left could not be read; everything it changed was discarded{moved}",
                            self.steps
                        )),
                        Err(undo) => error.context(format!(
                            "step {}: what the step left could not be read, and its change could not be discarded: {undo:#}",
                            self.steps
                        )),
                    });
                }
            };
            // Named as the step left them; the cases directory is put back with the rest of the
            // step's change by `undo`, after the reset, on every path below that discards it.
            let touched_named = cases_named(&cases, &touched);
            // A step that returned when the hour budget ended, failed (killed by it) or not, is
            // discarded whole like any failed step, recorded as a refusal, and the run stops on
            // the budget. One that reported keeps its step line, with its tokens and their
            // components and a summary saying it was discarded, so the steps' tokens add up to
            // the run's; what of the git directory could not be put back is named. One that
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
                            "step {}: the hour budget ran out during the step ({ended}), and the step's change could not be discarded: {undo:#}{not_put_back}",
                            self.steps
                        ));
                    }
                };
                let why = format!(
                    "step {}: the hour budget ran out during the step; everything the step changed was discarded: {ended}{not_put_back}{moved}",
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
                            self.harness.last_call(),
                        ),
                        self.harness.name(),
                        &report.action,
                        &format!("discarded at the run's end: {}", report.summary),
                    )?;
                }
                self.refused(events, BY_TAKT, &why)?;
                writeln!(out, "{why}")?;
                return Ok(Stop::Hours(h));
            }
            let report = match stepped {
                Ok(report) => report,
                Err(error) => {
                    return Err(match undo() {
                        Ok(moved) => error.context(format!(
                            "step {}: the harness failed; everything the step changed was discarded{moved}",
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
                    SteppedBy::Harness(report.tokens, harness.last_usage(), harness.last_call()),
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
                return Ok(Stop::StepRefused(why));
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
                return Ok(Stop::StepRefused(why));
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
                let left =
                    outside_before.differing(&Outside::read(&self.tree, &self.outside_dirs)?);
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
                return Ok(Stop::StepRefused(why));
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
                let moved = undo()?;
                record_step(self)?;
                let why = format!("{why}{moved}");
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
                    let moved = undo()?;
                    record_step(self)?;
                    return Err(error.context(format!(
                        "step {}: the commit of {}'s change failed; the change was discarded{moved}",
                        self.steps, report.action
                    )));
                }
            };
            // The step is on record before anything that can end the run, so the tokens it spent
            // are kept whatever follows: a case that declares no implementation artifact ends
            // the run here.
            record_step(self)?;
            self.case.set_revision("implementation", &head)?;
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
