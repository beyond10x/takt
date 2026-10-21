---
format: aep.planning-md/3
id: story:the-site-passes-its-screen-of-2026-10-20
kind: story
status: implemented
title: The public site passes its screen of 2026-10-20
summary: the 44 defects of the 2026-10-20 site screen, fixed in wave 25; s1-03 left to the docs template, part of s2-06 carried
relations:
- decomposes: epic:composable-agentic-work
- serves: vision:safe-agentic-coding
revision: 5
transitions:
- {from: "draft", to: "proposed", at: "2026-10-21T00:08:03Z", actor: "human:timo", revision: 3, decided_on: {"recorded":{"test_result":1}}}
- {from: "proposed", to: "active", at: "2026-10-21T00:08:04Z", actor: "human:timo", revision: 4, decided_on: {"recorded":{"test_result":1}}}
- {from: "active", to: "implemented", at: "2026-10-21T00:08:04Z", actor: "human:timo", revision: 5, decided_on: {"recorded":{"test_result":1}}}
---
# The site passes its screen of 2026-10-20

## What was wrong

A screen of the public site on 2026-10-20 (five finding sets, s1 to s5, at 390 to 1440 px, light and dark)
found 56 defects: 44 after 12 duplicates closed with their primary (6 high, 14 medium, 24 low).
Content contradicted the code, the CHANGELOG and `takt --help`; generated code blocks, tables and
charts ran past their column; the protocol viewer ignored the site's theme; and the shared docs
template (`@beyond10x/docs-system` 0.10.0) laid out the navbar, the footer, the home cards, the
head and the 404 page in ways takt's pages did not fit.

## What changed (wave 25)

| unit | commit | ids (duplicates in brackets) | where |
|---|---|---|---|
| 1 content | 5eddc108 | s5-01, s5-02, s5-04, s5-15, s4-02 (s5-03), s5-09, s5-08, s5-10, s5-05 (s1-10), s5-11 | `website/docs/**`, `website/product.json`, `website/scripts/generate.mjs` (the command list from `takt --help`) |
| 2, 3 rendering and layout | fb736003 | s4-01 (s1-07, s2-07, s5-06), s1-09, s2-04 (s1-11, s5-07), s2-09 (s1-15, s5-16), s5-14, s2-01 (s1-13), s1-12, s2-05, s1-08, s2-06 | `website/scripts/generate.mjs`, `website/scripts/rehype-layout.mjs`, `website/src/css/custom.css`, `website/src/components/Chart.tsx`, `website/src/theme/` |
| 4 protocol viewer | feea8fd1 | s3-01, s3-02, s4-07, s4-10 | `docs/site/protocols/`, `website/scripts/site-check.mjs` |
| 5 template overrides | 27782c0e | s1-01 (s5-12), s1-02 (s5-13), s1-05, s1-06, s2-08, s2-10, s2-11, s4-08, s2-02, s2-03, s4-03, s4-09, s1-14, s1-04, s3-03, s4-04, s4-05, s4-06, s1-16 | `website/src/css/custom.css`, `website/docusaurus.config.ts`, `website/product.json`, `website/src/theme/NotFound/`, `website/scripts/og-image.mjs`, `website/static/img/og.png` |

| review fixes | a7936c51 | s5-02 (the 0.2.3 row), s2-02 and s2-03 on the viewer, s4-10, s4-04 on the viewer, s2-06 (the scroll cue) | `website/docs/status.mdx`, `website/scripts/generate.mjs`, `website/scripts/site-check.mjs`, `website/src/css/custom.css`, `website/src/components/Chart.tsx`, `docs/site/protocols/` |

`site-check` now fails a page on text under 12 px (chart text under 11 px), console errors, failed
requests, an ignored stored theme and a wrapped or cut navbar; it checks the viewer at 1440, 1024,
1000, 768 and 390 px and loads a missing path (404, title, noindex).

The adversarial review at 27782c0e approved on two conditions (the 0.2.3 row; the viewer navbar
at 997-1009 px), both fixed in a7936c51 with five of its lows: the CLI pages name the commit
whose help they show, the viewer carries the social image, print keeps every table column, flags
in story titles stay whole (a word joiner after each hyphen), and the viewer's tables scroll in
their own frame.

## Left to the template

- s1-03: the AEP card has no family styling because AEP is not in the template's product family
  (`dist/family.js:11` `FAMILY_ORDER`, `:15` `COPY`; `dist/product-palette.js:23`
  `PRODUCT_SIGNATURES`), and the data check refuses an unknown id (`dist/family.js:39-48`). The
  template change: an `aep` signature (name, mark, hue, ink), its place in `FAMILY_ORDER` and
  `COPY`, and its URL; takt then writes `{"id": "aep", "relation": "uses"}`.
- Flags in story titles: the template's `StatusTable` (`dist/product.js:212`) prints titles as
  plain text, outside takt's rehype pass; takt's data now carries a word joiner after each hyphen
  of a flag, which a copied title keeps. A template that rendered code spans in titles would not
  need it.
- s4-06: `/takt/404.html` answers 200 because GitHub Pages serves an existing file; the page now
  carries `noindex`.

## Carried

- s2-06, the rest: at 390 and 1024 px the runs charts scroll in a shaded frame (the finding's
  expected "visible scroll cue"), but their titles and legends are drawn inside the 960 px SVG by
  `docs/metrics/timeline.py` and `journey.py` and scroll with it (83 and 30 texts hidden at 390):
  story:the-runs-charts-read-on-a-phone.
- The viewer has no skip link (old); `website/static/img/og.png` is built by
  `website/scripts/og-image.mjs`, which no build step runs.

## Acceptance

- Each id above: its after-shot or command output from the deployed site at the finding's
  viewport and theme, or the reason it stays open.
