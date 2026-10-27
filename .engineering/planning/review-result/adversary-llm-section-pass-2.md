---
format: aep.planning-md/3
id: review-result:adversary-llm-section-pass-2
kind: review-result
status: active
title: 'Adversary pass 2 on the llm section unit: APPROVE, fixes landed at c50e8d1'
relations:
- decides: story:llm-section-is-an-llm-routes-document
- reviews: story:llm-section-is-an-llm-routes-document
revision: 1
---
Opus adversary on the llm section unit (unit 34a7a75, reviewed at e43b251; fix at c50e8d1).

verdict: APPROVE, no blocker. cases: executed 20, red 4. origin: introduced 6, pre-existing 0.

- c2b colliding inline keys (1 and "1"): not reachable by a writer; known limit, no change.
- c3 symlink beside the run file reads a file outside the directory: fixed at c50e8d1 (canonical containment).
- c10 inline alias expansion bypasses llm alias budget, bounded by serde and the 2 MiB limit: known limit, no change.
- c11 named pipe hangs the run: fixed at c50e8d1 (regular-file check before read).
- llm_digest doc sits on struct Line, and ESS says the run ran under it: note, no change in this round.
- TOML inline table had no test: added llm_toml_run_file_accepted at c50e8d1.
