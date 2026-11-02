---
format: aep.planning-md/3
id: approval-record:container-image-published-on-release
kind: approval-record
status: approved
title: The release publishes the runtime image, and the run configuration member gets a neutral name
summary: one wave, one pull request, the next minor; the tag waits for the orchestrator
relations:
- decides: story:release-publishes-the-image
- decides: story:run-configuration-member-has-a-neutral-name
revision: 2
transitions:
- {from: "draft", to: "approved", at: "2026-11-02T10:19:09Z", actor: "human:timo", revision: 2}
---
# Approval: the release publishes the runtime image

## Decision

The orchestrator, under the owner's grant (2026-11-02), approves one wave on one integration
branch: every release tag publishes takt's runtime image as `ghcr.io/beyond10x/work-runner:<version>`
(linux/amd64), and a `loom.agent/1` document may name the run configuration member `runner`. The
owner's requirement: takt must "provide Docker images which we can deploy from the GitHub
registry"; a downstream platform adds only configuration on top, and that configuration carries
no product name. The image name is fixed before the first publish. The wave ends in one pull
request and the release-prepare commit for the next minor; the release tag is pushed only on the
orchestrator's word after the merge.

## Covered

| item | what |
|---|---|
| story:release-publishes-the-image | `release.yml` calls `image.yml` after the gate; `image.yml` builds, checks the version line, refuses an existing version, pushes; a pull request touching the image builds it without a push; base images pinned by digest |
| story:run-configuration-member-has-a-neutral-name | `runner` beside `takt` in a `loom.agent/1` document, both together refused |
| release prepare | 0.27.0: version, lock files, CHANGELOG, pages that name the version |

## Not covered

- The package's visibility in the registry, which is a setting of the organization.
- Platforms other than linux/amd64, a `latest` tag, a chart.
