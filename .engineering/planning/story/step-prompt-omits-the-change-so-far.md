---
format: aep.planning-md/3
id: story:step-prompt-omits-the-change-so-far
kind: story
status: implemented
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
revision: 6
transitions:
- {from: "draft", to: "proposed", at: "2026-10-20T14:29:00Z", actor: "human:timo", revision: 4}
- {from: "proposed", to: "active", at: "2026-10-20T14:29:00Z", actor: "human:timo", revision: 5}
- {from: "active", to: "implemented", at: "2026-10-20T14:29:00Z", actor: "human:timo", revision: 6, decided_on: {"recorded":{"test_result":1}}}
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

## Scope

- `crates/takt/src/harness.rs`: `StepContext` gains the change so far; `ClaudeCode::prompt`
  prints it in one block after the obligations and before the offered actions, so takt's own
  text ends the prompt. The block is fenced as data with a backtick run longer than any inside
  the diff and labelled as the case's change, not as instructions.
- `crates/takt/src/run.rs`: fills it before each judgement step from `base_commit`
  (`crates/takt/src/case.rs:418`) and `HEAD`, over the paths the case's accepted judgement steps
  committed, read from their `effect` events (`"paths": [...]`) and passed to git as the
  pathspec, so a commit on the branch by anything else stays out of the prompt; `.engineering/`
  excluded. Three texts: the diff; no
  change has been made yet; the change so far is not known (a case with no `base_commit`,
  `crates/takt/src/case.rs:412-416`, resumed).
- `crates/takt/src/git.rs`: a bounded diff reader that runs no configured code and gives the same
  bytes on every host: through the isolated path `trees_differ_outside` uses
  (`crates/takt/src/git.rs:2733-2773`: no config, no attributes, `--no-ext-diff --no-textconv`),
  with `--no-color` and fixed `-U`, diff-algorithm and rename options. `run_bounded`
  (`crates/takt/src/git.rs:102`) keeps the user's global and system config (`diff.external`,
  textconv, `color.*`) and is not used for it.
- Bounds: the diff is shown up to about 8 KiB; past it the block says the change is N bytes and is
  not shown. NUL and the other C0 controls but `\n` and `\t` are replaced, or the file shown as
  binary: the prompt is one argument (`crates/takt/src/harness.rs:612-613`), which cannot hold NUL
  and is refused at 131,072 bytes. With `B10X_GATES_POLICY` set, the diff passes
  `b10x-gates scan-text` first. The scan fails closed: a refused scan, and a scan that cannot run
  (`b10x-gates` not found, a policy that does not load, any non-zero exit), print "not shown" (as
  the push scan does, `crates/takt/src/forge.rs:2011-2042`; no policy found is "not scanned",
  never "passed", `AGENTS.md` § Commands): credentials never enter a prompt (`AGENTS.md` § Rules).
  The prompt stays in argv; moving it to standard input is another change.
- `crates/takt/tests/`: a fake harness's second prompt holds the first step's committed line, fenced;
  a first step's prompt says no change yet; a case without `base_commit` says not known; a change
  past the bound says not shown with its size; a configured `diff.external` and `color.ui=always`
  change nothing in the block; a NUL past byte 8000 of a committed file does not stop the next step;
  a commit on the branch that no accepted step made stays out of the block; with the policy set, a
  scan that refuses and a scan that cannot run both give "not shown".

## Acceptance

Revised by the step-6 review of the 0.5.0 cycle (its condition 1), because takt 0.5.0 judges the
steps a candidate targets:

- `scripts/check-finding` gives `not_reproduced` at the candidate's sha (the predicate exits 4).
- `takt scorecard --against <the trigger's saved scorecard> --steps change.handover` exits 0:
  better on `change.handover`, not worse on the run.
- Plain `takt scorecard --against` the promoted baseline's scorecard: not worse.
- Five scenario-1 trials keep the outcome: 5 of 5 `ApprovalRequired`, held-out acceptance passing.
