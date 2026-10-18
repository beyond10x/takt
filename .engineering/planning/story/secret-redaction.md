---
format: aep.planning-md/3
id: story:secret-redaction
kind: story
status: draft
title: No credential value reaches a prompt, events.jsonl or a case record
summary: a redactor on every prompt and record line; a credential type with no serializer and a non-leaking debug form
relations:
- decomposes: epic:composable-agentic-work
- serves: vision:safe-agentic-coding
revision: 1
---
# Story: secret-redaction

## Outcome

No credential value reaches a prompt, `events.jsonl` or a case record: every line takt writes to those passes a redactor that replaces the value of each credential in takt's own environment, and each string that has a known token form, with a fixed marker, and a credential takt holds in memory has a type that cannot be serialized and whose debug form does not print it.

## Context

From the tool-calling brainstorm of 2026-10-18. A secrets library it compared keeps a resolved credential in a type with no serializer and a debug form that leaks nothing, and runs a redactor before anything reaches a log or a model; a connector layer never serializes a binding and never accepts one as input; another uses placeholders (`${secret:<handle>}`) in place of values.

`product-requirements:takt` R4: "Secrets never reach the harness's environment, prompt or the run records"; its row records the environment as covered by a name denylist and "prompt, run records, filesystem: no story yet". R5 records the same for a hostile repository's commands. Where a value can enter today: the harness prompt is built from the intent and the story (`crates/takt/src/harness.rs`, `ClaudeCode::prompt`); a child's failure text is carried into errors and refusals (`harness.rs`, the `bail!` with up to 400 characters of the child's standard error, after `run_child`), and the run record appends what it is given (`crates/takt/src/events.rs`, `Events::emit`); evidence and case files are written by `crates/takt/src/case.rs` and `crates/takt/src/evidence.rs`.

## Scope

- `crates/takt/src/events.rs`: `Events::emit` passes the redactor (cited)
- `crates/takt/src/case.rs`, `crates/takt/src/evidence.rs`: case and evidence writers pass it (cited)
- `crates/takt/src/harness.rs`: the prompt passes it (cited)
- `crates/takt/src/forge.rs`: the credential names the redactor reads values for, shared with the environment rule (cited)
- new: a redactor module and a credential type (inferred)
- new: `crates/takt/tests/secret_redaction.rs` (inferred)

## Acceptance

With `TAKT_PROBE_TOKEN=probe-9f3c` in takt's environment and a test command that prints that value to standard error and exits 1, and a story whose body contains the same value, a search of the case directory (events, decisions, evidence) and of the prompt the fake harness received finds `probe-9f3c` nowhere and finds the redaction marker.

## Ambiguities

- Which token forms the redactor knows besides values from the environment (forge tokens, model provider keys, private key headers): `inferable`, a fixed list in the module, each form with a test.
- Whether a redacted prompt can still carry a story that quotes a token on purpose: `inferable`, no; the marker replaces it.
