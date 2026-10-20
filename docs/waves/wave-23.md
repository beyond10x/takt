# Wave 23: the prompt carries the change so far, a median only for a computed metric, the hand-off's environment, the harness proposes

Stage: **running**. Shape: one dispatcher in one checkout, one wave branch `wave/23` cut from
`main`, implementor sub-agents in the shared tree, one adversarial review and one full gate before
the close. Every unit is a judge surface.

## Units

| unit | story | acceptance (condensed) | tag |
|---|---|---|---|
| E | `story:step-prompt-omits-the-change-so-far` (the trial agent's candidate 1 of the 0.5.0 cycle, `trial-cand/2026-10-20-81f34956-1` at 58f5e3e7) | each judgement step's prompt carries the diff of the case's accepted steps, bounded and scanned, or says no change yet or not known; `change.handover` hands over in one request; its reviewer's approve at 58f5e3e7 stands as the unit's review | rsi, trial |
| A | `story:the-scorecard-names-no-metric-it-did-not-compute` | `number()` has no fallback; a `median <metric>` line prints only for a metric with a per-case figure; a test walks `METRICS` | rsi |
| B | `story:a-handed-off-command-is-read-with-the-wrappers-environment` | a file of the tree that sets the environment or directory before it execs its arguments fails the line closed naming the file and the form, or the handed-off command is read with that environment; the program handed off to is never an option's value | rsi |
| C | `story:a-trial-candidate-lands-in-the-repository-of-its-layer` | scoped before dispatch (story-scoper); with `story:a-trial-pins-another-repositorys-candidate` if the scoper finds one unit | rsi, ADR 0101 |
| D | `story:harness-proposes-takt-performs` (S2) | Claude Code proposes through a metaharness session executor (metaharness 0.10.1 at c2548b3b), takt's edit effect applies it under the step guard; `b10x-loom-sdk` 0.27.0 → 0.28.0 (91487203) in the same unit; authority unchanged | loom, ADR 0099 |

Out: `story:takt-models-its-inner-workings-in-ess` and `story:takt-reads-harness-traces-as-trace-ir`
(wave 24); S3 (`story:governor-holds-takts-cases`) is the owner's call.

## Order

E first: a `--no-ff` merge of the candidate, its two ESS files resolved and the citations
re-pointed. Then A and B at once (`scorecard.rs`; `run.rs`'s command reader) with D beside them
(`harness.rs`, `run.rs`'s step execution; dispatched after E's merge commit). C after the scoper.

## Pre-flight

| check | value |
|---|---|
| tree | `main` at fd395d2 (wave 22 merged, 0.6.0 tagged there, tag object 9ec5c478) |
| metaharness | 0.10.1 at c2548b3b (0.10.0 at 3d697011 has no published Release) |
| loom | 0.28.0 at 91487203 (tag object dbafa2e0) |
| builds | every cargo compile or test through `b10x-build-slot`, sub-agents too; one target per tree |

## Log

- 2026-10-20: wave opened from `main` at fd395d2; approved by the orchestrator under exception 5
  (E, A, B, C: ADR 0101; D: ADR 0099). 0.6.0's `release.yml` succeeded with 4 assets.
- 2026-10-20: E merged in 41c2a01: `ess/domains/takt.yaml` (68 hunks, all citations or equal
  prose) and `takt.citations.tsv` (2 hunks) resolved, six citations re-pointed; 1004/1004 with
  digests. The merged tree: `cargo test --workspace --locked --no-fail-fast` 1542 passed, 0
  failed, 6 ignored; fmt and clippy clean. A first run without `--no-fail-fast` stopped on
  `a_judge_surface_merge_after_the_owners_approval_is_recorded` (`--hours 0.001`, 3.6 s) at a
  load average near 24; alone it passed 3 of 3.
- 2026-10-20: C committed in 58b7c77 (`.agents/trial-agent.md`, `AGENTS.md` § Messages,
  `.agents/dispatcher-agent.md`). Scoped first: the story lands in the profile only; acceptance 3
  pins binary layers only and 6 names the promotion's layers as other stories'.
- 2026-10-20: A, B and D dispatched at 58b7c77; D on metaharness 0.10.1 (c2548b3b) and loom-sdk
  0.28.0 (91487203).
- 2026-10-20: units committed: B 092ab01 (`gate_compares_imports` 52 passed, 4 new), A e52b907
  (`scorecard` 22, `scorecard_cli` 20), D f8cde38 (`harness_proposes` 6, `b10x-session-executor`
  3; `b10x-takt-cli` 465 passed with its fake `claude` unchanged; a live run on a scratch crate
  with one failing test: `tests.pass` true, `repository.merge` waiting for its grant, 29665
  tokens). D waited for a build slot before its first compile, so its tests have no red run.
  D's hermetic floor: H9 (the vendor's version against the adapter's pin) is reported by row
  id, not gating; H1a (undeclared plugins) does not gate when every loaded plugin is
  `@builtin`; both accepted by the orchestrator until metaharness declares Claude Code 2.1.304.
  Citations c3685a3: 1011/1011 with digests.
- 2026-10-20: gate at c3685a3 (`task check`, through `b10x-build-slot`): exit 0; `cargo test
  --workspace --locked` 1557 passed, 0 failed, 6 ignored. Adversarial review started at
  c3685a3.
- 2026-10-20: review at c3685a3: needs-revision. D1 (medium): a proposed path git ignores was
  written by the edit effect before the guard refused it, so an operator's ignored file could be
  lost (3 of 5 probes); fixed in df49698 (refused before the write, with files for an action
  that records a choice; 3 tests). C1 (medium): the trial-agent profile named a private
  repository; fixed in 7f9cfbc. Filed: story:a-hand-off-is-read-from-an-allow-list (B-1 to B-6:
  11 probes slip the hand-off reader's deny-list or plant a program in a tree `PATH`
  directory); D2 to D4, ESS1, ESS2 and C2 to C4 (low) in the planner's queue. Clean: A, E (no
  prose lost in the merge; the change so far is in the session's prompt), D's acceptance 1, 2,
  4, 5 and 6; the reviewer's own live run on c3685a3 applied and committed the step, `tests.run`
  passed, `repository.merge` waits for its grant, 29044 tokens.
- 2026-10-20: gate at df49698: `task check` exit 0, 1560 passed, 0 failed, 6 ignored, citations
  1011/1011. The five stories moved to implemented on that `test_result`. No release: the
  changes sit under Unreleased in `CHANGELOG.md`.
