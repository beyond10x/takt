---
format: aep.planning-md/3
id: story:implementation-prompt-shows-the-tests-path
kind: story
status: active
title: An implementation step is not shown the path from the failing test to the code
tags:
- finding
- priority:medium
- rsi
refs:
- provider: finding
  reference: implementation-prompt-shows-the-tests-path
relations:
- serves: vision:safe-agentic-coding
scope:
- confidence: inferred
  path: crates/takt/src/harness.rs
revision: 4
transitions:
- {from: "draft", to: "proposed", at: "2026-10-28T22:21:39Z", actor: "human:timo", revision: 3}
- {from: "proposed", to: "active", at: "2026-10-28T22:21:39Z", actor: "human:timo", revision: 4}
---
# An implementation step is not shown the path from the failing test to the code

## What is wrong

In two self-runs of takt on its own stories (2026-10-28), the harness changed the function the
story names and not the code the failing test goes through:

- On story:a-duplicate-yaml-key-is-refused-at-its-line (claude harness, takt 0.18.0), it copied a
  hand-written locator already in the file instead of reading the parser's error; the adversarial
  pass found four ways around it.
- On story:credential-shape-sees-a-token-after-a-word-character (codex harness, takt at
  cb9012a2), it removed the word-character skip in `credential_shape` but did not change the
  failed-gate reducer the new test reads, so the test stayed red.

The `implementation.edit` prompt gives the failing tests' names and output but not where the
failing assertion reaches the code under change, nor the other callers of what the story names.

## Check

Not runnable by `scripts/check-finding`: it is a property of a harness's prompt. A test over the
prompt takt builds for `implementation.edit` stands in.

```expected_before
command: cargo test -p b10x-takt --lib -- implementation_prompt_names_the_failing_tests_path
exit: 101
healthy_exit: 0
output: 0 passed
source: crates/takt/src/harness.rs
```

## Outcome

The `implementation.edit` prompt names, for each failing test, the file and line of its failing
assertion, and asks the harness to change every function on the path from that assertion to the
behaviour the story names, not only the one the story names.

## Acceptance

1. The prompt for `implementation.edit` after a red `tests.run` holds each failing test's
   assertion location, read from the test output.
2. The prompt asks for every caller on the path from the assertion to the changed behaviour.
3. A self-run of a story whose fix spans two functions reaches a passing test within its
   implementation edits (recorded in the run log).

## Scope

- `crates/takt/src/harness.rs` (inferred)
