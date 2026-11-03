---
format: aep.planning-md/3
id: approval-record:wave-51-harness-mapping-carried
kind: approval-record
status: approved
title: 'Wave 51: the harness mapping carries effort, the cost cap, the version check and context'
summary: one wave of two units in order for 0.30.0
relations:
- decides: story:harness-section-is-metaharness-session-request
- decides: story:harness-flags-spell-metaharness-names
revision: 2
transitions:
- {from: "draft", to: "approved", at: "2026-11-03T23:15:00Z", actor: "human:timo", revision: 2}
---
# Approval: wave 51

## Decision

The owner, 2026-11-04, through the orchestrator: "Go: wave 51 as proposed. Units a then b in order,
the commits and the delivery you listed, 0.30.0. Your 3 decisions stand (context paths,
max-budget-usd in micro-USD, ESS scenarios as same-name cargo tests); record them in your store."

The wave is story:harness-section-is-metaharness-session-request (unit a) and
story:harness-flags-spell-metaharness-names (unit b), in that order: unit b starts from `wave/51`
once unit a has merged there. story:harness-child-confined stays out: dependency-blocker:loom-child-confinement
is open. A draft from a finding does not enter.

## The three decisions

1. `context`, in the run file's `harness` mapping and as `--context`, fills Loom's
   `SessionOverrides.context`: repository-relative plain paths of files the session executor
   copies. Any other path is refused before any case opens, naming it. The document metaharness
   checks is given the repository as its working directory.
2. `max-budget-usd` is read as a number or a decimal string and carried as micro-dollars
   (`SessionOverrides.max_budget_micro_usd`); a value finer than one micro-dollar is refused before
   any case opens, naming the key.
3. The conformance scenarios the two stories' acceptance names in `ess/ess-inputs.yaml` stay
   unrun, as for story:model-names-an-llm-route: this repository has no conformance runner. Each
   scenario is checked by a cargo test of the scenario's name.

## Selection, read at origin/main 1c3ba5178

`aep plan artifact waves --kind story --status active` (aep 0.79.0), after the wave's scope entries:

```
wave 1
  story:harness-section-is-metaharness-session-request (inferred)
wave 2
  story:harness-flags-spell-metaharness-names
collision: story:harness-flags-spell-metaharness-names story:harness-section-is-metaharness-session-request crates/takt-cli/src/main.rs
collision: story:harness-flags-spell-metaharness-names story:harness-section-is-metaharness-session-request crates/takt-cli/tests/harness_flags_spell_metaharness.rs
collision: story:harness-flags-spell-metaharness-names story:harness-section-is-metaharness-session-request crates/takt/src/run_config.rs
collision: story:harness-flags-spell-metaharness-names story:harness-section-is-metaharness-session-request ess/ess-inputs.yaml
collision: story:harness-flags-spell-metaharness-names story:harness-section-is-metaharness-session-request website/docs/reference/configuration.md
unassessed: story:harness-child-confined
2 wave(s), 5 collision(s), 1 unassessed
```

## Commits it authorises

The opening store and page commit, the unit commits, the merges into `wave/51`, the review and
evidence store commits, the release prepare, one pull request through the bot route and its merge
to `main`, the tag 0.30.0 and its release.
