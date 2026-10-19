---
title: One step at a time
description: The loop takt runs, what a judgement step may do, and how a run stops.
---

# One step at a time

A run is a loop:

```text
evaluate → frontier → step → effect → observation → validator → evidence → evaluate
```

until it reaches a named [stop](#stop).

## Evaluate

Canon's `evaluate` is a pure function from the protocol, the case, the evidence records, the
authority and the instant to the claims (TRUE, FALSE or UNKNOWN), the obligations, the admissible
actions and the legitimate outcomes. takt writes each result as a decision file in the case.

## Step

- A **judgement step**: the harness performs one admissible action in the worktree and reports.
  It is offered only the actions the frontier admits. A step that does more is refused and
  reverted. What the harness says is a trace, never evidence.
- A **deterministic step**: a command takt runs itself, such as the tests, the gate, opening the
  pull request, reading CI, or the merge under a grant.

The harness child loads no project or user settings, hooks or plugins, and its file tools are
confined to the tree. Every child takt starts gets an environment without credentials it does not
need, a timeout and a bounded tail of its output. takt adds no operating-system sandbox; the
confinement seam is not built.

## Stop

Every run ends on a named stop, each with its own exit code, so a script or an orchestrator can
drive takt without reading its output. The stops and their codes are in
[Exit codes](../reference/exit-codes.md).

Sources: `docs/design/takt.md` § 3, `crates/takt/tests/validator_confinement.rs`,
`crates/takt/src/harness.rs`, `takt run --help`.
