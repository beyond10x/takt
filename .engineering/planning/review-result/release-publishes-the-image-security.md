---
format: aep.planning-md/3
id: review-result:release-publishes-the-image-security
kind: review-result
status: active
title: Security review of the image publishing workflow
summary: '2 warnings: 1 fixed, 1 kept; 3 notes declined or no change'
relations:
- reviews: story:release-publishes-the-image
revision: 1
---
# Review: the release publishes the image

A security review of `.github/workflows/image.yml`, the `image` job of `release.yml` and the
`Dockerfile` pins at 1d1e4554c, against story:release-publishes-the-image. Read-only; no build ran.

## Findings

| # | severity | where | finding | outcome |
|---|---|---|---|---|
| 1 | warning | `release.yml` image job | a manual backfill of an older tag publishes an image built from that tag's unpinned base images | fixed at e79add9b4: the image job runs only on a tag push |
| 2 | warning | `release.yml` image and release jobs | the two run in parallel after the gate | kept: each is recovered by re-running it, and the version check keeps a re-run from replacing an image |
| 3 | note | `image.yml` read-back | it checks that the tag exists, not its digest | declined: replacing it in between takes a concurrent writer with `packages: write` |
| 4 | note | `image.yml` absent check | a packages-API 404 also answers for a hidden package | no change: a token that cannot read the package cannot push to it |
| 5 | note | `release.yml` header | the deleted header comment described the manual trigger | no change: workflow files carry no comments; the reference page describes the trigger |

Cleared: token scope per job and trigger, input injection, credentials in the build context, the
version check, replace protection, the pins.

## Out of scope

- The registry package's visibility and its link to the repository after the first push: an
  organization setting, checked after the first release.
- A same-repository pull request may edit `image.yml` and get `packages: write`; that comes with
  write access to the repository.
