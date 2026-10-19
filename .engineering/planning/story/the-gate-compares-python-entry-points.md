---
format: aep.planning-md/3
id: story:the-gate-compares-python-entry-points
kind: story
status: draft
title: The gate compares the Python files a directory or module argument runs
summary: python3 <dir> runs <dir>/__main__.py and python3 -m a.b runs a/b.py; neither was compared
tags:
- rsi
relations:
- decomposes: epic:recursive-self-improvement
- serves: vision:safe-agentic-coding
revision: 1
---
# Story: the-gate-compares-python-entry-points

## Outcome

`gate.run` compares at the base the files Python runs for a directory argument (`<dir>/__main__.py`)
and for `-m a.b` (`a/b.py`, `a/b/__main__.py` and the `__init__.py` files on the way), so a step
cannot change a gate command through them.

## Context

Found by the review of story:the-gate-compares-its-scripts-and-tool-config-at-the-base
(2026-10-19): edits to `tool/__main__.py`, `pkg/__main__.py` and `pkg/check.py` ran and recorded
pass. takt's own gates document is not affected; any repository whose gate runs Python this way is.

## Acceptance

- A step that edits `<dir>/__main__.py` for a gate command `python3 <dir>` is refused, naming the
  file: a test.
- The same for each file `-m a.b` resolves to: a test.
