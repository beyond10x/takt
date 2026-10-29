---
format: aep.planning-md/3
id: story:takt-gate-runs-the-tests-a-change-can-break
kind: story
status: draft
title: takt's gate runs no test beyond the story's own
summary: takt's gate runs specification, citations, changelog, format and lint; a candidate that breaks an existing test reaches pr.opened green.
tags:
- finding
- priority:high
- rsi
refs:
- provider: finding
  reference: takt-gate-runs-the-tests-a-change-can-break
relations:
- serves: vision:safe-agentic-coding
revision: 1
---
## What is wrong

takt's own gate (`.engineering/gates.yaml`, commands in `.engineering/assertions-project.yaml`)
runs the specification, the citations, the changelog, the format and the lint; the story's test
command runs only the story's own tests. Nothing in a run on this repository runs the rest of the
test suite, so a candidate that breaks an existing test reaches `pr.opened` green, and under
`--forge none` or `--forge local` no continuous integration runs it either. Found in wave 43:
U6's candidate reached `pr.opened` with two existing tests broken (`scan_adapter_cli` asserting
the old wording, and a new test file without its own home, which
`tests_have_a_home_of_their_own` refuses); the adversary found them.

## Check

```expected_before
command: grep -q 'process.succeeded("tests' .engineering/gates.yaml && { echo "the gate runs tests"; exit 0; } || { echo "the gate runs no test beyond the story's own"; exit 9; }
exit: 9
healthy_exit: 0
output: the gate runs no test beyond the story's own
```

## Outcome

takt's gate runs the tests a change can break, within a bound a run can afford: the test targets
of the crates the change touches and the guard tests (`effects`, `no_own_evaluation`,
`tests_have_a_home_of_their_own`), with a timeout that fits them.

## Acceptance

1. The predicate above exits 0.
2. A candidate that breaks an existing test outside the story's test command fails the gate,
   shown on a fixture repository whose gate document names a tests process.
