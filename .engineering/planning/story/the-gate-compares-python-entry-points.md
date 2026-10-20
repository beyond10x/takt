---
format: aep.planning-md/3
id: story:the-gate-compares-python-entry-points
kind: story
status: implemented
title: The gate compares the Python files a directory or module argument runs
summary: python3 <dir> runs <dir>/__main__.py and python3 -m a.b runs a/b.py; neither was compared
tags:
- rsi
relations:
- decomposes: epic:recursive-self-improvement
- serves: vision:safe-agentic-coding
revision: 5
transitions:
- {from: "draft", to: "proposed", at: "2026-10-20T00:48:39Z", actor: "human:timo", revision: 2}
- {from: "proposed", to: "active", at: "2026-10-20T00:48:39Z", actor: "human:timo", revision: 3}
- {from: "active", to: "implemented", at: "2026-10-20T01:41:21Z", actor: "human:timo", revision: 5, decided_on: {"recorded":{"test_result":1}}}
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

- After a step edits `<dir>/__main__.py` for a gate command `python3 <dir>`, `gate.run` runs nothing
  and writes no record, naming the file: a test. (Reworded by the dispatcher, wave 18, as for
  story:the-gate-compares-its-scripts-and-tool-config-at-the-base: the gate does not run; the step
  guard is not changed.)
- The same for each file `-m a.b` resolves to: a test.
