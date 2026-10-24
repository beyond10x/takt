---
format: aep.planning-md/3
id: approval-record:release-0-11-0
kind: approval-record
status: approved
title: Release 0.11.0 from main at the wave 36 merge, with the GitLab forge and the harness endpoint on loom 0.39.1
relations:
- decides: story:gitlab-forge
- decides: story:gitlab-ci-wait
- decides: story:harness-endpoint-is-configured
revision: 2
transitions:
- {from: "draft", to: "approved", at: "2026-10-24T00:31:47Z", actor: "human:timo", revision: 2}
---
# Approval: release 0.11.0

## Decision

The orchestrator, under the owner's delegation (2026-10-24): with loom 0.39.1 released and its
verify-release exit 0, move takt's pin, drop the by-name refusal, finish wave 36 with
story:harness-endpoint-is-configured and release takt through its own process, the goal path
first. approval-record:wave-36-merge merged the wave at 92afab2 into `main` at f7ecb80, which
carries story:gitlab-forge and story:gitlab-ci-wait as well: both were delivered on the wave
branch before the orchestrator's word, so the release ships them beside the endpoint rather than
holding them for a later wave (the dispatcher's decision, named in its done message). After
0.11.0 the orchestrator starts the trial agent's harness-arm run on both endpoints from the tag,
so the dispatcher sends no `[TRIAL takt]`.

## What the dispatcher decided under it

| item | decision | why |
|---|---|---|
| version | 0.11.0 | `[Unreleased]` at f7ecb80 holds Added and Changed lines and no Breaking line |
| revision | `main` at f7ecb80, plus one release commit: version 0.10.1 to 0.11.0 in `Cargo.toml`, `Cargo.lock`, `fuzz/Cargo.lock`, CHANGELOG `[Unreleased]` dated as `[0.11.0]` with a summary line, README and site naming 0.11.0, a 0.11.0 row in status, one wording on the wave 36 page the reviewer's confirmation at 92afab2 noted | |
| targets | x86_64 and aarch64 Linux, and `aarch64-apple-darwin` | `release.yml` builds all three, and `publish` needs `darwin.yml` |
| ESS | stays 0.68.0 | the newest ESS release is 0.68.0 (`gh release view -R beyond10x/ess`) |
| loom | 0.39.1 with metaharness 0.15.0, as merged at f7ecb80 | the pin moved in the wave (731d111) |
| gate | the pull request's checks on the release commit, the three `main` runs at f7ecb80, and `release.yml` with `darwin.yml` on the tag | |
