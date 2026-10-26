---
title: Protocol and case
description: A protocol says what must be true before work counts; a case is one unit of work held to it, kept as files in the repository.
---

# Protocol and case

## Protocol

A protocol is a Canon `protocol/1` document. It declares the artifacts a unit of work has, the
evidence kinds that can be recorded about them, the claims that evidence makes true, the
obligations those claims discharge, the actions that can be taken, and the outcomes a case can end
in. takt has no Rust model of a protocol.

`takt protocols` lists the built-in ids, and `--protocol` takes one of them or a `protocol/1`
file. [Protocols](../reference/protocols.md) describes each; the
[protocol viewer](pathname:///takt/protocols/) draws takt's three protocols.

In `story.delivery/1` every claim is about the implementation at its current revision, and a new
commit returns all of them to unknown.

## Case

A case is one unit of work held to one protocol. It opens on an intent (`--intent`) or on the
next ready story the planning store gives, and only on a clean tree. Its files live under
`.engineering/cases/<id>/` in the repository: the case, its authority, each decision Canon
returned, each evidence record a validator wrote, and `events.jsonl`, the run record
([reference](../reference/events-jsonl.md)).

A later run resumes a case from its files, and a reviewer can replay every decision from the same
inputs. Neither a judgement step nor a validator's command may write to a case's directory, so
every line in it was written by takt.

Sources: engineering-protocols 0.10.0 `protocols/story-delivery/1.yaml`, `docs/design/takt.md` § 2 and § 3, `takt run --help`.
