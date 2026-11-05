---
format: aep.planning-md/3
id: story:a-case-id-on-a-story-resume-is-refused
kind: story
status: draft
title: --case-id on a story resume is refused
summary: a --case-id given to a run that resumes a story's open case is refused, not dropped
tags:
- finding
relations:
- serves: vision:safe-agentic-coding
- decomposes: epic:composable-agentic-work
revision: 2
---
## Outcome

A `--case-id` given to a `takt run` that resumes an open case through `--story`, or with no case argument, is refused
with exit 1 and nothing changed, as `--case-id` beside `--case` is.

## Context

Found by wave 54's adversary pass (`w54_d_a_case_id_on_a_story_resume_is_refused` in
`crates/takt-cli/tests/w54_adversary_cli.rs`): `takt run --story story:probe --case-id zzz`, with the story's case `a`
open, exited 3 with "frontier: resumed story:probe: probe (case a)" and dropped `--case-id zzz` without a word. Wave
54 refuses `--case-id` only beside `--case` (`crates/takt-cli/src/main.rs`, before `case_to_resume`). Very likely
older than wave 54; the base was not run.

## Acceptance

With story `probe`'s case `a` open, `takt run --story story:probe --case-id zzz` exits 1 naming `a`, and no case `zzz`
and no branch `takt/zzz` exist.
