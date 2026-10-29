---
title: Configure a run
sidebar_position: 2
description: Put a run's harness, scan, endpoint, forge, test command and budget in one file, take the API key from a variable, and read the refusals a wrong file gets.
lede: One file passed with --config holds what stays the same from run to run, flags hold what changes, and takt refuses a file it does not fully understand before it starts anything.
source: takt run --help at 0.14.0; crates/takt/src/run_config.rs (refused_duplicate); crates/takt/src/scan.rs; crates/takt/src/harness.rs (EndpointFields); runs of takt 0.14.0 against the files on this page, 2026-10-26; the duplicate-key, scan refusal and scan none runs with a build of takt 0.21.0, 2026-10-29; CHANGELOG.md 0.12.0 to 0.14.0, 0.19.0 and 0.21.0
---

# Configure a run

Most settings are `takt run` flags. The ones you repeat on every run can go in
a file. The full list of keys and flags is in [Configuration](../reference/configuration.md). This
page goes through the usual choices in order.

## 1. Write the file

TOML or YAML, chosen by the file name's ending (`.toml`, `.yaml` or `.yml`):

```toml
iterations = 4
harness = "none"
forge = "none"
test_cmd = "cargo test --quiet --locked"
```

```sh
takt run --repo . --intent "keep the tests green" --config takt.toml --iterations 1
```

```console
step 1: tests.run {}
  effect: the test command exited 0: pass; 1 passed, 0 failed, 0 ignored over 2 result lines
  evidence: test_result pass
stopped: Iterations (1); steps 1, tokens 0, case ~/cfg-demo/.takt/cases/takt-20261026T233456Z
```

(The `case` and `frontier` lines are left out.) `--iterations 1` on the command line won over the
file's `4`. A flag wins over the file, and the file wins over the default.

