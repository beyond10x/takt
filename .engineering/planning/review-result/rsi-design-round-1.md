---
format: aep.planning-md/3
id: review-result:rsi-design-round-1
kind: review-result
status: active
title: Design critic, recursive-self-improvement stories, round 1
relations:
- reviews: story:scenario-corpus
- reviews: story:scorecard-reader
- reviews: story:finding-intake
- reviews: story:pinned-judge-and-promotion
- reviews: story:agent-profiles
- reviews: story:self-improvement-protocol
- reviews: story:first-recorded-improvement-turn
- reviews: story:loop-scorecard
revision: 1
---
needs-revision

story:self-improvement-protocol — its acceptance reaches `accepted` "after a promotion" and its scope builds the `binary.promote` validator, which reads the promotion record and the `takt promote` / `takt --version` revision that story:pinned-judge-and-promotion creates, and no depends_on edge runs from it to that story; add `depends_on story:pinned-judge-and-promotion` — .engineering/planning/story/self-improvement-protocol.md:54-55, .engineering/planning/story/pinned-judge-and-promotion.md:21-22,54-55
story:scorecard-reader — its acceptance and second check compare against "the promoted revision's record", but only story:pinned-judge-and-promotion writes a promotion record, and that story depends_on this one, so the baseline has no producer ahead of the reader; the body must say who records the first baseline (and its format), or move the baseline comparison into story:pinned-judge-and-promotion — .engineering/planning/story/scorecard-reader.md:48-49,54-55, .engineering/planning/story/pinned-judge-and-promotion.md:21-22
story:agent-profiles — it creates `.agents/operator-agent.md` while its acceptance reads the intake procedure that story:finding-intake writes into the same file, and finding-intake does not mark that file `new:`. The two stories each hold half of the operator profile and no edge orders them. Either `finding-intake depends_on agent-profiles` and the filing-through-intake sentence moves to finding-intake's acceptance, or the intake procedure moves into agent-profiles — .engineering/planning/story/agent-profiles.md:30,41-42, .engineering/planning/story/finding-intake.md:35
story:loop-scorecard — it adds the L2 turn to `.agents/operator-agent.md`, which story:agent-profiles creates, and no path of edges leads from it to story:agent-profiles; add `depends_on story:agent-profiles` — .engineering/planning/story/loop-scorecard.md:41, .engineering/planning/story/agent-profiles.md:30
story:finding-intake — its acceptance and second check run each filed predicate "on its scenario", and the scenario declaration (`scenarios/` file, `trial/<n>` base) is what story:scenario-corpus creates, with no depends_on edge; add `depends_on story:scenario-corpus` — .engineering/planning/story/finding-intake.md:44-46,50-51, .engineering/planning/story/scenario-corpus.md:30
story:pinned-judge-and-promotion — it is two things in one: the judge guard (typed grants, merge-time allow-list guard, base-revision reads) depends on the hardening stories and not on the scorecard, while `takt promote` and the promotion record depend on story:scorecard-reader. The single `depends_on story:scorecard-reader` holds the guard half behind the scorecard, and the seam is where finding 1's edge lands. Split into a judge-guard story and a promotion story, or say in the body why the two ship together — .engineering/planning/story/pinned-judge-and-promotion.md:18-22,47-48,52-55

**What you read.** 9 artifacts (the 8 stories and the epic) plus design:recursive-self-improvement, protocols/self-improvement/1.yaml and protocols/loop-improvement/1.yaml, using `aep plan artifact show` on each, `relations`, `graph` and `validate`. I walked all 52-artifact graph edges, including those outside the set. I found no cycle: the `depends_on` edges form a DAG, and the longest chain is run-record, scorecard-reader, self-improvement-protocol, first-recorded-improvement-turn, loop-scorecard, with side branches, so the set is not serialised. `validate` reported "valid". I traced 9 acceptance or check reads to a producer inside the set. Four of those have an edge (scorecard-reader to run-record is outside the set; protocol to finding-intake and scorecard-reader; first-recorded to corpus and protocol; loop-scorecard to first-recorded). The other five have none, and they are findings 1-3 and 5, which are listed above. The graph also shows `story:week-of-daily-runs depends_on story:scorecard-reader`, an edge from the other epic that is consistent with this set.

**What you could not establish.**
- The three hardening stories (story:validator-confinement, story:tree-hygiene, story:authority-hardening) are not in this store, so I did not check the graph around them. The prose Ordering sections are not a finding, because the edges cannot be written until those stories land.
- story:loop-scorecard needs three recorded L1 turns, but the set records only the first (story:first-recorded-improvement-turn), and nothing produces turns two and three. This is out of my lane (scope or acceptance) and did not set my verdict.
- story:first-recorded-improvement-turn names story:cli-protocol-defaults, which is not in this store. It is an outside dependency and not a design defect.
- Whether story:scorecard-reader's "two scenario 1 runs" need the scenario declaration from story:scenario-corpus, or can reuse existing case directories, is unclear. I left it out of the findings.

```findings
- file: .engineering/planning/story/self-improvement-protocol.md
  line: 54
  category: design
  severity: blocker
  verdict: needs-revision
  origin: introduced
  message: "its acceptance reaches accepted after a promotion and its scope builds the binary.promote validator, which reads the promotion record and takt promote revision that story:pinned-judge-and-promotion creates, and no depends_on edge runs from it to that story; add depends_on story:pinned-judge-and-promotion"
- file: .engineering/planning/story/scorecard-reader.md
  line: 54
  category: design
  severity: blocker
  verdict: needs-revision
  origin: introduced
  message: "its acceptance and second check compare against the promoted revision's record, but only story:pinned-judge-and-promotion writes a promotion record and that story depends_on this one, so the baseline has no producer ahead of the reader; the body must say who records the first baseline and its format, or the baseline comparison moves into story:pinned-judge-and-promotion"
- file: .engineering/planning/story/agent-profiles.md
  line: 41
  category: design
  severity: blocker
  verdict: needs-revision
  origin: introduced
  message: "it creates .agents/operator-agent.md while its acceptance reads the intake procedure story:finding-intake writes into the same file (not marked new there), so half of one profile sits in each story with no edge; either finding-intake depends_on agent-profiles and the filing-through-intake sentence moves to finding-intake's acceptance, or the intake procedure moves into agent-profiles"
- file: .engineering/planning/story/loop-scorecard.md
  line: 41
  category: design
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "it adds the L2 turn to .agents/operator-agent.md, which story:agent-profiles creates, and no edge path leads from it to story:agent-profiles; add depends_on story:agent-profiles"
- file: .engineering/planning/story/finding-intake.md
  line: 44
  category: design
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "its acceptance and second check run each filed predicate on its scenario, and the scenario declaration is what story:scenario-corpus creates, with no depends_on edge; add depends_on story:scenario-corpus"
- file: .engineering/planning/story/pinned-judge-and-promotion.md
  line: 18
  category: design
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "it is two things in one, the judge guard (typed grants, merge-time allow-list guard, base-revision reads) which depends on the hardening stories and not the scorecard, and promotion (takt promote, promotion record) which depends on story:scorecard-reader, so the single depends_on scorecard-reader holds the guard behind the scorecard; split at that seam or state why they ship together"
```
