---
title: Configuration
sidebar_position: 1
description: The run configuration file takt run --config reads, the loom.agent/1 takt member, the endpoint file, and every takt run flag grouped by purpose.
lede: A run's settings come from flags, from one configuration file and from defaults, in that order, and a file with a key takt does not know is refused before any case opens.
source: crates/takt/src/run_config.rs (RunConfig, SHAPE, from_toml, from_yaml); crates/takt/src/case.rs (Records); crates/takt/src/events.rs (Opened); crates/takt/src/replay.rs; crates/takt/src/scan.rs (Scan, of_run, refused_at_open, refused_to_publish); crates/takt/src/forge/gitlab.rs and forge/local.rs; crates/takt-cli/src/main.rs (RunArgs, LoopArgs, EndpointArgs, configured, local_forge, scenario_forge_of); crates/takt-cli/tests/local_forge_cli.rs; crates/takt/src/harness.rs (EndpointFields, EndpointConfig); crates/takt/src/console.rs (LINE_WIDTH, READS_SHOWN, DIFF_SHOWN, fit); CHANGELOG.md 0.19.0 and 0.21.0; takt run --help at 0.14.0; ADR 0099
---

# Configuration

`takt run` takes its settings from three places. A flag on the command line wins over the
configuration file, and the file wins over the flag's default. No setting has an environment
variable of its own; takt reads the variable `--endpoint-token-env` names. The commands, one by
one, are in the generated [CLI reference](./cli.md); a walk-through is in
[Configure a run](../guides/configure-a-run.md).

## The run configuration file

`takt run --config <FILE>` reads one file. The name decides the format:

| name ends in | format |
|---|---|
| `.toml` | TOML: the fields at the top level |
| `.yaml`, `.yml` | YAML: the fields at the top level, or in the `takt` member of a `loom.agent/1` document |
| anything else | refused |

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
stopped: Iterations (1); steps 1, tokens 0, case ~/demo/.takt/cases/takt-20261026T231214Z
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

