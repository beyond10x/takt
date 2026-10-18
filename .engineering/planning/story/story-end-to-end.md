---
format: aep.planning-md/3
id: story:story-end-to-end
kind: story
status: archived
title: 'M2: a planning-store story done end to end'
summary: the AEP frontier, a story protocol with gate, PR, CI and merge claims, the merge approval the only manual step
relations:
- decomposes: epic:composable-agentic-work
revision: 3
transitions:
- {from: "draft", to: "archived", at: "2026-10-18T01:43:14Z", actor: "human:timo", revision: 3}
---
## What

The frontier comes from the planning store: ready = status `proposed` (or `draft`), `blocked_by`
empty, every `depends_on` terminal; `aep plan artifact waves` keeps overlapping scopes apart. A
protocol for one story (in engineering-protocols) declares claims `tests.pass`, `gate.green`,
`pr.opened`, `ci.green`, `pr.merged` and the outcome `accepted`; validators observe the gate, the
pull request and CI through the repository's bot route and GitHub reads; the merge is an
`approval`. `--hours` and `--tokens` stop a run; AEP evidence is written on the story.

## Acceptance

One ready story of a small repository is taken by `takt run --repo R`, implemented, gated, opened
as a pull request and merged after one approval, and the story's `implemented` move carries a
`test_result` evidence record `takt` wrote; no other manual step occurred.
