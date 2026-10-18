---
format: aep.planning-md/3
id: review-result:rsi-acceptance-round-1
kind: review-result
status: active
title: Acceptance critic, recursive-self-improvement stories, round 1
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
story:finding-intake — the acceptance's "refused the second time" has no producer: `aep plan artifact new` accepts a second artifact with the same `--ref` (I created two with `finding:k1`), and Scope names only `.agents/operator-agent.md` and a predicate format, so the check `aep plan artifact list --ref finding:<key>` would return two — .engineering/planning/story/finding-intake.md:44
story:finding-intake — the acceptance joins two independent outcomes with "and" (duplicate refusal; every filed finding has a predicate a script parses and runs), so one can pass while the other fails, and no script is named in Scope — .engineering/planning/story/finding-intake.md:44
story:scorecard-reader — the acceptance joins two outcomes with "and" (token sums per row; marking a row worse against the baseline), and the second needs a promoted-revision baseline record that no story before it produces, since story:pinned-judge-and-promotion depends on this one, so the check has no fixture — .engineering/planning/story/scorecard-reader.md:47
story:pinned-judge-and-promotion — the acceptance joins three independent outcomes (judge-surface merge refused without `human:`; docs-only merge under either grant; `takt promote` gated on the scorecard with a record matching `takt --version`), so one can pass while the others fail — .engineering/planning/story/pinned-judge-and-promotion.md:52
story:agent-profiles — the acceptance states two outcomes for two roles joined by "and", and it is a fresh-session behaviour that cannot give the same answer twice, while its only repeatable checks (`b10x-gates scan-text`, the roles-table "never" mapping) cover neither "files a finding and sends `[FINDING takt]`" nor "refuses to mark its own fix verified" — .engineering/planning/story/agent-profiles.md:41
story:self-improvement-protocol — the acceptance joins two independent paths (`accepted` after a promotion; `declined` over a still-reproducing candidate) in one sentence, so one can pass while the other fails — .engineering/planning/story/self-improvement-protocol.md:53
story:first-recorded-improvement-turn — the acceptance says "`takt` shows a `self-improvement/1` case" but `takt` has no verb that shows a case (`Run`, `Approve`, `Protocols` only) and the protocol's id is `self.improvement`, so the sentence names no command or id string that can be matched — .engineering/planning/story/first-recorded-improvement-turn.md:45
story:first-recorded-improvement-turn — the check "A `[VERIFIED takt]` line ... was sent" is a chat message with no stored record, and the Context says messages were held and some never delivered, so it cannot be looked at twice — .engineering/planning/story/first-recorded-improvement-turn.md:52
story:loop-scorecard — the acceptance joins two independent outcomes with "and" (`loop.metric_below_target` reached from a validator record; the first L2 finding filed through story:finding-intake), and leaves out the definition-check and profile-L2-turn parts of the Outcome — .engineering/planning/story/loop-scorecard.md:50

Read: 8 of 8 ids (`aep plan artifact show` on each, plus `kinds` and `lifecycle story`), the two protocol files, `main.rs` subcommands, and `aep plan artifact new --ref` run on a copy of the store in a scratch directory.
Not established:
- story:scenario-corpus acceptance is observable, so I did not flag it.
- I did not run `improvement_protocols.rs`.
- Out of lane, for plan-critic-design:
  - story:self-improvement-protocol reaches `accepted` through `binary.promote` and `candidate.merge*` (`repository.merge.judge_surface`, `takt.promote`), but has no `depends_on story:pinned-judge-and-promotion`.
  - Prose references to story:validator-confinement, story:tree-hygiene, story:authority-hardening and story:cli-protocol-defaults name artifacts absent from this store (the confinement story here is story:confinement-seam).

```findings
- file: .engineering/planning/story/finding-intake.md
  line: 44
  category: acceptance
  severity: blocker
  verdict: needs-revision
  origin: introduced
  message: "the acceptance's 'refused the second time' has no producer: aep plan artifact new accepts a second artifact with the same --ref, and Scope names no mechanism that refuses, so the check 'aep plan artifact list --ref finding:<key>' would return two"
- file: .engineering/planning/story/finding-intake.md
  line: 44
  category: acceptance
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "the acceptance joins two independent outcomes with 'and' (duplicate refusal; every filed finding has a predicate a script parses and runs), and no script is named in Scope"
- file: .engineering/planning/story/scorecard-reader.md
  line: 47
  category: acceptance
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "the acceptance joins two outcomes with 'and' (token sums per row; marking a row worse against the baseline), and the second needs a promoted-revision baseline record that no earlier story produces, so the check has no fixture"
- file: .engineering/planning/story/pinned-judge-and-promotion.md
  line: 52
  category: acceptance
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "the acceptance joins three independent outcomes (judge-surface merge needs human grant; docs-only merge under either grant; takt promote gated on the scorecard with a record matching takt --version), so one can pass while the others fail"
- file: .engineering/planning/story/agent-profiles.md
  line: 41
  category: acceptance
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "the acceptance states two outcomes for two roles joined by 'and' and is a fresh-session behaviour that cannot give the same answer twice, while its repeatable checks cover neither filing a finding and sending [FINDING takt] nor refusing to mark its own fix verified"
- file: .engineering/planning/story/self-improvement-protocol.md
  line: 53
  category: acceptance
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "the acceptance joins two independent paths (accepted after a promotion; declined over a still-reproducing candidate) in one sentence, so one can pass while the other fails"
- file: .engineering/planning/story/first-recorded-improvement-turn.md
  line: 45
  category: acceptance
  severity: blocker
  verdict: needs-revision
  origin: introduced
  message: "the acceptance says 'takt shows a self-improvement/1 case' but takt has no verb that shows a case (Run, Approve, Protocols only) and the protocol's id is self.improvement (protocols/self-improvement/1.yaml:4), so it names no command or id string that can be matched"
- file: .engineering/planning/story/first-recorded-improvement-turn.md
  line: 52
  category: acceptance
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "the check 'A [VERIFIED takt] line with before and after numbers was sent' is a chat message with no stored record, and the Context says messages were held and some never delivered, so it cannot be looked at twice"
- file: .engineering/planning/story/loop-scorecard.md
  line: 50
  category: acceptance
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "the acceptance joins two independent outcomes with 'and' (loop.metric_below_target reached from a validator record; the first L2 finding filed through story:finding-intake) and leaves out the definition-check and profile-L2-turn parts of the Outcome"
```
