---
format: aep.planning-md/3
id: review-result:rsi-scope-round-1
kind: review-result
status: active
title: Scope critic, recursive-self-improvement stories, round 1
relations:
- reviews: story:scenario-corpus
- reviews: story:scorecard-reader
- reviews: story:finding-intake
- reviews: story:pinned-judge-and-promotion
- reviews: story:agent-profiles
- reviews: story:self-improvement-protocol
- reviews: story:first-recorded-improvement-turn
- reviews: story:loop-scorecard
- reviews: epic:recursive-self-improvement
revision: 1
---
needs-revision
epic:recursive-self-improvement — the epic promises "One promotion of a takt binary was decided by the scorecard", and no item claims an actual promotion, because story:first-recorded-improvement-turn ends "accepted or declined" with no promotion step (take it there) — .engineering/planning/epic/recursive-self-improvement.md:23
story:pinned-judge-and-promotion — the design's "median of tokens at most the baseline plus 10 %; at most one promotion a day" is narrowed to "no worse row" with nothing recording the drop of the median and the daily cap, and story:scorecard-reader's per-row 10 % is a different rule — .engineering/planning/story/pinned-judge-and-promotion.md:54
story:scorecard-reader — the design's "for one cycle after a change the previous definition's numbers are printed beside the new ones" is promised for the scorecard's metrics, but only story:loop-scorecard carries it, and only for loop metrics; this body is silent — .engineering/planning/design/recursive-self-improvement.md:90-92
epic:recursive-self-improvement — phase R3 ("takt proposes stories from its scorecards; the owner prioritises") is claimed by no item and excluded by neither the epic nor any story (story:loop-scorecard excludes only automating the L2 turn); the epic should state it deferred or take a story — .engineering/planning/design/recursive-self-improvement.md:151
story:finding-intake — the outcome adds a source-path requirement and a `--ref` refusal of duplicate filings, and no sentence of the design asks for either (the design asks only for an `expected_before` predicate) — .engineering/planning/story/finding-intake.md:19-20

**What you read.** 10 artifacts: the epic, the design and the 8 stories, plus `story:week-of-daily-runs` for coverage. Commands: `aep plan artifact show` for each, and `aep plan artifact graph`. I extracted about 26 promises from the design and epic. I traced 22 to an item. The 4 untraced ones are the 4 findings above, counting the two promotion-default rules as one. I did not run `aep plan artifact kinds` or `relations`.

**What I could not establish.**
- Out of my lane: the design's R2 entry condition "a scorecard baseline recorded" (design line 150) names no producer. Story:scorecard-reader reads a baseline "from the promoted revision's record", and nothing writes one. It may be a story:pinned-judge-and-promotion promotion-record question, so I left it for the design critic.
- Out of my lane: story:first-recorded-improvement-turn needs the fix from story:cli-protocol-defaults merged, but it carries no `depends_on` edge to it. That story is in the hardening branch, which is not in this store.
- Out of my lane: story:finding-intake and story:loop-scorecard both list `.agents/operator-agent.md` in their scope, and story:agent-profiles creates that file. Parallel safety belongs to its own critic.
- The validator stories named in the epic's Ordering (story:validator-confinement, story:tree-hygiene, story:authority-hardening) are not in this store. I treated their absence as the named, honest omission the epic describes.

```findings
- file: .engineering/planning/epic/recursive-self-improvement.md
  line: 23
  category: scope
  severity: blocker
  verdict: needs-revision
  origin: introduced
  message: "the epic promises 'One promotion of a takt binary was decided by the scorecard' and no drafted item claims an actual promotion, because story:first-recorded-improvement-turn ends 'accepted or declined' with no promotion step; it would most naturally take it"
- file: .engineering/planning/story/pinned-judge-and-promotion.md
  line: 54
  category: scope
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "the design's promotion defaults 'the median of tokens at most the baseline plus 10 %; at most one promotion a day' (design line 89) are narrowed to 'no worse row' and nothing records dropping the median and the daily cap"
- file: .engineering/planning/design/recursive-self-improvement.md
  line: 90
  category: scope
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "story:scorecard-reader does not claim the design's rule that a changed scorecard metric definition prints the previous numbers beside the new ones for one cycle; only story:loop-scorecard carries it, and only for loop metrics"
- file: .engineering/planning/design/recursive-self-improvement.md
  line: 151
  category: scope
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "phase R3 'takt proposes stories from its scorecards; the owner prioritises' is claimed by no item and excluded by neither the epic nor any story; the epic should state it deferred or take a story"
- file: .engineering/planning/story/finding-intake.md
  line: 19
  category: scope
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "the outcome adds a source-path requirement and a --ref refusal of duplicate filings that no sentence of the design asks for; the design asks only for an expected_before predicate"
```
