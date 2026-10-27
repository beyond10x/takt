---
format: aep.planning-md/3
id: review-result:config-proxy-design-round-1
kind: review-result
status: active
title: 'Design critic, round 1: config-proxy drafts'
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
story:harness-section-is-metaharness-session-request — its Decision makes `harness` a mapping, but `RunConfig.harness` is already the string behind `--harness` (`harness: harness`), and the body says neither that the string form is kept nor how the two are told apart — crates/takt/src/run_config.rs:81, .engineering/planning/story/harness-section-is-metaharness-session-request.md:54
story:model-names-an-llm-route — its Decision resolves `harness.model`, a key that only story:harness-section-is-metaharness-session-request creates, while that story depends_on this one; drop `harness.model` and name the existing run-file `model` key instead — .engineering/planning/story/model-names-an-llm-route.md:51, crates/takt/src/run_config.rs:83
story:one-run-file-for-host-and-loom-agent — acceptance 4 runs a step "from the mounted files alone", so the endpoint must come from the llm route that story:model-names-an-llm-route resolves, and only a depends_on edge to story:llm-section-is-an-llm-routes-document exists; add depends_on story:model-names-an-llm-route — .engineering/planning/story/one-run-file-for-host-and-loom-agent.md:66-67, `aep plan artifact graph`
story:run-flags-spell-metaharness-and-llm — it holds two things, the llm and endpoint flags (`--model-endpoint`, `--model-wire`, `--llm-config`, `--subscription-token-*`, `--api-key-secret`), which need only story:model-names-an-llm-route, and the harness-key flags (`--effort`, `--max-turns`, `--max-budget-usd`, `--strict-version`), which wait on two unreleased upstream stories, so the edge to story:harness-section-is-metaharness-session-request holds all of it behind them; split at that seam — .engineering/planning/story/run-flags-spell-metaharness-and-llm.md:47-50, .engineering/planning/story/harness-section-is-metaharness-session-request.md:60
story:harness-section-is-metaharness-session-request — its depends_on edge to story:model-names-an-llm-route has no reason written beside it and no acceptance reads route state, which leaves a four-deep chain (llm-section, model-names, harness-section, run-flags); the trade-off is to record the shared surface (`crates/takt/src/harness.rs` and `harness_of` in `crates/takt-cli/src/main.rs`) as the reason for the order, or to drop the edge and let the two items touch the same code in parallel — `aep plan artifact graph`

What I read: 6 artifacts (the 5 stories and the epic) with `aep plan artifact show`. I also ran `aep plan artifact relations`, `graph` and `validate` (valid; the three scope warnings and five prose-only reviews it printed are not about this set), plus `crates/takt/src/run_config.rs` and the loom 0.44.0 CHANGELOG and metaharness `spec.rs`. I walked 15 edges in the set (5 decomposes, 5 serves, 5 depends_on) and followed the whole graph output, including `epic:takt-on-loom-sdk` outside the set. There is no cycle. I traced about 9 acceptance reads to a producer inside the set, and 8 of them are recorded by an edge (directly or through model-names); the one-run-file read is the exception.

What I could not establish:
- Whether `harness.model`, `model-endpoint` and `model-wire` are allowed in the `harness` section beside an `llm` section. The harness-section Decision refuses neither, and run-flags acceptance 1 assumes they are allowed. I did not check this against metaharness's code, and I left it out of the findings because it may belong to the acceptance critic.
- The shared `ess/ess-inputs.yaml`, `ess/domains/takt.yaml`, `configuration.md`, `Cargo.toml` and `Cargo.lock` across the stories, and loom 0.44.0 beside llm 0.16.0 in one build, are parallel-safety territory and I did not judge them.

```findings
- file: .engineering/planning/story/harness-section-is-metaharness-session-request.md
  line: 54
  category: design
  severity: blocker
  verdict: needs-revision
  origin: introduced
  message: "its Decision makes `harness` a mapping, but `RunConfig.harness` is already the string behind `--harness` (`harness: harness`), and the body says neither that the string form is kept nor how the two are told apart"
- file: .engineering/planning/story/model-names-an-llm-route.md
  line: 51
  category: design
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "its Decision resolves `harness.model`, a key that only story:harness-section-is-metaharness-session-request creates, while that story depends_on this one; drop `harness.model` and name the existing run-file `model` key instead"
- file: .engineering/planning/story/one-run-file-for-host-and-loom-agent.md
  line: 66
  category: design
  severity: blocker
  verdict: needs-revision
  origin: introduced
  message: "acceptance 4 runs a step from the mounted files alone, so the endpoint must come from the llm route that story:model-names-an-llm-route resolves, and only a depends_on edge to story:llm-section-is-an-llm-routes-document exists; add depends_on story:model-names-an-llm-route"
- file: .engineering/planning/story/run-flags-spell-metaharness-and-llm.md
  line: 47
  category: design
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "it holds two things, the llm and endpoint flags that need only story:model-names-an-llm-route and the harness-key flags that wait on two unreleased upstream stories, so its edge to story:harness-section-is-metaharness-session-request holds all of it behind them; split at that seam"
- file: aep plan artifact graph
  category: design
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "story:harness-section-is-metaharness-session-request depends_on story:model-names-an-llm-route with no reason written beside it and no acceptance reading route state, which leaves a four-deep chain; either record the shared harness.rs and harness_of surface as the reason for the order, or drop the edge and accept the parallel edit"
```
