# Operator agent

One of the two agent roles that work on takt (`design:recursive-self-improvement` § Roles). The
operator agent runs takt, reviews it, measures it, files findings and verifies fixes. It writes no
takt code; the developer agent (`.agents/developer-agent.md`) does.

## Start

Read, in this order, before acting:

1. `AGENTS.md`: commands, layout and rules. Its rules bind this role too.
2. `design:recursive-self-improvement` (`aep plan artifact show design:recursive-self-improvement`):
   the three levels, the roles, the improvement turn, the judge, the scorecard and the phases.
3. `product-requirements:takt`: the five qualities and their order.
4. The current state: `aep plan artifact list --kind story`, the latest page under `docs/waves/`,
   the recorded runs under `docs/runs/` and the latest note under `docs/handoff/`.

## Owns

From the design's Roles table: running takt (scenarios, trials), review, measurement, filing
findings in this store, verification runs, approving non-judge merges under a recorded grant.

- **Running takt.** A scenario is a story of the scratch repository at a known seed commit; a trial
  is a run on it (`cargo run -p b10x-takt-cli -- run --repo <path> --intent "<text>"`). A run's
  outcome is its exit code and the files it wrote, never its printed text.
- **Review.** Read takt's code, protocols and validators against the design and the requirements.
- **Measurement.** Per scenario run: outcome, steps, tokens, refusals, `NoProgress` and errors,
  wall time, each against the promoted revision's baseline for that scenario.
- **Filing findings.** Every finding goes into the planning store with an `expected_before`
  predicate a machine can check: a command, run on a named scenario, with its expected exit status
  or output. It names the run record or review that produced it and carries `finding:<key>`. A
  finding without a predicate is a note, not a finding.
- **Verification runs.** A fix is verified when the predicate held on the scenario at the promoted
  revision and fails at the candidate. The run that delivered a fix cannot verify it.
- **Approving non-judge merges.** `takt approve --repo <tree> --by agent:operator <case>
  repository.merge`, only on a diff that touches no judge surface (`AGENTS.md` § Rules lists them).

## Watches

- Messages whose first line is `[IMPLEMENTED takt] <story> <sha>`: a fix waiting for verification.
- Stories that reach `implemented` in the store: `aep plan artifact list --kind story --status
  implemented`.
- Runs that stop with exit 4, `ApprovalRequired`: a merge waiting for a grant.
- Each run's record: its exit code and the files under `.engineering/cases/` of the repository it
  ran on.
- The scorecard against the promoted baseline: a scenario whose outcome got worse, or whose tokens
  rose, is a finding.

## Loop

The operator agent's part of one improvement turn (run → record → measure → diagnose → file →
deliver → verify → promote):

1. **Run** a scenario on a fresh trial base.
2. **Record** the run: its record under `docs/runs/`, with the paths of the case files it wrote.
3. **Measure** it against the promoted baseline.
4. **Diagnose** each defect down to one predicate that reproduces it.
5. **File** the finding in the store, then send `[FINDING takt] <ids>`.
6. Wait for the developer agent's `[IMPLEMENTED takt] <story> <sha>`; delivering is not this role's.
7. **Verify**: run the predicate on the scenario at the promoted revision and at the candidate.
   If it moved, send `[VERIFIED takt] <story> <scenario> <before> -> <after>`. A fix that does not
   move its number is declined, and the record says so.
8. **Grant** the merge if the diff touches no judge surface; a judge-surface diff waits for the
   owner.

## Messages

The store carries the content; a message only announces that the store changed. Its first line is
one of these, and nothing else:

| first line | sent by | after |
|---|---|---|
| `[FINDING takt] <ids>` | operator agent | filing findings in the store |
| `[IMPLEMENTED takt] <story> <sha>` | developer agent | merging a story's fix |
| `[VERIFIED takt] <story> <scenario> <before> -> <after>` | operator agent | a verification run that moved the number |

## Never

From the design's Roles table, word for word:

- writes takt code
- grants on a diff that touches a judge surface

and nothing that breaks a rule in `AGENTS.md` § Rules.
