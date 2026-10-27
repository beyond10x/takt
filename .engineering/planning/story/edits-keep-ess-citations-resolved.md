---
format: aep.planning-md/3
id: story:edits-keep-ess-citations-resolved
kind: story
status: active
title: Edits keep ESS read-from citations resolved
summary: re-point citations after line shifts before the gate (trial run 7)
tags:
- loom-agent
relations:
- serves: vision:safe-agentic-coding
- decomposes: epic:takt-as-a-loom-agent
- informed_by: approval-record:wave-38-trial-s7-defects
scope:
- confidence: cited
  path: crates/takt/src/harness.rs
- confidence: inferred
  path: crates/takt/src/run.rs
- confidence: cited
  path: crates/takt/src/validator.rs
revision: 8
transitions:
- {from: "draft", to: "proposed", at: "2026-10-26T13:48:10Z", actor: "human:timo", revision: 3}
- {from: "proposed", to: "active", at: "2026-10-26T20:53:54Z", actor: "human:timo", revision: 5}
---
# Story: edits-keep-ess-citations-resolved

## Outcome

A step that moves lines an `ess/` `read from:` citation points at is told, at its next judgement
step, which citations the `citations` gate found unresolved, line by line, so the model can
re-point them instead of editing on against a count it cannot act on.

## Context

- Trial run 7, steps 3 and 6: gate.run fail on gate `citations`, 24 `read from` citations of
  `crates/takt/src/git.rs` unresolved after the model's edits (`scripts/check-citations
  --require-digests` on the run's head: 1132 in-repository, 1108 resolved). The run stopped at
  `Iterations (8)` without a green gate; its draft pull request is
  https://github.com/beyond10x/takt/pull/17.
- The second kind-cluster trial, 2026-10-26: every edit of runs 7 and 8 left 20 citations
  unresolved, 9 `gate.run` failures each on `citations`. The checker prints one
  `<document>:<line>: unresolved: <cited>` line per citation and a summary last; the step was
  shown only the summary.

## Mechanism (decided 2026-10-26, approval-record:wave-38-trial-s7-defects)

takt does not re-point citations itself and does not refuse the handover. A failed `gate.run`
keeps, per failed gate, the lines of its command's output that name what failed (bounded in
bytes, with the count of lines left out), its evidence record keeps the same lines, and the next
judgement step's prompt shows them under the failed gate's id. Why: the gate commands are the
repository's own (`engineering-assertions-project/1` has no repair command), so a re-point in
takt would hard-code one repository's script into a binary that runs on any; the handover's
gating is protocol data, not takt code.

## Acceptance

1. After a `gate.run` whose `citations` command fails naming unresolved citations, the next
   judgement step's prompt contains each `<document>:<line>: unresolved:` line the command
   printed (up to the bound) under the gate's id, and the run record's `gate.run` evidence line
   keeps them in its `failed` field (amended at close: the canon evidence record keeps no output,
   so takt runs a failed command once more to read its lines): a test with a fake harness and a
   gate command that prints such lines.
2. A gate that passes adds nothing to the prompt; a passing run's prompt is unchanged.
3. The section is bounded in bytes; output past the bound is cut with the count of lines left
   out named.

## Scope

- Inferred: `crates/takt/src/validator.rs` (what `gate.run` keeps of a failure),
  `crates/takt/src/harness.rs` (the prompt's evidence lines), `crates/takt/src/run.rs`.
