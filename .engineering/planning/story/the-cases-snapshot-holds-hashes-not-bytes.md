---
format: aep.planning-md/3
id: story:the-cases-snapshot-holds-hashes-not-bytes
kind: story
status: draft
title: The cases snapshot holds large files by length and digest, not their bytes
summary: every file under .engineering/cases is read into memory before and after each step
tags:
- rsi
relations:
- decomposes: epic:recursive-self-improvement
- serves: vision:safe-agentic-coding
revision: 2
---
# Story: the-cases-snapshot-holds-hashes-not-bytes

## Outcome

The snapshot of `.engineering/cases` taken before and after each step holds a large file by its length
and sha256 and sets the original aside on disk for put-back, so a step's memory does not grow with the
size of the cases directory.

## Context

Found by the wave 15 transcripts review (2026-10-19): the snapshot (`run.rs:799-848`) reads every file
under `.engineering/cases`, for every case, into memory before and after each step. With 20 files of
16 MiB planted, peak RSS was 700,804 KiB against 56,040 KiB. Transcripts moved out of the directory
(story:step-transcripts-are-kept); any other large file there still costs its size.

## Acceptance

- With 20 files of 16 MiB under a case, a step's peak RSS stays within 2x of the empty case's: a test.
- A step that changes such a file is still undone byte for byte.
