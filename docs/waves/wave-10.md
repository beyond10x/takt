# Wave 10: steps that cannot forge, grants on the action's subject, the trial fetch, the scorecard

Skill: `aep:implementing` 0.23.1, wave mode. Stage: **closed**. Approval: the operator role approved this selection (option A: these four; `story:records-reach-base` in wave 11); each judge-surface merge needs the operator role's grant of its exact commit, and `grant-binds-to-action-subject` merges first. Mode: interactive.

## Units

| unit | serves | acceptance (condensed) | scope | judge surface |
|---|---|---|---|---|
| `story:step-cannot-forge-its-validators` | `vision:safe-agentic-coding` | a judgement step that changes a validator's inputs (`.cargo/**`, the test command's script, `.github/**`, `build.rs`, `rust-toolchain*`, `.engineering/**` but cases) earns no `pass` or `ci.green`: it is refused and put back | `crates/takt/src/run.rs`, `crates/takt/src/validator.rs` (cited); `crates/takt-cli/src/main.rs`, a test (inferred) | yes |
| `story:grant-binds-to-action-subject` | `vision:safe-agentic-coding` | a grant binds to the revision of the artifact its action is about, read from the protocol; a `self.improvement/1` case is granted at its `candidate` | `crates/takt/src/case.rs` (cited); a test (inferred) | yes |
| `story:trial-fetch-needs-no-user-ssh` | `vision:safe-agentic-coding` | the trial fetch needs no user credential; one that cannot authenticate fails within 30 s naming the reason | `crates/takt-cli/src/main.rs` `TrialRoute`, `crates/takt/src/forge.rs` (inferred) | yes |
| `story:scorecard-reader` | `vision:safe-agentic-coding` | `takt scorecard` prints per run the tokens summed from its events, with `--save` and `--against` | new `crates/takt/src/scorecard.rs`, `crates/takt/src/lib.rs`, `crates/takt-cli/src/main.rs` (cited); a test (inferred) | yes |

## Selection path

The operator role's option A of three, against the scopes at local main f3a8b1f. Shared files: `crates/takt-cli/src/main.rs` (step-cannot-forge: where the test command reaches the guard; trial-fetch: `TrialRoute`; scorecard-reader: a new subcommand) and `crates/takt/src/run.rs` (step-cannot-forge: the step guard; scorecard-reader: only the `events.emit` call sites its event fields need). Each brief names its functions; the coordinator checks `git merge-tree` before each merge.

`grant-binds-to-action-subject` was started before this opening, from the wave 09 branch tip cc879cb (its scope, the grant functions of `case.rs`, was untouched by the unit still under review); it merges as part of this wave.

## Left out

- `story:records-reach-base`: `case.rs` and `validator.rs`, shared with grant-binds and step-cannot-forge; wave 11.
- `story:pr-opens-when-change-is-done`, with `story:no-model-step-for-approval` and `story:no-inspect-for-stateless-harness` as one unit: `run.rs` step choice; wave 11 by collision.

## Pre-flight

| check | value |
|---|---|
| tree | `main` at b670264; `task check` exit 0 there (461 passed; 367 of 367 citations) |
| model budget | not stated; N = 2 |
| agent file | `AGENTS.md` read |
| gates policy for `scan-text` | the bot route's (`--policy` or `B10X_GATES_POLICY`) |

## Dispatch

`aep:implementor`, then `aep:adversary` (two passes at most) per unit; the last correction is verified by the coordinator. Every merge into `wave/2026-10-18-w10` waits for its grant.

## The commits this wave makes

The opening store commit, each unit's commits, the merges into `wave/2026-10-18-w10` (each under a grant), the closing store commit, the merge into local `main`. No push, no tag, no release, no pull request.

## Log

