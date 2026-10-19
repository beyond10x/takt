---
format: aep.planning-md/3
id: story:approve-then-run-resumes-the-case
kind: story
status: draft
title: After takt approve the documented next run resumes the case
tags:
- finding
refs:
- provider: finding
  reference: approve-then-run-resumes-the-case
relations:
- informed_by: review-result:findings-hunt-33ab42c
- decomposes: epic:composable-agentic-work
revision: 1
---
## Outcome

After `takt approve` on a case opened with `--intent`, the next step the operator is told to take resumes that case. Either a plain `takt run` resumes it, or the help, the approve help and the `ApprovalRequired` stop line name `takt run --case <id>`.

## Context

Both `takt run --help` exit 4 (`main.rs:253`) and `takt approve --help` (`main.rs:38-40`) say "the next run resumes the case". But `next_to_resume` (`run.rs:2712-2726`) only finds story cases, and a tree on a case's branch gets `None`. So the plain run goes to `take_next`, which takes the next ready story. Observed:
- the intent case `probe` stopped `ApprovalRequired (repository.merge)`, exit 4;
- `takt approve` exited 0, granted it, and printed "recorded: nothing; the case was opened on an intent, not a story";
- the next `takt run` printed `frontier: took story:remove-a-todo`, opened a second case and exited 5;
- `probe` keeps its grant, unresumed.

`takt scenario run --help` already names `takt run --case <case>`. Not the filed NoProgress stop-line finding: this is the `ApprovalRequired` path, where the documented next step is wrong.

```expected_before
scenario: 1
command: "$TAKT" run --repo . --intent probe --case-id probe --harness none --forge none --iterations 5 --test-cmd ./check.sh >/dev/null 2>&1; "$TAKT" approve --repo . --by human:reviewer probe repository.merge >/dev/null 2>&1 && "$TAKT" run --repo . --harness none --forge none --iterations 3 --test-cmd ./check.sh
exit: 5
output: frontier: took story:remove-a-todo
source: .engineering/planning/review-result/findings-hunt-33ab42c.md
```

## Scope

`crates/takt/src/run.rs` (`next_to_resume`), or `crates/takt-cli/src/main.rs` (`EXIT_CODES`, the `Approve` doc, `finish`'s stop line).

## Acceptance

In the predicate's run, the last command resumes `probe` and opens no story case. Or, if the fix is to the wording, `takt run --help`, `takt approve --help` and the stop line all name `takt run --case probe`.
