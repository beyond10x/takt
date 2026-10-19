---
title: Protocols
description: The protocols takt runs and the ones it holds for its own improvement, with the viewer that draws them.
---

# Protocols

The [protocol viewer](pathname:///takt/protocols/) shows every `protocol/1` document under
`protocols/`, `story.delivery/1` among them: its actions, evidence kinds, claims, obligations and
outcomes as tables, and as one graph of how they connect. It is built from the protocol files on
every site build.

## Built in

`takt protocols` lists the ids `--protocol` accepts without a file:

| id | from | a case is |
|---|---|---|
| `story.delivery/1` | this repository, embedded in the binary | one planning-store story |
| `software.change/1` (listed as `software-change/1`) | `b10x-canon-engineering` | one intent |
| `eval-gated-change/1`, `incident-response/1`, `ml-experiment/1`, `support-triage/1` | `b10x-canon-engineering` | see [Engineering protocols](https://beyond10x.github.io/engineering-protocols/) |

`--protocol` also takes a `protocol/1` file, read from the directory `takt` runs in.

## For takt's own improvement

| protocol | a case is | outcomes |
|---|---|---|
| `self.improvement/1` (`protocols/self-improvement/1.yaml`) | one improvement turn on takt | accepted, declined, unreproducible, abandoned |
| `loop.improvement/1` (`protocols/loop-improvement/1.yaml`) | one turn on the improvement loop itself | accepted, declined, abandoned |

`takt run --protocol protocols/loop-improvement/1.yaml --intent "<text>" --input
loop_baseline=<full commit id>` opens a `loop.improvement/1` case, and validators drive it (`loop.measure`,
`loop.definition.check`, `loop.compare`). `self.improvement/1` has no validators yet
(`story:self-improvement-protocol`): the agents perform its actions and record the evidence
([The loop](../self-improvement/the-loop.md)).

Sources: `takt protocols`, `protocols/`, `crates/takt/src/protocol.rs`,
`crates/takt-cli/src/main.rs` (`--input`), `crates/takt/src/loop_scorecard.rs`,
`crates/takt/tests/improvement_protocols.rs`, `design:recursive-self-improvement` § The loops as
protocols.
