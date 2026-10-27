---
format: aep.planning-md/3
id: story:gate-commands-find-programs-on-a-system-path
kind: story
status: draft
title: A gate command names its program without an absolute path or a PATH of its own
summary: gate commands find programs on a fixed system path, never the user's
relations:
- decomposes: epic:composable-agentic-work
- serves: vision:safe-agentic-coding
revision: 1
---
# Story: gate-commands-find-programs-on-a-system-path

## Outcome

A gates document on a fresh repository runs its commands by program name: a gate command names
`sh`, `git` or `cargo` without an absolute path and without a `PATH` of its own, and runs.

## Context

- The site's guide says so as a workaround (`website/docs/guides/run-a-story.md:27-28`, `:48`): "A
  gate's commands run with the environment cleared and no `PATH` lookup, so name the executable by
  its absolute path and give it a `PATH` yourself." A first `takt run` with a gates document
  written as a person writes one fails its gate for this reason alone.
- The commands are started by the engineering assertions library from the project file's admitted
  commands (`canon_engineering_assertions::collect`, `crates/takt/src/validator.rs:2435`); takt runs
  a failed command again with `env_clear()` and the command's own `env` (`validator.rs:2197-2201`).
- Credentials never enter a child's environment (`AGENTS.md` § Rules), so the user's `PATH` is not
  the answer either: a fixed system path, or the toolchain roots `--confinement substrate` already
  uses, is.
- Ambiguity: whether the default belongs in takt or in the assertions library (another
  repository); the story records which, with the reason, before code.

## Acceptance

Each item is a conformance scenario named in `ess/ess-inputs.yaml`, run by `task check`.

1. `gate/bare-program-name-runs`: a gates document whose command is `sh -c 'exit 0'` with no `env`
   makes `gate.run` pass; the same with `exit 1` fails it on the command's exit.
2. `gate/user-path-not-inherited`: a program found only on the parent's `PATH`, not on the fixed
   path, is not found by a gate command, and the refusal names the program and the path searched.
3. `gate/declared-path-wins`: a command whose `env` sets `PATH` runs with that `PATH`, as today.

## Scope

Inferred, not confirmed: `crates/takt/src/validator.rs` (`GateRun`), `crates/takt/tests/`, the
assertions library if the decision puts it there.
