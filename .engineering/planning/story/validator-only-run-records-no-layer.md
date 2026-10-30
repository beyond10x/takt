---
format: aep.planning-md/3
id: story:validator-only-run-records-no-layer
kind: story
status: implemented
title: A run that took only validator steps makes its case's runs unscorable
tags:
- finding
- rsi
refs:
- provider: finding
  reference: validator-only-run-records-no-layer
relations:
- serves: vision:safe-agentic-coding
scope:
- confidence: cited
  path: crates/takt/src/scorecard.rs
- confidence: cited
  path: crates/takt/tests/layers.rs
revision: 6
transitions:
- {from: "draft", to: "proposed", at: "2026-10-28T17:56:36Z", actor: "human:timo", revision: 4}
- {from: "proposed", to: "active", at: "2026-10-28T21:15:42Z", actor: "human:timo", revision: 5, executor: "agent:developer"}
- {from: "active", to: "implemented", at: "2026-10-29T21:30:14Z", actor: "human:timo", revision: 6, decided_on: {"recorded":{"test_result":1}}}
---
# A run that took only validator steps makes its case's runs unscorable

A run's `harness` and `model` layers are taken from its harness steps' lines (`Span::row`,
`crates/takt/src/scorecard.rs:722`). `layers_differ` (`crates/takt/src/scorecard.rs:244`) skips
those two layers (`STEP_LAYERS`, `crates/takt/src/scorecard.rs:308`) only for a run that took 0
steps (`crates/takt/src/scorecard.rs:282`). A run that took validator steps and no harness step
states no harness layer either, but its steps are above 0, so it is compared and found different
from every run of the same case that took a harness step: `takt scorecard --save` refuses the set,
exit 1, `records none`, unless `--across-layers` is passed, though that run ran on no harness at
all. A `Row` counts steps but cannot tell harness steps from validator steps.

Such runs are ordinary: a resume whose first step is a validator's (`tests.run`, `gate.run`,
`ci.read`) and which then stops on its budget, or stops `Waiting` on CI, takes no harness step.

The predicate runs scenario 1 offline on `story.delivery/1`: a first run with a scripted `claude`
(`--harness-program`, a qualified `--version`, a fixed `tests.write` reply, no model) and
`--iterations 3` takes `tests.run`, `gate.run` and the harness step `tests.write`; a resume of the
case with `--iterations 1` takes the validator step `tests.run` alone and stops `Iterations`; then
`takt scorecard --save` under the binary's own revision. The command exits 2 when the runs do not
take that shape. The defect is there when the save is refused naming the harness layer as
recorded by one run and `none` by the other; it is gone when the scorecard is saved, exit 0.

```expected_before
scenario: 1
command: h=$HOME; printf '%s\n' '{"type":"system","subtype":"init","model":"fake-model","claude_code_version":"2.1.309","tools":["Read","Edit","Write","Glob","Grep"]}' '{"type":"result","subtype":"success","result":"{\"action\": \"tests.write\", \"summary\": \"a test\"}","usage":{"input_tokens":3,"output_tokens":4}}' > "$h/reply.json"; printf '%s\n' '#!/bin/sh' 'if [ "$1" = --version ]; then echo "2.1.309 (Claude Code)"; exit 0; fi' 'echo "# a test" >> tests/test_todo.py' "cat $h/reply.json" > "$h/claude"; chmod +x "$h/claude"; git config user.name t && git config user.email t@example.invalid && XDG_STATE_HOME="$h/state" "$TAKT" run --repo . --intent "Remove a todo by its number" --protocol story.delivery/1 --harness claude --harness-program "$h/claude" --forge none --test-cmd ./check.sh --min-free-gib 0 --iterations 3 > "$h/run1.log" 2>&1; c=$(ls .takt/cases); XDG_STATE_HOME="$h/state" "$TAKT" run --repo . --case "$c" --harness claude --harness-program "$h/claude" --forge none --test-cmd ./check.sh --min-free-gib 0 --iterations 1 > "$h/run2.log" 2>&1; grep -q 'step 3: tests.write' "$h/run1.log" && grep -q 'step 1: tests.run' "$h/run2.log" && grep -q 'stopped: Iterations (1); steps 1,' "$h/run2.log" || { cat "$h/run1.log" "$h/run2.log"; echo "the first run took no harness step, or the resume did not take one validator step and stop" >&2; exit 2; }; r=$("$TAKT" --version | sed -n 's/.*(\([0-9a-f]\{40\}\).*/\1/p'); "$TAKT" scorecard --repo . --save "$h/sc" --revision "$r"
exit: 1
healthy_exit: 0
output: records none: the runs' layers differ
source: crates/takt/src/scorecard.rs
```

## Outcome

A run that took no harness step, whatever validator steps it took, does not make its case's runs
differ in the `harness` and `model` layers, so the set is scored and saved without
`--across-layers`.

## Context

- Wave 39's U2 (story:unqualified-stop-records-no-layer) exempted runs of 0 steps only; its
  commit names this case as left: a `Row` cannot tell harness steps from validator steps.
- `--across-layers` turns off the check for every run of the set, so using it to get past such a
  run also hides a real change of harness or model between runs.

## Acceptance

1. A case whose first run took a harness step and whose resume took only validator steps is
   scored and saved by `takt scorecard --save` without `--across-layers`; the predicate above
   gives `not_reproduced`.
2. Two runs that took harness steps on different harness versions or models are still refused
   without `--across-layers`, naming the layer and both runs, as `layers_differ` does today,
   also when a validator-only run sits between or before them.
3. A run that took a harness step and states no harness layer (an `opened` line with layers, a
   harness step line without a version) is still compared, not exempted.
4. The validator-only run still counts as a run of its case, with its outcome and its steps, in
   the printed and saved scorecard.

## Scope

- `crates/takt/src/scorecard.rs`: `Span` and `Span::row` count the run's harness steps apart
  from its validator steps, `Row` carries that count, and `layers_differ` exempts `STEP_LAYERS`
  on it instead of on `steps == 0` (cited)
- `crates/takt/tests/layers.rs`: a case whose resume takes only validator steps (cited)
