---
format: aep.planning-md/3
id: story:environment-allowlist
kind: story
status: draft
title: Every child of a run starts from an empty environment plus an allowlist
summary: env cleared, then a fixed allowlist; bot-route children also get what the route reads; replaces the name denylist
relations:
- decomposes: epic:composable-agentic-work
- serves: vision:safe-agentic-coding
revision: 1
---
# Story: environment-allowlist

## Outcome

Every child process a run starts (the harness child, the test command, the gate, git, the `aep` frontier and `cargo metadata`) starts from an empty environment plus a fixed allowlist of variables, and only the bot-route children of the forge also receive the variables the route reads; a variable whose name the old denylist would not have caught no longer reaches any child.

## Context

From the tool-calling brainstorm of 2026-10-18, which compared takt's harness with other agent hosts. A process host that clears the environment and then adds an allowlist fails closed: a credential under a name nobody anticipated never reaches the child. takt does the opposite today. `crates/takt/src/forge.rs:370-386` (`is_credential`) names credentials by prefix and suffix (`GITLAB_`, `SSH_AUTH_SOCK`, `_TOKEN`, `_API_KEY`, ...), and `without_credentials` (`forge.rs:401-408`) removes only those from what the child inherits; everything else is inherited. A credential under any other name (`DATABASE_URL` with a password in it, `AWS_SESSION`, a cloud CLI's config path) reaches the harness child and the test command, which are the two children that run code or text a model or the repository supplied.

`product-requirements:takt` R4 and R5 record the denylist as today's state ("built by a name denylist (story:validator-confinement; ...)"). story:confinement-seam records the same gap among its evidence: "the harness child inherits the environment except the keys the adapter removes".

The spawn sites that call `without_credentials` today: the harness (`crates/takt/src/harness.rs:220`), git (`crates/takt/src/git.rs:75`), the test and gate commands (`crates/takt/src/validator.rs:277`), the frontier (`crates/takt/src/frontier.rs:489`), `cargo metadata` (`crates/takt/src/run.rs:2224`) and the bot route (`crates/takt/src/forge.rs:753`, with `bot_route_reads` at `forge.rs:392-397`).

## Scope

- `crates/takt/src/forge.rs`: `is_credential`, `bot_route_reads`, `without_credentials` replaced by an allowlist builder (cited)
- `crates/takt/src/harness.rs`: the harness child's environment, including what its login needs (cited)
- `crates/takt/src/validator.rs`: the test and gate commands' environment (cited)
- `crates/takt/src/git.rs`, `crates/takt/src/frontier.rs`, `crates/takt/src/run.rs`: the remaining spawn sites (cited)
- new: `crates/takt/tests/environment_allowlist.rs` (inferred)

## Ordering

story:confinement-seam routes the same spawn sites through one adapter and takes the environment as an input; both edit `forge.rs`, `harness.rs`, `validator.rs`, `git.rs` and `frontier.rs`, so the two do not run in one wave. Either order works: landing this story first gives the seam an allowlisted environment to pass through.

## Acceptance

With `TAKT_PROBE_SECRET=probe-value` and `GH_TOKEN=probe-gh` in the parent environment, a run whose test command and fake harness each print their whole environment shows neither value in either child's output, while a bot-route forge child started by the same run still receives `GH_TOKEN`.

## Ambiguities

- What the harness child needs to find its login and its program (`HOME`, `PATH`, `XDG_*`, locale, `TERM`): `inferable` from a run under the allowlist, recorded in the test; a variable added to the allowlist later is a judge-surface change.
