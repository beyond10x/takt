---
format: aep.planning-md/3
id: review-result:rsi-parallel-safety-round-1
kind: review-result
status: active
title: Parallel-safety critic, recursive-self-improvement stories, round 1
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
story:scenario-corpus — its `crates/takt-cli/src/main.rs` scope (inferred) is also claimed by story:scorecard-reader (`scorecard`) and story:pinned-judge-and-promotion (`promote`, `--by`, `--version`). No body names the overlap and no `depends_on` edge orders scenario-corpus against either, so one wave would hold three agents on one file; either record the shared file as the reason for an ordering edge or split `main.rs`. — .engineering/planning/story/scenario-corpus.md:37, .engineering/planning/story/scorecard-reader.md:39, .engineering/planning/story/pinned-judge-and-promotion.md:41
story:pinned-judge-and-promotion — it and story:self-improvement-protocol both land on `crates/takt/src/case.rs` (inferred, both Scope sections; typed grants versus producer independence). Neither body mentions the other and the only edges leaving each go to scorecard-reader and finding-intake, so nothing orders them; either record the shared file as the reason for an ordering edge or split `case.rs`. — .engineering/planning/story/pinned-judge-and-promotion.md:39, .engineering/planning/story/self-improvement-protocol.md:42
story:pinned-judge-and-promotion — its Outcome reads the gates inputs from the promoted base, and its Context names the current read site `crates/takt/src/validator.rs:193-216` (cited, `observe` takes the tree). Its Scope does not list `validator.rs`, which story:self-improvement-protocol and story:loop-scorecard both list, so the real collision on that file is unstated (inferred); add the surface or say the change lives elsewhere. — .engineering/planning/story/pinned-judge-and-promotion.md:19-21, .engineering/planning/story/pinned-judge-and-promotion.md:27, .engineering/planning/story/pinned-judge-and-promotion.md:37-43
story:self-improvement-protocol — its acceptance reaches `accepted` "after a promotion with no worse scorecard row" and its Scope carries a `binary.promote` validator. The promotion record and `takt promote` are created by story:pinned-judge-and-promotion, and no `depends_on` edge runs from this story to it, so the reader's acceptance cannot be checked until the producer lands; add the edge or merge the two. — .engineering/planning/story/self-improvement-protocol.md:53-55, .engineering/planning/story/pinned-judge-and-promotion.md:20-22
story:agent-profiles — its acceptance has a session file a finding "through story:finding-intake's procedure", which story:finding-intake writes as the intake procedure into `.agents/operator-agent.md`. No `depends_on` edge runs from agent-profiles to finding-intake, so one wave would hold the reader and its producer; add the edge or merge the two. — .engineering/planning/story/agent-profiles.md:41-42, .engineering/planning/story/finding-intake.md:35
story:finding-intake — it writes the intake procedure into `.agents/operator-agent.md` (inferred), a file story:agent-profiles creates (cited, "new:"). The file does not exist yet (`ls .agents` fails in the worktree). Finding-intake's body does not say it writes into a file another item creates, and no edge orders the two (the dependency is one-directional in agent-profiles' acceptance); the body must name it. — .engineering/planning/story/finding-intake.md:35, .engineering/planning/story/agent-profiles.md:30
story:loop-scorecard — it also writes `.agents/operator-agent.md` (the L2 turn), the file story:agent-profiles creates. Agent-profiles' Out of Scope names loop-scorecard but loop-scorecard's body does not name agent-profiles, and no `depends_on` edge runs between them; add the edge with the file as its reason. — .engineering/planning/story/loop-scorecard.md:41, .engineering/planning/story/agent-profiles.md:52
story:scorecard-reader — its acceptance reads the case directories of "two scenario 1 runs", the run unit story:scenario-corpus defines (fresh `trial/<n>` base, `scenarios/` file). No `depends_on` edge runs to scenario-corpus; either add the edge or say the two runs are made by hand without it. — .engineering/planning/story/scorecard-reader.md:47-48, .engineering/planning/story/scenario-corpus.md:17-19

**What you read.** 8 stories, plus the 6 `depends_on` edges inside the set. Commands: `aep plan artifact graph`, `aep plan artifact relations`, `cat -n` of all 8 bodies, `ls` and `sed` on the worktree tree for `.agents`, `crates/takt/src/validator.rs:185-220` and `crates/takt/src/run.rs:15-30`, and `git status`.