- 2026-10-18: opened on the operator role's approval; the four stories moved to active.
- 2026-10-18: opening commit 9bd9d18; three trees from it (step-forge, trial-fetch, scorecard) with implementors dispatched; main 97623a6 (the four roles) merged into the wave branch at 053102a before any unit.
- 2026-10-18: grant-binds-to-action-subject committed at 2bb77b9 (grant_subject read from the protocol; 2 of 3 cases red on the base; task check 0, 435 passed); adversary pass 1 dispatched.
- 2026-10-18: grant-binds-to-action-subject adversary pass 1 CONFIRMED (1 red, pre-existing: a precondition match naming no subject is dropped and the grant binds to the other subject; prose still says implementation; one vacuous assertion), recorded as review-result:adversary-grant-subject-pass-1; release.rollback refused (no precondition, never a guess); correction 1 dispatched.
- 2026-10-18: trial-fetch-needs-no-user-ssh committed at 8a28b46 (HTTPS URL through the bot route; GIT_TERMINAL_PROMPT=0 and BatchMode on every forge child; 30 s fetch bound; scenario_run 11→14; 464 passed); adversary pass 1 dispatched.
- 2026-10-18: grant-binds-to-action-subject correction 1 at 405f9ba (subjectless matches refused; release.rollback refused; prose; a test that could not fail fixed); the wave branch merged into the unit at 938cc5a (9 citation-only conflicts resolved by the wave side, then fix); verified by the coordinator: task check exit 0, 471 passed; one adversary pass; [GRANT?] sent.
- 2026-10-18: trial-fetch-needs-no-user-ssh adversary pass 1 INFEASIBLE (2 red, pre-existing: a repository insteadOf rule sends the fetch to the user's ssh; the user's askpass programs reach git), recorded as review-result:adversary-trial-fetch-pass-1; correction 1 dispatched (GIT_ALLOW_PROTOCOL=https, GIT_ASKPASS empty).
- 2026-10-18: scorecard-reader committed at 7545103 (takt scorecard over events.jsonl; --save, --against with n/min/median/max, outcome-only worse; events: sub-second instants, base and observed revisions, TokenUsage; StoreEvidence instant renamed observed_at (a wave 09 duplicate key); scorecard 0→14, scorecard_cli 0→4; 479 passed); the harness token pass-through (45 StepReport literals in 24 test files) left as a follow-up; adversary pass 1 dispatched.
- 2026-10-18: grant-binds-to-action-subject granted by the owner for 938cc5a and merged first, at 93bd7c3.
- 2026-10-18: step-cannot-forge-its-validators committed at 38472f0 (judge surface refused at the step and compared around validators; tests.run and ci.read record nothing when the surface differs from the base; validator_confinement tests moved to a base-committed runner; 15 new cases, both Expected-before red on the base; 476 passed); adversary pass 1 dispatched.
- 2026-10-18: trial-fetch-needs-no-user-ssh correction 1 at c04b5aa (GIT_ALLOW_PROTOCOL=https on the fetch only; GIT_ASKPASS empty on forge children); wave branch merged into the unit at 3292f38; verified by the coordinator: task check exit 0, 478 passed; pass 1 outcome fixed; [GRANT?] sent.
- 2026-10-18: scorecard-reader adversary pass 1 NEEDS-CHANGE (9 red: a resumed case is worse than itself; a missing scenario passes; a definition change turns the outcome gate off; usage not passed from the harness; wall-time spans; overflow; saves named by the scenario repository), recorded as review-result:adversary-scorecard-pass-1; correction 1 dispatched.
- 2026-10-18: step-cannot-forge-its-validators adversary pass 1 NEEDS-CHANGE (7 red, pre-existing: a manifest build key, test = false, a sourced script, env/interpreter option arguments, wrappers, make), recorded as review-result:adversary-step-forge-pass-1; correction 1 dispatched with the link rule (a guarded path crossing a link or leaving the tree is refused).
- 2026-10-18: trial-fetch-needs-no-user-ssh granted for 3292f38 and merged at e9a6deb (live fetch verified by the reviewer); the push-path low (GIT_ALLOW_PROTOCOL=https or an explicit https push URL) folded into story:scan-before-push.
- 2026-10-18: scorecard-reader correction 1 at e7e8698 (the case as the unit; median outcome rank per scenario; no run counts worse; outcome compared under the current definition; Harness::last_usage; first at; skipped lines out; saturating sums; --save needs --revision and never overwrites; 493 passed); pass 1 outcome fixed; adversary pass 2 dispatched.
- 2026-10-18: step-cannot-forge-its-validators correction 1 at 7fb9313 (manifest build keys, command parsing through env/shell options and wrappers, build-tool recipe files, links never followed; 2 adversary cases ignored with reasons as decided; 489 passed); pass 1 outcome fixed; adversary pass 2 dispatched.
- 2026-10-18: scorecard-reader adversary pass 2 NEEDS-CHANGE (the median rank misses a regression in fewer than half the cases: 68 of 200 seeded strictly worse candidates passed; an accepted case later errored reads as error), recorded as review-result:adversary-scorecard-pass-2; correction 2 dispatched (dominance per outcome rank; accepted stays accepted), the coordinator verifies it.
- 2026-10-18: scorecard-reader correction 2 at 9ebc114 (dominance per outcome rank; a completed case stays completed), verified by the coordinator; wave branch merged into the unit at 1c3cf0f (task check exit 0, 518 passed); pass 2 outcome fixed; [GRANT?] sent.
- 2026-10-18: step-cannot-forge-its-validators adversary pass 2 NEEDS-CHANGE (12 red: three manifest-scanner bypasses, a false refusal for target dependencies, .npmrc, unknown wrappers, -c quoting and --, perl -I, ../ paths, deno, a lockout by an unrelated link), recorded as review-result:adversary-step-forge-pass-2; correction 2 dispatched (cargo metadata cross-check and fail-closed scanner; fail-closed command parsing; build-tool config; links refused only where validators read), the coordinator verifies it.
- 2026-10-18: scorecard-reader granted for 1c3cf0f and merged at 8b0afc5; three reviewer findings assigned to story:promotion.
- 2026-10-18: step-cannot-forge-its-validators correction 2 at 2445a51 (fail-closed manifest scanner plus a cargo metadata cross-check; fail-closed command parsing; build-tool config; links lock out only where something reads through them), verified by the coordinator (506 passed, 2 ignored); pass 2 outcome to record; the wave branch merge into the unit is being resolved (ess conflicts).
- 2026-10-18: wave branch merged into step-cannot-forge-its-validators at 631f594 (563 passed, 2 ignored); [GRANT?] refused by the reviewer (an ancestor .cargo/config forging a pass; a per-repository judge-surface list; pytest conftest.py; two ignore reasons); correction 3 dispatched with the reviewer's route tests.
- 2026-10-18: main merged into the wave branch at 00d4770 (ESS hardening, role profiles); five StepPerformer citations re-pointed to SteppedBy and accepted; task check exit 0, 518 passed.
- 2026-10-18: step-cannot-forge-its-validators [GRANT?] refused twice by the reviewer; correction 3 (an ancestor cargo config, a per-repository list, pytest configuration) and correction 4 (`python -m` modules, pytest dotfiles and toml, links towards listed paths, the put-back through links, the process group) at 65795ea, verified by the coordinator (596 passed, 4 ignored); granted on the reviewer's approve and merged at 22ad614.
- 2026-10-18: main (6109dd8) merged into the wave branch at c34246a; `task check` exit 0 at the wave head (443 of 443 citations; 596 passed, 0 failed, 4 ignored). The four stories carry a `Landed (wave 10)` section and a `test_result`, and are `implemented`.

## Close

| story | merge into the wave | correction rounds |
|---|---|---|
| `story:grant-binds-to-action-subject` | 93bd7c3 | 1 |
| `story:trial-fetch-needs-no-user-ssh` | e9a6deb | 1 |
| `story:scorecard-reader` | 8b0afc5 | 2 |
| `story:step-cannot-forge-its-validators` | 22ad614 | 4 |

Follow-ups, each carried by a story:

- `story:confinement-seam` (wave 13): a `cargo` wrapper in a PATH directory outside the tree; a writer rejoining takt's process group; a link above the tree that one run could not put back and the next run accepts (record what was not put back, so the next run refuses until it is cleared); `openat2` and a fail-closed profile.
- `story:judge-guard`: the low findings of grant-binds-to-action-subject; the stale step-guard comment about writes made once a case is accepted.
- `story:promotion`: the scorecard's saved-file provenance, case identity from the directory, linked case directories, flakiness and token spread.
- `story:scan-before-push`: the push side of the trial route (GIT_ALLOW_PROTOCOL=https, or an explicit https push URL).
- Out of scope by decision: a Makefile `include` and a Taskfile `dotenv:` file reaching the test command (the held-out acceptance covers them).
