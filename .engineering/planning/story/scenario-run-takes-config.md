---
format: aep.planning-md/3
id: story:scenario-run-takes-config
kind: story
status: draft
title: takt scenario run takes --config as takt run does
summary: takt scenario run refuses --config, so a trial set cannot run from the loom.agent/1 file takt run takes (trial of 0.13.1)
tags:
- finding
- rsi
refs:
- provider: finding
  reference: scenario-run-takes-config
relations:
- decomposes: epic:composable-agentic-work
revision: 2
---
## Outcome

`takt scenario run` takes `--config <FILE>` as `takt run` does: the same TOML or YAML file,
a `loom.agent/1` document's `takt` member included, with the same keys (`iterations`, `hours`,
`harness`, `model`, `endpoint_file`, `forge`, `test_cmd`), a flag on the command line winning
over the file and the file over the flag's default.

## Context

The trial of release 0.13.1 (scenario 1, the `--harness harness` arm on the openai-responses
wire, five runs) could give the new `loom.agent/1` configuration only to the runner's resumes
(`takt run --case <id> --config <file>`): `takt scenario run`, which opens every trial's case,
refuses the flag with exit 2, `unexpected argument '--config'`. `ScenarioRunArgs`
(`crates/takt-cli/src/main.rs:518`) flattens `LoopArgs` but not the `config` field `RunArgs`
holds (`:621`). A trial set therefore cannot run from the same file a container mounts, and a
scenario run's settings and a resume's can drift apart.

```expected_before
scenario: 1
command: "$TAKT" scenario run --help | grep -q -- '--config <FILE>' && echo 'scenario run takes --config' || { echo 'scenario run has no --config'; exit 1; }
exit: 1
healthy_exit: 0
output: scenario run has no --config
source: crates/takt-cli/src/main.rs
```

## Scope

- `crates/takt-cli/src/main.rs`: `ScenarioRunArgs` and `scenario_run`, the config read
  `takt run` uses; tests in `crates/takt-cli/tests`.
- `README.md`: the `--config` line names `takt scenario run` too.

## Acceptance

1. `takt scenario run --repo <scratch> 1 --config <file>` with `iterations: 0`, `harness: none`
   and `forge: none` in a `loom.agent/1` document's `takt` member stops `Iterations (0)`, exit 3:
   a test.
2. A flag given beside `--config` wins over the file's key of that name, and a key the file has
   that `takt run --config` refuses is refused before the case opens, naming the file and the
   line: a test.
