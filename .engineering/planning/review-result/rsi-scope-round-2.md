---
format: aep.planning-md/3
id: review-result:rsi-scope-round-2
kind: review-result
status: active
title: Scope critic, recursive-self-improvement stories, round 2
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
- reviews: epic:recursive-self-improvement
revision: 1
---
approve

**What you read.** 11 artifacts: the epic, the design (revision 2), the 9 stories, `story:week-of-daily-runs` (for the other epic's claims on scorecard figures) and `review-result:rsi-scope-round-1`. I ran `aep plan artifact show` on each and `aep plan artifact graph` with cwd the rsi-design worktree. I extracted 28 promises from the epic and the design and traced all 28 to exactly one item, or to a named exclusion.

Round 1's five findings, checked against the revised bodies:

| Round 1 finding | Now |
|---|---|
| The epic's promotion had no claimant | `story:first-recorded-improvement-turn` now ends in "the candidate's binary promoted", and its acceptance requires claim `promoted` true. |
| The median and daily cap were dropped | `story:promotion` carries "median of tokens at most the baseline's plus 10 %" and "no other promotion in the same UTC day". |
| The previous-definition print was only on loop metrics | `story:scorecard-reader` now has it in its Context and its Checks. |
| R3 was unclaimed | `epic:recursive-self-improvement` § Out of Scope defers it, and `story:loop-scorecard` repeats the exclusion. |
| Source path and `--ref` dedupe had no source in the design | Design revision 2 now asks for both (design lines 55-58), so the reach is gone. |

Two checks for overlap came out clean. The split of the old story is clean: `story:promotion` installs the binary and `story:judge-guard` reads the protocol and store from base and guards the merge, which matches design lines 75-78. Loop and scorecard metrics are claimed separately, by `story:loop-scorecard` and `story:scorecard-reader`.

**What I could not establish.**
- The design's "gates and assertions files … read from the promoted base revision" (design line 73) names `story:validator-confinement` for the gates inputs. That story is not in this store, so I cannot confirm it covers the assertions files. The epic's Ordering section names its absence, so I treated it as the honest omission.
- "Promotion defaults, changeable by the owner" (design line 91) is met only if a code constant changed by a judge-surface merge counts as changeable. `story:promotion` says "the defaults" and adds no configuration step. It is borderline, not a finding.
- R1 "takt runs the scratch repository's scenarios daily" (design line 151) has no story that schedules it. I treated it as excluded by the design's "A scheduler of its own" (design line 162) and by `story:week-of-daily-runs` placing the schedule outside the repository.
- Out of my lane: `story:loop-scorecard` and `story:finding-intake` both edit `.agents/operator-agent.md`. Both depend on `story:agent-profiles` but not on each other, so parallel safety should look at it. The missing `depends_on` edges to the hardening stories are named in the epic's Ordering section, which defers them.

```findings
[]
```
