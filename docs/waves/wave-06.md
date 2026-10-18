# Wave 06: Fix every finding of the 2026-10-18 code review and live trials

Skill: `aep:implementing` 0.23.1, wave mode. Stage: **closed** (six stories implemented, fix branch gate green at 95995b4). Approval: the operator approved option A on 2026-10-18 (one wave, two batches, function split in batch A), including proceeding with the one uncommitted file in the main checkout. Mode: interactive.

## Units

| unit | serves | acceptance (condensed) | scope |
|---|---|---|---|
| `story:tree-hygiene` | `vision:safe-agentic-coding` | open refused on a dirty tree; a step commits exactly its paths; a refusal discards index, ignored files, `.git/` writes and directory swaps; porcelain parsed with `-z`; a take is committed; `open_next` reads the base | `git.rs`, `run.rs` (step section, `restore`/`snapshot`, `open_next`), `case.rs` (open/create), `harness.rs` (`is_test_path`) (cited) |
| `story:validator-confinement` | `vision:safe-agentic-coding` | a test command that writes a grant, a record or a gates file is refused and reverted; gates and project file read from the base revision; child env without tokens, timeout, bounded output; no project/user hooks in the harness child; `gh --repo`; `CiRead` mutant killed | `validator.rs` (but `PrMerged`), `harness.rs` (spawn/env), `forge.rs`, `run.rs` (validator section), `frontier.rs` (`aep` spawn) (cited) |
| `story:authority-hardening` | `vision:safe-agentic-coding` | lock by `create_new`, 300/300; grant bound to a revision and only at the gate; `approve` records first; case ids sanitised; unreadable record stops the run; `latest_decision` numeric | `case.rs` (lock, authority, resume, decisions, evidence), `frontier.rs` (`approve`), `validator.rs` (`PrMerged`), `run.rs` (`is_due`) (cited) |
| `story:cli-protocol-defaults` | `vision:safe-agentic-coding` | store story defaults to `story.delivery/1` (embedded); `--protocol` mismatch on resume refused; forge refusal is a `stopped:` line; `ApprovalRequired` exit code differs from `Completed`; `--forge` help true | `takt-cli/src/main.rs` (cited), `run.rs` (`resume`, error-to-stop) (inferred), `protocol.rs` (inferred) |
| `story:loop-test-gaps` | `vision:safe-agentic-coding` | mutants 17 A–D each turn a test red; "nothing ready twice" asserted through `open_next`; `Frontier::next` used or gone | `tests/loop_gaps.rs` (new), `tests/frontier.rs` (cited), `frontier.rs` (inferred) |
| `story:docs-and-spec-true` | `vision:safe-agentic-coding` | `ess specify validate` exit 0; every `read from:` citation resolves; design status, README, AGENTS.md true | `ess/domains/takt.yaml`, `docs/design/takt.md`, `README.md`, `AGENTS.md` (cited) |

`story:docs-and-spec-true` `depends_on` the other five.

## Selection path

`aep plan artifact waves --kind story --status draft --format text` (aep 0.74.0). The verb places the six stories in five waves (1: authority-hardening; 2: cli-protocol-defaults, loop-test-gaps; 4: tree-hygiene; 5: validator-confinement; 6: docs-and-spec-true), because the typed scope is per file and the units share `run.rs`, `case.rs`, `frontier.rs`, `validator.rs` and `harness.rs`.

**Disagreement with the verb, reported:** the operator asked for one wave on one fix branch. This page runs it as one wave in two batches, using the split-one-file-by-function fallback for batch A: each brief names the functions it owns in each shared file, each implementor pastes its hunk headers, and `git merge-tree --write-tree` runs on every pair before the first merge. The verb's file-level reading is correct; the split rests on function ranges the typed scope cannot express.

| batch | units | runs |
|---|---|---|
| A | tree-hygiene, validator-confinement, authority-hardening, cli-protocol-defaults | in parallel, N = 4 |
| B1 | loop-test-gaps | after A has merged (line numbers in `run.rs`, `frontier.rs` move) |
| B2 | docs-and-spec-true | after B1 has merged (citations are checked against the final code) |

## The verb's three lists, verbatim (wave-06 stories only; full output 11 waves, 92 collisions, 0 unassessed)

### Waves

