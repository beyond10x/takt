---
format: aep.planning-md/3
id: story:docs-site-with-sidebar
kind: story
status: draft
title: The takt site is a beyond10x docs site with a sidebar, its pages generated from records
relations:
- decomposes: epic:composable-agentic-work
- serves: vision:safe-agentic-coding
revision: 1
---
## Outcome

https://beyond10x.github.io/takt/ is a Docusaurus site in the beyond10x docs style (`@beyond10x/docs-system`, `withProductSite`, a landing from `website/product.json`) with a sidebar: Start here (index, getting started, status), Concepts (protocol and case, one step at a time, evidence and the judge, roles), Guides (run a story, approve and resume, scenario run, variety run, read a run report), Self-improvement (the loop, scorecard, turns, runs, timeline, methodology: measured, derived, not recorded) and Reference (CLI, exit codes, `events.jsonl`, `takt-trace/1`, metrics fields, protocols). A `crates/takt-docs` generator writes the status data, the CLI reference and the metrics images under `website/`; `takt-docs generate --check` runs in `task check` and fails on drift. `/takt/protocols/` keeps its URL.

## Context

From the owner's ask of 2026-10-19 (several reporters over one trace, the journey image on the public site, a report per run, more self-improvement on the site, the beyond10x docs style with a sidebar) and the read-only design written for it the same day. The site is one hand-written `index.html` on `gh-pages` (story:public-site-content: "It stays a static page with no build framework"; this story replaces that sentence). `journey.svg` is shown only in the pitch deck, not on the site. The ess and conductor repositories run the same stack (`website/docusaurus.config.ts`, `website/product.json`, `.github/workflows/pages.yml`). The generator crate is code, so the developer builds it; the doc-writer owns `website/docs/**` and `product.json`.

Open, owner's decision (decision-blocker:site-publishing-path): publish by the doc-writer pushing `gh-pages` (the `AGENTS.md` § Roles sentence as it is), or by the sibling repositories' Pages-from-artifact workflow deployed from `main` (that sentence changes).

## Scope

- `website/` (new): config, sidebars, `product.json`, first pages (inferred)
- `crates/takt-docs/` (new): `generate`, `generate --check`, `provenance` (inferred)
- `.github/workflows/`: the pages workflow, per the publishing decision (inferred)
- `Taskfile.yml`: `site-build`; `check` gains `generate --check` (inferred)
- `README.md`, `AGENTS.md` § Layout and § Commands (inferred)
- `docs/site/protocols/`: the viewer built into `website/static/protocols/` (inferred)

## Acceptance

`npm --prefix website ci && npm --prefix website run build` exits 0; `task check` is green and fails after a hand edit of a generated file; the built site has a sidebar with the five sections above; after the first deploy `/takt/` and `/takt/protocols/` both answer 200.
