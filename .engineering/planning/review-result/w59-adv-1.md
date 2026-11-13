---
format: aep.planning-md/3
id: review-result:w59-adv-1
kind: review-result
status: active
title: 'Wave 59 adversarial pass at ed55ebdf: four findings, all fixed'
tags:
- scheduled-runs
relations:
- reviews: story:a-connector-read-action
revision: 1
---
# Wave 59 adversarial pass at ed55ebdf: four findings, all fixed

```findings
[
{"file":"crates/takt/src/connector_action.rs","line":426,"category":"acceptance","severity":"blocker","verdict":"CONFIRMED","origin":"introduced","message":"every non-success answer, including stale_description, a gateway 502 page and an unfollowed 307, was recorded as fail and made the outcome refused, though the story and design record fail only for the instance's refusal (w59_adv_stale_description_is_no_refusal_of_the_read, w59_adv_gateway_page_is_no_refusal_of_the_read, w59_adv_redirect_is_no_refusal_of_the_read)"},
{"file":"crates/takt/src/connector_action.rs","line":419,"category":"boundary","severity":"warning","verdict":"CONFIRMED","origin":"introduced","message":"the invoke was held to the client's fixed 120 s and not to what is left of the run's hours, unlike wave 58's command action (w59_adv_call_in_flight_is_held_to_the_hour_budget)"},
{"file":"crates/takt-cli/src/main.rs","line":2668,"category":"judgement","severity":"warning","verdict":"CONFIRMED","origin":"introduced","message":"an action bound in both command_actions and connector_actions was not refused, and the run silently performed the command binding (w59_adv_an_action_bound_twice_is_refused)"},
{"file":"crates/takt/src/connector_action.rs","line":457,"category":"boundary","severity":"note","verdict":"INFEASIBLE","origin":"introduced","message":"the kept answer was pretty-printed, so a 200 KB nested answer was written as 20.5 MB, past the 16 MiB read bound (w59_adv_kept_answer_is_bounded_like_the_answer_read)"}
]
```
