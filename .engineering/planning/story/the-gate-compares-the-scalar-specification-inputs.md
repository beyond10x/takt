---
format: aep.planning-md/3
id: story:the-gate-compares-the-scalar-specification-inputs
kind: story
status: draft
title: The gate compares the scalar fields of the specification inputs
summary: 'only the lists of ess-inputs.yaml and system.yaml are compared, not requires: or format:'
tags:
- from-finding
- rsi
relations:
- decomposes: epic:recursive-self-improvement
- serves: vision:safe-agentic-coding
- informed_by: approval-record:wave-55-review-correction
revision: 1
---
# Story: the-gate-compares-the-scalar-specification-inputs

## Outcome

`gate.green` does not hold when a step changed a scalar field of `ess/ess-inputs.yaml` or `ess/system.yaml` that decides
what the specification check reads (`requires:`, `format:`, a domain root), and the `ess/` root is not hard-coded where
a repository names another.

## Context

Found by the wave 55 security review at d0e8fba54 (`crates/takt/src/validator.rs:4032`):
story:the-gate-does-not-trust-a-step-accepted-citation compares only the top-level lists of the two files. The
adversary read `requires:` and `format:` as deciding no file, since `ess` reads the file strictly; whether a changed
`requires:` changes what `ess specify validate` accepts was not tested.

## Acceptance

- A probe with the pinned `ess` shows whether a changed `requires:` or `format:` changes the validation verdict; the
  finding records it.
- If it does: a step that changes it does not record `gate.green`: a test.
