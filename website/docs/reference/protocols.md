---
title: Protocols
sidebar_position: 8
description: The protocols takt runs and the ones it holds for its own improvement, with the viewer that draws them.
---

# Protocols

The [protocol viewer](pathname:///takt/protocols/) shows all eight protocols `takt protocols`
lists, `software.change/1` first: their actions, evidence kinds, claims, obligations and outcomes
as tables, and as one graph of how they connect. It is built on every site build from the documents of the
`b10x-canon-engineering` release takt pins.

## Built in

`takt protocols` lists the ids `--protocol` accepts without a file:

| id | from | a case is |
|---|---|---|
| `story.delivery/1` (listed as `story-delivery/1`) | `b10x-canon-engineering`, from 0.10.0 | one planning-store story |
| `self.improvement/1` (listed as `self-improvement/1`) | `b10x-canon-engineering`, from 0.10.0 | one improvement turn on takt |
| `software.change/1` (listed as `software-change/1`) | `b10x-canon-engineering` | one intent |
| `loop.improvement/1` (listed as `loop-improvement/1`) | `b10x-canon-engineering`, from 0.10.0 | one turn on the improvement loop itself |
| `eval-gated-change/1`, `incident-response/1`, `ml-experiment/1`, `support-triage/1` | `b10x-canon-engineering` | see [Engineering protocols](https://beyond10x.github.io/engineering-protocols/) |

`--protocol` takes either spelling, `story.delivery/1` or `story-delivery/1`: the registry names
a protocol by its directory, and a protocol's own id has a dot (and for `eval-gated-change`, an
underscore: `eval.gated_change`). `--protocol` also takes the path of a `protocol/1` file, which
wins over a built-in of the same name; a path is read against the directory `takt` runs in.

## For takt's own improvement

| protocol | a case is | outcomes |
|---|---|---|
| `self.improvement/1` | one improvement turn on takt | accepted, declined, unreproducible, abandoned |
| `loop.improvement/1` | one turn on the improvement loop itself | accepted, declined, abandoned |

`takt run --protocol loop.improvement/1 --intent "<text>" --input
loop_baseline=<full commit id>` opens a `loop.improvement/1` case, and validators drive it (`loop.measure`,
`loop.definition.check`, `loop.compare`).

Validators drive a `self.improvement/1` turn too (`story:self-improvement-protocol`). From 0.2.0:
`finding.file`, `scenario.reproduce`, `scenario.verify` and `scorecard.compare`, so a turn can
end `unreproducible` or `declined`. From 0.2.2 there are also `candidate.merge`,
`candidate.merge_judge_surface` and `binary.promote`, so a turn can reach `accepted`. How an
operator runs one: [The loop](../self-improvement/the-loop.md#running-a-turn).

Sources: `takt protocols`, engineering-protocols 0.10.0 `protocols/`, `crates/takt/src/protocol.rs`,
`crates/takt-cli/src/main.rs` (`--input`), `crates/takt/src/loop_scorecard.rs`,
`crates/takt/tests/improvement_protocols.rs`, `design:recursive-self-improvement` § The loops as
protocols.
