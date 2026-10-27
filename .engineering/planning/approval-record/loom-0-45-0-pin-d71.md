---
format: aep.planning-md/3
id: approval-record:loom-0-45-0-pin-d71
kind: approval-record
status: approved
title: takt pins the Loom release that takes metaharness 0.20.0 (D71)
revision: 2
transitions:
- {from: "draft", to: "approved", at: "2026-10-27T03:22:32Z", actor: "human:timo", revision: 2}
---
# Approval: takt pins the Loom release that takes metaharness 0.20.0 (D71)

## Decision

Session `b10x-chef-orchestrator`, 2026-10-27, decision D71, approved under the operator's grant
(class C: a dependency upgrade), on the operator's message of 2026-10-27 01:31Z about the codex
version check and the operator's rule "use cargo to use a local reference ... and then when it's
done, create the versions and PRs":

- Loom pins metaharness 0.20.0 and releases (a pin-only hop: no wave, no adversary); takt is
  patched onto that Loom tree first, its harness parity tests are rewritten with
  `story:codex-off-pin-proposes`, and a takt `run --harness codex` with codex 0.164.0 must reach
  `ApprovalRequired` as before.
- takt then pins the new `b10x-loom-sdk` on `integration/takt`: fetch, merge
  `origin/integration/takt`, the affected suites, a fast-forward push, never a force; the story
  is implemented with the pin commit. No takt pull request, tag or release.
- A build starts only with over 30G free on `/`; trees are finished with `--discard-cache`.
