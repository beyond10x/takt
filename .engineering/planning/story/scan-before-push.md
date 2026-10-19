---
format: aep.planning-md/3
id: story:scan-before-push
kind: story
status: implemented
title: pr.open scans commit messages and the diff before it pushes
summary: scan-text over base..head messages and diff before push_revision; a refused scan leaves no branch on the remote
relations:
- decomposes: epic:composable-agentic-work
- serves: vision:safe-agentic-coding
scope:
- confidence: cited
  path: crates/takt/src/forge.rs
- confidence: cited
  path: crates/takt/src/validator.rs
- confidence: inferred
  path: crates/takt/tests/scan_before_push.rs
revision: 8
transitions:
- {from: "draft", to: "proposed", at: "2026-10-18T20:13:47Z", actor: "human:timo", revision: 5}
- {from: "proposed", to: "active", at: "2026-10-18T20:13:47Z", actor: "human:timo", revision: 6}
- {from: "active", to: "implemented", at: "2026-10-19T06:14:33Z", actor: "human:timo", revision: 8, decided_on: {"recorded":{"test_result":1}}}
---
# Story: scan-before-push

## Outcome

`pr.open` scans everything it is about to publish before anything leaves the machine: the commit messages of `base..head` and the diff, as well as the pull request's title and body. A refused scan leaves no branch on the remote.

## Context

Found by an independent code review of 2026-10-18, takt at 2a22093 (line numbers unchanged at 2ec88d4).

- `pr.open` pushes the case's branch first (`crates/takt/src/validator.rs:751-753`, `push_revision`) and only then opens the pull request, where scan-text runs over the request and the published title and body (`crates/takt/src/forge.rs:875-882`).
- The pushed commits carry model-written content and free-text subjects: a step's commit subject is `<action>: <summary>`, the summary the harness reported (`crates/takt/src/run.rs:1066`). None of it is scanned.
- A refused scan therefore leaves the branch public; the refusal says so (`forge.rs:878`), but the content is out.

## Expected before

On takt at 2ec88d4, holds; after the fix, fails:

- A test with a stub forge and a case whose step commit subject contains a literal the policy forbids: `pr.open` calls `push_revision` before any scan-text call, and no scan-text call's input contains the commit messages or the diff of `base..head`.

## Scope

- `crates/takt/src/validator.rs`: `PrOpen::observe`, a scan before `push_revision` (cited)
- `crates/takt/src/forge.rs`: a scan argv over `git log --format=%B base..head` and the diff (cited)
- `crates/takt/tests/`: a stub-forge call-order test (inferred)

## Acceptance

With a stub forge, `pr.open` runs scan-text over `git log --format=%B base..head` and the diff of `base..head` before `push_revision`, and when that scan refuses, `push_revision` is never called and the observation records nothing.

## Landed (wave 13)

Read from `git diff --name-only 136f53d^1 136f53d` (the unit's merge into the wave branch) at the close of wave 13 (wave branch head 16ff976). pr.open scans every commit a push would publish and the pull request's title and body before anything is pushed; a refused scan pushes nothing. From wave 12.

- `crates/takt-cli/tests/scenario_run.rs`
- `crates/takt/src/forge.rs`
- `crates/takt/src/git.rs`
- `crates/takt/src/validator.rs`
- `crates/takt/tests/approval_record.rs`
- `crates/takt/tests/approval_record_adv1.rs`
- `crates/takt/tests/approval_record_adv2.rs`
- `crates/takt/tests/ci_pending_waits.rs`
- `crates/takt/tests/ci_pending_waits_adv1.rs`
- `crates/takt/tests/ci_pending_waits_adv2.rs`
- `crates/takt/tests/forge.rs`
- `crates/takt/tests/forge_scan_policy.rs`
- `crates/takt/tests/forge_scan_policy_adv1.rs`
- `crates/takt/tests/merge_under_authority.rs`
- `crates/takt/tests/records_reach_base_adv1.rs`
- `crates/takt/tests/records_reach_base_adv2.rs`
- `crates/takt/tests/scan_before_push.rs`
- `crates/takt/tests/scan_before_push_adv1.rs`
- `crates/takt/tests/scan_before_push_adv2.rs`
- `ess/domains/takt.citations.tsv`
- `ess/domains/takt.yaml`
