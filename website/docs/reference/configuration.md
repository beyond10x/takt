---
title: Configuration
sidebar_position: 1
description: The run configuration file takt run --config reads, the loom.agent/1 takt member, the endpoint file, and every takt run flag grouped by purpose.
lede: A run's settings come from flags, from one configuration file and from defaults, in that order, and a file with a key takt does not know is refused before any case opens.
source: crates/takt/src/run_config.rs (RunConfig, SHAPE, from_toml, from_yaml); crates/takt/src/case.rs (Records); crates/takt/src/events.rs (Opened); crates/takt/src/replay.rs; crates/takt/src/scan.rs (Scan, of_run, refused_at_open); crates/takt/src/forge/gitlab.rs and forge/bot.rs; crates/takt-cli/src/main.rs (RunArgs, EndpointArgs, configured); crates/takt/src/harness.rs (EndpointFields, EndpointConfig); takt run --help at 0.14.0; ADR 0099
---

# Configuration

`takt run` takes its settings from three places. A flag on the command line wins over the
configuration file, and the file wins over the flag's default. No setting has an environment
variable of its own; takt reads the variable `--endpoint-token-env` names and, under the `gates`
scan adapter, `B10X_GATES_POLICY` ([The scan](#the-scan)). The commands, one by one, are in the
generated [CLI reference](./cli.md); a walk-through is in
[Configure a run](../guides/configure-a-run.md).

## The run configuration file

`takt run --config <FILE>` reads one file. The name decides the format:

| name ends in | format |
|---|---|
| `.toml` | TOML: the fields at the top level |
| `.yaml`, `.yml` | YAML: the fields at the top level, or in the `takt` member of a `loom.agent/1` document |
| anything else | refused |

Every field is optional. Every field but `scan` is the `takt run` flag of the same name, with `-`
for `_`; `scan` has no flag.

| field | meaning |
|---|---|
| `story` | string: the story to deliver, by id; taken only when none of `--story`, `--intent` and `--case` is given; refused by `takt scenario run`, whose scenario names the story |
| `iterations` | integer: stop after this many steps |
| `hours` | number: stop after this many hours |
| `harness` | string: `claude`, `codex`, `harness` or `none`; default `claude` |
| `model` | string: the harness's model, when it takes one |
| `endpoint_file` | path: the endpoint file for `--harness harness`; a relative path is read beside the configuration file |
| `endpoint_token_env` | string: the variable holding the endpoint's key, as `--endpoint-token-env`; read only with `--harness harness` and when no credential flag is given |
| `forge` | string: `bot`, `gitlab` or `none` |
| `test_cmd` | string: the test command, split at white space, run without a shell; default `cargo test --quiet` |
| `state_dir` | path: the state directory, as `--state-dir`; a relative path is read beside the configuration file |
| `continue_past_red_base` | boolean: go on when the case's first test run fails at the unchanged base ([`RedBase`](../concepts/how-a-run-ends.md)) |
| `records` | string: `default` (the default) or `trial`, as `--records`: the decisions the case keeps ([Records profiles](#records-profiles)); checked when the file is read |
| `scan` | string: `gates` (the default) or `none`; the adapter that scans what a harness proposes ([The scan](#the-scan)); `none` is accepted only here, never as a flag |

`takt scenario run --config <FILE>` reads the same file, without `story`.

Other `takt run` flags have no key: `--repo`, `--intent`, `--case`, `--protocol`, `--input`,
`--tokens`, `--active-seconds`, `--min-free-gib`, `--gate`, `--gates`, `--project`,
`--commit-identity`, `--confinement`, `--endpoint-base-url`, `--endpoint-wire`,
`--endpoint-model`, `--endpoint-token-file`, `--endpoint-token-pointer` and `--endpoint-secret`.
A key for one of them is refused.

### A minimal TOML file

```toml
iterations = 1
harness = "none"
forge = "none"
test_cmd = "true"
```

Run against a scratch repository (`~/demo`, a git repository with one commit), the file is read
and the run stops on its budget:

```console
$ takt run --repo ~/demo --intent 'add a README line' --config run.toml
case takt-20261026T231214Z on software.change/1 in ~/demo; inputs risk=standard change_kind=feature affects_security_boundary=false; confinement none
frontier: implementation.edit (admissible), release.rollback (approval-required), repository.inspect (admissible), repository.merge (blocked), tests.run (admissible), tests.write (admissible)
step 1: tests.run {}
  effect: the test command exited 0: pass; 
  evidence: test_result pass
frontier: implementation.edit (admissible), release.rollback (approval-required), repository.inspect (admissible), repository.merge (approval-required), tests.run (admissible), tests.write (admissible)
stopped: Iterations (1); steps 1, tokens 0, case ~/demo/.engineering/cases/takt-20261026T231214Z
```

### A minimal loom.agent/1 document

This is the file Loom mounts at `/config/agent.yaml` for a `LoomAgent`. takt reads the `takt`
member and nothing else of it:

```yaml
format: loom.agent/1
agent: {id: takt-demo, name: takt-demo, brief: delivers the repository's stories}
model: {routes: routes.yaml}
takt:
  iterations: 1
  harness: none
  forge: none
  test_cmd: "true"
```

The same run, with the same output as above, starts from `--config agent.yaml`. A
`loom.agent/1` document with no `takt` member sets nothing, so every setting then comes from
flags: that is how a Loom release that does not admit a `takt` member is used, with the settings
in `spec.command.args`.

A YAML file with no `format` is read whole, as the fields themselves. A file with another
`format` is refused.

### What is refused

A file is refused before the case opens, exit 1, naming the file and the line where it knows one,
and never a value written in it:

| file | refusal |
|---|---|
| a key takt does not know | `refused at line 3`, then the shape below |
| a value of another type (`iterations: 1.5`, a number for a string) | `refused at line 1`, then the shape |
| a TOML key written twice | `refused at line 3`, the second one's line |
| a YAML key written twice | `refused at line 1` or the line where its mapping starts: not the duplicate's own line (naming it is planned) |
| a member of a `loom.agent/1` document that is neither Loom's nor `takt` | `refused`, listing Loom's members |
| a `format` other than `loom.agent/1` | `refused`, saying so |
| a YAML tag anywhere | `refused`, saying so |
| a name ending in none of `.toml`, `.yaml`, `.yml` | `refused`, saying so |

Loom's members, as takt reads them from Loom 0.43.0, are `format`, `agent`, `model`, `session`,
`loops`, `states`, `bindings`, `channel`, `writes`, `policy`, `expose`, `runtime` and `deploy`.
takt reads none of them.

An unknown key in a TOML file, run for real:

```console
$ takt run --repo ~/demo --intent x --config unknown.toml
takt: the run configuration file unknown.toml is refused at line 3: a run configuration file holds `iterations` and `hours`, numbers, `story`, `harness`, `model`, `endpoint_file`, `endpoint_token_env`, `forge`, `test_cmd`, `state_dir` and `records`, strings, `continue_past_red_base`, a boolean, and `scan`, `gates` or `none`, each optional; `records` is `default` or `trial`
```

An unknown member of a `loom.agent/1` document, and a file name no format claims:

```console
$ takt run --repo ~/demo --intent x --config member.yaml
takt: the run configuration file member.yaml is refused: a loom.agent/1 document holds a member that is neither one of Loom's (format, agent, model, session, loops, states, bindings, channel, writes, policy, expose, runtime, deploy) nor `takt`
$ takt run --repo ~/demo --intent x --config run.conf
takt: the run configuration file run.conf is refused: its name ends in neither .toml, .yaml nor .yml
```

### Precedence

A flag on the command line wins. The file's `iterations = 1` with `--iterations 2` on the command
line runs two steps:

```console
$ takt run --repo ~/demo --intent x --config run.toml --iterations 2
step 1: tests.run {}
step 2: repository.merge (none)
stopped: ApprovalRequired (repository.merge); steps 2, tokens 0, case ~/demo/.engineering/cases/prec
```

(Lines of the run that do not show the precedence are left out.)

`endpoint_file` is read only when the harness, after precedence, is `harness`. A file that sets
`endpoint_file` and a command line that says `--harness none` runs with no endpoint and is not
refused. A relative `endpoint_file` is read beside the configuration file, not against the
current directory.

## The endpoint file

`--harness harness` runs [b10x-harness](https://beyond10x.github.io/harness/)
([GitHub](https://github.com/beyond10x/harness)) against an LLM endpoint. Without any endpoint
setting it is the Anthropic Messages API on the claude.ai login in
`~/.claude/.credentials.json`. The endpoint file, `--endpoint-file <PATH>` or `endpoint_file` in
the run configuration, is TOML:

```toml
base_url = "https://llm.example.com/v1"
wire = "openai-responses"
model = "example-model"

[credential]
env = "LLM_API_KEY"
```

Each key is the flag `--endpoint-<key>` (`token_file` is `--endpoint-token-file`), except `env`,
which is `--endpoint-token-env`.

| key | meaning |
|---|---|
| `base_url` | the endpoint's origin and API prefix; default `https://api.anthropic.com/v1` |
| `wire` | `anthropic-messages` (default) or `openai-responses` |
| `model` | the model the endpoint serves; default `claude-opus-5`; `--model` wins over it |
| `token_file` | in `[credential]`: a file holding the token; a relative path is read beside the endpoint file |
| `token_pointer` | in `[credential]`: a JSON pointer into `token_file`; without it the whole file is the token |
| `secret` | in `[credential]`: `<namespace>/<name>`, a secret in the owner's secrets store, handed to b10x-harness by name |
| `env` | in `[credential]`: the name of an environment variable holding the key |

The `[credential]` table names exactly one of `token_file` (with an optional `token_pointer`),
`secret` or `env`: two, or a `token_pointer` alone, is refused. Any other key, or a key in the
wrong place, is refused at its line like the run configuration is:

```console
$ takt run --repo ~/demo --intent x --harness harness --endpoint-file badep.toml
takt: the endpoint file badep.toml is refused at line 2: an endpoint file holds `base_url`, `wire` and `model`, each a string, and a `[credential]` table of `token_file` with an optional `token_pointer`, of `secret`, or of `env`
```

A flag wins over the file, field by field, and the credential is taken whole from one source: if
any credential flag is given, the file's `[credential]` is not read. A credential is named, never
given. takt reads no secret: a token file's path or a secret's name goes to b10x-harness, which
resolves it.

Rules the run checks before a case opens, each exit 1:

- A base URL is `https://`, or `http://` to a loopback host, with no userinfo, query or fragment.
- A base URL other than the default needs a credential. The claude.ai token goes to the default
  base URL only.
- A token file that is missing, or whose pointer resolves to no token, is refused naming the path
  and the pointer.
- An `--endpoint-*` flag with a harness other than `harness` is refused.

```console
$ takt run --repo ~/demo --intent x --harness harness --endpoint-file nocred.toml
takt: the endpoint's base URL https://llm.example.com/v1 is not the default https://api.anthropic.com/v1, and no credential is given: the default credential, the claude.ai subscription token in ~/.claude/.credentials.json, is not sent to another host; give --endpoint-token-file, --endpoint-secret or --endpoint-token-env, or the endpoint file's `[credential]`
```

### A key from an environment variable

`--endpoint-token-env <NAME>` (the file's `env = "<NAME>"`) takes the API key from the variable
`NAME`. It works on the `openai-responses` wire only, until the session executor can pass an
API-key file. takt:

1. takes the variable out of its own environment before it starts anything, so no process it
   starts sees it (under `--confinement substrate` outside a delegated cgroup, `systemd-run` and
   the takt it re-runs are the exception);
2. on Linux with glibc, overwrites its value in its initial environment block, which
   `/proc/<pid>/environ` reads;
3. stages the value as a token file of mode 0600 in a new directory of mode 0700 under the first
   of `$XDG_RUNTIME_DIR`, `/dev/shm` and the temporary directory that b10x-harness's token-file
   reader accepts;
4. removes the file when the run ends, and on SIGTERM, SIGINT and SIGHUP (SIGKILL leaves it).

The case records the variable's name, never its value. A name that is no portable variable name,
another wire, or a variable that is unset or empty is refused before the case opens:

```console
$ takt run --repo ~/demo --intent x --config harness.toml
takt: the endpoint's credential variable LLM_API_KEY is not set; nothing was started
$ LLM_API_KEY=<token> takt run --repo ~/demo --intent x --harness harness --endpoint-file nocred.toml --endpoint-token-env LLM_API_KEY --endpoint-wire anthropic-messages
takt: the endpoint's credential variable LLM_API_KEY is given on the wire `anthropic-messages`: only openai-responses is admitted for an environment credential until the session executor can pass an API-key file; nothing was started
```

## Commit identity

takt commits as the identity the repository names in `.engineering/commit-identity.conf`, read
at the commit the case forks from:

```ini
[identity]
name = takt-demo
email = takt-demo@example.com
```

or as `--commit-identity '<name> <email>'`, which wins over the file, never as your git
configuration. With neither, it commits as `takt <takt@localhost>`, and a run whose forge is the
bot route, which pushes those commits, is refused before its case opens. The identity is recorded in the
case ([`commit-identity`](./case-files.md)), so a resumed case and `takt approve` commit as the
identity it was opened with.

## The forge

Without `--forge`, takt follows `origin`:

| `origin` | forge |
|---|---|
| a repository on github.com | `bot` |
| any other host, with `glab auth status --hostname <host>` exiting 0 | `gitlab` |
| anything else | `none`, with the reason in one line |

Each forge is an adapter, and the three are peers:

| forge | what it pushes and opens through | what scans what it publishes |
|---|---|---|
| `gitlab` | `git push` over HTTPS with `glab auth git-credential` as its one credential helper; the merge request opened, read and merged through `glab api` | the run's scan adapter ([The scan](#the-scan)) |
| `bot` | beyond10x's route for github.com: `b10x-gates bot` pushes the case's branch under a GitHub App and `b10x-gates api` opens and merges the pull request; `gh` reads its state and checks | `b10x-gates scan-text`, always: it is the `gates` adapter's own forge |
| `none` | nothing | nothing leaves the machine |

`--forge gitlab` without `glab` or its login is refused before the case opens. Under `scan:
none` it checks that the head is a commit and pushes without a scan. `--forge bot` takes an
`origin` on github.com only and refuses one on another host, naming it. `--forge none` opens no pull request: a delivery whose change reads ahead stops
`Unperformable` at `pr.opened`, exit 7, and prints `candidate: <full commit id>`: its code
commits, replayed on the base branch's tip under the configured identity and named by
`refs/takt/candidates/<case>`. With no identity configured it makes none and names the setting.

A scenario run creates its trial base only through the bot route: it refuses `--forge gitlab`,
and on another host it runs with `--forge none`.

## The scan

Codex and b10x-harness sessions may read outside the copy they work in, so takt accepts what they
propose only after a scan passed it, or after the run declared that nothing scans it. The scan is
an adapter, one per run, named by `scan` in the run configuration file:

| `scan` | what scans a proposal, the change so far a prompt shows, and what `--forge gitlab` publishes |
|---|---|
| `gates`, the default | `b10x-gates scan-text`, beyond10x's scanner ([b10x-gates](https://github.com/beyond10x/gates)), under the policy file `B10X_GATES_POLICY` names |
| `none` | nothing |

`none` is accepted only as a declaration in the file `--config` gives; no flag and no variable
sets it. What follows from the adapter:

- A Codex or b10x-harness run under `gates` with `B10X_GATES_POLICY` unset is refused before its
  case opens, exit 1, and nothing is written. The refusal names the adapter and both ways on: set
  the policy, or declare `scan: none`. A resumed case (`--case`) is refused the same way.
- Under `gates`, a proposal no scan passed is refused, `not scanned, which is not passed`, and so
  is one whose scan cannot run, `b10x-gates` missing among the causes.
- Under `none`, the run says so before its first step, `scan: none (declared in <file>); what
  --harness <harness> proposes is accepted unscanned, checked only for the shape of a
  credential`.
- `scan: none` with `B10X_GATES_POLICY` set is refused before the case opens: a run has one scan
  adapter, and a policy no scan reads would read as a scan that ran.
- The case's `opened` event records the adapter as `scan`.

Claude Code's file tools stay inside its copy, so a Claude Code run under `gates` needs no
policy. Every proposal, from any harness and under either adapter, is checked in process for the
shape of a credential, and scanned too under `gates` when a policy is set.

A scenario run cannot declare `none`: `takt scenario run --config` refuses a file that declares it,
naming the file, and with a harness that may read outside its copy it needs `B10X_GATES_POLICY`.

## Records profiles

`--records default|trial`, or `records` in the file, names which `decisions/NNNN.json` files the
case keeps. The flag wins over the file, and the file over the default, `default`.

| profile | what the case keeps |
|---|---|
| `default` | the latest decision alone; the numbering goes on from it, so the next decision is the next number, not `0001` again |
| `trial` | every decision, as the case kept them before the profile existed |

A scenario run (`takt scenario run`) and an improvement case (`self.improvement/1`,
`loop.improvement/1`) keep every decision whatever is asked. The case's `opened` event records the
profile the case keeps as `records`; a line without it was written before the profile and reads as
every decision kept. `takt replay` refuses a step whose decision a `default` run dropped, naming
`--records trial`: replay a step of a run made with it. The [case files](./case-files.md) page
lists the file.

## Every takt run flag

One line each, from `takt run --help` at 0.14.0. The generated [CLI reference](./cli.md) has the
full text of each, and the [exit codes](./exit-codes.md) are generated from the same help.

### What to work on

| flag | meaning |
|---|---|
| `--repo <REPO>` | the repository worktree (required) |
| `--intent <INTENT>` | what the run is meant to achieve; opens a case on `software.change/1` |
| `--story <ID>` | the story to deliver, by id, instead of the frontier's next ready one; one not listed or not ready is refused, exit 1, naming why |
| `--case <CASE>` | the case to resume, by id; not with `--intent` |
| `--protocol <PROTOCOL>` | a built-in id (`takt protocols`) or a `protocol/1` file |
| `--input <NAME=VALUE>` | a case input; repeatable; refused for a resumed case |
| `--case-id <CASE_ID>` | the case id; generated when absent |
| `--scenarios <SCENARIOS>` | the scenario checkout an improvement turn's finding names its scenario in |
| `--install-root <INSTALL_ROOT>` | where an improvement turn reads the promoted binary |

### Budget

| flag | meaning |
|---|---|
| `--iterations <N>` | stop after this many steps |
| `--hours <H>` | stop after this many hours |
| `--tokens <N>` | stop once the harness reported this many tokens |
| `--active-seconds <S>` | stop once active time (wall time less waits on the forge) reaches this |
| `--min-free-gib <GIB>` | stop `DiskLow` under this much free disk; default 10; `0` turns the check off |

How each ends a run: [How a run ends](../concepts/how-a-run-ends.md).

### Harness and endpoint

| flag | meaning |
|---|---|
| `--harness <HARNESS>` | `claude`, `codex`, `harness` (b10x-harness) or `none` (validators only); default `claude` |
| `--model <MODEL>` | the harness's model, when it takes one |
| `--endpoint-file <PATH>` | the endpoint file |
| `--endpoint-base-url <URL>` | the endpoint's base URL |
| `--endpoint-wire <WIRE>` | `anthropic-messages` or `openai-responses` |
| `--endpoint-model <MODEL>` | the model the endpoint serves |
| `--endpoint-token-file <PATH>` | a token file; not with `--endpoint-secret` |
| `--endpoint-token-pointer <POINTER>` | a JSON pointer into the token file |
| `--endpoint-secret <NS/NAME>` | a secret's name; takt reads no secret |
| `--endpoint-token-env <NAME>` | a variable holding the key; `openai-responses` only |

All `--endpoint-*` flags need `--harness harness`.

### Forge and gates

| flag | meaning |
|---|---|
| `--forge <FORGE>` | `bot`, `gitlab` or `none` (see above) |
| `--gate <GATE>` | the approval gate the harness may request to end the run; default the protocol's first action that requires a capability |
| `--gates <GATES>` | the gates document, relative to the repository; default `.engineering/gates.yaml` when it is a file |
| `--project <PROJECT>` | the `engineering-assertions-project/1` file; default `.engineering/assertions-project.yaml` when it is a file |

### Tests

| flag | meaning |
|---|---|
| `--test-cmd <TEST_CMD>` | the test command, no shell; default `cargo test --quiet` |
| `--continue-past-red-base` | go on past a red base, for a base whose tests fail on purpose |

### State

| flag | meaning |
|---|---|
| `--state-dir <DIR>` | the state directory the run keeps its session transcripts in (`<DIR>/transcripts`), in place of `$XDG_STATE_HOME/takt`; for a container, a directory on a kept volume |

### Records

| flag | meaning |
|---|---|
| `--records <default\|trial>` | the decisions the case keeps: `default` the latest alone, `trial` every one ([Records profiles](#records-profiles)); default `default` |

### Confinement and identity

| flag | meaning |
|---|---|
| `--confinement <C>` | `none` (default) or `substrate`: the test command run by Loom's `SubstrateRunner` with no network |
| `--commit-identity <'NAME <EMAIL>'>` | the author and committer of every commit of the case |

### The file

| flag | meaning |
|---|---|
| `--config <FILE>` | the run configuration file described above |

## Where the container reads its configuration

The container image runs `takt run --repo /workspace --config /config/agent.yaml`. What is
mounted where, and what a `LoomAgent` writes there, is in
[Container image](./container-image.md).
