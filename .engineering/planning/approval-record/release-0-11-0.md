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
revision: 4
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

## Gates

The shared source gate (common / Security and privacy) failed on `main` at f7ecb80 (run 40045300805)
and on the release pull request 12 (run 40046230738): 4 gitleaks `generic-api-key` findings from two
lines, neither a secret. One is a clean-shape fixture in `crates/takt/tests/forge.rs` (db65aad), the
key `tokens:` with a 23-character value of digits and letters, passed to `credential_shape` and
expected to yield none; the fixture is not repeated here, since the scanner reads this record too. The other
is the citation needle "holds no token there" beside its sha256 digest in
`ess/domains/takt.citations.tsv` (8c171e0, re-digested at 731d111). `b10x-gates check` from the
policy's baseline 68f6b6d to 3643522, `main` before the wave, passes over 32 commits; to f7ecb80 it
reports exactly the 4; the pinned gitleaks run per commit over 3643522..f7ecb80 names only those two
lines, and over f7ecb80..0d59fc1, the release commit, none.

The orchestrator, under the owner's grant (2026-10-24), chose a baseline move over two policy
exceptions, which would grow the policy secret for the same result: the takt baseline moves
68f6b6d -> f7ecb80 in the policy repository at 290fa29, through the bot route; the orchestrator
published the policy to the CI secret at 00:59Z, and the gate on `main` at f7ecb80 passed on its
re-run (run 40045300805). The first record of this decision, 46619e4 on `release/0.11.0`, quoted
the fixture and so failed the gate itself (run 40047728395, one finding); the release moves to
`release/0.11.0-2`, the same release commit 0d59fc1 and this record without the quote, and
`release/0.11.0` with pull request 12 stays unmerged, its deletion not under the grant.

## Released

| item | value |
|---|---|
| tag | `0.11.0`, annotated, tag object ea60728, on `main` at 45e54d7 (merge of `release/0.11.0-2` at 5be8c3a), author and committer `b10x-bot[bot]` |
| pull request | 13 from `release/0.11.0-2` (0d59fc1 the release commit, 5be8c3a the gates record): Build documentation, Security and privacy and the three task check jobs passed; pull request 12 from `release/0.11.0` closed unmerged, its record commit 46619e4 having quoted the fixture the gate flags |
| release run | 40049567292 success: tag provenance, three builds, `darwin.yml` (test-cli, test-library) and `task check` on the tagged commit, publish; darwin test-library failed once at teardown (`hdiutil detach` of the test's disk image, "Resource busy", `across_file_systems`) and passed on its re-run |
| GitHub Release | https://github.com/beyond10x/takt/releases/tag/0.11.0, not a draft: `takt-0.11.0-x86_64-unknown-linux-gnu.tar.gz`, `takt-0.11.0-aarch64-unknown-linux-gnu.tar.gz`, `takt-0.11.0-aarch64-apple-darwin.tar.gz`, `SHA256SUMS` |
| verification | the orchestrator's verify-release exit 0 at 02:08Z, with the announcement file |
| trial | started by the orchestrator on the two endpoints from the tag; the dispatcher sent no `[TRIAL takt]` |
