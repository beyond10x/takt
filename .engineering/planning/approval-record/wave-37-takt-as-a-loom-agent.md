---
format: aep.planning-md/3
id: approval-record:wave-37-takt-as-a-loom-agent
kind: approval-record
status: approved
title: 'Wave 37: takt as a Loom agent (image, environment credential, configuration contract, --story, harness pin) and a release'
tags:
- loom-agent
relations:
- decides: story:image-runs-the-toolchain-it-tests-with
- decides: story:gate-exec-finds-the-toolchain-in-the-image
- decides: story:image-ships-b10x-gates
- decides: story:image-cargo-home-is-writable
- decides: story:endpoint-credential-from-environment
- decides: story:one-config-contract-with-loom
- decides: story:run-names-its-story
- decides: story:image-harness-0-20-0
revision: 2
transitions:
- {from: "draft", to: "approved", at: "2026-10-26T13:48:01Z", actor: "human:timo", revision: 2}
---
# Approval: wave 37, takt as a Loom agent

## Decision

The orchestrator, under the owner's grant (2026-10-26), item: "one wave through the store, then a
release". File the ten takt gaps of the first kind-cluster trial as stories under one epic;
"Implement in this wave, each its own unit with an adversary pass: the image (OpenSSL deps,
toolchain on PATH, `b10x-gates` installed, writable CARGO_HOME, user home not under /home ...), the
env credential route, the config contract, the harness pin 0.20.0 ..., and `--story`. The citation
and git-excluded items stay accepted-not-started unless small." Then build the image locally, no
push, and "Release per AGENTS.md".

## Units

| unit | stories |
|---|---|
| U1 image | story:image-runs-the-toolchain-it-tests-with, story:gate-exec-finds-the-toolchain-in-the-image, story:image-ships-b10x-gates, story:image-cargo-home-is-writable |
| U2 credential | story:endpoint-credential-from-environment |
| U3 configuration | story:one-config-contract-with-loom |
| U4 story | story:run-names-its-story |
| U5 harness pin | story:image-harness-0-20-0, only if a loom tag on metaharness 0.19.0 exists before the gate |

Left out: story:git-excluded-files-are-not-step-changes and story:edits-keep-ess-citations-resolved
(draft; the item keeps them not started).

Commits this approval covers: one per unit, the store commits of the wave (this record, the
stories' moves, the close), the merge of `wave/37` into `main`, and the release that follows per
the item.

## Decisions taken under it

| fork | taken | why |
|---|---|---|
| the configuration contract | `--config` reads TOML (`.toml`) or YAML (`.yaml`, `.yml`) by extension, the same fields; a YAML file with `format: loom.agent/1` is a Loom agent document whose `takt` member is the run configuration, Loom's members unread, no member setting nothing; the image's entrypoint reads `/config/agent.yaml` | the orchestrator's recommendation, taken. Loom 0.42.0 refuses an unknown `loom.agent/1` member at admission (loom `crates/loom-sdk/src/agent.rs`, `unknown member`), so until loom admits a `takt` member a resource carries the run's settings as flags in `spec.command.args`; that admission is reported as a loom gap |
| the environment credential | takt reads the variable once, writes it to a 0600 file in a new 0700 directory under the first of `$XDG_RUNTIME_DIR`, `/dev/shm` and the temporary directory that b10x-harness's reader accepts, hands that file to the session executor as the endpoint's token file, keeps the variable out of every process it starts, and removes the file when the run ends | loom's session executor (0.41.0, 0.42.0) passes a token file or a secret's name and no variable's name, and `AGENTS.md` § Rules keeps credentials out of a harness's environment; the reader's rule is llm 0.15.0 `crates/llm-credentials/src/file.rs` |
| the harness pin | the orchestrator's answer, option A: "The loom agent running wave q13 has been told to move metaharness to 0.19.0 and release ... If a loom tag with metaharness 0.19.0 exists before your gate, pin it and harness 0.20.0 in this wave; otherwise B as you said, with the story's blocker naming the loom release it waits on." | takt reaches metaharness only through `b10x-loom-sdk`; loom 0.42.0 locks metaharness 0.15.0, whose b10x adapter pins harness 0.18.0; cargo refuses a `[patch]` pointing at the same git source |
| the wave's shape | one managed worktree, one branch `wave/37`, one `target/`; implementor sub-agents edit inside their scope and commit nothing; each unit committed by the dispatcher through the bot route; one adversary pass per unit | `.agents/dispatcher-agent.md`, and the orchestrator's "each its own unit with an adversary pass" |
