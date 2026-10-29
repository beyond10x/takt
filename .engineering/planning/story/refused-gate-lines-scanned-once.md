---
format: aep.planning-md/3
id: story:refused-gate-lines-scanned-once
kind: story
status: draft
title: A refused failed-gate block costs a scan per line, again for each prompt built
summary: refused_lines starts a scan process per bisection node and keeps no verdict across a step's prompt builds.
tags:
- finding
- priority:low
- rsi
refs:
- provider: finding
  reference: refused-gate-lines-scanned-once
relations:
- serves: vision:safe-agentic-coding
revision: 1
---
# A refused failed-gate block costs a scan per line, again for each prompt built

## What is wrong

When the secret scan refuses a failed gate's block, `refused_lines`
(`crates/takt/src/harness.rs`) bisects the check's lines and starts one `scan-text` process per
node, up to 2n-1 for n lines over a block of up to 8 KiB. Compiler output that names paths under
the operator's home refuses many lines at once. A step's prompt is built more than once (a Codex
step builds it for each list of context files it tries), and each build scans the same lines
again, since nothing records a line already judged. The cost is time per step, not a leak: every
text that reaches a prompt has passed the scan. Found in the review of the self-run branch's
merge into `integration/takt` (finding 5).

## Check

```expected_before
command: awk '/^fn refused_lines\(/{f=1} f&&/^}/{exit} f&&/scan_change\(/{s++} f&&/refused_lines\(head|refused_lines\(tail/{b++} END{if(s&&b){print "refused_lines scans each bisection node with a process of its own"; exit 9} exit 0}' crates/takt/src/harness.rs && ! grep -q 'scanned_lines\|ScanVerdicts' crates/takt/src/harness.rs
exit: 9
healthy_exit: 0
output: refused_lines scans each bisection node with a process of its own
```

The predicate reads the source: it holds while the bisection starts a scan per node and no
record of verdicts exists. A fix that keeps the bisection but records verdicts per line across
a step's prompt builds names that record `ScanVerdicts`, or the planner changes the predicate.

## Outcome

A step scans each failed-gate line at most once across the prompts it builds, and a block the
scan refuses costs a bounded number of scan processes (for example one per line, or one scan of
the whole block per line batch the scan can report on).

## Acceptance

1. The predicate above exits 0.
2. A test with a counting fake scan: a refused block of 64 lines built twice starts at most 64
   scan processes.
