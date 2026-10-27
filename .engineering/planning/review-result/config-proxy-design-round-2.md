---
format: aep.planning-md/3
id: review-result:config-proxy-design-round-2
kind: review-result
status: active
title: 'Design critic, round 2: config-proxy drafts'
tags:
- config-proxy
relations:
- reviews: story:llm-section-is-an-llm-routes-document
- reviews: story:model-names-an-llm-route
- reviews: story:harness-section-is-metaharness-session-request
- reviews: story:run-flags-spell-metaharness-and-llm
- reviews: story:one-run-file-for-host-and-loom-agent
- reviews: story:harness-flags-spell-metaharness-names
revision: 1
---
needs-revision
story:model-names-an-llm-route — its Decision fills the session's endpoint from a route when an `llm` section is present, but says nothing on a run that also gives an endpoint source (`--endpoint-file`, `--endpoint-*`, or the `--model-endpoint` family story:run-flags-spell-metaharness-and-llm adds). Today those flags win over the file, and acceptance 6 covers only the case with no `llm` section. Two sources claim the one endpoint and no item says which wins or whether the run is refused, so the Decision should state the rule and the acceptance should name a scenario for it — .engineering/planning/story/model-names-an-llm-route.md:53, .engineering/planning/story/model-names-an-llm-route.md:80, crates/takt-cli/src/main.rs:1698-1706

**What you read:** 6 artifacts (the epic and 5 of the 6 stories listed in your task, all 6 stories in fact) with `aep plan artifact show`, plus `story:one-config-contract-with-loom` and `story:endpoint-credential-from-environment`. I ran `aep plan artifact relations`, `graph` and `validate`; validate printed "valid", and its 3 scope warnings and 5 prose-only reviews are not about this set. I also read the round-1 design review, `crates/takt-cli/src/main.rs` (endpoint args), the `Dockerfile`, `examples/loom-agent/takt-agent.yaml`, loom 0.44.0's `CHANGELOG.md`, `Cargo.lock` and `agent.rs`, and llm 0.16.0's `routes.rs` signatures.

**Graph:** I walked 18 declared edges in the set: 6 decomposes, 6 serves and 11 depends_on counted from the graph output, which makes 23 rather than 18. The graph is a single order, llm-section, model-names, run-flags, one-run-file, harness-section, harness-flags, with no cycle. I followed the epic and vision edges outside the set. Every depends_on edge has a stated reason beside it. Most are a read of produced state, and the rest are named shared files (`harness_of`, `ess/ess-inputs.yaml`, the loom pin), so the chain is not a finding.

**Acceptance reads:** I traced 12 acceptance reads to a producer in the set, and all 12 are recorded by an edge, directly or transitively.

**What I could not establish:**
- Out of my lane (acceptance or scope): `story:one-run-file-for-host-and-loom-agent` Decision sets `spec.command.args` to `--config /config/takt.yaml`. The image `ENTRYPOINT` already ends in `--config /config/agent.yaml` (`Dockerfile:54`), and `story:one-config-contract-with-loom` acceptance 5 (implemented) fixes that entrypoint. The body names `Dockerfile` in scope but does not say what the entrypoint becomes.
- Whether passing both a route and an endpoint flag is meant to be refused or overridden. I could not tell from the bodies, so the finding asks for the rule to be stated and does not choose it.

```findings
- file: .engineering/planning/story/model-names-an-llm-route.md
  line: 53
  category: design
  severity: warning
  verdict: needs-revision
  origin: introduced
  message: "its Decision fills the session's endpoint from a route when an `llm` section is present, but says nothing on a run that also gives an endpoint source (`--endpoint-file`, `--endpoint-*`, or the `--model-endpoint` family story:run-flags-spell-metaharness-and-llm adds). Today those flags win over the file, and acceptance 6 covers only the case with no `llm` section. Two sources claim the one endpoint and no item says which wins or whether the run is refused, so the Decision should state the rule and the acceptance should name a scenario for it"
```
