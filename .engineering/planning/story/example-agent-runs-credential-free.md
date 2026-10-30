---
format: aep.planning-md/3
id: story:example-agent-runs-credential-free
kind: story
status: draft
title: The example LoomAgent runs takt with no secret in its container
summary: spec.boundary, no secretRef or secretFiles, one merged run
relations:
- decomposes: epic:composable-agentic-work
- serves: vision:safe-agentic-coding
- depends_on: story:connectors-forge-speaks-github-at-an-endpoint
- depends_on: story:git-and-model-through-loopback-relays
revision: 1
---
# Story: example-agent-runs-credential-free

## Outcome

`examples/loom-agent` and the Deploy on Kubernetes guide run takt as a `LoomAgent` with
`spec.boundary` and no `secretRef` or `secretFiles`: the agent container holds no secret, and the
run still opens and merges its pull request through the boundary.

## Context

The example and `examples/cluster-delivery` mount the GitHub App's key, the gates policy and the
model key into the takt container today, where the harness session can read them (Known
limitations: no OS sandbox holds the harness session). With Loom's `spec.boundary` the agent
container gets three loopback URLs (`LOOM_BOUNDARY_MODEL_URL`, `LOOM_BOUNDARY_CONNECTORS_URL`,
`LOOM_BOUNDARY_GIT_URL`), Loom refuses a secret beside the boundary, and the agent's egress is
DNS and namespace peers only.

Depends on story:connectors-forge-speaks-github-at-an-endpoint and
story:git-and-model-through-loopback-relays.

## Scope

- `examples/loom-agent/takt-agent.yaml`, `examples/loom-agent/workspace.yaml` (cited)
- `examples/cluster-delivery/README.md`, `architecture.dot`, `architecture.svg` (cited)
- `website/docs/guides/deploy-on-kubernetes.md`, `website/docs/reference/container-image.md` (cited)
- `website/docs/known-limitations.md` (inferred)

## Acceptance

1. The example `LoomAgent` holds `spec.boundary` and no `secretRef` or `secretFiles`, and the
   operator admits it.
2. In the running pod, `env` and every file the agent container can read hold no value of the
   App key, the gates policy or the model key (a scan of the container's file systems and
   environment for each value's digest finds none).
3. One run of the example ends with its pull request merged by `b10x-bot[bot]`, its run record
   in `examples/cluster-delivery/`.
