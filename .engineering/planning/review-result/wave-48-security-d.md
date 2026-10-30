---
format: aep.planning-md/3
id: review-result:wave-48-security-d
kind: review-result
status: active
title: Security review, wave 48 unit d
relations:
- reviews: story:connectors-forge-at-a-served-instance
revision: 1
---
needs-revision, resolved

Security review of unit d, at 1a4cda17: 10 cases in crates/takt/tests/w48_sec_d.rs, 5 red. A gitlab merge request read locked sent a second merge, an open pull request read merged true was taken as open, and a merged read at another head counted as applied. Fixed in fa12f45b. Another case's well-formed key cannot be told apart, so only the key form is checked, and that case was reworded. 10 of 10 green.

```findings
[
{"file":"crates/takt/src/forge/connectors.rs","line":1438,"category":"acceptance","severity":"warning","verdict":"NEEDS-CHANGE","origin":"introduced","message":"a gitlab merge request read locked after outcome_unknown counted as open, so a second merge went under a fresh key while the first may still be in progress; fixed: locked is not settled, nothing is sent and a later run reads it again"},
{"file":"crates/takt/src/forge/connectors.rs","line":1479,"category":"integrity","severity":"note","verdict":"INFEASIBLE","origin":"introduced","message":"a github read of state open with merged true counted as open; fixed: that answer is refused as malformed"},
{"file":"crates/takt/src/forge/connectors.rs","line":1360,"category":"acceptance","severity":"warning","verdict":"INFEASIBLE","origin":"introduced","message":"a read-back of merged was taken as this merge applied without checking the merged head; fixed: merged at another head is no merge, named"},
{"file":"crates/takt/src/forge/connectors.rs","line":283,"category":"integrity","severity":"note","verdict":"INFEASIBLE","origin":"introduced","message":"the unknown member admitted any string; a well-formed key of another case cannot be told apart and changes nothing sent, so the form alone is now checked (takt- and 64 lowercase hex digits)"}
]
```
