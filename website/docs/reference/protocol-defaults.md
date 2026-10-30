---
title: Protocol defaults
sidebar_position: 9
description: The protocol a story and an intent open on, and the value takt gives each case input nobody sets.
---

# Protocol defaults

takt ships one defaults document, `crates/takt/src/protocol_defaults.yaml`; this page is generated from it. A case takes each declared input from the first of `--input`, a story tag `input:<name>=<value>`, the run file's `protocols` section and this document. An input none of them sets refuses the case at open, naming the input and its values.

| opens | on |
|---|---|
| a story of the planning store | `story.delivery/3` |
| an `--intent` | `software.change/1` |

A `--protocol`, a story tag `protocol:<name>/<major>` or the run file's `protocol` chooses another.

| protocol | input | values | default |
|---|---|---|---|
| `software.change/1` | `risk` | `trivial`, `standard`, `elevated`, `critical` | `standard` |
| `software.change/1` | `change_kind` | `feature`, `bugfix`, `refactor` | `feature` |
| `software.change/1` | `affects_security_boundary` | `true`, `false` | `false` |
| `story.delivery/2` | `tests_first` | `required`, `optional` | `required` |
| `story.delivery/2` | `gate_on` | `after_handover`, `each_revision` | `after_handover` |
| `story.delivery/3` | `tests_first` | `required`, `optional` | `required` |
| `story.delivery/3` | `gate_on` | `after_handover`, `each_revision` | `after_handover` |
