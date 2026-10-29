---
title: What takt is
slug: /
sidebar_label: Overview
sidebar_position: 1
description: takt drives a case of an engineering protocol over a repository to a computed outcome, each step checked, each grant recorded, each run ending on a named stop.
---

# What takt is

takt is one open-source binary, Apache-2.0, that drives a repository's work through engineering
protocols. You give it an intent, a story id, or nothing, in which case it takes the next ready
story from the repository's planning store. It opens a case on a protocol and lets a coding agent
such as Claude Code take one admissible step at a time. After each step a program, not the model,
checks the result and records evidence about the exact commit it checked.

Whether the work is done is computed from those records. The rules come from a protocol, a data
file that says which claims must hold, which evidence makes them hold, which actions need a
person's grant and which outcomes the case can end in. [Canon](https://beyond10x.github.io/canon/)
([GitHub](https://github.com/beyond10x/canon)) evaluates the protocol; the model never decides it.

```sh
takt run --repo ~/demo --intent "make the failing test pass" --iterations 1
```

## What takt is responsible for

| takt owns | what that means |
|---|---|
| One case, driven to a computed outcome | A run opens or resumes one case on one protocol and steps it until an outcome is legitimate or a stop is reached. |
| Evidence bound to a revision | Its validators run the tests, the gates, open the pull request, read CI and merge; each writes a `canon-evidence/1` record naming the commit it observed. |
| Grants | An action that requires a capability, such as `repository.merge`, waits for a grant `takt approve` records for that revision. |
| The case record | Every decision, evidence record and event is a file under `.takt/cases/<case>/` in the repository; a later run resumes from it. |
| Named stops | Every run ends on a named stop with its own exit code, so a script or an orchestrator can drive it. |

## What takt is not

| takt is not | who does that instead |
|---|---|
| A coding harness | Claude Code, Codex or b10x-harness make the edits. takt reaches them through [metaharness](https://beyond10x.github.io/metaharness/) ([GitHub](https://github.com/beyond10x/metaharness)). |
| A model router | [llm](https://beyond10x.github.io/llm/) ([GitHub](https://github.com/beyond10x/llm)) and the endpoint you configure. takt names the endpoint; the harness calls the model. |
| A planner | [AEP](https://beyond10x.github.io/ecosystem/aep/) ([GitHub](https://github.com/beyond10x/aep)) holds the plan. takt reads its ready stories and records results on them. |
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
  ([GitHub](https://github.com/beyond10x/engineering-protocols)) supply the protocols takt runs,
  `story.delivery/1` among them, as built-ins.
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

How these layers fit inside the binary: [What takt is built on](./concepts/architecture.md).

## What it costs

On scenario 1, a 31-line change to a toy command-line program and the only scenario with
free-run arms, takt is slower and dearer than Claude Code running free with takt's isolation and
a shell: about 1.8x the cost and 2.4x the wall time ([Runs](/docs/self-improvement/runs)). In
return it records evidence, grants, a case record and named stops. One toy scenario and small
samples: these are first results, not a benchmark.

## Where to go next

- [Getting started](./getting-started.md): install the 0.14.0 binary and run one validated step.
- [What takt is built on](./concepts/architecture.md): the layers, and what happens in one run.
- [Protocol and case](./concepts/protocol-and-case.md): the two nouns everything else rests on.
- [How a run ends](./concepts/how-a-run-ends.md): every stop, its exit code, and what the next run does.
- [Run a story](./guides/run-a-story.md) and [Configure a run](./guides/configure-a-run.md).
- [Deploy on Kubernetes](./guides/deploy-on-kubernetes.md): takt as a Loom agent.
- [Status](/docs/status): every implemented story, and what is in the open wave.
