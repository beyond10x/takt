---
format: aep.planning-md/3
id: story:tool-scope-per-action
kind: story
status: draft
title: Each judgement action names its tool set and write scope, bound all or nothing
summary: inspect gets read tools, tests.write test paths, implementation.edit the story's scope; an unbindable tool stops the step
relations:
- decomposes: epic:composable-agentic-work
- serves: vision:safe-agentic-coding
revision: 1
---
# Story: tool-scope-per-action

## Outcome

Each judgement action a protocol offers names its tool set and the paths it may write: `repository.inspect` gets read tools only, `tests.write` writes test paths only, `implementation.edit` writes the story's scope, and the harness child of a step is started with exactly the chosen action's tools and scope, or the step is not taken.

## Context

From the tool-calling brainstorm of 2026-10-18. Three designs it compared point the same way: a graph-based agent that computes each node's tools from the state, a capability layer that gates a write on a concrete subject, and a harness that activates a step's tools bind-or-nothing (every declared tool is bound, or the step does not start). takt grants one tool set to every judgement step: `crates/takt/src/harness.rs:197-200` passes `--allowedTools Read,Edit,Write,Glob,Grep,MultiEdit` whatever the action. Scope is checked only after the step, from what changed: `crates/takt/src/run.rs:4090-4101` refuses `repository.inspect` that changed a path and `tests.write` that changed a non-test path (`is_test_path`, `harness.rs:274-290`); `implementation.edit` admits any path outside the guarded ones. The actions and their `effect` are protocol data (`protocols/story-delivery/1.yaml:99-140`).

Related: story:no-inspect-for-stateless-harness decides whether `repository.inspect` is offered at all; story:case-scoped-step-guard and story:step-cannot-forge-its-validators are the after-the-step line this story adds a before-the-step line to.

## Scope

- `protocols/story-delivery/1.yaml`: each judgement action's tool set and write scope, as data (cited)
- `crates/takt/src/protocol.rs`: reading them (inferred)
- `crates/takt/src/harness.rs`: the child's `--allowedTools` from the chosen action; the prompt's description of the scope (cited)
- `crates/takt/src/run.rs`: the after-the-step check judged against the declared scope instead of fixed rules (cited)
- `ess/domains/takt.yaml`: the action's tool set and scope, if they are new nouns (inferred)
- new: `crates/takt/tests/tool_scope_per_action.rs` (inferred)

## Acceptance

On a story case where the frontier admits only `repository.inspect`, the harness child is started with read tools only (its argument vector names no `Edit`, `Write` or `MultiEdit`), and an action whose declared tool set takt cannot bind is not stepped: the run stops with the action and the unbound tool named, and no harness child starts.

## Ambiguities

- Whether `protocol/1` admits a tool set and write scope per action, or whether that vocabulary belongs in the protocol schema upstream (the brainstorm's item on a risk and effects vocabulary placed it there): `needs-decision`; until decided, a takt-side table keyed by action id, read from a file in the repository, keeps the rule out of Rust.
- How `implementation.edit` learns the story's scope: `inferable` from the story's `## Scope` section in the planning store at the case's base revision.
