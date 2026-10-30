---
format: aep.planning-md/3
id: review-result:wave-48-security-e
kind: review-result
status: active
title: Security review, wave 48 unit e
relations:
- reviews: story:connectors-forge-at-a-served-instance
revision: 1
---
needs-revision, resolved

Security review of unit e and the tracking-ref put-back, at 959bacb8: invariants on the update's content, the scan, the conflict path, the put-back and error text hold. 1 case in crates/takt/tests/w48_sec_e.rs, red: a local-forge check run at the merge rewrote the record of base updates. Fixed in a1706c2a: branch.yaml and the record are kept across the checks, put back and the merge refused on a change. 1 of 1 green.

```findings
[
{"file":"crates/takt/src/validator.rs","line":5411,"category":"integrity","severity":"warning","verdict":"NEEDS-CHANGE","origin":"introduced","message":"repository.merge's writes() admitted any change to artifacts/base-update.json during the step, so a local-forge check run at the merge rewrote the record and made approvals_only take a forged merge; the merge-time guard still kept it from the merge. Fixed: branch.yaml and the record are read before the checks, and a change is put back and refuses the merge"},
{"file":"crates/takt/src/validator.rs","line":4929,"category":"judgement","severity":"note","verdict":"CONFIRMED","origin":"introduced","message":"the conflict decline said nothing was pushed and the branch was as it was, though riders may have been pushed earlier in the pass; reworded: no merge commit made, recorded or pushed, and nothing merged"}
]
```
