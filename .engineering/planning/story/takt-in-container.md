---
format: aep.planning-md/3
id: story:takt-in-container
kind: story
status: draft
title: The harness's login never reaches the case, the events or takt's output
summary: a static takt binary and the harness CLI in the container; the login mounted read-only for the run, never recorded
relations:
- decomposes: epic:benchmark-integration
- serves: vision:safe-agentic-coding
- informed_by: story:prebuilt-binaries-in-ci
- depends_on: story:bench-task-protocol
scope:
- confidence: inferred
  path: Taskfile.yml
- confidence: inferred
  path: crates/takt/src/harness.rs
revision: 11
---
# Story: takt-in-container

## Outcome

takt keeps the harness's login out of everything it records. When the harness's reply contains the content of the login file that harness reads, no part of that content reaches the case directory, the events or takt's own output. Inside a benchmark container the login is mounted read-only for the run's duration, and that mount is part of the runners' setup. The static binary for the container is story:takt-static-binary.

## Context

Both external benchmarks run each task in its own container (design:benchmark-integration). product-requirements:takt R4 and R5 say credentials never enter a harness's prompt or a run record. The login is the one credential the harness needs.

In a container the login is a file, not a variable. The `claude` harness removes every `CLAUDE_CODE_*` variable from the child (`crates/takt/src/harness.rs:215-219`), so a token passed that way would never reach the harness. The harness reads its login under `$HOME/.claude/`, and the runners mount that file read-only (story:harbor-agent-adapter, story:swe-bench-mini-runner). Running the container needs Docker, so it is not part of the acceptance.

A search for the token that finds nothing proves nothing. It finds nothing before the work too, and when nothing ran. So the check plants a canary as the login and uses a double that echoes it back. Today takt writes a step's reply into `events.jsonl` as the step's `summary` (`crates/takt/src/events.rs:88-92`), so at d17bc32 the canary is found.

The old Expected before, the static musl build that no Taskfile or release target produces (`.github/workflows/release.yml:78-80` builds only gnu targets), moves to story:takt-static-binary.

## Expected before

On takt at main, holds; after the change, fails:

- With the canary login and the echoing `claude` double of the acceptance, `grep -rF takt-canary-7f3a` finds the canary in the case's `events.jsonl`.

## Scope

- `crates/takt/src/harness.rs` (inferred)
- `crates/takt/src/events.rs` (inferred)
- `crates/takt-cli/tests/login_not_recorded.rs` (inferred)

## Acceptance

With `HOME` set to a fixture home whose `.claude/.credentials.json` holds the canary `takt-canary-7f3a`, and a `claude` double first on PATH that copies that file's content into its reply's summary, `takt run` exits 0 with the double's step line in `events.jsonl`, and `grep -rF takt-canary-7f3a <case dir> <takt's captured stdout and stderr>` exits 1 after `takt run --harness claude --forge none` on the fixture task of story:bench-task-protocol, where at d17bc32 it exits 0 on the case's `events.jsonl`.
