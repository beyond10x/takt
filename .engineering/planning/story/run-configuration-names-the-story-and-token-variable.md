---
format: aep.planning-md/3
id: story:run-configuration-names-the-story-and-token-variable
kind: story
status: active
title: The run configuration names the story and the endpoint's token variable
summary: story and endpoint_token_env in the run configuration file, so a deployment needs no flags (second kind-cluster trial)
tags:
- loom-agent
relations:
- decomposes: epic:takt-as-a-loom-agent
- serves: vision:safe-agentic-coding
scope:
- confidence: cited
  path: crates/takt-cli/src/main.rs
- confidence: cited
  path: crates/takt/src/run_config.rs
revision: 6
transitions:
- {from: "draft", to: "proposed", at: "2026-10-26T20:53:20Z", actor: "human:timo", revision: 3}
- {from: "proposed", to: "active", at: "2026-10-26T20:53:55Z", actor: "human:timo", revision: 4}
---
## Outcome

The run configuration file carries `story` and the endpoint's token variable
(`endpoint_token_env`) beside the fields it carries today, so a deployment that can hand takt
only a file names the story and the credential without command-line flags.

## Context

- `RunConfig` (`crates/takt/src/run_config.rs`) reads `iterations`, `hours`, `harness`, `model`,
  `endpoint_file`, `forge` and `test_cmd`; `--story` and `--endpoint-token-env` are flags only.
  `endpoint_file` can name `[credential] env`, but a relative one is read beside the
  configuration file, where a container mounts no second file.
- The second kind-cluster trial, 2026-10-26 (takt 0.13.1 as a `LoomAgent` on a local kind cluster) passed every setting as
  `spec.command.args` after the image's entrypoint: `--story`, `--hours 1`, `--forge none`,
  `--harness harness`, the three `--endpoint-*` fields and `--endpoint-token-env`, and
  `--test-cmd`. The `loom.agent/1` document Loom mounts carried no `takt` member, because the
  resource's schema in Loom 0.42.0 refuses one ("unknown field spec.agent.takt").

## Acceptance

1. A configuration file with `story = "<id>"` runs that story as `--story <id>` does, refused the
   same way when it is not ready; a flag wins over the file.
2. A configuration file with `endpoint_token_env = "<NAME>"` stages that variable as
   `--endpoint-token-env <NAME>` does, refused the same way, and never records the value.