- wave 1: `story:authority-hardening (inferred)`
- wave 2: `story:cli-protocol-defaults (inferred)`, `story:loop-test-gaps (inferred)`
- wave 4: `story:tree-hygiene (inferred)`
- wave 5: `story:validator-confinement (inferred)`
- wave 6: `story:docs-and-spec-true`

### Collisions between wave-06 stories (12)

- collision: story:authority-hardening story:cli-protocol-defaults crates/takt/src/run.rs (inferred)
- collision: story:authority-hardening story:loop-test-gaps crates/takt/src/frontier.rs (inferred)
- collision: story:authority-hardening story:tree-hygiene crates/takt/src/case.rs
- collision: story:authority-hardening story:tree-hygiene crates/takt/src/run.rs
- collision: story:authority-hardening story:validator-confinement crates/takt/src/frontier.rs
- collision: story:authority-hardening story:validator-confinement crates/takt/src/run.rs
- collision: story:authority-hardening story:validator-confinement crates/takt/src/validator.rs
- collision: story:cli-protocol-defaults story:tree-hygiene crates/takt/src/run.rs (inferred)
- collision: story:cli-protocol-defaults story:validator-confinement crates/takt/src/run.rs (inferred)
- collision: story:loop-test-gaps story:validator-confinement crates/takt/src/frontier.rs (inferred)
- collision: story:tree-hygiene story:validator-confinement crates/takt/src/harness.rs
- collision: story:tree-hygiene story:validator-confinement crates/takt/src/run.rs

### Unassessed (0)

- none

## Left out

- Finding 30, public module surface (`lib.rs` all `pub`): a visibility change ripples into every test target; recorded as not fixed in this wave. Its exit-code half is in `story:cli-protocol-defaults`.
- Grant revocation (`approve` cannot deny or revoke): a feature, not a defect; not in this wave.
- Handoff findings 2 (validator-owned actions offered when no validator is registered), 3 (kernel-side stop at the gate), 5 (run writes nothing to the story), 6 (cost compared to nothing): open stories already hold them (`story:run-record-and-story-evidence`, `story:story-done-end-to-end`).

## Pre-flight

