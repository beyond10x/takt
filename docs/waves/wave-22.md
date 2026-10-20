# Wave 22: every shell form read or refused, a trial that records its layers, a report by layer

Stage: **running**. Shape: one dispatcher in one checkout, one wave branch `wave/22` cut from
`main`, implementor sub-agents in the shared tree, one adversarial review and one full gate before
the close. Every unit is a judge surface.

## Units

| unit | story | acceptance (condensed) | tag |
|---|---|---|---|
| A | `story:the-gate-reads-every-shell-form` | the eleven ignored tests of `gate_compares_imports` pass without `#[ignore]`; `PYTHONPATH` set in the line, by `export`, in a sourced file or by `env` is taken as the binding's; `cd` and `env -C` move the run directory; a wrapper program the reader does not know fails closed | rsi |
| B | `story:a-trial-records-the-revision-of-each-layer`, `story:the-report-attributes-cost-and-failures-to-layers` | the run record holds a `layers` map read from what ran; the scorecard refuses runs whose layers differ unless asked; `takt report` attributes cost and failures by layer from each layer's records, a figure not stated shown as not reported, never 0 | rsi, ADR 0101 |
| D | `story:harness-proposes-takt-performs` | joins when metaharness 0.10.0 is tagged (Strict isolation with an embedder's working directory) | loom |

`story:step-prompt-omits-the-change-so-far` stays out: the trial agent's first candidate covers
it and comes back through its port onto 0.5.0.

## Order

A and B at once: A reads `crates/takt/src/run.rs`'s command reader and `validator.rs`; B the run
record in `run.rs`, `scorecard.rs`, `report.rs` and `trace.rs`. D after the tag.

## Pre-flight

| check | value |
|---|---|
| tree | `main` at 9f370b5 (wave 21 merged, the handoff note) |

## Log

- 2026-10-20: wave opened from `main` at 9f370b5.
- 2026-10-20: units committed: A 117fe17 (eleven shell forms read; wrappers; a program in no
  table given a known one fails closed), A follow-up 3952655 (the import walk fails closed at
  its cap of 64 and on a dynamic import), B 0be8357 (layers on the record, the report by layer);
  ESS ef4e004 (1001/1001 citations).
- 2026-10-20: the first full test run at ef4e004 was red: 1506 passed, 22 failed. 20 of the 22
  from B's `<program> --version` spawn at run open (no timeout, outside the step guard, one
  call too many on the tests' fake harness); fixed in d2d8ce5: the harness layer is read from
  the step's own stream (`claude_code_version` of the `system`/`init` event) onto the step
  line's `version`, no spawn. The other 2 (`takt_gates_document`) from A's fail-closed rule
  refusing takt's own gate line `scripts/gate-exec python3 -I scripts/check-citations`; fixed in
  3a1a39a: a file of the tree given a known program is compared whole and read as handing off
  to it. ESS text and 31 re-pointed citations in 7bf9ebb.
- 2026-10-20: gate at 7bf9ebb: `cargo fmt --all --check` clean, `cargo clippy --workspace
  --all-targets --locked -- -D warnings` clean (after boxing `SteppedBy::Harness`'s call
  report), `cargo test --workspace --locked` 1531 passed, 0 failed, 6 ignored; citations
  995/1001 with digests (6 unresolved by the boxing, found by the review as F1). Adversarial
  review started at 7bf9ebb.
- 2026-10-20: D (S2) does not join: metaharness 0.10.0 (3d697011) tagged after the gate; S2
  opens wave 23. Filed from the trial agent and the orchestrator's survey, drafts:
  story:the-scorecard-names-no-metric-it-did-not-compute, story:takt-reads-harness-traces-as-
  trace-ir, story:takt-models-its-inner-workings-in-ess (wave 24).
- 2026-10-20: adversarial review at 7bf9ebb: needs-revision. F1 (high, gate red): 6 citations
  of `run.rs` did not resolve with digests, the clippy boxing having changed their text after
  the citations were accepted; fixed in 39d366b (1001/1001). F2 (medium) and F3 (low): a Python
  file of the tree that sets `PYTHONPATH` before it execs its arguments lets a hand commit to
  the imported module pass the gate, and an option's value can be taken as the handed-off
  program; filed as story:a-handed-off-command-is-read-with-the-wrappers-environment (draft,
  no `expected_before` predicate yet: the probe is a gate run on a planted tree, written up in
  the story). F4, F5, F6 (low, low, planner) in the lows queue. Clean: no harness spawn outside
  a step; `version()` reads the first 8 non-empty lines; the ESS prose matches the code.
- 2026-10-20: 9292d6c: the trial agent's finding at 81f34956, `step_transcripts_cli` asserting
  on the bare `.local/state` substring (fails in a checkout under `~/.local/state`), fixed; 6
  passed.
- 2026-10-20: stories A and B (three) moved to implemented on `test_result` at 7bf9ebb. Release
  0.6.0 cut at the close (the bump and the CHANGELOG section for waves 21 and 22).
