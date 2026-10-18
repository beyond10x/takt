---
format: aep.planning-md/3
id: review-result:rsi-acceptance-round-2
kind: review-result
status: active
title: Acceptance critic, recursive-self-improvement stories, round 2
relations:
- reviews: story:scenario-corpus
- reviews: story:scorecard-reader
- reviews: story:finding-intake
- reviews: story:promotion
- reviews: story:judge-guard
- reviews: story:agent-profiles
- reviews: story:self-improvement-protocol
- reviews: story:first-recorded-improvement-turn
- reviews: story:loop-scorecard
revision: 1
---
needs-revision
story:finding-intake — the acceptance runs the check "at the promoted revision", which only story:promotion produces (the first baseline comes from `takt promote --baseline-only`), and this story depends on neither it nor anything that does, so the check has no revision to run at when the story lands; name a revision the story itself can build — .engineering/planning/story/finding-intake.md:51
story:finding-intake — the acceptance reads only "prints `reproduced` when the finding's defect is present", so a `scripts/check-finding` that always prints `reproduced` passes; the `not_reproduced` result the Outcome promises (line 23) is named in no acceptance or check — .engineering/planning/story/finding-intake.md:51

Round 1 outcomes, each checked against the revised bodies:
- Landed: all nine round 1 findings, apart from the lines flagged above.
- finding-intake: the duplicate refusal now has a producer, `scripts/file-finding`, which runs `aep plan artifact list --ref finding:<key>` first (line 29). The acceptance is one sentence.
- scorecard-reader: the acceptance is one outcome (token sums). The baseline row sits in Checks, where `--save` provides the fixture.
- promotion and judge-guard: each acceptance is one refusal. Tracing `promotion` also covered its own check: a promoted revision equal to what `--version` prints.
- agent-profiles: the acceptance is now a word-for-word match of the "never" cells against the design's roles table (`.engineering/planning/design/recursive-self-improvement.md:37`), which a script can run.
- self-improvement-protocol: the acceptance is the `accepted` path only. `declined` moved to Checks.
- first-recorded-improvement-turn: the acceptance now names `case.yaml`'s protocol and the last decision's legitimate outcome and `promoted` claim. All of these exist (`decision.rs:80`, `case.rs:362`). The stored-record check replaced the chat message.
- loop-scorecard: the acceptance is one outcome. The record names `loop_measurement` and `dual_reported`/`redefined`, which exist in `protocols/loop-improvement/1.yaml:30-39`.
- story:scenario-corpus is unchanged since round 1 and I still find it observable.

Read: 9 of 9 ids (`aep plan artifact show` on each; in the worktree the story files and the design were read from disk). I also read `protocols/self-improvement/1.yaml`, `decision.rs`, the `validator.rs` merge validator, `case.rs` and `main.rs`, and the epic. I read `critic-rubric.md` and `plan-critic-acceptance.md` first.

Not established:
- `aep plan artifact findings review-result:rsi-acceptance-round-1` prints "no review reviews this, so there is nothing to compare". The round 1 record has no `reviews` relation to compare against here, so I compared by reading the bodies.
- I did not run any script or `takt` verb, because none of them exists yet.
- Out of lane, for plan-critic-design: story:finding-intake has no `depends_on story:promotion`, though its acceptance needs a promoted revision.
- Out of lane, for plan-critic-design: story:judge-guard and story:self-improvement-protocol name story:validator-confinement, story:tree-hygiene and story:authority-hardening only in prose. Those stories are absent from this store, and the epic's Ordering section says the edges are added later.

```findings
- file: .engineering/planning/story/finding-intake.md
  line: 51
  category: acceptance
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "the acceptance runs the check at the promoted revision, which only story:promotion produces, and this story depends on neither it nor anything that does, so the check has no revision to run at when the story lands; name a revision the story itself can build"
- file: .engineering/planning/story/finding-intake.md
  line: 51
  category: acceptance
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "the acceptance reads only 'prints reproduced when the finding's defect is present', so a scripts/check-finding that always prints reproduced passes; the not_reproduced result the Outcome promises is named in no acceptance or check"
```
