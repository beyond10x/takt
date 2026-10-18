---
format: aep.planning-md/3
id: story:protocol-viewer
kind: story
status: draft
title: A public viewer of the protocols, built from the protocol files
relations:
- decomposes: epic:composable-agentic-work
- serves: vision:safe-agentic-coding
scope:
- confidence: cited
  path: docs/site/protocols
- confidence: cited
  path: docs/site/publish-protocols.sh
revision: 2
---
# Story: protocol-viewer

## Outcome

Every protocol takt runs under is shown on the public site as tables and a graph (actions, evidence, claims, capabilities, obligations, outcomes), built from the `protocol/1` files themselves, so a rebuild after a protocol change shows the change with nothing restated by hand.

## Context

Owner request of 2026-10-18: "visualize the protocols, make a simple viewer for them. source of truth IS the protocol itself. when we update the protocol and rebuild the website, it must be updated too", with an extensible web framework (Vue). Built as a Vue 3 + Vite app in `docs/site/protocols/`; `scripts/collect.mjs` reads `protocols/*/*.yaml` at build time; `docs/site/publish-protocols.sh` rebuilds and publishes it at `/takt/protocols/` on gh-pages.

## Scope

- `docs/site/protocols/**`, `docs/site/publish-protocols.sh` (cited)
- gh-pages: `protocols/` (cited)

## Acceptance

https://beyond10x.github.io/takt/protocols/ lists the three protocols of `protocols/` with their graphs, and a changed protocol file shows its change after `docs/site/publish-protocols.sh`, with no other edit.
