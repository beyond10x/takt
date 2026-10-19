# Run record: improvement turns 2 and 3 and the first promotion decided by the scorecard, 2026-10-18 to 2026-10-19

Two more improvement turns that Canon accepted, and one promotion that serves both. Each turn found
a defect in takt, checked it before and after on scenario 1, compared the candidate with the
baseline on the scorecard and merged the fix under a grant. Both fixes then reached `main`, and
`takt promote` installed `44f96a4`, which holds both fixes, once its own five runs showed no
scenario worse than the baseline. That one promotion serves both cases' `promoted` claim. Unlike
turn 1's, it had no `takt.promote` grant: the scorecard's verdict
decided it. The turns ran in phase R0 like turn 1, so **the case records were written by hand** by
the recorder program, not by a takt run.

| item | turn 2 | turn 3 |
|---|---|---|
| finding | `story:case-id-is-a-valid-branch-name`: `takt run --case-id 'a~b'` passed takt's own id check and then failed inside `git checkout -b`, because `takt/a~b` is not a valid branch name | `story:hours-bounds-a-harness-step`: `--hours` was checked only between steps, and a harness step kept its own 30-minute timeout, so a step could run past the budget |
| protocol | `self.improvement/1`, Canon 0.6.0 | the same |
| case | `turn-2-case-id`, copied unchanged into [`cases/turn-2-case-id/`](cases/turn-2-case-id/) | `turn-3-hours`, copied unchanged into [`cases/turn-3-hours/`](cases/turn-3-hours/) |
| revisions on `main` | finding `33e8c01`, baseline `a0b8576`, candidate `98719c3`, promotion `44f96a4` | finding `c220a80`, baseline `a0b8576`, candidate `9fb14cc`, promotion `44f96a4` |
| input | `surface: judge` | `surface: judge` |
| outcome | `accepted`: `legitimate` at the last decision, [`decisions/0009.json`](cases/turn-2-case-id/decisions/0009.json) | `accepted`: `legitimate` at the last decision, [`decisions/0008.json`](cases/turn-3-hours/decisions/0008.json) |

The promotion is in [`promotion/`](promotion/): the promotion record, the scorecard it was decided
on, and `takt promote`'s output.

## Commit ids in the case files

The case files were written before this repository's history was rewritten on 2026-10-19, and most
commit ids changed then. They are kept byte for byte as written, so they name the old ids. The
rewrite's commit map (one `old new` pair per line) gives the id each has on `main`:

| in the case files | on `main` | what it is | named in |
|---|---|---|---|
| `0e5d555` | `33e8c01` | merge of the store change that filed turn 2's finding | turn 2: `case.yaml`, e1 |
| `0f8d8c2` | `8e041e6` | the commit that filed turn 2's finding | turn 2: `notes.md` |
| `9922f12` | `333d369` | wave 10's merge into `main`, a second revision turn 2 reproduced at | turn 2: `notes.md` |
| `8d49743` | `d74da0c` | turn 2's first candidate, measured before review and superseded | turn 2: `notes.md` |
| `834205f` | `98719c3` | turn 2's candidate, "correction 2 after adversary pass 2" | turn 2: `case.yaml`, `authority.yaml`, e3 to e5, `notes.md` |
| `4fbdec6` | `0dd0b46` | the commit that filed turn 3's finding | turn 3: `notes.md` |
| `a9dd0d3` | `c220a80` | the store commit that retired `story:cli-small-fixes`, on `main` with turn 3's finding | turn 3: `case.yaml`, e1, `notes.md` |
| `cfbf7e4` | `9fb14cc` | turn 3's candidate, "correction 3 after review" | turn 3: `case.yaml`, `authority.yaml`, e3 to e5, `notes.md` |
| `3b0b748` | `76be9ed` | the merge of turn 3's candidate into `wave/2026-10-18-w12` | turn 3: `notes.md` |

`a0b8576`, the baseline, kept its id. `44f96a4`, the promotion, was written after the rewrite and is
an id on `main`. `a53ac13` in each `branch.yaml` is the first commit of the recorder's scratch tree,
as in turn 1, not a takt commit. Every id in the `on main` column is an ancestor of `44f96a4`
(`git merge-base --is-ancestor`).

## Evidence

Read from each case's `evidence/` and `notes.md`. Times are UTC. A revision is given as the case
file has it, with its id on `main` in brackets where the two differ.

### Turn 2

