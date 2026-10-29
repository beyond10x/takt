---
format: aep.planning-md/3
id: story:unqualified-stop-records-no-layer
kind: story
status: implemented
title: A run that stops HarnessUnqualified makes its case's runs unscorable
tags:
- finding
- rsi
refs:
- provider: finding
  reference: unqualified-stop-records-no-layer
relations:
- serves: vision:safe-agentic-coding
scope:
- confidence: inferred
  path: crates/takt/src/run.rs
- confidence: cited
  path: crates/takt/src/scorecard.rs
- confidence: cited
  path: crates/takt/tests/layers.rs
revision: 7
transitions:
- {from: "draft", to: "proposed", at: "2026-10-28T15:56:06Z", actor: "human:timo", revision: 5}
- {from: "proposed", to: "active", at: "2026-10-28T15:58:00Z", actor: "human:timo", revision: 6}
- {from: "active", to: "implemented", at: "2026-10-28T20:40:00Z", actor: "human:timo", revision: 7, decided_on: {"recorded":{"test_result":1}}, executor: "agent:planner"}
---
# A run that stops HarnessUnqualified makes its case's runs unscorable

A run opens with the layers it can read itself, takt's revision alone
(`crates/takt/src/run.rs:11560`); the `harness` and `model` layers of a run are taken from its
harness steps' lines (`Span::row`, `crates/takt/src/scorecard.rs:701`). A run or a resume that
stops `HarnessUnqualified` (`crates/takt/src/run.rs:11657`) stops before any step, so its row
records no harness layer. `layers_differ` (`crates/takt/src/scorecard.rs:237`) then finds that
run's stack different from every run of the same case that took a step, and
`takt scorecard --save` refuses the set, exit 1, unless `--across-layers` is passed, though the
unqualified run ran on no harness at all.

In the 0.17.0 trial, scenario 1, one Claude Code run spent its 60 iterations waiting on CI and
stopped `Waiting`; its resume stopped `HarnessUnqualified` (the harness binary's `--version` did
not answer within its 5 s deadline). Saving the arm's scorecard then needed `--across-layers`,
because that resume recorded no harness layer.

The predicate runs scenario 1 offline on `story.delivery/1`: a first run with a scripted `claude`
(`--harness-program`, a qualified `--version`, a fixed `tests.write` reply, no model) takes 3
steps; a resume of the case with a program whose `--version` exits 3 stops
`HarnessUnqualified`; then `takt scorecard --save` under the binary's own revision. The defect is
there when the save is refused naming the harness layer as recorded by one run and not the other;
it is gone when the scorecard is saved, exit 0.

```expected_before
scenario: 1
command: h=$HOME; printf '%s\n' '{"type":"system","subtype":"init","model":"fake-model","claude_code_version":"2.1.309","tools":["Read","Edit","Write","Glob","Grep"]}' '{"type":"result","subtype":"success","result":"{\"action\": \"tests.write\", \"summary\": \"a test\"}","usage":{"input_tokens":3,"output_tokens":4}}' > "$h/reply.json"; printf '%s\n' '#!/bin/sh' 'if [ "$1" = --version ]; then echo "2.1.309 (Claude Code)"; exit 0; fi' 'echo "# a test" >> tests/test_todo.py' "cat $h/reply.json" > "$h/claude"; printf '%s\n' '#!/bin/sh' 'exit 3' > "$h/unqualified"; chmod +x "$h/claude" "$h/unqualified"; git config user.name t && git config user.email t@example.invalid && XDG_STATE_HOME="$h/state" "$TAKT" run --repo . --intent "Remove a todo by its number" --protocol story.delivery/1 --harness claude --harness-program "$h/claude" --forge none --test-cmd ./check.sh --min-free-gib 0 --iterations 3 > "$h/run1.log" 2>&1; c=$(ls .takt/cases); XDG_STATE_HOME="$h/state" "$TAKT" run --repo . --case "$c" --harness claude --harness-program "$h/unqualified" --forge none --test-cmd ./check.sh --min-free-gib 0 --iterations 3 > "$h/run2.log" 2>&1; grep -q HarnessUnqualified "$h/run2.log" || { cat "$h/run1.log" "$h/run2.log"; echo "the resume did not stop HarnessUnqualified" >&2; exit 2; }; r=$("$TAKT" --version | sed -n 's/.*(\([0-9a-f]\{40\}\).*/\1/p'); "$TAKT" scorecard --repo . --save "$h/sc" --revision "$r"
exit: 1
healthy_exit: 0
output: records none: the runs' layers differ
source: crates/takt/src/scorecard.rs
```

## Outcome

A run that stopped before any harness step, `HarnessUnqualified` among such stops, does not make
its case's runs differ in layers, so the set is scored and saved without `--across-layers`.

## Context

- The 0.17.0 trial, scenario 1: a resume that stopped `HarnessUnqualified` recorded no harness
  layer, and the arm's scorecard was saved only with `--across-layers`.
- `--across-layers` turns off the check for every run of the set, so using it to get past this
  stop also hides a real change of harness or model between runs.

## Acceptance

1. A case whose first run took harness steps and whose resume stopped `HarnessUnqualified` is
   scored and saved by `takt scorecard --save` without `--across-layers`; the predicate above
   gives `not_reproduced`.
2. Two runs that took harness steps on different harness versions are still refused without
   `--across-layers`, naming the layer and both runs, as `layers_differ` does today.
3. The stopped run still counts as a run of its case, with its outcome and its 0 steps, in the
   printed and saved scorecard.

## Scope

- `crates/takt/src/scorecard.rs`: `layers_differ` and `Span::row`, which layers a run without a
  harness step brings to the comparison (cited)
- `crates/takt/src/run.rs`: the `opened` line's layers and the `HarnessUnqualified` stop, if the
  run records what its preflight read (inferred)
- `crates/takt/tests/layers.rs`: a case with a resume stopped `HarnessUnqualified` (cited)
