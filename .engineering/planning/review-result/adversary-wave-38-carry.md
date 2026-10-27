---
format: aep.planning-md/3
id: review-result:adversary-wave-38-carry
kind: review-result
status: active
title: Adversary pass on the wave-38 carry merged onto integration/takt
summary: 'red: 6 cases, 9 introduced findings, docs and ESS behind the carried code, a budget order at the last try'
relations:
- reviews: story:a-harness-step-without-an-action-is-retried
- reviews: story:run-configuration-names-the-story-and-token-variable
- reviews: story:run-state-directory-is-configurable
- reviews: story:scenario-run-takes-config
- reviews: story:edits-keep-ess-citations-resolved
- reviews: story:fresh-home-first-test-run-is-not-refused
- reviews: story:staging-tests-pass-under-a-shared-temp-directory
revision: 1
---
# Adversary pass on the wave-38 carry, merged onto integration/takt

Tree: `carry/wave-38-on-integration` at 02e8e366 (the merge 69b406e7 of 2db3e1d onto 490764cc,
the citation re-point bd60489f and 12 store files). Cases added:
`crates/takt/tests/config_docs_adv_carry.rs` (4 red), `crates/takt-cli/tests/config_docs_adv_carry.rs`
(1 red), `crates/takt/tests/no_action_budget_adv_carry.rs` (1 red). Executed 1875 -> 1881, red 6;
`cargo test -p b10x-takt --no-fail-fast` 1306 passed, 5 failed, 4 ignored; `cargo test -p
b10x-takt-cli --no-fail-fast` 569 passed, 1 failed, 1 ignored; every failure is one of the six.

Verdict: red. Origin: introduced 9, pre-existing 0, undecided 0.

| id | file:line | severity | measured |
|---|---|---|---|
| F1 | website/docs/reference/configuration.md:29 | warning | the field table lacks `story`, `endpoint_token_env`, `state_dir`; no flag table names `--state-dir` |
| F2 | website/docs/reference/configuration.md:40 | warning | says `--story` and `--endpoint-*` have no key and are refused as one |
| F3 | website/docs/reference/configuration.md:116 | note | quotes the pre-carry refusal text |
| F4 | website/docs/guides/configure-a-run.md:40 | warning | "eight keys", the new keys and `scenario run --config` called planned, the old refusal quoted |
| F5 | website/docs/guides/configure-a-run.md:145 | warning | the unknown-key example (`story`) is now a key takt reads |
| F6 | website/docs/known-limitations.md:21 | warning | rows 21, 22, 24, 25, 26, 27, 30 list the seven carried behaviours as open limitations |
| F7 | website/docs/guides/scenario-run.md:45 | note | `scenario run --config` called planned; how-a-run-ends.md:125 omits the retry; deploy-on-kubernetes.md:130/252 keep the `.cargo/bin` workaround (obsolescence inferred) |
| F8 | ess/domains/takt.yaml:3881 | note | the ESS has no word of the no-action retry |
| F9 | crates/takt/src/run.rs:696 | note | the try limit is checked before the budget, so a third try that spends the budget ends exit 5, not the exit 3 four places promise |

Not broken: the merge resolution (`REPEATED_FAILURES = 2` kept, no base hunk dropped), U2 with U4
(a retried step's prompt carries the failed gate lines), U4 with the repeated-failure stop, U5's
`state_dir` through `scenario run`, U3 with U1 in a fresh HOME on a red base.

Routing: every finding is introduced by the carry and is fixed on `integration/takt` as fix
commits; F9 is fixed in the code (the budget is asked before the try limit), as the four promises
read.

```findings
[
{"file":"website/docs/reference/configuration.md","line":29,"category":"contract-drift","severity":"warning","verdict":"CONFIRMED","origin":"introduced","message":"the field table lists 8 keys; the merged RunConfig also takes story, endpoint_token_env and state_dir (AGENTS.md: the table moves with run_config.rs), and no flag table names --state-dir; red: the_reference_field_table_lists_every_key_the_file_takes, every_takt_run_flag_is_on_the_configuration_reference"},
{"file":"website/docs/reference/configuration.md","line":40,"category":"contract-drift","severity":"warning","verdict":"CONFIRMED","origin":"introduced","message":"says --story and the --endpoint-* flags have no key and a key for one is refused; story and endpoint_token_env are read; red: a_flag_the_reference_says_has_no_key_is_refused_as_one"},
{"file":"website/docs/reference/configuration.md","line":116,"category":"contract-drift","severity":"note","verdict":"CONFIRMED","origin":"introduced","message":"quotes the pre-carry SHAPE refusal; takt now prints the 11-key text; red: the_refusal_each_page_quotes_is_the_one_takt_prints"},
{"file":"website/docs/guides/configure-a-run.md","line":40,"category":"contract-drift","severity":"warning","verdict":"CONFIRMED","origin":"introduced","message":"says the file holds eight keys, lines 44-47 call story/endpoint_token_env keys and scenario run --config planned, line 162 says the loom.agent/1 member holds the same eight keys, line 153 quotes the old refusal; all shipped by the carry"},
{"file":"website/docs/guides/configure-a-run.md","line":145,"category":"contract-drift","severity":"warning","verdict":"CONFIRMED","origin":"introduced","message":"the unknown-key example (`story` on line 4, refused at line 4) is now a valid key that takt reads; red: the_guides_unknown_key_example_is_refused_at_its_line"},
{"file":"website/docs/known-limitations.md","line":21,"category":"contract-drift","severity":"warning","verdict":"CONFIRMED","origin":"introduced","message":"rows 21, 22, 24, 25, 26, 27 and 30 list as open limitations (with workarounds and planned fixes) exactly the seven behaviours the carried stories ship"},
{"file":"website/docs/guides/scenario-run.md","line":45,"category":"contract-drift","severity":"note","verdict":"CONFIRMED","origin":"introduced","message":"caution says takt scenario run --config is planned and not in 0.14.0; the carry ships it. Also how-a-run-ends.md:125 (exit 5) omits the no-action retry, and deploy-on-kubernetes.md:130/252 keep the mkdir -p $HOME/.cargo/bin workaround as still needed (obsolescence inferred, not reproduced in a deployment)"},
{"file":"ess/domains/takt.yaml","line":3881,"category":"contract-drift","severity":"note","verdict":"CONFIRMED","origin":"introduced","message":"says a harness that fails ends the run with an error; the ESS says nothing of the no-action retry (NO_ACTION_TRIES, each try a step charged its tokens) that story:a-harness-step-without-an-action-is-retried ships"},
{"file":"crates/takt/src/run.rs","line":696,"category":"boundary","severity":"note","verdict":"CONFIRMED","origin":"introduced","message":"the try-limit check comes before host_stop(), so a third try that spends the budget ends exit 5 on the error; run.rs:8298, main.rs:959/972, CHANGELOG.md:24 and run-a-story.md:178 promise exit 3 when a try spent the budget; red: a_third_try_that_spends_the_step_budget_stops_on_the_budget"}
]
```
