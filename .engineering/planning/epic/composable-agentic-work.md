---
format: aep.planning-md/3
id: epic:composable-agentic-work
kind: epic
status: draft
title: 'Composable agentic work: one bounded, validated run at a time'
summary: A binary drives a repository through engineering protocols; Canon decides what is permitted, owed and earned
revision: 2
---
## Why

Safe agentic coding, applied. A YAML description of workflows made of engineering protocols and
building blocks (inner, outer and validation loops); a harness such as Claude Code drives a
workflow through the model; every step is validated by Canon over evidence, with AEP as the
record. One binary, pointed at a repository: it takes an intent or pulls the next unit from the
planning store and runs one iteration, three, or three hours. The design is `docs/design/takt.md`.

## Acceptance

`takt run` on a repository with a planning store and ready stories finishes one story with no
manual step but the merge approval, and every claim in the final decision document was established
by a validator's evidence record, none by the harness's text.
