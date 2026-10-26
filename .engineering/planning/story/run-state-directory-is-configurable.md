---
format: aep.planning-md/3
id: story:run-state-directory-is-configurable
kind: story
status: active
title: The run's state and transcript directory is configurable
summary: transcripts under an unkept HOME were lost when the pod was replaced (second kind-cluster trial)
tags:
- loom-agent
relations:
- decomposes: epic:takt-as-a-loom-agent
- serves: vision:safe-agentic-coding
scope:
- confidence: cited
  path: crates/takt-cli/src/main.rs
- confidence: cited
  path: crates/takt/src/run.rs
revision: 6
transitions:
- {from: "draft", to: "proposed", at: "2026-10-26T20:53:21Z", actor: "human:timo", revision: 3}
- {from: "proposed", to: "active", at: "2026-10-26T20:53:56Z", actor: "human:timo", revision: 4}
---
## Outcome

The directory takt keeps a run's state and session transcripts in can be named by the run's
configuration, so a container whose `HOME` is not kept can put them on a volume that is.

## Context

- takt keeps transcripts under `$XDG_STATE_HOME/takt/transcripts` (else `~/.local/state`); the
  case's events name them by that path.
- The second kind-cluster trial, 2026-10-26 (takt 0.13.1 as a `LoomAgent` on a local kind cluster): `HOME` is an empty-directory
  volume; when the resource was changed after the hour, the pod was replaced and the transcripts
  of runs 6 to 12 were gone, while the case in the workspace volume still names them. The
  resource has no field for a plain environment variable, so `XDG_STATE_HOME` could not be set.

## Acceptance

1. A run given a state directory by its configuration (or a flag) writes its transcripts there,
   and the case's events name them there.
2. Without one, the location is today's.
