---
format: aep.planning-md/3
id: story:a-gate-that-cannot-run-stops-the-run
kind: story
status: draft
title: A gate that cannot run on the change stops the run with a clear decline
summary: the first gate.run not run on changed gate inputs stops, naming them
tags:
- finding
- priority:medium
- trial-0.24.0
relations:
- informed_by: story:the-gate-compares-its-scripts-and-tool-config-at-the-base
revision: 1
---
## Outcome

When a change edits a file the gate compares at the base, so the gate cannot run at any later
revision, the run stops at the first such `gate.run` with a clear decline that names the changed
gate inputs and the owner's route, instead of offering more edits and handovers until the model
declines. The gate's base comparison stays as it is.

## Context

In a cluster trial run on 0.24.0 the change touched `Taskfile.yml`. Every later `gate.run` said
"not run; what its commands run or read from the tree differs from the base revision at …:
Taskfile.yml; no record, the claim stays unknown". The first run spent 365.1k and 127.8k tokens
after that line; its resume cycled handover, change.read and implementation.edit five times and
stopped `Declined (pr.open is blocked because gate.green is unknown; no offered action can
establish it …)` at 2,241,167 tokens: 2,734.1k tokens after the gate became unrunnable. The story
needed a new Taskfile step, so no edit could have made the gate run.

Where, at 0.24.0: `crates/takt/src/validator.rs:3456-3471` returns `nothing_recorded` for a
changed gate input, the same shape as a pending observation, so the loop keeps offering steps.
The prompt shows the gate only as an obligation status (`crates/takt/src/obligations.rs:38-39`,
`:337-339`, `:372`), not why it did not run. `Stop::Unperformable`
(`crates/takt/src/run.rs:126, 322-324`) already exists for "admissible, but nothing performs it".
The rule comes from story:the-gate-compares-its-scripts-and-tool-config-at-the-base, which added
the comparison but no stop. A second instance of the pattern: on `story.delivery/3` a case whose
`change.read` records `tests_only` is offered only `tests.write` while `pr.open` needs
`change.complete` (`protocols/story-delivery/3.yaml:149-159, 270-276`).

```expected_before
command: a scripted case whose implementation.edit changes Taskfile.yml, run to its end
output: the step after the gate.run "not run … differs from the base" is another harness step, and the run ends exit 9
```

## Acceptance

1. The first `gate.run` not run because a gate input changed stops the run with a named stop
   (`Unperformable (gate.green; …)` or a new one) listing the changed files and the route to land
   the gate change (the gates document or a grant), with no harness step after it.
2. The prompt of any step taken while the gate is not run says why, naming the files.
3. The same stop applies to a `tests_only` change under a protocol whose `pr.open` needs
   `change.complete`.
4. The base comparison is unchanged: a gate input edited in the change still makes the gate
   not run.
