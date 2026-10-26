---
format: aep.planning-md/3
id: epic:takt-as-a-loom-agent
kind: epic
status: draft
title: takt runs as a LoomAgent with no hand-made wrapper
summary: 'the ten takt gaps of the first kind-cluster trial: image, credential, configuration, story, step changes, citations'
tags:
- loom-agent
relations:
- serves: vision:safe-agentic-coding
revision: 1
---
# Epic: takt-as-a-loom-agent

## Outcome

takt runs as a `LoomAgent` of the `Command` variant on a local kind cluster with no hand-made
wrapper: the image builds and tests the repository it is given, runs its gates, scans its own
edits, takes its endpoint credential from the pod's environment, reads its run configuration from
the file Loom mounts, and works the story the resource names. The owner's goal of 2026-10-26:
"loom deployable agents + takt working as auto developer", later "we want to to be based on the
same foundation" as the coordinator that follows.

## Context

- The first trial (2026-10-26) ran takt 0.12.0 as a `LoomAgent` (`Command` variant) on loom 0.42.0
  in a local kind cluster. Runs 1 to 6 stopped on setup gaps; run 7 reached `Iterations (8)` with 8
  steps, tests.run pass at steps 2, 5 and 8 (117, 117, 118 passed, 0 failed) and gate.run fail at
  steps 3 and 6 on the `citations` gate (24 `read from` citations unresolved; 1132 in-repository,
  1108 resolved), and its branch became a draft pull request
  (https://github.com/beyond10x/takt/pull/17, not merged).
- The trial record lists ten takt gaps; each is a story under this epic, citing the run that
  showed it. Loom, llm, harness and metaharness gaps from the same record belong to those
  repositories.
- Loom's design for the variant (`docs/design/loom-agent-crd-deployment.md` § 9a, loom 0.42.0):
  `spec.agent` is written verbatim to `/config/agent.yaml`, `spec.secretRef` is the environment,
  `spec.secretFiles` are files, `HOME` is `/var/lib/agent`, the root is read-only.

## Stories

story:image-runs-the-toolchain-it-tests-with (OpenSSL build dependencies), story:gate-exec-finds-the-toolchain-in-the-image,
story:image-ships-b10x-gates, story:image-cargo-home-is-writable, story:image-harness-0-20-0,
story:endpoint-credential-from-environment, story:one-config-contract-with-loom,
story:run-names-its-story, story:git-excluded-files-are-not-step-changes,
story:edits-keep-ess-citations-resolved.

## Out of Scope

- The loom, llm, harness and metaharness gaps of the trial (a run-once Job, exit status in the
  resource's status, profile fields, plain environment values, several Secret files, opaque
  configuration files, host egress, emptyDir modes, CRD upgrades, the session header, a headless
  secrets store, the H9 pin): each is that repository's.
- Publishing the image to a registry.
