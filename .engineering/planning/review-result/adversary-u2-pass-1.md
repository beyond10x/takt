---
format: aep.planning-md/3
id: review-result:adversary-u2-pass-1
kind: review-result
status: active
title: 'Adversary pass 1 on U2: failed gate lines in the next prompt'
relations:
- reviews: story:edits-keep-ess-citations-resolved
revision: 1
---
# Adversary pass 1 on U2 (story:edits-keep-ess-citations-resolved)

Tree: `wave/38` at 417f4b33 plus the uncommitted unit edits. Test file added:
`crates/takt/tests/failed_gate_lines_adv1.rs`. `failed_gate_lines`: 5 passed; `failed_gate_lines_adv1`:
1 passed, 5 failed.

| id | file:line | verdict | origin | measured |
|---|---|---|---|---|
| F1 | crates/takt/src/validator.rs:173 | warning | introduced | a token preceded by an ANSI colour code escapes `credential_shape` and reaches the prompt and the run record |
| F2 | crates/takt/src/validator.rs:169 | warning | introduced | the credential check runs per line; a PEM private key's body lines are kept |
| F3 | crates/takt/src/validator.rs:2112 | warning | introduced | the re-run's exit status is ignored; a re-run that passes is shown as what failed |
| F4 | crates/takt/src/validator.rs:1445 | note | undecided | a gate script containing the shell's `:` builtin errors with a git pathspec `:/__main__.py` |
| F5 | crates/takt/src/validator.rs:2360 | note | introduced | every failed `process.succeeded` gate runs its command twice |
| F6 | crates/takt/src/harness.rs:725 | note | introduced | the failed-lines block gets `credential_shape` only, not the gates-policy scan scope files get |
| F7 | crates/takt/src/run.rs:10349 | note | introduced | the lines are kept on the run record's evidence line, not in the canon-evidence/1 record |

Not broken: re-run writes in the tree (reverted by the step guard), a re-run past the deadline,
credentials in the re-run's environment, the byte bounds, fences, a passing run's prompt
byte-identical, staleness, replay.

```findings
- file: crates/takt/src/validator.rs
  line: 173
  category: contract-drift
  severity: warning
  verdict: INFEASIBLE
  origin: introduced
  message: a token preceded by an ANSI colour code (ending in the word char `m`) escapes credential_shape and reaches the prompt and the run record
- file: crates/takt/src/validator.rs
  line: 169
  category: contract-drift
  severity: warning
  verdict: INFEASIBLE
  origin: introduced
  message: the credential check runs per line, so a PEM private key's body lines are kept; only its BEGIN line is withheld
- file: crates/takt/src/validator.rs
  line: 2112
  category: boundary
  severity: warning
  verdict: INFEASIBLE
  origin: introduced
  message: the re-run's exit status is ignored, so a re-run that passes is shown and recorded as what the failed gate printed
- file: crates/takt/src/validator.rs
  line: 1445
  category: boundary
  severity: note
  verdict: INFEASIBLE
  origin: undecided
  message: a gate script containing the shell's `:` builtin makes gate.run error with git pathspec `:/__main__.py` instead of failing the gate
- file: crates/takt/src/validator.rs
  line: 2360
  category: judgement
  severity: note
  verdict: CONFIRMED
  origin: introduced
  message: every failed process.succeeded gate runs its command twice, each up to the project timeout, side effects included
- file: crates/takt/src/harness.rs
  line: 725
  category: judgement
  severity: note
  verdict: CONFIRMED
  origin: introduced
  message: the failed-lines block gets only credential_shape, not the gates-policy scan that scope files entering the same prompt get
- file: crates/takt/src/run.rs
  line: 10349
  category: acceptance
  severity: note
  verdict: CONFIRMED
  origin: introduced
  message: acceptance 1 says the gate.run evidence record keeps the lines; only the run record's evidence line does
```
