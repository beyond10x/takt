---
format: aep.planning-md/3
id: story:report-cases-notes-skip-the-unknown-day
kind: story
status: draft
title: report --cases leaves a step's unknown tokens unexplained on the day unknown
tags:
- finding
refs:
- provider: finding
  reference: report-cases-notes-skip-the-unknown-day
revision: 1
---
# report --cases leaves a step's unknown tokens unexplained on the day `unknown`

## What is wrong

`takt report --cases` prints a note under its table for each reason a token figure is
`unknown`. `print_case_sums` (crates/takt/src/report.rs) looks for a harness step that recorded
no tokens only on dated rows (`a_step_unknown` skips the row whose day is `unknown`). When a run
whose `opened` instant does not read has such a step, and another case of the same report has no
run record, only the note on cases without a run record prints: the step's own unknown is
explained by a cause that is not its own.

Found by the adversarial pass of wave 46 at a7a3670e (docs/waves/wave-46.md), test
`the_notes_name_a_step_unknown_on_the_unknown_day_too`. takt writes instants that read, so only
a record edited by hand reaches it; low.

## Acceptance

1. A report whose day `unknown` holds a run with a harness step that recorded no tokens prints
   the note on harness steps with no tokens, beside the note on cases without a run record when
   one is there too.
2. The predicate below prints `not_reproduced` at the fix's revision.

```expected_before
repository: https://github.com/beyond10x/takt
seed: a7a3670eb3a3340ee1830dc43d8d3e149be24949
command: mkdir -p r/.takt/cases/a r/.takt/cases/b && printf '%s\n' '{"at":"x","step":0,"event":"opened","case":"a"}' '{"at":"x","step":1,"event":"step","by":"harness","action":"implementation.edit"}' '{"at":"x","step":1,"event":"stop","stop":"NoProgress","steps":1,"tokens":0}' > r/.takt/cases/a/events.jsonl && printf 'id: b\n' > r/.takt/cases/b/case.yaml && "$TAKT" report --cases r/.takt/cases > r/out.txt 2>&1; grep -q 'recorded no tokens, or 0' r/out.txt || { echo STEP_UNKNOWN_UNEXPLAINED; exit 3; }
exit: 3
healthy_exit: 0
output: STEP_UNKNOWN_UNEXPLAINED
source: docs/waves/wave-46.md
```