**What I could not establish.**
- **Surfaces.** 3 items have at least one cited surface (agent-profiles, self-improvement-protocol via `protocols/self-improvement/1.yaml`, first-recorded-improvement-turn via the scratch repository's scenario 1). 5 are inferred only (scenario-corpus, scorecard-reader, finding-intake, pinned-judge-and-promotion, loop-scorecard). 0 are unplaceable, but first-recorded-improvement-turn's "case directory" surface is thin.
- **Acceptance reads.** I traced 9 acceptance reads to a producer in the set. 6 are recorded by a `depends_on` edge (self-improvement-protocol to finding-intake and to scorecard-reader, pinned-judge to scorecard-reader, first-recorded to scenario-corpus and to self-improvement-protocol, loop-scorecard to first-recorded). 3 are not (self-improvement-protocol to pinned-judge, agent-profiles to finding-intake, scorecard-reader to scenario-corpus), and each is a finding above.
- **Already ordered.** scorecard.rs (scorecard-reader, then loop-scorecard) and validator.rs (self-improvement-protocol, then loop-scorecard) are ordered through first-recorded-improvement-turn, so I report no collision on them.
- **Outside the set.** first-recorded-improvement-turn needs the fix from `story:cli-protocol-defaults`, and several stories depend on `story:run-record-and-story-evidence` or on `story:validator-confinement`. I did not assess these because they are outside the set. The first is not in the store at all.
- **Other lanes.** The `.engineering/planning/story/loop-scorecard.md:46-47` "not ready until three L1 turns" gate and the unset typed `scope` frontmatter are not in my lane.

```findings
- file: .engineering/planning/story/scenario-corpus.md
  line: 37
  category: parallel-safety
  severity: blocker
  verdict: needs-revision
  origin: introduced
  message: "inferred surface: crates/takt-cli/src/main.rs is also in the scope of story:scorecard-reader (scorecard.md:39) and story:pinned-judge-and-promotion (pinned-judge-and-promotion.md:41); no body names the overlap and no depends_on edge orders scenario-corpus against either; add an ordering edge recording the file as its reason, or split main.rs"
- file: .engineering/planning/story/pinned-judge-and-promotion.md
  line: 39
  category: parallel-safety
  severity: blocker
  verdict: needs-revision
  origin: introduced
  message: "inferred surface: crates/takt/src/case.rs is also in the scope of story:self-improvement-protocol (self-improvement-protocol.md:42); neither body mentions the other and no depends_on edge orders them; add an ordering edge recording the file as its reason, or split case.rs"
- file: .engineering/planning/story/pinned-judge-and-promotion.md
  line: 27
  category: parallel-safety
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "inferred surface: the Outcome reads the gates inputs from the promoted base and the Context cites crates/takt/src/validator.rs:193-216 as the current read site, but Scope omits validator.rs, which story:self-improvement-protocol and story:loop-scorecard both list; the real collision on that file is unstated"
- file: .engineering/planning/story/self-improvement-protocol.md
  line: 53
  category: parallel-safety
  severity: blocker
  verdict: needs-revision
  origin: introduced
  message: "the acceptance reaches accepted after a promotion with no worse scorecard row, and the binary.promote validator reads the promotion record and takt promote that story:pinned-judge-and-promotion creates (pinned-judge-and-promotion.md:20-22); no depends_on edge runs from this story to it; add the edge or merge the two"
- file: .engineering/planning/story/agent-profiles.md
  line: 41
  category: parallel-safety
  severity: blocker
  verdict: needs-revision
  origin: introduced
  message: "the acceptance files a finding through story:finding-intake's procedure, which finding-intake writes into .agents/operator-agent.md (finding-intake.md:35); no depends_on edge runs from this story to finding-intake; add the edge or merge the two"
- file: .engineering/planning/story/finding-intake.md
  line: 35
  category: parallel-safety
  severity: blocker
  verdict: needs-revision
  origin: introduced
  message: "inferred surface: it writes the intake procedure into .agents/operator-agent.md, a file story:agent-profiles creates (agent-profiles.md:30) that does not exist yet; this body does not say it writes into a file another item creates and no edge orders the two"
- file: .engineering/planning/story/loop-scorecard.md
  line: 41
  category: parallel-safety
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "inferred surface: it also writes .agents/operator-agent.md, which story:agent-profiles creates; agent-profiles.md:52 names loop-scorecard but this body does not name agent-profiles and no depends_on edge runs between them; add the edge with the file as its reason"
- file: .engineering/planning/story/scorecard-reader.md
  line: 47
  category: parallel-safety
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "the acceptance reads the case directories of two scenario 1 runs, the run unit story:scenario-corpus defines (scenario-corpus.md:17-19); no depends_on edge runs from this story to scenario-corpus; add the edge or say the two runs are made by hand without it"
```
