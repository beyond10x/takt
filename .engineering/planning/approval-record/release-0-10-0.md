---
format: aep.planning-md/3
id: approval-record:release-0-10-0
kind: approval-record
status: approved
title: Release 0.10.0 from main at the wave 35 merge, with the macOS binary and loom 0.37.0
relations:
- decides: story:takt-confines-on-macos
revision: 3
transitions:
- {from: "draft", to: "approved", at: "2026-10-23T10:35:22Z", actor: "human:timo", revision: 2}
---
# Approval: release 0.10.0

## Decision

The orchestrator, under the owner's delegation (2026-10-23): once `check`, Documentation
validation and Shared source gates conclude success on `main` at ec3ac9c (the wave 35 merge), cut
0.10.0 through takt's release process, with the macOS binary and the loom 0.37.0 pin. 0.9.0's
"no macOS binary" line no longer applies. story:sibling-git-dirs-are-sealed stays a draft for the
next wave. After 0.10.0 the orchestrator starts the takt trial on the released binary, so the
dispatcher sends no `[TRIAL takt]`.

## What the dispatcher decided under it

| item | decision | why |
|---|---|---|
| version | 0.10.0 | the Breaking lines on macOS under `[Unreleased]` at ec3ac9c |
| revision | `main` at ec3ac9c, plus one release commit: version 0.9.0 to 0.10.0 in `Cargo.toml`, `Cargo.lock`, `fuzz/Cargo.lock`, CHANGELOG `[Unreleased]` dated as `[0.10.0]`, README and site naming 0.10.0 and the macOS binary | |
| targets | x86_64 and aarch64 Linux, and `aarch64-apple-darwin` | `release.yml` builds all three since 9adc328, and `publish` needs `darwin.yml` |
| ESS | stays 0.68 | the newest ESS release is 0.68.0 (`gh release view -R beyond10x/ess`) |
| CHANGELOG | `[0.9.0]` is left as released; `[0.10.0]` says it ships `aarch64-apple-darwin`, which 0.7.0 to 0.9.0 did not | a released section is history |
| gate | the pull request's checks on the release commit, the three `main` runs at ec3ac9c, and `release.yml` with `darwin.yml` on the tag | |

## Released

| item | value |
|---|---|
| tag | `0.10.0`, annotated, tag object 9f8214d, on `main` at 593e527 (merge of `release/0.10.0` at 17c2910), author and committer `b10x-bot[bot]` |
| release run | 39944747305 success: tag provenance, three builds, `darwin.yml` (test-cli, test-library) and `task check` on the tagged commit, publish |
| GitHub Release | https://github.com/beyond10x/takt/releases/tag/0.10.0, not a draft: `takt-0.10.0-x86_64-unknown-linux-gnu.tar.gz`, `takt-0.10.0-aarch64-unknown-linux-gnu.tar.gz`, `takt-0.10.0-aarch64-apple-darwin.tar.gz`, `SHA256SUMS` |
| trial | started by the orchestrator on the released binary; the dispatcher sent no `[TRIAL takt]` |
