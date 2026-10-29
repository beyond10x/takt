---
format: aep.planning-md/3
id: story:failed-gate-block-of-a-red-test-run-is-large
kind: story
status: draft
title: The failed gate block of a red test run costs a Codex step about 1,000 tokens a request
tags:
- finding
- priority:medium
- rsi
refs:
- provider: finding
  reference: failed-gate-block-of-a-red-test-run-is-large
relations:
- serves: vision:safe-agentic-coding
- informed_by: story:failed-gate-output-floods-prompt
revision: 1
---
# The failed gate block of a red test run costs a Codex step about 1,000 tokens a request

## What is wrong

After `tests.write` under tests first, the gate that runs the test command fails as expected, and
the next step prints up to 4 KiB of what it printed (`FAILED_LINES_PER_CHECK`,
`crates/takt/src/validator.rs`), on every request of the step. In the 0.18.1 trial's re-trial of
the candidate for story:codex-step-gets-each-file-once, the `implementation.edit` prompt carried a
4,241-byte block in 2 of 3 runs (two requests each); those two stopped `Tokens` at 86861 and 86930
over the 85088 cap, and the run whose block was withheld by the scan passed at 84922 (held-out
check passed). Most of the block is the test runner's tracebacks of the tests the case just wrote.
Whether it is shown at all depends on whether the output passes the scan, so the same step varies
by about 2,000 tokens a run.

A follow-up to story:failed-gate-output-floods-prompt, which chose the lines and kept the 4 KiB
bound.

## Check

The predicate was checked by a direct run only (exit 9 at 0.18.1), not yet by
`scripts/check-finding`; the reviewer runs `check-finding` before the story is scheduled.

The healthy bound (1 KiB for a red test run's gate block) is a proposal and a design call for the
planner: what a step needs is which tests failed and the summary line.

```expected_before
scenario: 1
command: printf '%s\n' '#!/bin/sh' '[ "$1" = "--version" ] && { echo "codex-cli 0.160.0"; exit 0; }' 'D=$(dirname "$0"); for a; do p=$a; done; n=$(ls "$D"/p.* 2>/dev/null | wc -l); printf "%s" "$p" > "$D/p.$n"; [ "$n" = 0 ] || exit 1' 'sed -i "s/^if __name__/    def test_remove(self):\n        self.assertEqual(self.run_cli(\"remove\", \"1\")[0], 1)\n\n\nif __name__/" tests/test_todo.py' 'd="$CODEX_HOME/sessions/2026/10/20"; mkdir -p "$d"; exec > "$d/rollout-2026-10-20T18-00-00-01a15a19-57e0-7d83-97b1-da333ebb91e5.jsonl"' 'r="Done.\\n{\\\"action\\\": \\\"tests.write\\\", \\\"summary\\\": \\\"wrote a test\\\", \\\"commit\\\": {\\\"subject\\\": \\\"test: add a test\\\", \\\"body\\\": \\\"Adds a test for removal.\\\\nIt covers the numbering.\\\\nIt covers refusals.\\\"}}"' 't="\"timestamp\":\"2026-10-20T18:00:00.000Z\""' 'printf "{%s,\"type\":\"session_meta\",\"payload\":{\"session_id\":\"01a15a19-57e0-7d83-97b1-da333ebb91e5\",\"id\":\"01a15a19-57e0-7d83-97b1-da333ebb91e5\",\"timestamp\":\"2026-10-20T18:00:00.000Z\",\"cwd\":\"%s\",\"originator\":\"codex_exec\",\"cli_version\":\"0.160.0\",\"source\":\"exec\",\"model_provider\":\"openai\"}}\n" "$t" "$PWD"' 'printf "{%s,\"type\":\"event_msg\",\"payload\":{\"type\":\"task_started\",\"turn_id\":\"01a15a19-5963-7d42-b749-21695314f7a4\"}}\n" "$t"' 'printf "{%s,\"type\":\"response_item\",\"payload\":{\"type\":\"message\",\"role\":\"assistant\",\"phase\":\"final_answer\",\"content\":[{\"type\":\"output_text\",\"text\":\"%s\"}]}}\n" "$t" "$r"' 'printf "{%s,\"type\":\"event_msg\",\"payload\":{\"type\":\"token_count\",\"info\":{\"total_token_usage\":{\"input_tokens\":100,\"cached_input_tokens\":40,\"output_tokens\":20,\"reasoning_output_tokens\":0,\"total_tokens\":120}}}}\n" "$t"' 'printf "{%s,\"type\":\"event_msg\",\"payload\":{\"type\":\"task_complete\",\"turn_id\":\"01a15a19-5963-7d42-b749-21695314f7a4\",\"last_agent_message\":\"%s\",\"duration_ms\":5}}\n" "$t" "$r"' > "$HOME/codex" && chmod +x "$HOME/codex" && printf 'scan: none\n' > "$HOME/run.yaml" && "$TAKT" run --repo . --intent "$(printf 'Remove a todo by its number\n\n## Scope\n\n- `todo.py`\n- `tests/test_todo.py`\n')" --protocol story.delivery/1 --harness codex --harness-program "$HOME/codex" --config "$HOME/run.yaml" --forge none --test-cmd ./check.sh --min-free-gib 0 --iterations 6 > "$HOME/run.log" 2>&1; test -s "$HOME/p.1" || { tail -3 "$HOME/run.log"; exit 2; }; n=$(awk '/^The last gate run failed/{f=1} f{c+=length($0)+1} f&&/^`+$/{print c; exit}' "$HOME/p.1"); echo "failed gate block: ${n:-0} bytes"; [ "${n:-0}" -gt 1024 ] && { echo "the failed gate block of a red test run is over 1 KiB"; exit 9; }; exit 0
exit: 9
healthy_exit: 0
output: the failed gate block of a red test run is over 1 KiB
source: docs/runs/2026-10-28-trial-0.18.1.md
```

The `source` is the trial's run record, to be written under `docs/runs/` from the trial's notes.

## Outcome

After a red test run that the protocol asks for (the run after `tests.write` that shows the new
tests red), the next step's failed-gate block names each failed test and the runner's summary line
within a bound the planner sets (1 KiB proposed), whether or not the rest of the output passes the
scan. A gate that fails for another reason keeps today's block.

## Acceptance

1. `scripts/check-finding` gives `reproduced` at the promoted revision and `not_reproduced` at the
   candidate, with the bound the planner set in the predicate.
2. A test: a red test run of three failing tests with tracebacks gives a block that names the
   three tests and the summary line, within the bound.
3. A re-trial on scenario 1 is not worse than the promoted baseline under the promotion rule.

## Scope

- `crates/takt/src/harness.rs`: `failed_block`, `red_tests_block` and their scanned forms (cited)
- `crates/takt/src/validator.rs`: `FAILED_LINES_PER_CHECK`, `failing_lines` (cited)
- `crates/takt/tests/failed_gate_lines.rs` (inferred)
