---
title: Protocols
description: The protocols takt runs and the ones it holds for its own improvement, with the viewer that draws them.
---

# Protocols

The [protocol viewer](pathname:///takt/protocols/) shows every `protocol/1` document under
`protocols/`: its actions, evidence kinds, claims, obligations and outcomes as tables, and as one
graph of how they connect. It is built from the protocol files on every site build, so it never
restates a protocol.

## Built in

`takt protocols` lists the ids `--protocol` accepts without a file:

| id | from | a case is |
|---|---|---|
| `story.delivery/1` | this repository, embedded in the binary | one planning-store story |
| `software.change/1` | `b10x-canon-engineering` | one intent |
| `eval-gated-change/1`, `incident-response/1`, `ml-experiment/1`, `support-triage/1` | `b10x-canon-engineering` | see [Engineering protocols](https://beyond10x.github.io/engineering-protocols/) |

`--protocol` also takes a `protocol/1` file, read from the directory `takt` runs in.

## `story.delivery/1`

| kind | names |
|---|---|
| artifacts | `intent`, `plan`, `implementation` |
| evidence | `test_result`, `gate_result`, `pull_request`, `ci_result`, `merge_observation` |
| claims | `tests.pass`, `gate.green`, `pr.opened`, `ci.green`, `pr.merged`, each about the current implementation revision |
| obligations | `tests.required`, `gate.required`, `ci.required`, `merge.required` |
| actions | `repository.inspect`, `tests.write`, `implementation.edit`, `tests.run`, `gate.run`, `pr.open` (needs `tests.pass` and `gate.green`), `ci.read`, `repository.merge` (needs `ci.green` and `pr.opened`, requires the capability `repository.merge`) |
| outcomes | `accepted` (requires `pr.merged`), `abandoned` (requires the decision `explicitly_abandoned`) |

## For takt's own improvement

| protocol | a case is | outcomes |
|---|---|---|
| `self.improvement/1` (`protocols/self-improvement/1.yaml`) | one improvement turn on takt | accepted, declined, unreproducible, abandoned |
| `loop.improvement/1` (`protocols/loop-improvement/1.yaml`) | one turn on the improvement loop itself | accepted, declined, abandoned |

takt does not run these yet: their validators are not built, and the agents perform their
actions and record the evidence ([The loop](../self-improvement/the-loop.md)).
`crates/takt/tests/improvement_protocols.rs` drives cases on both to their outcomes.

Sources: `takt protocols`, `protocols/`, `design:recursive-self-improvement` § The loops as
protocols.