| check | value |
|---|---|
| free disk | 145G on `/` |
| tree | main checkout on `main` at 01b72c6 with one uncommitted file, `.engineering/planning/decision-blocker/live-trial-repository.md`, another session's; no unit's surface touches it; the integration tree is forked from 01b72c6 and does not carry it |
| worktrees | wave 05's trees finished; the `pr-open-needs-commit` tree finished and archived |
| build | each tree its own `target/` (a finished tree's cache was 1432 MiB); build time not measured this wave, wave 05's page: incremental under 1 min, `task check` 2-3 min |
| model budget | not stated by the operator; N = 4 (the skill's default) |
| agent file | `AGENTS.md` read |

## Branches and triples

| unit | branch | worktree | build dir | scratch |
|---|---|---|---|---|
| integration | `fix/2026-10-18-review` | managed tree `wave-06-integration` | its `target/` | `~/.cache/takt-wave-06/scratch/int` |
| integration-fixes | `impl/integration-fixes` at 44a6414, one pass, correction 1 verified, merged at abaf771 | managed tree `w6-int-fixes` | its `target/` | `~/.cache/takt-wave-06/scratch/int` |
| tree-hygiene | `impl/tree-hygiene` at 8220677, two passes, correction 2 verified; merged via the validator branch at e1d75ec | managed tree `w6-tree-hygiene` | its `target/` | `~/.cache/takt-wave-06/scratch/tree-hygiene` |
| validator-confinement | `impl/validator-confinement` at 9ecd2b6, two passes, correction 2 verified; merged at e1d75ec | managed tree `w6-validator-confinement` | its `target/` | `~/.cache/takt-wave-06/scratch/validator-confinement` |
| authority-hardening | `impl/authority-hardening` at d829903, two passes, correction 2 verified, merged at 768db1c | managed tree `w6-authority-hardening` | its `target/` | `~/.cache/takt-wave-06/scratch/authority-hardening` |
| cli-protocol-defaults | `impl/cli-protocol-defaults` at 9271e74, two passes, correction 2 verified, merged at 316fbfc | managed tree `w6-cli-protocol-defaults` | its `target/` | `~/.cache/takt-wave-06/scratch/cli-protocol-defaults` |
| loop-test-gaps | `impl/loop-test-gaps` at f3f1cf9, one pass, merged at 38eba58 | managed tree `w6-loop-test-gaps` | its `target/` | `~/.cache/takt-wave-06/scratch/loop-test-gaps` |
| docs-and-spec-true | `impl/docs-and-spec-true` at cfd03d4, coordinator-verified, merged at 95995b4 | managed tree `w6-docs-and-spec-true` | its `target/` | `~/.cache/takt-wave-06/scratch/docs-and-spec-true` |

## Dispatch

`aep:implementor`, then `aep:adversary` (two passes at most) for the four code units of batch A; `aep:implementor` and one `aep:adversary` pass for `loop-test-gaps`; `aep:implementor` alone for `docs-and-spec-true`, verified by the coordinator against `ess specify validate` and a citation-resolving script. Unit brief at `<scratch>/brief.md`.

## The commits this wave makes

The opening store commit (this page, the six stories, their move to `active`), each unit's commits, the merges into `fix/2026-10-18-review`, the closing store commit, the merge of `fix/2026-10-18-review` into local `main`. No push, no tag, no release, no pull request.

## Log

- 2026-10-18: proposed; approved (option A); the six stories moved to active.
- 2026-10-18: opening commit ae94ccb (`ess specify validate`, `cargo fmt --all --check`, `aep plan artifact validate` exit 0); batch A: four trees from ae94ccb, `aep:implementor` dispatched to each, briefs at `<scratch>/brief.md`.
- 2026-10-18: cli-protocol-defaults green at 15cbfeb (red run: 3+1+7 failing and a compile error; green: 161→173 cases; fmt, clippy, test exit 0; 169,195 tokens, 84 tool uses, 11 min). Deviations: every `Run::run` error is a `stopped: error:` line with exit 5; `ApprovalRequired` exits 4; undeclared `--input` refused in `main.rs`; the protocol mismatch check is in `main.rs`, not `Run::resume`. A README patch for story:docs-and-spec-true is in its scratch. Adversary pass 1 dispatched.
- 2026-10-18: local main at 1bb8b31 merged into the fix branch at 400c211 (`git merge-tree` clean beforehand; `aep plan artifact validate` valid after), so story:docs-and-spec-true covers the new ESS entries. Five cross-epic `depends_on` edges added for the other epic (judge-guard on validator-confinement, tree-hygiene, authority-hardening; self-improvement-protocol on validator-confinement; first-recorded-improvement-turn on cli-protocol-defaults); they go into the closing store commit.
- 2026-10-18: cli-protocol-defaults adversary pass 1 NEEDS-CHANGE (176 cases, 3 red; 3 findings, all introduced, 1 blocker: the `--protocol` mismatch refusal runs after the resume has moved HEAD and rewritten `case.yaml`; 97,804 tokens, 29 tool uses, 4.5 min), recorded as `review-result:adversary-cli-pass-1`; correction 1 sent to the same implementor.
- 2026-10-18: authority-hardening green at 9b803fb (red run: 1 passed, 10 failed in the new lane; green: authority 0→11, merge_under_authority 30→30 with seven tests taken to the gate before granting; fmt, clippy, test exit 0; 239,732 tokens, 127 tool uses, 19 min). Out-of-range: one `check_id(id)?;` line at the top of `CaseDir::open` (tree-hygiene's range). Patches for the coordinator at integration: `takt-cli-approve-test.patch` (required: one takt-cli test goes red without it), `run-is-due-result.patch`, `frontier-record-revision.patch`. Adversary pass 1 dispatched.
- 2026-10-18: cli-protocol-defaults correction 1 green at 4358c28 (red: adv1 0/3, one new case failing on the wrong protocol name; green: 177 cases; fmt, clippy, test exit 0; 202,350 tokens, 16 tool uses, 3 min): refusals before any resume via `case_to_resume`; inputs refused for a resumed case. Adversary pass 2 dispatched.
- 2026-10-18: tree-hygiene green at 9618c80 (red run: 0 passed, 14 failed; green: lib 14→15, tree_hygiene 0→14; fmt, clippy, test exit 0; 260,077 tokens, 80 tool uses, 22 min). Decisions: the take is a commit on the case branch; open_next reads the store from the base. Patches for the coordinator: `takt-cli-frontier_cli-stub-ignored.patch` (required), `validator-pr-open-ignores-take.patch` (pr.open must not count the take commit). Cross-unit: `takt approve` leaves its store record uncommitted on the case branch, which validator-confinement's dirty-tree refusal will see; to settle at integration. Adversary pass 1 dispatched.
- 2026-10-18: validator-confinement returned red at cf3680d (3 merge_under_authority cases need the committed take; 259,848 tokens, 116 tool uses, 22 min). Coordinator merged impl/tree-hygiene into it (4bb3a11, one test conflict in gate_validator.rs) and reconciled at 545379f: `discard_all` → `discard_uncommitted` (removed by tree-hygiene), tree-hygiene's pr.open take patch applied, two tree_hygiene cases rewritten because a test command may no longer leave files. Package gate: fmt, clippy exit 0; 153 cases exit 0. Harness flags checked against `claude --help`: `--restricted` removes Bash, already in `--disallowedTools`. Adversary pass 1 dispatched.
- 2026-10-18: cli-protocol-defaults adversary pass 2 (179 cases, 2 red; 2 notes, both introduced: a second store read between the check and `open_next` (INFEASIBLE in practice, built with a stub), and `--help` silent on refused `--input` for a resumed case; 97,551 tokens, 28 tool uses, 5 min), recorded as `review-result:adversary-cli-pass-2`. Ledger (`aep plan artifact findings`): carried 0, new 2, resolved 3. Correction 2 sent to the same implementor; the coordinator verifies it.
- 2026-10-18: authority-hardening adversary pass 1 NEEDS-CHANGE (139 cases, 2 red; 3 findings, all introduced: grant filtered by in-memory revision while Canon reads disk (INFEASIBLE window), `has_evidence` accepts an empty record, `check_id` in another unit's `CaseDir::open` (merges cleanly, kept); 118,651 tokens, 29 tool uses, 7 min), recorded as `review-result:adversary-authority-pass-1`; correction 1 sent. Dry run of the batch-A merge (w6-dryrun at 1a09d27, all three merges textually clean, two takt-cli patches applied): clippy exit 0, tests exit 101 with 6 cross-unit failures in takt-cli (main.rs harness-error text, `approve` test not at the gate under story.delivery, and `case_to_resume` not seeing a take committed on a case branch); to be fixed on the fix branch after the merges.
- 2026-10-18: cli-protocol-defaults correction 2 at 9271e74 (adv2 red 0/2 before, green after; 179 cases; fmt, clippy, test exit 0; 238,929 tokens, 119 tool uses, 4 min): resume by the id `case_to_resume` found; `CheckedResume` wraps the frontier for the `open_next` path; `--input` help. Coordinator verification: only `main.rs` changed, no assertion removed. Claim verified (baseline ae94ccb has no `protocol_defaults_cli` target; treatment 11 passed). Merged into the fix branch at 316fbfc. authority-hardening correction 1 at 36f8405 (adv1 red 2→0; 139 cases; fmt, clippy, test exit 0; 254,686 tokens, 11 tool uses, 1.5 min); adversary pass 2 dispatched.
- 2026-10-18: validator-confinement adversary pass 1 (161 cases, 6 red; original findings 1 and 2 reproduced and closed; 5 new findings, all introduced, 4 blockers: `core.fsmonitor` planted by the test command runs in takt's own `git status` and writes a grant, with `GH_TOKEN`; the base branch moved by the test command decides the gate; a `setsid` descendant writes after the guard; admitted gate scripts read from the case branch; 121,769 tokens, 32 tool uses, 7 min), recorded as `review-result:adversary-validator-pass-1`. Correction 1 sent with a widened assignment: `git::run` hardening (git.rs), the base-commit pin (`Branch` and one line in `CaseDir::open`), `libc` if needed.
- 2026-10-18: authority-hardening adversary pass 2 (142 cases, 2 red; 3 findings: the judgement-step snapshot reverts a sibling case's grant (pre-existing at ae94ccb, filed as `story:case-scoped-step-guard`, draft), `has_evidence` stops at the first match (introduced), `check_id` accepts Unicode format characters (introduced, INFEASIBLE); 111,810 tokens, 35 tool uses, 8 min), recorded as `review-result:adversary-authority-pass-2`. Ledger: carried 0, new 3. Correction 2 sent; the sibling case is rewritten to assert today's behaviour, naming the new story.
- 2026-10-18: authority-hardening correction 2 at d829903 (adv2 red 2→0, authority 11→12; 143 cases; fmt, clippy, test exit 0; 278,364 tokens, 13 tool uses, 2 min). Coordinator verification: no assertion removed from a tracked test; the sibling case asserts today's state naming story:case-scoped-step-guard. Claim verified (baseline: 51/300 exclusive holds, 264 grants while held; treatment: 300/300). Merged into the fix branch at 768db1c.
- 2026-10-18: validator-confinement correction 1 at 90cabe1 (adv1 2/8 → 8/8 unchanged; validator_confinement 13→17, each new case shown red by a mutation; 165 cases; fmt, clippy, test exit 0; 355,950 tokens, 161 tool uses, 9 min): Checkpoint around observe, hardened `git::run`, base commit pinned in `branch.yaml`, Linux subreaper sweep (`libc` added), gate refuses when admitted executables differ from base. Residue stated: a process joining takt's group, global-config filter drivers, scripts called by admitted scripts. Adversary pass 2 dispatched.
- 2026-10-18: tree-hygiene adversary pass 1 NEEDS-CHANGE (146 cases, 6 red; 7 findings, all introduced, 1 blocker: an unreadable file under `.git` stops the put-back before anything is undone; also a FIFO hangs the run, other processes' `.git` writes reverted, ignored files compared by path only, hooks run during `restore`, the take commit runs repository hooks; 149,580 + 153,942 tokens, 39 tool uses, 22 min). The first findings block was refused by `aep` (unquoted YAML); the author resent it as JSON; recorded as `review-result:adversary-hygiene-pass-1`. Coordinator decision on F3: guard only config (code-running keys), hooks, info, HEAD and the case refs. Correction 1 sent, with `git::run` hardening aligned to validator-confinement's.
- 2026-10-18: tree-hygiene correction 1 at 48688d2 (adv1 0/6 → 6/6 unchanged; tree_hygiene 14→16; 149 cases; fmt, clippy, test exit 0; 320,529 tokens, 104 tool uses, 6 min). One own assertion (a stray `.git/monitor.sh` removed) dropped under the F3 decision; the config key that would run it is still put back. Adversary pass 2 dispatched.
- 2026-10-18: validator-confinement adversary pass 2 (167 cases, 1 red; pass 1's five findings closed; 1 new warning, introduced: an unpinned (pre-correction) case falls back to a merge-base that a test command can collapse onto HEAD; 134,194 tokens, 38 tool uses, 16 min), recorded as `review-result:adversary-validator-pass-2`. Ledger: carried 0, new 1. Coordinator decision: no fallback; an unpinned case gets no gate record. Correction 2 sent.
- 2026-10-18: tree-hygiene adversary pass 2 NEEDS-CHANGE (154 cases, 5 red; 5 findings, 1 blocker: a garbled `.git/HEAD` sends takt's git calls into the enclosing repository, measured moving its HEAD; also chmod 555 on hooks, ctime/inode not compared, sibling `config.worktree` unguarded, a stale `index.lock`; 104,941 tokens, 28 tool uses, 8 min), recorded as `review-result:adversary-hygiene-pass-2`. Ledger: carried 0, new 5, resolved 7. The adversary's first draft moved its own worktree's HEAD and it put it back (branch tip unchanged at 48688d2). Correction 2 sent.
- 2026-10-18: validator-confinement correction 2 at c0e8f4e (adv2 red 1→0; 167 cases; fmt, clippy, test exit 0; 370,807 tokens, 11 tool uses, 2 min): no merge-base fallback; an unpinned case gets no gate record. Coordinator verification: only assertions added; the own fallback case re-pinned to the decision. Waits for tree-hygiene correction 2 before both merge.
- 2026-10-18: tree-hygiene correction 2 at 8220677 (adv2 red 5→0; 156 cases; 373,815 tokens, 21 tool uses, 5 min); coordinator verification: one diagnostic read in an adversary case relaxed to accept git's exit 1, no assertion changed. Merged into the validator branch at 9ecd2b6 (git.rs conflict: tree-hygiene's pinned `git::output` plus validator-confinement's credential-free env; the validator guard adapted to `GitDirPutBack`); package gate 183 cases exit 0. Merged into the fix branch at e1d75ec. Claims verified for both stories. Integration fixes: worktree `w6-int-fixes`, branch `impl/integration-fixes` from e1d75ec, five handed-back patches applied, workspace tests 257 passed / 7 failed (all takt-cli, cross-unit); `aep:implementor` dispatched.
- 2026-10-18: integration fixes green at e48695d (7 red → 0; 264→269 cases; ess, fmt, clippy, test exit 0; 257,560 tokens, 102 tool uses, 16 min). Coordinator decisions taken: (A) approve commits its store record on the case branch and `CaseDir::resume` keeps the granted revision across approve-only commits (one parent, exact subject, store/evidence paths only), since only takt can commit on a case branch; (B) from a tree on a case branch, `Run::next_to_resume` finds only cases whose take is committed, a base-only `active` case resumes on the next run. One adversary pass dispatched on the integration fixes.
- 2026-10-18: batch B1 started: loop-test-gaps from e48695d (the integration-fixes head, so its mutants are re-expressed against the merged loop); `aep:implementor` dispatched.
- 2026-10-18: loop-test-gaps green at 9507da2 (loop_gaps 0→5, frontier 15→13 by merging two duplicate pairs; mutants A-D each kill exactly one test, red lines in the scratch logs; two extra frontier mutants killed; `Frontier::next` removed, no production caller; 135,593 tokens, 54 tool uses, 8 min). Adversary pass dispatched.
- 2026-10-18: integration adversary pass (273 cases, 4 red; 5 findings: a test command can move `main` or any ref but HEAD, unrefused (CONFIRMED, pre-existing to the integration, a validator-confinement gap); a step may move a sibling case branch (INFEASIBLE with the shipped harness); `pr.open` stuck behind an approve commit under a protocol that gates it (INFEASIBLE); help text promises a resume decision B dropped; the approval record never reaches the base (filed as `story:approval-record-reaches-base`, draft); 215,110 tokens, 69 tool uses, 14 min), recorded as `review-result:adversary-integration-pass-1`. Decisions: the take-on-base-only state is unsupported; all branch and tag refs are guarded around validators and steps. Correction 1 sent.
- 2026-10-18: loop-test-gaps adversary pass (221 cases, 0 red on the tree; 3 introduced warnings: four budget mutants the unit's tests let through, each killed by one of the adversary's three new cases; H4 `>=`→`>` equivalent; 101,969 tokens, 44 tool uses, 7 min), recorded as `review-result:adversary-loop-gaps-pass-1`. No implementation change needed; the adversary's cases committed at f3f1cf9.
- 2026-10-18: integration fixes correction 1 green (adv 4 red → 0; 274 cases; ess, fmt, clippy, test exit 0; 323,394 tokens, 159 tool uses, 38 min): every branch and tag ref guarded around validators and steps; pr.open pushes the granted revision past approve commits; help text states decision B. Coordinator decision added: branches checked out in another worktree are left out of the ref guard (another session's work); sent back for that one change.
- 2026-10-18: integration correction 1 narrowed twice (other worktrees' branches exempt; the base and the case's own branch always guarded), verified at 44a6414 (276 cases; one adversary assertion rewritten to decision B, no other assertion removed; 348,198 tokens, 188 tool uses, 42 min in all rounds). Local main 7f28e9f merged at b06d169; integration fixes at abaf771; loop-test-gaps at 38eba58. Fix branch gate at 38eba58: ess 0, fmt 0, clippy 0, 282 passed / 0 failed. Batch B2 dispatched: docs-and-spec-true from 38eba58.
- 2026-10-18: docs-and-spec-true at cfd03d4 (citations 181 of 244 stale → 300 of 300 resolve; README and AGENTS built from the operator role's drafts line by line; 328,369 tokens, 83 tool uses, 15 min). Coordinator verification: the citation script re-run (0 unresolved), `ess specify validate` valid, no private names in the four files. Merged at 95995b4.
- 2026-10-18: closed. Gate on the fix branch at 95995b4, one exit status per step: `ess specify validate` 0, `cargo fmt --all --check` 0, `cargo clippy --workspace --all-targets --locked -D warnings` 0, `cargo test --workspace --locked --no-fail-fast` 0 (30 binaries, 282 passed, 0 failed), `aep plan artifact validate` valid. Each story: a Landed (wave 06) section from the diff, a test_result, moved to implemented. Filed for later: story:case-scoped-step-guard, story:approval-record-reaches-base (both draft).

## Cost

Per-agent tokens, tool uses and minutes are in the log entries above, as each harness notification reported them. They are not summed here: an agent resumed for a correction reports per notification, and whether those counts are per round or cumulative is not known.
