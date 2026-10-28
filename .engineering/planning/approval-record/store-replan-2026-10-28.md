---
format: aep.planning-md/3
id: approval-record:store-replan-2026-10-28
kind: approval-record
status: approved
title: 'The owner''s operating decisions of 2026-10-28: wave 40 rsi first, the store re-plan, takt delivers every second or third story'
summary: wave 40 keeps bench-task-protocol as U1 and fills the rest with rsi stories; the re-plan's moves; takt runs on scratch clones, reviewed, observations filed as rsi findings
relations:
- decides: story:repository-scripts-are-rust
- decides: story:viewer-is-vue
- decides: story:bench-task-protocol
- decides: epic:protocol-settings-in-the-run-file
revision: 2
transitions:
- {from: "draft", to: "approved", at: "2026-10-28T19:08:43Z", actor: "human:timo", revision: 2, executor: "agent:planner"}
---
# Approval: the owner's operating decisions of 2026-10-28

## Who decided

The owner, 2026-10-28, recorded by the planner.

## Decisions

| item | decision | why |
|---|---|---|
| wave 40's pick | rsi first, per `AGENTS.md` § Rules: story:bench-task-protocol stays in the wave as its committed U1 (7be207a5), and every remaining slot goes to an open `rsi` story | the rule schedules every open `rsi` story before any story without the tag; U1 is already committed |
| the store re-plan | applied as below, each move on evidence checked at the wave tree's head | the store had drifted from what main holds |
| takt delivers takt | every second or third story of a wave is delivered by takt itself, `takt run` on a scratch clone with `--forge none`; its result is reviewed and passed adversarially like any unit before it joins the wave; each such run's observations are logged, and an observation that recurs is filed as a finding tagged `rsi` | takt's own delivery is the loop's richest source of findings |

## The re-plan

- To implemented, on the check run of main at 338082d8 (release 0.18.0): story:the-run-file-sets-protocol-settings (3e7d2353), story:takt-lists-a-protocols-settings (b6631593), story:the-workspace-seam-enters-and-leaves-managed-worktrees (636899e2), story:confinement-through-substrate-on-linux-and-macos (9a93b9de, release 0.16.0). epic:protocol-settings-in-the-run-file follows, its three stories implemented.
- Archived: story:test-summary-keeps-the-totals (shipped in 3080c5c2), story:loop-case-opener (folded into story:loop-scorecard by 162260eb), story:harness-tools-allowlist (done by story:claude-step-offers-tools-it-never-allows), story:resumed-run-records-no-layer (a duplicate of story:validator-only-run-records-no-layer, filed by wave 39 with a checkable predicate).
- Cleared: dependency-blocker:story-delivery-2-with-settings, dependency-blocker:engineering-gates-see-the-change and dependency-blocker:gate-programs-resolve-by-name by engineering-protocols 0.12.0; dependency-blocker:worktree-nested-keep-file by worktree 0.19.0.
- Filed: story:repository-scripts-are-rust and story:viewer-is-vue, under the rule that running code committed here is Rust with clap derive, and browser code Vue 3 with TypeScript, Vite and pnpm.
- Rewritten to Rust: story:bench-runs-schema, story:bench-subset-pinned and story:bench-runs-in-journey become `takt-bench` subcommands (story:baseline-runner-in-repo's crate); story:harbor-agent-adapter's takt side becomes `takt-bench harbor-run`, and where its Harbor agent class lives stays an open question in that story, with no exception assumed.
