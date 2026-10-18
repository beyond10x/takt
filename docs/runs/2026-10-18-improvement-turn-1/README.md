# Run record: improvement turn 1, 2026-10-18

The first improvement turn that Canon accepted. takt found a defect in itself, a fix was made, and
the fix was checked before and after on scenario 1, compared on the scorecard, merged and promoted,
each step as evidence on one case. The turn ran in phase R0: **the records were written by hand**
by a recorder program, not by a takt run.

| item | value |
|---|---|
| finding | `story:scenario-run-help-exit-codes`: `takt scenario run --help` shared `takt run`'s exit-code text, so it said exit 0 can mean "frontier: nothing ready", which only `takt run` prints |
| protocol | `self.improvement/1` (`protocols/self-improvement/1.yaml`), Canon 0.6.0 |
| case | `turn-1-scenario-help`, copied unchanged into [`case/`](case/) |
| revisions | finding filed at `eb54f3a`; baseline `0a776d2`; candidate `447423c`; promotion `a0b8576` |
| input | `surface: judge` (the change touches the judge surface, so its merge needs a judge-surface grant) |
| outcome | `accepted`: `legitimate` at the last decision, [`case/decisions/0008.json`](case/decisions/0008.json) |

## Evidence

Read from [`case/evidence/`](case/evidence/) and [`case/notes.md`](case/notes.md). Times are UTC.

| record | kind and result | subject, revision | producer | observed at | source |
|---|---|---|---|---|---|
| e1 | `finding_record` filed | finding, `eb54f3a` | `planner-run` | 14:40:03 | `scripts/file-finding` at main `eb54f3a` |
| e2 | `scenario_result` reproduced | baseline, `0a776d2` | `planner-run` | 14:40:03 | `scripts/check-finding`, exit 0, "reproduced" |
| e3 | `scenario_result` not_reproduced | candidate, `447423c` | `planner-run` | 14:40:03 | `scripts/check-finding`, exit 0, "not_reproduced" |
| e4 | `scorecard_result` no_regression | candidate, `447423c` | `planner-run` | 15:46:06 | baseline runs T-1..3 and candidate runs T-7..9 of scenario 1 ([`docs/metrics/scenario-runs.jsonl`](../../metrics/scenario-runs.jsonl)) |
| e5 | `merge_observation` merged | candidate, `447423c` | `developer-run` | 16:00:30 | merged into `wave/2026-10-18-w9` at `67ab6ce` under the grant, the wave into main at `b670264`; `git merge-base --is-ancestor 447423c b670264` exits 0 |
| e6 | `promotion_record` promoted | promotion, `a0b8576` | `developer-run` | 16:02:08 | `cargo install --locked --path crates/takt-cli` at main `a0b8576`; binary sha256 `b7af9ec9f681d8c87233dc0337a77c4ae20da2a2f51fa6e4586a2cec45d35c56`, recomputed by the planner |

e4 in figures: all six runs stopped at the merge approval (exit 4) with the held-out acceptance
`pass`, so no scenario is worse, which is the protocol's definition of the claim. The candidate's
median tokens were 151,626 against the baseline's 122,438 (+24%), n=3 each. Two candidate runs
(T-7, T-9) hit the known `story:no-inspect-for-stateless-harness` stop, one extra invocation each.
The candidate changes only help text, so the difference is put down to that defect; this is an
inference. The design's promotion default (median tokens at or below the baseline plus 10%) would
fail on these figures; whether it applies at n=3 is an open question.

## Grants

| grant | given by | how | recorded |
|---|---|---|---|
| `repository.merge.judge_surface` on `447423c` | the planner, under the owner's delegation on record (decision at `3ad0bd9`) | a grant message on `447423c`, 2026-10-18 | `case/authority.yaml`, 16:00:22 |
| the change `story:grant-binds-to-action-subject`, which made the merge grant recordable in this case | the owner | merged into wave 10 at `93bd7c3` | the wave's merge |
| `takt.promote` on `447423c` | the planner | | `case/authority.yaml`, 16:00:37 |

## Canon's final decision

The last decision, [`case/decisions/0008.json`](case/decisions/0008.json), computed at
2026-10-18T16:02:08Z from e1–e6 and both grants. Its outcomes, verbatim:

```json
"outcomes": {
  "abandoned": { "reasons": [ { "decision": "explicitly_abandoned", "present": false } ], "status": "blocked" },
  "accepted": { "status": "legitimate" },
  "declined": { "reasons": [ { "evidence": "scenario_result", "present": true, "subject": "candidate" } ], "status": "blocked" },
  "unreproducible": { "reasons": [ { "evidence": "scenario_result", "present": true, "subject": "baseline" } ], "status": "blocked" }
}
```

All six claims hold (`finding.cited`, `scenario.reproduced_before`, `scenario.passes_after`,
`scorecard.no_regression`, `fix.merged`, `promoted`), and the obligations `reproduction.required`,
`verification.required` and `regression.required` are discharged. Decisions 0000 to 0007 have
`accepted` blocked; `notes.md` says "accepted legitimate at decision 7", but the file that has it is
`0008.json`, the ninth decision counted from `0000.json`.

## Deviations from the protocol as designed

- **Hand records.** Phase R0: a recorder program wrote the evidence and asked Canon for each
  decision; no takt run drove the case.
- **The order of e4.** The scorecard was recorded while `scorecard.compare` was still blocked on
  `fix.merged`: the comparison ran on the candidate `447423c` before the merge could be recorded in
  this case. The candidate reached main unchanged, so the compared revision is the merged one.
- **The producer of e2–e4 is the planner, not the reviewer.** The case's verifier identity is
  `planner-run`, independent of the deliverer `developer-run`; the role that ran them is not the one
  the design names.
- **Tokens came from a shim, not from events.** The e4 figures are takt's stop-line sums, recorded
  per step by a `claude` shim, because `447423c` predates the run's `events.jsonl`.
- **The promotion is tied by sha256.** The binary embeds no revision (`takt --version` prints
  `takt 0.1.0`), so e6 is a build record tied to `a0b8576` by the binary's sha256, not the
  embedded-revision check the protocol describes.
- **The recorder is outside the repository.** It is a separate program, built against an export of
  `93bd7c3`. The case was recorded in a scratch tree outside the repository, so `case/branch.yaml`
  names that tree's first commit (`a53ac13`), not a takt commit.

## How to re-check

```
scripts/check-finding --scenarios <checkout of github.com/beyond10x/todo-app-example> story:scenario-run-help-exit-codes 0a776d2
scripts/check-finding --scenarios <checkout of github.com/beyond10x/todo-app-example> story:scenario-run-help-exit-codes 447423c
sha256sum ~/.cargo/bin/takt
```

The first prints `reproduced`, the second `not_reproduced`. The hash in e6 is of the binary installed
on 2026-10-18; compare it with the installed binary before reading e6 as current. A rebuild of
`a0b8576` need not give the same hash.
