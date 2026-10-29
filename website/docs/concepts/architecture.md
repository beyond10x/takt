---
title: What takt is built on
sidebar_position: 1
description: What takt is not, where it sits among the tools it uses, the layers inside the binary and what each decides, and what happens in one run.
lede: takt is an application on Loom's loom-sdk; Loom's runtime steps the case and its governor decides with Canon, a harness session only proposes, and takt keeps the validators, the evidence, git, the forge and the case record.
source: Cargo.toml and Cargo.lock at 0.14.0 and at 0.21.0 (loom 0.49.1); the versions table from Cargo.toml and Cargo.lock at the tags 0.13.0 and 0.20.0 and the 0.21.0 release commit, checked against 0.21.0 source; website/docs/index.md (What takt is not, Where it sits) before it moved here; CHANGELOG.md 0.21.0; crates/takt/src/lib.rs, run.rs, harness.rs, frontier.rs, validator.rs, case.rs, evidence.rs, run_config.rs; crates/takt/tests/no_own_evaluation.rs; docs/design/takt.md § 2, § 3 and § 5; CHANGELOG.md 0.12.0 to 0.14.0; ADR 0099; Loom 0.44.0 website/docs/index.md; metaharness website/docs/index.md
---

# What takt is built on

takt is an application on [Loom](https://beyond10x.github.io/loom/)
([GitHub](https://github.com/beyond10x/loom)). Loom's runtime drives each step of a [case](/docs/concepts/glossary#case), Loom's
governor decides what the case allows, and a [harness](/docs/concepts/glossary#harness) only proposes a change. What takt adds is
its own: the [validators](/docs/concepts/glossary#validator) and the [evidence](/docs/concepts/glossary#evidence) they write, git and the [forge](/docs/concepts/glossary#forge), the planning-store seam,
the case record, and the self-improvement tools (`scorecard`, `trace`, `report`, `view`,
`promote`, `replay`, `scenario`).

## What takt is not

| takt is not | who does that instead |
|---|---|
| A coding harness | Claude Code, Codex or b10x-harness make the edits. takt reaches them through [metaharness](https://beyond10x.github.io/metaharness/) ([GitHub](https://github.com/beyond10x/metaharness)). |
| A model router | [llm](https://beyond10x.github.io/llm/) ([GitHub](https://github.com/beyond10x/llm)) and the endpoint you configure. takt names the endpoint; the harness calls the model. |
| A planner | [AEP](https://beyond10x.github.io/ecosystem/aep/) ([GitHub](https://github.com/beyond10x/aep)) holds the plan. takt reads its ready [stories](/docs/concepts/glossary#story) and records results on them. |
| An orchestrator of agents | One case per run. [Loom](https://beyond10x.github.io/loom/) ([GitHub](https://github.com/beyond10x/loom)) runs agents, and runs a takt container once as a `LoomAgent`. |
| A CI system | Your forge's CI runs the checks. takt reads their result through the forge as evidence. Under `--forge local` takt runs the commands its run file names as the checks, on this machine. |

## Where it sits

```text
AEP                 the plan: ready stories
 │
takt                one case: steps, evidence, grants, stops
 ├─ Loom            Commission runtime steps it; the governor decides
 │   └─ Canon       evaluates the engineering protocol
 └─ metaharness     one hermetic session per judgement step
     └─ Claude Code | Codex | b10x-harness
                                  └─ llm → your endpoint
```

- [Canon](https://beyond10x.github.io/canon/) ([GitHub](https://github.com/beyond10x/canon))
  evaluates every case takt holds: claims, obligations, admissible actions, legitimate outcomes.
- [Engineering protocols](https://beyond10x.github.io/engineering-protocols/)
  ([GitHub](https://github.com/beyond10x/engineering-protocols)) supply the [protocols](/docs/concepts/glossary#protocol) takt runs
  as built-ins, the default `story.delivery/2` among them.
- [Loom](https://beyond10x.github.io/loom/) ([GitHub](https://github.com/beyond10x/loom)) is
  the runtime takt is built on: its Commission runtime drives each step and its governor decides.
- [metaharness](https://beyond10x.github.io/metaharness/)
  ([GitHub](https://github.com/beyond10x/metaharness)) runs each judgement step as one hermetic
  harness session.
- [Harness](https://beyond10x.github.io/harness/) ([GitHub](https://github.com/beyond10x/harness))
  is b10x-harness, one of the three agent loops takt can drive (`--harness harness`).
- [llm](https://beyond10x.github.io/llm/) ([GitHub](https://github.com/beyond10x/llm)) carries
  b10x-harness's model calls to the endpoint you name.
- [AEP](https://beyond10x.github.io/ecosystem/aep/) ([GitHub](https://github.com/beyond10x/aep))
  is the planning store takt takes stories from and records them in.
- [ESS](https://beyond10x.github.io/ess/) ([GitHub](https://github.com/beyond10x/ess)) holds
  takt's own specification, `ess/` in the repository, validated on every check.

The rest of this page goes through these layers as the binary links them.

## The layer stack

Top to bottom, as the binary is linked (versions from `Cargo.lock` at 0.21.0, with the loom 0.49.1 pin):

| layer | crate | version | sits under |
|---|---|---|---|
| takt, the binary | `b10x-takt-cli` | 0.21.0 | |
| the takt library | `b10x-takt` | 0.21.0 | the binary |
| loom-sdk | `b10x-loom-sdk` | 0.49.1 | the library |
| Commission, the runtime (`run_until_blocked`) | `b10x-loom-commission` | 0.49.1 | loom-sdk |
| the governor (`CanonGovernor`) | `b10x-loom-governor` | 0.49.1 | loom-sdk |
| the session executor | `b10x-loom-session-executor` | 0.49.1 | loom-sdk |
| metaharness | `metaharness` | 0.23.0 | the session executor |
| harness adapters: Claude Code, Codex, b10x-harness | `b10x-harness-*` | 0.22.0 | metaharness |
| llm: model routes and credentials | `b10x-llm-*` | 0.8.1 | loom-sdk |
| Canon | `b10x-canon` | 0.7.0 | the library |
| the protocols | `b10x-canon-engineering` | 0.13.0 | the library |
| the planning store | the `aep` command line (AEP) | | the library, as a program it runs |

### takt-cli: the binary

The crate `b10x-takt-cli` builds the `takt` program. It parses the command line (clap), reads a
run's configuration file, chooses the harness, the forge and the validators, and maps a run's
stop to its exit code.

- Decides: which unit a run works (`--intent`, `--case`, `--story`, or the next ready story),
  which validators run, what is refused before anything opens (exit 1).
- Never decides: whether a claim holds or an action is allowed.

### b10x-takt: the library

The crate `b10x-takt` (`use takt::…`) holds the loop (`run`), the case files (`case`), the
evidence records and the evidence port (`evidence`), the validators (`validator`), the harness
seam (`harness`), the planning-store seam (`frontier`), git (`git`), the forge (`forge`), and the
readers of a run record (`scorecard`, `trace`, `report`, `view`).

- Decides: what a step may touch (the step guard), which validator is due, the two pre-checks
  before each step (`ApprovalRequired` when only a [grant](/docs/concepts/glossary#grant) is left, `Unperformable` when nothing in
  the run can perform what is left), the two test stops (`RedBase`, `RepeatedTestFailure`) and
  the gate stop (`RepeatedGateFailure`),
  budgets and the free-disk check.
- Never decides: a claim, an obligation or an outcome. takt calls no Canon evaluation of its
  own; a test fails the build if it does (`no_own_evaluation.rs`). It never writes evidence from
  a model's text.

### loom-sdk: runtime, governor, session executor

`b10x-loom-sdk` 0.49.1, pinned by tag in `Cargo.toml`, is Loom's library for building on Loom.
takt uses three of its parts:

| part | crate | what it does for takt |
|---|---|---|
| Commission's runtime | `b10x-loom-commission` | Runs one step per call of `run_until_blocked`: asks the executor for a proposal, rechecks it against the [frontier](/docs/concepts/glossary#frontier), the case revision and the authority, then hands it to the effect. |
| The governor | `b10x-loom-governor` | `CanonGovernor` evaluates the case's protocol with Canon over the case's files and returns the frontier. It decides and never acts. |
| The session executor | `b10x-loom-session-executor` | Starts one metaharness session per judgement step (`loom_sdk::session`). |

- Decides (the governor): which actions are admissible, blocked or approval-required, and
  whether an outcome is legitimate.
- Never decides: who holds a grant. The governor supplies no authority; takt's `CaseAuthority`
  answers it from the case's `authority.yaml`
  ([Capabilities and approvals](./capabilities-and-approvals.md)).

loom-sdk also contains Loom's own model executor and intake. takt uses neither: its judgement
actor is a harness session. The `b10x-llm-*` 0.8.1 crates in takt's `Cargo.lock` arrive through
loom-sdk; takt makes no model call through them.

### metaharness and the harness adapters

[metaharness](https://beyond10x.github.io/metaharness/)
([GitHub](https://github.com/beyond10x/metaharness)) 0.23.0 is one interface to many agent
harnesses: it drives each from outside and keeps the run observable and hermetic. takt names
the harness with `--harness`:

| `--harness` | harness | runs on |
|---|---|---|
| `claude` (default) | Claude Code | the user's claude.ai login |
| `codex` | Codex | the user's ChatGPT login |
| `harness` | b10x-harness 0.22.0, on the `PATH` | the endpoint you configure, by default the Anthropic Messages API on the claude.ai login |
| `none` | no harness; only validators run | nothing |

Each judgement step is one session in a scratch copy of the tree, never the tree itself. The
session proposes a change; takt's edit effect applies it to the tree under the step guard and
commits it. metaharness builds the child's environment from its own allowlist, so a variable
takt withholds never reaches the harness. Claude Code runs with `--restricted`,
`--strict-mcp-config` and hooks off, offered the file tools `Read,Edit,Write,Glob,Grep` and no
shell.

- Decides: how to make the change it was asked for.
- Never decides: which actions are on offer, whether its change is kept, or whether anything is
  true. What the harness says is a trace, never evidence.

### b10x-harness, llm and the endpoint

[b10x-harness](https://beyond10x.github.io/harness/)
([GitHub](https://github.com/beyond10x/harness)) is the beyond10x agent loop. It makes its own
model calls through [llm](https://beyond10x.github.io/llm/)
([GitHub](https://github.com/beyond10x/llm)) to the endpoint `--endpoint-*` or `--endpoint-file`
names: a base URL, a wire (`anthropic-messages` or `openai-responses`), a model, and a credential
by name (a token file, a secret's name, or an environment variable's name), never the
credential's value in the case record.

### Canon and the engineering protocols

[Canon](https://beyond10x.github.io/canon/) ([GitHub](https://github.com/beyond10x/canon))
`b10x-canon` 0.7.0 is the calculus: from a protocol, a case, its evidence records, its authority
and an instant it computes each claim (true, false, unknown), the open obligations, the
admissible actions and the legitimate outcomes.
[Engineering protocols](https://beyond10x.github.io/engineering-protocols/)
([GitHub](https://github.com/beyond10x/engineering-protocols)) `b10x-canon-engineering` 0.13.0
holds the protocols takt runs as built-ins, and the assertion library takt's gate validator runs
a repository's gates through.

### AEP: the planning store

[AEP](https://beyond10x.github.io/ecosystem/aep/) ([GitHub](https://github.com/beyond10x/aep))
keeps the plan in `.engineering/planning`. takt reads and writes it only through the `aep`
command line, never its files: the ready stories, the move of a taken story to `active`, a
grant recorded as `approval` evidence, a test result and the move to `implemented` once a case is
accepted. No model takes part in choosing the unit.

### ESS: takt's own specification

takt itself is specified in [ESS](https://beyond10x.github.io/ess/)
([GitHub](https://github.com/beyond10x/ess)): `ess/system.yaml` and `ess/domains/` in the
repository, validated by `ess specify validate --path ess` in `task check`.

## Versions by release

What each takt release pins, from `Cargo.toml` and `Cargo.lock` at its tag:

| takt | loom | metaharness | b10x-harness needed for `--harness harness` | Canon | engineering protocols |
|---|---|---|---|---|---|
| 0.13.0 | 0.43.0 | 0.19.0 | 0.20.0 | 0.6.0 | 0.10.0 |
| 0.20.0 | 0.48.0 | 0.23.0 | 0.22.0 | 0.6.0 | 0.12.2 |
| 0.21.0 | 0.49.1 | 0.23.0 | 0.22.0 | 0.7.0 | 0.13.0 |

- **0.13.0** is marked breaking in the changelog: loom moved from 0.41.0 to 0.43.0, and
  `--harness harness` needed b10x-harness 0.20.0 on the `PATH` from then on.
- **0.21.0** takes Canon 0.7.0 and engineering protocols 0.13.0 with the loom 0.49.1 pin. The
  built-in `story.delivery/2` changes with it
  ([When an edit is admitted](./protocol-and-case.md#when-an-edit-is-admitted)), and Canon 0.7.0
  orders evidence in whole seconds ([Evidence and the judge](./evidence-and-the-judge.md)).

The preflight reports an older harness as an advisory gap, and the container image ships
b10x-harness 0.22.0.

## takt as a Loom agent

A run's settings can come from one file, `takt run --config <FILE>`: TOML, YAML, or a
`loom.agent/1` document, the file Loom mounts for a `LoomAgent` at `/config/agent.yaml`. takt
reads that document's `takt` member (`iterations`, `hours`, `harness`, `model`,
`endpoint_file`, `forge`, `test_cmd`, `continue_past_red_base`) and refuses, before any case
opens, a top-level member that is neither one of the members Loom 0.43.0 declares nor `takt`, an
unknown key, a mistyped value and a YAML tag, naming the file and the line. A flag on the command
line wins over the file.

The deployment shape is a container image that runs
`takt run --repo /workspace --config /config/agent.yaml`, deployed by Loom as a `LoomAgent` of
the `Command` variant, which Loom 0.45.0 runs once as a Kubernetes Job and whose
`status.run` then holds takt's exit code. How to set it up:
[Deploy on Kubernetes](../guides/deploy-on-kubernetes.md).

## Under the hood: one run, in order

1. **Open or resume a case.** `takt run` resolves the unit: `--case`, `--story`, `--intent`, the
   open case of a story being worked, or the next ready story from the planning store. A new
   case gets its own branch, `takt/<case>`, forked from the base, and its directory
   `.takt/cases/<case>/`. The run holds the case (`run.lock`) and appends an `opened` line
   to `events.jsonl`. See [Protocol and case](./protocol-and-case.md).
2. **Evaluate.** Loom's governor evaluates the case with Canon over its files. takt writes the
   result as `decisions/NNNN.json` and prints what the run may do (`may:`). See
   [The step loop and the frontier](./one-step-at-a-time.md).
3. **Pick the step.** A validator whose action is admissible and due goes first (the tests,
   the gate, a pull request, CI, the merge). Otherwise takt checks whether only a grant is left
   (`ApprovalRequired`) or nothing in the run can perform what is left (`Unperformable`), and
   if neither, offers the harness the admissible judgement actions.
4. **One step through Commission's runtime.** One call of `run_until_blocked`: the budgets and
   the free disk are checked first, then the executor proposes (the validator's action, or one
   harness session's proposal), the governor admits it, `CaseAuthority` allows an action that
   requires a capability, and the effect performs it. A harness proposal that does more than
   the frontier admits is refused and its change discarded. See
   [Capabilities and approvals](./capabilities-and-approvals.md).
5. **Validators write evidence.** A validator turns what it observed into a `canon-evidence/1`
   record bound to the revision, written under `evidence/` through the evidence port; the
   session's record goes into `observations.json`. See
   [Evidence and the judge](./evidence-and-the-judge.md).
6. **Record.** Each step, effect, evidence record, refusal and decision is a line in
   `events.jsonl`; the step's transcript is kept outside the repository, under
   `$XDG_STATE_HOME/takt/transcripts`, or under `<state_dir>/transcripts` when a state directory
   is named.
7. **Stop or go round.** After a `tests.run`, the two test stops are checked. Then the loop
   evaluates again, until an outcome is legitimate or a stop is reached, and the run prints
   `stopped: …` and exits with that stop's code. See [How a run ends](./how-a-run-ends.md).
