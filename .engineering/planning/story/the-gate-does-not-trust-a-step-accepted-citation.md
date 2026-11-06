---
format: aep.planning-md/3
id: story:the-gate-does-not-trust-a-step-accepted-citation
kind: story
status: implemented
title: gate.green does not hold on citation digests or specification inputs a step changed
summary: a step can add a needle row and run check-citations accept itself, and the citation gate passes
tags:
- rsi
relations:
- decomposes: epic:recursive-self-improvement
- serves: vision:safe-agentic-coding
scope:
- confidence: cited
  path: crates/takt/src/validator.rs
- confidence: inferred
  path: crates/takt/tests/gate_does_not_trust_step_citations.rs
revision: 7
transitions:
- {from: "draft", to: "proposed", at: "2026-11-06T04:03:36Z", actor: "human:timo", revision: 4}
- {from: "proposed", to: "active", at: "2026-11-06T04:03:36Z", actor: "human:timo", revision: 5}
- {from: "active", to: "implemented", at: "2026-11-06T12:28:44Z", actor: "human:timo", revision: 6, decided_on: {"recorded":{"test_result":1,"review_outcome":1,"verification":1}}}
---
# Story: the-gate-does-not-trust-a-step-accepted-citation

## Outcome

`gate.green` does not hold when a step changed what the specification and citation commands trust:
a digest recorded in `ess/domains/*.citations.tsv`, or a domain dropped from `ess/ess-inputs.yaml` or
`ess/system.yaml`. A digest says a person read the lines; a step is not that person.

## Context

Found by the review of story:the-gate-compares-its-scripts-and-tool-config-at-the-base
(2026-10-19): a step can write a needle row and run `scripts/check-citations accept` itself, and
`--require-digests` then exits 0; dropping `domains/takt.yaml` from `ess-inputs.yaml` makes
`ess specify validate --path ess` exit 0 while the file does not parse. The merge of `ess/**` still
needs the owner's grant, but the gate's evidence should not claim what nobody checked.

## Acceptance

- A step that adds or changes a digest in a citations file does not record `gate.green`: a test.
- A step that removes an entry from `ess/ess-inputs.yaml` or `ess/system.yaml` does not record
  `gate.green`: a test.

## Status

Built in wave 55 (unit d, d0e8fba54, corrected in dbd7bba5f), released in 0.33.0 from fedfbc28c, the merge of pull request 62.

- `crates/takt/src/validator.rs`: `trusted_input_changes` refuses the gate on a citation digest the base did not hold for a row, matched one to one, and on a specification entry dropped from `ess/ess-inputs.yaml` or `ess/system.yaml`; a row only re-pointed passes.
- `crates/takt/tests/gate_does_not_trust_step_citations.rs` (red, then green) and the review cases in `w55_adversary.rs` and `w55_security.rs`.

Review: review-result:w55-adv-1, the one-to-one finding fixed; a re-point to a copy is story:a-citation-re-pointed-to-a-copy-is-not-trusted.
