---
format: aep.planning-md/3
id: review-result:w58-adv-1
kind: review-result
status: active
title: 'Wave 58 adversarial pass at e05f05e4: four findings and a note, all fixed in a81a3d3e'
tags:
- scheduled-runs
relations:
- reviews: story:a-command-action
revision: 1
---
# Wave 58 adversarial pass at e05f05e4: four findings and a note, all fixed in a81a3d3e

```findings
[
{"file":"crates/takt/src/command_action.rs","line":145,"category":"acceptance","severity":"blocker","verdict":"CONFIRMED","origin":"introduced","message":"a binding's evidence kind is never checked against its action's may_produce, so a command action can record test_result pass and complete a case on tests.pass with the test command false (a_binding_writes_no_record_of_a_kind_its_action_may_not_produce); fixed in a81a3d3e"},
{"file":"crates/takt/src/command_action.rs","line":259,"category":"boundary","severity":"warning","verdict":"CONFIRMED","origin":"introduced","message":"a declared output the child leaves as a link pointing outside artifacts/ is kept and recorded pass (the_declared_output_left_as_a_link_out_of_artifacts_is_not_kept); fixed in a81a3d3e"},
{"file":"crates/takt/src/command_action.rs","line":233,"category":"contract-drift","severity":"warning","verdict":"CONFIRMED","origin":"introduced","message":"CommandAction runs a child but keeps the default observe_within, so the run's --hours deadline does not hold it (a_command_action_child_is_held_to_the_runs_hour_budget); fixed in a81a3d3e"},
{"file":"website/docs/reference/configuration.md","line":79,"category":"contract-drift","severity":"note","verdict":"CONFIRMED","origin":"introduced","message":"the reference says those eleven after adding command_actions as the twelfth field without a flag (the_reference_counts_the_fields_without_a_flag_as_it_lists_them); fixed in a81a3d3e"},
{"file":"crates/takt-cli/src/main.rs","line":2648,"category":"judgement","severity":"note","verdict":"INFEASIBLE","origin":"introduced","message":"takt scenario run checks the shape of command_actions and applies none, and the reference's scenario sentence does not list it; the reference sentence now lists command_actions, a81a3d3e"}
]
```
