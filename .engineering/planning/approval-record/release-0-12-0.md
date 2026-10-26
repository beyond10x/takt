---
format: aep.planning-md/3
id: approval-record:release-0-12-0
kind: approval-record
status: approved
title: 'Release 0.12.0 from main at 30f177d: takt run --config, a container image, loom 0.41.0'
revision: 5
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
| revision | `main` at 30f177d plus 030f2c6 (loom 0.41.0, ess 0.69.2 in CI), 2a49eab (`--config`, the image), 4c167b8 (the store), d34b87f (the design names loom 0.41.0) and the release commit, on `release/0.12.0-2` | |
| loom | 0.41.0 with metaharness 0.15.0 and the harness crates 0.18.0 | loom 0.41.0's lock; takt pins neither metaharness nor the harness directly, so the metaharness 0.18.0 / harness 0.19.0 pair waits for a loom release that takes it |
| configuration | one flag, `--config`, over the flags that exist; the credential stays in the endpoint file by reference | the existing flags covered every field; a file was what a pod's resource can carry (story:run-configuration-file) |
| image | `Dockerfile` in the repository, built locally, never pushed; `b10x-harness` 0.18.0 in it | the wire the lock holds is the harness 0.18.0 one |
| ESS | `check.yml`, `darwin.yml` install ess 0.69.2 (each SHA256 from its `SHA256SUMS`); `ess-inputs.yaml` requires ess 0.69 | the newest ESS release; takt pins no ESS crate |
| gate | `task check` in the release tree with a private ess 0.69.2: 162 results, 1747 passed, 0 failed, 5 ignored, exit 0; check-citations 1132 resolved, 0 unresolved; `release-notes --check` 17 releases | then the pull request's checks, the merge into `main` and `release.yml` with `darwin.yml` on the tag |
| trial | the diff touches `crates/` and `Cargo.lock`, so a `[TRIAL takt] 0.12.0 <sha>` is due after the tag; the orchestrator, which directs this item, routes it | `AGENTS.md` § Messages |
| branch | `release/0.12.0-2`, rebuilt from `main`; `release/0.12.0` (pull request 15, closed) stays unmerged | its feature commit set the container user's HOME under the home-directory root in the `Dockerfile`, a personal-path pattern the security scan refuses in any commit of the range; the image now uses `/var/lib/takt` |

## Released

| item | value |
|---|---|
| tag | `0.12.0`, annotated, tag object bfc4cbf, on `main` at 2962756 (merge of `release/0.12.0-2` at a107529, parents 30f177d and a107529), author and committer `b10x-bot[bot]` |
| pull request | 16 from `release/0.12.0-2` (030f2c6, 2a49eab, 4c167b8, d34b87f, a107529): Build documentation, Security and privacy and the three task check jobs passed; pull request 15 closed unmerged, `release/0.12.0` kept |
| release run | 40310071759 success at its first attempt: tag provenance, three builds, `task check` (check-static, test-cli, test-library) and `darwin.yml` (test-cli, test-library) on the tagged commit, publish |
| GitHub Release | https://github.com/beyond10x/takt/releases/tag/0.12.0, not a draft: `takt-0.12.0-x86_64-unknown-linux-gnu.tar.gz`, `takt-0.12.0-aarch64-unknown-linux-gnu.tar.gz`, `takt-0.12.0-aarch64-apple-darwin.tar.gz`, `SHA256SUMS` |
| verification | verify-release exit 0 with the announcement file; the runs on 2962756 (Release, check, Documentation validation, two Shared source gates) success |
| image | built locally from a clone at the tag with `REVISION` 2962756, never pushed; `takt --version` in it prints `takt 0.12.0 (2962756f3c54effd497f71f4730f57b52b996df7)` |
| trial | `[TRIAL takt] 0.12.0 2962756` is due; the orchestrator, which directs this item, routes it |
