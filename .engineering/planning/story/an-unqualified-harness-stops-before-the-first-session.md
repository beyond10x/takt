---
format: aep.planning-md/3
id: story:an-unqualified-harness-stops-before-the-first-session
kind: story
status: draft
title: An unqualified harness binary stops the run before its first model call
summary: preflight through metaharness qualify, no model call
tags:
- priority:high
relations:
- serves: vision:safe-agentic-coding
- decomposes: epic:downstream-story-delivery
scope:
- confidence: inferred
  path: crates/takt/src/harness.rs
- confidence: inferred
  path: crates/takt/src/main.rs
- confidence: inferred
  path: crates/takt/src/run.rs
- confidence: inferred
  path: crates/takt/tests
- confidence: inferred
  path: ess/domains/takt.yaml
revision: 2
---
# Story: an-unqualified-harness-stops-before-the-first-session

## Outcome

Before step 1, on open and on every resume, takt asks metaharness whether the harness binary it would run qualifies
under the run's hermetic mode, without a model call. A gating row that is not `ok` stops the run with a new stop,
`HarnessUnqualified { kind, version, rows }`, at exit 1. The case stays resumable.

The stop line names:
- the version and the failing row;
- the qualified versions;
- a qualified binary when one is installed. Candidates come from `HarnessBinaries::candidates()`, each probed by
  `--version` only.

takt never changes the user's default and never runs a candidate unasked. Advisory rows are printed and do not stop
the run.

## Context

- A downstream trial (2026-10-27) ran Claude Code 2.1.311 under Strict. Loom's executor held the proposal for an H11
  `gap` only after the session: 37,234 tokens spent (loom-session-executor `lib.rs:638-695`).
- metaharness pins 2.1.304/2.1.305/2.1.309 (metaharness-claude `lib.rs:207`) and reads H11 `gap` for a version with
  no memory canary (`launch.rs:221-222`).
- takt runs no `--version` before a session (only `promotion.rs:1351,1774,1923` at 110251a).
- H11 stays gating for Claude, because it is memory isolation. `story:codex-off-pin-proposes` covers Codex H9 only.

## Acceptance

Each is a named conformance scenario run by `task check`.

1. `harness_preflight::an_unqualified_claude_stops_before_a_model_call`: a fake `claude` reporting `2.1.311`. The
   fake model endpoint counts 0 requests, exit is 1, and the stop line names H11 and the qualified versions.
2. `harness_preflight::a_resume_rechecks_the_binary`: a case opened on a qualified fake, then resumed after the fake
   reports an unqualified version, stops before its next session.
3. `harness_preflight::an_advisory_row_does_not_stop`: a version whose only non-`ok` row is advisory runs, and the
   opening record names the row.
4. `harness_preflight::a_qualified_candidate_is_named`: with a qualified second binary among the candidates, the stop
   line names its path.

## Scope

- `crates/takt/src/harness.rs` (`SessionHarness::preflight`, candidates)
- `crates/takt/src/run.rs` (`Run::open` and resume call the preflight; the new stop)
- `crates/takt/src/main.rs` (exit code and stop line)
- `ess/domains/takt.yaml` (the `HarnessUnqualified` stop, before code)
- `crates/takt/tests/` (conformance scenarios)

## Delivery

This depends on metaharness `story:qualify-a-binary-without-a-model-call`, re-exported by loom-sdk, recorded here as
`dependency-blocker:metaharness-qualify-without-a-model-call`. The chain metaharness -> loom -> takt is delivered in
one workspace:
- Uncommitted `[patch]` path overrides in loom's and takt's `Cargo.toml`.
- Scenario 1 as the end-to-end check.
- Then pin-only releases in chain order. No intermediate release.
