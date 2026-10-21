---
format: aep.planning-md/3
id: story:the-protocol-viewer-installs-without-scripts
kind: story
status: draft
title: The protocol viewer installs its npm packages with install scripts off
tags:
- finding
refs:
- provider: finding
  reference: the-protocol-viewer-installs-without-scripts
relations:
- decomposes: epic:composable-agentic-work
revision: 1
---
## Outcome

The protocol viewer installs its npm packages with install scripts off, as the website build does.

## Context

`docs/site/publish-protocols.sh:14` runs `npm ci --no-audit --no-fund` with install scripts on;
the site build runs `npm --prefix website ci --ignore-scripts` (`.github/workflows/pages.yml:48`,
`Taskfile.yml:52`), so a dependency's install script runs only here. Found by a read-only review of 2026-10-21 at 0a61e14a.

## Scope

- `docs/site/publish-protocols.sh`.

## Acceptance

1. The script runs `npm ci --ignore-scripts` and the viewer still builds: the build output.
