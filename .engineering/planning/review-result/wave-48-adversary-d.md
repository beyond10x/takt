---
format: aep.planning-md/3
id: review-result:wave-48-adversary-d
kind: review-result
status: active
title: Adversary, wave 48 unit d
relations:
- reviews: story:connectors-forge-at-a-served-instance
revision: 1
---
needs-revision, resolved

Adversary pass on unit d (the outcome_unknown merge read-back), at 1a4cda17: 11 cases in crates/takt/tests/w48_adv_d.rs, 4 red, and one mutant (the closed check) only this file catches. Fixed in fa12f45b: merged at another head is no merge, and the refused count no longer saturates. Two cases no writer or caller reaches stay ignored with their reason; 9 green.

```findings
[
{"file":"crates/takt/src/forge/connectors.rs","line":1369,"category":"mutant","severity":"warning","verdict":"CONFIRMED","origin":"introduced","message":"replacing the Closed bail in settle_unknown with a fall-through left w48_unit_d green; w48_adv_d's closed case, now in the suite, kills it"},
{"file":"crates/takt/src/forge/connectors.rs","line":1360,"category":"judgement","severity":"note","verdict":"INFEASIBLE","origin":"introduced","message":"a pull request read merged counted as this merge applied without comparing its head; fixed: merged at another head is no merge, named"},
{"file":"crates/takt/src/forge/connectors.rs","line":1165,"category":"boundary","severity":"note","verdict":"INFEASIBLE","origin":"introduced","message":"at a refused count of u32::MAX the fresh key equalled the key answered outcome_unknown; fixed: the count is checked and nothing is sent"},
{"file":"crates/takt/src/forge/connectors.rs","line":1160,"category":"boundary","severity":"note","verdict":"INFEASIBLE","origin":"introduced","message":"the unknown entry is found only under the key the refused count gives, so a record whose count lags its chain resends the first key; no writer makes such a record, the case is kept ignored as a known limit"},
{"file":"crates/takt/src/forge/connectors.rs","line":1324,"category":"acceptance","severity":"note","verdict":"INFEASIBLE","origin":"undecided","message":"within one process read_again refuses a pull request read merged after an unknown merge before settle_unknown can call it applied; no caller merges again in the process, the case is kept ignored and a resume settles it"},
{"file":"crates/takt/src/forge/connectors.rs","line":1438,"category":"judgement","severity":"note","verdict":"INFEASIBLE","origin":"introduced","message":"a gitlab merge request read locked (a merge in progress) counted as open; fixed with the security review's finding"}
]
```
