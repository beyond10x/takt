---
format: aep.planning-md/3
id: story:release-publishes-the-image
kind: story
status: active
title: Every release tag publishes the runtime image to the GitHub container registry
summary: ghcr.io/beyond10x/work-runner:<version>, linux/amd64, built after the gate
tags:
- loom-agent
- priority:high
relations:
- serves: vision:safe-agentic-coding
- decomposes: epic:takt-as-a-loom-agent
revision: 3
transitions:
- {from: "draft", to: "proposed", at: "2026-11-02T10:19:09Z", actor: "human:timo", revision: 2}
- {from: "proposed", to: "active", at: "2026-11-02T10:19:09Z", actor: "human:timo", revision: 3}
---
# Story: release-publishes-the-image

## Outcome

Every release tag publishes takt's runtime image to the GitHub container registry as
`ghcr.io/beyond10x/work-runner:<version>`, linux/amd64, built from the tagged commit's
`Dockerfile`, so a downstream platform deploys it from the registry and adds only configuration.

## Context

- The owner (2026-11-02): takt must "provide Docker images which we can deploy from the GitHub
  registry"; a downstream platform adds only configuration on top.
- Until now no workflow built or pushed an image (`website/docs/reference/container-image.md`,
  "No registry"); a deployment built its own from `Dockerfile`.
- The image name carries no product name, so a downstream's configuration names none.

## Acceptance

1. `.github/workflows/release.yml` has a job that runs on every release tag after the gate, the
   macOS tests and the binary builds pass, and calls `.github/workflows/image.yml` with the
   tagged commit, the version and `push: true`, with `packages: write` on that job only.
2. `image.yml` builds `Dockerfile` for linux/amd64 with `REVISION` set to the commit, checks
   that `takt --version` in the image prints `takt <version> (<commit>)`, refuses a version the
   registry already holds, pushes `ghcr.io/beyond10x/work-runner:<version>` and reads it back.
3. A pull request that changes `Dockerfile`, `.dockerignore` or `image.yml` builds the image and
   checks its version line, with read-only permissions and no push.
4. Every action is pinned by commit, and the `Dockerfile`'s base images are pinned by digest.
5. `README.md`, `AGENTS.md` and the container image page name the published image and what a
   deployment mounts and sets.

## Out of Scope

- Other platforms than linux/amd64: the image downloads x86_64 tools.
- A `latest` tag, a Helm chart, the package's visibility setting in the registry.

## Scope

- `.github/workflows/release.yml`, `.github/workflows/image.yml`, `Dockerfile`, `README.md`,
  `AGENTS.md`, `website/docs/reference/container-image.md`, `website/docs/guides/install.md`.
