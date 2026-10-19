---
format: aep.planning-md/3
id: story:gate-limits-are-configurable
kind: story
status: draft
title: The gate's per-command and per-gate time limits come from the gates document
summary: the assertions crate fixes 300 s per command and per gate, so takt's workspace tests cannot be its gate
tags:
- rsi
relations:
- decomposes: epic:recursive-self-improvement
- serves: vision:safe-agentic-coding
revision: 1
---
# Story: gate-limits-are-configurable

## Outcome

The time limits of a gate run, per command and for the whole gate, are read from the gates
document, with today's 300 s as the default, so a repository whose test suite takes longer can make
it part of its gate.

## Context

Found while delivering story:takt-has-a-gates-document (wave 15, 2026-10-19): the limits are
constants in the upstream assertions crate (`canon-engineering-assertions`, `lib.rs:217-220`,
`:438-452`). takt's workspace tests take longer than 300 s on this host, so takt's gate holds only
the four fast commands and the tests run as the delivery's test command. The change lands upstream
first (engineering-protocols), then takt bumps its pin.

## Acceptance

- A gates document that sets a per-command limit of 1800 s runs a command that takes 400 s to
  completion: an upstream test.
- A gates document without the keys keeps 300 s: an upstream test.
- takt's gates document moves its workspace tests into the gate after the pin bump.
