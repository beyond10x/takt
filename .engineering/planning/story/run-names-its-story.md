---
format: aep.planning-md/3
id: story:run-names-its-story
kind: story
status: implemented
title: takt run --story names the story a run delivers
summary: a named story instead of the frontier's next, refused unless ready
tags:
- loom-agent
relations:
- serves: vision:safe-agentic-coding
- decomposes: epic:takt-as-a-loom-agent
scope:
- confidence: cited
  path: README.md
- confidence: cited
  path: crates/takt-cli/src/main.rs
- confidence: cited
  path: ess/domains/takt.yaml
revision: 7
transitions:
- {from: "draft", to: "proposed", at: "2026-10-26T13:48:05Z", actor: "human:timo", revision: 5}
- {from: "proposed", to: "active", at: "2026-10-26T13:48:05Z", actor: "human:timo", revision: 6}
- {from: "active", to: "implemented", at: "2026-10-26T16:16:36Z", actor: "human:timo", revision: 7, decided_on: {"recorded":{"test_result":1,"review_outcome":2}}}
---
# Story: run-names-its-story

## Outcome

An operator, or a `LoomAgent` resource, names the story a run delivers: `takt run --story <id>`
opens the case on that story instead of the frontier's next ready one.

## Context

- Trial: takt 0.12.0 takes only the frontier's next `proposed` story and has no `--story` flag;
  the trial moved `story:a-git-read-error-leaves-no-process` to `proposed` in the repository so
  the frontier would offer it, then resumed by `--case` from run 2 on.
- `takt scenario run` already opens a case on a named story (`take_story` in
  `crates/takt-cli/src/main.rs`) after checking it is ready by the frontier's rule.

## Acceptance

1. `takt run --story <id>` opens a case on `story.delivery/1` for that story and the store records
   it taken (`proposed` → `active`), as a frontier take does.
2. A story the store does not list, or one not ready by the frontier's rule (not `proposed`,
   blocked, or a dependency not done), is refused before any case opens, exit 1, naming the story
   and why.
3. `--story` with `--intent` or `--case` is refused by the command line.
4. Without `--story` the run takes the frontier's next ready story as before.

## Scope

- `crates/takt-cli/src/main.rs` (`RunArgs`, the run's unit selection), a CLI test under
  `crates/takt-cli/tests/`, `ess/domains/takt.yaml`, `README.md`.
