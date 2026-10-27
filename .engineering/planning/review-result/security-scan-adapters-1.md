---
format: aep.planning-md/3
id: review-result:security-scan-adapters-1
kind: review-result
status: active
title: Security review 1 of the scan and forge adapters
relations:
- reviews: story:scan-and-forge-are-adapters
revision: 1
---
# Security review 1 of the scan and forge adapters

Tree: `story/scan-forge-adapters` at 5d94795 plus the uncommitted unit. Verdict: confirmed, the
six invariants of the story hold but for the two findings below. Test file added:
`crates/takt/tests/proposal_path_sec1.rs` (2 red before the fix, 2 green after).

| # | file:line | verdict | origin | finding | what was done |
|---|---|---|---|---|---|
| 1 | crates/takt/src/run.rs:8354 | warning | undecided | the in-process credential-shape check did not read the paths a proposal names | fixed: a path is checked first, before any refusal that names it, and scanned with the rest under `gates` (CHANGELOG, Security) |
| 2 | ess/domains/takt.yaml:1125 | note | introduced | `RunOpened.scan` read as the adapter the proposals passed | fixed: it names the run's adapter, not a scan that ran |

```findings
[
{"file":"crates/takt/src/run.rs","line":8354,"category":"integrity","severity":"warning","verdict":"CONFIRMED","origin":"undecided","message":"the in-process credential-shape check did not read the paths a proposal names"},
{"file":"ess/domains/takt.yaml","line":1125,"category":"contract-drift","severity":"note","verdict":"CONFIRMED","origin":"introduced","message":"RunOpened.scan was documented as the adapter the proposals passed, yet a claude run with no policy records gates while no gates scan ran"}
]
```
