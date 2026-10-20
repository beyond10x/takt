---
format: aep.planning-md/3
id: story:step-prompt-omits-the-change-so-far
kind: story
status: draft
title: A judgement step's prompt omits the change its case has made, so the handover step re-reads the files to learn it
tags:
- finding
- rsi
refs:
- provider: finding
  reference: step-prompt-omits-the-change-so-far
relations:
- decomposes: epic:recursive-self-improvement
- serves: vision:safe-agentic-coding
revision: 2
---
# A judgement step's prompt omits the change its case has made, so the handover step re-reads the files to learn it

## Outcome

Each judgement step's prompt carries the change the case has made so far: the diff from the
case's `base_commit` to `HEAD` over the paths its accepted judgement steps committed
(`.engineering/` left out), bounded in bytes; or a line that says no change has been made yet; or,
for a case whose `base_commit` is not recorded, a line that says the change so far is not known.
A step can then tell a fresh case from a finished change without a tool call, and the
`change.handover` step of scenario 1 can hand over in one model request instead of two.

## Context

Found by the token proposer of the trial of takt 0.3.0 (bee77822) on scenario 1, five runs.

- The prompt is the intent, the protocol and step, the claims, the open obligations and the
  offered actions (`crates/takt/src/harness.rs:194-237`); `StepContext` has no field for what the
  case changed (`crates/takt/src/harness.rs:18-41`, built at `crates/takt/src/run.rs:7856-7866`).
  Each step is a fresh `claude -p` with `--no-session-persistence`, so nothing a step read is
  there at the next (`crates/takt/src/harness.rs:571-575`).
- So the prompts of step 3 (`tests.write`, nothing changed yet) and step 9 (`change.handover`,
  tests and implementation committed) are byte-identical but for the step number, in all five
  runs: the same claims (`gate.green: true`, `tests.pass: true`, `change.made: unknown`) and the
  same offered actions. Only the files tell the two states apart.
- In all five runs step 9 makes two requests: the first reads `todo.py` and `tests/test_todo.py`,
  the second hands over. Step 9 costs 15,875 to 15,962 tokens a run (24% of the median run,
  67,030). In the median run, request 1 reads 4,816 cached tokens and writes 1,616; request 2
  reads 6,432 and writes 2,512 (the two files) before a reply of 582 output tokens for the step.
- The case's change at step 9 is 2,718 bytes of `git diff` (the median run's tree, take commit to
  `HEAD`); at step 6 it is 1,318 bytes (the tests only).

Estimate (inferred, not run), a range the re-trial decides:

- Best case: step 9 hands over in its first request, about 8,000 to 8,200 tokens instead of
  15,962; step 6 pays the tests' diff in each of its three requests. Net about -5,900 tokens a
  run, a median near 61,100.
- Worst case: the model still reads both files in step 9 (and Claude Code's Edit needs a prior
  Read in step 6), so the diff is paid in every request: about +1,720 in step 6, +2,400 in step 9,
  about +4,100 a run, a median near 71,100.

```expected_before
scenario: 1
command: mkdir -p "$HOME/bin" "$HOME/fake" && printf '%s\n' '{"type":"result","subtype":"success","result":"{\"action\": \"tests.write\", \"summary\": \"a marker line\"}"}' > "$HOME/fake/tests-write.json" && printf '%s\n' '{"type":"result","subtype":"success","result":"{\"action\": \"repository.merge\", \"summary\": \"the change so far is in the prompt\"}"}' > "$HOME/fake/approval.json" && printf '%s\n' '#!/bin/sh' 'f="$HOME/fake"' 'in=$(cat)' 'case "$* $in" in *takt-marker-of-the-first-step*) echo prompt-carries-the-change-so-far >&2; cat "$f/approval.json"; exit 0 ;; esac' 'if [ -e "$f/called" ]; then echo prompt-omits-the-change-so-far >&2; exit 1; fi' 'touch "$f/called"' 'echo "# takt-marker-of-the-first-step" >> tests/test_todo.py' 'cat "$f/tests-write.json"' > "$HOME/bin/claude" && chmod +x "$HOME/bin/claude" && git config user.name t && git config user.email t@example.invalid && XDG_STATE_HOME="$HOME/state" PATH="$HOME/bin:$PATH" "$TAKT" run --repo . --intent "Remove a todo by its number" --forge none --test-cmd ./check.sh --iterations 6
exit: 5
healthy_exit: 4
output: claude exited 1: prompt-omits-the-change-so-far
source: crates/takt/src/harness.rs
```

