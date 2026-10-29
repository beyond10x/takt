---
format: aep.planning-md/3
id: review-result:goal-a-plan-scope-round-1
kind: review-result
status: active
title: Plan critic scope, goal A cluster-delivery set, round 1
relations:
- reviews: story:a-run-stops-at-a-default-token-budget
- reviews: story:a-killed-step-records-its-tokens
- reviews: story:bot-forge-reads-through-the-app
- reviews: story:forge-preflight
- reviews: story:pr-open-not-idempotent-on-resume
- reviews: story:fenced-forge-dispatch
- reviews: story:an-unattended-run-merges-under-a-delegated-grant
- reviews: story:report-sums-cases
revision: 1
---
needs-revision

- story:pr-open-not-idempotent-on-resume — claims the same outcome as story:fenced-forge-dispatch, which already says that after a `pr.open` write whose answer was lost the next step reads the forge and never sends the write again. It also targets a resumed case, which the goal's one-shot, person-free Job never does. Revise the body either to claim only what fenced-forge-dispatch leaves out, or to name the goal sentence that needs resume. — .engineering/planning/story/pr-open-not-idempotent-on-resume.md:55 (against .engineering/planning/story/fenced-forge-dispatch.md:21)

What I read: the parent README `examples/cluster-delivery/README.md` and `epic:takt-as-a-loom-agent` first. Then the 8 stories and the cleared decision-blocker in full. I also read `story:a-turn-pins-the-repository-of-its-forge`, `story:bench-subset-and-budget`, `story:a-red-base-stops-at-open`, `story:repeated-identical-test-failures-stop-the-run`, `story:week-of-daily-runs`, `story:git-excluded-files-are-not-step-changes`, `story:run-names-its-story` and `approval-record:goal-a-pod-run-safety`. Commands: `aep plan artifact show` for each, `aep plan artifact graph`, `aep plan artifact relations`.

Promises: I extracted 7 from the goal and the README's stop rules and Open table, and traced 7 to an item or to a named omission.
- Run in a kind cluster as a LoomAgent: the epic, not drafted here.
- 3 stories from 2 repositories: `story:run-names-its-story` and the README's own list of three.
- PR by `b10x-bot[bot]`: `story:bot-forge-reads-through-the-app`, `story:forge-preflight`.
- Merged to main with no person's step: `story:an-unattended-run-merges-under-a-delegated-grant`, with its blocker cleared as answer A.
- 3,000,000-token stop per run: `story:a-run-stops-at-a-default-token-budget`, with `story:a-killed-step-records-its-tokens` making it hold for killed steps.
- `takt report --cases` giving units accepted, tokens and interventions: `story:report-sums-cases`.
- Forge acts only on the repository the case named: `story:a-turn-pins-the-repository-of-its-forge`, deliberately left out (see below).

What I could not establish:
- `story:a-turn-pins-the-repository-of-its-forge` is deferred to wave 2 by `approval-record:goal-a-pod-run-safety`, as concerning improvement turns, not pod delivery runs. I took that as a named omission, not a gap, but the README's Open row still lists it for the pod.
- Whether the two llm stories are `proposed` is in llm's store, outside this parent's reach.
- Out of my lane, for the design critic: `story:report-sums-cases` counts every `authority.yaml` approval as an intervention. The agent-role grant from `story:an-unattended-run-merges-under-a-delegated-grant` would then count as one, and the goal expects none.
- Out of my lane, for the design critic: `story:a-killed-step-records-its-tokens` also asks that "the scorecard treats unknown as unknown", a scorecard promise the goal does not make. I left it out of the findings because the story is pre-existing and its core is traceable to the token stop.

```findings
- file: .engineering/planning/story/pr-open-not-idempotent-on-resume.md
  line: 55
  category: scope
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "story:pr-open-not-idempotent-on-resume claims the same outcome as story:fenced-forge-dispatch (a lost-answer pr.open is resolved by a read, never a second POST) and targets a resumed case the goal's one-shot Job never makes; the body should claim only what fenced-forge-dispatch leaves out or name the goal sentence that needs resume"
```
