---
format: aep.planning-md/3
id: review-result:config-proxy-parallel-round-1
kind: review-result
status: active
title: 'Parallel-safety critic, round 1: config-proxy drafts'
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
story:one-run-file-for-host-and-loom-agent — lands on `Cargo.toml` and `Cargo.lock` (the loom pin, cited in both bodies) with story:harness-section-is-metaharness-session-request, which also needs a Loom release for its partial `RunSpec`, and neither body mentions the other and no edge orders them; add an ordering edge that records the shared pin as its reason, or split the pin bump out so the two no longer share those files — .engineering/planning/story/one-run-file-for-host-and-loom-agent.md:76, .engineering/planning/story/harness-section-is-metaharness-session-request.md:88
story:one-run-file-for-host-and-loom-agent — lands on `ess/ess-inputs.yaml` (cited, both bodies) with story:model-names-an-llm-route, story:harness-section-is-metaharness-session-request and story:run-flags-spell-metaharness-and-llm, and its only edge is to story:llm-section-is-an-llm-routes-document, so a wave holds it beside all three; add an ordering edge that records the file as its reason, or split the scenario list per story — .engineering/planning/story/one-run-file-for-host-and-loom-agent.md:77, .engineering/planning/story/model-names-an-llm-route.md:81
story:one-run-file-for-host-and-loom-agent — acceptance 4 (`one-file/kind-run`) runs a step "from the mounted files alone", and the Decision mounts only `takt.yaml` and `llm.yaml`, so the endpoint can only come from the route resolution story:model-names-an-llm-route creates, and no `depends_on` edge runs from this story to it (its only edge is to story:llm-section-is-an-llm-routes-document, which validates and digests the section but fills no endpoint); add that edge, or merge the two — .engineering/planning/story/one-run-file-for-host-and-loom-agent.md:66, .engineering/planning/story/model-names-an-llm-route.md:20

What I read: 5 stories via `aep plan artifact show`, `aep plan artifact waves --kind story --status draft --format json` and `aep plan artifact list --format json`. I also read `crates/takt/src/run_config.rs` and `examples/loom-agent/` in the tree. The llm, model-names, harness-section and run-flags stories chain by edges (llm → model-names → harness-section → run-flags), so their shared files (`run_config.rs`, `harness.rs`, `main.rs`, `takt.yaml`, `configuration.md`) are ordered and not findings.

Placement and reads:
- Placement: 5 of 5 items have cited surfaces, and 2 of them also carry one inferred entry each (`crates/takt/tests`, `crates/takt-cli/tests`). 0 are unplaceable.
- Acceptance reads: 6 traced to a producer in the set. 4 are recorded by a direct `depends_on` edge, 1 by a transitive chain (run-flags acceptance 4 reads the `llm` section through model-names), and 1 has no edge (the third finding).

What I could not establish:
- story:model-names-an-llm-route's Decision names `harness.model`, a key story:harness-section-is-metaharness-session-request creates, while the edge runs the other way. No acceptance line reads it, so I raise no finding. It is out of my lane, for design or acceptance.
- story:harness-section-is-metaharness-session-request turns `RunConfig.harness` from a string (`crates/takt/src/run_config.rs`) into a mapping, which touches an existing contract. That is out of my lane, for design.
- The metaharness and Loom upstream stories that story:harness-section-is-metaharness-session-request waits on are outside the set, and I did not assess them.

```findings
- file: .engineering/planning/story/one-run-file-for-host-and-loom-agent.md
  line: 76
  category: parallel-safety
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "lands on Cargo.toml and Cargo.lock (the loom pin, cited in both bodies) with story:harness-section-is-metaharness-session-request, which also needs a Loom release for its partial RunSpec, and neither body mentions the other and no edge orders them; add an ordering edge that records the shared pin as its reason, or split the pin bump out so the two no longer share those files"
- file: .engineering/planning/story/one-run-file-for-host-and-loom-agent.md
  line: 77
  category: parallel-safety
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "lands on ess/ess-inputs.yaml (cited, both bodies) with story:model-names-an-llm-route, story:harness-section-is-metaharness-session-request and story:run-flags-spell-metaharness-and-llm, and its only edge is to story:llm-section-is-an-llm-routes-document, so a wave holds it beside all three; add an ordering edge that records the file as its reason, or split the scenario list per story"
- file: .engineering/planning/story/one-run-file-for-host-and-loom-agent.md
  line: 66
  category: parallel-safety
  severity: blocker
  verdict: needs-revision
  origin: introduced
  message: "acceptance 4 (one-file/kind-run) runs a step from the mounted files alone and the Decision mounts only takt.yaml and llm.yaml, so the endpoint can only come from the route resolution story:model-names-an-llm-route creates, and no depends_on edge runs from this story to it; add that edge, or merge the two"
```
