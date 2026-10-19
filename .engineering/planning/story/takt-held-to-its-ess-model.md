---
format: aep.planning-md/3
id: story:takt-held-to-its-ess-model
kind: story
status: draft
title: takt's code is held to its ESS model by generated conformance, mutation and protocol exploration
relations:
- decomposes: epic:composable-agentic-work
- serves: vision:safe-agentic-coding
- informed_by: review-result:main-fe93286-areas
- informed_by: story:property-and-fuzz-tests
revision: 2
---
## Outcome

takt's code is held to its own ESS model by ESS's generated checks, not only cited by it: `task check` runs `ess verify conform synthesize` for the parts of `ess/domains/takt.yaml` takt implements and runs the suite against takt, `ess verify conform mutate` audits that suite with specification mutants, and `ess verify protocol explore` explores the bounded schedules of the improvement protocols.

## Context

Owner question of 2026-10-19, after review-result:main-fe93286-areas: ESS already carries property-style checking (ess 0.60.0 `--help`): `verify conform synthesize` generates the suite a specification obliges, `verify conform mutate` derives one-edit specification mutants and reports a survivor as "a declared rule no synthesized scenario pins down", `verify conform check-history` checks a recorded history for linearizability, and `verify protocol explore` explores bounded schedules of a communicating protocol model. takt uses none of them today: `git grep` finds `ess verify conform` only in the terminal-frontend plan (story:takt-serve-read-only). takt's ESS document is checked by `ess specify validate` and by `scripts/check-citations`, which proves each declaration cites the code, not that the code does what the declaration says. Findings of that review a conformance suite over the model would have reached: C4.5 (the pull request's base never compared with the case's base) and P1.2 (a promotion's scorecard rows not bound to the binary), both relations the model can state; X10.5 and X10.6 (the improvement protocols' ordering and missing outcomes), which bounded exploration of the protocol model reaches.

## Scope

- `ess/domains/takt.yaml`: the relations and invariants named above (inferred)
- a conformance runner for takt's CLI and case files, or `ess generate cli` (inferred; ESS's runners are generated for Go and TypeScript, so a Rust target needs a runner or an adapter)
- `Taskfile.yml`: the conformance step (inferred)

## Acceptance

`task check` runs a synthesized conformance suite over at least the case, grant and promotion entities against takt and fails when a mutant that drops the "pull request base equals case base" rule survives `ess verify conform mutate`.

## The Rust route (ESS's answer, 2026-10-19)

ESS has a documented Rust runner, which the `synthesize --target` help did not name (ESS adds that line in 0.63.0): `ess verify conform synthesize --path ess --out suite.json` (`--target ir`), then a cargo test depending on the `ess-conformance` crate from the ESS repository at the pinned tag (not on crates.io), implementing `ess_conformance::target::ConformanceTarget` (identity, begin/end scenario, execute_command, query_view, observe_events, configure_external_outcome, redeliver_event; the rest default to unsupported) and running the crate's `Runner`. `ess verify conform mutate --emit/--collect` works with that suite. Guide: https://beyond10x.github.io/ess/docs/start/runners/rust. The Scope's "a runner or an adapter" is therefore a `ConformanceTarget` implementation over takt's CLI and case files.
