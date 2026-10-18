---
format: aep.planning-md/3
id: review-result:adversary-riders-pass-2
kind: review-result
status: active
title: Adversary, records-reach-base, pass 2
relations:
- reviews: story:records-reach-base
revision: 1
---
# Adversary pass 2: story:records-reach-base

Unit at 98813e3 (impl/riders; unit d0602ac, correction 1 98813e3). Verdict: NEEDS-CHANGE, 3 red cases
in `crates/takt/tests/records_reach_base_adv2.rs`; 365 → 368 in the b10x-takt lane. No rider bypass.

Attacked without a break: stale or foreign commits against the per-revision done rule; resume after
a revision move; planting `riders/<rev>/`; path matching; the move-after-test_result rule; approver
strings (`human:` alone, empty, whitespace, unicode, `Human:x`, `human:a:b`); the relaxed push; grant
counting; AEP_ACTOR; a refused test_result followed by a move.

```findings
[
  {
    "file": "crates/takt/src/run.rs",
    "category": "correctness",
    "severity": "warning",
    "message": "a move made below a later revision is tried again on every run: revision moves R1->R2 with the R1 move below R2; the store refuses the move at R2, nothing is kept, and every takt run --case of the accepted case records a store refusal again",
    "line": 1015
  },
  {
    "file": "crates/takt/src/run.rs",
    "category": "correctness",
    "severity": "warning",
    "message": "a refused test_result lets the merge take the base without the records: the store refuses the test_result before the merge (constructed); the move is refused, the merge proceeds, the base holds the story active with no test_result, and the records land on the branch after the merge",
    "line": 1313
  },
  {
    "file": "crates/takt/src/case.rs",
    "category": "correctness",
    "severity": "note",
    "message": "takt's own actor is accepted as the approver of a story merge: takt approve --by agent:takt succeeds; the store records the grant as takt approving its own merge",
    "line": 458
  },
  {
    "file": "ess/domains/takt.yaml",
    "category": "correctness",
    "severity": "note",
    "message": "ESS pr-merged prose omits the move-needs-test_result rule and the relaxed push condition: the documented merge does not say the move rides only above the test_result or that the branch head is pushed when it descends from the pull request head",
    "line": 1466
  }
]
```
