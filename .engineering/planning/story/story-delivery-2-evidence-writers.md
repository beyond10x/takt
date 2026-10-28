---
format: aep.planning-md/3
id: story:story-delivery-2-evidence-writers
kind: story
status: active
title: A story case on story.delivery/2 records tests_change and paths_changed, so it reaches pr.open
summary: takt writes the two evidence kinds story.delivery/2 reads
tags:
- priority:high
relations:
- decomposes: epic:downstream-story-delivery
- serves: vision:safe-agentic-coding
- informed_by: story:story-delivery-tests-first
revision: 3
transitions:
- {from: "draft", to: "proposed", at: "2026-10-28T13:11:30Z", actor: "human:timo", revision: 2}
- {from: "proposed", to: "active", at: "2026-10-28T13:11:30Z", actor: "human:timo", revision: 3}
---
## Outcome

A story case on the default protocol `story.delivery/2` reaches `pr.open` with a harness that writes its tests first: takt writes a `tests_change` record about the implementation at the revision a `tests.write` step committed, and `change.read` writes a `paths_changed` record beside its `change_record`, `both`, `tests_only`, `implementation_only` or `none` by the paths the revision changes outside `.engineering/` since the base, a test path being one `tests.write` may change. Without these writers `tests.shown_red` and `change.complete` never hold, so `implementation.edit` is never admitted under `tests_first: required` and `pr.open` never under any setting.

## Context

A run of takt 0.17.0 on a story of beyond10x/tools (2026-10-28; records outside the repository) stopped `RepeatedTestFailure` (exit 3) after `tests.write` and two red `tests.run` results, `implementation.edit` blocked, and, with `tests_first: optional`, `NoProgress` (exit 3) after a green gate, `pr.open` blocked. engineering-protocols 0.11.0 `protocols/story-delivery/2.yaml` declares both kinds; no validator or takt record in `crates/takt/src` writes either. The protocol is right; takt is missing the producers. The delivery ran on `story.delivery/1` instead.

## Scope

- `crates/takt/src/run.rs`: the `tests_change` record after a `tests.write` commit
- `crates/takt/src/validator.rs`: `ChangeRead` writes `paths_changed`
- `crates/takt/tests/`: a story run on the default protocol with a fake harness

## Acceptance

- `story_delivery_2_writers`: with no `--protocol`, a fake harness that writes a failing test, then the implementation, then hands over, reaches `pr.open` on a fake forge; the case holds one `tests_change` at the tests revision and one `paths_changed` `both` at the handed-over revision. On takt 0.17.0 the same run stops `RepeatedTestFailure` or `NoProgress`.
- A change of tests only is `tests_only`, and `pr.open` stays blocked.
