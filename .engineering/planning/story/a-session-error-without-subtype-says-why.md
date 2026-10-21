---
format: aep.planning-md/3
id: story:a-session-error-without-subtype-says-why
kind: story
status: draft
title: A session that ends in an error with no subtype says so, with no empty reason
tags:
- finding
refs:
- provider: finding
  reference: a-session-error-without-subtype-says-why
relations:
- decomposes: epic:composable-agentic-work
revision: 1
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
