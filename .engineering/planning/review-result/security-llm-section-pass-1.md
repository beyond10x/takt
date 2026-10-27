---
format: aep.planning-md/3
id: review-result:security-llm-section-pass-1
kind: review-result
status: active
title: 'Security review pass 1 on the llm section unit: NEEDS-CHANGE, fixed at c50e8d1'
relations:
- decides: story:llm-section-is-an-llm-routes-document
revision: 1
---
Opus security review on the llm section unit (unit 34a7a75, reviewed at e43b251; fix at c50e8d1).

verdict: NEEDS-CHANGE at e43b251; fixed at c50e8d1.

Invariants:
1 path beside the run file: BROKEN at e43b251 (symlink escape), fixed at c50e8d1.
2 no secret in opened record or events: HOLDS.
3 literal secret refused: HOLDS (inline form tested).
4 no credential file read by takt: HOLDS; the only new read is the llm document, now checked first.
5 refusals never echo a file value: HOLDS for secrets; the echoed path value is a note, unchanged.
6 no new network, shell-out or spawn: HOLDS.
7 bounds on the path form: BROKEN (unbounded read), fixed at c50e8d1 with a 2 MiB limit and a regular-file check.

Findings: symlink escape (fixed); directory and size read before checks (fixed); absolute path untested (test added at c50e8d1); takt parses inline sections with its own parser before llm (note, unchanged).
