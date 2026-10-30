---
format: aep.planning-md/3
id: review-result:wave-48-adversary-b
kind: review-result
status: active
title: Adversary, wave 48 unit b
relations:
- reviews: story:connectors-forge-at-a-served-instance
revision: 1
---
needs-revision, resolved

Adversary pass on unit b (keyed served writes, 5791d8fe): 5 cases in crates/takt/tests/w48_adv_b.rs, 4 red. Fixed in f1b329cb: the refusal count is kept in the case directory, the key covers the descriptor revision, and a new key is taken only on the instance's refused or not_attempted classification. 5 of 5 green; merged into wave/48 at 1109b070.

```findings
[
{"file":"crates/takt/src/forge/connectors.rs","line":537,"category":"concurrency","severity":"blocker","verdict":"NEEDS-CHANGE","origin":"introduced","message":"the refusal count lives only in the process, so every resumed run re-sends the first key and the host replays a stored merge refusal forever"},
{"file":"crates/takt/src/forge/connectors.rs","line":173,"category":"property","severity":"warning","verdict":"CONFIRMED","origin":"introduced","message":"the key omits the descriptor revision the host compares, so a refused write meets idempotency_conflict on every attempt after the revision moves"},
{"file":"crates/takt/src/forge/connectors.rs","line":958,"category":"boundary","severity":"warning","verdict":"INFEASIBLE","origin":"introduced","message":"any provider status outside 2xx takes a new key regardless of the host's classification, so an applied create answered 502 is applied again"}
]
```
