---
format: aep.planning-md/3
id: story:url-lint-and-backfill-gaps
kind: story
status: draft
title: The repository-URL lint covers reserved ranges, non-ASCII names and ports; a complete release reruns green
relations:
- decomposes: epic:recursive-self-improvement
- informed_by: story:finding-on-any-repository
- serves: vision:safe-agentic-coding
revision: 1
---
## Outcome

The repository-URL lint in `scripts/file-finding` refuses every IPv4 range IANA reserves as not globally reachable (192.0.0.0/24, 198.18.0.0/15, 224.0.0.0/4, 240.0.0.0/4, 255.255.255.255), host names with non-ASCII characters, and a port that is not a decimal number from 1 to 65535; and a rerun of the release workflow on a release that already holds every asset ends green without uploading, while a partial release still fails naming what is there.

## Context

From the reviewer's verdict "approve" on story:finding-on-any-repository at 8a943e1 (2026-10-19), low findings, not blocking. The lint is documented as a lint on the URL's text, not a boundary (`scripts/file-finding`, `ess/domains/takt.yaml`). Accepted today: 192.0.0.1, 198.18.0.1, 224.0.0.1, 240.0.0.1, 255.255.255.255; `ⓛⓞⓒⓐⓛⓗⓞⓢⓣ.ⓜⓔ` and `ｌocalhost.com` (refused by this machine's git, but a curl with IDN support maps the first to `localhost.me`); `example.com:abc` and `example.com:443:1`. The backfill in `.github/workflows/release.yml` is all-or-nothing since that unit, so a rerun of a complete, successful release exits 1.

## Scope

- `scripts/file-finding`: the host and port rules (inferred)
- `crates/takt-cli/tests/finding_scripts.rs`: the refusal rows (inferred)
- `.github/workflows/release.yml`: the backfill's complete-release case (inferred)

## Acceptance

Each host and port above is refused by `scripts/file-finding --predicate` with exit 2; `github.com` and a public IPv4 address are still accepted; against a fake `gh` listing all four assets, the backfill exits 0 with 0 uploads, and listing a strict subset it exits 1 naming them.
