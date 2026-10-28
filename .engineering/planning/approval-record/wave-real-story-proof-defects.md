---
format: aep.planning-md/3
id: approval-record:wave-real-story-proof-defects
kind: approval-record
status: approved
title: A fix wave for the defects of the first real-story delivery
summary: four stories, the gate link fixed upstream, then proof on two repositories
relations:
- decides: story:story-delivery-2-evidence-writers
- decides: story:adopted-orphans-are-reaped-while-a-child-runs
- decides: story:gates-project-checked-at-open
- decides: story:bot-merge-commit-has-a-body
revision: 2
transitions:
- {from: "draft", to: "approved", at: "2026-10-28T13:11:30Z", actor: "human:timo", revision: 2}
---
# Approval: a fix wave for the defects of the first real-story delivery

## Decision

The orchestrator, under the owner's grant, 2026-10-28, after takt 0.17.0 delivered a story of beyond10x/tools only on `--protocol story.delivery/1` and repository-side workarounds. The owner, 2026-10-28: "what we're needing soon now actually is really proof that we can take this thing, point it to a story within the repo and see it solving that story."

| item | decision |
|---|---|
| the wave | story:story-delivery-2-evidence-writers, story:adopted-orphans-are-reaped-while-a-child-runs, story:gates-project-checked-at-open with story:bot-merge-commit-has-a-body; each a failing test first, then the fix, an adversary pass after green; on `integration/takt`, fast-forwarded to `main` first |
| the gate link | fixed where it is, in engineering-protocols (`command_paths`), released first; takt takes the pin with ESS 0.74.0 |
| release | takt with the new engineering-protocols pin |
| proof | the released binary, the default protocol, no `--protocol` and no `--test-cmd`, one real story in each of two other repositories, run 1 to the merge grant and run 2 to an accepted case |
| stop rule | the same stop reason twice, a red base at open, a gate failing twice on one cause |
