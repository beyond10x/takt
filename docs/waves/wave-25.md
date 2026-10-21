# Wave 25: the site's defects of the 2026-10-20 screen

Stage: **closing**. Shape: one dispatcher in one checkout, one wave branch `wave/25` cut from
`main`, one sub-agent at a time, units grouped by cause, not by finding. The site deploys from
`main` (`.github/workflows/pages.yml`); this wave is no release of takt.

## Why

The owner asked for every defect of the site screen of 2026-10-20 to be fixed (44 defects: 6
high, 14 medium, 24 low; 12 duplicates close with their primary), relayed by the orchestrator.

## Units

| unit | cause | defects (primaries) | where |
|---|---|---|---|
| 1 | content contradicts the code, the CHANGELOG or `takt --help` | s5-01, s5-02, s5-04, s4-02, s5-05, s5-09, s5-08, s5-10, s5-15, s5-11 | `website/docs/**`, `website/product.json`, `website/scripts/generate.mjs` |
| 2 | generated reference blocks and code spans | s4-01, s2-04, s2-09, s5-14 | `website/scripts/generate.mjs`, `website/src/css/custom.css` |
| 3 | tables, charts and code blocks wider than their column | s1-08, s2-01, s1-12, s2-05, s2-06, s1-09 | `website/src/css/custom.css`, `website/src/components/Chart.tsx`, the pages |
| 4 | the protocol viewer | s3-01, s3-02, s4-07, s4-10 | `docs/site/protocols/` |
| E | the trial agent's candidate `trial-cand/2026-10-20-fd395d22-2` at fafe582d (story:step-spends-a-request-reading-its-scope-files): the Claude route names the intent's scope files as `@path` mentions; Claude median 52,504 vs 73,788 tokens | its review approve-on-conditions at b9b3f278, conditions met at fafe582d | `crates/takt/src/harness.rs`, ESS |
| 5 | the shared template (`@beyond10x/docs-system`): navbar, footer, home cards, status list, head, 404 | the rest | takt's overrides where the template cannot be changed from takt; the template's own file named otherwise |
| fix 1 | the adversarial review at 27782c0e: C1, C2 and five lows | s5-02 (the 0.2.3 row), s2-02 and s2-03 on the viewer, s4-10, s4-04 on the viewer, s2-06 (the scroll cue) | `website/`, `docs/site/protocols/` |

## Log

- 2026-10-20: wave opened from `main` at 7f2843b; unit 1 dispatched.
- 2026-10-20: unit 1 committed in 5eddc10: ten content defects (twelve ids with duplicates);
  `generate.mjs` reads the command list from `takt --help`, so `view` and `replay` got their
  sections; `npm run build` exit 0, `site-check` 24 pages at 2 widths, 0 failed.
- 2026-10-20: unit E merged in 742dfb0 (the trial agent's candidate fafe582d); citations
  1017/1017. Units 2 and 3 dispatched together (the generator's blocks and the layout).
- 2026-10-21: units 2 and 3 committed in fb736003 (18 ids with duplicates): code blocks wrap on
  screen with their copied text unchanged; entities escaped outside code spans only; words with
  hyphens kept whole by a rehype pass and theme overrides of inline code and code tokens (no CSS
  property stops Chrome breaking after a hyphen); the template's chip pass no longer turns kind
  words into chips; every table scrolls in its own frame; charts shrink to their column. A
  probe on all 23 pages at five widths leaves 9 flag breaks on the status page, in text the
  template's StatusTable prints. Unit 4 (the protocol viewer) dispatched.
- 2026-10-21: unit 4 committed in feea8fd1: the viewer reads the site's stored theme (the key
  Docusaurus namespaces under `future.v4`), redraws its graph per mode, has a full head (no
  favicon 404) and the site's navbar; `site-check` now fails a page on console errors, failed
  requests or an ignored stored theme. Unit 5 (the template-level defects) dispatched.
- 2026-10-21: unit 5 committed in 27782c0e (21 ids with duplicates; s1-03 left to the template):
  a probe of 66 checks against the served build, 37 red before, 1 after; `site-check` fails
  text under 12 px. Gate at 27782c0e: `task check` exit 0, 1603 passed, 0 failed, 6 ignored,
  citations 1017/1017. The adversarial review dispatched at 27782c0e.
- 2026-10-21: adversarial review at 27782c0e: approve on conditions C1 (the status release
  table skips 0.2.3) and C2 (the viewer navbar wraps and truncates at 997-1009 px), lows L1-L7;
  fix round 1 dispatched with C1, C2, s4-10, L1-L6 and the chart scroll cue of s2-06; L7 and
  the og image check deferred.
- 2026-10-21: fix round 1 committed in a7936c51: the 0.2.3 row; the viewer navbar spans the
  window, does not wrap and matches the docs navbar within 1.5 px from 997 to 1920 px; the CLI
  pages name the commit whose help they show; print keeps every column; a word joiner keeps
  flags whole in story titles; the viewer carries the social image and its tables scroll in
  their own frame; `site-check` 52 checks, 0 failed (2 failed on 27782c0e). s2-06 keeps its
  titles and legends inside the scrolling SVG: carried as story:the-runs-charts-read-on-a-phone.
- 2026-10-21: close. story:step-spends-a-request-reading-its-scope-files and
  story:the-site-passes-its-screen-of-2026-10-20 implemented on test_result; s1-03 left to the
  template (`@beyond10x/docs-system` 0.10.0: `dist/family.js:11,15`,
  `dist/product-palette.js:23`).
