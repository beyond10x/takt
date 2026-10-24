---
format: aep.planning-md/3
id: approval-record:release-0-11-1
kind: approval-record
status: approved
title: 'Release 0.11.1 from main at 91d36c4: loom 0.40.0, and the checks on ess 0.69.0'
revision: 3
transitions:
- {from: "draft", to: "approved", at: "2026-10-24T05:20:03Z", actor: "human:timo", revision: 2}
---
# Approval: release 0.11.1

## Decision

The orchestrator, under the owner's grant (2026-10-24): "release takt 0.11.1: move the loom pins
to loom 0.40.0 (released, orchestrator verify-release exit 0) and release through takt's own
process", "if takt pins ESS crates or the ess CLI, move them to 0.69.0 in the same release", and
"keep the gates baseline at f7ecb80".

loom 0.40.0 (its CHANGELOG): requires ESS 0.69.0; the session executor reads a b10x-harness action
line streamed in several `text-delta` events as one line (loom's story:b10x-text-deltas, from
takt-trial cycle 0118 on takt 0.11.0, which ended with "the session ended without the JSON line
naming its action"); breaking: `model::run::SelectActionOutcome` gains `SelectionExists`, which
takt names nowhere (`git grep SelectActionOutcome -- crates`: nothing).

## What the dispatcher decided under it

| item | decision | why |
|---|---|---|
| version | 0.11.1 | no `**Breaking:**` line: takt names no type loom 0.40.0 marks breaking, and `task check` passes with no change to `crates/*/src` |
| revision | `main` at 91d36c4 plus one release commit: `b10x-loom-sdk` tag 0.39.1 to 0.40.0 in `Cargo.toml`, `Cargo.lock` and `fuzz/Cargo.lock` (13 loom entries each at 0.40.0, loom 858643a6, none at 0.39.1); the workspace version 0.11.0 to 0.11.1; CHANGELOG `[0.11.1] - 2026-10-24` with a summary line, two Changed and one Fixed bullet, and the compare links; README, getting-started and status name 0.11.1 and loom 0.40.0; status gets a 0.11.1 row | |
| ESS | takt pins no ESS crate (`Cargo.lock` holds none); the `ess` the checks install moves 0.68.0 to 0.69.0 in `check.yml` and `darwin.yml`, each SHA256 from the 0.69.0 release's `SHA256SUMS` | the orchestrator's instruction; the newest ESS release is 0.69.0 (`gh release view -R beyond10x/ess`) |
| loom | 0.40.0 with metaharness 0.15.0 | loom 0.40.0's `Cargo.lock` holds metaharness 0.15.0 |
| citations | the three `read from: Loom 0.39.1` citations in `ess/domains/takt.yaml` stay as they are: `crates/loom-session-executor/src/lib.rs` lines 257-260, 460-467 and 820-830 read the same at 0.40.0 (`git diff 0.39.1 0.40.0` on that file: 18 insertions, none in those ranges) | a citation names the release it was read at, as the 0.28.0 and 0.35.0 ones do |
| design | `docs/design/takt.md` names loom 0.40.0 in its crate table, the doc-writer's edit in a commit of its own; line 108 keeps `(loom 0.39.1)`, the release that introduced the secret route | |
| gate | `task check` in the release tree with a private `ess` 0.69.0 on the path: 160 results, 1733 passed, 0 failed, 5 ignored, exit 0; check-citations: 1127 resolved, 0 unresolved; `release-notes --check`: CHANGELOG.md: 16 releases in Keep a Changelog 1.1.0 form; then the pull request's checks, the merge into `main`, and `release.yml` with `darwin.yml` on the tag | the gates baseline stays f7ecb80 |
| trial | the diff against the latest promotion record's revision touches `Cargo.toml`, the lockfiles and `.github/workflows/`, so the dispatcher sends `[TRIAL takt] 0.11.1 <sha>` to the trial agent after the tag, naming the orchestrator's harness-arm trial so one cycle runs, not two | `AGENTS.md` § Messages |

## Released

| item | value |
|---|---|
| tag | `0.11.1`, annotated, tag object 5eec383, on `main` at 0a369ab (merge of `release/0.11.1` at bca3951, parents 91d36c4 and bca3951), author and committer `b10x-bot[bot]` |
| pull request | 14 from `release/0.11.1` (5efa982 the design's loom 0.40.0, bca3951 the release commit): Build documentation, Security and privacy and the three task check jobs passed |
| release run | 40066850668 success at its first attempt: tag provenance, three builds, `task check` (check-static, test-cli, test-library) and `darwin.yml` (test-cli, test-library) on the tagged commit, publish |
| GitHub Release | https://github.com/beyond10x/takt/releases/tag/0.11.1, not a draft: `takt-0.11.1-x86_64-unknown-linux-gnu.tar.gz`, `takt-0.11.1-aarch64-unknown-linux-gnu.tar.gz`, `takt-0.11.1-aarch64-apple-darwin.tar.gz`, `SHA256SUMS` |
| verification | verify-release exit 0 with the announcement file, which the brain session posted; the six runs on 0a369ab (Release, check, Documentation validation, Documentation site, two Shared source gates) success |
| trial | `[TRIAL takt] 0.11.1 0a369ab` sent to the trial agent after the tag, naming the orchestrator's harness-arm trial so one cycle runs |
