---
format: aep.planning-md/3
id: story:the-gate-compares-its-scripts-and-tool-config-at-the-base
kind: story
status: implemented
title: gate.run compares every script and tool config a gate reads at the base
summary: a step could edit scripts/check-citations to exit 0 and gate.green would hold
tags:
- rsi
relations:
- decomposes: epic:recursive-self-improvement
- serves: vision:safe-agentic-coding
revision: 4
transitions:
- {from: "draft", to: "proposed", at: "2026-10-19T20:05:24Z", actor: "human:timo", revision: 2}
- {from: "proposed", to: "active", at: "2026-10-19T20:05:25Z", actor: "human:timo", revision: 3}
- {from: "active", to: "implemented", at: "2026-10-19T23:22:53Z", actor: "human:timo", revision: 4, decided_on: {"recorded":{"test_result":1}}}
---
# Story: the-gate-compares-its-scripts-and-tool-config-at-the-base

## Outcome

`gate.run` compares at the base every file a gate command reads that a step could edit: every file
under `scripts/`, and the tool configuration files (`.cargo/config.toml`, `clippy.toml`,
`rustfmt.toml`, `Taskfile.yml`), not only the executables the project file names; a step that
changes one is refused, and the gate never runs an edited copy.

## Context

Found while delivering story:takt-has-a-gates-document (wave 15, 2026-10-19): `GateRun` compares
only the executables named in `.engineering/assertions-project.yaml` with the base
(`validator.rs:947`, `:782`). `scripts/check-citations` is an argument to `python3`, so a step could
edit it to exit 0 and `gate.green` would hold; the same for the tool configuration files, which the
test command checks at the base and the gate validator does not. The merge of such an edit still
needs the owner's grant (`scripts/**` is a judge surface), but `gate.green` must not hold on it.

## Acceptance

- A step that edits `scripts/check-citations` to exit 0 is refused before the gate runs, naming the
  file: a test.
- The same for each named tool configuration file.
- An unchanged repository's gate runs as today.
