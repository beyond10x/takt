---
format: aep.planning-md/3
id: approval-record:metaharness-0-15-0
kind: approval-record
status: approved
title: metaharness 0.15.0 through loom 0.39.0, then release takt
revision: 4
transitions:
- {from: "draft", to: "approved", at: "2026-10-23T15:23:06Z", actor: "human:timo", revision: 2}
---
# Approval: metaharness 0.15.0 through loom 0.39.0, then release takt

## Decision

The orchestrator, under the owner's delegation (2026-10-23): "pin metaharness 0.15.0 (released and
verified 2026-10-23: tag = main f39e7739c, bot|bot, verify-release.sh exit 0; it accepts Claude
Code 2.1.309 and Codex 0.160.0), release takt through its own process".

takt reaches metaharness only through loom: `crates/loom-session-executor/Cargo.toml` pins
`metaharness` and `metaharness-b10x` at tag `0.13.0`, and cargo refuses a `[patch]` of the same
source ("patches must point to different sources"). The orchestrator then decided: "loom 0.39.0 is
the metaharness 0.15.0 pin with a **Breaking:** bullet naming the re-exported types that change
(RunSpec, FloorInputs, Credential, AuditReport; lib.rs:101), same as loom 0.37.0
(CHANGELOG.md:96). ... Go ahead with the loom release through its own process, then takt pins loom
0.39.0 and releases."

loom re-exports metaharness (`pub use metaharness;`, `crates/loom-session-executor/src/lib.rs:101`),
so the pin is a break in loom's public API and moves loom's minor version; loom's rules refuse a
`**Breaking:**` bullet on a patch tag.

## What follows from it

- With metaharness 0.15.0, `--harness claude` proposes on Claude Code 2.1.304, 2.1.305 and
  2.1.309, and `--harness codex` runs on Codex 0.153.4 and 0.160.0.
- takt pins `b10x-loom-sdk` tag `0.39.0` and releases through its own process.
- After the release the orchestrator starts the trial on the released binary on a stock install,
  so the dispatcher sends no `[TRIAL takt]`.

## Also decided

The orchestrator: "A, written as 'Decider: operator'". decision-blocker:gitlab-trial-project
carries the body line `Decider: operator`; the public repository names no person.

## What the dispatcher decided under it

| item | decision | why |
|---|---|---|
| version | 0.10.1 | no `**Breaking:**` line: takt names none of the types loom 0.38.0 and 0.39.0 mark breaking, and `task check` passes with no change to `crates/*/src` |
| tests | `crates/takt/tests/harness_parity.rs` holds Codex 0.143.0 as the version metaharness does not pin, was 0.160.0 | metaharness 0.15.0 pins 0.160.0, so the two H9 tests that ran a Codex other than the pinned one failed on it; 0.143.0 is metaharness's own off-pin control |
| CHANGELOG | `[0.10.1]` carries a summary line, one Changed and two Fixed bullets | the release notes render the summary and Changed only |
| ESS | stays 0.68 | the newest ESS release is 0.68.0 |
| gate | `task check` exit 0 (1672 tests passed, 0 failed), `release-notes --check`, `check-citations --require-digests` (1056 resolved), the pull request's checks, then `release.yml` with `darwin.yml` on the tag | |

## Released

| item | value |
|---|---|
| loom | tag `0.39.0`, annotated, tag object fa37ef05, on loom `main` at 380e6315, author and committer `b10x-bot[bot]`; `release` run 39984749588 success; https://github.com/beyond10x/loom/releases/tag/0.39.0 by the bot |
| takt tag | `0.10.1`, annotated, tag object 3e14766, on `main` at 54b8dd8 (merge of `release/0.10.1` at 8090901), author and committer `b10x-bot[bot]` |
| release run | 39988441680 success: tag provenance, three builds, `darwin.yml` (test-cli, test-library) and `task check` on the tagged commit, publish |
| GitHub Release | https://github.com/beyond10x/takt/releases/tag/0.10.1, not a draft: `takt-0.10.1-x86_64-unknown-linux-gnu.tar.gz`, `takt-0.10.1-aarch64-unknown-linux-gnu.tar.gz`, `takt-0.10.1-aarch64-apple-darwin.tar.gz`, `SHA256SUMS` |
| trial | started by the orchestrator on the released binary on a stock install; the dispatcher sent no `[TRIAL takt]` |
