# Wave 31: a Codex step's context fits its launch argument, is recorded and is not read again

Stage: **closing**. Shape: one dispatcher in one checkout, one wave branch `wave/31` cut from
`main` at dde961c, approved by the orchestrator under the owner's delegation.

## Units

| unit | story | where | commit |
|---|---|---|---|
| 1 | story:codex-prompt-exceeds-launch-argument, story:codex-context-not-recorded | `crates/takt/src/harness.rs`, `crates/session-executor/src/lib.rs`, `ess/domains/takt.yaml`, `ess/domains/takt.citations.tsv` | 683fc42 |
| 2 | story:codex-step-rereads-its-context (candidate `trial-cand/2026-10-21-bb4072a3-2` at f4f8d8d) | `crates/takt/src/harness.rs`, `ess/domains/` | 9cae2d8 |
| 3 | a boundary test from the wave's review | `crates/takt/src/harness.rs` | 571b52c |
| plan | the two findings' stories, the candidate's merge grant, a predicate the prompt line cannot pass | `.engineering/planning/` | 3c47096, 9110945 |

Unit 1 measures each Codex prompt with the context files it would carry, as the one launch
argument metaharness checks against its 131,072-byte limit, and leaves out the files that do not
fit. Each kept file is recorded under `takt.prompt.context` with its path, digest and size.

Unit 2 merged the candidate into unit 1: the prompt names the files kept under "Files in scope",
so the line is inside each measured argument. Measured on Codex, scenario 1, 5 runs a side:
median run tokens 84,984 at the candidate against 101,585 at cabc09a2
(approval-record:candidate-bb4072a3-2-merge-grant).

## Left from the review

The review at 9cae2d8 returned 0 blocking findings and these notes, not filed:

- A context file that cannot be read is sent as an empty file (`.unwrap_or_default()`).
- The prompt line names `AGENTS.md` under "Files in scope" though it is no scope file; changing
  the wording needs a re-trial.
- Every context file is read even when the prompt alone is over the limit.
- The context record is written before the session starts.
- The ESS context entry declares no type.

## Log

- 2026-10-22: wave opened from `main` at dde961c (3c47096).
- 2026-10-22: unit 1 (683fc42); `check-citations` 1058 of 1058.
- 2026-10-22: the candidate merged as unit 2 (9cae2d8), conflicts resolved in `harness.rs` and
  `takt.yaml`.
- 2026-10-22: the wave's review at 9cae2d8 approved with 0 blocking findings. Two notes fixed:
  a test at the byte limit (571b52c), red on two mutants of the budget, and
  story:codex-context-not-recorded's predicate, which the new prompt line passed without the
  record (9110945).
- 2026-10-22: `task check` at 9110945: exit 0, 1666 passed, 0 failed, 5 ignored.
