---
format: aep.planning-md/3
id: story:docs-and-spec-true
kind: story
status: implemented
title: Docs and the ESS domain document say what the code does
summary: Fix 66 stale citations, design status, README, AGENTS.md
tags:
- wave-06
relations:
- decomposes: epic:composable-agentic-work
- serves: vision:safe-agentic-coding
- depends_on: story:tree-hygiene
- depends_on: story:validator-confinement
- depends_on: story:authority-hardening
- depends_on: story:cli-protocol-defaults
- depends_on: story:loop-test-gaps
scope:
- confidence: cited
  path: AGENTS.md
- confidence: cited
  path: README.md
- confidence: cited
  path: docs/design/takt.md
- confidence: cited
  path: ess/domains/takt.yaml
revision: 9
transitions:
- {from: "draft", to: "proposed", at: "2026-10-18T09:49:59Z", actor: "human:timo", revision: 6}
- {from: "proposed", to: "active", at: "2026-10-18T09:49:59Z", actor: "human:timo", revision: 7}
- {from: "active", to: "implemented", at: "2026-10-18T12:01:24Z", actor: "human:timo", revision: 9, decided_on: {"recorded":{"test_result":1}}}
---
# Story: docs-and-spec-true

## Outcome

The design document, README, AGENTS.md and the ESS domain document say what the code does after this wave, and every `read from:` citation lands on the line it names.

## Context

Findings of the 2026-10-18 code review, widened by an independent re-check:

| # | defect | where |
|---|---|---|
| 21 | the status line says nothing past M1 is built; JSONL events, `--until`, `--workflow` are described and absent | `docs/design/takt.md:3`, `takt.md:35-44`, `README.md:9` |
| 22 | 59 of 234 in-repository `read from:` citations land on unrelated lines and 7 are off by one or two | `ess/domains/takt.yaml` |
| — | `AGENTS.md` says the library holds "workflow types"; none exist | `AGENTS.md:11` |
| — | two citations of `.engineering/planning/story/takt-domain-model.md:42` mean line 43 | `ess/domains/takt.yaml:76`, `takt.yaml:536` |

This story runs after the code units of its wave have merged, so the citations are checked against the final code.

## Scope

- `ess/domains/takt.yaml` (cited)
- `docs/design/takt.md`, `README.md`, `AGENTS.md` (cited)

## Acceptance

`ess specify validate --path ess` exits 0; a script that resolves every `read from:` citation in `ess/domains/takt.yaml` finds each cited line containing the cited symbol or text; the design status line and README name what is built per milestone and mark unbuilt features as unbuilt; `AGENTS.md` names only modules that exist.

## Landed (wave 06)

Read from `git diff --stat 38eba58 cfd03d4` at the close of wave 06 (fix branch head 95995b4).

- `AGENTS.md`
- `README.md`
- `docs/design/takt.md`
- `ess/domains/takt.yaml`
