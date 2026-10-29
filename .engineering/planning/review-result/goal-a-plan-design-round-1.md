---
format: aep.planning-md/3
id: review-result:goal-a-plan-design-round-1
kind: review-result
status: active
title: Plan critic design, goal A cluster-delivery set, round 1
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

- story:an-unattended-run-merges-under-a-delegated-grant — its outcome and acceptance need a reviewer verdict at the revision, but no reviewer step, validator or evidence kind exists in the run or in `story.delivery`, and no story in the set builds one. The story therefore holds two things, the verdict and the grant, and a model's verdict cannot be evidence under AGENTS.md § Rules. Split the reviewer-verdict step into its own story and add a `depends_on` edge from this one to it, or state in the body where the verdict comes from. — `.engineering/planning/story/an-unattended-run-merges-under-a-delegated-grant.md:20`, `:45`, `:53`; `grep -rniE "reviewer" protocols/story-delivery/3.yaml crates/takt/src/run.rs crates/takt/src/validator.rs` finds nothing
- story:pr-open-not-idempotent-on-resume — it and story:fenced-forge-dispatch both claim that a `pr.open` whose answer was lost is resolved by a read and never re-sent, and no edge records the sharing. The body says "one unit delivers both", but only `fenced-forge-dispatch` carries a `depends_on` edge, and that edge runs to `bot-forge-reads-through-the-app`. Both stories would be marked done by the same change, and fenced's acceptance covers only the merge, so its `pr.open` half is unchecked. Merge the two into one story, or narrow fenced's `pr.open` claim to the outcome record and add a `depends_on` edge from `pr-open-not-idempotent-on-resume` to `fenced-forge-dispatch`. — `.engineering/planning/story/pr-open-not-idempotent-on-resume.md:55`, `.engineering/planning/story/fenced-forge-dispatch.md:21`
- story:report-sums-cases — its acceptance reads a stop line whose tokens are `unknown` when the run recorded none. Today the stop line's `tokens` is a `u64` that a killed step writes as 0 (`events.rs:283-289`). Defining what the record holds for unknown tokens is what story:a-killed-step-records-its-tokens does, and no edge links the two. A `depends_on` or `informed_by` edge from `report-sums-cases` to `a-killed-step-records-its-tokens` would record it; without it the pod-run token sums count killed steps as 0. — `.engineering/planning/story/report-sums-cases.md:57`, `.engineering/planning/story/a-killed-step-records-its-tokens.md:35`

What I read: 8 stories in full, the parent epic, the unattended-merge-grant blocker, `examples/cluster-delivery/README.md`, and the code at `events.rs`, `validator.rs`, `case.rs` and `protocols/story-delivery/3.yaml`. I ran `aep plan artifact relations`, `graph` and `validate` (valid). From the graph I walked 21 edges around these ids, including edges to outside artifacts such as `week-of-daily-runs`, `scan-policy-outside-the-tree` and `a-connectors-forge-adapter`. I found no cycle and no serial chain: three stories fan in on `bot-forge-reads-through-the-app`. I traced 5 acceptance reads to a producer in the set; 2 have an edge (`forge-preflight` and `fenced-forge-dispatch` to `bot-forge-reads-through-the-app`) and 3 have none, which are findings 2 and 3 and the unbuilt reviewer step in finding 1.

What I could not establish:
- Whether `story:a-run-stops-at-a-default-token-budget` conflicts with `story:run-configuration-names-the-story-and-token-variable`. Both touch the run file's `tokens` key. That one is outside the set and in parallel-safety's lane.
- Out of my lane: `an-unattended-run...` still lists its role and verdict ambiguity under a cleared blocker and uses the placeholder `agent:<role>`. `forge-preflight` and `fenced-forge-dispatch` are `active` and `a-killed-step-records-its-tokens` is `draft`, so status mixes across the set. Several stories declare no scope.
- Out of my lane: the set's decomposes parents are mixed. `a-killed-step-records-its-tokens` sits under `epic:recursive-self-improvement`, and `forge-preflight`, `fenced-forge-dispatch` and `report-sums-cases` under `epic:composable-agentic-work`, not under `epic:takt-as-a-loom-agent`.

```findings
- file: .engineering/planning/story/an-unattended-run-merges-under-a-delegated-grant.md
  line: 20
  category: design
  severity: blocker
  verdict: needs-revision
  origin: introduced
  message: "its outcome and acceptance need a reviewer verdict at the revision, but no reviewer step, validator or evidence kind exists in the run or in story.delivery and no story in the set builds one, so the story holds two things (the verdict and the grant); split the reviewer-verdict step into its own story with a depends_on edge, or state where the verdict comes from"
- file: .engineering/planning/story/pr-open-not-idempotent-on-resume.md
  line: 55
  category: design
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "it and story:fenced-forge-dispatch both claim that a lost-answer pr.open is resolved by a read and never re-sent, no edge records the sharing, and fenced's acceptance covers only the merge; merge the two or narrow fenced's pr.open claim and add a depends_on edge from this story to story:fenced-forge-dispatch"
- file: .engineering/planning/story/report-sums-cases.md
  line: 57
  category: design
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "its acceptance reads a stop line whose tokens are unknown when none were recorded, which story:a-killed-step-records-its-tokens defines (the stop line's tokens is a u64 written as 0 today), and no edge links the two; add depends_on or informed_by to story:a-killed-step-records-its-tokens"
```
