# Wave 24: a session step sees the git state of its copy

Stage: **running**. Shape: one dispatcher in one checkout, one wave branch `wave/24` cut from
`main`, one sub-agent at a time (the owner, 2026-10-20), one adversarial review and one full gate
before the close. Every unit is a judge surface.

## Units

| unit | story | acceptance (condensed) | tag |
|---|---|---|---|
| A | `story:a-session-step-sees-the-git-state-of-its-copy` | the metaharness session's scratch copy carries the case tree's git state through a `.git` file naming a git directory outside the copy, the case tree's `.git` never written; Glob sees no git internals; the trial agent's predicate goes from exit 3 to 4; a five-run re-trial by the trial agent reports the median and how many runs opened with Glob | rsi |
| B | `story:second-harness-adapter` | `--harness codex`: the session executor drives metaharness `Kind::Codex` (codex-cli 0.153.4) in the same scratch copy under the same proposal contract, hermetic `On` with the verdict per step and H9 gating; writes stay in the copy, no network; one live run each on a scratch crate under both harnesses with equal decisions, one scenario-1 run with Codex; then the trial agent's two five-run sets side by side | loom, the owner's direction |

Drafted for later waves: `story:a-session-step-loads-no-auto-memory`,
`story:the-cli-suite-fits-its-ci-budget`, `story:a-hand-off-is-read-from-an-allow-list`,
`story:takt-models-its-inner-workings-in-ess`, `story:takt-reads-harness-traces-as-trace-ir`,
`story:handover-costs-a-harness-call-of-its-own`.

## Pre-flight

| check | value |
|---|---|
| tree | `main` at 6ad32d3 (wave 23 merged) |
| probe | `~/.cache/takt-r0/w24/probe/result.md` on the dispatcher's machine: `tests.write` median 37,858 tokens and 4 requests as is, 27,020 and 3 with the git state in the copy (3 runs each) |

## Log

- 2026-10-20: wave opened from `main` at 6ad32d3; unit A approved by the orchestrator under
  exception 5 (ADR 0099, 0101) on the probe's result.
- 2026-10-20: wave 23's `check.yml` on 6ad32d3: attempt 1 cancelled by the job's 30-minute
  limit (1551 tests passed before the cut; the summed test-binary time was 1433 s against 839 s
  at fd395d2), attempt 2 green in 23 min 7 s. The workflow is unchanged;
  `story:the-cli-suite-fits-its-ci-budget` is drafted.
- 2026-10-20: unit B added on the owner's direction (relayed by the orchestrator): `--harness
  claude|codex|harness`, each through metaharness, one session-executor seam with the kind as
  a parameter.
- 2026-10-20: unit A committed in ed13e8d: the copy's `.git` is a file naming a git directory
  outside the copy (objects through alternates, read-only); `check-finding` on the trial agent's
  predicate: `not_reproduced` (exit 4, was 3). Measured without a model: Claude Code's start-up
  git commands exit 128 in the copy at 6ad32d3 and print the tree's values with the change.
- 2026-10-20: the trial agent's runs stopped on the owner's word; the codex arm first, no live
  Claude runs of the dispatcher's until it works.
- 2026-10-20: unit B committed in 85cde41. Codex runs hermetic `On` (the verdict per step, H9
  gating), in its own sandbox (workspace-write, no network), with the decision hook allowing
  Bash and `apply_patch` inside the copy only; measured: git writes in the copy exit 128 under
  that sandbox and the tree's object mtimes stay unchanged. Live under `--harness codex`: a
  scratch crate reached `tests.pass` with decisions equal to the Claude run of the wave 23
  review (76,203 tokens); scenario 1 ran 10 steps to `Unperformable (pr.opened; no forge)`,
  held-out check PASS, 141,997 tokens. The `harness` arm is built and not run live.
- 2026-10-20: gate at 85cde41: `task check` exit 0, 1576 passed, 0 failed, 6 ignored. Review at
  85cde41: approve, with F-A1 (high): Codex's sandbox grants whole-filesystem read and its hook
  reads command words only, so credential files were readable and their text could reach a
  proposed file or the step summary; and F-A2 (medium): `codex` resolved on takt's `PATH` picked
  0.160.0, and a held session's tokens went uncharged. The orchestrator did not accept F-A1;
  fixed in 5741420: a credential-shape list and the policy scan on every proposal's files and
  summary, fail closed (18 planted cases refused), Codex's home in the session scratch (by
  `expanduser` the four credential files are not found; by absolute path they are still
  readable, so the scan with the network off is the guard), `codex` resolved on the child's
  `PATH`, held tokens charged. The gates policy matched none of the three credential formats;
  the shape list is takt's. Lows B1 to B3 (split index, a symlink in the carried index, a
  partial clone) are in the planner's queue.
- 2026-10-20: gate at 5741420: `task check` exit 0, 1583 passed, 0 failed, 6 ignored, citations
  1016/1016. Both stories moved to implemented on that `test_result`. Open after the merge: the
  trial agent's five-run sets, one with `--harness codex` and one with `--harness claude` (unit
  A's acceptance 4: the median and how many runs opened with Glob), both with
  `B10X_GATES_POLICY` set.
