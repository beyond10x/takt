---
format: aep.planning-md/3
id: review-result:adversary-scan-adapters-pass-1
kind: review-result
status: active
title: Adversary pass 1 on the scan and forge adapters
relations:
- reviews: story:scan-and-forge-are-adapters
revision: 1
---
# Adversary pass 1 on the scan and forge adapters

Tree: `story/scan-forge-adapters` at 5d94795 plus the uncommitted unit. Test files added:
`crates/takt/tests/scan_adapter_adv1.rs` (1 red), `crates/takt-cli/tests/scan_adapter_adv1.rs`
(2 red), `crates/takt/tests/gates_is_one_adapter_adv1.rs` (2 green, a stronger guard of
Acceptance 6). Suite after the cases: 1830 passed, 8 failed (these 3, the security reviewer's 2,
and 3 in `self_improvement_cli` that fail at the base in this tree), 5 ignored.

| # | file:line | verdict | origin | finding | what was done |
|---|---|---|---|---|---|
| 1 | crates/takt/src/scan.rs:21 | warning | introduced | a one-key TOML table `scan = { none = {} }` was read as `none` | fixed: `ScanName` is read from a string only; both cases green |
| 2 | website/docs/reference/configuration.md:28 | warning | introduced | the reference lacked `scan` and quoted the old refusal; the Design's doc pages were not done | fixed: the docs branch merged into the unit; the case is green |
| 3 | crates/takt/tests/gates_is_one_adapter.rs:49 | note | introduced | the literal rule misses `concat!` and continued literals and exempts every CLI `///` line | kept: `gates_is_one_adapter_adv1.rs` catches the first two |
| 4 | crates/takt/src/run.rs:10544 | note | introduced | `opened` records `gates` for a run in which nothing was scanned | fixed in the record's contract: `RunOpened.scan` names the run's adapter, not a scan that ran |
| 5 | crates/takt/src/harness.rs:517 | note | introduced | scope and context files are scanned under the environment's policy, not the run's `Scan`; the CLI keeps the two equal | kept: no CLI path splits them |
| 6 | crates/takt/src/scan/gates.rs:24 | note | introduced | an empty `B10X_GATES_POLICY` counts as set, as documented | kept |

```findings
[
{"file":"crates/takt/src/scan.rs","line":21,"category":"acceptance","severity":"warning","verdict":"NEEDS-CHANGE","origin":"introduced","message":"ScanName deserialized a unit variant from a one-key TOML table, so `scan = { none = {} }` opened a codex run unscanned instead of being refused as Acceptance 4 requires"},
{"file":"website/docs/reference/configuration.md","line":28,"category":"contract-drift","severity":"warning","verdict":"NEEDS-CHANGE","origin":"introduced","message":"the configuration reference lacked the `scan` field and quoted a refusal the binary no longer prints, and the Design's doc pages were not updated"},
{"file":"crates/takt/tests/gates_is_one_adapter.rs","line":49,"category":"mutant","severity":"note","verdict":"CONFIRMED","origin":"introduced","message":"the Acceptance-6 rule misses concat!-built and backslash-continued literals and exempts every CLI `///` line, so three mutants survive"},
{"file":"crates/takt/src/run.rs","line":10544,"category":"judgement","severity":"note","verdict":"CONFIRMED","origin":"introduced","message":"the opened line records scan gates for a run with no policy in which nothing was scanned, which reads as a scan that ran"},
{"file":"crates/takt/src/harness.rs","line":517,"category":"contract-drift","severity":"note","verdict":"INFEASIBLE","origin":"introduced","message":"scope and context files are scanned under the environment's policy rather than the run's Scan, contrary to the ESS seam text; the CLI keeps them equal, library callers can split them"},
{"file":"crates/takt/src/scan/gates.rs","line":24,"category":"boundary","severity":"note","verdict":"INFEASIBLE","origin":"introduced","message":"an empty B10X_GATES_POLICY counts as a policy, so a codex run opens its case and is refused only at its step, and scan none with it set empty is refused"}
]
```
