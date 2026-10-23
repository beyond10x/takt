---
format: aep.planning-md/3
id: approval-record:release-0-9-0
kind: approval-record
status: approved
title: Release 0.9.0 from main at the wave 34 merge, Linux only, ESS 0.68
relations:
- decides: story:takt-uses-loom-session-executor
revision: 2
transitions:
- {from: "draft", to: "approved", at: "2026-10-23T02:13:33Z", actor: "human:timo", revision: 2}
---
# Approval: release 0.9.0

## Decision

The orchestrator, under the owner's delegation (2026-10-23): "release takt 0.9.0 from main
68f6b6d", because the harness comparison trial needs a takt release that carries Loom's session
executor. This is not the release that closes epic:takt-on-loom-sdk
(approval-record:takt-on-loom-sdk-closing-grant item 5); the epic stays open on
story:takt-harness-trait-retired.

## What the dispatcher decided under it

| item | decision | why |
|---|---|---|
| version | 0.9.0 | the error a refused or unreadable case ends with changes, and a session that writes outside its copy ends in an error instead of a `StepRefused` stop (CHANGELOG `[Unreleased]` at 68f6b6d) |
| revision | main at the wave 34 merge 68f6b6d, plus one release commit: version 0.8.0 to 0.9.0 in `Cargo.toml`, `Cargo.lock`, `fuzz/Cargo.lock`, CHANGELOG `[Unreleased]` dated as `[0.9.0]` | |
| targets | x86_64 and aarch64 Linux, as `release.yml` builds since 70919da | loom-sdk 0.35.0 builds on macOS, but the darwin run at 1215f27 has 105 failing tests (story:takt-confines-on-macos) |
| ESS | 0.67 to 0.68 in `ess/ess-inputs.yaml`, `check.yml` and `darwin.yml`, each SHA256 from the 0.68.0 release's `SHA256SUMS` | the newest ESS release is 0.68.0 |
| gate | the pull request's `check` run on the release commit, and `release.yml` on the tag | the local disk was 23G free at the release commit, under the 25G at which builds pause; the full `task check` at 59c523d passed (1657 passed, 0 failed, 5 ignored) and the release commit changes versions and docs only |
| after the tag | `[TRIAL takt] 0.9.0 <sha>` to the trial agent once the GitHub Release and its assets are verified: the diff since 0.8.0 touches `crates/**` | `AGENTS.md` § Messages |
