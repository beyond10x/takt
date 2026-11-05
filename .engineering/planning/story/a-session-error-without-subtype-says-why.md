---
format: aep.planning-md/3
id: story:a-session-error-without-subtype-says-why
kind: story
status: rejected
title: A session that ends in an error with no subtype says so, with no empty reason
tags:
- finding
refs:
- provider: finding
  reference: a-session-error-without-subtype-says-why
relations:
- decomposes: epic:composable-agentic-work
- serves: vision:safe-agentic-coding
revision: 4
transitions:
- {from: "draft", to: "proposed", at: "2026-11-05T02:14:31Z", actor: "human:timo", revision: 3}
- {from: "proposed", to: "rejected", at: "2026-11-05T02:14:32Z", actor: "human:timo", revision: 4}
---
## Outcome

A session that ends in an error with no subtype is reported with a reason a reader can use, never
"the session ended in an error ()".

## Context

The session executor stores `subtype.clone().unwrap_or_default()` when a `SessionEnded` event has
`is_error` and no subtype (`crates/session-executor/src/lib.rs:445`), and formats it as
"the session ended in an error ({subtype})" (`lib.rs:544`): the message then names an empty
reason. Found by a read-only review of 2026-10-21 at 0a61e14a.

## Scope

- `crates/session-executor/src/lib.rs`, tests.

## Acceptance

1. An error end with no subtype yields a message that says the session reported no subtype and
   carries what it did report (its final answer or turn count), with no empty parentheses: a test.

## Decision

Rejected by approval-record:wave-54-four-findings as stale: the crate it names, `crates/session-executor`, no longer
exists in this repository.
