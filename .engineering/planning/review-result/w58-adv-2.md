---
format: aep.planning-md/3
id: review-result:w58-adv-2
kind: review-result
status: active
title: 'Wave 58 reviewer''s confirm at d326c51d: one finding, fixed'
tags:
- scheduled-runs
relations:
- reviews: story:a-command-action
revision: 1
---
# Wave 58 reviewer's confirm at d326c51d: one finding, fixed

```findings
[
{"file":"crates/takt/src/validator.rs","line":8413,"category":"acceptance","severity":"blocker","verdict":"CONFIRMED","origin":"introduced","message":"the evidence-kind check runs only in Run::open, so a case resumed with --case and a binding of another action's kind records test_result pass and completes on tests.pass with the test command false (a_resumed_case_records_no_kind_its_action_may_not_produce); fixed by running the bound validators' open checks on resume and by CommandAction checking the kind against the case's pinned protocol at each observation"}
]
```
