---
format: aep.planning-md/3
id: story:run-configuration-file
kind: story
status: implemented
title: takt run takes its configuration from a mounted file, and an image runs it in a container
summary: takt run --config <FILE> and a Dockerfile, so a LoomAgent pod runs takt on /workspace with /config
tags:
- loom-agent
relations:
- serves: vision:safe-agentic-coding
- depends_on: epic:takt-on-loom-sdk
scope:
- confidence: cited
  path: .dockerignore
- confidence: cited
  path: AGENTS.md
- confidence: cited
  path: Dockerfile
- confidence: cited
  path: README.md
- confidence: cited
  path: crates/takt-cli/src/main.rs
- confidence: cited
  path: crates/takt-cli/tests/run_config_cli.rs
- confidence: cited
  path: crates/takt/src/lib.rs
- confidence: cited
  path: crates/takt/src/run_config.rs
- confidence: cited
  path: ess/domains/takt.yaml
revision: 6
transitions:
- {from: "draft", to: "proposed", at: "2026-10-26T09:55:12Z", actor: "human:timo", revision: 4}
- {from: "proposed", to: "active", at: "2026-10-26T09:55:12Z", actor: "human:timo", revision: 5}
- {from: "active", to: "implemented", at: "2026-10-26T10:19:08Z", actor: "human:timo", revision: 6, decided_on: {"recorded":{"test_result":1,"review_outcome":1}}}
---
# Story: run-configuration-file

## Outcome

An operator who deploys takt as a Loom agent in a cluster starts it from one container image with the
repository mounted at `/workspace` and the run's configuration mounted at `/config`, and changes the run
(iterations or hours, harness, model, endpoint, forge, test command) by changing the mounted file, not
the image or its command.

## Context

- Loom's design for deployable agents (`docs/design/loom-agent-crd-deployment.md` § 9a, slice C, loom
  0.41.0) runs a takt delivery as a second `LoomAgent` variant: the pod runs a takt image with takt's
  own entrypoint, and the resource carries takt's run configuration, which the controller mounts as a
  file.
- Before this story every setting of a run was a flag of `takt run`; the endpoint alone had a file
  (`--endpoint-file`, story:harness-endpoint-is-configured). A flag set is fixed in a pod's command;
  a mounted file is what the resource can carry.
- The flags themselves already cover the configuration: `--iterations`, `--hours`, `--harness`,
  `--model`, `--endpoint-file` (whose `[credential]` names the credential by reference), `--forge`,
  `--test-cmd`. What was missing is reading them from a file, and an image.

## Scope

- `crates/takt/src/run_config.rs` (new), `crates/takt/src/lib.rs`, `crates/takt-cli/src/main.rs`
  (`RunArgs::configured`, `parsed`), `crates/takt-cli/tests/run_config_cli.rs` (new),
  `ess/domains/takt.yaml` (`takt.work.RunConfiguration`), `Dockerfile` and `.dockerignore` (new),
  `README.md` § Container, `AGENTS.md` § Commands.

## Acceptance

1. `takt run --config <FILE>` reads `iterations`, `hours`, `harness`, `model`, `endpoint_file`,
   `forge` and `test_cmd` from a TOML file; a case opened from a file alone records the file's
   endpoint and model (`run_config_cli.rs`, `the_file_sets_harness_budget_forge_model_and_endpoint`).
2. A flag given on the command line wins over the file, and the file over the flag's default
   (`a_flag_wins_over_the_file`).
3. A relative `endpoint_file` is read beside the configuration file; an absolute one stays
   (`run_config.rs` unit tests).
4. A key the file may not hold is refused before any case opens, exit 1, naming the file and the
   line and repeating no value (`an_unknown_key_is_refused_before_any_case`); a missing file is
   refused naming it (`a_missing_file_is_refused_naming_it`).
5. `docker build` of the repository's `Dockerfile` produces an image whose entrypoint is
   `takt run --repo /workspace --config /config/run.toml`, run as uid 10001, with `aep`,
   `b10x-harness` and `ess` at pinned versions verified by SHA256; `README.md` § Container states the
   mount contract.

## Out of Scope

- Publishing the image to any registry.
- The `LoomAgent` fields that select an image and command (loom's slice C), the cluster, its
  Secrets, and a run inside a pod.
- Credentials in the pod: the endpoint file names a credential; how the cluster provides it is the
  deployment's.

## Ambiguities

- Whether b10x-harness resolves a secret's name (`secret = "<namespace>/<name>"`) inside a pod with no
  OS keychain is not known here and needs no decision: the first in-cluster run settles it; until then a token
  file mounted from a Secret is the other credential form the endpoint file takes.
