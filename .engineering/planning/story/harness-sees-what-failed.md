---
format: aep.planning-md/3
id: story:harness-sees-what-failed
kind: story
status: draft
title: A judgement step is told which tests failed
tags:
- finding
refs:
- provider: finding
  reference: harness-sees-what-failed
relations:
- decomposes: epic:composable-agentic-work
revision: 2
---
## Outcome

When the last test or gate run failed, the prompt of the next judgement step names what failed: the failing tests' names and the runner's failure lines (bounded), taken from the observation takt recorded. The evidence record keeps the same lines. A case opened on a base whose own tests fail says so at open.

## Context

From the planner's operator-path variety round on takt at 12cffde (`.engineering/planning/review-result/variety-ops-a29d8c7.md`). The prompt gives the step only `tests.pass: false` (`crates/takt/src/harness.rs:117-157`), and the step may not run commands (`:149`), so on a red base or after a breaking edit the model guesses. The `tests.run` record keeps the last 5 lines of output (`crates/takt/src/validator.rs:281-305`), which in unittest or cargo are the summary, not the failure. Run H (a red base) stopped NoProgress after 9 steps with every test and gate run failing. This bears on the token cost the scorecard measures: a step that cannot see the failure spends a call finding it.

```expected_before
scenario: 1
command: sed -i "s/\[\"done\"\] = True/[\"done\"] = False/" todo.py && git -c user.name=r -c user.email=r@example.invalid commit -qam red && mkdir -p "$HOME/bin" && echo IyEvYmluL3NoCnByaW50ZiAnJXMnICIkMiIgPiAiJEhPTUUvcHJvbXB0IgplY2hvICd7InJlc3VsdCI6IntcImFjdGlvblwiOlwicmVwb3NpdG9yeS5pbnNwZWN0XCIsXCJzdW1tYXJ5XCI6XCJzXCJ9IiwidXNhZ2UiOnsiaW5wdXRfdG9rZW5zIjoxLCJvdXRwdXRfdG9rZW5zIjoxfX0nCg== | base64 -d > "$HOME/bin/claude" && chmod +x "$HOME/bin/claude" && PATH="$HOME/bin:$PATH" "$TAKT" run --repo . --forge none --iterations 3 --test-cmd "sh check.sh" >/dev/null 2>&1; grep -q test_done_marks_one_item "$HOME/prompt" || grep "tests.pass" "$HOME/prompt"
exit: 0
output: - tests.pass: false
source: .engineering/planning/review-result/variety-ops-a29d8c7.md
```

## Scope

- `crates/takt/src/harness.rs`: the prompt's evidence lines (cited)
- `crates/takt/src/validator.rs`: what `tests.run` and `gate.run` keep of a failure (cited)
- `crates/takt/src/run.rs`: a note at open when the base is red (inferred)

## Acceptance

With `done()` broken on scenario 1, the next judgement step's prompt contains `test_done_marks_one_item` and the assertion line; a passing run's prompt is unchanged; the prompt's failure section is bounded in bytes.

## From the review of test-summary-keeps-the-totals (merged at 5ac726f)

Three low findings, never filed before, folded here because this story reads the same test output:

- Only the last `failures:` list of the test output is read; a run whose earlier binaries failed names only the last binary's tests.
- The doc comment at `crates/takt/src/validator.rs:232-239@5ac726f` no longer describes what the summary keeps.
- Under `--no-fail-fast`, when a test binary aborts, the kept line is the target name, not a failing test.

Acceptance, added: with cargo output holding two `failures:` lists, the step is told the failing tests of both; with an aborted binary under `--no-fail-fast`, it is told the abort and the binary.
