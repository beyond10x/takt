---
format: aep.planning-md/3
id: approval-record:release-0-13-1
kind: approval-record
status: approved
title: 'Release 0.13.1 from main at 7c4295a: --endpoint-token-env staging on macOS'
tags:
- loom-agent
revision: 3
transitions:
- {from: "draft", to: "approved", at: "2026-10-26T17:28:24Z", actor: "human:timo", revision: 2}
---
# Approval: release 0.13.1

## Decision

The orchestrator, under the owner's grant (2026-10-26), the item that opened wave 37: "Release per
takt's process, following the 0.12.0 pattern", and on the failed 0.13.0 run: "Continue from where
you stopped: apply the code fixes, then the gate, PR, release as planned."

0.13.0 was tagged on `main` at 7c4295a (approval-record:release-0-13-0). Its release run,
40364113239, failed in `darwin.yml` on the tagged commit, so publish was skipped and 0.13.0 has no
GitHub Release. A tag is not moved, so the fix ships as 0.13.1.

## What the dispatcher decided under it

| item | decision | why |
|---|---|---|
| version | 0.13.1, no `**Breaking:**` line | two Fixed bullets; nothing a caller names changed |
| revision | `main` at 7c4295a plus one release commit on `release/0.13.1` | |
| fix | staging resolves the links in a candidate's ancestors before the token-file rule (`resolved_parent` in `crates/takt/src/harness.rs`), so macOS's temporary directory under `/var`, a link to `/private/var`, passes; a candidate that is itself a link is still refused; a new unit test fails without it | three `harness::endpoint_tests` failed on macOS with "/var is a symbolic link" |
| tests | three tests in `crates/takt-cli/tests/endpoint_token_env_adv1.rs` run on Linux only: two read `/proc`, and two write outside the tree from a test command, which the macOS test sandbox refuses ("Operation not permitted") | two of them failed on macOS; the third read `/proc` and passed there without testing anything |
| ESS | `takt.work.EnvironmentVariable` names the resolved ancestors and cites `resolved_parent` | |
| gate | `task check` on the release tree, the pull request's checks, `darwin.yml` dispatched on `release/0.13.1` before the tag, then the merge into `main` and `release.yml` with `darwin.yml` on the tag | 0.13.0's gate ran no macOS job before the tag |
| image | rebuilt locally from a clone at the 0.13.1 tag as `b10x/takt:0.13.1`, never pushed | |
| trial | the trial named for 0.13.0 moves to 0.13.1 | 0.13.0 has no binaries |

## Released

| item | value |
|---|---|
| tag | `0.13.1`, annotated, tag object f7889dc, on `main` at 02a6806 (merge of `release/0.13.1` at c6be083, parents 7c4295a and c6be083), author and committer `b10x-bot[bot]` |
| gate | `task check` on the release tree: 169 results, 1798 passed, 0 failed, 6 ignored, exit 0; check-citations 1159 resolved, 0 unresolved; `release-notes --check` 19 releases |
| pull request | 20 from `release/0.13.1`: Build documentation, Security and privacy and the three task check jobs passed; `darwin.yml` dispatched on the branch (run 40370503690) passed before the tag; `release/0.13.1` and `release/0.13.0-2` deleted, both tips on `main` |
| release run | 40373175484 success at its first attempt: tag provenance, three builds, `task check` and `darwin.yml` on the tagged commit, publish |
| GitHub Release | https://github.com/beyond10x/takt/releases/tag/0.13.1, not a draft: `takt-0.13.1-x86_64-unknown-linux-gnu.tar.gz`, `takt-0.13.1-aarch64-unknown-linux-gnu.tar.gz`, `takt-0.13.1-aarch64-apple-darwin.tar.gz`, `SHA256SUMS` |
| verification | verify-release exit 0, then exit 0 with the announcement file; the runs on 02a6806 (Release, check, Documentation validation, Documentation site, two Shared source gates) success |
| image | `b10x/takt:0.13.1` built locally from a clone at the tag with `REVISION` 02a6806, never pushed; as uid 10001: `takt 0.13.1 (02a680699990abb1e0eaed7e16e7c2773030a5d4)`, b10x-gates 0.1.23, cargo 1.99.1, b10x-harness 0.20.0, aep 0.77.1, ess 0.69.2, `/opt/rust` 755 root, tini 0.19.0; the `b10x/takt:0.13.0` image removed |
| trial | `[TRIAL takt] 0.13.1 02a6806` is due; the orchestrator, which directs this item, routes it |
