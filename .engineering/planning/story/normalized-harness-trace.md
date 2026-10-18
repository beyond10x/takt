---
format: aep.planning-md/3
id: story:normalized-harness-trace
kind: story
status: draft
title: One harness-neutral trace per run, for semantic comparison; raw usage for cost
summary: steps of any harness recorded in one operation vocabulary for cross-harness comparison; token and cost figures only from raw usage
tags:
- priority:low
relations:
- decomposes: epic:recursive-self-improvement
- serves: vision:safe-agentic-coding
- depends_on: story:scorecard-reader
scope:
- confidence: inferred
  path: crates/takt/src/harness.rs
- confidence: inferred
  path: ess/domains/takt.yaml
revision: 3
---
# Story: normalized-harness-trace

## Outcome

Every harness step takt drives, and every run of a free-running harness in a comparison, is recorded in one harness-neutral trace: the same operation vocabulary (inspect, change, verify, research, wait, coordinate; file reads, file modifications, commands, test runs, pull-request and CI operations) whatever harness produced it. Two runs of the same scenario under different harnesses, or under takt and without it, can then be compared and checked on what they did, not only on what they cost. Cost, token and timing figures are never taken from the normalized form: they come from each harness's raw usage record, kept beside it unchanged.

## Context

Owner decision of 2026-10-18, lower priority than every story planned so far. The operator's baseline comparison of scenario 1 (takt against a free-running Claude Code and a free-running Codex) compares tokens, cost and wall time from raw usage records, but cannot say whether the runs did the same work: Claude Code reports `input_tokens`, `cache_read_input_tokens` and `cache_creation_input_tokens` separately, Codex counts cached tokens inside `input_tokens`, and their transcripts name tools differently (Edit, Write, MultiEdit against apply_patch). A comparison of behaviour needs one vocabulary; a comparison of cost needs the raw counters, because a normalizer that folds cache tokens into input changes the figure (observed in a reference implementation outside this repository, which stores Claude's input as input + cache read + cache write).

The semantic trace serves the RSI scorecard (design:recursive-self-improvement § 5) and a cross-harness evaluation (story:second-harness-adapter): a fix that lowers tokens while dropping the verify step is visible as a behaviour change, not only as a cheaper run.

A reference implementation of such a normalization exists outside this repository (a Rust CLI with adapters for Claude Code and Codex logs, an SQLite intermediate representation and an operation-tree sidecar); whether takt reads its format, calls it, or carries its own adapter is open.

## Expected before

On takt at 3ad0bd9, holds; after the fix, fails:

- After a story run, no file under the case directory records the harness steps in a harness-neutral form: `ls .engineering/cases/<case>/` shows no trace file, and the per-step token components are not recorded (the stop line carries one summed figure, `crates/takt/src/harness.rs:232-245`).

## Scope

- `crates/takt/src/harness.rs`: keep each step's raw usage and transcript reference (inferred)
- a trace module or an adapter to an external normalizer (inferred)
- `ess/domains/takt.yaml`: a `Trace` and `TraceOperation` declaration (inferred; spec first)

## Acceptance

For one scenario run under takt and one free-running Claude Code run of the same scenario, the trace of each lists its operations in the shared vocabulary, a comparison report states which operation kinds one run has and the other lacks, and the raw usage of each run (input, output, cache read, cache write, as each harness reported them) is recorded beside the trace and is the only source of every token and cost figure in the report.