| record | kind and result | subject, revision | producer | observed at | source |
|---|---|---|---|---|---|
| e1 | `finding_record` filed | finding, `0e5d555` (`33e8c01`) | `planner-run` | 10-18 19:23:16 | `scripts/file-finding` in `0f8d8c2` (`8e041e6`), on `main` at `0e5d555` |
| e2 | `scenario_result` reproduced | baseline, `a0b8576` | `planner-run` | 10-18 19:23:16 | `scripts/check-finding` at `a0b8576`: "reproduced"; the check's output was written at 18:39:22 |
| e3 | `scenario_result` not_reproduced | candidate, `834205f` (`98719c3`) | `planner-run` | 10-18 19:54:29 | `scripts/check-finding` at `834205f`: "not_reproduced"; output written at 19:41:19 |
| e4 | `scorecard_result` no_regression | candidate, `834205f` (`98719c3`) | `planner-run` | 10-18 19:54:29 | baseline runs T-14..18 at `a0b8576`, candidate runs T-24..28 at `834205f` |
| e5 | `merge_observation` merged | candidate, `834205f` (`98719c3`) | `planner-run` | 10-19 05:03:03 | see [Merge and promotion](#merge-and-promotion) |
| e6 | `promotion_record` promoted | promotion, `44f96a4` | `planner-run` | 10-19 05:03:03 | the promotion record, [`promotion/44f96a4….json`](promotion/44f96a48de6b0a89db6801fd1d85372896e98781.json) |

e4 in figures, from `notes.md`: all ten runs stopped at the merge approval (exit 4) with the
held-out acceptance PASS. Tokens: baseline 122,361 / 138,160 / 107,205 / 122,811 / 122,192 (median
122,361, maximum 138,160); candidate 122,095 / 153,345 / 122,465 / 134,828 / 122,177 (median
122,465). Under the promotion rule decided on 2026-10-18 (n=5 a side; tokens are worse when the
candidate's median is above the baseline's maximum) the candidate is worse on nothing. T-27 stopped
once on `NoProgress` and the runner resumed it once before it reached the approval; the baseline
runs had no resume. The candidate first measured, `8d49743` (`d74da0c`, runs T-19..23, median
122,202), was superseded when review moved the candidate to `834205f`.

### Turn 3

| record | kind and result | subject, revision | producer | observed at | source |
|---|---|---|---|---|---|
| e1 | `finding_record` filed | finding, `a9dd0d3` (`c220a80`) | `planner-run` | 10-18 18:34:15 | filed in `4fbdec6` (`0dd0b46`) at 18:31:07, on `main` at `a9dd0d3`; observed at is `a9dd0d3`'s commit time |
| e2 | `scenario_result` reproduced | baseline, `a0b8576` | `planner-run` | 10-18 22:32:50 | `scripts/check-finding` at `a0b8576`: exit 0, "reproduced" |
| e3 | `scenario_result` not_reproduced | candidate, `cfbf7e4` (`9fb14cc`) | `planner-run` | 10-18 23:32:28 | `scripts/check-finding` at `cfbf7e4`: exit 1, "not_reproduced" |
| e4 | `scorecard_result` no_regression | candidate, `cfbf7e4` (`9fb14cc`) | `planner-run` | 10-18 23:40:27 | baseline runs T-14..18 at `a0b8576`, candidate runs T-39..43 at `cfbf7e4` |
| e5 | `merge_observation` merged | candidate, `cfbf7e4` (`9fb14cc`) | `planner-run` | 10-19 05:03:03 | see [Merge and promotion](#merge-and-promotion) |
| e6 | `promotion_record` promoted | promotion, `44f96a4` | `planner-run` | 10-19 05:03:03 | the promotion record |

e4 in figures, from `notes.md`: all ten runs reached the merge approval with the held-out acceptance
PASS. Candidate tokens 122,243 / 122,151 / 122,055 / 121,884 / 153,293, median 122,151 against the
baseline's maximum 138,160 (the same baseline runs as turn 2's): worse on nothing. The candidate
binary's sha256 is `eaa32ab7…4018`, named in the measurement's summary.

## Merge and promotion

| step | turn 2 | turn 3 | read from |
|---|---|---|---|
| merged into `wave/2026-10-18-w12` | at `0cb62c0`, 2026-10-18 20:47:47 | at `76be9ed`, 2026-10-18 23:32:23 | the merge commits |
| the wave merged into `main` | at `06e3227`, 2026-10-19 04:04:43 | the same merge | the merge commit |
| promoted | `44f96a4`, 2026-10-19 05:02:23 | the same promotion | the promotion record |

Both cases have no line in `notes.md` for e5 or e6. The recorder script that wrote them names them
"e5 (merge_observation) and e6 (promotion_record) for R0 turns 2 and 3, after the scorecard-decided
promotion of 44f96a4", and it stops before writing anything unless `takt --version` prints
`takt 0.1.0 (44f96a48de6b0a89db6801fd1d85372896e98781)`. The merge times above are commit times.

### The promotion's scorecard

`takt promote` compared five runs of scenario 1 at `44f96a4` with the saved baseline, `a0b8576`
(T-14..18). The five runs are T-44..48. Each ran the same debug build, sha256 `8ded63ac…76eb`
(each run's `binary.txt`); the scorecard names its revision `44f96a4` (`named_by: build`). It is not
the binary the promotion then built and installed.

| run | case | outcome | exit | resumes | steps | tokens | wall | held-out acceptance | pull request |
|---|---|---|---|---|---|---|---|---|---|
| T-44 | `scenario-1-trial-71` | ApprovalRequired | 4 | 0 | 13 | 121,495 | 104 s | PASS | https://github.com/beyond10x/todo-app-example/pull/66 |
| T-45 | `scenario-1-trial-72` | ApprovalRequired | 4 | 0 | 13 | 121,906 | 90 s | PASS | https://github.com/beyond10x/todo-app-example/pull/67 |
| T-46 | `scenario-1-trial-73` | ApprovalRequired | 4 | 0 | 13 | 122,058 | 94 s | PASS | https://github.com/beyond10x/todo-app-example/pull/68 |
| T-47 | `scenario-1-trial-74` | ApprovalRequired | 4 | 0 | 13 | 121,952 | 99 s | PASS | https://github.com/beyond10x/todo-app-example/pull/69 |
| T-48 | `scenario-1-trial-75` | ApprovalRequired | 4 | 0 | 13 | 121,986 | 88 s | PASS | https://github.com/beyond10x/todo-app-example/pull/70 |

Outcome, steps and tokens are the scorecard's rows,
[`promotion/scorecard-44f96a4….json`](promotion/scorecard-44f96a48de6b0a89db6801fd1d85372896e98781.json);
exit, resumes, wall time, the held-out acceptance and the pull request are each run's
`result.json`.

- **Held-out acceptance**: 5 of 5 PASS. **Outcome**: 5 of 5 `ApprovalRequired`.
- **Tokens**: minimum 121,495, median 121,952, maximum 122,058. The saved baseline: minimum
  107,205, median 122,361, maximum 138,160. The median is 409 tokens below the baseline's.
- **Rule**: 5 trials a scenario, a margin of 1 case, "tokens worse when the median exceeds the saved
  maximum". The median, 121,952, is under 138,160.
- **Verdict**, verbatim: "no scenario worse than the baseline". The promotion record has `worse: 0`.

From the baseline's rows in [`docs/metrics/README.md`](../../metrics/README.md): T-14..18 took 18
steps each and a median of 127 s; T-44..48 took 13 steps each and a median of 94 s. The records do
not say why. The scorecard states no cost.

### The build and the install

From [`promotion/promote.out`](promotion/promote.out), exit 0. `takt promote` cloned `44f96a4` into
a fresh directory, then ran `cargo build --locked --offline --release -p b10x-takt-cli --bin takt`
and `cargo install --locked --offline --force --path /work/crates/takt-cli`, each "confined in
bwrap". It installed `~/.cargo/bin/takt` with sha256
`0146e9dcfcfbff0be5c5e7ff27ccb7824ad13bf935864dc5fbeb968649b5c269` and wrote the promotion record.

The record ([`promotion/44f96a4….json`](promotion/44f96a48de6b0a89db6801fd1d85372896e98781.json))
names the revision, its tree `0986f1f0…c29d` (the tree of `44f96a4` on `main`), the time
2026-10-19T05:02:23Z, the baseline, the rule, `worse: 0`, the installed binary's sha256 and the
sha256 of the binary it replaced, `d10dd5e1…58c5`. It ties the scorecard by sha256,
`25b32aa6…c2a6`; [`promotion/44f96a4….scorecard.json`](promotion/44f96a48de6b0a89db6801fd1d85372896e98781.scorecard.json),
the copy kept beside the record, is byte-identical to the candidate scorecard and has that hash.
Its `revision_stated_by` is `null` and its `runs` list is empty.

On 2026-10-19, after the promotion, `sha256sum ~/.cargo/bin/takt` printed the same hash, and
`takt --version` printed `takt 0.1.0 (44f96a48de6b0a89db6801fd1d85372896e98781)`.

## Grants

| grant | given by | recorded |
|---|---|---|
| turn 2: `repository.merge.judge_surface` on `834205f` (`98719c3`) | `agent:planner`, on the reviewer's verdict "approve" | `cases/turn-2-case-id/authority.yaml`, 2026-10-18T19:54:33Z |
| turn 3: `repository.merge.judge_surface` on `cfbf7e4` (`9fb14cc`) | `agent:planner`, on the reviewer's verdict "approve" | `cases/turn-3-hours/authority.yaml`, 2026-10-18T23:32:23Z, the time of the merge commit that followed the grant; the grant's own time was not recorded |
| `takt.promote` | none, in either case | |

## Canon's final decisions

Turn 2's last decision, [`decisions/0009.json`](cases/turn-2-case-id/decisions/0009.json), and turn
3's, [`decisions/0008.json`](cases/turn-3-hours/decisions/0008.json), were both computed at
2026-10-19T05:03:03Z from e1 to e6 and the merge grant. Their outcomes are the same, verbatim:

```json
"outcomes": {
  "abandoned": { "reasons": [ { "decision": "explicitly_abandoned", "present": false } ], "status": "blocked" },
  "accepted": { "status": "legitimate" },
  "declined": { "reasons": [ { "evidence": "scenario_result", "present": true, "subject": "candidate" } ], "status": "blocked" },
  "unreproducible": { "reasons": [ { "evidence": "scenario_result", "present": true, "subject": "baseline" } ], "status": "blocked" }
}
```

In both, all six claims hold (`finding.cited`, `scenario.reproduced_before`,
`scenario.passes_after`, `scorecard.no_regression`, `fix.merged`, `promoted`), and the obligations
`reproduction.required`, `verification.required` and `regression.required` are discharged. Every
earlier decision has `accepted` blocked: turn 2's 0000 to 0008, turn 3's 0000 to 0007. The action
`binary.promote` stays `approval-required` (`takt.promote`: none), because no grant was recorded;
the claim `promoted` holds on e6.

## Deviations from the protocol as designed

- **Hand records.** Phase R0: the recorder program wrote the evidence and asked Canon for each
  decision; no takt run drove either case.
- **No promotion grant.** Turn 1's case holds a `takt.promote` grant; these two hold none. The
  promotion went ahead on the scorecard's verdict, so `binary.promote` is still approval-required at
  the last decision.
- **The producer of every record is the planner.** e1 to e6 are all `planner-run`, e5 and e6
  included, which turn 1 recorded as `developer-run`. The verifier identity is independent of the
  deliverer, `developer-run`; the role that ran the checks is not the one the design names.
- **Turn 2's times are recording times.** Its e2 and e3 carry the time the recorder wrote them,
  later than the checks' output files; turn 3's e2 to e4 carry the output files' times.
- **e5 and e6 have no notes.** Neither `notes.md` says what e5 observed; the merge commits above
  are read from `main`.
- **The scorecard's runs used a debug build, not the installed binary.** T-44..48 ran a build whose
  hash differs from the promoted binary's, and the scorecard's rows carry `binary: null`.
- **The protocol copy is older than the repository's.** Each case holds
  `protocols/self-improvement/1.yaml` as it was when the case opened. At `44f96a4` four lines of
  that file's descriptions also allow a repository at a seed in place of a scenario; nothing else
  differs.

## How to re-check

```
scripts/check-finding --scenarios <checkout of github.com/beyond10x/todo-app-example> story:case-id-is-a-valid-branch-name a0b8576
scripts/check-finding --scenarios <checkout of github.com/beyond10x/todo-app-example> story:case-id-is-a-valid-branch-name 98719c3
scripts/check-finding --scenarios <checkout of github.com/beyond10x/todo-app-example> story:hours-bounds-a-harness-step a0b8576
scripts/check-finding --scenarios <checkout of github.com/beyond10x/todo-app-example> story:hours-bounds-a-harness-step 9fb14cc
git merge-base --is-ancestor 98719c3 06e3227 && git merge-base --is-ancestor 9fb14cc 06e3227
sha256sum ~/.cargo/bin/takt
takt --version
```

The first and third print `reproduced`, the second and fourth `not_reproduced`. The hash in the
promotion record is of the binary installed on 2026-10-19; compare it with the installed binary
before reading e6 as current. A rebuild of `44f96a4` need not give the same hash.
