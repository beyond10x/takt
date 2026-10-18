---
format: aep.planning-md/3
id: story:pluggable-seams
kind: story
status: archived
title: 'M4: the five seams with a second harness'
summary: Harness, Frontier, Validator, Forge, Confinement as traits; a second harness gives the same decisions
relations:
- decomposes: epic:composable-agentic-work
revision: 3
transitions:
- {from: "draft", to: "archived", at: "2026-10-18T01:43:14Z", actor: "human:timo", revision: 3}
---
## What

Five traits in `b10x-takt`: `Harness` (the judgement actor), `Frontier` (the planning store),
`Validator` (an evidence producer), `Forge` (pull request, merge and release through the bot
route) and `Confinement` (none, Substrate later). A second harness adapter (Codex through llm)
proves the seam; validators come from the engineering assertion providers where one exists.

## Acceptance

The M2 story runs under the Claude Code adapter and under the second adapter, and the two runs'
final decision documents are byte-equal apart from evidence ids and instants.
