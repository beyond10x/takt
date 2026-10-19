# Wave 14: toward a self-improvement turn the installed takt accepts: the judge guard, the self-improvement protocol, a scorecard bound to its binary, an anchored promotion chain, a confined check run

Skill: `aep:implementing` 0.23.1, wave mode. Stage: **running**. Approval: the planner role named these units and lanes for release 0.2.0, which ships when one `self.improvement/1` case reaches `accepted` with every piece of evidence recorded by validators, promoted by the installed takt; each judge-surface merge needs the planner's grant of its exact commit, on a reviewer verdict. Mode: interactive, up to five implementors in parallel.

## Units

| lane | unit | serves | acceptance (condensed) | judge surface |
|---|---|---|---|---|
| A1 | `story:judge-guard` | `vision:safe-agentic-coding` | a judge-surface merge granted by `agent:planner` is refused unless a delegation is on record at the base, and refused again once revoked | yes |
| A2 | `story:self-improvement-protocol` | `vision:safe-agentic-coding` | a `self.improvement/1` case reaches its outcomes with every claim from a validator; part 1 (declined, unreproducible) in parallel with A1, part 2 (the merge validators, accepted) after A1's grant | yes |
| B1 | `story:scorecard-save-checks-binary` | `vision:safe-agentic-coding` | a save under a revision other than its runs' binary exits 1 naming the case | yes |
| B2 | `story:promotion-chain-head-anchored` | `vision:safe-agentic-coding` | a deleted or replaced latest promotion record is refused, naming it | yes |
| C | `story:check-finding-run-step-confined` | `vision:safe-agentic-coding` | the predicate writes nothing on the host, reads nothing of the caller's home, and no process left by one check changes the next | yes |
| carry-over | improvement turn 4: `story:step-guard-sees-test-script-imports` (granted at 1e2fdb3 in wave 13's time, off the critical path) | `vision:safe-agentic-coding` | a step cannot shadow the test runner the test script loads | yes |

## Selection path

The planner's wave 14 after the owner's word to bring the next release forward with self-improvement: lane A on the critical path (judge-guard, then the self-improvement protocol, its first part started beside it on files judge-guard does not hold), lane B (the scorecard save the protocol depends on, then the anchored chain), lane C (the confined check run that guards the protocol's evidence). The lanes branched from wave 13's tip 4e1aa08 while its close was reviewed; this branch opens from main at 5df99e9, after wave 13 and the CI fix for bare test repositories, and each lane merges it before its grant.

## Left out

Until after release 0.2.0, on the planner's resequencing: `story:every-yaml-read-refuses-aliases`, `story:rejudge-is-bounded`, `story:scorecard-binds-binary-digest`, `story:story-delivery-tests-first`, and the lows of `story:review-of-main-lows`.

## Pre-flight

| check | value |
|---|---|
| tree | `main` at 52b63f0 (wave 13): `task check` exit 0, 975 passed, citations 658 of 658; 5df99e9 adds the bare-repository CI fix, a CI-like run at its branch 975 passed, 0 failed |
| model budget | not stated; N = 5 |

## The commits this wave makes

One opening commit (this page), each unit's commits on its own branch, the merges of the units into this branch on their grants, a merge of main when it moves, the closing planning-store commit, and the merge into local main. No push, tag or release.

## Log

- 2026-10-19: wave opened from main at 5df99e9; lanes A1, A2, B1, B2 and C already running from 4e1aa08.
