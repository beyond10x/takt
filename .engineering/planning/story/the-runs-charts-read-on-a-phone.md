---
format: aep.planning-md/3
id: story:the-runs-charts-read-on-a-phone
kind: story
status: draft
title: The runs charts read on a phone
summary: titles and legends of the runs charts outside the scrolling plot at 390 and 1024 px
relations:
- decomposes: epic:composable-agentic-work
- serves: vision:safe-agentic-coding
revision: 1
---
# The runs charts read on a phone

## What is wrong

The two charts of the runs page (`docs/metrics/timeline.svg`, `docs/metrics/journey.svg`) are
drawn 960 px wide by `docs/metrics/timeline.py` and `docs/metrics/journey.py`, with their titles
and legends inside the SVG. The site shows them in a frame that shrinks with its column down to
885 px and then scrolls, with a shaded edge as the cue (wave 25, fb736003 and a7936c51). At 390
px, 83 of 120 texts of the first chart and 30 of 118 of the second are outside the frame until the reader
scrolls sideways, the title and the legend among them; at 1024 px, 33 and 10
(`charts.cjs` of the wave 25 review, after a7936c51; finding s2-06 of the 2026-10-20 site screen).

## What would change

The generators write the title and the legend apart from the plot (as HTML beside the chart, or
as a second, narrow SVG), so that only the plot scrolls. The chart text floor of 11 px stays.

## Acceptance

- At 390 and 1024 px on the runs page, each chart's title and every legend entry are inside the
  viewport without a sideways scroll.
- `npm run site-check` exit 0.

Owner of the change: the doc-writer (`docs/**`).
