---
format: aep.planning-md/3
id: story:scorecard-help-names-the-held-out-check
kind: story
status: proposed
title: takt scorecard --help says outcomes come from events.jsonl only; a scenario run's comes from the held-out check
tags:
- finding
refs:
- provider: finding
  reference: scorecard-help-names-the-held-out-check
relations:
- serves: vision:safe-agentic-coding
- decomposes: epic:recursive-self-improvement
revision: 2
transitions:
- {from: "draft", to: "proposed", at: "2026-10-19T17:13:45Z", actor: "human:timo", revision: 2}
---
# Story: scorecard-help-names-the-held-out-check

## Outcome

`takt scorecard --help` says where a run's outcome is read from: the case's `events.jsonl`, and for a scenario run the held-out check's result beside it. It no longer says `events.jsonl` only.

## Context

Found by the independent read-only review of the public site of 2026-10-19 (the scorecard page carried the same clause). The help text of `takt scorecard` (`crates/takt-cli/src/main.rs:49-50`) says each run's outcome and figures are "read from the case's `events.jsonl` only". The scorecard reads a scenario run's outcome from the held-out check, never from the run's own record (`crates/takt/src/scorecard.rs:12-17`, `HELD_OUT_FILE`). The help contradicts the code it documents; an operator reading it would trust a run's own stop for a scenario case.

This is the finding of the first validator-driven improvement turn (RSI part 1, `self.improvement/1`): it is checkable offline by `scripts/check-finding`, so its before and after are machine results, as for story:scenario-run-help-exit-codes.

```expected_before
scenario: 1
command: "$TAKT" scorecard --help
exit: 0
output: events.jsonl` only
source: crates/takt/src/scorecard.rs
```

## Scope

- `crates/takt-cli/src/main.rs`: the `scorecard` command's doc comment (inferred)

## Acceptance

At the candidate, `scripts/check-finding story:scorecard-help-names-the-held-out-check <candidate>` prints `not_reproduced`, and `takt scorecard --help` names the held-out check as the source of a scenario run's outcome.
