---
format: aep.planning-md/3
id: review-result:adversary-riders-pass-1
kind: review-result
status: active
title: Adversary, records-reach-base, pass 1
relations:
- reviews: story:records-reach-base
revision: 1
---
# Adversary pass 1: story:records-reach-base

Unit at d0602ac (impl/riders, from 8b0afc5). Verdict: NEEDS-CHANGE. Cases: 361 → 365 in the
b10x-takt lane, 4 red, in `crates/takt/tests/records_reach_base_adv1.rs` (unedited by the
implementor).

Attacked without a break: rider shapes (symlinks, executable bit, mode 120000, renames, merge
commits, a second file, edits to another file, a second test_result or move, a move not last); a
harness planting kept files (the cases directory is snapshotted and put back around judgement and
validator steps); grant counting (first decision per capability, as grants_at); the relaxed push;
resume after accepted; AEP_ACTOR on the approval, the test_result and the move.

```findings
[
  {
    "file": "crates/takt/src/run.rs",
    "category": "correctness",
    "severity": "warning",
    "message": "records written at an earlier revision reach the base under a later grant: records written before a merge that waits at R1; operator commits a fix, grant at R2; done() matches the old records, so the base merged at R2 holds a test_result about R1",
    "line": 937
  },
  {
    "file": "crates/takt/src/case.rs",
    "category": "correctness",
    "severity": "warning",
    "message": "a move to implemented rides without a test_result rider below it: head = approval + move commit with the kept story bytes and no test_result commit rides, so the base gets an implemented story with no test_result",
    "line": 1050
  },
  {
    "file": "crates/takt/src/case.rs",
    "category": "correctness",
    "severity": "note",
    "message": "the test_result rider is compared by bytes only, not by its path: the kept test_result bytes added under a different file name in the evidence directory ride",
    "line": 1066
  },
  {
    "file": "crates/takt/src/case.rs",
    "category": "correctness",
    "severity": "note",
    "message": "approver system or service:<name> accepted despite the human/agent-only contract: takt approve --by system succeeds and the store records the operator's approval as actor system",
    "line": 670
  },
  {
    "file": "ess/domains/takt.yaml",
    "category": "correctness",
    "severity": "note",
    "message": "ESS pr-open prose still names only the approval's own commits as riders: the documented push condition omits the test_result and move riders the code now admits; the citation check stays green",
    "line": 1453
  }
]
```
