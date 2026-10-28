---
format: aep.planning-md/3
id: story:failed-gate-output-floods-prompt
kind: story
status: implemented
title: A failed gate's raw output floods the next step's prompt
tags:
- finding
- rsi
refs:
- provider: finding
  reference: failed-gate-output-floods-prompt
relations:
- serves: vision:safe-agentic-coding
scope:
- confidence: cited
  path: crates/takt/src/harness.rs
- confidence: cited
  path: crates/takt/src/validator.rs
- confidence: cited
  path: crates/takt/tests/failed_gate_lines.rs
revision: 6
transitions:
- {from: "draft", to: "proposed", at: "2026-10-28T15:56:05Z", actor: "human:timo", revision: 4}
- {from: "proposed", to: "active", at: "2026-10-28T15:57:59Z", actor: "human:timo", revision: 5}
- {from: "active", to: "implemented", at: "2026-10-28T20:40:00Z", actor: "human:timo", revision: 6, decided_on: {"recorded":{"test_result":1}}, executor: "agent:planner"}
---
# A failed gate's raw output floods the next step's prompt

After a red gate, the next judgement step's prompt carries what the failed gate's command printed
(`FAILED_CHECKS_LABEL`, "The last gate run failed", `crates/takt/src/harness.rs:1147`; the block
`failed_block`, `crates/takt/src/harness.rs:1189`). The lines are chosen by `failing_lines`
(`crates/takt/src/validator.rs:212`) within 4 KiB per gate and 8 KiB over all
(`FAILED_LINES_PER_CHECK`, `FAILED_LINES_TOTAL`, `crates/takt/src/validator.rs:146-150`): every
non-blank line while they fit, so a unittest `-v` run reaches the prompt as its passing tests'
`... ok` lines, the usage text the program printed, and each error's traceback frames.

In the 0.17.0 trial, scenario 1 (openai-responses arm, 5 runs), the implementation.edit prompt of
the first run was 8786 bytes against 3516 in the 0.13.1 trial's first run on the same arm (the
step-6 transcripts, line 1). 4052 bytes of it were the `./check.sh` output, with 96 lines left
out, mostly argparse tracebacks of the tests the case had just written. Across the 5 runs the
implementation.edit step spent 23458 to 31083 tokens against 18465 to 23158 in the 0.13.1 runs,
and `takt scorecard --against` the 0.13.1 set gave a median of tokens 10051 higher.

What the step needs is which tests failed and why: each failed test's name and its last error
line, bounded, not the command's raw output.

The predicate runs scenario 1 offline on `story.delivery/1` with a scripted `claude` named by
`--harness-program` (it answers `--version` with a qualified version and replies with a fixed
action; no model is called). Its `tests.write` adds a test that calls `remove`, which the seed's
argparse refuses; the next `tests.run` and `gate.run` fail, and step 6, `implementation.edit`, is
prompted with the failed gate. The predicate reads that prompt from the step's `takt.prompt`
record. The defect is there when the block holds a `Traceback (most recent call last)` line, a
frame header that is never a test's name nor its last error line; it is gone when the block holds
none. A prompt without the block's opening line is `error`, so a fix keeps that line.

```expected_before
scenario: 1
command: h=$HOME; printf '%s\n' '{"type":"system","subtype":"init","model":"fake-model","claude_code_version":"2.1.309","tools":["Read","Edit","Write","Glob","Grep"]}' > "$h/init.json"; printf '%s\n' '{"type":"result","subtype":"success","result":"{\"action\": \"tests.write\", \"summary\": \"a failing test of remove\"}","usage":{"input_tokens":3,"output_tokens":4}}' > "$h/tw.json"; printf '%s\n' '{"type":"result","subtype":"success","result":"{\"action\": \"implementation.edit\", \"summary\": \"an edit\"}","usage":{"input_tokens":3,"output_tokens":4}}' > "$h/ie.json"; printf '%s\n' 'import unittest' 'import todo' 'class RemoveTest(unittest.TestCase):' '    def test_remove(self):' '        self.assertEqual(todo.main(["--file", "t.json", "remove", "1"]), 0)' > "$h/test_remove.py"; printf '%s\n' '#!/bin/sh' 'if [ "$1" = --version ]; then echo "2.1.309 (Claude Code)"; exit 0; fi' "cat $h/init.json" "if [ -f tests/test_remove.py ]; then echo '# edited' >> todo.py; cat $h/ie.json; else cp $h/test_remove.py tests/; cat $h/tw.json; fi" > "$h/claude"; chmod +x "$h/claude"; git config user.name t && git config user.email t@example.invalid && XDG_STATE_HOME="$h/state" "$TAKT" run --repo . --intent "Remove a todo by its number" --protocol story.delivery/1 --harness claude --harness-program "$h/claude" --forge none --test-cmd ./check.sh --min-free-gib 0 --iterations 6 > "$h/run.log" 2>&1; grep -rhs '"takt.prompt"' "$h/state/takt/transcripts" | grep 'The last gate run failed' > "$h/failed"; test -s "$h/failed" || { cat "$h/run.log"; echo "no step prompt carried a failed gate" >&2; exit 2; }; grep -q 'Traceback (most recent call last)' "$h/failed" || exit 0; echo "a failed gate's raw traceback reached the next step's prompt" >&2; exit 9
exit: 9
healthy_exit: 0
output: a failed gate's raw traceback reached the next step's prompt
source: crates/takt/src/harness.rs
```

## Outcome

After a red gate, the next judgement step's prompt names each failed test and its last error
line, within a stated bound, instead of the lines the gate's command printed.

## Context

- The 0.17.0 trial, scenario 1: the implementation.edit prompt grew from 3516 to 8786 bytes, 4052
  of them the failed gate's printed output; the median of tokens rose by 10051 against the 0.13.1
  runs on the same arm.
- The block came in 0.15.0. Its selection keeps a checker's summary and unindented lines first,
  which suits a compiler's or a citation checker's output and not a test runner's traceback.

## Acceptance

1. After a gate whose test command fails, the next judgement step's prompt names each failed test
   and its last error line, and holds none of the command's passing-test lines, usage text or
   traceback frames; the predicate above gives `not_reproduced`.
2. The block keeps its opening line (`FAILED_CHECKS_LABEL`) and stays within a stated bound in
   bytes, however many tests failed; a test left out past the bound is counted, as `omitted` is.
3. Output that names no failed test (a compiler's, a citation checker's) is still chosen by the
   rule `failing_lines` applies today.
4. A credential's shape and a private key's block are withheld as `failing_lines` withholds them.

## Scope

- `crates/takt/src/validator.rs`: `failing_lines`, `FailedCheck` and the bounds, where the gate's
  printed lines are chosen (cited)
- `crates/takt/src/harness.rs`: `failed_block`, the block of the prompt (cited)
- `crates/takt/tests/failed_gate_lines.rs`: a test runner's output reduced to failed names and
  last error lines (cited)
