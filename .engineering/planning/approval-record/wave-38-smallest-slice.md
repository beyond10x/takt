---
format: aep.planning-md/3
id: approval-record:wave-38-smallest-slice
kind: approval-record
status: approved
title: 'Wave 38 cut to its smallest slice: red tests stop the run, and release 0.14.0'
tags:
- loom-agent
relations:
- decides: story:repeated-identical-test-failures-stop-the-run
- decides: story:a-red-base-stops-at-open
- decides: story:a-duplicate-yaml-key-is-refused-at-its-line
revision: 2
transitions:
- {from: "draft", to: "approved", at: "2026-10-26T22:46:28Z", actor: "human:timo", revision: 2}
---
# Approval: wave 38 cut to its smallest slice, and release 0.14.0

## Decision

The owner, 2026-10-26, relayed by the orchestrator: "just implement the smallest vertical slice to
get a stable takt release ... running trials is not your business". The orchestrator's scope, the
same day: "U1 (same failing test names twice stop the run) + U2 (red base stops at open) + the
run_config_adv1 re-pin. Drop step 2 entirely: no host run, no model calls, no endpoint; the units
are proven by takt's own tests (the CLI tests with the fake harness ...). Skip U3-U8; leave them
accepted in the store for the next wave and say so in the wave page. Then: one PR, merge when
green, tag 0.14.0".

This narrows approval-record:wave-38-trial-s7-defects: wave 38 delivers
story:repeated-identical-test-failures-stop-the-run and story:a-red-base-stops-at-open, and the
re-pin of the ignored case in `crates/takt/tests/run_config_adv1.rs`
(story:a-duplicate-yaml-key-is-refused-at-its-line). The other seven stories stay `active` for
the next wave; their reviewed code is kept on the branch `carry/wave-38-u4-u5`.

## Decisions taken under it

| fork | taken | why |
|---|---|---|
| repeated failures | 2 `tests.run` results in a row, each at a new revision, failing the same set stop the run (`REPEATED_FAILURES = 2`), replacing the 3 of approval-record:wave-38-trial-s7-defects | the scope above: "same failing test names twice stop the run" |
| the opt-out | `--continue-past-red-base` on `takt run` and `takt scenario run`, and `continue_past_red_base` in `takt run --config`; the scenario run's `--config` stays with story:scenario-run-takes-config | adversary pass 1 on U1, finding 5; `takt scenario run --config` is a unit left for the next wave |
| end-to-end | none on the host; the stops are proven by `crates/takt-cli/tests/test_failure_stops_cli.rs` and `test_failure_stops_cli_adv1.rs`, a harness double on the program path that counts its calls | the scope above: "no host run, no model calls, no endpoint" |
| a red base on purpose | the existing tests whose fixture base fails its tests call `Run::continue_past_red_base` or pass the flag | they test what follows a red base, not the stop |
| version | 0.14.0, the default red-base stop marked Breaking | a run on a red base that went on before now stops |
