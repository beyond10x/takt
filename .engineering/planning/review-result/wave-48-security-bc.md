---
format: aep.planning-md/3
id: review-result:wave-48-security-bc
kind: review-result
status: active
title: Security review, wave 48 units b1 and c1
relations:
- reviews: story:git-through-the-connectors-git-relay
revision: 1
---
needs-revision, resolved

Security review of units b1 and c1, at e9f209d1: invariants on non-loopback origins, credentials on relay lines, the CI archive checks and non-vacuous served tests hold. 7 cases in crates/takt/tests/w48_sec_bc.rs, 2 red: the mapping applied on gitlab, and percent-encoded relay segments. Fixed in fa12f45b; 7 of 7 green.

```findings
[
{"file":"crates/takt/src/forge/connectors.rs","line":783,"category":"contract-drift","severity":"warning","verdict":"INFEASIBLE","origin":"introduced","message":"the git/ relay mapping applied to the gitlab connector too, so a gitlab project git/<a>/<b> behind a loopback relay was named <a>/<b>; fixed: github only"},
{"file":"crates/takt/src/forge/connectors.rs","line":463,"category":"boundary","severity":"note","verdict":"INFEASIBLE","origin":"introduced","message":"relay_project admitted percent-encoded segments, which the relay decodes and the API calls would not; fixed: a relay origin with a percent-encoded part is refused before the case opens"}
]
```
