---
format: aep.planning-md/3
id: approval-record:trial-2026-10-21-bb4072a3-hand-over
kind: approval-record
status: approved
title: 'Trial 2026-10-21-bb4072a3: the pin and the context change go to the dispatcher as one candidate, then C is drafted'
relations:
- informed_by: story:step-spends-a-request-reading-its-scope-files
revision: 2
transitions:
- {from: "draft", to: "approved", at: "2026-10-21T22:45:43Z", actor: "human:timo", revision: 2}
---
## Decision

On 2026-10-22 the owner's side decided, for trial cycle 2026-10-21-bb4072a3 (base `main` at
bb4072a3, metaharness 0.11.0, Codex arm, 5 runs per set): option A, then option C.

> A, then C: hand mpin + mctx to the dispatcher as one [CANDIDATE] after the reviewer verdict;
> then draft C.

- `mpin`: commit 1 of the candidate branch, the metaharness 0.11.0 pin.
- `mctx`: commit 2, Codex steps get `AGENTS.md` and the story's scope files as the run's context.
- The trial agent hands both to the dispatcher as one `[CANDIDATE takt]` hand-over once the
  reviewer's verdict at the candidate's sha is in; the dispatcher takes it as a unit of the next
  wave (AGENTS.md § Messages).
- Then option C is drafted.

## Why recorded here

The trial agent never files to a planning store (AGENTS.md § Roles), so the dispatcher records
the decision it acts on. The figures the decision rests on (each set's run median) are in the
trial cycle's record and in the `[CANDIDATE]` body; none is restated here.
