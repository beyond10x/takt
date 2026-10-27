---
format: aep.planning-md/3
id: review-result:config-proxy-scope-round-1
kind: review-result
status: active
title: 'Scope critic, round 1: config-proxy drafts'
tags:
- config-proxy
relations:
- reviews: story:llm-section-is-an-llm-routes-document
- reviews: story:model-names-an-llm-route
- reviews: story:harness-section-is-metaharness-session-request
- reviews: story:run-flags-spell-metaharness-and-llm
- reviews: story:one-run-file-for-host-and-loom-agent
revision: 1
---
needs-revision
story:run-flags-spell-metaharness-and-llm — the epic promises "the same keys on the command line as `metaharness run`", but this story gives ten flags plus `--llm-config`, while story:harness-section-is-metaharness-session-request lets the section carry every non-takt-owned `RunSpec` key (for example `context`, `plugin-dir`, `toolchain`, `prices`, `retain-dir`); the body must either add those flags or record in Out of Scope why only the ten are carried, because today only takt-owned keys are excluded — .engineering/planning/story/run-flags-spell-metaharness-and-llm.md:47
story:llm-section-is-an-llm-routes-document — the Decision adds a `format: takt.run/1` member that no sentence of the epic asks for and no acceptance covers, and takt today refuses any `format` other than `loom.agent/1` (`crates/takt/src/run_config.rs:173`); the body must tie it to a promise and give it a scenario, or drop it and its use in story:one-run-file-for-host-and-loom-agent — .engineering/planning/story/llm-section-is-an-llm-routes-document.md:50

What I read: the epic, then 5 stories whole, the graph (grep for config, endpoint, llm and flags: no other open item claims these outcomes), `crates/takt/src/run_config.rs`, and the metaharness 0.19.0 `RunSpec` flag list. Commands: `aep plan artifact show` on the epic and each story, `aep plan artifact graph`, `git show 0.19.0:crates/metaharness-protocol/src/spec.rs`. I extracted 8 promises from the epic and traced all 8 to an item: llm section, harness section, validated by the libraries' own code, takt adds only keys it owns, same keys on the command line as metaharness, same as llm, one file for a host run and a LoomAgent, by-path form for llm only with no ambient lookup. One of the eight (the metaharness command-line keys) is claimed only in part, which is the first finding. No two items claim the same outcome, and the epic's four out-of-scope items are honoured.

What I could not establish (out of my lane, so none of it sets the verdict):
- Design: takt's existing top-level `harness` is a string (`run_config.rs:78`) and the new `harness` section is a mapping. No story says how the two coexist, or how the existing `harness` and `model` keys relate to `harness.kind` and `harness.model`.
- Design/safety: story 3's takt-owned list omits other `RunSpec` keys that could widen confinement (`driver`, `hooks`, `tool-surface`, `catalogue`, `plugin-dir`, `plugin`, `spec`, `credentials`, `cgroup-root`).
- Acceptance: story 5's item 4 needs a kind cluster.

```findings
- file: .engineering/planning/story/run-flags-spell-metaharness-and-llm.md
  line: 47
  category: scope
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "the epic promises \"the same keys on the command line as `metaharness run`\", but this story gives ten flags plus `--llm-config`, while story:harness-section-is-metaharness-session-request lets the section carry every non-takt-owned `RunSpec` key (for example `context`, `plugin-dir`, `toolchain`, `prices`, `retain-dir`); the body must either add those flags or record in Out of Scope why only the ten are carried, because today only takt-owned keys are excluded"
- file: .engineering/planning/story/llm-section-is-an-llm-routes-document.md
  line: 50
  category: scope
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "the Decision adds a `format: takt.run/1` member that no sentence of the epic asks for and no acceptance covers, and takt today refuses any `format` other than `loom.agent/1` (`crates/takt/src/run_config.rs:173`); the body must tie it to a promise and give it a scenario, or drop it and its use in story:one-run-file-for-host-and-loom-agent"
```
