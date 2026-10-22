---
format: aep.planning-md/3
id: approval-record:release-0-8-0
kind: approval-record
status: approved
title: takt 0.8.0 is released from the wave 33 merge, Linux only
relations:
- decides: story:the-loop-is-run-until-blocked
revision: 2
transitions:
- {from: "draft", to: "approved", at: "2026-10-22T15:25:29Z", actor: "human:timo", revision: 2}
---
# Approval: release 0.8.0

## Decision

The orchestrator, under the owner's delegation (2026-10-22): "release takt through its own process,
then hand acceptance 5 to the trial agent as planned."

## What the dispatcher decided under it

| item | decision | why |
|---|---|---|
| version | 0.8.0 | the stop order changes: a case that only a grant moves now stops exit 4, and one with no performer exit 7, where 0.7.0 gave exit 3 on a spent budget (CHANGELOG `[Unreleased]`); `--confinement` is new |
| revision | main at the wave 33 merge, plus one release commit: version 0.7.0 to 0.8.0 in `Cargo.toml` and `Cargo.lock`, CHANGELOG `[Unreleased]` dated as `[0.8.0]` | |
| targets | x86_64 and aarch64 Linux, as `release.yml` builds | 0.7.0's CHANGELOG said macOS returns in 0.7.1; it does not return in 0.8.0: loom-sdk 0.34.0's `b10x-loom-intake-slice` does not compile on macOS (`nix::sys::wait::waitid`, `src/executor.rs:1352`, darwin run 39800445914); the CHANGELOG says so |
| CHANGELOG | the lines waves 27-32 left out are added: Loom's governor decides the cases, validators as Commission effects, `--confinement`, the Codex context files, the replay after a grant, a denial over a grant, the sweep hold, metaharness 0.11.0, loom-sdk 0.34.0 | each from its commit since 0.7.0 |
| gate | `task check` exit 0 on the release commit before the tag; `release.yml` gates the tagged commit again | |
| after the tag | `[TRIAL takt] 0.8.0 <sha>` to the trial agent once the GitHub Release and its assets are verified: the diff since 0.7.0 touches `crates/**` | `AGENTS.md` § Messages |
