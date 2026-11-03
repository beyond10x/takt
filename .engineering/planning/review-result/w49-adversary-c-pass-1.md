---
format: aep.planning-md/3
id: review-result:w49-adversary-c-pass-1
kind: review-result
status: active
title: 'Wave 49 adversary pass 1 on unit c: the token rule'
relations:
- reviews: story:the-token-rule-holds-for-an-unchanged-binary
revision: 1
---
# Adversary pass 1 on unit c (wave 49)

```
unit: c, story:the-token-rule-holds-for-an-unchanged-binary (tree rv/49-adv-c at dc3040c64, working tree)
verdict: red
cases: executed 5→10, red 3
origin: introduced 7, pre-existing 0, undecided 0
wrote-outside-worktree: none
needs-coordinator: yes
```

New test file `crates/takt/tests/w49_adv_token_rule.rs`; no implementation file touched.

Red, each first run alone (`cargo test -p b10x-takt --test w49_adv_token_rule <name>`):

- `requests_in_counts_a_codex_rollouts_model_responses` panicked at :109: "a Codex step of two model responses records its requests, else every comparison over its case is refused"
- `requests_in_counts_a_b10x_harness_records_model_responses` panicked at :130, same message for b10x-harness.
- `exactly_five_percent_over_a_fractional_saved_median_holds`: `left: 1 right: 0`, printing "median tokens per request 350.0 exceeds the saved median 333.3 by more than 5 %" for 1050/3 against 1000/3.
- Green: `requests_in_counts_claude_codes_messages_once_each` (control) and `most_cases_one_request_longer_at_the_same_cost_hold` (catches judging case totals instead of per request).

Suite: `--test token_rule`: `ok. 5 passed`; `--test w49_adv_token_rule`: `FAILED. 2 passed; 3 failed`.

Findings (what reaches each in brackets):

1. `harness.rs:1227` `requests_in` reads only Claude Code `assistant` records and metaharness `event:"usage"` records; the session executor passes the harness's raw output, so Codex rollouts and b10x-harness records give no requests and every comparison over such a case is refused. [scenario run plus `scorecard --against` comparing harnesses; none of 26 kept real Codex step transcripts matches either shape] NEEDS-CHANGE.
2. `scorecard.rs:965` rounds each figure down to thousandths before comparing, so exactly 5 % over is refused. INFEASIBLE.
3. No route left to re-record a baseline: `promotion.rs:2647` refuses `--baseline-only` once a store holds one; `--rebaseline` refuses the old runs' unknown requests (`:2983`); ESS `takt.yaml:2831-2833` says the refusal lasts until a baseline is recorded. [the live promotion store's latest scorecard has no `requests`, so the next `takt promote` exits 1] NEEDS-CHANGE.
4. Acceptance item 3: `design:recursive-self-improvement` still states the old rule (at the tree's base).
5. Acceptance item 1: `token_rule.rs` uses made-up figures, not the recorded 0.2.2 and 44f96a48 scorecards, which lack `requests`.
6. Out of scope: a candidate making more requests at the same or lower cost each passes with more total tokens; nothing judges totals or request counts now.
7. Note: `TOKEN_TOLERANCE_PERCENT` (`scorecard.rs:140`) is not a `Rule` field, so a saved scorecard does not carry the tolerance.

Attacked and not broken: division by 0 requests; overflow (u128/i128); `promote` and `scorecard --against` agree; repeated or interleaved Claude message ids dedupe; identical binary with +3.4 % drift and one more request holds (+2.4 % a request); pre-`requests` scorecards refused with a clear message; help text and ESS match the code.

```findings
[{"file":"crates/takt/src/harness.rs","line":1227,"category":"acceptance","severity":"warning","verdict":"NEEDS-CHANGE","origin":"introduced","message":"requests_in reads no requests from a Codex rollout or b10x-harness record, so every comparison over such scenario cases is refused (w49_adv_token_rule.rs::requests_in_counts_a_codex_rollouts_model_responses, ::requests_in_counts_a_b10x_harness_records_model_responses)"},
{"file":"crates/takt/src/scorecard.rs","line":965,"category":"boundary","severity":"note","verdict":"INFEASIBLE","origin":"introduced","message":"rounding down to thousandths refuses a candidate exactly 5 % over (w49_adv_token_rule.rs::exactly_five_percent_over_a_fractional_saved_median_holds)"},
{"file":"crates/takt/src/promotion.rs","line":2647,"category":"contract-drift","severity":"warning","verdict":"NEEDS-CHANGE","origin":"introduced","message":"a store whose baseline predates requests cannot get a new baseline: --baseline-only refuses and --rebaseline refuses the unknown requests, contrary to ESS takt.yaml:2831-2833"},
{"file":"design:recursive-self-improvement","category":"acceptance","severity":"warning","verdict":"NEEDS-CHANGE","origin":"introduced","message":"the design still states the median-over-saved-maximum rule and its token open question is not closed"},
{"file":"crates/takt/tests/token_rule.rs","line":1,"category":"acceptance","severity":"warning","verdict":"NEEDS-CHANGE","origin":"introduced","message":"acceptance asks for a test over the recorded 0.2.2 and 44f96a48 scorecards; these figures are made up, and the recordings lack requests so the rule refuses them"},
{"file":"crates/takt/src/scorecard.rs","line":1790,"category":"out-of-scope","severity":"note","verdict":"CONFIRMED","origin":"introduced","message":"a candidate making more requests at the same or lower cost each passes with more total tokens; nothing judges totals or request counts now"},
{"file":"crates/takt/src/scorecard.rs","line":140,"category":"judgement","severity":"note","verdict":"CONFIRMED","origin":"introduced","message":"the tolerance is a constant, not a Rule field, so a saved scorecard does not record it and a tolerance change goes unseen (w49_adv_token_rule.rs::most_cases_one_request_longer_at_the_same_cost_hold pins per-request over totals)"}]
```
