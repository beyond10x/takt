---
format: aep.planning-md/3
id: story:turn-grants-are-bound-outside-the-case-dir
kind: story
status: draft
title: A turn's grants and recorded anchors are held where a step cannot write
summary: grants and anchors sit in the case directory, which same-user processes can write between runs
tags:
- rsi
relations:
- decomposes: epic:recursive-self-improvement
- serves: vision:safe-agentic-coding
revision: 1
---
# Story: turn-grants-are-bound-outside-the-case-dir

## Outcome

The grants a self-improvement turn reads (`takt approve`) and the anchors it records at open (the
candidate, the base, the judge's digest) are bound outside the writable case directory, or bound to
it by a digest held outside it, so a file planted in the case directory between runs is refused
rather than trusted.

## Context

Found by the review of story:a-judge-surface-candidate-merges-after-its-grant (2026-10-19) and the
earlier RSI part 1 reviews: grants and anchors are files under `.engineering/cases/<id>/`, which any
same-user process can edit between two runs of a turn; a forged grant or anchor is read as the
owner's.

## Acceptance

- A grant file written into the case directory by hand, not by `takt approve`, is refused on the next
  run of the turn, naming the file: a test.
- An anchor changed in the case directory between runs is refused the same way: a test.
- A turn whose files are untouched resumes as today.
