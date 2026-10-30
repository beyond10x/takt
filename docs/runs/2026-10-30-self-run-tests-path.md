# Self-run: takt 0.23.0 delivers a story whose fix spans two functions

One takt run on 2026-10-30 (UTC), the recorded self-run that Acceptance 3 of
`story:implementation-prompt-shows-the-tests-path` asks for. takt 0.23.0, the release, delivered
`story:credential-shape-sees-a-token-after-a-word-character` of its own planning store in a
scratch clone of this repository. The story's fix needs two functions changed:
`credential_shape` (`crates/takt/src/run.rs`) and the failed-gate reducer its test reads,
`failing_lines` (`crates/takt/src/validator.rs`). The run's tests passed after its second
implementation edit, and the repository's gate passed at the handed-over revision. It stopped at
the pull request, `Unperformable (pr.opened; nothing performs it without the forge)`, exit 7,
because the run had `--forge none`. No pull request was opened, read or merged.

Every figure below is read from the run's `run.log` (its step lines and its `stopped:` line) or
from the case's `events.jsonl` (its `opened` line and its harness step lines). The record is kept
outside the repository: `run.log`, `exit.txt`, `commits.txt`, `diff.patch`, the case directory
and the step transcripts.

## Setup

| item | value | source |
|---|---|---|
| takt | `takt 0.23.0 (f6fb9c85655e21f17acd2a1fbca5de1b6d2c42c0)`, the release asset `takt-0.23.0-x86_64-unknown-linux-gnu.tar.gz`, checked against the release's `SHA256SUMS` | `run.log` first line; `opened` line `binary` |
| story | `story:credential-shape-sees-a-token-after-a-word-character`: a token of every shape `credential_shape` knows is found wherever it starts, also right after a word character, so every caller withholds it | `frontier:` line |
| base | `eaee0e24`, a local commit above `0ed03023` (public) that moves the story to `proposed`; it is not in the public history. Base branch `integration/takt`. The same base as self-run 3 of 2026-10-28 | `run.log` first line; `change.read` line |
| harness | Codex, `codex-cli 0.166.0`, model `gpt-6.1-terra`; takt's preflight advised that the adapter is pinned to 0.153.4 and 0.160.0 (row H9, a gap, not a refusal) | `opened` line `harness_program`; step lines `model` |
| command | `takt run --repo <tree> --story story:credential-shape-sees-a-token-after-a-word-character --protocol story.delivery/3 --input tests_first=required --input gate_on=after_handover --harness codex --model gpt-6.1-terra --forge none --test-cmd "b10x-build-slot cargo test -p b10x-takt --lib --test failed_gate_lines" --iterations 40 --min-free-gib 26 --commit-identity <identity>` | the runner script; `settings:` line |
| confinement | none: the test command ran on the host, unconfined | `case` line |
| gate | `.engineering/gates.yaml` of the base: specification, citations, changelog, format, lint | `gate.run` line |

## The run

| step | action | result | tokens |
|---|---|---|---|
| 1 | `tests.run` | pass, 187 passed | |
| 2 | `tests.write` | committed `crates/takt/tests/failed_gate_lines.rs` (37f36884) | 175,824 |
| 3 | `tests.run` | fail: `a_failed_test_name_with_a_joined_credential_is_withheld`, `credentials_joined_to_words_are_detected` | |
| 4 | `implementation.edit` | committed `crates/takt/src/run.rs` (a1902a3e), in `credential_shape` | 232,489 |
| 5 | `tests.run` | fail: `a_failed_test_name_with_a_joined_credential_is_withheld`; 188 passed | |
| 6 | `implementation.edit` | committed `crates/takt/src/validator.rs` (efaa9126), in `failing_lines` | 183,846 |
| 7 | `tests.run` | pass, 189 passed | |
| 8 | `change.handover` | no file changed | 34,428 |
| 9 | `change.read` | ahead of `integration/takt` at `eaee0e24`; 1 test path and 2 implementation paths | |
| 10 | `gate.run` | pass: every gate true | |

`candidate: 3cd6ac772d110e5b3fd7ca396458d38116d75b90`; `stopped: Unperformable (pr.opened;
nothing performs it without the forge); steps 10, tokens 626587, wall 6m17s, commits 3 (3
paths)`; exit 7.

## What the implementation prompts held

The step 4 prompt named each failing test with the file and line of its failing assertion, read
from the test output:

```text
- a_failed_test_name_with_a_joined_credential_is_withheld at crates/takt/tests/failed_gate_lines.rs:761:5
- credentials_joined_to_words_are_detected at crates/takt/tests/failed_gate_lines.rs:747:9
```

and asked to "read each failing test from its assertion and follow the calls from there to the
behaviour the intent names. Change every function on that path that the test's outcome depends
on, the callers of what the intent names among them, not only the function the intent names".
The step 6 prompt named the one test still failing, at `failed_gate_lines.rs:761:5`, with the
same request. Step 4 changed the function the story names; step 6 changed the reducer the second
test goes through (`if !reading.tests.is_empty() || reading.unnamed > 0`), and the tests passed.

## Against self-run 3

Self-run 3 (2026-10-28, takt 0.18.0 at `cb9012a2`, codex 0.165.0 as its harness line names it, the same story, base,
model and test command, `story.delivery/2`) changed `credential_shape` in one edit, left the test
red, and stopped `RepeatedTestFailure` after 5 steps and 272,988 tokens: the failed-gate reducer
was not changed. This run changed both functions and reached a passing test and a green gate.

## Not proven by this run

- That the candidate is correct beyond the two tests and the gate: no review verdict is recorded.
- A delivery with a forge: every step past `pr.opened` (`pr.open`, `ci.read`,
  `repository.merge`).
- That the prompt's assertion locations caused the second edit: one run, no run of 0.23.0
  without them.
- The same with another harness, model or story.
