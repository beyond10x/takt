---
format: aep.planning-md/3
id: story:handover-costs-a-harness-call-of-its-own
kind: story
status: draft
title: The handover costs a harness call of its own, after the edit that completed the change
tags:
- finding
- rsi
refs:
- provider: finding
  reference: handover-costs-a-harness-call-of-its-own
relations:
- decomposes: epic:recursive-self-improvement
revision: 2
---
# The handover costs a harness call of its own, after the edit that completed the change

## Outcome

A write step whose edit completes the change can hand the change over in the same reply. takt
then records the handover choice at the revision that step commits, and the run reaches `pr.open`
without calling the harness again. On scenario 1 the run takes 12 steps instead of 13 and makes
two harness calls instead of three.

## Context

Found by the no-op-steps proposer of the trial of takt 0.6.0 (fd395d22) on scenario 1, five runs.

- Step 9, `change.handover`, is a whole Claude Code call: 15,812 to 15,898 tokens, 3 turns,
  9.7 to 12.8 s (the cycle's `takt report`, `report-trig.txt:38,94,172,220,268`). On the
  change-so-far candidate (story:step-prompt-omits-the-change-so-far) it still cost 8,476 to
  8,604 tokens, one request each, in that candidate's five re-trial runs at 0.5.0.
- It changes no file and records one choice. Its effect is `"paths":[]` (trig-3
  `events.jsonl:28`), and its only record is a `handover` with no result: subject, revision and
  producer `takt.choice`, nothing else (trig-3 `evidence/0007-handover-9.json`). Nothing else reads
  its reply: the pull request's body is the intent and the claims that hold
  (`crates/takt/src/validator.rs:1888-1904,1921-1943`).
- Compared with step 6, step 9 knows only `tests.pass: true` and `gate.green: true`, which takt
  itself recorded in steps 7 and 8. The tree is the one step 6 left, and the intent and offered
  actions are the same.
- In 43 of 43 recorded scenario-1 runs (four trial cycles, takt 0.3.0 to 0.6.0 and their
  candidates; `grep -o '"by":"harness"[^}]*"action":"[^"]*"'` over each run's `events.jsonl`),
  the harness steps were `tests.write`, `implementation.edit`, `change.handover`. Step 9 never
  edited.
- Today a write step cannot hand over. The reply names one action
  (`crates/takt/src/harness.rs:237-238`), and `parse` ignores any other field
  (`crates/takt/src/harness.rs:243-260`). A choice action that changed a file is refused and its
  change discarded (`crates/takt/src/run.rs:9737-9755`).
- The protocol does not need the extra step. `change.handover`'s precondition is only
  `change.handed_over` unknown (`protocols/story-delivery/1.yaml:135-145`), so it is offered at a
  red revision today. `pr.open` still needs `tests.pass`, `gate.green`, the handover and
  `change.made` (`protocols/story-delivery/1.yaml:173-178`). A new commit returns the handover to
  unknown (`protocols/story-delivery/1.yaml:89-95`). So a handover recorded with an edit that
  turns out red blocks `pr.open` until the next edit, and that edit resets it.

Estimate (inferred, not run):

- Best case, the model sets the field at step 6. One step and one harness call go, along with
  9.7 to 12.8 s. That is -15,812 tokens a run at 0.6.0, or -8,476 on top of the change-so-far
  candidate, minus the rider's prompt line. The line is about 60 tokens in each of the 6 to 7
  requests of steps 3 and 6, so under 500 a run. Low estimate: about -7,900 a run.
- Worst case, the model never sets the field: about +500 a run, and step 9 runs as today.

```expected_before
scenario: 1
command: mkdir -p "$HOME/bin" "$HOME/fake" && printf '%s\n' '{"type":"result","subtype":"success","result":"{\"action\": \"implementation.edit\", \"summary\": \"the edit that completes the change\", \"handover\": true}"}' > "$HOME/fake/edit.json" && printf '%s\n' '#!/bin/sh' 'f="$HOME/fake"' 'if [ -e "$f/called" ]; then echo takt-called-the-harness-again-after-its-handover >&2; exit 1; fi' 'touch "$f/called"' 'echo "# the edit of the first call" >> todo.py' 'cat "$f/edit.json"' > "$HOME/bin/claude" && chmod +x "$HOME/bin/claude" && git config user.name t && git config user.email t@example.invalid && XDG_STATE_HOME="$HOME/state" PATH="$HOME/bin:$PATH" "$TAKT" run --repo . --protocol story.delivery/1 --intent "Remove a todo by its number" --forge none --test-cmd ./check.sh --iterations 12 > "$HOME/out" 2>&1; rc=$?; cat "$HOME/out"; if [ "$rc" -eq 7 ]; then [ -e "$HOME/fake/called" ] && grep -q "stopped: Unperformable (pr.opened;" "$HOME/out" || exit 9; fi; exit "$rc"
exit: 5
healthy_exit: 7
output: takt-called-the-harness-again-after-its-handover
source: crates/takt/src/run.rs
```

The fake `claude`'s first call appends a comment line to `todo.py` and replies
`implementation.edit` with `"handover": true`. Its second call exits 1 and prints the defect's
text, so at fd395d22 the run stops on the harness error, exit 5: takt ran `tests.run` and
`gate.run` at the edit's revision, then called the harness a second time to learn the handover.
Once the change is in, takt records the handover at the edit's revision, then runs `tests.run`,
`gate.run` and `change.read` (ahead). With `--forge none` nothing performs `pr.open`, so the run
stops `Unperformable`, exit 7, without a second harness call. An approval answer cannot give the
healthy exit here: under `--forge none` story.delivery/1 offers the harness no approval before
`pr.opened`, because `repository.merge` needs `ci.green`. Any other failure of the candidate
gives exit 5 without the defect's text, or another status, and `scripts/check-finding` reads
either as `error`.

## Scope

- `crates/takt/src/harness.rs`: `StepReport` gains `handover: bool`. `parse` reads an optional
  boolean `"handover"`: absent means false, and any other type is an error. When `change.handover`
  is offered, the prompt says that a write action whose edit completes the change (the story's
  tests written and the implementation done) may add `"handover": true`, and that takt then
  records the handover at the revision the edit makes and opens the pull request once the tests
  and the gate pass there.
- `crates/takt/src/run.rs`: after a write step's commit, if the report sets `handover` and
  `change.handover` was offered at that step, takt writes the records of
  `shape.choices_of("change.handover")` for the new revision, as the choice branch does
  (`crates/takt/src/run.rs:9766-9806`), with their `evidence` events. Nothing is recorded for a
  refused step, a step that changed no file, or an action that is not a write.
- `protocols/story-delivery/1.yaml`: the descriptions of `handover` (:42-47) and
  `change.handover` (:135-141) name the rider. Claims, preconditions and obligations stay as
  they are.
- `crates/takt/tests/`: with a fake harness, an edit carrying the field reaches `Unperformable`
  at `pr.opened` (`--forge none`) after one harness call. Without the field the next judgement
  step is called as today. With the field on a red revision, `pr.open` stays blocked, and the
  next edit returns `change.handed_over` to unknown. The field on a refused step records nothing.
- Overlaps the change-so-far candidate in `crates/takt/src/harness.rs` (the prompt) and
  `crates/takt/src/run.rs`; it goes on top of it.

## Acceptance

- `scripts/check-finding` gives `not_reproduced` at the candidate's sha (the predicate exits 7).
- `takt scorecard --against` the trigger's saved scorecard: better on tokens, not worse on the
  outcome.
- Five scenario-1 trials keep the outcome: 5 of 5 `ApprovalRequired`, held-out acceptance
  passing. The number of runs whose `change.handover` step is gone gets recorded.

## Planner notes

- Filed 2026-10-20 from the trial agent's cycle 2026-10-20-fd395d22 (0.6.0); its reviewer accepted it on conditions, check-finding reproduced at fd395d22 (~/.cache/takt-trials/2026-10-20-fd395d22/reviews/ on the trial agent's machine). No candidate this cycle: the cycle's scope-files proposal also changes harness.rs.
- Conditions, binding on the implementation: (1) route (b), no rule change: takt records change.handover as its own step, by: takt, with no harness call and no file changed, at the first decision at R that offers it, after R's validators ran, and only when the harness step that committed R carried the handover field; ESS declares the step. Route (a) would widen the AGENTS.md exception for takt-performed actions and takes the owner's decision. (2) run.rs names no protocol action: the field applies to the one choice the decision at R offers; with none or several, nothing is recorded. (3) The predicate above is the reviewer's stricter form: exit 7 stands only with one harness call and a stop at pr.opened (a missing gates document also gives 7), else 9; tests show that a version which always hands over fails. (4) Cut from a revision holding story:step-prompt-omits-the-change-so-far (wave/23 41c2a01), check-finding reproduced there first; judged run-level against five runs at that base, not with --steps change.handover.
