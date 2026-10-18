---
format: aep.planning-md/3
id: release-plan:takt-roadmap
kind: release-plan
status: draft
title: takt roadmap by horizon, with work always available
summary: Now (waves 10-11), Next (12-13), Later; outcomes in PRD order; the ready queue holds two waves beyond the open one
relations:
- derived_from: product-requirements:takt
- serves: vision:safe-agentic-coding
revision: 4
---
# Release plan: takt roadmap, from 2026-10-18

The order of takt's work by horizon, kept by the planner role. product-requirements:takt sets the outcomes and their priority; this plan says which stories serve them and in what order. Read with `aep plan artifact waves --kind story --status draft` for what can run at once.

## Standing rule: work is always available

The ready queue holds at least two waves beyond the one open: stories in `draft` whose `depends_on` are implemented or in an open wave, each with a typed scope. The planner checks it at every wave close and tops it up from the next horizon, by decomposing the next epic or design build step, before the developer role opens the next wave. A unit may start early beside an open wave when its scope is disjoint by merge-tree and it commits nothing without its grant.

## Outcomes, in priority order (product-requirements:takt § 5)

1. Scenario 1 accepted unattended: one `takt run` invocation plus one grant, daily for a week.
2. The first takt release promoted by its own scorecard, under the pinned judge.
3. Repositories outside the organization: a forge over the user's `gh` login.
4. Continuous runs: planning as a unit, a tree of cases, one deterministic dispatcher.

Done on 2026-10-18: the first improvement turn, `self.improvement/1`, recorded by hand (phase R0) and accepted (docs/runs/2026-10-18-improvement-turn-1/).

## Now: waves 10 and 11 (open)

| story | serves |
|---|---|
| step-cannot-forge-its-validators | security; every outcome |
| grant-binds-to-action-subject (merged into wave 10) | outcome 2 |
| trial-fetch-needs-no-user-ssh (granted) | outcome 1 |
| scorecard-reader (granted) | outcome 2 |
| records-reach-base | outcome 1 |
| pr-opens-when-change-is-done, no-model-step-for-approval, no-inspect-for-stateless-harness (one unit) | outcome 1; removes the waste the journey view shows |
| binary-embeds-revision | outcome 2 |

## Next: waves 12 and 13

| story | serves | waits on |
|---|---|---|
| story-done-end-to-end | outcome 1, the unattended proof on scenario 1 | records-reach-base, the efficiency unit |
| scan-before-push (with the push-over-ssh low) | security | records-reach-base (same PrOpen code) |
| promotion | outcome 2 | scorecard-reader, binary-embeds-revision, the token and flakiness rule (design:recursive-self-improvement § Scorecard, open) |
| case-scoped-step-guard | reliability | none |
| signal-releases-the-case | reliability | none |
| forge-preflight, untracked-cargo-lock-remedy, abandon-a-case, cli-small-fixes | usability | none |

## Later

| story or epic | serves | waits on |
|---|---|---|
| week-of-daily-runs | outcome 1, the week | story-done-end-to-end |
| judge-guard | outcome 2 | promotion |
| self-improvement-protocol (validators drive the turn, phase R1) | outcome 2 | judge-guard, finding-intake (done), scorecard-reader |
| loop-scorecard | the L2 loop | three recorded L1 turns |
| plain-gh-forge | outcome 3 | none |
| confinement-seam (fail-closed profile, openat2) | security | none; large |
| normalized-harness-trace | measurement | harness-journey-timeline (done) |
| second-harness-adapter | the seams (M4) | normalized-harness-trace helps |
| epic:continuous-orchestration (build steps 2–8 of design:continuous-orchestration) | outcome 4 | declared-loops, workflow-schema-and-loader; decompose before wave 14 |
| epic:benchmark-integration (design:benchmark-integration): six realistic scenarios and the in-repository runner first (scenario-corpus-realistic, baseline-runner-in-repo, bench-nouns-in-ess), then Terminal-Bench 2.0 and SWE-bench Verified Mini through the organization's benchmark instrument | measurement for every outcome | none for the first three; the container and protocol stories for the benchmarks |
| workflow-schema-and-loader, declared-loops | M3 and outcome 4 | the store-guard constraint in workflow-schema-and-loader |

## Open decisions held by the owner

- The token rule and flaky-outcome tolerance for promotion (design:recursive-self-improvement § Scorecard).
- The order of the tool-calling stories from the 2026-10-18 brainstorm, filed as drafts on 2026-10-18: story:environment-allowlist, story:tool-scope-per-action, story:in-call-tool-guard (after tool-scope-per-action), story:secret-redaction, story:fenced-forge-dispatch.
