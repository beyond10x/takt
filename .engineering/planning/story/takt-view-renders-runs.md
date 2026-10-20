---
format: aep.planning-md/3
id: story:takt-view-renders-runs
kind: story
status: implemented
title: takt view writes one offline HTML file that renders a set of runs, one lane each
summary: 'the owner''s direction of 2026-10-20: read the trials'' rollouts in a small offline web app inside takt'
tags:
- process
relations:
- decomposes: epic:recursive-self-improvement
- serves: vision:safe-agentic-coding
revision: 4
transitions:
- {from: "draft", to: "proposed", at: "2026-10-19T23:24:24Z", actor: "human:timo", revision: 2}
- {from: "proposed", to: "active", at: "2026-10-19T23:24:24Z", actor: "human:timo", revision: 3}
- {from: "active", to: "implemented", at: "2026-10-20T00:08:24Z", actor: "human:timo", revision: 4, decided_on: {"recorded":{"test_result":1}}}
---
# Story: takt-view-renders-runs

## Outcome

`takt view <dir>` writes one offline HTML file that renders a set of runs: one lane per run on a
shared time axis, each `takt-trace/1` operation a mark on its lane, and a click on a mark opens
that step's transcript. The trial loop and a person read a cycle's rollouts in a browser with no
server and no network.

## Context

The owner's direction of 2026-10-20: the trials' rollouts are read in a small web app inside takt,
styled after an MIT-licensed dashboard (its stylesheet, its layout, Vue from its vendored copy),
with that code's attribution kept. `docs/metrics/journey.py` stays as it is.

## Scope

- `viewer/` (new): the HTML template, the stylesheet, the script and the vendored Vue, with the
  MIT notice of the code it is derived from.
- `crates/takt-cli/src/main.rs`: the `view` subcommand; its code beside it in `crates/takt-cli/src/`
  or `crates/takt/src/`.
- A CLI test under `crates/takt-cli/tests/`.

## Acceptance

1. `takt view <dir> --output <file>` reads every `takt-trace/1` file and step transcript under
   `<dir>` and writes one self-contained HTML file; it opens offline (no request leaves the
   page): a test checks the file holds no `http://` or `https://` script or stylesheet source.
2. One lane per run on one time axis; each operation of the trace is a mark at its time; a mark
   names its step, action and outcome.
3. A click on a mark opens that step's transcript, read from the file the trace's step line names.
4. A directory with no trace is refused, exit 1, naming the directory, with nothing written.
5. The derived code carries its MIT notice in `viewer/`.
