---
format: aep.planning-md/3
id: story:in-call-tool-guard
kind: story
status: draft
title: A takt-owned pre-tool hook refuses an out-of-scope tool call during the step
summary: default-deny hook through --settings; unreadable config or a failed hook denies; the put-back stays the second line
relations:
- decomposes: epic:composable-agentic-work
- serves: vision:safe-agentic-coding
- depends_on: story:tool-scope-per-action
revision: 1
---
# Story: in-call-tool-guard

## Outcome

During a judgement step, a tool call outside the chosen action's scope is refused before it runs, by a takt-owned pre-tool hook the harness child is started with; the hook denies everything when its configuration cannot be read or when it fails, and the put-back after the step stays as the second line.

## Context

From the tool-calling brainstorm of 2026-10-18. Two agent hosts it compared guard each tool call as it happens: default-deny, a configuration that cannot be loaded denies every call, and the hook is wired so that its own failure blocks (`|| exit 2`); one runs argument bounds, then a before hook, then approval, then the call, and a failed hook blocks. takt checks only after the step: the step runs, and a change outside what the action may touch is refused and put back (`crates/takt/src/run.rs:4090-4110`, the guarded-path put-back around `run.rs:3395-3481`). What happened during the step (a read of a file outside the tree, a write that was put back after a test had already read it) is not prevented.

Today the harness child is started with every hook turned off: `crates/takt/src/harness.rs:110-115` passes `--restricted`, `--strict-mcp-config` and `--settings '{"disableAllHooks":true}'`, so that no hook from the user, the project or the tree runs.

## Scope

- `crates/takt/src/harness.rs`: `ISOLATION`, the `--settings` that carries the takt hook instead of turning all hooks off (cited)
- `crates/takt-cli/src/main.rs`: a hook verb that reads one tool call and answers allow or deny (inferred)
- new: a module for the hook's decision against the action's scope (inferred)
- new: `crates/takt/tests/in_call_tool_guard.rs` (inferred)

## Ordering

- depends_on story:tool-scope-per-action: the hook enforces the scope that story declares.

## Acceptance

Given a tool call that writes `src/lib.rs` under the `tests.write` scope, the hook verb exits with the harness's blocking status and names the path; given any tool call while its scope file is missing or unreadable, it exits the same way; and a fake harness step that ignores the hook and writes `src/lib.rs` anyway is still refused and put back after the step.

## Ambiguities

- Whether a hook passed through `--settings` runs under `--restricted` while the user's, project's and tree's hooks still do not: `inferable` only from a run of the harness CLI, which must be observed before the hook is relied on; the brainstorm asserts it and nobody has checked it.
- Where the hook's scope file lives so the step cannot write it: outside the tree, written by takt before the step (`inferable`).
