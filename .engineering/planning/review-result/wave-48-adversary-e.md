---
format: aep.planning-md/3
id: review-result:wave-48-adversary-e
kind: review-result
status: active
title: Adversary, wave 48 unit e
relations:
- reviews: story:connectors-forge-at-a-served-instance
revision: 1
---
needs-revision, resolved

Adversary pass on unit e (a pull request behind its base brought up to date by takt's own merge commit) and the git 2.56 tracking-ref put-back, at 959bacb8: 4 cases in crates/takt/tests/w48_adv_e.rs, 2 red, and two mutants only this file catches. A base update whose push the forge did not take wedged the case, and a lagging forge made the pass final. Fixed in a1706c2a: a head carrying the revision with only takt's commits is accepted, and the same update is pushed again. 4 of 4 green.

```findings
[
{"file":"crates/takt/src/validator.rs","line":5500,"category":"concurrency","severity":"blocker","verdict":"NEEDS-CHANGE","origin":"introduced","message":"a base update named in branch.yaml whose push the forge did not take left a head neither pushed nor the revision, and every later pass stopped at nothing to merge; fixed: a head that carries the revision with only takt's riders and recorded updates is accepted, and the same update is pushed again"},
{"file":"crates/takt/src/validator.rs","line":4991,"category":"concurrency","severity":"warning","verdict":"NEEDS-CHANGE","origin":"introduced","message":"a pass observed again while the forge lags the update settled on nothing to merge, final at the revision; fixed with the same change: the pass pushes the same commit again and waits or merges"},
{"file":"crates/takt/src/validator.rs","line":4922,"category":"mutant","severity":"warning","verdict":"CONFIRMED","origin":"introduced","message":"removing the head-holds-the-judged-tip exit left w48_unit_e green; pinned by w48_adv_e"},
{"file":"crates/takt/src/forge/connectors.rs","line":775,"category":"mutant","severity":"warning","verdict":"CONFIRMED","origin":"introduced","message":"gitlab_behind had no case and a mutated count comparison stayed green; pinned by w48_adv_e"},
{"file":"crates/takt/src/forge/connectors.rs","line":1508,"category":"contract-drift","severity":"note","verdict":"CONFIRMED","origin":"introduced","message":"merge_request.get does not ask for include_diverged_commits_count, which the pinned GitLab source does not declare; the docs and ESS now say need_rebase is what triggers an update on gitlab"},
{"file":"crates/takt/src/validator.rs","line":5676,"category":"judgement","severity":"note","verdict":"CONFIRMED","origin":"introduced","message":"the off-base guard runs after update_base, so an off-base head gets one more update before refusal; kept: run first it makes the conflict decline unreachable for a head over a rewritten base, and the extra update is capped by the record and never merged"},
{"file":"crates/takt/src/case.rs","line":2340,"category":"judgement","severity":"note","verdict":"INFEASIBLE","origin":"introduced","message":"the rider rule takes any second parent above the pinned base, not one on the base; reaching it needs a case-directory write the step guard puts back, and the merge-time guard refuses it"},
{"file":"crates/takt/src/forge.rs","line":1572,"category":"judgement","severity":"note","verdict":"INFEASIBLE","origin":"introduced","message":"the put-back reverts every refs/remotes change made during the push, not only the push's own; nothing fetches in the same checkout during a push"}
]
```
