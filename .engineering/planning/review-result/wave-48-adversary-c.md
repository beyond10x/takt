---
format: aep.planning-md/3
id: review-result:wave-48-adversary-c
kind: review-result
status: active
title: Adversary, wave 48 unit c
relations:
- reviews: story:git-through-the-connectors-git-relay
revision: 1
---
needs-revision, resolved

Adversary pass on unit c (git relay refusals, 778cb31e): 10 cases in crates/takt-cli/tests/w48_adv_c.rs, 5 red, all older than the unit. Fixed on rv/48-adv-c: the trial path reaches the relay through the userinfo-free URL, control characters in a reason are escaped, and a trial refusal names the whole reason. 10 of 10 green; merged into wave/48 at a9e440f7.

```findings
[
{"file":"crates/takt-cli/src/main.rs","line":3050,"category":"acceptance","severity":"warning","verdict":"CONFIRMED","origin":"pre-existing","message":"the trial fetch and push name origin, so a token in its userinfo is sent as Basic auth to a relay that answers 401, which takt run avoids by stripping userinfo"},
{"file":"crates/takt-cli/src/main.rs","line":4485,"category":"boundary","severity":"note","verdict":"INFEASIBLE","origin":"pre-existing","message":"one_line passes ESC and BEL from the relay's reason into the run's stop line"},
{"file":"crates/takt-cli/src/main.rs","line":3146,"category":"boundary","severity":"note","verdict":"INFEASIBLE","origin":"pre-existing","message":"the trial refusal line passes ESC and BEL from the relay's reason through to stderr"},
{"file":"crates/takt-cli/src/main.rs","line":3115,"category":"boundary","severity":"note","verdict":"INFEASIBLE","origin":"pre-existing","message":"the trial refusal keeps only the first tab-free line of the porcelain summary, so a reason with a tab or newline is truncated"},
{"file":"crates/takt-cli/tests/git_relay_refusals.rs","line":1,"category":"judgement","severity":"note","verdict":"CONFIRMED","origin":"pre-existing","message":"the AC4 no-Authorization assertions cannot fail because the stand-in never answers 401"}
]
```
