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

Every field is optional. Every field but `scan`, `forge_commands`, `transport` and `credential` is the `takt run` flag of the same name, with `-`
for `_`; those four have no flag.

| field | meaning |
|---|---|
| `story` | string: the story to deliver, by id; taken only when none of `--story`, `--intent` and `--case` is given; refused by `takt scenario run`, whose scenario names the story |
| `iterations` | integer: stop after this many steps |
| `hours` | number: stop after this many hours |
| `harness` | string: `claude`, `codex`, `harness` or `none`; default `claude` |
| `model` | string: the harness's model, when it takes one; with an `llm` section under `--harness harness`, the route or alias of that section it names, whose endpoint, wire and upstream model the session uses |
| `endpoint_file` | path: the endpoint file for `--harness harness`; a relative path is read beside the configuration file |
| `endpoint_token_env` | string: the variable holding the endpoint's key, as `--endpoint-token-env`; read only with `--harness harness` and when no credential flag is given |
| `forge` | string: `bot`, `gitlab`, `command` or `none` |
| `test_cmd` | string: the test command, split at white space, run without a shell; default `cargo test --quiet` |
| `state_dir` | path: the state directory, as `--state-dir`: the run's transcripts go to `<state_dir>/transcripts` and what a discarded step sets aside to `<state_dir>/set-aside` ([The state directory](#the-state-directory)); a relative path is read beside the configuration file |
| `continue_past_red_base` | boolean: go on when the case's first test run fails at the unchanged base ([`RedBase`](../concepts/how-a-run-ends.md)) |
| `records` | string: `default` (the default) or `trial`, as `--records`: the decisions the case keeps ([Records profiles](#records-profiles)); checked when the file is read |
| `commit` | mapping of `style` (`semantic`, the default, `repository` or `action`), `trailer` (default `Refs`; empty for none) and `types` (an action to its commit type), as `--commit-style`, `--commit-trailer` and `--commit-types`: how a step commit's message is written ([Commit messages](#commit-messages)); checked when the file is read |
| `scan` | string: `gates` (the default) or `none`; the adapter that scans what a harness proposes ([The scan](#the-scan)); `none` is accepted only here, never as a flag |
| `forge_commands` | a mapping read with `forge: command`: `open`, `read`, `checks` and `merge`, each the argument list that action runs, without a shell ([The forge](#the-forge)), with `id_pattern`, a regular expression whose first group is the request number in what `open` printed, and `refused_exit`, the exit codes that are a refusal; no flag |
| `transport` | `glab`, the default, or `api`: how `forge: gitlab` reaches the host ([The GitLab API transport](#the-gitlab-api-transport)); no flag |
| `credential` | `{env: NAME}` or `{file: PATH}`, where the `api` transport's token comes from, by reference only; no flag |
| `llm` | an `llm.routes/1` document, or `{path: <file>}` naming one beside the run file (`.toml` read by llm's TOML parser, `.yaml` or `.yml` by its YAML one); llm validates it, a refused section opens no case, and the `opened` line records its digest as `llm_digest`; under `--harness harness`, `model` names one of its routes, and no endpoint source (`--model-endpoint`, `--model-wire`, `--subscription-token-file`, `--subscription-token-pointer`, `--api-key-secret`, the `--endpoint-*` names, `endpoint_file`, `endpoint_token_env`) may sit beside the section, both refused before any case opens |
| `protocol` | string: the protocol a story from the planning store opens on, `<name>/<major>` as `takt protocols` lists it; the flag `--protocol` wins over it, and a story tag `protocol:<name>/<major>` wins over it for that story; an `--intent` opens on `software.change/1` whatever it says |
| `protocols` | a mapping of a protocol, by name (every revision) or `<name>/<major>`, to a mapping of its declared case inputs, each set to one of the values the protocol declares; an unknown protocol, input or value is refused at its line before any case opens. An input is taken from `--input`, else a story tag `input:<name>=<value>`, else this section, else takt's built-in default for the protocol; each value is recorded with its source in the case's `case.yaml` |

`takt scenario run --config <FILE>` reads the same file, without `story`; it checks the shape of `protocol` and `protocols` and applies neither.

Other `takt run` flags have no key: `--repo`, `--intent`, `--case`, `--input`,
`--tokens`, `--active-seconds`, `--min-free-gib`, `--gate`, `--gates`, `--project`,
`--commit-identity`, `--confinement`, `--llm-config`, `--model-endpoint`, `--model-wire`,
`--subscription-token-file`, `--subscription-token-pointer` and `--api-key-secret`, and the old
`--endpoint-*` names of the endpoint flags below. A key for one of them is refused.

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
takt reads one value of them, `runtime.state_dir`, and none of the rest. Inside `runtime` it
accepts the members Loom declares, `state_dir`, `connectors` and `confinement`, and refuses any
other. A `runtime.state_dir` that is not a path string, or is empty or blank, is refused,
without its value.

An unknown key in a TOML file, run for real:

```console
$ takt run --repo ~/demo --intent x --config unknown.toml
takt: the run configuration file unknown.toml is refused at line 3: a run configuration file holds `iterations` and `hours`, numbers, `story`, `harness`, `model`, `endpoint_file`, `endpoint_token_env`, `forge`, `test_cmd`, `state_dir`, `records` and `protocol`, strings, `continue_past_red_base`, a boolean, `scan`, `gates` or `none`, `llm`, an `llm.routes/1` document or `{path: <file>}`, `transport`, glab or api, `credential`, a mapping naming an env variable or a file, `forge_commands`, a mapping of argument lists, and `protocols`, a mapping of a protocol to its declared inputs, each optional; `records` is `default` or `trial`
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

Each forge is an adapter, and the four are peers:

| forge | what it pushes and opens through | what scans what it publishes |
|---|---|---|
| `gitlab` | `git push` over HTTPS with `glab auth git-credential` as its one credential helper; the merge request opened, read and merged through `glab api`, or through the API itself under `transport: api` | the run's scan adapter ([The scan](#the-scan)) |
| `bot` | beyond10x's route for github.com: `b10x-gates bot` pushes the case's branch under a GitHub App and `b10x-gates api` opens and merges the pull request; `gh` reads its state and checks | `b10x-gates scan-text`, always: it is the `gates` adapter's own forge |
| `command` | the commands of the run file's `forge_commands`, each an argument list run without a shell; the push stays takt's scanned push over HTTPS to the project `origin` names | the run's scan adapter ([The scan](#the-scan)) |
| `none` | nothing | nothing leaves the machine |

`--forge gitlab` without `glab` or its login is refused before the case opens. Under `scan:
none` it checks that the head is a commit and pushes without a scan. `--forge bot` takes an
`origin` on github.com only and refuses one on another host, naming it. `--forge none` opens no pull request: a delivery whose change reads ahead stops
`Unperformable` at `pr.opened`, exit 7, and prints `candidate: <full commit id>`: its code
commits, replayed on the base branch's tip under the configured identity and named by
`refs/takt/candidates/<case>`. With no identity configured it makes none and names the setting.

A scenario run creates its trial base only through the bot route: it refuses `--forge gitlab` and
`--forge command`, and on another host it runs with `--forge none`.

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
- **Output.** `open` prints the request's URL or number, read by `id_pattern` (default: the last
  integer of the last URL on standard output). `read` prints JSON with `state` (`open`, `merged` or
  `closed`), `head` (the commit, or an object with `sha` and `branch`), `branch` (when `head` is the
  commit alone) and `base`. `checks` prints JSON, a list of `{name, state}`. Output that cannot be
  read is an error naming the command and the field.
- **Exit codes.** 0 is success; a code in `refused_exit` is a refusal, any other a failure. Standard
  error of each command goes to a file under the state directory (`forge-command/`, in a directory of
  its own per run when no state directory is given).
- **Missing entries.** A protocol that opens, reads checks or merges needs the entries for those
  actions; a missing one is refused before the case opens, naming it.
- **Push and scan.** The push stays takt's scanned push over HTTPS to the project `origin` names,
  under the run's scan adapter. No entry reports the merge target's tip, so the judge guard refuses a
  merge through this forge until one does.

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
