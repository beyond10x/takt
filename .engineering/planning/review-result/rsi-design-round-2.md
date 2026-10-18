---
format: aep.planning-md/3
id: review-result:rsi-design-round-2
kind: review-result
status: active
title: Design critic, recursive-self-improvement stories, round 2
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
approve

**What you read.** I read 9 artifacts: the 9 stories in the set, plus the superseded `story:pinned-judge-and-promotion`, whose supersedes edges I checked. I also read `review-result:rsi-design-round-1` and the design's cycle passage. I ran `aep plan artifact show`, `relations`, `graph` and `validate` against the worktree store. `validate` printed "valid". I walked all declared edges in the graph, including those to artifacts outside the set (`story:run-record-and-story-evidence`, `story:week-of-daily-runs`, the epics). The `depends_on` edges form a DAG with no cycle. The in-set chain is scorecard-reader, promotion, judge-guard, self-improvement-protocol, first-recorded-improvement-turn, loop-scorecard. Scenario-corpus, agent-profiles and finding-intake sit beside it, so the set is not serialised. I traced 17 acceptance or check reads to a producer inside the set. 14 are covered by a direct `depends_on` edge, and 3 are covered only by a path through other edges:
- `binary.promote` reads the promotion record: self-improvement-protocol reaches promotion through judge-guard.
- The first turn's promotion claim: first-recorded-improvement-turn reaches promotion through self-improvement-protocol, then judge-guard.
- The operator profile's L2 turn: loop-scorecard reaches finding-intake's edits to the same file through first-recorded-improvement-turn, then self-improvement-protocol, then finding-intake.

Round 1 findings, each checked against the revised bodies:
- **Reader, producer and promotion edges:** these now hold. Judge-guard and promotion are split and each names its own surface. Scorecard-reader's `--against <saved>` takes any saved scorecard, and `story:promotion` owns the baseline choice and writes the first baseline with `--baseline-only`. That removes the reversed baseline read.
- **Operator profile and corpus edges:** `finding-intake` and `loop-scorecard` now `depends_on` agent-profiles. `finding-intake` and `first-recorded-improvement-turn` now `depends_on` scenario-corpus.
- **Shared-file edges:** every edge that exists because two stories edit one file carries its reason in the Ordering text, so none counts as an unexplained chain. These are `crates/takt-cli/src/main.rs`, `lib.rs`, `case.rs`, `validator.rs` and `.agents/operator-agent.md`.

**What you could not establish.**
- **Unreleased hardening stories:** `story:validator-confinement`, `story:tree-hygiene`, `story:authority-hardening` and `story:cli-protocol-defaults` are not in this store. Their prose Ordering lines are outside-the-set dependencies and not a design defect.
- **Turns two and three (out of lane):** `story:loop-scorecard` needs three recorded L1 turns, and nothing in the set produces turns two and three. Its body states this as a gate, so it does not set my verdict. The scope and acceptance critics may want it.
- **"One cycle" (out of lane):** the "one cycle" for dual-reporting in `story:scorecard-reader` (lines 30-31 and check line 60) is not defined. If a cycle ends at the next promotion, the boundary is something `story:promotion` creates, and promotion depends on scorecard-reader. That is a checkability question for the acceptance critic. I could not name a design fix without choosing the definition.

```findings
[]
```
