---
format: aep.planning-md/3
id: story:a-command-forge-runs-the-configured-commands
kind: story
status: draft
title: A forge that runs the exact commands the run file names, as argv
summary: 'forge: command, placeholders per argv element, typed exits'
tags:
- priority:medium
relations:
- serves: vision:safe-agentic-coding
- decomposes: epic:downstream-story-delivery
scope:
- confidence: inferred
  path: crates/takt/src/forge.rs
- confidence: inferred
  path: crates/takt/src/forge/command.rs
- confidence: inferred
  path: crates/takt/src/main.rs
- confidence: inferred
  path: crates/takt/src/run_config.rs
- confidence: inferred
  path: crates/takt/tests
- confidence: inferred
  path: ess/domains/takt.yaml
revision: 2
---
# Story: a-command-forge-runs-the-configured-commands

## Outcome

`forge: command` is a forge adapter on the `Forge` trait of `story:scan-and-forge-are-adapters`. It runs the exact
commands the repository-local run file names, in a section `forge_commands`:

```
forge: command
forge_commands:
  open:   ["glab", "mr", "create", "--source-branch", "{branch}", "--target-branch", "{base}", "--title", "{title}", "--description-file", "{body_file}", "--yes"]
  read:   ["mr-state", "{number}"]
  checks: ["mr-checks", "{number}", "{head}"]
  merge:  ["mr-merge", "{number}", "{head}"]
  id_pattern: '/merge_requests/(\d+)'
  refused_exit: [3]
```

- **Arguments.** Each entry is an argv list. The placeholders `{branch}`, `{base}`, `{title}`, `{body_file}`,
  `{number}` and `{head}` are substituted inside their own element, and the argv runs through the forge `Runner`
  (`forge.rs:417-422`). No shell, so a title holding `;` or `$(...)` is one argument.
- **Output.** `open` must print the request URL or number, parsed by `id_pattern` (default: the last integer of the
  last URL on stdout). `read` prints JSON with `state` and `head`. `checks` prints JSON with each check's `name` and
  `state`. Output that cannot be parsed is a typed error naming the command and the field.
- **Exit codes.** 0 is ok. An exit in `refused_exit` is `ForgeRefused { command, exit }`. Any other exit is
  `ForgeFailed { command, exit }`. stderr goes to the state dir.
- **Missing commands.** An action the protocol needs whose entry is missing is refused at open, naming the entry.
- **Push.** It stays takt's scanned push. The command forge does not replace the scan.

## Context

- Operator, 2026-10-27: forges should be configurable by exact commands, for example to reference `glab` or another
  tool, because forges are adapters now.
- The seam: `Forge` (`crates/takt/src/forge.rs:328-397` on `story/scan-forge-adapters` 4d83760), with
  `open_pull_request`, `pull_request`, `checks` and `merge`, and the `Runner` trait.
- A downstream trial (2026-10-27) ran with `--forge none` and never exercised a forge.

## Acceptance

Each is a named conformance scenario run by `task check`, on a fake command that records its argv and replies from a
script.

1. `command_forge::opens_with_the_configured_argv`: the fake receives exactly the configured argv with placeholders
   substituted, and the parsed number is the request takt reads next.
2. `command_forge::a_title_is_one_argument`: a title holding `; rm -rf x` and `$(id)` arrives as one argv element, and
   nothing else runs.
3. `command_forge::unparseable_output_is_named`: `open` printing no URL gives a typed error naming `open` and
   `id_pattern`.
4. `command_forge::a_refused_exit_is_typed`: an exit in `refused_exit` gives `ForgeRefused`, and another non-zero exit
   gives `ForgeFailed`.
5. `command_forge::a_missing_entry_is_refused_at_open`: no `merge` entry under a protocol that merges is refused before
   the case opens.
6. `command_forge::merge_carries_the_head`: `merge` receives the head the checks were read on.

## Scope

- `crates/takt/src/forge/command.rs` (new)
- `crates/takt/src/forge.rs` (adapter registration)
- `crates/takt/src/run_config.rs` (`forge_commands` section)
- `crates/takt/src/main.rs` (`--forge command`)
- `ess/domains/takt.yaml` (the command forge and its errors, before code)
- `crates/takt/tests/` (conformance scenarios)

## Out of scope

- A live run against a real forge. It waits for the operator's identity choice.
