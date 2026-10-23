---
format: aep.planning-md/3
id: story:harness-child-confined
kind: story
status: active
title: The harness session, git, the forge and aep run under a Substrate profile through loom-sdk
summary: Fail-closed profiles with named egress for the harness; the tests ignored for confinement pass
tags:
- loom
relations:
- decomposes: epic:takt-on-loom-sdk
- serves: vision:safe-agentic-coding
- depends_on: story:second-harness-adapter
- depends_on: story:confinement-seam
revision: 5
transitions:
- {from: "draft", to: "proposed", at: "2026-10-21T23:29:26Z", actor: "human:timo", revision: 3}
- {from: "proposed", to: "active", at: "2026-10-21T23:29:26Z", actor: "human:timo", revision: 4}
---
# Story: harness-child-confined

## Outcome

Every child a run starts besides the test command — the harness session, git, the bot-route forge and `aep` — runs under a Substrate profile through loom-sdk, fail-closed: the harness session writes only inside its scratch copy, reaches only the destinations its profile names, and cannot outlive its step. The tests ignored for confinement pass.

## Context

- `story:confinement-seam` routes every spawn through one seam and runs the test command under the `SubstrateRunner` of loom-sdk; this story routes the remaining children through a confining profile.
- Ignored until this story: M9, a writer that escapes the step's process group (`crates/takt/tests/step_cannot_forge_its_validators.rs:1500`), and a process that rejoins takt's own group (`crates/takt/tests/hours_bounds_a_harness_step_adv1.rs:321`). The routes recorded on `story:confinement-seam` that close by confining the harness child (M16, M17, M18) are re-run here; the routes through the test command (M8, M19, M20, C6, C8) are `story:confinement-seam`'s.
- The harness session is started by metaharness, which takes a confinement provider from its embedder (metaharness 0.9.3, `crates/metaharness/src/process.rs:96-101`, `ProcessEnvelope`); the session executor of `story:harness-proposes-takt-performs` supplies one backed by the confinement of loom-sdk.
- Credentials never enter a harness's environment, its prompt or a case record (`AGENTS.md` § Rules).
- A story blocked by a gap of loom-sdk stays blocked; takt builds no substitute.

## Scope

- `crates/takt/src/confinement.rs` (profiles for the harness, git, the forge and `aep`)
- `crates/takt/src/git.rs`, `crates/takt/src/forge.rs`, `crates/takt/src/frontier.rs` (spawn under a profile)
- `crates/<session-executor>/` (the `ProcessEnvelope` handed to metaharness)
- `crates/takt/tests/step_cannot_forge_its_validators.rs`, `crates/takt/tests/hours_bounds_a_harness_step_adv1.rs` (the ignore removed)
- new: `crates/takt/tests/harness_confined.rs`

## Acceptance

1. The two tests ignored for confinement (`step_cannot_forge_its_validators.rs:1500`, `hours_bounds_a_harness_step_adv1.rs:321`) are no longer ignored and pass.
2. A harness session that writes outside its scratch copy, or into the object store of the repository, is refused by the profile; a test probes each.
3. A harness session that connects to a destination its profile does not name fails to connect; a test probes one.
4. A backend that cannot enforce a profile ends the run with a refusal that names it, and starts no child unconfined; a test.
5. With a profile that refuses every program, a run starts no harness, git, forge or `aep` child.

## Blocked by

- loom-sdk exports confinement for an arbitrary child — program, arguments, working directory, environment and profile; today it exports only `TestRunner::run(&TestCommand, root)` (Loom 0.26.0 `crates/loom-intake-slice/src/confinement.rs:33-36`, `crates/loom-sdk/src/lib.rs:77-80`). Blocks confining the harness child, git, the forge and `aep`.
- A profile with named egress for a harness child; Substrate offers an egress aperture, and Loom confines with no network only (`crates/loom-intake-slice/src/confinement.rs:511-514`). Blocks confining Claude Code and Codex, which reach a model API.

## Out of Scope

Toolchains other than Rust for the test command; `workflow/1`.

## Finding (2026-10-23, the wave-34 review at 79ec8c0)

- `finding:codex-and-harness-admission-untested` (low). Codex and b10x-harness are admitted by bare program name (`crates/takt/src/harness.rs:560-561`, `:570`), not by the path metaharness resolves; under `Substrate`, which checks the file name, that is the same, but only Claude's refusal has a test (`crates/takt/tests/confinement.rs:1390`). `expected_before`: `grep -cE 'Kind::Codex|Kind::B10x' crates/takt/tests/confinement.rs` prints 0.
