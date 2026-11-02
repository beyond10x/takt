---
format: aep.planning-md/3
id: approval-record:container-image-published-on-release
kind: approval-record
status: approved
title: The release publishes the runtime image to the GitHub container registry
summary: one wave, one pull request, the next minor; the tag waits for the orchestrator
relations:
- decides: story:release-publishes-the-image
revision: 5
transitions:
- {from: "draft", to: "approved", at: "2026-11-02T10:19:09Z", actor: "human:timo", revision: 2}
---
# Approval: the release publishes the runtime image

## Decision

The orchestrator, under the owner's grant (2026-11-02), approves one wave on one integration
branch: every release tag publishes takt's runtime image as `ghcr.io/beyond10x/takt:<version>`
(linux/amd64). The owner's requirement: takt must "provide Docker images which we can deploy
from the GitHub registry"; a downstream platform adds only configuration on top. The wave ends
in one pull request and the release-prepare commit for the next minor; the release tag is pushed
only on the orchestrator's word after the merge.

A neutral image name and a neutral run configuration member were asked for first, then withdrawn
the same day: an image built from its own repository carries that repository's name, and the
configuration keys stay as they are. story:run-configuration-member-has-a-neutral-name is
archived unbuilt.

## Covered

| item | what |
|---|---|
| story:release-publishes-the-image | `release.yml` calls `image.yml` after the gate; `image.yml` builds, checks the version line, refuses an existing version, pushes; a pull request touching the image builds it without a push; base images pinned by digest |
| release prepare | 0.27.0: version, lock files, CHANGELOG, pages that name the version |

## Not covered

- The package's visibility in the registry, which is a setting of the organization.
- Platforms other than linux/amd64, a `latest` tag, a chart.

## Released

| item | what |
|---|---|
| pull request | beyond10x/takt#52, merged at 97c56193 |
| tag | 0.27.0 on 97c56193, pushed through the bot route |
| release workflow | actions run 41432901115, success; the GitHub Release published 2026-11-02T11:47:45Z with 4 assets |
| image | `ghcr.io/beyond10x/takt:0.27.0` in the registry, public |
| verification | the release verification exited 0; the announcement was sent |