Every key the file may hold, and which of them have no flag, is in
[The run configuration file](../reference/configuration.md#the-run-configuration-file). The
file's `story` is taken only when none of `--story`, `--intent` and `--case` is given.
`takt scenario run --config` reads the same file and refuses a `story` in it, since the scenario
names the story.

## 2. Pick the harness

| `harness` | what takes the judgement steps |
|---|---|
| `claude` (default) | Claude Code, as you are logged in to it |
| `codex` | Codex |
| `harness` | b10x-harness, talking to the LLM endpoint you name |
| `none` | nothing: only the validators run, and the run stops at the first judgement step |

`model` names the model for harnesses that take one.

### Codex and b10x-harness: the scan

Claude Code, the default, needs none of this. Codex and b10x-harness sessions may read outside
their copy, so what they propose is accepted only after a scan passed it. The scan is an
adapter, `scan` in the file:

| `scan` | what scans a proposal |
|---|---|
| `gates` (default) | [b10x-gates](https://github.com/beyond10x/gates), beyond10x's scanner, under the policy file `B10X_GATES_POLICY` names |
| `none` | nothing; each proposal is still checked for the shape of a credential |

With a gates policy, give it in the variable:

```sh
B10X_GATES_POLICY=<policy file> takt run --repo . --harness codex --forge none \
  --intent "add a function sub(left, right) that subtracts right from left, with a test"
```

With neither the policy nor `scan = "none"`, the run is refused before its case opens, exit 1,
and the refusal names both ways on:

```console
$ takt run --repo demo --harness codex --forge none --intent "add a function sub"
takt: --harness codex runs a session that may read outside its copy, and the scan adapter `gates` has no policy (B10X_GATES_POLICY is not set): what it proposes would not be scanned, and not scanned is never passed; set B10X_GATES_POLICY to a gates policy file, or declare `scan: none` in the run configuration file given with --config (`takt run --config <file>`) to accept what it proposes unscanned, checked only for the shape of a credential; nothing was opened
```

Without b10x-gates, declare in the file that nothing scans. `scan = "none"` is accepted only in a
file, never as a flag:

```toml title="run.toml"
scan = "none"
harness = "codex"
forge = "none"
```

The run says so before its first step:

```console
$ takt run --repo demo --config run.toml --iterations 0 --intent "add a function sub"
scan: none (declared in run.toml); what --harness codex proposes is accepted unscanned, checked only for the shape of a credential
```

(The other lines of that run are left out.) The case's `opened` event records the adapter as
`scan`. `scan = "none"` with `B10X_GATES_POLICY` set is refused too: a run has one scan adapter
([The scan](../reference/configuration.md#the-scan)).

## 3. Name the endpoint for b10x-harness

The endpoint is a second TOML file. `endpoint_file` in the run's file names it, and a relative
path is read beside the run's file, not from the current directory:

```toml
hours = 1
harness = "harness"
endpoint_file = "endpoint.toml"
forge = "none"
```

```toml
base_url = "https://llm.example.com/v1"
wire = "openai-responses"
model = "example-model"

[credential]
env = "LLM_API_KEY"
```

`wire` is the API the endpoint speaks, `anthropic-messages` or `openai-responses`. The
`[credential]` table holds one of three things: `token_file` (with an optional `token_pointer`
into a JSON file), `secret` (a name in your secrets store, resolved by b10x-harness), or `env`
(the name of a variable that holds the key). Each field also has an `--endpoint-*` flag, which wins
over the file. takt reads the file only when the harness is `harness`.

## 4. Pass the key in a variable

```sh
export LLM_API_KEY=<token>
takt run --repo . --config harness.toml --intent "..."
```

The `env` credential, or the flag `--endpoint-token-env LLM_API_KEY`, works on the
`openai-responses` wire only in 0.14.0. takt takes the variable out of its own environment before
it starts anything, so no step, harness or test command sees it. It writes the key to a file of
mode 0600 in a new directory of mode 0700 and removes the file when the run ends. b10x-harness
reads the key from that file. Since 0.13.1 this works on macOS too. The case records the
variable's name, never its value.

A missing or empty variable stops the run before it starts:

```console
$ takt run --repo . --intent x --config harness.toml
takt: the endpoint's credential variable LLM_API_KEY is not set; nothing was started
$ LLM_API_KEY= takt run --repo . --intent x --config harness.toml
takt: the endpoint's credential variable LLM_API_KEY is empty; nothing was started
```

## 5. Choose the forge

| `forge` | what happens at the pull request |
|---|---|
| not set | `bot` for a github.com `origin`, `gitlab` for another host `glab` is logged in to, else `none` |
| `bot` | beyond10x's route for github.com: `b10x-gates bot` pushes the branch under a GitHub App and `b10x-gates api` opens and merges the pull request; `gh` reads its checks |
| `gitlab` | `git` pushes over HTTPS and `glab api` opens, reads and merges the merge request, after the run's scan adapter scanned what it publishes |
| `local` | a bare repository on this machine, the tree's `origin`, is the forge: `git` pushes to it, a request record in it is the pull request, the run file's `local_checks` are the checks, and the merge is made in it; never a default ([The local forge](../reference/configuration.md#the-local-forge)) |
| `none` | no pull request: a story stops `Unperformable` at `pr.opened`, exit 7, and prints its candidate commit |

How each forge is set up: [Configuration](../reference/configuration.md#the-forge). The local
forge needs `local_checks` beside it in the file:

```yaml
forge: local
local_checks: [cargo, test, --quiet]
```

## 6. Set the test command and the base check

`test_cmd` is split at white space and run without a shell, so pipes and `&&` do not work; the
default is `cargo test --quiet`. When the case's first test run fails before any step has
changed the tree, the run stops `RedBase`, exit 3, before any model call:

```console
stopped: RedBase (the tests fail at the case's base 6a2a2318, before any step changed it: tests::it_works; to rerun pass `--lib`)
```

For a base whose tests fail on purpose, such as a story that fixes them, set
`continue_past_red_base = true` or pass `--continue-past-red-base`. Both stop rules are described
in [How a run ends](../concepts/how-a-run-ends.md#the-two-test-stops).

## What a wrong file gets

`--config` is read strictly. A file that is not entirely understood is refused before the case
opens, exit 1. The refusal names the line and never the value written there:

| the file | refused at |
|---|---|
| an unknown key, such as `stroy` on line 4 of a TOML file | line 4 |
| a key written twice, in TOML or YAML, the second on line 4 | line 4, the duplicate's own line, naming the key ([A key written twice](#a-key-written-twice)) |
| a `loom.agent/1` member that is neither Loom's nor `takt` | no line; the message lists Loom's members |

```console
$ takt run --repo . --intent x --config stroy.toml
takt: the run configuration file stroy.toml is refused at line 4: a run configuration file holds `iterations` and `hours`, numbers, `story`, `harness`, `model`, `endpoint_file`, `endpoint_token_env`, `forge`, `forge_instance`, `test_cmd`, `state_dir`, `records` and `protocol`, strings, `continue_past_red_base`, a boolean, `scan`, `gates` or `none`, `llm`, an `llm.routes/1` document or `{path: <file>}`, `transport`, glab or api, `credential`, a mapping naming an env variable or a file, `forge_commands`, a mapping of argument lists, `local_checks`, an argument list, `setup`, a list of commands each with a name and an argv, and `protocols`, a mapping of a protocol to its declared inputs, each optional; `records` is `default` or `trial`
```

### A key written twice

A key written twice is refused at the duplicate's own line, in TOML and, from 0.19.0, in YAML.
The refusal names the key when it is one takt or Loom declares. This file sets `harness` on
lines 2 and 4:

```yaml title="dup.yaml"
iterations: 1
harness: none
forge: none
harness: none
```

```console
$ takt run --repo repo --intent x --config dup.yaml
takt: the run configuration file dup.yaml is refused at line 4: it holds the key `harness` twice in one mapping
```

The same in TOML says `table` for `mapping`. From 0.21.0 a duplicate after an alias key, a
tagged key or a plain key ending in a colon is found at its line too. Three rare YAML shapes are
still refused at the line where the mapping starts, without the key
([Known limitations](../known-limitations.md)).

## The same file under Loom

A YAML file may also be a `loom.agent/1` document, the format Loom mounts at
`/config/agent.yaml`. Its `takt` member holds the same keys, and takt reads nothing else of it:

```yaml
format: loom.agent/1
agent:
  id: takt-demo
  name: takt-demo
  brief: Delivers the ready stories of the repository mounted at /workspace.
takt:
  hours: 1
  harness: harness
  endpoint_file: endpoint.toml
  forge: none
  test_cmd: cargo test --quiet --locked
```

That is the container image's default `/config/agent.yaml`
([Run in a container](./run-in-a-container.md)). Loom 0.45.0 itself refuses a `takt` member in a
`LoomAgent`'s `spec.agent`, so on a cluster the run's file is a plain `takt.yaml` in
`spec.configFiles` instead ([Deploy on Kubernetes](./deploy-on-kubernetes.md)).