The same run, with the same output as above, starts from `--config agent.yaml`. The document's
`runtime.state_dir`, when it has one, is the run's default state directory
([The state directory](#the-state-directory)). A
`loom.agent/1` document with no `takt` member sets nothing, so every setting then comes from
flags: that is how a Loom release that does not admit a `takt` member is used, with the settings
in `spec.command.args`.

A YAML file with no `format` is read whole, as the fields themselves. A file with another
`format` is refused.

### Every field

Every field is optional. Every field but `scan`, `forge_commands`, `forge_instance`, `forge_endpoint`, `local_checks`, `setup`, `transport` and `credential` is the `takt run` flag of the same name, with `-`
for `_`; those eight have no flag.

| field | meaning |
|---|---|
| `story` | string: the story to deliver, by id; taken only when none of `--story`, `--intent` and `--case` is given; refused by `takt scenario run`, whose scenario names the story |
| `iterations` | integer: stop after this many steps |
| `hours` | number: stop after this many hours |
| `tokens` | whole number above zero: stop once the harness reported this many tokens, as `--tokens`; with neither, 3000000 |
| `harness` | string: `claude`, `codex`, `harness` or `none`; default `claude` |
| `model` | string: the harness's model, when it takes one; with an `llm` section under `--harness harness`, the route or alias of that section it names, whose endpoint, wire and upstream model the session uses |
| `endpoint_file` | path: the endpoint file for `--harness harness`; a relative path is read beside the configuration file |
| `endpoint_token_env` | string: the variable holding the endpoint's key, as `--endpoint-token-env`; read only with `--harness harness` and when no credential flag is given |
| `forge` | string: `gitlab`, `command`, `connectors`, `local` or `none` |
| `test_cmd` | string: the test command, split at white space, run without a shell; default `cargo test --quiet` |
| `state_dir` | path: the state directory, as `--state-dir`: the run's transcripts go to `<state_dir>/transcripts` and what a discarded step sets aside to `<state_dir>/set-aside` ([The state directory](#the-state-directory)); a relative path is read beside the configuration file |
| `continue_past_red_base` | boolean: go on when the case's first test run fails at the unchanged base ([`RedBase`](../concepts/how-a-run-ends.md)) |
| `records` | string: `default` (the default) or `trial`, as `--records`: the decisions the case keeps ([Records profiles](#records-profiles)); checked when the file is read |
| `commit` | mapping of `style` (`semantic`, the default, `repository` or `action`), `trailer` (default `Refs`; empty for none) and `types` (an action to its commit type), as `--commit-style`, `--commit-trailer` and `--commit-types`: how a step commit's message is written ([Commit messages](#commit-messages)); checked when the file is read |
| `scan` | string: `none`, the one adapter; not set, nothing unscanned is accepted ([The scan](#the-scan)); accepted only here, never as a flag |
| `forge_commands` | a mapping read with `forge: command`: `open`, `read`, `checks` and `merge`, each the argument list that action runs, without a shell ([The forge](#the-forge)), with `id_pattern`, a regular expression whose first group is the request number in what `open` printed, and `refused_exit`, the exit codes that are a refusal; no flag |
| `forge_instance` | string: the connector `forge: connectors` calls and, after one `/`, its instance, `<connector>[/<instance>]` as `connectors invoke` takes its target: the connector `gitlab` or `github`, the instance a word of letters, digits, `.`, `_` and `-`; checked when the file is read, needed by `forge: connectors` and refused under any other forge ([The connectors forge](#the-connectors-forge)); no flag |
| `forge_endpoint` | string: the URL of a served Connectors instance that `forge: connectors` calls over its served wire instead of running `connectors`, `https://`, or `http://` to a loopback host alone (`localhost`, `127.0.0.0/8`, `[::1]`), with no userinfo, query or fragment; no credential goes with a call to it; checked when the file is read, refused under any other forge ([The connectors forge](#the-connectors-forge)); no flag |
| `local_checks` | argument list: the checks `forge: local` runs on a request's head, without a shell, as the test command is run (its confinement, environment and timeout), in a fresh repository under the state directory that fetched the head from the hub by its id and names no remote; exit 0 passes, any other exit or the timeout fails; a pass is kept for that head and that argument list, so changed `local_checks` run again, and a fail is run again on the next read; not run while less than `--min-free-gib` is free; under `--confinement none` they share the run's `CARGO_TARGET_DIR`, under `substrate` they build in the fresh repository's own `target/`, and a program the adapter cannot run (anything but `cargo`) is refused before any case opens; required by `forge: local` and refused under any other forge, before any case opens; no flag |
| `setup` | a list of `{name, argv}` commands, run once in the tree when the case opens, before the red-base check and any model call, under the test command's environment and confinement; `argv` is a program and its arguments, run without a shell; each duration is recorded on a `setup` line of the run record and the output kept under the state directory; a command that does not end 0 stops the run `SetupFailed`, exit 1; a resumed case runs them again only when its base moved; no flag |
| `transport` | `glab`, the default, or `api`: how `forge: gitlab` reaches the host ([The GitLab API transport](#the-gitlab-api-transport)); no flag |
| `credential` | `{env: NAME}` or `{file: PATH}`, where the `api` transport's token comes from, by reference only; no flag |
| `llm` | an `llm.routes/1` document, or `{path: <file>}` naming one beside the run file (`.toml` read by llm's TOML parser, `.yaml` or `.yml` by its YAML one); llm validates it, a refused section opens no case, and the `opened` line records its digest as `llm_digest`; under `--harness harness`, `model` names one of its routes, and no endpoint source (`--model-endpoint`, `--model-wire`, `--subscription-token-file`, `--subscription-token-pointer`, `--api-key-secret`, the `--endpoint-*` names, `endpoint_file`, `endpoint_token_env`) may sit beside the section, both refused before any case opens |
| `protocol` | string: the protocol a story from the planning store opens on, `<name>/<major>` as `takt protocols` lists it; the flag `--protocol` wins over it, and a story tag `protocol:<name>/<major>` wins over it for that story; an `--intent` opens on `software.change/1` whatever it says; with none of them a story opens on `story.delivery/3` ([defaults](protocol-defaults.md)) |
| `protocols` | a mapping of a protocol, by name (every revision) or `<name>/<major>`, to a mapping of its declared case inputs, each set to one of the values the protocol declares; an unknown protocol, input or value is refused at its line before any case opens. An input is taken from `--input`, else a story tag `input:<name>=<value>`, else this section, else takt's built-in default for the protocol; each value is recorded with its source in the case's `case.yaml` |

`takt scenario run --config <FILE>` reads the same file, without `story`; it checks the shape of `protocol` and `protocols` and applies neither.

Other `takt run` flags have no key: `--repo`, `--intent`, `--case`, `--input`,
`--active-seconds`, `--min-free-gib`, `--verbose`, `--color`, `--quiet-steps`, `--gate`, `--gates`, `--project`,
`--commit-identity`, `--confinement`, `--confinement-needs`, `--confinement-cgroup-root`, `--llm-config`, `--model-endpoint`, `--model-wire`,
`--subscription-token-file`, `--subscription-token-pointer` and `--api-key-secret`, and the old
`--endpoint-*` names of the endpoint flags below. A key for one of them is refused.

### What is refused

A file is refused before the case opens, exit 1, naming the file and the line where it knows one,
and never a value written in it:

| file | refusal |
|---|---|
| a key takt does not know | `refused at line 3`, then the shape below |
| a value of another type (`iterations: 1.5`, a number for a string) | `refused at line 1`, then the shape |
| `tokens` that is no whole number above zero (`tokens: 0`, `tokens: x`) | `refused at line 2`, exit 2, as `--tokens x` does not parse |
| `forge_endpoint` that is no `https://` URL and no `http://` URL to a loopback host (`forge_endpoint: http://example.com`) | `refused at line 3`, naming `forge_endpoint` and the rule it breaks, never the URL, exit 2, as for `tokens` |
| a key written twice, TOML or YAML | `refused at line 4`, the duplicate's own line, saying it holds the key twice in one mapping (a table in TOML) and naming the key when takt or Loom declares it ([an example](../guides/configure-a-run.md#a-key-written-twice)); three rare YAML shapes keep the line where the mapping starts, without the key ([Known limitations](../known-limitations.md)) |
| a member of a `loom.agent/1` document that is neither Loom's nor `takt` | `refused`, listing Loom's members |
| a `format` other than `loom.agent/1` | `refused`, saying so |
| a YAML tag anywhere | `refused`, saying so |
| a name ending in none of `.toml`, `.yaml`, `.yml` | `refused`, saying so |

Loom's members, as takt reads them from Loom 0.43.0, are `format`, `agent`, `model`, `session`,
`loops`, `states`, `bindings`, `channel`, `writes`, `policy`, `expose`, `runtime` and `deploy`.
takt reads one value of them, `runtime.state_dir`, and none of the rest. Inside `runtime` it
accepts the members Loom declares, `state_dir`, `connectors` and `confinement`, and refuses any
other. A `runtime.state_dir` that is not a path string, or is empty or blank, is refused,
without its value.

An unknown key in a TOML file, run for real:

```console
$ takt run --repo ~/demo --intent x --config unknown.toml
takt: the run configuration file unknown.toml is refused at line 3: a run configuration file holds `iterations` and `hours`, numbers, `tokens`, a whole number above zero, `story`, `harness`, `model`, `endpoint_file`, `endpoint_token_env`, `forge`, `forge_instance`, `forge_endpoint`, `test_cmd`, `state_dir`, `records` and `protocol`, strings, `continue_past_red_base`, a boolean, `scan`, `none`, `llm`, an `llm.routes/1` document or `{path: <file>}`, `transport`, glab or api, `credential`, a mapping naming an env variable or a file, `forge_commands`, a mapping of argument lists, `local_checks`, an argument list, `setup`, a list of commands each with a name and an argv, and `protocols`, a mapping of a protocol to its declared inputs, each optional; `records` is `default` or `trial`
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
stopped: ApprovalRequired (repository.merge); steps 2, tokens 0, case ~/demo/.takt/cases/prec
```

(Lines of the run that do not show the precedence are left out.)

`endpoint_file` is read only when the harness, after precedence, is `harness`. A file that sets
`endpoint_file` and a command line that says `--harness none` runs with no endpoint and is not
refused. A relative `endpoint_file` is read beside the configuration file, not against the
current directory.

### The state directory

Everything a run keeps on the host, outside the repository, goes under one directory, the state
directory. It is the first of these that is set:

1. `--state-dir`;
2. `state_dir` in the run configuration file, or in the `takt` member of a `loom.agent/1`
   document;
3. `runtime.state_dir` of a `loom.agent/1` document, which is Loom's own key for it;
4. `$XDG_STATE_HOME/takt`, or `~/.local/state/takt` without `XDG_STATE_HOME`.

A relative path from the file (either key) is read beside the file; one from `--state-dir` is
read against the working directory. With one of the first three set, the run's session
transcripts are under `<state_dir>/transcripts` and what a discarded step set aside is under
`<state_dir>/set-aside`, so nothing a run keeps on the host is written under
`$XDG_STATE_HOME/takt`. For a container, name a directory on a kept volume. A state directory
whose `transcripts` or `set-aside` cannot be used (inside the repository, a file on its way, not
writable) is refused before anything opens, naming it and why, exit 1. A replay, a promotion and
the judges are not under it: they stay in `$XDG_STATE_HOME/takt`.

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
- A base URL other than the default needs a credential, unless its host is a loopback host: there
  a relay beside the run may hold the model's key, and the endpoint takes none (`kind: none` in
  the case record), as does an `llm` route's anonymous endpoint there, and Loom's session
  executor starts that session (since 0.25.0, on Loom 0.52.0). The claude.ai token goes to the
  default base URL only.
- A token file that is missing, or whose pointer resolves to no token, is refused naming the path
  and the pointer.
- An `--endpoint-*` flag with a harness other than `harness` is refused.

```console
$ takt run --repo ~/demo --intent x --harness harness --endpoint-file nocred.toml
takt: the endpoint's base URL https://llm.example.com/v1 is not the default https://api.anthropic.com/v1, and no credential is given: the default credential, the claude.ai subscription token in ~/.claude/.credentials.json, is not sent to another host, and only an endpoint on a loopback host takes none; give --endpoint-token-file, --endpoint-secret or --endpoint-token-env, or the endpoint file's `[credential]`
```

### A key from an environment variable

`--endpoint-token-env <NAME>` (the file's `env = "<NAME>"`) takes the API key from the variable
`NAME`. It works on the `openai-responses` wire only, until the session executor can pass an
API-key file. takt:

1. takes the variable out of its own environment before it starts anything, so no process it
   starts sees it;
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

## The llm section

The run file's `llm` member, or `--llm-config <FILE>`, holds an `llm.routes/1` document, inline or
as `{path: <file>}`. llm validates it, and a refused document opens no case: takt relays llm's
refusal.

- A `path` is read beside the run file. It is refused when it is absolute, when it has a `..` part,
  or when it resolves through a symbolic link to a place outside the run file's directory.
  `--llm-config` names its file from the working directory and has none of these limits.
- The file must be a regular file of at most 2 MiB (2097152 bytes). A larger file, or one that is
  not a regular file, is refused.
- The name ends in `.toml`, `.yaml` or `.yml`: `.toml` is read by llm's TOML parser, the other two
  by its YAML one. Any other name is refused.
- Under `--harness harness`, takt runs a route's `responses` protocol only. A route on the
  `messages` protocol is refused before any case opens, as is any other protocol: a `messages`
  route maps to the `anthropic-messages` wire, which takes no staged key, so in this release only
  the `responses` protocol takes a route's key.
- Under `--harness harness`, takt resolves the key of the route `--model` names with llm before it
  starts any process, and takes out of its own environment every variable whose value is exactly
  that key, and every variable an `env:` reference anywhere in the document names, in any endpoint.
  So neither the test command nor b10x-harness inherits one. A copy of the key with other bytes
  around it, such as a trailing newline or a prefix, matches no variable and is not removed. Under
  `--confinement substrate` outside a delegated run, the removal waits until the substrate has
  opened in the run.
- Under `--harness none` the document is validated and its digest is recorded, but no route is
  resolved and no variable is removed. `--harness claude` and `--harness codex` refuse an `llm`
  section.

## Commit identity

takt commits as the identity the repository names in `.engineering/commit-identity.conf`, read
at the commit the case forks from:

```ini
[identity]
name = takt-demo
email = takt-demo@example.com
```

or as `--commit-identity '<name> <email>'`, which wins over the file, never as your git
configuration. With neither, it commits as `takt <takt@localhost>`, and a run whose forge pushes
those commits, any forge but `none`, is refused before its case opens. The identity is recorded in
the case ([`commit-identity`](./case-files.md)), so a resumed case and `takt approve` commit as
the identity it was opened with.

## The forge

Without `--forge`, takt follows `origin`:

| `origin` | forge |
|---|---|
| a repository on a host, with `glab auth status --hostname <host>` exiting 0 | `gitlab` |
| anything else | `none`, with the reason in one line |

Each forge is an adapter, and the five are peers:

| forge | what it pushes and opens through | what scans what it publishes |
|---|---|---|
| `gitlab` | `git push` over HTTPS with `glab auth git-credential` as its one credential helper; the merge request opened, read and merged through `glab api`, or through the API itself under `transport: api` | nothing: it publishes only under a declared `scan: none` ([The scan](#the-scan)) |
| `command` | the commands of the run file's `forge_commands`, each an argument list run without a shell; the push stays takt's push over HTTPS to the project `origin` names | nothing: it publishes only under a declared `scan: none` ([The scan](#the-scan)) |
| `connectors` | `connectors invoke <connector>[/<instance>] <operation>` on the target the run file's `forge_instance` names, once per action, or with `forge_endpoint` one `POST /v1alpha2/invoke` to that served instance; the push stays takt's push over HTTPS to the project `origin` names | nothing: it publishes only under a declared `scan: none` ([The scan](#the-scan)) |
| `local` | a bare repository on this machine, the hub the tree's `origin` names by its path, outside the tree; the tree is cloned from it with `git clone --no-local`, and a tree that shares object files with it is refused: `git push` to it, a request record under its `takt/requests`, the run file's `local_checks` as the checks, the merge made in the hub under the run's commit identity and refused when the base moved, and no merge at all when the base already holds the head, the request then recorded merged by the commit that holds it; never a default | nothing: nothing leaves the machine |
| `none` | nothing | nothing leaves the machine |

`--forge gitlab` without `glab` or its login is refused before the case opens. A forge that
publishes, `gitlab`, `command` or `connectors`, is refused before the case opens unless the run
file declares `scan: none`; under it takt checks that the head is a commit and pushes without a
scan. `--forge none` opens no pull request: a delivery whose change reads ahead stops
`Unperformable` at `pr.opened`, exit 7, and prints `candidate: <full commit id>`: its code
commits, replayed on the base branch's tip under the configured identity and named by
`refs/takt/candidates/<case>`. With no identity configured it makes none and names the setting.

A scenario run creates its trial base `trial/<n>` on `origin` itself, with your own git
([Scenario runs](../guides/scenario-run.md)), and its pull request targets that branch: it
refuses `--forge gitlab`, `--forge command` and `--forge connectors` before anything is cut. By
default it has no forge; `--forge local` opens its request in the hub `origin` names.

### The GitLab API transport

`forge: gitlab` runs `glab api` under glab's own login. With `transport: api` it calls the GitLab
REST API over HTTPS itself, under a credential the run file names by reference:

```yaml
forge: gitlab
transport: api
credential:
  file: /etc/takt/gitlab-token
```

- **Credential.** `{env: NAME}` is a variable of takt's environment; `{file: PATH}` is a regular
  file of mode 600 owned by the user and outside the repository's tree, a relative path read beside
  the run file. A variable that is unset or empty, a file open to group or others, owned by another
  user or inside the tree, is refused before the case opens, naming the source and never its value.
- **Never written.** The token is held in memory only: not in the run file, an argument line, the
  state directory, a case record, an event or an error. Where an answer of the host is quoted, the
  token in it is replaced by `<redacted>`. The push passes it to git through a credential helper
  that reads it from its own environment, set for that one command.
- **Calls.** Merge request create, read and merge, and the head pipeline of the merge request,
  bound to the head the request reports, so a check read for another head never counts. A redirect
  is never followed. Plain HTTP is used only to a loopback host.
- **Default.** Without `transport`, or with `transport: glab`, nothing changes; `credential` is
  refused there, as are both keys under another forge.

### The command forge

`forge: command` runs the exact commands the run file names, each an argument list and never a
shell line, so a title holding `;` or `$(...)` is one argument:

```yaml
forge: command
forge_commands:
  open: [glab, mr, create, --source-branch, "{branch}", --target-branch, "{base}", --title, "{title}", --description-file, "{body_file}", --yes]
  read: [mr-state, "{number}"]
  checks: [mr-checks, "{number}", "{head}"]
  merge: [mr-merge, "{number}", "{head}"]
  id_pattern: '/merge_requests/(\d+)'
  refused_exit: [3]
```

- **Placeholders** are substituted once, inside the argument that holds them: `open` takes
  `{branch}`, `{base}`, `{title}` and `{body_file}` (a file under the case's artifacts directory holding the
  body), `read` takes `{number}`, `checks` and `merge` take `{number}` and `{head}`. `merge` receives
  the head the checks were read on. A placeholder an entry does not take, written in braces as a word
  of lowercase letters and `_`, and a placeholder as the first element, the program, are refused
  before the case opens.
- **Request number.** `open` prints the request's URL or number. `id_pattern`'s first group, in
  the last match, is the number. Without it the number comes only from the request path segment of
  a URL: the digits that make the whole segment after a `merge_requests`, `pull` or `pulls`
  segment, in the last URL that has one, else a lone number on the last line. A URL that names no
  request gives no number: a `.../merge_requests/new` page, a host with a port, a project whose
  name holds a digit. `pr.open` is then refused, naming `id_pattern`, and nothing is recorded. Any
  number is read back through `read` before `open` returns: a request `read` does not report as an
  open request for the case's branch, or cannot read, is refused naming `number`, and its number
  is not recorded.
- **Output.** `read` prints JSON with `state` (`open`, `merged` or
  `closed`), `head` (the commit, or an object with `sha` and `branch`), `branch` (when `head` is the
  commit alone) and `base`. `checks` prints JSON, a list of `{name, state}`. Output that cannot be
  read is an error naming the command and the field.
- **Exit codes.** 0 is success; a code in `refused_exit` is a refusal, any other a failure. Standard
  error of each command goes to a file under the state directory (`forge-command/`, in a directory of
  its own per run when no state directory is given).
- **Missing entries.** A protocol that opens, reads checks or merges needs the entries for those
  actions; a missing one is refused before the case opens, naming it.
- **Push and scan.** The push stays takt's push over HTTPS to the project `origin` names,
  published only under a declared `scan: none` ([The scan](#the-scan)).
- **Merge target's tip.** No entry reports it, since `read` describes one request and not a
  branch. `repository.merge` reads it through git, as the push goes: `git ls-remote` of
  `refs/heads/<base>` in the project `origin` names, over HTTPS alone, then that commit fetched into
  no ref. The tree needs an `origin` remote naming the project, and git must be able to read it
  with the credentials the push uses. A project with no such branch, an answer that is no commit
  id, or a command that fails gives no tip, and the judge guard merges nothing. With a tip, the
  `merge` entry runs once, at the head the checks were read on, and the merge is read back through
  `read`.

### The connectors forge

`forge: connectors` with `forge_instance` calls a Connectors connector's instance once per action.
`forge_instance` is the target as `connectors invoke` takes it: `gitlab` or `github`, the connector,
and after one `/` the instance; with the instance left out the connector's only instance is used.
It is never a default: no run reaches it without both keys.

```yaml
forge: connectors
forge_instance: gitlab/work
```

- **Two routes.** Without `forge_endpoint` each action is one `connectors --output json invoke
  <connector>[/<instance>] <operation> --schema <s> --revision <r> --input-json <json>`, run as an
  argument list without a shell. With `forge_endpoint`, the URL of a served instance, each action is
  one `POST /v1alpha2/invoke` there after one `GET /v1/describe`, and `connectors` is not run. No
  `Authorization` header and no credential of any kind goes with those calls: takt holds none, the
  instance holds the provider's. `forge_endpoint` is `https://`, or `http://` to a loopback host
  alone; any other value is refused at its line, exit 2.

```yaml
forge: connectors
forge_instance: github
forge_endpoint: http://127.0.0.1:8402
```

- **Operations of `gitlab`.** `merge_request.create` opens the request at the case branch's commit,
  `merge_request.get` reads it, `pipeline.get` reads the pipeline its head names and
  `merge_request.merge` merges it. The project is the one `origin` names.
- **Operations of `github`.** `pull_request.create` opens the pull request at the case branch's
  commit, `pull_request.get` reads its `state`, `merged`, `head.ref`, `head.sha` and `base.ref`,
  `check_runs.list` (every page) and `commit.status` read the checks of the head's sha,
  `pull_request.merge` merges it and `branch.get` reads the merge target's tip. `owner` and `repo`
  are the ones `origin` names.
- **Preflight.** Before the case opens every operation the connector's map needs is described:
  `connectors invoke <target> <operation> --describe` for each, or the one `GET /v1/describe`, whose
  instance must be the one `forge_instance` names after its `/`. A missing one is exit 1, naming each
  missing operation, and no case opens. The run prints the instance's connector, name and descriptor
  revision.
- **Checks.** On `gitlab` the checks are the status of the pipeline the request's head names; a
  pipeline of another head is not read and the checks stay pending, and `merge` carries that
  pipeline. On `github` they pass when every check run is `completed` with `success`, `neutral` or
  `skipped` and the combined status is `success`, or `pending` with no status; they fail on a check
  run concluded `failure`, `cancelled` or `timed_out`, or a combined status `failure` or `error`;
  else they are pending, and a head nothing has reported on is pending.
- **Merge.** The merge sends the head the checks were read on. A merge that fails, a 409 among
  them, is no merge: the pull request is read again before any further write, and one that is no
  longer open at that head is not merged at all.
- **Behind its base.** Before the merge, `repository.merge` reads whether the pull request is
  behind its base: on `github` when `mergeable_state` is `behind`, on `gitlab` when
  `diverged_commits_count` is above 0 or `detailed_merge_status` is `need_rebase`. One that is
  behind is brought up to date once per merge attempt: the base's tip the judge guard judged is
  merged into the head as takt's own merge commit (first parent the head, second that tip, the
  run's commit identity), recorded in the case's `artifacts/base-update.json` (previous head,
  base tip, merge, time), scanned and pushed as takt's riders are, and merged at that head once
  its checks pass. A ruleset that requires the branch to be up to date then accepts the merge. A
  base that conflicts with the head stops naming the conflicting files, with nothing committed,
  pushed or merged. A state the provider has not computed yet reads as not known, and the merge
  goes as it would without the read.
- **Keyed writes.** With `forge_endpoint` the open and the merge each carry an `idempotency_key`:
  `takt-` and a SHA-256 of the case's id, the operation, the instance's descriptor revision and
  the input, so the same case and write send the same key. An instance with standing grants
  replays a key sent again, so a write repeated after a lost answer, a provider's 5xx or
  `outcome_unknown` is not made twice. A write whose answer's `mutation.classification` is
  `refused` or `not_attempted` takes a new key next time; `applied`, `unknown` or none keeps it.
  That count is kept in the case's `artifacts/pull-request.refusals.json`, so a resumed run takes
  the next key too, and a moved descriptor revision gives a new key. An instance without standing
  grants refuses the key, before running anything, as `unsupported`; takt then sends that write
  and every later one with no key. `approval_required` (no standing grant covers the
  write), `idempotency_conflict` and `outcome_unknown` end the write, naming the code; after a
  merge's `outcome_unknown` the pull request is read again before any further write. The next
  merge reads it first: merged, none is sent; open and unmerged at the same head, the merge goes
  once per run under a fresh key, both keys recorded in the same file; closed or at another head,
  nothing is merged. The served route is tested against a served instance of Connectors 0.62.0
  (`connectors-catalog-provider --config`) with standing grants, running the `github` definition
  of connectors-definitions 0.6.0.
- **Push and scan.** The push stays takt's push over HTTPS to the project `origin` names, or
  over `http` to an `origin` on a loopback host, port kept and no redirect followed (`http://` to
  another host is refused before the case opens); it is published only under a declared
  `scan: none`. A git line to such a relay, here and on `takt scenario run`'s trial route, runs in
  an empty home without your global or system git configuration, so no credential helper of yours
  and no `~/.netrc` reaches it, and it is refused before anything is sent, naming the key and the
  file, when the repository's configuration or a file it includes sets a credential helper,
  `http.cookieFile`, `http.curloptResolve`, a URL-scoped `http.<url>.extraHeader`,
  `http.<url>.proxy` or `http.<url>.followRedirects` other than `false`, an `http.proxy` off
  loopback, or a rewrite that puts a userinfo on the relay's URL. `connectors invoke` refuses a write without
  `--approval-file` (`approval_required`), and none is given here, so on the CLI route the open and
  the merge fail with that refusal. Only `github` reports the merge target's tip; on `gitlab` the
  judge guard refuses a merge through this forge.

### The local forge

`forge: local` makes a bare repository on this machine the forge: the hub. Nothing leaves the
machine and nothing is scanned. It is never a default. The run file names the checks:

```yaml
forge: local
local_checks: [cargo, test, --quiet]
```

- **The hub and the tree.** The tree's `origin` names the hub by its path, outside the tree. The
  tree is a clone of it made with `git clone --no-local`, so it shares no object file with the
  hub; a plain clone of a local path hard-links them. Refused before any case opens, exit 1: a
  tree with no `origin`, an `origin` that is no local path or no directory, a hub inside the tree
  or holding it, a tree whose git directory lies in the hub (a worktree of it), and a tree that
  shares object files with it (hard links, an alternates file, a linked object directory).
- **Identity.** The merge commit in the hub is made under the run's commit identity:
  `--commit-identity`, or `identity.name` and `identity.email` in
  `.engineering/commit-identity.conf`. Without one the run is refused before any case opens.
- **Open.** `pr.open` pushes the case's branch to the hub and writes a request record,
  `<hub>/takt/requests/<n>.json`, holding the branch, the base, the title and the body. `n` is taken
  by an exclusive create, so two runs at once never share a number.
- **Checks.** `local_checks` is an argument list run without a shell, as the test command is: under
  the run's confinement, with its environment cleared of credentials and a scratch home, in a
  fresh repository under the state directory that fetched the head from the hub by its id and names
  no remote. Exit 0 passes; any other exit, or the end of the hour they may take, fails. A pass is
  kept for that head and that argument list; a fail is run again by the next read. They do not run
  while less than `--min-free-gib` is free. Under `--confinement substrate` a program other than
  `cargo` is refused before any case opens.
- **Merge.** After a grant of `repository.merge`, the merge is made in the hub with no work tree:
  `git merge-tree --write-tree` of the base's tip and the head, a merge commit, then the base moved
  from the tip it was judged at to that commit. A conflict is refused naming its paths, and a base
  another writer moved meanwhile is refused, not overwritten. A base whose tip already holds the
  head moves no ref; the request is recorded merged by the commit that holds it. A request whose
  head does not hold the base's tip is behind it, and is first brought up to date as on the
  connectors forge ([Behind its base](#the-connectors-forge)).
- **Read.** A request is merged only when its record names the head and a commit on the hub's base
  holds it, so a record written by hand merges nothing. The merge target's tip is the hub's
  `refs/heads/<base>`.

The run prints the hub on its forge line:
`forge: local (<hub>); pr.open, ci.read and repository.merge go through the hub on this machine,
the checks are the run file's local_checks, run as the test command is`. A walk-through:
[Run a story](../guides/run-a-story.md#on-this-machine-the-local-forge).

## The scan

Codex and b10x-harness sessions may read outside the copy they work in, so what they propose
would have to pass a scan; so would what a forge publishes. The scan is an adapter, one per run,
named by `scan` in the run configuration file, and takt has one:

| `scan` | what scans a proposal, the change so far a prompt shows, and what a forge publishes |
|---|---|
| not set | nothing, and nothing unscanned is accepted |
| `none` | nothing |

`none` is accepted only as a declaration in the file `--config` gives; no flag and no variable
sets it. What follows from the adapter:

- Not set, a Codex or b10x-harness run is refused before its case opens, exit 1, and nothing is
  written. The refusal names `scan: none` in `--config` as the way on. A resumed case (`--case`)
  is refused the same way.
- Not set, a run through a forge that publishes, `gitlab`, `command` or `connectors`, is refused
  before its case opens, exit 1, naming `scan: none` and `--forge none`. `local` and `none`
  publish nothing.
- Not set, a proposal of a session that may read outside its copy is refused, `not scanned, which
  is not passed`.
- Under `none`, the run says so before its first step, `scan: none (declared in <file>); what
  --harness <harness> proposes is accepted unscanned, checked only for the shape of a
  credential`.
- The case's `opened` event records the adapter as `scan`, `unset` when the file declares none.

Claude Code's file tools stay inside its copy, so a Claude Code run needs no declaration unless its
forge publishes. Every proposal, from any harness and under either adapter, is checked in process
for the shape of a credential.

A scenario run reads `scan` from its `--config` file the same way: with a harness that may read
outside its copy it is refused before anything is cut unless the file declares `scan: none`.

## Records profiles

`--records default|trial`, or `records` in the file, names which `decisions/NNNN.json` files the
case keeps. The flag wins over the file, and the file over the default, `default`. A resumed
case (`--case`) keeps the profile its first run was opened under unless `--records` says
otherwise; the file's `records` counts only for a new case.

| profile | what the case keeps |
|---|---|
| `default` | the latest decision of this run alone, and every decision an earlier run kept; the numbering goes on from it, so the next decision is the next number, not `0001` again |
| `trial` | every decision, as the case kept them before the profile existed |

A scenario run (`takt scenario run`) and an improvement case (`self.improvement/1`,
`loop.improvement/1`) keep every decision whatever is asked. The case's `opened` event records the
profile the case keeps as `records`; a line without it was written before the profile and reads as
every decision kept. `takt replay` refuses a step whose decision a `default` run dropped, naming
`--records trial`: replay a step of a run made with it. The [case files](./case-files.md) page
lists the file.

## Commit messages

A step's commit takes the message the run's style asks for. Flags win over the `commit` section of
the file, and the file over the defaults.

| style | the message |
|---|---|
| `semantic` | the default: the harness's reply carries `commit: {subject, body}`, a subject `<type>: <imperative subject>` of at most 72 characters and a body of 3 or 4 lines saying the intent |
| `repository` | the harness writes the message as the repository's instruction files require; its subject is one line of at most 72 characters |
| `action` | `<action>: <summary>`, as takt wrote it before styles |

The repository wins. When a line of an instruction file read at the case's base (`AGENTS.md` and
`CLAUDE.md` unless configured) matches `(?i)commit (message|style|convention)`, the style in effect
is `repository` whatever `style` says, so `style` chooses between `semantic` and `action` only for a
repository whose instructions say nothing. The case's `opened` event records it as `commit`: the
style in effect (`style`), the one configured (`configured`), the `trailer`, the `types` and, when
a line made the repository's style the effective one, its `rule` with the file and the line number.

The type of an action defaults to `test` for `tests.write` and to `feat` for `implementation.edit`,
which is `fix` for a story tagged `bug`; any other action is `chore`, and `types` overrides each.

takt checks the shape of the reply under every style but `action`: a `commit` with a subject and a
body, the subject on one line of at most 72 characters and, under `semantic`, opening with the
action's type, the body 3 or 4 lines with none blank and none opening with a word and a colon (the
shape of a trailer, or of one of takt's own lines). A reply that fails the check, or has no
`commit`, is not used: the message is `<type>: <story title>` (the intent's first line for a case
opened on an intent) with the summary as its body, and a `commit.fallback` event says why.

takt writes the trailer itself, after a blank line, as `<trailer>: <story id>` from the case's
story; it never reads one from the reply, and a case opened on an intent has none.

## Every takt run flag

One line each, from `takt run --help` at 0.14.0, and the console flags from `LoopArgs` in
`crates/takt-cli/src/main.rs` at 0.22.0. The generated [CLI reference](./cli.md) has the
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
| `--tokens <N>` | stop once the harness reported this many tokens; default 3000000, or the run file's `tokens` |
| `--active-seconds <S>` | stop once active time (wall time less waits on the forge) reaches this |
| `--min-free-gib <GIB>` | stop `DiskLow` under this much free disk; default 10; `0` turns the check off |

How each ends a run: [How a run ends](../concepts/how-a-run-ends.md).

### Console

| flag | meaning |
|---|---|
| `--verbose` | print the full `frontier:` line at each decision, besides `may:` and `may now:` |
| `--color <WHEN>` | `auto` (default): colour when standard output is a terminal and `NO_COLOR` is unset or empty; `always`; `never`. Colour never reaches a case's records (0.21.0) |
| `--quiet-steps` | list neither a harness step's tool calls under its `said:` line nor its commit's diff before its `effect:` line; nothing else changes (0.21.0) |

A harness step lists at most 20 tool calls, then `… <n> more`, and at most 120 lines of its
commit's diff, then `… <n> more lines (git show <sha>)`. Each listed line is escaped, and cut at
100 terminal columns with a wide character counting two; a tab prints as spaces to the next
multiple of four columns (0.21.0).

What the console prints: [The console](../guides/run-a-story.md#the-console) and
[What a step lists](../guides/run-a-story.md#what-a-step-lists).

### Harness and endpoint

| flag | meaning |
|---|---|
| `--harness <HARNESS>` | `claude`, `codex`, `harness` (b10x-harness) or `none` (validators only); default `claude` |
| `--harness-program <PATH>` | an absolute path: the program every step starts, in place of the one found on `PATH`; `opened` records its path, its sha256 and its `--version` line; refused when it is not absolute, and with `--harness none`, which starts no session |
| `--model <MODEL>` | the harness's model, when it takes one; under `--harness harness` a route of the `llm` section, when the run has one |
| `--llm-config <FILE>` | an `llm.routes/1` document (`.toml`, `.yaml` or `.yml`) that replaces the run file's `llm` section; with or without `--config` |
| `--effort <LEVEL>` | the reasoning effort, as `metaharness run --effort`; refused until takt's launch carries it |
| `--max-turns <N>` | a ceiling on the session's turns, as `metaharness run --max-turns`; refused under `--harness codex`, whose launch takes no turn ceiling |
| `--max-budget-usd <USD>` | a ceiling in US dollars on the session's spend, as `metaharness run --max-budget-usd`; refused until takt's launch carries it |
| `--strict-version` | refuse before the run when the vendor version is outside metaharness's pin; refused until takt's launch carries it |
| `--context <FILE>` | a file whose text the prompt carries, repeatable, as `metaharness run --context`; refused until takt's launch carries it |

| `--endpoint-file <PATH>` | the endpoint file |
| `--model-endpoint <URL>` | the endpoint's base URL |
| `--model-wire <WIRE>` | `anthropic-messages` or `openai-responses` |
| `--subscription-token-file <PATH>` | a token file; not with `--api-key-secret` |
| `--subscription-token-pointer <POINTER>` | a JSON pointer into the token file |
| `--api-key-secret <NS/NAME>` | a secret's name; takt reads no secret |
| `--endpoint-token-env <NAME>` | a variable holding the key; `openai-responses` only |

The five flags above that name a session setting (`--effort`, `--max-turns`, `--max-budget-usd`, `--strict-version`, `--context`) take the run file's `harness` mapping's keys under the same names, and a flag wins over the same key in the file. A session setting, from the file's mapping or from a flag, is refused with `--harness none`, which starts no session, unless its only key is `kind`. `max-turns` is a whole number above zero, in the file or on the command line.

The names above are metaharness's and llm's; the old names `--endpoint-base-url`,
`--endpoint-wire`, `--endpoint-model` (the same flag as `--model`), `--endpoint-token-file`,
`--endpoint-token-pointer` and `--endpoint-secret` keep working for one minor release. Each
prints one line on standard error naming the new name, and a flag given under both names is
refused. `--endpoint-token-env` and `--endpoint-file` keep their names.

The endpoint flags (`--model-endpoint`, `--model-wire`, `--subscription-token-file`,
`--subscription-token-pointer`, `--api-key-secret` and their old names, but not `--endpoint-model`,
which is `--model`) need `--harness harness`, and an `llm` section may not sit beside them.

### Forge and gates

| flag | meaning |
|---|---|
| `--forge <FORGE>` | `gitlab`, `command`, `connectors`, `local` or `none` ([The forge](#the-forge)) |
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
| `--state-dir <DIR>` | the state directory the run keeps its host records in: session transcripts in `<DIR>/transcripts` and what a discarded step sets aside in `<DIR>/set-aside`, in place of `$XDG_STATE_HOME/takt`; wins over `state_dir` and a `loom.agent/1` document's `runtime.state_dir` ([The state directory](#the-state-directory)); for a container, a directory on a kept volume |

### Records

| flag | meaning |
|---|---|
| `--records <default\|trial>` | the decisions the case keeps: `default` the run's latest alone, `trial` every one ([Records profiles](#records-profiles)); default `default` |

### Commit messages

| flag | meaning |
|---|---|
| `--commit-style <STYLE>` | `semantic` (default), `repository` or `action` ([Commit messages](#commit-messages)); the repository's own rule wins over it |
| `--commit-trailer <KEY>` | the trailer takt writes from the case's story, `Refs` by default; an empty key writes none |
| `--commit-types <ACTION=TYPE>` | the commit type of an action, repeatable or comma-separated |

### Confinement and identity

| flag | meaning |
|---|---|
| `--confinement <C>` | `none` (default): the test command through Substrate's `local` adapter, declared insecure, on the host as this user; `substrate`: through the first Substrate adapter of the host that serves it (`systemd` around `linux`, `linux`, `macos`), with no network, or refused by name |
| `--confinement-needs <NAME>` | a contract family or fact of Substrate's (`limits`, `network_egress`, `private_tmpdir`, …) the test command needs beyond what every run needs; an adapter that does not serve or prove it is not chosen, and with none left the run is refused before anything starts. Repeatable |
| `--confinement-cgroup-root <DIR>` | for `substrate`: a cgroup v2 directory delegated to this user, under which the `linux` adapter bounds each run without `systemd` |
| `--commit-identity <'NAME <EMAIL>'>` | the author and committer of every commit of the case |

### The file

| flag | meaning |
|---|---|
| `--config <FILE>` | the run configuration file described above |

## Where the container reads its configuration

The container image runs `takt run --repo /workspace --config /config/agent.yaml`. What is
mounted where, and what a `LoomAgent` writes there, is in
[Container image](./container-image.md).
