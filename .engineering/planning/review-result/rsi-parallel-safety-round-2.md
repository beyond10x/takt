---
format: aep.planning-md/3
id: review-result:rsi-parallel-safety-round-2
kind: review-result
status: active
title: Parallel-safety critic, recursive-self-improvement stories, round 2
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
story:finding-intake — its acceptance runs `scripts/check-finding` "at the promoted revision", which is the baseline story:promotion records (the first one by `takt promote --baseline-only`), and no `depends_on` edge runs from this story to story:promotion (FI is unordered against promotion, scorecard-reader and judge-guard), so one wave would hold the reader and its producer; add the edge, or say the acceptance uses a named git revision instead of the promoted one — .engineering/planning/story/finding-intake.md:51-52, .engineering/planning/story/promotion.md:30-32

**What you read.** 9 stories in full (`cat -n` each), `review-result:rsi-parallel-safety-round-1`, the `depends_on` edges from `aep plan artifact graph`, and `aep plan artifact waves`. I built the transitive order by hand and checked every pair with no path between them. Those pairs were agent-profiles against scenario-corpus, scorecard-reader, promotion and judge-guard, and finding-intake against scorecard-reader, promotion and judge-guard. Their scopes (`main.rs`, `lib.rs`, `forge.rs`, `scorecard.rs`, `promotion.rs`, `build.rs`, `case.rs`, `run.rs`, `validator.rs`, `.agents/`, `.claude/agents/`, `AGENTS.md`, `scripts/`) share no file.

**Round 1 against this round.**
- **Fixed.**
  - Shared files `main.rs`, `lib.rs`, `case.rs` and `validator.rs`: the set now has edges, and each Ordering section names the file as the reason.
  - The `scorecard.rs` and `validator.rs` overlaps between scorecard-reader, loop-scorecard and self-improvement-protocol are ordered through first-recorded-improvement-turn.
  - `.agents/operator-agent.md`: edges run from finding-intake and loop-scorecard to agent-profiles, and each body names the file.
  - scorecard-reader now depends on scenario-corpus.
  - self-improvement-protocol reaches the promotion record through judge-guard to promotion.
  - judge-guard now lists `validator.rs`.
  - agent-profiles' acceptance no longer reads finding-intake's procedure.
- **Still open.** One new finding, above. It is the only reader beside a producer that the set leaves unordered.

**What I could not establish.**
- **Surfaces.** 6 stories have at least one cited surface (scenario-corpus, promotion, judge-guard, self-improvement-protocol, agent-profiles, first-recorded-improvement-turn). 3 have inferred surfaces only (scorecard-reader, finding-intake, loop-scorecard). 0 are unplaceable. The inferred ones weaken a collision claim and not the ordering conclusion above, because every file they list is already ordered.
- **Acceptance reads.** I traced 13 reads to a producer in the set. 12 are recorded by a `depends_on` edge, direct or through a chain (self-improvement-protocol to promotion goes via judge-guard). 1 is not recorded, and it is the finding above.
- **Scope frontmatter.** `aep plan artifact waves` lists all 9 stories as "unassessed" because none has a typed `scope`. That makes the CLI's collision check blind to this set. My conclusions rest on the bodies' Scope sections. This is a store-validator matter and I did not let it set my verdict.
- **Outside the set, not assessed.**
  - Edges to story:validator-confinement, story:tree-hygiene, story:authority-hardening and story:cli-protocol-defaults, which are not in the store.
  - The `run-record-and-story-evidence` edge from scorecard-reader.
  - The "three recorded L1 turns" gate in loop-scorecard, whose turns 2 and 3 fall outside this set.
- **Verdict on concurrency.** Apart from the one finding, the set is safe to work in the order its edges give. Without the edge, finding-intake runs alongside promotion, scorecard-reader and judge-guard.

```findings
- file: .engineering/planning/story/finding-intake.md
  line: 51
  category: parallel-safety
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "the acceptance runs scripts/check-finding at the promoted revision, which is the baseline story:promotion records (promotion.md:30-32, first one by takt promote --baseline-only); no depends_on edge runs from this story to story:promotion and the two are unordered, so one wave would hold the reader and its producer; add the edge, or say the acceptance uses a named git revision instead of the promoted one"
```
