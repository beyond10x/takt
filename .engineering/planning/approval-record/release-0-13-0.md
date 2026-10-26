---
format: aep.planning-md/3
id: approval-record:release-0-13-0
kind: approval-record
status: approved
title: 'Release 0.13.0 from main at 378159a with wave 37: takt as a Loom agent, loom 0.43.0'
tags:
- loom-agent
revision: 4
transitions:
- {from: "draft", to: "approved", at: "2026-10-26T16:17:41Z", actor: "human:timo", revision: 2}
---
# Approval: release 0.13.0

## Decision

The orchestrator, under the owner's grant (2026-10-26), the item that opened wave 37: implement the
takt gaps of the first kind-cluster trial, then "Release per takt's process, following the 0.12.0
pattern (approval-record:release-0-12-0)", building the image locally as `b10x/takt:<new version>`
and pushing it nowhere.

## What the dispatcher decided under it

| item | decision | why |
|---|---|---|
| version | 0.13.0, Breaking | the image's entrypoint reads `/config/agent.yaml`; `takt protocols` spells the ids as the registry does; `--harness harness` needs b10x-harness 0.20.0 (metaharness 0.19.0) |
| revision | `main` at 378159a plus wave 37 (`wave/37` at c99f46d: fourteen commits, every one `b10x-bot[bot]`) and the release commit, on `release/0.13.0`; the wave lands on `main` with the release's merge | one pull request runs the checks over both |
| loom | 0.43.0 with metaharness 0.19.0 and the harness crates 0.20.0 | approval-record:wave-37-loom-0-43-0 |
| image | `Dockerfile` built locally from a clone at the tag, never pushed: b10x-harness 0.20.0, b10x-gates 0.1.23, aep 0.77.1, ess 0.69.2, rust 1.99 under `/opt/rust` (not writable by the run), `tini` as the entrypoint | |
| ESS | `check.yml` and `darwin.yml` stay on ess 0.69.2, the newest release on 2026-10-26 | |
| gate | `task check` on the release tree, then the pull request's checks, the merge into `main` and `release.yml` with `darwin.yml` on the tag | |
| trial | the diff touches `crates/` and `Cargo.lock`, so a `[TRIAL takt] 0.13.0 <sha>` is due after the tag; the orchestrator, which directs this item, routes it | `AGENTS.md` § Messages |
| branches | `release/0.11.0` and `release/0.12.0` kept: neither tip is on `main` | approval-record:wave-37-engineering-protocols-0-10-0 § Branches |

## Rebuild

Pull request 18 from `release/0.13.0` failed `common / Security and privacy` on one finding: gitleaks' `generic-api-key` read a needle of `ess/domains/takt.citations.tsv` ending "API-key file" before its tab and `sha256:` digest as a key, in 6f244ac and the six later commits that change the file. `release/0.13.0-2` rebuilds the fifteen commits from `main` at 378159a through the bot with that needle shortened to end at "can pass an"; every tree is otherwise identical, and the wave page, this record and the commit messages name the rebuilt commits (the wave's last commit before this release is c99f46d, not on `wave/37`). Pull request 18 is closed unmerged and `release/0.13.0` kept; the review records and the stories' test results name the commits they were made at, which stay on it. gitleaks over `378159a..release/0.13.0-2`: no leaks.

## Outcome

The tag `0.13.0` (tag object 1530f43) is on `main` at 7c4295a, the merge of pull request 19. Release run 40364113239 failed: on the tagged commit, `darwin.yml` failed three `harness::endpoint_tests` ("/var is a symbolic link") and two `endpoint_token_env_adv1` tests ("Operation not permitted"). Publish was skipped, so 0.13.0 has no GitHub Release and no binaries. The tag stays; approval-record:release-0-13-1 ships the fix as 0.13.1.
