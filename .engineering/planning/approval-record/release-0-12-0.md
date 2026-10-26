---
format: aep.planning-md/3
id: approval-record:release-0-12-0
kind: approval-record
status: approved
title: 'Release 0.12.0 from main at 30f177d: takt run --config, a container image, loom 0.41.0'
revision: 2
transitions:
- {from: "draft", to: "approved", at: "2026-10-26T10:20:15Z", actor: "human:timo", revision: 2}
---
# Approval: release 0.12.0

## Decision

The orchestrator, under the owner's grant (2026-10-26), item S1: "Pin b10x-loom-sdk at loom tag
0.41.0", "a container image for takt: a `Dockerfile` ... that builds takt from the source tree,
entrypoint `takt run` with its repository at `/workspace`, its run configuration ... read from a
mounted file under `/config`", "If an existing `takt run` flag set already covers the configuration,
use it; add a story only for what is missing", "Build it locally with `docker build` ... do not push
to any registry", and "Release per AGENTS.md".

## What the dispatcher decided under it

| item | decision | why |
|---|---|---|
| version | 0.12.0, `Breaking: none.` | `### Added` holds `takt run --config` and the `Dockerfile`, a feature; loom 0.41.0 marks nothing breaking and no existing flag or output changed |
| revision | `main` at 30f177d plus 6782a48 (loom 0.41.0, ess 0.69.2 in CI), 4435f7a (`--config`, the image), 6ed533f (the store), 880828f (the design names loom 0.41.0) and the release commit | |
| loom | 0.41.0 with metaharness 0.15.0 and the harness crates 0.18.0 | loom 0.41.0's lock; takt pins neither metaharness nor the harness directly, so the metaharness 0.18.0 / harness 0.19.0 pair waits for a loom release that takes it |
| configuration | one flag, `--config`, over the flags that exist; the credential stays in the endpoint file by reference | the existing flags covered every field; a file was what a pod's resource can carry (story:run-configuration-file) |
| image | `Dockerfile` in the repository, built locally, never pushed; `b10x-harness` 0.18.0 in it | the wire the lock holds is the harness 0.18.0 one |
| ESS | `check.yml`, `darwin.yml` install ess 0.69.2 (each SHA256 from its `SHA256SUMS`); `ess-inputs.yaml` requires ess 0.69 | the newest ESS release; takt pins no ESS crate |
| gate | `task check` in the release tree with a private ess 0.69.2: 162 results, 1747 passed, 0 failed, 5 ignored, exit 0; check-citations 1132 resolved, 0 unresolved; `release-notes --check` 17 releases | then the pull request's checks, the merge into `main` and `release.yml` with `darwin.yml` on the tag |
| trial | the diff touches `crates/` and `Cargo.lock`, so a `[TRIAL takt] 0.12.0 <sha>` is due after the tag; the orchestrator, which directs this item, routes it | `AGENTS.md` § Messages |
