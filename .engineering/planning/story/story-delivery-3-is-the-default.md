---
format: aep.planning-md/3
id: story:story-delivery-3-is-the-default
kind: story
status: draft
title: A store story opens on story.delivery/2, which dead-ends after a red gate with passing tests
tags:
- finding
- priority:high
- rsi
refs:
- provider: finding
  reference: story-delivery-3-is-the-default
relations:
- serves: vision:safe-agentic-coding
revision: 1
---
# A store story opens on story.delivery/2, which dead-ends after a red gate with passing tests

## What is wrong

A story from the planning store opens on `story.delivery/2` unless the run names another
(`protocol_for_story`, `crates/takt/src/protocol_defaults.yaml`). Under its `tests_first =
required`, `implementation.edit` needs `tests.shown_red`, which needs `tests.pass` false. When the
tests pass and the repository's gate fails (format, lint, citations), the only write the
protocol offers is `tests.write`: a gate failure can be fixed only by first writing a failing
test. The harness declines or loops.

Seen in self-runs of takt on its own stories, `--harness codex`, `--forge none`, default protocol:

- run 2 (takt 0.18.0, story:a-duplicate-yaml-key-is-refused-at-its-line): the tests passed, the
  gate failed on format and citations, only `tests.write` was offered; stopped `Declined`, exit 9,
  after 9 steps.
- run 4 (story:duplicate-key-reader-limits): green by step 6, then seven loops of `tests.write`,
  `tests.run` passing, handover, gate failing, until `Iterations(40)`, exit 3, 1281575 tokens.
- run 10 (same story): the first gate failed on citations only; stopped `Declined`, exit 9 ("only
  tests.write is admissible and no test edit would resolve it").

On the same story with `story.delivery/3`, which also admits `implementation.edit` while the tests
pass and the gate or continuous integration fails, runs 9, 11 and 12 reached a green gate (each
then stopped `Unperformable` on `pr.open`, as `--forge none` has no forge).

`story.delivery/3` is not in this tree yet: the branch `loop/selfrun` (ee0c8add, 54a734fc) adds
it as takt's own built-in `protocols/story-delivery/3.yaml`, with its input defaults, and keeps
`story.delivery/2` the default.

## Check

A source check of the defaults document the binary embeds. The library test
`a_store_story_opens_on_story_delivery_2` (`crates/takt/tests/protocol_defaults.rs`) passes today
and is renamed for `/3` by the fix.

```expected_before
command: grep -qx 'protocol_for_story: story.delivery/2' crates/takt/src/protocol_defaults.yaml
exit: 0
healthy_exit: 1
source: crates/takt/src/protocol_defaults.yaml
```

## Outcome

`story.delivery/3` is a built-in, and a store story with no protocol named by the flag, the run
file or a story tag opens on it, with `tests_first: required` and `gate_on: after_handover` by
default. A case already open on `story.delivery/2` resumes on it. The specification, the README,
`AGENTS.md` and the site name `story.delivery/3` as the default.

## Acceptance

1. The predicate above exits 0.
2. `cargo test -p b10x-takt --test protocol_defaults` passes with
   `a_store_story_opens_on_story_delivery_3` in place of `..._2`; `takt protocol show` lists
   `story.delivery/3`'s inputs and defaults.
3. A store story whose tests pass at a revision where the gate fails is offered
   `implementation.edit` (a scripted-harness test).
4. A case opened on `story.delivery/2` before the change resumes on `story.delivery/2`.
5. `python3 -I scripts/check-citations --require-digests` and `task site-check` pass.

## Scope

- `crates/takt/src/protocol_defaults.yaml`: `protocol_for_story` and the `/3` inputs (cited)
- `crates/takt/src/protocol.rs`: the built-in `/3` and `default_protocol` (cited)
- `protocols/story-delivery/3.yaml`: new, from `loop/selfrun` (inferred)
- `crates/takt/tests/protocol_defaults.rs` (cited)
- `crates/takt-cli/tests/protocol_defaults_cli.rs`, `protocol_defaults_adv1.rs`, `protocol_defaults_adv2.rs` (inferred)
- `crates/takt/tests/protocols_builtin_adv1.rs`: names `story.delivery/3` as an unknown major (cited)
- `ess/domains/takt.yaml` and `ess/domains/takt.citations.tsv`: the default named at lines 4450 and 4464 (cited)
- `README.md` (cited)
- `AGENTS.md`: the `protocols/` layout row (inferred)
- `website/docs/concepts/protocol-and-case.md`, `website/docs/guides/run-a-story.md`,
  `website/docs/reference/configuration.md`, `website/docs/reference/protocol-defaults.md`,
  `website/docs/reference/protocols.md` (cited)