The fake `claude` performs `tests.write` at its first call (one comment line holding a marker
appended to the test file). At its second call it reads its arguments and its standard input. A
prompt without the marker: it exits 1, so the run stops on the harness error, exit 5, as at
bee77822. A prompt with the marker: it answers `repository.merge`, an action the second step
offers under its approvals, and the run stops `ApprovalRequired`, exit 4. Any other error of the
candidate stays exit 5 without the defect's text, which `scripts/check-finding` reads as `error`,
not as fixed. Checked at bee77822 outside the confinement: the defect branch gave exit 5 with
`prompt-omits-the-change-so-far`; a fake whose second call answers `repository.merge`
unconditionally gave exit 4, `stopped: ApprovalRequired (repository.merge)`.


## Trial result: candidate rejected after 2 of 3 re-trials

Candidate branch `trial-cand/2026-10-20-bee77822-1`, cut from bee77822 (0.3.0): `a26735b9` (the
diff as `-U3` hunks), then `0c9311cb` (non-test files whole, tests as hunks, the block labelled as
the complete change). The Scope above held in both: the isolated bounded reader, the 8 KiB bound,
NUL handling, the fail-closed scan, the effect-event pathspec, the three texts, the fenced block
before the offered actions. `cargo test -p b10x-takt` at `0c9311cb`: 933 passed, 0 failed. Not
checked by `scripts/check-finding` at either candidate sha; the predicate run against the
candidate's debug build outside the confinement gave exit 4, its healthy exit.

The step it targets moved. At `0c9311cb` the `change.handover` step handed over in one model
request in 5 of 5 runs, 8,405 to 8,588 tokens, against two requests and 15,875 to 15,962 tokens
in 5 of 5 runs at bee77822 (`takt report` step rows). At `a26735b9` it did so in 1 of 5: the model
still read `todo.py` to see the whole implementation.

The scenario did not. Re-trial 2 (`0c9311cb`), as `takt scorecard --against` printed it:

```
against 7ae5b05b63971efa76c1e8bb9acfc1e8f390b56e (the revision of takt under test)
rule  trials 5 a scenario  margin 1 case  tokens worse when the median exceeds the saved maximum
scenario 1  saved      n 5  tokens min 66319 median 66421 max 83589
scenario 1  candidate  n 5  tokens min 58707 median 67628 max 71082
scenario 1  median tokens +1207
scenario 1  outcomes by rank held  tokens held  not worse
```

```
scenario 1  saved      n 5  tokens min 66674 median 67030 max 84088
scenario 1  candidate  n 5  tokens min 58707 median 67628 max 71082
scenario 1  median tokens +598
scenario 1  outcomes by rank held  tokens held  not worse
```

Re-trial 1 (`a26735b9`) against the promoted baseline: median 75938, +9517, not worse.

Reason: no scenario is worse, but the target scenario is not better on tokens (median 67,628
against 66,421 and 67,030). The shortfall comes from steps the candidate does not change: in
re-trial 2, `tests.write` took an extra request in 2 of 5 runs (36,301 and 36,924 tokens, with a
`Glob **/*` among its reads) and `implementation.edit` in 1 of 5 (34,606). bee77822 had extra
requests in the same two steps in 2 of 5 runs. The two runs with none came to 58,707 and
59,627 tokens. No change inside this Scope moves those requests, so a third re-trial was not run:
it would have measured the model's variance in steps 3 and 6, not the change.

The trial's figures and rollouts are in the trial agent's cycle record, outside this repository.
