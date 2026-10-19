---
format: aep.planning-md/3
id: story:docs-site-with-sidebar
kind: story
status: active
title: The takt site is a beyond10x docs site with a sidebar, its pages generated from records
relations:
- decomposes: epic:composable-agentic-work
- serves: vision:safe-agentic-coding
- depends_on: story:main-history-names-no-private-repository
revision: 6
transitions:
- {from: "draft", to: "proposed", at: "2026-10-19T01:34:31Z", actor: "human:timo", revision: 4}
- {from: "proposed", to: "active", at: "2026-10-19T04:15:44Z", actor: "human:timo", revision: 5}
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

`npm --prefix website ci && npm --prefix website run build` exits 0; `task check` is green; the generated pages (status, charts, CLI reference, exit codes, protocol viewer) are written by `website/scripts/generate.mjs` at build time and none is committed, so no generated file can be edited by hand (decided 2026-10-19 in place of a committed-output check); `b10x-gates scan-text` over every file of the built site exits 0; the built site has a sidebar with the five sections above; after the first deploy `/takt/` and `/takt/protocols/` both answer 200.

Visual acceptance (owner, 2026-10-19: the hand-written site "looks like garbage"; a headless-browser check of gh-pages 568ef20 at 1440 and 390 px found ten defects, among them 8 of 10 section links off-screen at 390 px, protocol-graph node text at about 5 px and overlapping chart labels): a `task site-check` screenshots every page at 1440 and 390 px wide with headless Chrome and fails when any page scrolls horizontally (`document.documentElement.scrollWidth > innerWidth`), when any HTML or SVG text renders below 11 px, or when two text boxes of one chart intersect; the screenshots go with every grant request that changes a page, and the reviewer looks at them.

## Publishing (decided 2026-10-19)

decision-blocker:site-publishing-path is cleared with option B: the site deploys from `main` through `.github/workflows/pages.yml` and the sibling repositories' reusable project-site workflow; the `AGENTS.md` § Roles sentence on `gh-pages` changes in the same unit, and the organisation site gets its one-time roster entry.
