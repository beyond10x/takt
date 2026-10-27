---
format: aep.planning-md/3
id: story:codex-off-pin-proposes
kind: story
status: proposed
title: A Codex step on a codex outside metaharness's pin proposes its change
summary: metaharness 0.20.0 makes H9 advisory; takt takes it through a loom-sdk pin
relations:
- serves: vision:safe-agentic-coding
- decomposes: epic:takt-on-loom-sdk
revision: 2
transitions:
- {from: "draft", to: "proposed", at: "2026-10-27T02:27:17Z", actor: "human:timo", revision: 2, decided_on: {"recorded":{"approval":1}}}
---
# A Codex step on a codex outside metaharness's pin proposes its change

## Context

Owner, 2026-10-27: "Another defect is that currently my current codex version is just not working
together with takt because it, for whatever reason, is doing some weird version check, which I
think doesn't make much sense because we're using codex via metaharness, and metaharness should
just use that protocol from codex and just work. There shouldn't be a hard failure due to that."
And: "make sure its tracked in the takt planning store, and efficiently implemented and
integrated".

Reproduced on 2026-10-27 with takt on loom-sdk 0.43.0 (metaharness 0.19.0): `codex --version` on
the operator's `PATH` printed `codex-cli 0.160.0`, but the `PATH` metaharness gives its child
(`$HOME/.local/bin` first) resolved a `codex` at 0.164.0. `takt run --harness codex --forge none`
on a scratch crate spent 46,182 tokens in step 2 and then stopped, exit 5: "the session proposed
nothing: the session's hermetic floor has a gating gap: H9 Gap: the vendor is 0.164.0 and the
adapter is pinned to ["0.153.4", "0.160.0"]". takt passes no `--strict-version` and pins no
version itself: the hold is Loom's session executor, which holds a Codex proposal whose gating
rows are not `ok`, over metaharness's H9 row, which was gating.

metaharness 0.20.0 (`story:off-pin-vendor-version-is-advisory` in its store) makes H9 advisory,
warns the version once and keeps `--strict-version` as the operator's opt-in. takt reaches
metaharness only through loom-sdk's exact tag, so the pin moves through a Loom release first.

## Outcome

A `takt run --harness codex` whose child codex is outside metaharness's Codex pin proposes and
applies its step's change, with the version recorded in the step's call report, as a run on a
pinned codex does.

## Acceptance

1. takt depends on a loom-sdk release whose `b10x-loom-session-executor` takes metaharness 0.20.0
   or later.
2. `takt run --harness codex --forge none` on a scratch crate whose child `codex` reports a version
   outside the pin (0.164.0 here; the operator's `PATH` printing 0.160.0) passes the step that
   stopped on H9 before: no `H9` in the stop line, exit not 5.
3. `crates/takt/tests/harness_parity.rs` `a_codex_session_on_another_version_proposes_nothing` and
   the F-A2 held-session test assert the new behaviour, the assertion messages naming this story.

## Out of Scope

- metaharness's change itself (its store) and Loom's pin hop (Loom's store).
