---
format: aep.planning-md/3
id: story:frontier-next-ready-story
kind: story
status: draft
title: Pull the next ready story from the planning store as the unit of a run
summary: The Frontier seam with an aep adapter and the ready rule as code; the story goes active when taken
relations:
- decomposes: epic:composable-agentic-work
- serves: vision:safe-agentic-coding
- depends_on: story:takt-domain-model
scope:
- confidence: cited
  path: crates/takt-cli/src/main.rs
- confidence: inferred
  path: crates/takt/src/frontier.rs
- confidence: cited
  path: crates/takt/src/lib.rs
- confidence: cited
  path: crates/takt/src/run.rs
- confidence: inferred
  path: crates/takt/tests/frontier.rs
revision: 4
---
# Story: frontier-next-ready-story

## Outcome

An operator runs `takt run --repo R` with no `--intent` and the binary takes the next ready story of the planning store in R as the unit, opens a case for it and records in the store that the story is being worked, with no model involved in the choice.

## Context

`docs/design/takt.md` § 3: `unit := intent | frontier(repo)`, with the note "AEP: ready stories and waves; no model"; § 4 M2: "the AEP frontier (ready rule as code, `artifact waves`)"; § 3 Seams: `Frontier` is the planning store behind a trait with one adapter. Today the unit is one required intent (`crates/takt-cli/src/main.rs:30-32`, `crates/takt/src/run.rs:21-22`) and the kernel never reads the store.

The adapter reads the store the way `AGENTS.md:14` allows, through the `aep` command line: `aep plan artifact list --format json` (id, kind, status, title, path, relations, `blocked_by`), `aep plan artifact show <id>` for the body, `aep plan artifact waves --format json --status proposed` for the order, `aep plan artifact move <id> --to active` to take a story. The ready rule, as code: a story is ready when its status is `proposed`, its `blocked_by` is empty, every `depends_on` target is `implemented` or `archived`, and it comes first in the order `waves` prints. Taking a story moves it `proposed → active`; the lifecycle describes `proposed` as "refined and agreed enough to pick up" and `active` as "being worked on now", so a story with an open case is never taken twice. A store may refuse the move (a profile can require the story to serve an objective); then the story is not taken, the refusal is printed verbatim, and the run goes on to the next ready story or exits saying nothing is ready.

The case for a story: `intent.md` is the story title followed by its body; the case id is derived from the story id and the instant; the protocol is what `--protocol` or the workflow names (`protocols/story-delivery/1.yaml` once story:story-delivery-protocol exists; `software.change/1` is enough to demonstrate this story).

## Domain relations

- Run → Unit: one intent, required today. Inferred from `crates/takt/src/run.rs:21-22` and `crates/takt-cli/src/main.rs:30-32`; this story makes the unit an intent or a story.
- Story → Case: a story is taken at `proposed` and is `active` while its case is open, so one run takes it. Inferred from the lifecycle the store enforces: aep, `artifacts/lifecycles/story.yaml` (`transitions`, `descriptions`), from the tree `.engineering/project.yaml` pins under `protocols`. Whether a later run resumes that case is decision-blocker:approval-grant-and-case-resume, not this story.
- Case → Protocol: exactly one. Settled by b10x-canon 0.6.0 `ess/domains/protocol.yaml:644-651`, entity `canon.protocol.Case`, relation `protocol`.

## Scope

- new: `crates/takt/src/frontier.rs` (the `Frontier` trait, the `aep` adapter, the ready rule as a pure function over the listed artifacts, unit-tested)
- `crates/takt/src/run.rs` (a run opened on a unit from the frontier)
- `crates/takt/src/lib.rs` (export)
- `crates/takt-cli/src/main.rs` (`--intent` optional; without it the frontier is asked)
- new: `crates/takt/tests/frontier.rs` with a fixture store under `crates/takt/tests/fixtures/`

## Ordering

- depends_on story:takt-domain-model: no shared file; the nouns are declared before stories build on them, the epic's Why: "declared in an ESS domain document before stories build on them". This is the first code story of the epic, so every code story follows the domain model through it.

## Acceptance

On a repository whose planning store holds one `proposed` story with no blocker and no unmet `depends_on`, `takt run --repo R --harness none --iterations 1` opens a case whose `intent.md` is that story and leaves the story at status `active`, and on a store with no such story it opens no case and exits saying nothing is ready.

## Out of Scope

Evidence written back on the story and the move to `implemented` (story:run-record-and-story-evidence); more than one unit per run (story:declared-loops); resuming a story that is already `active`.

## Ambiguities

- Which status means ready: `inferable`, aep `artifacts/lifecycles/story.yaml` `descriptions` (`proposed`: refined and agreed enough to pick up).
- Reading the store through the CLI versus linking the `aep` crates: `inferable` as the choice recorded here, `AGENTS.md:14` names the verbs as the only door to the store, and the CLI is what the store validates.
- A story already `active` with an open case from an earlier run: `requires-stakeholder-input`, decision-blocker:approval-grant-and-case-resume.
