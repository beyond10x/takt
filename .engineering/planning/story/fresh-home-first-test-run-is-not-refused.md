---
format: aep.planning-md/3
id: story:fresh-home-first-test-run-is-not-refused
kind: story
status: active
title: The first tests.run in a fresh HOME is not refused for the cargo cache
summary: a run whose HOME has no .cargo yet stops StepRefused at its first tests.run (second kind-cluster trial, runs 1-5)
tags:
- loom-agent
relations:
- decomposes: epic:takt-as-a-loom-agent
- serves: vision:safe-agentic-coding
revision: 4
transitions:
- {from: "draft", to: "proposed", at: "2026-10-26T20:53:19Z", actor: "human:timo", revision: 3}
- {from: "proposed", to: "active", at: "2026-10-26T20:53:54Z", actor: "human:timo", revision: 4}
---
## Outcome

The first `tests.run` of a run whose `HOME` holds no `.cargo` yet is not refused for the cargo
cache it leaves there, so a run in a fresh container reaches its first test result without a
step done by hand beforehand.

## Context

- The second kind-cluster trial, 2026-10-26 (takt 0.13.1 as a `LoomAgent` of the `Command` variant on a local kind cluster,
  `HOME` an empty directory): runs 1 to 5 each stopped `StepRefused` at step 1, exit 3, 0 tokens:
  "tests.run wrote what its validator does not: /var/lib/agent/.cargo". The pod restarted takt
  four times with the same result.
- `program_dirs` (`crates/takt/src/run.rs`) watches `$CARGO_HOME/bin`, else `~/.cargo/bin`; when
  it does not exist, its nearest existing directory is watched, so creating `~/.cargo` is a
  change outside the tree.
- The trial's workaround, a wrapper before takt: `mkdir -p "$HOME/.cargo/bin" && cargo fetch
  --locked`. Runs 6 to 12 ran with it and no `tests.run` was refused.

## Acceptance

1. A run started with `HOME` an empty directory reaches a test result at its first `tests.run`
   (pass or fail), no `StepRefused` naming `~/.cargo`.
2. A program written into `~/.cargo/bin` by a step is still a change outside the tree and stops
   the run, as today.
