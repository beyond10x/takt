---
title: Protocol and case
description: A protocol says what must be true before work counts; a case is one unit of work held to it, kept as files in the repository.
---

# Protocol and case

## Protocol

A protocol is a Canon `protocol/1` document. It declares the artifacts a unit of work has, the
evidence kinds that can be recorded about them, the claims that evidence makes true, the
obligations those claims discharge, the actions that can be taken, and the outcomes a case can end
in. A protocol is data, never code: takt has no model of a protocol written in Rust.

takt embeds one protocol of its own and reads the rest from Canon's engineering protocols:

| protocol | a case is | where |
|---|---|---|
| `story.delivery/1` | one story from the planning store: tests, gate, pull request, CI and merge, `accepted` once the pull request is merged | embedded in the binary; `protocols/story-delivery/1.yaml` |
| `software.change/1` | one intent given on the command line | `b10x-canon-engineering` |
| `self.improvement/1`, `loop.improvement/1` | one improvement turn on takt, or on the improvement loop itself | `protocols/` (not run by takt yet; see [The loop](../self-improvement/the-loop.md)) |

`takt protocols` lists the built-in ids. `--protocol` takes a built-in id or a `protocol/1` file.
The [protocol viewer](pathname:///takt/protocols/) draws every protocol under `protocols/` from the
files themselves.

In `story.delivery/1` every claim is about the implementation at its current revision:
`tests.pass`, `gate.green`, `pr.opened`, `ci.green`, `pr.merged`. A new commit returns all of them
to unknown. `pr.open` needs `tests.pass` and `gate.green`; `repository.merge` needs `ci.green` and
`pr.opened`, and it requires the capability `repository.merge`, which only a grant gives.

## Case

A case is one unit of work held to one protocol. It opens on an intent (`--intent`) or on the
next ready story the planning store gives, and only on a clean tree. Its files live under
`.engineering/cases/<id>/` in the repository: the case, its authority, each decision Canon
returned, each evidence record a validator wrote, and `events.jsonl`, the run record
([reference](../reference/events-jsonl.md)).

Because a case is files, a later run resumes it (a plain `takt run` resumes the open case of a
story being worked), and a reviewer can replay every decision from the same inputs. A case's
directory is one a judgement step and a validator's command may not write: a line in it was
written by takt.

Sources: `protocols/story-delivery/1.yaml`, `docs/design/takt.md` § 2 and § 3, `takt run --help`.
