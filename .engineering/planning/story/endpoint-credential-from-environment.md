---
format: aep.planning-md/3
id: story:endpoint-credential-from-environment
kind: story
status: active
title: The harness endpoint's credential comes from an environment variable
summary: takt stages the variable as a protected token file; the variable reaches no child (trial run 5)
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
  path: crates/takt/src/harness.rs
- confidence: cited
  path: ess/domains/takt.yaml
revision: 7
transitions:
- {from: "draft", to: "proposed", at: "2026-10-26T13:48:04Z", actor: "human:timo", revision: 6}
- {from: "proposed", to: "active", at: "2026-10-26T13:48:04Z", actor: "human:timo", revision: 7}
---
# Story: endpoint-credential-from-environment

## Outcome

An operator who deploys takt where the model key arrives as an environment variable (a
`LoomAgent`'s `spec.secretRef`) points `--harness harness` at it by the variable's name, and the
key reaches b10x-harness without entering the harness's environment, its prompt or a case record.

## Context

- Trial: the endpoint credential could be a token file or a secrets-store name only. The store
  needs a Secret Service the pod has no D-Bus for, so the trial staged the key as a file through
  `spec.secretFiles` and unset the variable before takt. Run 5 failed with the token under `/tmp`:
  b10x-harness's credential reader refused it, "secret source does not satisfy the required access
  protections" (llm 0.15.0 `llm-credentials` `file.rs`: every directory on the path owned by root
  or the process's user and writable by nobody else, unless root-owned and sticky; the file a
  regular file with one link that only its owner can read).
- Loom's session executor (loom 0.41.0 and 0.42.0) hands b10x-harness a token file or a secret's
  name, not a variable's name, and takt's rules keep credentials out of a harness's environment.
  So takt reads the variable once and hands the harness a token file it writes where that reader
  accepts it.

## Decision

The variable is staged as a file by takt (recorded in approval-record:wave-37-takt-as-a-loom-agent):
the alternative, the harness reading the variable itself (`--api-key-env`), needs a loom session
executor that passes a variable's name and puts the key into the harness's environment, which
`AGENTS.md` § Rules forbids.

## Acceptance

1. The endpoint file's `[credential]` takes `env = "<NAME>"`, and `--endpoint-token-env <NAME>`
   gives it by flag, exclusive with the token file and the secret; a name that is not a portable
   variable name is refused before any case opens, exit 1.
2. An unset or empty variable is refused before any case opens, exit 1, naming the variable and
   printing no value.
3. Before the case opens takt writes the value into a file of mode 0600 in a new directory of mode
   0700 under the first of `$XDG_RUNTIME_DIR`, `/dev/shm` and the system temporary directory whose
   every directory from `/` passes the reader's rule; none passing is refused, exit 1, naming each
   candidate and why. The file is handed to the session executor as the endpoint's `token_file`,
   no pointer, and removed with its directory when the run ends, on success and on every error
   after it was written.
4. The variable is in the environment of no process takt starts (test command, gates, harness
   session, forge client).
5. The run's `opened` line records the credential as `kind: environment` with the variable's
   `name`, never the value or the staged path's contents; nothing of the value is in the case.

## Out of Scope

- A loom session executor that passes a variable's name.
- Removing a staged file after the process is killed (SIGKILL): the file stays until the
  directory's filesystem is cleared.

## Scope

- `crates/takt/src/harness.rs` (endpoint credential), `crates/takt-cli/src/main.rs` (`EndpointArgs`),
  a CLI test under `crates/takt-cli/tests/`, `ess/domains/takt.yaml` (`takt.work.EndpointCredential`),
  `README.md`.
